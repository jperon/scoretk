// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage d'un sous-ensemble de `lily/parser.yy` de LilyPond, Copyright (C)
// 1998--2026 Han-Wen Nienhuys et al., modifié (réécrit en Rust) en 2026.
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

//! Analyseur syntaxique du sous-ensemble LilyPond.
//!
//! Suit les productions de `lily/parser.yy` pour la musique :
//! `music_list`, `sequential_music`, `simultaneous_music`,
//! `note_chord_element`, `chord_body_element`, `post_events`, `duration`,
//! `repeated_music`, `alternative_music`, `context_change` et `\score`. Tout ce
//! qui sort du sous-ensemble (Scheme arbitraire, commandes inconnues) est
//! ignoré avec un avertissement : la lecture ne doit jamais échouer sur une
//! décoration, seulement sur une source illisible.

use std::collections::BTreeMap;

use mscz2xml::model::{
    ArpeggioType, ArticulationType, BreathType, Duration, DurationType, FermataType, Fraction,
    GraceType, KeyMode, NoteHeadGroup, OrnamentType, Placement, StemDirection, TremoloKind,
};

use super::ast::{
    Chord, ChordKind, Context, Document, GroupBlock, GroupKind, HeaderAst, LyricsBlock, Music,
    NoteSpec, PitchSpec, PostEvent, RepeatKind, ScoreAst, StaffBlock, Syllable, VoiceBlock,
};
use super::lexer::{tokenize, Kind, Token};
use super::pitch::{self, Language};
use super::warnings::Warnings;

/// Analyseur d'une source LilyPond.
pub(crate) struct Parser<'a> {
    /// Jetons de la source.
    tokens: Vec<Token>,
    /// Position courante.
    position: usize,
    /// Langue des hauteurs.
    language: Language,
    /// Variables musicales déjà définies.
    variables: BTreeMap<String, Music>,
    /// Dernier accord lu, pour la répétition `q`.
    last_notes: Option<Vec<NoteSpec>>,
    /// Durée par défaut (celle du dernier jeton de durée).
    default_duration: Duration,
    /// Avertissements collectés.
    warnings: &'a mut Warnings,
    /// Groupe de tête posé par `\tweak style`.
    pending_head: NoteHeadGroup,
}

/// Résultat de l'analyse.
pub(crate) fn parse(source: &str, warnings: &mut Warnings) -> Document {
    let tokens = tokenize(source);
    let mut parser = Parser {
        tokens,
        position: 0,
        language: Language::default(),
        variables: BTreeMap::new(),
        last_notes: None,
        default_duration: Duration::quarter(),
        warnings,
        pending_head: NoteHeadGroup::Normal,
    };
    parser.document()
}

impl<'a> Parser<'a> {
    //------------------------------------------------------------------
    // Jetons
    //------------------------------------------------------------------

    /// Jeton courant.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    /// Jeton à une distance donnée.
    fn peek_at(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.position + offset)
    }

    /// Consomme le jeton courant.
    fn bump(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).cloned();
        if token.is_some() {
            self.position += 1;
        }
        token
    }

    /// Vrai en fin de source.
    fn at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }

    /// Consomme la ponctuation donnée.
    fn eat_punct(&mut self, character: char) -> bool {
        if self.peek().is_some_and(|token| token.is_punct(character)) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    /// Consomme la commande donnée.
    fn eat_command(&mut self, name: &str) -> bool {
        if self.peek().is_some_and(|token| token.is_command(name)) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    /// Vrai si les deux jetons suivants forment `<<`.
    fn at_double_angle_open(&self) -> bool {
        matches!(
            (self.peek(), self.peek_at(1)),
            (Some(first), Some(second))
                if first.is_punct('<') && second.is_punct('<') && second.column == first.column + 1
        )
    }

    /// Vrai si les deux jetons suivants forment `>>`.
    fn at_double_angle_close(&self) -> bool {
        matches!(
            (self.peek(), self.peek_at(1)),
            (Some(first), Some(second))
                if first.is_punct('>') && second.is_punct('>') && second.column == first.column + 1
        )
    }

    /// Avertissement à la position courante.
    fn warn(&mut self, message: impl Into<String>) {
        let (line, column) = self
            .peek()
            .map_or((0, 0), |token| (token.line, token.column));
        self.warnings.push(line, column, message);
    }

    /// Texte du jeton courant, pour les messages.
    fn current_text(&self) -> String {
        self.peek().map_or_else(String::new, ToString::to_string)
    }

    //------------------------------------------------------------------
    // Document
    //------------------------------------------------------------------

    /// Analyse un document complet.
    fn document(&mut self) -> Document {
        let mut document = Document::default();
        document.language = "nederlands".to_owned();

        while !self.at_end() {
            let Some(token) = self.peek().cloned() else {
                break;
            };
            match &token.kind {
                Kind::Command if token.text == "version" => {
                    self.position += 1;
                    if let Some(Token {
                        kind: Kind::Str,
                        text,
                        ..
                    }) = self.bump()
                    {
                        document.version = Some(text);
                    }
                }
                Kind::Command if token.text == "language" => {
                    self.position += 1;
                    if let Some(Token {
                        kind: Kind::Str,
                        text,
                        ..
                    }) = self.bump()
                    {
                        match Language::from_name(&text) {
                            Some(language) => {
                                self.language = language;
                                document.language = text;
                            }
                            None => self.warn(format!("langue de hauteurs inconnue : {text}")),
                        }
                    }
                }
                Kind::Command if token.text == "header" => {
                    self.position += 1;
                    let header = self.parse_header();
                    document.header.entries.extend(header.entries);
                }
                Kind::Command if token.text == "score" => {
                    self.position += 1;
                    let score = self.parse_score();
                    document.score = Some(score);
                }
                Kind::Command if token.text == "book" || token.text == "bookpart" => {
                    self.position += 1;
                    self.skip_braced();
                }
                Kind::Command if token.text == "include" => {
                    self.position += 1;
                    self.bump();
                    self.warn("\\include ignoré : le fichier inclus n'est pas lu");
                }
                Kind::Command if token.text == "markup" || token.text == "markuplist" => {
                    self.position += 1;
                    self.parse_markup_text();
                }
                Kind::Word => {
                    // Assignation `nom = musique` ou mot isolé.
                    if self
                        .peek_at(1)
                        .is_some_and(|next| next.is_punct('='))
                    {
                        let name = token.text.clone();
                        self.position += 2;
                        let music = self.parse_music();
                        self.variables.insert(name, music);
                    } else {
                        self.position += 1;
                        self.warn(format!("mot hors contexte ignoré : {}", token.text));
                    }
                }
                Kind::Command => {
                    self.position += 1;
                    self.skip_command_arguments(&token.text);
                }
                Kind::Punct(';') => {
                    self.position += 1;
                }
                _ => {
                    self.position += 1;
                }
            }
        }

        document
    }

    /// Bloc `\header { champ = valeur … }`.
    fn parse_header(&mut self) -> HeaderAst {
        let mut header = HeaderAst::default();
        if !self.eat_punct('{') {
            return header;
        }
        while !self.at_end() && !self.eat_punct('}') {
            let Some(token) = self.bump() else {
                break;
            };
            if !token.is_punct('=') {
                continue;
            }
            let Some(name) = token_text(&token) else {
                continue;
            };
            let value = self.parse_header_value();
            if !name.is_empty() {
                header.entries.push((name, value));
            }
        }
        header
    }

    /// Valeur d'un champ de `\header`.
    fn parse_header_value(&mut self) -> String {
        match self.peek().cloned() {
            Some(Token {
                kind: Kind::Str,
                text,
                ..
            }) => {
                self.position += 1;
                text
            }
            Some(token) if token.is_command("markup") => {
                self.position += 1;
                self.parse_markup_text()
            }
            Some(Token {
                kind: Kind::Number,
                text,
                ..
            }) => {
                self.position += 1;
                text
            }
            Some(Token {
                kind: Kind::Word,
                text,
                ..
            }) => {
                self.position += 1;
                text
            }
            _ => String::new(),
        }
    }

    //------------------------------------------------------------------
    // Bloc \score et contextes
    //------------------------------------------------------------------

    /// Bloc `\score { … }`.
    fn parse_score(&mut self) -> ScoreAst {
        let mut score = ScoreAst::default();
        if !self.eat_punct('{') {
            self.warn("\\score sans accolade");
            return score;
        }
        let mut elements = Vec::new();
        while !self.at_end() {
            if self.eat_punct('}') {
                break;
            }
            if self.at_double_angle_open() {
                self.position += 2;
                elements.extend(self.parse_context_list());
            } else if self.peek().is_some_and(|token| token.is_command("new")) {
                self.position += 1;
                if let Some(group) = self.parse_new() {
                    elements.push(group);
                }
            } else if self
                .peek()
                .is_some_and(|token| token.is_command("layout") || token.is_command("midi")
                    || token.is_command("paper"))
            {
                self.position += 1;
                self.skip_braced();
            } else if self.peek().is_some_and(|token| token.is_command("header")) {
                self.position += 1;
                self.parse_header();
            } else {
                // Musique sans contexte : une portée anonyme.
                let music = self.parse_music();
                if !music.is_empty() {
                    let mut staff = StaffBlock::new();
                    staff.voices.push(VoiceBlock {
                        name: None,
                        music,
                    });
                    elements.push(GroupBlock {
                        kind: GroupKind::Staff,
                        instrument_name: None,
                        short_instrument_name: None,
                        staves: vec![staff],
                        lyrics: Vec::new(),
                    });
                }
            }
        }
        score.groups = elements;
        score
    }

    /// Liste de contextes d'un `<< … >>` de `\score`.
    fn parse_context_list(&mut self) -> Vec<GroupBlock> {
        let mut groups = Vec::new();
        while !self.at_end() && !self.at_double_angle_close() {
            if self.eat_command("new") {
                if let Some(group) = self.parse_new() {
                    groups.push(group);
                }
            } else {
                let music = self.parse_music();
                if !music.is_empty() {
                    let mut staff = StaffBlock::new();
                    staff.voices.push(VoiceBlock {
                        name: None,
                        music,
                    });
                    groups.push(GroupBlock {
                        kind: GroupKind::Staff,
                        instrument_name: None,
                        short_instrument_name: None,
                        staves: vec![staff],
                        lyrics: Vec::new(),
                    });
                }
            }
        }
        if self.at_double_angle_close() {
            self.position += 2;
        }
        groups
    }

    /// `\new Nom [= "id"] [\with { … }] musique`.
    fn parse_new(&mut self) -> Option<GroupBlock> {
        let name = match self.bump() {
            Some(Token {
                kind: Kind::Word,
                text,
                ..
            }) => text,
            other => {
                if let Some(token) = other {
                    self.warn(format!("contexte attendu après \\new, trouvé {token}"));
                }
                return None;
            }
        };
        let kind = GroupKind::from_name(&name);
        let mut context_name = None;
        if self.eat_punct('=') {
            if let Some(Token {
                kind: Kind::Str,
                text,
                ..
            }) = self.bump()
            {
                context_name = Some(text);
            }
        }

        let mut instrument_name = None;
        let mut short_instrument_name = None;
        let mut verse = None;
        if self.eat_command("with") {
            let properties = self.parse_with_block();
            for (property, value) in properties {
                match property.as_str() {
                    "instrumentName" => instrument_name = Some(value),
                    "shortInstrumentName" => short_instrument_name = Some(value),
                    "stanza" => {
                        verse = value
                            .trim_end_matches('.')
                            .parse::<usize>()
                            .ok()
                            .and_then(|number| number.checked_sub(1))
                    }
                    _ => {}
                }
            }
        }

        // `\new Lyrics \lyricsto "…" { … }` et `\new Lyrics \addlyrics { … }`.
        if kind == GroupKind::Other && name == "Lyrics" {
            let target = if self.eat_command("lyricsto") {
                match self.bump() {
                    Some(Token {
                        kind: Kind::Str,
                        text,
                        ..
                    }) => text,
                    _ => String::new(),
                }
            } else {
                self.eat_command("addlyrics");
                String::new()
            };
            let syllables = self.parse_lyric_music();
            let block = LyricsBlock {
                target,
                syllables,
                verse: verse.unwrap_or(0),
            };
            return Some(GroupBlock {
                kind: GroupKind::Other,
                instrument_name: None,
                short_instrument_name: None,
                staves: Vec::new(),
                lyrics: vec![block],
            });
        }

        // Contenu du contexte.
        let mut staff = StaffBlock::new();
        staff.name = context_name.clone();
        staff.instrument_name = instrument_name.clone();
        staff.short_instrument_name = short_instrument_name.clone();

        match kind {
            GroupKind::PianoStaff | GroupKind::ChoirStaff | GroupKind::StaffGroup => {
                let mut group = GroupBlock {
                    kind,
                    instrument_name,
                    short_instrument_name,
                    staves: Vec::new(),
                    lyrics: Vec::new(),
                };
                let content = self.parse_music();
                self.collect_staves(content, &mut group.staves, &mut group.lyrics);
                if group.staves.is_empty() {
                    group.staves.push(StaffBlock::new());
                }
                Some(group)
            }
            GroupKind::Voice => {
                staff.voices.push(VoiceBlock {
                    name: context_name,
                    music: self.parse_music(),
                });
                self.collect_addlyrics(&mut staff);
                Some(GroupBlock {
                    kind: GroupKind::Staff,
                    instrument_name,
                    short_instrument_name,
                    staves: vec![staff],
                    lyrics: Vec::new(),
                })
            }
            _ => {
                let content = self.parse_music();
                let mut voices = Vec::new();
                let mut lyrics = Vec::new();
                self.collect_voices(content, &mut voices, &mut lyrics);
                if voices.is_empty() {
                    voices.push(VoiceBlock {
                        name: None,
                        music: Music::Empty,
                    });
                }
                staff.voices = voices;
                staff.lyrics.extend(lyrics);
                self.collect_addlyrics(&mut staff);
                Some(GroupBlock {
                    kind: GroupKind::Staff,
                    instrument_name,
                    short_instrument_name,
                    staves: vec![staff],
                    lyrics: Vec::new(),
                })
            }
        }
    }

    /// Répartit le contenu d'un groupe en portées.
    fn collect_staves(
        &mut self,
        music: Music,
        staves: &mut Vec<StaffBlock>,
        lyrics: &mut Vec<LyricsBlock>,
    ) {
        match music {
            Music::Sim(items) => {
                for item in items {
                    match item {
                        Music::New(group) => {
                            staves.extend(group.staves);
                            lyrics.extend(group.lyrics);
                        }
                        Music::Lyrics(block) => lyrics.push(*block),
                        other => {
                            let mut staff = StaffBlock::new();
                            let mut voices = Vec::new();
                            let mut staff_lyrics = Vec::new();
                            self.collect_voices(other, &mut voices, &mut staff_lyrics);
                            staff.voices = voices;
                            staff.lyrics = staff_lyrics;
                            staves.push(staff);
                        }
                    }
                }
            }
            Music::New(group) => staves.extend(group.staves),
            Music::Lyrics(block) => lyrics.push(*block),
            other => {
                let mut staff = StaffBlock::new();
                let mut voices = Vec::new();
                self.collect_voices(other, &mut voices, lyrics);
                staff.voices = voices;
                staves.push(staff);
            }
        }
    }

    /// Répartit le contenu d'une portée en voix.
    fn collect_voices(
        &mut self,
        music: Music,
        voices: &mut Vec<VoiceBlock>,
        lyrics: &mut Vec<LyricsBlock>,
    ) {
        match music {
            Music::Sim(items) => {
                for item in items {
                    match item {
                        Music::New(group) => {
                            for staff in group.staves {
                                voices.extend(staff.voices);
                                lyrics.extend(staff.lyrics);
                            }
                            lyrics.extend(group.lyrics);
                        }
                        Music::Lyrics(block) => lyrics.push(*block),
                        other => voices.push(VoiceBlock {
                            name: None,
                            music: other,
                        }),
                    }
                }
            }
            Music::New(group) => {
                for staff in group.staves {
                    voices.extend(staff.voices);
                    lyrics.extend(staff.lyrics);
                }
                lyrics.extend(group.lyrics);
            }
            Music::Lyrics(block) => lyrics.push(*block),
            Music::Empty => {}
            other => voices.push(VoiceBlock {
                name: None,
                music: other,
            }),
        }
    }

    /// Attache un `\addlyrics` qui suit une voix.
    fn collect_addlyrics(&mut self, staff: &mut StaffBlock) {
        while self.eat_command("addlyrics") {
            let syllables = self.parse_lyric_music();
            let target = staff
                .voices
                .last()
                .and_then(|voice| voice.name.clone())
                .unwrap_or_default();
            let verse = staff.lyrics.len();
            staff.lyrics.push(LyricsBlock {
                target,
                syllables,
                verse,
            });
        }
    }

    /// Bloc `\with { … }` : couples propriété/valeur.
    fn parse_with_block(&mut self) -> Vec<(String, String)> {
        let mut properties = Vec::new();
        if !self.eat_punct('{') {
            return properties;
        }
        while !self.at_end() && !self.eat_punct('}') {
            let Some(token) = self.bump() else {
                break;
            };
            if token.is_punct(';') {
                continue;
            }
            let name = token_text(&token);
            if !self.eat_punct('=') {
                continue;
            }
            let value = self.parse_property_value();
            if let Some(name) = name {
                properties.push((name, value));
            }
        }
        properties
    }

    /// Valeur d'une propriété (`\with`, `\header`, `\set`).
    fn parse_property_value(&mut self) -> String {
        match self.peek().cloned() {
            Some(Token {
                kind: Kind::Str,
                text,
                ..
            }) => {
                self.position += 1;
                text
            }
            Some(token) if token.is_command("markup") => {
                self.position += 1;
                self.parse_markup_text()
            }
            Some(Token {
                kind: Kind::Number,
                text,
                ..
            }) => {
                self.position += 1;
                text
            }
            Some(Token {
                kind: Kind::Word,
                text,
                ..
            }) => {
                self.position += 1;
                text
            }
            Some(token) if token.is_punct('#') => {
                self.position += 1;
                self.parse_scheme_atom()
            }
            Some(token) if token.is_punct('{') => {
                self.parse_markup_text()
            }
            _ => {
                self.bump();
                String::new()
            }
        }
    }

    //------------------------------------------------------------------
    // Paroles
    //------------------------------------------------------------------

    /// Contenu d'un bloc de paroles (`\lyricsto`, `\addlyrics`, `\lyricmode`).
    fn parse_lyric_music(&mut self) -> Vec<Syllable> {
        if !self.eat_punct('{') {
            // Un unique mot ou une variable : rien à aligner.
            self.skip_music_token();
            return Vec::new();
        }
        let mut syllables = Vec::new();
        while !self.at_end() && !self.eat_punct('}') {
            if self.eat_punct(';') {
                continue;
            }
            match self.peek().cloned() {
                Some(token) if token.is_command("skip") => {
                    self.position += 1;
                    // La durée de `\skip` est consommée.
                    if self
                        .peek()
                        .is_some_and(|token| token.kind == Kind::Number)
                    {
                        self.position += 1;
                    }
                    syllables.push(Syllable {
                        text: String::new(),
                        hyphen: false,
                        extender: false,
                        skip: true,
                    });
                }
                Some(token) if token.is_command("set") => {
                    self.position += 1;
                    while !self.at_end()
                        && !self.peek().is_some_and(|token| token.is_punct('='))
                    {
                        self.position += 1;
                    }
                    self.bump();
                    self.parse_property_value();
                }
                Some(token) if token.is_command("markup") => {
                    self.position += 1;
                    let text = self.parse_markup_text();
                    syllables.push(Syllable {
                        text,
                        hyphen: false,
                        extender: false,
                        skip: false,
                    });
                }
                Some(Token {
                    kind: Kind::Word,
                    text,
                    ..
                }) => {
                    self.position += 1;
                    let mut text = text;
                    // Ponctuation collée au mot (`ta.`, `Nel,`).
                    while let Some(next) = self.peek().cloned() {
                        if next.line == self.tokens[self.position - 1].line
                            && next.column == self.tokens[self.position - 1].column + 1
                            && matches!(next.kind, Kind::Punct('.') | Kind::Punct(',')
                                | Kind::Punct('!') | Kind::Punct('?') | Kind::Punct(':')
                                | Kind::Punct(';') | Kind::Punct('\'') | Kind::Punct('-'))
                        {
                            text.push_str(&next.text);
                            self.position += 1;
                        } else {
                            break;
                        }
                    }
                    let hyphen = self.eat_lyric_hyphen();
                    let extender = self.eat_lyric_extender();
                    syllables.push(Syllable {
                        text,
                        hyphen,
                        extender,
                        skip: false,
                    });
                }
                Some(Token {
                    kind: Kind::Str,
                    text,
                    ..
                }) => {
                    self.position += 1;
                    let hyphen = self.eat_lyric_hyphen();
                    let extender = self.eat_lyric_extender();
                    syllables.push(Syllable {
                        text,
                        hyphen,
                        extender,
                        skip: false,
                    });
                }
                Some(_) => {
                    self.position += 1;
                }
                None => break,
            }
        }
        syllables
    }

    /// Consomme `--` (deux tirets consécutifs).
    fn eat_lyric_hyphen(&mut self) -> bool {
        if self.peek().is_some_and(|token| token.is_punct('-'))
            && self.peek_at(1).is_some_and(|token| token.is_punct('-'))
        {
            self.position += 2;
            return true;
        }
        false
    }

    /// Consomme `__` (deux soulignés consécutifs).
    fn eat_lyric_extender(&mut self) -> bool {
        if self.peek().is_some_and(|token| token.is_punct('_'))
            && self.peek_at(1).is_some_and(|token| token.is_punct('_'))
        {
            self.position += 2;
            return true;
        }
        false
    }

    //------------------------------------------------------------------
    // Musique
    //------------------------------------------------------------------

    /// Analyse une expression musicale.
    fn parse_music(&mut self) -> Music {
        if self.at_end() {
            return Music::Empty;
        }
        if self.at_double_angle_open() {
            self.position += 2;
            let mut items = Vec::new();
            while !self.at_end() && !self.at_double_angle_close() {
                if self.peek().is_some_and(|token| token.is_command("\\")) {
                    self.position += 1;
                    continue;
                }
                let item = self.parse_music();
                if !item.is_empty() {
                    items.push(item);
                }
            }
            if self.at_double_angle_close() {
                self.position += 2;
            }
            return Music::Sim(items);
        }
        let Some(token) = self.peek().cloned() else {
            return Music::Empty;
        };
        match &token.kind {
            Kind::Punct('{') => {
                self.position += 1;
                let mut items = Vec::new();
                while !self.at_end() && !self.eat_punct('}') {
                    let item = self.parse_music();
                    if !item.is_empty() {
                        items.push(item);
                    }
                }
                Music::Seq(items)
            }
            Kind::Punct('<') => self.parse_chord_group(),
            Kind::Punct('}') | Kind::Punct('>') | Kind::Punct('|') | Kind::Punct(')') => {
                self.position += 1;
                Music::Empty
            }
            Kind::Command => self.parse_command(),
            Kind::Word => self.parse_chord(false),
            Kind::Str | Kind::Number | Kind::Punct(_) => {
                self.position += 1;
                Music::Empty
            }
        }
    }

    /// Commande musicale.
    fn parse_command(&mut self) -> Music {
        let Some(Token {
            kind: Kind::Command,
            text: name,
            ..
        }) = self.peek().cloned()
        else {
            return Music::Empty;
        };
        self.position += 1;
        let context = |context| Music::Context(context);
        match name.as_str() {
            "fixed" => {
                let reference = self.parse_optional_pitch();
                Music::Context(Context::Fixed {
                    reference,
                    music: Box::new(self.parse_music()),
                })
            }
            "relative" => {
                let reference = self.parse_optional_pitch();
                Music::Context(Context::Relative {
                    reference,
                    music: Box::new(self.parse_music()),
                })
            }
            "absolute" => self.parse_music(),
            "transpose" => {
                let from = self.parse_optional_pitch().unwrap_or(PitchSpec {
                    step: 0,
                    alter: 0,
                    octave: 0,
                    explicit_octave: false,
                    force: false,
                    cautionary: false,
                });
                let to = self.parse_optional_pitch().unwrap_or(from);
                Music::Context(Context::Transpose {
                    from,
                    to,
                    music: Box::new(self.parse_music()),
                })
            }
            "tuplet" => {
                let (numerator, denominator) = self.parse_ratio().unwrap_or((3, 2));
                Music::Context(Context::Tuplet {
                    numerator,
                    denominator,
                    music: Box::new(self.parse_music()),
                })
            }
            "times" => {
                // `\times n/d` multiplie les durées : c'est un n-olet d/n.
                let (numerator, denominator) = self.parse_ratio().unwrap_or((2, 3));
                Music::Context(Context::Tuplet {
                    numerator: denominator,
                    denominator: numerator,
                    music: Box::new(self.parse_music()),
                })
            }
            "grace" | "slashedGrace" => Music::Context(Context::Grace {
                kind: GraceType::Grace8,
                slash: false,
                music: Box::new(self.parse_music()),
            }),
            "acciaccatura" => Music::Context(Context::Grace {
                kind: GraceType::Acciaccatura,
                slash: true,
                music: Box::new(self.parse_music()),
            }),
            "appoggiatura" => Music::Context(Context::Grace {
                kind: GraceType::Appoggiatura,
                slash: false,
                music: Box::new(self.parse_music()),
            }),
            "afterGrace" => {
                let main = Box::new(self.parse_music());
                let grace = Box::new(self.parse_music());
                Music::Context(Context::AfterGrace { main, grace })
            }
            "repeat" => self.parse_repeat(),
            "alternative" => {
                let content = self.parse_music();
                Music::Context(Context::Alternative(blocks_of(content)))
            }
            "key" => {
                let Some(pitch) = self.parse_optional_pitch() else {
                    return Music::Empty;
                };
                let mode = match self.bump() {
                    Some(Token {
                        kind: Kind::Command,
                        text,
                        ..
                    })
                    | Some(Token {
                        kind: Kind::Word,
                        text,
                        ..
                    }) => pitch::key_mode(&text),
                    _ => KeyMode::Major,
                };
                context(Context::Key {
                    fifths: pitch::fifths_of(pitch.step, pitch.alter),
                    mode,
                })
            }
            "time" => match self.parse_ratio() {
                Some((numerator, denominator)) => context(Context::Time {
                    numerator: i32::try_from(numerator).unwrap_or(4),
                    denominator: i32::try_from(denominator).unwrap_or(4),
                }),
                None => Music::Empty,
            },
            "clef" => {
                let name = match self.bump() {
                    Some(Token {
                        kind: Kind::Str,
                        text,
                        ..
                    })
                    | Some(Token {
                        kind: Kind::Word,
                        text,
                        ..
                    }) => text,
                    _ => String::new(),
                };
                match pitch::clef_from_name(&name) {
                    Some(clef) => context(Context::Clef(clef)),
                    None => {
                        self.warn(format!("clef inconnue : {name}"));
                        Music::Empty
                    }
                }
            }
            "partial" => match self.parse_duration() {
                Some(duration) => context(Context::Partial(duration)),
                None => Music::Empty,
            },
            "bar" => {
                let text = match self.bump() {
                    Some(Token {
                        kind: Kind::Str,
                        text,
                        ..
                    }) => text,
                    other => other.map_or_else(String::new, |token| token.text),
                };
                context(Context::Bar(text))
            }
            "section" => context(Context::Section),
            "break" | "noBreak" | "pageBreak" | "pageTurn" => context(Context::Break),
            "tempo" => self.parse_tempo(),
            "mark" => {
                let text = self.parse_markup_text();
                context(Context::Mark(text))
            }
            "ottava" => {
                let negative = self.eat_punct('-');
                if self.peek().is_some_and(|token| token.is_punct('#')) {
                    self.position += 1;
                }
                let magnitude = self
                    .bump()
                    .and_then(|token| token.as_integer())
                    .unwrap_or(0);
                let shift = if negative { -magnitude } else { magnitude };
                context(Context::Ottava(shift.clamp(-2, 2) as i8))
            }
            "sustainOn" => context(Context::Sustain(true)),
            "sustainOff" => context(Context::Sustain(false)),
            "voiceOne" => context(Context::Voice(0)),
            "voiceTwo" => context(Context::Voice(1)),
            "voiceThree" => context(Context::Voice(2)),
            "voiceFour" => context(Context::Voice(3)),
            "stemUp" => context(Context::Stem(StemDirection::Up)),
            "stemDown" => context(Context::Stem(StemDirection::Down)),
            "set" => self.parse_set(),
            "unset" => {
                self.skip_property_path();
                Music::Empty
            }
            "once" => {
                if self.eat_command("override") {
                    self.parse_override(true)
                } else {
                    self.parse_music()
                }
            }
            "override" => self.parse_override(false),
            "revert" => {
                self.skip_property_path();
                if self.peek().is_some_and(|token| token.is_punct('=')) {
                    self.position += 1;
                    self.skip_scheme_value();
                }
                Music::Empty
            }
            "tweak" => {
                self.parse_tweak();
                // Le `\tweak` porte sur la musique qui suit.
                self.parse_music()
            }
            "new" => match self.parse_new() {
                Some(group) => Music::New(Box::new(group)),
                None => Music::Empty,
            },
            "with" => {
                self.parse_with_block();
                self.parse_music()
            }
            "layout" | "midi" | "paper" | "header" | "markup" | "markuplist" => {
                self.skip_braced();
                Music::Empty
            }
            "lyricsto" | "addlyrics" | "lyricmode" => {
                // `\lyricmode { … }` isolé : rien à aligner.
                if self.eat_punct('{') {
                    let mut depth = 1usize;
                    while !self.at_end() && depth > 0 {
                        let Some(token) = self.bump() else {
                            break;
                        };
                        if token.is_punct('{') {
                            depth += 1;
                        } else if token.is_punct('}') {
                            depth -= 1;
                        }
                    }
                } else {
                    self.bump();
                }
                Music::Empty
            }
            "skip" => {
                let (duration, multiplier) = self.parse_optional_duration_spec();
                Music::Chord(Chord {
                    kind: ChordKind::Skip,
                    duration: Some(duration.unwrap_or(self.default_duration)),
                    multiplier,
                    posts: Vec::new(),
                })
            }
            "fermata" | "shortfermata" | "longfermata" | "verylongfermata" => Music::Empty,
            _ if is_articulation_command(&name) || is_dynamic_command(&name) => {
                // Événement isolé sans accord à décorer : ignoré.
                Music::Empty
            }
            _ => {
                self.warn(format!("commande ignorée : \\{name}"));
                self.skip_command_arguments(&name);
                Music::Unknown(name)
            }
        }
    }

    /// `\tempo` : texte puis éventuel `4 = 120`.
    fn parse_tempo(&mut self) -> Music {
        let mut text = String::new();
        if self.peek().is_some_and(|token| token.kind == Kind::Str) {
            if let Some(token) = self.bump() {
                text = token.text;
            }
        } else if self.peek().is_some_and(|token| token.is_command("markup")) {
            self.position += 1;
            text = self.parse_markup_text();
        } else if self.peek().is_some_and(|token| token.kind == Kind::Word) {
            if let Some(token) = self.bump() {
                text = token.text;
            }
        }
        // Unité de battement puis `= valeur`.
        let mut beats_per_minute = None;
        if self.peek().is_some_and(|token| token.is_command("breve")) {
            self.position += 1;
        } else if let Some(token) = self.peek().cloned() {
            if token.as_integer().is_some() || token.is_punct('.') {
                self.position += 1;
            }
        }
        if self.eat_punct('=') {
            if let Some(token) = self.bump() {
                beats_per_minute = token.as_decimal();
            }
        }
        Music::Context(Context::Tempo {
            text,
            beats_per_minute,
        })
    }

    /// `\repeat`.
    fn parse_repeat(&mut self) -> Music {
        let kind = match self.bump() {
            Some(Token {
                kind: Kind::Word,
                text,
                ..
            }) => RepeatKind::from_word(&text),
            _ => RepeatKind::Volta,
        };
        let count = match self.bump() {
            Some(Token {
                kind: Kind::Number,
                text,
                ..
            }) => text.parse().unwrap_or(2),
            _ => 2,
        };
        let body = Box::new(self.parse_music());
        let mut alternatives = Vec::new();
        if self.eat_command("alternative") {
            let content = self.parse_music();
            alternatives = blocks_of(content);
        }
        Music::Context(Context::Repeat {
            kind,
            count,
            body,
            alternatives,
        })
    }

    /// `\set`.
    fn parse_set(&mut self) -> Music {
        let path = self.skip_property_path();
        if !self.eat_punct('=') {
            return Music::Empty;
        }
        let fraction = self.parse_scheme_fraction();
        let is_measure_length = path
            .iter()
            .any(|part| part.eq_ignore_ascii_case("measurelength"));
        match (is_measure_length, fraction) {
            (true, Some(fraction)) => Music::Context(Context::MeasureLength(fraction)),
            _ => Music::Empty,
        }
    }

    /// `\override` et `\once \override`.
    fn parse_override(&mut self, once: bool) -> Music {
        let path = self.skip_property_path();
        let mut direction = None;
        if self.eat_punct('=') {
            let value_start = self.position;
            if self.peek().is_some_and(|token| token.is_punct('#')) {
                self.position += 1;
                let atom = self.parse_scheme_atom();
                if path.iter().any(|part| part == "direction") {
                    direction = match atom.to_ascii_uppercase().as_str() {
                        "UP" => Some(StemDirection::Up),
                        "DOWN" => Some(StemDirection::Down),
                        _ => None,
                    };
                }
            } else {
                self.position = value_start;
                self.skip_scheme_value();
            }
        }
        match (once, direction) {
            (true, Some(direction)) => Music::Context(Context::StemOverride(direction)),
            _ => Music::Empty,
        }
    }

    /// `\tweak` : propriété, valeur, puis musique décorée.
    fn parse_tweak(&mut self) {
        let path = self.skip_property_path();
        if !self.eat_punct('=') {
            return;
        }
        if !self.peek().is_some_and(|token| token.is_punct('#')) {
            self.skip_scheme_value();
            return;
        }
        self.position += 1;
        let value = self.parse_scheme_atom();
        if path.iter().any(|part| part == "style") {
            if let Some(group) = head_group_from_style(&value) {
                self.pending_head = group;
            }
        }
    }

    //------------------------------------------------------------------
    // Accords et durées
    //------------------------------------------------------------------

    /// Accord, silence, silence multi-mesures ou répétition d'accord.
    fn parse_chord(&mut self, in_chord: bool) -> Music {
        let Some(Token {
            kind: Kind::Word,
            text,
            ..
        }) = self.peek().cloned()
        else {
            return Music::Empty;
        };

        // Silences.
        if !in_chord {
            match text.as_str() {
                "r" | "R" | "s" => {
                    self.position += 1;
                    let (duration, multiplier) = self.parse_optional_duration_spec();
                    let posts = self.parse_posts();
                    return Music::Chord(Chord {
                        kind: match text.as_str() {
                            "R" => ChordKind::MultiRest,
                            "s" => ChordKind::Skip,
                            _ => ChordKind::Rest,
                        },
                        duration,
                        multiplier,
                        posts,
                    });
                }
                "q" => {
                    self.position += 1;
                    let (duration, multiplier) = self.parse_optional_duration_spec();
                    let posts = self.parse_posts();
                    return Music::Chord(Chord {
                        kind: ChordKind::Notes(self.last_notes.clone().unwrap_or_default()),
                        duration,
                        multiplier,
                        posts,
                    });
                }
                _ => {}
            }
        }

        let Some(pitch) = self.parse_pitch_spec() else {
            // Mot qui n'est pas une hauteur : variable ou symbole isolé.
            self.position += 1;
            if self.variables.contains_key(&text) {
                let music = self.variables[&text].clone();
                return music;
            }
            self.warn(format!("mot ignoré dans la musique : {text}"));
            return Music::Empty;
        };

        let head = std::mem::take(&mut self.pending_head);
        let mut note_posts = self.parse_posts_until_note_end();
        let notes = vec![NoteSpec {
            pitch,
            posts: std::mem::take(&mut note_posts),
            head_group: head,
        }];
        let (duration, multiplier) = self.parse_optional_duration_spec();
        // Les post-événements peuvent suivre la durée d'un accord.
        let posts = self.parse_posts();
        if !in_chord {
            self.last_notes = Some(notes.clone());
        }
        Music::Chord(Chord {
            kind: ChordKind::Notes(notes),
            duration,
            multiplier,
            posts,
        })
    }

    /// Accord `< … >` avec durée et post-événements.
    fn parse_chord_group(&mut self) -> Music {
        self.position += 1;
        let mut notes = Vec::new();
        while !self.at_end() && !self.eat_punct('>') {
            // `\tweak` dans l'accord décore la note suivante.
            if self.peek().is_some_and(|token| token.is_command("tweak")) {
                self.position += 1;
                self.parse_tweak();
                continue;
            }
            let Some(pitch) = self.parse_pitch_spec() else {
                self.position += 1;
                continue;
            };
            let head = std::mem::take(&mut self.pending_head);
            let posts = self.parse_posts_until_note_end();
            notes.push(NoteSpec {
                pitch,
                posts,
                head_group: head,
            });
        }
        if notes.is_empty() {
            return Music::Empty;
        }
        let (duration, multiplier) = self.parse_optional_duration_spec();
        let posts = self.parse_posts();
        self.last_notes = Some(notes.clone());
        Music::Chord(Chord {
            kind: ChordKind::Notes(notes),
            duration,
            multiplier,
            posts,
        })
    }

    /// Post-événements internes à un accord, jusqu'à la note suivante.
    fn parse_posts_until_note_end(&mut self) -> Vec<PostEvent> {
        let mut posts = Vec::new();
        loop {
            let Some(token) = self.peek().cloned() else {
                break;
            };
            if !starts_post_event(&token) {
                break;
            }
            let before = self.position;
            let mut parsed = self.parse_post_event();
            posts.append(&mut parsed);
            if self.position == before {
                break;
            }
        }
        posts
    }

    /// Post-événements après un accord.
    fn parse_posts(&mut self) -> Vec<PostEvent> {
        let mut posts = Vec::new();
        loop {
            let Some(token) = self.peek().cloned() else {
                break;
            };
            if !starts_post_event(&token) {
                break;
            }
            let before = self.position;
            let mut parsed = self.parse_post_event();
            posts.append(&mut parsed);
            if self.position == before {
                break;
            }
        }
        posts
    }

    /// Un post-événement, s'il commence ici.
    fn parse_post_event(&mut self) -> Vec<PostEvent> {
        let Some(token) = self.peek().cloned() else {
            return Vec::new();
        };
        match &token.kind {
            Kind::Punct('~') => {
                self.position += 1;
                vec![PostEvent::Tie]
            }
            Kind::Punct('(') => {
                self.position += 1;
                vec![PostEvent::SlurStart(0)]
            }
            Kind::Punct(')') => {
                self.position += 1;
                vec![PostEvent::SlurStop(0)]
            }
            Kind::Punct('[') | Kind::Punct(']') => {
                self.position += 1;
                Vec::new()
            }
            Kind::Punct(':') => {
                // Trémolo `:8`.
                self.position += 1;
                let value = self
                    .bump()
                    .and_then(|token| token.as_integer())
                    .unwrap_or(8);
                vec![PostEvent::Tremolo(
                    TremoloKind::Single,
                    u8::try_from(value.clamp(1, 1024)).unwrap_or(8),
                )]
            }
            Kind::Punct('^') | Kind::Punct('_') | Kind::Punct('-') => {
                let placement = if token.is_punct('_') || token.is_punct('-') {
                    Placement::Below
                } else {
                    Placement::Above
                };
                let direction = token.text.clone();
                self.position += 1;
                self.parse_direction_event(&direction, placement)
            }
            Kind::Command if token.text == "=" => {
                self.position += 1;
                let number = self
                    .peek()
                    .and_then(|token| token.as_integer())
                    .unwrap_or(0);
                if self.peek().is_some_and(|token| token.kind == Kind::Number) {
                    self.position += 1;
                }
                if self.eat_punct('(') {
                    vec![PostEvent::SlurStart(u8::try_from(number).unwrap_or(0))]
                } else if self.eat_punct(')') {
                    vec![PostEvent::SlurStop(u8::try_from(number).unwrap_or(0))]
                } else {
                    Vec::new()
                }
            }
            Kind::Command => {
                self.position += 1;
                self.parse_direction_event(&format!("\\{}", token.text), Placement::Below)
            }
            _ => Vec::new(),
        }
    }

    /// Événement qui suit `^`, `_` ou `-`.
    fn parse_direction_event(&mut self, direction: &str, placement: Placement) -> Vec<PostEvent> {
        let Some(token) = self.peek().cloned() else {
            return Vec::new();
        };
        // `-` suivi d'un autre tiret ou d'un symbole d'articulation.
        if direction == "-" {
            if let Kind::Punct(character) = token.kind {
                match character {
                    '-' => {
                        self.position += 1;
                        return vec![PostEvent::Articulation(ArticulationType::Tenuto)];
                    }
                    '.' => {
                        self.position += 1;
                        return vec![PostEvent::Articulation(ArticulationType::Staccato)];
                    }
                    '>' => {
                        self.position += 1;
                        return vec![PostEvent::Articulation(ArticulationType::Accent)];
                    }
                    '^' => {
                        self.position += 1;
                        return vec![PostEvent::Articulation(ArticulationType::Marcato)];
                    }
                    '!' => {
                        self.position += 1;
                        return vec![PostEvent::Articulation(ArticulationType::Staccatissimo)];
                    }
                    '_' => {
                        self.position += 1;
                        return vec![PostEvent::Articulation(ArticulationType::Portato)];
                    }
                    _ => {}
                }
            }
            // `-1` : doigté.
            if token.kind == Kind::Number {
                self.position += 1;
                return vec![PostEvent::Fingering(token.text)];
            }
            // `-\markup { … }` : doigté textuel.
            if token.is_command("markup") {
                self.position += 1;
                let text = self.parse_markup_text();
                if text.is_empty() {
                    return Vec::new();
                }
                return vec![PostEvent::Fingering(text)];
            }
            return Vec::new();
        }

        match &token.kind {
            Kind::Str => {
                self.position += 1;
                vec![PostEvent::Text(token.text.clone(), placement)]
            }
            Kind::Command if token.text == "markup" => {
                self.position += 1;
                let text = self.parse_markup_text();
                if text.is_empty() {
                    Vec::new()
                } else {
                    vec![PostEvent::Text(text, placement)]
                }
            }
            Kind::Command if token.text == "<" => {
                self.position += 1;
                vec![PostEvent::HairpinStart(true)]
            }
            Kind::Command if token.text == ">" => {
                self.position += 1;
                vec![PostEvent::HairpinStart(false)]
            }
            Kind::Command if token.text == "!" => {
                self.position += 1;
                vec![PostEvent::HairpinStop]
            }
            Kind::Command => {
                self.position += 1;
                let name = token.text.clone();
                if let Some(post) = post_event_for_command(&name, placement) {
                    return vec![post];
                }
                Vec::new()
            }
            Kind::Punct('(') => {
                self.position += 1;
                vec![PostEvent::SlurStart(0)]
            }
            Kind::Punct(')') => {
                self.position += 1;
                vec![PostEvent::SlurStop(0)]
            }
            Kind::Number => {
                self.position += 1;
                vec![PostEvent::Fingering(token.text)]
            }
            _ => Vec::new(),
        }
    }

    /// Hauteur écrite, si le jeton courant en est une.
    fn parse_optional_pitch(&mut self) -> Option<PitchSpec> {
        self.parse_pitch_spec()
    }

    /// Hauteur écrite : nom, altérations forcées, marques d'octave.
    fn parse_pitch_spec(&mut self) -> Option<PitchSpec> {
        let (step, alter) = match self.peek() {
            Some(Token {
                kind: Kind::Word,
                text,
                ..
            }) => self.language.parse(text)?,
            _ => return None,
        };
        self.position += 1;
        let mut pitch = PitchSpec {
            step,
            alter,
            octave: 0,
            explicit_octave: false,
            force: false,
            cautionary: false,
        };
        // `!` et `?` peuvent précéder ou suivre les marques d'octave.
        loop {
            match self.peek() {
                Some(token) if token.is_punct('!') => {
                    pitch.force = true;
                    self.position += 1;
                }
                Some(token) if token.is_punct('?') => {
                    pitch.cautionary = true;
                    self.position += 1;
                }
                Some(token) if token.is_punct('\'') => {
                    pitch.octave += 1;
                    pitch.explicit_octave = true;
                    self.position += 1;
                }
                Some(token) if token.is_punct(',') => {
                    pitch.octave -= 1;
                    pitch.explicit_octave = true;
                    self.position += 1;
                }
                Some(token) if token.is_punct('=') => {
                    // Contrôle d'octave `c=`: la hauteur suivante est absolue.
                    self.position += 1;
                    while let Some(next) = self.peek() {
                        if next.is_punct('\'') {
                            pitch.octave += 1;
                            pitch.explicit_octave = true;
                            self.position += 1;
                        } else if next.is_punct(',') {
                            pitch.octave -= 1;
                            pitch.explicit_octave = true;
                            self.position += 1;
                        } else {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
        Some(pitch)
    }

    /// Vrai si le mot est un nom de hauteur dans la langue courante.
    fn is_pitch_word(&self, word: &str) -> bool {
        self.language.parse(word).is_some()
    }

    /// Durée, si le jeton courant en porte une.
    fn parse_optional_duration(&mut self) -> Option<Duration> {
        self.parse_duration().map(|(duration, _)| duration)
    }

    /// Durée et multiplicateur, si le jeton courant en porte une.
    fn parse_optional_duration_spec(&mut self) -> (Option<Duration>, Option<Fraction>) {
        match self.parse_duration() {
            Some((duration, multiplier)) => (Some(duration), multiplier),
            None => (None, None),
        }
    }

    /// Durée écrite : `\breve`, `\longa`, chiffre, points, multiplicateur.
    fn parse_duration(&mut self) -> Option<(Duration, Option<Fraction>)> {
        let mut duration = if let Some(token) = self.peek().cloned() {
            if token.is_command("breve") {
                self.position += 1;
                Some(Duration::new(DurationType::Breve))
            } else if token.is_command("longa") || token.is_command("maxima") {
                self.position += 1;
                Some(Duration::new(DurationType::Long))
            } else if let Some(value) = token.as_integer() {
                self.position += 1;
                duration_from_value(value)
            } else {
                None
            }
        } else {
            None
        };
        let Some(mut duration) = duration.take() else {
            return None;
        };
        while self.eat_punct('.') {
            duration.dots = duration.dots.saturating_add(1);
        }
        self.default_duration = duration;
        // Multiplicateur `*n/d` (durée allongée).
        let mut multiplier = None;
        if self.eat_punct('*') {
            if let Some((numerator, denominator)) = self.parse_ratio() {
                multiplier = Fraction::try_new(i64::from(numerator), i64::from(denominator)).ok();
            }
        }
        Some((duration, multiplier))
    }

    /// Fraction `n/d` ou `n`.
    fn parse_ratio(&mut self) -> Option<(u32, u32)> {
        let token = self.peek()?.clone();
        if token.kind != Kind::Number {
            return None;
        }
        self.position += 1;
        let (numerator, denominator) = token.as_fraction()?;
        Some((
            u32::try_from(numerator).ok()?,
            u32::try_from(denominator).ok()?,
        ))
    }

    //------------------------------------------------------------------
    // Markup, Scheme et valeurs
    //------------------------------------------------------------------

    /// Texte d'un `\markup`.
    fn parse_markup_text(&mut self) -> String {
        let mut pieces = Vec::new();
        if self.peek().is_some_and(|token| token.is_punct('{')) {
            self.position += 1;
            let mut depth = 1usize;
            while !self.at_end() {
                if self.eat_punct('}') {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                } else if self.eat_punct('{') {
                    depth += 1;
                } else {
                    self.collect_markup_atom(&mut pieces);
                }
            }
        } else {
            // Markup sans accolades (`\markup \italic "x"`) : on collecte
            // tant que les jetons restent du texte.
            while let Some(token) = self.peek().cloned() {
                match token.kind {
                    Kind::Str | Kind::Command => self.collect_markup_atom(&mut pieces),
                    Kind::Word => {
                        if self.is_pitch_word(&token.text) {
                            break;
                        }
                        self.collect_markup_atom(&mut pieces);
                    }
                    Kind::Number => self.collect_markup_atom(&mut pieces),
                    _ => break,
                }
            }
        }
        pieces.join(" ")
    }

    /// Un atome de markup : chaîne, mot ou commande décorative.
    fn collect_markup_atom(&mut self, pieces: &mut Vec<String>) {
        match self.peek().cloned() {
            Some(Token {
                kind: Kind::Str,
                text,
                ..
            }) => {
                self.position += 1;
                pieces.push(text);
            }
            Some(Token {
                kind: Kind::Word,
                text,
                ..
            }) => {
                self.position += 1;
                pieces.push(text);
            }
            Some(Token {
                kind: Kind::Number,
                ..
            }) => {
                self.position += 1;
            }
            Some(token) if token.is_command("markup") => {
                self.position += 1;
            }
            Some(Token {
                kind: Kind::Command,
                ..
            }) => {
                // Commande de mise en forme (`\italic`, `\bold`…) : ses
                // arguments sont collectés normalement.
                self.position += 1;
            }
            Some(_) => {
                self.position += 1;
            }
            None => {}
        }
    }

    /// Valeur Scheme après `#` : atome ou liste entre parenthèses.
    fn parse_scheme_atom(&mut self) -> String {
        if self.eat_punct('(') {
            let mut depth = 1usize;
            let mut pieces = Vec::new();
            while !self.at_end() && depth > 0 {
                let Some(token) = self.bump() else {
                    break;
                };
                if token.is_punct('(') {
                    depth += 1;
                } else if token.is_punct(')') {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                } else if let Kind::Number | Kind::Word | Kind::Str = token.kind {
                    pieces.push(token.text);
                }
            }
            return pieces.join(" ");
        }
        if self.eat_punct('\'') {
            return self
                .bump()
                .map_or_else(String::new, |token| token.text);
        }
        if self.eat_punct('-') {
            return format!(
                "-{}",
                self.bump().map_or_else(String::new, |token| token.text)
            );
        }
        self.bump().map_or_else(String::new, |token| token.text)
    }

    /// Fraction d'une valeur Scheme (`#9/8`, `#(ly:make-moment 9/8)`).
    fn parse_scheme_fraction(&mut self) -> Option<Fraction> {
        if self.eat_punct('#') {
            return self.scheme_fraction_body();
        }
        if self.peek().is_some_and(|token| token.kind == Kind::Number) {
            let token = self.bump()?;
            return fraction_from_token(&token);
        }
        None
    }

    /// Corps d'une valeur Scheme numérique.
    fn scheme_fraction_body(&mut self) -> Option<Fraction> {
        if self.peek().is_some_and(|token| token.is_punct('(')) {
            let atom = self.parse_scheme_atom();
            let number = atom.split_whitespace().last()?.to_owned();
            return parse_fraction_text(&number);
        }
        let token = self.bump()?;
        fraction_from_token(&token)
    }

    /// Saute une valeur Scheme complète.
    fn skip_scheme_value(&mut self) {
        if self.eat_punct('#') || self.eat_punct('$') {
            let _ = self.parse_scheme_atom();
        } else if self.peek().is_some_and(|token| token.is_punct('(')) {
            let _ = self.parse_scheme_atom();
        } else {
            self.bump();
        }
    }

    /// Saute un chemin de propriété (`Stem.direction`), et le renvoie.
    fn skip_property_path(&mut self) -> Vec<String> {
        let mut parts = Vec::new();
        while let Some(token) = self.peek().cloned() {
            match &token.kind {
                Kind::Word => {
                    parts.push(token.text.clone());
                    self.position += 1;
                }
                Kind::Punct('.') => {
                    self.position += 1;
                }
                _ => break,
            }
        }
        parts
    }

    /// Saute les arguments heuristiques d'une commande inconnue.
    fn skip_command_arguments(&mut self, name: &str) {
        if name.is_empty() {
            return;
        }
        loop {
            let Some(token) = self.peek().cloned() else {
                return;
            };
            match &token.kind {
                Kind::Punct('{') => {
                    self.skip_braced();
                    return;
                }
                Kind::Str | Kind::Number => {
                    self.position += 1;
                }
                Kind::Punct('#') | Kind::Punct('$') => {
                    self.position += 1;
                    self.skip_scheme_value();
                    return;
                }
                Kind::Word => {
                    // Un nom de note est un argument musical : on le laisse.
                    if self.is_pitch_word(&token.text) {
                        return;
                    }
                    self.position += 1;
                    return;
                }
                _ => return,
            }
        }
    }

    /// Saute un bloc `{ … }` ou `<< … >>` équilibré.
    fn skip_braced(&mut self) {
        let mut depth = 0usize;
        let mut angle = 0usize;
        while let Some(token) = self.peek().cloned() {
            if token.is_punct('{') {
                depth += 1;
                self.position += 1;
            } else if token.is_punct('}') {
                self.position += 1;
                if depth > 0 {
                    depth -= 1;
                    if depth == 0 && angle == 0 {
                        return;
                    }
                } else {
                    return;
                }
            } else if self.at_double_angle_open() {
                angle += 1;
                self.position += 2;
            } else if self.at_double_angle_close() {
                self.position += 2;
                if angle > 0 {
                    angle -= 1;
                    if angle == 0 && depth == 0 {
                        return;
                    }
                }
            } else {
                self.position += 1;
            }
        }
    }

    /// Saute un unique jeton de musique.
    fn skip_music_token(&mut self) {
        if self.at_double_angle_open() {
            self.skip_braced();
        } else if self.peek().is_some_and(|token| token.is_punct('{')) {
            self.skip_braced();
        } else {
            self.bump();
        }
    }
}

/// Vrai si le jeton commence un post-événement (`post_events`).
fn starts_post_event(token: &Token) -> bool {
    match &token.kind {
        Kind::Punct(character) => matches!(character, '~' | '(' | ')' | '[' | ']' | ':' | '^' | '_' | '-'),
        Kind::Command => {
            token.text == "="
                || token.text == "<"
                || token.text == ">"
                || token.text == "!"
                || token.text == "markup"
                || is_articulation_command(&token.text)
                || is_dynamic_command(&token.text)
        }
        _ => false,
    }
}

/// Texte d'un jeton, s'il en porte un.
fn token_text(token: &Token) -> Option<String> {
    match token.kind {
        Kind::Word => Some(token.text.clone()),
        Kind::Command => Some(token.text.clone()),
        _ => None,
    }
}

/// Durée d'un chiffre de figure (`4`, `8`, `1`…).
fn duration_from_value(value: i64) -> Option<Duration> {
    let kind = match value {
        1 => DurationType::Whole,
        2 => DurationType::Half,
        4 => DurationType::Quarter,
        8 => DurationType::Eighth,
        16 => DurationType::Sixteenth,
        32 => DurationType::ThirtySecond,
        64 => DurationType::SixtyFourth,
        128 => DurationType::OneHundredTwentyEighth,
        256 => DurationType::TwoHundredFiftySixth,
        512 => DurationType::FiveHundredTwelfth,
        1024 => DurationType::OneThousandTwentyFourth,
        _ => return None,
    };
    Some(Duration::new(kind))
}

/// Fraction d'un jeton numérique.
fn fraction_from_token(token: &Token) -> Option<Fraction> {
    let (numerator, denominator) = token.as_fraction()?;
    Fraction::try_new(numerator, denominator).ok()
}

/// Fraction d'un texte `n/d` ou `n`.
fn parse_fraction_text(text: &str) -> Option<Fraction> {
    match text.split_once('/') {
        Some((numerator, denominator)) => Fraction::try_new(
            numerator.parse().ok()?,
            denominator.parse().ok()?,
        )
        .ok(),
        None => Fraction::try_new(text.parse().ok()?, 1).ok(),
    }
}

/// Vrai pour une commande d'articulation connue.
fn is_articulation_command(name: &str) -> bool {
    matches!(
        name,
        "staccato"
            | "tenuto"
            | "accent"
            | "marcato"
            | "portato"
            | "espressivo"
            | "staccatissimo"
            | "fermata"
            | "shortfermata"
            | "longfermata"
            | "verylongfermata"
            | "breathe"
            | "caesura"
            | "upbow"
            | "downbow"
            | "flageolet"
            | "open"
            | "stopped"
            | "snappizzicato"
            | "trill"
            | "turn"
            | "reverseturn"
            | "slashturn"
            | "mordent"
            | "prall"
            | "prallprall"
            | "haydnturn"
            | "upmordent"
            | "arpeggio"
            | "arpeggioArrowUp"
            | "arpeggioArrowDown"
            | "arpeggioBracket"
            | "glissando"
            | "startTrillSpan"
            | "stopTrillSpan"
    )
}

/// Vrai pour une nuance prédéfinie de LilyPond.
fn is_dynamic_command(name: &str) -> bool {
    matches!(
        name,
        "ppppp"
            | "pppp"
            | "ppp"
            | "pp"
            | "p"
            | "mp"
            | "mf"
            | "f"
            | "ff"
            | "fff"
            | "ffff"
            | "fffff"
            | "fp"
            | "sf"
            | "sfp"
            | "sff"
            | "sfz"
            | "fz"
            | "sp"
            | "spp"
            | "rfz"
            | "cresc"
            | "decresc"
            | "dim"
    )
}

/// Événement d'une commande de post-traitement connue.
fn post_event_for_command(name: &str, placement: Placement) -> Option<PostEvent> {
    let event = match name {
        "staccato" => PostEvent::Articulation(ArticulationType::Staccato),
        "staccatissimo" => PostEvent::Articulation(ArticulationType::Staccatissimo),
        "accent" => PostEvent::Articulation(ArticulationType::Accent),
        "marcato" => PostEvent::Articulation(ArticulationType::Marcato),
        "tenuto" => PostEvent::Articulation(ArticulationType::Tenuto),
        "portato" => PostEvent::Articulation(ArticulationType::Portato),
        "espressivo" => PostEvent::Articulation(ArticulationType::SoftAccent),
        "breathe" => PostEvent::Articulation(ArticulationType::BreathMark),
        "caesura" => PostEvent::Articulation(ArticulationType::Caesura),
        "upbow" => PostEvent::Articulation(ArticulationType::UpBow),
        "downbow" => PostEvent::Articulation(ArticulationType::DownBow),
        "flageolet" => PostEvent::Articulation(ArticulationType::Harmonic),
        "open" => PostEvent::Articulation(ArticulationType::Open),
        "stopped" => PostEvent::Articulation(ArticulationType::Stopped),
        "snappizzicato" => PostEvent::Articulation(ArticulationType::SnapPizzicato),
        "fermata" => PostEvent::Articulation(ArticulationType::Fermata),
        "shortfermata" | "verylongfermata" => {
            PostEvent::Articulation(ArticulationType::FermataShort)
        }
        "longfermata" => PostEvent::Articulation(ArticulationType::FermataLong),
        "trill" => PostEvent::Ornament(OrnamentType::TrillMark),
        "turn" => PostEvent::Ornament(OrnamentType::Turn),
        "reverseturn" => PostEvent::Ornament(OrnamentType::InvertedTurn),
        "slashturn" => PostEvent::Ornament(OrnamentType::TurnSlash),
        "mordent" => PostEvent::Ornament(OrnamentType::Mordent),
        "prall" => PostEvent::Ornament(OrnamentType::InvertedMordent),
        "prallprall" => PostEvent::Ornament(OrnamentType::InvertedMordentLong),
        "haydnturn" => PostEvent::Ornament(OrnamentType::Haydn),
        "upmordent" => PostEvent::Ornament(OrnamentType::Schleifer),
        "arpeggio" => PostEvent::Arpeggio(ArpeggioType::Normal),
        "arpeggioArrowUp" => PostEvent::Arpeggio(ArpeggioType::Up),
        "arpeggioArrowDown" => PostEvent::Arpeggio(ArpeggioType::Down),
        "arpeggioBracket" => PostEvent::Arpeggio(ArpeggioType::Bracket),
        "glissando" => PostEvent::Glissando,
        "startTrillSpan" => PostEvent::TrillStart,
        "stopTrillSpan" => PostEvent::TrillStop,
        "bendAfter" => PostEvent::BendAfter(0),
        "cresc" => PostEvent::HairpinStart(true),
        "decresc" | "dim" => PostEvent::HairpinStart(false),
        name if is_dynamic_command(name) => PostEvent::Dynamic(name.to_owned(), placement),
        _ => return None,
    };
    Some(event)
}

/// Groupe de tête correspondant à un style de `\tweak`.
fn head_group_from_style(style: &str) -> Option<NoteHeadGroup> {
    let group = match style {
        "cross" => NoteHeadGroup::Cross,
        "harmonic-mixed" | "harmonic" => NoteHeadGroup::Diamond,
        "slash" => NoteHeadGroup::Slash,
        "xcircle" => NoteHeadGroup::XCircle,
        "do" => NoteHeadGroup::Do,
        "re" => NoteHeadGroup::Re,
        "mi" => NoteHeadGroup::Mi,
        "fa" => NoteHeadGroup::Fa,
        "sol" => NoteHeadGroup::Sol,
        "la" => NoteHeadGroup::La,
        "ti" => NoteHeadGroup::Ti,
        "triangle" => NoteHeadGroup::TriangleUp,
        "mensural" => NoteHeadGroup::Other("mensural".to_owned()),
        _ => return None,
    };
    Some(group)
}

/// Découpe un `\alternative { { … } { … } }` en blocs.
fn blocks_of(music: Music) -> Vec<Music> {
    match music {
        Music::Seq(items) => items,
        Music::Sim(items) => items,
        Music::Empty => Vec::new(),
        other => vec![other],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_music_only(source: &str) -> Music {
        let mut warnings = Warnings::new();
        let mut parser = Parser {
            tokens: tokenize(source),
            position: 0,
            language: Language::English,
            variables: BTreeMap::new(),
            last_notes: None,
            default_duration: Duration::quarter(),
            warnings: &mut warnings,
            pending_head: NoteHeadGroup::Normal,
        };
        parser.parse_music()
    }

    #[test]
    fn accord_avec_duree_et_liaison() {
        let music = parse_music_only("c'4~ d8.");
        let Music::Seq(items) = music else {
            panic!("séquence attendue");
        };
        assert_eq!(items.len(), 2);
        let Music::Chord(first) = &items[0] else {
            panic!("accord attendu");
        };
        assert!(first.posts.contains(&PostEvent::Tie));
        assert_eq!(first.duration, Some(Duration::quarter()));
    }

    #[test]
    fn accord_multi_notes() {
        let music = parse_music_only("<c e g>2");
        let Music::Chord(chord) = music else {
            panic!("accord attendu");
        };
        let ChordKind::Notes(notes) = &chord.kind else {
            panic!("notes attendues");
        };
        assert_eq!(notes.len(), 3);
        assert_eq!(chord.duration, Some(Duration::half()));
    }

    #[test]
    fn commandes_de_contexte() {
        let music = parse_music_only("\\key g \\major \\time 3/4 \\clef bass");
        let Music::Seq(items) = music else {
            panic!("séquence attendue");
        };
        assert!(items.iter().any(|item| matches!(
            item,
            Music::Context(Context::Key { fifths: 1, mode: KeyMode::Major })
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            Music::Context(Context::Time {
                numerator: 3,
                denominator: 4
            })
        )));
    }

    #[test]
    fn n_olet_et_reprise() {
        let music = parse_music_only("\\tuplet 3/2 { c8 d8 e8 } \\repeat volta 2 { f4 g4 }");
        let Music::Seq(items) = music else {
            panic!("séquence attendue");
        };
        assert!(matches!(items[0], Music::Context(Context::Tuplet { .. })));
        assert!(matches!(
            items[1],
            Music::Context(Context::Repeat {
                kind: RepeatKind::Volta,
                count: 2,
                ..
            })
        ));
    }
}
