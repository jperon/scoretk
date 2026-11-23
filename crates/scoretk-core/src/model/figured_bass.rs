// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/figuredbass.h` de MuseScore Studio, Copyright (C)
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

//! Chiffrage figuré (basse continue).
//!
//! Portage de `FiguredBass` et `FiguredBassItem`
//! (`engraving/dom/figuredbass.h`), réduit aux propriétés que
//! `ExportMusicXml::figuredBass` écrit : les chiffres d'un côté de la note,
//! leur durée et les traits de prolongation.

use super::fraction::{Fraction, Tick};

/// Modificateur d'un chiffre (accident ou barre).
///
/// Correspond à `FiguredBassItem::Modifier` ; les valeurs numériques du
/// fichier `.mscx` suivent cet ordre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FiguredBassModifier {
    /// Aucun modificateur (`NONE`).
    None,
    /// Double bémol (`flat-flat`).
    DoubleFlat,
    /// Bémol (`flat`).
    Flat,
    /// Bécarre (`natural`).
    Natural,
    /// Dièse (`sharp`).
    Sharp,
    /// Double dièse (`double-sharp`).
    DoubleSharp,
    /// Croix (`cross`).
    Cross,
    /// Barre oblique inverse (`backslash`).
    Backslash,
    /// Barre oblique (`slash`).
    Slash,
}

impl FiguredBassModifier {
    /// Modificateur correspondant à la valeur `<prefix>`/`<suffix>` du fichier
    /// (`FiguredBassItem::Modifier` : `NONE` = 0, `DOUBLEFLAT` = 1, …,
    /// `SLASH` = 8).
    pub const fn from_mcsx(value: i32) -> Self {
        match value {
            1 => Self::DoubleFlat,
            2 => Self::Flat,
            3 => Self::Natural,
            4 => Self::Sharp,
            5 => Self::DoubleSharp,
            6 => Self::Cross,
            7 => Self::Backslash,
            8 => Self::Slash,
            _ => Self::None,
        }
    }

    /// Nom MusicXML du modificateur (`Modifier2MusicXml`).
    pub const fn musicxml_name(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::DoubleFlat => Some("flat-flat"),
            Self::Flat => Some("flat"),
            Self::Natural => Some("natural"),
            Self::Sharp => Some("sharp"),
            Self::DoubleSharp => Some("double-sharp"),
            Self::Cross => Some("cross"),
            Self::Backslash => Some("backslash"),
            Self::Slash => Some("slash"),
        }
    }
}

/// Trait de prolongation d'un chiffre.
///
/// Correspond à `FiguredBassItem::ContLine` (`NONE` = 0, `SIMPLE` = 1,
/// `EXTENDED` = 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FiguredBassContLine {
    /// Aucun trait.
    #[default]
    None,
    /// Trait simple (`<extend type="stop"/>`).
    Simple,
    /// Trait prolongé (`<extend type="start|continue"/>`).
    Extended,
}

impl FiguredBassContLine {
    /// Trait correspondant à la valeur `<continuationLine>` du fichier.
    pub const fn from_mcsx(value: i32) -> Self {
        match value {
            1 => Self::Simple,
            2 => Self::Extended,
            _ => Self::None,
        }
    }
}

/// Chiffre d'une basse continue (`FiguredBassItem`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FiguredBassItem {
    /// Accident ou barre avant le chiffre.
    pub prefix: Option<FiguredBassModifier>,
    /// Chiffre affiché (aucun pour un simple trait).
    pub digit: Option<u8>,
    /// Accident ou barre après le chiffre.
    pub suffix: Option<FiguredBassModifier>,
    /// Trait de prolongation.
    pub cont_line: FiguredBassContLine,
    /// Encadrements `b0..b4` du `<brackets>` (`FiguredBassItem::Parenthesis` :
    /// 0 aucun, 1 parenthèse ouvrante, 2 fermante, 3 crochet ouvrant,
    /// 4 fermant). MuseScore en dérive `parenthesesMode()` : la présence d'un
    /// encadrement sur la partie affichée (préfixe, chiffre ou suffixe) suffit
    /// à écrire `parentheses="yes"` à l'export MusicXML
    /// (`engraving/dom/figuredbass.cpp:629-640` et
    /// `exportmusicxml.cpp:6832-6834`).
    pub brackets: [u8; 5],
}

impl FiguredBassItem {
    /// `FiguredBassItem::startsWithParenthesis`
    /// (`engraving/dom/figuredbass.cpp:629-640`).
    pub fn starts_with_parenthesis(&self) -> bool {
        if self.prefix.is_some() {
            return self.brackets[0] != 0;
        }
        if self.digit.is_some() {
            return self.brackets[1] != 0;
        }
        if self.suffix.is_some() {
            return self.brackets[2] != 0;
        }
        false
    }
}

/// Chiffrage figuré attaché à un segment.
///
/// Correspond à `FiguredBass` dans `engraving/dom/figuredbass.h`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiguredBass {
    /// Chiffres, dans l'ordre du fichier (`FiguredBass::items()`).
    pub items: Vec<FiguredBassItem>,
    /// Durée du chiffrage (`<ticks>`), qui peut dépasser celle de la note.
    pub ticks: Fraction,
    /// Vrai quand le chiffrage est placé au-dessus de la portée
    /// (`<placement>above</placement>`).
    pub place_above: bool,
    /// Vrai quand les chiffres sont entre parenthèses (`<parentheses>`).
    pub parentheses: bool,
    /// Vrai si le chiffrage est visible.
    pub visible: bool,
}

impl FiguredBass {
    /// Construit un chiffrage vide, au-dessous de la portée.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            ticks: Fraction::ZERO,
            place_above: false,
            parentheses: false,
            visible: true,
        }
    }

    /// Tick de fin du chiffrage depuis son segment.
    pub fn end_tick(&self, segment_tick: Tick) -> Tick {
        segment_tick + self.ticks
    }
}

impl Default for FiguredBass {
    fn default() -> Self {
        Self::new()
    }
}
