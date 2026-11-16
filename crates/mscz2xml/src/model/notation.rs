//! Notations attachées aux accords et aux notes.
//!
//! Portage des objets de `engraving/dom/` que l'export MusicXML écrit dans
//! `<notations>` en plus des articulations et des liaisons de prolongation :
//! points d'orgue (`fermata.h`), ornements (`ornament.h`), trémolos
//! (`tremolosinglechord.h`, `tremolotwochords.h`), arpèges (`arpeggio.h`),
//! glissandos (`glissando.h`), lignes d'accord (`chordline.h`) et doigtés
//! (`fingering.h`).
//!
//! Les types de ce module sont volontairement indépendants des lecteurs : les
//! noms SMuFL sont traduits dans `read/`, et l'export ne manipule que ces
//! variantes.

use super::annotation::Placement;
use super::note::Accidental;

/// Type de point d'orgue.
///
/// Correspond aux `SymId` de `engraving/dom/fermata.h` reconnus par
/// `ExportMusicXml::fermata`. Le corps MusicXML associé est `angled`,
/// `square`, `double-angled`, `double-square`, `double-dot`, `half-curve` ou
/// `curlew` ; le point d'orgue normal n'a pas de corps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FermataType {
    /// Point d'orgue normal (`fermataAbove` / `fermataBelow`).
    Normal,
    /// Point d'orgue court (`angled`).
    Short,
    /// Point d'orgue long (`square`).
    Long,
    /// Point d'orgue très court (`double-angled`).
    VeryShort,
    /// Point d'orgue très long (`double-square`).
    VeryLong,
    /// Point d'orgue long de Henze (`double-dot`).
    LongHenze,
    /// Point d'orgue court de Henze (`half-curve`).
    ShortHenze,
    /// Signe de courlis (`curlew`).
    Curlew,
}

/// Point d'orgue attaché à un segment.
///
/// Correspond à `Fermata` dans `engraving/dom/fermata.h`. C'est une annotation
/// de segment dans MuseScore ; l'export l'écrit dans les `<notations>` de
/// l'accord ou du silence de la même piste.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fermata {
    /// Forme du point d'orgue.
    pub kind: FermataType,
    /// Placement vertical ; `Above` donne `type="upright"`.
    pub placement: Placement,
    /// Vrai si le point d'orgue est visible.
    pub visible: bool,
}

impl Fermata {
    /// Construit un point d'orgue normal au-dessus de la portée.
    pub const fn new(kind: FermataType) -> Self {
        Self {
            kind,
            placement: Placement::Above,
            visible: true,
        }
    }
}

/// Type d'ornement.
///
/// Correspond aux `SymId` de `engraving/dom/ornament.h` traduits par
/// `symIdToOrnam` dans `exportmusicxml.cpp`. Les variantes qui ne diffèrent
/// que par leurs attributs MusicXML sont distinctes : `mordent` et
/// `mordent long="yes"` n'ont pas le même nom de balise.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OrnamentType {
    /// Trille (`trill-mark`).
    TrillMark,
    /// Gruppetto (`turn`).
    Turn,
    /// Gruppetto renversé (`inverted-turn`).
    InvertedTurn,
    /// Gruppetto barré (`turn slash="yes"`).
    TurnSlash,
    /// Gruppetto vertical (`vertical-turn`).
    VerticalTurn,
    /// Gruppetto vertical renversé (`inverted-vertical-turn`).
    InvertedVerticalTurn,
    /// Mordant (`mordent`).
    Mordent,
    /// Mordant inversé (`inverted-mordent`).
    InvertedMordent,
    /// Mordant inversé long (`inverted-mordent long="yes"`).
    InvertedMordentLong,
    /// Mordant long (`mordent long="yes"`).
    MordentLong,
    /// Mordant long avec approche par le bas (`mordent long approach="below"`).
    MordentLongApproachBelow,
    /// Mordant long avec approche par le haut (`mordent long approach="above"`).
    MordentLongApproachAbove,
    /// Mordant inversé long avec approche par le bas.
    InvertedMordentLongApproachBelow,
    /// Mordant inversé long avec approche par le haut.
    InvertedMordentLongApproachAbove,
    /// Mordant inversé long avec sortie par le bas.
    InvertedMordentLongDepartureBelow,
    /// Mordant inversé long avec sortie par le haut.
    InvertedMordentLongDepartureAbove,
    /// Ornement de Haydn (`haydn`).
    Haydn,
    /// Schleifer (`schleifer`).
    Schleifer,
    /// Autre ornement, nom SMuFL conservé (`other-ornament smufl="…"`).
    Other(String),
}

/// Altération d'ornement, avec son placement.
///
/// Correspond à `Ornament::accidentalsAboveAndBelow()` : l'export écrit une
/// `<accidental-mark>` par entrée, dans l'ordre du fichier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OrnamentAccidental {
    /// Altération elle-même.
    pub accidental: Accidental,
    /// Placement de l'altération.
    pub placement: Placement,
}

/// Ornement attaché à un accord.
///
/// Correspond à `Ornament` dans `engraving/dom/ornament.h` (sous-classe
/// d'`Articulation`) réduit aux champs exportés.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ornament {
    /// Forme de l'ornement.
    pub kind: OrnamentType,
    /// Ancre explicite, si le fichier la donne (`placement="above|below"`).
    pub placement: Option<Placement>,
    /// Altérations écrites au-dessus et au-dessous de l'ornement.
    pub accidentals: Vec<OrnamentAccidental>,
    /// Vrai si l'ornement est visible.
    pub visible: bool,
}

impl Ornament {
    /// Construit un ornement sans altération ni ancre explicite.
    pub fn new(kind: OrnamentType) -> Self {
        Self {
            kind,
            placement: None,
            accidentals: Vec::new(),
            visible: true,
        }
    }
}

/// Forme d'un trémolo MusicXML.
///
/// Correspond à `TremoloChordType` et à `TremoloType` dans
/// `engraving/dom/tremolo.h` : un trémolo simple n'a qu'un `type="single"`,
/// un trémolo à deux notes porte `type="start"` puis `type="stop"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TremoloKind {
    /// Trémolo sur une note (`single`).
    Single,
    /// Premier accord d'un trémolo à deux notes (`start`).
    Start,
    /// Second accord d'un trémolo à deux notes (`stop`).
    Stop,
    /// Trémolo non mesuré (`unmeasured`, buzz roll).
    Unmeasured,
}

/// Trémolo d'un accord.
///
/// Correspond à `Tremolo` / `Chord::tremoloSingleChord()` et
/// `Chord::tremoloTwoChord()` dans `engraving/dom/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tremolo {
    /// Forme MusicXML.
    pub kind: TremoloKind,
    /// Nombre de barres (1 pour `r8`/`c8`, 6 pour `r256`/`c256`).
    pub count: u8,
}

/// Type d'arpège.
///
/// Correspond à `ArpeggioType` dans `engraving/dom/arpeggio.h`. Les variantes
/// droites (`UP_STRAIGHT`, `DOWN_STRAIGHT`) ne sont pas distinguées par
/// MusicXML et sont rangées avec `Up` / `Down`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArpeggioType {
    /// Arpège normal.
    Normal,
    /// Arpège vers le haut.
    Up,
    /// Arpège vers le bas.
    Down,
    /// Accord non arpégé (crochet).
    Bracket,
}

/// Arpège attaché à un accord.
///
/// Correspond à `Arpeggio` dans `engraving/dom/arpeggio.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Arpeggio {
    /// Type d'arpège.
    pub kind: ArpeggioType,
    /// Vrai si l'arpège est visible.
    pub visible: bool,
}

impl Arpeggio {
    /// Construit un arpège visible du type donné.
    pub const fn new(kind: ArpeggioType) -> Self {
        Self {
            kind,
            visible: true,
        }
    }
}

/// Type de ligne d'accord (`ChordLine`).
///
/// Correspond à `ChordLineType` dans `engraving/dom/chordline.h`. Les valeurs
/// numériques des fichiers MuseScore 3 sont `1` = `FALL`, `2` = `DOIT`,
/// `3` = `PLOP`, `4` = `SCOOP`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChordLineType {
    /// Chute (`falloff`).
    Fall,
    /// Montée (`doit`).
    Doit,
    /// Plop (`plop`).
    Plop,
    /// Scoop (`scoop`).
    Scoop,
}

/// Style de trait d'un glissando.
///
/// Correspond à `LineType` dans `engraving/types/types.h`, limité aux valeurs
/// que `glissando()` sait exporter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineStyle {
    /// Trait plein (`solid`).
    Solid,
    /// Traits interrompus (`dashed`).
    Dashed,
    /// Pointillés (`dotted`).
    Dotted,
}

/// Type de glissando.
///
/// Correspond à `GlissandoType` dans `engraving/dom/glissando.h`.
/// [`GlissandoType::Straight`] est exporté en `<slide>`, [`Wavy`] en
/// `<glissando line-type="wavy">`.
///
/// [`Wavy`]: GlissandoType::Wavy
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GlissandoType {
    /// Glissando droit, avec son style de trait (`<slide>`).
    Straight(LineStyle),
    /// Glissando ondulé (`<glissando line-type="wavy">`).
    Wavy,
}

/// Glissando attaché à une note.
///
/// Correspond à `Glissando` dans `engraving/dom/glissando.h`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glissando {
    /// Type de glissando.
    pub kind: GlissandoType,
    /// Texte affiché (`gliss.` par défaut).
    pub text: String,
    /// Vrai si le texte est affiché (`showText`).
    pub show_text: bool,
    /// Vrai si le glissando est visible.
    pub visible: bool,
}

impl Glissando {
    /// Construit un glissando droit plein avec le texte affiché.
    pub fn new(kind: GlissandoType) -> Self {
        Self {
            kind,
            text: String::new(),
            show_text: true,
            visible: true,
        }
    }
}

/// Type de trille.
///
/// Correspond à `TrillType` dans `engraving/dom/trill.h`, traduit par
/// `wavyLineStart` en symbole d'ornement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrillType {
    /// Trille avec ligne ondulée (`trill-mark`).
    TrillLine,
    /// Ligne pure, sans symbole.
    PureLine,
    /// Ligne avec prall supérieur.
    UpperPrallLine,
    /// Ligne avec prall prall.
    PrallPrallLine,
    /// Ligne avec prall inférieur.
    DownPrallLine,
}

/// Trille sous forme de spanner (`Spanner type="Trill"`).
///
/// Correspond à `Trill` dans `engraving/dom/trill.h` : l'export écrit le
/// symbole et `<wavy-line type="start">` au début, `<wavy-line type="stop">` à
/// la fin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trill {
    /// Type de trille.
    pub kind: TrillType,
    /// Altération éventuellement affichée avec le trille.
    pub accidental: Option<Accidental>,
}

impl Trill {
    /// Construit un trille avec ligne ondulée.
    pub fn new(kind: TrillType) -> Self {
        Self {
            kind,
            accidental: None,
        }
    }
}

/// Liaison d'expression sous forme de spanner (`Spanner type="Slur"`).
///
/// Correspond à `Slur` dans `engraving/dom/slur.h`, réduite aux propriétés
/// lues par `slurTieLineStyle` : placement explicite et visibilité.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slur {
    /// Placement explicite (`orientation`/`placement` MusicXML), `None` quand
    /// MuseScore laisse la direction automatique.
    pub placement: Option<Placement>,
    /// Vrai si la liaison est visible.
    pub visible: bool,
}

impl Slur {
    /// Construit une liaison visible à direction automatique.
    pub const fn new() -> Self {
        Self {
            placement: None,
            visible: true,
        }
    }
}

impl Default for Slur {
    fn default() -> Self {
        Self::new()
    }
}

/// Complément de notation porté par un [`super::Spanner`].
///
/// Les spanners génériques ne gardent que leurs extrémités ; ce complément
/// porte ce que l'export MusicXML doit écrire pour les liaisons d'expression,
/// les glissandos et les trilles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpannerNotation {
    /// Liaison d'expression.
    Slur(Slur),
    /// Glissando.
    Glissando(Glissando),
    /// Trille.
    Trill(Trill),
}

/// Doigté attaché à une note.
///
/// Correspond à `Fingering` dans `engraving/dom/fingering.h` (`<Fingering>`
/// enfant de `<Note>`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fingering {
    /// Texte du doigté (`1`, `2`, `T`, `p`…).
    pub text: String,
    /// Placement vertical explicite, `None` quand le style décide.
    pub placement: Option<Placement>,
    /// Vrai si le doigté est visible.
    pub visible: bool,
}

impl Fingering {
    /// Construit un doigté visible au-dessus de la portée.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            placement: None,
            visible: true,
        }
    }
}
