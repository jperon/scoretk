//! Accords, silences, articulations et éléments rythmiques d'un segment.
//!
//! Portage de `ChordRest` (`engraving/dom/chordrest.h`), `Chord`
//! (`engraving/dom/chord.h`), `Rest` (`engraving/dom/rest.h`) et
//! `Articulation` (`engraving/dom/articulation.h`).
//!
//! Les notes ne sont pas possédées par l'accord : `Chord::notes` contient des
//! [`NoteId`] vers l'arène [`super::Score::notes`], ce qui permet aux liaisons
//! de prolongation et aux autres objets de référencer une note sans pointeur
//! cyclique.

use super::duration::{Duration, DurationType};
use super::fraction::Fraction;
use super::ids::{NoteId, TupletId};
use super::lyrics::Lyrics;
use super::notation::{Arpeggio, ChordLineType, Ornament, Tremolo};

/// Mode de hampe d'un accord.
///
/// Correspond à `BeamMode` dans `engraving/types/types.h`, utilisé par
/// `ChordRest::beamMode()` dans `engraving/dom/chordrest.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BeamMode {
    /// Automatique.
    #[default]
    Auto,
    /// Pas de ligature.
    None,
    /// Début de ligature.
    Begin,
    /// Rupture des ligatures de niveau 2 et plus (`begin16`).
    Begin16,
    /// Rupture des ligatures de niveau 3 et plus (`begin32`).
    Begin32,
    /// Milieu de ligature.
    Mid,
    /// Fin de ligature.
    End,
    /// Crochet avant.
    ForwardHook,
    /// Crochet arrière.
    BackwardHook,
}

/// Direction de hampe.
///
/// Correspond à `DirectionV` / `MScore::Direction` utilisé par
/// `Chord::stemDirection()` dans `engraving/dom/chord.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StemDirection {
    /// Automatique selon la position sur la portée.
    #[default]
    Auto,
    /// Hampe vers le haut.
    Up,
    /// Hampe vers le bas.
    Down,
}

/// Type d'appoggiature ou de note d'ornement.
///
/// Correspond à `NoteType` dans `engraving/dom/chord.h` et à
/// `Chord::graceType()` / `Chord::noteType()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GraceType {
    /// Acciaccatura (petite note barrée).
    Acciaccatura,
    /// Appoggiature (petite note non barrée).
    Appoggiatura,
    /// Ornement de valeur d'une quadruple croche.
    Grace4,
    /// Ornement de valeur d'une croche, après l'accord.
    Grace8After,
    /// Ornement de valeur d'une double croche.
    Grace16,
    /// Ornement de valeur d'une double croche, après l'accord.
    Grace16After,
    /// Ornement de valeur d'une triple croche.
    Grace32,
    /// Ornement de valeur d'une triple croche, après l'accord.
    Grace32After,
}

/// Ancre verticale d'une articulation.
///
/// Correspond à `ArticulationAnchor` dans `engraving/dom/articulation.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ArticulationAnchor {
    /// Automatique.
    #[default]
    Auto,
    /// Au-dessus.
    Up,
    /// Au-dessous.
    Down,
}

/// Type d'articulation.
///
/// Correspond aux `SymId` d'articulation de `engraving/dom/articulation.h`
/// (`Articulation::symId()`), transcrits en variantes nommées.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ArticulationType {
    /// Point d'articulation (staccato).
    Staccato,
    /// Staccatissimo.
    Staccatissimo,
    /// Accent.
    Accent,
    /// Accent fort (marcato).
    Marcato,
    /// Tenuto.
    Tenuto,
    /// Portato (point et trait).
    Portato,
    /// Accent doux (`soft-accent`).
    SoftAccent,
    /// Accent doux avec staccato (`soft-accent` + `staccato`).
    SoftAccentStaccato,
    /// Accent doux avec tenuto (`soft-accent` + `tenuto`).
    SoftAccentTenuto,
    /// Accent doux avec tenuto et staccato (`soft-accent` +
    /// `detached-legato`).
    SoftAccentTenutoStaccato,
    /// Accentuation (`stress`).
    Stress,
    /// Atténuation (`unstress`).
    Unstress,
    /// Spiccato (`spiccato`).
    Spiccato,
    /// Point d'orgue.
    Fermata,
    /// Point d'orgue court.
    FermataShort,
    /// Point d'orgue long.
    FermataLong,
    /// Point d'orgue très court.
    FermataVeryShort,
    /// Point d'orgue très long.
    FermataVeryLong,
    /// Respiration.
    BreathMark,
    /// Césure.
    Caesura,
    /// Coup d'archet vers le haut.
    UpBow,
    /// Coup d'archet vers le bas.
    DownBow,
    /// Harmonique.
    Harmonic,
    /// Note ouverte.
    Open,
    /// Note étouffée.
    Stopped,
    /// Pizzicato sec (snap).
    SnapPizzicato,
    /// Autre articulation, nom MuseScore conservé tel quel.
    Other(String),
}

/// Articulation attachée à un accord.
///
/// Correspond à `Articulation` dans `engraving/dom/articulation.h`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Articulation {
    /// Type d'articulation.
    pub kind: ArticulationType,
    /// Ancre verticale.
    pub anchor: ArticulationAnchor,
    /// Vrai si l'articulation est visible ; une articulation invisible ouvre
    /// `<notations print-object="no">`.
    pub visible: bool,
    /// Sens du symbole (`Articulation::up()`), tiré du suffixe `Above` /
    /// `Below` du nom SMuFL. Sert à l'attribut `type` de `strong-accent`.
    pub up: bool,
}

impl Articulation {
    /// Construit une articulation visible avec ancre automatique.
    pub fn new(kind: ArticulationType) -> Self {
        Self {
            kind,
            anchor: ArticulationAnchor::Auto,
            visible: true,
            up: true,
        }
    }
}

/// Accord.
///
/// Correspond à `Chord` dans `engraving/dom/chord.h`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chord {
    /// Notes de l'accord, référencées dans l'arène des notes.
    pub notes: Vec<NoteId>,
    /// Accords d'agrément qui précèdent ou suivent cet accord.
    ///
    /// Correspond à `Chord::graceNotes()` dans `engraving/dom/chord.h`
    /// (`m_graceNotes`) : les accords d'agrément ne sont pas des éléments de
    /// segment, ils vivent donc ici. Leur `Chord::grace` donne le type
    /// (appoggiature, acciaccatura, ornement après l'accord…), leur
    /// [`GraceChord::duration`] leur durée. Leurs notes restent dans
    /// l'arène [`super::Score::notes`].
    pub grace_chords: Vec<GraceChord>,
    /// Direction de hampe.
    pub stem: StemDirection,
    /// Type d'ornement, si cet accord est lui-même un accord d'agrément.
    pub grace: Option<GraceType>,
    /// Vrai si l'ornement est barré.
    pub grace_slash: bool,
    /// Articulations attachées à l'accord.
    pub articulations: Vec<Articulation>,
    /// Vrai si l'accord est en petit format.
    pub small: bool,
    /// Vrai si l'accord appartient à un trémolo à deux notes
    /// (`Chord::tremoloTwoChord()`).
    pub tremolo_two_chord: bool,
    /// Ornements autonomes (`<Ornament>` enfant de `<Chord>`).
    pub ornaments: Vec<Ornament>,
    /// Trémolo de l'accord, si le fichier en donne un.
    pub tremolo: Option<Tremolo>,
    /// Arpège de l'accord (`<Arpeggio>`).
    pub arpeggio: Option<Arpeggio>,
    /// Ligne d'accord (`<ChordLine>` : chute, montée, plop, scoop).
    pub chord_line: Option<ChordLineType>,
}

/// Accord d'agrément rattaché à un accord principal.
///
/// Équivalent de chaque élément de `Chord::graceNotes()` dans
/// `engraving/dom/chord.h` : un `Chord` MuseScore est lui-même un
/// `ChordRest` et porte donc sa durée. Comme il n'est pas un élément de
/// segment, la durée est stockée à côté de l'accord plutôt que dans le
/// [`ChordRest`] du segment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraceChord {
    /// Durée notée de l'accord d'agrément (points compris dans le type
    /// [`Duration`]).
    pub duration: Duration,
    /// Accord d'agrément ; son [`Chord::grace`] indique le type.
    pub chord: Chord,
}

impl GraceChord {
    /// Construit un accord d'agrément vide du type donné.
    pub fn new(grace: GraceType, duration: Duration) -> Self {
        let chord = Chord {
            grace: Some(grace),
            ..Chord::default()
        };
        Self { duration, chord }
    }

    /// Accès à l'accord d'agrément.
    pub fn chord(&self) -> &Chord {
        &self.chord
    }

    /// Accès mutable à l'accord d'agrément.
    pub fn chord_mut(&mut self) -> &mut Chord {
        &mut self.chord
    }
}

/// Silence.
///
/// Correspond à `Rest` dans `engraving/dom/rest.h`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rest {
    /// Vrai pour le silence qui occupe toute la mesure
    /// (`DurationType::Measure`).
    pub full_measure: bool,
    /// Vrai pour un silence « gap » qui ne compte pas dans la mesure.
    pub gap: bool,
}

/// Contenu rythmique d'un accord ou d'un silence.
///
/// Correspond à `ChordRest` dans `engraving/dom/chordrest.h` : les champs
/// communs (durée, n-olet, paroles, ligature) sont portés par le wrapper, et
/// `kind` distingue un [`Chord`] d'un [`Rest`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChordRest {
    /// Figure et points.
    pub duration: Duration,
    /// Durée réelle (`DurationElement::ticks`) quand `<duration>` la donne,
    /// indépendamment de la figure.
    ///
    /// MuseScore lit `<duration>` dans les ticks de l'élément et garde la
    /// figure dans `durationType` : les deux diffèrent pour un trémolo à deux
    /// notes ou une figure étirée. `None` signifie « durée de la figure ».
    pub ticks: Option<Fraction>,
    /// N-olet auquel appartient cet élément, si c'est le cas.
    pub tuplet: Option<TupletId>,
    /// Paroles attachées à cet accord ou silence.
    pub lyrics: Vec<Lyrics>,
    /// Mode de ligature.
    pub beam_mode: BeamMode,
    /// Direction de la ligature lue dans `<Beam><StemDirection>`.
    ///
    /// MuseScore attache l'objet `Beam` lu au premier accord de la ligature ;
    /// sa direction explicite (`Beam::direction()`) l'emporte alors sur la
    /// direction calculée. `None` pour une ligature sans direction écrite
    /// (les fichiers 4.x écrivent un `<Beam>` vide).
    pub beam_direction: Option<StemDirection>,
    /// Accord ou silence.
    pub kind: ChordRestKind,
}

/// Variante d'un [`ChordRest`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChordRestKind {
    /// Accord.
    Chord(Chord),
    /// Silence.
    Rest(Rest),
}

impl ChordRest {
    /// Construit un accord vide, avec la durée donnée.
    pub fn chord(duration: Duration) -> Self {
        Self {
            duration,
            ticks: None,
            tuplet: None,
            lyrics: Vec::new(),
            beam_mode: BeamMode::Auto,
            beam_direction: None,
            kind: ChordRestKind::Chord(Chord::default()),
        }
    }

    /// Construit un silence avec la durée donnée.
    pub fn rest(duration: Duration) -> Self {
        let full_measure = duration.is_measure();
        Self {
            duration,
            ticks: None,
            tuplet: None,
            lyrics: Vec::new(),
            beam_mode: BeamMode::Auto,
            beam_direction: None,
            kind: ChordRestKind::Rest(Rest {
                full_measure,
                gap: false,
            }),
        }
    }

    /// Construit un silence de mesure entière.
    pub fn full_measure_rest() -> Self {
        Self::rest(Duration::measure())
    }

    /// Vrai si l'élément est un accord.
    pub const fn is_chord(&self) -> bool {
        matches!(self.kind, ChordRestKind::Chord(_))
    }

    /// Vrai si l'élément est un silence.
    pub const fn is_rest(&self) -> bool {
        matches!(self.kind, ChordRestKind::Rest(_))
    }

    /// Vrai si l'élément est un silence de mesure entière.
    pub fn is_full_measure_rest(&self) -> bool {
        match &self.kind {
            ChordRestKind::Rest(rest) => rest.full_measure || self.duration.is_measure(),
            ChordRestKind::Chord(_) => false,
        }
    }

    /// Accès à l'accord, si cet élément en est un.
    pub fn chord_ref(&self) -> Option<&Chord> {
        match &self.kind {
            ChordRestKind::Chord(chord) => Some(chord),
            ChordRestKind::Rest(_) => None,
        }
    }

    /// Accès mutable à l'accord, si cet élément en est un.
    pub fn chord_mut(&mut self) -> Option<&mut Chord> {
        match &mut self.kind {
            ChordRestKind::Chord(chord) => Some(chord),
            ChordRestKind::Rest(_) => None,
        }
    }

    /// Accès au silence, si cet élément en est un.
    pub fn rest_ref(&self) -> Option<&Rest> {
        match &self.kind {
            ChordRestKind::Rest(rest) => Some(rest),
            ChordRestKind::Chord(_) => None,
        }
    }

    /// Accès mutable au silence, si cet élément en est un.
    pub fn rest_mut(&mut self) -> Option<&mut Rest> {
        match &mut self.kind {
            ChordRestKind::Rest(rest) => Some(rest),
            ChordRestKind::Chord(_) => None,
        }
    }

    /// Durée notée, points compris.
    ///
    /// Pour un silence de mesure entière, vaut zéro : utiliser
    /// [`ChordRest::ticks_in_measure`] avec la longueur réelle de la mesure.
    pub fn ticks(&self) -> Fraction {
        self.duration.ticks()
    }

    /// Durée effective, en remplaçant un silence de mesure entière par la
    /// longueur de la mesure.
    pub fn ticks_in_measure(&self, measure_len: Fraction) -> Fraction {
        if self.is_full_measure_rest() {
            measure_len
        } else {
            self.ticks()
        }
    }

    /// Vrai si la figure est une mesure.
    pub const fn is_measure_duration(&self) -> bool {
        self.duration.is_measure()
    }
}

impl Default for ChordRest {
    fn default() -> Self {
        Self::rest(Duration::new(DurationType::Invalid))
    }
}
