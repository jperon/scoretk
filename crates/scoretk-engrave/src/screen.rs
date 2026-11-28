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

//! Page geometry for a small e-ink screen (plan §4, "Pixel-grid hinting").
//!
//! This is a new, screen-specific entry point: no existing layout code is
//! changed. The staff size is chosen so that one staff space covers an
//! **integer number of pixels**; the rasteriser then snaps lines and stems to
//! that grid. The page keeps the screen's aspect ratio in both orientations.

use scoretk_core::model::Score;

use crate::layout::PageSpec;
use crate::style::SP;

/// Physical pitch of a pixel on the reference screen (200 ppi, 5″/800 px).
pub const SCREEN_PIXEL_PITCH_MM: f64 = 25.4 / 200.0;
/// Staff size bounds, in pixels per staff space, for usable screens.
pub const SCREEN_STAFF_PX_MIN: i32 = 10;
pub const SCREEN_STAFF_PX_MAX: i32 = 20;
/// Reference short side of the Kobo Mini's display, in pixels.
const REFERENCE_SHORT_PX: f64 = 600.0;
const REFERENCE_STAFF_PX: i32 = 14;

/// Whole pixels per staff space for the given screen.
///
/// The short side is used so that rotating the device does not change the
/// notation size: a 600 × 800 and an 800 × 600 screen both get 14 px/sp.
pub fn screen_staff_px(width_px: u32, height_px: u32) -> i32 {
    if width_px == 0 || height_px == 0 {
        return REFERENCE_STAFF_PX;
    }
    let scaled = (REFERENCE_STAFF_PX as f64 * width_px.min(height_px) as f64 / REFERENCE_SHORT_PX)
        .round() as i32;
    scaled.clamp(SCREEN_STAFF_PX_MIN, SCREEN_STAFF_PX_MAX)
}

/// Whole pixels per staff space at an explicit zoom.
///
/// The auto-fitted `screen_staff_px` is multiplied by `scale` and rounded, so
/// the raster grid stays integral. `PageSpec::for_px` and `Document` must use
/// this one function: the stored staff size and the drawn staff size then
/// always agree.
pub fn screen_staff_px_at(width_px: u32, height_px: u32, scale: f64) -> i32 {
    let base = screen_staff_px(width_px, height_px);
    ((base as f64 * scale.max(1.0 / base as f64)).round() as i32).max(1)
}

impl PageSpec {
    /// Builds a page geometry for a caller-owned screen buffer.
    ///
    /// `width_px`/`height_px` are the dimensions of the visible page; margins
    /// are a few pixels instead of the 15 mm print margins. All page dimensions
    /// are whole numbers of staff spaces (and therefore a whole number of
    /// pixels at the chosen staff size), which keeps the raster grid stable.
    pub fn for_screen(width_px: u32, height_px: u32) -> Self {
        Self::for_px(width_px, height_px, 1.0)
    }

    /// Same geometry at an explicit zoom. `scale` multiplies the pixels per
    /// staff space, so a larger zoom fits fewer staff spaces in the same
    /// displayed pixel page. // Cantus passes `verovio_scale / 40` here: its
    /// `pageWidth = innerWidth * 100 / scale` compensates the physical page.
    pub fn for_px(width_px: u32, height_px: u32, scale: f64) -> Self {
        Self::for_staff_px(
            width_px,
            height_px,
            screen_staff_px_at(width_px, height_px, scale) as f64,
        )
    }

    /// Geometry of a Verovio page expressed in *displayed* pixels.
    ///
    /// Cantus sends `pageWidth = innerWidth * 100 / scale` and `scale` in
    /// percent. Verovio's staff space is 18 of its 100 % page units, hence
    /// `18 * scale / 100` displayed pixels (7.2 px at the default 40 %). The
    /// e-ink `screen_staff_px` would give 10–20 px and stack only one system
    /// per page; the zoom is already included in `scale` here.
    pub fn for_verovio_px(width_px: u32, height_px: u32, scale: f64) -> Self {
        // `scale` here is the zoom ratio (Verovio percent / 40).
        let staff_px = crate::options::VEROVIO_STAFF_PX_AT_100
            * scale.max(1.0 / 40.0)
            * crate::options::VEROVIO_DEFAULT_SCALE_PERCENT as f64
            / 100.0;
        Self::for_staff_px(width_px, height_px, staff_px.max(0.5))
    }

    /// Common geometry for a page whose staff space is a number of pixels.
    fn for_staff_px(width_px: u32, height_px: u32, staff_px: f64) -> Self {
        // Pixel coordinates round-trip exactly through the fixed-point grid:
        // fixed * staff_px / 1024 == px. The page therefore keeps the screen's
        // aspect ratio without a whole-staff-space quantisation error.
        let to_sp = |px: i32| (px as f64 * SP as f64 / staff_px).round() as i32;
        let width = width_px.min(i32::MAX as u32) as i32;
        let height = height_px.min(i32::MAX as u32) as i32;
        // The margins are in whole staff spaces, not a handful of pixels:
        // SMuFL outlines hang outside their layout box (Leland's `gClef8va`
        // reaches about 3.4 sp above the top staff line, see its bbox in
        // `scoretk-smufl/src/glyphs.rs`). With only 6–8 px of margin the
        // first system was clipped at the top and the clef touched the left
        // edge. Two spaces at the sides and four at the top/bottom keep all
        // emitted outlines inside the caller's buffer. `assign_pages` uses
        // `margin_top` for both the top and the bottom of a page.
        let margin_x = (2.0 * staff_px).round().max(1.0) as i32;
        let margin_y = (4.0 * staff_px).round().max(1.0) as i32;
        Self {
            spatium_mm: staff_px * SCREEN_PIXEL_PITCH_MM,
            page_height: to_sp(height),
            margin_left: to_sp(margin_x),
            margin_top: to_sp(margin_y),
            printable_width: to_sp(width - 2 * margin_x).max(SP),
            // `margin_top` already reserves the top and bottom clearance:
            // the print page's `staffUpperBorder`/`staffLowerBorder` (7 sp
            // each, `styledef.cpp`) would be added on top of it. On a screen
            // whose page height shrinks with the zoom, that double margin
            // left room for a single system per page.
            staff_upper_border: 0,
            staff_lower_border: 0,
            ..Self::default()
        }
    }

    /// Screen geometry that also carries the score's title frame and a text
    /// scale fitted to the screen (`Engraver::new` does not read the `Score`).
    ///
    /// `for_screen` alone leaves `title_frame_height` at zero: credits were
    /// then drawn over the first system. The frame height comes from the same
    /// computation as the print page (`from_score`), while frame text is
    /// scaled by the ratio of the screen's printable width to the score's.
    /// A 22 pt title that fits an A4 page therefore fits a 600 px screen
    /// without changing any layout algorithm.
    pub fn for_screen_with_score(width_px: u32, height_px: u32, score: &Score) -> Self {
        Self::for_screen_with_score_at(width_px, height_px, score, 1.0, 1.0)
    }

    /// [`Self::for_screen_with_score`] at an explicit staff zoom and an
    /// explicit multiplier of every text style.
    ///
    /// `scale` multiplies the pixels per staff space (integer, as in
    /// [`Self::for_px`]); `text_scale` reaches `PageSpec::user_text_scale`,
    /// on top of the automatic credit fit computed here. Neither changes the
    /// other: text can grow while the staff size and the note glyphs stay.
    pub fn for_screen_with_score_at(
        width_px: u32,
        height_px: u32,
        score: &Score,
        scale: f64,
        text_scale: f64,
    ) -> Self {
        let mut spec = Self::for_px(width_px, height_px, scale);
        let print = Self::from_score(score);
        spec.title_frame_height = print.title_frame_height;
        spec.text_scale =
            (spec.printable_width as f64 / print.printable_width.max(SP) as f64).clamp(0.0, 1.0);
        spec.user_text_scale = text_scale;
        spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{Credit, CreditStyle, Part, RichText, Score, Staff};

    #[test]
    fn a_staff_space_is_a_whole_number_of_pixels_in_both_orientations() {
        assert_eq!(screen_staff_px(800, 600), 14);
        assert_eq!(screen_staff_px(600, 800), 14);
        for (w, h) in [(320, 240), (800, 600), (1024, 768), (600, 800)] {
            let px = screen_staff_px(w, h);
            let spec = PageSpec::for_screen(w, h);
            assert!((SCREEN_STAFF_PX_MIN..=SCREEN_STAFF_PX_MAX).contains(&px));
            // Every dimension maps back to the screen's pixels to within the
            // fixed-point rounding of one staff-space unit.
            let to_px = |fixed: i32| fixed * px / SP;
            let margin_x = 4 * px; // two spaces on each side
            assert!((to_px(spec.printable_width) - (w as i32 - margin_x)).abs() <= 1);
            assert!((to_px(spec.page_height) - h as i32).abs() <= 1);
            assert!(spec.printable_width > 0);
            assert!(spec.page_height > 0);
            // Tall clefs reach ~3.4 sp above the top staff line: keep at
            // least that much room, and at least one space at the sides.
            assert!(to_px(spec.margin_top) * 10 >= 34 * px);
            assert!(to_px(spec.margin_left) >= px);
        }
    }

    #[test]
    fn zoom_changes_pixels_per_staff_space_not_the_displayed_page() {
        let normal = PageSpec::for_px(800, 600, 1.0);
        let zoomed = PageSpec::for_px(800, 600, 2.0);
        let page_px = |spec: &PageSpec, fixed: i32| {
            let staff_px = (spec.spatium_mm / SCREEN_PIXEL_PITCH_MM) as i32;
            fixed * staff_px / SP
        };
        assert!(
            (page_px(&normal, normal.printable_width + 2 * normal.margin_left) - 800).abs() <= 2
        );
        assert!(
            (page_px(&zoomed, zoomed.printable_width + 2 * zoomed.margin_left) - 800).abs() <= 2
        );
        // A doubled staff is twice as many millimetres, hence half as many
        // staff spaces fit the same 800 px.
        assert_eq!(zoomed.spatium_mm, normal.spatium_mm * 2.0);
        let ratio = zoomed.printable_width as f64 / normal.printable_width as f64;
        assert!(ratio > 0.45 && ratio <= 0.5, "width ratio {ratio}");
    }

    #[test]
    fn caller_scale_sizes_notes_and_text_independently() {
        let score = Score::new();
        let plain = PageSpec::for_screen_with_score_at(600, 800, &score, 1.0, 1.0);
        let zoomed = PageSpec::for_screen_with_score_at(600, 800, &score, 2.0, 1.0);
        let text = PageSpec::for_screen_with_score_at(600, 800, &score, 1.0, 2.0);
        assert_eq!(
            screen_staff_px_at(600, 800, 2.0),
            2 * screen_staff_px(600, 800)
        );
        assert_eq!(zoomed.spatium_mm, 2.0 * plain.spatium_mm);
        // Text scaling leaves the staff geometry alone, and vice versa.
        assert_eq!(text.spatium_mm, plain.spatium_mm);
        assert_eq!(text.printable_width, plain.printable_width);
        assert_eq!(zoomed.user_text_scale, 1.0);
        assert_eq!(text.user_text_scale, 2.0);
        let em = crate::layout::text_em;
        assert_eq!(em(&text, 10.0), 2 * em(&plain, 10.0));
    }

    #[test]
    fn verovio_page_units_hold_more_staff_space_than_screen_units() {
        // Cantus at 1200 × 800 px, zoom 40 %: Verovio's staff is 7.2 px, so
        // the page is about 167 sp wide and 111 sp tall.
        let spec = PageSpec::for_verovio_px(1200, 800, 1.0);
        let screen = PageSpec::for_px(1200, 800, 1.0);
        assert_eq!(spec.spatium_mm, 7.2 * SCREEN_PIXEL_PITCH_MM);
        assert!(spec.printable_width > screen.printable_width * 2);
        assert!(spec.page_height > screen.page_height * 2);
        let zoomed = PageSpec::for_verovio_px(1200, 800, 2.0);
        assert_eq!(zoomed.spatium_mm, spec.spatium_mm * 2.0);
    }

    #[test]
    fn a_screen_page_fits_the_score_title_frame() {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno."));
        score.add_staff_with(part, Staff::new(part));
        let mut credit = Credit::new(
            RichText::new("A rather long title with several words"),
            CreditStyle::Title,
        );
        credit.page = 1;
        credit.vbox_height = 10.0;
        score.add_credit(credit);

        let print = PageSpec::from_score(&score);
        let screen = PageSpec::for_screen_with_score(600, 800, &score);
        assert_eq!(screen.title_frame_height, print.title_frame_height);
        assert!(screen.title_frame_height > 0);
        assert!(screen.text_scale > 0.0 && screen.text_scale < 1.0);
        // The scaled title must be narrower than the screen's printable
        // width, unlike the unscaled print-size credit run.
        let font_size = CreditStyle::Title.default_font_size();
        let em = (font_size * 25.4 / 72.0 / screen.spatium_mm * SP as f64) as i32;
        let scaled = (em as f64 * screen.text_scale) as i32;
        let title = "A rather long title with several words";
        assert!(
            crate::layout::text_run_width(title, scaled, false) < screen.printable_width,
            "title still wider than the screen"
        );
    }

    #[test]
    fn the_page_keeps_the_screen_aspect_ratio() {
        let spec = PageSpec::for_screen(800, 600);
        let page_width = spec.printable_width + 2 * spec.margin_left;
        // 800/600 within one staff space of rounding error.
        assert!((page_width as f64 / spec.page_height as f64 - 800.0 / 600.0).abs() < 0.01);
        assert!(spec.spatium_mm > 1.0);
    }
}
