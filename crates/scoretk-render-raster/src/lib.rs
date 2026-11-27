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

//! A minimal, portable scanline coverage rasteriser for engraved scores.
//!
//! `no_std` + `alloc`, no C dependency: the caller owns the 8-bit grey buffer.
//! Geometry is fixed point (1/256 of a pixel), so the same code gives the same
//! bytes on the host, on wasm and on the Kobo's Cortex-A8.

#![no_std]

pub mod display;
pub mod stride;

extern crate alloc;

use alloc::vec::Vec;

/// Fixed-point unit: one pixel is `ONE` units.
pub const SHIFT: u32 = 8;
pub const ONE: i32 = 1 << SHIFT;
const SUBROWS: i32 = 4;
const SUBROW_STEP: i32 = ONE / SUBROWS;
const SUBROW_CENTER: i32 = SUBROW_STEP / 2;

/// A point in fixed-point pixel coordinates (1/256 of a pixel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub const fn from_pixels(x: i32, y: i32) -> Self {
        Self {
            x: x * ONE,
            y: y * ONE,
        }
    }
}

/// One edge of a filled outline.
#[derive(Clone, Copy, Debug)]
pub enum Edge {
    Line(Point, Point),
    Quad(Point, Point, Point),
}

/// A closed outline built from lines and quadratic Bézier segments.
#[derive(Clone, Debug, Default)]
pub struct Path {
    edges: Vec<Edge>,
    start: Option<Point>,
    last: Option<Point>,
}

impl Path {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn move_to(&mut self, p: Point) {
        self.close();
        self.start = Some(p);
        self.last = Some(p);
    }

    pub fn line_to(&mut self, p: Point) {
        if let Some(from) = self.last {
            self.edges.push(Edge::Line(from, p));
            self.last = Some(p);
        }
    }

    pub fn quad_to(&mut self, control: Point, p: Point) {
        if let Some(from) = self.last {
            self.edges.push(Edge::Quad(from, control, p));
            self.last = Some(p);
        }
    }

    /// Closes the current contour with a straight edge back to its start.
    pub fn close(&mut self) {
        if let (Some(start), Some(last)) = (self.start, self.last) {
            if start != last {
                self.edges.push(Edge::Line(last, start));
            }
            self.last = Some(start);
        }
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Bounding box in fixed-point units, or `None` for an empty path.
    pub fn bounds(&self) -> Option<(i32, i32, i32, i32)> {
        let mut it = self.edges.iter();
        let first = it.next()?;
        let (p0, p1) = endpoints(first);
        let mut min_x = p0.x.min(p1.x);
        let mut max_x = p0.x.max(p1.x);
        let mut min_y = p0.y.min(p1.y);
        let mut max_y = p0.y.max(p1.y);
        for edge in it {
            for p in control_points(edge) {
                min_x = min_x.min(p.x);
                max_x = max_x.max(p.x);
                min_y = min_y.min(p.y);
                max_y = max_y.max(p.y);
            }
        }
        Some((min_x, min_y, max_x, max_y))
    }
}

fn endpoints(edge: &Edge) -> (Point, Point) {
    match *edge {
        Edge::Line(a, b) => (a, b),
        Edge::Quad(a, _, b) => (a, b),
    }
}

fn control_points(edge: &Edge) -> [Point; 3] {
    match *edge {
        Edge::Line(a, b) => [a, b, b],
        Edge::Quad(a, c, b) => [a, c, b],
    }
}

/// Caller-owned 8-bit grey framebuffer. 255 is white, 0 is black.
pub struct Gray8<'a> {
    buf: &'a mut [u8],
    width: usize,
    height: usize,
    /// Distance in bytes between two rows. `new` sets it to `width`; the FFI
    /// sets it from KOReader's `BlitBuffer8` (`new_with_stride`).
    stride: usize,
}

impl<'a> Gray8<'a> {
    pub fn new(buf: &'a mut [u8], width: usize, height: usize) -> Option<Self> {
        if width == 0 || height == 0 || buf.len() < width.saturating_mul(height) {
            return None;
        }
        Some(Self {
            buf,
            width,
            height,
            stride: width,
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// The first `width * height` backing bytes. For a strided buffer this is
    /// *not* a tightly packed image; use [`Self::stride`] to walk its rows.
    pub fn as_slice(&self) -> &[u8] {
        &self.buf[..self.width * self.height]
    }

    pub fn clear(&mut self, grey: u8) {
        for row in 0..self.height {
            let start = row * self.stride;
            self.buf[start..start + self.width].fill(grey);
        }
    }

    /// Fills a pixel-snapped integer rectangle with `ink` darkness.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, ink: u8) {
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(self.width as i32);
        let y1 = (y + h).min(self.height as i32);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for row in y0..y1 {
            let start = (row as usize) * self.stride + x0 as usize;
            let end = (row as usize) * self.stride + x1 as usize;
            for px in &mut self.buf[start..end] {
                *px = blend(*px, ink, 255);
            }
        }
    }

    /// Fills an outline with analytic quadratic crossings and 4 subrows of
    /// vertical coverage. Horizontal coverage is exact to 1/256 of a pixel.
    pub fn fill_path(&mut self, path: &Path, ink: u8) {
        let Some((min_x, min_y, max_x, max_y)) = path.bounds() else {
            return;
        };
        let px0 = (min_x / ONE).max(0);
        let py0 = (min_y / ONE).max(0);
        let px1 = ((max_x + ONE - 1) / ONE).min(self.width as i32);
        let py1 = ((max_y + ONE - 1) / ONE).min(self.height as i32);
        if px0 >= px1 || py0 >= py1 {
            return;
        }
        let mut coverage = alloc::vec![0u16; (px1 - px0) as usize];
        let mut crossings: Vec<i32> = Vec::new();
        for py in py0..py1 {
            coverage.fill(0);
            for sub in 0..SUBROWS {
                let y = py * ONE + sub * SUBROW_STEP + SUBROW_CENTER;
                crossings.clear();
                for edge in &path.edges {
                    match *edge {
                        Edge::Line(a, b) => line_crossing(a, b, y, &mut crossings),
                        Edge::Quad(a, c, b) => quad_crossings(a, c, b, y, &mut crossings),
                    }
                }
                crossings.sort_unstable();
                for pair in crossings.as_chunks::<2>().0 {
                    let (from, to) = (pair[0], pair[1]);
                    let first = (from / ONE).max(px0);
                    let last = ((to + ONE - 1) / ONE).min(px1);
                    for column in first..last {
                        let left = from.max(column * ONE);
                        let right = to.min((column + 1) * ONE);
                        if right > left {
                            coverage[(column - px0) as usize] += (right - left) as u16;
                        }
                    }
                }
            }
            let row = (py as usize) * self.stride;
            for (offset, units) in coverage.iter().enumerate() {
                if *units == 0 {
                    continue;
                }
                let alpha = ((*units as u32) * 255 / (SUBROWS as u32 * ONE as u32)).min(255);
                let idx = row + px0 as usize + offset;
                self.buf[idx] = blend(self.buf[idx], ink, alpha);
            }
        }
    }
}

fn blend(dst: u8, ink: u8, alpha: u32) -> u8 {
    let value = (dst as u32) * (255 - alpha) + (ink as u32) * alpha + 127;
    (value / 255) as u8
}

fn line_crossing(a: Point, b: Point, y: i32, out: &mut Vec<i32>) {
    if y < a.y.min(b.y) || y >= a.y.max(b.y) {
        return;
    }
    let dy = (b.y - a.y) as i64;
    let x = (a.x as i64 * dy + (y - a.y) as i64 * (b.x - a.x) as i64) / dy;
    out.push(x as i32);
}

fn quad_crossings(a: Point, c: Point, b: Point, y: i32, out: &mut Vec<i32>) {
    let ay = a.y as i64;
    let cy = c.y as i64;
    let by = b.y as i64;
    let qa = ay - 2 * cy + by;
    let qb = 2 * (cy - ay);
    let qc = ay - y as i64;
    if qa == 0 {
        if qb != 0 {
            let t = -qc * ONE as i64 / qb;
            if (0..ONE as i64).contains(&t) {
                out.push(quad_x(a, c, b, t as i32));
            }
        }
        return;
    }
    let disc = qb * qb - 4 * qa * qc;
    if disc < 0 {
        return;
    }
    let root = isqrt(disc as u64) as i64;
    let den = 2 * qa;
    let mut ts = [
        (-qb - root) * ONE as i64 / den,
        (-qb + root) * ONE as i64 / den,
    ];
    if ts[0] > ts[1] {
        ts.swap(0, 1);
    }
    if ts[0] == ts[1] {
        // Integer `isqrt` can truncate two very close roots to the same
        // fixed-point t near a tangency. The quadratic really crosses the
        // scanline twice there (`disc > 0`), and dropping both crossings
        // makes the crossing count odd: the outer and inner contours of a
        // glyph like `o` are then paired across the hole, which draws a
        // diagonal slash through it ("Recreati n" in Edwin titles).
        if disc > 0 {
            let x = quad_x(a, c, b, ts[0] as i32);
            out.push(x);
            out.push(x);
        } else if ts[0] <= 1 || ts[0] >= i64::from(ONE) - 1 {
            // A shared vertex of two consecutive edges counts once.
            out.push(quad_x(a, c, b, ts[0] as i32));
        }
        return;
    }
    for t in ts {
        if (0..ONE as i64).contains(&t) {
            out.push(quad_x(a, c, b, t as i32));
        }
    }
}

/// De Casteljau evaluation of the x coordinate, `t` in 0..ONE.
fn quad_x(a: Point, c: Point, b: Point, t: i32) -> i32 {
    let ac = a.x + (((c.x - a.x) * t) >> SHIFT);
    let cb = c.x + (((b.x - c.x) * t) >> SHIFT);
    ac + (((cb - ac) * t) >> SHIFT)
}

/// Integer square root, no `libm` and no floating point.
fn isqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let bits = 64 - n.leading_zeros();
    let mut x = 1u64 << (bits + 1).div_ceil(2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_is_pixel_snapped() {
        let mut buf = [255u8; 16];
        let mut fb = Gray8::new(&mut buf, 4, 4).unwrap();
        fb.fill_rect(1, 1, 2, 1, 0);
        assert_eq!(fb.as_slice()[4], 255);
        assert_eq!(fb.as_slice()[5], 0);
        assert_eq!(fb.as_slice()[6], 0);
        assert_eq!(fb.as_slice()[7], 255);
    }

    #[test]
    fn quad_outline_covers_its_interior() {
        let mut buf = [255u8; 64 * 64];
        let mut fb = Gray8::new(&mut buf, 64, 64).unwrap();
        let mut path = Path::new();
        path.move_to(Point::from_pixels(10, 20));
        path.quad_to(Point::new(20 * ONE, 10 * ONE), Point::from_pixels(30, 20));
        path.quad_to(Point::new(40 * ONE, 30 * ONE), Point::from_pixels(30, 40));
        path.quad_to(Point::new(20 * ONE, 50 * ONE), Point::from_pixels(10, 40));
        path.quad_to(Point::new(0, 30 * ONE), Point::from_pixels(10, 20));
        path.close();
        fb.fill_path(&path, 0);
        assert_eq!(buf[20 * 64 + 20], 0, "centre of the outline is black");
        assert_eq!(buf[10 * 64 + 10], 255, "outside the outline is white");
    }
}
