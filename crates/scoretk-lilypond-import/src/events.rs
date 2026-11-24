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
    ChordLineType, Duration, DurationType, Fraction, GraceType, HairpinType, Key, KeySig,
    NoteHeadGroup, Ornament, OrnamentType, OttavaType, Placement, StemDirection, Tremolo,
};

use crate::ast::{
    Chord, ChordKind, Context, GroupBlock, GroupKind, LyricsBlock, Music, NoteSpec, PitchSpec,
    PostEvent, RepeatKind, TagFilter,
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
    /// Changement d'ottava (`\ottava n`), 0 pour la fin ; le booléen indique
    /// si le crochet est affiché (`\override OttavaBracket.transparent`).
    Ottava(i8, bool),
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
    /// `\set Timing.measurePosition` : position dans la mesure courante.
    MeasurePosition(Fraction),
    /// `\set tieWaitForNote = ##t/#f`.
    TieWaitForNote(bool),
    /// `\transposition` : hauteur jouée par un do écrit, datée.
    Transposition(PitchSpec),
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
    /// Temps réservé sans élément (`s` ou `\skip`).
    Skip {
        /// Vrai pour `s` : sa durée devient celle des notes suivantes ;
        /// faux pour `\skip <durée numérique>` (Notation Reference §2.2.2).
        sets_default: bool,
    },
    /// Silence de mesure entière ou multi-mesures ; `explicit` indique qu'une
    /// figure a été écrite après `R` (`R4.`, `R1*3`…).
    MultiRest { explicit: bool },
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
    /// Vrai pour une note de cue (musique insérée par `\cueDuring`).
    pub small: bool,
    /// Faux pour les notes d'un `NullVoice` : invisibles mais datées.
    pub visible: bool,
    /// Faux pour une musique sans performer MIDI (`ClusterNoteEvent`).
    pub play: bool,
    /// Vrai pour une musique gravée sans performer MIDI, construction
    /// LilyPond que MuseScore ne peut pas exprimer (`Note::unperformed`).
    pub unperformed: bool,
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
    /// Fraction de la note principale écoulée avant l'ornement après.
    pub fraction: Fraction,
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
    /// Vrai pour un `R` suivi d'une figure explicite (`R4.`, `R1*3`).
    pub multi_rest_explicit: bool,
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
    /// Notes du dernier élément joué (note ou accord), pour les durées
    /// seules (`expand-repeat-notes!`).
    last_chord_notes: Option<Vec<NoteEvent>>,
    /// Notes du dernier `EventChord` écrit entre chevrons, seule référence
    /// de `q` (`expand-repeat-chords!`).
    last_event_chord_notes: Option<Vec<NoteEvent>>,
    /// Durée par défaut à la fin de la voix, propagée à la musique qui
    /// suit le `<< … >>` (`lily/music-sequence.cc`).
    default_duration: Duration,
    /// Multiplicateur `*` de la durée par défaut, propagé aux notes sans
    /// durée explicite (`c4*3/2 d` vaut `d4*3/2`).
    default_multiplier: Fraction,
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
    /// Nom du contexte Voice courant (`\new Voice = "sop"`), pour `\voices`.
    voice_name: Option<String>,
    /// Position courante.
    cursor: Fraction,
    /// État d'octave.
    octave: OctaveState,
    /// Vrai pendant la lecture d'une musique de cue insérée par `\cueDuring`.
    cue: bool,
    /// Numéro du passage courant d'une reprise dépliée, pour filtrer les
    /// `\volta` (`lily/unfolded-repeat-iterator.cc`,
    /// `lily/alternative-sequence-iterator.cc`). `None` hors dépliage : la
    /// musique `\volta` reste alors dans le modèle pour la gravure.
    repeat_pass: Option<u32>,
    /// Vrai sous un `NullVoice` : les notes restent datées mais invisibles.
    invisible_notes: bool,
    /// Vrai dans une musique dont les notes ne sont pas jouées (clusters).
    silent_notes: bool,
    /// Zone `skipTypesetting` : les accords sont parcourus pour la résolution
    /// relative mais aucun événement n'est émis et le temps est replié.
    skip_event_emission: bool,
    /// Dernier état relatif atteint, avant que le `Context::Relative`
    /// englobant ne restaure l'état d'origine. Sert à enchaîner les voix d'un
    /// `<< … \ … >>` comme `music_list_to_relative` (`lily/music-sequence.cc`).
    relative_final: Option<OctaveState>,
    /// Notes résolues du dernier élément émis (note ou accord), pour les
    /// durées seules.
    last_chord_notes: Option<Vec<NoteEvent>>,
    /// Notes résolues du dernier `EventChord` (`< … >`), seule référence de
    /// `q` : une note nue ne met pas à jour la répétition d'accord.
    last_event_chord_notes: Option<Vec<NoteEvent>>,
    /// Transposition en vigueur (degrés diatoniques, demi-tons).
    transpose: (i32, i32),
    /// Décalage ajouté aux hauteurs déjà résolues par `\harmonicByFret`
    /// (`scm/tablature.scm:351-366`) : contrairement à `\transpose`, il ne
    /// décale pas la référence relative.
    harmonic_shift: i32,
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
    /// Fraction par défaut des `\afterGrace` (`afterGraceFraction`).
    after_grace_fraction: Fraction,
    /// Cible d'un `\afterGrace`.
    grace_after_target: Option<usize>,
    /// Durée par défaut.
    default_duration: Duration,
    /// Multiplicateur `*` associé à la durée par défaut.
    default_multiplier: Fraction,
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
    /// `\unfoldRepeats` : déplier les reprises volta.
    unfold_repeats: bool,
    /// Pile des tags des expressions tag traversees.
    tag_stack: Vec<Vec<String>>,
    /// Pile des filtres keepWithTag/removeWithTag actifs.
    filter_stack: Vec<(TagFilter, Vec<String>, Vec<Vec<String>>)>,
    /// Rang d'émission.
    order: usize,
    /// Avertissements.
    warnings: &'a mut Warnings,
}

impl<'a> Interpreter<'a> {
    /// Interprète une voix et rend son état relatif final, pour enchaîner les
    /// voix ou portées d'un même `<< … >>` (`lily/music-sequence.cc`).
    /// `start` est le tick de départ : zéro pour une voix neuve, la fin de la
    /// musique précédente pour un contexte nommé retrouvé dans une séquence
    /// `{ … }`.
    pub(crate) fn run(
        music: &Music,
        warnings: &'a mut Warnings,
        octave: OctaveState,
        start: Fraction,
        name: Option<&str>,
    ) -> (InterpretedVoice, OctaveState) {
        let mut interpreter = Self {
            voice: InterpretedVoice::default(),
            voice_name: name.map(str::to_owned),
            cursor: start,
            cue: false,
            repeat_pass: None,
            invisible_notes: false,
            silent_notes: false,
            skip_event_emission: false,
            octave,
            relative_final: None,
            last_chord_notes: None,
            last_event_chord_notes: None,
            transpose: (0, 0),
            harmonic_shift: 0,
            midi_instrument_transpose: 0,
            tuplets: Vec::new(),
            grace: None,
            pending_grace: Vec::new(),
            after_grace_fraction: Fraction::new(3, 4),
            grace_after_target: None,
            default_duration: Duration::quarter(),
            default_multiplier: Fraction::ONE,
            stem_override: None,
            staff_target: None,
            tuplet_scale: Fraction::ONE,
            temporary: false,
            // `\unfoldRepeats` est une fonction musicale : elle ne concerne
            // que son argument, porté par `Context::UnfoldRepeats`. Le
            // dépliage n'est plus un état global de la voix, sinon la musique
            // écrite *avant* la commande dans la même expression serait
            // dépliée aussi (`repeat-unfold-all.ly`).
            unfold_repeats: false,
            tag_stack: Vec::new(),
            filter_stack: Vec::new(),
            order: 0,
            warnings,
        };
        // Une référence relative transmise par la voix précédente d'un
        // `<< … >>` tient lieu du `\relative` sans argument qui enveloppe
        // chaque branche : `music_list_to_relative` (`lily/music-sequence.cc`)
        // enchaîne leurs dernières hauteurs. Les `\relative` internes aux
        // branches gardent leur propre ré-ancrage.
        let musique = match music {
            Music::Context(Context::Relative {
                chain: true,
                music: interior,
                ..
            }) if octave.relative && octave.previous.is_some() => interior.as_ref(),
            _ => music,
        };
        interpreter.walk(musique);
        // Un ornement écrit en fin d'expression n'a pas d'accord suivant auquel
        // s'attacher (`grace-end-expression.ly`) : `scm/music-functions.scm`
        // le joue quand même. Le rattacher au dernier accord comme ornement
        // « après » le rend audible dans le MIDI sans décaler la mesure.
        let pending = std::mem::take(&mut interpreter.pending_grace);
        if !pending.is_empty() {
            let target =
                interpreter
                    .voice
                    .events
                    .iter_mut()
                    .rev()
                    .find_map(|event| match &mut event.kind {
                        EventKind::Chord(chord) => Some(chord),
                        _ => None,
                    });
            if let Some(event) = target {
                for mut grace in pending {
                    // Une grâce de fin de voix sonne **après** la note
                    // principale, à sa fin (`grace-end.ly`) : seul un type
                    // « After » la place après à l'export, et `fraction = 1`
                    // la pose au bout de la durée de l'accord (le MIDI de
                    // référence laisse la note principale plus courte et joue
                    // les grâces sur la fin du temps).
                    grace.kind = match grace.duration.kind {
                        DurationType::Eighth => GraceType::Grace8After,
                        DurationType::Sixteenth => GraceType::Grace16After,
                        DurationType::ThirtySecond => GraceType::Grace32After,
                        _ => grace.kind,
                    };
                    grace.fraction = Fraction::ONE;
                    event.grace_after.push(grace);
                }
            } else {
                // Voix qui ne contient **que** des grâces
                // (`grace-multiple.ly`) : LilyPond les joue au moment
                // courant (`lily/grace-iterator.cc`). Sans accord porteur,
                // elles étaient perdues ; on pose un accord vide au début de
                // la voix, l'export écrit alors les `<grace>` (sans note
                // principale).
                interpreter.voice.events.push(VoiceEvent {
                    tick: start,
                    order: interpreter.order,
                    kind: EventKind::Chord(Box::new(ChordEvent {
                        content: ChordContent::Notes(Vec::new()),
                        written: Duration::quarter(),
                        explicit_ticks: None,
                        actual: Duration::quarter().ticks(),
                        multi_rest_explicit: false,
                        tuplet: None,
                        articulations: Vec::new(),
                        ornaments: Vec::new(),
                        arpeggio: None,
                        tremolo: None,
                        chord_line: None,
                        stem: StemDirection::Auto,
                        staff: None,
                        fingerings: Vec::new(),
                        grace_before: pending,
                        grace_after: Vec::new(),
                        breath: None,
                    })),
                });
                interpreter.order += 1;
                interpreter.cursor += Duration::quarter().ticks();
            }
        }
        interpreter.voice.end = interpreter.cursor;
        let octave_finale = interpreter.relative_final.unwrap_or(interpreter.octave);
        (interpreter.voice, octave_finale)
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
            Music::Sim(items) | Music::SimVoices(items) => self.simultaneous(items),
            Music::Chord(chord) => self.emit_chord(chord),
            Music::Context(context) => self.context(context),
            Music::Tag { tags, music } => {
                self.tag_stack.push(tags.clone());
                self.walk(music);
                self.tag_stack.pop();
            }
            Music::Filtered {
                filter,
                wanted,
                groups,
                music,
            } => {
                self.filter_stack
                    .push((*filter, wanted.clone(), groups.clone()));
                self.walk(music);
                self.filter_stack.pop();
            }
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
        let mut last_duration = self.default_duration;
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
                // (`generic_to_relative_octave`, `lily/music.cc`) et avec sa
                // durée par défaut (`lily/parser.yy` `NEWCONTEXT`).
                self.octave = child.octave;
                // La musique d'un contexte est un élément de la séquence : la
                // suite se résout depuis sa dernière hauteur
                // (`generic_to_relative_octave`, `lily/music.cc`). Cet état
                // doit aussi sortir du `\relative` englobant, sans quoi la
                // portée suivante d'un groupe ré-ancre sur la référence par
                // défaut (`spacing-multi-tuplet.ly` : si4 au lieu de si3).
                if self.octave.relative {
                    self.relative_final = Some(self.octave);
                }
                self.last_chord_notes = child.last_chord_notes;
                self.last_event_chord_notes = child.last_event_chord_notes.clone();
                last_duration = child.default_duration;
                self.default_multiplier = child.default_multiplier;
            }
        }
        // La musique du contexte fait partie de la séquence : elle avance la
        // voix d'origine de sa durée, comme LilyPond.
        self.cursor = end;
        self.default_duration = last_duration;
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
        // La durée par défaut est propre à chaque branche : un `s2.` écrit
        // dans la première ne doit pas donner sa durée aux accords nus de la
        // suivante (`lily/music-sequence.cc` `simultaneous_music`).
        let default_duration = self.default_duration;
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
        let mut last_duration = self.default_duration;
        for item in branches {
            self.default_duration = default_duration;
            let child = self.spawn_at(start, None, item, Vec::new(), true);
            end = end.max(child.end);
            // `music_list_to_relative` enchaîne les branches : la suivante
            // part de la dernière note de celle-ci, et la continuation du
            // `>>` de la dernière note de la dernière branche.
            self.octave = child.octave;
            self.last_chord_notes = child.last_chord_notes;
            self.last_event_chord_notes = child.last_event_chord_notes.clone();
            last_duration = child.default_duration;
            self.default_multiplier = child.default_multiplier;
        }
        self.cursor = end.max(start);
        self.default_duration = last_duration;
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
            voice_name: name.clone(),
            cursor: start,
            cue: self.cue,
            repeat_pass: self.repeat_pass,
            invisible_notes: self.invisible_notes,
            silent_notes: self.silent_notes,
            skip_event_emission: self.skip_event_emission,
            octave: self.octave,
            relative_final: None,
            transpose: self.transpose,
            harmonic_shift: self.harmonic_shift,
            midi_instrument_transpose: self.midi_instrument_transpose,
            tuplets: Vec::new(),
            grace: None,
            pending_grace: Vec::new(),
            after_grace_fraction: self.after_grace_fraction,
            grace_after_target: None,
            default_duration: self.default_duration,
            default_multiplier: self.default_multiplier,
            last_chord_notes: self.last_chord_notes.clone(),
            last_event_chord_notes: self.last_event_chord_notes.clone(),
            stem_override: self.stem_override,
            staff_target: None,
            tuplet_scale: self.tuplet_scale * self.open_tuplet_scale(),
            temporary,
            unfold_repeats: self.unfold_repeats,
            tag_stack: self.tag_stack.clone(),
            filter_stack: self.filter_stack.clone(),
            order: 0,
            warnings: &mut *self.warnings,
        };
        child.walk(music);
        // Un ornement en fin de voix sans accord suivant est transféré à
        // la voix mère : il s'attachera au prochain accord (le `emit_chord`
        // de la voix mère consomme `pending_grace`). C'est le cas d'un
        // `\volta` ne contenant que silences et ornements
        // (`volta-spec-after-grace.ly`, `scm/music-functions.scm`).
        self.pending_grace.append(&mut child.pending_grace);
        child.voice.end = child.cursor;
        let result = SpawnResult {
            end: child.cursor,
            octave: child.octave,
            last_chord_notes: child.last_chord_notes,
            last_event_chord_notes: child.last_event_chord_notes,
            default_duration: child.default_duration,
            default_multiplier: child.default_multiplier,
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
            Context::Partial(len) => {
                if len.is_positive() {
                    self.meta(MetaEvent::Partial(*len));
                }
            }
            Context::MeasureLength(length) => {
                if length.is_positive() {
                    self.meta(MetaEvent::MeasureLength(*length));
                }
            }
            Context::MeasurePosition(position) => {
                self.meta(MetaEvent::MeasurePosition(*position));
            }
            Context::TieWaitForNote(on) => self.meta(MetaEvent::TieWaitForNote(*on)),
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
            Context::Ottava(shift, visible) => self.event(EventKind::Ottava(*shift, *visible)),
            Context::OctaveCheck(pitch) => {
                if let Some(message) = self.octave.check_octave(pitch) {
                    self.warn(message);
                }
            }
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
            // `\voices` (`ly/music-functions-init.ly:2631-2665`).
            Context::Voices { ids, music } => self.voices(ids, music),
            // `\after` : `mus` avance le curseur, `ev` est joué `delta` plus
            // loin dans un contexte parallèle.
            Context::After {
                delta,
                event,
                music,
            } => {
                let start = self.cursor;
                if !event.is_empty() {
                    self.spawn_at(start + *delta, None, event, Vec::new(), true);
                }
                self.walk(music);
            }
            Context::AfterGrace {
                main,
                grace,
                fraction,
            } => {
                self.walk(main);
                let target = self
                    .voice
                    .events
                    .iter()
                    .rposition(|event| matches!(event.kind, EventKind::Chord(_)));
                if let Some(target) = target {
                    let saved = self.grace_after_target.replace(target);
                    let saved_fraction = self.after_grace_fraction;
                    self.after_grace_fraction = *fraction;
                    self.grace = Some(GraceFrame {
                        // Seul un type « After » place l'ornement après la
                        // note à l'export.
                        kind: GraceType::Grace8After,
                        slash: false,
                    });
                    self.walk(grace);
                    self.grace = None;
                    self.after_grace_fraction = saved_fraction;
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
                ..
            } => self.repeat(*kind, *count, body, alternatives),
            Context::Transpose {
                from,
                to,
                absolute,
                music,
            } => {
                let delta = transpose_delta(from, to);
                let saved_transpose = self.transpose;
                let saved_octave = self.octave;
                self.transpose = (saved_transpose.0 + delta.0, saved_transpose.1 + delta.1);
                // `TransposedMusic` porte `no-relative-callback`
                // (`scm/define-music-types.scm`) : un `\transpose` littéral
                // reste absolu même dans un `\relative`, et la référence
                // relative à la sortie du bloc est celle d'avant le bloc
                // (Notation Reference §1.2.2). Une fonction `make-relative`
                // a en revanche déjà relu ses arguments dans le contexte
                // relatif avant de les transposer : sa référence est gardée.
                if *absolute {
                    self.octave = saved_octave.sans_relative();
                }
                self.walk(music);
                self.octave = saved_octave;
                self.transpose = saved_transpose;
            }
            Context::Harmonic { semitones, music } => {
                // `calc-harmonic-pitch` ajoute l'intervalle aux notes déjà
                // résolues : ni l'octave relative ni la transposition écrite
                // environnante ne sont modifiées.
                let saved = self.harmonic_shift;
                self.harmonic_shift += i32::from(*semitones);
                self.walk(music);
                self.harmonic_shift = saved;
            }
            // `\transposition` de portée : le décalage concert est appliqué
            // par le constructeur, seulement si la partie porte `<transpose>`.
            Context::InstrumentTranspose(pitch) => self.meta(MetaEvent::Transposition(*pitch)),
            // `\transposition` du bloc `\midi` : vaut pour toute la partition
            // et s'applique ici à toutes les voix, même si la partie ne porte
            // pas `<transpose>` (plusieurs portées dans le même groupe).
            Context::MidiInstrumentTranspose(pitch) => {
                self.midi_instrument_transpose = crate::pitch::instrument_transpose(pitch).1;
            }
            // `\instrumentSwitch` pose `instrumentTransposition` à partir de
            // ce point (`ly/music-functions-init.ly:868-890`). On transpose
            // les hauteurs **écrites** des notes suivantes plutôt que de
            // poser une propriété d'instrument : le changement arrive en
            // cours de mesure (`instrument-switch.ly`) et l'écrivain MusicXML
            // ne peut exprimer `<transpose>` qu'au début d'une mesure
            // (`exportmusicxml.rs`, `write_instrument_details`).
            Context::InstrumentSwitch(pitch) => {
                let (diatonic, chromatic) = crate::pitch::instrument_transpose(pitch);
                self.transpose = (self.transpose.0 + diatonic, self.transpose.1 + chromatic);
            }
            Context::UnfoldRepeats { music } => {
                let previous = self.unfold_repeats;
                self.unfold_repeats = true;
                self.walk(music);
                self.unfold_repeats = previous;
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
            Context::Relative {
                reference, music, ..
            } => {
                let saved = self.octave;
                self.octave.start_relative(reference.as_ref());
                self.walk(music);
                self.octave = saved;
            }
            Context::Invisible { music } => {
                // `NullVoice` (`ly/performer-init.ly:229-236`) n'a que
                // Tie/Beam/Slur performers, pas de Note_performer : ses notes
                // ne sonnent pas, en plus d'être invisibles.
                let saved_visible = self.invisible_notes;
                let saved_silent = self.silent_notes;
                self.invisible_notes = true;
                self.silent_notes = true;
                self.walk(music);
                self.invisible_notes = saved_visible;
                self.silent_notes = saved_silent;
            }
            Context::PartCombine { first, second } => {
                let start = self.cursor;
                self.walk(first);
                self.spawn_at(start, None, second, Vec::new(), true);
                let mut deuxieme = self.voice.spawned.pop().expect("deuxième partie");
                crate::partcombine::retire_unissons(&self.voice.clone(), &mut deuxieme.interpreted);
                self.cursor = self.cursor.max(deuxieme.interpreted.end);
                self.voice.spawned.push(deuxieme);
            }
            Context::Silent { music } => {
                let saved = self.silent_notes;
                self.silent_notes = true;
                self.walk(music);
                self.silent_notes = saved;
            }
            Context::Skipped { music } => {
                // `skipTypesetting` : les notes sont lues (leur octave
                // relative est résolue) mais le temps de la zone est replié
                // (`score-performer.cc:151-176`).
                let saved_cursor = self.cursor;
                let saved_silent = self.silent_notes;
                let saved_emission = self.skip_event_emission;
                self.silent_notes = true;
                self.skip_event_emission = true;
                self.walk(music);
                self.cursor = saved_cursor;
                self.silent_notes = saved_silent;
                self.skip_event_emission = saved_emission;
            }
            // `\retrograde` (`ly/music-functions-init.ly:2138-2143`,
            // `scm/modal-transforms.scm:183-290`) : la musique est lue dans
            // l'ordre écrit (résolution relative comprise), puis renversée.
            Context::Retrograde { music } => self.retrograde(music),
            Context::NonRelative { music } => {
                // `make-non-relative-music` soustrait la musique à la
                // conversion `\relative` environnante (`ly:make-music-relative!`
                // ne descend pas dedans) : les hauteurs écrites restent dans
                // l'octave absolue par défaut, quelle que soit la référence du
                // `\relative`. Sous `\fixed`, en revanche, la transposition
                // s'applique bien aux parties, donc leur référence est conservée.
                let saved = self.octave;
                self.octave = saved.sans_relative();
                self.walk(music);
                self.octave = saved;
            }
            // `Quote_iterator::process` (`lily/quote-iterator.cc:161-211`) :
            // la citation est jouée au **temps absolu** de la partition. À
            // l'instant T du `\cueDuring`, `start_mom = now_mom()` et
            // `event_idx_` saute les événements antérieurs ; la citation
            // s'arrête à `T + durée de l'argument principal`. On interprète
            // donc la citation complète depuis zéro et on ne garde que les
            // accords de `[T, T + duration)`, dans la voix courante (piste de
            // la portée), avec le drapeau de cue pour les têtes réduites.
            Context::Cue {
                music, duration, ..
            } => {
                let debut = self.cursor;
                let fin = debut + *duration;
                let saved = self.cue;
                self.cue = true;
                let _ = self.spawn_at(Fraction::ZERO, None, music, Vec::new(), false);
                self.cue = saved;
                if let Some(enfant) = self.voice.spawned.pop() {
                    let child = enfant.interpreted;
                    // Les n-olets de la citation sont recopiés avec leur
                    // parent décalé ; seuls les accords de la fenêtre sont
                    // gardés.
                    let base = self.voice.tuplets.len();
                    for mut tuplet in child.tuplets {
                        if let Some(parent) = tuplet.parent {
                            tuplet.parent = Some(base + parent);
                        }
                        self.voice.tuplets.push(tuplet);
                    }
                    for mut event in child.events {
                        if event.tick < debut || event.tick >= fin {
                            continue;
                        }
                        if let EventKind::Chord(chord) = &mut event.kind {
                            if let Some(index) = chord.tuplet {
                                chord.tuplet = Some(base + index);
                            }
                        } else {
                            continue;
                        }
                        event.order = self.order;
                        self.order += 1;
                        self.voice.events.push(event);
                    }
                }
                self.cursor = fin;
            }
            // `\volta n, m musique` : LilyPond ne joue la musique que pour les
            // passages listés (`lily/unfolded-repeat-iterator.cc`,
            // `lily/alternative-sequence-iterator.cc`). Hors dépliage, la
            // musique reste dans le modèle (gravure) : `repeat_pass` est `None`.
            Context::Volta { numbers, music } => {
                let jouer = self
                    .repeat_pass
                    .is_none_or(|passage| numbers.is_empty() || numbers.contains(&passage));
                if jouer {
                    self.walk(music);
                }
            }
            // Un `\\alternative` imbriqué dans une fin de reprise (alternatives
            // de la portée d'un `\\repeat volta`) n'est pas réclamé par un
            // `\\repeat` interne : LilyPond l'utilise pour empiler les
            // crochets (`volta-bracket-nest`, Notation Reference §4.1
            // « Nested repeats »). Le modèle gravé garde toutes ses fins ;
            // les ignorer perdait leurs notes (`g1`, `a1`).
            Context::Alternative(items) => {
                for item in items {
                    self.walk(item);
                }
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
        if numerator == 0 || denominator == 0 || self.skip_event_emission {
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
                let blocks = alternatives.len();
                let mut alt_ends: Vec<OctaveState> = Vec::new();
                for tour in 1..=count.max(1) {
                    self.octave = reference;
                    let passage_sauve = self.repeat_pass.replace(tour);
                    self.walk(body);
                    let corps_fin = self.octave;
                    if blocks == 0 {
                        self.repeat_pass = passage_sauve;
                        continue;
                    }
                    // `lily/parser.yy` (REPEAT ALTERNATIVE) : moins
                    // d'alternatives que de reprises, la première est
                    // multipliée ; l'excédent est ignoré sinon
                    // (`repeat-unfold.ly`).
                    let seuil = count.max(1).saturating_sub(blocks as u32) + 1;
                    let index = if tour <= seuil {
                        0
                    } else {
                        (tour - seuil) as usize
                    }
                    .min(blocks - 1);
                    if let Some(alternative) = alternatives.get(index) {
                        self.octave = if index == 0 {
                            corps_fin
                        } else {
                            alt_ends.get(index - 1).copied().unwrap_or(corps_fin)
                        };
                        self.walk(alternative);
                        let fin = self.octave;
                        if alt_ends.len() == index {
                            alt_ends.push(fin);
                        } else if let Some(emplacement) = alt_ends.get_mut(index) {
                            *emplacement = fin;
                        }
                    }
                    self.repeat_pass = passage_sauve;
                }
            }
            RepeatKind::Ignored => {
                if self.unfold_repeats {
                    // `\unfoldRepeats` remplace les `PercentRepeatedMusic`
                    // par `UnfoldedRepeatedMusic` (`scm/music-functions.scm`,
                    // `unfold-repeats` : type `percent`), donc le corps est
                    // réellement joué n fois.
                    let reference = self.octave;
                    for _ in 0..count.max(1) {
                        self.octave = reference;
                        self.walk(body);
                    }
                } else {
                    // `lily/percent-repeat-iterator.cc` : le corps est joué
                    // une fois, puis n−1 `PercentEvent` de la longueur du
                    // corps sont seulement des places occupées (la gravure
                    // affiche des `%`). Le temps avance de n × la durée du
                    // corps, sans rejouer les notes.
                    let debut = self.cursor;
                    self.walk(body);
                    let longueur = self.cursor - debut;
                    if count > 1 && longueur > Fraction::ZERO {
                        self.cursor += longueur * Fraction::new(i64::from(count - 1), 1);
                    }
                }
            }
            // Sans `\unfoldRepeats`, le facteur de trémolo est déjà appliqué
            // aux durées par le parseur (§4.2.2) : ce bras ne peut survenir
            // que sur un arbre construit à la main, on lit le corps tel quel.
            // Sous `\unfoldRepeats`, `unfold-repeats` (`scm/music-functions.scm`)
            // en fait `UnfoldedRepeatedMusic` : le corps est joué n fois.
            RepeatKind::Tremolo => {
                if self.unfold_repeats {
                    let reference = self.octave;
                    for _ in 0..count.max(1) {
                        self.octave = reference;
                        self.walk(body);
                    }
                } else {
                    self.walk(body);
                }
            }
            RepeatKind::Volta => {
                let start_octave = self.octave;
                let blocks = alternatives.len();
                if self.unfold_repeats {
                    // `\unfoldRepeats` : le MIDI de LilyPond joue les N
                    // passages ; on les déroule dans le modèle, chaque corps
                    // repartant de la même référence relative. Les
                    // alternatives sont relatives **séquentiellement** :
                    // `make-volta-set` (scm/define-music-callbacks.scm)
                    // renvoie le corps puis un `SequentialAlternativeMusic`,
                    // et le parcours par défaut prend la fin de l'alternative
                    // précédente comme référence de la suivante.
                    let plays = count.max(2);
                    let threshold = plays.saturating_sub(blocks as u32) + 1;
                    let mut alt_ends: Vec<OctaveState> = Vec::new();
                    for pass in 1..=plays {
                        self.octave = start_octave;
                        let passage_sauve = self.repeat_pass.replace(pass);
                        self.walk(body);
                        let body_end = self.octave;
                        if blocks > 0 {
                            let index = if pass <= threshold {
                                0
                            } else {
                                (pass - threshold) as usize
                            };
                            let index = index.min(blocks - 1);
                            if let Some(alternative) = alternatives.get(index) {
                                self.octave = if index == 0 {
                                    body_end
                                } else {
                                    alt_ends.get(index - 1).copied().unwrap_or(body_end)
                                };
                                self.walk(alternative);
                                let end = self.octave;
                                if alt_ends.len() == index {
                                    alt_ends.push(end);
                                } else if let Some(slot) = alt_ends.get_mut(index) {
                                    *slot = end;
                                }
                            }
                        }
                        self.repeat_pass = passage_sauve;
                    }
                } else {
                    // Sans `\unfoldRepeats`, le MIDI joue le corps et les
                    // endings une fois, mais la partition gravée garde ses
                    // marques de reprise : on les modélise pour l'export.
                    // `\volta` appartient à la reprise **innermost** : on
                    // annule le passage hérité d'un `\repeat unfold`
                    // englobant, sinon ses numéros filtreraient les branches
                    // de cette reprise non dépliée
                    // (`volta-spec-volta-in-unfold`).
                    let passage_sauve = self.repeat_pass.take();
                    let start = self.cursor;
                    self.walk(body);
                    let body_end = self.cursor;
                    let body_octave = self.octave;
                    let mut sections = Vec::new();
                    let mut end = body_end;
                    let mut alt_octave = body_octave;
                    // `\repeat volta n` avec plus de n alternatives :
                    // « More alternatives than repeats. Junking excess
                    // alternatives » (`lily/parser.yy` REPEAT ALTERNATIVE) ;
                    // les surnuméraires ne sont ni gravées ni jouées.
                    let gardees = alternatives.len().min(count.max(1) as usize);
                    for (index, alternative) in alternatives.iter().take(gardees).enumerate() {
                        let alt_start = self.cursor;
                        self.octave = alt_octave;
                        self.walk(alternative);
                        alt_octave = self.octave;
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
                    self.repeat_pass = passage_sauve;
                }
            }
        }
    }

    /// Émet un accord, un silence ou un ornement.
    fn emit_chord(&mut self, chord: &Chord) {
        let written = chord.duration.unwrap_or(self.default_duration);
        // Un `*n/d` explicite devient la durée par défaut des notes suivantes
        // (`lily/parser.yy` `DURATION` : `c4*3/2 d` vaut `d4*3/2`). Écrire une
        // durée réinitialise le multiplicateur, comme chez LilyPond.
        let multiplier = chord.multiplier.unwrap_or(if chord.duration.is_some() {
            Fraction::ONE
        } else {
            self.default_multiplier
        });
        // `\scaleDurations n/d { … }` modifie la musique contenue, pas la
        // durée par défaut du flux après la commande : la note nue qui suit
        // le bloc ne doit pas hériter du facteur d'échelle.
        let default_multiplier = if chord.from_scale {
            self.default_multiplier
        } else {
            multiplier
        };
        let tuplet_factor = self.tuplet_scale * self.open_tuplet_scale();
        let mut actual = written.ticks() * multiplier * tuplet_factor;

        // Un ornement ne prend pas de temps dans la mesure.
        if let Some((kind, slash)) = self.grace.as_ref().map(|frame| (frame.kind, frame.slash)) {
            let mut notes = match &chord.kind {
                ChordKind::Notes(notes) => self.chord_notes(notes),
                // Une durée nue (`\grace { 8 }`) est résolue par
                // `expand-repeat-notes!` avant l'enveloppement dans
                // `GraceMusic` (`scm/music-functions.scm:972-1015`,
                // `:1237-1248`) : elle rejoue la note précédente, comme dans
                // le flux normal (ci-dessous, `ChordKind::PitchRepeat`).
                ChordKind::PitchRepeat => match self.last_chord_notes.clone() {
                    Some(mut notes) => {
                        for note in &mut notes {
                            note.tie = false;
                        }
                        notes
                    }
                    None => Vec::new(),
                },
                _ => Vec::new(),
            };
            // `def-grace-function` (`scm/music-functions.scm:1237-1248`) place
            // le `~` sur le accord du bloc d'agrément (`event_chord` +
            // `Tie_engraver`) ; comme pour un accord normal (`:1209-1213`), il
            // doit marquer toutes ses notes, sinon la liaison vers la note
            // principale est perdue avant même le constructeur.
            let grace_tie = chord
                .posts
                .iter()
                .any(|post| matches!(post, PostEvent::Tie));
            if grace_tie {
                for note in &mut notes {
                    note.tie = true;
                }
            }
            // `tie-grace.ly` : `c8 ~ 16 ~` est une chaîne de liaisons entre
            // deux grâces de même hauteur. `Tie_performer` n'émet qu'une
            // attaque pour cette chaîne (`lily/tie-performer.cc`), mais le
            // MusicXML devrait sinon écrire deux `<grace>` : ne pas ajouter
            // la seconde quand elle prolonge exactement la précédente.
            if grace_tie
                && self.pending_grace.last().is_some_and(|previous| {
                    previous.notes.len() == notes.len()
                        && previous
                            .notes
                            .iter()
                            .zip(&notes)
                            .all(|(before, after)| before.pitch == after.pitch)
                })
            {
                self.default_duration = written;
                self.default_multiplier = default_multiplier;
                return;
            }
            // Un agrément de fin lié à sa note principale ne joue pas de
            // seconde attaque : `Tie_performer::listen_tie` referme la tête
            // en attente sur la note suivante de même hauteur
            // (`lily/tie-performer.cc:94-135`). Sans cette règle,
            // `v4_linked-ties.ly` (`\afterGrace 1 b4~ { b8 }`) écrivait un
            // `<grace>` supplémentaire que l'oracle comptait comme une note.
            if let Some(target) = self.grace_after_target {
                let main_tied: Option<Vec<u8>> = match self.voice.events.get(target) {
                    Some(VoiceEvent {
                        kind: EventKind::Chord(chord),
                        ..
                    }) => match &chord.content {
                        ChordContent::Notes(main) => Some(
                            main.iter()
                                .filter(|note| note.tie)
                                .map(|note| note.pitch)
                                .collect(),
                        ),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(tied) = main_tied {
                    if !tied.is_empty()
                        && !notes.is_empty()
                        && notes.iter().all(|note| tied.contains(&note.pitch))
                    {
                        self.default_duration = written;
                        self.default_multiplier = default_multiplier;
                        return;
                    }
                }
            }
            if !notes.is_empty() {
                let grace = GraceEvent {
                    notes,
                    duration: written,
                    kind,
                    slash,
                    fraction: self.after_grace_fraction,
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
            self.default_multiplier = default_multiplier;
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
        // constructeur le reconnaîtra d'après le silence multi-mesures. La
        // figure explicite est conservée pour distinguer `R4.` (silence daté)
        // de `R` nu (`lily/parser.yy:3117`).
        let multi_rest_explicit = matches!(&chord.kind, ChordKind::MultiRest { explicit: true });
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
                    // Les durées seules reprennent la dernière note ou le
                    // dernier accord (`expand-repeat-notes!`) ; `q` ne
                    // reprend qu'un `EventChord` (`< … >`,
                    // `expand-repeat-chords!`, `scm/music-functions.scm`
                    // l. 920-945) : une note nue laisse la référence de `q`
                    // inchangée (`czerny-op840-4` : `g4 < b d > q g q q`).
                    self.last_chord_notes = Some(notes.clone());
                    if chord.bracketed {
                        self.last_event_chord_notes = Some(notes.clone());
                    }
                    ChordContent::Notes(notes)
                }
            }
            // `q` reprend les hauteurs déjà résolues du dernier `EventChord` :
            // les résoudre de nouveau depuis ses marques écrites les
            // décalerait d'une octave à chaque répétition.
            ChordKind::Repeat => match self.last_event_chord_notes.clone() {
                Some(mut notes) => {
                    // `q` rejoue les hauteurs du dernier accord, pas ses
                    // prolongations : `repetition-chord` ne pose que la durée
                    // et les articulations (`scm/ly-syntax-constructors.scm`
                    // l. 228-232) ; la note répétée *ferme* une liaison
                    // éventuelle mais n'en commence pas une nouvelle. Sans
                    // cela, `link_ties` prolongeait la liaison du `q` vers
                    // l'accord suivant (`catacombae`, A3 avalé).
                    for note in &mut notes {
                        note.tie = false;
                    }
                    // Le `q` rempli est lui-même un accord : il devient la
                    // référence des deux mécanismes de répétition.
                    self.last_chord_notes = Some(notes.clone());
                    self.last_event_chord_notes = Some(notes.clone());
                    ChordContent::Notes(notes)
                }
                None => {
                    // `expand-repeat-chords!` avertit « Bad chord repetition »
                    // et ne produit aucune note.
                    self.warn("q sans accord précédent : répétition ignorée");
                    ChordContent::Notes(Vec::new())
                }
            },
            // Durée seule : elle reprend la dernière note **ou** le dernier
            // accord (`expand-repeat-notes!`, l. 972-1015) ; elle ne devient
            // pas la référence de `q`.
            ChordKind::PitchRepeat => match self.last_chord_notes.clone() {
                Some(mut notes) => {
                    for note in &mut notes {
                        note.tie = false;
                    }
                    ChordContent::Notes(notes)
                }
                None => {
                    self.warn("durée seule sans note précédente : répétition ignorée");
                    ChordContent::Notes(Vec::new())
                }
            },
            ChordKind::Rest => ChordContent::Rest {
                visible: !chord.invisible,
            },
            ChordKind::Skip => ChordContent::Skip { sets_default: true },
            ChordKind::SkipNumeric => ChordContent::Skip {
                sets_default: false,
            },
            ChordKind::MultiRest { explicit } => ChordContent::MultiRest {
                explicit: *explicit,
            },
        };
        if let ChordContent::Notes(notes) = &mut content {
            for note in notes {
                note.tie |= chord_tie;
            }
        }
        if self.skip_event_emission {
            // `chord_notes` a déjà résolu les hauteurs et mis à jour la
            // référence relative et la durée par défaut. Le performer ne
            // traite pas la zone : ni événement, ni temps, mais l'octave
            // relative de la suite est bien celle de la dernière note lue.
            self.default_duration = written;
            self.default_multiplier = default_multiplier;
            return;
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

        // `s` et `\skip` réservent du temps sans rien afficher : on ne crée
        // aucun élément (l'export écrit `<forward>`). Notation Reference §2.2.2 :
        // `s4` fixe la durée des notes suivantes, `\skip 4` (durée numérique)
        // non. Un silence transparent reste un `Rest` invisible, distinct.
        if let ChordContent::Skip { sets_default } = content {
            // Un `s` étiqueté retiré par le filtre ne réserve pas de temps :
            // LilyPond filtre l'arbre musical avant l'interprétation, donc
            // ni le curseur ni la durée par défaut ne doivent être touchés.
            if !self.tags_pass_filters() {
                return;
            }
            self.cursor += actual;
            if sets_default {
                self.default_duration = written;
                self.default_multiplier = default_multiplier;
            }
            return;
        }
        let stem = self.stem_override.take().unwrap_or(StemDirection::Auto);
        let grace_before = std::mem::take(&mut self.pending_grace);
        // Un multiplicateur `*n/d`, écrit ou hérité de la durée par défaut,
        // change la durée réelle : elle doit être portée par `ticks`, sinon
        // l'export écrit la figure nue et décale toutes les attaques suivantes.
        // La mise à l'échelle d'un n-olet reste portée par le `Tuplet`, sauf
        // celle qui vient d'un n-olet **extérieur à la voix** : une branche de
        // `<< >>` ouverte dans un `\times` est lue par une voix sœur qui n'a
        // pas le `Tuplet` dans ses propres n-olets (`tuplet_scale` seul). Sans
        // `ticks`, l'export écrivait la figure nue (0,5 au lieu de 1/3) et
        // allongeait la mesure (waltz-op39-15, rimsky-korsakov-op11-1).
        let base_ticks = written.ticks() * multiplier * self.tuplet_scale;
        let explicit_ticks = if base_ticks != written.ticks() {
            Some(base_ticks)
        } else {
            None
        };
        if !actual.is_positive() {
            actual = Fraction::ZERO;
        }

        let event = ChordEvent {
            content,
            written,
            explicit_ticks,
            actual,
            multi_rest_explicit,
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
        if !self.tags_pass_filters() {
            // Le filtre retire ces notes AVANT l'interprétation chez LilyPond :
            // elles ne consomment pas de temps. Leur octave a déjà été
            // résolue par `chord_notes` (le `\relative` porte sur tout
            // l'arbre), ce qui est précisément l'effet recherché.
            return;
        }
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
        self.default_multiplier = default_multiplier;
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
            // Capturer avant que le contexte relatif englobant ne restaure
            // son état : la voix suivante en hérite.
            if self.octave.relative {
                self.relative_final = Some(self.octave);
            }
        }
        events
    }

    /// Construit l'événement d'une note écrite.
    fn note_event(&mut self, note: &NoteSpec) -> Option<NoteEvent> {
        // Dans `\drummode` et `\chordmode`, le nom est un événement dont la
        // hauteur est déjà MIDI : ni octave relative, ni `\transpose`, ni
        // `\transposition` de portée ne s'appliquent (le performer MIDI de
        // LilyPond joue directement la hauteur de son table d'instruments).
        if let Some(pitch) = note.midi {
            // Un nom de `\chordmode` se transpose : `\transpose c g,` décale
            // la fondamentale et tous les degrés (LilyPond transpose la
            // musique avant de réaliser l'accord). La percussion, elle, garde
            // la hauteur de sa table.
            let pitch = if note.transposable {
                // La table des accords donne la fondamentale à l'octave 4.
                // Sous `\fixed`, LilyPond déplace cette octave de référence :
                // `\fixed c,` joue `f` à F3 (`set_reference` dans
                // `pitch.rs`, `ly/parser.yy` `chord_body`). La percussion,
                // elle, garde la hauteur absolue de sa table.
                let fixed_shift = if self.octave.relative {
                    0
                } else {
                    (i32::from(self.octave.reference) - 3) * 12
                };
                u8::try_from(
                    (i32::from(pitch) + self.transpose.1 + self.harmonic_shift + fixed_shift)
                        .clamp(0, 127),
                )
                .unwrap_or(pitch)
            } else {
                pitch
            };
            let naturels = [0, 2, 4, 5, 7, 9, 11];
            let semiton = i32::from(pitch) % 12;
            let step = naturels
                .iter()
                .rposition(|naturel| *naturel <= semiton)
                .unwrap_or(0) as u8;
            let alter = (semiton - naturels[usize::from(step)]) as i8;
            let tpc = crate::pitch::tpc_of(step, alter);
            return Some(NoteEvent {
                pitch,
                tpc,
                tpc_transposed: tpc,
                tie: note.posts.iter().any(|post| matches!(post, PostEvent::Tie)),
                cautionary: note.pitch.cautionary,
                head_group: note.head_group.clone(),
                small: self.cue,
                visible: !self.invisible_notes,
                play: !self.silent_notes,
                unperformed: self.silent_notes || self.invisible_notes,
            });
        }
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
        )
        .and_then(|pitch| u8::try_from(i32::from(pitch) + self.harmonic_shift).ok()) else {
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
            small: self.cue,
            visible: !self.invisible_notes,
            play: !self.silent_notes,
            unperformed: self.silent_notes || self.invisible_notes,
        })
    }

    /// Vrai si les tags courants passent tous les filtres actifs.
    fn tags_pass_filters(&self) -> bool {
        let tags: Vec<String> = self.tag_stack.iter().flatten().cloned().collect();
        self.filter_stack.iter().all(|(filter, wanted, groups)| {
            crate::parser::tags_kept(&tags, wanted, *filter, groups)
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
        if !self.tags_pass_filters() {
            return;
        }
        self.voice.events.push(VoiceEvent {
            tick: self.cursor,
            order: self.order,
            kind,
        });
        self.order += 1;
    }

    /// `\voices` (`ly/music-functions-init.ly:2631-2665`) : chaque branche du
    /// `<< … \\ … >>` est affectée à une voix. Une branche affectée au nom de
    /// la voix courante la prolonge : ses événements restent dans la même
    /// voix, donc ses liaisons se lient à la musique qui précède et suit
    /// (`voices-command.ly`).
    fn voices(&mut self, ids: &[crate::ast::VoicesId], music: &Music) {
        let branches: &[Music] = match music {
            Music::Sim(items) | Music::SimVoices(items) => items,
            _ => {
                self.walk(music);
                return;
            }
        };
        let start = self.cursor;
        let mut end = start;
        let mut prolongee = false;
        let mut etat_continue = None;
        for (index, branch) in branches.iter().enumerate() {
            let nom = ids.get(index).and_then(crate::ast::VoicesId::name);
            if !prolongee && nom.is_some() && nom == self.voice_name.as_deref() {
                prolongee = true;
                self.walk(branch);
                end = end.max(self.cursor);
                etat_continue = Some((
                    self.octave,
                    self.last_chord_notes.clone(),
                    self.last_event_chord_notes.clone(),
                    self.default_duration,
                    self.default_multiplier,
                ));
                continue;
            }
            let child = self.spawn_at(start, nom.map(str::to_owned), branch, Vec::new(), true);
            end = end.max(child.end);
            self.octave = child.octave;
            self.last_chord_notes = child.last_chord_notes;
            self.last_event_chord_notes = child.last_event_chord_notes;
            self.default_duration = child.default_duration;
            self.default_multiplier = child.default_multiplier;
        }
        if let Some((octave, notes, accords, duree, multiplier)) = etat_continue {
            self.octave = octave;
            self.last_chord_notes = notes;
            self.last_event_chord_notes = accords;
            self.default_duration = duree;
            self.default_multiplier = multiplier;
        }
        self.cursor = end.max(start);
    }

    /// `\retrograde` (`ly/music-functions-init.ly:2138-2143`,
    /// `retrograde-music` dans `scm/modal-transforms.scm:183-290`).
    ///
    /// L'argument est d'abord interprété dans l'ordre écrit : LilyPond y
    /// résout les octaves relatives avant de renverser la liste des éléments
    /// (vérifié : `\relative c' { c4 \retrograde { b c } d }` joue C4 B3,
    /// l'ordre écrit donnant `b` = B3 puis `c` = C4). Les événements datés
    /// sont ensuite renversés par [`reverse_voice`].
    fn retrograde(&mut self, music: &Music) {
        if self.grace.is_some() {
            self.warn("\\retrograde dans un ornement ignoré");
            self.walk(music);
            return;
        }
        let start = self.cursor;
        // Même amorçage qu'une voix sœur (`spawn_at`) : l'argument hérite de
        // l'octave, de la transposition et des durées par défaut, mais ses
        // événements sont renversés avant d'être rendus à la voix courante.
        let mut child = Interpreter {
            voice: InterpretedVoice::default(),
            voice_name: self.voice_name.clone(),
            cursor: start,
            cue: self.cue,
            repeat_pass: self.repeat_pass,
            invisible_notes: self.invisible_notes,
            silent_notes: self.silent_notes,
            skip_event_emission: self.skip_event_emission,
            octave: self.octave,
            relative_final: None,
            transpose: self.transpose,
            harmonic_shift: self.harmonic_shift,
            midi_instrument_transpose: self.midi_instrument_transpose,
            tuplets: Vec::new(),
            grace: None,
            pending_grace: Vec::new(),
            after_grace_fraction: self.after_grace_fraction,
            grace_after_target: None,
            default_duration: self.default_duration,
            default_multiplier: self.default_multiplier,
            last_chord_notes: self.last_chord_notes.clone(),
            last_event_chord_notes: self.last_event_chord_notes.clone(),
            stem_override: self.stem_override,
            staff_target: None,
            tuplet_scale: self.tuplet_scale * self.open_tuplet_scale(),
            temporary: self.temporary,
            unfold_repeats: self.unfold_repeats,
            tag_stack: self.tag_stack.clone(),
            filter_stack: self.filter_stack.clone(),
            order: 0,
            warnings: &mut *self.warnings,
        };
        child.walk(music);
        child.voice.end = child.cursor;
        self.pending_grace.append(&mut child.pending_grace);
        let mut voice = child.voice;
        let end = voice.end;
        reverse_voice(&mut voice, start, end);
        // La suite de la musique se résout à partir de la dernière note
        // **écrite** de l'argument, comme sans `\retrograde`.
        self.octave = child.octave;
        if child.relative_final.is_some() {
            self.relative_final = child.relative_final;
        }
        self.last_chord_notes = child.last_chord_notes;
        self.last_event_chord_notes = child.last_event_chord_notes;
        self.default_duration = child.default_duration;
        self.default_multiplier = child.default_multiplier;
        // Fusion dans la voix courante : les n-olets de l'argument gardent
        // leurs index en décalant leurs parents.
        let base = self.voice.tuplets.len();
        for mut tuplet in voice.tuplets {
            if let Some(parent) = tuplet.parent {
                tuplet.parent = Some(parent + base);
            }
            self.voice.tuplets.push(tuplet);
        }
        for event in &mut voice.events {
            if let EventKind::Chord(chord) = &mut event.kind {
                if let Some(index) = chord.tuplet {
                    chord.tuplet = Some(index + base);
                }
            }
        }
        self.voice.events.extend(voice.events);
        self.voice.repeats.extend(voice.repeats);
        self.voice.spawned.extend(voice.spawned);
        self.voice.end = self.voice.end.max(voice.end);
        self.cursor = self.cursor.max(voice.end);
        for (order, event) in self.voice.events.iter_mut().enumerate() {
            event.order = order;
        }
    }

    /// Avertissement.
    fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(0, 0, message);
    }
}

/// Renverse une voix interprétée autour de l'intervalle `[start, end]`.
///
/// Port de `retrograde-music` (`scm/modal-transforms.scm:183-290`) au niveau
/// des événements. La liste des éléments est lue à l'envers : chaque accord
/// occupe le miroir de sa propre durée (les durées restent avec leur
/// élément), et les événements ponctuels restent attachés à l'accord qui les
/// portait. `reverse-span!` (l. 189-200) inverse le sens des soufflets, des
/// liaisons d'expression et des pédales ; `filter-ties!` (l. 203-221) reporte
/// les liaisons de prolongation sur la note qui les suit.
fn reverse_voice(voice: &mut InterpretedVoice, start: Fraction, end: Fraction) {
    let span = start + end;
    // Durée de l'accord qui commence à chaque tick, pour replacer les
    // événements ponctuels émis avec lui (liaisons, soufflets, pédales).
    let mut durations: std::collections::BTreeMap<Fraction, Fraction> =
        std::collections::BTreeMap::new();
    // Notes de la voix dans l'ordre écrit, pour retrouver les liaisons.
    let mut notes: Vec<(u8, usize, usize, Fraction, Fraction, bool)> = Vec::new();
    for (event_index, event) in voice.events.iter().enumerate() {
        let EventKind::Chord(chord) = &event.kind else {
            continue;
        };
        durations.entry(event.tick).or_insert(chord.actual);
        let ChordContent::Notes(chord_notes) = &chord.content else {
            continue;
        };
        for (note_index, note) in chord_notes.iter().enumerate() {
            notes.push((
                note.pitch,
                event_index,
                note_index,
                event.tick,
                event.tick + chord.actual,
                note.tie,
            ));
        }
    }
    notes.sort_by_key(|note| (note.3, note.1, note.2));
    // Liaisons : la note liée vise la première note de même hauteur qui
    // commence à sa fin (`lily/tie-performer.cc`, `link_ties`).
    let mut edges: Vec<(usize, usize, usize, usize)> = Vec::new();
    for (index, note) in notes.iter().enumerate() {
        if !note.5 {
            continue;
        }
        if let Some(offset) = notes[index + 1..]
            .iter()
            .position(|next| next.0 == note.0 && next.3 == note.4)
        {
            let target = &notes[index + 1 + offset];
            edges.push((note.1, note.2, target.1, target.2));
        }
    }
    for event in &mut voice.events {
        if let EventKind::Chord(chord) = &mut event.kind {
            if let ChordContent::Notes(chord_notes) = &mut chord.content {
                for note in chord_notes {
                    note.tie = false;
                }
            }
        }
    }
    // Renversement des positions.
    for event in &mut voice.events {
        let original = event.tick;
        let duration = durations.get(&original).copied();
        let new_tick = match &event.kind {
            EventKind::Chord(chord) => span - original - chord.actual,
            _ => span - original - duration.unwrap_or(Fraction::ZERO),
        };
        // `reverse-span!` : un départ devient une fin, et réciproquement.
        let swapped = match &event.kind {
            EventKind::SlurStart(number) => Some(EventKind::SlurStop(*number)),
            EventKind::SlurStop(number) => Some(EventKind::SlurStart(*number)),
            EventKind::HairpinStart(_) => Some(EventKind::HairpinStop),
            EventKind::HairpinStop => Some(EventKind::HairpinStart(HairpinType::Crescendo)),
            EventKind::TrillStart => Some(EventKind::TrillStop),
            EventKind::TrillStop => Some(EventKind::TrillStart),
            EventKind::Pedal(on) => Some(EventKind::Pedal(!*on)),
            _ => None,
        };
        if let Some(kind) = swapped {
            event.kind = kind;
        }
        event.tick = new_tick;
    }
    // Liaisons retournées : la cible d'origine devient la note liée, et
    // l'ancienne note liée la cible (elles restent adjacentes et de même
    // hauteur après le miroir).
    for (_, _, target_event, target_note) in edges {
        if let Some(EventKind::Chord(chord)) =
            voice.events.get_mut(target_event).map(|e| &mut e.kind)
        {
            if let ChordContent::Notes(chord_notes) = &mut chord.content {
                if let Some(note) = chord_notes.get_mut(target_note) {
                    note.tie = true;
                }
            }
        }
    }
    voice.events.sort_by_key(|event| (event.tick, event.order));
    for (order, event) in voice.events.iter_mut().enumerate() {
        event.order = order;
    }
    // Chaque voix simultanée est renversée dans sa propre durée
    // (`simultaneous-music`, l. 271-276 : les éléments sont parcourus un à
    // un, sans croiser les branches). Le bloc garde sa place dans la
    // séquence renversée : ses branches se replacent sur le miroir de leur
    // durée, d'où le même intervalle que la voix englobante (une branche de
    // longueur égale au bloc — le cas courant — se renverse exactement).
    let spawned = std::mem::take(&mut voice.spawned);
    for mut child in spawned {
        reverse_voice(&mut child.interpreted, start, end);
        voice.spawned.push(child);
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
        Music::Seq(items) | Music::Sim(items) | Music::SimVoices(items) => {
            items.iter().all(is_context_only)
        }
        Music::Context(context) => context_is_context_only(context),
        Music::Tag { music, .. } | Music::Filtered { music, .. } => is_context_only(music),
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
        | Context::Retrograde { music, .. }
        | Context::Transpose { music, .. }
        | Context::Harmonic { music, .. } => is_context_only(music),
        Context::AfterGrace { main, grace, .. } => is_context_only(main) && is_context_only(grace),
        Context::After { event, music, .. } => is_context_only(event) && is_context_only(music),
        Context::Voices { music, .. } => is_context_only(music),
        Context::Volta { music, .. } => is_context_only(music),
        Context::Repeat {
            body, alternatives, ..
        } => is_context_only(body) && alternatives.iter().all(is_context_only),
        Context::Alternative(items) => items.iter().all(is_context_only),
        Context::PartCombine { .. } => false,
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
    Breath {
        kind,
        placement,
        pause_ms: None,
    }
}
