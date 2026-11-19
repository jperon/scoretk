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

//! Analyse d'une ligne de musique ABC.
//!
//! Le sous-ensemble couvert est celui de l'écrivain, plus ce qu'un air ABC
//! courant emploie : gammes majeures et mineures, rythme brisé, ornements
//! abrégés, paroles `w:`, reprises et volatas. Les constructions inconnues
//! produisent un avertissement et sont ignorées plutôt que de faire échouer la
//! lecture d'un air entier.

use scoretk_core::model::{
    Accidental, AccidentalType, Articulation, ArticulationType, BarLineType, ChordRest, Duration,
    DurationType, Fraction, GraceChord, GraceType, Note, NoteId, Ornament, OrnamentType,
};

use crate::reader::{AnnotationSpec, EventData, GraceData, Position, Reader, TupletSpec, Voice};
use scoretk_abc_common::decorations::{self, Decoration};
use scoretk_abc_common::duration;
use scoretk_abc_common::pitch;
use scoretk_abc_common::{Error, Result, Warning};

/// Analyse une ligne de musique pour une voix.
pub(crate) fn parse_line(reader: &mut Reader, voice: usize, text: &str, line: usize) -> Result<()> {
    reader.voices[voice].last_line_chords.clear();
    reader.voices[voice].next_verse = 0;
    let mut parser = Parser {
        reader,
        voice,
        line,
        chars: text.chars().collect(),
        pos: 0,
        last_length: String::new(),
    };
    parser.parse()
}

/// Analyseur d'une ligne.
struct Parser<'a> {
    /// État de lecture.
    reader: &'a mut Reader,
    /// Indice de la voix courante.
    voice: usize,
    /// Numéro de ligne.
    line: usize,
    /// Caractères de la ligne.
    chars: Vec<char>,
    /// Position dans la ligne.
    pos: usize,
    /// Dernière durée lue (après `read_note`).
    last_length: String,
}

impl Parser<'_> {
    /// Boucle principale.
    fn parse(&mut self) -> Result<()> {
        while let Some(character) = self.peek() {
            if character.is_whitespace() {
                self.pos += 1;
                continue;
            }
            match character {
                '"' => self.parse_annotation(),
                '!' | '+' => self.parse_decoration(),
                '[' => self.parse_bracket()?,
                '|' | ':' => self.parse_bar()?,
                '(' => self.parse_paren(),
                ')' => {
                    self.close_slur();
                    self.pos += 1;
                }
                '{' => self.parse_grace()?,
                '}' => self.pos += 1,
                'A'..='G' | 'a'..='g' | 'z' | 'Z' | 'x' | 'X' | '^' | '_' | '=' => {
                    self.parse_chord_or_note()?
                }
                '.' => {
                    self.push_decoration(Decoration::Articulation(ArticulationType::Staccato));
                    self.pos += 1;
                }
                // Symboles redéfinissables (§ 4.16) : H-W, h-w et `~`.
                'H'..='W' | 'h'..='w' | '~' => self.handle_user_symbol(character),
                // Guillemets de regroupement (§ 4.7 de la norme) : purement
                // typographiques, les programmes doivent les ignorer.
                '`' => self.pos += 1,
                '$' | '\\' | '*' | ']' => self.pos += 1,
                '&' => {
                    self.warn("superposition « & » ignorée");
                    self.pos += 1;
                }
                '<' | '>' => {
                    self.pos += 1;
                }
                other => {
                    return Err(Error::LigneInvalide {
                        line: self.line,
                        message: format!("caractère inattendu « {other} »"),
                    });
                }
            }
        }
        Ok(())
    }

    // --- accès de base ---------------------------------------------------

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek();
        if character.is_some() {
            self.pos += 1;
        }
        character
    }

    fn voice(&mut self) -> &mut Voice {
        &mut self.reader.voices[self.voice]
    }

    fn warn(&mut self, message: impl Into<String>) {
        self.reader.warnings.push(Warning::new(self.line, message));
    }

    // --- annotations et décorations --------------------------------------

    fn parse_annotation(&mut self) {
        self.pos += 1;
        let mut value = String::new();
        while let Some(character) = self.bump() {
            if character == '"' {
                break;
            }
            value.push(character);
        }
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return;
        }
        let (above, text) = match trimmed.chars().next() {
            Some('^') => (true, trimmed[1..].trim()),
            Some('_') => (false, trimmed[1..].trim()),
            Some('<') | Some('>') | Some('@') => (true, trimmed[1..].trim()),
            _ => (true, trimmed),
        };
        if text.is_empty() {
            return;
        }
        let harmony = looks_like_chord(text);
        self.voice().annotations.push(AnnotationSpec::Text {
            text: text.to_owned(),
            above,
            harmony,
        });
    }

    fn parse_decoration(&mut self) {
        let delimiter = self.bump();
        let mut name = String::new();
        while let Some(character) = self.bump() {
            if Some(character) == delimiter {
                break;
            }
            name.push(character);
        }
        let Some(decoration) = decorations::parse_decoration(&name) else {
            self.warn(format!("décoration « {name} » inconnue, ignorée"));
            return;
        };
        match decoration {
            Decoration::HairpinStart(kind) => {
                let position = self.voice().position();
                let voice = self.voice();
                voice.hairpins.push((position, position, kind));
                voice
                    .open_hairpins
                    .push((voice.hairpins.len() - 1, position));
            }
            Decoration::HairpinEnd => {
                let voice = self.voice();
                if let Some((index, start)) = voice.open_hairpins.pop() {
                    let position = voice_position(voice);
                    if let Some(entry) = voice.hairpins.get_mut(index) {
                        entry.0 = start;
                        entry.1 = position;
                    }
                }
            }
            other => self.push_decoration(other),
        }
    }

    fn push_decoration(&mut self, decoration: Decoration) {
        self.voice().decorations.push(decoration);
    }

    /// Traite un symbole redéfinissable (`U:` ou définition par défaut).
    fn handle_user_symbol(&mut self, symbol: char) {
        self.pos += 1;
        if let Some(name) = self.reader.user_symbols.get(&symbol).cloned() {
            if let Some(decoration) = decorations::parse_decoration(&name) {
                self.push_decoration(decoration);
            } else {
                self.warn(format!(
                    "décoration « {name} » du symbole « {symbol} » non prise en charge"
                ));
            }
            return;
        }
        let default = match symbol {
            'H' => Decoration::Articulation(ArticulationType::Fermata),
            'L' => Decoration::Articulation(ArticulationType::Accent),
            'M' => Decoration::Ornament(OrnamentType::Mordent),
            'P' => Decoration::Ornament(OrnamentType::InvertedMordent),
            'T' => Decoration::Ornament(OrnamentType::TrillMark),
            'u' => Decoration::Articulation(ArticulationType::UpBow),
            'v' => Decoration::Articulation(ArticulationType::DownBow),
            '~' => Decoration::Ornament(OrnamentType::Turn),
            _ => {
                self.warn(format!("symbole « {symbol} » non défini, ignoré"));
                return;
            }
        };
        self.push_decoration(default);
    }

    /// Ignore ou applique les symboles redéfinissables dans un accord ou un
    /// agrément (`read_note` est alors appelée sans passer par la boucle
    /// principale).
    fn consume_user_symbols(&mut self) {
        while let Some(symbol) = self.peek() {
            if matches!(symbol, 'H'..='W' | 'h'..='w' | '~') {
                self.handle_user_symbol(symbol);
            } else {
                break;
            }
        }
    }

    // --- accords, notes et silences --------------------------------------

    fn parse_chord_or_note(&mut self) -> Result<()> {
        let tick = self.voice().tick;
        if matches!(self.peek(), Some('Z' | 'X')) {
            self.bump();
            self.read_length_text();
            let remaining = {
                let voice = self.voice();
                let nominal = voice.time.measure_len();
                (nominal - voice.tick).max(Fraction::ZERO)
            };
            let actual = if remaining.is_positive() {
                remaining
            } else {
                Fraction::QUARTER
            };
            let nominal = voice_measure_len(self.voice());
            let mut chord_rest = ChordRest::full_measure_rest();
            if actual != nominal {
                chord_rest = ChordRest::rest(Duration::new(DurationType::Whole));
                chord_rest.ticks = Some(actual);
            }
            self.push_event(chord_rest, tick);
            self.voice().tick += actual;
            self.maybe_broken_rhythm();
            return Ok(());
        }
        if matches!(self.peek(), Some('z' | 'x')) {
            let visible = self.bump() == Some('z');
            let length_text = self.read_length_text();
            let nominal = self.parse_length(&length_text)?;
            let (nominal, actual) = self.apply_rhythm(nominal)?;
            let duration = duration::figure_for(nominal);
            let mut chord_rest = ChordRest::rest(duration.0);
            if !visible {
                if let Some(rest) = chord_rest.rest_mut() {
                    rest.visible = false;
                }
            }
            if actual != duration.0.ticks() {
                chord_rest.ticks = Some(actual);
            }
            self.push_event(chord_rest, tick);
            self.voice().tick += actual;
            self.maybe_broken_rhythm();
            return Ok(());
        }

        // Lecture des notes : un accord entre crochets, ou une note seule.
        if self.peek() == Some('[') {
            self.bump();
            let mut notes: Vec<(NoteId, u8, bool)> = Vec::new();
            loop {
                self.skip_spaces();
                match self.peek() {
                    Some(']') => {
                        self.bump();
                        break;
                    }
                    None => {
                        return Err(Error::LigneInvalide {
                            line: self.line,
                            message: "accord non fermé".to_owned(),
                        });
                    }
                    _ => {
                        let (note, _, _) = self.read_note()?;
                        let pitch = note.pitch;
                        let id = self.reader.score.add_note(note);
                        notes.push((id, pitch, false));
                    }
                }
            }
            let length_text = self.read_length_text();
            let nominal = self.parse_length(&length_text)?;
            let tie = self.eat_tie();
            if tie {
                for note in &mut notes {
                    note.2 = true;
                }
            }
            let (nominal, actual) = self.apply_rhythm(nominal)?;
            let duration = duration::figure_for(nominal);
            if notes.is_empty() {
                return Ok(());
            }
            let mut chord_rest = ChordRest::chord(duration.0);
            if let Some(chord) = chord_rest.chord_mut() {
                for (id, _, _) in &notes {
                    chord.notes.push(*id);
                }
            }
            if actual != duration.0.ticks() {
                chord_rest.ticks = Some(actual);
            }
            self.link_ties(&notes);
            let scale = self.tuplet_scale();
            self.push_event(chord_rest, tick);
            self.voice().tick += actual * scale;
            self.maybe_broken_rhythm();
            return Ok(());
        }

        let (note, pitch, tie) = self.read_note()?;
        let length_text = self.last_length.clone();
        let nominal = self.parse_length(&length_text)?;
        let (nominal, actual) = self.apply_rhythm(nominal)?;
        let duration = duration::figure_for(nominal);
        let id = self.reader.score.add_note(note);
        if let Some(previous) = self.voice().pending_ties.remove(&pitch) {
            self.reader.score.note_mut(previous).tie_forward = Some(id);
            self.reader.score.note_mut(id).tie_back = Some(previous);
        }
        if tie {
            self.voice().pending_ties.insert(pitch, id);
        }
        let mut chord_rest = ChordRest::chord(duration.0);
        if let Some(chord) = chord_rest.chord_mut() {
            chord.notes.push(id);
        }
        if actual != duration.0.ticks() {
            chord_rest.ticks = Some(actual);
        }
        let scale = self.tuplet_scale();
        self.push_event(chord_rest, tick);
        self.voice().tick += actual * scale;
        self.maybe_broken_rhythm();
        Ok(())
    }

    /// Lit une hauteur : altération, lettre, octave, durée.
    fn read_note(&mut self) -> Result<(Note, u8, bool)> {
        self.last_length.clear();
        // Les guillemets peuvent aussi apparaître entre les notes d'un accord
        // ou d'un agrément : ils sont ignorés comme dans la ligne principale.
        while self.peek() == Some('`') {
            self.pos += 1;
        }
        self.consume_user_symbols();
        let mut alter: Option<i32> = None;
        let mut explicit = AccidentalType::None;
        loop {
            match self.peek() {
                Some('^') => {
                    self.bump();
                    alter = Some(alter.unwrap_or(0) + 1);
                    explicit = if alter == Some(1) {
                        AccidentalType::Sharp
                    } else {
                        AccidentalType::DoubleSharp
                    };
                }
                Some('_') => {
                    self.bump();
                    alter = Some(alter.unwrap_or(0) - 1);
                    explicit = if alter == Some(-1) {
                        AccidentalType::Flat
                    } else {
                        AccidentalType::DoubleFlat
                    };
                }
                Some('=') => {
                    self.bump();
                    alter = Some(0);
                    explicit = AccidentalType::Natural;
                }
                _ => break,
            }
        }
        let letter = self.bump().ok_or(Error::LigneInvalide {
            line: self.line,
            message: "hauteur attendue".to_owned(),
        })?;
        if !letter.is_ascii_alphabetic() || !('A'..='G').contains(&letter.to_ascii_uppercase()) {
            return Err(Error::LigneInvalide {
                line: self.line,
                message: format!("hauteur « {letter} » invalide"),
            });
        }
        let letter = letter.to_ascii_uppercase();
        let mut octave = if self.chars[self.pos - 1].is_ascii_lowercase() {
            5
        } else {
            4
        };
        while let Some(mark) = self.peek() {
            match mark {
                '\'' => {
                    octave += 1;
                    self.pos += 1;
                }
                ',' => {
                    octave -= 1;
                    self.pos += 1;
                }
                _ => break,
            }
        }
        let text = self.read_length_text();
        self.last_length = text.clone();
        let tie = self.eat_tie();

        let absolute = {
            let voice = self.voice();
            let absolute = alter.unwrap_or_else(|| voice.accidental.current(letter));
            if alter.is_some() {
                voice.accidental.set(letter, absolute);
            }
            absolute
        };
        let pitch = (octave + 1) * 12 + natural_semitone(letter) + absolute;
        if !(0..=127).contains(&pitch) {
            return Err(Error::LigneInvalide {
                line: self.line,
                message: format!("hauteur hors de la plage MIDI : {pitch}"),
            });
        }
        let mut note = Note::new(pitch as u8, pitch::tpc_of(letter, absolute));
        if alter.is_some() {
            note.accidental = Some(Accidental::new(explicit));
        }
        Ok((note, pitch as u8, tie))
    }

    /// Lit le texte de durée après une note (`""`, `2`, `/2`, `3/2`).
    fn read_length_text(&mut self) -> String {
        let mut text = String::new();
        let mut slash = false;
        while let Some(character) = self.peek() {
            match character {
                '0'..='9' => {
                    text.push(character);
                    self.pos += 1;
                }
                '/' if !slash => {
                    slash = true;
                    text.push('/');
                    self.pos += 1;
                }
                _ => break,
            }
        }
        text
    }

    /// Consomme un tiret de prolongation.
    fn eat_tie(&mut self) -> bool {
        if self.peek() == Some('-') {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Convertit un texte de durée en fraction de ronde.
    fn parse_length(&self, text: &str) -> Result<Fraction> {
        let unit = self.reader.voices[self.voice].unit;
        duration::parse_multiplier(text, unit).ok_or_else(|| Error::DureeInvalide {
            line: self.line,
            text: text.to_owned(),
        })
    }

    /// Applique le rythme brisé en attente et le facteur de la note courante.
    fn apply_rhythm(&mut self, nominal: Fraction) -> Result<(Fraction, Fraction)> {
        let factor = self.voice().pending_rhythm.take().unwrap_or(Fraction::ONE);
        let nominal = nominal * factor;
        if !nominal.is_positive() {
            return Err(Error::DureeInvalide {
                line: self.line,
                text: "durée nulle".to_owned(),
            });
        }
        Ok((nominal, nominal))
    }

    // --- rythme brisé ----------------------------------------------------

    /// Consomme `<`/`>` et ajuste la note précédente et la suivante.
    fn maybe_broken_rhythm(&mut self) {
        let Some(symbol) = self.peek() else {
            return;
        };
        if symbol != '<' && symbol != '>' {
            return;
        }
        let mut levels = 0u32;
        while self.peek() == Some(symbol) {
            self.pos += 1;
            levels += 1;
        }
        levels = levels.min(2);
        let stronger = if symbol == '>' {
            Fraction::new((1 << (levels + 1)) - 1, 1 << levels)
        } else {
            Fraction::new(1, 1 << levels)
        };
        let weaker = if symbol == '>' {
            Fraction::new(1, 1 << levels)
        } else {
            Fraction::new((1 << (levels + 1)) - 1, 1 << levels)
        };
        let voice = &mut self.reader.voices[self.voice];
        if let Some(event) = voice.current.events.last_mut() {
            let notated = event.chord_rest.duration.ticks();
            let actual = event.chord_rest.ticks.unwrap_or(notated);
            if let Some(duration) = duration::dotted(event.chord_rest.duration, stronger)
                .or_else(|| duration::halved(event.chord_rest.duration, levels))
            {
                event.chord_rest.duration = duration;
            }
            let new_actual = actual * stronger;
            if new_actual != event.chord_rest.duration.ticks() {
                event.chord_rest.ticks = Some(new_actual);
            }
            if let Some(lyrics) = event.chord_rest.lyrics.last() {
                let _ = lyrics;
            }
        }
        voice.pending_rhythm = Some(weaker);
    }

    // --- événements ------------------------------------------------------

    /// Ajoute un événement à la mesure, décorations et paroles comprises.
    fn push_event(&mut self, mut chord_rest: ChordRest, tick: Fraction) {
        let decorations = std::mem::take(&mut self.voice().decorations);
        let mut annotations = std::mem::take(&mut self.voice().annotations);
        let mut grace = self.voice().grace.take();
        for decoration in decorations {
            match decoration {
                Decoration::Articulation(kind) => {
                    if let Some(chord) = chord_rest.chord_mut() {
                        chord.articulations.push(Articulation::new(kind));
                    }
                }
                Decoration::Ornament(kind) => {
                    if let Some(chord) = chord_rest.chord_mut() {
                        chord.ornaments.push(Ornament::new(kind));
                    }
                }
                Decoration::Dynamic(text) => annotations.push(AnnotationSpec::Dynamic(text)),
                Decoration::HairpinStart(_) | Decoration::HairpinEnd => {}
            }
        }
        if let Some(grace_data) = grace.take() {
            if let Some(chord) = chord_rest.chord_mut() {
                for (id, length, acciaccatura) in grace_data.notes {
                    let grace_type = if acciaccatura {
                        GraceType::Acciaccatura
                    } else {
                        GraceType::Appoggiatura
                    };
                    let mut grace_chord =
                        GraceChord::new(grace_type, duration::figure_for(length).0);
                    grace_chord.chord.notes.push(id);
                    chord.grace_chords.push(grace_chord);
                }
            }
        }
        let base_len = chord_rest.duration.ticks();
        let tuplet = self.assign_tuplet(base_len);
        let voice = &mut self.reader.voices[self.voice];
        let measure = voice.measure_index();
        let index = voice.current.events.len();
        voice.current.events.push(EventData {
            tick,
            chord_rest,
            tuplet,
            annotations,
        });
        voice.last_line_chords.push((measure, index));
    }

    /// Rattache une note à la liaison en attente et enregistre la sienne.
    fn link_ties(&mut self, notes: &[(NoteId, u8, bool)]) {
        for (id, pitch, tie) in notes {
            if let Some(previous) = self.voice().pending_ties.remove(pitch) {
                self.reader.score.note_mut(previous).tie_forward = Some(*id);
                self.reader.score.note_mut(*id).tie_back = Some(previous);
            }
            if *tie {
                self.voice().pending_ties.insert(*pitch, *id);
            }
        }
    }

    // --- n-olets ---------------------------------------------------------

    /// Facteur d'échelle de tous les n-olets ouverts.
    fn tuplet_scale(&self) -> Fraction {
        let voice = &self.reader.voices[self.voice];
        let mut scale = Fraction::ONE;
        for index in &voice.open_tuplets {
            if let Some(spec) = voice.tuplets.get(*index) {
                scale *= Fraction::new(spec.ratio.denominator(), spec.ratio.numerator());
            }
        }
        scale
    }

    /// Associe l'événement au n-olet le plus interne et décrémente les comptes.
    fn assign_tuplet(&mut self, base_len: Fraction) -> Option<usize> {
        let voice = &mut self.reader.voices[self.voice];
        let inner = voice.open_tuplets.last().copied()?;
        // Tous les n-olets ouverts dont la durée de base n'est pas encore
        // connue prennent celle de cette première note.
        for index in &voice.open_tuplets {
            if let Some(spec) = voice.tuplets.get_mut(*index) {
                if spec.base_len.is_zero() {
                    spec.base_len = base_len;
                    spec.len = spec.base_len * Fraction::new(spec.ratio.denominator(), 1);
                }
            }
        }
        // Seul le n-olet le plus interne compte chaque note ; un n-olet
        // englobant compte les groupes internes, pas leurs notes.
        let mut current = Some(inner);
        while let Some(index) = current {
            let parent = if let Some(spec) = voice.tuplets.get_mut(index) {
                spec.remaining -= 1;
                spec.parent
            } else {
                None
            };
            if voice
                .tuplets
                .get(index)
                .is_some_and(|spec| spec.remaining <= 0)
            {
                if let Some(position) = voice.open_tuplets.iter().position(|id| *id == index) {
                    voice.open_tuplets.remove(position);
                }
                current = parent;
            } else {
                break;
            }
        }
        Some(inner)
    }

    /// Ouvre un n-olet ou un slur.
    fn parse_paren(&mut self) {
        self.pos += 1;
        if !self.peek().is_some_and(|value| value.is_ascii_digit()) {
            let position = self.voice().position();
            self.voice().open_slurs.push(position);
            return;
        }
        let mut numbers = [0i64; 3];
        let mut count = 0usize;
        let mut current = String::new();
        while count < 3 {
            match self.peek() {
                Some(value) if value.is_ascii_digit() => {
                    current.push(value);
                    self.pos += 1;
                }
                Some(':') if count < 2 => {
                    numbers[count] = current.parse().unwrap_or(0);
                    current.clear();
                    count += 1;
                    self.pos += 1;
                }
                _ => break,
            }
        }
        numbers[count] = current.parse().unwrap_or(0);
        count += 1;
        let actual = numbers[0].max(1);
        let normal = if count > 1 && numbers[1] > 0 {
            numbers[1]
        } else {
            2
        };
        let notes = if count > 2 && numbers[2] > 0 {
            numbers[2]
        } else {
            actual
        };
        let ratio = Fraction::new(actual, normal);
        let parent = self
            .reader
            .voices
            .get(self.voice)
            .and_then(|voice| voice.open_tuplets.last().copied());
        let voice = &mut self.reader.voices[self.voice];
        let index = voice.tuplets.len();
        voice.tuplets.push(TupletSpec {
            position: voice.position(),
            ratio,
            base_len: Fraction::ZERO,
            len: Fraction::ZERO,
            parent,
            remaining: notes,
        });
        voice.open_tuplets.push(index);
    }

    fn close_slur(&mut self) {
        let end = self.voice().position();
        let voice = self.voice();
        if let Some(start) = voice.open_slurs.pop() {
            voice.slurs.push((start, end));
        }
    }

    // --- crochets : accord, volta ou champ en ligne ----------------------

    fn parse_bracket(&mut self) -> Result<()> {
        // Volta `[1` ou `[1,2`.
        if self.peek_at(1).is_some_and(|value| value.is_ascii_digit()) {
            self.pos += 1;
            if let Some(numbers) = self.read_volta_numbers() {
                self.voice().current.volta = Some(numbers);
            }
            return Ok(());
        }

        // Champ en ligne `[K:G]`, `[M:6/8]`, `[V:2]`…
        if let Some(letter) = self.peek_at(1) {
            if letter.is_ascii_alphabetic() && self.peek_at(2) == Some(':') {
                let mut content = String::new();
                self.pos += 1;
                while let Some(character) = self.bump() {
                    if character == ']' {
                        break;
                    }
                    content.push(character);
                }
                if let Some((name, value)) = content.split_once(':') {
                    let letter = name.chars().next().unwrap_or(' ');
                    self.reader.handle_field(letter, value.trim(), self.line)?;
                    self.voice = self.reader.current;
                }
                return Ok(());
            }
        }
        // Le crochet n'a pas encore été consommé.
        self.pos += 1;
        self.parse_chord()
    }

    fn parse_chord(&mut self) -> Result<()> {
        // Le crochet a déjà été consommé par `parse_bracket`.
        let mut notes = Vec::new();
        loop {
            self.skip_spaces();
            match self.peek() {
                Some(']') => {
                    self.bump();
                    break;
                }
                None => {
                    return Err(Error::LigneInvalide {
                        line: self.line,
                        message: "accord non fermé".to_owned(),
                    });
                }
                _ => {
                    let (note, _, _) = self.read_note()?;
                    notes.push(note);
                }
            }
        }
        let length_text = self.read_length_text();
        let nominal = self.parse_length(&length_text)?;
        let tie = self.eat_tie();
        let (nominal, _) = self.apply_rhythm(nominal)?;
        let duration = duration::figure_for(nominal);
        let tick = self.voice().tick;
        let mut chord_rest = ChordRest::chord(duration.0);
        let mut ids = Vec::new();
        let mut pitches = Vec::new();
        for note in notes {
            let pitch = note.pitch;
            let id = self.reader.score.add_note(note);
            if let Some(chord) = chord_rest.chord_mut() {
                chord.notes.push(id);
            }
            ids.push((id, pitch, tie));
            pitches.push(pitch);
        }
        self.link_ties(&ids);
        let scale = self.tuplet_scale();
        self.push_event(chord_rest, tick);
        self.voice().tick += nominal * scale;
        self.maybe_broken_rhythm();
        Ok(())
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    // --- barres de mesure ------------------------------------------------

    fn parse_bar(&mut self) -> Result<()> {
        let mut end = BarLineType::Normal;
        let mut start_repeat = false;
        match self.peek() {
            Some('|') => {
                self.bump();
                match self.peek() {
                    Some('|') => {
                        self.bump();
                        end = BarLineType::Double;
                    }
                    Some(']') => {
                        self.bump();
                        end = BarLineType::End;
                    }
                    Some(':') => {
                        self.bump();
                        start_repeat = true;
                    }
                    _ => {}
                }
            }
            Some(':') => {
                self.bump();
                if self.peek() == Some('|') {
                    self.bump();
                    end = BarLineType::EndRepeat;
                } else if self.peek() == Some(':') {
                    self.bump();
                    end = BarLineType::EndStartRepeat;
                    start_repeat = true;
                } else {
                    end = BarLineType::EndRepeat;
                }
            }
            _ => {}
        }
        // Norme 2.1 § 4.9 : `|1` et `:|2` abrègent `|[1` et `:[2`.
        let volta = self.read_volta_numbers();
        self.finish_bar(end, start_repeat, volta)
    }

    /// Termine la mesure sur une barre, avec une volta éventuelle.
    fn finish_bar(
        &mut self,
        end: BarLineType,
        start_repeat: bool,
        volta: Option<Vec<u8>>,
    ) -> Result<()> {
        let voice = &mut self.reader.voices[self.voice];
        let empty = voice.current.events.is_empty() && voice.tick.is_zero();
        if empty {
            if let Some(last) = voice.measures.last_mut() {
                last.end_bar = end;
            }
            if start_repeat {
                voice.current.start_bar = BarLineType::StartRepeat;
            }
            voice.current.volta = volta;
            return Ok(());
        }
        let measure = voice.measure_index();
        if let Some(numbers) = voice.current.volta.clone() {
            voice.voltas.push((measure, measure, numbers));
        }
        voice.finish_measure(end);
        self.reader.voices[self.voice].current.volta = volta;
        if start_repeat {
            self.reader.voices[self.voice].next_start_bar = BarLineType::StartRepeat;
        }
        Ok(())
    }

    /// Lit une liste de numéros de volta (`1`, `1,2`, `1-3`) après une barre.
    fn read_volta_numbers(&mut self) -> Option<Vec<u8>> {
        let debut = self.pos;
        let mut numbers = Vec::new();
        let mut current = String::new();
        while let Some(character) = self.peek() {
            if character.is_ascii_digit() {
                current.push(character);
                self.pos += 1;
            } else if character == ',' || character == '-' {
                if let Ok(value) = current.parse::<u8>() {
                    numbers.push(value);
                }
                current.clear();
                self.pos += 1;
            } else {
                break;
            }
        }
        if let Ok(value) = current.parse::<u8>() {
            numbers.push(value);
        }
        if self.pos == debut {
            None
        } else {
            Some(numbers)
        }
    }

    // --- agréments -------------------------------------------------------

    fn parse_grace(&mut self) -> Result<()> {
        self.pos += 1;
        let mut notes = Vec::new();
        loop {
            self.skip_spaces();
            // La barre oblique marque une acciaccatura ; elle peut précéder
            // chaque note du groupe, la norme ne la réservant pas à la
            // première.
            let mut acciaccatura = false;
            if self.peek() == Some('/') {
                acciaccatura = true;
                self.pos += 1;
            }
            match self.peek() {
                Some('}') => {
                    self.bump();
                    break;
                }
                None => {
                    return Err(Error::LigneInvalide {
                        line: self.line,
                        message: "agrément non fermé".to_owned(),
                    });
                }
                _ => {
                    let (note, _, _) = self.read_note()?;
                    let length_text = self.last_length.clone();
                    let length = self.parse_length(&length_text)?;
                    let id = self.reader.score.add_note(note);
                    notes.push((id, length, acciaccatura));
                }
            }
        }
        match &mut self.voice().grace {
            Some(existing) => existing.notes.extend(notes),
            None => self.voice().grace = Some(GraceData { notes }),
        }
        Ok(())
    }
}

/// Longueur d'une mesure en cours (nominale), ou zéro.
fn voice_measure_len(voice: &Voice) -> Fraction {
    voice.time.measure_len()
}

/// Position courante d'une voix.
fn voice_position(voice: &Voice) -> Position {
    Position {
        measure: voice.measure_index(),
        tick: voice.tick,
    }
}

/// Demi-ton naturel d'une lettre (do = 0).
fn natural_semitone(letter: char) -> i32 {
    match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        _ => 11,
    }
}

/// Vrai si un texte ressemble à un accord chiffré (`C`, `Am7`, `F/A`…).
fn looks_like_chord(text: &str) -> bool {
    let mut characters = text.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if !('A'..='G').contains(&first) {
        return false;
    }
    let rest: String = characters.collect();
    if rest.len() > 12 {
        return false;
    }
    rest.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || matches!(
                character,
                '#' | 'b' | '/' | '+' | '-' | '(' | ')' | ',' | '\'' | '°' | 'ø' | '∆'
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{OrnamentType, SpannerType, StaffId, Track};

    fn parse(music: &str) -> crate::reader::ReadOutput {
        let source = format!("X:1\nT:Test\nM:4/4\nL:1/8\nK:C\n{music}\n");
        crate::reader::read_abc_with(&source, &crate::reader::ReadOptions::default())
            .expect("lecture")
    }

    fn chord_rest(score: &scoretk_core::model::Score, measure: usize, index: usize) -> &ChordRest {
        score.measures[measure]
            .segments
            .iter()
            .filter_map(|segment| segment.chord_rest(Track::new(StaffId::from_index(0), 0)))
            .nth(index)
            .expect("accord")
    }

    /// Hauteurs et durées des accords d'une voix, pour comparer deux lectures.
    fn contenu(output: &crate::reader::ReadOutput) -> (Vec<u8>, Vec<f64>) {
        let mut pitches = Vec::new();
        let mut durees = Vec::new();
        for measure in &output.score.measures {
            for segment in &measure.segments {
                if let Some(chord_rest) =
                    segment.chord_rest(Track::new(StaffId::from_index(0), 0))
                {
                    if let Some(chord) = chord_rest.chord_ref() {
                        pitches.extend(
                            chord
                                .notes
                                .iter()
                                .map(|id| output.score.note(*id).pitch),
                        );
                    }
                    durees.push(chord_rest.duration.ticks().as_f64());
                }
            }
        }
        (pitches, durees)
    }

    /// Numéros de voltas du modèle.
    fn voltas(output: &crate::reader::ReadOutput) -> Vec<Vec<u8>> {
        output
            .score
            .spanners
            .iter()
            .filter(|spanner| matches!(spanner.kind, SpannerType::Volta))
            .filter_map(|spanner| spanner.volta.as_ref().map(|volta| volta.endings.clone()))
            .collect()
    }

    #[test]
    fn symboles_redefiniables_et_non_definis() {
        let source = "\
X:1
T:Symbols
M:4/4
L:1/8
K:C
U:k = !trill!
V:1
kvC2 (DE) [GB]2 z2 | k^d2 e2 f2 g2 | C D k |]
V:2
C, D, E, F, | qG, A, B, c |]
";
        let output = crate::reader::read_abc_with(source, &crate::reader::ReadOptions::default())
            .expect("lecture");
        assert_eq!(output.score.measures.len(), 3);
        assert_eq!(
            contenu(&output),
            (
                vec![60, 62, 64, 67, 71, 75, 76, 77, 79, 60, 62],
                vec![
                    0.25, 0.125, 0.125, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.125, 0.125
                ]
            )
        );
        assert_eq!(output.score.notes.len(), 19, "les deux voix sont lues");
        let trilles = output
            .score
            .measures
            .iter()
            .flat_map(|measure| &measure.segments)
            .filter_map(|segment| {
                segment.chord_rest(Track::new(StaffId::from_index(0), 0))
            })
            .filter_map(|chord_rest| chord_rest.chord_ref())
            .flat_map(|chord| chord.ornaments.iter())
            .filter(|ornament| ornament.kind == OrnamentType::TrillMark)
            .count();
        assert_eq!(trilles, 2, "U:k = !trill! est appliqué");
        assert!(
            output
                .warnings
                .iter()
                .any(|warning| warning.message.contains('q')),
            "le symbole non défini « q » doit avertir : {:?}",
            output.warnings
        );
    }

    #[test]
    fn voltas_inline_sans_crochet() {
        let inline = parse("|: C D |1 E F :|2 G A |]");
        let crochets = parse("|: C D |[1 E F :|[2 G A |]");
        assert_eq!(inline.score.measures.len(), 3);
        assert_eq!(
            contenu(&inline),
            (vec![60, 62, 64, 65, 67, 69], vec![0.125, 0.125, 0.125, 0.125, 0.125, 0.125])
        );
        assert_eq!(voltas(&inline), vec![vec![1], vec![2]]);
        assert!(inline.score.measures[0].start_bar_line.is_repeat_start());
        assert!(inline.score.measures[1].end_bar_line.is_repeat_end());

        // La forme abrégée doit donner exactement la même musique que `[1`.
        assert_eq!(contenu(&inline), contenu(&crochets));
        assert_eq!(voltas(&inline), voltas(&crochets));

        // Cas limite : plusieurs numéros, ici sans note après la barre finale.
        let multiple = parse("C D |1,2 E F |]");
        assert_eq!(voltas(&multiple), vec![vec![1, 2]]);
    }

    #[test]
    fn backquotes_entre_notes_ignorees() {
        let avec = parse("C2-C2``D``E``F``G | [CEG]4``z4 | A4``A4 |]");
        let sans = parse("C2-C2D E F G | [CEG]4z4 | A4A4 |]");
        assert_eq!(avec.score.measures.len(), 3);
        assert_eq!(
            contenu(&avec),
            (
                vec![60, 60, 62, 64, 65, 67, 60, 64, 67, 69, 69],
                vec![0.25, 0.25, 0.125, 0.125, 0.125, 0.125, 0.5, 0.5, 0.5, 0.5]
            )
        );
        // Sans les guillemets, la musique ordinaire doit rester identique.
        assert_eq!(contenu(&avec), contenu(&sans));

        // Cas limite : guillemets en fin de ligne, juste avant une barre.
        let fin = parse("C D`` |");
        assert_eq!(fin.score.measures.len(), 1);
        assert_eq!(contenu(&fin).0, vec![60, 62]);
    }

    #[test]
    fn note_simple() {
        let output = parse("C D E F |");
        let measure = &output.score.measures[0];
        assert_eq!(
            measure
                .segments
                .iter()
                .filter(|segment| segment
                    .chord_rest(Track::new(StaffId::from_index(0), 0))
                    .is_some())
                .count(),
            4
        );
    }

    #[test]
    fn alterations_et_octaves() {
        let output = parse("^C _E =F | c' C, |");
        let pitches: Vec<u8> = output.score.notes.iter().map(|note| note.pitch).collect();
        assert_eq!(pitches, vec![61, 63, 65, 84, 48]);
    }

    #[test]
    fn accords() {
        let output = parse("[CEG]2 |");
        assert_eq!(
            chord_rest(&output.score, 0, 0)
                .chord_ref()
                .expect("accord")
                .notes
                .len(),
            3
        );
    }

    #[test]
    fn tuplets_et_rythme_brise() {
        let output = parse("(3 C D E F>G A |");
        assert!(output.score.measures[0].segments.len() >= 5);
        assert_eq!(output.score.tuplets.len(), 1);
    }

    #[test]
    fn reprises_et_voltas() {
        let output = parse("|: C D | [1 E F :| [2 G A |]");
        assert!(output.score.measures.len() >= 3);
        assert!(output
            .score
            .measures
            .iter()
            .any(|measure| measure.start_bar_line.is_repeat_start()));
    }
}
