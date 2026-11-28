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

//! Rendering options for callers that replace Verovio (cantus).
//!
//! The defaults are a no-op: an `Engraver` built with `EngraveOptions::default()`
//! produces exactly the same bytes as `Engraver::from_score` (the print
//! oracle of `docs/agents/engraving.md` § 6 is unchanged).
//!
//! Mapping from the Verovio options cantus sets today
//! (`src/worker.coffee:168-194`):
//!
//! | Verovio | scoretk |
//! | --- | --- |
//! | `pageWidth`, `pageHeight` (units at 100 %) | `page_width_px`, `page_height_px` (display px) |
//! | `scale` (percent, default 40) | `scale = verovio_scale / 40` (1.0 at 40 %) |
//! | `adjustPageHeight` | `adjust_page_height` |
//! | `lyricSize` (MEI units, default 4.5) | `lyric_size` (same unit) |
//! | `condense: "auto"`, `condenseFirstPage: false` | `hide_empty_staves` (first system kept) |
//! | `systemMaxPerPage: 0` (unlimited) / `1` | `max_systems_per_page` / `one_system_per_page` |
//! | `breaks: "auto"` (the Verovio default) | `ignore_encoded_breaks` |
//! | `transpose` (signed semitones) | `transpose_semitones` |
//!
//! Cantus computes `pageWidth = innerWidth * 100 / scale`, so the displayed
//! page stays `innerWidth` px wide while the staff grows with `scale`. Passing
//! the *displayed* px and the ratio `scale/40` reproduces that behaviour.
//! Verovio's page geometry is not a screen geometry: at its default 40 % zoom
//! one staff space is 7.2 px, not the 10–20 px chosen by `screen_staff_px` for
//! e-ink. `PageUnits::Verovio` selects that scale, so the same displayed page
//! holds as many systems and measures as in Verovio.

use alloc::vec::Vec;

/// Verovio's default `lyricSize` in MEI units; our 10 pt style is 1.0.
pub const VEROVIO_LYRIC_SIZE: f64 = 4.5;
/// Verovio's default zoom. `scale = verovio_scale / 40` keeps our default at 1.0.
pub const VEROVIO_DEFAULT_SCALE_PERCENT: u32 = 40;
/// Pixels per staff space at `scale = 1.0` on the reference short side
/// (`screen::REFERENCE_SHORT_PX`, 600 px), from `screen_staff_px`.
pub const REFERENCE_STAFF_PX: i32 = 14;
/// Verovio's staff space at 100 % zoom, in displayed pixels: 18 of its
/// 100 % page units, i.e. 7.2 px at cantus' default `scale = 40` %.
/// Measured on Verovio 4.5.1: an A4 viewBox of 30 000 units in a 1 200 px
/// page has staff lines 180 units apart.
pub const VEROVIO_STAFF_PX_AT_100: f64 = 18.0;

/// Interpretation of `page_width_px`/`page_height_px`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageUnits {
    /// Keep the score's print page (the default, byte-identical path).
    Score,
    /// E-ink screen geometry: an integer number of pixels per staff space.
    Screen,
    /// Verovio geometry: 7.2 px per staff space at 40 % zoom.
    Verovio,
}

/// Options of one engraving pass. All of them are additive: the default
/// values keep the print layout byte for byte.
#[derive(Clone, Debug, PartialEq)]
pub struct EngraveOptions {
    /// Staff-size multiplier (zoom). 1.0 keeps the screen layout of
    /// `PageSpec::for_screen`; 2.0 doubles the pixels per staff space and
    /// therefore halves the number of staff spaces that fit the page.
    pub scale: f64,
    /// Displayed page width in pixels. `None` keeps the score's print width.
    pub page_width_px: Option<u32>,
    /// Displayed page height in pixels. `None` keeps the score's print height.
    pub page_height_px: Option<u32>,
    /// Page geometry used with the pixel dimensions above.
    pub page_units: PageUnits,
    /// Ignore `<print new-system>`/`<print new-page>` inherited from the
    /// MusicXML and break lines by width only (Verovio `breaks: "auto"`).
    pub ignore_encoded_breaks: bool,
    /// Maximum number of systems per page; 0 means unlimited (Verovio's
    /// `systemMaxPerPage: 0`).
    pub max_systems_per_page: usize,
    /// Shrink the page viewBox to the last system's bottom plus a margin,
    /// as Verovio's `adjustPageHeight`.
    pub adjust_page_height: bool,
    /// Put one system on each page (Verovio's horizontal scroll mode,
    /// `systemMaxPerPage: 1`).
    pub one_system_per_page: bool,
    /// Lyric size in Verovio MEI units (default 4.5). The lyric point size is
    /// `10 pt * lyric_size / 4.5`.
    pub lyric_size: f64,
    /// Multiplier of every text style (credits, lyrics, measure numbers,
    /// instrument names), applied on top of `PageSpec::text_scale` for frame
    /// credits. 1.0 keeps the print size; it never changes the staff size.
    pub text_scale: f64,
    /// Hide staves with no note in a system, except the first system
    /// (`condense: "auto"`, `condenseFirstPage: false`).
    pub hide_empty_staves: bool,
    /// Chromatic transposition in signed semitones (0 = none). Applied to
    /// written note pitches with the model's `transpose_tpc` helper.
    pub transpose_semitones: i32,
    /// Runtime part-size override: `(part index, user mag in percent)`.
    /// It replaces `Staff::user_mag_percent` for that part, so cantus can
    /// change a staff size without re-reading the score.
    pub part_mag_percent: Vec<(usize, i32)>,
}

impl Default for EngraveOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            page_width_px: None,
            page_height_px: None,
            page_units: PageUnits::Score,
            ignore_encoded_breaks: false,
            max_systems_per_page: 0,
            adjust_page_height: false,
            one_system_per_page: false,
            lyric_size: VEROVIO_LYRIC_SIZE,
            text_scale: 1.0,
            hide_empty_staves: false,
            transpose_semitones: 0,
            part_mag_percent: Vec::new(),
        }
    }
}

impl EngraveOptions {
    /// Builds options from the values cantus sends to Verovio.
    ///
    /// `page_width`/`page_height` are the Verovio page dimensions in 100 %
    /// units (i.e. `innerWidth * 100 / scale`); `scale_percent` is Verovio's
    /// zoom. The displayed pixels are recovered by multiplying back, and our
    /// zoom is the ratio to the reference 40 %.
    #[allow(clippy::too_many_arguments)] // mirrors the Verovio option set
    pub fn from_verovio(
        page_width: u32,
        page_height: u32,
        scale_percent: u32,
        lyric_size: f64,
        adjust_page_height: bool,
        hide_empty_staves: bool,
        systems_per_page: usize,
        transpose_semitones: i32,
    ) -> Self {
        let zoom = scale_percent.max(1) as f64 / VEROVIO_DEFAULT_SCALE_PERCENT as f64;
        let to_px = |units: u32| -> u32 {
            ((u64::from(units) * u64::from(scale_percent.max(1)) / 100).max(1))
                .min(u64::from(u32::MAX)) as u32
        };
        Self {
            scale: zoom,
            page_width_px: Some(to_px(page_width)),
            page_height_px: Some(to_px(page_height)),
            page_units: PageUnits::Verovio,
            // Verovio's default `breaks` is "auto": encoded breaks are
            // ignored and systems are filled to the printable width.
            ignore_encoded_breaks: true,
            max_systems_per_page: systems_per_page,
            adjust_page_height,
            one_system_per_page: systems_per_page == 1,
            lyric_size,
            hide_empty_staves,
            transpose_semitones,
            ..Self::default()
        }
    }

    /// Percent size of a part, when cantus overrides the model value.
    pub fn part_mag(&self, part: usize) -> Option<i32> {
        self.part_mag_percent
            .iter()
            .find(|(index, _)| *index == part)
            .map(|(_, percent)| *percent)
    }

    /// Fixed-point staff magnitude (1024 = 1.0) of a model percentage.
    pub fn mag_fixed(percent: i32) -> i32 {
        ((percent.clamp(1, 400) as i64 * 1024 + 50) / 100) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_a_no_op() {
        let options = EngraveOptions::default();
        assert_eq!(options.scale, 1.0);
        assert_eq!(options.lyric_size, VEROVIO_LYRIC_SIZE);
        assert_eq!(options.text_scale, 1.0);
        assert!(options.page_width_px.is_none() && options.page_height_px.is_none());
        assert_eq!(options.page_units, PageUnits::Score);
        assert!(!options.ignore_encoded_breaks);
        assert_eq!(options.max_systems_per_page, 0);
        assert!(!options.adjust_page_height && !options.one_system_per_page);
        assert!(!options.hide_empty_staves);
        assert_eq!(options.transpose_semitones, 0);
        assert!(options.part_mag_percent.is_empty());
    }

    #[test]
    fn verovio_options_recover_display_pixels_and_zoom() {
        // cantus: innerWidth 1200, scale 40 → pageWidth 3000 units.
        let options = EngraveOptions::from_verovio(3000, 2000, 40, 4.5, true, true, 1, 2);
        assert_eq!(options.page_width_px, Some(1200));
        assert_eq!(options.page_height_px, Some(800));
        assert_eq!(options.scale, 1.0);
        assert_eq!(options.page_units, PageUnits::Verovio);
        assert!(options.ignore_encoded_breaks);
        assert_eq!(options.max_systems_per_page, 1);
        assert!(options.adjust_page_height && options.hide_empty_staves);
        assert!(options.one_system_per_page);
        assert_eq!(options.transpose_semitones, 2);
        // Doubling the zoom halves the displayed page in 100 % units but
        // doubles our pixels per staff space.
        let zoomed = EngraveOptions::from_verovio(1500, 1000, 80, 4.5, false, false, 0, 0);
        assert_eq!(zoomed.page_width_px, Some(1200));
        assert_eq!(zoomed.scale, 2.0);
        assert!(!zoomed.one_system_per_page);
        assert_eq!(zoomed.max_systems_per_page, 0);
    }

    #[test]
    fn verovio_auto_breaks_ignore_encoded_breaks_and_page_units_are_not_screen_units() {
        // Cantus vertical mode: 500 × 900 px at 40 %, `systemMaxPerPage: 0`.
        let options = EngraveOptions::from_verovio(1250, 2250, 40, 4.5, true, false, 0, 0);
        assert_eq!(options.page_width_px, Some(500));
        assert_eq!(options.page_height_px, Some(900));
        assert!(
            options.ignore_encoded_breaks,
            "Verovio breaks default to auto"
        );
        assert_eq!(options.max_systems_per_page, 0, "0 means unlimited");
        assert!(!options.one_system_per_page);
        assert_eq!(options.page_units, PageUnits::Verovio);
        // At 40 %, the page holds 500 / 7.2 staff spaces across, not the
        // 500 / 12 given by the e-ink `screen_staff_px` for the same pixels.
        let staff_px = VEROVIO_STAFF_PX_AT_100 * 40.0 / 100.0;
        assert_eq!(staff_px, 7.2);
        assert!((staff_px - 14.0 * 500.0 / 600.0).abs() > 4.0);
    }

    #[test]
    fn part_mag_override_and_fixed_point() {
        let mut options = EngraveOptions::default();
        options.part_mag_percent.push((1, 70));
        assert_eq!(options.part_mag(1), Some(70));
        assert_eq!(options.part_mag(0), None);
        assert_eq!(EngraveOptions::mag_fixed(70), 717);
        assert_eq!(EngraveOptions::mag_fixed(100), 1024);
    }
}
