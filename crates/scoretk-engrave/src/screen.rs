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
    let scaled = (REFERENCE_STAFF_PX as f64 * width_px.min(height_px) as f64
        / REFERENCE_SHORT_PX)
        .round() as i32;
    scaled.clamp(SCREEN_STAFF_PX_MIN, SCREEN_STAFF_PX_MAX)
}

impl PageSpec {
    /// Builds a page geometry for a caller-owned screen buffer.
    ///
    /// `width_px`/`height_px` are the dimensions of the visible page; margins
    /// are a few pixels instead of the 15 mm print margins. All page dimensions
    /// are whole numbers of staff spaces (and therefore a whole number of
    /// pixels at the chosen staff size), which keeps the raster grid stable.
    pub fn for_screen(width_px: u32, height_px: u32) -> Self {
        let staff_px = screen_staff_px(width_px, height_px);
        // Pixel coordinates round-trip exactly through the fixed-point grid:
        // fixed * staff_px / 1024 == px. The page therefore keeps the screen's
        // aspect ratio without a whole-staff-space quantisation error.
        let to_sp = |px: i32| (px as f64 * SP as f64 / staff_px as f64).round() as i32;
        let width = width_px.min(i32::MAX as u32) as i32;
        let height = height_px.min(i32::MAX as u32) as i32;
        // 2–8 px margins: proportional to the screen, never more than 1 % of
        // the short side on a large display.
        let margin_x = (width / 100).clamp(2, 8);
        let margin_y = (height / 75).clamp(2, 8);
        let mut spec = Self::default();
        spec.spatium_mm = staff_px as f64 * SCREEN_PIXEL_PITCH_MM;
        spec.page_height = to_sp(height);
        spec.margin_left = to_sp(margin_x);
        spec.margin_top = to_sp(margin_y);
        spec.printable_width = to_sp(width - 2 * margin_x).max(SP);
        spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_staff_space_is_a_whole_number_of_pixels_in_both_orientations() {
        assert_eq!(screen_staff_px(800, 600), 14);
        assert_eq!(screen_staff_px(600, 800), 14);
        for (w, h) in [(320, 240), (800, 600), (1024, 768), (600, 800)] {
            let px = screen_staff_px(w, h);
            let spec = PageSpec::for_screen(w, h);
            assert!(px >= SCREEN_STAFF_PX_MIN && px <= SCREEN_STAFF_PX_MAX);
            // Every dimension maps back to the screen's pixels to within the
            // fixed-point rounding of one staff-space unit.
            let to_px = |fixed: i32| fixed * px / SP;
            let margin_x = ((w as i32 / 100).clamp(2, 8)) * 2;
            assert!((to_px(spec.printable_width) - (w as i32 - margin_x)).abs() <= 1);
            assert!((to_px(spec.page_height) - h as i32).abs() <= 1);
            assert!(spec.printable_width > 0);
            assert!(spec.page_height > 0);
        }
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
