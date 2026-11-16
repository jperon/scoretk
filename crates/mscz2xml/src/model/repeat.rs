//! Reprises : marqueurs, sauts et propriétés de volta.
//!
//! Portage de `Marker` (`engraving/dom/marker.h`), `Jump`
//! (`engraving/dom/jump.h`) et des propriétés de `Volta`
//! (`engraving/dom/volta.h`) utiles à l'export MusicXML. Dans MuseScore, un
//! marqueur ou un saut est un enfant direct de `Measure` (`Measure::el()`), et
//! une volta est un `Spanner` ; le modèle range les deux premiers dans
//! [`super::Measure::repeat_marks`] et la dernière dans
//! [`super::Spanner`].

use super::annotation::Placement;
use super::ids::Track;

/// Type d'un marqueur de reprise.
///
/// Correspond à `MarkerType` dans `engraving/types/types.h`. Le libellé
/// `<label>` d'un fichier `.mscx` est converti par `TConv::fromXml` ; les
/// étiquettes reconnues sont celles de la table `MARKER_TYPES` du C++
/// (`"segno"`, `"codab"`, `"coda"`…), les autres donnent [`Self::User`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkerType {
    /// Segno.
    Segno,
    /// Segno barré.
    VarSegno,
    /// Coda.
    Coda,
    /// Coda variée.
    VarCoda,
    /// Double coda.
    Codetta,
    /// Fine.
    Fine,
    /// « To Coda ».
    ToCoda,
    /// « To Coda » en symbole.
    ToCodaSym,
    /// « Da Coda ».
    DaCoda,
    /// « Da Doppia Coda ».
    DaDoubleCoda,
    /// Marqueur personnalisé (type deviné d'après les sauts à l'export).
    User,
}

impl MarkerType {
    /// Convertit un libellé `<label>` comme `TConv::fromXml`.
    pub fn from_label(label: &str) -> Self {
        match label {
            "segno" => Self::Segno,
            "varsegno" => Self::VarSegno,
            "codab" => Self::Coda,
            "varcoda" => Self::VarCoda,
            "codetta" => Self::Codetta,
            "fine" => Self::Fine,
            "coda" => Self::ToCoda,
            "codasym" => Self::ToCodaSym,
            "dacoda" => Self::DaCoda,
            "dadblcoda" => Self::DaDoubleCoda,
            _ => Self::User,
        }
    }

    /// Vrai pour les marqueurs écrits au début de la mesure
    /// (`repeatAtMeasureStart`).
    pub const fn is_start_marker(self) -> bool {
        matches!(
            self,
            Self::Segno | Self::VarSegno | Self::Coda | Self::VarCoda | Self::Codetta
        )
    }

    /// Vrai pour les marqueurs écrits à la fin de la mesure
    /// (`repeatAtMeasureStop`).
    pub const fn is_stop_marker(self) -> bool {
        matches!(
            self,
            Self::Fine | Self::ToCoda | Self::ToCodaSym | Self::DaCoda | Self::DaDoubleCoda
        )
    }
}

/// Marqueur de reprise.
///
/// Correspond à `Marker` dans `engraving/dom/marker.h`, enfant direct de la
/// mesure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marker {
    /// Type du marqueur.
    pub kind: MarkerType,
    /// Libellé `<label>`, utilisé pour apparier marqueurs et sauts.
    pub label: String,
    /// Texte affiché (`Fine`, `To Coda`…).
    pub text: String,
    /// Placement.
    pub placement: Placement,
    /// Piste d'attache (`staffIdx * VOICES`, comme `MeasureRead`).
    pub track: Track,
}

/// Type d'un saut.
///
/// Correspond à `JumpType` dans `engraving/types/types.h`, déduit à la lecture
/// de (`jumpTo`, `playUntil`, `continueAt`) par `Jump::jumpType()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JumpType {
    /// Da Capo.
    Dc,
    /// Da Capo al Fine.
    DcAlFine,
    /// Da Capo al Coda.
    DcAlCoda,
    /// Dal Segno al Coda.
    DsAlCoda,
    /// Dal Segno al Fine.
    DsAlFine,
    /// Dal Segno.
    Ds,
    /// Da Capo al Doppia Coda.
    DcAlDoubleCoda,
    /// Dal Segno al Doppia Coda.
    DsAlDoubleCoda,
    /// Dal Doppio Segno.
    Dss,
    /// Dal Doppio Segno al Coda.
    DssAlCoda,
    /// Dal Doppio Segno al Doppia Coda.
    DssAlDoubleCoda,
    /// Dal Doppio Segno al Fine.
    DssAlFine,
    /// Saut personnalisé.
    User,
}

impl JumpType {
    /// Déduit le type des trois cibles, comme `jumpTypeTable` dans
    /// `engraving/dom/jump.cpp`.
    pub fn from_targets(jump_to: &str, play_until: &str, continue_at: &str) -> Self {
        match (jump_to, play_until, continue_at) {
            ("start", "end", "") => Self::Dc,
            ("start", "fine", "") => Self::DcAlFine,
            ("start", "coda", "codab") => Self::DcAlCoda,
            ("segno", "coda", "codab") => Self::DsAlCoda,
            ("segno", "fine", "") => Self::DsAlFine,
            ("segno", "end", "") => Self::Ds,
            ("start", "varcoda", "codab") => Self::DcAlDoubleCoda,
            ("segno", "varcoda", "codab") => Self::DsAlDoubleCoda,
            ("varsegno", "end", "") => Self::Dss,
            ("varsegno", "coda", "codab") => Self::DssAlCoda,
            ("varsegno", "varcoda", "codab") => Self::DssAlDoubleCoda,
            ("varsegno", "fine", "") => Self::DssAlFine,
            _ => Self::User,
        }
    }

    /// Vrai si le saut se joue « Da Capo ».
    pub const fn is_da_capo(self) -> bool {
        matches!(
            self,
            Self::Dc | Self::DcAlFine | Self::DcAlCoda | Self::DcAlDoubleCoda
        )
    }

    /// Vrai si le saut se joue « Dal Segno ».
    pub const fn is_dal_segno(self) -> bool {
        matches!(
            self,
            Self::Ds
                | Self::DsAlCoda
                | Self::DsAlFine
                | Self::DsAlDoubleCoda
                | Self::Dss
                | Self::DssAlCoda
                | Self::DssAlDoubleCoda
                | Self::DssAlFine
        )
    }
}

/// Saut de reprise.
///
/// Correspond à `Jump` dans `engraving/dom/jump.h`, enfant direct de la mesure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jump {
    /// Type déduit des trois cibles.
    pub kind: JumpType,
    /// Texte affiché (`D.C. al Fine`…).
    pub text: String,
    /// Cible du saut (`start`, `segno`, libellé de marqueur…).
    pub jump_to: String,
    /// Marqueur où le saut s'arrête (`fine`, `coda`, libellé…).
    pub play_until: String,
    /// Marqueur où la lecture reprend (`codab`, libellé…).
    pub continue_at: String,
    /// Placement.
    pub placement: Placement,
    /// Piste d'attache.
    pub track: Track,
}

impl Jump {
    /// Construit un saut et déduit son type.
    pub fn new(
        text: impl Into<String>,
        jump_to: impl Into<String>,
        play_until: impl Into<String>,
        continue_at: impl Into<String>,
        placement: Placement,
        track: Track,
    ) -> Self {
        let jump_to = jump_to.into();
        let play_until = play_until.into();
        let continue_at = continue_at.into();
        Self {
            kind: JumpType::from_targets(&jump_to, &play_until, &continue_at),
            text: text.into(),
            jump_to,
            play_until,
            continue_at,
            placement,
            track,
        }
    }
}

/// Élément de reprise attaché à une mesure.
///
/// Préserve l'ordre du fichier, comme `Measure::el()` : l'export écrit les
/// marqueurs au début et les sauts à la fin dans cet ordre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepeatMark {
    /// Marqueur (Segno, Coda, Fine…).
    Marker(Marker),
    /// Saut (D.C., D.S.…).
    Jump(Jump),
}

/// Propriétés propres à une volta.
///
/// Correspond aux champs de `Volta` (`engraving/dom/volta.h`) lus dans le
/// `.mscx` : `<endings>` et `<endHookType>`. `Volta::voltaType()` vaut
/// `CLOSED` dès que le crochet de fin n'est pas `HookType::NONE`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoltaInfo {
    /// Numéros d'ending (`1, 2`), triés comme `Volta::setEndings`.
    pub endings: Vec<u8>,
    /// Vrai si la volta est fermée (`stop` à l'export, sinon `discontinue`).
    pub closed: bool,
}
