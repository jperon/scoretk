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

use std::collections::{BTreeMap, BTreeSet};

use scoretk_core::model::{
    ArpeggioType, ArticulationType, Duration, DurationType, Fraction, GraceType, KeyMode,
    NoteHeadGroup, OrnamentType, Placement, StemDirection, TremoloKind, TupletNumberType,
};

use crate::ast::VoicesId;
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
    /// Dernier type de trémolo écrit (`:8`), comme
    /// `parser->default_tremolo_type_` (`lily/parser.yy:3584-3596`).
    default_tremolo_type: u8,
    /// Silence invisible demandé par `\once \override Rest.transparent`.
    pending_invisible_rest: bool,
    /// Crochet d'ottava masqué par `\override OttavaBracket.transparent`.
    pending_invisible_ottava: bool,
    /// Partitions rencontrées au niveau supérieur, dans l'ordre du fichier.
    partitions: Vec<Partition>,
    /// `\transposition` trouvé dans le bloc `\midi` de la partition courante.
    midi_transposition: Option<PitchSpec>,
    /// `#(set-global-staff-size n)` (Notation Reference §27.2).
    staff_size: Option<f64>,
    /// Argument du dernier `\fontsize #n` lu dans un markup de `\header`.
    last_markup_fontsize: Option<f64>,
    /// Champ `\header` en cours d'analyse, pour les marqueurs de markup.
    current_header_field: Option<String>,
    /// Champs `\header` dont le markup contient `\bold`.
    header_bold_fields: Vec<String>,
    /// Vélocité lue dans `\set Staff.midiMinimumVolume = #n` (§24.4.2),
    /// pour la nuance suivante.
    pending_dynamic_velocity: Option<u8>,
    /// Genre de trémolo porté par `\set Staff.scoretkTremoloKind = #n`.
    pending_tremolo_kind: Option<TremoloKind>,
    /// Groupe de tête exact porté par le markup privé `scoretk-head-…`.
    pending_exact_head: Option<NoteHeadGroup>,
    /// `\articulate` a été rencontré (`ly/articulate.ly`).
    playback_articulate: bool,
    /// `\unfoldRepeats` a été rencontré : le MIDI déplie les reprises.
    playback_unfold_repeats: bool,
    /// `midiInstrument` lu par `\set` en attendant le contexte `Staff` auquel
    /// il s'applique (`lily/staff-performer.cc:244`). Une propriété de
    /// contexte n'est **pas** une musique : l'émettre dans le flux en ferait
    /// une portée anonyme surnuméraire dans un `\new PianoStaff << … >>`.
    pending_midi_instrument: Option<String>,
    /// Fonctions musicales définies en Scheme (`define-music-function`…).
    music_functions: BTreeMap<String, MusicFunction>,
    /// Définitions `\addInstrumentDefinition` : nom → transposition
    /// d'instrument (`ly/music-functions-init.ly:46-54`).
    instrument_definitions: BTreeMap<String, PitchSpec>,
    /// Profondeur d'expansion des blocs embarqués et des fonctions.
    expansion_depth: u32,
    /// Fraction par défaut d'un `\afterGrace` sans argument
    /// (`afterGraceFraction`, 3/4 dans `ly/music-functions-init.ly`).
    after_grace_fraction: Fraction,
    /// `\set melismaBusyProperties = #'()` a été vu : les paroles ne sautent
    /// plus les notes occupées (liaisons d'expression, liaisons…).
    melisma_busy_disabled: bool,
    /// `\set Score.skipTypesetting = ##t` : `lily/score-performer.cc:151-176`
    /// n'appelle pas `PROCESS_MUSIC` et replie le temps (`midiSkipOffset`) ;
    /// les notes de la zone ne sont ni jouées ni comptées dans le temps.
    skip_typesetting: bool,
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
    /// Identifiants de durée définis par `#(ly:make-duration …)` :
    /// durée de base et facteur de compression (`lily/duration-scheme.cc`).
    duration_identifiers: BTreeMap<String, (Duration, Fraction)>,
    /// Musiques déclarées par `\addQuote`, consultées par `\cueDuring`
    /// (`ly/music-functions-init.ly:56-60,545-574`).
    quotes: BTreeMap<String, Music>,
    /// Quotes currently being resolved, to stop mutual `\addQuote` cycles.
    resolving_quotes: BTreeSet<String>,
    /// Identifiants Scheme liés à un entier (`num = #3`).
    numbers: BTreeMap<String, i64>,
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
    /// Default tokens of an optional argument (`Some` only for an optional
    /// predicate). LilyPond's `function_arglist` accepts `(type? default)`
    /// and `(type? *unspecified*)`; an absent argument is replaced by the
    /// default expression re-parsed as if it had been written at the call.
    optional_defaults: Vec<Option<Vec<Token>>>,
    /// Corps musical (`#{ … #}`), absent pour une fonction Scheme pure.
    body: Option<Vec<Token>>,
    /// Fonction dont le corps Scheme est un seul paramètre (`music`) : elle
    /// renvoie cet argument (`music-function-string-markup.ly`).
    returns: Option<usize>,
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
    /// `\articulate` vu dans cette partition seulement.
    articulate: bool,
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

type FiltreTags = (TagFilter, Vec<String>, Vec<Vec<String>>);

/// Filtre structurel des étiquettes, avant l'extraction des portées d'un
/// `\score`.
///
/// `keepWithTag`/`removeWithTag` sont des fonctions musicales
/// (`ly/music-functions-init.ly:919-933`, `:2090-2100`) qui appellent
/// `music-filter` (`scm/lily/music-functions.scm:110-112`) : le prédicat est
/// appliqué à **chaque** expression, et une expression refusée disparaît avec
/// tout son sous-arbre. Le filtrage différé de `Music::Filtered` dans
/// `events.rs` ne suffit pas lorsque l'expression marquée est un contexte : la
/// portée refusée resterait dans le modèle, et les deux versions d'un
/// `\keepWithTag` seraient jouées (`contrapunctusXVI/…-lys/score.ly`).
///
/// On ne descend pas dans `Music::Context` : `\relative` doit encore voir
/// toutes ses branches pour choisir la hauteur de référence
/// (`Chop-28-16`, cf. `ast.rs` `Music::Filtered`). Le filtre différé continue
/// de juger les événements de ces branches.
fn filtrer_structures_tags(music: Music, filtres: &mut Vec<FiltreTags>) -> Music {
    let mut filtre_seq = |items: Vec<Music>| -> Vec<Music> {
        items
            .into_iter()
            .map(|item| filtrer_structures_tags(item, filtres))
            .filter(|item| !item.is_empty())
            .collect()
    };
    match music {
        Music::Tag { tags, music } => {
            let garde = filtres
                .iter()
                .all(|(filter, wanted, groups)| tags_kept(&tags, wanted, *filter, groups));
            if garde {
                Music::Tag {
                    tags,
                    music: Box::new(filtrer_structures_tags(*music, filtres)),
                }
            } else {
                Music::Empty
            }
        }
        Music::Filtered {
            filter,
            wanted,
            groups,
            music,
        } => {
            filtres.push((filter, wanted.clone(), groups.clone()));
            let interieur = filtrer_structures_tags(*music, filtres);
            filtres.pop();
            Music::Filtered {
                filter,
                wanted,
                groups,
                music: Box::new(interieur),
            }
        }
        Music::Seq(items) => Music::Seq(filtre_seq(items)),
        Music::Sim(items) => Music::Sim(filtre_seq(items)),
        Music::SimVoices(items) => Music::SimVoices(filtre_seq(items)),
        other => other,
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
        default_tremolo_type: 8,
        pending_invisible_rest: false,
        pending_invisible_ottava: false,
        partitions: Vec::new(),
        midi_transposition: None,
        staff_size: None,
        last_markup_fontsize: None,
        current_header_field: None,
        header_bold_fields: Vec::new(),
        pending_dynamic_velocity: None,
        pending_tremolo_kind: None,
        pending_exact_head: None,
        playback_articulate: false,
        playback_unfold_repeats: false,
        pending_midi_instrument: None,
        music_functions: BTreeMap::new(),
        instrument_definitions: BTreeMap::new(),
        expansion_depth: 0,
        melisma_busy_disabled: false,
        skip_typesetting: false,
        after_grace_fraction: Fraction::new(3, 4),
        staff_local_timing: false,
        note_mode: NoteMode::Notes,
        in_relative_function: false,
        tag_groups: default_tag_groups(),
        scheme_variables: BTreeMap::new(),
        duration_identifiers: BTreeMap::new(),
        numbers: BTreeMap::new(),
        quotes: BTreeMap::new(),
        resolving_quotes: BTreeSet::new(),
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
        document.staff_size = self.staff_size;
        document.staff_local_timing = self.staff_local_timing;
        document.playback_unfold_repeats = self.playback_unfold_repeats;
        // `showFirstLength`/`showLastLength` sont des variables musicales
        // (`showLastLength = R1*3`) : `skip-as-needed`
        // (`scm/music-functions.scm:1526-1600`) en prend la longueur et coupe
        // la partition pour le MIDI.
        document.show_first_length = self
            .variables
            .get("showFirstLength")
            .map(|music| music_duration(music, Duration::whole()));
        document.show_last_length = self
            .variables
            .get("showLastLength")
            .map(|music| music_duration(music, Duration::whole()));

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
                document.playback_articulate = partition.articulate;
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
                document.header.font_sizes.extend(header.font_sizes);
                document.header.bold_fields.extend(header.bold_fields);
            }
            Kind::Command if token.text == "paper" => {
                self.position += 1;
                let paper = self.parse_header();
                document.paper.entries.extend(paper.entries);
            }
            Kind::Command if token.text == "addQuote" => {
                // `ly/music-functions-init.ly:56-60` : `addQuote` déclare une
                // musique citable ; elle n'est pas jouée ici, seulement
                // mémorisée pour un `\cueDuring` ultérieur.
                self.position += 1;
                let nom = match self.peek().cloned() {
                    Some(Token {
                        kind: Kind::Str | Kind::Word,
                        text,
                        ..
                    }) => {
                        self.position += 1;
                        text
                    }
                    _ => String::new(),
                };
                let music = self.parse_music();
                self.quotes.insert(nom, music);
            }
            Kind::Command if token.text == "score" => {
                self.position += 1;
                let (score, has_layout, has_midi, unfold_repeats, articulate) = self.parse_score();
                self.partitions.push(Partition {
                    score,
                    explicit: true,
                    has_layout,
                    has_midi,
                    unfold_repeats,
                    articulate,
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
            Kind::Command if token.text == "addInstrumentDefinition" => {
                self.add_instrument_definition();
            }
            Kind::Command if token.text == "parallelMusic" => {
                self.position += 1;
                self.parse_parallel_music();
            }
            Kind::Word => {
                // Assignation `nom = musique`, y compris les noms pointés
                // des alists (`violin.1 = { … }`, `lily/lexer.ll`).
                let (name, extra) = self.nom_pointe(&token.text, 1);
                if self
                    .peek_at(1 + extra)
                    .is_some_and(|next| next.is_punct('='))
                {
                    self.position += 2 + extra;
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
                if let Some(langage) = scheme_note_names_language(&datum) {
                    self.language = langage;
                }
                if let Some(size) = scheme_staff_size(&datum) {
                    self.staff_size = Some(size);
                }
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
        self.playback_articulate = false;
        let mut groups = self.parse_score_element();
        let unfold_repeats = self.playback_unfold_repeats;
        let articulate = self.playback_articulate;
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
            articulate,
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
            self.last_markup_fontsize = None;
            self.current_header_field = Some(name.clone());
            let value = self.parse_header_value();
            self.current_header_field = None;
            if let Some(size) = self.last_markup_fontsize.take() {
                // `\fontsize #n` est une commande de markup standard ; sa
                // valeur est le transport de la taille de texte du modèle.
                header.font_sizes.push((name.clone(), size));
            }
            if !name.is_empty() {
                header.entries.push((name, value));
            }
        }
        header.bold_fields = std::mem::take(&mut self.header_bold_fields);
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
    fn parse_score(&mut self) -> (ScoreAst, bool, bool, bool, bool) {
        let mut score = ScoreAst::default();
        let mut has_layout = false;
        let mut has_midi = false;
        self.midi_transposition = None;
        // `\unfoldRepeats` est propre au score courant
        // (`scm/music-functions.scm`) : un second `\score` qui déplie ses
        // reprises ne doit pas déplier celles du premier, dont l'oracle
        // retient le MIDI (`scm/midi.scm`, `base.midi`).
        self.playback_unfold_repeats = false;
        self.playback_articulate = false;
        if !self.eat_punct('{') {
            self.warn("\\score sans accolade");
            return (score, has_layout, has_midi, false, false);
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
        (
            score,
            has_layout,
            has_midi,
            self.playback_unfold_repeats,
            self.playback_articulate,
        )
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
        let mut filtres = Vec::new();
        let music = filtrer_structures_tags(music, &mut filtres);
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
                sequence_start: false,
                instrument_name: None,
                short_instrument_name: None,
                midi_instrument: None,
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
                self.context_groups(items, wrappers, Music::Sim, false, true)
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
                self.context_groups(items, wrappers, Music::Seq, true, false)
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
        sequential: bool,
        simultaneous: bool,
    ) -> Vec<GroupBlock> {
        // Musique simultanée (`<< … >>`) : chaque élément sans contexte est un
        // contexte à lui seul. `<< \relative … \addlyrics … \relative … >>`
        // (le `\addlyrics` est un `Music::Lyrics`, donc un contexte) joue
        // **deux portées** plus la piste de paroles ; les réunir en une portée
        // à plusieurs voix faisait fusionner leurs unissons à l'oracle
        // (`samia-a4`, `Nunc-dimittis`).
        if simultaneous {
            let mut groups = Vec::new();
            for item in items {
                if has_context(&item) {
                    groups.extend(self.score_groups_in(item, wrappers));
                } else {
                    groups.push(anonymous_group(wrap_all(wrappers, item)));
                }
            }
            return groups;
        }
        // `{ c4 \new Voice { … } e4 }` : une voix ouverte dans une séquence
        // est un `Voice` enfant du `Staff` implicite, pas une portée sœur
        // (`lily/context.cc` : le contexte demandé se crée sous le contexte
        // courant). La sortir en groupe séparé ouvrait une partie de plus et
        // décalait la suite (`slur-scoring.ly`, `slur-broken-trend.ly`) ; la
        // séquence entière reste donc une seule portée, l'interpréteur
        // ouvrant la voix au bon tick (`nested_context`).
        if sequential
            && items.iter().any(has_context)
            && items
                .iter()
                .all(|item| !has_context(item) || is_voice_context(item))
        {
            return vec![anonymous_group(wrap_all(wrappers, combine(items)))];
        }
        // Un `<< { … } \new Staff { … } >>` au milieu d'une séquence est un
        // élément comme un autre : LilyPond avance le curseur de sa durée
        // (`sequential_music`) et la portée du `\new Staff` naît à ce tick
        // (`lily/context-specced-music-iterator.cc:38-99`). La musique
        // environnante reçoit donc un silence de la durée de l'élément, et la
        // portée ouverte un silence de la durée écoulée avant lui
        // (`staff-*-ossia*.ly`).
        let ossia: Vec<bool> = items.iter().map(is_ossia_block).collect();
        let mut decalages: Vec<Fraction> = Vec::with_capacity(items.len());
        let mut ecoule = Fraction::ZERO;
        for (position, item) in items.iter().enumerate() {
            decalages.push(ecoule);
            ecoule += music_duration_with_contexts(item, self.default_duration);
            let _ = position;
        }
        let rest_items: Vec<Music> = items
            .iter()
            .enumerate()
            .filter_map(|(position, item)| {
                if has_context(item) {
                    ossia[position].then(|| {
                        skip_music(music_duration_with_contexts(item, self.default_duration))
                    })
                } else {
                    Some(item.clone())
                }
            })
            .collect();
        let mut rest = if rest_items.is_empty() {
            None
        } else {
            Some(rest_items)
        };
        let mut groups = Vec::new();
        let mut premier = true;
        for (position, item) in items.into_iter().enumerate() {
            let avant = groups.len();
            if has_context(&item) {
                let mut ouverts = self.score_groups_in(item, wrappers);
                if ossia[position] {
                    let decalage = decalages[position];
                    for group in &mut ouverts {
                        for staff in &mut group.staves {
                            for voice in &mut staff.voices {
                                voice.music = Music::Seq(vec![
                                    skip_music(decalage),
                                    std::mem::replace(&mut voice.music, Music::Empty),
                                ]);
                            }
                        }
                        // La portée démarre au début de la séquence puis saute
                        // `decalage` : elle ne se chaîne pas après la musique
                        // environnante.
                        group.sequence_start = false;
                    }
                }
                groups.extend(ouverts);
            } else if let Some(rest) = rest.take() {
                groups.push(anonymous_group(wrap_all(wrappers, combine(rest))));
            }
            // Dans une séquence `{ … }`, la musique d'un élément commence
            // après celle des éléments précédents (`lily/parser.yy`
            // `sequential_music`) : les contextes nommés qui reparaissent
            // continuent dans le temps, au lieu de repartir du tick 0 comme
            // les frères d'un `<< … >>`.
            if sequential && !premier && avant < groups.len() && !ossia[position] {
                // Seul le **premier** groupe de l'élément porte la marque :
                // les suivants (autres portées/voix du même élément) en
                // héritent et ne doivent pas rouvrir un nouvel élément.
                groups[avant].sequence_start = true;
            }
            premier = false;
        }
        groups
    }

    /// Avertit quand une séquence `{ … }` porte plusieurs contextes : leurs
    /// positions respectives se perdent, ils deviennent des portées
    /// parallèles plutôt que rien.
    fn warn_sequential_contexts(&mut self, items: &[Music]) {
        if items.iter().filter(|item| has_context(item)).count() > 1 {
            self.warn("plusieurs contextes dans une séquence `{ … }` : portées successives");
        }
    }

    /// Liste de contextes d'un `<< … >>` de `\score`.
    ///
    /// La liste entière est une musique simultanée : c'est [`Self::score_groups`]
    /// qui décide quels éléments sont des contextes et réunit les autres en
    /// une seule portée anonyme.
    fn parse_context_list(&mut self) -> Vec<GroupBlock> {
        let mut items = Vec::new();
        // `\\` (VoiceSeparator) crée des `Voice` dans la portée courante, pas
        // des contextes frères : `<< { … } \\ { … } >>` au niveau du `\score`
        // est **une** portée à deux voix, alors que `<< { … } { … } >>` (idiome
        // Mutopia) ouvre un contexte par élément (`lily/parser.yy`
        // `simultaneous_music`/`context_prefix`). La distinction se perdrait
        // en gardant `Music::Sim` seul.
        let mut voix = false;
        while !self.at_end() && !self.at_double_angle_close() {
            if self.peek().is_some_and(|token| token.is_command("\\")) {
                self.position += 1;
                voix = true;
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
        if voix {
            return vec![anonymous_group(Music::Sim(items))];
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
    /// Nom de contexte après `=`, chaîne (`"up"`) ou symbole nu (`up`) :
    /// LilyPond accepte les deux (`CHANGE STRING '=' simple_string`,
    /// `lily/parser.yy` ; les sources anciennes écrivent `\new Staff = one`
    /// et `\context Voice = VA`). Ne lire que `Kind::Str` perdait le nom et
    /// cassait la résolution de `\change`/`\context` vers cette portée.
    fn parse_context_id(&mut self) -> Option<String> {
        if !self.eat_punct('=') {
            return None;
        }
        match self.bump() {
            Some(Token {
                kind: Kind::Str | Kind::Word,
                text,
                ..
            }) => Some(text),
            _ => None,
        }
    }

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
                let context_name = self.parse_context_id();
                let mut instrument_name = None;
                let mut short_instrument_name = None;
                let mut midi_instrument = None;
                let mut midi_expression = None;
                let mut midi_pan = None;
                if self.eat_command("with") {
                    for (property, value) in self.parse_with_block() {
                        match property.as_str() {
                            "instrumentName" => instrument_name = Some(value),
                            "shortInstrumentName" => short_instrument_name = Some(value),
                            "midiExpression" => midi_expression = scheme_unit(&value),
                            "midiPanPosition" => midi_pan = scheme_unit(&value),
                            // `Staff_performer::new_instrument_string`
                            // (`lily/staff-performer.cc:244`) : cette
                            // propriété fixe le programme MIDI, pas le nom.
                            "midiInstrument" => midi_instrument = Some(value),
                            _ => {}
                        }
                    }
                }
                let content = self.parse_music();
                let midi_instrument =
                    midi_instrument.or_else(|| self.pending_midi_instrument.take());
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
                let mut staves = self.staff_blocks_from_content(
                    content,
                    context_name,
                    existing,
                    instrument_name.clone(),
                    short_instrument_name.clone(),
                    midi_instrument.clone(),
                    false,
                    false,
                    false,
                );
                for staff in &mut staves {
                    staff.midi_expression = midi_expression;
                    staff.midi_pan = midi_pan;
                }
                return Music::New(Box::new(GroupBlock {
                    kind: GroupKind::Staff,
                    existing,
                    sequence_start: false,
                    instrument_name,
                    short_instrument_name,
                    midi_instrument,
                    staves,
                    lyrics: Vec::new(),
                }));
            }
            let structural = self.peek().is_some_and(|token| {
                token.kind == Kind::Word && GroupKind::from_name(&token.text) != GroupKind::Other
            });
            if !structural {
                // `\context ChordNames` (`context_find_or_create`,
                // `lily/parser.yy` `context_prefix`) est un **contexte**
                // enfant, une piste MIDI à part : dans un `\context Staff`
                // c'est un enfant du `Staff` (`ly/engraver-init.ly` l. 492,
                // `\accepts ChordNames`), pas une voix. Le relire comme un
                // `\new` conserve son nom et sa musique propre.
                // Seuls les contextes sans performer restent des grilles
                // muettes (`lily/note-name-engraver.cc` : NoteNames n'a pas
                // de performer MIDI, contrairement à ChordNames).
                let debut = self.position;
                let nom = self.bump().map(|token| token.text).unwrap_or_default();
                if self.eat_punct('=') {
                    self.bump();
                }
                if self.eat_command("with") {
                    self.parse_with_block();
                }
                if nom == "FiguredBass"
                    || nom == "NoteNames"
                    || nom == "FretBoards"
                    || nom == "NullVoice"
                {
                    return self.skip_nonplaying_grid(&nom);
                }
                self.position = debut;
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
        // Contextes de gravure sans performer MIDI (`lily/note-name-engraver.cc`,
        // `lily/figured-bass-engraver.cc`) : leur musique n'est pas jouée.
        if name == "NoteNames" || name == "FiguredBass" || name == "FretBoards" {
            self.skip_nonplaying_grid(&name);
            return None;
        }
        // `NullVoice` (`ly/performer-init.ly:225-234`, `ly/engraver-init.ly`)
        // n'a pas de Note_performer : ses notes sont invisibles et muettes,
        // mais sa trame porte les paroles alignées par `\lyricsto "nom"`. On
        // garde donc la voix (nommée) dans une portée `muted` : le MIDI la
        // saute, les autres écrivains gardent les paroles.
        if name == "NullVoice" {
            let context_name = self.parse_context_id();
            let music = Music::Context(Context::Invisible {
                music: Box::new(self.parse_music()),
            });
            let mut staff = StaffBlock::new();
            staff.muted = true;
            staff.visible = false;
            staff.name = context_name.clone();
            staff.existing = existing;
            staff.voices.push(VoiceBlock {
                name: context_name,
                music,
            });
            return Some(GroupBlock {
                kind: GroupKind::Staff,
                existing,
                sequence_start: false,
                instrument_name: None,
                short_instrument_name: None,
                midi_instrument: None,
                staves: vec![staff],
                lyrics: Vec::new(),
            });
        }
        // `Devnull` (`ly/engraver-init.ly:1125-1133`) jette silencieusement
        // toute la musique qu'on lui donne : la portée reste comme cible de
        // `\change Staff`, mais ses notes ne sonnent pas.
        if name.eq_ignore_ascii_case("devnull") {
            let context_name = self.parse_context_id();
            let music = Music::Context(Context::Silent {
                music: Box::new(self.parse_music()),
            });
            let mut staff = StaffBlock::new();
            staff.muted = true;
            staff.visible = false;
            staff.name = context_name.clone();
            staff.existing = existing;
            staff.voices.push(VoiceBlock {
                name: context_name,
                music,
            });
            return Some(GroupBlock {
                kind: GroupKind::Staff,
                existing,
                sequence_start: false,
                instrument_name: None,
                short_instrument_name: None,
                midi_instrument: None,
                staves: vec![staff],
                lyrics: Vec::new(),
            });
        }
        let mut kind = GroupKind::from_name(&name);
        // `\new Dynamics` ne porte normalement que des silences (et des
        // indications de nuance) : le jouer comme une portée ordinaire plutôt
        // que comme un contexte inconnu, sinon son contenu resterait dans la
        // portée englobante comme contexte ignoré et décalerait la musique
        // voisine (`dynamics-text-dynamics-context`). Il reste cependant un
        // `Performer_group` sans `Note_performer`
        // (`ly/performer-init.ly:406-411`) : même une note écrite par erreur
        // dans ce bloc (`andre-sonatine.ly` : `s4 b16\< s s8`) n'est pas
        // jouée. Son contenu est donc lu en silence, tout en restant gravé.
        let dynamics = name.eq_ignore_ascii_case("dynamics");
        if dynamics {
            kind = GroupKind::Staff;
        }
        let context_name = self.parse_context_id();

        let mut instrument_name = None;
        let mut short_instrument_name = None;
        let mut midi_instrument = None;
        let mut midi_expression = None;
        let mut midi_pan = None;
        let mut verse = None;
        let mut midi_silent = false;
        let mut merge_unisons = false;
        if self.eat_command("with") {
            let debut_with = self.position;
            // `lily/parser.yy:1625` (`context_modification_arg`) : après
            // `\with`, un identifiant musical (`\RemoveEmptyStaves`,
            // `ly/context-mods-init.ly:52`) est une modification de contexte.
            // Le consommer seul, sinon la musique suivante serait prise pour
            // son argument (`remove-empty-context-mod.ly`).
            let identifiant_mod = self.peek().is_some_and(|token| token.kind == Kind::Command)
                && !self.peek().is_some_and(|token| token.is_command("with"));
            if identifiant_mod {
                self.position += 1;
            }
            let properties = if identifiant_mod {
                // L'identifiant était l'unique argument de `\with` : le bloc
                // qui suit est la musique de la portée, pas un second `\with`.
                Vec::new()
            } else {
                self.parse_with_block()
            };
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
                    "midiInstrument" => midi_instrument = Some(value),
                    "midiExpression" => midi_expression = scheme_unit(&value),
                    "midiPanPosition" => midi_pan = scheme_unit(&value),
                    "stanza" => verse = stanza_verse(&value),
                    "midiMergeUnisons" => {
                        merge_unisons = matches!(value.trim(), "#t" | "##t" | "t" | "true")
                    }
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
            // `\context Lyrics = nom` **sans paroles** est une déclaration de
            // contexte (`lily/parser.yy` `context_prefix contexted_basic_music`
            // imbrique la musique suivante) : au niveau du MIDI, le contexte
            // suivant reste une piste à part (`Abendlied` : `\context Lyrics =
            // SopranX` puis `\context Staff = women`). Ne pas consommer une
            // musique qui n'est pas des paroles ; sinon elle était lue comme
            // des syllabes (ou sautée), puis ses voix réparties en portées
            // fantômes (`midi2ly` : `\context Lyrics=trackA \trackA`, où
            // `\trackA` contient des `\context Voice`).
            let paroles_suivent = self.peek().is_some_and(|token| {
                token.is_punct('{')
                    || token.is_command("lyricmode")
                    || (token.kind == Kind::Command
                        && matches!(self.variables.get(&token.text), Some(Music::Lyrics(_))))
            });
            if !paroles_suivent {
                return None;
            }
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
                sequence_start: false,
                instrument_name: None,
                short_instrument_name: None,
                midi_instrument: None,
                staves: Vec::new(),
                lyrics,
            });
        }

        // Contenu du contexte.
        let mut staff = StaffBlock::new();
        staff.muted = midi_silent;
        staff.merge_unisons = merge_unisons;
        staff.name = context_name.clone();
        staff.existing = existing;
        staff.instrument_name = instrument_name.clone();
        staff.short_instrument_name = short_instrument_name.clone();
        staff.midi_instrument = midi_instrument.clone();
        staff.midi_expression = midi_expression;
        staff.midi_pan = midi_pan;

        // `\makeClusters` (`scm/music-functions.scm::note-to-cluster`)
        // remplace les notes par des `ClusterNoteEvent` que le performer MIDI
        // ne joue pas : portée gravée mais muette. La détection porte sur tout
        // l'intervalle de jetons de la musique de la portée, pas seulement le
        // premier, car la fonction peut s'appliquer à une expression plus
        // tardive (y compris une référence de variable).
        let debut_musique = self.position;

        // `\new Score` est un contexte de niveau Score qui contient des
        // portées (`lily/parser.yy` `context_prefix`, `lily/score.cc`) : son
        // contenu doit être réparti en portées comme celui d'un
        // `StaffGroup`, sinon les `\new Staff` enfants deviennent des voix
        // d'une seule portée (`caesura-style.ily` : quatre `\new Staff`).
        let kind = if name.eq_ignore_ascii_case("score") {
            GroupKind::StaffGroup
        } else {
            kind
        };
        match kind {
            GroupKind::PianoStaff | GroupKind::ChoirStaff | GroupKind::StaffGroup => {
                let mut group = GroupBlock {
                    kind,
                    existing,
                    sequence_start: false,
                    instrument_name,
                    short_instrument_name,
                    midi_instrument: midi_instrument.clone(),
                    staves: Vec::new(),
                    lyrics: Vec::new(),
                };
                let (content, voix) = self.parse_group_content();
                group.midi_instrument =
                    midi_instrument.or_else(|| self.pending_midi_instrument.take());
                let vide = matches!(&content, Music::Sim(items) | Music::Seq(items) if items.is_empty())
                    || matches!(&content, Music::Empty);
                if vide {
                    return None;
                }
                if voix {
                    // Un seul `Staff` implicite du groupe, à plusieurs voix.
                    self.push_anonymous_staff(content, &[], &mut group.staves);
                } else {
                    self.collect_staves(content, &mut group.staves, &mut group.lyrics);
                }
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
                let midi_instrument =
                    midi_instrument.or_else(|| self.pending_midi_instrument.take());
                Some(GroupBlock {
                    // La nature est conservée : l'interpréteur doit distinguer
                    // un `\new Voice` rencontré dans une voix d'une portée
                    // imbriquée (`Music::New`).
                    kind: GroupKind::Voice,
                    existing,
                    sequence_start: false,
                    instrument_name,
                    short_instrument_name,
                    midi_instrument,
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
                let mut content = self.parse_music();
                self.note_mode = mode_precedent;
                let midi_instrument =
                    midi_instrument.or_else(|| self.pending_midi_instrument.take());
                staff.midi_instrument = midi_instrument.clone();
                staff.muted |= self.tokens[debut_musique..self.position]
                    .iter()
                    .any(|token| token.is_command("makeClusters"));
                if dynamics {
                    // `Dynamics` n'a que `Piano_pedal_performer`
                    // (`ly/performer-init.ly:406-411`) : ses notes sont
                    // gravées dans le modèle (`play = false`, écrit `<cue/>`
                    // en MusicXML) mais n'émettent aucune attaque MIDI.
                    content = Music::Context(Context::Silent {
                        music: Box::new(content),
                    });
                    staff.muted = true;
                }
                // Un contexte de **portée** imbriqué (`\context ChordNames`,
                // `\new Dynamics`…) est un contexte enfant de `Staff`, donc
                // une piste MIDI distincte (`ly/engraver-init.ly` l. 492 :
                // `Staff` accepte `ChordNames`). Il doit devenir une portée
                // sœur ; le laisser dans les voix de la portée faisait
                // fusionner ses unissons avec la mélodie à l'oracle
                // (`LullabyBrahms-C` : 17 accords confondus avec la voix).
                // Les `\new Voice` restent des voix de cette portée.
                let mut staves = self.staff_blocks_from_content(
                    content,
                    staff.name.clone(),
                    existing,
                    staff.instrument_name.clone(),
                    staff.short_instrument_name.clone(),
                    staff.midi_instrument.clone(),
                    staff.muted,
                    merge_unisons,
                    kind == GroupKind::Other,
                );
                for built in &mut staves {
                    built.midi_expression = staff.midi_expression;
                    built.midi_pan = staff.midi_pan;
                }
                Some(GroupBlock {
                    kind,
                    existing,
                    sequence_start: false,
                    instrument_name,
                    short_instrument_name,
                    midi_instrument,
                    staves,
                    lyrics: Vec::new(),
                })
            }
        }
    }

    /// Portées d'un contexte de type `Staff` : une seule portée avec ses voix,
    /// ou plusieurs si le contenu ouvre des contextes enfants de portée
    /// (`\context ChordNames`, `\new Dynamics`…), qui sont des pistes MIDI
    /// distinctes (`ly/engraver-init.ly` l. 492, `\accepts ChordNames`).
    #[allow(clippy::too_many_arguments)] // propriétés du contexte Staff à propager
    fn staff_blocks_from_content(
        &mut self,
        content: Music,
        context_name: Option<String>,
        existing: bool,
        instrument_name: Option<String>,
        short_instrument_name: Option<String>,
        midi_instrument: Option<String>,
        muted: bool,
        merge_unisons: bool,
        group_like: bool,
    ) -> Vec<StaffBlock> {
        // Un `\set Staff.midiInstrument` lu dans la musique de la portée est
        // une propriété de contexte, pas une musique : il ne doit pas ouvrir
        // de portée surnuméraire.
        let midi_instrument = midi_instrument.or_else(|| self.pending_midi_instrument.take());
        let addlyrics_target = last_voice_name(&content);
        if has_staff_context(&content, group_like, context_name.is_none()) {
            let mut staves = Vec::new();
            let mut lyrics = Vec::new();
            self.collect_staves(content, &mut staves, &mut lyrics);
            if let Some(premiere) = staves.first_mut() {
                if premiere.name.is_none() {
                    premiere.existing = existing;
                }
                premiere.name = premiere.name.clone().or(context_name);
                premiere.instrument_name = premiere.instrument_name.clone().or(instrument_name);
                premiere.short_instrument_name = premiere
                    .short_instrument_name
                    .clone()
                    .or(short_instrument_name);
                premiere.midi_instrument = premiere.midi_instrument.clone().or(midi_instrument);
                premiere.muted |= muted;
                premiere.merge_unisons |= merge_unisons;
                premiere.lyrics.splice(0..0, lyrics);
                self.collect_addlyrics_target(premiere, addlyrics_target);
            }
            return staves;
        }
        // Une seule musique de portée : les voix sont ouvertes par
        // l'interpréteur au tick courant, pas relues chacune depuis zéro.
        let mut staff = StaffBlock::new();
        staff.name = context_name;
        staff.existing = existing;
        staff.instrument_name = instrument_name;
        staff.short_instrument_name = short_instrument_name;
        staff.midi_instrument = midi_instrument;
        staff.muted = muted;
        staff.merge_unisons = merge_unisons;
        collect_lyrics_in_music(&content, &mut staff.lyrics);
        staff.voices.push(VoiceBlock {
            name: None,
            music: content,
        });
        self.collect_addlyrics_target(&mut staff, addlyrics_target);
        vec![staff]
    }

    /// Répartit le contenu d'un groupe en portées.
    fn collect_staves(
        &mut self,
        music: Music,
        staves: &mut Vec<StaffBlock>,
        lyrics: &mut Vec<LyricsBlock>,
    ) {
        self.collect_staves_in(music, &mut Vec::new(), staves, lyrics, true);
    }

    /// Contenu d'un groupe de portées (`\new StaffGroup`, `PianoStaff`,
    /// `ChoirStaff`, `GrandStaff`, `OneStaff`, `\new Score`).
    ///
    /// Renvoie la musique et le drapeau « `\\` vu » : `<< { c } { e } >>`
    /// joue deux portées, une par élément (le groupe accepte des `Staff`,
    /// `ly/engraver-init.ly`), alors que `<< { c } \\ { e } >>`
    /// (VoiceSeparator) crée deux `Voice` dans une seule portée. Vérifié sur
    /// LilyPond 2.27 : deux pistes MIDI dans le premier cas, une dans le
    /// second. Le jeton `\\` est consommé par `parse_music`, d'où cette
    /// lecture au seul endroit où les jetons bruts du groupe sont encore là.
    fn parse_group_content(&mut self) -> (Music, bool) {
        if !self.at_double_angle_open() {
            return (self.parse_music(), false);
        }
        self.position += 2;
        let mut items = Vec::new();
        let mut voix = false;
        while !self.at_end() && !self.at_double_angle_close() {
            if self.peek().is_some_and(|token| token.is_command("\\")) {
                self.position += 1;
                voix = true;
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
        (Music::Sim(items), voix)
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
        // Vrai dans une séquence : les `\new Voice` y continuent la portée en
        // cours. Faux entre éléments simultanés (`<< … >>`) : un `\new
        // TabVoice` y est une portée à part (`bend-spanner-simple`).
        merge_voice: bool,
    ) {
        match music {
            Music::Context(context) => match Wrapper::of(&context) {
                Some((wrapper, inner)) => {
                    let inner = inner.clone();
                    wrappers.push(wrapper);
                    self.collect_staves_in(inner, wrappers, staves, lyrics, merge_voice);
                    wrappers.pop();
                }
                None => self.push_anonymous_staff(Music::Context(context), wrappers, staves),
            },
            Music::Sim(items) => {
                // Dans un groupe de portées, chaque élément simultané sans
                // contexte a **sa** portée : `\new StaffGroup << { c } { e } >>`
                // joue deux staves (deux pistes MIDI vérifiées sur 2.27).
                // (`<< { c } \\ { e } >>` est traité plus haut, dans
                // `parse_group_content`, car le jeton `\\` y est encore
                // présent.)
                if !items.iter().any(has_context) {
                    for item in items {
                        self.push_anonymous_staff(item, wrappers, staves);
                    }
                    return;
                }
                self.context_staves(items, wrappers, staves, lyrics, false, false);
            }
            Music::Seq(items) => {
                // Un `{ … }` contenant un contexte décrit lui aussi la
                // structure : le contexte est créé à la position courante.
                if !items.iter().any(has_context) {
                    self.push_anonymous_staff(Music::Seq(items), wrappers, staves);
                    return;
                }
                self.warn_sequential_contexts(&items);
                self.context_staves(items, wrappers, staves, lyrics, true, true);
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
                if merge_voice && group.kind == GroupKind::Voice && group.staves.len() == 1 {
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
        sequential: bool,
        merge_voice: bool,
    ) {
        // Chaque élément sans contexte du groupe est une portée à lui seul :
        // `\new StaffGroup << \mus \new Dynamics { … } \mus >>` joue deux
        // portées de notes (deux pistes MIDI) plus la portée Dynamics.
        let mut premier = true;
        for item in items {
            let avant = staves.len();
            if has_context(&item) {
                self.collect_staves_in(item, wrappers, staves, lyrics, merge_voice);
            } else {
                self.push_anonymous_staff(item, wrappers, staves);
            }
            // Dans une séquence, l'élément suivant commence après le
            // précédent (`lily/parser.yy` `sequential_music`) : la première
            // portée qu'il crée porte la marque, comme les groupes de
            // `\score` (`context_groups`).
            if sequential && !premier && avant < staves.len() {
                staves[avant].sequence_start = true;
                // Les autres portées du même élément (un `<< … >>` qui
                // contient un `\new Staff`) sont créées au même instant, pas
                // après la première : elles héritent du début de l'élément
                // (`lily/context-specced-music-iterator.cc:38-99`).
                for staff in &mut staves[avant + 1..] {
                    staff.sequence_continue = true;
                }
            }
            premier = false;
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
        // `\new Lyrics \lyricmode { … }` : le mode explicite précède le bloc.
        self.eat_command("lyricmode");
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
            let mut voix = false;
            while !self.at_end() && !self.at_double_angle_close() {
                if self.peek().is_some_and(|token| token.is_command("\\")) {
                    self.position += 1;
                    voix = true;
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
            if voix {
                return Music::SimVoices(items);
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
                    } else if self.peek().is_some_and(|token| token.is_punct('~')) {
                        // Élément vide (variable sans musique, `\testVshape`)
                        // entre un accord et sa liaison : le `~` porte sur la
                        // musique précédente (`c1\testVshape~ c`, `vshape.ly`).
                        // Sans cela il répétait la note. On ne rattache que la
                        // liaison : les autres post-événements d'un bloc lu
                        // comme musique (`\markup { 8 … }` de `ottava.ly`)
                        // doivent garder leur lecture.
                        let posts = self.parse_posts();
                        if let Some(Music::Chord(chord)) = items.last_mut() {
                            chord.posts.extend(posts);
                        }
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
            // `$nom`/`#nom` insèrent une variable définie par
            // `#(define nom #{ … #})` (Extending §1) ou par une assignation
            // LilyPond ordinaire (`music = \relative { … }`) : dans un corps
            // `#{ … #}`, `embedded_scm` est un élément musical
            // (`lily/parser.yy:3221-3225`) et `#music` y désigne cette
            // variable Scheme. Les autres datum sont ignorés sans bruit.
            Kind::Scheme => {
                let datum = token.text.clone();
                self.position += 1;
                // `#(ly:parser-set-note-names …)` vaut aussi au milieu d'une
                // musique (`note-names.ly`), pas seulement au niveau
                // supérieur.
                if let Some(langage) = scheme_note_names_language(&datum) {
                    self.language = langage;
                }
                if let Some(music) = self.scheme_variables.get(&datum).cloned() {
                    music
                } else if let Some(music) = self.variables.get(&datum).cloned() {
                    music
                } else if datum.contains("music-map") {
                    // `music-map` transforme des événements (nuances,
                    // articulations) mais garde hauteurs et attaques
                    // (`music-map-articulations.ly`).
                    crate::lexer::embedded_music_body(&datum)
                        .map(|corps| self.parse_embedded_music(&corps))
                        .unwrap_or(Music::Empty)
                } else if datum.trim_start().starts_with("(descend-to-context")
                    || datum.trim_start().starts_with("(ascend-to-context")
                {
                    // `descend-to-context`/`ascend-to-context`
                    // (`scm/music-functions.scm:710-721`) posent une
                    // recherche de contexte sur leur premier argument
                    // musical. Quand le contexte n'existe pas, LilyPond
                    // joue cette musique dans le contexte courant
                    // (`context-descend-only-not-found.ly`) ; relire ce
                    // premier `#{ … #}` suffit, sans évaluer le Scheme.
                    crate::lexer::embedded_music_body(&datum)
                        .map(|corps| self.parse_embedded_music(&corps))
                        .unwrap_or(Music::Empty)
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
        // `lily-library.scm` `collect-music-aux` (via `toplevel-music-handler`,
        // `ly/init.ly:66-86`) engraves a variable whose value is a `\score`,
        // just like an explicit score.  `lily/book.cc:125-127` adds every
        // `\score` of a `\book` to the book.  Reading such a variable with
        // `parse_music` sees `\score`/`\book` as unknown commands: the score
        // body is then swallowed by the next construct (a `\magnifyMusic`
        // wrapper, a Scheme markup variable, a tagged stanza) and the reader
        // emits an empty score.  Parse the score expression here instead.
        // The partition is registered at the definition: the reader keeps a
        // single score and LilyPond engraves the variable when it is used.
        if self.peek().is_some_and(|token| token.is_command("score")) {
            let token = self.peek().cloned().expect("score token");
            self.position += 1;
            let (score, has_layout, has_midi, unfold_repeats, articulate) = self.parse_score();
            self.partitions.push(Partition {
                score,
                explicit: true,
                has_layout,
                has_midi,
                unfold_repeats,
                articulate,
                line: token.line,
                column: token.column,
            });
            self.variables.insert(name, Music::Empty);
            return;
        }
        if self
            .peek()
            .is_some_and(|token| token.is_command("book") || token.is_command("bookpart"))
        {
            self.position += 1;
            let mut document = Document::default();
            self.parse_book(&mut document);
            self.variables.insert(name, Music::Empty);
            return;
        }
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
            if let Ok(valeur) = text.trim().parse::<i64>() {
                self.position += 1;
                self.variables.remove(&name);
                self.numbers.insert(name, valeur);
                return;
            }
            if let Some((duree, facteur)) = scheme_duration_definition(&text) {
                self.position += 1;
                self.variables.remove(&name);
                self.duration_identifiers.insert(name, (duree, facteur));
                return;
            }
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
        // Une enveloppe (`\fixed c'`, `\relative …`) peut séparer les noms
        // de voix du bloc (`parallelmusic-partial.ly`) : elle s'applique à
        // chacune des voix produites.
        let mut wrappers = Vec::new();
        loop {
            let wrapper = match self.peek().cloned() {
                Some(Token {
                    kind: Kind::Command,
                    text,
                    ..
                }) if text == "fixed" || text == "relative" => {
                    self.position += 1;
                    let reference = self.parse_optional_pitch();
                    if text == "fixed" {
                        Wrapper::Fixed(reference)
                    } else {
                        Wrapper::Relative(reference, false)
                    }
                }
                Some(Token {
                    kind: Kind::Command,
                    text,
                    ..
                }) if text == "absolute" => {
                    self.position += 1;
                    Wrapper::Relative(None, false)
                }
                _ => break,
            };
            wrappers.push(wrapper);
        }
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
            let music = wrap_all(&wrappers, music);
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
            self.skip_function_arguments(&function);
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
        let saved_invisible_ottava = self.pending_invisible_ottava;
        let saved_tuplet_number = self.pending_tuplet_number;
        let arguments = self.read_function_arguments(&function);
        self.default_duration = saved_duration;
        self.default_multiplier = saved_multiplier;
        self.pending_head = saved_head;
        self.pending_invisible_rest = saved_invisible_rest;
        self.pending_invisible_ottava = saved_invisible_ottava;
        self.pending_tuplet_number = saved_tuplet_number;
        let Some(body) = function.body else {
            if let Some(index) = function.returns {
                // Le corps est un paramètre : la fonction rend cet argument,
                // relu comme musique (`function_arglist_reparse`).
                let tokens = arguments.get(index).cloned().unwrap_or_default();
                return self.parse_token_stream(tokens);
            }
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
        let defaults = function.optional_defaults.clone();
        let mut arguments = Vec::with_capacity(predicates.len());
        for (index, predicate) in predicates.iter().enumerate() {
            let predicate = predicate_type(predicate);
            if let Some(Some(default)) = defaults.get(index) {
                if !self.argument_present(predicate) {
                    arguments.push(default.clone());
                    continue;
                }
            }
            let start = self.position;
            self.read_function_argument(predicate);
            let end = self.position.min(self.tokens.len());
            arguments.push(self.tokens.get(start..end).unwrap_or_default().to_vec());
        }
        arguments
    }

    /// Whether the next token can satisfy an optional argument's predicate.
    ///
    /// An optional argument must not swallow the following required argument
    /// (or the next function call): LilyPond only consumes it when the next
    /// token is a music expression (or a value of the requested type).
    fn argument_present(&self, predicate: &str) -> bool {
        let Some(token) = self.peek() else {
            return false;
        };
        if predicate.ends_with("music?") || predicate.ends_with("music-list?") {
            if token.is_punct('{') || token.is_punct('<') || self.at_double_angle_open() {
                return true;
            }
            if token.kind == Kind::Word {
                return self.language.parse(&token.text).is_some()
                    || matches!(token.text.as_str(), "r" | "R" | "s" | "q");
            }
            if token.kind == Kind::Command {
                return matches!(
                    token.text.as_str(),
                    "relative"
                        | "fixed"
                        | "absolute"
                        | "transpose"
                        | "new"
                        | "context"
                        | "grace"
                        | "acciaccatura"
                        | "appoggiatura"
                        | "afterGrace"
                        | "tuplet"
                        | "times"
                        | "repeat"
                        | "unfoldRepeats"
                        | "keepWithTag"
                        | "removeWithTag"
                        | "tag"
                        | "once"
                        | "override"
                        | "set"
                        | "clef"
                        | "key"
                        | "time"
                        | "bar"
                        | "skip"
                        | "chordmode"
                        | "drummode"
                );
            }
            return false;
        }
        if predicate.ends_with("pitch?") {
            return token.kind == Kind::Word && self.language.parse(&token.text).is_some();
        }
        if predicate.ends_with("duration?") {
            return token.kind == Kind::Number
                || (token.kind == Kind::Word
                    && matches!(token.text.as_str(), "breve" | "longa" | "maxima"));
        }
        if predicate.ends_with("number?")
            || predicate.ends_with("integer?")
            || predicate == "index?"
        {
            return token.kind == Kind::Number || token.kind == Kind::Scheme || token.is_punct('-');
        }
        if predicate.contains("context-mod") {
            return token.is_command("with") || token.kind == Kind::Scheme;
        }
        if predicate == "markup?" || predicate == "ly:markup?" {
            return token.is_command("markup") || token.kind == Kind::Str;
        }
        matches!(token.kind, Kind::Word | Kind::Str | Kind::Scheme)
    }

    /// Reads a `\addQuote` definition whose name was used before its
    /// declaration (or whose declaration follows the current position).
    ///
    /// `ly/music-functions-init.ly:56-60` stores quoted music as an
    /// identifier; `Quote_iterator` looks it up when the quote is played, not
    /// when it is written. The token range of the definition is re-parsed with
    /// the current parser state, then cached like an ordinary quote. Mutual
    /// references are cut by `resolving_quotes`.
    fn resolve_quote(&mut self, name: &str) {
        if name.is_empty() || self.resolving_quotes.contains(name) {
            return;
        }
        let Some((start, end)) = self.quote_range(name) else {
            return;
        };
        let tokens = self.tokens[start..end].to_vec();
        let saved_tokens = std::mem::replace(&mut self.tokens, tokens);
        let saved_position = self.position;
        let saved_duration = self.default_duration;
        let saved_multiplier = self.default_multiplier;
        let saved_language = self.language;
        self.position = 0;
        self.resolving_quotes.insert(name.to_owned());
        let music = self.parse_music();
        self.resolving_quotes.remove(name);
        self.tokens = saved_tokens;
        self.position = saved_position;
        self.default_duration = saved_duration;
        self.default_multiplier = saved_multiplier;
        self.language = saved_language;
        self.quotes.insert(name.to_owned(), music);
    }

    /// Token range of the music in `\addQuote <name> <music>`.
    fn quote_range(&self, name: &str) -> Option<(usize, usize)> {
        for index in 0..self.tokens.len().saturating_sub(1) {
            let named = matches!(
                self.tokens.get(index + 1).map(|token| &token.kind),
                Some(Kind::Str | Kind::Word)
            ) && self.tokens[index + 1].text == name;
            if !self.tokens[index].is_command("addQuote") || !named {
                continue;
            }
            let start = index + 2;
            let mut depth = 0i32;
            let mut end = start;
            while end < self.tokens.len() {
                let token = &self.tokens[end];
                let boundary = end > start
                    && depth == 0
                    && (token.is_punct('<')
                        || (token.kind == Kind::Command
                            && matches!(
                                token.text.as_str(),
                                "addQuote"
                                    | "score"
                                    | "book"
                                    | "bookpart"
                                    | "language"
                                    | "header"
                                    | "paper"
                            )));
                if boundary {
                    break;
                }
                if token.is_punct('{') || token.is_punct('<') || token.is_punct('[') {
                    depth += 1;
                } else if token.is_punct('}') || token.is_punct('>') || token.is_punct(']') {
                    depth -= 1;
                }
                end += 1;
            }
            return Some((start, end));
        }
        None
    }

    /// Consomme la hauteur secondaire d'un `\pitchedTrill`.
    ///
    /// C'est un `ly:music?` d'un seul événement : une hauteur, ses marques
    /// d'octave et une durée éventuelle. La laisser dans le flux la ferait
    /// jouer comme une note ordinaire (`trill-spanner-pitched.ly`).
    fn saute_pitch_secondaire(&mut self) {
        let est_hauteur = self.peek().is_some_and(|token| {
            token.kind == Kind::Word && self.language.parse(&token.text).is_some()
        });
        if !est_hauteur {
            return;
        }
        self.position += 1;
        while self.peek().is_some_and(|token| {
            token.is_punct('\'')
                || token.is_punct(',')
                || token.is_punct('!')
                || token.is_punct('?')
        }) {
            self.position += 1;
        }
        if self.peek().is_some_and(|token| token.kind == Kind::Number) {
            self.position += 1;
        }
        while self.eat_punct('.') {}
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
        // `ly:context-mod?` : `\with { … }` (ou un contexte Scheme). La
        // commande `\with` est consommée et le bloc seul devient l'argument,
        // pour que le corps puisse la placer derrière son propre `\with`
        // (`\new Voice \with #style { … }`, `note-head-style.ly`).
        if predicate.contains("context-mod") {
            if self.peek().is_some_and(|token| token.is_command("with")) {
                self.position += 1;
                if self.peek().is_some_and(|token| token.is_punct('{')) {
                    self.skip_braced();
                } else {
                    self.bump();
                }
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
    fn skip_function_arguments(&mut self, function: &MusicFunction) {
        let predicates = function.predicates.clone();
        let defaults = function.optional_defaults.clone();
        for (index, predicate) in predicates.iter().enumerate() {
            let predicate = predicate_type(predicate);
            if let Some(Some(_)) = defaults.get(index) {
                if !self.argument_present(predicate) {
                    continue;
                }
            }
            if predicate.ends_with("music?") || predicate.ends_with("music-list?") {
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

    /// Nom de variable pointé commençant à `position` (`violin.1`) et nombre
    /// de jetons supplémentaires consommés après le premier.
    fn nom_pointe(&self, premier: &str, depart: usize) -> (String, usize) {
        let mut nom = premier.to_owned();
        let mut extra = 0usize;
        while self
            .peek_at(depart + extra)
            .is_some_and(|token| token.is_punct('.'))
            && self
                .peek_at(depart + 1 + extra)
                .is_some_and(|token| matches!(token.kind, Kind::Word | Kind::Number))
        {
            nom.push('.');
            nom.push_str(&self.peek_at(depart + 1 + extra).unwrap().text);
            extra += 2;
        }
        (nom, extra)
    }

    /// Vrai si le jeton courant est à l'intérieur d'un bloc `\grace { … }`.
    fn dans_contexte_grace(&self) -> bool {
        let mut profondeur = 0i32;
        let mut grace_depuis = None;
        for token in self.tokens.iter().take(self.position) {
            if token.kind == Kind::Punct('{') {
                profondeur += 1;
                if grace_depuis.is_none() && token.text.is_empty() {
                    // marqué à l'ouverture suivant le `\grace`
                }
            } else if token.kind == Kind::Punct('}') {
                profondeur -= 1;
                if let Some(d) = grace_depuis {
                    if profondeur < d {
                        grace_depuis = None;
                    }
                }
            }
            if token.is_command("grace") || token.is_command("slashedGrace") {
                // Le `{` d'ouverture est le jeton suivant.
                grace_depuis = Some(profondeur + 1);
            }
        }
        grace_depuis.is_some_and(|d| profondeur >= d)
    }

    /// Consomme une structure de temps en tête de `\time`
    /// (`\time 1,2,3,1 4/4`, `ly/property-init.ly`) : une suite de nombres
    /// séparés par des virgules, uniquement si elle est suivie d'une armure.
    fn consume_beat_structure(&mut self) {
        let debut = self.position;
        let mut vu = false;
        while self.peek().is_some_and(|token| {
            (token.kind == Kind::Number && !token.text.contains('/')) || token.is_punct(',')
        }) {
            vu = true;
            self.position += 1;
        }
        if !vu || !self.peek().is_some_and(|token| token.kind == Kind::Number) {
            self.position = debut;
        }
    }

    /// Commande musicale.
    /// `\addInstrumentDefinition "nom" #\`(…)` : mémorise la transposition
    /// de l'instrument (`ly/music-functions-init.ly:46-54`). Appelable au
    /// niveau supérieur (forme courante) comme dans une musique.
    fn add_instrument_definition(&mut self) {
        self.position += 1; // `\addInstrumentDefinition`
        let nom = match self.bump() {
            Some(Token {
                kind: Kind::Str | Kind::Word,
                text,
                ..
            }) => text,
            _ => String::new(),
        };
        let datum = match self.bump() {
            Some(Token {
                kind: Kind::Scheme,
                text,
                ..
            }) => text,
            _ => String::new(),
        };
        if let Some(pitch) = instrument_transposition_datum(&datum) {
            self.instrument_definitions.insert(nom, pitch);
        }
    }

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
        // `\breve`, `\longa` et `\maxima` employés seuls sont des durées
        // nues : ils répètent la note ou l'accord précédent, comme le ferait
        // un chiffre (`lily/parser.yy` `event_chord: duration`,
        // `scm/music-functions.scm::expand-repeat-notes!`). Le lecteur les
        // ignorait, perdant les reprises `\longa \breve 1 2 4 8 8` de
        // `monochords.ly` (7 attaques par groupe dans la référence).
        if matches!(name.as_str(), "breve" | "longa" | "maxima") {
            return self.parse_bare_duration();
        }
        self.position += 1;
        // Une variable musicale masque les commandes intégrées, comme dans
        // LilyPond (`identifiant = { … }` puis `\identifiant`).
        let simple = name
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .unwrap_or(&name);
        let (lookup, extra) = self.nom_pointe(simple, 0);
        if let Some(music) = self.variables.get(&lookup).cloned() {
            self.position += extra;
            // Une valeur qui n'est pas de la musique utilisable (commande
            // inconnue) ne doit pas disparaître en silence au point d'appel ;
            // une musique vide est normale (`m = { }`).
            if matches!(music, Music::Unknown(_)) {
                self.warn(format!("variable \\{name} sans musique utilisable ignorée"));
            }
            // `\repeat … \etc` : la musique qui suit l'appel de la variable
            // est le corps manquant de la fonction partielle.
            let music = match music {
                Music::Context(Context::Repeat {
                    kind,
                    count,
                    alternatives,
                    partial: true,
                    count_missing,
                    ..
                }) => {
                    let count = if count_missing {
                        self.parse_repeat_count()
                    } else {
                        count
                    };
                    Music::Context(Context::Repeat {
                        kind,
                        count,
                        body: Box::new(self.parse_music()),
                        alternatives,
                        partial: false,
                        count_missing: false,
                    })
                }
                other => other,
            };
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
                let music = self.parse_music();
                // Un `\\relative` appliqué à un `<< … >>` est distribué à ses
                // branches par `score_groups_in` : leurs hauteurs s'enchaînent
                // (`music_list_to_relative`). Un `\\relative` propre à une
                // branche ré-ancre au contraire sa propre conversion.
                let chain = matches!(&music, Music::Sim(_));
                Music::Context(Context::Relative {
                    reference,
                    chain,
                    music: Box::new(music),
                })
            }
            "absolute" => {
                // ly/music-functions-init.ly:32-36 : \absolute enveloppe sa
                // musique dans un RelativeOctaveMusic, la soustrayant au
                // contexte relatif environnant. Les marques d'octave restent
                // littérales et la référence de la suite est restaurée.
                Music::Context(Context::NonRelative {
                    music: Box::new(self.parse_music()),
                })
            }
            // `\tag` marque la musique suivante ; `\keepWithTag` et
            // `\removeWithTag` filtrent récursivement les expressions ainsi
            // marquées (Notation Reference §22.2.2, `ly/music-functions-init.ly`).
            "tag" => {
                let tags = self.parse_tag_names();
                if self.peek().is_some_and(starts_post_event) {
                    // `\tag #'part ^4` : l'expression marquée est un
                    // post-événement de la note précédente ; il ne produit
                    // aucune musique (`tag-filter.ly`).
                    let _ = self.parse_post_event();
                    Music::Empty
                } else {
                    Music::Tag {
                        tags,
                        music: Box::new(self.parse_music()),
                    }
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
            // `\unfoldRepeats` est une fonction musicale : elle transforme
            // **son argument** (`ly/music-functions-init.ly:2616-2625`,
            // `scm/music-functions.scm:408-438`), pas toute la musique qui
            // précède dans la même expression. Le drapeau de score reste
            // posé pour l'export MIDI ; le dépliage effectif est porté par le
            // contexte (`Context::UnfoldRepeats`).
            "unfoldRepeats" => {
                self.playback_unfold_repeats = true;
                let music = self.parse_music();
                Music::Context(Context::UnfoldRepeats {
                    music: Box::new(music),
                })
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
                let debut = self.position;
                let first = non_relative(self.parse_music());
                let second = non_relative(self.parse_music());
                // Les grâces et les durées mises à l'échelle changent le
                // moment utilisé par le combiner (`part-combiner.scm`) :
                // notre règle simple ne s'applique pas à ces contextes
                // (`part-combine-inside-grace`, `-inside-scale-durations`).
                let hors_portee = self.dans_contexte_grace()
                    || self.tokens[debut..self.position.min(self.tokens.len())]
                        .iter()
                        .any(|token| {
                            token.is_command("grace")
                                || token.is_command("scaleDurations")
                                || token.is_command("times")
                                || token.is_command("tuplet")
                        });
                if first.is_empty() {
                    second
                } else if second.is_empty() || second == first {
                    // Deux voix strictement identiques sont des unissons :
                    // le part combiner ne les joue qu'une fois.
                    first
                } else if hors_portee {
                    Music::Sim(vec![first, second])
                } else {
                    Music::Context(Context::PartCombine {
                        first: Box::new(first),
                        second: Box::new(second),
                    })
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
                // `ac:unfoldMusic` (`ly/articulate.ly:437-487`, appelé par
                // `ac:startup-replacements` :855-860) déplie complètement les
                // reprises de son argument avant d'articuler les notes.
                let music = self.parse_music();
                Music::Context(Context::UnfoldRepeats {
                    music: Box::new(music),
                })
            }
            // `\retrograde` (`ly/music-functions-init.ly:2138-2143`) renvoie
            // sa musique dans l'ordre inverse (`retrograde-music`,
            // `scm/modal-transforms.scm:183-290`). La conversion relative a
            // lieu avant le renversement : la transformation se fait sur les
            // événements interprétés, pas sur l'arbre (`events.rs`).
            "retrograde" => Music::Context(Context::Retrograde {
                music: Box::new(self.parse_music()),
            }),
            // `\offset` (`ly/music-functions-init.ly:1283-1317`) déplace une
            // propriété de grob : `\offset propriété valeurs item`. Un item
            // qui est une liste de clés (`BreathingSign`, `Voice.Beam`,
            // `Slur`) produit un override ; sinon c'est la musique qui suit
            // (elle reçoit un tweak). Sans cette lecture, la propriété et les
            // valeurs restaient dans le flux et devenaient des notes ou des
            // durées (`offsets.ly`).
            // `\approximatePitch` (`ly/music-functions-init.ly:239-246`)
            // pose `pitch-approximate` sur la note et la renvoie : transparent
            // pour le modèle. Ignorée, la commande décalait les notes qui
            // suivent (`approximate-pitch-head-direction-tweak.ly`).
            "approximatePitch" => self.parse_music(),
            // `\shape` et `\vshape` (`ly/music-functions-init.ly:2191-2230`
            // et `:2675-2681`) décalent les points de contrôle d'un grob :
            // une liste Scheme de décalages, puis une liste de clés ou la
            // musique qui suit. Sans effet MIDI ; laissés dans le flux, la
            // liste et l'item produisaient des notes en trop (`vshape.ly`).
            "shape" | "vshape" => {
                self.skip_scheme_value();
                if self.eat_command("etc") {
                    // `\vshape #'(…) \etc` : fonction partielle, l'item
                    // viendra à l'appel (`vshape.ly`).
                    Music::Empty
                } else if self.at_offset_key_list() {
                    let _ = self.skip_property_path();
                    Music::Empty
                } else {
                    self.parse_music()
                }
            }
            "offset" => {
                self.parse_offset_arguments();
                if self.at_offset_key_list() {
                    let _ = self.skip_property_path();
                    Music::Empty
                } else {
                    self.parse_music()
                }
            }
            // `transposition` (`ly/music-functions-init.ly`) est une fonction
            // musicale à un argument `ly:pitch?` qui pose la propriété
            // `instrumentTransposition` du contexte Staff. Elle n'ajoute aucune
            // musique, mais décale la hauteur concert : la hauteur écrite reste
            // celle de la source et la transposition est portée par
            // `Instrument.transpose`. Sans argument, LilyPond prend `c'`, qui
            // est aussi la valeur par défaut. La consommer comme une commande
            // inconnue laissait l'argument devenir une note parasite.
            // `ly/music-functions-init.ly:46-54` : enregistre une aliste
            // d'instrument. On n'en extrait que `instrumentTransposition`,
            // seule propriété audible, afin que `\instrumentSwitch` puisse
            // poser la même transposition que `\transposition` (`:868-890`).
            "addInstrumentDefinition" => {
                self.add_instrument_definition();
                Music::Empty
            }
            // `\instrumentSwitch` (`ly/music-functions-init.ly:868-890`) pose
            // `instrumentTransposition` à partir de ce point. Voir
            // `Context::InstrumentSwitch` (`ast.rs`) : la transposition est
            // portée par les hauteurs écrites suivantes, et non par la
            // propriété d'instrument, parce que le changement arrive en cours
            // de mesure dans `instrument-switch.ly` et que l'écrivain MusicXML
            // ne pose `<transpose>` qu'au début d'une mesure.
            "instrumentSwitch" | "switchInstrument" => {
                let nom = match self.bump() {
                    Some(Token {
                        kind: Kind::Str | Kind::Word,
                        text,
                        ..
                    }) => text,
                    _ => String::new(),
                };
                self.instrument_definitions
                    .get(&nom)
                    .copied()
                    .map(|pitch| Music::Context(Context::InstrumentSwitch(pitch)))
                    .unwrap_or(Music::Empty)
            }
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
            // `ly/music-functions-init.ly:779-797` : `\harmonicByFret` transmet
            // sa musique à `calc-harmonic-pitch`, qui *ajoute* à chaque hauteur
            // l'intervalle du partiel correspondant au frettage
            // (`scm/tablature.scm:297-320,342-366`). Par exemple `#5` donne le
            // partiel 3, `(2 0 0)`, soit deux octaves : `g,4.` joue G5 et non
            // G3 (`faure-sicilienne-guitare-doigtee.ly`, coda). L'intervalle
            // s'ajoute aux hauteurs **déjà résolues**, sans décaler la
            // référence d'un `\relative` environnant : un `Context::Harmonic`
            // dédié, et non `Context::Transpose` (qui modifierait cette
            // référence, comme le fait un `\transpose` littéral).
            "harmonicByFret" => {
                let fret = self
                    .peek()
                    .filter(|token| token.is_scheme())
                    .map(|token| token.text.trim().trim_start_matches('#').to_owned());
                if fret.is_some() {
                    self.position += 1;
                }
                let (octave, step, alter) = harmonic_fret_pitch(fret.as_deref().unwrap_or("0"));
                let semitones = [0, 2, 4, 5, 7, 9, 11][usize::from(step)]
                    + i32::from(alter)
                    + 12 * i32::from(octave);
                Music::Context(Context::Harmonic {
                    semitones: i8::try_from(semitones).unwrap_or(0),
                    music: Box::new(self.parse_music()),
                })
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
            // `\compressMusic` est l'ancien nom de `\scaleDurations`
            // (`python/convertrules.py` l. 2765, règle 2.11.48) : même argument
            // `#'(n . d)` et mêmes durées multipliées par n/d.
            "scaleDurations" | "compressMusic" => {
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
            "after" => {
                // `\after delta ev mus` (`ly/music-functions-init.ly:67-83`) :
                // `\context Bottom << { \skip delta <> ev } #mus >>`. `mus`
                // avance le curseur ; `ev` (souvent un post-événement) est
                // joué à part, `delta` plus loin. Ignoré, le délai devenait
                // une durée qui décalait la musique (`after.ly`).
                let saved_duration = self.default_duration;
                let saved_multiplier = self.default_multiplier;
                let delta = self
                    .parse_duration()
                    .map(|(duration, multiplier)| {
                        duration.ticks() * multiplier.unwrap_or(Fraction::ONE)
                    })
                    .unwrap_or(Fraction::ZERO);
                self.default_duration = saved_duration;
                self.default_multiplier = saved_multiplier;
                // Un `\tweak` porte sur la musique qui suit : c'est un
                // événement (tweak + note), pas un simple post-événement.
                let mut tweaked = false;
                while self.eat_command("tweak") {
                    // `parse_tweak` consomme propriété et valeur (avec ou sans
                    // `=`) : la musique suivante est la note à décorer.
                    self.parse_tweak();
                    tweaked = true;
                }
                let event = if tweaked {
                    self.parse_music()
                } else if self.peek().is_some_and(starts_post_event) {
                    let _ = self.parse_posts();
                    Music::Empty
                } else {
                    self.parse_music()
                };
                let music = self.parse_music();
                Music::Context(Context::After {
                    delta,
                    event: Box::new(event),
                    music: Box::new(music),
                })
            }
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
            // `\parenthesize` enveloppe la musique qui suit sans changer sa
            // durée ni sa nature (`ly/music-functions-init.ly`). Le laisser
            // au cas « commande inconnue » coupait le `\grace` en deux : la
            // note devenait une note normale (`tablature-grace-notes.ly`).
            "parenthesize" => self.parse_music(),
            "markupMap" => {
                // `ly/music-functions-init.ly:1173-1174` : chemin, fonction
                // de markup, puis la musique qui reste jouée telle quelle.
                if self.peek().is_some_and(|token| {
                    matches!(token.kind, Kind::Word | Kind::Scheme) || token.is_punct('.')
                }) {
                    self.position += 1;
                    while self.eat_punct('.') {
                        if self.peek().is_some_and(|token| token.kind == Kind::Word) {
                            self.position += 1;
                        }
                    }
                }
                if self.peek().is_some_and(|token| token.is_scheme()) {
                    self.position += 1;
                } else if self.eat_command("markup") {
                    while !self.at_end()
                        && !self.peek().is_some_and(|token| token.is_command("etc"))
                    {
                        if self.eat_punct('{') {
                            self.skip_braced();
                        } else {
                            self.position += 1;
                        }
                    }
                    self.eat_command("etc");
                }
                self.parse_music()
            }
            // `ly/music-functions-init.ly:462` : `contextPropertyCheck` prend
            // un chemin de propriété et une valeur Scheme. Un nombre nu est une
            // valeur Scheme valide (`Staff.meterScalingFactor 1`,
            // `polymetric-time-warning-measure-length-irregular.ly`) : le
            // laisser dans le flux créait une durée d'un temps et décalait
            // toutes les notes.
            "contextPropertyCheck" => {
                let mut parties = 0;
                while parties < 3
                    && self
                        .peek()
                        .is_some_and(|token| token.kind == Kind::Word || token.is_punct('.'))
                {
                    self.position += 1;
                    parties += 1;
                }
                if self.peek().is_some_and(|token| {
                    matches!(
                        token.kind,
                        Kind::Scheme | Kind::Number | Kind::Word | Kind::Str | Kind::Command
                    )
                }) {
                    self.position += 1;
                }
                Music::Empty
            }
            "volta" => {
                // `\volta 2,1 musique` : liste de numéros, puis la musique
                // (`volta-bracket-warning-start-conflicting-numbers.ly`).
                // LilyPond la marque `volta-numbers` ; le filtre s'applique au
                // dépliage (`lily/unfolded-repeat-iterator.cc`,
                // `lily/alternative-sequence-iterator.cc`).
                let numbers = self.parse_volta_numbers();
                Music::Context(Context::Volta {
                    numbers,
                    music: Box::new(self.parse_music()),
                })
            }
            // `ly/music-functions-init.ly:1351` : quatre arguments
            // (armure, base, structure de temps, exceptions de liaison).
            // Les laisser dans le flux créaient une durée nue et un accord
            // vide (`time-signature-settings-by-staff.ly`).
            "overrideTimeSignatureSettings" => {
                for _ in 0..2 {
                    if self.peek().is_some_and(|token| {
                        token.kind == Kind::Number || token.is_scheme() || token.kind == Kind::Word
                    }) {
                        self.position += 1;
                    }
                }
                while self.peek().is_some_and(|token| {
                    token.kind == Kind::Number || token.is_punct(',') || token.kind == Kind::Word
                }) {
                    self.position += 1;
                }
                if self.peek().is_some_and(|token| token.is_scheme()) {
                    self.position += 1;
                }
                Music::Empty
            }
            "autoChange" => {
                // `ly/music-functions-init.ly:278-303` : `\\autoChange`
                // répartit la musique entre deux portées selon un seuil ; le
                // MIDI joue toutes ses notes, on lit donc la musique une
                // seule fois (les deux `\with` et la hauteur sont ignorés).
                let _ = self.parse_optional_pitch();
                for _ in 0..2 {
                    if self.eat_command("with") {
                        let _ = self.parse_with_block();
                    }
                }
                // `autochange-relative.ly` : les hauteurs données à
                // `\autoChange` sont finales, un `\relative` extérieur ne
                // doit pas les recalculer ; un `\relative` à l'intérieur, lui,
                // s'applique.
                Music::Context(Context::NonRelative {
                    music: Box::new(self.parse_music()),
                })
            }
            "repeat" => self.parse_repeat(),
            "*" | "%" => {
                // `ly/music-functions-init.ly:742-751` : raccourcis de
                // `\\repeat unfold` (`\\*`) et `\\repeat percent` (`\\%`).
                let count = match self.bump() {
                    Some(Token {
                        kind: Kind::Number,
                        text,
                        ..
                    }) => text.parse().unwrap_or(2),
                    // `#(+ 4 32)` est un seul jeton Scheme pour le lexer ;
                    // la fonction musicale reçoit la valeur évaluée 36.
                    Some(Token {
                        kind: Kind::Scheme,
                        text,
                        ..
                    }) => scheme_integer_count(&text)
                        .and_then(|value| u32::try_from(value).ok())
                        .unwrap_or(2),
                    _ => 2,
                };
                let kind = if name == "*" {
                    RepeatKind::Unfold
                } else {
                    RepeatKind::Ignored
                };
                Music::Context(Context::Repeat {
                    kind,
                    count,
                    body: Box::new(self.parse_music()),
                    alternatives: Vec::new(),
                    partial: false,
                    count_missing: false,
                })
            }
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
                let mode_word = match self.bump() {
                    Some(Token {
                        kind: Kind::Command | Kind::Word,
                        ref text,
                        ..
                    }) => text.clone(),
                    _ => String::new(),
                };
                let mode = if mode_word.is_empty() {
                    KeyMode::Major
                } else {
                    pitch::key_mode(&mode_word)
                };
                context(Context::Key {
                    fifths: pitch::fifths_of(pitch.step, pitch.alter)
                        + pitch::mode_fifths_offset(&mode_word),
                    mode,
                })
            }
            "timeAbbrev" => {
                // `\timeAbbrev [structure] [armure]` : la structure est une
                // liste de nombres séparés par des virgules, sans armure
                // obligatoire (`break-alignment-anchor-average-clamp.ly`).
                while self.peek().is_some_and(|token| {
                    (token.kind == Kind::Number && !token.text.contains('/')) || token.is_punct(',')
                }) {
                    self.position += 1;
                }
                if self
                    .peek()
                    .is_some_and(|token| token.kind == Kind::Number && token.text.contains('/'))
                {
                    let _ = self.parse_ratio();
                }
                Music::Empty
            }
            "time" => {
                self.consume_beat_structure();
                match self.parse_ratio() {
                    // Un dénominateur nul ferait paniquer `Fraction::new` ; un
                    // numérateur nul est signalé « unsupported time signature »
                    // par LilyPond et l'indication est ignorée (mesure par défaut).
                    Some((0, _)) | Some((_, 0)) => {
                        self.warn("indication de mesure invalide ignorée");
                        Music::Empty
                    }
                    Some((numerator, denominator)) => context(Context::Time {
                        numerator: i32::try_from(numerator).unwrap_or(4),
                        denominator: i32::try_from(denominator).unwrap_or(4),
                    }),
                    None => Music::Empty,
                }
            }
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
                // L'argument de `\mark` est **un** markup
                // (`lily/parser.yy`, `markup` → `simple_markup`) : un mot nu
                // (`\mark A`), une chaîne, une commande (`\mark \default`) ou
                // une commande `\markup` suivie de son bloc. Après cet
                // argument, la musique reprend : `\mark \default 4` doit
                // répéter l'accord précédent, pas avaler le `4` dans
                // l'étiquette.
                let explicit = self.peek().is_some_and(|token| token.is_command("markup"));
                if explicit {
                    self.position += 1;
                }
                let text = match self.peek().cloned() {
                    Some(token)
                        if token.kind == Kind::Word
                            && !matches!(token.text.as_str(), "r" | "R" | "s") =>
                    {
                        self.position += 1;
                        token.text
                    }
                    Some(token) if token.kind == Kind::Str => {
                        self.position += 1;
                        token.text
                    }
                    Some(token) if token.kind == Kind::Command && !explicit => {
                        self.position += 1;
                        token.text
                    }
                    _ => self.parse_markup_text(),
                };
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
                let visible = !std::mem::take(&mut self.pending_invisible_ottava);
                context(Context::Ottava(shift.clamp(-2, 2) as i8, visible))
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
                let chemin = self.skip_property_path();
                // `define-deprecated-property` (scm/define-context-properties.scm
                // :1001-1055) peut aliaser une propriété sur `skipTypesetting`
                // (`context-property-deprecated-unset.ly`). `\unset` de
                // l'alias doit rétablir le temps comme `\set … = ##f`.
                if chemin
                    .iter()
                    .any(|part| part.eq_ignore_ascii_case("skiptypesetting"))
                    || self.tokens.iter().any(|token| {
                        token.kind == Kind::Scheme
                            && token.text.contains("define-deprecated-property")
                            && token.text.contains("skipTypesetting")
                            && chemin
                                .iter()
                                .any(|part| token.text.contains(&format!("'{part}")))
                    })
                {
                    self.skip_typesetting = false;
                }
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
            // `\simultaneous { … }` est le jumeau de `<< … >>`
            // (`SIMULTANEOUS braced_music_list`, `lily/parser.yy` l. 1581-1588 ;
            // `scm/define-music-types.scm` l. 719) : les éléments du bloc sont
            // simultanés, alors que le lire comme une séquence décalait la
            // seconde voix après la première (`bach_air_bmv_1068`, guitare).
            "simultaneous" => {
                if self.peek().is_some_and(|token| token.is_punct('{')) {
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
                    Music::Sim(items)
                } else {
                    self.parse_music()
                }
            }
            // `\lyrics`, `\notemode`… ne font que changer le mode de lecture
            // des jetons suivants ; la musique enveloppée reste la même.
            _ if is_mode_head(&name) => self.parse_music(),
            // `\markup` accepte une chaîne, une commande de mise en forme ou un
            // bloc entre accolades. Sans accolade, on s'arrête devant la
            // musique au lieu d'avaler le reste du fichier jusqu'à une `}` ;
            // avec un bloc, on le saute comme le fait le lexer amont.
            "markup" | "markuplist" => {
                let text = self.parse_markup_text();
                if let Some(ident) = text
                    .trim_matches('"')
                    .strip_prefix("scoretk-head-")
                    .and_then(scoretk_lilypond_common::noteheads::group_from_name)
                {
                    self.pending_exact_head = Some(ident);
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
            // `scm/music-functions.scm:2144-2188` : skip-of-length et
            // mmrest-of-length produisent un silence de la durée exacte de
            // leur argument musical, sans jouer ses notes.
            // `ly/music-functions-init.ly` : applyMusic applique une
            // procédure Scheme à la musique qui suit. Les deux procédures du
            // corpus sont skip-of-length et mmrest-of-length
            // (scm/music-functions.scm:2144-2188).
            "applyMusic" => {
                let proc = self
                    .peek()
                    .filter(|token| token.is_scheme())
                    .map(|token| token.text.clone());
                if proc.is_some() {
                    self.position += 1;
                }
                let argument = self.parse_music();
                if proc.as_deref().is_some_and(|text| {
                    text.contains("skip-of-length") || text.contains("mmrest-of-length")
                }) {
                    let duree = music_duration(&argument, self.default_duration);
                    Music::Chord(Chord {
                        kind: ChordKind::SkipNumeric,
                        duration: Some(Duration::whole()),
                        multiplier: if duree == Fraction::ONE {
                            None
                        } else {
                            Some(duree)
                        },
                        from_scale: false,
                        bracketed: false,
                        posts: Vec::new(),
                        invisible: true,
                    })
                } else {
                    argument
                }
            }
            // `ly/music-functions-init.ly:1241-1245` : musicMap applique
            // une procédure à la musique. Le cas du corpus est
            // `#skip->rest` (skip-of-length.ly) : transformer les silences
            // en silences visibles, sans changer les attaques.
            "musicMap" => {
                let proc = self
                    .peek()
                    .filter(|token| token.is_scheme())
                    .map(|token| token.text.clone());
                if proc.is_some() {
                    self.position += 1;
                }
                let mut music = self.parse_music();
                if proc
                    .as_deref()
                    .is_some_and(|text| text.contains("skip->rest"))
                {
                    skip_to_rest(&mut music);
                }
                music
            }
            "skip-of-length" | "mmrest-of-length" => {
                let argument = self.parse_music();
                let duree = music_duration(&argument, self.default_duration);
                Music::Chord(Chord {
                    // Les deux produisent un silence qui avance le temps ;
                    // `SkipNumeric` garantit l'avance exacte de la durée
                    // (une `MultiRest` est arrondie à la mesure entière).
                    kind: ChordKind::SkipNumeric,
                    duration: Some(Duration::whole()),
                    multiplier: if duree == Fraction::ONE {
                        None
                    } else {
                        Some(duree)
                    },
                    from_scale: false,
                    bracketed: false,
                    posts: Vec::new(),
                    invisible: true,
                })
            }
            "skip" => {
                // Notation Reference §2.2.2 : `s4` fixe la durée des notes
                // suivantes, `\skip 4` non. Le parseur ne doit donc pas
                // laisser la durée de l'argument dans la durée par défaut.
                let previous_duration = self.default_duration;
                let previous_multiplier = self.default_multiplier;
                // Un nombre invalide (`\skip 0`) est tout de même consommé
                // comme durée : il ne doit pas être relu comme musique.
                let argument_est_duree = self.peek().is_some_and(|token| {
                    token.kind == Kind::Number
                        || token.is_command("breve")
                        || token.is_command("longa")
                        || token.is_command("maxima")
                });
                let (duration, multiplier) = self.parse_optional_duration_spec();
                self.default_duration = previous_duration;
                self.default_multiplier = previous_multiplier;
                if duration.is_none() && !argument_est_duree && self.at_toplevel_music() {
                    // `ly/music-functions-init.ly:2267-2274` : l'argument de
                    // `\skip` peut être une musique (`duration-or-music?`),
                    // enveloppée dans un `SkippedMusic` : elle avance le temps
                    // sans jouer ses notes (skip-music.ly).
                    let music = self.parse_music();
                    return silence_music(music);
                }
                let (duration, multiplier) = self.resolve_chord_duration(duration, multiplier);
                Music::Chord(Chord {
                    kind: ChordKind::SkipNumeric,
                    duration,
                    multiplier,
                    from_scale: false,
                    bracketed: false,
                    posts: Vec::new(),
                    invisible: true,
                })
            }
            "pitchedTrill" => {
                // `ly/music-functions-init.ly:1707-1712` : le second argument
                // est la tête de note entre parenthèses du trille, seulement
                // gravée ; elle ne joue pas dans le MIDI. La note principale
                // (avec `\startTrillSpan`) est lue normalement.
                let principale = self.parse_chord(false);
                self.saute_pitch_secondaire();
                principale
            }
            "cueDuring" | "cueDuringWithClef" | "quoteDuring" => {
                // `ly/music-functions-init.ly:545-574,2023-2032` : la citation est
                // insérée dans une CueVoice ; le MIDI joue les notes de la
                // citation pendant que l'argument principal (des silences)
                // occupe le temps. On tronque la citation à la durée de cet
                // argument.
                let nom = match self.peek().cloned() {
                    Some(Token {
                        kind: Kind::Str | Kind::Word,
                        text,
                        ..
                    }) => {
                        self.position += 1;
                        text
                    }
                    _ => String::new(),
                };
                // Direction `#DOWN`/`#UP`, puis l'armure pour la variante
                // `cueDuringWithClef` : ce sont des arguments Scheme ou des
                // chaînes, sans effet sur le temps joué.
                if self.peek().is_some_and(|token| token.is_scheme()) {
                    self.position += 1;
                }
                if name == "cueDuringWithClef"
                    && self.peek().is_some_and(|token| token.kind == Kind::Str)
                {
                    self.position += 1;
                }
                let principal = self.parse_music();
                let duree = music_duration(&principal, self.default_duration);
                // A quote may refer to a `\addQuote` declared later in the
                // file (`input/regression/quote-cyclic.ly`). LilyPond keeps
                // quoted music as an identifier, resolved when the quote is
                // used; read the definition on demand instead of dropping it.
                if !self.quotes.contains_key(&nom) {
                    self.resolve_quote(&nom);
                }
                self.quotes
                    .get(&nom)
                    .map(|quote| {
                        // La citation n'est pas tronquée ici : `Quote_iterator`
                        // la découpe au temps absolu de la partition
                        // (`quote-iterator.cc:161-211`, `start_mom`). Le
                        // parseur ne connaît pas ce temps ; l'interpréteur
                        // garde les événements de la fenêtre
                        // `[curseur, curseur + durée)`.
                        Music::Context(Context::Cue {
                            cue_notes: name != "quoteDuring",
                            music: Box::new(quote.clone()),
                            duration: duree,
                        })
                    })
                    .unwrap_or(Music::Empty)
            }
            // `ly/music-functions-init.ly:955-964` : `\killCues` retire les
            // notes de cue de `\cueDuring` mais garde les citations de
            // `\quoteDuring` ; le temps occupé reste (silences).
            "killCues" => kill_cues(self.parse_music()),
            // `ly/music-functions-init.ly:348-354` : `beamExceptions` est une
            // fonction Scheme à argument musical. Elle produit une valeur de
            // propriété pour `\set`, jamais de la musique : laisser son
            // argument dans le flux créait 16 accords vides de 32e
            // (`auto-beam-exceptions.ly`).
            "beamExceptions" => {
                let _ = self.parse_music();
                Music::Empty
            }
            // `ly/string-tunings-init.ly:29-37` : `stringTuning` convertit un
            // accord en liste de hauteurs, valeur de `\set stringTunings`.
            "stringTuning" => {
                let _ = self.parse_music();
                Music::Empty
            }
            // `ly/property-init.ly:439-490` : `\incipit [offset] music`
            // grave la musique dans un `\score` de `\markup`
            // (MensuralStaff). C'est une image, absente du MIDI. L'argument
            // peut être un `\new Contexte \with { … } { … }`
            // (`incipit-other-context.ly`).
            "incipit" => {
                if self
                    .peek()
                    .is_some_and(|token| token.kind == Kind::Number || token.is_scheme())
                {
                    self.position += 1;
                }
                if self.eat_command("new") || self.eat_command("context") {
                    if self.peek().is_some_and(|token| token.kind == Kind::Word) {
                        self.position += 1;
                    }
                    if self.eat_command("with") {
                        self.parse_with_block();
                    }
                }
                let _ = self.parse_music();
                Music::Empty
            }
            // `ly/music-functions-init.ly:2631-2640` : `voices` prend une
            // `key-list?` (numéros, noms, chaînes) puis la musique. La liste
            // laissée dans le flux devenait des durées et décalait les voix
            // (`voices-command.ly`).
            "voices" => {
                // `ly/music-functions-init.ly:2631-2665` : liste de voix
                // (numéros, noms, chaînes) puis la musique simultanée. Une
                // branche affectée au nom de la voix courante la prolonge
                // (liaisons comprises, `voices-command.ly`).
                let mut ids = Vec::new();
                while let Some(token) = self.peek().cloned() {
                    match token.kind {
                        Kind::Number => {
                            if let Some(number) = token.as_integer() {
                                ids.push(VoicesId::Number(u8::try_from(number).unwrap_or(0)));
                            }
                            self.position += 1;
                        }
                        Kind::Str | Kind::Scheme | Kind::Word => {
                            let name = token
                                .text
                                .trim()
                                .trim_start_matches('#')
                                .trim_start_matches('\'')
                                .trim_matches('"')
                                .to_owned();
                            ids.push(VoicesId::Name(name));
                            self.position += 1;
                        }
                        Kind::Punct(',') => self.position += 1,
                        _ => break,
                    }
                }
                let music = self.parse_music();
                Music::Context(Context::Voices {
                    ids,
                    music: Box::new(music),
                })
            }
            "compressMMRests" => {
                // `ly/music-functions-init.ly:452-459` : ne change que la
                // gravure des silences ; les hauteurs et attaques restent.
                self.parse_music()
            }
            // `ly/property-init.ly:140,970`, `ly/declarations-init.ly:78` :
            // commandes sans argument. Une durée nue qui suit est de la
            // musique, pas leur argument.
            // `ly/declarations-init.ly:61-64` : `break`, `noBreak` et
            // `allowBreak` sont des `LineBreakEvent` sans argument. Une durée
            // nue qui suit reste de la musique (`allow-break.ly`).
            "oneVoice" | "autoBeamOff" | "autoBeamOn" | "noBeam" | "beamUp" | "beamDown"
            | "beamNeutral" | "allowBreak" | "startStaff" | "stopStaff" | "fine" | "breathe"
            | "caesura" => Music::Empty,
            "startMeasureCount" | "stopMeasureCount" => {
                // `ly/spanners-init.ly:36-37` : événements de portée sans
                // argument ; une durée nue qui suit reste de la musique.
                Music::Empty
            }
            "textMark" | "textEndMark" => {
                // `ly/music-functions-init.ly:2365-2373` : un argument de
                // markup, rien de plus. La durée nue qui suit reste de la
                // musique (`text-mark-marklengthon.ly`).
                if self.peek().is_some_and(|token| token.is_command("markup")) {
                    self.position += 1;
                    self.parse_markup_text();
                } else {
                    self.read_function_argument("markup?");
                }
                Music::Empty
            }
            // `ly/music-functions-init.ly:655-670` : featherDurations
            // multiplie la durée de chaque note par `scale^i`, puis
            // compresse le tout pour conserver la durée totale.
            "featherDurations" => {
                let scale = self
                    .parse_ratio()
                    .map(|(n, d)| {
                        Fraction::try_new(i64::from(n), i64::from(d)).unwrap_or(Fraction::ONE)
                    })
                    .unwrap_or(Fraction::ONE);
                let mut music = self.parse_music();
                feather_music(&mut music, &scale);
                music
            }
            "augmentum" => {
                // `ly/ancient-init.ly:124-130` : `shift-duration-log expr 0 1`
                // ajoute une mora (un point) à chaque durée de l'expression.
                let music = self.parse_music();
                ajoute_point(music, self.default_duration)
            }
            "fermata" | "shortfermata" | "longfermata" | "verylongfermata" => Music::Empty,
            // Les durées nues `\breve`, `\longa`, `\maxima` sont traitées en
            // amont de `parse_command`, avant consommation du jeton, par
            // `parse_bare_duration` (durée, points et multiplicateurs lus par
            // `parse_duration`).
            "version" => {
                // `\version "x"` est une commande de haut niveau, mais un
                // `\include` peut l'injecter au milieu d'une musique. Ne
                // consommer que la chaîne : une durée nue qui suit est une
                // musique à part entière (`include-identifier.ily`).
                if self.peek().is_some_and(|token| token.kind == Kind::Str) {
                    self.position += 1;
                }
                Music::Empty
            }
            "makeClusters" => {
                // `scm/music-functions.scm` (`note-to-cluster`) remplace les
                // NoteEvent par des ClusterNoteEvent, sans performer MIDI :
                // la piste Cluster est vide dans le MIDI de référence.
                // Conserver la musique dans le modèle avec `play = false`
                // garde les notes pour tous les autres formats.
                Music::Context(Context::Silent {
                    music: Box::new(self.parse_music()),
                })
            }
            _ if is_articulation_command(&name) || is_dynamic_command(&name) => {
                // Événement isolé sans accord à décorer : ignoré.
                Music::Empty
            }
            _ => {
                // `#(define nom #{ … #})` crée un identifiant LilyPond :
                // `\nom` le désigne aussi bien que `$nom` ou `#nom`
                // (`utf-8-scheme.ly`). La résolution vient **après** les
                // commandes intégrées : `#(define ottava (let … (define-music-function
                // … #)))` (`Chop-28-19`) enveloppe la commande d'origine, et
                // le lecteur ne sait pas évaluer la fonction Scheme ; prendre
                // le corps stocké perdait l'octavation (une note en trop,
                // cinq attaques décalées). La commande intégrée reste donc
                // prioritaire, la variable Scheme servant de repli.
                if let Some(music) = self.scheme_variables.get(&lookup).cloned() {
                    self.position += extra;
                    return music;
                }
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

    /// Compte d'une répétition partielle (`\repeat percent \etc` puis
    /// `\testPercent 4`) : nombre ou identifiant lié à un entier
    /// (`lily/parser.yy`, `REPEAT simple_string partial_function`).
    fn parse_repeat_count(&mut self) -> u32 {
        match self.peek().cloned() {
            Some(Token {
                kind: Kind::Number,
                text,
                ..
            }) => {
                self.position += 1;
                text.parse().unwrap_or(2)
            }
            Some(Token {
                kind: Kind::Command,
                text,
                ..
            }) => {
                self.position += 1;
                self.numbers.get(&text).copied().unwrap_or(2) as u32
            }
            _ => 2,
        }
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
                partial: true,
                count_missing: true,
            });
        }
        let count = match self.bump() {
            Some(Token {
                kind: Kind::Number,
                text,
                ..
            }) => text.parse().unwrap_or(2),
            // `num = #3` puis `\repeat unfold \num` (`lily/parser.yy`).
            Some(Token {
                kind: Kind::Command,
                text,
                ..
            }) => self.numbers.get(&text).copied().unwrap_or(2) as u32,
            _ => 2,
        };
        if self.peek().is_some_and(|token| token.is_command("etc")) {
            self.position += 1;
            return Music::Context(Context::Repeat {
                kind,
                count,
                body: Box::new(Music::Empty),
                alternatives: Vec::new(),
                partial: true,
                count_missing: false,
            });
        }
        // `lily/parser.yy:1560-1569` : le corps peut être vide quand
        // `\alternative` suit directement `\repeat volta n`. Appeler
        // `parse_music()` ici consommerait le `\alternative` comme un
        // événement vide et perdrait toutes les fins (`repeat-petrucci-
        // alternatives.ly`).
        let mut corps = if self
            .peek()
            .is_some_and(|token| token.is_command("alternative"))
        {
            Music::Empty
        } else {
            self.parse_music()
        };
        // LilyPond accepte `\alternative` dans les accolades du corps
        // (`\repeat volta n { … \alternative { … } }`) : le retirer du corps
        // et l'enregistrer comme endings de la reprise.
        let mut alternatives = Vec::new();
        extract_alternatives(&mut corps, &mut alternatives);
        let body = Box::new(corps);
        // `\repeat tremolo` est gardé tel quel : l'étirement des durées ou le
        // dépliage dépendent de `\unfoldRepeats` **au lieu d'utilisation** de
        // la variable musicale, pas de sa définition. Une variable définie en
        // tête de fichier puis rejouée dans un `\score { \unfoldRepeats … }`
        // (`promenade-6.ly`) doit être dépliée, même si elle a été analysée
        // avant le `\unfoldRepeats`. L'interpréteur applique donc le facteur
        // à la lecture (`events.rs`, `RepeatKind::Tremolo`).
        if alternatives.is_empty() && self.eat_command("alternative") {
            let content = self.parse_music();
            alternatives = blocks_of(content);
        }
        Music::Context(Context::Repeat {
            kind,
            count,
            body,
            alternatives,
            partial: false,
            count_missing: false,
        })
    }

    /// `\set`.
    fn parse_set(&mut self) -> Music {
        let path = self.skip_property_path();
        if !self.eat_punct('=') {
            return Music::Empty;
        }
        // `\set Staff.midiInstrument = "violin"` (`lily/parser.yy` `set_property`,
        // `lily/staff-performer.cc:244`) : la valeur est une chaîne, pas de la
        // musique. Elle est conservée comme propriété du contexte à venir :
        // l'émettre comme un évènement musical créerait une portée anonyme
        // dans `<< \set … \new Staff … >>` (bwv-1006a_6).
        if path
            .iter()
            .any(|part| part.eq_ignore_ascii_case("midiinstrument"))
        {
            self.pending_midi_instrument = Some(self.parse_property_value());
            return Music::Empty;
        }
        // Notation Reference §24.4.2 : `midiMinimumVolume` et
        // `midiMaximumVolume` règlent la plage MIDI des nuances. Quand les
        // deux valent la même fraction, le modèle retrouve la vélocité
        // numérique que le symbole `\p` ne porte pas.
        if path
            .iter()
            .any(|part| part.eq_ignore_ascii_case("midiminimumvolume"))
        {
            let value = self
                .peek()
                .filter(|token| token.is_scheme())
                .and_then(|token| token.text.trim_start_matches('#').parse::<f64>().ok())
                .map(|fraction| (fraction * 127.0).round().clamp(0.0, 127.0) as u8);
            if let Some(value) = value {
                self.position += 1;
                self.pending_dynamic_velocity = Some(value);
                return Music::Empty;
            }
        }
        if path
            .iter()
            .any(|part| part.eq_ignore_ascii_case("scoretktremolokind"))
        {
            let kind = self
                .peek()
                .filter(|token| token.is_scheme())
                .and_then(|token| token.text.trim_start_matches('#').parse::<u8>().ok())
                .and_then(|number| match number {
                    1 => Some(TremoloKind::Start),
                    2 => Some(TremoloKind::Stop),
                    3 => Some(TremoloKind::Unmeasured),
                    _ => None,
                });
            if let Some(kind) = kind {
                self.position += 1;
                self.pending_tremolo_kind = Some(kind);
                return Music::Empty;
            }
        }
        if path
            .iter()
            .any(|part| part.eq_ignore_ascii_case("midimaximumvolume"))
            && self.peek().is_some_and(|token| token.is_scheme())
        {
            self.position += 1;
            return Music::Empty;
        }
        // `\set Timing.timing = ##f` / `##t` a le même effet que
        // `\cadenzaOn` / `\cadenzaOff`. Le lexer lit `##f` comme le jeton
        // Scheme `#f` (et `##t` comme `#t`).
        if path.iter().any(|part| part.eq_ignore_ascii_case("timing"))
            && !path
                .iter()
                .any(|part| part.eq_ignore_ascii_case("skiptypesetting"))
        {
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
        if path
            .iter()
            .any(|part| part.eq_ignore_ascii_case("tiewaitfornote"))
        {
            eprintln!("SET MIN reached");
            let value = self
                .peek()
                .filter(|token| token.is_scheme())
                .and_then(
                    |token| match token.text.trim().to_ascii_lowercase().as_str() {
                        "#t" => Some(true),
                        "#f" => Some(false),
                        _ => None,
                    },
                );
            if let Some(on) = value {
                self.position += 1;
                return Music::Context(Context::TieWaitForNote(on));
            }
        }
        // `scm/define-context-properties.scm:716` et
        // `lily/score-performer.cc:151-176` : `skipTypesetting` coupe
        // l'interprétation MIDI et replie le temps de la zone sautée.
        if path
            .iter()
            .any(|part| part.eq_ignore_ascii_case("skiptypesetting"))
        {
            if let Some(on) = self
                .peek()
                .filter(|token| token.is_scheme())
                .and_then(
                    |token| match token.text.trim().to_ascii_lowercase().as_str() {
                        "#t" => Some(true),
                        "#f" => Some(false),
                        _ => None,
                    },
                )
            {
                self.position += 1;
                self.skip_typesetting = on;
            }
        }
        let fraction = self.parse_scheme_fraction();
        // `\set Timing.beatStructure = 1,1` : liste de nombres séparés par
        // des virgules. Le premier est lu comme fraction ; laisser `,1` dans
        // le flux musical en faisait une marque d'octave suivie d'une durée
        // de blanche ronde, décalant la suite de quatre noires
        // (No03_Albumblatt, `\global`).
        if fraction.is_some() || self.peek().is_some_and(|token| token.kind == Kind::Number) {
            while self.eat_punct(',') {
                if self.peek().is_some_and(|token| token.kind == Kind::Number) {
                    self.position += 1;
                } else {
                    break;
                }
            }
        }
        let is_measure_length = path
            .iter()
            .any(|part| part.eq_ignore_ascii_case("measurelength"));
        let is_measure_position = path
            .iter()
            .any(|part| part.eq_ignore_ascii_case("measureposition"));
        match (is_measure_length, is_measure_position, fraction) {
            (true, _, Some(fraction)) => Music::Context(Context::MeasureLength(fraction)),
            // `\set Timing.measurePosition = #(ly:make-moment -1 4)` : la
            // position dans la mesure courante (lily/timing-translator.cc
            // l. 470-505). La valeur négative est conservée.
            (_, true, Some(fraction)) => Music::Context(Context::MeasurePosition(fraction)),
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
        if path.iter().any(|part| part == "OttavaBracket")
            && path.iter().any(|part| part == "transparent")
        {
            // Un crochet transparent (`scm/define-grobs.scm:2708`) ne change
            // pas la hauteur écrite, mais l'ottava qu'il précède ne doit pas
            // être réécrite avec un `<octave-shift>` visible à l'export.
            self.eat_punct('=');
            self.skip_scheme_value();
            self.pending_invisible_ottava = true;
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
                    // `markup-note-grob-style.ly` : une valeur de propriété
                    // est un markup (`\markup { \note {16} #UP }`) ; le
                    // consommer entièrement évite que son `{16}` soit relu
                    // comme une durée nue, créant un accord vide.
                    if self.peek().is_some_and(|token| token.is_command("markup")) {
                        self.position += 1;
                        self.parse_markup_text();
                    } else if self.eat_command("musicLength") {
                        // `\musicLength 16` : valeur de propriété
                        // (`spacing-space-to-barline.ly`).
                        let _ = self.parse_duration();
                    } else {
                        self.skip_scheme_value();
                    }
                }
            }
        }
        match (once, direction) {
            (true, Some(direction)) => Music::Context(Context::StemOverride(direction)),
            _ => Music::Empty,
        }
    }

    /// Propriété d'un `\tweak` : un seul symbole, même dans l'ancienne
    /// syntaxe `#'style`. `skip_property_path` enchaînerait les deux symboles
    /// Scheme consécutifs (`#'style #'cross`) et avalerait la valeur.
    fn parse_tweak_property(&mut self) -> Vec<String> {
        if let Some(token) = self.peek().cloned() {
            if token.is_scheme() {
                self.position += 1;
                let symbol = token
                    .text
                    .trim()
                    .trim_start_matches(['#', '\''])
                    .trim()
                    .to_owned();
                return vec![symbol];
            }
        }
        self.skip_property_path()
    }

    /// `\tweak` : propriété, valeur, puis musique décorée.
    fn parse_tweak(&mut self) {
        let path = self.parse_tweak_property();
        // `\tweak` s'écrit `\tweak propriété valeur` (`lily/parser.yy`),
        // sans `=` ; certains fichiers gardent un `=` toléré.
        let had_eq = self.eat_punct('=');
        let Some(token) = self.peek().cloned() else {
            return;
        };
        // Valeur signée `-2` : le signe et le nombre doivent être consommés
        // ensemble, même sans `=`, sinon le `-2` est lu comme musique et
        // allonge l'accord d'un temps (`parenthesize-chords.ly`).
        if token.is_punct('-')
            && self
                .peek_at(1)
                .is_some_and(|suivant| suivant.kind == Kind::Number)
        {
            self.position += 2;
            return;
        }
        // La syntaxe standard n'a pas de `=` : le `\tweak` n'étant plus un
        // post-événement, le consommer ici laisse la note suivante intacte.
        let _ = had_eq;
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

    /// Arguments de `\offset` : la propriété (`symbol-list-or-symbol?`) et la
    /// valeur de décalage (`scheme?`). L'item n'est pas consommé.
    fn parse_offset_arguments(&mut self) {
        // Propriété : un symbole (éventuellement pointé) ou une liste Scheme.
        if self.peek().is_some_and(|token| token.is_scheme()) {
            self.position += 1;
        } else {
            let _ = self.skip_property_path();
        }
        // Valeurs : datum Scheme, nombre (éventuellement signé) ou liste.
        self.skip_scheme_value();
    }

    /// Vrai si le jeton courant commence une liste de clés de grob
    /// (`BreathingSign`, `Voice.Beam`, `Slur`…) et non une musique. Les noms
    /// de grob et de contexte commencent par une majuscule, à la différence
    /// des hauteurs et des variables usuelles.
    fn at_offset_key_list(&self) -> bool {
        match self.peek() {
            Some(Token {
                kind: Kind::Word,
                text,
                ..
            }) => text.chars().next().is_some_and(char::is_uppercase),
            _ => false,
        }
    }

    //------------------------------------------------------------------
    // Accords et durées
    //------------------------------------------------------------------

    /// Recompose un nom d'accord coupé par le lexer (`c:m7`, `c/g`,
    /// `c:m7.5-`). La durée qui suit (un nombre sans `:` ni `/`) n'en fait pas
    /// partie et reste pour `parse_optional_duration_spec`.
    /// Consomme un modificateur d'accord qui suit un degré (`7m`, `7sus`),
    /// s'il y en a un (`default-chord-modifier-list`,
    /// `scm/chord-entry.scm:251-257`).
    fn consume_chord_modifier(&mut self, nom: &mut String) {
        let Some(Token {
            kind: Kind::Word,
            text,
            ..
        }) = self.peek().cloned()
        else {
            return;
        };
        if matches!(
            text.as_str(),
            "m" | "min" | "maj" | "dim" | "aug" | "sus" | "add"
        ) {
            nom.push_str(&text);
            self.position += 1;
        }
    }

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
                                // `c:7+` : le `+` est un CHORD_MODIFIER
                                // (lily/lexer.ll:1062-1067) qui rend la
                                // septième majeure (chord-name-major7.ly).
                                if self.eat_punct('+') {
                                    nom.push('+');
                                }
                                // `c:7m` : un modificateur peut suivre le
                                // degré (`chord_item` de `lily/parser.yy`
                                // l. 3886-3898, modificateurs et degrés dans
                                // l'ordre écrit).
                                self.consume_chord_modifier(&mut nom);
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
                                self.consume_chord_modifier(&mut nom);
                                // Le lexer fait de `m7.5` un seul mot : le
                                // `-`/`+` qui altère le dernier degré
                                // (`min7.5-`, `7+`) suit comme ponctuation
                                // (`CHORD_MODIFIER`, `lily/lexer.ll:1062-1067`).
                                if self.peek().is_some_and(|token| token.is_punct('-')) {
                                    self.position += 1;
                                    nom.push('-');
                                } else if self.eat_punct('+') {
                                    nom.push('+');
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
                        self.consume_chord_modifier(&mut nom);
                    }
                }
                Some(Token {
                    kind: Kind::Punct('/'),
                    ..
                }) => {
                    self.position += 1;
                    nom.push('/');
                    // `/ +d` : basse explicite (`scm/chord-entry.scm`
                    // l. 46-53, `interpret-bass`). Le `+` doit rester dans le
                    // nom, sinon le `d` devient un accord séparé au temps
                    // suivant (`bes/+d` décalait toute la grille).
                    if self.eat_punct('+') {
                        nom.push('+');
                    }
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
        let music = self.parse_chord_inner(in_chord);
        if self.skip_typesetting {
            // Le temps est replié, mais la résolution relative porte sur tout
            // l'arbre (`ly/music-functions-init.ly:2055-2075`) : garder les
            // accords dans une enveloppe silencieuse pour que
            // `Interpreter::chord_notes` mette à jour la référence d'octave.
            Music::Context(Context::Skipped {
                music: Box::new(music),
            })
        } else {
            music
        }
    }

    fn parse_chord_inner(&mut self, in_chord: bool) -> Music {
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
                    let explicit = duration.is_some();
                    let (duration, multiplier) = self.resolve_chord_duration(duration, multiplier);
                    let posts = self.parse_posts();
                    let invisible = std::mem::take(&mut self.pending_invisible_rest);
                    return Music::Chord(Chord {
                        kind: match text.as_str() {
                            "R" => ChordKind::MultiRest { explicit },
                            "s" => ChordKind::Skip,
                            _ => ChordKind::Rest,
                        },
                        duration,
                        multiplier,
                        from_scale: false,
                        bracketed: false,
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
                        bracketed: false,
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
            if text == "q" {
                // `q` répète le dernier accord (`expand-repeat-chords!`,
                // `scm/music-functions.scm:920-945`). En `\chordmode`, le mot
                // passait pour un nom d'accord inconnu et l'accord répété
                // disparaissait (`chord-names-spanbar.ly`).
                self.position += 1;
                let (duration, multiplier) = self.parse_optional_duration_spec();
                let (duration, multiplier) = self.resolve_chord_duration(duration, multiplier);
                let posts = self.parse_posts();
                return Music::Chord(Chord {
                    kind: ChordKind::Repeat,
                    duration,
                    multiplier,
                    from_scale: false,
                    bracketed: false,
                    posts,
                    invisible: false,
                });
            }
            self.position += 1;
            // Les marques d'octave font partie du nom (`bes,`, `f''`,
            // `chord_body` de `lily/parser.yy` : `pitch_or_tonic_pitch` les
            // porte, puis la durée). Le lexer les coupe en ponctuation : les
            // lire avant la durée, sinon `bes,2` laisse `,` et `2` à la
            // musique suivante, qui rejoue l'accord une octave trop haut.
            let mut octave_marks = 0i32;
            while let Some(token) = self.peek() {
                if token.is_punct('\'') {
                    octave_marks += 1;
                    self.position += 1;
                } else if token.is_punct(',') {
                    octave_marks -= 1;
                    self.position += 1;
                } else {
                    break;
                }
            }
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
                // La table donne la fondamentale à l'octave 4 ; les marques du
                // nom décalent l'accord (`bes,` = un Bb plus bas), le `\fixed`
                // environnant étant appliqué ensuite par `events.rs`.
                if octave_marks != 0 {
                    let shift = octave_marks * 12;
                    for pitch in &mut pitches {
                        *pitch = (i32::from(*pitch) + shift).clamp(0, 127) as u8;
                    }
                }
                let head = self
                    .pending_exact_head
                    .take()
                    .unwrap_or_else(|| std::mem::take(&mut self.pending_head));
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
                    // Un nom de `\chordmode` est un `EventChord`
                    // (`chord_body` de `lily/parser.yy`), seule référence de
                    // `q` (`expand-repeat-chords!`, `scm/music-functions.scm`
                    // l. 920-945) : sans cela `q` après un accord nommé était
                    // « Bad chord repetition » (`chord-names-spanbar.ly`).
                    bracketed: self.note_mode == NoteMode::Chords,
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
            // Commentaire privé `% scoretk-head-…`, lu comme mot par le
            // lexeur : il désigne le groupe exact de la note suivante.
            if let Some(ident) = text.strip_prefix("scoretk-head-") {
                self.pending_exact_head =
                    scoretk_lilypond_common::noteheads::group_from_name(ident);
                self.position += 1;
                return Music::Empty;
            }
            // Mot qui n'est pas une hauteur : variable ou symbole isolé.
            self.position += 1;
            if self.variables.contains_key(&text) {
                let music = self.variables[&text].clone();
                return music;
            }
            self.warn(format!("mot ignoré dans la musique : {text}"));
            return Music::Empty;
        };

        let head = self
            .pending_exact_head
            .take()
            .unwrap_or_else(|| std::mem::take(&mut self.pending_head));
        let mut note_posts = self.parse_posts_until_note_end();
        let (duration, multiplier) = self.parse_optional_duration_spec();
        // `pitch_or_music` de `lily/parser.yy:3714-3742` accepte aussi les
        // marques d'octave **après** la durée (`c8'`, production
        // `erroneous_quotes`, l. 3367-3371) : LilyPond avertit
        // (« octave marks must precede duration ») mais transpose bien la
        // note. Les lire sinon comme une durée nue laissait la hauteur à
        // l'octave inférieure (`25EF-01.ly`, vérifié sur le MIDI 2.26).
        let mut pitch = pitch;
        self.apply_trailing_octave_marks(&mut pitch, duration.is_some());
        let notes = vec![NoteSpec {
            pitch,
            posts: std::mem::take(&mut note_posts),
            head_group: head,
            midi: None,
            transposable: false,
        }];
        let (duration, multiplier) = self.resolve_chord_duration(duration, multiplier);
        // Les post-événements peuvent suivre la durée d'un accord.
        let posts = self.parse_posts();
        Music::Chord(Chord {
            kind: ChordKind::Notes(notes),
            duration,
            multiplier,
            from_scale: false,
            bracketed: false,
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
            let head = self
                .pending_exact_head
                .take()
                .unwrap_or_else(|| std::mem::take(&mut self.pending_head));
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
            bracketed: true,
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
            kind: ChordKind::PitchRepeat,
            duration,
            multiplier,
            from_scale: false,
            bracketed: false,
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
                // `tremolo_type` (`lily/parser.yy:3584-3596`) : `':'` seul
                // reprend le dernier type écrit (8 au départ), `':' UNSIGNED`
                // le fixe. Le nombre n'est consommé que s'il est écrit : sinon
                // `c4: d4` avalait la hauteur `d`, qui devenait une durée nue
                // répétant `c` (Egmont, `violinoone.ly` : `a'''2.:8 a''':`).
                self.position += 1;
                let value = match self.peek().and_then(|token| token.as_integer()) {
                    Some(value) => {
                        self.position += 1;
                        let value = u8::try_from(value.clamp(1, 1024)).unwrap_or(8);
                        self.default_tremolo_type = value;
                        value
                    }
                    None => self.default_tremolo_type,
                };
                vec![PostEvent::Tremolo(
                    self.pending_tremolo_kind
                        .take()
                        .unwrap_or(TremoloKind::Single),
                    value,
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
                // `\= #'id` numérote un `FingerGlideSpanner`
                // (`ly/music-functions-init.ly:767-770`) : l'identifiant est
                // un datum Scheme, sans musique.
                if self.peek().is_some_and(|token| token.is_scheme()) {
                    self.position += 1;
                    return Vec::new();
                }
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
                // `bendAfter` prend un argument (ton de la chute,
                // `ly/music-functions-init.ly:357`), écrit `#10` ou `10`.
                // Le laisser dans le flux en faisait une durée nue qui
                // répétait la note et décalait tout
                // (`bend-after-start-in-grace.ly`).
                if name == "single" {
                    // `\single` (`ly/music-functions-init.ly:2249-2264`)
                    // convertit ses overrides en tweaks sur la musique qui
                    // suit ; en post-événement, consommer les overrides
                    // (`\omit Fingering`, `\override …`) et continuer avec
                    // les post-événements de la note (`fingering-no-stencil.ly`).
                    if self.eat_command("omit") {
                        let _ = self.skip_property_path();
                    } else if self.eat_command("override") {
                        let _ = self.parse_override(false);
                    }
                    return Vec::new();
                }
                if name == "shape" || name == "vshape" {
                    self.skip_scheme_value();
                    if self.at_offset_key_list() {
                        let _ = self.skip_property_path();
                    }
                    return Vec::new();
                }
                if name == "tweak" {
                    // `\tweak propriété valeur` en position de post-événement
                    // (`lily/parser.yy` : `tweak` est une musique *et* un
                    // post-événement). Consommer la propriété et sa valeur,
                    // puis laisser la boucle lire l'événement modifié (`^1`).
                    // Sans cela la valeur restait dans le flux et `^1` était
                    // lu comme une musique neuve qui répétait la note
                    // (`finger-glide-spanner-id.ly`). Le `\tweak` lui-même a
                    // déjà été consommé par la branche `Kind::Command` ci-dessus.
                    let path = self.parse_tweak_property();
                    let _ = self.eat_punct('=');
                    let value = self.peek().cloned();
                    match &value {
                        Some(token)
                            if token.is_scheme()
                                || token.kind == Kind::Str
                                || token.kind == Kind::Number =>
                        {
                            self.position += 1;
                        }
                        Some(token) if token.is_punct('-') || token.is_punct('+') => {
                            self.skip_scheme_value();
                        }
                        _ => {}
                    }
                    if let Some(token) = value {
                        if token.is_scheme()
                            && (path.iter().any(|part| part == "style") || path.is_empty())
                        {
                            let style = token.text.trim().trim_start_matches('\'').trim();
                            if let Some(group) = head_group_from_style(style) {
                                // `\tweak` est une *pré*-événement de
                                // `lily/parser.yy` : même placé après une note,
                                // il modifie la musique qui suit (`b2 \tweak
                                // color #red ^1 c''4` modifie le doigté). Le
                                // faire porter par la note courante empêchait
                                // de relire les têtes écrites par scoretk,
                                // qui place le style juste avant la hauteur.
                                self.pending_head = group;
                            }
                        }
                    }
                    return Vec::new();
                }
                if !name.is_empty() && name.chars().all(|c| c.is_ascii_digit()) {
                    // `\5` : numéro de corde (`stringNumber`), post-événement
                    // sans argument. Passer par `parse_direction_event`
                    // consommait le `-` du doigté qui suit comme un tenuto
                    // et laissait son chiffre comme durée nue, répétant la
                    // note (`script-stack-order1.ly`).
                    return Vec::new();
                }
                if name == "glide" {
                    // `glide = #(make-music 'FingerGlideEvent)`
                    // (`ly/declarations-init.ly:79`) : post-événement sans
                    // argument ni effet MIDI. Passer par
                    // `parse_direction_event` consommait le `^` du doigté qui
                    // suit et laissait son `1` comme durée nue, répétant la
                    // note (`b2 \glide ^1`, `finger-glide-spanner-*`).
                    return Vec::new();
                }
                if name == "bendAfter" {
                    let delta = match self.peek().cloned() {
                        Some(token) if token.is_scheme() => {
                            self.position += 1;
                            token.text.trim().parse().unwrap_or(0)
                        }
                        Some(token) => match token.as_integer() {
                            Some(value) => {
                                self.position += 1;
                                i8::try_from(value).unwrap_or(0)
                            }
                            None => 0,
                        },
                        None => 0,
                    };
                    return vec![PostEvent::BendAfter(delta)];
                }
                if let Some(mut post) = post_event_for_command(&name, Placement::Below) {
                    attach_dynamic_velocity(&mut post, &mut self.pending_dynamic_velocity);
                    return vec![post];
                }
                if is_articulation_command(&name) {
                    // Script connu (`ly/script-init.ly`) mais sans équivalent
                    // dans le modèle : le consommer seul. Passer par
                    // `parse_direction_event` avalait la commande suivante
                    // (`\downprall \times 2/3 { … }` perdait le n-olet).
                    return Vec::new();
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
            // `-"texte"` : ancienne écriture du doigté textuel, encore
            // acceptée (`lily/parser.yy` `post_event: '-' fingering` ;
            // `fingering` ne prend qu'un `UNSIGNED`, mais
            // `post_event_nofinger: script_dir direction_reqd_event` et
            // `direction_reqd_event: gen_text_def`, or `gen_text_def` accepte
            // `STRING`, l. 3435-3445). Ne pas consommer la chaîne laissait le
            // `~` qui suivait sans note : `Lobt.ly`, `re2-"4--5" \\fermata
            // ~re4`, perdait sa liaison (attaque en trop à la mesure 4).
            if token.kind == Kind::Str {
                self.position += 1;
                return vec![PostEvent::Text(token.text.clone(), placement)];
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
                let text = text.trim_matches('"').to_owned();
                if text.is_empty() {
                    Vec::new()
                } else if let Some(name) = text.strip_prefix("scoretk-head-") {
                    match scoretk_lilypond_common::noteheads::group_from_name(name) {
                        Some(group) => vec![PostEvent::HeadExact(group)],
                        None => Vec::new(),
                    }
                } else if let Some(kind) = match text.strip_prefix("scoretk-tremolo-") {
                    Some("start") => Some(TremoloKind::Start),
                    Some("stop") => Some(TremoloKind::Stop),
                    Some("unmeasured") => Some(TremoloKind::Unmeasured),
                    _ => None,
                } {
                    vec![PostEvent::TremoloKindOverride(kind)]
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
                if name == "offset" {
                    // `-\offset propriété valeurs item` : le `-` en fait un
                    // tweak dirigé (`ly/music-functions-init.ly:1283-1317`).
                    // La propriété et les valeurs sont consommées, l'item
                    // (post-événement) suit dans la boucle.
                    self.parse_offset_arguments();
                    return Vec::new();
                }
                if name == "tag" {
                    // `-\tag #'part ^4` (`tag-filter.ly`) : le tag marque le
                    // post-événement qui suit. Sans consommation du nom, le
                    // `^4` restait dans le flux et répétait la note.
                    let _ = self.parse_tag_names();
                    return self.parse_post_event();
                }
                if let Some(mut post) = post_event_for_command(&name, placement) {
                    attach_dynamic_velocity(&mut post, &mut self.pending_dynamic_velocity);
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
            // `script_abbreviation` (`lily/parser.yy` l. 3469) : après `^` ou
            // `_`, un signe de ponctuation est lui-même une articulation
            // (`dash` + nom du signe). Sans cette branche, `d''16^.` laissait
            // le `.` dans le flux : le `4` du doigté suivant `^4` était alors
            // lu comme une durée de noire et rejouait l'accord, décalant la
            // suite du triolet.
            Kind::Punct('.') => {
                self.position += 1;
                vec![PostEvent::Articulation(ArticulationType::Staccato)]
            }
            Kind::Punct('!') => {
                self.position += 1;
                vec![PostEvent::Articulation(ArticulationType::Staccatissimo)]
            }
            Kind::Punct('>') => {
                self.position += 1;
                vec![PostEvent::Articulation(ArticulationType::Accent)]
            }
            Kind::Punct('-') => {
                self.position += 1;
                vec![PostEvent::Articulation(ArticulationType::Tenuto)]
            }
            Kind::Punct('^') => {
                self.position += 1;
                vec![PostEvent::Articulation(ArticulationType::Marcato)]
            }
            Kind::Punct('_') => {
                self.position += 1;
                vec![PostEvent::Articulation(ArticulationType::Portato)]
            }
            // `dashPlus` (signe `+`) n'a pas d'équivalent dans le modèle :
            // consommer le signe suffit à ne pas le laisser dans le flux.
            Kind::Punct('+') => {
                self.position += 1;
                Vec::new()
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

    /// Marques d'octave placées après la durée d'une note (`c8'`).
    ///
    /// `lily/parser.yy:3714-3742` (`pitch_or_music`, `erroneous_quotes`) les
    /// accepte avec un avertissement et les ajoute à l'octave écrite — ou à
    /// l'octave contrôlée si la note porte un `=` (`octave_check`).
    fn apply_trailing_octave_marks(&mut self, pitch: &mut PitchSpec, with_duration: bool) {
        let mut count = 0i32;
        while let Some(token) = self.peek() {
            let delta = match token.kind {
                Kind::Punct('\'') => 1,
                Kind::Punct(',') => -1,
                _ => break,
            };
            count += delta;
            self.position += 1;
        }
        if count == 0 {
            return;
        }
        if with_duration {
            self.warn("octave marks must precede duration");
        }
        pitch.explicit_octave = true;
        let delta = count.clamp(i32::from(i8::MIN), i32::from(i8::MAX)) as i8;
        if let Some(check) = pitch.octave_check.as_mut() {
            *check = check.saturating_add(delta);
        } else {
            pitch.octave = pitch.octave.saturating_add(delta);
        }
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
        let mut facteur_initial = Fraction::ONE;
        let mut duration = if let Some(token) = self.peek().cloned() {
            if token.is_command("breve") {
                self.position += 1;
                Some(Duration::new(DurationType::Breve))
            } else if token.is_command("maxima") {
                self.position += 1;
                // `scm/define-music-display-methods.scm:114` : `\maxima` a
                // log2 = -3, donc 8 rondes, contre 4 pour `\longa`. Le modèle
                // n'a pas de type « maxima » : on porte le facteur ×2 dans le
                // multiplicateur, qui devient aussi celui de la durée par
                // défaut (une note nue après `\maxima` vaut 8 rondes).
                facteur_initial = Fraction::new(2, 1);
                Some(Duration::new(DurationType::Long))
            } else if token.is_command("longa") {
                self.position += 1;
                Some(Duration::new(DurationType::Long))
            } else if token.kind == Kind::Command
                && self.duration_identifiers.contains_key(&token.text)
            {
                // `\wholeHalved` après une note : identifiant de durée Scheme
                // (`lily/duration-scheme.cc`), avec son facteur de
                // compression. La durée écrite suivante (ou son absence)
                // devient la durée par défaut, facteur compris.
                let (duree, facteur) = self.duration_identifiers[&token.text];
                self.position += 1;
                facteur_initial = facteur;
                Some(duree)
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
        let multiplier = multiplier.map_or(facteur_initial, |ecrit| ecrit * facteur_initial);
        self.default_multiplier = multiplier;
        // Aucun appelant ne distingue `None` de `Some(1)` (tous passent par
        // `map_or(ticks, …)` ou `unwrap_or(Fraction::ONE)`) : on conserve la
        // convention historique d'un multiplicateur absent pour un facteur
        // implicite de 1, plutôt que de renvoyer systématiquement `Some`.
        Some((
            duration,
            if multiplier == Fraction::ONE {
                None
            } else {
                Some(multiplier)
            },
        ))
    }

    /// Numéros d'un `\volta n, m` (entiers séparés par des virgules) ou d'une
    /// liste Scheme `#'(1 2)`. Lit les entiers de la forme Scheme.
    fn parse_volta_numbers(&mut self) -> Vec<u32> {
        let mut numbers = Vec::new();
        if let Some(Token {
            kind: Kind::Scheme,
            text,
            ..
        }) = self.peek().cloned()
        {
            self.position += 1;
            let mut courant = String::new();
            for caractere in text.chars() {
                if caractere.is_ascii_digit() {
                    courant.push(caractere);
                } else if !courant.is_empty() {
                    if let Ok(value) = courant.parse::<u32>() {
                        numbers.push(value);
                    }
                    courant.clear();
                }
            }
            if let Ok(value) = courant.parse::<u32>() {
                numbers.push(value);
            }
            return numbers;
        }
        while let Some(token) = self.peek().cloned() {
            if token.kind == Kind::Number {
                if let Some(value) = token.as_integer().and_then(|n| u32::try_from(n).ok()) {
                    numbers.push(value);
                }
                self.position += 1;
            } else if token.is_punct(',') {
                self.position += 1;
            } else {
                break;
            }
        }
        numbers
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
    #[expect(
        dead_code,
        reason = "ancien garde-fou : `parse_markup_text` gère désormais les accolades et les marqueurs privés"
    )]
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
            // Nom de la dernière commande de markup lue, pour décider si une
            // accolade qui suit est son argument ou le début de la musique.
            let mut derniere_commande: Option<String> = None;
            while let Some(token) = self.peek().cloned() {
                match token.kind {
                    Kind::Punct('{')
                        if derniere_commande
                            .as_deref()
                            .is_some_and(markup_prend_liste_entre_accolades) =>
                    {
                        // `lily/parser.yy:4160-4165` : une liste entre
                        // accolades qui suit une commande de markup est son
                        // argument (`markup_braced_list`), pas de la musique.
                        // La laisser en place créait une partition parasite
                        // (`\markup \right-column { "0" "0" }` dans un
                        // `\with` ou au niveau supérieur) et la vraie portée
                        // était écartée comme « partition supplémentaire ».
                        self.position += 1;
                        let mut profondeur = 1usize;
                        while !self.at_end() && profondeur > 0 {
                            if self.eat_punct('}') {
                                profondeur -= 1;
                            } else if self.eat_punct('{') {
                                profondeur += 1;
                            } else {
                                self.collect_markup_atom(&mut pieces);
                            }
                        }
                        // `\line { … }` est **un** markup complet : le jeton
                        // suivant appartient à la musique. Continuer la boucle
                        // avalait la durée nue qui suit (`\path`/`#'()` dans
                        // `trivial-path.ly` : le `4` de reprise disparaissait
                        // avec la note finale).
                        break;
                    }
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
                        derniere_commande = Some(token.text.clone());
                    }
                    Kind::Str => {
                        self.collect_markup_atom(&mut pieces);
                        derniere_commande = None;
                    }
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
                        derniere_commande = None;
                    }
                    Kind::Number => {
                        self.collect_markup_atom(&mut pieces);
                        derniere_commande = None;
                    }
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
                ref text,
                ..
            }) if text == "fontsize" => {
                // Notation Reference §8.3 : `\fontsize #n` décale la taille
                // de police. On retient `n` pour le modèle, sans changer le
                // texte collecté.
                self.position += 1;
                self.last_markup_fontsize =
                    self.peek().cloned().and_then(|token| match token.kind {
                        Kind::Scheme | Kind::Number => token
                            .text
                            .trim_start_matches('#')
                            .trim()
                            .parse::<f64>()
                            .ok(),
                        _ => None,
                    });
            }
            Some(Token {
                kind: Kind::Command,
                ref text,
                ..
            }) if text == "bold" => {
                self.position += 1;
                if let Some(field) = self.current_header_field.clone() {
                    if !self.header_bold_fields.contains(&field) {
                        self.header_bold_fields.push(field);
                    }
                }
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
        // Valeur signée `-2`/`+2` : le signe et le nombre forment une seule
        // valeur (`\once \override Fingering.Y-offset = -2`). Ne consommer
        // que le signe laissait le `2` dans la musique, qui devenait une
        // durée et allongeait l'accord (`fingering-y-offset-new.ly`).
        if self
            .peek()
            .is_some_and(|token| token.is_punct('-') || token.is_punct('+'))
            && self
                .peek_at(1)
                .is_some_and(|token| token.kind == Kind::Number || token.is_scheme())
        {
            self.position += 2;
            return;
        }
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
            // Ancienne syntaxe `\override Context #'prop = valeur` (avant
            // 2.18, `python/convertrules.py`) : le nom de propriété est un
            // symbole Scheme quoté, que le lexer livre en un seul jeton
            // (`'expand-limit`). Ne pas le consommer laissait `= 1` dans la
            // musique : le `1` devenait une durée de blanche ronde et
            // décalait la suite de quatre noires (Symphony5_1/2,
            // `\override MultiMeasureRest #'expand-limit = 1`).
            if let Kind::Scheme = token.kind {
                let Some(symbol) = token.text.trim_start().strip_prefix('\'') else {
                    break;
                };
                let symbol = symbol.trim();
                if symbol.is_empty()
                    || !symbol.chars().all(|caractere| {
                        caractere.is_alphanumeric() || matches!(caractere, '-' | '_')
                    })
                {
                    break;
                }
                parts.push(symbol.to_owned());
                self.position += 1;
                if !self.eat_punct('.') {
                    break;
                }
                continue;
            }
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
                // `Context #'prop` : le nom quoté suit le contexte sans
                // point ; la boucle le lit à l'itération suivante.
                let suite_scheme = self.peek().is_some_and(|suivant| {
                    suivant.kind == Kind::Scheme && suivant.text.trim_start().starts_with('\'')
                });
                if !suite_scheme {
                    break;
                }
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
                    // `nom = …` est une affectation de variable musicale, pas
                    // un argument. `\tieUp` (commande sans argument que le
                    // lecteur ignore) avalait le `td` de `td = …` : les
                    // definitions suivantes disparaissaient, dont `su`
                    // (`\change Staff = up`) dans bwv798.
                    if self.peek_at(1).is_some_and(|next| next.is_punct('=')) {
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

/// Reconnaît un identifiant de durée Scheme `#(ly:make-duration …)`.
///
/// `lily/duration-scheme.cc:53-99` : le premier argument est le logarithme
/// base 2 de la durée (−1 = brève, −2 = longue, 0 = ronde, 1 = blanche…),
/// le second le nombre de points, puis vient un facteur de compression
/// (`num/den` ou deux entiers). Le facteur est porté à part, comme
/// `TDuration::compressed()` du C++.
/// Langue des noms de notes posée par `#(ly:parser-set-note-names
/// (ly:assoc-get '<lang> language-pitch-names))` (`note-names.ly`,
/// `lily/parser-scheme.cc`).
/// `#(set-global-staff-size n)` : la taille globale en points, d'après la
/// Notation Reference 2.26 §27.2 (20 pt par défaut, 7,03 mm de haut).
fn scheme_staff_size(datum: &str) -> Option<f64> {
    let words: Vec<&str> = datum.split_whitespace().collect();
    // Le lexeur retire le `#` initial du datum Scheme.
    if words.first().copied() != Some("(set-global-staff-size") {
        return None;
    }
    words.get(1)?.trim_end_matches(')').parse::<f64>().ok()
}

fn scheme_note_names_language(datum: &str) -> Option<Language> {
    if !datum.contains("ly:parser-set-note-names") || !datum.contains("language-pitch-names") {
        return None;
    }
    for mot in datum.split_whitespace() {
        if !mot.starts_with('\'') {
            continue;
        }
        let nom = mot
            .trim_start_matches('\'')
            .trim_end_matches(|c: char| !c.is_alphanumeric());
        if let Some(langage) = Language::from_name(nom) {
            return Some(langage);
        }
    }
    None
}

fn scheme_duration_definition(datum: &str) -> Option<(Duration, Fraction)> {
    let inner = datum.trim().strip_prefix('(')?.strip_suffix(')')?;
    let mots: Vec<&str> = inner.split_whitespace().collect();
    if mots.first().map(|mot| mot.trim_start_matches('(')) != Some("ly:make-duration") {
        return None;
    }
    let log: i32 = mots.get(1)?.parse().ok()?;
    let points: u8 = mots.get(2).and_then(|mot| mot.parse().ok()).unwrap_or(0);
    let facteur = match (mots.get(3), mots.get(4)) {
        (Some(rational), _) if rational.contains('/') => {
            let (num, den) = rational.trim_end_matches(')').split_once('/')?;
            Fraction::try_new(num.parse().ok()?, den.parse().ok()?)?
        }
        (Some(num), Some(den)) => {
            Fraction::try_new(num.parse().ok()?, den.trim_end_matches(')').parse().ok()?)?
        }
        (Some(entier), _) => Fraction::from_integer(entier.trim_end_matches(')').parse().ok()?),
        _ => Fraction::ONE,
    };
    // Le modèle n'a pas de type « maxima » : log2 = -3 est une longue
    // multipliée par deux, comme dans `parse_duration`.
    let facteur = if log == -3 {
        facteur * Fraction::new(2, 1)
    } else {
        facteur
    };
    let kind = match log {
        -3 | -2 => DurationType::Long,
        -1 => DurationType::Breve,
        0 => DurationType::Whole,
        1 => DurationType::Half,
        2 => DurationType::Quarter,
        3 => DurationType::Eighth,
        4 => DurationType::Sixteenth,
        5 => DurationType::ThirtySecond,
        6 => DurationType::SixtyFourth,
        7 => DurationType::OneHundredTwentyEighth,
        _ => return None,
    };
    let mut duree = Duration::new(kind);
    duree.dots = points;
    Some((duree, facteur))
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
    // `lily/parser.yy`, `function_arglist_optional`: a predicate wrapped in
    // parentheses carries the value used when the argument is omitted. The
    // default is stored as tokens (not as a Scheme value) so that the body is
    // re-parsed exactly as LilyPond does with the substituted argument.
    let optional_defaults = predicates
        .iter()
        .map(|predicate| optional_argument_default(predicate))
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
    // Corps Scheme réduit à un paramètre (`#(define-music-function (text music)
    // (markup? ly:music?) music)`) : la fonction renvoie cette musique. Sans
    // cela elle passait pour une fonction sans corps et ne produisait rien
    // (`music-function-string-markup.ly`).
    let returns = if body.is_none() && kind == FunctionKind::Music {
        scheme_body_parameter(&body_source, &params)
    } else {
        None
    };
    Some(MusicFunction {
        kind,
        params,
        predicates,
        optional_defaults,
        body,
        returns,
        relative_body,
        warned: false,
    })
}

/// Tokens used when an optional music-function argument is absent.
///
/// `lily/parser.yy` (`function_arglist_optional`) and
/// `scm/music-functions.scm` use `(type? *unspecified*)` and
/// `(type? #{ default #})`. `#{ … #}` is an ordinary LilyPond music
/// expression, so it is tokenised like a small source fragment.
fn optional_argument_default(predicate: &str) -> Option<Vec<Token>> {
    let trimmed = predicate.trim();
    if !trimmed.starts_with('(') {
        return None;
    }
    if trimmed.contains("*unspecified*") {
        return Some(Vec::new());
    }
    crate::lexer::embedded_music_body(trimmed)
        .as_deref()
        .map(crate::lexer::tokenize)
}

/// Type predicate inside an optional predicate such as
/// `((ly:pitch? #{ f #})`: strips the wrapper parentheses so that the
/// existing argument reader recognises the type.
fn predicate_type(predicate: &str) -> &str {
    let mut text = predicate.trim();
    while let Some(inner) = text.strip_prefix('(') {
        text = inner.trim_start();
    }
    text.split(|character: char| character.is_whitespace() || character == ')' || character == '#')
        .next()
        .unwrap_or(text)
}

/// Indice du paramètre renvoyé par une fonction dont la **dernière** forme du
/// corps Scheme est un nom de paramètre (`music`, `note`), `None` sinon.
///
/// Les formes qui précèdent (mise en garde, `set!` d'une propriété de grob)
/// ne produisent pas de musique : la valeur de la fonction est sa dernière
/// expression (`#(define-music-function (note) (ly:music?) … note)`,
/// `approximatePitch`, `ly/music-functions-init.ly:239-246`).
fn scheme_body_parameter(body_source: &str, params: &[String]) -> Option<usize> {
    use crate::lexer::{scheme_string_end, skip_scheme_block_comment};
    let characters: Vec<char> = body_source.chars().collect();
    let mut index = 0;
    let mut last: Option<String> = None;
    while index < characters.len() {
        index = skip_scheme_space(&characters, index);
        let Some(&character) = characters.get(index) else {
            break;
        };
        match character {
            ')' | ']' => break,
            '(' | '[' => {
                // Sauter la forme (liste) équilibrée.
                let mut depth = 0usize;
                while let Some(&current) = characters.get(index) {
                    match current {
                        '(' | '[' => {
                            depth += 1;
                            index += 1;
                        }
                        ')' | ']' => {
                            depth = depth.saturating_sub(1);
                            index += 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        '"' => index = scheme_string_end(&characters, index),
                        ';' => {
                            while characters.get(index).is_some_and(|c| *c != '\n') {
                                index += 1;
                            }
                        }
                        '#' if characters.get(index + 1) == Some(&'|') => {
                            index = skip_scheme_block_comment(&characters, index);
                        }
                        _ => index += 1,
                    }
                }
                last = None;
            }
            '"' => {
                index = scheme_string_end(&characters, index);
                last = None;
            }
            ';' => {
                while characters.get(index).is_some_and(|c| *c != '\n') {
                    index += 1;
                }
            }
            '#' if characters.get(index + 1) == Some(&'|') => {
                index = skip_scheme_block_comment(&characters, index);
                last = None;
            }
            _ => {
                let (word, next) = read_scheme_word(&characters, index);
                if word.is_empty() {
                    index += 1;
                    last = None;
                } else {
                    index = next;
                    last = Some(word);
                }
            }
        }
    }
    let word = last?;
    params.iter().position(|param| *param == word)
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

/// Vrai si l'élément est un contexte `Voice` (`\new Voice` ou
/// `\context Voice`), seul contexte qui s'imbrique dans une séquence sans
/// ouvrir de portée.
fn is_voice_context(music: &Music) -> bool {
    match music {
        Music::New(group) => group.kind == GroupKind::Voice,
        Music::Tag { music, .. } => is_voice_context(music),
        _ => false,
    }
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
    Relative(Option<PitchSpec>, bool),
    /// `\unfoldRepeats` : enveloppe transparente pour la structure, mais
    /// porte le dépliage des reprises de son argument.
    UnfoldRepeats,
    /// `\harmonicByFret` / `\harmonicByRatio` : décalage ajouté aux
    /// hauteurs déjà résolues (`scm/tablature.scm:351-366`).
    Harmonic(i8),
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
            Context::Relative {
                reference,
                music,
                chain,
            } => Some((Self::Relative(*reference, *chain), music)),
            Context::UnfoldRepeats { music } => Some((Self::UnfoldRepeats, music)),
            Context::Harmonic { semitones, music } => Some((Self::Harmonic(*semitones), music)),
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
            Self::Relative(reference, chain) => Context::Relative {
                reference,
                chain,
                music,
            },
            Self::UnfoldRepeats => Context::UnfoldRepeats { music },
            Self::Harmonic(semitones) => Context::Harmonic { semitones, music },
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
    // `\relative` appliqué à un groupe de portées : `to_relative_octave`
    // parcourt leurs voix en séquence (`music_list_to_relative`,
    // `lily/music-sequence.cc`), la première note d'une portée se résolvant
    // depuis la dernière de la précédente. `chain` marque cette continuité
    // (`bar-line-define-bar-glyph.ly` : `\relative \new StaffGroup << … >>`).
    let wrappers: Vec<Wrapper> = wrappers
        .iter()
        .map(|wrapper| match wrapper {
            Wrapper::Relative(reference, _) => Wrapper::Relative(*reference, true),
            other => *other,
        })
        .collect();
    for staff in staves {
        for voice in &mut staff.voices {
            let music = std::mem::replace(&mut voice.music, Music::Empty);
            voice.music = wrap_all(&wrappers, music);
        }
    }
}

/// Vrai si la musique contient un contexte enfant de portée qui ouvre sa
/// propre piste (`\context ChordNames`, `\new Dynamics`, `\new Staff` dans
/// un groupe).
///
/// Un `\new Voice` reste une voix de la portée. Dans un **vrai** `Staff`, un
/// `\context Staff` imbriqué ne crée pas de portée non plus : `\context`
/// **retrouve** le contexte existant (`Context::find_create_context`,
/// `lily/context.cc:269-272`), donc le Staff englobant ; le traiter comme une
/// portée sœur dédoublait les portées de `air-tromb.ly` (12 au lieu de 4).
/// Dans un contexte de groupe inconnu (`SemiChoirStaff` défini par un
/// `\layout { \context { \ChoirStaff \name "SemiChoirStaff" } }`),
/// `group_like` est vrai : ses `\new Staff` enfants sont les portées du
/// groupe (`Nunc-dimittis`, `Mendelssohn-BartholdyF_op_23_No_1`).
fn has_staff_context(music: &Music, group_like: bool, anonyme: bool) -> bool {
    match music {
        Music::New(group) => {
            group.kind == GroupKind::ChordNames
                || group.kind == GroupKind::Other
                || (group_like && group.kind == GroupKind::Staff)
        }
        Music::Sim(items) => items.iter().any(|item| {
            // `\new Staff` à l'intérieur d'un `<< … >>` est créé dans le
            // contexte parent (Staff n'accepte pas Staff, `lily/context.cc`
            // `context_create`) : c'est une portée sœur avec sa propre piste
            // MIDI (`adding-an-extra-staff.ly`, LSR). `\new Dynamics` est
            // représenté comme `GroupKind::Staff` mais marqué `muted` : il
            // reste une portée silencieuse et ne doit pas déclencher ce
            // partage.
            let nouveau_staff = matches!(
                item,
                Music::New(group)
                    if group.kind == GroupKind::Staff
                    && !group.staves.first().is_some_and(|staff| {
                        staff.muted || staff.name.is_some()
                    })
            );
            (nouveau_staff && anonyme && !group_like)
                || has_staff_context(item, group_like, anonyme)
        }),
        Music::Seq(items) => items
            .iter()
            .any(|item| has_staff_context(item, group_like, anonyme)),
        Music::Tag { music, .. } | Music::Filtered { music, .. } => {
            has_staff_context(music, group_like, anonyme)
        }
        Music::Context(context) => Wrapper::of(context)
            .is_some_and(|(_, inner)| has_staff_context(inner, group_like, anonyme)),
        _ => false,
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
        sequence_start: false,
        instrument_name: None,
        short_instrument_name: None,
        midi_instrument: None,
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
            // `parse_command` a une branche explicite pour `\simultaneous`
            // (bloc simultané) ; le garder ici le fait reconnaître comme
            // musique au niveau supérieur (`at_toplevel_music`).
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

/// Ajoute un point à chaque durée d'une musique.
///
/// `ly/ancient-init.ly:124-130` : `\augmentum` applique
/// `shift-duration-log expr 0 1` au changement de durée uniquement, en
/// laissant le log (la figure) intact, ce qui ajoute une mora de chant
/// grégorien à chaque note.
/// Retire les `\\alternative` imbriqués dans un corps de reprise etcollecte leurs
/// endings dans `alternatives` (`lily/parser.yy` `REPEAT ... alternative`).
fn extract_alternatives(music: &mut Music, alternatives: &mut Vec<Music>) {
    match music {
        Music::Seq(items) | Music::Sim(items) => {
            let mut index = 0;
            while index < items.len() {
                if let Music::Context(Context::Alternative(blocks)) = &mut items[index] {
                    alternatives.append(blocks);
                    items.remove(index);
                } else {
                    extract_alternatives(&mut items[index], alternatives);
                    index += 1;
                }
            }
        }
        Music::Context(Context::Relative { music, .. })
        | Music::Context(Context::Fixed { music, .. })
        | Music::Context(Context::NonRelative { music })
        | Music::Context(Context::Cue { music, .. })
        | Music::Context(Context::Volta { music, .. })
        | Music::Context(Context::Tuplet { music, .. })
        | Music::Context(Context::Transpose { music, .. })
        | Music::Context(Context::Harmonic { music, .. })
        | Music::Context(Context::AfterGrace { main: music, .. })
        | Music::Context(Context::After { music, .. })
        | Music::Context(Context::Voices { music, .. }) => {
            extract_alternatives(music, alternatives);
        }
        Music::Tag { music, .. } | Music::Filtered { music, .. } => {
            extract_alternatives(music, alternatives);
        }
        _ => {}
    }
}

/// Duree d'une expression musicale, en ticks de ronde.
fn music_duration(music: &Music, default: Duration) -> Fraction {
    match music {
        Music::Chord(chord) => {
            chord.duration.unwrap_or(default).ticks() * chord.multiplier.unwrap_or(Fraction::ONE)
        }
        Music::Seq(items) => items
            .iter()
            .map(|item| music_duration(item, default))
            .fold(Fraction::ZERO, |total, d| total + d),
        Music::Sim(items) => items
            .iter()
            .map(|item| music_duration(item, default))
            .fold(Fraction::ZERO, |total, d| if d > total { d } else { total }),
        // `\repeat unfold n` (et `volta`, déplié dans le MIDI) : la durée est
        // celle du corps multipliée par le nombre de passages. Sans ce cas, la
        // durée de l'argument de `\cueDuring` était fausse quand la citation
        // contenait un repeat (`cue-clef*.ly`).
        Music::Context(Context::Repeat { count, body, .. }) => {
            music_duration(body, default) * Fraction::from_integer(*count as i64)
        }
        Music::Context(context) => music_duration(context_music(context), default),
        Music::Tag { music, .. } | Music::Filtered { music, .. } => music_duration(music, default),
        _ => Fraction::ZERO,
    }
}

/// Musique contenue dans un contexte, pour le calcul de duree des citations.
fn context_music(context: &Context) -> &Music {
    match context {
        Context::Relative { music, .. }
        | Context::Fixed { music, .. }
        | Context::NonRelative { music }
        | Context::Tuplet { music, .. }
        | Context::Transpose { music, .. }
        | Context::Harmonic { music, .. }
        | Context::Skipped { music } => music,
        _ => &Music::Empty,
    }
}

fn ajoute_point(music: Music, default_duration: Duration) -> Music {
    match music {
        Music::Chord(mut chord) => {
            // Une note nue n'a pas de figure propre dans l'AST : la reprendre
            // à la durée par défaut du parseur, comme `lily/parser.yy` le fait
            // avant d'appliquer `shift-duration-log`.
            let mut duration = chord.duration.unwrap_or(default_duration);
            duration.dots = duration.dots.saturating_add(1);
            chord.duration = Some(duration);
            Music::Chord(chord)
        }
        Music::Seq(items) => Music::Seq(
            items
                .into_iter()
                .map(|item| ajoute_point(item, default_duration))
                .collect(),
        ),
        Music::Sim(items) => Music::Sim(
            items
                .into_iter()
                .map(|item| ajoute_point(item, default_duration))
                .collect(),
        ),
        Music::Tag { tags, music } => Music::Tag {
            tags,
            music: Box::new(ajoute_point(*music, default_duration)),
        },
        Music::Filtered {
            filter,
            wanted,
            groups,
            music,
        } => Music::Filtered {
            filter,
            wanted,
            groups,
            music: Box::new(ajoute_point(*music, default_duration)),
        },
        other => other,
    }
}

/// Rend une musique silencieuse en conservant ses durées.
///
/// `ly/music-functions-init.ly:2267-2274` : `\skip <musique>` construit un
/// `SkippedMusic`, dont l'élément est traversé sans produire de notes, mais
/// en avançant le temps de ses accord. Chaque accord devient donc un
/// `SkipNumeric` de même durée. Limite : les contextes imbriqués ne sont pas
/// parcourus (le temps qu'ils portent est ignoré).
/// `\killCues` (`ly/music-functions-init.ly:955-964`) : retire les notes de
/// cue insérées par `\cueDuring` en conservant leur durée (silences) ; les
/// citations de `\quoteDuring` (qui n'ont pas `quoted-context-id "cue"`)
/// restent jouées.
fn kill_cues(music: Music) -> Music {
    match music {
        // Le silence garde la durée de l'argument principal (la citation
        // n'est plus tronquée dans l'AST).
        Music::Context(Context::Cue {
            cue_notes: true,
            duration,
            ..
        }) => Music::Chord(Chord {
            kind: ChordKind::SkipNumeric,
            duration: Some(Duration::whole()),
            multiplier: if duration == Fraction::ONE {
                None
            } else {
                Some(duration)
            },
            from_scale: false,
            bracketed: false,
            posts: Vec::new(),
            invisible: true,
        }),
        Music::Context(Context::Cue {
            music,
            cue_notes: false,
            duration,
        }) => Music::Context(Context::Cue {
            music: Box::new(kill_cues(*music)),
            cue_notes: false,
            duration,
        }),
        Music::Seq(items) => Music::Seq(items.into_iter().map(kill_cues).collect()),
        Music::Sim(items) => Music::Sim(items.into_iter().map(kill_cues).collect()),
        Music::Tag { tags, music } => Music::Tag {
            tags,
            music: Box::new(kill_cues(*music)),
        },
        Music::Filtered {
            filter,
            wanted,
            groups,
            music,
        } => Music::Filtered {
            filter,
            wanted,
            groups,
            music: Box::new(kill_cues(*music)),
        },
        Music::New(mut group) => {
            for staff in &mut group.staves {
                for voice in &mut staff.voices {
                    voice.music = kill_cues(std::mem::replace(&mut voice.music, Music::Empty));
                }
            }
            Music::New(group)
        }
        Music::Context(Context::Relative {
            reference,
            chain,
            music,
        }) => Music::Context(Context::Relative {
            reference,
            chain,
            music: Box::new(kill_cues(*music)),
        }),
        Music::Context(Context::Fixed { reference, music }) => Music::Context(Context::Fixed {
            reference,
            music: Box::new(kill_cues(*music)),
        }),
        Music::Context(Context::Transpose {
            from,
            to,
            absolute,
            music,
        }) => Music::Context(Context::Transpose {
            from,
            to,
            absolute,
            music: Box::new(kill_cues(*music)),
        }),
        Music::Context(Context::NonRelative { music }) => Music::Context(Context::NonRelative {
            music: Box::new(kill_cues(*music)),
        }),
        Music::Context(Context::Invisible { music }) => Music::Context(Context::Invisible {
            music: Box::new(kill_cues(*music)),
        }),
        Music::Context(Context::Tuplet {
            numerator,
            denominator,
            number_type,
            music,
        }) => Music::Context(Context::Tuplet {
            numerator,
            denominator,
            number_type,
            music: Box::new(kill_cues(*music)),
        }),
        Music::Context(Context::Grace { kind, slash, music }) => Music::Context(Context::Grace {
            kind,
            slash,
            music: Box::new(kill_cues(*music)),
        }),
        Music::Context(Context::AfterGrace {
            main,
            grace,
            fraction,
        }) => Music::Context(Context::AfterGrace {
            main: Box::new(kill_cues(*main)),
            grace: Box::new(kill_cues(*grace)),
            fraction,
        }),
        Music::Context(Context::After {
            delta,
            event,
            music,
        }) => Music::Context(Context::After {
            delta,
            event: Box::new(kill_cues(*event)),
            music: Box::new(kill_cues(*music)),
        }),
        Music::Context(Context::Voices { ids, music }) => Music::Context(Context::Voices {
            ids,
            music: Box::new(kill_cues(*music)),
        }),
        Music::Context(Context::Repeat {
            kind,
            count,
            body,
            alternatives,
            partial,
            count_missing,
        }) => Music::Context(Context::Repeat {
            kind,
            count,
            body: Box::new(kill_cues(*body)),
            alternatives: alternatives.into_iter().map(kill_cues).collect(),
            partial,
            count_missing,
        }),
        Music::Context(Context::Alternative(items)) => Music::Context(Context::Alternative(
            items.into_iter().map(kill_cues).collect(),
        )),
        Music::Context(Context::Volta { numbers, music }) => Music::Context(Context::Volta {
            numbers,
            music: Box::new(kill_cues(*music)),
        }),
        other => other,
    }
}

fn silence_music(music: Music) -> Music {
    match music {
        Music::Chord(chord) => Music::Chord(Chord {
            kind: ChordKind::SkipNumeric,
            posts: Vec::new(),
            invisible: true,
            ..chord
        }),
        Music::Seq(items) => Music::Seq(items.into_iter().map(silence_music).collect()),
        Music::Sim(items) => Music::Sim(items.into_iter().map(silence_music).collect()),
        Music::Tag { tags, music } => Music::Tag {
            tags,
            music: Box::new(silence_music(*music)),
        },
        Music::Filtered {
            filter,
            wanted,
            groups,
            music,
        } => Music::Filtered {
            filter,
            wanted,
            groups,
            music: Box::new(silence_music(*music)),
        },
        // `\skip \musique` et `\killCues` : une musique enveloppée
        // (`\relative`, `\transpose`, reprise…) doit garder sa durée. La
        // remplacer par `Empty` faisait disparaître le temps occupé
        // (`quote-kill-cues` : la note suivante arrivait une mesure trop tôt).
        Music::Context(Context::Relative {
            reference,
            chain,
            music,
        }) => Music::Context(Context::Relative {
            reference,
            chain,
            music: Box::new(silence_music(*music)),
        }),
        Music::Context(Context::Fixed { reference, music }) => Music::Context(Context::Fixed {
            reference,
            music: Box::new(silence_music(*music)),
        }),
        Music::Context(Context::Transpose {
            from,
            to,
            absolute,
            music,
        }) => Music::Context(Context::Transpose {
            from,
            to,
            absolute,
            music: Box::new(silence_music(*music)),
        }),
        Music::Context(Context::NonRelative { music }) => Music::Context(Context::NonRelative {
            music: Box::new(silence_music(*music)),
        }),
        Music::Context(Context::Invisible { music }) => Music::Context(Context::Invisible {
            music: Box::new(silence_music(*music)),
        }),
        Music::Context(Context::Volta { numbers, music }) => Music::Context(Context::Volta {
            numbers,
            music: Box::new(silence_music(*music)),
        }),
        Music::Context(Context::Alternative(items)) => Music::Context(Context::Alternative(
            items.into_iter().map(silence_music).collect(),
        )),
        Music::Context(Context::Repeat {
            kind,
            count,
            body,
            alternatives,
            partial,
            count_missing,
        }) => Music::Context(Context::Repeat {
            kind,
            count,
            body: Box::new(silence_music(*body)),
            alternatives: alternatives.into_iter().map(silence_music).collect(),
            partial,
            count_missing,
        }),
        Music::Context(Context::Tuplet {
            numerator,
            denominator,
            number_type,
            music,
        }) => Music::Context(Context::Tuplet {
            numerator,
            denominator,
            number_type,
            music: Box::new(silence_music(*music)),
        }),
        Music::Context(Context::Grace { kind, slash, music }) => Music::Context(Context::Grace {
            kind,
            slash,
            music: Box::new(silence_music(*music)),
        }),
        Music::Context(Context::AfterGrace {
            main,
            grace,
            fraction,
        }) => Music::Context(Context::AfterGrace {
            main: Box::new(silence_music(*main)),
            grace: Box::new(silence_music(*grace)),
            fraction,
        }),
        Music::Context(Context::After {
            delta,
            event,
            music,
        }) => Music::Context(Context::After {
            delta,
            event: Box::new(silence_music(*event)),
            music: Box::new(silence_music(*music)),
        }),
        Music::Context(Context::Voices { ids, music }) => Music::Context(Context::Voices {
            ids,
            music: Box::new(silence_music(*music)),
        }),
        _ => Music::Empty,
    }
}

/// Commandes de markup dont l'argument peut être une liste entre accolades.
///
/// Listées d'après les signatures de `scm/define-markup-commands.scm` :
/// `(markup-list?)`, `(ly:music?)` pour `rhythm` et `(ly:duration?)` pour
/// `note`. Une accolade qui suit une autre commande (`\foo` sans argument,
/// `\etc` partiel) ouvre au contraire la musique : la consommer ferait
/// disparaître la partition (`markup-invalid-stencil.ly`, `textetc.ly`).
fn markup_prend_liste_entre_accolades(name: &str) -> bool {
    matches!(
        name,
        "line"
            | "rhythm"
            | "note"
            | "first-visible"
            | "fill-line"
            | "justify-line"
            | "concat"
            | "justify"
            | "wordwrap"
            | "wordwrap-center"
            | "overlay"
            | "column"
            | "dir-column"
            | "center-column"
            | "left-column"
            | "right-column"
            | "center-align"
            // Commandes de style : elles prennent un argument markup, qui
            // peut être une liste entre accolades (`lily/parser.yy`
            // `markup_braced_list`). `\markup \bold {a Tempo}` laissait la
            // liste dans le flux musical : `a` (la note la) était jouée et
            // la suite décalée (franck_ave_maria, fin de la main droite).
            | "bold"
            | "italic"
            | "bold-italic"
            | "underline"
            | "box"
            | "circle"
            | "rounded-box"
            | "smallCaps"
            | "large"
            | "small"
            | "smaller"
            | "huge"
            | "tiny"
            | "whiteout"
            | "transparent"
    )
}

/// Partiel associé à un frettage par `scm/tablature.scm:308-320`
/// (`fret-partials`), puis hauteur ajoutée par `partial-pitch`
/// (`scm/tablature.scm:297-306`) : (octaves, degré, altération).
fn harmonic_fret_pitch(fret: &str) -> (i8, u8, i8) {
    let partial = match fret {
        "0" => 0,
        "12" => 1,
        "7" | "19" => 2,
        "5" | "24" => 3,
        "4" | "9" | "16" => 4,
        "3" => 5,
        "2.7" => 6,
        "2.3" => 7,
        "2" => 8,
        _ => 0,
    };
    match partial {
        0 => (0, 0, 0),
        1 => (1, 0, 0),
        2 => (1, 4, 0),
        3 => (2, 0, 0),
        4 => (2, 2, 0),
        5 => (2, 4, 0),
        6 => (2, 6, 0),
        7 => (3, 0, 0),
        _ => (3, 1, 0),
    }
}

fn starts_music_command(name: &str) -> bool {
    is_mode_head(name)
        || matches!(
            name,
            "relative"
                | "fixed"
                | "absolute"
                | "autoChange"
                | "partCombine"
                | "unfoldRepeats"
                | "harmonicByFret"
                | "transpose"
                | "tuplet"
                | "times"
                | "scaleDurations"
                | "compressMusic"
                | "grace"
                | "slashedGrace"
                | "acciaccatura"
                | "appoggiatura"
                | "afterGrace"
                | "repeat"
                | "alternative"
                | "new"
                // `\context Staff …` est le jumeau de `\new Staff` au niveau
                // supérieur : sans lui, la portée demandée était sautée et le
                // `\relative << { } \\ { } >>` qui suit était lu comme
                // plusieurs portées (idiome Mutopia), perdant la polyphonie
                // (`collision-heads`, `cluster`…).
                | "context"
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
                || token.text == "("
                || token.text == ")"
                || token.text == "rest"
                || token.text == "bendAfter"
                || token.text == "single"
                // `\5` : numéro de corde, post-événement sans argument
                // (`stringNumber`, `ly/declarations-init.ly`).
                || (!token.text.is_empty()
                    && token.text.chars().all(|c| c.is_ascii_digit()))
                // `\shape`/`\vshape` entre une note et ses post-événements
                // (`c1 \vshape #'(…) ~ (`, `vshape.ly`) : consommer les
                // décalages et laisser la boucle lire `~` et `(`.
                || token.text == "shape"
                || token.text == "vshape"
                // `\tweak` est aussi un post-événement (`lily/parser.yy`) :
                // `b2 \tweak color #red ^1` modifie l'événement qui suit.
                // `parse_post_event` le transforme en `TweakHead` pour notre
                // écrivain, qui le place après la note décorée.
                || token.text == "tweak"
                // `glide = #(make-music 'FingerGlideEvent)`
                // (`ly/declarations-init.ly:79`) : post-événement sans effet
                // MIDI. Le laisser hors du post-événement cassait la suite
                // (`b2 \glide ^1` dupliquait la note).
                || token.text == "glide"
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

/// `instrumentTransposition` d'une définition d'instrument
/// (`ly/music-functions-init.ly:46-54`). La valeur est une paire Scheme
/// `#`((instrumentTransposition . ,(ly:make-pitch -1 6 FLAT)) …)` : LilyPond
/// range l'octave interne `-1` à la même hauteur que `\transposition bes`
/// dans notre representation (`pitch::instrument_transpose` retranche 12).
fn instrument_transposition_datum(datum: &str) -> Option<PitchSpec> {
    let cle = "instrumentTransposition";
    let debut_cle = datum.find(cle)? + cle.len();
    let reste = &datum[debut_cle..];
    let debut_pitch = reste.find("(ly:make-pitch")?;
    let reste = &reste[debut_pitch..];
    let fin = reste.find(')')?;
    let mots: Vec<&str> = reste[1..fin].split_whitespace().collect();
    if mots.first() != Some(&"ly:make-pitch") {
        return None;
    }
    let octave: i8 = mots.get(1)?.parse().ok()?;
    let step: u8 = mots.get(2)?.parse().ok()?;
    let alteration = match mots.get(3).map(|mot| mot.to_ascii_uppercase()) {
        Some(mot) if mot.contains("DOUBLE-FLAT") => -2,
        Some(mot) if mot.contains("DOUBLE-SHARP") => 2,
        Some(mot) if mot.contains("FLAT") => -1,
        Some(mot) if mot.contains("SHARP") => 1,
        _ => 0,
    };
    Some(PitchSpec {
        step,
        alter: alteration,
        octave: octave.checked_add(1)?,
        octave_check: None,
        explicit_octave: true,
        force: false,
        cautionary: false,
    })
}

/// Compte arithmétique d'une répétition abrégée, par exemple `#(+ 4 32)`.
///
/// `\*` et `\%` sont des fonctions musicales dont l'argument est de type
/// `number?` (`ly/music-functions-init.ly:742-751`) : le lexer amont lit un
/// **datum Scheme entier** (`lily/lexer.ll:407-419`, `parse_embedded_scheme`
/// de `lily/parse-scm.cc:206-307`), et Guile évalue `(+ 4 32)` avant de
/// l'envoyer à la fonction. Le lecteur ne dispose pas d'évaluateur Scheme
/// (`docs/agents/lilypond-scheme.md` §8) ; on reconnaît donc uniquement les
/// appels arithmétiques sur des littéraux entiers, qui suffisent à
/// `beat-performer.ly`. Toute autre forme retombe sur le compte par défaut.
fn scheme_integer_count(datum: &str) -> Option<i64> {
    /// Découpe une liste Scheme en atomes en respectant les parenthèses.
    fn atoms(list: &str) -> Vec<&str> {
        let mut result = Vec::new();
        let mut depth = 0usize;
        let mut start = 0usize;
        for (index, character) in list.char_indices() {
            match character {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                c if c.is_whitespace() && depth == 0 => {
                    if start < index {
                        result.push(&list[start..index]);
                    }
                    start = index + c.len_utf8();
                }
                _ => {}
            }
        }
        if start < list.len() {
            result.push(&list[start..]);
        }
        result
    }

    let text = datum.trim();
    let inner = text.strip_prefix('(')?.strip_suffix(')')?;
    let parts = atoms(inner);
    let (operator, operands) = parts.split_first()?;
    if operands.is_empty() {
        return None;
    }
    let mut values = Vec::with_capacity(operands.len());
    for operand in operands {
        let value = operand
            .parse::<i64>()
            .ok()
            .or_else(|| scheme_integer_count(operand))?;
        values.push(value);
    }
    match *operator {
        "+" => values.into_iter().try_fold(0i64, i64::checked_add),
        "-" => {
            let (first, rest) = values.split_first()?;
            rest.iter()
                .try_fold(*first, |total, value| total.checked_sub(*value))
        }
        "*" => values.into_iter().try_fold(1i64, i64::checked_mul),
        // Le quotient d'un compte doit être entier (`(/ (+ 4 32) 2)` vaut 18).
        "/" => {
            let (first, rest) = values.split_first()?;
            rest.iter()
                .try_fold(*first, |total, value| total.checked_div(*value))
        }
        _ => None,
    }
}

/// `musicMap #skip->rest` : transformer les silences invisibles en silences
/// visibles, récursivement (ly/music-functions-init.ly:1241-1245).
fn skip_to_rest(music: &mut Music) {
    match music {
        Music::Seq(items) | Music::Sim(items) => {
            for item in items {
                skip_to_rest(item);
            }
        }
        Music::Chord(chord) if matches!(chord.kind, ChordKind::Skip | ChordKind::SkipNumeric) => {
            chord.kind = ChordKind::Rest;
            chord.invisible = false;
        }
        Music::Context(Context::Tuplet { music, .. })
        | Music::Context(Context::Grace { music, .. })
        | Music::Context(Context::Fixed { music, .. })
        | Music::Context(Context::Relative { music, .. })
        | Music::Context(Context::NonRelative { music })
        | Music::Context(Context::Transpose { music, .. })
        | Music::Context(Context::Harmonic { music, .. })
        | Music::Context(Context::Silent { music })
        | Music::Context(Context::Skipped { music })
        | Music::Context(Context::Cue { music, .. })
        | Music::Context(Context::Invisible { music }) => skip_to_rest(music),
        Music::Tag { music, .. } | Music::Filtered { music, .. } => skip_to_rest(music),
        _ => {}
    }
}

/// Applique `\featherDurations` (ly/music-functions-init.ly:655-670) :
/// facteur `scale^i` sur la i-ème note, puis normalisation de la durée totale.
fn feather_music(music: &mut Music, scale: &Fraction) {
    fn visiter(music: &mut Music, scale: &Fraction, index: &mut usize) {
        match music {
            Music::Seq(items) | Music::Sim(items) => {
                for item in items {
                    visiter(item, scale, index);
                }
            }
            Music::Chord(chord) => {
                let mut facteur = Fraction::ONE;
                for _ in 0..*index {
                    facteur *= *scale;
                }
                let multiplicateur = chord.multiplier.unwrap_or(Fraction::ONE) * facteur;
                chord.multiplier = Some(multiplicateur);
                chord.from_scale = true;
                *index += 1;
            }
            Music::Context(Context::Tuplet { music, .. })
            | Music::Context(Context::Grace { music, .. })
            | Music::Context(Context::Fixed { music, .. })
            | Music::Context(Context::Relative { music, .. })
            | Music::Context(Context::NonRelative { music })
            | Music::Context(Context::Transpose { music, .. })
            | Music::Context(Context::Harmonic { music, .. })
            | Music::Context(Context::Silent { music })
            | Music::Context(Context::Skipped { music })
            | Music::Context(Context::Cue { music, .. })
            | Music::Context(Context::Invisible { music }) => visiter(music, scale, index),
            Music::Tag { music, .. } | Music::Filtered { music, .. } => {
                visiter(music, scale, index)
            }
            _ => {}
        }
    }
    let avant = music_duration(music, Duration::quarter());
    let mut index = 0;
    visiter(music, scale, &mut index);
    let apres = music_duration(music, Duration::quarter());
    if apres > Fraction::ZERO && avant != apres {
        let normalisation = avant / apres;
        scale_music(music, &normalisation);
    }
}

/// Multiplie par `factor` le multiplicateur `*n/d` de chaque accord d'une
/// musique. C'est le mécanisme déjà utilisé par l'interpréteur pour les durées
/// allongées : aucune nouvelle durée n'est introduite.
pub(crate) fn scale_music(music: &mut Music, factor: &Fraction) {
    match music {
        Music::Seq(items) | Music::Sim(items) | Music::SimVoices(items) => {
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

/// Vrai pour un `<< … \\ … >>` mêlant musique et contextes (ossia) : la
/// portée ouverte y naît au tick de l'élément, pas après lui.
fn is_ossia_block(music: &Music) -> bool {
    match music {
        Music::Sim(items) | Music::SimVoices(items) => {
            items.iter().any(has_context) && items.iter().any(|item| !has_context(item))
        }
        _ => false,
    }
}

/// Durée d'un élément de séquence, contextes compris.
fn music_duration_with_contexts(music: &Music, default: Duration) -> Fraction {
    match music {
        Music::New(group) => group
            .staves
            .iter()
            .flat_map(|staff| staff.voices.iter())
            .map(|voice| music_duration(&voice.music, default))
            .fold(
                Fraction::ZERO,
                |total, duree| {
                    if duree > total {
                        duree
                    } else {
                        total
                    }
                },
            ),
        _ => music_duration(music, default),
    }
}

/// Silence de `ticks` (en rondes), pour conserver le temps d'un élément de
/// séquence (`\skip`, `ChordKind::SkipNumeric` : sa durée ne devient pas la
/// durée par défaut).
fn skip_music(ticks: Fraction) -> Music {
    if !ticks.is_positive() {
        return Music::Empty;
    }
    Music::Chord(Chord {
        kind: ChordKind::SkipNumeric,
        duration: Some(Duration::whole()),
        multiplier: if ticks == Fraction::ONE {
            None
        } else {
            Some(ticks)
        },
        from_scale: false,
        bracketed: false,
        posts: Vec::new(),
        invisible: false,
    })
}

/// Applique un facteur d'échelle aux musiques contenues dans une commande.
fn scale_context(context: &mut Context, factor: &Fraction) {
    match context {
        Context::Tuplet { music, .. }
        | Context::Grace { music, .. }
        | Context::Fixed { music, .. }
        | Context::Relative { music, .. }
        | Context::NonRelative { music, .. }
        | Context::Transpose { music, .. }
        | Context::Harmonic { music, .. } => scale_music(music, factor),
        Context::AfterGrace { main, grace, .. } => {
            scale_music(main, factor);
            scale_music(grace, factor);
        }
        Context::After { event, music, .. } => {
            scale_music(event, factor);
            scale_music(music, factor);
        }
        Context::Voices { music, .. } => scale_music(music, factor),
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
        Context::Volta { music, .. } => scale_music(music, factor),
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
            // `ly/script-init.ly` : `make-articulation` définit ces scripts
            // comme post-événements. En omettre un faisait sortir la commande
            // du post-événement, et le `~` (ou la parenthèse de liaison) qui
            // la suivait était avalé : `g1 \downprall ~ g1` perdait sa
            // liaison (bwv-988-v22, attaque en trop à la mesure 12).
            | "downmordent"
            | "downprall"
            | "upprall"
            | "prallup"
            | "pralldown"
            | "prallmordent"
            | "lineprall"
            | "bachschleifer"
            | "coda"
            | "varcoda"
            | "segno"
            | "signumcongruentiae"
            | "halfopen"
            | "heel"
            | "toe"
            | "lheel"
            | "rheel"
            | "ltoe"
            | "rtoe"
            | "heelcircle"
            | "varheel"
            | "vartoe"
            | "henzelongfermata"
            | "henzeshortfermata"
            | "veryshortfermata"
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
fn attach_dynamic_velocity(post: &mut PostEvent, pending: &mut Option<u8>) {
    if let PostEvent::Dynamic(_, _, velocity) = post {
        *velocity = pending.take();
    }
}

fn post_event_for_command(name: &str, placement: Placement) -> Option<PostEvent> {
    let event = match name {
        // `\\(`/`\\)` sont des commandes ponctuelles (`lily/parser.yy`
        // `direction_less_event`) ; les laisser hors des post-événements
        // coupait la lecture juste après elles, et le `~` qui suivait était
        // avalé (`Chop-28-4`, `gs4->\(~ gs4` perdait sa liaison).
        "(" => PostEvent::SlurStart(0),
        ")" => PostEvent::SlurStop(0),
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
        "upmordent" | "bachschleifer" => PostEvent::Ornament(OrnamentType::Schleifer),
        "downmordent" => PostEvent::Ornament(OrnamentType::Mordent),
        "henzelongfermata" => PostEvent::Articulation(ArticulationType::FermataLong),
        "henzeshortfermata" => PostEvent::Articulation(ArticulationType::FermataShort),
        "veryshortfermata" => PostEvent::Articulation(ArticulationType::FermataVeryShort),
        // Les variantes de prall et les scripts de cordes (talon, pointe,
        // demi-ouvert) n'ont pas d'équivalent exact dans le modèle : la
        // commande est consommée comme post-événement, sans ornement inventé.
        "downprall" | "upprall" | "prallup" | "pralldown" | "prallmordent" | "lineprall"
        | "coda" | "varcoda" | "segno" | "signumcongruentiae" | "halfopen" | "heel" | "toe"
        | "lheel" | "rheel" | "ltoe" | "rtoe" | "heelcircle" | "varheel" | "vartoe" => return None,
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
        name if is_dynamic_command(name) => PostEvent::Dynamic(name.to_owned(), placement, None),
        _ => return None,
    };
    Some(event)
}

/// Groupe de tête correspondant à un style de `\tweak`.
fn head_group_from_style(style: &str) -> Option<NoteHeadGroup> {
    scoretk_lilypond_common::noteheads::group(style)
}

/// Numéro de couplet (à partir de zéro) d'une valeur de `stanza`.
///
/// `\set stanza = "2."` et `\with { stanza = "2." }` donnent le même numéro ;
/// un libellé non numérique (`"R."` pour un refrain) laisse le numéro par
/// défaut, qui sera attribué par ordre d'apparition.
/// Propriété MIDI exprimée comme une fraction de la plage 0..=1
/// (`midiExpression`, `midiPanPosition`, §24.8).
fn scheme_unit(value: &str) -> Option<f64> {
    value
        .trim()
        .trim_start_matches(['#', '\''])
        .trim()
        .parse::<f64>()
        .ok()
        .map(|fraction| fraction.clamp(0.0, 1.0))
}

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
            default_tremolo_type: 8,
            staff_size: None,
            last_markup_fontsize: None,
            current_header_field: None,
            header_bold_fields: Vec::new(),
            pending_dynamic_velocity: None,
            pending_tremolo_kind: None,
            pending_exact_head: None,
            pending_invisible_rest: false,
            pending_invisible_ottava: false,
            partitions: Vec::new(),
            midi_transposition: None,
            playback_articulate: false,
            playback_unfold_repeats: false,
            pending_midi_instrument: None,
            music_functions: BTreeMap::new(),
            instrument_definitions: BTreeMap::new(),
            expansion_depth: 0,
            melisma_busy_disabled: false,
            skip_typesetting: false,
            after_grace_fraction: Fraction::new(3, 4),
            staff_local_timing: false,
            note_mode: NoteMode::Notes,
            in_relative_function: false,
            tag_groups: default_tag_groups(),
            scheme_variables: BTreeMap::new(),
            duration_identifiers: BTreeMap::new(),
            numbers: BTreeMap::new(),
            quotes: BTreeMap::new(),
            resolving_quotes: BTreeSet::new(),
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
                pending_midi_instrument: None,
                position: 1,
                language: Language::English,
                variables: BTreeMap::new(),
                default_duration: Duration::quarter(),
                default_multiplier: Fraction::ONE,
                warnings: &mut warnings,
                pending_head: NoteHeadGroup::Normal,
                pending_tuplet_number: None,
                default_tremolo_type: 8,
                staff_size: None,
                last_markup_fontsize: None,
                current_header_field: None,
                header_bold_fields: Vec::new(),
                pending_dynamic_velocity: None,
                pending_tremolo_kind: None,
                pending_exact_head: None,
                pending_invisible_rest: false,
                pending_invisible_ottava: false,
                partitions: Vec::new(),
                midi_transposition: None,
                playback_articulate: false,
                playback_unfold_repeats: false,
                music_functions: BTreeMap::new(),
                instrument_definitions: BTreeMap::new(),
                expansion_depth: 0,
                melisma_busy_disabled: false,
                skip_typesetting: false,
                after_grace_fraction: Fraction::new(3, 4),
                staff_local_timing: false,
                note_mode: NoteMode::Notes,
                in_relative_function: false,
                tag_groups: default_tag_groups(),
                scheme_variables: BTreeMap::new(),
                duration_identifiers: BTreeMap::new(),
                numbers: BTreeMap::new(),
                quotes: BTreeMap::new(),
                resolving_quotes: BTreeSet::new(),
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
