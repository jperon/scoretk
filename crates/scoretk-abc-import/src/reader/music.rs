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
    DurationType, Fermata, FermataType, Fraction, GraceChord, GraceType, Note, NoteId, Ornament,
    OrnamentType, Slur, Spanner, SpannerAnchor, SpannerNotation, SpannerType, Track, Tremolo,
    TremoloKind,
};

use crate::reader::{
    rhythm, AnnotationSpec, EventData, GraceData, Position, Reader, TupletSpec, Voice, VoiceFields,
};
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
                // `=20` après une barre est un numéro de mesure ; comme
                // l'élément `errors` de `abc2xml.py` (ligne 431), on le signale
                // et on le saute au lieu d'essayer d'en faire une note.
                '=' if self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) => {
                    self.warn("numéro de mesure « =… » ignoré");
                    self.pos += 1;
                    while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                        self.pos += 1;
                    }
                }
                // Une altération en fin de ligne, sans note après elle, est
                // sautée localement comme le `errors` d'`abc2xml.py` (ligne 431).
                '^' | '_' | '=' if !self.accidental_has_note() => {
                    self.warn("altération sans note, ignorée");
                    self.pos += 1;
                }
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
                    // Port de `abc2xml.py`, `voice_overlay` (ligne 425) et
                    // `mergePartMeasure` (ligne 951) : `&` ouvre une voix
                    // supplémentaire sur la même portée, au même tick. Sans
                    // cela les notes superposées s'enchaînaient et toutes les
                    // attaques après la première étaient décalées.
                    self.pos += 1;
                    self.start_overlay();
                }
                '<' | '>' => {
                    self.pos += 1;
                }
                // Port de `abc2xml.py` 269, `basenote` (ligne 367) et
                // `noteActn` (ligne 549) : `y` est un espacement typographique,
                // jamais une note ; il est écarté avec sa durée.
                'y' => {
                    self.warn("espacement « y » ignoré");
                    self.pos += 1;
                }
                // Port de `abc2xml.py::tie` (ligne 380, `oneOf('.- -')`) : un
                // tiret séparé de la note par un espace, un rythme brisé ou un
                // agrément reste une liaison, pas une erreur de lecture.
                '-' => self.tie_last_note(),
                // Port de l'élément `errors` de la grammaire d'`abc2xml.py`
                // (ligne 431, `pas(r'[^:&|]')` avec `errorWarn`) : un symbole
                // égaré est signalé et sauté localement. Un air entier n'est
                // jamais rejeté pour un tel caractère.
                other => {
                    self.warn(format!("caractère égaré « {other} » ignoré"));
                    self.pos += 1;
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
        let prefixed = matches!(trimmed.chars().next(), Some('^' | '_' | '<' | '>' | '@'));
        let (above, text) = match trimmed.chars().next() {
            Some('^') => (true, trimmed[1..].trim()),
            Some('_') => (false, trimmed[1..].trim()),
            Some('<') | Some('>') | Some('@') => (true, trimmed[1..].trim()),
            _ => (true, trimmed),
        };
        // Un texte vide après un préfixe de placement (`">"`) est une
        // annotation à part entière : `abc2xml.py` (`text_expression`, lignes
        // 402/487, `convert` ligne 2022) écrit un `<words/>` vide. Leport
        // la supprimait silencieusement.
        // An explicit placement prefix forces `text_expression` in
        // `abc2xml.py:415`; `"^F"` is a text annotation, not an F chord.
        let harmony = !prefixed && looks_like_chord(text);
        self.voice().annotations.push(AnnotationSpec::Text {
            text: text.to_owned(),
            above,
            harmony,
        });
    }

    fn parse_decoration(&mut self) {
        let delimiter = self.bump().expect("délimiteur de décoration");
        // Portage de `abc2xml.py` 269, grammaire `long_decoration` (ligne
        // 374) et `linebreak` (ligne 432) : un nom de décoration ne contient
        // ni espace ni l'autre délimiteur, et doit être fermé. Sinon le `!`
        // (ou `+`) est un saut de ligne et la musique qui suit est lue
        // normalement ; l'ancien code avalait toute la fin de la ligne.
        let ferme = self.chars[self.pos..]
            .iter()
            .position(|&c| c == delimiter || c.is_whitespace() || c == '!' || c == '+');
        if !ferme.is_some_and(|index| self.chars[self.pos + index] == delimiter) {
            self.warn("décoration non fermée, traitée comme saut de ligne".to_owned());
            return;
        }
        let mut name = String::new();
        while let Some(character) = self.bump() {
            if character == delimiter {
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
            // A direction such as `!D.C.!` is often written after the last
            // note of a measure. Decorations are only flushed by `push_event`,
            // so route text directions to annotations, which `finish_bar`
            // attaches to the last event of the measure.
            Decoration::Text(text) => self.voice().annotations.push(AnnotationSpec::Text {
                text,
                above: true,
                harmony: false,
            }),
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
            let length = self.read_length_text();
            // Port de `abc2xml.py::fixSlurs.f` (lignes 767-775) : `Z24` est
            // réécrit en 24 pauses de mesure `Z|Z|…|Z`. Le nombre compte des
            // mesures, il n'allonge pas une pause unique.
            let repetitions = length.parse::<usize>().unwrap_or(1).max(1);
            for iteration in 0..repetitions {
                let tick = self.voice().tick;
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
                // Chaque pause remplie ferme sa mesure ; la dernière est
                // fermée par la barre écrite qui suit le `Znn`.
                if iteration + 1 < repetitions {
                    self.finish_bar(BarLineType::Normal, false, None)?;
                }
            }
            // Un marqueur après une pause de mesure est sans effet sur une
            // pause pleine ; on le consomme et on vide le facteur en attente.
            let _ = self.read_broken_factors();
            self.voice().pending_rhythm = None;
            return Ok(());
        }
        if matches!(self.peek(), Some('z' | 'x')) {
            let visible = self.bump() == Some('z');
            let length_text = self.read_length_text();
            let nominal = self.parse_length(&length_text)?;
            let (nominal, actual) = self.apply_rhythm(nominal)?;
            let duration = duration::figure_for(nominal);
            // An explicit `z` covering a whole measure is the same musical
            // object as `Z`: keep the measure-rest figure so the model does
            // not gain a spurious `<type>`/`<dot>` on re-read. Its written
            // duration still comes from the explicit multiplier via
            // `chord_rest.ticks`/measure length.
            let mut chord_rest = if actual == self.voice().time.measure_len() && tick.is_zero() {
                ChordRest::full_measure_rest()
            } else {
                ChordRest::rest(duration.0)
            };
            if !visible {
                if let Some(rest) = chord_rest.rest_mut() {
                    rest.visible = false;
                }
            }
            if actual != duration.0.ticks() {
                chord_rest.ticks = Some(actual);
            }
            // A rest inside a tuplet advances the tick by the same scaled
            // duration as a note: ABC 2.1 §4.13 and `abc2xml.py::mkNote`
            // (lines 1306, 1366) count and scale rests like notes. Without
            // this, `(2:3ez/2fz/2` in 12/8 is read as 7/16 instead of 15/32.
            let scale = self.tuplet_scale();
            self.push_event(chord_rest, tick);
            self.voice().tick += actual * scale;
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
                    // Port de `abc2xml.py`, `chord_note = dec_note | rest | b1`
                    // (lignes 383-385) : une décoration peut précéder chaque
                    // note d'un accord (`[.e.g]`, `[C(E]`). On la signale et
                    // on la saute, comme le fait `errorWarn` pour `b1`.
                    Some('z' | 'Z' | 'x' | 'X') => {
                        // Port de `chord_note = dec_note | rest | b1` (ligne 385) :
                        // un silence peut figurer dans un accord ; il n'a pas
                        // de hauteur, mais il emporte sa durée écrite.
                        self.warn("silence sans hauteur dans un accord ignoré");
                        self.pos += 1;
                        let _ = self.read_length_text();
                    }
                    // ABC 2.1 §4.11: slurs may begin/end inside chords;
                    // `abc2xml.py:371-387` allows `slur_beg` as a chord
                    // note decoration. The old code dropped these.
                    Some('(') => self.parse_paren(),
                    Some(')') => {
                        self.close_slur();
                        self.pos += 1;
                    }
                    Some('.' | '!' | '+' | '~' | ',' | '`' | '$' | '<' | '>' | '"') => {
                        self.warn("symbole égaré dans un accord ignoré");
                        self.pos += 1;
                    }
                    // Un accord coupé par une fin de ligne n'est pas une
                    // raison de rejeter tout l'air : on le referme ici.
                    None => {
                        self.warn("accord non fermé, traité comme fermé");
                        break;
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
            return Ok(());
        }

        let (note, pitch, tie) = self.read_note()?;
        let length_text = self.last_length.clone();
        let nominal = self.parse_length(&length_text)?;
        let (nominal, actual) = self.apply_rhythm(nominal)?;
        let duration = duration::figure_for(nominal);
        let id = self.reader.score.add_note(note);
        self.drop_foreign_ties(&[pitch]);
        if let Some((previous, _)) = self.voice().pending_ties.remove(&pitch) {
            self.reader.score.note_mut(previous).tie_forward = Some(id);
            self.reader.score.note_mut(id).tie_back = Some(previous);
        }
        if tie {
            let position = self.voice().position();
            self.voice().pending_ties.insert(pitch, (id, position));
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
        let mut microtonal = false;
        loop {
            match self.peek() {
                // Une fraction Microtonale (`^3^F`, `^/C`) appartient au
                // même jeton que le premier signe : abc2xml.py
                // (`alteration` 368-369, `decTupAcc` 1241-1253) ignore la
                // fraction non supportée et garde UN seul dièse/bémol.
                // Additionner le second `^` donnait F## (= G) au lieu de F#.
                Some('^' | '_' | '=') if microtonal => {
                    self.bump();
                }
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
                // Port de `abc2xml.py`, `alteration` (ligne 368) : le
                // numérateur/le dénominateur d'une altération microtonale
                // (`^3^F`, `^/C`) appartient au même jeton. Comme le script
                // avec `edo=12`, on garde le demi-ton déjà lu.
                Some(value) if value.is_ascii_digit() || value == '/' => {
                    microtonal = true;
                    self.pos += 1;
                }
                _ => break,
            }
        }
        // Micro-intervalles (`^/C`, `_/c`, `^3/2C`…) : le modèle n'a pas de
        // hauteur fractionnaire. Comme abc2xml.py avec edo=12, on arrondit au
        // demi-ton lu par `^`/`_` et on saute localement la fraction.
        if microtonal
            || self.peek() == Some('/')
            || (self.peek().is_some_and(|value| value.is_ascii_digit())
                && self.peek_at(1) == Some('/'))
        {
            while self.peek().is_some_and(|value| value.is_ascii_digit()) {
                self.pos += 1;
            }
            while self.peek() == Some('/') {
                self.pos += 1;
            }
            while self.peek().is_some_and(|value| value.is_ascii_digit()) {
                self.pos += 1;
            }
            self.warn("micro-intervalle arrondi au demi-ton le plus proche");
        }
        // Port de l'élément `errors` d'`abc2xml.py` (ligne 431) : un chiffre
        // orphelin après une altération (`^3^F`) ou un rythme brisé est
        // signalé et sauté localement, comme le reste du nombre d'altération.
        while self
            .peek()
            .is_some_and(|character| character.is_ascii_digit())
        {
            self.warn("chiffre égaré avant une hauteur, ignoré");
            self.pos += 1;
        }
        // Port de la grammaire insensible aux espaces d'`abc2xml.py`
        // (`abc_voice.zonderWit()`, ligne 505) : `= (ed)` sur deux lignes
        // jointes est une altération suivie de la note `e`, pas une erreur.
        self.skip_spaces();
        // Port de `abc2xml.py`, `dec_note = Optional(decorations) + pitch`
        // (lignes 377/384) : un phrasé ou un staccato peut séparer l'altération
        // de la note (`= (ed)`). Ces décorations sont signalées et sautées.
        while matches!(self.peek(), Some('(' | ')' | '.')) {
            // `abc2xml.py:377,384` (`dec_note = decorations + pitch`): a
            // slur can sit between an accidental and its note, including
            // inside a chord (`[E4(e4]`, ABC 2.1 §4.11). Honour it instead
            // of silently dropping the phrase mark.
            match self.peek() {
                Some('(') => self.parse_paren(),
                Some(')') => {
                    self.close_slur();
                    self.pos += 1;
                }
                _ => self.pos += 1,
            }
            self.skip_spaces();
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
        // La hauteur lue est la hauteur **écrite** ; le modèle porte la
        // hauteur **réelle** (écrite + transposition), comme `abc2xml.py` qui
        // écrit `<pitch>` tel quel et déclare l'écart dans `<transpose>`.
        let written = (octave + 1) * 12 + natural_semitone(letter) + absolute;
        let transpose = self.voice().transpose;
        let pitch = written + transpose;
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

    /// Lit le texte de durée après une note (`""`, `2`, `/2`, `3/2`, `e//`).
    ///
    /// Port de la grammaire `note_length` d'`abc2xml.py` (ligne 362) : un
    /// nombre, TOUS les slashs, puis un nombre. Un seul slash consommé laisse-
    /// rait `//` dans le flux et ferait échouer la lecture d'un air réel.
    fn read_length_text(&mut self) -> String {
        let mut text = String::new();
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            text.push(self.bump().expect("chiffre"));
        }
        while self.peek() == Some('/') {
            text.push(self.bump().expect("slash"));
        }
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            text.push(self.bump().expect("chiffre"));
        }
        text
    }

    /// Consomme un tiret de prolongation.
    ///
    /// Port de `abc2xml.py::tie` (ligne 380, `oneOf('.- -')`) : la forme `.-`
    /// est un seul tiret, pas un staccato suivi d'une erreur.
    fn eat_tie(&mut self) -> bool {
        if self.peek() == Some('-') {
            self.pos += 1;
            true
        } else if self.peek() == Some('.') && self.peek_at(1) == Some('-') {
            self.pos += 2;
            true
        } else {
            false
        }
    }

    /// Multiple de l'unité `L:` lu par la grammaire `note_length`.
    fn parse_mult(&self, text: &str) -> Result<Fraction> {
        rhythm::note_length(text).ok_or_else(|| Error::DureeInvalide {
            line: self.line,
            text: text.to_owned(),
        })
    }

    /// Convertit un texte de durée en fraction de ronde.
    ///
    /// Port de l'action `note_length` d'`abc2xml.py` (lignes 362/473) :
    /// `rhythm::note_length` calcule le multiple de l'unité `L:`, qu'on
    /// multiplie ici par cette unité pour obtenir une fraction de ronde.
    fn parse_length(&self, text: &str) -> Result<Fraction> {
        Ok(self.reader.voices[self.voice].unit * self.parse_mult(text)?)
    }

    // --- rythme brisé ----------------------------------------------------

    /// Consomme un éventuel marqueur `<`/`>` placé après la durée lue et
    /// renvoie `(facteur pour cette note, facteur pour la suivante)`.
    ///
    /// Port de `abc2xml.py::convertBroken` (ligne 614) : dans le script, la
    /// transformation parcourt la mesure et, pour chaque `broken`, applique
    /// `doBroken(prev, brk, x)` (ligne 594) à la note précédente ET à la
    /// suivante. Ici, le marqueur suit toujours la note qu'il modifie : on
    /// applique immédiatement son facteur à la durée lue et on garde le
    /// facteur de la note suivante dans `pending_rhythm`. La durée définitive
    /// est ainsi connue AVANT de pousser l'événement, ce qui garde le curseur
    /// (`voice.tick`) aligné sur les attaques réelles — contrairement à une
    /// correction après coup qui laissait la mesure courte et décalait tout
    /// le reste de l'air (1 102 airs perdus en N2 après le port de `doMaat`).
    fn read_broken_factors(&mut self) -> (Fraction, Fraction) {
        let sauvegarde = self.pos;
        self.skip_spaces();
        let Some(symbol @ ('<' | '>')) = self.peek() else {
            self.pos = sauvegarde;
            return (Fraction::ONE, Fraction::ONE);
        };
        let mut niveaux = 0usize;
        while self.peek() == Some(symbol) {
            self.pos += 1;
            niveaux += 1;
        }
        rhythm::broken_factors(symbol, niveaux)
    }

    /// Applique le rythme brisé en attente et le facteur de la note courante.
    fn apply_rhythm(&mut self, nominal: Fraction) -> Result<(Fraction, Fraction)> {
        // Facteur laissé par le marqueur qui suivait la note précédente, puis
        // facteur du marqueur qui suit celle-ci (consommé par `read_broken_factors`).
        let attente = self.voice().pending_rhythm.take().unwrap_or(Fraction::ONE);
        let (ici, suivant) = self.read_broken_factors();
        self.voice().pending_rhythm = Some(suivant);
        let nominal = nominal * attente * ici;
        if !nominal.is_positive() {
            return Err(Error::DureeInvalide {
                line: self.line,
                text: "durée nulle".to_owned(),
            });
        }
        Ok((nominal, nominal))
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
                    } else if kind == ArticulationType::Fermata {
                        // ABC 2.1 §4.14 (`abc2xml.py::doDecos`, lignes
                        // 1491-1492): `H`/`!fermata!` before a rest is a
                        // fermata on that rest. The pivot model has no
                        // articulation on `Rest`, but `rest_notations`
                        // writes `<fermata>` from a segment annotation.
                        annotations
                            .push(AnnotationSpec::Fermata(Fermata::new(FermataType::Normal)));
                    }
                }
                Decoration::Ornament(kind) => {
                    if let Some(chord) = chord_rest.chord_mut() {
                        chord.ornaments.push(Ornament::new(kind));
                    }
                }
                Decoration::Tremolo(count) => {
                    // `abc2xml.py:1464-1469` writes a single-note tremolo on
                    // the note carrying `!/!` (`abc2xml.py:1688`).
                    if let Some(chord) = chord_rest.chord_mut() {
                        chord.tremolo = Some(Tremolo {
                            kind: TremoloKind::Single,
                            count,
                        });
                    }
                }
                Decoration::Dynamic(text) => annotations.push(AnnotationSpec::Dynamic(text)),
                Decoration::Text(text) => annotations.push(AnnotationSpec::Text {
                    text,
                    above: true,
                    harmony: false,
                }),
                Decoration::HairpinStart(_) | Decoration::HairpinEnd => {}
            }
        }
        if let Some(grace_data) = grace.take() {
            if let Some(chord) = chord_rest.chord_mut() {
                // Port de `abc2xml.py::doGrace` (lignes 680-686) : les notes
                // d'une suite `{}` sont toutes marquées comme agréments et
                // comptées (`numNts`) ; `mkNote` (lignes 1314-1315) leur donne
                // ensuite une unité fixe de 1/8 pour une note seule, 1/16 pour
                // plusieurs. Le modèle pivot conserve ici la durée ÉCRITE du
                // jeton (comme il conserve la figure écrite d'un n-olet) :
                // c'est l'exporteur qui décide de la durée d'exécution, et
                // l'écrivain ABC réutilise cette valeur pour la réécrire. Les
                // agréments ne déplacent pas le curseur, comme dans le script.
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

    /// Ouvre une voix superposée sur la portée de la voix courante.
    ///
    /// Port de `abc2xml.py::voice_overlay` (ligne 425) et `mergePartMeasure`
    /// (ligne 951) : une note après `&` attaque au même tick que la première
    /// note du groupe, sur une voix distincte de la même portée. L'export en
    /// tire un `<backup>` au lieu d'enchaîner les notes.
    fn start_overlay(&mut self) {
        let root = self
            .reader
            .voices
            .get(self.voice)
            .and_then(|voice| voice.overlay_of)
            .unwrap_or(self.voice);
        let (unit, time, key, clef, transpose, tick, measures) = {
            let voice = &self.reader.voices[root];
            let tick = voice
                .current
                .events
                .first()
                .map_or(voice.tick, |event| event.tick);
            (
                voice.unit,
                voice.time.clone(),
                voice.key.clone(),
                voice.clef,
                voice.transpose,
                tick,
                voice.measures.len(),
            )
        };
        // Le modèle n'a que quatre voix par portée : on réutilise les trois
        // voix de superposition d'une mesure à l'autre, en les parcourant dans
        // l'ordre à chaque `&` (les voix déjà pourvues sont refermées par la
        // barre, voir `finish_bar`).
        let cursor_value = self.reader.overlay_cursor.get(&root).copied().unwrap_or(0);
        let mut pool_len = self.reader.overlay_pool.get(&root).map_or(0, Vec::len);
        if cursor_value >= pool_len && pool_len < 3 {
            let number = self.reader.voices.len() + 1;
            let fields = VoiceFields {
                number,
                name: String::new(),
                clef,
                transpose: Some(transpose),
            };
            let mut overlay = Voice::new(&fields, unit, time, key);
            overlay.overlay_of = Some(root);
            overlay.tick = tick;
            // Les mesures déjà refermées de la racine restent vides ici ;
            // leur longueur doit être nulle pour ne pas étirer une anacrouse.
            for _ in 0..measures {
                overlay.finish_measure(BarLineType::Normal);
                if let Some(last) = overlay.measures.last_mut() {
                    last.len = Fraction::ZERO;
                }
            }
            let index = self.reader.voices.len();
            self.reader.voices.push(overlay);
            self.reader
                .overlay_pool
                .entry(root)
                .or_default()
                .push(index);
            pool_len += 1;
        }
        let pool = &self.reader.overlay_pool[&root];
        let index = pool[cursor_value % pool_len];
        self.reader.overlay_cursor.insert(root, cursor_value + 1);
        self.reader.voices[index].tick = tick;
        self.voice = index;
        self.reader.current = index;
    }

    /// Vrai si une lettre de hauteur suit l'altération courante, en ignorant
    /// les espaces, les décorations de phrasé et les autres altérations.
    ///
    /// Port de la grammaire insensible aux espaces d'`abc2xml.py`
    /// (`abc_voice.zonderWit()`, ligne 505) et de la tolérance de `errors`
    /// (ligne 431) : une altération orpheline est sautée, pas fatale.
    fn accidental_has_note(&self) -> bool {
        let mut index = self.pos;
        while let Some(character) = self.chars.get(index).copied() {
            if character.is_whitespace()
                || matches!(character, '(' | ')' | '.' | '^' | '_' | '=' | '/')
                || character.is_ascii_digit()
            {
                index += 1;
            } else {
                return ('A'..='G').contains(&character.to_ascii_uppercase());
            }
        }
        false
    }

    /// Rattache à la dernière note un tiret rencontré dans la boucle
    /// principale (après un espace, un rythme brisé ou un agrément).
    ///
    /// Port de `abc2xml.py::tie` (ligne 380) et de la grammaire insensible aux
    /// espaces du script (`abc_voice.zonderWit()`, ligne 505) : dans `G4 - G8`
    /// ou `A>-A`, le tiret appartient à la note précédente. Le modèle ne
    /// referme la liaison que quand la note suivante est lue (`link_ties`).
    fn tie_last_note(&mut self) {
        self.pos += 1;
        let Some((measure, index)) = self.voice().last_line_chords.last().copied() else {
            self.warn("liaison sans note précédente, ignorée");
            return;
        };
        let voice = &self.reader.voices[self.voice];
        // La mesure en cours n'est pas encore dans `voice.measures` : son
        // index vaut précisément `measures.len()` tant qu'elle n'est pas finie.
        let events = if measure == voice.measures.len() {
            &voice.current.events
        } else {
            &voice.measures[measure].events
        };
        let notes: Vec<(NoteId, u8)> = events
            .get(index)
            .and_then(|event| event.chord_rest.chord_ref())
            .map(|chord| {
                chord
                    .notes
                    .iter()
                    .map(|id| (*id, self.reader.score.note(*id).pitch))
                    .collect()
            })
            .unwrap_or_default();
        if notes.is_empty() {
            self.warn("liaison sans note précédente, ignorée");
            return;
        }
        for (id, pitch) in notes {
            let position = self.voice().position();
            self.voice().pending_ties.insert(pitch, (id, position));
        }
    }

    /// Tick absolu d'une position dans une voix (somme des mesures écrites).
    fn voice_tick(measures: &[crate::reader::MeasureData], position: &Position) -> Fraction {
        let mut tick = Fraction::ZERO;
        for measure in measures.iter().take(position.measure) {
            tick += measure.len;
        }
        tick + position.tick
    }

    /// Abandonne les liaisons en attente dont la hauteur n'est pas présente.
    ///
    /// Portage de `abc2xml.py` 269, `MusicXml.doNotations` (lignes 1445-1460) :
    /// une liaison ouverte n'est conservée que si elle appartient à une autre
    /// superposition (1446), si sa hauteur est dans l'accord courant (1447,
    /// `pitches` posé par `convertChord`, lignes 653-665), si la note courante
    /// est une note d'accord non première (1448) ou un agrément (1450), ou si
    /// sa hauteur est celle de la note courante (1449, `ptup` sans altération,
    /// ligne 1370). Sinon elle est signalée (« tie between different pitches »)
    /// et convertie en slur (1451-1460), jamais laissée en place : les voix
    /// d'agrément et les superpositions ont ici leur propre `pending_ties`, et
    /// le chemin accord transmet toutes les hauteurs de l'accord.
    ///
    /// Sans ce contrôle une liaison `G-A G` traverserait le `A` et relierait
    /// les deux `G`, alors qu'abc2midi compte les deux notes : c'était les
    /// notes liées en fin de liaison que le comparateur écarte (cause 78 airs,
    /// § 15.3 de abc.md). Le changement naïf consistant à conserver *toutes*
    /// les liaisons en attente casse cinq airs du lot de lecture (§ 55 de
    /// abc.md) ; il est verrouillé par `tests/tie_chains.rs`.
    fn drop_foreign_ties(&mut self, pitches: &[u8]) {
        let foreign: Vec<u8> = self
            .voice()
            .pending_ties
            .keys()
            .copied()
            .filter(|pitch| !pitches.contains(pitch))
            .collect();
        let destination = self.voice().position();
        for pitch in foreign {
            let Some((previous, origin)) = self.voice().pending_ties.remove(&pitch) else {
                continue;
            };
            // `abc2xml.py::doNotations` (lignes 1445-1460) : une liaison `-`
            // dont la hauteur n'est pas dans la note suivante est convertie
            // en slur, pas abandonnée. On retire le `tie_forward` et on crée
            // un spanner de liaison d'expression entre les deux notes.
            self.reader.score.note_mut(previous).tie_forward = None;
            let voice = self.voice();
            let start_tick = Self::voice_tick(&voice.measures, &origin);
            let end_tick = Self::voice_tick(&voice.measures, &destination);
            // `self.voice` indexes the ABC reader's voice list, not the
            // model's 0-3 voice slots: scores with more than four voices
            // would panic here (ABC 2.1 § 7; MuseScore has four voices per
            // staff). Clamp to a valid model voice; the spanner's staff is
            // refined when the parts are built.
            let track = Track::new(
                scoretk_core::model::StaffId::from_index(0),
                (self.voice % 4) as u8,
            );
            let mut spanner = Spanner::new(
                SpannerType::Slur,
                SpannerAnchor::new(start_tick, track),
                SpannerAnchor::new(end_tick, track),
            );
            spanner.notation = Some(SpannerNotation::Slur(Slur::new()));
            self.reader.score.add_spanner(spanner);
            self.warn(format!(
                "liaison entre hauteurs différentes ({pitch}) convertie en slur"
            ));
        }
    }

    /// Rattache une note à la liaison en attente et enregistre la sienne.
    fn link_ties(&mut self, notes: &[(NoteId, u8, bool)]) {
        let pitches: Vec<u8> = notes.iter().map(|(_, pitch, _)| *pitch).collect();
        self.drop_foreign_ties(&pitches);
        for (id, pitch, tie) in notes {
            if let Some((previous, _)) = self.voice().pending_ties.remove(pitch) {
                self.reader.score.note_mut(previous).tie_forward = Some(*id);
                self.reader.score.note_mut(*id).tie_back = Some(previous);
            }
            if *tie {
                let position = self.voice().position();
                self.voice().pending_ties.insert(*pitch, (*id, position));
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
        // Port de `abc2xml.py::tuplet_start` (lignes 395/484/2012) : (2, 4, 8)
        // sont des duoles/trioles sur 3, les autres sur 2 ; sans indication
        // le nombre de notes vaut le numérateur. `mkTmod` (ligne 728) écrit
        // ensuite `tmnum`/`tmden` dans `<time-modification>`.
        let (tmnum, tmden, ntup) = rhythm::tuplet_start(&numbers[..=count]);
        let actual = tmnum.max(1);
        let normal = tmden.max(1);
        let notes = ntup.max(1);
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
        // Un `)` lu juste après une note clôt la liaison sur CETTE note. Le
        // tick courant a déjà été avancé par `parse_chord_or_note` : l'utiliser
        // tel quel décalait l'extrémité d'une note à chaque aller-retour
        // (`(cde)f` → `(cdef)g` → `(cdefg)`, ABC 2.1 § 4.11).
        let voice = self.voice();
        let end = voice
            .current
            .events
            .last()
            .map(|event| crate::reader::Position {
                measure: voice.measure_index(),
                tick: event.tick,
            })
            .unwrap_or_else(|| voice.position());
        if let Some(start) = voice.open_slurs.pop() {
            voice.slurs.push((start, end));
        }
    }

    // --- crochets : accord, volta ou champ en ligne ----------------------

    fn parse_bracket(&mut self) -> Result<()> {
        // Barre invisible `[|]` ou `[]` (§ 4.8) ; `[|` seul est une barre
        // épaisse et se poursuit comme une barre ordinaire.
        if matches!(self.peek_at(1), Some('|' | ']')) {
            self.pos += 1;
            if self.peek() == Some('|') && self.peek_at(1) == Some(']') {
                self.pos += 2;
                // Port de `abc2xml.py`, `volta` (lignes 419-421) et
                // `bar_left` (ligne 427) : un numéro peut suivre la barre
                // invisible (`[|]1`) comme une barre ordinaire (`|1`).
                let volta = self.read_volta_numbers();
                return self.finish_bar(BarLineType::Normal, false, volta);
            }
            if self.peek() == Some(']') {
                self.pos += 1;
                let volta = self.read_volta_numbers();
                return self.finish_bar(BarLineType::Normal, false, volta);
            }
            return self.parse_bar();
        }

        // Port de `abc2xml.py`, `volta_text = Suppress('[') + pas(r'"[^"]+"')`
        // (ligne 417) et `bar_left` (ligne 424) : `["Coda"` est une fin de
        // reprise étiquetée. La norme ABC 2.1 § 4.9 est muette sur le texte
        // des voltas ; comme le script, on consomme la chaîne entière pour ne
        // pas lire son contenu comme des hauteurs. Le modèle ne porte pas de
        // texte de volta : on signale et on saute localement, les notes qui
        // suivent restent lisibles.
        if self.peek_at(1) == Some('"') {
            self.pos += 2; // '[' puis le guillemet d'ouverture
            while self.peek().is_some_and(|value| value != '"') {
                self.pos += 1;
            }
            if self.peek() == Some('"') {
                self.pos += 1;
            }
            self.warn("texte de volta ignoré");
            return Ok(());
        }

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
        let mut notes: Vec<(scoretk_core::model::Note, Fraction, bool)> = Vec::new();
        loop {
            self.skip_spaces();
            match self.peek() {
                Some(']') => {
                    self.bump();
                    break;
                }
                // Port de `abc2xml.py`, `chord_note = dec_note | rest | b1`
                // (lignes 383-385) : une décoration peut précéder chaque note
                // d'un accord (`[.e.g]`, `[C(E]`). On la signale et on la saute.
                Some('z' | 'Z' | 'x' | 'X') => {
                    self.warn("silence sans hauteur dans un accord ignoré");
                    self.pos += 1;
                    let _ = self.read_length_text();
                }
                // ABC 2.1 §4.11 allows a slur into a chord (`[C(E]`);
                // `abc2xml.py:371-387` parses `(` as a chord-note
                // decoration. Open/close the phrase here instead of
                // dropping it as a stray symbol.
                Some('(') => self.parse_paren(),
                Some(')') => {
                    self.close_slur();
                    self.pos += 1;
                }
                // ABC 2.1 §4.17 : une décoration longue peut précéder chaque
                // note d'un accord. La consommer comme jeton entier (`!pppp!`)
                // et non lettre par lettre : sinon le `p` est lu comme une
                // hauteur et l'accord est rejeté (`VolumeInChords`). Port de
                // `abc2xml.py::chord_note`/`dec_note` (lignes 383-385).
                Some('!' | '+') => self.parse_decoration(),
                Some('.' | '~' | ',' | '`' | '$' | '<' | '>' | '"') => {
                    self.warn("symbole égaré dans un accord ignoré");
                    self.pos += 1;
                }
                // Un accord coupé par une fin de ligne ne fait pas rejeter
                // tout l'air : on le referme ici avec un avertissement.
                None => {
                    self.warn("accord non fermé, traité comme fermé");
                    break;
                }
                _ => {
                    // `abc2xml.py:383,387`: `chord_note = dec_note` carries its
                    // own `Optional(tie)`, e.g. `[C-EG]` ties only the C.
                    let (note, _, note_tie) = self.read_note()?;
                    // Port de `abc2xml.py::convertChord` (ligne 651) : la
                    // durée écrite d'une note d'accord est le PRODUIT de sa
                    // durée propre et de celle de l'accord. `[G2G,2]` vaut
                    // donc une noire, et non un croche comme si l'on ignorait
                    // les `2` internes (cause des airs `book1.abc` X:0142/0177).
                    let propre = self.parse_mult(&self.last_length.clone())?;
                    notes.push((note, propre, note_tie));
                }
            }
        }
        let length_text = self.read_length_text();
        let chord_mult = self.parse_mult(&length_text)?;
        let multiple = notes
            .first()
            .map_or(chord_mult, |(_, propre, _)| *propre * chord_mult);
        let nominal = self.voice().unit * multiple;
        let tie = self.eat_tie();
        let (nominal, _) = self.apply_rhythm(nominal)?;
        let duration = duration::figure_for(nominal);
        let tick = self.voice().tick;
        let mut chord_rest = ChordRest::chord(duration.0);
        let mut ids = Vec::new();
        let mut pitches = Vec::new();
        for (note, _, note_tie) in notes {
            let pitch = note.pitch;
            let id = self.reader.score.add_note(note);
            if let Some(chord) = chord_rest.chord_mut() {
                chord.notes.push(id);
            }
            // A chord-level `-` still ties every note; a per-note `-` inside
            // the brackets ties only that one.
            ids.push((id, pitch, note_tie || tie));
            pitches.push(pitch);
        }
        self.link_ties(&ids);
        let scale = self.tuplet_scale();
        self.push_event(chord_rest, tick);
        self.voice().tick += nominal * scale;
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
                        // `|:|` is one combined start+end repeat token
                        // (`abc2xml.py:428-429`, `bars = ':')* '['* '|'+`).
                        // Without consuming the trailing `|`, a second empty
                        // bar is parsed and resets the end barline to Normal,
                        // losing the repeat entirely (ABC 2.1 §4.9).
                        if self.peek() == Some('|') {
                            self.bump();
                            end = BarLineType::EndStartRepeat;
                        }
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
        // Port de `abc2xml.py::mergePartMeasure` (ligne 951) : toutes les voix
        // superposées se ferment sur la même barre que leur voix racine.
        let root = self
            .reader
            .voices
            .get(self.voice)
            .and_then(|voice| voice.overlay_of)
            .unwrap_or(self.voice);
        let overlays: Vec<usize> = (0..self.reader.voices.len())
            .filter(|&index| index != root && self.reader.voices[index].overlay_of == Some(root))
            .collect();
        for index in overlays {
            // Même vide, la mesure est refermée : sinon l'index de mesure de
            // la voix superposée dérive et ses notes tombent une mesure plus
            // tôt que celles de la racine. Une mesure vide garde une longueur
            // nulle pour ne pas étirer une mesure courte de la racine.
            let overlay = &mut self.reader.voices[index];
            let empty = overlay.current.events.is_empty() && overlay.tick.is_zero();
            overlay.finish_measure(end);
            if empty {
                if let Some(last) = overlay.measures.last_mut() {
                    last.len = Fraction::ZERO;
                }
            }
        }
        // La barre est toujours traitée par la voix racine : c'est elle qui
        // porte les volatas et les barres de reprise de la mesure.
        self.reader.overlay_cursor.remove(&root);
        self.voice = root;
        self.reader.current = root;
        let voice = &mut self.reader.voices[self.voice];
        let empty = voice.current.events.is_empty() && voice.tick.is_zero();
        if empty {
            // Une barre sans reprise sur une mesure vide ne doit pas écraser
            // la barre de fin de la mesure précédente : `:|` suivi de `|:`
            // porte les deux répétitions, et `:|` suivi de `||` garde le `:|`
            // (ABC 2.1 §4.9, `abc2xml.py::mergePartMeasure`).
            if end != BarLineType::Normal {
                if let Some(last) = voice.measures.last_mut() {
                    if !last.end_bar.is_repeat_end()
                        || end.is_repeat_end()
                        || end != BarLineType::Double
                    {
                        last.end_bar = end;
                    }
                }
            }
            if start_repeat {
                voice.current.start_bar = BarLineType::StartRepeat;
            }
            // Une mesure vide peut déjà porter une volta écrite avant la
            // barre : ne l'efface pas quand aucun numéro en ligne ne suit.
            if let Some(numbers) = volta {
                voice.current.volta = Some(numbers);
            }
            // Annotations between barlines (`"Segno"[|]"_D.S."[|]`) belong
            // to an empty measure with no event to carry them. ABC 2.1 §4.19
            // and `abc2xml.py:2022` still write them as `<words>`: attach to
            // the previous measure's last event before returning, just as
            // the non-empty branch does for end-of-measure annotations.
            if !voice.annotations.is_empty()
                && voice
                    .measures
                    .last()
                    .and_then(|measure| measure.events.last())
                    .is_some()
            {
                let annotations = std::mem::take(&mut voice.annotations);
                if let Some(event) = voice
                    .measures
                    .last_mut()
                    .and_then(|measure| measure.events.last_mut())
                {
                    event.annotations.extend(annotations);
                }
            }
            // With no previous event (annotation before the first bar), keep
            // the pending annotations: the next event will carry them.
            // Après la barre, la musique revient à la voix racine : les notes
            // qui suivent ne sont plus des superpositions.
            self.voice = root;
            self.reader.current = root;
            return Ok(());
        }
        let measure = voice.measure_index();
        if let Some(numbers) = voice.current.volta.clone() {
            voice.voltas.push((measure, measure, numbers));
        }
        // Une annotation lue après la dernière note (`(gf)"Segno"||`) restait
        // en attente et dérivait vers la mesure suivante, ou était perdue en
        // fin d'air. `abc2xml.py::convert` (x.name == 'text') l'écrit dans la
        // mesure courante, juste avant la barre : on la rattache au dernier
        // événement.
        let annotations = std::mem::take(&mut voice.annotations);
        if !annotations.is_empty() {
            if let Some(event) = voice.current.events.last_mut() {
                event.annotations.extend(annotations);
            }
        }
        voice.finish_measure(end);
        self.reader.voices[self.voice].current.volta = volta;
        if start_repeat {
            self.reader.voices[self.voice].next_start_bar = BarLineType::StartRepeat;
        }
        self.voice = root;
        self.reader.current = root;
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
        // `read_note()` enregistre les altérations explicites dans l'état de
        // mesure (ABC 2.1 §4.2). Un `^c` dans `{^cde}` ne concerne que la note
        // d'agrément : le restaurer empêche le `c` suivant d'hériter du dièse.
        let accidental = self.voice().accidental.clone();
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
                // Port de `abc2xml.py`, `grace_stem = Optional(decorations)
                // + stem` (lignes 398-399) : un accord peut former le `:
                // stem` d'un agrément (`{[Ge]}`). Les crochets sont sautés
                // localement, les notes sont gardées.
                Some('[' | ']') => {
                    self.warn("accord dans un agrément : crochet ignoré");
                    self.pos += 1;
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
        // Un `=` explicite dans l'agrément lève l'altération courante pour
        // la suite de la mesure (ABC 2.1 §4.2, spec 853-855 ; Haydn
        // `=e{g/2}fe/2` en K:Eb). On ne garde que ces remises à zéro : un
        // dièse ou un bémol d'agrément reste local à la note d'agrément.
        let grace_state = self.voice().accidental.clone();
        self.voice().accidental = accidental;
        for letter in ['C', 'D', 'E', 'F', 'G', 'A', 'B'] {
            if grace_state.current(letter) == 0 {
                self.voice().accidental.set(letter, 0);
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
    // `abc2xml.py:411-416`: `chord_sym ^ text_expression` chooses the longest
    // match. A chord is `root + kind + degrees + /bass`; arbitrary words that
    // merely start with A-G (`End`, `Fine`, `BREAK`) are text. The old test
    // accepted any alphanumeric suffix and misread them as `<harmony>`.
    const KINDS: &[&str] = &[
        "m7b5", "mi7b5", "maj7", "Maj7", "ma7", "min7", "mi7", "dim7", "aug7", "sus2", "sus4",
        "maj9", "Maj9", "ma9", "min9", "mi9", "maj11", "Maj11", "ma11", "min11", "mi11", "maj13",
        "Maj13", "ma13", "min13", "mi13", "m6", "ma6", "min6", "mi6", "sus", "maj", "Maj", "min",
        "dim", "aug", "mi", "ma", "m7", "M7", "o7", "-7", "+7", "m9", "M6", "11", "13", "min",
        "tri", "7", "6", "9", "m", "M", "o", "+", "-",
    ];
    const DEGREES: &[&str] = &["11", "13", "2", "4", "5", "6", "7", "9"];

    let mut rest = text;
    let Some(first) = rest.chars().next() else {
        return false;
    };
    if !('A'..='G').contains(&first) {
        return false;
    }
    rest = &rest[first.len_utf8()..];
    if rest.starts_with(['#', 'b', '=']) {
        rest = &rest[1..];
    }
    // Optional chord kind, longest first (abc2xml.py:412 `seventh|sixth|…|triad`).
    if let Some(kind) = KINDS.iter().find(|kind| rest.starts_with(**kind)) {
        rest = &rest[kind.len()..];
        if rest.starts_with("sus") && !kind.starts_with("sus") {
            rest = &rest[3..];
        }
    }
    // Zero or more degrees, each with an optional accidental.
    loop {
        let token = rest.trim_start_matches(['#', 'b', '=']);
        let skipped = rest.len() - token.len();
        if let Some(degree) = DEGREES.iter().find(|degree| token.starts_with(**degree)) {
            rest = &token[degree.len()..];
        } else {
            if skipped > 0 {
                return false;
            }
            break;
        }
    }
    // Optional bass after `/`.
    if let Some(bass) = rest.strip_prefix('/') {
        let mut bass = bass;
        if let Some(root) = bass.chars().next() {
            if !('A'..='G').contains(&root) {
                return false;
            }
            bass = &bass[root.len_utf8()..];
        } else {
            return false;
        }
        if bass.starts_with(['#', 'b', '=']) {
            bass = &bass[1..];
        }
        rest = bass;
    }
    rest.is_empty()
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
                if let Some(chord_rest) = segment.chord_rest(Track::new(StaffId::from_index(0), 0))
                {
                    if let Some(chord) = chord_rest.chord_ref() {
                        pitches.extend(chord.notes.iter().map(|id| output.score.note(*id).pitch));
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
    fn barres_invisibles_entre_crochets() {
        let invisible = parse("C D E F|[|]G A B c|]");
        let normale = parse("C D E F|G A B c|]");
        assert_eq!(invisible.score.measures.len(), 2);
        assert_eq!(
            contenu(&invisible),
            (vec![60, 62, 64, 65, 67, 69, 71, 72], vec![0.125; 8])
        );
        assert_eq!(contenu(&invisible), contenu(&normale));

        // Limites : `[]`, barre épaisse `[|`, et accord ordinaire inchangé.
        let vide = parse("C D[]E F|]");
        assert_eq!(vide.score.measures.len(), 2);
        assert_eq!(contenu(&vide).0, vec![60, 62, 64, 65]);
        let epaisse = parse("C D[|E F|]");
        assert_eq!(epaisse.score.measures.len(), 2);
        assert_eq!(contenu(&epaisse).0, vec![60, 62, 64, 65]);
        let accord = parse("[CEG]2 z2|]");
        assert_eq!(contenu(&accord).0, vec![60, 64, 67]);
    }

    #[test]
    fn microintervalles_arrondis_au_demi_ton() {
        let source = "\
X:1
T:Microtones
M:4/4
L:1/8
K:C
V:1
^/C2 _/B2 =C2 ^^C2 | [^/D^/F]2 z2 | {^/G}A2 B2 c2 |]
V:2
C, D, E, F, | ^/G, A, B, c |]
";
        let output = crate::reader::read_abc_with(source, &crate::reader::ReadOptions::default())
            .expect("lecture");
        assert_eq!(output.score.measures.len(), 3);
        assert_eq!(
            contenu(&output),
            (
                vec![61, 70, 60, 62, 63, 66, 69, 71, 72],
                vec![0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25]
            )
        );
        assert_eq!(output.score.notes.len(), 18, "les deux voix sont lues");
        assert!(
            output
                .warnings
                .iter()
                .any(|warning| warning.message.contains("micro-intervalle")),
            "avertissement d'arrondi absent : {:?}",
            output.warnings
        );

        // Sans la barre de micro-intervalle, la musique ordinaire est inchangée.
        let ordinaire = parse("^C2 _B2 =C2 ^^C2 | [^D^F]2 z2 | {^G}A2 B2 c2 |]");
        assert_eq!(contenu(&output), contenu(&ordinaire));

        // Cas limite : slashs multiples et fin de ligne sans note suivante.
        let double = parse("^//C2 |");
        assert_eq!(contenu(&double).0, vec![61]);
        let incomplet = "X:1\nT:Test\nM:4/4\nL:1/8\nK:C\nC ^/ |\n";
        let orphelin =
            crate::reader::read_abc_with(incomplet, &crate::reader::ReadOptions::default())
                .expect("une altération orpheline est sautée, pas fatale");
        assert_eq!(orphelin.score.notes.len(), 1, "seul le `C` est lu");
        assert_eq!(orphelin.score.notes[0].pitch, 60);
        assert!(
            orphelin
                .warnings
                .iter()
                .any(|warning| warning.message.contains("altération sans note")),
            "l'altération orpheline doit être signalée"
        );
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
                vec![0.25, 0.125, 0.125, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.125, 0.125]
            )
        );
        assert_eq!(output.score.notes.len(), 19, "les deux voix sont lues");
        let trilles = output
            .score
            .measures
            .iter()
            .flat_map(|measure| &measure.segments)
            .filter_map(|segment| segment.chord_rest(Track::new(StaffId::from_index(0), 0)))
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
            (
                vec![60, 62, 64, 65, 67, 69],
                vec![0.125, 0.125, 0.125, 0.125, 0.125, 0.125]
            )
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

        // Une mesure vide entre la volta et les notes conserve son numéro.
        let vide = parse("C D |[1 | E F |]");
        assert_eq!(voltas(&vide), vec![vec![1]]);
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
