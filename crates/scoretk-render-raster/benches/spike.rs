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

//! P0 spike: one dense synthetic page drawn by the in-house scanline
//! rasteriser and by tiny-skia. `cargo bench -p scoretk-render-raster`.
//! The benchmark is host-only: it measures time and peak heap, not quality.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use scoretk_render_raster::{Gray8, Path, Point, ONE};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

const WIDTH: usize = 800;
const HEIGHT: usize = 600;
const SYSTEMS: usize = 5;
const STAVES: usize = 2;
const GLYPHS_PER_STAFF: usize = 60;

struct CountingAllocator;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            let now = CURRENT.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new = System.realloc(ptr, layout, new_size);
        if !new.is_null() {
            let now = CURRENT.fetch_add(new_size, Ordering::Relaxed) + new_size;
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        new
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

struct Scene {
    heads: Vec<(i32, i32)>,
    lines: Vec<Rect>,
}

impl Scene {
    fn new() -> Self {
        let mut heads = Vec::new();
        let mut lines = Vec::new();
        let system_height = HEIGHT as i32 / SYSTEMS as i32;
        for system in 0..SYSTEMS {
            let top = system as i32 * system_height + 20;
            for staff in 0..STAVES {
                let staff_top = top + staff as i32 * 46;
                for line in 0..5 {
                    lines.push(Rect {
                        x: 40,
                        y: staff_top + line * 4,
                        w: 720,
                        h: 1,
                    });
                }
                for i in 0..GLYPHS_PER_STAFF {
                    let x = 46 + ((i % 30) * 23) as i32;
                    let y = staff_top + 8 + ((i / 30) * 18) as i32;
                    heads.push((x, y));
                    lines.push(Rect {
                        x: x + 5,
                        y: y - 16,
                        w: 1,
                        h: 22,
                    });
                }
                lines.push(Rect {
                    x: 758,
                    y: staff_top,
                    w: 1,
                    h: 17,
                });
            }
        }
        Self { heads, lines }
    }

    fn glyph_count(&self) -> usize {
        self.heads.len()
    }

    fn draw_ours(&self, fb: &mut Gray8<'_>) {
        for rect in &self.lines {
            fb.fill_rect(rect.x, rect.y, rect.w, rect.h, 0);
        }
        for &(x, y) in &self.heads {
            let mut path = notehead(x, y);
            fb.fill_path(&path, 0);
            path.close();
        }
    }

    fn draw_tiny_skia(&self, pixmap: &mut Pixmap) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(0, 0, 0, 255);
        paint.anti_alias = true;
        for rect in &self.lines {
            if let Some(r) = tiny_skia::Rect::from_xywh(
                rect.x as f32,
                rect.y as f32,
                rect.w as f32,
                rect.h as f32,
            ) {
                pixmap.fill_rect(r, &paint, Transform::identity(), None);
            }
        }
        for &(x, y) in &self.heads {
            let mut b = PathBuilder::new();
            b.move_to((x - 6) as f32, y as f32);
            b.quad_to((x - 6) as f32, (y - 4) as f32, x as f32, (y - 4) as f32);
            b.quad_to((x + 6) as f32, (y - 4) as f32, (x + 6) as f32, y as f32);
            b.quad_to((x + 6) as f32, (y + 4) as f32, x as f32, (y + 4) as f32);
            b.quad_to((x - 6) as f32, (y + 4) as f32, (x - 6) as f32, y as f32);
            b.close();
            if let Some(path) = b.finish() {
                pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::EvenOdd,
                    Transform::identity(),
                    None,
                );
            }
        }
    }
}

/// Hardcoded quadratic notehead outline: four quadratic edges per glyph.
fn notehead(cx: i32, cy: i32) -> Path {
    let p = |x: i32, y: i32| Point {
        x: (cx + x) * ONE,
        y: (cy + y) * ONE,
    };
    let mut path = Path::new();
    path.move_to(p(-6, 0));
    path.quad_to(p(-6, -4), p(0, -4));
    path.quad_to(p(6, -4), p(6, 0));
    path.quad_to(p(6, 4), p(0, 4));
    path.quad_to(p(-6, 4), p(-6, 0));
    path.close();
    path
}

fn measure<F: FnMut()>(iterations: usize, mut draw: F) -> (f64, usize) {
    for _ in 0..2 {
        draw();
    }
    let mut times = Vec::new();
    let mut peak = 0usize;
    for _ in 0..iterations {
        let before = CURRENT.load(Ordering::Relaxed);
        PEAK.store(before, Ordering::Relaxed);
        let started = Instant::now();
        draw();
        times.push(started.elapsed().as_secs_f64() * 1000.0);
        peak = peak.max(PEAK.load(Ordering::Relaxed).saturating_sub(before));
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (times[times.len() / 2], peak)
}

fn main() {
    let scene = Scene::new();
    let iterations = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    println!(
        "scene: {}x{}, {} systems x {} staves, {} quadratic glyphs, {} rects",
        WIDTH,
        HEIGHT,
        SYSTEMS,
        STAVES,
        scene.glyph_count(),
        scene.lines.len()
    );

    let (ours_ms, ours_heap) = measure(iterations, || {
        let mut buf = vec![255u8; WIDTH * HEIGHT];
        let mut fb = Gray8::new(&mut buf, WIDTH, HEIGHT).unwrap();
        scene.draw_ours(&mut fb);
        std::hint::black_box(fb.as_slice()[0]);
    });
    println!(
        "scanline: median {:.3} ms/page, peak heap {} bytes",
        ours_ms, ours_heap
    );

    let (tiny_ms, tiny_heap) = measure(iterations, || {
        let mut pixmap = Pixmap::new(WIDTH as u32, HEIGHT as u32).unwrap();
        pixmap.fill(tiny_skia::Color::WHITE);
        scene.draw_tiny_skia(&mut pixmap);
        std::hint::black_box(pixmap.data()[0]);
    });
    println!(
        "tiny-skia: median {:.3} ms/page, peak heap {} bytes",
        tiny_ms, tiny_heap
    );
}
