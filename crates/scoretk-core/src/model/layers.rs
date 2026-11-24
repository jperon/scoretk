// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
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

//! Optional score layers and structured conversion losses.
//!
//! The event spine lives in `CoreScore` (always present). Everything a
//! direction may legitimately drop is assigned to one of the layers below;
//! [`ConversionReport`] records what was dropped, with an optional position.
//! No reader or writer is required to fill the report yet: this module is
//! additive and keeps every existing `Score` field reachable by `Deref`.

use std::borrow::Cow;

use super::chord::{BeamMode, StemDirection};
use super::fraction::Fraction;
use super::ids::Track;
use super::style::StyleSettings;
use super::text::{Credit, TboxText};

/// Set of model layers, stored as a compact bit set.
///
/// This is a hand-written bit set rather than a dependency: the core stays
/// dependency-light and portable. The bit values are part of the internal API
/// and are not serialised in any file format.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Layers(u8);

impl Layers {
    /// No layer.
    pub const NONE: Self = Self(0);
    /// Exact time, ids, arenas and the event spine (always present).
    pub const COMMON: Self = Self(1 << 0);
    /// Measure grid, key, time, clef, tuplets and repeats.
    pub const METRIC: Self = Self(1 << 1);
    /// Grace notes and expressive events.
    pub const EVENTS: Self = Self(1 << 2);
    /// Lyrics, credits and analysis.
    pub const TEXT: Self = Self(1 << 3);
    /// Velocity, transposition and other playback-only data.
    pub const PLAYBACK: Self = Self(1 << 4);
    /// Style, layout and engraving flags.
    pub const ENGRAVING: Self = Self(1 << 5);
    /// Per-format payloads no other direction can interpret.
    pub const EXTENSIONS: Self = Self(1 << 6);
    /// All declared layers.
    pub const ALL: Self = Self(
        Self::COMMON.0
            | Self::METRIC.0
            | Self::EVENTS.0
            | Self::TEXT.0
            | Self::PLAYBACK.0
            | Self::ENGRAVING.0
            | Self::EXTENSIONS.0,
    );

    /// Empty set.
    pub const fn empty() -> Self {
        Self::NONE
    }

    /// Raw bit pattern, for diagnostics only.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// True when every layer of `other` is also in `self`.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Union of two layer sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Human-readable name of a single layer (or a combination).
    pub fn name(self) -> &'static str {
        match self {
            Self::COMMON => "common",
            Self::METRIC => "metric",
            Self::EVENTS => "events",
            Self::TEXT => "text",
            Self::PLAYBACK => "playback",
            Self::ENGRAVING => "engraving",
            Self::EXTENSIONS => "extensions",
            _ => "mixed",
        }
    }
}

/// One structured loss: data the core could hold but the conversion dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loss {
    /// Layer the lost item belongs to.
    pub layer: Layers,
    /// Stable, format-independent description of the item.
    pub item: Cow<'static, str>,
    /// Position in the score, when known.
    pub tick: Option<Fraction>,
    /// Track in the score, when known.
    pub track: Option<Track>,
}

impl Loss {
    /// Builds a loss without position information.
    pub fn new(layer: Layers, item: impl Into<Cow<'static, str>>) -> Self {
        Self {
            layer,
            item: item.into(),
            tick: None,
            track: None,
        }
    }
}

/// Ordered list of losses produced by a read or a write.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConversionReport {
    /// Losses, in the order they were observed (never sorted: order is data).
    pub losses: Vec<Loss>,
}

impl ConversionReport {
    /// Empty report.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a loss, with optional tick and track.
    pub fn record(
        &mut self,
        layer: Layers,
        item: impl Into<Cow<'static, str>>,
        tick: Option<Fraction>,
        track: Option<Track>,
    ) {
        self.losses.push(Loss {
            layer,
            item: item.into(),
            tick,
            track,
        });
    }

    /// Number of recorded losses in a given layer.
    pub fn lost(&self, layer: Layers) -> usize {
        self.losses
            .iter()
            .filter(|loss| loss.layer == layer)
            .count()
    }
}

/// L1 metric structure. Empty until step 4 of the migration plan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metric {}

/// L3 text and analysis. Empty until later migration steps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Text {}

/// L4 playback data. Empty until step 3 of the migration plan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Playback {}

/// L5 engraving and layout: score-wide style and frame texts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Engraving {
    /// Global style settings (page size, hidden staves, lyrics placement…).
    pub style: StyleSettings,
    /// Title-frame texts exported as MusicXML `<credit>`.
    pub credits: Vec<Credit>,
    /// `TBox` frame texts exported as MusicXML directional `<words>`.
    pub tbox_texts: Vec<TboxText>,
}

/// L5 engraving flags attached to a [`super::ChordRest`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChordRestEngraving {
    /// Beam mode (`BeamMode` in `engraving/types/types.h`).
    pub beam_mode: BeamMode,
    /// Beam direction read from `<Beam><StemDirection>`.
    ///
    /// MuseScore attaches the `Beam` object it reads to the first chord of the
    /// group; its explicit direction then overrides the computed one. `None`
    /// means no direction was written (4.x files write an empty `<Beam>`).
    pub beam_direction: Option<StemDirection>,
}

/// L5 engraving flags attached to a [`super::Chord`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChordEngraving {
    /// Stem direction (`Chord::stemDirection()` in `engraving/dom/chord.h`).
    pub stem: StemDirection,
}

/// L6 per-format extensions. Empty until step 5 of the migration plan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Extensions {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_sets() {
        assert_eq!(Layers::NONE.bits(), 0);
        let both = Layers::METRIC.union(Layers::ENGRAVING);
        assert!(both.contains(Layers::METRIC));
        assert!(both.contains(Layers::ENGRAVING));
        assert!(!both.contains(Layers::TEXT));
        assert!(Layers::ALL.contains(both));
        assert_eq!(Layers::ENGRAVING.name(), "engraving");
    }

    #[test]
    fn report_records_losses_in_order() {
        let mut report = ConversionReport::new();
        report.record(Layers::ENGRAVING, "credit/font", None, None);
        report.record(Layers::METRIC, "barline", Some(Fraction::QUARTER), None);
        assert_eq!(report.losses.len(), 2);
        assert_eq!(report.lost(Layers::ENGRAVING), 1);
        assert_eq!(report.losses[1].layer, Layers::METRIC);
        assert_eq!(report.losses[1].tick, Some(Fraction::QUARTER));
    }
}
