// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/fret.h` de MuseScore Studio, Copyright (C)
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

//! Grille d'accord (`FretDiagram`).
//!
//! Correspond à `FretDiagram` dans `engraving/dom/fret.h`. Le diagramme est
//! porté par un `<FretDiagram>` du `.mscx` (`TRead::read(FretDiagram*)`,
//! `engraving/rw/read460/tread.cpp:806-870`) et s'exporte en `<frame>` dans
//! l'élément `<harmony>` (`writeMusicXml(const FretDiagram*)`,
//! `exportmusicxml.cpp:9049-9122`).

/// Type de point d'une corde (`FretItem::DotType`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FretDotType {
    /// Point plein (`normal`).
    #[default]
    Normal,
    /// Croix (`cross`) : jamais exportée en `<frame-note>`.
    Cross,
    /// Carré (`square`).
    Square,
}

/// Type de marqueur d'une corde (`FretItem::MarkerType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FretMarkerType {
    /// Cercle (`circle`) : exporté `<fret>0</fret>`.
    Circle,
    /// Autre marqueur, non exporté dans `<frame>`.
    Other,
}

/// Point d'une corde.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FretDot {
    /// Indice de corde interne (`<string no>`), 0 = corde la plus grave.
    pub string: u8,
    /// Frette du point, avant `fretOffset`.
    pub fret: u8,
    /// Type de point.
    pub kind: FretDotType,
}

/// Marqueur d'une corde.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FretMarker {
    /// Indice de corde interne.
    pub string: u8,
    /// Type de marqueur.
    pub kind: FretMarkerType,
}

/// Barre (`<barre start end>fret</barre>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FretBarre {
    /// Première corde, `-1` si non précisée.
    pub start: i8,
    /// Dernière corde, `-1` pour la dernière corde.
    pub end: i8,
    /// Frette de la barre.
    pub fret: u8,
}

/// Grille d'accord.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FretDiagram {
    /// Nombre de cordes (`<strings>`, défaut de style 6).
    pub strings: u8,
    /// Nombre de frettes affichées (`<frets>`, défaut de style 4).
    pub frets: u8,
    /// Décalage de la première frette (`<fretOffset>`) : `first-fret` vaut
    /// `fret_offset + 1`.
    pub fret_offset: u8,
    /// Vrai si le sillet est dessiné (`<showNut>`).
    pub show_nut: bool,
    /// Points, dans l'ordre du fichier.
    pub dots: Vec<FretDot>,
    /// Marqueurs, dans l'ordre du fichier.
    pub markers: Vec<FretMarker>,
    /// Barres, dans l'ordre du fichier.
    pub barres: Vec<FretBarre>,
}

impl Default for FretDiagram {
    fn default() -> Self {
        Self {
            strings: 6,
            frets: 4,
            fret_offset: 0,
            show_nut: true,
            dots: Vec::new(),
            markers: Vec::new(),
            barres: Vec::new(),
        }
    }
}
