// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage des productions `event_chord`, `note_chord_element`,
// `post_events`, `duration`, `repeated_music` et `alternative_music` de
// `lily/parser.yy` de LilyPond, Copyright (C) 1998--2026 Han-Wen Nienhuys et
// al., modifié (réécrit en Rust) en 2026 : interprétation d'une voix en
// événements datés.
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

//! Interprétation d'une voix : de l'arbre syntaxique aux événements datés.
//!
//! LilyPond est un langage de musique linéaire : les hauteurs relatives,
//! l'octave par défaut, les n-olets et les reprises n'existent qu'à
//! l'interprétation. Cette passe produit, pour une voix :
//!
//! - des accords et silences datés (figure écrite, durée réelle, ornements,
//!   articulations, paroles attachées plus tard) ;
//! - des changements de contexte datés (`\key`, `\time`, `\clef`, `\bar`…) ;
//! - des spanners datés (liaisons, soufflets, ottavas, pédales, trilles) ;
//! - les n-olets et les sections de reprise, que le constructeur transforme en
//!   objets du modèle.

use scoretk_core::model::{
    Arpeggio, ArpeggioType, Articulation, ArticulationType, BarLineType, Breath, BreathType,
    ChordLineType, Duration, Fraction, GraceType, HairpinType, Key, KeySig, NoteHeadGroup,
    Ornament, OrnamentType, OttavaType, Placement, StemDirection, Tremolo,
};

use crate::ast::{
    Chord, ChordKind, Context, GroupBlock, GroupKind, LyricsBlock, Music, NoteSpec, PitchSpec,
    PostEvent, RepeatKind,
};
use crate::pitch::OctaveState;
use crate::warnings::Warnings;

/// Événement daté d'une voix.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VoiceEvent {
    /// Tick absolu.
    pub tick: Fraction,
    /// Rang d'émission, pour stabiliser l'ordre à tick égal.
    pub order: usize,
    /// Contenu.
    pub kind: EventKind,
}

/// Contenu d'un événement de voix.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum EventKind {
    /// Accord, silence ou silence multi-mesures.
    Chord(Box<ChordEvent>),
    /// Changement de contexte ou de trame.
    Meta(MetaEvent),
    /// Annotation (nuance, tempo, texte, repère).
    Annotation(AnnotationEvent),
    /// Début de liaison d'expression (numéro LilyPond).
    SlurStart(u8),
    /// Fin de liaison d'expression.
    SlurStop(u8),
    /// Début de soufflet.
    HairpinStart(HairpinType),
    /// Fin de soufflet.
    HairpinStop,
    /// Début de trille.
    TrillStart,
    /// Fin de trille.
    TrillStop,
    /// Changement d'ottava (`\ottava n`), 0 pour la fin.
    Ottava(i8),
    /// Pédale (`true` pour `\sustainOn`).
    Pedal(bool),
    /// Glissando sur l'accord courant.
    Glissando,
    /// Respiration.
    Breath(BreathType),
}

/// Changement de contexte.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MetaEvent {
    /// `\clef`.
    Clef(scoretk_core::model::ClefType),
    /// `\key`.
    Key(KeySig),
    /// `\time`.
    Time { numerator: i32, denominator: i32 },
    /// `\partial`.
    Partial(Fraction),
    /// `\set Timing.measureLength`.
    MeasureLength(Fraction),
    /// `\cadenzaOn` (`true`) / `\cadenzaOff` (`false`) : fin des barres
    /// automatiques pendant la cadence.
    Cadenza(bool),
    /// `\bar`.
    Bar(BarLineType),
    /// `\section`.
    Section,
    /// `\break`.
    Break,
}

/// Annotation datée.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AnnotationEvent {
    /// Nuance.
    Dynamic {
        /// Texte (`p`, `mf`, `dolce`…).
        text: String,
        /// Placement.
        placement: Placement,
        /// Vrai pour un texte personnalisé.
        custom: bool,
    },
    /// Indication de tempo.
    Tempo {
        /// Texte affiché.
        text: String,
        /// Battements par minute.
        beats_per_minute: Option<f64>,
    },
    /// Repère de répétition.
    Mark(String),
    /// Texte de portée.
    Text {
        /// Texte.
        text: String,
        /// Placement.
        placement: Placement,
    },
}

/// Notes et silence d'un accord.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ChordContent {
    /// Notes de l'accord.
    Notes(Vec<NoteEvent>),
    /// Silence, visible ou invisible.
    Rest {
        /// Vrai si le silence est affiché.
        visible: bool,
    },
    /// Temps réservé sans élément (`s`).
    Skip,
    /// Silence de mesure entière ou multi-mesures.
    MultiRest,
}

/// Note datée d'un accord.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NoteEvent {
    /// Hauteur concert (ou écrite sans transposition).
    pub pitch: u8,
    /// Classe de hauteur tonale concert.
    pub tpc: i8,
    /// Classe de hauteur tonale écrite.
    pub tpc_transposed: i8,
    /// Vrai si la note commence une liaison de prolongation.
    pub tie: bool,
    /// Vrai pour une altération de courtoisie.
    pub cautionary: bool,
    /// Groupe de tête.
    pub head_group: NoteHeadGroup,
}

/// Accord d'agrément.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GraceEvent {
    /// Notes.
    pub notes: Vec<NoteEvent>,
    /// Durée notée.
    pub duration: Duration,
    /// Type d'ornement.
    pub kind: GraceType,
    /// Vrai si l'ornement est barré.
    pub slash: bool,
}

/// Portée cible d'un `\change`, avec la position de la commande pour les
/// avertissements du constructeur.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StaffTarget {
    /// Nom du contexte cible.
    pub name: String,
    /// Ligne de la commande `\change`.
    pub line: u32,
    /// Colonne de la commande `\change`.
    pub column: u32,
}

/// Accord ou silence daté.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChordEvent {
    /// Contenu.
    pub content: ChordContent,
    /// Figure écrite.
    pub written: Duration,
    /// Durée réelle explicite (`ticks` du modèle), quand un multiplicateur
    /// `*n/d` l'allonge.
    pub explicit_ticks: Option<Fraction>,
    /// Durée réelle en fractions de ronde (n-olet compris).
    pub actual: Fraction,
    /// N-olet le plus interne, index local dans [`InterpretedVoice::tuplets`].
    pub tuplet: Option<usize>,
    /// Articulations.
    pub articulations: Vec<ArticulationType>,
    /// Ornements.
    pub ornaments: Vec<OrnamentType>,
    /// Arpège.
    pub arpeggio: Option<ArpeggioType>,
    /// Trémolo.
    pub tremolo: Option<Tremolo>,
    /// Chute ou montée (`\bendAfter`).
    pub chord_line: Option<ChordLineType>,
    /// Direction de hampe explicite.
    pub stem: StemDirection,
    /// Portée cible d'un `\change` en vigueur pour cet accord.
    pub staff: Option<StaffTarget>,
    /// Doigtés de la note principale.
    pub fingerings: Vec<String>,
    /// Ornements qui précèdent.
    pub grace_before: Vec<GraceEvent>,
    /// Ornements qui suivent.
    pub grace_after: Vec<GraceEvent>,
    /// Respiration attachée.
    pub breath: Option<BreathType>,
}

/// N-olet construit par la voix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TupletBuild {
    /// Ratio `n/d`.
    pub ratio: Fraction,
    /// Durée écrite du premier enfant.
    pub base_len: Fraction,
    /// Tick de début.
    pub tick: Fraction,
    /// Durée réelle.
    pub len: Fraction,
    /// N-olet parent, index local.
    pub parent: Option<usize>,
    /// Nombre de notes jouées.
    pub actual_notes: i64,
    /// Nombre de notes normales.
    pub normal_notes: i64,
    /// Affichage du nombre.
    pub number_type: scoretk_core::model::TupletNumberType,
}

/// Section de reprise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RepeatSection {
    /// Début du corps.
    pub start: Fraction,
    /// Fin de la reprise (fin du premier ending, ou du corps).
    pub end: Fraction,
    /// Nombre de lectures.
    pub count: u8,
    /// Endings successifs, dans l'ordre.
    pub alternatives: Vec<AlternativeSection>,
}

/// Un ending de reprise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AlternativeSection {
    /// Début.
    pub start: Fraction,
    /// Fin.
    pub end: Fraction,
    /// Numéro d'ending, à partir de 1.
    pub number: u8,
}

/// Résultat de l'interprétation d'une voix.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct InterpretedVoice {
    /// Événements datés.
    pub events: Vec<VoiceEvent>,
    /// N-olets.
    pub tuplets: Vec<TupletBuild>,
    /// Reprises.
    pub repeats: Vec<RepeatSection>,
    /// Tick de fin (fin du dernier événement).
    pub end: Fraction,
    /// Voix ouvertes depuis l'intérieur de celle-ci (`\new Voice`, `\\`).
    pub spawned: Vec<SpawnedVoice>,
}

/// Voix ouverte depuis l'intérieur d'une autre voix.
///
/// Correspond à un contexte `Voice` créé par `context_prefix` /
/// `NEWCONTEXT` (`lily/parser.yy` l. 1685-1690) ou à une branche de passage
/// polyphonique temporaire (`<< … \\ … >>`, « Temporary polyphonic
/// passages » de la référence de notation) : la voix appartient à la portée
/// courante et commence au tick de l'ouverture.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SpawnedVoice {
    /// Nom du contexte (`\new Voice = "…"`), s'il existe.
    pub name: Option<String>,
    /// Paroles `\addlyrics` attachées au contexte ouvert.
    pub lyrics: Vec<LyricsBlock>,
    /// Vrai pour une branche de passage polyphonique temporaire
    /// (`<< … \\ … >>`) : la voix ne dure que le temps du passage et peut
    /// partager une piste avec les branches d'un autre passage, plus loin
    /// dans la pièce. Faux pour un `\new Voice` hors d'un tel passage, qui
    /// reste une voix à part entière.
    pub temporary: bool,
    /// Voix interprétée, en ticks absolus.
    pub interpreted: InterpretedVoice,
}

/// État final d'une voix ouverte, pour la lecture relative séquentielle de
/// LilyPond : la musique qui suit un contexte ou une branche simultanée part
/// de sa dernière note.
struct SpawnResult {
    /// Tick de fin.
    end: Fraction,
    /// État d'octave à la fin de la voix.
    octave: OctaveState,
    /// Notes du dernier accord, pour la répétition `q`.
    last_chord_notes: Option<Vec<NoteEvent>>,
}

/// Ornements d'agrément en cours.
struct GraceFrame {
    kind: GraceType,
    slash: bool,
}

/// Interprète d'une voix.
pub(crate) struct Interpreter<'a> {
    /// Résultat en construction.
    voice: InterpretedVoice,
    /// Position courante.
    cursor: Fraction,
    /// État d'octave.
    octave: OctaveState,
    /// Notes résolues du dernier accord émis, pour la répétition `q`.
    last_chord_notes: Option<Vec<NoteEvent>>,
    /// Transposition en vigueur (degrés diatoniques, demi-tons).
    transpose: (i32, i32),
    /// Décalage concert du `\transposition` du bloc `\midi`, en demi-tons.
    /// Il vaut pour toutes les portées et n'est pas porté par `<transpose>`
    /// lorsque la partie en réunit plusieurs.
    midi_instrument_transpose: i32,
    /// N-olets ouverts, index locaux.
    tuplets: Vec<usize>,
    /// Ornements en cours d'analyse.
    grace: Option<GraceFrame>,
    /// Ornements en attente de l'accord suivant.
    pending_grace: Vec<GraceEvent>,
    /// Cible d'un `\afterGrace`.
    grace_after_target: Option<usize>,
    /// Durée par défaut.
    default_duration: Duration,
    /// Hampe explicite en attente.
    stem_override: Option<StemDirection>,
    /// Portée cible courante d'un `\change` (nom du contexte).
    staff_target: Option<StaffTarget>,
    /// Facteur de n-olet hérité d'une voix ouverte à l'intérieur d'un
    /// `\tuplet` (les n-olets ouverts du parent ne sont pas dans `tuplets`).
    tuplet_scale: Fraction,
    /// Vrai dans une branche de passage polyphonique temporaire : les
    /// contextes qui y sont ouverts en héritent et pourront partager une piste
    /// avec ceux d'un autre passage.
    temporary: bool,
    /// Rang d'émission.
    order: usize,
    /// Avertissements.
    warnings: &'a mut Warnings,
}

impl<'a> Interpreter<'a> {
    /// Interprète une voix.
    pub(crate) fn run(music: &Music, warnings: &'a mut Warnings) -> InterpretedVoice {
        let mut interpreter = Self {
            voice: InterpretedVoice::default(),
            cursor: Fraction::ZERO,
            octave: OctaveState::default(),
            last_chord_notes: None,
            transpose: (0, 0),
            midi_instrument_transpose: 0,
            tuplets: Vec::new(),
            grace: None,
            pending_grace: Vec::new(),
            grace_after_target: None,
            default_duration: Duration::quarter(),
            stem_override: None,
            staff_target: None,
            tuplet_scale: Fraction::ONE,
            temporary: false,
            order: 0,
            warnings,
        };
        interpreter.walk(music);
        interpreter.voice.end = interpreter.cursor;
        interpreter.voice
    }

    /// Parcourt une expression musicale.
    fn walk(&mut self, music: &Music) {
        match music {
            Music::Empty | Music::Lyrics(_) => {}
            Music::New(group) => self.nested_context(group),
            Music::Seq(items) => {
                for item in items {
                    self.walk(item);
                }
            }
            Music::Sim(items) => self.simultaneous(items),
            Music::Chord(chord) => self.emit_chord(chord),
            Music::Context(context) => self.context(context),
            Music::Unknown(_) => {}
        }
    }

    /// Contexte `\new` rencontré dans une voix.
    ///
    /// `\new Voice` ouvre une voix de la portée courante (`context_prefix` /
    /// `NEWCONTEXT`, `lily/parser.yy` l. 1685-1690). Un `\context Staff`
    /// rencontré dans une musique de portée retrouve cette portée courante :
    /// ses voix s'ouvrent donc sur cette portée, exactement comme un passage
    /// polyphonique temporaire. C'est l'idiome des pièces qui enchâssent
    /// `\context Staff << \new Voice … >>` dans une portée déjà ouverte.
    /// Les groupes de portées restent ignorés avec un avertissement.
    fn nested_context(&mut self, group: &GroupBlock) {
        if !matches!(group.kind, GroupKind::Voice | GroupKind::Staff) {
            self.warn("contexte imbriqué ignoré dans une voix");
            return;
        }
        if self.grace.is_some() {
            // Dans un ornement, les notes ne prennent pas de temps : les
            // garder dans la voix courante évite de perdre l'ornement (la
            // voix ouverte devrait sinon hériter du cadre d'agrément).
            for staff in &group.staves {
                for voice in &staff.voices {
                    self.walk(&voice.music);
                }
            }
            return;
        }
        // Un `\context Staff` à musique simple (une seule portée, une seule
        // voix) retrouve le contexte existant (`context_find_or_create`,
        // `lily/context.cc`) : il n'ouvre pas une voix sœur au tick courant,
        // il poursuit la lecture dans la voix en cours. Ouvrir une voix
        // ferait repartir son compteur d'attaque à zéro dans le MIDI écrit :
        // `grace-staff-length.ly` plaçait ainsi son ornement et sa note au
        // début de la portée au lieu de la deuxième mesure (tick 4).
        // Un `\new Staff` (y compris sous `\incipit`) crée au contraire une
        // vraie portée, qui garde sa propre voix et sa propre trame.
        if group.existing
            && group.kind == GroupKind::Staff
            && group.staves.len() == 1
            && group.staves[0].voices.len() == 1
        {
            self.walk(&group.staves[0].voices[0].music);
            return;
        }
        let start = self.cursor;
        let mut end = start;
        for staff in &group.staves {
            let mut lyrics: Vec<LyricsBlock> = staff.lyrics.clone();
            lyrics.extend(group.lyrics.iter().cloned());
            for voice in &staff.voices {
                let child = self.spawn_at(
                    start,
                    voice.name.clone(),
                    &voice.music,
                    lyrics.clone(),
                    self.temporary,
                );
                end = end.max(child.end);
                // La musique d'un contexte est un élément de la séquence :
                // LilyPond poursuit la lecture relative à sa dernière note
                // (`generic_to_relative_octave`, `lily/music.cc`).
                self.octave = child.octave;
                self.last_chord_notes = child.last_chord_notes;
            }
        }
        // La musique du contexte fait partie de la séquence : elle avance la
        // voix d'origine de sa durée, comme LilyPond.
        self.cursor = end;
    }

    /// Musique simultanée `<< … >>` d'une voix.
    ///
    /// Chaque branche musicale est interprétée à partir du même tick : la
    /// première continue dans la voix courante, les suivantes ouvrent une voix
    /// sœur (`simultaneous_music` dans `lily/parser.yy` l. 1581-1588, et
    /// « Temporary polyphonic passages » : `\\` sépare des voix temporaires).
    /// Après le `>>`, la voix d'origine reprend au tick le plus avancé. Les
    /// commandes de contexte écrites directement dans le `<< >>` (`\clef`,
    /// `\key`, `\bar`…) portent sur la portée et restent dans la voix
    /// d'origine.
    ///
    /// Lecture relative : `Music_sequence::simultaneous_relative_callback`
    /// (`lily/music-sequence.cc`) parcourt les branches dans l'ordre, chacune
    /// relativement à la dernière note de la précédente, et la musique qui
    /// suit le `>>` part de la dernière note de la **dernière** branche.
    /// Lire les branches en parallèle depuis la note qui précède le `<<`
    /// décalerait tout le reste d'une octave.
    fn simultaneous(&mut self, items: &[Music]) {
        if self.grace.is_some() {
            // Ornement : lecture séquentielle, comme avant (les ornements ne
            // font pas avancer le curseur).
            for item in items {
                self.walk(item);
            }
            return;
        }
        let start = self.cursor;
        let mut musical: Vec<&Music> = Vec::new();
        for item in items {
            if is_context_only(item) {
                self.walk(item);
            } else {
                musical.push(item);
            }
        }
        let mut branches = musical.into_iter();
        let Some(first) = branches.next() else {
            self.cursor = self.cursor.max(start);
            return;
        };
        self.walk(first);
        let mut end = self.cursor;
        for item in branches {
            let child = self.spawn_at(start, None, item, Vec::new(), true);
            end = end.max(child.end);
            // `music_list_to_relative` enchaîne les branches : la suivante
            // part de la dernière note de celle-ci, et la continuation du
            // `>>` de la dernière note de la dernière branche.
            self.octave = child.octave;
            self.last_chord_notes = child.last_chord_notes;
        }
        self.cursor = end.max(start);
    }

    /// Ouvre une voix sœur dans la portée et l'interprète à partir de
    /// `start`, en reprenant l'état courant (octave, transposition, durée par
    /// défaut). Rend son état final.
    fn spawn_at(
        &mut self,
        start: Fraction,
        name: Option<String>,
        music: &Music,
        lyrics: Vec<LyricsBlock>,
        temporary: bool,
    ) -> SpawnResult {
        // Type explicite : la voix enfant emprunte les avertissements pour la
        // seule durée de son interprétation, pas pour `'a`.
        let mut child = Interpreter {
            voice: InterpretedVoice::default(),
            cursor: start,
            octave: self.octave,
            transpose: self.transpose,
            midi_instrument_transpose: self.midi_instrument_transpose,
            tuplets: Vec::new(),
            grace: None,
            pending_grace: Vec::new(),
            grace_after_target: None,
            default_duration: self.default_duration,
            last_chord_notes: self.last_chord_notes.clone(),
            stem_override: self.stem_override,
            staff_target: None,
            tuplet_scale: self.tuplet_scale * self.open_tuplet_scale(),
            temporary,
            order: 0,
            warnings: &mut *self.warnings,
        };
        child.walk(music);
        child.voice.end = child.cursor;
        let result = SpawnResult {
            end: child.cursor,
            octave: child.octave,
            last_chord_notes: child.last_chord_notes,
        };
        self.voice.spawned.push(SpawnedVoice {
            name,
            lyrics,
            temporary,
            interpreted: child.voice,
        });
        result
    }

    /// Facteur des n-olets ouverts dans cette voix.
    fn open_tuplet_scale(&self) -> Fraction {
        self.tuplets
            .iter()
            .filter_map(|index| self.voice.tuplets.get(*index))
            .fold(Fraction::ONE, |factor, tuplet| {
                if tuplet.ratio.numerator() == 0 {
                    factor
                } else {
                    factor * Fraction::new(tuplet.ratio.denominator(), tuplet.ratio.numerator())
                }
            })
    }

    /// Traite une commande de contexte.
    fn context(&mut self, context: &Context) {
        match context {
            Context::Clef(clef) => self.meta(MetaEvent::Clef(*clef)),
            Context::Key { fifths, mode } => {
                // LilyPond transpose l'armure avec la musique : `\key c \major`
                // sous `\transpose c d` vaut ré majeur. Les quintes se
                // déduisent de la transposition du do naturel.
                let shifted = i32::from(*fifths) + fifths_shift(self.transpose.0, self.transpose.1);
                let fifths = i8::try_from(shifted).unwrap_or(*fifths);
                let mut key = KeySig::new(Key::new(fifths));
                key.mode = *mode;
                self.meta(MetaEvent::Key(key));
            }
            Context::Time {
                numerator,
                denominator,
            } => self.meta(MetaEvent::Time {
                numerator: *numerator,
                denominator: *denominator,
            }),
            Context::Partial(duration) => {
                let len = duration.ticks();
                if len.is_positive() {
                    self.meta(MetaEvent::Partial(len));
                }
            }
            Context::MeasureLength(length) => {
                if length.is_positive() {
                    self.meta(MetaEvent::MeasureLength(*length));
                }
            }
            Context::Cadenza(on) => self.meta(MetaEvent::Cadenza(*on)),
            Context::Bar(text) => {
                if let Some(kind) = bar_line_type(text) {
                    self.meta(MetaEvent::Bar(kind));
                }
            }
            Context::Section => self.meta(MetaEvent::Section),
            Context::Break => self.meta(MetaEvent::Break),
            Context::Tempo {
                text,
                beats_per_minute,
            } => {
                if !text.trim().is_empty() || beats_per_minute.is_some() {
                    self.annotation(AnnotationEvent::Tempo {
                        text: text.clone(),
                        beats_per_minute: *beats_per_minute,
                    });
                }
            }
            Context::Mark(text) => {
                if !text.trim().is_empty() {
                    self.annotation(AnnotationEvent::Mark(text.clone()));
                }
            }
            Context::Ottava(shift) => self.event(EventKind::Ottava(*shift)),
            Context::Sustain(on) => self.event(EventKind::Pedal(*on)),
            Context::Change { name, line, column } => {
                self.staff_target = Some(StaffTarget {
                    name: name.clone(),
                    line: *line,
                    column: *column,
                });
            }
            Context::Voice(_) => {}
            Context::Stem(direction) => {
                self.stem_override = Some(*direction);
            }
            Context::StemOverride(direction) => self.stem_override = Some(*direction),
            Context::Tuplet {
                numerator,
                denominator,
                number_type,
                music,
            } => self.tuplet(*numerator, *denominator, *number_type, music),
            Context::Grace { kind, slash, music } => self.grace_scope(*kind, *slash, music),
            Context::AfterGrace { main, grace } => {
                self.walk(main);
                let target = self
                    .voice
                    .events
                    .iter()
                    .rposition(|event| matches!(event.kind, EventKind::Chord(_)));
                if let Some(target) = target {
                    let saved = self.grace_after_target.replace(target);
                    self.grace = Some(GraceFrame {
                        // Seul un type « After » place l'ornement après la
                        // note à l'export.
                        kind: GraceType::Grace8After,
                        slash: false,
                    });
                    self.walk(grace);
                    self.grace = None;
                    self.grace_after_target = saved;
                } else {
                    self.warn("\\afterGrace sans accord à décorer");
                }
            }
            Context::Repeat {
                kind,
                count,
                body,
                alternatives,
            } => self.repeat(*kind, *count, body, alternatives),
            Context::Transpose { from, to, music } => {
                let delta = transpose_delta(from, to);
                let saved = self.transpose;
                self.transpose = (saved.0 + delta.0, saved.1 + delta.1);
                self.walk(music);
                self.transpose = saved;
            }
            // `\transposition` de portée : le décalage concert est appliqué
            // par le constructeur, seulement si la partie porte `<transpose>`.
            Context::InstrumentTranspose(_) => {}
            // `\transposition` du bloc `\midi` : vaut pour toute la partition
            // et s'applique ici à toutes les voix, même si la partie ne porte
            // pas `<transpose>` (plusieurs portées dans le même groupe).
            Context::MidiInstrumentTranspose(pitch) => {
                self.midi_instrument_transpose = crate::pitch::instrument_transpose(pitch).1;
            }
            Context::Fixed { reference, music } => {
                let saved = self.octave;
                self.octave.relative = false;
                if let Some(reference) = reference {
                    self.octave.set_reference(reference);
                }
                self.walk(music);
                self.octave = saved;
            }
            Context::Relative { reference, music } => {
                let saved = self.octave;
                self.octave.start_relative(reference.as_ref());
                self.walk(music);
                self.octave = saved;
            }
            Context::NonRelative { music } => {
                // `make-non-relative-music` soustrait la musique à la
                // conversion `\relative` environnante (`ly:make-music-relative!`
                // ne descend pas dedans) : les hauteurs écrites restent dans
                // l'octave absolue par défaut, quelle que soit la référence du
                // `\relative`. Sous `\fixed`, en revanche, la transposition
                // s'applique bien aux parties, donc leur référence est conservée.
                let saved = self.octave;
                if self.octave.relative {
                    self.octave = OctaveState::default();
                } else {
                    self.octave.relative = false;
                }
                self.walk(music);
                self.octave = saved;
            }
            Context::Alternative(_) => {
                self.warn("\\alternative sans \\repeat ignoré");
            }
        }
    }

    /// `\tuplet n/d { … }`.
    fn tuplet(
        &mut self,
        numerator: u32,
        denominator: u32,
        number_type: scoretk_core::model::TupletNumberType,
        music: &Music,
    ) {
        if numerator == 0 || denominator == 0 {
            self.walk(music);
            return;
        }
        let ratio = Fraction::try_new(i64::from(numerator), i64::from(denominator))
            .unwrap_or(Fraction::ONE);
        let start = self.cursor;
        let index = self.voice.tuplets.len();
        let parent = self.tuplets.last().copied();
        self.voice.tuplets.push(TupletBuild {
            ratio,
            base_len: Fraction::ZERO,
            tick: start,
            len: Fraction::ZERO,
            parent,
            actual_notes: i64::from(numerator),
            normal_notes: i64::from(denominator),
            number_type,
        });
        self.tuplets.push(index);
        let first_chord = self.voice.events.len();
        self.walk(music);
        self.tuplets.pop();
        let len = self.cursor - start;
        if let Some(tuplet) = self.voice.tuplets.get_mut(index) {
            tuplet.len = len;
            let base = self.voice.events[first_chord..]
                .iter()
                .find_map(|event| match &event.kind {
                    EventKind::Chord(chord) => Some(chord.written.ticks()),
                    _ => None,
                })
                .filter(|base| base.is_positive())
                .unwrap_or_else(|| {
                    if ratio.numerator() > 0 {
                        len * Fraction::new(ratio.denominator(), ratio.numerator())
                    } else {
                        len
                    }
                });
            tuplet.base_len = base;
        }
    }

    /// `\grace`, `\acciaccatura`, `\appoggiatura`.
    fn grace_scope(&mut self, kind: GraceType, slash: bool, music: &Music) {
        let saved = self.grace.replace(GraceFrame { kind, slash });
        let saved_pending = std::mem::take(&mut self.pending_grace);
        self.walk(music);
        let collected = std::mem::replace(&mut self.pending_grace, saved_pending);
        self.pending_grace.extend(collected);
        self.grace = saved;
    }

    /// `\repeat`.
    fn repeat(&mut self, kind: RepeatKind, count: u32, body: &Music, alternatives: &[Music]) {
        match kind {
            RepeatKind::Unfold => {
                // Portage de `lily/music.cc` et `lily/music-sequence.cc` : le
                // wrapper de reprise n'a pas de `to-relative-callback`, donc le
                // corps est converti en octaves absolues une seule fois avant
                // d'être rejoué. Chaque passage repart de la même hauteur de
                // référence ; la laisser avancer ferait monter les notes
                // marquées d'une octave à chaque reprise.
                let reference = self.octave;
                for _ in 0..count.max(1) {
                    self.octave = reference;
                    self.walk(body);
                }
            }
            RepeatKind::Ignored => {
                self.warn("\\repeat non pris en charge : musique lue une fois");
                self.walk(body);
            }
            RepeatKind::Volta => {
                let start = self.cursor;
                self.walk(body);
                let body_end = self.cursor;
                let mut sections = Vec::new();
                let mut end = body_end;
                for (index, alternative) in alternatives.iter().enumerate() {
                    let alt_start = self.cursor;
                    self.walk(alternative);
                    let alt_end = self.cursor;
                    if index == 0 {
                        end = alt_end;
                    }
                    sections.push(AlternativeSection {
                        start: alt_start,
                        end: alt_end,
                        number: u8::try_from(index + 1).unwrap_or(u8::MAX),
                    });
                }
                self.voice.repeats.push(RepeatSection {
                    start,
                    end,
                    count: u8::try_from(count.clamp(1, 255)).unwrap_or(2),
                    alternatives: sections,
                });
            }
        }
    }

    /// Émet un accord, un silence ou un ornement.
    fn emit_chord(&mut self, chord: &Chord) {
        let written = chord.duration.unwrap_or(self.default_duration);
        let multiplier = chord.multiplier.unwrap_or(Fraction::ONE);
        let tuplet_factor = self.tuplet_scale * self.open_tuplet_scale();
        let mut actual = written.ticks() * multiplier * tuplet_factor;

        // Un ornement ne prend pas de temps dans la mesure.
        if let Some((kind, slash)) = self.grace.as_ref().map(|frame| (frame.kind, frame.slash)) {
            let notes = match &chord.kind {
                ChordKind::Notes(notes) => self.chord_notes(notes),
                _ => Vec::new(),
            };
            if !notes.is_empty() {
                let grace = GraceEvent {
                    notes,
                    duration: written,
                    kind,
                    slash,
                };
                if let Some(target) = self.grace_after_target {
                    if let Some(VoiceEvent {
                        kind: EventKind::Chord(chord),
                        ..
                    }) = self.voice.events.get_mut(target)
                    {
                        chord.grace_after.push(grace);
                    }
                } else {
                    self.pending_grace.push(grace);
                }
            }
            self.default_duration = written;
            return;
        }

        // `\rest` convertit la note (ou l'accord) écrite en silence : c'est le
        // comportement de `event_chord` dans `lily/parser.yy`. La note écrite
        // est tout de même résolue, car elle avance la référence `\relative`
        // (`Event_chord::to_relative_octave` s'applique avant la conversion).
        let note_is_rest = |notes: &[NoteSpec]| {
            notes.iter().any(|note| {
                note.posts
                    .iter()
                    .any(|post| matches!(post, PostEvent::Rest))
            })
        };
        let is_rest = chord
            .posts
            .iter()
            .any(|post| matches!(post, PostEvent::Rest))
            || match &chord.kind {
                ChordKind::Notes(notes) => note_is_rest(notes),
                _ => false,
            };
        // Un `~` porté par l'accord (et non par une note de `<…>`) lie toutes
        // ses notes : LilyPond (event_chord + Tie_engraver) attache une
        // liaison d'accord à chaque note, vers la note de même hauteur qui
        // suit.
        let chord_tie = chord
            .posts
            .iter()
            .any(|post| matches!(post, PostEvent::Tie));
        // Un silence de mesure entière couvre toute la mesure : le
        // constructeur le reconnaîtra d'après le silence multi-mesures.
        let mut content = match &chord.kind {
            ChordKind::Notes(notes) => {
                // La référence relative est celle de la première note de
                // l'accord, pas de la dernière. Résoudre les hauteurs même
                // pour un `\rest` avance cette référence, comme LilyPond.
                let notes = self.chord_notes(notes);
                if is_rest {
                    ChordContent::Rest {
                        visible: !chord.invisible,
                    }
                } else {
                    self.last_chord_notes = Some(notes.clone());
                    ChordContent::Notes(notes)
                }
            }
            // `q` reprend les hauteurs déjà résolues du dernier accord : les
            // résoudre de nouveau depuis ses marques écrites les décalerait
            // d'une octave à chaque répétition.
            ChordKind::Repeat => match self.last_chord_notes.clone() {
                Some(notes) => ChordContent::Notes(notes),
                None => {
                    self.warn("q sans accord précédent : répétition ignorée");
                    ChordContent::Notes(Vec::new())
                }
            },
            ChordKind::Rest => ChordContent::Rest {
                visible: !chord.invisible,
            },
            ChordKind::Skip => ChordContent::Skip,
            ChordKind::MultiRest => ChordContent::MultiRest,
        };
        if let ChordContent::Notes(notes) = &mut content {
            for note in notes {
                note.tie |= chord_tie;
            }
        }

        let mut articulations = Vec::new();
        let mut ornaments = Vec::new();
        let mut arpeggio = None;
        let mut tremolo = None;
        let mut chord_line = None;
        let mut breaths: Vec<BreathType> = Vec::new();
        let mut fingerings = Vec::new();
        for post in &chord.posts {
            self.apply_post(
                post,
                &mut articulations,
                &mut ornaments,
                &mut arpeggio,
                &mut tremolo,
                &mut chord_line,
                &mut breaths,
                &mut fingerings,
            );
        }
        // Post-événements des notes (articulations internes à l'accord).
        if let ChordKind::Notes(notes) = &chord.kind {
            for note in notes {
                for post in &note.posts {
                    self.apply_post(
                        post,
                        &mut articulations,
                        &mut ornaments,
                        &mut arpeggio,
                        &mut tremolo,
                        &mut chord_line,
                        &mut breaths,
                        &mut fingerings,
                    );
                }
            }
        }

        // `s` réserve du temps sans rien afficher : on ne crée aucun élément
        // (l'export écrit `<forward>`). Un silence transparent reste un
        // `Rest` invisible, distinct.
        if matches!(content, ChordContent::Skip) {
            self.cursor += actual;
            self.default_duration = written;
            return;
        }
        let stem = self.stem_override.take().unwrap_or(StemDirection::Auto);
        let grace_before = std::mem::take(&mut self.pending_grace);
        // Seul un multiplicateur explicite devient `ticks` dans le modèle :
        // la mise à l'échelle d'un n-olet est portée par le `Tuplet`.
        let explicit_ticks = chord.multiplier.map(|_| actual);
        if !actual.is_positive() {
            actual = Fraction::ZERO;
        }

        let event = ChordEvent {
            content,
            written,
            explicit_ticks,
            actual,
            tuplet: self.tuplets.last().copied(),
            articulations,
            ornaments,
            arpeggio,
            tremolo,
            chord_line,
            stem,
            staff: self.staff_target.clone(),
            fingerings,
            grace_before,
            grace_after: Vec::new(),
            breath: breaths.first().copied(),
        };
        self.voice.events.push(VoiceEvent {
            tick: self.cursor,
            order: self.order,
            kind: EventKind::Chord(Box::new(event)),
        });
        self.order += 1;
        self.cursor += actual;
        for breath in breaths {
            self.voice.events.push(VoiceEvent {
                tick: self.cursor - actual,
                order: self.order,
                kind: EventKind::Breath(breath),
            });
            self.order += 1;
        }
        self.default_duration = written;
    }

    /// Applique un post-événement.
    #[allow(clippy::too_many_arguments)]
    fn apply_post(
        &mut self,
        post: &PostEvent,
        articulations: &mut Vec<ArticulationType>,
        ornaments: &mut Vec<OrnamentType>,
        arpeggio: &mut Option<ArpeggioType>,
        tremolo: &mut Option<Tremolo>,
        chord_line: &mut Option<ChordLineType>,
        breaths: &mut Vec<BreathType>,
        fingerings: &mut Vec<String>,
    ) {
        match post {
            PostEvent::Rest | PostEvent::Tie => {}
            PostEvent::SlurStart(number) => {
                self.event(EventKind::SlurStart(*number));
            }
            PostEvent::SlurStop(number) => {
                self.event(EventKind::SlurStop(*number));
            }
            PostEvent::Articulation(kind) => match kind {
                ArticulationType::BreathMark => breaths.push(BreathType::Comma),
                ArticulationType::Caesura => breaths.push(BreathType::Caesura),
                other => articulations.push(other.clone()),
            },
            PostEvent::Ornament(kind) => ornaments.push(kind.clone()),
            PostEvent::Dynamic(text, placement) => {
                self.annotation(AnnotationEvent::Dynamic {
                    text: dynamic_text(text),
                    placement: *placement,
                    custom: !is_predefined_dynamic(text),
                });
            }
            PostEvent::Text(text, placement) => {
                if !text.trim().is_empty() {
                    self.annotation(AnnotationEvent::Text {
                        text: text.clone(),
                        placement: *placement,
                    });
                }
            }
            PostEvent::Fingering(text) => fingerings.push(text.clone()),
            PostEvent::HairpinStart(crescendo) => {
                self.event(EventKind::HairpinStart(if *crescendo {
                    HairpinType::Crescendo
                } else {
                    HairpinType::Diminuendo
                }))
            }
            PostEvent::HairpinStop => self.event(EventKind::HairpinStop),
            PostEvent::TrillStart => self.event(EventKind::TrillStart),
            PostEvent::TrillStop => self.event(EventKind::TrillStop),
            PostEvent::Glissando => self.event(EventKind::Glissando),
            PostEvent::Arpeggio(kind) => *arpeggio = Some(*kind),
            PostEvent::Tremolo(kind, value) => {
                let count = tremolo_count(*value);
                *tremolo = Some(Tremolo { kind: *kind, count });
            }
            PostEvent::BendAfter(value) => {
                *chord_line = Some(if *value < 0 {
                    ChordLineType::Fall
                } else {
                    ChordLineType::Doit
                });
            }
        }
    }

    /// Résout les hauteurs des notes d'un accord.
    ///
    /// Chaque note de l'accord se place par rapport à la précédente, mais la
    /// hauteur rendue à la suite est celle de la première note, comme
    /// `Event_chord::to_relative_octave` (`lily/music.cc`) : `<f d'> <d b'>`
    /// place le ré du second accord par rapport au fa du premier.
    fn chord_notes(&mut self, notes: &[NoteSpec]) -> Vec<NoteEvent> {
        let mut events = Vec::new();
        let mut reference = None;
        for (index, note) in notes.iter().enumerate() {
            let event = self.note_event(note);
            if index == 0 {
                // Capturé après la résolution : c'est l'octave de la
                // première note, transposition non comprise.
                reference = self.octave.previous;
            }
            events.extend(event);
        }
        if let Some(reference) = reference {
            self.octave.previous = Some(reference);
        }
        events
    }

    /// Construit l'événement d'une note écrite.
    fn note_event(&mut self, note: &NoteSpec) -> Option<NoteEvent> {
        let octave = self.octave.resolve(&note.pitch);
        let written_step = note.pitch.step as i32;
        let written_alter = i32::from(note.pitch.alter);
        let (step, alter, octave) = transpose_pitch(
            written_step,
            written_alter,
            i32::from(octave),
            self.transpose.0,
            self.transpose.1,
        );
        let Some(ecrite) = crate::pitch::midi_of(
            u8::try_from(step).unwrap_or(0),
            i8::try_from(alter.clamp(-127, 127)).unwrap_or(0),
            i8::try_from(octave).unwrap_or(4),
        ) else {
            self.warn("hauteur hors de la plage MIDI (0..127) : note ignorée");
            return None;
        };
        // Le `\transposition` du bloc `\midi` décale la hauteur concert de
        // toutes les voix, sans changer la notation : c'est le performer MIDI
        // de LilyPond qui le joue, et l'oracle compare à ce MIDI.
        let concert = i32::from(ecrite) + self.midi_instrument_transpose;
        let Ok(pitch) = u8::try_from(concert) else {
            self.warn("hauteur hors de la plage MIDI (0..127) : note ignorée");
            return None;
        };
        if pitch > 127 {
            self.warn("hauteur hors de la plage MIDI (0..127) : note ignorée");
            return None;
        }
        // `\transpose` transpose la musique écrite elle-même ; il ne déclare
        // pas d'instrument transpositeur. La classe tonale écrite est donc
        // celle de la hauteur transposée, comme `pitch`. Le décalage concert
        // de `\transposition` de portée est appliqué plus tard par le constructeur,
        // uniquement si la partie porte `<transpose>`.
        let tpc = crate::pitch::tpc_of(
            u8::try_from(step).unwrap_or(0),
            i8::try_from(alter.clamp(-2, 2)).unwrap_or(0),
        );
        let tie = note.posts.iter().any(|post| matches!(post, PostEvent::Tie));
        Some(NoteEvent {
            pitch,
            tpc,
            tpc_transposed: tpc,
            tie,
            cautionary: note.pitch.cautionary,
            head_group: note.head_group.clone(),
        })
    }

    /// Émet un événement de contexte.
    fn meta(&mut self, meta: MetaEvent) {
        self.event(EventKind::Meta(meta));
    }

    /// Émet une annotation.
    fn annotation(&mut self, annotation: AnnotationEvent) {
        self.event(EventKind::Annotation(annotation));
    }

    /// Émet un événement au curseur courant.
    fn event(&mut self, kind: EventKind) {
        self.voice.events.push(VoiceEvent {
            tick: self.cursor,
            order: self.order,
            kind,
        });
        self.order += 1;
    }

    /// Avertissement.
    fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(0, 0, message);
    }
}

/// Changement d'octave et d'altération d'une transposition.
fn transpose_pitch(
    step: i32,
    alter: i32,
    octave: i32,
    diatonic: i32,
    chromatic: i32,
) -> (i32, i32, i32) {
    let raw = step + diatonic;
    let octave_shift = raw.div_euclid(7);
    let new_step = raw.rem_euclid(7);
    let natural = [0, 2, 4, 5, 7, 9, 11][usize::try_from(new_step).unwrap_or(0)];
    let target = [0, 2, 4, 5, 7, 9, 11][usize::try_from(step).unwrap_or(0)] + alter + chromatic;
    let new_alter = target - natural - octave_shift * 12;
    (new_step, new_alter, octave + octave_shift)
}

/// Vrai si l'expression ne contient que des commandes de contexte — aucune
/// note, aucun `\new`, aucune parole.
///
/// Dans un `<< … >>` d'une portée, ces commandes (`\clef`, `\key`, `\time`,
/// `\bar`, `\set`…) portent sur la portée : elles ne doivent pas ouvrir une
/// voix comme le fait une branche musicale.
fn is_context_only(music: &Music) -> bool {
    match music {
        Music::Empty | Music::Unknown(_) | Music::Lyrics(_) => true,
        Music::Chord(_) | Music::New(_) => false,
        Music::Seq(items) | Music::Sim(items) => items.iter().all(is_context_only),
        Music::Context(context) => context_is_context_only(context),
    }
}

/// Vrai pour une commande de contexte sans note, y compris ses variantes qui
/// portent une musique (`\tuplet`, `\grace`, `\repeat`, `\fixed`…).
fn context_is_context_only(context: &Context) -> bool {
    match context {
        Context::Tuplet { music, .. }
        | Context::Grace { music, .. }
        | Context::Fixed { music, .. }
        | Context::Relative { music, .. }
        | Context::NonRelative { music, .. }
        | Context::Transpose { music, .. } => is_context_only(music),
        Context::AfterGrace { main, grace } => is_context_only(main) && is_context_only(grace),
        Context::Repeat {
            body, alternatives, ..
        } => is_context_only(body) && alternatives.iter().all(is_context_only),
        Context::Alternative(items) => items.iter().all(is_context_only),
        _ => true,
    }
}

/// Intervalle d'un `\transpose from to`.
fn transpose_delta(from: &PitchSpec, to: &PitchSpec) -> (i32, i32) {
    let diatonic = i32::from(to.step) - i32::from(from.step)
        + 7 * (i32::from(to.octave) - i32::from(from.octave));
    let semitones = |pitch: &PitchSpec| {
        [0, 2, 4, 5, 7, 9, 11][usize::from(pitch.step.min(6))] + i32::from(pitch.alter)
    };
    let chromatic =
        semitones(to) - semitones(from) + 12 * (i32::from(to.octave) - i32::from(from.octave));
    (diatonic, chromatic)
}

/// Décalage de quintes d'une transposition, pour transposer une armure.
///
/// La transposition du do naturel donne la classe tonale du nouvel intervalle
/// (ré pour `\transpose c d`, si bémol pour `\transpose c bf`), donc son
/// nombre de quintes.
fn fifths_shift(diatonic: i32, chromatic: i32) -> i32 {
    let (step, alter, _) = transpose_pitch(0, 0, 0, diatonic, chromatic);
    i32::from(crate::pitch::fifths_of(
        u8::try_from(step).unwrap_or(0),
        i8::try_from(alter).unwrap_or(0),
    ))
}

/// Nombre de barres d'un trémolo à partir de la figure (`:8` → 1).
fn tremolo_count(value: u8) -> u8 {
    let mut value = u32::from(value.max(8));
    let mut count = 1u8;
    while value > 8 && count < 8 {
        value /= 2;
        count += 1;
    }
    count
}

/// Texte d'une nuance.
fn dynamic_text(text: &str) -> String {
    text.trim().to_owned()
}

/// Vrai pour une nuance prédéfinie de la sortie MusicXML.
fn is_predefined_dynamic(text: &str) -> bool {
    matches!(
        text.trim(),
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
            | "n"
    )
}

/// Type de barre de mesure d'une chaîne `\bar`.
pub(crate) fn bar_line_type(text: &str) -> Option<BarLineType> {
    let kind = match text.trim() {
        "|" => BarLineType::Normal,
        "||" => BarLineType::Double,
        "|." => BarLineType::End,
        ".|" => BarLineType::StartRepeat,
        ".|:" | "|:" => BarLineType::StartRepeat,
        ":|" | ":|." => BarLineType::EndRepeat,
        ":|:|" | ":|:" => BarLineType::EndStartRepeat,
        ";" | ";;" => BarLineType::Dotted,
        "!" => BarLineType::Short,
        "-" => BarLineType::Tick,
        _ => return None,
    };
    Some(kind)
}

/// Ornement `\ottava` du modèle.
pub(crate) fn ottava_type(shift: i8) -> Option<OttavaType> {
    match shift {
        1 => Some(OttavaType::OctaveUp),
        -1 => Some(OttavaType::OctaveDown),
        2 => Some(OttavaType::FifteenthUp),
        -2 => Some(OttavaType::FifteenthDown),
        _ => None,
    }
}

/// Construit les objets d'articulation d'un accord.
pub(crate) fn articulation_objects(kinds: &[ArticulationType]) -> Vec<Articulation> {
    kinds
        .iter()
        .map(|kind| Articulation::new(kind.clone()))
        .collect()
}

/// Construit les objets d'ornement d'un accord.
pub(crate) fn ornament_objects(kinds: &[OrnamentType]) -> Vec<Ornament> {
    kinds
        .iter()
        .map(|kind| Ornament::new(kind.clone()))
        .collect()
}

/// Construit l'arpège d'un accord.
pub(crate) fn arpeggio_object(kind: ArpeggioType) -> Arpeggio {
    Arpeggio::new(kind)
}

/// Construit la respiration d'un accord.
pub(crate) fn breath_object(kind: BreathType, placement: Placement) -> Breath {
    Breath { kind, placement }
}
