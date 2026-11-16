//! Notes, hauteurs, altérations affichées et têtes de note.
//!
//! Portage de `Note` (`engraving/dom/note.h`). Les liaisons de prolongation
//! (`Tie` dans `engraving/dom/tie.h`) sont représentées ici par des
//! [`NoteId`] : plus de pointeur `tieBack` / `tieForward`, donc plus de cycle.

use super::ids::NoteId;
use super::notation::Fingering;

/// Constantes de classe de hauteur tonale (`Tpc`).
///
/// Valeurs de `engraving/dom/pitchspelling.h` : `-9` est invalide, puis les
/// bémols triples, doubles, simples, les notes naturelles, les dièses simples,
/// doubles et triples, par quintes.
pub mod tpc {
    /// Valeur invalide (`TPC_INVALID`).
    pub const INVALID: i8 = -9;

    /// Fa naturel (`TPC_F`).
    pub const F: i8 = 13;
    /// Do naturel (`TPC_C`).
    pub const C: i8 = 14;
    /// Sol naturel (`TPC_G`).
    pub const G: i8 = 15;
    /// Ré naturel (`TPC_D`).
    pub const D: i8 = 16;
    /// La naturel (`TPC_A`).
    pub const A: i8 = 17;
    /// Mi naturel (`TPC_E`).
    pub const E: i8 = 18;
    /// Si naturel (`TPC_B`).
    pub const B: i8 = 19;

    /// Fa bémol (`TPC_F_B`).
    pub const F_FLAT: i8 = 6;
    /// Do bémol (`TPC_C_B`).
    pub const C_FLAT: i8 = 7;
    /// Sol bémol (`TPC_G_B`).
    pub const G_FLAT: i8 = 8;
    /// Ré bémol (`TPC_D_B`).
    pub const D_FLAT: i8 = 9;
    /// La bémol (`TPC_A_B`).
    pub const A_FLAT: i8 = 10;
    /// Mi bémol (`TPC_E_B`).
    pub const E_FLAT: i8 = 11;
    /// Si bémol (`TPC_B_B`).
    pub const B_FLAT: i8 = 12;

    /// Fa dièse (`TPC_F_S`).
    pub const F_SHARP: i8 = 20;
    /// Do dièse (`TPC_C_S`).
    pub const C_SHARP: i8 = 21;
    /// Sol dièse (`TPC_G_S`).
    pub const G_SHARP: i8 = 22;
    /// Ré dièse (`TPC_D_S`).
    pub const D_SHARP: i8 = 23;
    /// La dièse (`TPC_A_S`).
    pub const A_SHARP: i8 = 24;
    /// Mi dièse (`TPC_E_S`).
    pub const E_SHARP: i8 = 25;
    /// Si dièse (`TPC_B_S`).
    pub const B_SHARP: i8 = 26;
}

/// Tête de note.
///
/// Correspond à `NoteHeadType` dans `engraving/types/types.h` et à
/// `Note::headType()` dans `engraving/dom/note.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoteHeadType {
    /// Tête déterminée par la durée (`HEAD_AUTO`).
    Auto,
    /// Brève (`HEAD_BREVIS`).
    Breve,
    /// Ronde (`HEAD_WHOLE`).
    Whole,
    /// Blanche (`HEAD_HALF`).
    Half,
    /// Noire (`HEAD_QUARTER`).
    Quarter,
    /// Tête en croix.
    Cross,
    /// Tête en losange.
    Diamond,
    /// Tête triangulaire.
    Triangle,
    /// Tête en barre oblique.
    Slash,
    /// Tête carrée.
    Square,
    /// Tête en X.
    X,
    /// Autre tête, valeur brute du type MuseScore.
    Other(u8),
}

/// Groupe de tête de note.
///
/// Correspond à `NoteHeadGroup` dans `engraving/types/types.h`, distinct de
/// [`NoteHeadType`] (qui donne le remplissage et la figure). Les variantes
/// couvrent les groupes du corpus ; les autres conservent leur nom MuseScore.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum NoteHeadGroup {
    /// Tête normale (`HEAD_NORMAL`).
    #[default]
    Normal,
    /// Tête en croix (`HEAD_CROSS`).
    Cross,
    /// Tête en plus (`HEAD_PLUS`).
    Plus,
    /// Tête encerclée d'une croix (`HEAD_XCIRCLE`).
    XCircle,
    /// Tête vide barrée (`HEAD_WITHX`).
    WithX,
    /// Triangle vers le haut (`HEAD_TRIANGLE_UP`).
    TriangleUp,
    /// Triangle vers le bas (`HEAD_TRIANGLE_DOWN`).
    TriangleDown,
    /// Tête barrée (`HEAD_SLASHED1`).
    Slashed1,
    /// Tête barrée en sens inverse (`HEAD_SLASHED2`).
    Slashed2,
    /// Losange (`HEAD_DIAMOND`).
    Diamond,
    /// Ancien losange (`HEAD_DIAMOND_OLD`).
    DiamondOld,
    /// Tête encerclée (`HEAD_CIRCLED`).
    Circled,
    /// Grande tête encerclée (`HEAD_CIRCLED_LARGE`).
    CircledLarge,
    /// Grande flèche (`HEAD_LARGE_ARROW`).
    LargeArrow,
    /// Variante de brève (`HEAD_BREVIS_ALT`).
    BrevisAlt,
    /// Tête en barre oblique (`HEAD_SLASH`).
    Slash,
    /// Grand losange (`HEAD_LARGE_DIAMOND`).
    LargeDiamond,
    /// Syllabe `do` (têtes de solfège).
    Do,
    /// Syllabe `re`.
    Re,
    /// Syllabe `mi`.
    Mi,
    /// Syllabe `fa`.
    Fa,
    /// Syllabe `sol`.
    Sol,
    /// Syllabe `la`.
    La,
    /// Syllabe `ti`.
    Ti,
    /// Croix épaisse (`HEAD_HEAVY_X`).
    HeavyCross,
    /// Croix épaisse avec chapeau (`HEAD_HEAVY_X_HAT`).
    HeavyCrossHat,
    /// Groupe non modélisé, nom MuseScore conservé.
    Other(String),
}

/// Type d'altération affichée.
///
/// Correspond à `AccidentalType` dans `engraving/types/types.h` et à
/// `Accidental::accidentalType()` dans `engraving/dom/accidental.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AccidentalType {
    /// Pas d'altération.
    None,
    /// Bémol.
    Flat,
    /// Bécarre.
    Natural,
    /// Dièse.
    Sharp,
    /// Double bémol.
    DoubleFlat,
    /// Double dièse.
    DoubleSharp,
    /// Bécarre-bémol.
    NaturalFlat,
    /// Bécarre-dièse.
    NaturalSharp,
    /// Triple bémol.
    TripleFlat,
    /// Triple dièse.
    TripleSharp,
    /// Autre altération, valeur brute du type MuseScore.
    Other(i16),
}

/// Altération affichée devant une note.
///
/// Correspond à `Accidental` dans `engraving/dom/accidental.h`. Le type est
/// distinct du `tpc` de la note : une note peut être portée par un `tpc` sans
/// altération affichée, ou l'inverse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Accidental {
    /// Type d'altération.
    pub kind: AccidentalType,
    /// Vrai si l'altération est entre crochets.
    pub bracket: bool,
    /// Vrai si l'altération est entre parenthèses.
    pub parentheses: bool,
    /// Vrai si l'altération est visible.
    pub visible: bool,
}

impl Accidental {
    /// Construit une altération visible, sans crochets ni parenthèses.
    pub const fn new(kind: AccidentalType) -> Self {
        Self {
            kind,
            bracket: false,
            parentheses: false,
            visible: true,
        }
    }
}

/// Note d'un accord.
///
/// Correspond à `Note` dans `engraving/dom/note.h`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// Hauteur MIDI (0-127).
    pub pitch: u8,
    /// Classe de hauteur tonale concert (`Tpc`, voir [`tpc`]).
    pub tpc: i8,
    /// Classe de hauteur tonale transposée (pour les instruments
    /// transpositeurs).
    pub tpc_transposed: i8,
    /// Note suivante d'une liaison de prolongation, si cette note en commence
    /// une.
    pub tie_forward: Option<NoteId>,
    /// Note précédente d'une liaison de prolongation, si cette note en termine
    /// une.
    pub tie_back: Option<NoteId>,
    /// Altération affichée, si elle diffère de l'armure.
    pub accidental: Option<Accidental>,
    /// Tête de note demandée.
    pub head: NoteHeadType,
    /// Groupe de tête de note (`<head>`), distinct du type ci-dessus.
    pub head_group: NoteHeadGroup,
    /// Vrai si la note est visible.
    pub visible: bool,
    /// Vrai pour une note fantôme (note morte, guitare).
    pub ghost: bool,
    /// Vrai si la note est jouée.
    pub play: bool,
    /// Doigtés attachés à la note (`<Fingering>`), exportés en
    /// `<technical><fingering>`.
    pub fingerings: Vec<Fingering>,
}

impl Note {
    /// Construit une note normale, audible et sans liaison.
    pub const fn new(pitch: u8, tpc: i8) -> Self {
        Self {
            pitch,
            tpc,
            tpc_transposed: tpc,
            tie_forward: None,
            tie_back: None,
            accidental: None,
            head: NoteHeadType::Auto,
            head_group: NoteHeadGroup::Normal,
            visible: true,
            ghost: false,
            play: true,
            fingerings: Vec::new(),
        }
    }

    /// Vrai si la note commence une liaison de prolongation.
    pub const fn is_tie_start(&self) -> bool {
        self.tie_forward.is_some()
    }

    /// Vrai si la note termine une liaison de prolongation.
    pub const fn is_tie_stop(&self) -> bool {
        self.tie_back.is_some()
    }

    /// Vrai si la note est au milieu d'une chaîne de liaisons.
    pub const fn is_tie_middle(&self) -> bool {
        self.tie_forward.is_some() && self.tie_back.is_some()
    }
}
