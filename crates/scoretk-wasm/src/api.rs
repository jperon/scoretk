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
  /** Format d'entrée, `--from` de la CLI (`mscz`, `mscx`, `musicxml`, `xml`, `mxl`, `ly`, `lilypond`, `abc`, `mid`, `midi`). Détecté par le contenu si absent. */
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
}

/** Changement de tempo de la performance. */
export interface TimelineTempo { tick: number; timeUs: number; microsPerQuarter: number }
/** Mesure jouée ; `xmlId` est celui de la partie 0. `ordinal` est la seule clé d'ancrage à utiliser (`number` n'est pas exposé : 0-basé côté modèle, 1-basé et remis à 1 par section côté MusicXML). */
export interface TimelineMeasure { measure: number; ordinal: number; segment: number; passage: number; xmlId: string; tick: number; endTick: number; timeUs: number; endTimeUs: number; timeMs: number }
/** Attaque de note ; `xmlId` est l'identifiant MusicXML stable. */
export interface TimelineNote { tick: number; timeUs: number; durationTicks: number; measure: number; staff: number; voice: number; chord: number; note: number; pitch: number; passage: number; grace: boolean; tremolo: boolean; xmlId: string }
/** Pause de section insérée. */
export interface TimelinePause { tick: number; endTick: number; timeUs: number; endTimeUs: number; seconds: number }
/** Carte des temps du rendu MIDI. */
export interface Timeline { division: number; ticks: number; tempo: TimelineTempo[]; measures: TimelineMeasure[]; notes: TimelineNote[]; pauses: TimelinePause[] }
/** Résultat de `convertWithTimeline`. */
export interface PlaybackResult { musicxml: Uint8Array; midi: Uint8Array; timeline: Timeline }

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

    let score = scoretk::read(input, from).map_err(js_error)?;

    let mut write_options = scoretk::Options::default();
    if let Some(date) = options.encoding_date {
        write_options.encoding_date = date;
    }
    if let Some(software) = options.software {
        write_options.software = software;
    }
    if let Some(concert_pitch) = options.concert_pitch {
        write_options.concert_pitch = concert_pitch;
    }
    if let Some(expand_repeats) = options.expand_repeats {
        write_options.midi.expand_repeats = expand_repeats;
    }

    scoretk::write(&score, to, &write_options).map_err(js_error)
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

    let score = scoretk::read(input, from).map_err(js_error)?;

    let mut write_options = scoretk::Options::default();
    if let Some(date) = options.encoding_date {
        write_options.encoding_date = date;
    }
    if let Some(software) = options.software {
        write_options.software = software;
    }
    if let Some(concert_pitch) = options.concert_pitch {
        write_options.concert_pitch = concert_pitch;
    }
    if let Some(expand_repeats) = options.expand_repeats {
        write_options.midi.expand_repeats = expand_repeats;
    }

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

/// Décrit les sept formats reconnus et leurs capacités compilées.
#[wasm_bindgen(skip_typescript)]
pub fn formats() -> js_sys::Array {
    let array = js_sys::Array::new();
    for format in scoretk::Format::ALL {
        array.push(&formats::to_js(format));
    }
    array
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
