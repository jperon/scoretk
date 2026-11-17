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
mod music;

use std::collections::BTreeMap;

use scoretk_core::model::{
    BarLineType, ClefType, Fraction, HairpinType, Key, KeySig, NoteId, Score, TimeSig,
};

use crate::decorations::Decoration;
use crate::error::{Error, Result, Warning};
use crate::pitch;

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
        if text.ends_with('\\') {
            text.pop();
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
    for line in lines {
        if is_field(line, 'X') && !current.is_empty() {
            tunes.push(std::mem::take(&mut current));
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
    /// Nom affiché.
    pub name: String,
    /// Clef demandée.
    pub clef: Option<ClefType>,
}

/// Analyse les champs d'une ligne `V:`.
pub(crate) fn parse_voice_fields(text: &str) -> VoiceFields {
    let mut number = 1usize;
    let mut name = String::new();
    let mut clef = None;
    for (index, part) in text.split_whitespace().enumerate() {
        if index == 0 && !part.contains('=') {
            number = part.trim_start_matches('"').parse().unwrap_or(1);
            continue;
        }
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let value = value.trim_matches('"');
        match key.to_ascii_lowercase().as_str() {
            "name" | "nm" => name = value.to_owned(),
            "clef" => clef = pitch::parse_clef(value),
            _ => {}
        }
    }
    VoiceFields { number, name, clef }
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
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Nuance (`!p!`).
    Dynamic(String),
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
    /// Nom affiché.
    pub name: String,
    /// Clef demandée.
    pub clef: Option<ClefType>,
    /// Unité `L:` en vigueur.
    pub unit: Fraction,
    /// Indication de mesure en vigueur.
    pub time: TimeSig,
    /// Armure en vigueur.
    pub key: KeySig,
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
    pub pending_ties: BTreeMap<u8, NoteId>,
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
            name: fields.name.clone(),
            clef: fields.clef,
            unit,
            time: time.clone(),
            key: key.clone(),
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
        self.key = key.clone();
        self.accidental = pitch::AccidentalState::new(key.transposed_key.fifths());
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
        let pickup = self.measures.is_empty() && actual.is_positive() && actual < nominal;
        let mut len = if actual.is_zero() {
            if nominal.is_positive() {
                nominal
            } else {
                Fraction::from_integer(1)
            }
        } else if pickup {
            actual
        } else {
            actual.max(nominal)
        };
        if !len.is_positive() {
            len = Fraction::from_integer(1);
        }
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
    /// Tempo global `(battements par seconde, texte)`.
    pub tempo: Option<(f64, String)>,
    /// Groupement `%%score`.
    pub score_spec: Option<String>,
    /// Titre.
    pub title: String,
    /// Compositeur.
    pub composer: String,
    /// Source.
    pub source: String,
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
            tempo: None,
            score_spec: None,
            title: String::new(),
            composer: String::new(),
            source: String::new(),
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
            if self.voices.is_empty() {
                let fields = VoiceFields {
                    number: 1,
                    name: String::new(),
                    clef: self.default_clef,
                };
                let voice = Voice::new(
                    &fields,
                    self.header_unit,
                    self.header_time.clone(),
                    self.header_key.clone(),
                );
                self.voices.push(voice);
            }
            if !self.seen_key {
                return Err(Error::EnTeteIncomplet("K:"));
            }
            self.header_done = true;
            let current = self.current;
            music::parse_line(self, current, text, line.number)?;
        }
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
            other => self.warnings.push(Warning::new(
                number,
                format!("directive « %%{other} » ignorée"),
            )),
        }
    }

    /// Traite un champ d'en-tête ou de corps.
    pub(crate) fn handle_field(&mut self, letter: char, value: &str, number: usize) -> Result<()> {
        match letter {
            'X' | 'R' | 'Z' | 'N' | 'H' | 'A' | 'D' | 'F' | 'G' | 'B' | 'W' => {}
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
            }
            'K' => {
                let parsed = pitch::parse_key(value).ok_or_else(|| Error::TonaliteInvalide {
                    line: number,
                    text: value.to_owned(),
                })?;
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
                for voice in &mut self.voices {
                    if voice.key.transposed_key != parsed.key.transposed_key
                        || voice.key.mode != parsed.key.mode
                    {
                        voice.schedule_key(parsed.key.clone());
                    }
                }
                for (name, value) in &parsed.parameters {
                    if name == "clef" {
                        if let Some(clef) = pitch::parse_clef(value) {
                            self.default_clef = Some(clef);
                            for voice in &mut self.voices {
                                if voice.clef != Some(clef) {
                                    voice.schedule_clef(clef);
                                }
                            }
                        }
                    }
                }
            }
            'V' => {
                let fields = parse_voice_fields(value);
                if let Some(index) = self
                    .voices
                    .iter()
                    .position(|voice| voice.number == fields.number)
                {
                    self.current = index;
                    if !fields.name.is_empty() {
                        self.voices[index].name = fields.name;
                    }
                    if let Some(clef) = fields.clef {
                        self.voices[index].clef = Some(clef);
                    }
                } else {
                    let fields = VoiceFields {
                        clef: fields.clef.or(self.default_clef),
                        ..fields
                    };
                    let voice = Voice::new(
                        &fields,
                        self.header_unit,
                        self.header_time.clone(),
                        self.header_key.clone(),
                    );
                    self.current = self.voices.len();
                    self.voices.push(voice);
                }
            }
            'P' | 'U' | 'I' => self
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
                clef: None,
            };
            let voice = Voice::new(
                &fields,
                self.header_unit,
                self.header_time.clone(),
                self.header_key.clone(),
            );
            self.voices.push(voice);
        }
        for voice in &mut self.voices {
            if !voice.current.events.is_empty() || voice.measures.is_empty() {
                voice.finish_measure(BarLineType::Normal);
            }
            if !voice.open_slurs.is_empty() {
                self.warnings.push(Warning::new(
                    0,
                    format!("slur non fermé dans la voix {}", voice.number),
                ));
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
    let numerator: i32 = numerator
        .trim()
        .parse()
        .map_err(|_| Error::MesureInvalide {
            line,
            text: trimmed.to_owned(),
        })?;
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
    let len = crate::duration::parse_multiplier(trimmed, Fraction::ONE).ok_or_else(|| {
        Error::DureeInvalide {
            line,
            text: trimmed.to_owned(),
        }
    })?;
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
    let mut rest = text.trim();
    let mut label = String::new();
    if let Some(stripped) = rest.strip_prefix('"') {
        if let Some(end) = stripped.find('"') {
            label = stripped[..end].to_owned();
            rest = stripped[end + 1..].trim();
        }
    }
    let (_, bpm) = rest.split_once('=')?;
    let bpm: f64 = bpm.trim().parse().ok()?;
    if bpm <= 0.0 {
        return None;
    }
    Some((bpm / 60.0, label))
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
                clef: Some(ClefType::F),
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
