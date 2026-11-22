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

use scoretk_core::model::{
    ArpeggioType, ArticulationType, Duration, DurationType, Fraction, GraceType, KeyMode,
    NoteHeadGroup, OrnamentType, Placement, StemDirection, TremoloKind, TupletNumberType,
};

use crate::ast::{
    Chord, ChordKind, Context, Document, GroupBlock, GroupKind, HeaderAst, LyricsBlock, Music,
    NoteSpec, PitchSpec, PostEvent, RepeatKind, ScoreAst, StaffBlock, Syllable, TagFilter,
    VoiceBlock,
};
use crate::lexer::{self, Kind, Token};
use crate::pitch::{self, Language};
use crate::warnings::Warnings;
use scoretk_lilypond_common::clefs;

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
    /// Durée par défaut (celle du dernier jeton de durée).
    default_duration: Duration,
    /// Multiplicateur `*n/d` de cette durée par défaut : chez LilyPond le
    /// facteur fait partie de `parser->default_duration_` (`Duration`,
    /// `lily/parser.yy` l. 4644), donc une note nue hérite aussi de lui.
    default_multiplier: Fraction,
    /// Avertissements collectés.
    warnings: &'a mut Warnings,
    /// Groupe de tête posé par `\tweak style`.
    pending_head: NoteHeadGroup,
    /// Affichage du nombre du prochain n-olet (`\override TupletNumber…`).
    pending_tuplet_number: Option<TupletNumberType>,
    /// Silence invisible demandé par `\once \override Rest.transparent`.
    pending_invisible_rest: bool,
    /// Partitions rencontrées au niveau supérieur, dans l'ordre du fichier.
    partitions: Vec<Partition>,
    /// `\transposition` trouvé dans le bloc `\midi` de la partition courante.
    midi_transposition: Option<PitchSpec>,
    /// `\articulate` a été rencontré (`ly/articulate.ly`).
    playback_articulate: bool,
    /// `\unfoldRepeats` a été rencontré : le MIDI déplie les reprises.
    playback_unfold_repeats: bool,
    /// Fonctions musicales définies en Scheme (`define-music-function`…).
    music_functions: BTreeMap<String, MusicFunction>,
    /// Profondeur d'expansion des blocs embarqués et des fonctions.
    expansion_depth: u32,
    /// Fraction par défaut d'un `\afterGrace` sans argument
    /// (`afterGraceFraction`, 3/4 dans `ly/music-functions-init.ly`).
    after_grace_fraction: Fraction,
    /// `\set melismaBusyProperties = #'()` a été vu : les paroles ne sautent
    /// plus les notes occupées (liaisons d'expression, liaisons…).
    melisma_busy_disabled: bool,
    /// `\layout` déplace explicitement `Timing_translator` de `Score` vers
    /// `Staff` : les indications `\time` deviennent propres à chaque portée.
    staff_local_timing: bool,
    /// Mode courant des noms de notes (`\drummode` ou `\chordmode`), qui
    /// remplace la notation diatonique par des événements à hauteur MIDI.
    note_mode: NoteMode,
    /// Vrai pendant l'expansion du corps d'une fonction `make-relative` : un
    /// `\transpose` y reçoit des hauteurs déjà lues dans le `\relative`
    /// environnant, et non des hauteurs absolues.
    in_relative_function: bool,
    /// Groupes de tags déclarés par `\tagGroup` (Notation Reference §22.2.2).
    /// Les deux groupes internes de LilyPond sont posés par défaut dans
    /// `scm/music-functions.scm`.
    tag_groups: Vec<Vec<String>>,
    /// Variables musicales définies en Scheme (`#(define nom #{ … #})`) et
    /// insérées par `$nom` (Extending §1).
    scheme_variables: BTreeMap<String, Music>,
}

/// Profondeur d'expansion maximale (garde-fou contre la récursion).
const MAX_EXPANSION_DEPTH: u32 = 32;

/// Interprétation des noms de notes dans la musique courante.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum NoteMode {
    /// Noms de notes ordinaires (`a`, `bes`, `c'`…).
    #[default]
    Notes,
    /// Noms de percussions (`bd`, `sn`, `hh`…), portés par `\drummode`.
    Drums,
    /// Noms d'accords (`c:m7`, `c/g`…), portés par `\chordmode` dans une
    /// portée : LilyPond les joue comme des notes.
    Chords,
}

/// Nature d'une fonction définie en Scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FunctionKind {
    /// `define-music-function` : produit de la musique.
    Music,
    /// `define-void-function` / `define-scheme-function` : rien à lire.
    Silent,
}

/// Fonction musicale mémorisée à sa définition.
#[derive(Clone, Debug)]
struct MusicFunction {
    /// Nature de la définition.
    kind: FunctionKind,
    /// Noms des paramètres, dans l'ordre.
    params: Vec<String>,
    /// Prédicats d'argument, un par paramètre.
    predicates: Vec<String>,
    /// Corps musical (`#{ … #}`), absent pour une fonction Scheme pure.
    body: Option<Vec<Token>>,
    /// Vrai si la définition utilise `make-relative` : les arguments sont
    /// lus dans le contexte relatif environnant avant d'être insérés dans le
    /// corps (`scm/music-functions.scm`), y compris sous un `\transpose`.
    relative_body: bool,
    /// Vrai si l'avertissement d'appel a déjà été émis.
    warned: bool,
}

/// Une partition lue au niveau supérieur du fichier.
struct Partition {
    /// Arbre de la partition.
    score: ScoreAst,
    /// Vrai pour un bloc `\score` explicite.
    explicit: bool,
    /// Blocs de sortie vus dans ce `\score` (§24.3 : `\midi` sans `\layout`
    /// signifie une partition qui ne produit que du MIDI).
    has_layout: bool,
    has_midi: bool,
    /// `\unfoldRepeats` vu dans cette partition seulement.
    unfold_repeats: bool,
    /// Ligne du premier jeton.
    line: u32,
    /// Colonne du premier jeton.
    column: u32,
}

/// Groupes de tags posés au départ par LilyPond (`scm/music-functions.scm`) :
/// les deux tags internes utilisés par `\autoChange` et `\partCombine`.
fn default_tag_groups() -> Vec<Vec<String>> {
    vec![
        vec!["$autoChange".to_owned()],
        vec!["$partCombine".to_owned()],
    ]
}

/// Groupe auquel appartient un tag, d'après `\tagGroup` (Notation Reference
/// §22.2.2). `None` correspond à un tag sans groupe, ce qui compte dans la
/// comparaison : `tags-keep-predicate` de `scm/music-functions.scm` compare
/// aussi les groupes absents (Guile `#f`).
fn tag_group_of(name: &str, groups: &[Vec<String>]) -> Option<usize> {
    groups
        .iter()
        .position(|group| group.iter().any(|tag| tag == name))
}

/// Vrai si une expression portant `tags` survit au filtre.
///
/// Portage de `tags-keep-predicate` et `tags-remove-predicate`
/// (`scm/music-functions.scm`) : une expression sans tag est toujours gardée ;
/// `\removeWithTag` retire les expressions qui portent un des tags nommés ;
/// `\keepWithTag` garde les expressions qui portent un des tags nommés, ou
/// dont aucun tag n'appartient à un groupe des tags nommés.
pub(crate) fn tags_kept(
    tags: &[String],
    wanted: &[String],
    filter: TagFilter,
    groups: &[Vec<String>],
) -> bool {
    if tags.is_empty() {
        return true;
    }
    match filter {
        TagFilter::Remove => !tags.iter().any(|tag| wanted.contains(tag)),
        TagFilter::Keep => {
            if tags.iter().any(|tag| wanted.contains(tag)) {
                return true;
            }
            let wanted_groups: Vec<_> = wanted
                .iter()
                .map(|name| tag_group_of(name, groups))
                .collect();
            !tags
                .iter()
                .any(|tag| wanted_groups.contains(&tag_group_of(tag, groups)))
        }
    }
}

/// Noms de tags d'un argument Scheme : `#'a`, `#'(a b)`, ou une liste à
/// virgules. Les guillemets Scheme sont retirés (`scm/music-functions.scm`
/// attend des symboles, pas des chaînes).
fn scheme_tag_names(datum: &str) -> Vec<String> {
    let mut text = datum.trim();
    if let Some(rest) = text.strip_prefix('\'') {
        text = rest.trim();
    }
    if let Some(inner) = text
        .strip_prefix('(')
        .and_then(|part| part.strip_suffix(')'))
    {
        split_scheme_words(inner)
            .into_iter()
            .map(|word| word.trim_start_matches('\'').trim().to_owned())
            .filter(|word| !word.is_empty())
            .collect()
    } else {
        let name = text.trim_start_matches('\'').trim();
        if name.is_empty() {
            Vec::new()
        } else {
            vec![name.to_owned()]
        }
    }
}

/// Résultat de l'analyse.
pub(crate) fn parse(
    source: &str,
    resolve: &mut dyn FnMut(&str, Option<&str>) -> Option<String>,
    warnings: &mut Warnings,
) -> Document {
    let tokens = crate::include::expand(source, resolve, warnings);
    let mut parser = Parser {
        tokens,
        position: 0,
        language: Language::default(),
        variables: BTreeMap::new(),
        default_duration: Duration::quarter(),
        default_multiplier: Fraction::ONE,
        warnings,
        pending_head: NoteHeadGroup::Normal,
        pending_tuplet_number: None,
        pending_invisible_rest: false,
        partitions: Vec::new(),
        midi_transposition: None,
        playback_articulate: false,
        playback_unfold_repeats: false,
        music_functions: BTreeMap::new(),
        expansion_depth: 0,
        melisma_busy_disabled: false,
        after_grace_fraction: Fraction::new(3, 4),
        staff_local_timing: false,
        note_mode: NoteMode::Notes,
        in_relative_function: false,
        tag_groups: default_tag_groups(),
        scheme_variables: BTreeMap::new(),
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

    /// Garde de progression pour les boucles qui consomment des jetons.
    ///
    /// `before` est la position en début d'itération. Renvoie `false` quand la
    /// boucle n'a pas avancé : le jeton bloquant est alors consommé (s'il en
    /// reste un) et un avertissement est émis, pour que l'appelant sorte de sa
    /// boucle locale au lieu de tourner en accumulant des éléments vides — le
    /// `|` non consommé faisait encore boucler l'analyse (c79a320).
    fn guard_progress(&mut self, before: usize) -> bool {
        if self.position > before {
            return true;
        }
        self.warn("analyse bloquée sur un jeton : progression forcée");
        if !self.at_end() {
            self.position += 1;
        }
        false
    }

    //------------------------------------------------------------------
    // Document
    //------------------------------------------------------------------

    /// Analyse un document complet.
    fn document(&mut self) -> Document {
        let mut document = Document {
            language: "nederlands".to_owned(),
            ..Document::default()
        };

        while !self.at_end() {
            let before = self.position;
            self.toplevel_item(&mut document);
            self.guard_progress(before);
        }
        document.melisma_busy_disabled = self.melisma_busy_disabled;
        document.staff_local_timing = self.staff_local_timing;
        document.playback_articulate = self.playback_articulate;
        document.playback_unfold_repeats = self.playback_unfold_repeats;

        // Choix de la partition : un bloc `\score` explicite l'emporte sur la
        // musique de haut niveau ; à égalité, la première rencontrée gagne.
        // Le MusicXML représente la partition gravée : même quand un autre
        // `\score` porte un `\midi` (partition que LilyPond joue, transposée),
        // c'est le premier `\score` gravé qui fait foi.
        let engraved = self
            .partitions
            .iter()
            .position(|partition| partition.explicit)
            .or(if self.partitions.is_empty() {
                None
            } else {
                Some(0)
            });
        if let Some(engraved) = engraved {
            // Une seconde partition `\midi` sans `\layout` joue la même
            // musique que la partition gravée, privée de `\transpose`
            // (Notation Reference §1.2.2, §24.3). LilyPond n'écrit qu'un MIDI,
            // celui de cette partition : c'est donc elle qui fournit le modèle
            // et sa hauteur de concert, quand elle est structurellement
            // identique à la partition gravée. Sinon on garde la règle de §7
            // (premier score gravé) et on ne retient qu'un intervalle pour
            // l'écrivain MIDI.
            let midi = self
                .partitions
                .iter()
                .enumerate()
                .find(|(index, partition)| {
                    *index != engraved && partition.has_midi && !partition.has_layout
                })
                .map(|(index, _)| index);
            let detected = self.partitions.get(engraved).and_then(|score| {
                midi.and_then(|index| {
                    crate::playback::detect(&score.score, &self.partitions[index].score)
                })
            });
            // LilyPond écrit un MIDI par score qui porte `\midi`, dans
            // l'ordre du fichier ; l'oracle retient le premier produit. Si la
            // partition gravée porte elle-même `\midi`, c'est son MIDI
            // (transposé) qui fait référence : on garde alors le score gravé.
            // Sinon, le premier score `\midi` sans `\layout` est le seul
            // joué (AStMichel : 666 notes contre 130 gravées) et fournit le
            // modèle, même si sa musique diffère du score gravé.
            let first_midi = self
                .partitions
                .iter()
                .position(|partition| partition.has_midi);
            let chosen = first_midi.unwrap_or(engraved);
            document.playback_transposition = if chosen == engraved { detected } else { None };
            for (index, partition) in self.partitions.iter().enumerate() {
                if index != chosen {
                    self.warnings.push(
                        partition.line,
                        partition.column,
                        "partition supplémentaire ignorée",
                    );
                }
            }
            if self.partitions.get(chosen).is_some() {
                let partition = self.partitions.swap_remove(chosen);
                document.playback_unfold_repeats = partition.unfold_repeats;
                document.score = Some(partition.score);
            }
        }
        document
    }

    /// Un élément du niveau supérieur du fichier (ou d'un `\book`) : en-tête,
    /// bloc `\score`, `\book`, commande ignorée ou expression musicale.
    fn toplevel_item(&mut self, document: &mut Document) {
        let Some(token) = self.peek().cloned() else {
            return;
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
                // LilyPond accepte le nom entre guillemets (`\language
                // "deutsch"`, `ly/music-functions-init.ly:982-993`) ou comme
                // simple mot (`\language deutsch`, écrit par certains
                // fichiers, dont `Lassus_Alleluja`).
                if let Some(Token {
                    kind: Kind::Str | Kind::Word,
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
                let (score, has_layout, has_midi, unfold_repeats) = self.parse_score();
                self.partitions.push(Partition {
                    score,
                    explicit: true,
                    has_layout,
                    has_midi,
                    unfold_repeats,
                    line: token.line,
                    column: token.column,
                });
            }
            Kind::Command if token.text == "book" || token.text == "bookpart" => {
                self.position += 1;
                self.parse_book(document);
            }
            Kind::Command if token.text == "markup" || token.text == "markuplist" => {
                self.position += 1;
                self.parse_markup_text();
            }
            // `\parallelMusic voix… { musique }` définit une variable par voix
            // (`ly/music-functions-init.ly`) : c'est une commande de haut
            // niveau, comme `\score`, et non une musique à jouer.
            Kind::Command if token.text == "parallelMusic" => {
                self.position += 1;
                self.parse_parallel_music();
            }
            Kind::Word => {
                // Assignation `nom = musique` ou mot isolé.
                if self.peek_at(1).is_some_and(|next| next.is_punct('=')) {
                    let name = token.text.clone();
                    self.position += 2;
                    self.assign(name);
                } else if self.at_toplevel_music() {
                    self.parse_toplevel_music(&token);
                } else {
                    self.position += 1;
                    self.warn(format!("mot hors contexte ignoré : {}", token.text));
                }
            }
            Kind::Command => {
                if self.at_toplevel_music() {
                    self.parse_toplevel_music(&token);
                } else {
                    let debut_arguments = self.position + 1;
                    self.position += 1;
                    // Au niveau supérieur, un bloc `{ … }` après une commande
                    // comme `\layout` ou `\paper` est un réglage, pas de la
                    // musique : il doit être sauté.
                    self.skip_command_arguments(&token.text, true);
                    if token.text == "layout" && self.timing_translator_deplacé(debut_arguments) {
                        self.staff_local_timing = true;
                    }
                }
            }
            // `lily/parser.yy` accepte une chaîne comme identifiant :
            // `"|" = \bar "||"` redéfinit le contrôle de mesure. Sans ce cas,
            // la chaîne est sautée et `=` puis `\bar` avalent la musique qui
            // suit (bar-check-redefine.ly).
            Kind::Str if self.peek_at(1).is_some_and(|next| next.is_punct('=')) => {
                let name = token.text.clone();
                self.position += 2;
                self.assign(name);
            }
            Kind::Punct(';') => {
                self.position += 1;
            }
            // Expression Scheme isolée : lue comme un seul jeton. Seules les
            // définitions `#(define nom #{ … #})` ont un effet (variable
            // musicale) ; les réglages comme `#(ly:set-option …)` ou
            // `#(set-global-staff-size n)` restent ignorés sans bruit.
            Kind::Scheme => {
                let datum = token.text.clone();
                self.position += 1;
                if !self.record_after_grace_fraction(&datum) {
                    let _ = self.record_scheme_define(&datum);
                }
            }
            // Musique embarquée au niveau supérieur : une expression musicale.
            Kind::Embedded | Kind::Punct('{') | Kind::Punct('<') => {
                self.parse_toplevel_music(&token);
            }
            _ => {
                self.position += 1;
            }
        }
    }

    /// Bloc `\book { … }` ou `\bookpart { … }` : mêmes éléments qu'au niveau
    /// du fichier, `\score` et musique de haut niveau compris.
    fn parse_book(&mut self, document: &mut Document) {
        if !self.eat_punct('{') {
            return;
        }
        while !self.at_end() && !self.eat_punct('}') {
            let before = self.position;
            self.toplevel_item(document);
            self.guard_progress(before);
        }
    }

    /// Vrai si le jeton courant ouvre une expression musicale de haut niveau
    /// (partition implicite), telle que `parse_music` l'accepte.
    fn at_toplevel_music(&self) -> bool {
        if self.at_double_angle_open() {
            return true;
        }
        match self.peek().map(|token| &token.kind) {
            Some(Kind::Embedded) => true,
            Some(Kind::Punct('{') | Kind::Punct('<')) => true,
            Some(Kind::Command) => {
                let text = self
                    .peek()
                    .map(|token| token.text.as_str())
                    .unwrap_or_default();
                self.variables.contains_key(text)
                    || self.music_functions.contains_key(text)
                    || starts_music_command(text)
            }
            Some(Kind::Word) => {
                let text = self
                    .peek()
                    .map(|token| token.text.as_str())
                    .unwrap_or_default();
                self.variables.contains_key(text)
                    || matches!(text, "r" | "R" | "s" | "q")
                    || self.is_pitch_word(text)
            }
            _ => false,
        }
    }

    /// Expression musicale de haut niveau : une partition implicite, comme le
    /// `scorify-music` de `toplevel-music-handler`.
    fn parse_toplevel_music(&mut self, token: &Token) {
        // Comme pour un `\score` explicite, `\unfoldRepeats` lu dans cette
        // musique de haut niveau ne concerne que cette partition implicite.
        self.playback_unfold_repeats = false;
        let mut groups = self.parse_score_element();
        let unfold_repeats = self.playback_unfold_repeats;
        if groups.is_empty() {
            return;
        }
        if let Some(staff) = groups.last_mut().and_then(|group| group.staves.last_mut()) {
            self.collect_addlyrics(staff);
        }
        self.partitions.push(Partition {
            score: ScoreAst {
                groups,
                lyrics: Vec::new(),
            },
            explicit: false,
            has_layout: false,
            has_midi: false,
            unfold_repeats,
            line: token.line,
            column: token.column,
        });
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
            let Some(name) = token_text(&token) else {
                continue;
            };
            if !self.eat_punct('=') {
                continue;
            }
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
            Some(token) if token.is_scheme() => {
                // Valeur Scheme (`##f`…) : consommée, sans texte d'en-tête.
                self.position += 1;
                String::new()
            }
            _ => String::new(),
        }
    }

    //------------------------------------------------------------------
    // Bloc \score et contextes
    //------------------------------------------------------------------

    /// Bloc `\score { … }`, avec les blocs de sortie vus (`\layout`, `\midi`).
    fn parse_score(&mut self) -> (ScoreAst, bool, bool, bool) {
        let mut score = ScoreAst::default();
        let mut has_layout = false;
        let mut has_midi = false;
        self.midi_transposition = None;
        // `\unfoldRepeats` est propre au score courant
        // (`scm/music-functions.scm`) : un second `\score` qui déplie ses
        // reprises ne doit pas déplier celles du premier, dont l'oracle
        // retient le MIDI (`scm/midi.scm`, `base.midi`).
        self.playback_unfold_repeats = false;
        if !self.eat_punct('{') {
            self.warn("\\score sans accolade");
            return (score, has_layout, has_midi, false);
        }
        while !self.at_end() {
            if self.eat_punct('}') {
                break;
            }
            if self.peek().is_some_and(|token| token.is_command("midi")) {
                has_midi = true;
            }
            if self.peek().is_some_and(|token| token.is_command("layout")) {
                has_layout = true;
            }
            let before = self.position;
            score.groups.extend(self.parse_score_element());
            self.guard_progress(before);
        }
        // `\midi { \transposition … }` ne contient pas de musique, mais pose
        // la transposition de jeu de toute la partition. L'envelopper autour
        // des voix permet à l'interprète de décaler les hauteurs concert comme
        // le fait le performer MIDI de LilyPond.
        if let Some(pitch) = self.midi_transposition {
            for group in &mut score.groups {
                for staff in &mut group.staves {
                    for voice in &mut staff.voices {
                        let music = std::mem::replace(&mut voice.music, Music::Empty);
                        voice.music = Music::Seq(vec![
                            Music::Context(Context::MidiInstrumentTranspose(pitch)),
                            music,
                        ]);
                    }
                }
            }
        }
        (score, has_layout, has_midi, self.playback_unfold_repeats)
    }

    /// Un élément du corps de `\score`, également une expression musicale de
    /// haut niveau : `<< … >>`, `\new …`, `\layout`/`\midi`/`\paper`,
    /// `\header` ou musique sans contexte (portée anonyme).
    fn parse_score_element(&mut self) -> Vec<GroupBlock> {
        let mut elements = Vec::new();
        if self.at_double_angle_open() {
            self.position += 2;
            elements.extend(self.parse_context_list());
        } else if self.peek().is_some_and(|token| token.is_command("new")) {
            self.position += 1;
            if let Some(group) = self.parse_new() {
                elements.push(group);
            }
        } else if self.peek().is_some_and(|token| {
            token.is_command("layout") || token.is_command("midi") || token.is_command("paper")
        }) {
            let midi = self.peek().is_some_and(|token| token.is_command("midi"));
            let layout = self.peek().is_some_and(|token| token.is_command("layout"));
            self.position += 1;
            let start = self.position;
            self.skip_braced();
            if midi {
                self.midi_transposition = self.find_midi_transposition(start);
            }
            if layout && self.timing_translator_deplacé(start) {
                self.staff_local_timing = true;
            }
        } else if self.peek().is_some_and(|token| token.is_command("header")) {
            self.position += 1;
            self.parse_header();
        } else {
            // Musique sans contexte explicite : une portée anonyme, sauf si
            // une fonction musicale enveloppe des contextes (`\transpose`,
            // `\relative`, `\fixed`…), qui donnent alors la structure.
            let music = self.parse_music();
            elements.extend(self.score_groups(music));
        }
        elements
    }

    /// Groupes de portées portés par une musique de `\score`.
    ///
    /// LilyPond applique `\transpose`, `\relative` et `\fixed` à une musique
    /// quelconque, contextes compris : la structure de portées est donc celle
    /// du contenu, et chaque voix reçoit les enveloppes qui la précèdent.
    fn score_groups(&mut self, music: Music) -> Vec<GroupBlock> {
        self.score_groups_in(music, &mut Vec::new())
    }

    /// [`Self::score_groups`], avec la pile des enveloppes rencontrées.
    fn score_groups_in(&mut self, music: Music, wrappers: &mut Vec<Wrapper>) -> Vec<GroupBlock> {
        match music {
            Music::Empty => Vec::new(),
            // Un `\tag` peut envelopper un contexte (`\tag #'x \new Staff …`) :
            // le filtre a déjà été appliqué à l'analyse, l'étiquette n'a plus
            // de rôle ici et la structure est celle de la musique intérieure.
            Music::Tag { music, .. } => self.score_groups_in(*music, wrappers),
            Music::Filtered {
                filter,
                wanted,
                groups: tag_groups,
                music,
            } => {
                // Le filtre doit survivre à l'extraction de la structure :
                // sans lui l'interpréteur perdrait les étiquettes à filtrer.
                let mut result = self.score_groups_in(*music, wrappers);
                for group in &mut result {
                    for staff in &mut group.staves {
                        for voice in &mut staff.voices {
                            voice.music = Music::Filtered {
                                filter,
                                wanted: wanted.clone(),
                                groups: tag_groups.clone(),
                                music: Box::new(std::mem::replace(&mut voice.music, Music::Empty)),
                            };
                        }
                    }
                }
                result
            }
            Music::Context(context) => match Wrapper::of(&context) {
                Some((wrapper, inner)) => {
                    let inner = inner.clone();
                    wrappers.push(wrapper);
                    let groups = self.score_groups_in(inner, wrappers);
                    wrappers.pop();
                    groups
                }
                None => vec![anonymous_group(wrap_all(wrappers, Music::Context(context)))],
            },
            Music::New(mut group) => {
                wrap_staves(wrappers, &mut group.staves);
                vec![*group]
            }
            Music::Lyrics(block) => vec![GroupBlock {
                kind: GroupKind::Other,
                existing: false,
                instrument_name: None,
                short_instrument_name: None,
                staves: Vec::new(),
                lyrics: vec![*block],
            }],
            Music::Sim(items) => {
                // Au niveau d'un `\score`, un `<< … >>` sans contexte ouvre
                // un contexte par élément : c'est l'idiome des partitions
                // chorales Mutopia (`<< \soprano \alto … >>` dans le score
                // MIDI), que LilyPond joue sur une piste par élément
                // (`lily/parser.yy` `context_prefix`, `lily/context.cc`).
                // Dans une portée, les mêmes éléments sont des voix : cette
                // distinction est faite par `collect_staves`.
                if !items.iter().any(has_context) {
                    if items.len() <= 1 {
                        return vec![anonymous_group(wrap_all(wrappers, Music::Sim(items)))];
                    }
                    return items
                        .into_iter()
                        .map(|item| anonymous_group(wrap_all(wrappers, item)))
                        .collect();
                }
                self.context_groups(items, wrappers, Music::Sim)
            }
            Music::Seq(items) => {
                // Un `{ … }` peut lui aussi contenir des contextes : LilyPond
                // les crée à la position courante, comme dans `<< … >>`. Sans
                // contexte, la séquence reste une musique ordinaire, donc une
                // portée anonyme.
                if !items.iter().any(has_context) {
                    return vec![anonymous_group(wrap_all(wrappers, Music::Seq(items)))];
                }
                self.warn_sequential_contexts(&items);
                self.context_groups(items, wrappers, Music::Seq)
            }
            other => vec![anonymous_group(wrap_all(wrappers, other))],
        }
    }

    /// Répartit les éléments d'une musique qui contient des contextes entre
    /// les groupes qu'ils portent et la musique sans contexte voisine, réunie
    /// en une seule portée anonyme à la place du premier élément concerné.
    ///
    /// `combine` reconstruit la musique des éléments restants : simultanée
    /// pour `<< … >>`, séquentielle pour `{ … }`.
    fn context_groups(
        &mut self,
        items: Vec<Music>,
        wrappers: &mut Vec<Wrapper>,
        combine: fn(Vec<Music>) -> Music,
    ) -> Vec<GroupBlock> {
        let rest: Vec<Music> = items
            .iter()
            .filter(|item| !has_context(item))
            .cloned()
            .collect();
        let mut rest = if rest.is_empty() { None } else { Some(rest) };
        let mut groups = Vec::new();
        for item in items {
            if has_context(&item) {
                groups.extend(self.score_groups_in(item, wrappers));
            } else if let Some(rest) = rest.take() {
                groups.push(anonymous_group(wrap_all(wrappers, combine(rest))));
            }
        }
        groups
    }

    /// Avertit quand une séquence `{ … }` porte plusieurs contextes : leurs
    /// positions respectives se perdent, ils deviennent des portées
    /// parallèles plutôt que rien.
    fn warn_sequential_contexts(&mut self, items: &[Music]) {
        if items.iter().filter(|item| has_context(item)).count() > 1 {
            self.warn("plusieurs contextes dans une séquence `{ … }` : portées parallèles");
        }
    }

    /// Liste de contextes d'un `<< … >>` de `\score`.
    ///
    /// La liste entière est une musique simultanée : c'est [`Self::score_groups`]
    /// qui décide quels éléments sont des contextes et réunit les autres en
    /// une seule portée anonyme.
    fn parse_context_list(&mut self) -> Vec<GroupBlock> {
        let mut items = Vec::new();
        while !self.at_end() && !self.at_double_angle_close() {
            // `\\` sépare des contextes simultanés : il ne produit rien.
            if self.peek().is_some_and(|token| token.is_command("\\")) {
                self.position += 1;
                continue;
            }
            let before = self.position;
            let music = self.parse_music();
            if !music.is_empty() {
                items.push(music);
            }
            self.guard_progress(before);
        }
        if self.at_double_angle_close() {
            self.position += 2;
        }
        self.score_groups(Music::Sim(items))
    }

    /// `\new` (`existing == false`) ou `\context` (`existing == true`).
    ///
    /// Un `\context` dont le nom est une portée, un groupe de portées ou une
    /// voix ouvre un contexte structurel comme `\new` : c'est l'idiome des
    /// partitions d'orchestre
    /// `\context StaffGroup << \context Staff = "haut" … >>`. Le traiter comme
    /// un simple changement de contexte réduirait ces portées à autant de voix
    /// d'une seule portée (au plus quatre dans le modèle), perdant la musique
    /// des portées suivantes. Les autres noms (`\context { \Staff … }` d'un
    /// `\midi`, `\context Score`, `\context Lyrics`…) restent traversés.
    fn parse_context_command(&mut self, existing: bool) -> Music {
        if existing {
            // Un `\context Staff` dont le contenu porte des voix nommées
            // retrouve la portée englobante (`context_find_or_create`) :
            // c'est l'idiome du piano `\context Voice = "one" \rightHand`
            // où `rightHand` enchâsse `\context Staff << \new Voice … >>`.
            // Ouvrir une portée fille porterait ces voix au-delà de la limite
            // de quatre du modèle et écraserait la musique (lamortdase).
            if self.peek().is_some_and(|token| {
                token.kind == Kind::Word && GroupKind::from_name(&token.text) == GroupKind::Staff
            }) {
                self.bump();
                let context_name = if self.eat_punct('=') {
                    match self.bump() {
                        Some(Token {
                            kind: Kind::Str,
                            text,
                            ..
                        }) => Some(text),
                        _ => None,
                    }
                } else {
                    None
                };
                let mut instrument_name = None;
                let mut short_instrument_name = None;
                if self.eat_command("with") {
                    for (property, value) in self.parse_with_block() {
                        match property.as_str() {
                            "instrumentName" => instrument_name = Some(value),
                            "shortInstrumentName" => short_instrument_name = Some(value),
                            _ => {}
                        }
                    }
                }
                let content = self.parse_music();
                // Règle LilyPond : tout le contenu d'un `\context Staff`
                // appartient à **cette** portée. Les voix ouvertes par
                // `\new Voice`, `\context Voice` ou `\\` sont des voix
                // *dans* la portée, créées au moment où la musique les
                // rencontre (`lily/context.cc` `find_create_context`,
                // `lily/parser.yy` `context_change`, `ly/engraver-init.ly`
                // `\accepts Voice`). On garde donc une seule musique à
                // interpréter : l'interpréteur ouvre les voix sœurs aux bons
                // ticks et libère leur piste quand elles finissent. Les
                // n-ups de plus de quatre voix simultanées sont répartis sur
                // des portées sœurs par le constructeur.
                let mut staff = StaffBlock::new();
                staff.name = context_name;
                staff.instrument_name = instrument_name.clone();
                staff.short_instrument_name = short_instrument_name.clone();
                let addlyrics_target = last_voice_name(&content);
                collect_lyrics_in_music(&content, &mut staff.lyrics);
                staff.voices.push(VoiceBlock {
                    name: None,
                    music: content,
                });
                self.collect_addlyrics_target(&mut staff, addlyrics_target);
                return Music::New(Box::new(GroupBlock {
                    kind: GroupKind::Staff,
                    existing,
                    instrument_name,
                    short_instrument_name,
                    staves: vec![staff],
                    lyrics: Vec::new(),
                }));
            }
            let structural = self.peek().is_some_and(|token| {
                token.kind == Kind::Word && GroupKind::from_name(&token.text) != GroupKind::Other
            });
            if !structural {
                let nom = self.bump().map(|token| token.text).unwrap_or_default();
                if self.eat_punct('=') {
                    self.bump();
                }
                if self.eat_command("with") {
                    self.parse_with_block();
                }
                // `\context ChordNames` arrive ici. LilyPond joue sa grille
                // dans le MIDI (vérifié en 2.26) : on laisse la musique nue
                // devenir une portée anonyme, comme pour `\new ChordNames`.
                // Seul FiguredBass reste une grille muette.
                if nom == "FiguredBass" {
                    return self.skip_nonplaying_grid(&nom);
                }
                return self.parse_music();
            }
        }
        match self.parse_new_context(existing) {
            Some(group) if group.staves.is_empty() && !group.lyrics.is_empty() => {
                // Un `\new Lyrics` peut porter plusieurs couplets (un couplet
                // aligné imbriqué) : les conserver tous, pas seulement le
                // premier, sinon la portée suivante perd ses paroles.
                let blocks: Vec<Music> = group
                    .lyrics
                    .into_iter()
                    .map(|lyrics| Music::Lyrics(Box::new(lyrics)))
                    .collect();
                if blocks.len() == 1 {
                    blocks.into_iter().next().unwrap()
                } else {
                    Music::Sim(blocks)
                }
            }
            Some(group) => Music::New(Box::new(group)),
            None => Music::Empty,
        }
    }

    /// `\new Nom [= "id"] [\with { … }] musique`.
    fn parse_new(&mut self) -> Option<GroupBlock> {
        self.parse_new_context(false)
    }

    /// Contexte structurel d'un `\new` ou d'un `\context` : le nom non
    /// structurel et le `\context Staff` à contenu musical sont traversés par
    /// [`Self::parse_context_command`] avant d'arriver ici.
    fn parse_new_context(&mut self, existing: bool) -> Option<GroupBlock> {
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
        let mut midi_silent = false;
        if self.eat_command("with") {
            let debut_with = self.position;
            let properties = self.parse_with_block();
            // `ly/performer-init.ly:32` : le `Staff_performer` rend la portée
            // audible dans le MIDI. Un `\with { \remove "Staff_performer" }`
            // (pianoReduction des partitions MuseScore) retire donc cette
            // portée du MIDI de référence. La portée et ses notes restent dans
            // le modèle (pivot de tous les formats) ; on relève seulement le
            // drapeau `Staff::muted` que l'écrivain MIDI utilisera pour ne pas
            // les jouer.
            midi_silent = self.tokens[debut_with..self.position]
                .iter()
                .enumerate()
                .any(|(index, token)| {
                    token.is_command("remove")
                        && self.tokens.get(debut_with + index + 1).is_some_and(|next| {
                            next.kind == Kind::Str && next.text.contains("Staff_performer")
                        })
                });
            for (property, value) in properties {
                match property.as_str() {
                    "instrumentName" => instrument_name = Some(value),
                    "shortInstrumentName" => short_instrument_name = Some(value),
                    "stanza" => verse = stanza_verse(&value),
                    _ => {}
                }
            }
        }

        // `\new FiguredBass` porte une grille de chiffres que LilyPond ne joue
        // pas dans le MIDI. `\new ChordNames`, lui, produit bien une piste
        // jouée (vérifié en compilant une grille avec LilyPond 2.26) : il suit
        // le chemin ordinaire des portées, ci-dessous, comme `\new DrumStaff`.
        if kind == GroupKind::Other && name == "FiguredBass" {
            self.skip_nonplaying_grid(&name);
            return None;
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
            let (syllables, block_verse, couplets) = self.parse_lyric_music();
            let block = LyricsBlock {
                target,
                syllables,
                verse: block_verse.or(verse).unwrap_or(0),
            };
            let mut lyrics = vec![block];
            lyrics.extend(couplets);
            return Some(GroupBlock {
                kind: GroupKind::Other,
                existing,
                instrument_name: None,
                short_instrument_name: None,
                staves: Vec::new(),
                lyrics,
            });
        }

        // Contenu du contexte.
        let mut staff = StaffBlock::new();
        staff.muted = midi_silent;
        staff.name = context_name.clone();
        staff.instrument_name = instrument_name.clone();
        staff.short_instrument_name = short_instrument_name.clone();

        match kind {
            GroupKind::PianoStaff | GroupKind::ChoirStaff | GroupKind::StaffGroup => {
                let mut group = GroupBlock {
                    kind,
                    existing,
                    instrument_name,
                    short_instrument_name,
                    staves: Vec::new(),
                    lyrics: Vec::new(),
                };
                let content = self.parse_music();
                let vide = matches!(&content, Music::Sim(items) | Music::Seq(items) if items.is_empty())
                    || matches!(&content, Music::Empty);
                if vide {
                    return None;
                }
                self.collect_staves(content, &mut group.staves, &mut group.lyrics);
                if group.staves.is_empty() && group.lyrics.is_empty() {
                    // Un groupe vide (`\new PianoStaff << >>`) n'engrave
                    // aucune portée chez LilyPond : la déclarer créerait une
                    // portée fantôme dans la partie MusicXML.
                    return None;
                }
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
                    // La nature est conservée : l'interpréteur doit distinguer
                    // un `\new Voice` rencontré dans une voix d'une portée
                    // imbriquée (`Music::New`).
                    kind: GroupKind::Voice,
                    existing,
                    instrument_name,
                    short_instrument_name,
                    staves: vec![staff],
                    lyrics: Vec::new(),
                })
            }
            _ => {
                // `\new DrumStaff` : le contenu est en mode percussion même
                // sans `\drummode` explicite (`ly/drumpitch-init.ly`).
                let mode_precedent = self.note_mode;
                if name.eq_ignore_ascii_case("drumstaff") {
                    self.note_mode = NoteMode::Drums;
                }
                let content = self.parse_music();
                self.note_mode = mode_precedent;
                // Une seule musique de portée, comme pour `\context Staff` :
                // les voix sont ouvertes par l'interpréteur au tick courant,
                // pas relues chacune depuis zéro.
                let addlyrics_target = last_voice_name(&content);
                collect_lyrics_in_music(&content, &mut staff.lyrics);
                staff.voices.push(VoiceBlock {
                    name: None,
                    music: content,
                });
                self.collect_addlyrics_target(&mut staff, addlyrics_target);
                Some(GroupBlock {
                    kind: GroupKind::Staff,
                    existing,
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
        self.collect_staves_in(music, &mut Vec::new(), staves, lyrics);
    }

    /// [`Self::collect_staves`], avec la pile des enveloppes rencontrées :
    /// `\new PianoStaff \transpose c d << … >>` garde ses portées, chacune
    /// transposée.
    fn collect_staves_in(
        &mut self,
        music: Music,
        wrappers: &mut Vec<Wrapper>,
        staves: &mut Vec<StaffBlock>,
        lyrics: &mut Vec<LyricsBlock>,
    ) {
        match music {
            Music::Context(context) => match Wrapper::of(&context) {
                Some((wrapper, inner)) => {
                    let inner = inner.clone();
                    wrappers.push(wrapper);
                    self.collect_staves_in(inner, wrappers, staves, lyrics);
                    wrappers.pop();
                }
                None => self.push_anonymous_staff(Music::Context(context), wrappers, staves),
            },
            Music::Sim(items) => {
                // Même règle que pour les groupes de `\score` : un `<< … >>`
                // sans contexte ne fait qu'une portée, avec ses éléments
                // simultanés (voix) ; un `\new` y définit la structure.
                if !items.iter().any(has_context) {
                    self.push_anonymous_staff(Music::Sim(items), wrappers, staves);
                    return;
                }
                self.context_staves(items, wrappers, staves, lyrics, Music::Sim);
            }
            Music::Seq(items) => {
                // Un `{ … }` contenant un contexte décrit lui aussi la
                // structure : le contexte est créé à la position courante.
                if !items.iter().any(has_context) {
                    self.push_anonymous_staff(Music::Seq(items), wrappers, staves);
                    return;
                }
                self.warn_sequential_contexts(&items);
                self.context_staves(items, wrappers, staves, lyrics, Music::Seq);
            }
            Music::New(mut group) => {
                wrap_staves(wrappers, &mut group.staves);
                // `lily/parser.yy` `context_prefix` (l. 1686-1690) : `\new` et
                // `\context Voice` créent ou retrouvent un contexte **Voice
                // dans le Staff courant** (`context_create` /
                // `context_find_or_create`, `lily/context.cc`). Ces voix
                // appartiennent donc à la portée en cours, pas à des portées
                // sœurs : c'est le constructeur qui répartit leurs emplacements
                // et ouvre des portées supplémentaires seulement si plus de
                // quatre voix sont simultanées.
                if group.kind == GroupKind::Voice && group.staves.len() == 1 {
                    let mut voice_staff = group.staves.remove(0);
                    if let Some(current) = staves.iter_mut().find(|staff| staff.name.is_none()) {
                        current.voices.append(&mut voice_staff.voices);
                        current.lyrics.append(&mut voice_staff.lyrics);
                    } else {
                        staves.push(voice_staff);
                    }
                    lyrics.append(&mut group.lyrics);
                } else {
                    staves.extend(group.staves);
                }
            }
            Music::Lyrics(block) => lyrics.push(*block),
            other => {
                // Musique nue : une portée anonyme ; ses paroles restent au
                // niveau du groupe, comme avant.
                let mut staff = StaffBlock::new();
                let mut voices = Vec::new();
                let music = wrap_all(wrappers, other);
                self.collect_voices(music, &mut voices, lyrics);
                staff.voices = voices;
                staves.push(staff);
            }
        }
    }

    /// Pousse une portée anonyme portant `music`, avec les enveloppes
    /// rencontrées ; ses paroles restent sur la portée.
    fn push_anonymous_staff(
        &mut self,
        music: Music,
        wrappers: &[Wrapper],
        staves: &mut Vec<StaffBlock>,
    ) {
        let mut staff = StaffBlock::new();
        let mut voices = Vec::new();
        let mut staff_lyrics = Vec::new();
        self.collect_voices(wrap_all(wrappers, music), &mut voices, &mut staff_lyrics);
        staff.voices = voices;
        staff.lyrics = staff_lyrics;
        staves.push(staff);
    }

    /// Répartit les éléments d'une musique qui contient des contextes entre
    /// les portées qu'ils portent et la musique sans contexte voisine,
    /// réunie en une seule portée anonyme.
    ///
    /// `combine` reconstruit la musique des éléments restants : simultanée
    /// pour `<< … >>`, séquentielle pour `{ … }`.
    fn context_staves(
        &mut self,
        items: Vec<Music>,
        wrappers: &mut Vec<Wrapper>,
        staves: &mut Vec<StaffBlock>,
        lyrics: &mut Vec<LyricsBlock>,
        combine: fn(Vec<Music>) -> Music,
    ) {
        let rest: Vec<Music> = items
            .iter()
            .filter(|item| !has_context(item))
            .cloned()
            .collect();
        let mut rest = if rest.is_empty() { None } else { Some(rest) };
        for item in items {
            if has_context(&item) {
                self.collect_staves_in(item, wrappers, staves, lyrics);
            } else if let Some(rest) = rest.take() {
                self.push_anonymous_staff(combine(rest), wrappers, staves);
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
        self.collect_voices_in(music, &mut Vec::new(), voices, lyrics);
    }

    /// [`Self::collect_voices`], avec la pile des enveloppes rencontrées :
    /// `\new Staff \transpose c d << \new Voice … >>` garde ses voix,
    /// chacune transposée.
    fn collect_voices_in(
        &mut self,
        music: Music,
        wrappers: &mut Vec<Wrapper>,
        voices: &mut Vec<VoiceBlock>,
        lyrics: &mut Vec<LyricsBlock>,
    ) {
        match music {
            Music::Context(context) => match Wrapper::of(&context) {
                Some((wrapper, inner)) => {
                    let inner = inner.clone();
                    wrappers.push(wrapper);
                    self.collect_voices_in(inner, wrappers, voices, lyrics);
                    wrappers.pop();
                }
                None => voices.push(VoiceBlock {
                    name: None,
                    music: wrap_all(wrappers, Music::Context(context)),
                }),
            },
            Music::Sim(items) => {
                for item in items {
                    match item {
                        Music::New(mut group) => {
                            wrap_staves(wrappers, &mut group.staves);
                            for staff in group.staves {
                                voices.extend(staff.voices);
                                lyrics.extend(staff.lyrics);
                            }
                            lyrics.extend(group.lyrics);
                        }
                        Music::Lyrics(block) => lyrics.push(*block),
                        other => voices.push(VoiceBlock {
                            name: None,
                            music: wrap_all(wrappers, other),
                        }),
                    }
                }
            }
            Music::New(mut group) => {
                wrap_staves(wrappers, &mut group.staves);
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
                music: wrap_all(wrappers, other),
            }),
        }
    }

    /// Attache un `\addlyrics` qui suit une voix.
    fn collect_addlyrics(&mut self, staff: &mut StaffBlock) {
        let target = staff.voices.last().and_then(|voice| voice.name.clone());
        self.collect_addlyrics_target(staff, target);
    }

    /// Attache un `\addlyrics` dont la cible est déjà connue : le contenu
    /// d'une portée garde ses voix en une seule musique, donc le nom de la
    /// dernière voix déclarée est cherché dans cette musique.
    fn collect_addlyrics_target(&mut self, staff: &mut StaffBlock, target: Option<String>) {
        while self.eat_command("addlyrics") {
            let (syllables, block_verse, couplets) = self.parse_lyric_music();
            let target = target.clone().unwrap_or_default();
            let verse = block_verse.unwrap_or(staff.lyrics.len());
            staff.lyrics.push(LyricsBlock {
                target,
                syllables,
                verse,
            });
            staff.lyrics.extend(couplets);
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
            Some(token) if token.is_scheme() => {
                self.position += 1;
                scheme_text_value(&token.text)
            }
            Some(token) if token.is_punct('{') => self.parse_markup_text(),
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
    ///
    /// Renvoie les syllabes, le rang de couplet porté par `\stanza "n."`
    /// (LilyPond 2.27) et les couplets alignés déclarés par un `\new Lyrics`
    /// imbriqué. Un `<< … >>` dans des paroles aligne des couplets : il ne
    /// referme donc pas le bloc courant, sinon la musique qui suit serait lue
    /// comme une portée supplémentaire.
    fn parse_lyric_music(&mut self) -> (Vec<Syllable>, Option<usize>, Vec<LyricsBlock>) {
        if !self.eat_punct('{') {
            // `\v` peut désigner un `\lyricmode { … }` mémorisé dans une
            // variable : `\new Lyrics \lyricsto "s" \v` doit relire ses
            // syllabes (comme le fait LilyPond pour un identifiant musical).
            if let Some(Token {
                kind: Kind::Command,
                text,
                ..
            }) = self.peek().cloned()
            {
                if let Some(Music::Lyrics(block)) = self.variables.get(&text).cloned() {
                    self.position += 1;
                    return (block.syllables, Some(block.verse), Vec::new());
                }
            }
            // Un unique mot ou une variable : rien à aligner.
            self.skip_music_token();
            return (Vec::new(), None, Vec::new());
        }
        let mut syllables = Vec::new();
        let mut verse = None;
        let mut couplets = Vec::new();
        // Profondeur des `<< … >>` traversés : un `}` à l'intérieur d'un
        // alignement de couplets n'appartient pas au bloc de paroles courant.
        let mut alignement = 0usize;
        // Position en début d'itération, absente à la première : le bloc vient
        // d'ouvrir sa propre accolade et rien n'a encore été lu dedans.
        let mut before = None;
        while !self.at_end() {
            if let Some(before) = before {
                self.guard_progress(before);
            }
            before = Some(self.position);
            if self.at_double_angle_open() {
                self.position += 2;
                alignement += 1;
                continue;
            }
            if self.at_double_angle_close() {
                self.position += 2;
                alignement = alignement.saturating_sub(1);
                continue;
            }
            if self.peek().is_some_and(|token| token.is_punct('}')) {
                self.position += 1;
                if alignement == 0 {
                    break;
                }
                continue;
            }
            if self.eat_punct(';') {
                continue;
            }
            match self.peek().cloned() {
                Some(token) if token.is_command("skip") => {
                    self.position += 1;
                    // La durée de `\skip` est consommée, mais elle ne pilote
                    // pas l'alignement : `\skip` est un simple espace.
                    if self.peek().is_some_and(|token| token.kind == Kind::Number) {
                        self.position += 1;
                    }
                    syllables.push(Syllable {
                        text: String::new(),
                        hyphen: false,
                        extender: false,
                        skip: true,
                        ticks: None,
                    });
                }
                Some(token) if token.is_command("set") => {
                    self.position += 1;
                    let mut property = String::new();
                    while !self.at_end() && !self.peek().is_some_and(|token| token.is_punct('=')) {
                        if let Some(token) = self.bump() {
                            property.push_str(&token.text);
                        }
                    }
                    self.bump();
                    let value = self.parse_property_value();
                    if property == "stanza" {
                        verse = stanza_verse(&value);
                    }
                }
                Some(token) if token.is_command("stanza") => {
                    self.position += 1;
                    let text = match self.bump() {
                        Some(Token {
                            kind: Kind::Str | Kind::Word,
                            text,
                            ..
                        }) => text,
                        _ => String::new(),
                    };
                    verse = stanza_verse(&text);
                }
                Some(token) if token.is_command("markup") => {
                    self.position += 1;
                    let text = self.parse_markup_text();
                    let ticks = self.parse_lyric_duration();
                    syllables.push(Syllable {
                        text,
                        hyphen: false,
                        extender: false,
                        skip: false,
                        ticks,
                    });
                }
                Some(token) if token.is_command("new") => {
                    // `\new Lyrics … { … }` dans un `<< … >>` de paroles :
                    // un couplet supplémentaire, aligné sur les mêmes notes.
                    // Il faut consommer tout le contexte (nom, `\with`, cible
                    // et corps) pour ne pas refermer le couplet courant sur le
                    // `}` de `\with` et laisser la suite devenir une portée.
                    self.position += 1;
                    self.bump(); // type de contexte (`Lyrics`)
                    if self.eat_punct('=') {
                        self.bump();
                    }
                    if self.eat_command("with") {
                        self.skip_braced();
                    }
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
                    let (new_syllables, new_verse, suivants) = self.parse_lyric_music();
                    couplets.push(LyricsBlock {
                        target,
                        syllables: new_syllables,
                        verse: new_verse.unwrap_or(verse.unwrap_or(0) + 1),
                    });
                    couplets.extend(suivants);
                }
                Some(Token {
                    kind: Kind::Word, ..
                }) => {
                    // Mot de paroles entier, ponctuation collée comprise :
                    // `Ah_!` et `qu'il` sont une syllabe chacun.
                    self.push_lyric_word(&mut syllables);
                }
                Some(Token {
                    kind: Kind::Punct(_),
                    ..
                }) if lexer::mot_de_paroles(&self.tokens, self.position).is_some() => {
                    // Mot qui commence par un signe typographique
                    // (`«Ah»`, `’tis`) : guillemets et apostrophe restent
                    // dans le texte de la syllabe.
                    self.push_lyric_word(&mut syllables);
                }
                Some(Token {
                    kind: Kind::Str,
                    text,
                    ..
                }) => {
                    self.position += 1;
                    let hyphen = self.eat_lyric_hyphen();
                    let extender = self.eat_lyric_extender();
                    let ticks = self.parse_lyric_duration();
                    syllables.push(Syllable {
                        text,
                        hyphen,
                        extender,
                        skip: false,
                        ticks,
                    });
                }
                Some(Token {
                    kind: Kind::Command,
                    text,
                    ..
                }) => {
                    // Un identifiant musical (`\vA`) peut contenir un
                    // `\lyricmode` : ses syllabes s'insèrent ici, comme
                    // LilyPond le fait pour une variable de paroles.
                    self.position += 1;
                    if let Some(Music::Lyrics(block)) = self.variables.get(&text).cloned() {
                        syllables.extend(block.syllables);
                        if verse.is_none() {
                            verse = Some(block.verse);
                        }
                    }
                }
                Some(token) if token.is_punct('~') => {
                    // Élision (`l'~ami`) : gardée dans le texte de la
                    // syllabe précédente, que l'export MusicXML écrit tel quel.
                    self.position += 1;
                    if let Some(previous) = syllables.last_mut() {
                        previous.text.push('~');
                    }
                }
                Some(token) if token.is_punct('_') => {
                    // `_` prolonge la syllabe précédente (comme `__`) : le
                    // trait d'extension est porté par cette syllabe et la note
                    // suivante n'en reçoit pas de nouvelle.
                    self.position += 1;
                    if let Some(previous) = syllables.last_mut() {
                        previous.extender = true;
                    }
                }
                Some(_) => {
                    self.position += 1;
                }
                None => break,
            }
        }
        (syllables, verse, couplets)
    }

    /// Durée explicite d'une syllabe de paroles (`la4`, `la2.`), en ticks.
    /// Sans durée, `None` : l'alignement se fait alors une syllabe par note.
    fn parse_lyric_duration(&mut self) -> Option<Fraction> {
        // `parse_duration` change la durée par défaut du flux musical : les
        // paroles ne doivent pas la modifier pour la musique qui suit.
        let previous = self.default_duration;
        let previous_multiplier = self.default_multiplier;
        let parsed = self.parse_duration();
        self.default_duration = previous;
        self.default_multiplier = previous_multiplier;
        let (duration, multiplier) = parsed?;
        let ticks = duration.ticks();
        Some(multiplier.map_or(ticks, |value| ticks * value))
    }

    /// Ajoute aux syllabes le mot de paroles qui commence à la position
    /// courante (voir [`lexer::mot_de_paroles`]), puis sa césure, son
    /// mélisme et sa durée explicite éventuels.
    fn push_lyric_word(&mut self, syllables: &mut Vec<Syllable>) {
        let Some((text, consumed)) = lexer::mot_de_paroles(&self.tokens, self.position) else {
            // Ne devrait pas arriver : l'appelant a vérifié le jeton.
            self.position += 1;
            return;
        };
        self.position += consumed;
        let hyphen = self.eat_lyric_hyphen();
        let extender = self.eat_lyric_extender();
        let ticks = self.parse_lyric_duration();
        syllables.push(Syllable {
            text,
            hyphen,
            extender,
            skip: false,
            ticks,
        });
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
                let before = self.position;
                let item = self.parse_music();
                if !item.is_empty() {
                    items.push(item);
                }
                self.guard_progress(before);
            }
            if self.at_double_angle_close() {
                self.position += 2;
            }
            return Music::Sim(items);
        }
        let Some(token) = self.peek().cloned() else {
            return Music::Empty;
        };
        // Assignation dans un flux musical (`m = { … }` ou fonction Scheme).
        // Un nom de note suivi de `=` est un contrôle d'octave (`a=''`), pas
        // une variable : l'exclure évite d'avaler la musique qui suit.
        if token.kind == Kind::Word
            && self.language.parse(&token.text).is_none()
            && self.peek_at(1).is_some_and(|next| next.is_punct('='))
        {
            let name = token.text.clone();
            self.position += 2;
            self.assign(name);
            return Music::Empty;
        }
        match &token.kind {
            Kind::Punct('{') => {
                self.position += 1;
                let mut items = Vec::new();
                while !self.at_end() && !self.eat_punct('}') {
                    let before = self.position;
                    let item = self.parse_music();
                    if !item.is_empty() {
                        items.push(item);
                    }
                    self.guard_progress(before);
                }
                Music::Seq(items)
            }
            Kind::Punct('<') => self.parse_chord_group(),
            // `|` est un contrôle de mesure (bar check, `lily/parser.yy` `'|'`
            // → `BarCheck`) : il ne crée aucune barre et ne coupe jamais la
            // trame des mesures, même en cadence. On le consomme et on
            // l'ignore ; seule une barre explicite `\bar "…"` ferme une mesure.
            Kind::Punct('}') | Kind::Punct('>') | Kind::Punct('|') | Kind::Punct(')') => {
                self.position += 1;
                Music::Empty
            }
            Kind::Command => self.parse_command(),
            Kind::Word => self.parse_chord(false),
            // Durée nue après une note ou un accord : LilyPond répète
            // l'élément précédent avec cette durée (`lily/parser.yy`).
            Kind::Number => self.parse_bare_duration(),
            // Musique embarquée `#{ … #}` : du LilyPond à relire.
            Kind::Embedded => {
                self.position += 1;
                self.parse_embedded_music(&token.text)
            }
            // Expression Scheme isolée dans la musique, lue d'un seul tenant.
            // `$nom` insère une variable définie par `#(define nom #{ … #})`
            // (Extending §1) ; les autres datum sont ignorés sans bruit.
            Kind::Scheme => {
                let datum = token.text.clone();
                self.position += 1;
                if let Some(music) = self.scheme_variables.get(&datum).cloned() {
                    music
                } else {
                    let _ = self.record_scheme_define(&datum);
                    Music::Empty
                }
            }
            Kind::Str | Kind::Punct(_) => {
                self.position += 1;
                Music::Empty
            }
        }
    }

    /// Affectation `nom = …` : fonction musicale Scheme ou variable musicale.
    fn assign(&mut self, name: String) {
        if name == "afterGraceFraction" {
            if let Some(fraction) = self.parse_scale_fraction() {
                self.after_grace_fraction = fraction;
                return;
            }
        }
        if let Some(Token {
            kind: Kind::Scheme,
            text,
            ..
        }) = self.peek().cloned()
        {
            if let Some(function) = music_function_definition(&text) {
                self.position += 1;
                self.variables.remove(&name);
                self.music_functions.insert(name, function);
                return;
            }
        }
        let music = self.parse_music();
        self.music_functions.remove(&name);
        self.variables.insert(name, music);
    }

    /// `\parallelMusic voix… { musique }` : découpe la musique aux contrôles de
    /// mesure `|` et affecte les tranches aux variables nommées, en tournant
    /// d'une voix à l'autre (`ly/music-functions-init.ly`, fonction
    /// `parallelMusic`).
    fn parse_parallel_music(&mut self) {
        let names = self.parse_parallel_voice_names();
        let block = self.take_braced_tokens();
        if names.is_empty() {
            self.warn("\\parallelMusic sans nom de voix ignoré");
            return;
        }
        let Some(tokens) = block else {
            self.warn("musique de \\parallelMusic absente ou invalide");
            return;
        };
        let Some(segments) = crate::parallel::split_at_bar_checks(&tokens) else {
            self.warn("musique parallèle sans contrôle de mesure ignorée");
            return;
        };
        // Répartition circulaire : la première tranche de chaque mesure va à la
        // première voix, la deuxième à la suivante, etc. Une voix sans tranche
        // reste définie et vide, comme le `concatenate!` de LilyPond.
        let mut voices: Vec<Vec<Token>> = vec![Vec::new(); names.len()];
        for (index, segment) in segments.into_iter().enumerate() {
            voices[index % names.len()].extend(segment);
        }
        for (name, tokens) in names.into_iter().zip(voices) {
            let music = self.parse_token_stream(tokens);
            self.music_functions.remove(&name);
            self.variables.insert(name, music);
        }
    }

    /// Noms de voix de `\parallelMusic` : liste Scheme `#'(A B C)` ou liste à
    /// virgules `A,B,C` (`\parallelMusic haut,bas` depuis LilyPond 2.24).
    fn parse_parallel_voice_names(&mut self) -> Vec<String> {
        if let Some(Token {
            kind: Kind::Scheme,
            text,
            ..
        }) = self.peek().cloned()
        {
            self.position += 1;
            return crate::parallel::voice_names(&text);
        }
        let mut names = Vec::new();
        while let Some(Token {
            kind: Kind::Word,
            text,
            ..
        }) = self.peek().cloned()
        {
            self.position += 1;
            names.push(text);
            if !self.eat_punct(',') {
                break;
            }
        }
        names
    }

    /// Lit un bloc `{ … }` et rend ses jetons intérieurs, accolades exclues.
    /// Le bloc est consommé ; `None` si le flux n'ouvre pas un bloc.
    fn take_braced_tokens(&mut self) -> Option<Vec<Token>> {
        if !self.peek().is_some_and(|token| token.is_punct('{')) {
            return None;
        }
        let start = self.position + 1;
        let mut depth = 0usize;
        while let Some(token) = self.peek().cloned() {
            if token.is_punct('{') {
                depth += 1;
            } else if token.is_punct('}') {
                depth -= 1;
                if depth == 0 {
                    let inner = self.tokens[start..self.position].to_vec();
                    self.position += 1;
                    return Some(inner);
                }
            }
            self.position += 1;
        }
        // Bloc non refermé : on garde ce qui a été lu, la fin de fichier sera
        // signalée par la suite de l'analyse.
        Some(self.tokens[start..].to_vec())
    }

    /// Relit la source d'un bloc `#{ … #}` comme une expression musicale
    /// séquentielle.
    fn parse_embedded_music(&mut self, source: &str) -> Music {
        if self.expansion_depth >= MAX_EXPANSION_DEPTH {
            self.warn("profondeur d'expansion de la musique embarquée dépassée");
            return Music::Empty;
        }
        let tokens = crate::lexer::tokenize(source);
        self.parse_token_stream(tokens)
    }

    /// Analyse une suite de jetons comme une séquence musicale, puis rend la
    /// main au flux appelant (bloc embarqué, corps de fonction).
    fn parse_token_stream(&mut self, tokens: Vec<Token>) -> Music {
        let saved_tokens = std::mem::replace(&mut self.tokens, tokens);
        let saved_position = std::mem::replace(&mut self.position, 0);
        self.expansion_depth += 1;
        let mut items = Vec::new();
        while !self.at_end() {
            let before = self.position;
            let item = self.parse_music();
            if !item.is_empty() {
                items.push(item);
            }
            // Un jeton non consommé ne doit pas faire boucler la relecture.
            self.guard_progress(before);
        }
        self.expansion_depth -= 1;
        self.tokens = saved_tokens;
        self.position = saved_position;
        Music::Seq(items)
    }

    /// Développe un appel `\nom arg…` de fonction musicale.
    fn expand_music_function(&mut self, name: &str) -> Music {
        let Some(function) = self.music_functions.get(name).cloned() else {
            return Music::Empty;
        };
        if self.expansion_depth >= MAX_EXPANSION_DEPTH {
            // Consommer les arguments sans les interpréter : sinon le bloc de
            // musique resterait dans le flux et produirait des notes
            // parasites.
            self.skip_function_arguments(&function.predicates);
            self.warn(format!(
                "profondeur d'expansion dépassée dans la fonction \\{name}"
            ));
            return Music::Empty;
        }
        // La lecture des arguments analyse leur musique pour en trouver la
        // fin, puis le corps est **relu** après substitution
        // (`function_arglist_reparse` / `REPARSE`, `lily/parser.yy` l. 1949 :
        // la première analyse est annulée par `MYREPARSE`). Cette première
        // lecture ne doit donc laisser aucun état d'analyse derrière elle :
        // sinon `\barre 3 { a b c' d' g8 }` donnait aux notes nues la durée 8
        // lue à la fin de l'argument.
        let saved_duration = self.default_duration;
        let saved_multiplier = self.default_multiplier;
        let saved_head = self.pending_head.clone();
        let saved_invisible_rest = self.pending_invisible_rest;
        let saved_tuplet_number = self.pending_tuplet_number;
        let arguments = self.read_function_arguments(&function);
        self.default_duration = saved_duration;
        self.default_multiplier = saved_multiplier;
        self.pending_head = saved_head;
        self.pending_invisible_rest = saved_invisible_rest;
        self.pending_tuplet_number = saved_tuplet_number;
        let Some(body) = function.body else {
            // Fonction Scheme pure : rien à jouer, avertissement une seule
            // fois par fonction.
            if function.kind == FunctionKind::Music && !function.warned {
                self.warn(format!(
                    "fonction musicale sans corps musical ignorée : \\{name}"
                ));
            }
            if let Some(stored) = self.music_functions.get_mut(name) {
                stored.warned = true;
            }
            return Music::Empty;
        };
        let tokens = substitute_function_tokens(&body, &function.params, &arguments);
        let relative_body = function.relative_body;
        let saved = self.in_relative_function;
        if relative_body {
            self.in_relative_function = true;
        }
        let music = self.parse_token_stream(tokens);
        self.in_relative_function = saved;
        music
    }

    /// Lit les arguments d'un appel, un par prédicat.
    fn read_function_arguments(&mut self, function: &MusicFunction) -> Vec<Vec<Token>> {
        let predicates = function.predicates.clone();
        let mut arguments = Vec::with_capacity(predicates.len());
        for predicate in &predicates {
            let start = self.position;
            self.read_function_argument(predicate);
            let end = self.position.min(self.tokens.len());
            arguments.push(self.tokens.get(start..end).unwrap_or_default().to_vec());
        }
        arguments
    }

    /// Consomme un argument selon son prédicat de fonction musicale.
    fn read_function_argument(&mut self, predicate: &str) {
        let predicate = predicate.trim();
        if predicate.ends_with("music?") || predicate.ends_with("music-list?") {
            let _ = self.parse_music();
            return;
        }
        if predicate.ends_with("pitch?") {
            let _ = self.parse_optional_pitch();
            return;
        }
        if predicate.ends_with("duration?") {
            // Un prédicat ne consomme pas la durée : `duration?` sert à
            // vérifier un argument, pas à fixer la durée par défaut.
            let duration = self.default_duration;
            let multiplier = self.default_multiplier;
            let _ = self.parse_duration();
            self.default_duration = duration;
            self.default_multiplier = multiplier;
            return;
        }
        if predicate == "markup?" || predicate == "ly:markup?" {
            if self.peek().is_some_and(|token| token.is_command("markup")) {
                self.position += 1;
                self.skip_braced();
            } else {
                self.bump();
            }
            return;
        }
        if predicate.ends_with("number?")
            || predicate.ends_with("integer?")
            || predicate == "index?"
        {
            // Signe éventuellement séparé : `-2` est deux jetons.
            if self.peek().is_some_and(|token| token.is_punct('-')) {
                self.position += 1;
            }
            self.bump();
            return;
        }
        let Some(token) = self.peek().cloned() else {
            return;
        };
        if token.is_punct('{') || self.at_double_angle_open() {
            self.skip_braced();
        } else {
            // Chaîne, symbole `#'x`, datum Scheme, mot : un seul jeton.
            self.bump();
        }
    }

    /// Consomme les arguments d'un appel sans les interpréter (garde-fou de
    /// profondeur).
    fn skip_function_arguments(&mut self, predicates: &[String]) {
        let predicates = predicates.to_vec();
        for predicate in &predicates {
            if predicate.trim().ends_with("music?") || predicate.trim().ends_with("music-list?") {
                self.skip_music_token();
            } else {
                self.read_function_argument(predicate);
            }
        }
    }

    /// Noms de tags d'un argument de `\tag`, `\keepWithTag`, `\removeWithTag`
    /// ou `\tagGroup` : `#'a`, `#'(a b)`, `"a"`, `a,b` ou `a` (Notation
    /// Reference §22.2.2).
    fn parse_tag_names(&mut self) -> Vec<String> {
        let Some(token) = self.peek().cloned() else {
            return Vec::new();
        };
        match token.kind {
            Kind::Scheme => {
                self.position += 1;
                scheme_tag_names(&token.text)
            }
            Kind::Str => {
                self.position += 1;
                vec![token.text]
            }
            Kind::Word => {
                let mut names = vec![token.text];
                self.position += 1;
                while self.eat_punct(',') {
                    match self.peek().cloned() {
                        Some(next) if next.kind == Kind::Word || next.kind == Kind::Str => {
                            names.push(next.text);
                            self.position += 1;
                        }
                        _ => break,
                    }
                }
                names
            }
            _ => Vec::new(),
        }
    }

    /// `#(define nom #{ … #})` : mémorise une variable musicale, insérable par
    /// `$nom` (Extending §1). Les autres datum Scheme sont consommés sans
    /// effet, comme le reste du sous-ensemble Scheme.
    /// `#(define afterGraceFraction (cons 15 16))`.
    fn record_after_grace_fraction(&mut self, datum: &str) -> bool {
        let text = datum.trim();
        let text = text.strip_prefix('#').unwrap_or(text);
        let Some(name) = text
            .strip_prefix("(define ")
            .and_then(|rest| rest.split_whitespace().next())
        else {
            return false;
        };
        if name != "afterGraceFraction" {
            return false;
        }
        if let Some((numerator, denominator)) = after_grace_pair(datum) {
            self.after_grace_fraction = Fraction::new(numerator, denominator);
            return true;
        }
        false
    }

    fn record_scheme_define(&mut self, datum: &str) -> bool {
        let Some((name, body)) = scheme_music_definition(datum) else {
            return false;
        };
        let music = self.parse_embedded_music(&body);
        self.scheme_variables.insert(name, music);
        true
    }

    /// Commande musicale.
    fn parse_command(&mut self) -> Music {
        let Some(Token {
            kind: Kind::Command,
            text: name,
            line,
            column,
        }) = self.peek().cloned()
        else {
            return Music::Empty;
        };
        self.position += 1;
        // Une variable musicale masque les commandes intégrées, comme dans
        // LilyPond (`identifiant = { … }` puis `\identifiant`).
        let lookup = name
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .unwrap_or(&name)
            .to_owned();
        if let Some(music) = self.variables.get(&lookup).cloned() {
            // Une valeur qui n'est pas de la musique utilisable (commande
            // inconnue) ne doit pas disparaître en silence au point d'appel ;
            // une musique vide est normale (`m = { }`).
            if matches!(music, Music::Unknown(_)) {
                self.warn(format!("variable \\{name} sans musique utilisable ignorée"));
            }
            return music;
        }
        // Une fonction musicale définie en Scheme prend le pas sur les
        // commandes intégrées du même nom.
        if self.music_functions.contains_key(&name) {
            return self.expand_music_function(&name);
        }
        let context = |context| Music::Context(context);
        match obsolete_music_command(&name).unwrap_or(name.as_str()) {
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
            // `\tag` marque la musique suivante ; `\keepWithTag` et
            // `\removeWithTag` filtrent récursivement les expressions ainsi
            // marquées (Notation Reference §22.2.2, `ly/music-functions-init.ly`).
            "tag" => {
                let tags = self.parse_tag_names();
                Music::Tag {
                    tags,
                    music: Box::new(self.parse_music()),
                }
            }
            "keepWithTag" | "removeWithTag" => {
                let wanted = self.parse_tag_names();
                let filter = if name == "keepWithTag" {
                    TagFilter::Keep
                } else {
                    TagFilter::Remove
                };
                // Ne pas filtrer ici : LilyPond applique `music-filter` au
                // résultat de la fonction musicale, donc après la conversion
                // relative de toutes les branches. On diffère le filtrage à
                // l'interprétation (Music::Filtered).
                let music = self.parse_music();
                let groups = self.tag_groups.clone();
                Music::Filtered {
                    filter,
                    wanted,
                    groups,
                    music: Box::new(music),
                }
            }
            // Déclarations de groupes de tags : elles ne produisent pas de
            // musique, mais changent le prédicat des `\keepWithTag` suivants.
            "tagGroup" => {
                let group = self.parse_tag_names();
                if !group.is_empty() {
                    self.tag_groups
                        .retain(|existing| !group.iter().any(|tag| existing.contains(tag)));
                    self.tag_groups.push(group);
                }
                Music::Empty
            }
            "resetTagGroups" => {
                self.tag_groups = default_tag_groups();
                Music::Empty
            }
            // `\tagGroupRef` et `\resetTagGroup` servent à modifier un groupe
            // existant ; consommés sans musique (le sous-ensemble ne retient
            // que les déclarations simples de `\tagGroup`).
            "tagGroupRef" => {
                let _ = self.parse_tag_names();
                Music::Empty
            }
            "resetTagGroup" | "addToTagGroup" | "removeFromTagGroup" => {
                let _ = self.parse_tag_names();
                let _ = self.parse_tag_names();
                Music::Empty
            }
            // `unfoldRepeats` (`music-functions-init.ly`) déplie les reprises
            // et écarte les alternatives ; le lecteur déplie déjà les reprises,
            // la commande est donc transparente.
            "unfoldRepeats" => {
                // LilyPond ne déplie les reprises que sous cette fonction
                // (`ly/music-functions-init.ly`) ; le MIDI d'un score sans
                // elle joue le corps et les `\alternative` une fois.
                self.playback_unfold_repeats = true;
                self.parse_music()
            }
            // `\partcombine` (ancien nom de `\partCombine`, ainsi que
            // `\partCombineUp`/`\partCombineDown`) est défini dans
            // `ly/music-functions-init.ly` : `make-directed-part-combine-music`
            // place les deux parties dans des voix simultanées (« one » et
            // « two ») de la même portée, avec `make-non-relative-music` pour
            // ne pas recalculer leurs hauteurs. L'argument optionnel
            // `chord-range` (une paire Scheme, `#'(0 . 8)` par défaut) ne
            // produit aucune musique. Les consommer comme une commande
            // inconnue sautait la première branche et lisait la seconde à la
            // suite de la voix courante, en décalant son octave relative.
            "partcombine" | "partCombine" | "partCombineUp" | "partCombineDown" => {
                if self.peek().is_some_and(|token| token.is_scheme()) {
                    self.position += 1;
                }
                // `make-non-relative-music` enveloppe chaque partie : un
                // `\relative` extérieur ne recalcule pas leurs hauteurs. Sans
                // cela, le second `\relative \partCombine …` du test de
                // régression lisait ses deux branches en chaîne et décalait
                // l'octave de la suite. Sous `\fixed`, la référence est gardée
                // (la transposition s'applique aux parties).
                let non_relative = |music: Music| {
                    if music.is_empty() {
                        music
                    } else {
                        Music::Context(Context::NonRelative {
                            music: Box::new(music),
                        })
                    }
                };
                let first = non_relative(self.parse_music());
                let second = non_relative(self.parse_music());
                if first.is_empty() {
                    second
                } else if second.is_empty() {
                    first
                } else {
                    Music::Sim(vec![first, second])
                }
            }
            // `\articulate` (`ly/articulate.ly`, `define-music-function
            // (music) (ly:music?)`) applique des articulations à la musique
            // qu'il reçoit, sans changer les hauteurs. Il est transparent pour
            // le lecteur : la musique qui suit reste dans l'enveloppe courante
            // (`\transpose … \unfoldRepeats \articulate << … >>` de
            // `musicxml2ly`). Le consommer comme une commande inconnue
            // interrompait la musique et perdait la transposition.
            "articulate" => {
                self.playback_articulate = true;
                self.parse_music()
            }
            // `transposition` (`ly/music-functions-init.ly`) est une fonction
            // musicale à un argument `ly:pitch?` qui pose la propriété
            // `instrumentTransposition` du contexte Staff. Elle n'ajoute aucune
            // musique, mais décale la hauteur concert : la hauteur écrite reste
            // celle de la source et la transposition est portée par
            // `Instrument.transpose`. Sans argument, LilyPond prend `c'`, qui
            // est aussi la valeur par défaut. La consommer comme une commande
            // inconnue laissait l'argument devenir une note parasite.
            "transposition" => {
                let pitch = self.parse_optional_pitch().unwrap_or(PitchSpec {
                    step: 0,
                    alter: 0,
                    octave: 1,
                    octave_check: None,
                    explicit_octave: true,
                    force: false,
                    cautionary: false,
                });
                Music::Context(Context::InstrumentTranspose(pitch))
            }
            "transpose" => {
                let from = self.parse_optional_pitch().unwrap_or(PitchSpec {
                    step: 0,
                    alter: 0,
                    octave: 0,
                    octave_check: None,
                    explicit_octave: false,
                    force: false,
                    cautionary: false,
                });
                let to = self.parse_optional_pitch().unwrap_or(from);
                Music::Context(Context::Transpose {
                    from,
                    to,
                    // Un `\transpose` littéral est absolu sous `\relative`
                    // (§1.2.2) ; dans une fonction `make-relative`, les notes
                    // ont déjà été lues dans le contexte relatif.
                    absolute: !self.in_relative_function,
                    music: Box::new(self.parse_music()),
                })
            }
            "tuplet" => {
                let (numerator, denominator) = self.parse_ratio().unwrap_or((3, 2));
                // `ly/music-functions-init.ly:2470` : `tuplet` prend une durée
                // optionnelle de regroupement (`\tuplet 3/2 8 { … }`), qui ne
                // devient pas la durée par défaut des notes suivantes. La
                // consommer évite qu'une durée nue soit lue comme une note.
                let default = self.default_duration;
                let default_multiplier = self.default_multiplier;
                if self.peek().is_some_and(|token| {
                    token.as_integer().is_some()
                        || token.is_command("breve")
                        || token.is_command("longa")
                        || token.is_command("maxima")
                }) {
                    let _ = self.parse_duration();
                    self.default_duration = default;
                    self.default_multiplier = default_multiplier;
                }
                let number_type = self.pending_tuplet_number.take().unwrap_or_default();
                Music::Context(Context::Tuplet {
                    numerator,
                    denominator,
                    number_type,
                    music: Box::new(self.parse_music()),
                })
            }
            "times" => {
                // `\times n/d` multiplie les durées : c'est un n-olet d/n.
                let (numerator, denominator) = self.parse_ratio().unwrap_or((2, 3));
                let number_type = self.pending_tuplet_number.take().unwrap_or_default();
                Music::Context(Context::Tuplet {
                    numerator: denominator,
                    denominator: numerator,
                    number_type,
                    music: Box::new(self.parse_music()),
                })
            }
            "scaleDurations" => {
                // `\scaleDurations n/d` multiplie toutes les durées de la
                // musique par n/d, exactement comme `c4*n/d`, sans n-olet.
                // On réutilise donc le multiplicateur `*n/d` déjà interprété
                // par `events.rs` : les imbrications se composent par produit.
                let factor = self.parse_scale_fraction();
                let mut music = self.parse_music();
                match factor {
                    Some(factor) => scale_music(&mut music, &factor),
                    None => self.warn("fraction de \\scaleDurations invalide ignorée"),
                }
                music
            }
            "grace" | "slashedGrace" => Music::Context(Context::Grace {
                kind: GraceType::Appoggiatura,
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
                // Argument optionnel `fraction` (échelle `7/8` ou paire
                // Scheme) ; sinon `afterGraceFraction` (3/4 par défaut).
                let fraction = self
                    .parse_scale_fraction()
                    .unwrap_or(self.after_grace_fraction);
                let main = Box::new(self.parse_music());
                let grace = Box::new(self.parse_music());
                Music::Context(Context::AfterGrace {
                    main,
                    grace,
                    fraction,
                })
            }
            "repeat" => self.parse_repeat(),
            "alternative" => {
                let content = self.parse_music();
                Music::Context(Context::Alternative(blocks_of(content)))
            }
            // `\parallelMusic` définit des variables et ne joue rien : il peut
            // apparaître dans une variable ou un bloc musical comme au niveau
            // supérieur.
            "parallelMusic" => {
                self.parse_parallel_music();
                Music::Empty
            }
            // `\octaveCheck` est une fonction musicale (`ly/music-functions-init.ly:1274`)
            // qui prend une hauteur et produit un `RelativeOctaveCheck` :
            // aucune note, aucune durée, et la hauteur ne doit pas devenir la
            // référence `\relative` suivante.
            "octaveCheck" => match self.parse_optional_pitch() {
                Some(pitch) => context(Context::OctaveCheck(pitch)),
                None => {
                    self.warn("\\octaveCheck sans hauteur ignoré");
                    Music::Empty
                }
            },
            // Commandes de style d'agrément ou de dynamique : elles ne
            // changent ni les hauteurs ni les durées, seulement la gravure.
            "crescTextCresc" | "dimTextDecresc" | "dimTextDecr" | "dimTextDim" | "crescHairpin"
            | "dimHairpin" | "arpeggioArrowUp" | "arpeggioArrowDown" | "arpeggioNormal" => {
                Music::Empty
            }
            "language" => {
                // `\language` vaut aussi au milieu de la musique, comme le
                // fichier de langue inclus par `\include "italiano.ly"`.
                // Le nom peut être un mot, comme au niveau du document.
                if let Some(Token {
                    kind: Kind::Str | Kind::Word,
                    text,
                    ..
                }) = self.bump()
                {
                    match Language::from_name(&text) {
                        Some(language) => self.language = language,
                        None => self.warn(format!("langue de hauteurs inconnue : {text}")),
                    }
                }
                Music::Empty
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
                // Un dénominateur nul ferait paniquer `Fraction::new` dans le
                // constructeur : on rejette l'indication ici.
                Some((_, 0)) => {
                    self.warn("indication de mesure invalide ignorée");
                    Music::Empty
                }
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
                match clefs::parse(&name) {
                    Some(clef) => context(Context::Clef(clef)),
                    None => {
                        self.warn(format!("clef inconnue : {name}"));
                        Music::Empty
                    }
                }
            }
            "partial" => {
                // `\partial` ne change pas la durée par défaut : la musique
                // qui suit `\partial 8` garde la figure écrite sur ses notes
                // (Notation Reference §2.3.3 « Upbeats »).
                let duration = self.default_duration;
                let multiplier = self.default_multiplier;
                let parsed = self.parse_duration();
                self.default_duration = duration;
                self.default_multiplier = multiplier;
                match parsed {
                    // Notation Reference §2.3.3 « Upbeats » : l'argument de
                    // `\partial` est une durée, multiplicateur `*n/d` compris
                    // (`\partial 8*5` = cinq croches). Comme pour les durées de
                    // paroles (§ parse_lyric_duration), on applique ici le
                    // multiplicateur : la figure seule ne peut pas représenter
                    // 5/8 et le multiplicateur était jusqu'ici perdu, laissant la
                    // mesure plus courte que la somme de ses accords.
                    Some((duration, multiplier)) => {
                        let ticks = duration.ticks();
                        let len = multiplier.map_or(ticks, |value| ticks * value);
                        context(Context::Partial(len))
                    }
                    None => Music::Empty,
                }
            }
            // `property-init.ly` : `cadenzaOn` pose `Timing.timing = ##f`
            // (plus de barre automatique) et `cadenzaOff` le remet à `##t`.
            "cadenzaOn" => context(Context::Cadenza(true)),
            "cadenzaOff" => context(Context::Cadenza(false)),
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
            "change" => {
                // `CHANGE STRING '=' simple_string` : le type de contexte
                // (`Staff` ou `"Staff"`) est consommé, seul le nom compte.
                self.bump();
                self.eat_punct('=');
                let name = self.bump().map_or(String::new(), |token| token.text);
                context(Context::Change { name, line, column })
            }
            "section" => context(Context::Section),
            "break" | "noBreak" | "pageBreak" | "pageTurn" => context(Context::Break),
            "tempo" => self.parse_tempo(),
            "mark" => {
                // `\mark \markup { … }` : contrairement aux autres appelants
                // de `parse_markup_text`, `\mark` reçoit encore la commande
                // `\markup`. La consommer pour que le bloc entre accolades soit
                // lu et épuisé par `parse_markup_text` : sinon le bloc reste
                // dans le flux et, traité comme de la musique, l'argument de
                // `\box { A }` devient une note parasite qui décale toutes les
                // mesures suivantes (régression Angels_Carol).
                if self.peek().is_some_and(|token| token.is_command("markup")) {
                    self.position += 1;
                }
                let text = self.parse_markup_text();
                context(Context::Mark(text))
            }
            "ottava" => {
                let mut negative = self.eat_punct('-');
                let magnitude = match self.peek().cloned() {
                    Some(token) if token.is_scheme() => {
                        // `#1`, `#-1` : signe éventuellement porté par le datum.
                        self.position += 1;
                        let text = token.text.trim();
                        match text.strip_prefix('-') {
                            Some(rest) => {
                                negative = true;
                                rest.parse().unwrap_or(0)
                            }
                            None => text.parse().unwrap_or(0),
                        }
                    }
                    _ => self
                        .bump()
                        .and_then(|token| token.as_integer())
                        .unwrap_or(0),
                };
                let shift = if negative { -magnitude } else { magnitude };
                context(Context::Ottava(shift.clamp(-2, 2) as i8))
            }
            "sustainOn" => context(Context::Sustain(true)),
            "sustainOff" => context(Context::Sustain(false)),
            "voiceOne" => context(Context::Voice(0)),
            "voiceTwo" => context(Context::Voice(1)),
            "voiceThree" => context(Context::Voice(2)),
            "voiceFour" => context(Context::Voice(3)),
            // `\stemNeutral` est un alias de `\revert Stem.direction`
            // (`ly/property-init.ly`) : la hampe redevient automatique, sans
            // consommer la musique suivante.
            "stemNeutral" => context(Context::Stem(StemDirection::Auto)),
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
                let path = self.skip_property_path();
                if path.iter().any(|part| part == "TupletNumber") {
                    self.pending_tuplet_number = None;
                }
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
            "new" | "context" => self.parse_context_command(name == "context"),
            "with" => {
                self.parse_with_block();
                self.parse_music()
            }
            // `\figures` et ses alias ouvrent une grille de chiffres non jouée
            // : la sauter, sans faire échouer le fichier.
            _ if is_nonplaying_mode_head(&name) => self.skip_nonplaying_grid(&name),
            // `\drummode`, `\chordmode` et `\chords` changent l'interprétation
            // des noms. LilyPond joue ces événements dans son MIDI (y compris
            // sous ChordNames) : on les lit comme des notes à hauteur MIDI
            // (tables de `modes.rs`). Le mode est restauré après l'expression
            // pour ne pas déteindre sur la suite.
            "drummode" | "drums" | "chordmode" | "chords" => {
                let mode = if name == "chordmode" || name == "chords" {
                    NoteMode::Chords
                } else {
                    NoteMode::Drums
                };
                let precedent = self.note_mode;
                self.note_mode = mode;
                let music = self.parse_music();
                self.note_mode = precedent;
                if matches!(music, Music::Empty) {
                    // Cas limite (fin de fichier après la commande) : comme
                    // pour une grille ignorée, signaler la limite plutôt que
                    // de faire échouer la lecture.
                    self.warnings.mark_music_ignored();
                }
                music
            }
            // `\lyrics`, `\simultaneous`, `\notemode`… ne font que changer le
            // mode de lecture des jetons suivants ; la musique enveloppée
            // reste la même. Sans ce cas, la commande était traitée comme
            // inconnue et sa musique sautée, laissant le fichier sans
            // partition.
            _ if is_mode_head(&name) => self.parse_music(),
            // `\markup` accepte une chaîne, une commande de mise en forme ou un
            // bloc entre accolades. Sans accolade, on s'arrête devant la
            // musique au lieu d'avaler le reste du fichier jusqu'à une `}` ;
            // avec un bloc, on le saute comme le fait le lexer amont.
            "markup" | "markuplist" => {
                if self.markup_argument_est_entre_accolades() {
                    self.skip_braced();
                } else {
                    self.parse_markup_text();
                }
                Music::Empty
            }
            "layout" | "midi" | "paper" | "header" => {
                let start = self.position;
                self.skip_braced();
                if name == "layout" && self.timing_translator_deplacé(start) {
                    self.staff_local_timing = true;
                }
                Music::Empty
            }
            "lyricsto" | "addlyrics" | "lyricmode" => {
                let target = if name == "lyricsto" {
                    match self.bump() {
                        Some(Token {
                            kind: Kind::Str,
                            text,
                            ..
                        }) => text,
                        _ => String::new(),
                    }
                } else {
                    String::new()
                };
                let (syllables, block_verse, couplets) = self.parse_lyric_music();
                if syllables.is_empty() && couplets.is_empty() {
                    Music::Empty
                } else {
                    let mut couplets = couplets;
                    couplets.insert(
                        0,
                        crate::ast::LyricsBlock {
                            target,
                            syllables,
                            verse: block_verse.unwrap_or(0),
                        },
                    );
                    let blocks: Vec<Music> = couplets
                        .into_iter()
                        .map(|block| Music::Lyrics(Box::new(block)))
                        .collect();
                    if blocks.len() == 1 {
                        blocks.into_iter().next().unwrap()
                    } else {
                        Music::Sim(blocks)
                    }
                }
            }
            "skip" => {
                // Notation Reference §2.2.2 : `s4` fixe la durée des notes
                // suivantes, `\skip 4` non. Le parseur ne doit donc pas
                // laisser la durée de l'argument dans la durée par défaut.
                let previous_duration = self.default_duration;
                let previous_multiplier = self.default_multiplier;
                let (duration, multiplier) = self.parse_optional_duration_spec();
                self.default_duration = previous_duration;
                self.default_multiplier = previous_multiplier;
                let (duration, multiplier) = self.resolve_chord_duration(duration, multiplier);
                Music::Chord(Chord {
                    kind: ChordKind::SkipNumeric,
                    duration,
                    multiplier,
                    from_scale: false,
                    posts: Vec::new(),
                    invisible: true,
                })
            }
            "fermata" | "shortfermata" | "longfermata" | "verylongfermata" => Music::Empty,
            _ if is_articulation_command(&name) || is_dynamic_command(&name) => {
                // Événement isolé sans accord à décorer : ignoré.
                Music::Empty
            }
            _ => {
                self.warn(format!("commande ignorée : \\{name}"));
                // Dans la musique, le bloc `{ … }` qui suit une commande
                // inconnue est de la musique ordinaire et non son argument.
                self.skip_command_arguments(&name, false);
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
                beats_per_minute = if token.is_scheme() {
                    token.scheme_decimal()
                } else {
                    token.as_decimal()
                };
            }
        }
        Music::Context(Context::Tempo {
            text,
            beats_per_minute,
        })
    }

    /// `\repeat`.
    fn parse_repeat(&mut self) -> Music {
        // Les partitions anciennes écrivent parfois le type entre guillemets :
        // `\repeat "unfold" 2 { … }` a le même sens que `\repeat unfold 2`.
        let kind = match self.bump() {
            Some(Token {
                kind: Kind::Word | Kind::Str,
                text,
                ..
            }) => RepeatKind::from_word(&text),
            _ => RepeatKind::Volta,
        };
        // `\etc` clôt une fonction musicale partielle (`REPEAT … partial_function`
        // dans `lily/parser.yy`) : les arguments manquants seront fournis plus
        // tard, quand la variable sera appliquée. La musique qui suit ne fait
        // pas partie de cette définition : ne pas la consommer comme corps.
        if self.peek().is_some_and(|token| token.is_command("etc")) {
            self.position += 1;
            return Music::Context(Context::Repeat {
                kind,
                count: 2,
                body: Box::new(Music::Empty),
                alternatives: Vec::new(),
            });
        }
        let count = match self.bump() {
            Some(Token {
                kind: Kind::Number,
                text,
                ..
            }) => text.parse().unwrap_or(2),
            _ => 2,
        };
        if self.peek().is_some_and(|token| token.is_command("etc")) {
            self.position += 1;
            return Music::Context(Context::Repeat {
                kind,
                count,
                body: Box::new(Music::Empty),
                alternatives: Vec::new(),
            });
        }
        let body = Box::new(self.parse_music());
        if kind == RepeatKind::Tremolo && !self.playback_unfold_repeats {
            // Notation Reference §4.2.2 : « The duration of the tremolo equals
            // the duration of the braced expression multiplied by the number
            // of repeats ». Le MIDI de LilyPond joue les deux notes du corps
            // tenues `n ×` leur durée écrite, sans nouvelles attaques. Étirer
            // les durées conserve les hauteurs et décale correctement la musique
            // qui suit, sans déplier le trémolo en n passages.
            //
            // Sous `\unfoldRepeats`, `scm/music-functions.scm`
            // (`unfold-repeats`, type `tremolo`) remplace au contraire le
            // trémolo par `UnfoldedRepeatedMusic` : le corps est joué n fois
            // avec ses durées écrites. On garde alors le contexte pour que
            // l'interpréteur le rejoue.
            let mut body = *body;
            let factor = Fraction::from_integer(count.max(1) as i64);
            scale_music(&mut body, &factor);
            return body;
        }
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
        // `\set Timing.timing = ##f` / `##t` a le même effet que
        // `\cadenzaOn` / `\cadenzaOff`. Le lexer lit `##f` comme le jeton
        // Scheme `#f` (et `##t` comme `#t`).
        if path.iter().any(|part| part.eq_ignore_ascii_case("timing")) {
            let cadenza =
                self.peek()
                    .filter(|token| token.is_scheme())
                    .and_then(
                        |token| match token.text.trim().to_ascii_lowercase().as_str() {
                            "#f" => Some(true),
                            "#t" => Some(false),
                            _ => None,
                        },
                    );
            if let Some(on) = cadenza {
                self.position += 1;
                return Music::Context(Context::Cadenza(on));
            }
        }
        // `melismaBusyProperties` vide : `\lyricsto` n'ignore plus les notes
        // sous liaison d'expression ni les continuations de liaison. La liste
        // par défaut est `(melismaBusy slurMelismaBusy tieMelismaBusy
        // beamMelismaBusy completionBusy)` (scm/define-context-properties.scm) ;
        // `beamMelismaBusy` ne s'active que si `autoBeaming = ##f`
        // (lily/beam-engraver.cc), que le lecteur ne modélise pas.
        if path
            .iter()
            .any(|part| part.eq_ignore_ascii_case("melismabusyproperties"))
            && self
                .peek()
                .filter(|token| token.is_scheme())
                .is_some_and(|token| is_empty_scheme_list(&token.text))
        {
            self.melisma_busy_disabled = true;
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
        if path.iter().any(|part| part == "Rest") && path.iter().any(|part| part == "transparent") {
            // `\once \override Rest.transparent = ##t` : silence invisible.
            self.eat_punct('=');
            self.skip_scheme_value();
            self.pending_invisible_rest = true;
            return Music::Empty;
        }
        if path.iter().any(|part| part == "TupletNumber") {
            self.eat_punct('=');
            if path.iter().any(|part| part == "stencil") {
                self.pending_tuplet_number = Some(TupletNumberType::None);
            } else if path.iter().any(|part| part == "text") {
                self.pending_tuplet_number = Some(TupletNumberType::Ratio);
            } else {
                self.pending_tuplet_number = Some(TupletNumberType::Number);
            }
            self.skip_scheme_value();
            return Music::Empty;
        }
        let mut direction = None;
        if self.eat_punct('=') {
            let value_start = self.position;
            match self.peek().cloned() {
                Some(token) if token.is_scheme() => {
                    self.position += 1;
                    if path.iter().any(|part| part == "direction") {
                        let atom = token.text.trim().trim_start_matches('\'').trim();
                        direction = match atom.to_ascii_uppercase().as_str() {
                            "UP" => Some(StemDirection::Up),
                            "DOWN" => Some(StemDirection::Down),
                            _ => None,
                        };
                    }
                }
                _ => {
                    self.position = value_start;
                    self.skip_scheme_value();
                }
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
        let Some(token) = self.peek().cloned() else {
            return;
        };
        if !token.is_scheme() {
            self.skip_scheme_value();
            return;
        }
        self.position += 1;
        // `#'cross` : le datum porte son quote, sans signification ici.
        let value = token.text.trim().trim_start_matches('\'').trim().to_owned();
        if path.iter().any(|part| part == "style") {
            if let Some(group) = head_group_from_style(&value) {
                self.pending_head = group;
            }
        }
    }

    //------------------------------------------------------------------
    // Accords et durées
    //------------------------------------------------------------------

    /// Recompose un nom d'accord coupé par le lexer (`c:m7`, `c/g`,
    /// `c:m7.5-`). La durée qui suit (un nombre sans `:` ni `/`) n'en fait pas
    /// partie et reste pour `parse_optional_duration_spec`.
    fn parse_chord_name(&mut self, first: &str) -> String {
        let mut nom = first.to_string();
        loop {
            let suivant = self.peek().cloned();
            match suivant {
                Some(Token {
                    kind: Kind::Punct(':'),
                    ..
                }) => {
                    self.position += 1;
                    nom.push(':');
                    if let Some(Token { kind, text, .. }) = self.peek().cloned() {
                        match kind {
                            // `c:7` : le chiffre est la qualité, la durée
                            // éventuelle qui suit n'en fait pas partie.
                            Kind::Number => {
                                nom.push_str(&text);
                                self.position += 1;
                            }
                            // `c:m7`, `c:maj7` : le lexer coupe la lettre et
                            // le chiffre ; les deux font partie de la qualité.
                            Kind::Word => {
                                nom.push_str(&text);
                                self.position += 1;
                                if let Some(Token {
                                    kind: Kind::Number,
                                    text: chiffre,
                                    ..
                                }) = self.peek().cloned()
                                {
                                    nom.push_str(&chiffre);
                                    self.position += 1;
                                }
                            }
                            _ => {}
                        }
                    }
                    while self.peek().is_some_and(|token| token.is_punct('.')) {
                        self.position += 1;
                        nom.push('.');
                        if let Some(Token {
                            kind: Kind::Number,
                            text,
                            ..
                        }) = self.peek().cloned()
                        {
                            nom.push_str(&text);
                            self.position += 1;
                        }
                        if self.peek().is_some_and(|token| token.is_punct('-')) {
                            self.position += 1;
                            nom.push('-');
                        }
                    }
                }
                Some(Token {
                    kind: Kind::Punct('/'),
                    ..
                }) => {
                    self.position += 1;
                    nom.push('/');
                    if let Some(Token {
                        kind: Kind::Word,
                        text,
                        ..
                    }) = self.peek().cloned()
                    {
                        nom.push_str(&text);
                        self.position += 1;
                    }
                }
                _ => break,
            }
        }
        nom
    }

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
                    let (duration, multiplier) = self.resolve_chord_duration(duration, multiplier);
                    let posts = self.parse_posts();
                    let invisible = std::mem::take(&mut self.pending_invisible_rest);
                    return Music::Chord(Chord {
                        kind: match text.as_str() {
                            "R" => ChordKind::MultiRest,
                            "s" => ChordKind::Skip,
                            _ => ChordKind::Rest,
                        },
                        duration,
                        multiplier,
                        from_scale: false,
                        posts,
                        invisible,
                    });
                }
                "q" => {
                    self.position += 1;
                    let (duration, multiplier) = self.parse_optional_duration_spec();
                    let (duration, multiplier) = self.resolve_chord_duration(duration, multiplier);
                    let posts = self.parse_posts();
                    return Music::Chord(Chord {
                        kind: ChordKind::Repeat,
                        duration,
                        multiplier,
                        from_scale: false,
                        posts,
                        invisible: false,
                    });
                }
                _ => {}
            }
        }

        // `\drummode` / `\chordmode` : les mots ne sont pas des hauteurs de la
        // langue courante mais des événements dont la table donne la hauteur
        // MIDI jouée par LilyPond. Un nom inconnu est consommé (progression
        // garantie) et avertit, comme pour un mot ordinaire.
        if self.note_mode != NoteMode::Notes {
            self.position += 1;
            // `a2:m` : la durée peut précéder la qualité (`chord_body` de
            // `lily/parser.yy`). La consommer avant de recomposer le nom, sinon
            // `parse_chord_name` s'arrête sur le chiffre et l'accord est lu
            // majeur.
            let (duration, multiplier) = self.parse_optional_duration_spec();
            let (duration, multiplier) = self.resolve_chord_duration(duration, multiplier);
            // Le lexer coupe `c:m7` en `c`, `:`, `m7` et `c/g` en `c`, `/`,
            // `g` : recomposer le nom complet avant de consulter la table.
            let nom = if self.note_mode == NoteMode::Chords {
                self.parse_chord_name(&text)
            } else {
                text.clone()
            };
            let eventuelles: Option<(Vec<u8>, Option<u8>)> = match self.note_mode {
                NoteMode::Drums => crate::modes::drum_pitch(&nom).map(|pitch| (vec![pitch], None)),
                NoteMode::Chords => crate::modes::chord_pitches(self.language, &nom),
                NoteMode::Notes => None,
            };
            if let Some((mut pitches, basse)) = eventuelles {
                if let Some(basse) = basse {
                    pitches.push(basse);
                }
                let head = std::mem::take(&mut self.pending_head);
                let posts = self.parse_posts_until_note_end();
                let notes: Vec<NoteSpec> = pitches
                    .into_iter()
                    .map(|pitch| NoteSpec {
                        pitch: PitchSpec::default(),
                        posts: posts.clone(),
                        head_group: head.clone(),
                        midi: Some(pitch),
                        transposable: self.note_mode == NoteMode::Chords,
                    })
                    .collect();
                let chord_posts = self.parse_posts();
                return Music::Chord(Chord {
                    kind: ChordKind::Notes(notes),
                    duration,
                    multiplier,
                    from_scale: false,
                    posts: chord_posts,
                    invisible: false,
                });
            }
            // Consommer la durée et les événements qui suivent le nom inconnu
            // pour qu'ils ne se transforment pas en note parasite. Une durée
            // consommée pour un nom ignoré ne fixe pas la durée par défaut.
            let previous_duration = self.default_duration;
            let previous_multiplier = self.default_multiplier;
            let _ = self.parse_optional_duration_spec();
            self.default_duration = previous_duration;
            self.default_multiplier = previous_multiplier;
            let _ = self.parse_posts();
            self.warn(format!("nom de mode ignoré dans la musique : {text}"));
            return Music::Empty;
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
            midi: None,
            transposable: false,
        }];
        let (duration, multiplier) = self.parse_optional_duration_spec();
        let (duration, multiplier) = self.resolve_chord_duration(duration, multiplier);
        // Les post-événements peuvent suivre la durée d'un accord.
        let posts = self.parse_posts();
        Music::Chord(Chord {
            kind: ChordKind::Notes(notes),
            duration,
            multiplier,
            from_scale: false,
            posts,
            invisible: false,
        })
    }

    /// Accord `< … >` avec durée et post-événements.
    fn parse_chord_group(&mut self) -> Music {
        self.position += 1;
        let mut notes = Vec::new();
        while !self.at_end() && !self.eat_punct('>') {
            let before = self.position;
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
                midi: None,
                transposable: false,
            });
            self.guard_progress(before);
        }
        if notes.is_empty() {
            return Music::Empty;
        }
        let (duration, multiplier) = self.parse_optional_duration_spec();
        let (duration, multiplier) = self.resolve_chord_duration(duration, multiplier);
        let posts = self.parse_posts();
        Music::Chord(Chord {
            kind: ChordKind::Notes(notes),
            duration,
            multiplier,
            from_scale: false,
            posts,
            invisible: false,
        })
    }

    /// Durée seule dans le flux musical : elle répète la note ou l'accord
    /// précédent, exactement comme `q` (`lily/parser.yy`, `note_chord_element`
    /// accepte une durée sans hauteur). C'est l'idiome `<c e g>4 8 4 8` des
    /// accompagnements, où chaque durée rejoue le dernier accord.
    fn parse_bare_duration(&mut self) -> Music {
        let before = self.position;
        let (duration, multiplier) = self.parse_optional_duration_spec();
        if duration.is_none() {
            // Jeton numérique qui n'est pas une durée (reste de `\tempo 4.=50`,
            // fraction laissée par une commande non reconnue) : le consommer
            // une seule fois. `parse_duration` a déjà avancé pour un entier
            // qui n'est pas une durée (`50`) ; l'avancer encore avalerait le
            // jeton suivant (`\time` par exemple) et perdrait la mesure.
            if self.position == before {
                self.position += 1;
            }
            return Music::Empty;
        }
        Music::Chord(Chord {
            kind: ChordKind::Repeat,
            duration,
            multiplier,
            from_scale: false,
            posts: self.parse_posts(),
            invisible: false,
        })
    }

    /// Post-événements internes à un accord, jusqu'à la note suivante.
    fn parse_posts_until_note_end(&mut self) -> Vec<PostEvent> {
        let mut posts = Vec::new();
        while let Some(token) = self.peek().cloned() {
            if !starts_post_event(&token) {
                break;
            }
            let before = self.position;
            let mut parsed = self.parse_post_event();
            posts.append(&mut parsed);
            if !self.guard_progress(before) {
                break;
            }
        }
        posts
    }

    /// Post-événements après un accord.
    fn parse_posts(&mut self) -> Vec<PostEvent> {
        let mut posts = Vec::new();
        while let Some(token) = self.peek().cloned() {
            if !starts_post_event(&token) {
                break;
            }
            let before = self.position;
            let mut parsed = self.parse_post_event();
            posts.append(&mut parsed);
            if !self.guard_progress(before) {
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
            // `\rest` s'applique à la note elle-même, pas au jeton suivant :
            // il ne doit pas passer par `parse_direction_event` (réservé à
            // `^`, `_`, `-` et aux commandes d'articulation qui les suivent).
            Kind::Command if token.text == "rest" => {
                self.position += 1;
                vec![PostEvent::Rest]
            }
            // `\<`, `\>` et `\!` sont des événements isolés (`lily/parser.yy`
            // `direction_less_event`), pas des directions : les laisser passer
            // par la branche générique appellerait `parse_direction_event`,
            // qui consommerait la commande suivante comme argument — avalant
            // par exemple un `\acciaccatura` placé juste après `\!` et
            // transformant son ornement en note ordinaire qui décale la suite.
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
                // `lily/parser.yy` : `post_event_nofinger: direction_less_event`
                // est un événement isolé (`\fermata`, `\p`, `\trill`, …) : il ne
                // consomme aucun jeton. Seule `script_dir music_function_call`
                // prend un argument, et les directions `^`, `_`, `-` sont déjà
                // traitées plus haut. Avaler ici le jeton suivant faisait
                // disparaître un `\times`/`\tuplet` placé après un post-événement.
                let name = token.text.clone();
                self.position += 1;
                if let Some(post) = post_event_for_command(&name, Placement::Below) {
                    return vec![post];
                }
                self.parse_direction_event(&format!("\\{name}"), Placement::Below)
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
            octave_check: None,
            explicit_octave: false,
            force: false,
            cautionary: false,
        };
        // `!` et `?` peuvent précéder ou suivre les marques d'octave. Après
        // `=`, les marques désignent l'octave contrôlée, absolue, et non le
        // décalage relatif (`lily/relative-octave-check.cc`).
        let mut checking = false;
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
                    if checking {
                        *pitch.octave_check.get_or_insert(pitch.octave) += 1;
                    } else {
                        pitch.octave += 1;
                    }
                    pitch.explicit_octave = true;
                    self.position += 1;
                }
                Some(token) if token.is_punct(',') => {
                    if checking {
                        *pitch.octave_check.get_or_insert(pitch.octave) -= 1;
                    } else {
                        pitch.octave -= 1;
                    }
                    pitch.explicit_octave = true;
                    self.position += 1;
                }
                Some(token) if token.is_punct('=') => {
                    // `quotes` après `=` donne l'octave absolue contrôlée
                    // (`absolute-octave = q - 1` dans `lily/parser.yy`) ; les
                    // marques avant `=` appartiennent à la hauteur relative
                    // et ne comptent pas ici (`ly/relative-octave-check.cc`).
                    pitch.octave_check = Some(0);
                    pitch.explicit_octave = true;
                    checking = true;
                    self.position += 1;
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

    /// Durée d'un accord : à défaut de durée écrite, la durée par défaut
    /// **de l'analyse**, multiplicateur compris (`parser->default_duration_`,
    /// `lily/parser.yy` l. 4644, où le facteur fait partie de la `Duration`).
    /// La figer ici évite qu'un `\repeat` rejoue un corps dont la première
    /// note nue hérite de la dernière durée du passage précédent.
    fn resolve_chord_duration(
        &self,
        duration: Option<Duration>,
        multiplier: Option<Fraction>,
    ) -> (Option<Duration>, Option<Fraction>) {
        match duration {
            Some(duration) => (Some(duration), multiplier),
            None => (Some(self.default_duration), Some(self.default_multiplier)),
        }
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
        let mut duration = duration.take()?;
        while self.eat_punct('.') {
            duration.dots = duration.dots.saturating_add(1);
        }
        self.default_duration = duration;
        // Multiplicateurs `*n/d` enchaînés (durée allongée) : LilyPond les
        // multiplie dans l'ordre (`R1*12/8*4` vaut 1 × 12/8 × 4 mesures).
        let mut multiplier = None;
        while self.eat_punct('*') {
            if let Some((numerator, denominator)) = self.parse_ratio() {
                if let Some(value) = Fraction::try_new(i64::from(numerator), i64::from(denominator))
                {
                    multiplier = Some(multiplier.map_or(value, |previous| previous * value));
                }
            }
        }
        // Écrire une durée réinitialise le multiplicateur (`c4*3/2 d4` : `d4`
        // vaut 1/4) ; sans multiplicateur écrit, la durée par défaut n'en a
        // pas non plus.
        self.default_multiplier = multiplier.unwrap_or(Fraction::ONE);
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

    /// Vrai si l'expression de markup commence par un bloc entre accolades,
    /// éventuellement après des commandes de mise en forme (`\markup \column
    /// { … }`, `^\markup \bold {a Tempo}`). Le bloc doit alors être sauté
    /// d'un seul tenant : le laisser en place le ferait lire comme une musique
    /// parasite (régressions `franck_ave_maria`, `duration-line-start-stop`,
    /// `satb-template-with-keyboard-reduction`).
    fn markup_argument_est_entre_accolades(&self) -> bool {
        let mut index = self.position;
        while let Some(token) = self.tokens.get(index) {
            match token.kind {
                Kind::Command => {
                    if self.variables.contains_key(&token.text)
                        || self.music_functions.contains_key(&token.text)
                        || starts_music_command(&token.text)
                        || is_mode_head(&token.text)
                    {
                        return false;
                    }
                    index += 1;
                }
                // `\override #'(…)` et les autres arguments Scheme font
                // partie de la commande de mise en forme.
                Kind::Scheme => index += 1,
                Kind::Punct('{') => return true,
                _ => return false,
            }
        }
        false
    }

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
                    Kind::Command => {
                        // Une commande qui est une variable, une fonction
                        // musicale ou une commande de musique n'appartient
                        // pas au texte : elle est appelée juste après le
                        // markup (`\markup "étiquette" \musique ...`).
                        if self.variables.contains_key(&token.text)
                            || self.music_functions.contains_key(&token.text)
                            || starts_music_command(&token.text)
                            || is_mode_head(&token.text)
                        {
                            break;
                        }
                        self.collect_markup_atom(&mut pieces);
                    }
                    Kind::Str => self.collect_markup_atom(&mut pieces),
                    Kind::Word => {
                        // Silences et temps réservés : ce sont de la musique,
                        // pas du texte. Sans cela `\markup "x" r2 r4 aes`
                        // avale les deux silences dans l'annotation et la
                        // note suivante est jouée quatre noires trop tôt.
                        if self.is_pitch_word(&token.text)
                            || matches!(token.text.as_str(), "r" | "R" | "s")
                        {
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
                kind: Kind::Number, ..
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

    /// Fraction d'une valeur Scheme (`#9/8`, `#(ly:make-moment 9/8)`).
    fn parse_scheme_fraction(&mut self) -> Option<Fraction> {
        let token = self.peek().cloned()?;
        match token.kind {
            Kind::Scheme => {
                self.position += 1;
                let (numerator, denominator) = token.scheme_fraction()?;
                Fraction::try_new(numerator, denominator)
            }
            Kind::Number => {
                self.position += 1;
                fraction_from_token(&token)
            }
            _ => None,
        }
    }

    /// Fraction d'un `\scaleDurations` : jeton `n/d` ou datum Scheme
    /// (`#'(n . d)`, `#(ly:make-moment n d)`…).
    fn parse_scale_fraction(&mut self) -> Option<Fraction> {
        let token = self.peek().cloned()?;
        match token.kind {
            Kind::Number => {
                self.position += 1;
                fraction_from_token(&token)
            }
            Kind::Scheme => {
                self.position += 1;
                scale_scheme_fraction(&token.text)
            }
            _ => None,
        }
    }

    /// Saute une valeur Scheme complète.
    ///
    /// Le cas courant est l'unique jeton [`Kind::Scheme`] du lexer ; une liste
    /// écrite sans `#` reste sautée par équilibrage des parenthèses.
    fn skip_scheme_value(&mut self) {
        match self.peek() {
            Some(token) if token.is_scheme() => {
                self.position += 1;
            }
            Some(token) if token.is_punct('(') => self.skip_paren_list(),
            _ => {
                self.bump();
            }
        }
    }

    /// Saute une liste équilibrée `( … )`.
    fn skip_paren_list(&mut self) {
        let mut depth = 0usize;
        while let Some(token) = self.bump() {
            if token.is_punct('(') {
                depth += 1;
            } else if token.is_punct(')') {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
        }
    }

    /// Saute un chemin de propriété (`Stem.direction`), et le renvoie.
    fn skip_property_path(&mut self) -> Vec<String> {
        let mut parts = Vec::new();
        // Un chemin de propriété est une suite de mots séparés par des points,
        // pas une suite de mots consécutifs : `\unset Staff.keepAliveInterfaces`
        // suivi de `musA = …` doit laisser `musA` au lecteur de variables.
        while let Some(token) = self.peek().cloned() {
            let Kind::Word = token.kind else {
                break;
            };
            // Un nom de note est le début de la musique qui suit, pas un
            // élément du chemin (`\unset melismaBusyProperties f1`).
            if self.is_pitch_word(&token.text) {
                break;
            }
            parts.push(token.text.clone());
            self.position += 1;
            if !self.eat_punct('.') {
                break;
            }
        }
        parts
    }

    /// Ignore une grille d'accords ou de chiffres : LilyPond ne la joue pas
    /// dans le MIDI (`ChordNames` / `FiguredBass`). On lit l'expression pour
    /// la consommer entièrement (variables, blocs imbriqués) puis on la jette,
    /// afin qu'aucune de ses notes n'atteigne la partition. Quand c'est la
    /// seule musique, on signale la limite du sous-ensemble pour renvoyer une
    /// partition vide plutôt qu'une erreur (cf. `lib.rs`).
    fn skip_nonplaying_grid(&mut self, name: &str) -> Music {
        let _ = self.parse_music();
        self.warn(format!("\\{name} : grille non jouée, ignorée"));
        self.warnings.mark_music_ignored();
        Music::Empty
    }

    /// Saute les arguments heuristiques d'une commande inconnue.
    fn skip_command_arguments(&mut self, name: &str, braced_argument: bool) {
        if name.is_empty() {
            return;
        }
        loop {
            let Some(token) = self.peek().cloned() else {
                return;
            };
            match &token.kind {
                Kind::Punct('{') => {
                    // Un `{ … }` qui suit une commande inconnue **dans la
                    // musique** est de la musique ordinaire, pas son argument :
                    // `\autoBeamOff`, `\dynamicUp`, `\oneVoice`,
                    // `\compressMMRests`… n'ont aucun argument et le bloc qui
                    // les suit doit être lu. Le sauter faisait disparaître
                    // toute la musique des voix écrites
                    // `<< \autoBeamOff { … } >>` (régressions Mutopia Greis,
                    // Deutschlandlied, …). Au niveau supérieur, au contraire,
                    // `\layout`/`\paper` portent un bloc de réglages à sauter.
                    if braced_argument {
                        self.skip_braced();
                    }
                    return;
                }
                Kind::Str | Kind::Number => {
                    self.position += 1;
                }
                // Une expression Scheme est un seul jeton : le lire suffit.
                Kind::Scheme => {
                    self.position += 1;
                    return;
                }
                Kind::Word => {
                    // Les noms de hauteur **et de silence** sont de la musique :
                    // `lily/parser.yy` ne leur donne la valeur d'argument que
                    // dans un `music_function_call`, jamais après une commande
                    // sans argument. `\autoBeamOff r8` doit donc garder son `r` :
                    // l'avaler transformait le `8` suivant en note de durée nue
                    // reprenant la hauteur précédente.
                    if self.is_pitch_word(&token.text)
                        || matches!(token.text.as_str(), "r" | "R" | "s" | "q")
                    {
                        return;
                    }
                    self.position += 1;
                    return;
                }
                _ => return,
            }
        }
    }

    /// Cherche `\transposition <hauteur>` dans le bloc `\midi` déjà parcouru.
    ///
    /// Le bloc ne contient que des réglages de performer : les parcourir comme
    /// de la musique ajouterait des notes parasites. La hauteur est lue dans
    /// la langue courante du document.
    fn find_midi_transposition(&self, start: usize) -> Option<PitchSpec> {
        let tokens = self.tokens.get(start..self.position)?;
        for (index, token) in tokens.iter().enumerate() {
            if !token.is_command("transposition") {
                continue;
            }
            let Some(next) = tokens.get(index + 1) else {
                return Some(PitchSpec {
                    step: 0,
                    alter: 0,
                    octave: 1,
                    octave_check: None,
                    explicit_octave: true,
                    force: false,
                    cautionary: false,
                });
            };
            if next.kind != Kind::Word {
                continue;
            }
            let (step, alter) = self.language.parse(&next.text)?;
            let mut octave = 0;
            for mark in tokens.iter().skip(index + 2) {
                if mark.is_punct('\'') {
                    octave += 1;
                } else if mark.is_punct(',') {
                    octave -= 1;
                } else {
                    break;
                }
            }
            return Some(PitchSpec {
                step,
                alter,
                octave,
                octave_check: None,
                explicit_octave: octave != 0,
                force: false,
                cautionary: false,
            });
        }
        None
    }

    /// Vrai si le bloc `\layout` parcouru déplace `Timing_translator` du
    /// contexte `Score` vers `Staff` (`\remove` puis `\consists`, idiomatique
    /// de la polymétrie). Sans ce déplacement, le traducteur vit dans `Score`
    /// et un `\time` écrit dans une portée vaut pour toutes les portées.
    fn timing_translator_deplacé(&self, start: usize) -> bool {
        let Some(tokens) = self.tokens.get(start..self.position) else {
            return false;
        };
        let mut retiré = false;
        let mut ajouté = false;
        for (index, token) in tokens.iter().enumerate() {
            let est_translator = |suivant: Option<&Token>| {
                suivant.is_some_and(|t| t.kind == Kind::Str && t.text == "Timing_translator")
            };
            if token.is_command("remove") && est_translator(tokens.get(index + 1)) {
                retiré = true;
            }
            if token.is_command("consists") && est_translator(tokens.get(index + 1)) {
                ajouté = true;
            }
        }
        retiré && ajouté
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
        if self.at_double_angle_open() || self.peek().is_some_and(|token| token.is_punct('{')) {
            self.skip_braced();
        } else {
            self.bump();
        }
    }
}

/// Reconnaît `#(define nom #{ … #})` dans un datum Scheme et rend le nom et
/// le corps musical. Les autres formes de `define` (nombres, réglages,
/// fonctions) ne sont pas des variables musicales.
fn scheme_music_definition(datum: &str) -> Option<(String, String)> {
    let characters: Vec<char> = datum.chars().collect();
    let mut index = skip_scheme_space(&characters, 0);
    if characters.get(index) != Some(&'(') {
        return None;
    }
    index += 1;
    index = skip_scheme_space(&characters, index);
    let (head, next) = read_scheme_word(&characters, index);
    if head != "define" && head != "define-public" {
        return None;
    }
    index = skip_scheme_space(&characters, next);
    let (name, next) = read_scheme_word(&characters, index);
    if name.is_empty() || next == index {
        return None;
    }
    let body = crate::lexer::embedded_music_body(datum)?;
    Some((name, body))
}

/// Reconnaît une définition de fonction musicale dans un datum Scheme
/// (`#(define-music-function …)`).
///
/// Le corps `#{ … #}` est mémorisé comme jetons ; sans lui, la fonction est
/// Scheme pur et ses appels ne produiront rien.
fn music_function_definition(datum: &str) -> Option<MusicFunction> {
    let characters: Vec<char> = datum.chars().collect();
    let mut index = skip_scheme_space(&characters, 0);
    if characters.get(index) != Some(&'(') {
        return None;
    }
    index += 1;
    let (head, next) = read_scheme_word(&characters, index);
    index = next;
    let kind = match head.as_str() {
        "define-music-function" => FunctionKind::Music,
        "define-void-function" | "define-scheme-function" => FunctionKind::Silent,
        _ => return None,
    };
    let (params_text, next) = read_scheme_list(&characters, index)?;
    index = next;
    let (predicates_text, next) = read_scheme_list(&characters, index)?;
    index = next;

    let mut params = split_scheme_words(&params_text);
    // Signatures anciennes `(parser location a b)` et `(nom location a b)`
    // (LilyPond 2.8) : les deux premiers noms désignent le parseur ou la
    // fonction elle-même et la position, pas des arguments. Les confondre
    // ferait consommer la musique qui suit l'appel comme paramètres.
    if params.len() >= 2 && params[1] == "location" {
        params.drain(..2);
    }
    let predicates = split_scheme_words(&predicates_text);
    let arity = params.len().max(predicates.len());
    let params: Vec<String> = (0..arity)
        .map(|position| params.get(position).cloned().unwrap_or_default())
        .collect();
    let predicates: Vec<String> = (0..arity)
        .map(|position| {
            predicates
                .get(position)
                .cloned()
                .unwrap_or_else(|| "ly:music?".to_owned())
        })
        .collect();

    let body_source: String = characters[index.min(characters.len())..].iter().collect();
    // Le corps utile est le premier `#{ … #}` ; tout ce qui le précède est
    // du Scheme. `make-relative` y relit ses arguments dans le contexte
    // relatif avant de les insérer dans le corps, y compris sous un
    // `\transpose` (`scm/music-functions.scm`, `make-relative`).
    let relative_body = body_source
        .split_once("#{")
        .is_some_and(|(prefix, _)| prefix.contains("make-relative"));
    let body =
        crate::lexer::embedded_music_body(&body_source).map(|body| crate::lexer::tokenize(&body));
    Some(MusicFunction {
        kind,
        params,
        predicates,
        body,
        relative_body,
        warned: false,
    })
}

/// Saute espaces et commentaires de ligne Scheme.
fn skip_scheme_space(characters: &[char], mut index: usize) -> usize {
    loop {
        while characters
            .get(index)
            .is_some_and(|character| character.is_whitespace())
        {
            index += 1;
        }
        if characters.get(index) == Some(&';') {
            while characters
                .get(index)
                .is_some_and(|character| *character != '\n')
            {
                index += 1;
            }
            continue;
        }
        return index;
    }
}

/// Lit un atome Scheme (nom de fonction, paramètre) et l'indice qui le suit.
fn read_scheme_word(characters: &[char], start: usize) -> (String, usize) {
    let mut index = skip_scheme_space(characters, start);
    let begin = index;
    while characters
        .get(index)
        .is_some_and(|character| !character.is_whitespace() && !matches!(character, '(' | ')'))
    {
        index += 1;
    }
    let word = characters[begin..index].iter().collect();
    (word, index)
}

/// Lit la liste Scheme `( … )` à `start` et renvoie son contenu (sans les
/// parenthèses) et l'indice qui la suit.
fn read_scheme_list(characters: &[char], start: usize) -> Option<(String, usize)> {
    let mut index = skip_scheme_space(characters, start);
    if characters.get(index) != Some(&'(') {
        return None;
    }
    index += 1;
    let begin = index;
    let mut depth = 1usize;
    while index < characters.len() {
        match characters[index] {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    let inner = characters[begin..index].iter().collect();
                    return Some((inner, index + 1));
                }
            }
            '"' => {
                index += 1;
                while index < characters.len() {
                    match characters[index] {
                        '\\' => index += 2,
                        '"' => break,
                        _ => index += 1,
                    }
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

/// Découpe une liste Scheme en mots de premier niveau.
fn split_scheme_words(text: &str) -> Vec<String> {
    let characters: Vec<char> = text.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for character in characters {
        if character == '(' {
            depth += 1;
            current.push(character);
        } else if character == ')' {
            depth = depth.saturating_sub(1);
            current.push(character);
        } else if character.is_whitespace() && depth == 0 {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Remplace `#a`, `$a` et `\a` par l'argument du paramètre `a` (substitution
/// de jetons, sans évaluation Scheme).
fn substitute_function_tokens(
    body: &[Token],
    params: &[String],
    arguments: &[Vec<Token>],
) -> Vec<Token> {
    let mut tokens = Vec::with_capacity(body.len());
    for token in body {
        let replaced = match &token.kind {
            Kind::Scheme | Kind::Command => params.iter().position(|param| *param == token.text),
            _ => None,
        };
        if let Some(index) = replaced {
            if let Some(argument) = arguments.get(index) {
                tokens.extend(argument.iter().cloned());
                continue;
            }
        }
        tokens.push(token.clone());
    }
    tokens
}

/// Fonction musicale qui enveloppe une musique quelconque, contextes compris.
///
/// LilyPond les applique récursivement à tout ce que contient la musique
/// (`\transpose`, `\relative`, `\fixed` sont des fonctions Scheme de
/// `music-functions-init.ly`) : la construction des portées doit donc
/// descendre à travers elles et laisser la transformation à chaque voix.
#[derive(Clone, Copy)]
enum Wrapper {
    /// `\transpose de vers`.
    Transpose {
        from: PitchSpec,
        to: PitchSpec,
        absolute: bool,
    },
    /// `\fixed [référence]`.
    Fixed(Option<PitchSpec>),
    /// `\relative [référence]`.
    Relative(Option<PitchSpec>),
}

impl Wrapper {
    /// Enveloppe que porte `context`, avec la musique qu'elle contient.
    fn of(context: &Context) -> Option<(Self, &Music)> {
        match context {
            Context::Transpose {
                from,
                to,
                absolute,
                music,
            } => Some((
                Self::Transpose {
                    from: *from,
                    to: *to,
                    absolute: *absolute,
                },
                music,
            )),
            Context::Fixed { reference, music } => Some((Self::Fixed(*reference), music)),
            Context::Relative { reference, music } => Some((Self::Relative(*reference), music)),
            _ => None,
        }
    }

    /// Applique l'enveloppe à une musique.
    fn wrap(self, music: Music) -> Music {
        let music = Box::new(music);
        Music::Context(match self {
            Self::Transpose { from, to, absolute } => Context::Transpose {
                from,
                to,
                absolute,
                music,
            },
            Self::Fixed(reference) => Context::Fixed { reference, music },
            Self::Relative(reference) => Context::Relative { reference, music },
        })
    }
}

/// Réapplique une pile d'enveloppes (`[externe, …, interne]`) à une musique.
fn wrap_all(wrappers: &[Wrapper], music: Music) -> Music {
    wrappers
        .iter()
        .rev()
        .fold(music, |music, wrapper| wrapper.wrap(music))
}

/// Applique une pile d'enveloppes à toutes les voix d'une portée ou d'un
/// groupe : la transformation porte sur chaque voix du contenu.
fn wrap_staves(wrappers: &[Wrapper], staves: &mut [StaffBlock]) {
    if wrappers.is_empty() {
        return;
    }
    for staff in staves {
        for voice in &mut staff.voices {
            let music = std::mem::replace(&mut voice.music, Music::Empty);
            voice.music = wrap_all(wrappers, music);
        }
    }
}

fn has_context(music: &Music) -> bool {
    match music {
        Music::New(_) | Music::Lyrics(_) => true,
        Music::Context(context) => {
            Wrapper::of(context).is_some_and(|(_, inner)| has_context(inner))
        }
        Music::Sim(items) | Music::Seq(items) => items.iter().any(has_context),
        Music::Tag { music, .. } | Music::Filtered { music, .. } => has_context(music),
        _ => false,
    }
}

/// Paroles portées par la musique d'une portée (`\new Lyrics`, `\lyricsto`,
/// `Music::Lyrics`) : elles restent sur la portée même quand toute sa
/// musique est gardée en une seule expression. Les paroles d'un `\new Voice`
/// sont déjà portées par ce groupe et ne sont pas prises ici.
fn collect_lyrics_in_music(music: &Music, out: &mut Vec<LyricsBlock>) {
    match music {
        Music::New(group) => {
            if group.kind != GroupKind::Voice {
                out.extend(group.lyrics.iter().cloned());
            }
            for staff in &group.staves {
                for voice in &staff.voices {
                    collect_lyrics_in_music(&voice.music, out);
                }
            }
        }
        Music::Lyrics(block) => out.push((**block).clone()),
        Music::Seq(items) | Music::Sim(items) => {
            for item in items {
                collect_lyrics_in_music(item, out);
            }
        }
        Music::Context(context) => {
            if let Some((_, inner)) = Wrapper::of(context) {
                collect_lyrics_in_music(inner, out);
            }
        }
        Music::Tag { music, .. } | Music::Filtered { music, .. } => {
            collect_lyrics_in_music(music, out)
        }
        _ => {}
    }
}

/// Nom de la dernière voix explicitement nommée d'une musique, pour
/// rattacher un `\addlyrics` qui la suit.
fn last_voice_name(music: &Music) -> Option<String> {
    match music {
        Music::New(group) if group.kind == GroupKind::Voice => group
            .staves
            .first()
            .and_then(|staff| staff.voices.last())
            .and_then(|voice| voice.name.clone()),
        Music::Context(Context::Voice(_)) => None,
        Music::Seq(items) | Music::Sim(items) => items.iter().rev().find_map(last_voice_name),
        Music::Context(context) => {
            Wrapper::of(context).and_then(|(_, inner)| last_voice_name(inner))
        }
        Music::Tag { music, .. } | Music::Filtered { music, .. } => last_voice_name(music),
        _ => None,
    }
}

/// Portée anonyme qui porte une musique sans contexte explicite.
fn anonymous_group(music: Music) -> GroupBlock {
    let mut staff = StaffBlock::new();
    if !music.is_empty() {
        staff.voices.push(VoiceBlock { name: None, music });
    }
    GroupBlock {
        kind: GroupKind::Staff,
        existing: false,
        instrument_name: None,
        short_instrument_name: None,
        staves: vec![staff],
        lyrics: Vec::new(),
    }
}

/// Têtes de mode qui décrivent une grille de chiffres non jouée
/// (`mode_changing_head` de `lily/parser.yy`). Le contexte FiguredBass ne
/// produit pas de notes dans le MIDI de référence. `chords`, lui, est joué
/// par LilyPond et suit le chemin de `chordmode`.
fn is_nonplaying_mode_head(name: &str) -> bool {
    matches!(name, "figuremode" | "figures")
}

/// Commandes de changement de mode (`mode_changing_head` et
/// `mode_changing_head_with_context` de `lily/parser.yy`, ainsi que
/// `SIMULTANEOUS`). Elles modifient l'interprétation lexicale des jetons
/// suivants (chordes, figures, paroles) ; pour le lecteur, la musique
/// enveloppée reste une musique ordinaire, sauf les grilles de chiffres non
/// jouées ([`is_nonplaying_mode_head`]).
fn is_mode_head(name: &str) -> bool {
    matches!(
        name,
        "chordmode"
            | "figuremode"
            | "notemode"
            | "drummode"
            | "chords"
            | "figures"
            | "lyrics"
            | "drums"
            | "simultaneous"
    )
}

/// Vrai si la commande ouvre une expression musicale reconnue par
/// `parse_music` (hors variables musicales, testées à part).
/// Noms de commandes musicales supprimés, acceptés tels quels par le lecteur.
///
/// `python/convertrules.py` des sources LilyPond 2.26/2.27 les réécrit dans
/// la règle `@rule((2, 11, 51))` (et `(2, 11, 52)` pour `setHairpinDim`) :
/// `\setTextCresc -> \crescTextCresc`, `\sustainDown -> \sustainOn`, etc.
/// Seuls les noms qui changent la musique jouée sont couverts ici ; c'est un
/// alias de lecture, pas un port de `convert-ly`.
fn obsolete_music_command(name: &str) -> Option<&'static str> {
    match name {
        "octave" => Some("octaveCheck"),
        "arpeggioUp" => Some("arpeggioArrowUp"),
        "arpeggioDown" => Some("arpeggioArrowDown"),
        "arpeggioNeutral" => Some("arpeggioNormal"),
        "setTextCresc" => Some("crescTextCresc"),
        "setTextDecresc" => Some("dimTextDecresc"),
        "setTextDecr" => Some("dimTextDecr"),
        "setTextDim" => Some("dimTextDim"),
        "setHairpinCresc" => Some("crescHairpin"),
        "setHairpinDecresc" => Some("dimHairpin"),
        "setHairpinDim" => Some("dimHairpin"),
        "sustainUp" => Some("sustainOff"),
        "sustainDown" => Some("sustainOn"),
        "sostenutoDown" => Some("sostenutoOn"),
        "sostenutoUp" => Some("sostenutoOff"),
        _ => None,
    }
}

fn starts_music_command(name: &str) -> bool {
    is_mode_head(name)
        || matches!(
            name,
            "relative"
                | "fixed"
                | "absolute"
                | "unfoldRepeats"
                | "transpose"
                | "tuplet"
                | "times"
                | "scaleDurations"
                | "grace"
                | "slashedGrace"
                | "acciaccatura"
                | "appoggiatura"
                | "afterGrace"
                | "repeat"
                | "alternative"
                | "new"
                | "with"
                | "key"
                | "time"
                | "clef"
                | "partial"
                | "bar"
                | "cadenzaOn"
                | "cadenzaOff"
                | "section"
                | "break"
                | "noBreak"
                | "pageBreak"
                | "pageTurn"
                | "tempo"
                | "mark"
                | "ottava"
                | "sustainOn"
                | "sustainOff"
                | "voiceOne"
                | "voiceTwo"
                | "voiceThree"
                | "voiceFour"
                | "stemUp"
                | "stemDown"
                | "set"
                | "unset"
                | "once"
                | "override"
                | "revert"
                | "tweak"
                | "skip"
                | "tag"
                | "keepWithTag"
                | "removeWithTag"
                | "tagGroup"
                | "resetTagGroups"
                | "tagGroupRef"
                | "resetTagGroup"
                | "addToTagGroup"
                | "removeFromTagGroup"
        )
}

/// Vrai si le jeton commence un post-événement (`post_events`).
fn starts_post_event(token: &Token) -> bool {
    match &token.kind {
        Kind::Punct(character) => matches!(
            character,
            '~' | '(' | ')' | '[' | ']' | ':' | '^' | '_' | '-'
        ),
        Kind::Command => {
            token.text == "="
                || token.text == "<"
                || token.text == ">"
                || token.text == "!"
                || token.text == "markup"
                || token.text == "rest"
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
    Fraction::try_new(numerator, denominator)
}

/// Fraction portée par un datum Scheme de `\scaleDurations` : paire pointée
/// `'(n . d)`, appel `(ly:make-moment n d)` ou `(ly:make-moment n/d)`, ou
/// simple fraction `n/d`.
fn after_grace_pair(datum: &str) -> Option<(i64, i64)> {
    let start = datum.find("(cons")? + "(cons".len();
    let inner = datum[start..].trim_start();
    let close = inner.find(')')?;
    let mut parts = inner[..close].split_whitespace();
    let numerator = parts.next()?.parse().ok()?;
    let denominator = parts.next()?.parse().ok()?;
    Some((numerator, denominator))
}

fn scale_scheme_fraction(datum: &str) -> Option<Fraction> {
    let text = datum.trim();
    let body = text.strip_prefix('\'').unwrap_or(text).trim();
    let inner = body
        .strip_prefix('(')
        .and_then(|text| text.strip_suffix(')'))
        .map(str::trim);
    let atom = if let Some(inner) = inner {
        if let Some(args) = inner.strip_prefix("ly:make-moment") {
            let args = args.trim();
            if let Some((numerator, denominator)) = args.split_once(char::is_whitespace) {
                return Fraction::try_new(
                    numerator.trim().parse().ok()?,
                    denominator.trim().parse().ok()?,
                );
            }
            args
        } else {
            // Paire pointée `'(n . d)`.
            let (numerator, denominator) = inner.split_once('.')?;
            return Fraction::try_new(
                numerator.trim().parse().ok()?,
                denominator.trim().parse().ok()?,
            );
        }
    } else {
        body
    };
    let (numerator, denominator) = atom.split_once('/').unwrap_or((atom, "1"));
    Fraction::try_new(
        numerator.trim().parse().ok()?,
        denominator.trim().parse().ok()?,
    )
}

/// Multiplie par `factor` le multiplicateur `*n/d` de chaque accord d'une
/// musique. C'est le mécanisme déjà utilisé par l'interpréteur pour les durées
/// allongées : aucune nouvelle durée n'est introduite.
fn scale_music(music: &mut Music, factor: &Fraction) {
    match music {
        Music::Seq(items) | Music::Sim(items) => {
            for item in items {
                scale_music(item, factor);
            }
        }
        Music::Chord(chord) => {
            let multiplier = chord.multiplier.unwrap_or(Fraction::ONE) * *factor;
            chord.multiplier = Some(multiplier);
            chord.from_scale = true;
        }
        Music::Context(context) => scale_context(context, factor),
        Music::Tag { music, .. } | Music::Filtered { music, .. } => scale_music(music, factor),
        Music::Empty | Music::New(_) | Music::Lyrics(_) | Music::Unknown(_) => {}
    }
}

/// Applique un facteur d'échelle aux musiques contenues dans une commande.
fn scale_context(context: &mut Context, factor: &Fraction) {
    match context {
        Context::Tuplet { music, .. }
        | Context::Grace { music, .. }
        | Context::Fixed { music, .. }
        | Context::Relative { music, .. }
        | Context::NonRelative { music, .. }
        | Context::Transpose { music, .. } => scale_music(music, factor),
        Context::AfterGrace { main, grace, .. } => {
            scale_music(main, factor);
            scale_music(grace, factor);
        }
        Context::Repeat {
            body, alternatives, ..
        } => {
            scale_music(body, factor);
            for alternative in alternatives {
                scale_music(alternative, factor);
            }
        }
        Context::Alternative(items) => {
            for item in items {
                scale_music(item, factor);
            }
        }
        _ => {}
    }
}

/// Valeur texte d'un datum Scheme.
///
/// Une chaîne `"…"` est déquottée et déséchappée comme la chaîne LilyPond
/// équivalente ; tout autre datum est renvoyé tel quel.
fn scheme_text_value(datum: &str) -> String {
    let datum = datum.trim();
    if datum.len() >= 2 && datum.starts_with('"') && datum.ends_with('"') {
        return datum[1..datum.len() - 1].replace("\\\"", "\"");
    }
    datum.to_owned()
}

/// Vrai pour la liste Scheme vide `#'()` (`()` ou `'()` comme texte de jeton).
fn is_empty_scheme_list(datum: &str) -> bool {
    let datum = datum.trim().trim_start_matches('\'').trim();
    datum.split_whitespace().collect::<String>() == "()"
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
        "rest" => PostEvent::Rest,
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

/// Numéro de couplet (à partir de zéro) d'une valeur de `stanza`.
///
/// `\set stanza = "2."` et `\with { stanza = "2." }` donnent le même numéro ;
/// un libellé non numérique (`"R."` pour un refrain) laisse le numéro par
/// défaut, qui sera attribué par ordre d'apparition.
fn stanza_verse(value: &str) -> Option<usize> {
    value
        .trim()
        .trim_end_matches('.')
        .trim()
        .parse::<usize>()
        .ok()
        .and_then(|number| number.checked_sub(1))
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
    use crate::lexer::tokenize;

    fn parse_music_only(source: &str) -> Music {
        let mut warnings = Warnings::new();
        let mut parser = Parser {
            tokens: tokenize(source),
            position: 0,
            language: Language::English,
            variables: BTreeMap::new(),
            default_duration: Duration::quarter(),
            default_multiplier: Fraction::ONE,
            warnings: &mut warnings,
            pending_head: NoteHeadGroup::Normal,
            pending_tuplet_number: None,
            pending_invisible_rest: false,
            partitions: Vec::new(),
            midi_transposition: None,
            playback_articulate: false,
            playback_unfold_repeats: false,
            music_functions: BTreeMap::new(),
            expansion_depth: 0,
            melisma_busy_disabled: false,
            after_grace_fraction: Fraction::new(3, 4),
            staff_local_timing: false,
            note_mode: NoteMode::Notes,
            in_relative_function: false,
            tag_groups: default_tag_groups(),
            scheme_variables: BTreeMap::new(),
        };
        parser.parse_music()
    }

    /// Items d'une expression, une expression seule comptant pour un.
    fn items(music: Music) -> Vec<Music> {
        match music {
            Music::Seq(items) => items,
            other => vec![other],
        }
    }

    #[test]
    fn garde_de_progression() {
        // Sur `c d`, une itération qui consomme passe, une itération bloquée
        // avertit et force l'avancement, et la fin de flux signale l'arrêt sans
        // tenter de consommer un jeton inexistant.
        let (progresse, bloque, fin, position, avertissements) = {
            let mut warnings = Warnings::new();
            let mut parser = Parser {
                tokens: tokenize("c d"),
                position: 1,
                language: Language::English,
                variables: BTreeMap::new(),
                default_duration: Duration::quarter(),
                default_multiplier: Fraction::ONE,
                warnings: &mut warnings,
                pending_head: NoteHeadGroup::Normal,
                pending_tuplet_number: None,
                pending_invisible_rest: false,
                partitions: Vec::new(),
                midi_transposition: None,
                playback_articulate: false,
                playback_unfold_repeats: false,
                music_functions: BTreeMap::new(),
                expansion_depth: 0,
                melisma_busy_disabled: false,
                after_grace_fraction: Fraction::new(3, 4),
                staff_local_timing: false,
                note_mode: NoteMode::Notes,
                in_relative_function: false,
                tag_groups: default_tag_groups(),
                scheme_variables: BTreeMap::new(),
            };
            let progresse = parser.guard_progress(0);
            let bloque = parser.guard_progress(1);
            let fin = parser.guard_progress(2);
            let position = parser.position;
            drop(parser);
            (progresse, bloque, fin, position, warnings.into_vec())
        };
        assert!(progresse);
        assert!(!bloque);
        assert!(!fin);
        assert_eq!(position, 2);
        assert_eq!(avertissements.len(), 2);
        assert!(
            avertissements
                .iter()
                .all(|warning| warning.message.contains("progression forcée")),
            "avertissements : {avertissements:?}"
        );
    }

    #[test]
    fn accord_avec_duree_et_liaison() {
        let items = items(parse_music_only("{ c'4~ d8. }"));
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
        let items = items(parse_music_only(
            "{ \\key g \\major \\time 3/4 \\clef bass }",
        ));
        assert!(items.iter().any(|item| matches!(
            item,
            Music::Context(Context::Key {
                fifths: 1,
                mode: KeyMode::Unknown
            })
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
        let items = items(parse_music_only(
            "{ \\tuplet 3/2 { c8 d8 e8 } \\repeat volta 2 { f4 g4 } }",
        ));
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
