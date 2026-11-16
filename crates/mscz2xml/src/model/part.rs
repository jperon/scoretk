//! Parties, portées et instruments.
//!
//! Portage de `Part` (`engraving/dom/part.h`), `Staff`
//! (`engraving/dom/staff.h`) et `Instrument` (`engraving/dom/instrument.h`).
//! Une partie possède une ou plusieurs portées ; la liste `staves` et le champ
//! [`Staff::part`] sont maintenus cohérents par [`super::Score::add_staff`].

use super::fraction::Fraction;
use super::ids::{PartId, StaffId};
use super::segment::{Clef, ClefType};

/// Instrument d'une partie.
///
/// Version minimale de `Instrument` dans `engraving/dom/instrument.h`, limitée
/// à ce que l'export MusicXML utilise pour `<score-part>` et `<transpose>`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Instrument {
    /// Identifiant MusicXML / identifiant MuseScore de l'instrument, par
    /// exemple `"keyboard.piano"`.
    pub id: String,
    /// Nom long de l'instrument.
    pub long_name: String,
    /// Nom court de l'instrument.
    pub short_name: String,
    /// Nom de piste (`Instrument::trackName`) : c'est lui que MuseScore écrit
    /// dans `<instrument-name>`.
    pub track_name: String,
    /// Transposition écrite, si l'instrument est transpositeur.
    pub transpose: Option<Transpose>,
    /// Numéro de programme MIDI (0-127), si connu.
    pub midi_program: Option<u8>,
}

impl Instrument {
    /// Construit un instrument à partir de ses identifiants et de ses noms.
    pub fn new(
        id: impl Into<String>,
        long_name: impl Into<String>,
        short_name: impl Into<String>,
    ) -> Self {
        let id = id.into();
        let long_name = long_name.into();
        let short_name = short_name.into();
        Self {
            id,
            track_name: long_name.clone(),
            long_name,
            short_name,
            transpose: None,
            midi_program: None,
        }
    }
}

/// Transposition d'un instrument.
///
/// Correspond à `Interval` / aux valeurs de transposition de `Instrument`
/// dans `engraving/dom/instrument.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Transpose {
    /// Intervalle diatonique en degrés.
    pub diatonic: i32,
    /// Intervalle chromatique en demi-tons.
    pub chromatic: i32,
}

impl Transpose {
    /// Construit une transposition.
    pub const fn new(diatonic: i32, chromatic: i32) -> Self {
        Self {
            diatonic,
            chromatic,
        }
    }
}

/// Partie de la partition.
///
/// Correspond à `Part` dans `engraving/dom/part.h`. Le champ `name` correspond
/// à `partName()` (nom de mixer), `long_name` et `short_name` aux noms affichés.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    /// Identifiant dans [`super::Score::parts`], attribué par
    /// [`super::Score::add_part`].
    pub id: PartId,
    /// Nom de la partie (mixer, identifiant MusicXML).
    pub name: String,
    /// Nom long affiché au début de la portée.
    pub long_name: String,
    /// Nom court affiché sur les portées suivantes.
    pub short_name: String,
    /// Instrument principal de la partie.
    pub instrument: Instrument,
    /// Portées appartenant à la partie, dans l'ordre.
    pub staves: Vec<StaffId>,
}

impl Part {
    /// Construit une partie avec un nom long et un nom court.
    ///
    /// Le nom de mixer reprend le nom long et l'instrument reprend les deux
    /// noms ; l'appelant peut les remplacer ensuite.
    pub fn new(long_name: impl Into<String>, short_name: impl Into<String>) -> Self {
        let long_name = long_name.into();
        let short_name = short_name.into();
        Self {
            id: PartId::from_raw(0),
            name: long_name.clone(),
            instrument: Instrument {
                long_name: long_name.clone(),
                short_name: short_name.clone(),
                track_name: long_name.clone(),
                ..Instrument::default()
            },
            long_name,
            short_name,
            staves: Vec::new(),
        }
    }
}

/// Portée d'une partie.
///
/// Correspond à `Staff` dans `engraving/dom/staff.h`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staff {
    /// Identifiant dans [`super::Score::staves`], attribué par
    /// [`super::Score::add_staff`].
    pub id: StaffId,
    /// Partie à laquelle la portée appartient.
    pub part: PartId,
    /// Nombre de lignes de la portée (5 pour une portée standard).
    pub lines: u8,
    /// Vrai si la portée est en petit format.
    pub small: bool,
    /// Étirement temporel de la portée (`Staff::timeStretch`, indication de
    /// mesure locale). Vaut un hors indication locale.
    pub time_stretch: Fraction,
    /// Clef par défaut de la portée (`Staff::defaultClefType()`).
    ///
    /// MuseScore s'en sert quand aucun `<Clef>` n'ouvre la partition : la mise
    /// en page génère alors une clef de ce type au début de la première mesure.
    /// Par défaut, la paire vaut sol/sol comme `ClefTypeList`.
    pub default_clef: Clef,
}

impl Staff {
    /// Construit une portée standard de cinq lignes pour une partie.
    pub const fn new(part: PartId) -> Self {
        Self {
            id: StaffId::from_raw(0),
            part,
            lines: 5,
            small: false,
            time_stretch: Fraction::ONE,
            default_clef: Clef::new(ClefType::G),
        }
    }
}
