//! Annotations de segment et placement.
//!
//! Portage minimal des objets que MuseScore range dans
//! `Segment::annotations()` (`engraving/dom/segment.h`) : dynamiques
//! (`engraving/dom/dynamic.h`), textes de portée
//! (`engraving/dom/stafftext.h`), textes système
//! (`engraving/dom/systemtext.h`), tempos (`engraving/dom/tempotext.h`),
//! repères de répétition (`engraving/dom/rehearsalmark.h`), changements
//! d'instrument (`engraving/dom/instrumentchange.h`) et accords
//! (`engraving/dom/harmony.h`).
//!
//! Seuls les champs utiles à l'export MusicXML sont conservés ; les variantes
//! peuvent être enrichies sans changer la structure du modèle.

use super::notation::Fermata;
use super::text::{RichText, TextFormat};

/// Placement vertical d'un élément.
///
/// Correspond à `PlacementV` dans `engraving/types/types.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Placement {
    /// Au-dessus de la portée.
    #[default]
    Above,
    /// Au-dessous de la portée.
    Below,
}

/// Cadre d'un repère de répétition.
///
/// Correspond à `FrameType` dans `engraving/dom/textbase.h`, propriété
/// `frameType` du style ou du texte (`ExportMusicXml::rehearsal` écrit
/// `enclosure` d'après elle).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RehearsalFrame {
    /// Cadre rectangulaire : valeur par défaut du style MuseScore 4.7, que
    /// MusicXML rend par l'absence d'attribut (`enclosure="square"` implicite).
    #[default]
    Rectangle,
    /// Cadre circulaire (`enclosure="circle"`).
    Circle,
    /// Aucun cadre (`enclosure="none"`).
    NoFrame,
}

/// Annotation attachée à un segment.
///
/// Dans MuseScore, ces objets sont dans `Segment::annotations()` et peuvent
/// appartenir à une piste précise. La variante [`Annotation::Other`] permet
/// d'attendre les types moins prioritaires sans bloquer les lecteurs.
#[derive(Clone, Debug, PartialEq)]
pub enum Annotation {
    /// Nuance (`Dynamic` dans `engraving/dom/dynamic.h`).
    Dynamic(Dynamic),
    /// Texte de portée (`StaffText` dans `engraving/dom/stafftext.h`).
    StaffText(StaffText),
    /// Texte d'expression (`Expression` dans `engraving/dom/expression.h`) :
    /// même portage que le texte de portée.
    Expression(StaffText),
    /// Texte système (`SystemText` dans `engraving/dom/systemtext.h`).
    SystemText(SystemText),
    /// Indication de tempo (`TempoText` dans `engraving/dom/tempotext.h`).
    Tempo(Tempo),
    /// Repère de répétition (`RehearsalMark` dans
    /// `engraving/dom/rehearsalmark.h`).
    RehearsalMark(RehearsalMark),
    /// Changement d'instrument (`InstrumentChange` dans
    /// `engraving/dom/instrumentchange.h`), réduit à son texte affiché.
    InstrumentChange(InstrumentChange),
    /// Symbole d'accord (`Harmony` dans `engraving/dom/harmony.h`).
    Harmony(Harmony),
    /// Point d'orgue (`Fermata` dans `engraving/dom/fermata.h`).
    Fermata(Fermata),
    /// Annotation non encore modélisée.
    Other {
        /// Nom du type MuseScore (par exemple `"Fermata"`).
        kind: String,
        /// Texte éventuel.
        text: String,
        /// Placement.
        placement: Placement,
    },
}

impl Annotation {
    /// Placement de l'annotation.
    pub const fn placement(&self) -> Placement {
        match self {
            Self::Dynamic(value) => value.placement,
            Self::StaffText(value) | Self::Expression(value) => value.placement,
            Self::SystemText(value) => value.placement,
            Self::Tempo(value) => value.placement,
            Self::RehearsalMark(value) => value.placement,
            Self::InstrumentChange(value) => value.placement,
            Self::Harmony(value) => value.placement,
            Self::Fermata(value) => value.placement,
            Self::Other { placement, .. } => *placement,
        }
    }

    /// Vrai si l'annotation est visible (donc exportée).
    pub const fn visible(&self) -> bool {
        match self {
            Self::Dynamic(value) => value.visible,
            Self::StaffText(value) | Self::Expression(value) => value.visible,
            Self::SystemText(value) => value.visible,
            Self::Tempo(value) => value.visible,
            Self::RehearsalMark(value) => value.visible,
            Self::InstrumentChange(value) => value.visible,
            Self::Harmony(_) | Self::Fermata(_) | Self::Other { .. } => true,
        }
    }

    /// Texte affiché, si l'annotation en a un.
    pub fn text(&self) -> &str {
        match self {
            Self::Dynamic(value) => &value.text,
            Self::StaffText(value) | Self::Expression(value) => value.text.plain(),
            Self::SystemText(value) => value.text.plain(),
            Self::Tempo(value) => value.text.plain(),
            Self::RehearsalMark(value) => value.text.plain(),
            Self::InstrumentChange(value) => value.text.plain(),
            Self::Harmony(value) => &value.text,
            Self::Fermata(_) => "",
            Self::Other { text, .. } => text,
        }
    }
}

/// Défauts d'un style de texte MuseScore (`<style>` d'une annotation).
///
/// Un style fixe un placement vertical et une police que le fichier `.mscx` ne
/// répète pas : `<style>expression</style>` suffit, par exemple, à rendre un
/// `StaffText` italique au-dessous de la portée. Seuls les styles utiles aux
/// annotations exportées sont décrits ; les autres gardent les valeurs par
/// défaut (au-dessus, format neutre).
pub fn text_style_defaults(style: &str) -> (Placement, TextFormat) {
    match style {
        // `TextStyleType::EXPRESSION`.
        "expression" => (
            Placement::Below,
            TextFormat {
                italic: true,
                ..TextFormat::default()
            },
        ),
        _ => (Placement::Above, TextFormat::default()),
    }
}

/// Nuance.
///
/// Correspond à `Dynamic` dans `engraving/dom/dynamic.h`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dynamic {
    /// Texte de la nuance (`"p"`, `"mf"`, `"sfz"`…).
    pub text: String,
    /// Vélocité MIDI associée (0-127), si connue.
    pub velocity: Option<u8>,
    /// Placement.
    pub placement: Placement,
    /// Vrai quand le texte est personnalisé (`hasCustomText` dans MuseScore).
    ///
    /// Un texte personnalisé est exporté en `<other-dynamics>` même s'il
    /// ressemble à une nuance MusicXML.
    pub custom: bool,
    /// Vrai si la nuance est visible (les invisibles ne sont pas exportées,
    /// `ExportMusicXml::canWrite`).
    pub visible: bool,
}

impl Dynamic {
    /// Construit une nuance au-dessous de la portée.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            velocity: None,
            placement: Placement::Below,
            custom: false,
            visible: true,
        }
    }
}

/// Texte de portée.
///
/// Correspond à `StaffText` dans `engraving/dom/stafftext.h`.
#[derive(Clone, Debug, PartialEq)]
pub struct StaffText {
    /// Texte, avec son balisage (`<b>`, `<font>`…).
    pub text: RichText,
    /// Placement.
    pub placement: Placement,
    /// Vrai si le texte est visible.
    pub visible: bool,
}

impl StaffText {
    /// Construit un texte au-dessus de la portée.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: RichText::new(text),
            placement: Placement::Above,
            visible: true,
        }
    }
}

/// Texte système.
///
/// Correspond à `SystemText` dans `engraving/dom/systemtext.h`.
#[derive(Clone, Debug, PartialEq)]
pub struct SystemText {
    /// Texte, avec son balisage.
    pub text: RichText,
    /// Placement.
    pub placement: Placement,
    /// Vrai si le texte est visible.
    pub visible: bool,
}

impl SystemText {
    /// Construit un texte système au-dessus de la portée.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: RichText::new(text),
            placement: Placement::Above,
            visible: true,
        }
    }
}

/// Indication de tempo.
///
/// Correspond à `TempoText` dans `engraving/dom/tempotext.h`.
#[derive(Clone, Debug, PartialEq)]
pub struct Tempo {
    /// Texte affiché (`"Allegro"`, `"♩ = 120"`…), avec son balisage.
    pub text: RichText,
    /// Battements par seconde, si la valeur est numérique.
    pub beats_per_second: Option<f64>,
    /// Vrai si le tempo suit les modifications de texte.
    pub follow_text: bool,
    /// Placement.
    pub placement: Placement,
    /// Vrai si le tempo est visible (un tempo invisible n'exporte que son
    /// `<sound tempo>`).
    pub visible: bool,
}

impl Tempo {
    /// Construit un tempo au-dessus de la portée.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: RichText::new(text),
            beats_per_second: None,
            follow_text: false,
            placement: Placement::Above,
            visible: true,
        }
    }
}

/// Repère de répétition.
///
/// Correspond à `RehearsalMark` dans `engraving/dom/rehearsalmark.h`.
#[derive(Clone, Debug, PartialEq)]
pub struct RehearsalMark {
    /// Texte du repère (`"A"`, `"1"`…), avec son balisage.
    pub text: RichText,
    /// Placement.
    pub placement: Placement,
    /// Cadre du repère.
    pub frame: RehearsalFrame,
    /// Vrai si le repère est visible.
    pub visible: bool,
}

impl RehearsalMark {
    /// Construit un repère au-dessus de la portée, avec le cadre du style.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: RichText::new(text),
            placement: Placement::Above,
            frame: RehearsalFrame::Rectangle,
            visible: true,
        }
    }
}

/// Changement d'instrument.
///
/// Correspond à `InstrumentChange` dans `engraving/dom/instrumentchange.h`.
/// MuseScore exporte son texte comme un `<words>` (`ExportMusicXml::words`),
/// puis les détails de l'instrument (`writeInstrumentChange`), que le modèle
/// ne porte pas encore.
#[derive(Clone, Debug, PartialEq)]
pub struct InstrumentChange {
    /// Texte affiché, avec son balisage.
    pub text: RichText,
    /// Placement.
    pub placement: Placement,
    /// Vrai si le changement est visible.
    pub visible: bool,
}

impl InstrumentChange {
    /// Construit un changement d'instrument au-dessus de la portée.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: RichText::new(text),
            placement: Placement::Above,
            visible: true,
        }
    }
}

/// Symbole d'accord.
///
/// Correspond à `Harmony` dans `engraving/dom/harmony.h`. Les hauteurs sont
/// des classes de hauteur tonale (`tpc`), comme dans MuseScore.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Harmony {
    /// Texte affiché (`"Cm7"`, `"F/A"`…).
    pub text: String,
    /// Tpc de la fondamentale, ou [`super::note::tpc::INVALID`].
    pub root_tpc: i8,
    /// Tpc de la basse, si l'accord est renversé.
    pub bass_tpc: Option<i8>,
    /// Type d'accord (chiffrage interne).
    pub kind: String,
    /// Placement.
    pub placement: Placement,
}

impl Harmony {
    /// Construit un accord au-dessus de la portée.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            root_tpc: super::note::tpc::INVALID,
            bass_tpc: None,
            kind: String::new(),
            placement: Placement::Above,
        }
    }
}
