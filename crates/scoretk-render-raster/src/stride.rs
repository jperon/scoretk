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

//! Strided framebuffers for hosts such as KOReader's `BlitBuffer8`.
//!
//! Additive API: [`Gray8::new`] still produces a tightly packed image, so
//! every existing render is unchanged. The new constructor only adds a row
//! pitch; all draw calls keep using the same `width`/`height` coordinates and
//! leave the padding bytes untouched.

use super::Gray8;

impl<'a> Gray8<'a> {
    /// Wraps a caller-owned buffer with `stride` bytes per row.
    ///
    /// Returns `None` for a zero-sized image, a stride smaller than the width,
    /// or a buffer too small for `stride * height` bytes.
    pub fn new_with_stride(
        buf: &'a mut [u8],
        width: usize,
        height: usize,
        stride: usize,
    ) -> Option<Self> {
        if width == 0 || height == 0 || stride < width {
            return None;
        }
        if buf.len() < stride.saturating_mul(height) {
            return None;
        }
        Some(Self {
            buf,
            width,
            height,
            stride,
        })
    }

    /// Bytes between the start of two consecutive rows.
    pub fn stride(&self) -> usize {
        self.stride
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{display::draw_display_list, Point};
    use scoretk_engrave::display::{DisplayItem, DisplayList, ItemKind};

    #[test]
    fn drawing_a_strided_buffer_leaves_the_padding_untouched() {
        let (width, height, stride) = (8usize, 4usize, 12usize);
        let mut pixels = [0xAAu8; 12 * 4];
        let mut list = DisplayList::new(0);
        list.push(DisplayItem::rect(ItemKind::StaffLine, 0, 0, 8 * 1024, 1024));
        {
            let mut buf = Gray8::new_with_stride(&mut pixels, width, height, stride).unwrap();
            assert_eq!(buf.stride(), stride);
            draw_display_list(&list, &mut buf, Point::new(0, 0), 8);
        }
        for row in 0..height {
            assert!(pixels[row * stride..row * stride + width]
                .iter()
                .any(|value| *value < 128));
            assert!(pixels[row * stride + width..(row + 1) * stride]
                .iter()
                .all(|value| *value == 0xAA));
        }
    }

    #[test]
    fn rejects_an_invalid_stride_or_buffer() {
        let mut pixels = [255u8; 16];
        assert!(Gray8::new_with_stride(&mut pixels, 8, 2, 4).is_none());
        assert!(Gray8::new_with_stride(&mut pixels, 8, 3, 8).is_none());
        assert!(Gray8::new_with_stride(&mut pixels, 0, 2, 8).is_none());
    }
}
