// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see the LICENSE file at the root of this repository or
// <https://www.gnu.org/licenses/gpl-3.0.html>.

//! API exposée à JavaScript : [`convert`], [`convert_with_timeline`],
//! [`detect`], [`formats`].
//!
//! La surface TypeScript fait foi via [`TS_API`] : toutes les fonctions sont
//! marquées `skip_typescript` pour éviter les types approximatifs de
//! wasm-bindgen (`Option<String>` sortirait en `string | undefined`).

use std::collections::BTreeMap;

use wasm_bindgen::prelude::*;

use crate::formats;
use crate::options::ApiOptions;

/// Options de [`convert`], objet littéral JavaScript (voir [`TS_API`]).
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "ConvertOptions")]
    pub type ConvertOptions;
}

/// Surface TypeScript. Les quatre fonctions sont marquées
/// `skip_typescript` : c'est cette section qui fait foi, pour obtenir
/// `string | null` et `FormatInfo[]` au lieu des types approximatifs de
/// wasm-bindgen.
#[wasm_bindgen(typescript_custom_section)]
const TS_API: &'static str = r#"
/** Options de `convert`. Toutes sont facultatives. */
export interface ConvertOptions {
  /** Format d'entrée, `--from` de la CLI (`mscz`, `mscx`, `musicxml`, `xml`, `mxl`, `mei`, `ly`, `lilypond`, `abc`, `mid`, `midi`, `gabc`). Détecté par le contenu si absent. */
  from?: string;
  /** Format de sortie (`--to` de la CLI). Défaut : `"musicxml"`. */
  to?: string;
  /** Contenu de `<encoding-date>` (AAAA-MM-JJ). Défaut : constante de la bibliothèque (sortie reproductible). */
  encodingDate?: string;
  /** Contenu de `<software>` (MusicXML). Défaut : celui de la bibliothèque. */
  software?: string;
  /** Exporter les hauteurs en sons réels (MusicXML). */
  concertPitch?: boolean;
  /** Dérouler les reprises, voltas et sauts dans la sortie MIDI. Défaut : `true`. */
  expandRepeats?: boolean;
  /** Import MIDI : quantification au tick source, sans n-olets ni simplification. Option additive : absente, l'import reste celui de MuseScore 4.7.4 (double croche). */
  midiExactTicks?: boolean;
  /** Export MIDI : écrit les textes génériques, copyrights et repères (`FF 01`/`FF 02`/`FF 06`). Option additive : absente, l'export reste celui de MuseScore 4.7.4. */
  midiText?: boolean;
  /** Fichiers d'un dossier, indexés par leur chemin relatif à sa racine (ex. `"noaten.ily"`, `"sub/x.ily"`), avec leurs octets UTF-8. Résout les `\include` des sources LilyPond ; ignoré pour les autres formats. Option additive : absente, le comportement est inchangé. */
  includes?: Record<string, Uint8Array>;
  /** Vue moderne du chant grégorien : clé de sol (`8vb` si le registre est bas) et portée à 5 lignes au lieu de la clé d'ut/4 lignes. Additive, défaut `false`. */
  gabcModernClef?: boolean;
  /** Chant grégorien : porte les neumes par des glyphes SMuFL (`smufl="chant…"`) dans le MusicXML (XSD 4.0). Additive, défaut `false`. */
  gabcNeumeSmufl?: boolean;
  /** MEI : porteurs 5.1 au-delà de l'exporteur 4.7.4 (vitesses des nuances, identité des instruments, ordre des credits…). Additive, défaut `false`. */
  meiFullSupport?: boolean;
  /** MusicXML : porteurs XSD 4.0 au-delà de l'exporteur 4.7.4, sans attribut privé. Additive, défaut `false`. */
  xmlFullSupport?: boolean;
  /** MusicXML : préserve les valeurs du modèle au lieu des valeurs de mise en page déduites par MuseScore 4.7.4. Additive, défaut `false`. */
  xmlLosslessRoundTrip?: boolean;
}

/** Changement de tempo de la performance. */
export interface TimelineTempo { tick: number; timeUs: number; microsPerQuarter: number }
/** Mesure jouée ; `xmlId` est celui de la partie 0. `ordinal` est la seule clé d'ancrage à utiliser (`number` n'est pas exposé : 0-basé côté modèle, 1-basé et remis à 1 par section côté MusicXML). */
export interface TimelineMeasure { measure: number; ordinal: number; segment: number; passage: number; xmlId: string; tick: number; endTick: number; timeUs: number; endTimeUs: number; timeMs: number }
/** Attaque de note ; `xmlId` est l'identifiant MusicXML stable. */
/** Attaque de note ; `xmlId` est l'identifiant MusicXML stable. `sourceOffset` / `sourceEnd` sont des indices UTF-16 depuis le début du fichier source quand le format en a une (GABC), `null` sinon. */
export interface TimelineNote { tick: number; timeUs: number; durationTicks: number; measure: number; staff: number; voice: number; chord: number; note: number; pitch: number; passage: number; grace: boolean; tremolo: boolean; xmlId: string; sourceOffset: number | null; sourceEnd: number | null }
/** Pause de section insérée. */
export interface TimelinePause { tick: number; endTick: number; timeUs: number; endTimeUs: number; seconds: number }
/** Carte des temps du rendu MIDI. */
export interface Timeline { division: number; ticks: number; tempo: TimelineTempo[]; measures: TimelineMeasure[]; notes: TimelineNote[]; pauses: TimelinePause[] }
/** Résultat de `convertWithTimeline`. */
export interface PlaybackResult { musicxml: Uint8Array; midi: Uint8Array; timeline: Timeline }
/** Bloc que la conversion n'a pas pu transporter ; `tick` est en rondes. */
export interface DetailedLoss { layer: string; item: string; tick: number | null; track: number | null }
/** Résultat de `convertDetailed` : mêmes octets que `convert`, plus le rapport. */
export interface DetailedResult { bytes: Uint8Array; losses: DetailedLoss[] }

/** Capacités d'un format, telles que compilées dans ce paquet. */
export interface FormatInfo {
  name: string;
  extensions: string[];
  read: boolean;
  write: boolean;
}

/** Convertit un fichier de partition. Lève une `Error` (message Rust) en cas d'échec. */
export function convert(input: Uint8Array, options?: ConvertOptions): Uint8Array;
/** Produit le MusicXML identifié, le MIDI et la carte des temps du même rendu. Lève une `Error` en cas d'échec. */
export function convertWithTimeline(input: Uint8Array, options?: ConvertOptions): PlaybackResult;
/** Convertit comme `convert` et renvoie aussi les pertes structurées (API additive). */
export function convertDetailed(input: Uint8Array, options?: ConvertOptions): DetailedResult;
/** Version de la bibliothèque. */
export function version(): string;
/** Format détecté d'après le contenu, ou `null`. */
export function detect(input: Uint8Array): string | null;
/** Formats reconnus et sens pris en charge. */
export function formats(): FormatInfo[];
"#;

/// Convertit un fichier de partition d'un format vers un autre.
///
/// `input` est le contenu binaire du fichier. `from` est détecté par le
/// contenu s'il est absent ; `to` vaut `"musicxml"` par défaut. Toute erreur
/// (format inconnu ou non pris en charge, entrée illisible, échec de
/// conversion) devient une `Error` JavaScript portant le message Rust.
#[wasm_bindgen(skip_typescript)]
pub fn convert(input: &[u8], options: Option<ConvertOptions>) -> Result<Vec<u8>, JsError> {
    let options = ApiOptions::from_js(options.as_ref());

    let from = match options.from.as_deref() {
        Some(text) => parse_format(text)?,
        None => scoretk::Format::detect(input).ok_or_else(|| {
            JsError::new(
                "format d'entrée non reconnu d'après le contenu ; \
                 précisez `from` (mscz, mscx, musicxml, xml, mxl, ly, lilypond, abc, mid, midi)",
            )
        })?,
    };
    let to = match options.to.as_deref() {
        Some(text) => parse_format(text)?,
        None => scoretk::Format::MusicXml,
    };
    if !from.supports_read() {
        return Err(JsError::new(&format!(
            "format « {from} » non pris en charge en lecture"
        )));
    }
    if !to.supports_write() {
        return Err(JsError::new(&format!(
            "format « {to} » non pris en charge en écriture"
        )));
    }

    let score = read_score(input, from, &options)?;

    let write_options = build_write_options(&options);

    scoretk::write(&score, to, &write_options).map_err(js_error)
}

/// Comme [`convert`], mais renvoie aussi le rapport de pertes structuré.
///
/// Les octets sont exactement ceux de [`convert`] ; seule la forme de retour
/// change (`{ bytes, losses }`). Cette fonction est additive : `convert` et
/// `convertWithTimeline` gardent leur signature et leur comportement.
#[wasm_bindgen(js_name = "convertDetailed", skip_typescript)]
pub fn convert_detailed(input: &[u8], options: Option<ConvertOptions>) -> Result<JsValue, JsError> {
    let options = ApiOptions::from_js(options.as_ref());
    let from = match options.from.as_deref() {
        Some(text) => parse_format(text)?,
        None => scoretk::Format::detect(input).ok_or_else(|| {
            JsError::new(
                "format d'entrée non reconnu d'après le contenu ; \
                 précisez `from` (mscz, mscx, musicxml, xml, mxl, ly, lilypond, abc, mid, midi)",
            )
        })?,
    };
    let to = match options.to.as_deref() {
        Some(text) => parse_format(text)?,
        None => scoretk::Format::MusicXml,
    };
    if !from.supports_read() {
        return Err(JsError::new(&format!(
            "format « {from} » non pris en charge en lecture"
        )));
    }
    if !to.supports_write() {
        return Err(JsError::new(&format!(
            "format « {to} » non pris en charge en écriture"
        )));
    }
    let (score, _) = scoretk::read_with_report(input, from).map_err(js_error)?;
    let write_options = build_write_options(&options);
    let (bytes, report) =
        scoretk::write_with_report(&score, to, &write_options).map_err(js_error)?;

    let result = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &result,
        &JsValue::from_str("bytes"),
        &js_sys::Uint8Array::from(bytes.as_slice()).into(),
    );
    let losses = js_sys::Array::new();
    for loss in &report.losses {
        let item = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &item,
            &JsValue::from_str("layer"),
            &JsValue::from_str(loss.layer.name()),
        );
        let _ = js_sys::Reflect::set(
            &item,
            &JsValue::from_str("item"),
            &JsValue::from_str(loss.item.as_ref()),
        );
        let _ = js_sys::Reflect::set(
            &item,
            &JsValue::from_str("tick"),
            &loss
                .tick
                .map_or(JsValue::NULL, |tick| JsValue::from_f64(tick.as_f64())),
        );
        let _ = js_sys::Reflect::set(
            &item,
            &JsValue::from_str("track"),
            &loss
                .track
                .map_or(JsValue::NULL, |track| JsValue::from_f64(track.raw() as f64)),
        );
        losses.push(&item);
    }
    let _ = js_sys::Reflect::set(&result, &JsValue::from_str("losses"), &losses);
    Ok(result.into())
}

/// Produit le MusicXML identifié, le MIDI et la carte des temps du même rendu.
///
/// `from` est résolu comme dans [`convert`] (détection par le contenu si
/// absent). `to` est ignoré : la sortie contient toujours les deux formats.
/// Le MusicXML porte `id` **et** `xml:id` sur chaque mesure et chaque note ;
/// ces valeurs sont exactement les `xmlId` de `timeline`. Les noms de champs
/// JavaScript sont en `camelCase`, le type Rust reste `Timemap` (voir
/// `docs/agents/timemap.md`).
#[wasm_bindgen(js_name = "convertWithTimeline", skip_typescript)]
pub fn convert_with_timeline(
    input: &[u8],
    options: Option<ConvertOptions>,
) -> Result<JsValue, JsError> {
    let options = ApiOptions::from_js(options.as_ref());

    let from = match options.from.as_deref() {
        Some(text) => parse_format(text)?,
        None => scoretk::Format::detect(input).ok_or_else(|| {
            JsError::new(
                "format d'entrée non reconnu d'après le contenu ; \
                 précisez `from` (mscz, mscx, musicxml, xml, mxl, ly, lilypond, abc, mid, midi)",
            )
        })?,
    };
    if !from.supports_read() {
        return Err(JsError::new(&format!(
            "format « {from} » non pris en charge en lecture"
        )));
    }

    let score = read_score(input, from, &options)?;

    let write_options = build_write_options(&options);

    let playback = scoretk::write_playback(&score, &write_options).map_err(js_error)?;
    let result = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &result,
        &JsValue::from_str("musicxml"),
        &js_sys::Uint8Array::from(playback.musicxml.as_slice()).into(),
    );
    let _ = js_sys::Reflect::set(
        &result,
        &JsValue::from_str("midi"),
        &js_sys::Uint8Array::from(playback.midi.as_slice()).into(),
    );
    let _ = js_sys::Reflect::set(
        &result,
        &JsValue::from_str("timeline"),
        &crate::timemap_js::timeline_to_js(&playback.timemap),
    );
    Ok(result.into())
}

/// Devine le format d'après le contenu, ou `null` si rien ne correspond.
#[wasm_bindgen(skip_typescript)]
pub fn detect(input: &[u8]) -> JsValue {
    match scoretk::Format::detect(input) {
        Some(format) => JsValue::from_str(format.as_str()),
        None => JsValue::NULL,
    }
}

/// Décrit les huit formats reconnus et leurs capacités compilées.
#[wasm_bindgen(skip_typescript)]
pub fn formats() -> js_sys::Array {
    let array = js_sys::Array::new();
    for format in scoretk::Format::ALL {
        array.push(&formats::to_js(format));
    }
    array
}

/// Construit les options d'écriture de la façade à partir de `ConvertOptions`.
///
/// Toutes les options sont additives : absentes, `scoretk::Options::default()`
/// s'applique et les octets restent ceux de MuseScore 4.7.4. Unique point de
/// passage pour `convert`, `convertDetailed` et `convertWithTimeline`.
fn build_write_options(options: &ApiOptions) -> scoretk::Options {
    let mut write_options = scoretk::Options::default();
    if let Some(date) = options.encoding_date.as_deref() {
        write_options.encoding_date = date.to_owned();
    }
    if let Some(software) = options.software.as_deref() {
        write_options.software = software.to_owned();
    }
    if let Some(concert_pitch) = options.concert_pitch {
        write_options.concert_pitch = Some(concert_pitch);
    }
    if let Some(expand_repeats) = options.expand_repeats {
        write_options.midi.expand_repeats = expand_repeats;
    }
    if let Some(modern) = options.gabc_modern_clef {
        write_options.gabc_modern_clef = modern;
    }
    if let Some(smufl) = options.gabc_neume_smufl {
        write_options.gabc_neume_smufl = smufl;
    }
    if let Some(full) = options.mei_full_support {
        write_options.mei_full_support = full;
    }
    if let Some(full) = options.xml_full_support {
        write_options.xml_full_support = full;
    }
    if let Some(lossless) = options.xml_lossless_round_trip {
        write_options.xml_lossless_round_trip = lossless;
    }
    if let Some(midi_text) = options.midi_text {
        write_options.midi.smf_text = midi_text;
    }
    write_options
}

/// Lit la source, en résolvant ses `\include` depuis `options.includes` quand
/// l'entrée est du LilyPond. Sans la table, le chemin est exactement celui de
/// `scoretk::read` (les `\include` restent ignorés avec un avertissement).
fn read_score(
    input: &[u8],
    from: scoretk::Format,
    options: &ApiOptions,
) -> Result<scoretk::Score, JsError> {
    if let (true, Some(files)) = (from == scoretk::Format::Ly, options.includes.as_ref()) {
        let mut resolve = lilypond_resolver(files);
        return scoretk::read_lilypond_with_includes(input, &mut resolve).map_err(js_error);
    }
    if from == scoretk::Format::Midi && options.midi_exact_ticks == Some(true) {
        let midi_options = scoretk::MidiImportOptions {
            smf_exact_ticks: true,
            ..Default::default()
        };
        return scoretk::read_midi_with(input, &midi_options).map_err(js_error);
    }
    scoretk::read(input, from).map_err(js_error)
}

/// Résolveur en mémoire pour les `\include` LilyPond d'un dossier.
///
/// LilyPond cherche un fichier inclus d'abord dans le dossier du fichier qui
/// l'inclut, puis dans son chemin de recherche (`lily/includable-lexer.cc`,
/// `lily/sources.cc`). Ici le chemin de recherche est la racine du dossier
/// fournie par `includes` : on essaie donc le chemin relatif au fichier
/// incluant, puis celui relatif à la racine. Les `.` et `..` sont normalisés
/// lexicalement, sans sortir de la racine. Un fichier absent n'est pas une
/// erreur : le lecteur l'ignore avec un avertissement (comme pour
/// `ly2video-events.ily` dans le lot alde-friezen).
fn lilypond_resolver<'a>(
    files: &'a BTreeMap<String, Vec<u8>>,
) -> impl FnMut(&str, Option<&str>) -> Option<String> + 'a {
    move |requested, includer| {
        let requested = normalize_include_path(requested);
        let mut candidates = Vec::with_capacity(2);
        if let Some((directory, _)) = includer.and_then(|path| path.rsplit_once('/')) {
            candidates.push(normalize_include_path(&format!("{directory}/{requested}")));
        }
        candidates.push(requested);
        candidates
            .iter()
            .find_map(|path| files.get(path))
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .map(str::to_owned)
    }
}

/// Normalise un chemin relatif POSIX (`.` et `..` résolus, racine conservée).
fn normalize_include_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// `Format::from_str` fournit déjà le message listant les formats acceptés
/// (`Error::FormatInconnu`).
fn parse_format(text: &str) -> Result<scoretk::Format, JsError> {
    text.parse::<scoretk::Format>()
        .map_err(|error| JsError::new(&error.to_string()))
}

fn js_error(error: scoretk::Error) -> JsError {
    JsError::new(&error.to_string())
}
