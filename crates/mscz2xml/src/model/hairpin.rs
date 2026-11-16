// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de mscz2xml
//
// Portage de `engraving/dom/hairpin.h` de MuseScore Studio, Copyright (C)
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
use super::spanner::LineFont;

/// Compléments d'un soufflet.
///
/// Correspond aux champs de `Hairpin` (`engraving/dom/hairpin.h`) lus par
/// `ExportMusicXml::hairpin` : `HairpinType::CRESC_LINE`/`DIM_LINE`
/// (`line`), `hairpinCircledTip` (`niente`), `direction` (`placement`) et
/// `beginText`/`endText`.
#[derive(Clone, Debug, PartialEq)]
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
    /// Police du texte de début.
    pub begin_font: LineFont,
    /// Police du texte de fin.
    pub end_font: LineFont,
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
            // Police par défaut des textes de soufflet dans MuseScore.
            begin_font: LineFont {
                family: "FreeSerif".to_owned(),
                size: 12.0,
                italic: true,
                ..LineFont::default()
            },
            end_font: LineFont {
                family: "FreeSerif".to_owned(),
                size: 12.0,
                italic: true,
                ..LineFont::default()
            },
        }
    }
}

impl Default for Hairpin {
    fn default() -> Self {
        Self::new()
    }
}
