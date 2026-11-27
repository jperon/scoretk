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

//! One engraved score, its screen geometry and a reusable 8-bit grey buffer.

use scoretk_engrave::{screen_staff_px, Engraver, PageSpec};
use scoretk_render_raster::display::draw_display_list;
use scoretk_render_raster::{Gray8, Point};

use crate::reader;

/// A document opened for one screen size.
///
/// Only the engraver's compact atoms and the single page buffer survive
/// [`Document::open`]: the source bytes and the pivot `Score` are freed
/// inside that function before the handle is returned.
pub struct Document {
    engraver: Engraver,
    /// Allocated once per document, reused by every page render.
    buffer: Vec<u8>,
    width: usize,
    height: usize,
    staff_px: i32,
}

impl Document {
    /// Reads a score and prepares it for a `width × height` screen.
    pub fn open(path: &str, width: u32, height: u32) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("invalid page size: width and height must be positive".to_owned());
        }
        let bytes = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| "page size overflow".to_owned())?;
        let (engraver, staff_px) = {
            // Both the input bytes and the `Score` die at the end of this
            // block; `Engraver` keeps atoms, breaks, staves and credits only.
            let score = reader::read_score(path)?;
            let staff_px = screen_staff_px(width, height);
            let engraver = Engraver::new(&score, PageSpec::for_screen(width, height))
                .map_err(|error| format!("cannot engrave {path}: {error}"))?;
            (engraver, staff_px)
        };
        Ok(Self {
            engraver,
            buffer: vec![255; bytes],
            width: width as usize,
            height: height as usize,
            staff_px,
        })
    }

    pub fn page_count(&self) -> usize {
        self.engraver.page_count()
    }

    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Draws a 0-based page into the reusable buffer and returns its bytes.
    ///
    /// The display list is local to this call: no page is cached between
    /// renders, so memory does not grow with the number of pages.
    pub fn page(&mut self, page: usize) -> Result<&[u8], String> {
        if page >= self.page_count() {
            return Err(format!(
                "page {page} out of range (document has {} pages)",
                self.page_count()
            ));
        }
        let list = self.engraver.page(page);
        // The protocol has no padding, so the row pitch is exactly `width`.
        let Some(mut gray) =
            Gray8::new_with_stride(&mut self.buffer, self.width, self.height, self.width)
        else {
            return Err("invalid page buffer".to_owned());
        };
        draw_display_list(&list, &mut gray, Point::new(0, 0), self.staff_px);
        Ok(&self.buffer)
    }
}

/// One open + render of up to `pages` pages + close, for the heap example.
pub fn cycle(path: &str, width: u32, height: u32, pages: usize) -> Result<(), String> {
    let mut document = Document::open(path, width, height)?;
    let drawn = pages.min(document.page_count());
    for page in 0..drawn {
        document.page(page)?;
    }
    Ok(())
}
