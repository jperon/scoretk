// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/note.h`, `engraving/dom/note.cpp`
// (`customizeVelocity`), `engraving/dom/tie.h`, `engraving/dom/pitchspelling.h`
// et `engraving/types/types.h` (`VeloType`) de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see the LICENSE file at the root of this repository or
// <https://www.gnu.org/licenses/gpl-3.0.html>.

//! Notes, hauteurs, altérations affichées et têtes de note.
//!
//! Portage de `Note` (`engraving/dom/note.h`). Les liaisons de prolongation
//! (`Tie` dans `engraving/dom/tie.h`) sont représentées ici par des
//! [`NoteId`] : plus de pointeur `tieBack` / `tieForward`, donc plus de cycle.

use super::ids::NoteId;
use super::layers::NotePlayback;
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

/// Type de vélocité de note (`VeloType` de `engraving/types/types.h`).
///
/// Le sens de [`NotePlayback::user_velocity`] dépend de cette valeur :
/// `Pid::VELO_TYPE` (`<veloType>` du `.mscx`) vaut `USER_VAL` par défaut, comme
/// `Note::m_veloType` dans `engraving/dom/note.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VeloType {
    /// Pourcentage ajouté à la vélocité calculée (`OFFSET_VAL`, XML `offset`).
    OffsetVal,
    /// Vélocité absolue (`USER_VAL`, XML `user`).
    UserVal,
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
    /// Vrai quand l'altération a le rôle « utilisateur » (`AccidentalRole::USER`) :
    /// MuseScore l'exporte en `cautionary` (ou `editorial` avec des crochets).
    pub user: bool,
    /// Altération réduite (`<small>1</small>`) : exportée avec `size="cue"`.
    pub small: bool,
}

impl Accidental {
    /// Construit une altération visible, sans crochets ni parenthèses.
    pub const fn new(kind: AccidentalType) -> Self {
        Self {
            kind,
            bracket: false,
            parentheses: false,
            visible: true,
            user: false,
            small: false,
        }
    }
}

/// Provenance d'une note projetée depuis un format à source textuelle.
///
/// Les deux décalages sont en **unités de code UTF-16 depuis le début du
/// fichier entier** (indices de chaîne JavaScript) : `offset` désigne la
/// première lettre de hauteur chantée, `end` la fin du jeton source. Les deux
/// indices grégoriens pointent vers `Gregorian::syllables` puis
/// `Syllable::notes`, ce qui permet à un pont MusicXML de retrouver la note
/// L6 exacte même quand la projection a sauté une altération non chantée.
/// Champ additif : `None` pour toutes les notes qui ne viennent pas d'un
/// format source textuel (aucun changement pour MuseScore, MIDI, …).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoteSource {
    /// Décalage UTF-16 de la lettre de hauteur chantée.
    pub offset: usize,
    /// Décalage UTF-16 juste après le jeton de la note (fin comprise).
    pub end: usize,
    /// Index de la syllabe dans `Gregorian::syllables`.
    pub gregorian_syllable: usize,
    /// Index de la note dans `Syllable::notes`.
    pub gregorian_note: usize,
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
    /// Direction explicite de la liaison de prolongation (`Pid::SLUR_DIRECTION`
    /// du `<Tie>`) : `Some(true)` pour `<up>up</up>`, `Some(false)` pour
    /// `<up>down</up>`, `None` quand la direction est automatique.
    ///
    /// MuseScore n'exporte `orientation`/`placement` que si la direction est
    /// explicite (`slurTieLineStyle`).
    pub tie_up: Option<bool>,
    /// Vrai si le départ de liaison est affiché (`Tie::visible()`, lu dans
    /// `<Tie><visible>`).
    ///
    /// Le `<tied type="start">` est alors dans un `<notations
    /// print-object="no">`.
    pub tie_forward_visible: bool,
    /// Vrai si l'arrivée de liaison est affichée.
    pub tie_back_visible: bool,
    /// Altération affichée, si elle diffère de l'armure.
    pub accidental: Option<Accidental>,
    /// Tête de note demandée.
    pub head: NoteHeadType,
    /// Groupe de tête de note (`<head>`), distinct du type ci-dessus.
    pub head_group: NoteHeadGroup,
    /// Vrai si la note est visible.
    pub visible: bool,
    /// Vrai si la tête est entre parenthèses (`HAS_PARENTHESES`).
    pub parentheses: bool,
    /// Vrai pour une note fantôme (note morte, guitare).
    pub ghost: bool,
    /// `<small>1</small>` de la note : `isSmallNote` (`exportmusicxml.cpp:4181-4184`)
    /// l'ajoute au `<small>` de l'accord pour écrire `size="cue"`.
    pub small: bool,
    /// Données de lecture (L4) : `play`, `unperformed`, vélocités et
    /// `velo_type`. Voir [`NotePlayback`] pour la sémantique de chaque champ.
    pub playback: NotePlayback,
    /// Doigtés attachés à la note (`<Fingering>`), exportés en
    /// `<technical><fingering>`.
    pub fingerings: Vec<Fingering>,
    /// Provenance textuelle de la note, quand le format source en a une.
    pub source: Option<NoteSource>,
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
            tie_up: None,
            tie_forward_visible: true,
            tie_back_visible: true,
            accidental: None,
            head: NoteHeadType::Auto,
            head_group: NoteHeadGroup::Normal,
            visible: true,
            parentheses: false,
            ghost: false,
            small: false,
            playback: NotePlayback::DEFAULT,
            fingerings: Vec::new(),
            source: None,
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
