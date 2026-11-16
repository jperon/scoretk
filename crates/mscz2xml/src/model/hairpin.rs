//! Détails complémentaires d'un soufflet.
//!
//! [`Hairpin`] complète [`super::spanner::Spanner`] pour tout ce que
//! `engraving/dom/hairpin.h` porte et que l'export MusicXML utilise :
//! distinction entre soufflet dessiné (`<wedge>`) et soufflet en texte
//! (`<dashes>`), niente, placement, textes de début et de fin.
//!
//! Le type (crescendo/diminuendo) reste dans
//! [`super::spanner::SpannerType::Hairpin`] ; ce module ne le duplique pas.

use super::annotation::Placement;

/// Compléments d'un soufflet.
///
/// Correspond aux champs de `Hairpin` (`engraving/dom/hairpin.h`) lus par
/// `ExportMusicXml::hairpin` : `HairpinType::CRESC_LINE`/`DIM_LINE`
/// (`line`), `hairpinCircledTip` (`niente`), `direction` (`placement`) et
/// `beginText`/`endText`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hairpin {
    /// Vrai pour un soufflet en trait pointillé (`<dashes>` dans MusicXML).
    ///
    /// Faux pour un soufflet dessiné (`<wedge>`).
    pub line: bool,
    /// Vrai quand la pointe est entourée d'un cercle (`niente="yes"`).
    pub niente: bool,
    /// Placement au-dessus ou au-dessous de la portée.
    pub placement: Placement,
    /// Texte de début, éventuellement avec des `<sym>` MuseScore.
    pub begin_text: String,
    /// Texte de fin, éventuellement avec des `<sym>` MuseScore.
    pub end_text: String,
}

impl Hairpin {
    /// Construit un soufflet dessiné, sans texte, placé au-dessous.
    pub fn new() -> Self {
        Self {
            line: false,
            niente: false,
            placement: Placement::Below,
            begin_text: String::new(),
            end_text: String::new(),
        }
    }
}

impl Default for Hairpin {
    fn default() -> Self {
        Self::new()
    }
}
