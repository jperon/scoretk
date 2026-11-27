// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Données de `fonts/leland/` de MuseScore Studio 4.7.4, Copyright (C)
// MuseScore BVBA and Klaus Rettinghaus, sous SIL Open Font License 1.1,
// converties en Rust (fichier généré) en 2026.
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

//! Sous-ensemble SMuFL Leland pour la gravure sur appareils contraints.
//!
//! `no_std`, sans dépendance : le tableau [`glyphs`] est généré par
//! `scripts/gen-smufl-subset.py` à partir de la police livrée par MuseScore
//! 4.7.4 et n'est jamais édité à la main. Les coordonnées sont des font units
//! (em = 1000, une portée = [`SPATIUM_UNITS`] unités) ; les valeurs de
//! [`DEFAULTS`] sont en fixed point 1/1024 de portée.
//!
//! Un contour est une suite de [`Edge`] : [`Edge::Move`] est absolu, les
//! autres sont relatifs au point courant. Aucune allocation n'est nécessaire
//! pour parcourir une police.

#![no_std]

pub mod glyphs;

pub use glyphs::{
    Anchor, Default, Edge, Glyph, ANCHORS, ANCHOR_NAMES, DEFAULTS, EDGES, GLYPHS, SPATIUM_UNITS,
    UPEM,
};

/// Cherche un glyphe par son nom SMuFL (table triée par nom).
pub fn glyph(name: &str) -> Option<&'static Glyph> {
    GLYPHS
        .binary_search_by(|glyph| glyph.name.cmp(name))
        .ok()
        .map(|index| &GLYPHS[index])
}

/// Index d'un glyphe dans [`GLYPHS`], pour les listes d'affichage.
pub fn glyph_index(name: &str) -> Option<u16> {
    GLYPHS
        .binary_search_by(|glyph| glyph.name.cmp(name))
        .ok()
        .and_then(|index| u16::try_from(index).ok())
}

/// Cherche un glyphe par son codepoint Unicode privé SMuFL.
pub fn glyph_by_codepoint(codepoint: u16) -> Option<&'static Glyph> {
    GLYPHS.iter().find(|glyph| glyph.codepoint == codepoint)
}

/// Ancre SMuFL d'un glyphe, en font units relatifs à l'origine du glyphe.
pub fn anchor(glyph: &Glyph, name: &str) -> Option<(i16, i16)> {
    let name_index = ANCHOR_NAMES
        .iter()
        .position(|candidate| *candidate == name)?;
    let start = usize::from(glyph.anchors.0);
    let end = start + usize::from(glyph.anchors.1);
    ANCHORS[start..end]
        .iter()
        .find(|anchor| usize::from(anchor.name) == name_index)
        .map(|anchor| (anchor.x, anchor.y))
}

/// Valeur d'un `engravingDefaults` SMuFL, en fixed point 1/1024 de portée.
pub fn default(name: &str) -> Option<i16> {
    DEFAULTS
        .iter()
        .find(|value| value.name == name)
        .map(|value| value.value)
}

/// Convertit des font units en 1/1024 de portée (arrondi au plus proche).
pub const fn units_to_fixed(units: i16) -> i32 {
    (units as i32 * 1024 + SPATIUM_UNITS as i32 / 2) / SPATIUM_UNITS as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_glyphs_are_present() {
        for name in [
            "noteheadBlack",
            "noteheadHalf",
            "noteheadWhole",
            "noteheadDoubleWhole",
            "gClef",
            "fClef",
            "cClef",
            "accidentalSharp",
            "accidentalFlat",
            "accidentalNatural",
            "restQuarter",
            "restWhole",
            "flag8thUp",
            "augmentationDot",
            "timeSig0",
        ] {
            assert!(glyph(name).is_some(), "glyph absent : {name}");
        }
    }

    #[test]
    fn outlines_are_well_formed() {
        for g in &GLYPHS {
            let start = usize::from(g.edges.0);
            let end = start + usize::from(g.edges.1);
            assert!(end <= EDGES.len(), "{} : arêtes hors limites", g.name);
            let mut x = 0i32;
            let mut y = 0i32;
            let mut has_move = false;
            for edge in &EDGES[start..end] {
                match *edge {
                    Edge::Move(mx, my) => {
                        has_move = true;
                        x = i32::from(mx);
                        y = i32::from(my);
                    }
                    Edge::Line(dx, dy) => {
                        assert!(has_move, "{} : ligne sans Move", g.name);
                        x += i32::from(dx);
                        y += i32::from(dy);
                    }
                    Edge::Quad(qx, qy, ex, ey) => {
                        assert!(has_move, "{} : courbe sans Move", g.name);
                        assert!(x + i32::from(qx) < 1 << 16, "{} : qx déborde", g.name);
                        assert!(y + i32::from(qy) < 1 << 16, "{} : qy déborde", g.name);
                        x += i32::from(ex);
                        y += i32::from(ey);
                    }
                }
            }
            assert!(x.abs() < 1 << 16 && y.abs() < 1 << 16, "{} : point final déborde", g.name);
        }
    }

    #[test]
    fn known_metrics_match_smufl() {
        // Leland : em 1000, une portée = 250 font units, tête de noire large
        // de 1.3 sp (valeurs de `leland_metadata.json`).
        assert_eq!(SPATIUM_UNITS, 250);
        let black = glyph("noteheadBlack").unwrap();
        assert_eq!(black.advance, 325);
        assert_eq!(black.bbox, [0, -133, 325, 132]);
        assert_eq!(units_to_fixed(125), 512); // 0.5 sp
        assert_eq!(default("stemThickness"), Some(102)); // 0.1 sp
    }
}
