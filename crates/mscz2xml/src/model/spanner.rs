//! Spanners génériques.
//!
//! Portage minimal de `Spanner` (`engraving/dom/spanner.h`) et des objets qui
//! en dérivent : liaison d'expression (`Slur`), soufflet (`Hairpin`), ottava,
//! pédale, volta, ligne de texte et trille. Les extrémités sont des positions
//! (`tick`, `track`), comme dans `Spanner::tick()` / `Spanner::track()` et
//! `Spanner::endTick()` / `Spanner::endTrack()`.

use super::fraction::Tick;
use super::hairpin::Hairpin;
use super::ids::Track;
use super::repeat::VoltaInfo;

/// Extrémité d'un spanner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpannerAnchor {
    /// Position horizontale.
    pub tick: Tick,
    /// Piste de rattachement.
    pub track: Track,
}

impl SpannerAnchor {
    /// Construit une extrémité.
    pub const fn new(tick: Tick, track: Track) -> Self {
        Self { tick, track }
    }
}

/// Sens d'un soufflet.
///
/// Correspond à `HairpinType` dans `engraving/dom/hairpin.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HairpinType {
    /// Crescendo.
    Crescendo,
    /// Diminuendo.
    Diminuendo,
}

/// Type d'ottava.
///
/// Correspond à `OttavaType` dans `engraving/dom/ottava.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OttavaType {
    /// `8va`.
    OctaveUp,
    /// `8vb`.
    OctaveDown,
    /// `15ma`.
    FifteenthUp,
    /// `15mb`.
    FifteenthDown,
}

/// Type de spanner.
///
/// Chaque variante correspond à une classe dérivée de `Spanner` dans
/// `engraving/dom/` ; [`SpannerType::Other`] permet d'attendre les types moins
/// prioritaires sans bloquer les lecteurs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SpannerType {
    /// Liaison d'expression (`Slur` dans `engraving/dom/slur.h`).
    Slur,
    /// Soufflet (`Hairpin` dans `engraving/dom/hairpin.h`).
    Hairpin(HairpinType),
    /// Ottava (`Ottava` dans `engraving/dom/ottava.h`).
    Ottava(OttavaType),
    /// Pédale (`Pedal` dans `engraving/dom/pedal.h`).
    Pedal,
    /// Volta (`Volta` dans `engraving/dom/volta.h`).
    Volta,
    /// Ligne de texte (`TextLine` dans `engraving/dom/textline.h`).
    TextLine,
    /// Trille (`Trill` dans `engraving/dom/trill.h`).
    Trill,
    /// Glissando (`Glissando` dans `engraving/dom/glissando.h`).
    Glissando,
    /// Autre spanner, nom MuseScore conservé tel quel.
    Other(String),
}

/// Spanner générique.
///
/// Correspond à `Spanner` et à ses dérivés dans `engraving/dom/spanner.h`.
/// Les extrémités sont des positions et non des pointeurs vers les éléments,
/// ce qui garde le modèle `Send`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spanner {
    /// Type de spanner.
    pub kind: SpannerType,
    /// Début.
    pub start: SpannerAnchor,
    /// Fin.
    pub end: SpannerAnchor,
    /// Texte éventuel (volta, ligne de texte, trille…).
    pub text: Option<String>,
    /// Propriétés de volta, pour [`SpannerType::Volta`].
    pub volta: Option<VoltaInfo>,
    /// Numéro d'ordre pour les spanners imbriqués (par exemple les reprises
    /// de slurs).
    pub number: u8,
    /// Compléments propres aux soufflets.
    pub hairpin: Option<Hairpin>,
}

impl Spanner {
    /// Construit un spanner sans texte, de numéro zéro.
    pub fn new(kind: SpannerType, start: SpannerAnchor, end: SpannerAnchor) -> Self {
        Self {
            kind,
            start,
            end,
            text: None,
            volta: None,
            number: 0,
            hairpin: None,
        }
    }
}
