// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// La lecture couvre un sous-ensemble de la norme ABC 2.1
// (<https://abcnotation.com/wiki/abc:standard:v2.1>) et suit le comportement de
// `abc2xml.py` de Willem Vree (LGPL) ; aucune ligne de ce script n'est copiée.
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

//! Lecture ABC : préparation des lignes, champs d'en-tête et orchestration.
//!
//! Le lecteur travaille en trois temps :
//!
//! 1. [`prepare`] découpe la source en lignes, gère les continuations `\` et
//!    retire les commentaires `%` (les directives `%%` sont conservées) ;
//! 2. les champs d'en-tête (`X:`, `T:`, `M:`, `L:`, `K:`, `V:`…) remplissent
//!    l'état global et créent les voix ;
//! 3. [`music::parse_line`] remplit les mesures de la voix courante, puis
//!    [`build`] construit le modèle pivot.
//!
//! Le fichier peut contenir plusieurs morceaux `X:` ; [`read_abc`] lit le
//! premier et [`read_abc_tune`] permet de choisir.

mod build;
mod lyrics;
mod measure;
mod music;
mod rhythm;

use std::collections::BTreeMap;

use scoretk_core::model::{
    BarLineType, ClefType, Fermata, Fraction, HairpinType, Key, KeySig, NoteId, Score, TimeSig,
    TremoloKind,
};

use scoretk_abc_common::decorations::Decoration;
use scoretk_abc_common::pitch;
use scoretk_abc_common::{Error, Result, Warning};

/// Options de lecture.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadOptions {
    /// Indice du morceau à lire quand la source en contient plusieurs
    /// (base zéro).
    pub tune: usize,
}

/// Résultat d'une lecture avec avertissements.
#[derive(Clone, Debug)]
pub struct ReadOutput {
    /// Partition lue.
    pub score: Score,
    /// Constructions ignorées ou approximées.
    pub warnings: Vec<Warning>,
}

/// Lit le premier morceau d'une source ABC.
pub fn read_abc(src: &str) -> Result<Score> {
    read_abc_tune(src, 0)
}

/// Lit le morceau d'indice `tune` d'une source ABC.
pub fn read_abc_tune(src: &str, tune: usize) -> Result<Score> {
    read_abc_with(src, &ReadOptions { tune }).map(|output| output.score)
}

/// Lit un morceau ABC et renvoie aussi les avertissements.
pub fn read_abc_with(src: &str, options: &ReadOptions) -> Result<ReadOutput> {
    let lines = prepare(src);
    let tunes = split_tunes(&lines);
    let tune = tunes.get(options.tune).ok_or(Error::MorceauHorsLimite {
        index: options.tune,
        count: tunes.len(),
    })?;
    let mut reader = Reader::default();
    reader.parse_tune(tune)?;
    reader.finish()
}

/// Ligne de la source, numérotée.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Line {
    /// Numéro dans la source (base un).
    number: usize,
    /// Texte, continuation comprise.
    text: String,
}

/// Prépare les lignes : CRLF, continuations `\` et commentaires.
fn prepare(src: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut pending: Option<Line> = None;
    for (index, raw) in src
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .enumerate()
    {
        let number = index + 1;
        let mut text = strip_comment(raw).trim_end().to_owned();
        if let Some(mut previous) = pending.take() {
            // Port de la norme ABC 2.1 § 2.2.6 (ligne 1529 de l'archive
            // locale) : « any information fields and stylesheet directives
            // are processed at the point where the physical line-break
            // occurs » ; pré-processer les continuations en une seule ligne
            // est déconseillé (ligne 349). `abc2xml.py` reconnaît chaque champ
            // indépendamment de `mcont` et l'émet comme champ en ligne
            // (lignes 803-818). Un champ ou une directive après un `\` ne
            // doit donc pas être collé à la ligne de musique : on clôt celle-
            // ci et on traite le champ normalement.
            let est_champ_ou_directive = text.starts_with("%%")
                || (text.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                    && text.chars().nth(1) == Some(':'));
            if est_champ_ou_directive {
                if !previous.text.trim().is_empty() {
                    lines.push(previous);
                }
            } else {
                previous.text.push(' ');
                previous.text.push_str(text.trim_start());
                text = previous.text;
                let continued = text.ends_with('\\');
                if continued {
                    text.pop();
                }
                pending = Some(Line {
                    number: previous.number,
                    text,
                });
                if !continued {
                    if let Some(line) = pending.take() {
                        if !line.text.trim().is_empty() {
                            lines.push(line);
                        }
                    }
                }
                continue;
            }
        }
        if text.ends_with('\\') {
            text.pop();
            pending = Some(Line { number, text });
            continue;
        }
        // Port de la lecture de `abc2xml.py` (ligne 505, `abc_voice` analysé
        // sur tout le texte d'une voix, espaces inclus) : une altération en fin
        // de ligne (`... =` puis `(ed)` sur la suivante) appartient à la note
        // suivante. On joint donc ces deux lignes comme pour `\`.
        let champ = text.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && text.chars().nth(1) == Some(':');
        let alteration_fin = !champ
            && text
                .chars()
                .last()
                .is_some_and(|c| matches!(c, '^' | '_' | '='));
        if alteration_fin {
            pending = Some(Line { number, text });
            continue;
        }
        if !text.trim().is_empty() {
            lines.push(Line { number, text });
        }
    }
    if let Some(line) = pending {
        if !line.text.trim().is_empty() {
            lines.push(line);
        }
    }
    lines
}

/// Retire un commentaire `%` hors guillemets.
///
/// Les lignes `%%` sont des directives, conservées telles quelles ; un `%`
/// dans une annotation `"…"` n'est pas un commentaire.
fn strip_comment(line: &str) -> &str {
    let trimmed = line.trim_start();
    if trimmed.starts_with("%%") {
        return line;
    }
    let mut in_quote = false;
    for (index, character) in line.char_indices() {
        match character {
            '"' => in_quote = !in_quote,
            '%' if !in_quote => return &line[..index],
            _ => {}
        }
    }
    line
}

/// Découpe la source en morceaux, chacun commençant par `X:`.
fn split_tunes(lines: &[Line]) -> Vec<Vec<Line>> {
    let mut tunes: Vec<Vec<Line>> = Vec::new();
    let mut current: Vec<Line> = Vec::new();
    let mut seen_x = false;
    for line in lines {
        // `abc2xml.py::splitHeaderVoices` (ligne 789) : le premier `X:` ouvre
        // le premier air ; les champs globaux qui le précèdent (`M:`, `O:`,
        // `R:`…) en font partie. Seul un `X:` suivant sépare un nouvel air.
        if is_field(line, 'X') && seen_x && !current.is_empty() {
            tunes.push(std::mem::take(&mut current));
        }
        if is_field(line, 'X') {
            seen_x = true;
        }
        current.push(line.clone());
    }
    if !current.is_empty() {
        tunes.push(current);
    }
    tunes
}

/// Vrai si la ligne est le champ `letter:` (hors `%%`).
fn is_field(line: &Line, letter: char) -> bool {
    let trimmed = line.text.trim_start();
    let mut characters = trimmed.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if first != letter {
        return false;
    }
    characters.next() == Some(':')
}

/// Valeur d'un champ `letter: valeur`.
fn field_value(line: &Line) -> Option<(char, &str)> {
    let trimmed = line.text.trim_start();
    let mut characters = trimmed.chars();
    let letter = characters.next()?;
    if !letter.is_ascii_alphabetic() || characters.next() != Some(':') {
        return None;
    }
    let offset = letter.len_utf8() + 1;
    Some((letter, trimmed[offset..].trim()))
}

/// Définition d'une voix `V:`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VoiceFields {
    /// Numéro ABC.
    pub number: usize,
    /// Nom affiché (`name=`/`nm=`, ABC 2.1 §7.1).
    pub name: String,
    /// Nom court affiché (`snm=`, `abc2xml.py::voiceNamesAndMaps`).
    pub short_name: String,
    /// Clef demandée.
    pub clef: Option<ClefType>,
    /// Transposition demandée, en demi-tons (`transpose=`/`t=`).
    pub transpose: Option<i32>,
}

/// Transposition d'un champ de paramètres (`transpose=` ou `t=`).
///
/// `abc2xml.py` accepte les deux orthographes ; une valeur invalide est
/// ignorée avec un avertissement plutôt que de faire échouer le morceau.
fn parse_transpose_parameter(
    parameters: &[(String, String)],
    line: usize,
    warnings: &mut Vec<Warning>,
) -> Option<i32> {
    parameters
        .iter()
        .find(|(name, _)| name == "transpose" || name == "t")
        .and_then(|(_, value)| parse_transpose_value(value, line, warnings))
}

/// Analyse une valeur de transposition signée.
fn parse_transpose_value(value: &str, line: usize, warnings: &mut Vec<Warning>) -> Option<i32> {
    match value.trim().parse::<i32>() {
        Ok(number) => Some(number),
        Err(_) => {
            warnings.push(Warning::new(
                line,
                format!("transposition « {value} » ignorée"),
            ));
            None
        }
    }
}

/// Analyse les champs d'une ligne `V:`.
/// Découpe les champs d'un `V:` en respectant les guillemets doubles.
fn split_voice_fields(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for character in text.chars() {
        match character {
            '"' => {
                in_quotes = !in_quotes;
                current.push(character);
            }
            character if character.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            other => current.push(other),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

pub(crate) fn parse_voice_fields(text: &str) -> VoiceFields {
    let mut number = 1usize;
    let mut name = String::new();
    let mut short_name = String::new();
    let mut clef = None;
    let mut transpose = None;
    // Un texte entre guillemets peut contenir des espaces (`name="Voix 1"`) :
    // ABC 2.1 § 8.2. Découper avec `split_whitespace` tronquait le nom au
    // premier espace, rendant l'écriture instable.
    let parts = split_voice_fields(text);
    for (index, part) in parts.iter().enumerate() {
        let part = part.as_str();
        if index == 0 && !part.contains('=') {
            let first = part.trim_start_matches('"');
            if let Ok(value) = first.parse() {
                number = value;
            } else {
                // Port de `abc2xml.py::splitHeaderVoices` (ligne 779) et
                // `voiceNamesAndMaps` (ligne 2253) : un nom de voix (`V:S`)
                // est un identifiant, pas le numéro 1. `0` demande au lecteur
                // de lui attribuer un numéro unique, conservé pour `%%score`.
                name = first.to_owned();
                number = 0;
            }
            continue;
        }
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let value = value.trim_matches('"');
        match key.to_ascii_lowercase().as_str() {
            "name" | "nm" => name = value.to_owned(),
            "subname" | "snm" => short_name = value.to_owned(),
            "clef" => clef = pitch::parse_clef(value),
            "transpose" | "t" => transpose = value.parse().ok(),
            _ => {}
        }
    }
    VoiceFields {
        number,
        name,
        short_name,
        clef,
        transpose,
    }
}

/// Position dans une voix : mesure et tick dans la mesure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Position {
    /// Indice de mesure.
    pub measure: usize,
    /// Tick dans la mesure.
    pub tick: Fraction,
}

/// Annotation en attente, posée sur l'accord suivant.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AnnotationSpec {
    /// Texte libre (`"^texte"`).
    Text {
        /// Texte affiché.
        text: String,
        /// Au-dessus (`true`) ou au-dessous (`false`).
        above: bool,
        /// Vrai pour un accord chiffré.
        harmony: bool,
    },
    /// Nuance (`!p!`) et vélocité transportée par la décoration privée
    /// `!scoretk-velo-N!` (ABC 2.1 §11, les décorations inconnues sont
    /// ignorées par les autres programmes).
    Dynamic(String, Option<u8>),
    /// Point d'orgue sur un silence (`Hz`, `Hx`, `!fermata!z`). Le modèle
    /// n'attache pas d'articulation à un `Rest`, mais l'exporteur lit les
    /// `Annotation::Fermata` de segment sur chaque note, silence compris
    /// (`notations.rs::rest_notations` ; `abc2xml.py::doDecos` 1491-1492).
    Fermata(Fermata),
    /// Champ Q: en cours d'air (`abc2xml.py::doTempo`, 1884-1906) : chaque
    /// occurrence produit sa propre direction, pas seulement la dernière.
    Tempo(f64, String),
}

/// Accord d'agrément en attente.
#[derive(Clone, Debug)]
pub(crate) struct GraceData {
    /// Notes et durées des agréments, avec l'indicateur d'acciaccatura
    /// (`/` devant la note).
    pub notes: Vec<(NoteId, Fraction, bool)>,
}

/// Spécification de n-olet, résolue à la construction.
#[derive(Clone, Debug)]
pub(crate) struct TupletSpec {
    /// Position de début.
    pub position: Position,
    /// Ratio `nombre / normal`.
    pub ratio: Fraction,
    /// Durée notée d'une note.
    pub base_len: Fraction,
    /// Durée totale.
    pub len: Fraction,
    /// N-olet parent, s'il est imbriqué.
    pub parent: Option<usize>,
    /// Nombre de notes restant à lire.
    pub remaining: i64,
}

/// Accord ou silence d'une mesure, avant construction du modèle.
#[derive(Clone, Debug)]
pub(crate) struct EventData {
    /// Tick dans la mesure.
    pub tick: Fraction,
    /// Accord ou silence.
    pub chord_rest: scoretk_core::model::ChordRest,
    /// N-olet éventuel.
    pub tuplet: Option<usize>,
    /// Annotations lues avant l'événement.
    pub annotations: Vec<AnnotationSpec>,
}

/// Mesure en cours de lecture.
#[derive(Clone, Debug)]
pub(crate) struct MeasureData {
    /// Événements, dans l'ordre du fichier.
    pub events: Vec<EventData>,
    /// Barre de début.
    pub start_bar: BarLineType,
    /// Barre de fin.
    pub end_bar: BarLineType,
    /// Numéros de volta qui commencent ici.
    pub volta: Option<Vec<u8>>,
    /// Longueur réelle.
    pub len: Fraction,
    /// Longueur nominale.
    pub nominal_len: Fraction,
}

impl MeasureData {
    /// Mesure vide.
    pub(crate) fn new() -> Self {
        Self {
            events: Vec::new(),
            start_bar: BarLineType::Normal,
            end_bar: BarLineType::Normal,
            volta: None,
            len: Fraction::ZERO,
            nominal_len: Fraction::ZERO,
        }
    }
}

/// Voix ABC en cours de lecture.
#[derive(Clone, Debug)]
pub(crate) struct Voice {
    /// Numéro ABC.
    pub number: usize,
    /// Voix racine d'une superposition `&` (port de `abc2xml.py::voice_overlay`,
    /// ligne 425) : cette voix partage la portée de la racine.
    pub overlay_of: Option<usize>,
    /// Nom affiché.
    pub name: String,
    /// Nom court affiché (`snm=`).
    pub short_name: String,
    /// Identifiant d'instrument transmis par `%%scoretk instrument` (ABC 2.1
    /// §11 : les directives `%%` inconnues sont ignorées par les autres
    /// programmes ; ici elle complète `nm=`/`snm=` et `%%MIDI program`).
    pub instrument_id: Option<String>,
    /// Clef demandée.
    pub clef: Option<ClefType>,
    /// Unité `L:` en vigueur.
    pub unit: Fraction,
    /// Indication de mesure en vigueur.
    pub time: TimeSig,
    /// Armure en vigueur.
    pub key: KeySig,
    /// Transposition en vigueur, en demi-tons (hauteur réelle = écrite + transposition).
    pub transpose: i32,
    /// Transposition d'instrument seule, sans les decalages d'octave des clefs.
    pub instrument_transpose: i32,
    /// Indication de mesure initiale.
    pub initial_time: TimeSig,
    /// Armure initiale.
    pub initial_key: KeySig,
    /// Clef initiale.
    pub initial_clef: Option<ClefType>,
    /// Mesures terminées.
    pub measures: Vec<MeasureData>,
    /// Mesure en cours.
    pub current: MeasureData,
    /// Tick courant dans la mesure.
    pub tick: Fraction,
    /// Altérations de la mesure.
    pub accidental: pitch::AccidentalState,
    /// N-olets rencontrés.
    pub tuplets: Vec<TupletSpec>,
    /// Pile des n-olets ouverts.
    pub open_tuplets: Vec<usize>,
    /// Slurs `(début, fin)`.
    pub slurs: Vec<(Position, Position)>,
    /// Pile des slurs ouverts.
    pub open_slurs: Vec<Position>,
    /// Soufflets.
    pub hairpins: Vec<(Position, Position, HairpinType)>,
    /// Pile des soufflets ouverts.
    pub open_hairpins: Vec<(usize, Position)>,
    /// Liaisons en attente, par hauteur.
    pub pending_ties: BTreeMap<u8, (NoteId, Position)>,
    /// Agréments en attente.
    pub grace: Option<GraceData>,
    /// Décorations en attente.
    pub decorations: Vec<Decoration>,
    /// Annotations en attente.
    pub annotations: Vec<AnnotationSpec>,
    /// Accords ajoutés par la dernière ligne de musique, pour `w:`.
    pub last_line_chords: Vec<(usize, usize)>,
    /// Numéro de couplet de la prochaine ligne `w:`.
    pub next_verse: usize,
    /// Vrai si un mot de paroles est ouvert (césure `-` en attente).
    pub lyric_word_open: bool,
    /// Barre de début de la prochaine mesure.
    pub next_start_bar: BarLineType,
    /// Facteur de rythme brisé appliqué à la prochaine note.
    pub pending_rhythm: Option<Fraction>,
    /// Changements d'armure `(mesure, tick, armure)`.
    pub key_changes: Vec<(usize, Fraction, Option<KeySig>)>,
    /// Changements de mesure `(mesure, tick, mesure)`.
    pub time_changes: Vec<(usize, Fraction, Option<TimeSig>)>,
    /// Changements de clef `(mesure, tick, clef)`.
    pub clef_changes: Vec<(usize, Fraction, Option<ClefType>)>,
    /// Voltas `(mesure de début, mesure de fin, numéros)`.
    pub voltas: Vec<(usize, usize, Vec<u8>)>,
}

impl Voice {
    /// Nouvelle voix avec les états globaux.
    pub(crate) fn new(fields: &VoiceFields, unit: Fraction, time: TimeSig, key: KeySig) -> Self {
        Self {
            number: fields.number,
            overlay_of: None,
            name: fields.name.clone(),
            short_name: fields.short_name.clone(),
            instrument_id: None,
            clef: fields.clef,
            unit,
            time: time.clone(),
            key: key.clone(),
            transpose: 0,
            instrument_transpose: 0,
            initial_time: time,
            accidental: pitch::AccidentalState::new(key.concert_key.fifths()),
            initial_key: key,
            initial_clef: fields.clef,
            measures: Vec::new(),
            current: MeasureData::new(),
            tick: Fraction::ZERO,
            tuplets: Vec::new(),
            open_tuplets: Vec::new(),
            slurs: Vec::new(),
            open_slurs: Vec::new(),
            hairpins: Vec::new(),
            open_hairpins: Vec::new(),
            pending_ties: BTreeMap::new(),
            grace: None,
            decorations: Vec::new(),
            annotations: Vec::new(),
            last_line_chords: Vec::new(),
            next_verse: 0,
            lyric_word_open: false,
            next_start_bar: BarLineType::Normal,
            pending_rhythm: None,
            key_changes: Vec::new(),
            time_changes: Vec::new(),
            clef_changes: Vec::new(),
            voltas: Vec::new(),
        }
    }

    /// Indice de la mesure en cours.
    pub(crate) fn measure_index(&self) -> usize {
        self.measures.len()
    }

    /// Position courante.
    pub(crate) fn position(&self) -> Position {
        Position {
            measure: self.measure_index(),
            tick: self.tick,
        }
    }

    /// Enregistre un changement d'armure à la position courante.
    pub(crate) fn schedule_key(&mut self, key: KeySig) {
        let measure = self.measure_index();
        let tick = self.tick;
        // Un champ `K:` répété identique est un remplacement (ABC 2.1 §3.2,
        // `abc2xml.py::doKey`) : il doit produire son propre `<key>`, mais ne
        // doit pas effacer les altérations déjà vues dans la mesure (§4.2).
        // `finish_measure` réinitialise déjà l'état à chaque barre.
        let changed = self.key.transposed_key != key.transposed_key || self.key.mode != key.mode;
        self.key = key.clone();
        if changed {
            self.accidental = pitch::AccidentalState::new(key.transposed_key.fifths());
        }
        self.key_changes.push((measure, tick, Some(key)));
    }

    /// Enregistre un changement de mesure à la position courante.
    pub(crate) fn schedule_time(&mut self, time: TimeSig) {
        let measure = self.measure_index();
        let tick = self.tick;
        self.time = time.clone();
        self.time_changes.push((measure, tick, Some(time)));
    }

    /// Enregistre un changement de clef à la position courante.
    pub(crate) fn schedule_clef(&mut self, clef: ClefType) {
        let measure = self.measure_index();
        let tick = self.tick;
        self.clef = Some(clef);
        self.clef_changes.push((measure, tick, Some(clef)));
    }

    /// Termine la mesure en cours et en ouvre une nouvelle.
    pub(crate) fn finish_measure(&mut self, end_bar: BarLineType) {
        let actual = self.tick;
        let nominal = self.time.measure_len();
        // Port de `abc2xml.py::doMaat` (ligne 674) et de la grammaire
        // `measure`/`bar_right` (lignes 428-436) : la mesure garde sa
        // longueur ÉCRITE, anacrouse comprise ; voir `measure::do_maat`.
        let len = measure::do_maat(actual, nominal);
        let mut measure = std::mem::replace(&mut self.current, MeasureData::new());
        measure.len = len;
        measure.nominal_len = nominal;
        measure.end_bar = end_bar;
        measure.start_bar = if self.next_start_bar != BarLineType::Normal {
            self.next_start_bar
        } else {
            measure.start_bar
        };
        self.next_start_bar = BarLineType::Normal;
        self.measures.push(measure);
        self.tick = Fraction::ZERO;
        self.accidental = pitch::AccidentalState::new(self.key.concert_key.fifths());
    }
}

/// État de lecture d'un morceau.
pub(crate) struct Reader {
    /// Partition en construction.
    pub score: Score,
    /// Voix lues.
    pub voices: Vec<Voice>,
    /// Voix courante.
    pub current: usize,
    /// Avertissements.
    pub warnings: Vec<Warning>,
    /// Unité globale `L:`.
    pub unit: Fraction,
    /// Mesure globale.
    pub time: TimeSig,
    /// Armure globale.
    pub key: KeySig,
    /// Transposition globale (`%%MIDI transpose` ou paramètre de `K:`), en demi-tons.
    pub midi_transpose: i32,
    /// Transposition globale d'instrument seule (sans decalage de clef).
    pub instrument_transpose: i32,
    /// Programme GM lu dans `%%MIDI program` (base zéro dans le modèle).
    pub midi_program: Option<u8>,
    /// Canal lu dans `%%MIDI channel` (base zéro dans le modèle).
    pub midi_channel: Option<u8>,
    /// Tempo global `(battements par seconde, texte)`.
    pub tempo: Option<(f64, String)>,
    /// Premier champ Q: (en-tête), conservé même si un Q: en ligne suit.
    pub header_tempo: Option<(f64, String)>,
    /// Symboles redéfinis par `U:` (lettre → nom de décoration).
    pub user_symbols: BTreeMap<char, String>,
    /// Numéros attribués aux voix nommées `V:S`, `V:A`… (port de
    /// `abc2xml.py::splitHeaderVoices`, ligne 779).
    pub voice_ids: BTreeMap<String, usize>,
    /// Voix superposées créées pour chaque racine (`&`, ligne 425) ; elles sont
    /// réutilisées de mesure en mesure car le modèle n'a que quatre voix.
    pub overlay_pool: BTreeMap<usize, Vec<usize>>,
    /// Prochaine voix superposée à utiliser dans la mesure courante.
    pub overlay_cursor: BTreeMap<usize, usize>,
    /// Groupement `%%score`.
    pub score_spec: Option<String>,
    /// Titre.
    pub title: String,
    /// Compositeur.
    pub composer: String,
    /// Source.
    pub source: String,
    /// Champs `W:` (texte imprimé, ABC 2.1 §3.1), dans l'ordre du fichier.
    pub credits: Vec<String>,
    /// Métadonnées complémentaires `%%scoretk meta <nom> <valeur>` (ABC 2.1
    /// §11) : les seuls champs standard `T:`/`C:` ne portent pas tous ceux du modèle.
    pub meta: Vec<(String, String)>,
    /// Textes de cadre `%%scoretk tbox <index> <texte>` (ABC 2.1 §11).
    pub tbox_texts: Vec<(usize, String)>,
    /// Vélocité lue dans `!scoretk-velo-N!`, pour la prochaine nuance.
    pub pending_dynamic_velocity: Option<u8>,
    /// Type de trémolo lu dans `!scoretk-tremolo-<nom>!`, pour le prochain
    /// `!…!` de trémolo (les noms standards ne portent que `single`).
    pub pending_tremolo_kind: Option<TremoloKind>,
    /// Vrai si un champ `K:` a été lu.
    pub seen_key: bool,
    /// Clef par défaut issue des paramètres de `K:`.
    pub default_clef: Option<ClefType>,
    /// Armure initiale de l'en-tête.
    pub header_key: KeySig,
    /// Indication de mesure initiale de l'en-tête.
    pub header_time: TimeSig,
    /// Unité `L:` initiale de l'en-tête.
    pub header_unit: Fraction,
    /// Vrai dès la première ligne de musique.
    pub header_done: bool,
}

impl Default for Reader {
    fn default() -> Self {
        Self {
            score: Score::new(),
            voices: Vec::new(),
            current: 0,
            warnings: Vec::new(),
            unit: Fraction::new(1, 8),
            time: TimeSig::new(4, 4),
            key: KeySig::new(Key::C),
            midi_transpose: 0,
            instrument_transpose: 0,
            midi_program: None,
            midi_channel: None,
            tempo: None,
            header_tempo: None,
            user_symbols: BTreeMap::new(),
            voice_ids: BTreeMap::new(),
            overlay_pool: BTreeMap::new(),
            overlay_cursor: BTreeMap::new(),
            score_spec: None,
            title: String::new(),
            composer: String::new(),
            source: String::new(),
            credits: Vec::new(),
            meta: Vec::new(),
            tbox_texts: Vec::new(),
            pending_dynamic_velocity: None,
            pending_tremolo_kind: None,
            seen_key: false,
            default_clef: None,
            header_key: KeySig::new(Key::C),
            header_time: TimeSig::new(4, 4),
            header_unit: Fraction::new(1, 8),
            header_done: false,
        }
    }
}

impl Reader {
    /// Lit un morceau.
    fn parse_tune(&mut self, lines: &[Line]) -> Result<()> {
        for line in lines {
            let text = line.text.trim();
            if text.is_empty() {
                continue;
            }
            if let Some(directive) = text.strip_prefix("%%") {
                self.handle_directive(directive, line.number);
                continue;
            }
            if text.starts_with('%') {
                continue;
            }
            if let Some((letter, value)) = field_value(line) {
                if letter == 'w' {
                    if let Some(voice) = self.voices.get_mut(self.current) {
                        lyrics::apply(voice, value, line.number, &mut self.warnings);
                    }
                    continue;
                }
                self.handle_field(letter, value, line.number)?;
                continue;
            }
            if !self.seen_key {
                // Port de `abc2xml.py::splitHeaderVoices` (lignes 833-837) :
                // tout ce qui précède le champ `K:` est un en-tête. Une ligne
                // sans champ (`3/8=120` par exemple) est signalée et sautée,
                // au lieu de faire rejeter l'air entier. Une ligne de musique
                // (notes ou barres) sans K: est tolérée avec l'armure C
                // fournie par défaut par `abc2xml.py:2423` (ABC 2.1 §3.1
                // exige K:, mais l'alternative lit ces airs).
                // A prose line (e.g. `N bars 13 and 21 are |f2ed| …`) is not
                // music even though it mentions notes: require the FIRST
                // token to be made only of ABC music characters.
                const MUSIC_CHARS: &str = "ABCDEFGabcdefgzZxX0123456789|[]():,!'\"^+_=<>/-\\";
                let tokens: Vec<&str> = text.split_whitespace().collect();
                let music_token = |token: &str| {
                    // Une décoration longue `!pppp!` contient des lettres
                    // quelconques (ABC 2.1 §4.14) : la retirer avant de
                    // décider si la ligne est de la musique, sinon un accord
                    // `[!pppp!c!ffff!D]` est pris pour de la prose et l'air
                    // entier est lu sans notes (`VolumeInChords`).
                    // Un champ en ligne `[V:1]` peut ouvrir la ligne de
                    // musique (ABC 2.1 §7.3) : le retirer avant la
                    // classification, sinon son `V` fait croire à de la
                    // prose (`parsing.test__abc3` X:360).
                    let mut reste = token;
                    while reste.starts_with('[') {
                        let Some(fin) = reste.find(']') else { break };
                        let champ = &reste[1..fin];
                        if !champ.contains(':') {
                            break; // accord `[CEG]`, pas un champ
                        }
                        reste = reste[fin + 1..].trim_start();
                    }
                    let mut nu = String::new();
                    let mut decoration = None;
                    for c in reste.chars() {
                        if let Some(delimiter) = decoration {
                            if c == delimiter {
                                decoration = None;
                            }
                            continue;
                        }
                        if c == '!' || c == '+' {
                            decoration = Some(c);
                            continue;
                        }
                        nu.push(c);
                    }
                    !nu.is_empty() && nu.chars().all(|c| MUSIC_CHARS.contains(c))
                };
                let music_line = tokens.first().is_some_and(|first| {
                    if first.len() >= 2 {
                        music_token(first)
                    } else {
                        // `C D E F |` starts with spaced single notes; a
                        // prose line `N bars …` does not.
                        music_token(first)
                            && tokens.get(1).is_some_and(|second| music_token(second))
                    }
                });
                if !music_line {
                    self.warnings.push(Warning::new(
                        line.number,
                        format!("ligne « {text} » avant K: ignorée"),
                    ));
                    continue;
                }
                self.warnings.push(Warning::new(
                    line.number,
                    "champ K: absent, do majeur pris par défaut".to_owned(),
                ));
                self.seen_key = true;
                self.header_key = scoretk_core::model::KeySig::new(scoretk_core::model::Key::C);
                self.key = self.header_key.clone();
            }
            if self.voices.is_empty() {
                let fields = VoiceFields {
                    number: 1,
                    name: String::new(),
                    short_name: String::new(),
                    clef: self.default_clef,
                    transpose: None,
                };
                let mut voice = Voice::new(
                    &fields,
                    self.header_unit,
                    self.header_time.clone(),
                    self.header_key.clone(),
                );
                voice.transpose = self.midi_transpose;
                voice.instrument_transpose = self.instrument_transpose;
                self.voices.push(voice);
            }
            if !self.seen_key {
                return Err(Error::EnTeteIncomplet("K:"));
            }
            self.header_done = true;
            let current = self.current;
            music::parse_line(self, current, text, line.number)?;
        }
        // A tune with no music line at all and no K: is empty; the default
        // C major key from `abc2xml.py:2423` still yields a valid score.
        Ok(())
    }

    /// Traite une directive `%%`.
    fn handle_directive(&mut self, directive: &str, number: usize) {
        let (name, value) = directive
            .split_once(char::is_whitespace)
            .map_or((directive, ""), |(name, value)| (name, value.trim()));
        match name.to_ascii_lowercase().as_str() {
            "score" | "staves" => {
                if self.score_spec.is_none() {
                    self.score_spec = Some(value.to_owned());
                } else {
                    self.warnings.push(Warning::new(
                        number,
                        "une seule directive %%score est prise en compte",
                    ));
                }
            }
            "abc-version" | "abc-include" | "unit" => {}
            // §11 : une directive `%%` est un champ d'information ignoré par
            // les programmes qui ne la connaissent pas. Celle-ci porte
            // l'identifiant d'instrument du modèle, que ni `%%MIDI program`
            // (numéro GM) ni `nm=`/`snm=` (noms affichés) ne peuvent porter.
            "scoretk"
                if value
                    .split_whitespace()
                    .next()
                    .is_some_and(|key| key.eq_ignore_ascii_case("instrument")) =>
            {
                let id = value
                    .split_once(char::is_whitespace)
                    .map_or("", |(_, rest)| rest.trim());
                if !id.is_empty() && !self.voices.is_empty() {
                    self.voices[self.current].instrument_id = Some(id.to_owned());
                }
            }
            "midi"
                if value
                    .split_whitespace()
                    .next()
                    .is_some_and(|key| key.eq_ignore_ascii_case("transpose")) =>
            {
                let amount = value.split_whitespace().nth(1).unwrap_or("");
                if let Some(transpose) = parse_transpose_value(amount, number, &mut self.warnings) {
                    self.midi_transpose = transpose;
                    self.instrument_transpose = transpose;
                    // La directive globale s'applique aux voix déjà créées ;
                    // les voix suivantes héritent de `midi_transpose`.
                    for voice in &mut self.voices {
                        voice.transpose = transpose;
                        voice.instrument_transpose = transpose;
                    }
                }
            }
            "scoretk"
                if value
                    .split_whitespace()
                    .next()
                    .is_some_and(|key| key.eq_ignore_ascii_case("meta")) =>
            {
                let mut words = value.splitn(3, char::is_whitespace);
                let _ = words.next();
                if let (Some(name), Some(text)) = (words.next(), words.next().map(str::trim)) {
                    if !text.is_empty() {
                        self.meta.push((name.to_ascii_lowercase(), text.to_owned()));
                    }
                }
            }
            "scoretk"
                if value
                    .split_whitespace()
                    .next()
                    .is_some_and(|key| key.eq_ignore_ascii_case("tbox")) =>
            {
                let mut words = value.splitn(3, char::is_whitespace);
                let _ = words.next();
                if let (Some(index), Some(text)) = (
                    words.next().and_then(|value| value.parse::<usize>().ok()),
                    words.next().map(str::trim),
                ) {
                    if !text.is_empty() {
                        self.tbox_texts.push((index, text.to_owned()));
                    }
                }
            }
            "midi" => {
                // §6.2 : `%%MIDI program` (GM 1–128) et `%%MIDI channel`
                // (1–16) sont les seules valeurs de playback que l'ABC porte
                // sans extension ; le modèle stocke des indices base zéro.
                let mut words = value.split_whitespace();
                let key = words.next().unwrap_or_default().to_ascii_lowercase();
                let amount = words.next().and_then(|value| value.parse::<u16>().ok());
                match (key.as_str(), amount) {
                    ("program", Some(program)) if (1..=128).contains(&program) => {
                        self.midi_program = Some((program - 1) as u8);
                    }
                    ("channel", Some(channel)) if (1..=16).contains(&channel) => {
                        self.midi_channel = Some((channel - 1) as u8);
                    }
                    _ => self.warnings.push(Warning::new(
                        number,
                        format!("directive « %%MIDI {value} » ignorée"),
                    )),
                }
            }
            other => self.warnings.push(Warning::new(
                number,
                format!("directive « %%{other} » ignorée"),
            )),
        }
    }

    /// Traite un champ d'en-tête ou de corps.
    pub(crate) fn handle_field(&mut self, letter: char, value: &str, number: usize) -> Result<()> {
        match letter {
            // §3.1 : `W:` est un texte imprimé séparé des paroles `w:`. Le
            // modèle le range dans `engraving.credits`, comme les `<credit>`
            // MusicXML dont il est la contrepartie ABC.
            'W' => self.credits.push(value.to_owned()),
            'X' | 'R' | 'Z' | 'N' | 'H' | 'A' | 'D' | 'F' | 'G' | 'B' => {}
            'T' => {
                if self.title.is_empty() {
                    self.title = value.to_owned();
                }
            }
            'C' => {
                if self.composer.is_empty() {
                    self.composer = value.to_owned();
                }
            }
            'O' | 'S' => {
                if self.source.is_empty() {
                    self.source = value.to_owned();
                }
            }
            'M' => {
                let time = parse_time(value, number)?;
                if !self.header_done {
                    self.header_time = time.clone();
                }
                self.time = time.clone();
                for voice in &mut self.voices {
                    if voice.time.numerator != time.numerator
                        || voice.time.denominator != time.denominator
                    {
                        voice.schedule_time(time.clone());
                    }
                }
            }
            'L' => {
                self.unit = parse_unit(value, number)?;
                if !self.header_done {
                    self.header_unit = self.unit;
                }
                for voice in &mut self.voices {
                    voice.unit = self.unit;
                }
            }
            'Q' => {
                self.tempo = parse_tempo(value);
                if !self.header_done {
                    self.header_tempo = self.tempo.clone();
                }
                // `abc2xml.py::doTempo` writes a direction for every Q:
                // field. A single `tempo` slot kept only the last one, so an
                // inline [Q:] erased the header label.
                if let Some((bps, text)) = &self.tempo {
                    if !self.voices.is_empty() {
                        self.voices[self.current]
                            .annotations
                            .push(AnnotationSpec::Tempo(*bps, text.clone()));
                    }
                }
            }
            'K' => {
                // Port de `abc2xml.py::doHeaderField` (lignes 1812-1813,
                // 1824-1828) : les altérations explicites séparées par un
                // espace (`K:C ^F _B`) sont signalées et sautées ; le modèle
                // pivot n'a pas de signature graphique pour les porter.
                let nettoye = strip_key_accidentals(value, number, &mut self.warnings);
                let parsed = match pitch::parse_key(&nettoye) {
                    Some(parsed) => parsed,
                    None => {
                        // Un champ sans tonique (`K:treble+8`) ne fait pas
                        // rejeter l'air : le script n'écrit alors aucune
                        // armure, soit C majeur par défaut.
                        self.warnings.push(Warning::new(
                            number,
                            format!("tonalité « {value} » non reconnue, C majeur pris par défaut"),
                        ));
                        scoretk_abc_common::pitch::ParsedKey {
                            key: KeySig::new(Key::C),
                            exotic_mode: None,
                            parameters: Vec::new(),
                        }
                    }
                };
                self.seen_key = true;
                if !self.header_done {
                    self.header_key = parsed.key.clone();
                }
                self.key = parsed.key.clone();
                if let Some(mode) = parsed.exotic_mode {
                    self.warnings.push(Warning::new(
                        number,
                        format!("mode {mode} ramené à son armure majeure"),
                    ));
                }
                // `abc2xml.py` writes a `<key>` for every K: field, even an
                // identical one (ABC 2.1 §3.2 "Repeated information fields":
                // it is a replacement). Keep each occurrence as a key change
                // so the exporter emits one `<key>` per field; the previous
                // change-only condition dropped redundant fields.
                // ABC 2.1 §7.3 : un champ `[K:…]` dans le corps s'applique à
                // la voix courante seulement, tandis que l'`K:` d'en-tête vaut
                // pour toutes les voix. `abc2xml.py` traite les champs inline
                // pendant `mkPart` (`doFields`, ligne 2029) et n'appelle
                // `doHeaderField` (2271) que pour cette partie. Étendre un
                // champ inline à toutes les voix ajoutait à chacune un
                // changement d'armure à la mesure de l'autre voix.
                let targeted: Vec<usize> = if self.header_done {
                    vec![self.current]
                } else {
                    (0..self.voices.len()).collect()
                };
                for index in &targeted {
                    self.voices[*index].schedule_key(parsed.key.clone());
                }
                if let Some(transpose) =
                    parse_transpose_parameter(&parsed.parameters, number, &mut self.warnings)
                {
                    self.midi_transpose = transpose;
                    self.instrument_transpose = transpose;
                    for index in &targeted {
                        self.voices[*index].transpose = transpose;
                        self.voices[*index].instrument_transpose = transpose;
                    }
                }
                // `abc2xml.py::doClef` (lignes 1736-1744) : un suffixe
                // d'octave sur la clef (`treble+8`, `bass-8`, `+15`…) décale
                // toutes les hauteurs d'un ou deux octaves (`s.gtrans`).
                // Le champ peut être nu (`K: treble+8`) ou en paramètre
                // `clef=treble+8` : ne traiter que les Jetons sans `=`.
                // A K: clef change carries an absolute octave shift, not an
                // increment: `[K:... clef=treble-8]` then `[K:... clef=treble]`
                // must cancel out. Applying -12 every time treble-8 reappears
                // accumulated -48 and made later notes negative (Veni veni
                // Emmanuel). ABC 2.1 §4.6; `abc2xml.py::doClef` lines 1736-1744.
                let previous_clefs: Vec<_> = self.voices.iter().map(|voice| voice.clef).collect();
                let old_shift = self
                    .default_clef
                    .map_or(0, |clef| 12 * i32::from(clef.octave_change()));
                let new_clef = parsed
                    .parameters
                    .iter()
                    .find(|(name, _)| name == "clef")
                    .and_then(|(_, value)| pitch::parse_clef(value))
                    .or_else(|| {
                        value
                            .split_whitespace()
                            .find(|token| pitch::parse_clef(token).is_some())
                            .and_then(pitch::parse_clef)
                    });
                let new_shift = new_clef.map_or(0, |clef| 12 * i32::from(clef.octave_change()));
                let delta = new_shift - old_shift;
                if let Some(clef) = new_clef {
                    self.default_clef = Some(clef);
                    for index in &targeted {
                        self.voices[*index].schedule_clef(clef);
                    }
                    // `abc2xml.py::doClef` (lignes 1730-1738) repose
                    // `s.gtrans = gtrans` : le décalage d'octave est **absolu**,
                    // pas un incrément global. Une voix dont le `V:` porte
                    // `clef=treble-8` a déjà reçu -12 : un `[K:… clef=bass]`
                    // doit le lui retirer, alors que `delta` (clef du `K:`
                    // global) vaut 0. Calculer l'écart depuis la clef courante
                    // de chaque voix, même quand le décalage global est nul.
                    for index in &targeted {
                        let voice = &mut self.voices[*index];
                        let previous = previous_clefs[*index];
                        // A voice already on the new clef (e.g. from V:) has
                        // already received this shift.
                        if previous == Some(clef) {
                            continue;
                        }
                        let voice_old =
                            previous.map_or(old_shift, |clef| 12 * i32::from(clef.octave_change()));
                        voice.transpose += new_shift - voice_old;
                    }
                }
                if delta != 0 {
                    self.midi_transpose += delta;
                }
            }
            'V' => {
                let mut fields = parse_voice_fields(value);
                if fields.number == 0 {
                    // Un nom de voix reçoit un numéro unique, mémorisé afin que
                    // `%%score (S A)` puisse le retrouver plus tard.
                    let name = fields.name.clone();
                    let number = if let Some(&number) = self.voice_ids.get(&name) {
                        number
                    } else {
                        let number = self.voice_ids.values().copied().max().unwrap_or(0) + 1;
                        self.voice_ids.insert(name, number);
                        number
                    };
                    fields.number = number;
                }
                if let Some(index) = self
                    .voices
                    .iter()
                    .position(|voice| voice.number == fields.number)
                {
                    self.current = index;
                    if !fields.name.is_empty() {
                        self.voices[index].name = fields.name;
                    }
                    if !fields.short_name.is_empty() {
                        self.voices[index].short_name = fields.short_name;
                    }
                    if let Some(clef) = fields.clef {
                        // Le `K:` a déjà appliqué le décalage de sa propre
                        // clef à toutes les voix : n'ajouter que la différence
                        // pour ne pas le compter deux fois (ex. `K:… clef=treble-8`
                        // suivi de `V:1 … clef=treble-8`).
                        let applied = self
                            .default_clef
                            .map_or(0, |value| 12 * i32::from(value.octave_change()));
                        self.voices[index].transpose +=
                            12 * i32::from(clef.octave_change()) - applied;
                        self.voices[index].clef = Some(clef);
                    }
                    if let Some(transpose) = fields.transpose {
                        self.voices[index].transpose =
                            transpose + self.midi_transpose - self.instrument_transpose;
                        self.voices[index].instrument_transpose = transpose;
                    }
                } else {
                    let instrument_transpose =
                        fields.transpose.unwrap_or(self.instrument_transpose);
                    let transpose = fields.transpose.map_or(self.midi_transpose, |t| {
                        t + self.midi_transpose - self.instrument_transpose
                    });
                    let fields = VoiceFields {
                        clef: fields.clef.or(self.default_clef),
                        ..fields
                    };
                    let mut voice = Voice::new(
                        &fields,
                        self.header_unit,
                        self.header_time.clone(),
                        self.header_key.clone(),
                    );
                    let applied = self
                        .default_clef
                        .map_or(0, |value| 12 * i32::from(value.octave_change()));
                    voice.transpose = transpose
                        + fields
                            .clef
                            .map_or(0, |clef| 12 * i32::from(clef.octave_change()))
                        - applied;
                    voice.instrument_transpose = instrument_transpose;
                    self.current = self.voices.len();
                    self.voices.push(voice);
                }
            }
            'U' => {
                // Norme 2.1 § 4.16 : `U: k = !trill!` associe une décoration
                // à une lettre H-W, h-w ou au symbole `~`.
                let definition = value.trim();
                let parsed = definition.split_once('=').and_then(|(symbol, name)| {
                    let symbol = symbol.trim().chars().next()?;
                    if !matches!(symbol, 'H'..='W' | 'h'..='w' | '~') {
                        return None;
                    }
                    let name = name
                        .trim()
                        .trim_matches(['!', '+', '"'])
                        .to_ascii_lowercase();
                    Some((symbol, name))
                });
                if let Some((symbol, name)) = parsed {
                    self.user_symbols.insert(symbol, name);
                } else {
                    self.warnings.push(Warning::new(
                        number,
                        format!("définition « U: {definition} » ignorée"),
                    ));
                }
            }
            'P' | 'I' => self
                .warnings
                .push(Warning::new(number, format!("champ « {letter}: » ignoré"))),
            other => self.warnings.push(Warning::new(
                number,
                format!("champ inconnu « {other}: » ignoré"),
            )),
        }
        Ok(())
    }

    /// Termine la lecture et construit le modèle.
    fn finish(mut self) -> Result<ReadOutput> {
        if self.voices.is_empty() {
            let fields = VoiceFields {
                number: 1,
                name: String::new(),
                short_name: String::new(),
                clef: None,
                transpose: None,
            };
            let mut voice = Voice::new(
                &fields,
                self.header_unit,
                self.header_time.clone(),
                self.header_key.clone(),
            );
            voice.transpose = self.midi_transpose;
            voice.instrument_transpose = self.instrument_transpose;
            self.voices.push(voice);
        }
        for voice in &mut self.voices {
            let end = voice
                .current
                .events
                .last()
                .map(|event| Position {
                    measure: voice.measure_index(),
                    tick: event.tick,
                })
                .unwrap_or_else(|| voice.position());
            if !voice.current.events.is_empty() || voice.measures.is_empty() {
                voice.finish_measure(BarLineType::Normal);
            }
            // `abc2xml.py:1516-1525` writes `<slur type="start">` on the note
            // after `(` even when no `)` closes it (ABC 2.1 §4.11). Close
            // those spanners at the last event so the start is not lost.
            if !voice.open_slurs.is_empty() {
                self.warnings.push(Warning::new(
                    0,
                    format!("slur non fermé dans la voix {}", voice.number),
                ));
                while let Some(start) = voice.open_slurs.pop() {
                    voice.slurs.push((start, end));
                }
            }
            if !voice.open_hairpins.is_empty() {
                self.warnings.push(Warning::new(
                    0,
                    format!("soufflet non fermé dans la voix {}", voice.number),
                ));
            }
            if !voice.pending_ties.is_empty() {
                self.warnings.push(Warning::new(
                    0,
                    format!("liaison sans note suivante dans la voix {}", voice.number),
                ));
            }
        }
        let score = build::build(&mut self)?;
        Ok(ReadOutput {
            score,
            warnings: self.warnings,
        })
    }
}

/// Retire d'un champ `K:` les altérations explicites séparées par un espace.
///
/// Port de `abc2xml.py` 269, `doHeaderField` (lignes 1812-1813 et 1824-1828) :
/// `^F`, `_B`, `=c`, `^/f` sont des altérations graphiques de la signature.
/// Le modèle pivot ne les porte pas ; on les signale et on les saute plutôt
/// que de rejeter tout l'air.
fn strip_key_accidentals(value: &str, line: usize, warnings: &mut Vec<Warning>) -> String {
    let chars: Vec<char> = value.chars().collect();
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() {
        let character = chars[index];
        if matches!(character, '^' | '_' | '=') && (index == 0 || chars[index - 1].is_whitespace())
        {
            let mut end = index + 1;
            while end < chars.len() && (chars[end].is_ascii_digit() || chars[end] == '/') {
                end += 1;
            }
            if end < chars.len() && chars[end].is_ascii_alphabetic() {
                let token: String = chars[index..=end].iter().collect();
                warnings.push(Warning::new(
                    line,
                    format!("altération explicite « {token} » dans K: ignorée"),
                ));
                index = end + 1;
                continue;
            }
        }
        out.push(character);
        index += 1;
    }
    out
}

/// Analyse une indication de mesure.
fn parse_time(text: &str, line: usize) -> Result<TimeSig> {
    let trimmed = text.trim();
    if trimmed == "C" {
        return Ok(TimeSig::new(4, 4));
    }
    if trimmed == "C|" {
        return Ok(TimeSig::new(2, 2));
    }
    if trimmed.eq_ignore_ascii_case("none") {
        return Ok(TimeSig::new(4, 4));
    }
    let (numerator, denominator) =
        trimmed
            .split_once('/')
            .ok_or_else(|| Error::MesureInvalide {
                line,
                text: trimmed.to_owned(),
            })?;
    // Port de `abc2xml.py::doFields` (ligne 1800, `eval(beats)`) : une mesure
    // additive telle que `2+3/8` vaut la somme de ses termes, soit 5/8.
    let numerator: i32 = numerator
        .trim()
        .split('+')
        .map(|term| {
            term.trim()
                .parse::<i32>()
                .map_err(|_| Error::MesureInvalide {
                    line,
                    text: trimmed.to_owned(),
                })
        })
        .sum::<Result<i32>>()?;
    let denominator: i32 = denominator
        .trim()
        .parse()
        .map_err(|_| Error::MesureInvalide {
            line,
            text: trimmed.to_owned(),
        })?;
    if numerator <= 0 || denominator <= 0 {
        return Err(Error::MesureInvalide {
            line,
            text: trimmed.to_owned(),
        });
    }
    Ok(TimeSig::new(numerator, denominator))
}

/// Analyse une unité `L:`.
fn parse_unit(text: &str, line: usize) -> Result<Fraction> {
    let trimmed = text.trim();
    let len = scoretk_abc_common::duration::parse_multiplier(trimmed, Fraction::ONE).ok_or_else(
        || Error::DureeInvalide {
            line,
            text: trimmed.to_owned(),
        },
    )?;
    if !len.is_positive() {
        return Err(Error::DureeInvalide {
            line,
            text: trimmed.to_owned(),
        });
    }
    Ok(len)
}

/// Analyse un tempo `Q:`.
fn parse_tempo(text: &str) -> Option<(f64, String)> {
    let text = text.trim();
    // `abc2xml.py::doTempo` (ligne 1886) finds the quoted label anywhere in
    // the field, not just as a leading prefix: `Q:1/4=80 "Slow"` is common.
    let (label, rest) = if let Some(start) = text.find('"') {
        let after = &text[start + 1..];
        if let Some(end) = after.find('"') {
            let label = after[..end].to_owned();
            // Remove every quoted segment from the numeric scan: a second
            // label (`[Q:"left" 1/4=170"right"]`) would otherwise land in
            // the bpm string and make the parse fail.
            let mut rest = String::new();
            let mut position = 0;
            while let Some(open) = text[position..].find('"') {
                rest.push_str(&text[position..position + open]);
                let after_open = position + open + 1;
                match text[after_open..].find('"') {
                    Some(close) => position = after_open + close + 1,
                    None => {
                        position = text.len();
                        break;
                    }
                }
            }
            rest.push_str(&text[position..]);
            (label, rest)
        } else {
            (String::new(), text.to_owned())
        }
    } else {
        (String::new(), text.to_owned())
    };
    let (_, bpm) = match rest.split_once('=') {
        Some(parts) => parts,
        None => {
            // A label-only `Q:` still produces a tempo direction; the
            // alternative defaults to 120 bpm (`abc2xml.py::doTempo`,
            // lignes 1890-1895). An empty field is not a tempo at all.
            if label.is_empty() {
                return None;
            }
            return Some((120.0 / 60.0, label));
        }
    };
    let bpm: f64 = bpm.trim().parse().ok()?;
    if bpm <= 0.0 {
        return None;
    }
    Some((bpm / 60.0, label))
}

#[cfg(test)]
mod clef_octave_tests {
    /// `abc2xml.py::doClef` (lignes 1736-1744) : `treble+8` décale d'un
    /// octave toutes les hauteurs (`s.gtrans += 1`). Le port ne lisait que la
    /// clef graphique et perdait le décalage (4 airs RV/ABC octave clef).
    #[test]
    fn clef_avec_octave_decale_les_hauteurs() {
        let score =
            crate::reader::read_abc("X:1\nM:4/4\nL:1/4\nK:C\n[K: treble+8]C4|\n").expect("lecture");
        let pitches: Vec<u8> = score.notes.iter().map(|note| note.pitch).collect();
        assert_eq!(pitches, vec![72], "hauteurs: {:?}", pitches);
    }

    /// `abc2xml.py::doClef` (ligne 1730) pose `s.gtrans = gtrans` de façon
    /// absolue : un `[K: clef=bass]` en cours d'air annule le `-8` d'un
    /// `V:1 clef=treble-8`. Le port ne recalculait que l'écart global et
    /// laissait la voix un octave trop bas après le changement de clef.
    /// `abc2xml.py` écrit un `<key>` pour chaque champ `K:` répété
    /// (ABC 2.1 §3.2 : un champ répété est un remplacement), mais un champ
    /// identique ne doit pas effacer la mémoire des altérations de la mesure
    /// (§4.2) : un `=B` reste valable jusqu'à la barre suivante.
    #[test]
    fn k_repetee_identique_garde_les_alterations_de_mesure() {
        let score = crate::reader::read_abc("X:1\nM:4/4\nL:1/8\nK:F\n=B c [K:F] B c |]\n")
            .expect("lecture");
        let pitches: Vec<u8> = score.notes.iter().map(|note| note.pitch).collect();
        assert_eq!(pitches, vec![71, 72, 71, 72], "hauteurs: {:?}", pitches);
    }

    #[test]
    fn changement_de_clef_annule_octave_de_la_voix() {
        let score = crate::reader::read_abc(
            "X:1\nM:4/4\nL:1/4\nK:C\nV:1 clef=treble-8\nC4|\n[K:C clef=bass]C4|\n",
        )
        .expect("lecture");
        let pitches: Vec<u8> = score.notes.iter().map(|note| note.pitch).collect();
        assert_eq!(pitches, vec![48, 60], "hauteurs: {:?}", pitches);
    }
}

#[cfg(test)]
mod preamble_tests {
    use super::*;

    /// `abc2xml.py::splitHeaderVoices` (ligne 789) : le premier `X:` ouvre le
    /// premier air ; les champs globaux qui le précèdent (`M:`, `O:`, `R:`…)
    /// appartiennent à cet air. Le port les traitait comme un air séparé,
    /// sans `K:`, ce qui rejetait 8 airs entiers (English.abc, Reels.abc…).
    #[test]
    fn champs_globaux_avant_premier_x_appartiennent_au_premier_air() {
        let text = "M:4/4\nO:Scottish\nR:Strathspey\n\nX:1\nT:Test\nK:D\ne<A A2|\n";
        let lines = prepare(text);
        let tunes = split_tunes(&lines);
        assert_eq!(
            tunes.len(),
            1,
            "airs: {:?}",
            tunes.iter().map(|t| &t[0].text).collect::<Vec<_>>()
        );
        assert_eq!(tunes[0][0].text, "M:4/4");
    }

    /// Le deuxième `X:` ouvre bien un air distinct.
    #[test]
    fn deux_airs_avec_preambule() {
        let text = "M:4/4\nX:1\nK:C\nC|\nX:2\nK:G\nG|\n";
        let tunes = split_tunes(&prepare(text));
        assert_eq!(tunes.len(), 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lignes_et_commentaires() {
        let lines = prepare("X:1\nT:Titre % commentaire\nC:\\\n  Auteur\n");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1].text, "T:Titre");
        assert_eq!(lines[2].text, "C: Auteur");
    }

    #[test]
    fn guillemets_protegent_le_pourcent() {
        assert_eq!(strip_comment("\"100%\" C"), "\"100%\" C");
        assert_eq!(strip_comment("%%score 1 2"), "%%score 1 2");
        assert_eq!(strip_comment("C D % fin"), "C D ");
    }

    #[test]
    fn champs() {
        assert_eq!(
            field_value(&Line {
                number: 1,
                text: "K:G".into()
            }),
            Some(('K', "G"))
        );
        assert_eq!(
            parse_voice_fields("1 name=\"Piano\" clef=bass"),
            VoiceFields {
                number: 1,
                name: "Piano".into(),
                short_name: String::new(),
                clef: Some(ClefType::F),
                transpose: None,
            }
        );
        assert_eq!(
            parse_voice_fields("2 transpose=-2 clef=bass"),
            VoiceFields {
                number: 2,
                name: String::new(),
                short_name: String::new(),
                clef: Some(ClefType::F),
                transpose: Some(-2),
            }
        );
        let time = parse_time("6/8", 1).expect("6/8");
        assert_eq!(time.measure_len(), Fraction::new(3, 4));
        assert_eq!(parse_unit("1/16", 1), Ok(Fraction::new(1, 16)));
        assert_eq!(
            parse_tempo("\"Allegro\" 1/4=120"),
            Some((2.0, "Allegro".into()))
        );
    }

    #[test]
    fn morceaux_multiples() {
        let lines = prepare("X:1\nK:C\nC|\nX:2\nK:G\nG|");
        let tunes = split_tunes(&lines);
        assert_eq!(tunes.len(), 2);
        assert_eq!(tunes[1][0].text, "X:2");
    }
}
