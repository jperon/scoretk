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

//! Arbre syntaxique du sous-ensemble LilyPond lu.
//!
//! Les productions reprennent celles de `lily/parser.yy` : `music_list`,
//! `sequential_music` (`{ … }`), `simultaneous_music` (`<< … >>`),
//! `note_chord_element`, `repeated_music`, `alternative_music`,
//! `context_change` (`\new`, `\with`) et `\score`. Les événements de portée
//! (`\key`, `\time`, `\clef`…) restent des commandes du flux musical, comme
//! dans le langage : c'est l'interpréteur qui les replace sur la trame.

use scoretk_core::model::{
    ArpeggioType, ArticulationType, ClefType, GraceType, KeyMode, NoteHeadGroup, OrnamentType,
    Placement, TremoloKind, TupletNumberType,
};

/// Classe d'octave d'une hauteur écrite.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PitchSpec {
    /// Degré diatonique : 0 = do, 1 = ré, …, 6 = si.
    pub step: u8,
    /// Altération en demi-tons.
    pub alter: i8,
    /// Marques d'octave écrites (`'` positifs, `,` négatifs).
    pub octave: i8,
    /// Contrôle d'octave `n='` : octave absolue imposée, ou `None`.
    pub octave_check: Option<i8>,
    /// Vrai si le nom donne une octave explicite (marques ou `=`).
    pub explicit_octave: bool,
    /// Altération forcée (`!`).
    pub force: bool,
    /// Altération de courtoisie (`?`).
    pub cautionary: bool,
}

/// Événement attaché à une note ou à un accord (`post_event`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PostEvent {
    /// `\rest` : la note écrite est en fait un silence (`lily/parser.yy`,
    /// `event_chord` ; `Note_event` converti en `Rest_event`).
    Rest,
    /// Liaison de prolongation (`~`).
    Tie,
    /// Début de liaison d'expression (`(`, `\=N(`) avec son numéro.
    SlurStart(u8),
    /// Fin de liaison d'expression (`)`, `\=N)`).
    SlurStop(u8),
    /// Articulation (`-.`, `->`, `\fermata`…).
    Articulation(ArticulationType),
    /// Ornement (`\trill`, `\mordent`…).
    Ornament(OrnamentType),
    /// Nuance (`\p`, `\mf`).
    Dynamic(String, Placement),
    /// Texte (`^\markup { … }`, `_\markup`).
    Text(String, Placement),
    /// Doigté (`-1`, `^\markup { "T" }`).
    Fingering(String),
    /// Début de soufflet (`\<`, `\>`, `\cresc`, `\decresc`).
    HairpinStart(bool),
    /// Fin de soufflet (`\!`).
    HairpinStop,
    /// Début de trille (`\startTrillSpan`).
    TrillStart,
    /// Fin de trille (`\stopTrillSpan`).
    TrillStop,
    /// Glissando (`\glissando`).
    Glissando,
    /// Arpège (`\arpeggio`).
    Arpeggio(ArpeggioType),
    /// Trémolo (`:8`, `:16`…), valeur de la figure.
    Tremolo(TremoloKind, u8),
    /// Chute ou montée (`\bendAfter`).
    BendAfter(i8),
}

/// Note d'un accord.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NoteSpec {
    /// Hauteur écrite.
    pub pitch: PitchSpec,
    /// Événements de la note (liaison, articulations internes).
    pub posts: Vec<PostEvent>,
    /// Groupe de tête (`\tweak style #'cross`).
    pub head_group: NoteHeadGroup,
    /// Hauteur MIDI imposée (`\drummode`, `\chordmode`) : dans ces modes les
    /// noms ne sont pas des hauteurs diatoniques et l'interprète n'applique
    /// ni octave relative ni transposition.
    pub midi: Option<u8>,
}

/// Contenu rythmique d'un élément.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ChordKind {
    /// Notes de l'accord.
    Notes(Vec<NoteSpec>),
    /// Silence (`r`).
    Rest,
    /// Silence invisible (`s`).
    Skip,
    /// Silence multi-mesures (`R`).
    MultiRest,
    /// Répétition du dernier accord (`q`).
    ///
    /// Les hauteurs sont celles, déjà résolues, du dernier accord joué :
    /// l'analyse syntaxique ne les connaît pas, contrairement à
    /// l'interprétation.
    Repeat,
}

/// Accord, silence ou silence multi-mesures avec sa durée.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Chord {
    /// Notes ou silence.
    pub kind: ChordKind,
    /// Figure écrite, points compris.
    pub duration: Option<scoretk_core::model::Duration>,
    /// Durée réelle quand un multiplicateur `*n/d` l'allonge.
    pub multiplier: Option<scoretk_core::model::Fraction>,
    /// Événements attachés à l'accord.
    pub posts: Vec<PostEvent>,
    /// Vrai pour un silence rendu invisible (`Rest.transparent`).
    pub invisible: bool,
}

/// Commande qui change le contexte ou la trame.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Context {
    /// `\clef`.
    Clef(ClefType),
    /// `\key`.
    Key { fifths: i8, mode: KeyMode },
    /// `\time`.
    Time { numerator: i32, denominator: i32 },
    /// `\partial`.
    Partial(scoretk_core::model::Duration),
    /// `\set Timing.measureLength = #n/d`.
    MeasureLength(scoretk_core::model::Fraction),
    /// `\cadenzaOn` (`true`) / `\cadenzaOff` (`false`), ou
    /// `\set Timing.timing = ##f` / `##t`. La valeur `true` coupe les barres
    /// automatiques jusqu'au prochain `false` ou à une barre explicite.
    Cadenza(bool),
    /// `\bar "…"`, ou une barre `|` écrite dans la musique.
    Bar(String),
    /// `\section`.
    Section,
    /// `\break`.
    Break,
    /// `\tempo`.
    Tempo {
        text: String,
        beats_per_minute: Option<f64>,
    },
    /// `\mark`.
    Mark(String),
    /// `\ottava`.
    Ottava(i8),
    /// `\sustainOn` / `\sustainOff`.
    Sustain(bool),
    /// `\voiceOne` … `\voiceFour`.
    Voice(u8),
    /// `\stemUp` / `\stemDown`.
    Stem(scoretk_core::model::StemDirection),
    /// `\once \override Stem.direction = #UP`.
    StemOverride(scoretk_core::model::StemDirection),
    /// `\tuplet` : ratio, affichage du nombre et musique.
    Tuplet {
        numerator: u32,
        denominator: u32,
        number_type: TupletNumberType,
        music: Box<Music>,
    },
    /// `\grace` et variantes.
    Grace {
        kind: GraceType,
        slash: bool,
        music: Box<Music>,
    },
    /// `\afterGrace` : accord principal puis ornements.
    AfterGrace { main: Box<Music>, grace: Box<Music> },
    /// `\repeat`.
    Repeat {
        kind: RepeatKind,
        count: u32,
        body: Box<Music>,
        alternatives: Vec<Music>,
    },
    /// `\transpose`.
    Transpose {
        from: PitchSpec,
        to: PitchSpec,
        /// Vrai pour un `\transpose` littéral : son contenu reste absolu dans
        /// un `\relative` environnant (Notation Reference §1.2.2). Faux quand
        /// le corps vient d'une fonction `make-relative`, qui a déjà relu ses
        /// arguments dans le contexte relatif avant de construire le
        /// `TransposedMusic` (`scm/music-functions.scm`).
        absolute: bool,
        music: Box<Music>,
    },
    /// `\transposition` : transposition de l'instrument pour la hauteur
    /// concert, sans changer la hauteur écrite (`instrumentTransposition` de
    /// `ly/engraver-init.ly`).
    InstrumentTranspose(PitchSpec),
    /// `\transposition` posé dans le bloc `\midi` : il concerne toutes les
    /// portées (réglage du performer MIDI), et non un seul instrument. Son
    /// décalage est appliqué à toutes les voix sans déclarer `<transpose>`
    /// quand la partie réunit plusieurs portées.
    MidiInstrumentTranspose(PitchSpec),
    /// `\fixed` : octave de référence absolue.
    Fixed {
        reference: Option<PitchSpec>,
        music: Box<Music>,
    },
    /// `\relative` : hauteurs relatives.
    Relative {
        reference: Option<PitchSpec>,
        music: Box<Music>,
    },
    /// `make-non-relative-music` (`ly/music-functions-init.ly`) : musique
    /// soustraite à la conversion d'un `\relative` environnant (ses hauteurs
    /// écrites gardent l'octave absolue par défaut). `\partCombine` enveloppe
    /// ainsi ses deux parties (`make-directed-part-combine-music`).
    NonRelative { music: Box<Music> },
    /// `\alternative` isolé (déjà consommé par `\repeat`).
    Alternative(Vec<Music>),
    /// `\change <Contexte> = <nom>` : les notes qui suivent passent sur la
    /// portée nommée, sans changer de voix.
    Change {
        /// Nom du contexte cible (portée).
        name: String,
        /// Ligne de la commande `\change`, pour les avertissements.
        line: u32,
        /// Colonne de la commande `\change`.
        column: u32,
    },
}

/// Nature d'une reprise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RepeatKind {
    /// `\repeat volta n`.
    Volta,
    /// `\repeat unfold n`.
    Unfold,
    /// `\repeat percent n` ou `\repeat tremolo` : lue une fois.
    Ignored,
}

impl RepeatKind {
    /// Nature correspondant au mot de LilyPond.
    pub(crate) fn from_word(word: &str) -> Self {
        match word {
            "volta" => Self::Volta,
            "unfold" => Self::Unfold,
            _ => Self::Ignored,
        }
    }
}

/// Expression musicale.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Music {
    /// Rien (musique vide ou ignorée).
    Empty,
    /// Séquence `{ … }`.
    Seq(Vec<Music>),
    /// Simultané `<< … >>`.
    ///
    /// L'interpréteur le lit branche par branche : la première reste dans la
    /// voix courante, les suivantes ouvrent des voix sœurs au même tick ; `\\`
    /// sépare les passages polyphoniques temporaires (« Temporary polyphonic
    /// passages » de la référence de notation).
    Sim(Vec<Music>),
    /// Accord, silence ou silence multi-mesures.
    Chord(Chord),
    /// Bloc `\new …` rencontré dans le flux musical.
    New(Box<GroupBlock>),
    /// Bloc de paroles `\new Lyrics …`.
    Lyrics(Box<LyricsBlock>),
    /// Commande de contexte ou structurelle.
    Context(Context),
    /// Commande inconnue, ignorée avec un avertissement.
    Unknown(String),
}

impl Music {
    /// Vrai si l'expression ne produit rien.
    pub(crate) const fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

/// Description d'une portée ou d'un groupe dans un bloc `\score`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GroupKind {
    /// Une portée seule (`\new Staff`).
    Staff,
    /// `\new PianoStaff`.
    PianoStaff,
    /// `\new ChoirStaff`.
    ChoirStaff,
    /// `\new StaffGroup`.
    StaffGroup,
    /// `\new Voice` directement dans le `\score`.
    Voice,
    /// Contexte inconnu (`\new Lyrics`, `\new FiguredBass`…).
    Other,
}

impl GroupKind {
    /// Nature correspondant au nom de contexte LilyPond.
    ///
    /// Les noms de contextes sont insensibles à la casse pour LilyPond :
    /// `\new voice` et `\new Voice` désignent le même contexte. Les partitions
    /// Mutopia anciennes écrivent souvent `\new voice`, `\context staff`…
    pub(crate) fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "staff" | "drumstaff" => Self::Staff,
            "pianostaff" => Self::PianoStaff,
            "choirstaff" => Self::ChoirStaff,
            "staffgroup" => Self::StaffGroup,
            "voice" => Self::Voice,
            _ => Self::Other,
        }
    }
}

/// Une voix nommée ou anonyme d'une portée.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VoiceBlock {
    /// Nom du contexte (`\new Voice = "…"`), s'il existe.
    pub name: Option<String>,
    /// Musique de la voix.
    pub music: Music,
}

/// Un bloc de paroles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LyricsBlock {
    /// Nom de la voix cible (`\lyricsto "…"`), ou vide pour `\addlyrics`.
    pub target: String,
    /// Syllabes, dans l'ordre.
    pub syllables: Vec<Syllable>,
    /// Rang du couplet (`stanza`), à partir de zéro.
    pub verse: usize,
}

/// Une syllabe de paroles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Syllable {
    /// Texte de la syllabe.
    pub text: String,
    /// Vrai si la syllabe est suivie de `--` (mot continué).
    pub hyphen: bool,
    /// Vrai si la syllabe est suivie de `__` (mélisme).
    pub extender: bool,
    /// Vrai pour `\skip 1` : pas de syllabe sur cet accord.
    pub skip: bool,
    /// Durée explicite d'une syllabe (`la4`), pour un alignement par tick
    /// quand les paroles ne sont pas rattachées par `\lyricsto`.
    pub ticks: Option<scoretk_core::model::Fraction>,
}

/// Une portée du bloc `\score`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StaffBlock {
    /// Nom du contexte (`\new Staff = "…"`).
    pub name: Option<String>,
    /// Noms d'instrument (`\with { instrumentName = … }`).
    pub instrument_name: Option<String>,
    /// Nom abrégé (`shortInstrumentName`).
    pub short_instrument_name: Option<String>,
    /// Voix de la portée.
    pub voices: Vec<VoiceBlock>,
    /// Paroles attachées à une voix de la portée.
    pub lyrics: Vec<LyricsBlock>,
}

impl StaffBlock {
    /// Portée vide.
    pub(crate) fn new() -> Self {
        Self {
            name: None,
            instrument_name: None,
            short_instrument_name: None,
            voices: Vec::new(),
            lyrics: Vec::new(),
        }
    }
}

/// Un groupe du bloc `\score` (partie).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GroupBlock {
    /// Nature du contexte.
    pub kind: GroupKind,
    /// Vrai pour un `\context` (contexte existant retrouvé), faux pour un
    /// `\new` : seul le premier peut prolonger la voix courante sans ouvrir
    /// une nouvelle piste.
    pub existing: bool,
    /// Nom d'instrument du groupe (`\with`).
    pub instrument_name: Option<String>,
    /// Nom abrégé du groupe.
    pub short_instrument_name: Option<String>,
    /// Portées du groupe ; un `\new Staff` isolé en compte une.
    pub staves: Vec<StaffBlock>,
    /// Paroles directement dans le groupe.
    pub lyrics: Vec<LyricsBlock>,
}

/// Contenu d'un bloc `\score`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ScoreAst {
    /// Groupes et portées, dans l'ordre du fichier.
    pub groups: Vec<GroupBlock>,
    /// Paroles trouvées hors portée.
    pub lyrics: Vec<LyricsBlock>,
}

/// Métadonnées lues dans `\header`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HeaderAst {
    /// Champ du `\header` et sa valeur, dans l'ordre du fichier.
    pub entries: Vec<(String, String)>,
}

impl HeaderAst {
    /// Valeur d'un champ, s'il existe.
    pub(crate) fn get(&self, field: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(name, _)| name == field)
            .map(|(_, value)| value.as_str())
    }
}

/// Document LilyPond lu.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Document {
    /// Version annoncée par `\version`.
    pub version: Option<String>,
    /// Langue des hauteurs (`\language`).
    pub language: String,
    /// En-tête.
    pub header: HeaderAst,
    /// Bloc `\score`, s'il existe.
    pub score: Option<ScoreAst>,
    /// `\set melismaBusyProperties = #'()` a été lu : `\lyricsto` ne saute
    /// alors aucune note. Par défaut, les notes sous une liaison
    /// d'expression et les continuations de liaison sont sautées.
    pub melisma_busy_disabled: bool,
    /// Le bloc `\layout` a explicitement retiré `Timing_translator` du
    /// contexte `Score` et l'a ajouté à `Staff` : les `\time` sont alors
    /// propres à chaque portée (polymétrie). Sans ce déplacement, tout
    /// `\time` vaut pour toutes les portées à partir de sa position.
    pub staff_local_timing: bool,
}
