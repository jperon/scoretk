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

use mscz2xml::model::{
    Arpeggio, ArpeggioType, Articulation, ArticulationType, BarLineType, Breath, BreathType,
    ChordLineType, Duration, FernataAlias as _, Fraction, GraceType, HairpinType, Key, KeyMode,
    KeySig, NoteHeadGroup, Ornament, OrnamentType, OttavaType, Placement, StemDirection, Tremolo,
    TremoloKind,
};

use super::ast::{Chord, ChordKind, Context, Music, NoteSpec, PitchSpec, PostEvent, RepeatKind};
use super::pitch::OctaveState;
use super::warnings::Warnings;

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
    Clef(mscz2xml::model::ClefType),
    /// `\key`.
    Key(KeySig),
    /// `\time`.
    Time { numerator: i32, denominator: i32 },
    /// `\partial`.
    Partial(Fraction),
    /// `\set Timing.measureLength`.
    MeasureLength(Fraction),
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
    /// Silence, visible ou non.
    Rest {
        /// Vrai si le silence est affiché.
        visible: bool,
    },
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

/// Accord ou silence daté.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChordEvent {
    /// Contenu.
    pub content: ChordContent,
    /// Figure écrite.
    pub written: Duration,
    /// Durée réelle (`ticks` du modèle), `None` si égale à la figure.
    pub ticks: Option<Fraction>,
    /// Durée réelle en fractions de ronde.
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
    /// Transposition en vigueur (degrés diatoniques, demi-tons).
    transpose: (i32, i32),
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
    /// Rang d'émission.
    order: usize,
    /// Avertissements.
    warnings: &'a mut Warnings,
    /// Langue des hauteurs.
    language: super::pitch::Language,
}

impl<'a> Interpreter<'a> {
    /// Interprète une voix.
    pub(crate) fn run(
        music: &Music,
        language: super::pitch::Language,
        warnings: &'a mut Warnings,
    ) -> InterpretedVoice {
        let mut interpreter = Self {
            voice: InterpretedVoice::default(),
            cursor: Fraction::ZERO,
            octave: OctaveState::default(),
            transpose: (0, 0),
            tuplets: Vec::new(),
            grace: None,
            pending_grace: Vec::new(),
            grace_after_target: None,
            default_duration: Duration::quarter(),
            stem_override: None,
            order: 0,
            warnings,
            language,
        };
        interpreter.walk(music);
        interpreter.voice.end = interpreter.cursor;
        interpreter.voice
    }

    /// Parcourt une expression musicale.
    fn walk(&mut self, music: &Music) {
        match music {
            Music::Empty | Music::Variable(_) | Music::Lyrics(_) => {}
            Music::New(_) => {
                self.warn("contexte imbriqué ignoré dans une voix");
            }
            Music::Seq(items) | Music::Sim(items) => {
                for item in items {
                    self.walk(item);
                }
            }
            Music::Chord(chord) => self.emit_chord(chord),
            Music::Context(context) => self.context(context),
            Music::Unknown(_) => {}
        }
    }

    /// Traite une commande de contexte.
    fn context(&mut self, context: &Context) {
        match context {
            Context::Clef(clef) => self.meta(MetaEvent::Clef(*clef)),
            Context::Key { fifths, mode } => {
                let mut key = KeySig::new(Key::new(*fifths));
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
            Context::Voice(_) => {}
            Context::Stem(direction) => {
                self.stem_override = Some(*direction);
            }
            Context::StemOverride(direction) => self.stem_override = Some(*direction),
            Context::Tuplet {
                numerator,
                denominator,
                music,
            } => self.tuplet(*numerator, *denominator, music),
            Context::Grace { kind, slash, music } => self.grace_scope(*kind, *slash, music),
            Context::AfterGrace { main, grace } => {
                self.walk(main);
                let target = self.voice.events.iter().rposition(|event| {
                    matches!(event.kind, EventKind::Chord(_))
                });
                if let Some(target) = target {
                    let saved = self.grace_after_target.replace(target);
                    self.grace = Some(GraceFrame {
                        kind: GraceType::Grace8,
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
                self.octave.relative = true;
                self.octave.previous = None;
                match reference {
                    Some(reference) => self.octave.set_reference(reference),
                    None => self.octave.reference = 4,
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
    fn tuplet(&mut self, numerator: u32, denominator: u32, music: &Music) {
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
                for _ in 0..count.max(1) {
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
        let tuplet_factor = self
            .tuplets
            .iter()
            .filter_map(|index| self.voice.tuplets.get(*index))
            .fold(Fraction::ONE, |factor, tuplet| {
                if tuplet.ratio.numerator() == 0 {
                    factor
                } else {
                    factor * Fraction::new(tuplet.ratio.denominator(), tuplet.ratio.numerator())
                }
            });
        let mut actual = written.ticks() * multiplier * tuplet_factor;

        // Un ornement ne prend pas de temps dans la mesure.
        if let Some(frame) = &self.grace {
            let notes = match &chord.kind {
                ChordKind::Notes(notes) => notes
                    .iter()
                    .filter_map(|note| self.note_event(note))
                    .collect(),
                _ => Vec::new(),
            };
            if !notes.is_empty() {
                let grace = GraceEvent {
                    notes,
                    duration: written,
                    kind: frame.kind,
                    slash: frame.slash,
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

        // Un silence de mesure entière couvre toute la mesure : le
        // constructeur le reconnaîtra d'après le silence multi-mesures.
        let content = match &chord.kind {
            ChordKind::Notes(notes) => ChordContent::Notes(
                notes
                    .iter()
                    .filter_map(|note| self.note_event(note))
                    .collect(),
            ),
            ChordKind::Rest => ChordContent::Rest { visible: true },
            ChordKind::Skip => ChordContent::Rest { visible: false },
            ChordKind::MultiRest => ChordContent::MultiRest,
        };

        let mut articulations = Vec::new();
        let mut ornaments = Vec::new();
        let mut arpeggio = None;
        let mut tremolo = None;
        let mut chord_line = None;
        let mut breaths = Vec::new();
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

        let stem = self.stem_override.take().unwrap_or(StemDirection::Auto);
        let grace_before = std::mem::take(&mut self.pending_grace);
        let ticks = if actual == written.ticks() {
            None
        } else {
            Some(actual)
        };
        if !actual.is_positive() {
            actual = Fraction::ZERO;
        }

        let event = ChordEvent {
            content,
            written,
            ticks,
            actual,
            tuplet: self.tuplets.last().copied(),
            articulations,
            ornaments,
            arpeggio,
            tremolo,
            chord_line,
            stem,
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
            PostEvent::Tie => {}
            PostEvent::SlurStart(number) => {
                self.event(EventKind::SlurStart(*number));
            }
            PostEvent::SlurStop(number) => {
                self.event(EventKind::SlurStop(*number));
            }
            PostEvent::Articulation(kind) => articulations.push(kind.clone()),
            PostEvent::Ornament(kind) => ornaments.push(kind.clone()),
            PostEvent::Fermata(kind) => {
                articulations.push(fermata_articulation(*kind));
            }
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
            PostEvent::HairpinStart(crescendo) => self.event(EventKind::HairpinStart(if *crescendo {
                HairpinType::Crescendo
            } else {
                HairpinType::Diminuendo
            })),
            PostEvent::HairpinStop => self.event(EventKind::HairpinStop),
            PostEvent::TrillStart => self.event(EventKind::TrillStart),
            PostEvent::TrillStop => self.event(EventKind::TrillStop),
            PostEvent::Glissando => self.event(EventKind::Glissando),
            PostEvent::Arpeggio(kind) => *arpeggio = Some(*kind),
            PostEvent::Tremolo(kind, value) => {
                let count = tremolo_count(*value);
                *tremolo = Some(Tremolo {
                    kind: *kind,
                    count,
                });
            }
            PostEvent::BendAfter(value) => {
                *chord_line = Some(if *value < 0 {
                    ChordLineType::Fall
                } else {
                    ChordLineType::Doit
                });
            }
            PostEvent::Breath(kind) => breaths.push(*kind),
            PostEvent::AfterGrace => {}
        }
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
        let pitch = super::pitch::midi_of(
            u8::try_from(step).unwrap_or(0),
            i8::try_from(alter.clamp(-127, 127)).unwrap_or(0),
            i8::try_from(octave).unwrap_or(4),
        )?;
        let tie = note.posts.iter().any(|post| matches!(post, PostEvent::Tie));
        Some(NoteEvent {
            pitch,
            tpc: super::pitch::tpc_of(u8::try_from(step).unwrap_or(0), i8::try_from(alter.clamp(-2, 2)).unwrap_or(0)),
            tpc_transposed: super::pitch::tpc_of(note.pitch.step, note.pitch.alter),
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

/// Intervalle d'un `\transpose from to`.
fn transpose_delta(from: &PitchSpec, to: &PitchSpec) -> (i32, i32) {
    let diatonic = i32::from(to.step) - i32::from(from.step)
        + 7 * (i32::from(to.octave) - i32::from(from.octave));
    let semitones = |pitch: &PitchSpec| {
        [0, 2, 4, 5, 7, 9, 11][usize::from(pitch.step.min(6))] + i32::from(pitch.alter)
    };
    let chromatic = semitones(to) - semitones(from) + 12 * (i32::from(to.octave) - i32::from(from.octave));
    (diatonic, chromatic)
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

/// Articulation correspondant à un point d'orgue.
fn fermata_articulation(kind: mscz2xml::model::FermataType) -> ArticulationType {
    use mscz2xml::model::FermataType;
    match kind {
        FermataType::Normal => ArticulationType::Fermata,
        FermataType::Short | FermataType::VeryShort => ArticulationType::FermataShort,
        FermataType::Long => ArticulationType::FermataLong,
        FermataType::VeryLong => ArticulationType::FermataVeryLong,
        FermataType::LongHenze | FermataType::ShortHenze | FermataType::Curlew => {
            ArticulationType::Fermata
        }
    }
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
    kinds.iter().map(|kind| Ornament::new(kind.clone())).collect()
}

/// Construit l'arpège d'un accord.
pub(crate) fn arpeggio_object(kind: ArpeggioType) -> Arpeggio {
    Arpeggio::new(kind)
}

/// Construit la respiration d'un accord.
pub(crate) fn breath_object(kind: BreathType, placement: Placement) -> Breath {
    Breath { kind, placement }
}
