//! Modèle de partition en mémoire.
//!
//! Ce module ne lit ni n'écrit aucun XML : il ne contient que les structures
//! manipulées par les lecteurs `.mscx` et par l'export MusicXML. Il est
//! volontairement sans entrées/sorties, sans threads, sans `Rc<RefCell<_>>` et
//! sans pointeur cyclique, afin de rester `Send + Sync`, portable et
//! compilable pour `wasm32-unknown-unknown`.
//!
//! # Temps
//!
//! Les positions et durées sont des [`Fraction`] exactes, en unités de
//! **ronde** comme dans MuseScore : `1/1` est une ronde, `1/4` une noire. Une
//! mesure 4/4 vaut donc `1/1`. [`Score::division`] donne le nombre de divisions
//! MusicXML par noire ; [`Score::tick_to_divisions`] et
//! [`Score::divisions_to_tick`] font la conversion.
//!
//! # Pistes
//!
//! Chaque [`Segment`] range ses éléments par piste ([`Track`]). La formule est
//! celle de MuseScore : `track = staff_idx * VOICES + voice`, avec
//! [`VOICES`] = 4. Les éléments non rythmiques (clef, armure, indication de
//! mesure, barre de mesure) occupent en pratique la voix 0 de leur portée.
//!
//! # Arènes et identifiants
//!
//! Les objets qui ont besoin d'être référencés depuis plusieurs endroits
//! (notes, n-olets, spanners) vivent dans des `Vec` possédées par [`Score`] et
//! sont désignés par des identifiants typés ([`NoteId`], [`TupletId`],
//! [`SpannerId`]). Les accords et silences restent possédés par leur segment,
//! comme dans MuseScore. Aucun type ne contient de pointeur vers son parent :
//! les liaisons de prolongation sont des [`NoteId`] dans [`Note`].
//!
//! # Parcours typique
//!
//! ```text
//! Score
//! ├── parts: Vec<Part>            (instrument, noms, portées)
//! ├── staves: Vec<Staff>
//! ├── measures: Vec<Measure>      (numéro, tick, len, nominal_len, barres)
//! │   └── segments: Vec<Segment>  (tick, type, élément par piste,
//! │                                annotations avec piste)
//! ├── notes: Vec<Note>            (pitch, tpc, liaisons, altération, tête)
//! ├── tuplets: Vec<Tuplet>        (ratio, base_len, imbrication)
//! └── spanners: Vec<Spanner>      (début/fin en tick et piste, type)
//! ```
//!
//! # Correspondance avec MuseScore
//!
//! Chaque type public cite, dans sa documentation, le fichier C++ dont il est
//! le portage. Les sources de référence sont dans `src/engraving/dom/`,
//! `src/engraving/types/` et
//! `src/importexport/musicxml/internal/export/exportmusicxml.cpp`.

pub mod annotation;
pub mod chord;
pub mod duration;
pub mod fraction;
pub mod hairpin;
pub mod ids;
pub mod lyrics;
pub mod measure;
pub mod note;
pub mod part;
pub mod repeat;
pub mod score;
pub mod segment;
pub mod spanner;
pub mod text;
pub mod tuplet;

pub use annotation::{
    text_style_defaults, Annotation, Dynamic, Harmony, InstrumentChange, Placement, RehearsalFrame,
    RehearsalMark, StaffText, SystemText, Tempo,
};
pub use chord::{
    Articulation, ArticulationAnchor, ArticulationType, BeamMode, Chord, ChordRest, ChordRestKind,
    GraceChord, GraceType, Rest, StemDirection,
};
pub use duration::{Duration, DurationType};
pub use fraction::{Fraction, Tick};
pub use hairpin::Hairpin;
pub use ids::{MeasureId, NoteId, PartId, SpannerId, StaffId, Track, TupletId, VOICES};
pub use lyrics::{Lyrics, LyricsSyllabic};
pub use measure::Measure;
pub use note::{tpc, Accidental, AccidentalType, Note, NoteHeadType};
pub use part::{Instrument, Part, Staff, Transpose};
pub use repeat::{Jump, JumpType, Marker, MarkerType, RepeatMark, VoltaInfo};
pub use score::{Score, ScoreMetadata, DEFAULT_DIVISION};
pub use segment::{
    BarLine, BarLineType, Breath, BreathType, Clef, ClefSign, ClefType, Key, KeyMode, KeySig,
    Segment, SegmentAnnotation, SegmentItem, SegmentType, TimeSig, TimeSigType,
};
pub use spanner::{HairpinType, OttavaType, Spanner, SpannerAnchor, SpannerType};
pub use text::{
    symbol_character, Credit, CreditJustify, CreditStyle, CreditValign, RichText, TextFormat,
    TextFragment, DEFAULT_TEXT_FONT, DEFAULT_TEXT_SIZE, MUSICAL_TEXT_FONT,
};
pub use tuplet::{Tuplet, TupletNumberType};

#[cfg(test)]
mod tests;
