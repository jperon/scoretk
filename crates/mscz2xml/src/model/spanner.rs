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
use super::notation::SpannerNotation;
use super::repeat::VoltaInfo;
use super::text::RichText;

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

/// Crochet d'extrémité d'une ligne de texte.
///
/// Correspond à `HookType` (`engraving/dom/textlinebase.h`). Les valeurs
/// numériques du fichier `.mscx` suivent cet ordre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HookType {
    /// Aucun crochet (`line-end="none"`).
    #[default]
    None,
    /// Crochet droit d'un côté (`line-end="up"`/`"down"`).
    Hook90,
    /// Crochet droit des deux côtés (`line-end="both"`).
    Hook90T,
    /// Flèche (`line-end="arrow"`).
    Arrow,
    /// Flèche pleine (`line-end="arrow"`).
    ArrowFilled,
}

impl HookType {
    /// Type correspondant à la valeur `<beginHookType>` du fichier.
    pub fn from_mcsx(value: i32) -> Self {
        match value {
            1 => Self::Hook90,
            2 => Self::Hook90T,
            3 => Self::Arrow,
            4 => Self::ArrowFilled,
            _ => Self::None,
        }
    }
}

/// Style de trait d'une ligne de texte.
///
/// Correspond à `LineType` : les valeurs `0`, `1` et `2` du fichier `.mscx`
/// sont `solid`, `dashed` et `dotted`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextLineStyle {
    /// Trait plein (`line-type="solid"`).
    #[default]
    Solid,
    /// Traits (`line-type="dashed"`, exporté en `<dashes>` quand il n'y a pas
    /// de crochet).
    Dashed,
    /// Pointillés (`line-type="dotted"`).
    Dotted,
}

impl TextLineStyle {
    /// Style correspondant à la valeur `<lineStyle>` du fichier.
    pub fn from_mcsx(value: i32) -> Self {
        match value {
            1 => Self::Dashed,
            2 => Self::Dotted,
            _ => Self::Solid,
        }
    }

    /// Nom MusicXML (`line-type`).
    pub const fn musicxml_name(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Dashed => "dashed",
            Self::Dotted => "dotted",
        }
    }
}

/// Police d'un texte de ligne.
///
/// Correspond aux propriétés `beginFontFace`/`beginFontSize` (et `end…`) de
/// `TextLineBase` ; le style MuseScore 4.7 donne `Edwin` en 10 points.
#[derive(Clone, Debug, PartialEq)]
pub struct LineFont {
    /// Police.
    pub family: String,
    /// Taille en points.
    pub size: f64,
    /// Gras.
    pub bold: bool,
    /// Italique.
    pub italic: bool,
    /// Souligné.
    pub underline: bool,
    /// Barré.
    pub strike: bool,
}

impl Default for LineFont {
    fn default() -> Self {
        Self {
            family: super::text::DEFAULT_TEXT_FONT.to_owned(),
            size: super::text::DEFAULT_TEXT_SIZE,
            bold: false,
            italic: false,
            underline: false,
            strike: false,
        }
    }
}

/// Informations propres à une ligne de texte (`TextLine`).
///
/// Correspond à `TextLineBase` (`engraving/dom/textlinebase.h`), réduit aux
/// propriétés que `ExportMusicXml::textLine` et `writeHairpinText` écrivent.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLineInfo {
    /// Vrai si le trait est visible (`lineVisible`).
    pub line_visible: bool,
    /// Texte de début (`beginText`).
    pub begin_text: Option<RichText>,
    /// Texte de fin (`endText`).
    pub end_text: Option<RichText>,
    /// Vrai si la ligne appartient au système (`system="only-top"`).
    pub system: bool,
    /// Placement (`above` par défaut).
    pub placement: super::annotation::Placement,
    /// Style de trait.
    pub line_style: TextLineStyle,
    /// Crochet de début.
    pub begin_hook: HookType,
    /// Crochet de fin.
    pub end_hook: HookType,
    /// Hauteur du crochet de début, en espaces.
    pub begin_hook_height: f64,
    /// Hauteur du crochet de fin, en espaces.
    pub end_hook_height: f64,
    /// Police du texte de début.
    pub begin_font: LineFont,
    /// Police du texte de fin.
    pub end_font: LineFont,
}

impl Default for TextLineInfo {
    fn default() -> Self {
        Self {
            line_visible: true,
            begin_text: None,
            end_text: None,
            system: false,
            placement: super::annotation::Placement::Above,
            line_style: TextLineStyle::Solid,
            begin_hook: HookType::None,
            end_hook: HookType::None,
            begin_hook_height: 1.5,
            end_hook_height: 1.5,
            begin_font: LineFont::default(),
            end_font: LineFont::default(),
        }
    }
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
#[derive(Clone, Debug, PartialEq)]
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
    /// Complément propre aux liaisons d'expression, glissandos et trilles.
    pub notation: Option<SpannerNotation>,
    /// Compléments propres aux lignes de texte.
    pub text_line: Option<TextLineInfo>,
    /// Vrai si le spanner et ses traits sont affichés (`<visible>`).
    ///
    /// Une ottava invisible décale quand même la hauteur écrite ; seul son
    /// `<octave-shift>` est omis.
    pub visible: bool,
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
            notation: None,
            text_line: None,
            visible: true,
        }
    }
}
