// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/tremolobar.h` de MuseScore Studio, Copyright (C)
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

//! Barre de vibrato (bend de guitare).
//!
//! Portage de `TremoloBar` (`engraving/dom/tremolobar.h`). L'export MusicXML
//! l'écrit en `<technical><bend><bend-alter>…</bend-alter><with-bar>…`
//! (`ExportMusicXml::writeGuitarBend`).
/// Demi-ton exprimé en centièmes (`PitchValue::PITCH_FOR_SEMITONE`).
pub const PITCH_FOR_SEMITONE: f64 = 100.0;

/// Type de barre de vibrato (`TremoloBarType`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TremoloBarType {
    /// « Dip » (`dip`), courbe par défaut.
    #[default]
    Dip,
    /// « Dive » (`dive`).
    Dive,
    /// « Release up » (`release (up)`).
    ReleaseUp,
    /// « Inverted dip » (`inverted dip`).
    InvertedDip,
    /// « Return » (`return`).
    Return,
    /// « Release down » (`release (down)`).
    ReleaseDown,
    /// Courbe personnalisée (`custom`).
    Custom,
}

impl TremoloBarType {
    /// Nom MusicXML du type (`TConv::toXml(TremoloBarType)`).
    pub const fn musicxml_name(self) -> &'static str {
        match self {
            Self::Dip => "dip",
            Self::Dive => "dive",
            Self::ReleaseUp => "release (up)",
            Self::InvertedDip => "inverted dip",
            Self::Return => "return",
            Self::ReleaseDown => "release (down)",
            Self::Custom => "custom",
        }
    }
}

/// Point de la courbe (temps en pourcentage, hauteur en centièmes de demi-ton).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TremoloBarPoint {
    /// Position dans la note, de 0 à 100 (`PitchValue::time`).
    pub time: i32,
    /// Hauteur en centièmes de demi-ton (`PitchValue::pitch`).
    pub pitch: i32,
}

/// Barre de vibrato attachée à un segment.
///
/// Correspond à `TremoloBar` dans `engraving/dom/tremolobar.h`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TremoloBar {
    /// Type de courbe, déduit des points (`parseTremoloBarTypeFromCurve`).
    pub kind: TremoloBarType,
    /// Points de la courbe (`<point time="…" pitch="…"/>`).
    pub points: Vec<TremoloBarPoint>,
}

impl TremoloBar {
    /// Construit une barre de type « dip » sans point.
    pub fn new() -> Self {
        Self {
            kind: TremoloBarType::Dip,
            points: Vec::new(),
        }
    }

    /// Construit une barre à partir de sa courbe, en reconnaissant les types
    /// prédéfinis (`parseTremoloBarTypeFromCurve`) ; sinon `custom`.
    pub fn from_points(points: Vec<TremoloBarPoint>) -> Self {
        let curve = |values: &[(i32, i32)]| {
            values.len() == points.len()
                && values
                    .iter()
                    .zip(&points)
                    .all(|((time, pitch), point)| point.time == *time && point.pitch == *pitch)
        };
        let kind = if curve(&[(0, 0), (30, -100), (60, 0)]) {
            TremoloBarType::Dip
        } else if curve(&[(0, 0), (60, -150)]) {
            TremoloBarType::Dive
        } else if curve(&[(0, -150), (60, 0)]) {
            TremoloBarType::ReleaseUp
        } else if curve(&[(0, 0), (30, 100), (60, 0)]) {
            TremoloBarType::InvertedDip
        } else if curve(&[(0, 0), (60, 150)]) {
            TremoloBarType::Return
        } else if curve(&[(0, 150), (60, 0)]) {
            TremoloBarType::ReleaseDown
        } else {
            TremoloBarType::Custom
        };
        Self { kind, points }
    }

    /// Amplitude maximale de la courbe, en demi-tons (`bend-alter`).
    ///
    /// Reproduit `writeGuitarBend` : seul le point de plus grande amplitude
    /// (valeur absolue) compte, et la valeur est divisée par
    /// [`PITCH_FOR_SEMITONE`].
    pub fn bend_alter(&self) -> f64 {
        let mut max_pitch = 0;
        for point in &self.points {
            if point.pitch == 0 {
                continue;
            }
            max_pitch = if point.pitch < 0 {
                max_pitch.min(point.pitch)
            } else {
                max_pitch.max(point.pitch)
            };
        }
        f64::from(max_pitch) / PITCH_FOR_SEMITONE
    }
}

impl Default for TremoloBar {
    fn default() -> Self {
        Self::new()
    }
}
