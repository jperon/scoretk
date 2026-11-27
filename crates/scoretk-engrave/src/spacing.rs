// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port of `src/engraving/rendering/score/horizontalspacing.cpp:730-812,
// 1216-1276, 1438-1465` and `src/engraving/rendering/paddingtable.cpp:30-81,
// 94-104, 214-248` of MuseScore Studio 4.7.4 (tag v4.7.4), Copyright (C)
// MuseScore Limited and others, modified (rewritten in Rust) in 2026. Only
// compact numbers are kept: no `EngravingItem` graph and no floating-point
// libm call.
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

//! Duration spacing and the shape/padding rules of `HorizontalSpacing`.

use scoretk_core::model::StyleSettings;

use crate::style::SP;

/// Internal fixed-point scale (1/1 048 576), only used while computing the
/// power; callers receive 1/1024 staff spaces like the rest of the engraver.
const F: i64 = 1 << 20;
const LN2: i64 = 726_817; // ln(2) * F

/// `log2(x)` in 1/F units for `x > 0` in 1/F units.
///
/// `horizontalspacing.cpp:811` calls `pow(slope, log2(ticks / quarter))`.
/// A no_std crate has no libm, so log2 is evaluated with the atanh series
/// `ln(m) = 2 * (z + z³/3 + z⁵/5 + z⁷/7)`, `z = (m-1)/(m+1)`, after reducing
/// `m` to `[1, 2)`. Four terms are below 1e-6 there.
fn log2_fixed(x: i64) -> i64 {
    let mut mantissa = x.max(1);
    let mut exponent = 0i64;
    while mantissa >= 2 * F {
        mantissa /= 2;
        exponent += 1;
    }
    while mantissa < F {
        mantissa *= 2;
        exponent -= 1;
    }
    let z = (mantissa - F) * F / (mantissa + F);
    let z2 = z * z / F;
    let mut ln = z;
    let mut term = z;
    for divisor in [3i64, 5, 7] {
        term = term * z2 / F;
        ln += term / divisor;
    }
    exponent * F + ln * 2 * F / LN2
}

/// `2^y` in 1/F units for `y` in 1/F units, by Taylor expansion of `exp`.
fn exp2_fixed(y: i64) -> i64 {
    let floor = y.div_euclid(F);
    let fraction = y.rem_euclid(F);
    let z = fraction * LN2 / F;
    let mut result = F;
    let mut term = F;
    for divisor in [1i64, 2, 6, 24, 120] {
        term = term * z / F;
        result += term / divisor;
    }
    if floor >= 0 {
        result <<= floor.min(20) as u32;
    } else {
        result >>= (-floor).min(20) as u32;
    }
    result
}

/// // Port of `HorizontalSpacing::durationStretchForTicks`
/// (`horizontalspacing.cpp:805-812`) and the denominator of
/// `chordRestSegmentNaturalWidth` (`:743`): `slope^log2(ticks / quarter)`.
/// `ticks` and `quarter` use the same unit (480 per quarter in the model).
pub fn duration_stretch(slope: f64, ticks: i32, quarter: i32) -> i32 {
    if ticks <= 0 || quarter <= 0 || slope <= 0.0 {
        return 1024;
    }
    let log_slope = log2_fixed(((slope * F as f64) as i64).max(1));
    let log_ratio = log2_fixed((i64::from(ticks) * F / i64::from(quarter)).max(1));
    let power = exp2_fixed(log_slope * log_ratio / F);
    ((power * i64::from(SP) + F / 2) / F) as i32
}

/// The shape kind of an item in a segment, as far as the padding table is
/// concerned (`paddingtable.cpp` uses `ElementType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Note,
    Rest,
    Stem,
    Accidental,
    Dot,
    Ledger,
    Clef,
    KeySig,
    TimeSig,
    BarLine,
    Lyrics,
}

/// One shape element: `(left, right, top, bottom)` in 1/1024 staff spaces,
/// relative to the segment x, with the item's kind.
pub type Shape = (i32, i32, i32, i32, ShapeKind);

/// // Port of `PaddingTable::createTable` (`paddingtable.cpp:41-81, 94-104,
/// 214-248`) and `HorizontalSpacing::computePadding` (`:1438-1465`): the
/// horizontal padding between two neighbouring shapes, in 1/1024 sp. The
/// style values come from the score, not from the 4.7.4 defaults.
pub fn padding(a: ShapeKind, b: ShapeKind, style: &StyleSettings) -> i32 {
    use ShapeKind::*;
    let sp = |spatiums: f64| (spatiums * SP as f64) as i32;
    let style_sp = |value: f64| (value * SP as f64) as i32;
    let note_bar = style_sp(style.bar_note_distance_sp);
    let accidental_note = style_sp(style.accidental_note_distance_sp);
    match (a, b) {
        // `paddingtable.cpp:55` keeps note-note at the 0.1 sp minimum;
        // `:239,244` set note-stem to `minNoteDistance`.
        (Note, Note) => SP / 10,
        (Note, Stem) | (Stem, Note) => style_sp(style.min_note_distance_sp),
        (Note, Rest) | (Rest, Note) | (Stem, Rest) | (Rest, Stem) => sp(0.5),
        (Note, Accidental) | (Accidental, Note) => accidental_note.max(sp(0.35)),
        (Accidental, Stem) | (Stem, Accidental) => sp(0.35),
        (Note, Clef) | (Rest, Clef) => sp(0.8),
        (Note, KeySig) | (Rest, KeySig) => sp(0.75),
        (Note, TimeSig) | (Rest, TimeSig) => sp(0.75),
        (Note, BarLine) => note_bar,
        (BarLine, Note) | (BarLine, Rest) => note_bar,
        (BarLine, Accidental) => style_sp(style.bar_accidental_distance_sp),
        (Dot, Note) | (Dot, Rest) => {
            style_sp(style.dot_note_distance_sp).max(style_sp(style.dot_dot_distance_sp))
        }
        (Dot, Accidental) => sp(0.35),
        (Dot, BarLine) => sp(0.8),
        (Rest, Accidental) => sp(0.45),
        (Rest, BarLine) => sp(1.65),
        (Stem, Stem) => sp(0.85),
        (Ledger, Note) | (Note, Ledger) | (Stem, Ledger) | (Ledger, Stem) => sp(0.35),
        (Lyrics, _) | (_, Lyrics) => style_sp(style.lyrics_min_distance_sp),
        _ => SP / 10,
    }
}

/// // Port of `HorizontalSpacing::minHorizontalDistance(const Shape&, const
/// Shape&, …)` (`horizontalspacing.cpp:1216-1276`): the largest distance a
/// previous shape's right edge must have before this shape's left edge,
/// counting `padding` and the 0.1 sp absolute minimum. Vertical clearance is
/// 0.2 sp, or 0.1 sp next to an accidental (`:1636-1646`).
pub fn min_shape_distance(previous: &[Shape], next: &[Shape], style: &StyleSettings) -> i32 {
    let mut distance = i32::MIN;
    for &(l2, _r2, t2, b2, kind2) in next {
        for &(_l1, r1, t1, b1, kind1) in previous {
            let clearance = if matches!(kind1, ShapeKind::Accidental)
                || matches!(kind2, ShapeKind::Accidental)
            {
                SP / 10
            } else {
                SP / 5
            };
            let intersects = t1 - clearance <= b2 && t2 - clearance <= b1;
            let mut pad = padding(kind1, kind2, style).max(SP / 10);
            if !intersects && matches!(kind1, ShapeKind::Note) && matches!(kind2, ShapeKind::Note) {
                // Same-voice notes that do not overlap vertically may kern.
                pad = 0;
            }
            distance = distance.max(r1 - l2 + pad);
        }
    }
    distance.max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_stretch_follows_the_measure_spacing_slope() {
        // `horizontalspacing.cpp:805-812` with the 4.7.4 default slope 1.5.
        assert_eq!(duration_stretch(1.5, 480, 480), 1024);
        assert_eq!(duration_stretch(1.5, 240, 480), 683);
        assert_eq!(duration_stretch(1.5, 960, 480), 1536);
        assert_eq!(duration_stretch(1.5, 1920, 480), 2304);
        assert_eq!(duration_stretch(1.5, 720, 480), 1298);
        // `tests/corpus/v3/moonlight.mscx` sets `measureSpacing` 1.14.
        assert_eq!(duration_stretch(1.14, 240, 480), 898);
    }

    #[test]
    fn padding_table_uses_the_score_style_values() {
        let mut style = StyleSettings::for_version(4);
        assert_eq!(padding(ShapeKind::Note, ShapeKind::BarLine, &style), 1280);
        style.bar_note_distance_sp = 0.6;
        assert_eq!(padding(ShapeKind::Note, ShapeKind::BarLine, &style), 614);
        style.accidental_note_distance_sp = 0.12;
        assert_eq!(padding(ShapeKind::Note, ShapeKind::Accidental, &style), 358);
        assert_eq!(padding(ShapeKind::Stem, ShapeKind::Stem, &style), 870);
    }

    #[test]
    fn shapes_separate_when_their_vertical_ranges_overlap() {
        let style = StyleSettings::for_version(4);
        let head = (0, 1024, 0, 1024, ShapeKind::Note);
        let next = (1024, 2048, 0, 1024, ShapeKind::Note);
        assert_eq!(min_shape_distance(&[head], &[next], &style), 102);
        let high = (1024, 2048, 4 * SP, 5 * SP, ShapeKind::Note);
        assert_eq!(min_shape_distance(&[head], &[high], &style), 0);
    }
}
