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

//! Measures one open + three renders + close cycle with dhat.
//!
//! ```sh
//! cargo run -p scoretk-engrave-ffi --features memory-report \
//!   --example heap_cycle -- FILE [WIDTH HEIGHT PAGES]
//! ```
//!
//! dhat writes `dhat-heap.json` into the current directory: run from a
//! throwaway directory. The allocator is feature-gated, so a delivered `.so`
//! never links dhat.

use std::ffi::{CStr, CString};

use scoretk_engrave_ffi::{st_close, st_open, st_page_count, st_render_page};

#[cfg(feature = "memory-report")]
#[global_allocator]
static ALLOCATOR: dhat::Alloc = dhat::Alloc;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: heap_cycle FILE [WIDTH HEIGHT PAGES]");
    let width: i32 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(600);
    let height: i32 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(800);
    let wanted: i32 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3);

    // KOReader already owns its BlitBuffer8: allocate the caller's framebuffer
    // and the C string *before* starting the profiler, so dhat measures only
    // the library's allocations, as in the CLI SVG baseline of §20.2.
    let mut pixels = vec![255u8; (width * height) as usize];
    let path = CString::new(path).expect("path contains a NUL byte");

    #[cfg(feature = "memory-report")]
    let _profiler = dhat::Profiler::new_heap();

    let mut error = [0i8; 256];
    let doc = st_open(
        path.as_ptr(),
        width,
        height,
        error.as_mut_ptr(),
        error.len(),
    );
    assert!(
        !doc.is_null(),
        "st_open failed: {}",
        // SAFETY: the error buffer is NUL-terminated on failure.
        unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy()
    );
    let pages = st_page_count(doc);
    assert!(pages > 0);

    let drawn = wanted.min(pages);
    for page in 0..drawn {
        let code = st_render_page(doc, page, pixels.as_mut_ptr(), width, height, width);
        assert_eq!(code, 0, "st_render_page({page}) returned {code}");
    }
    st_close(doc);
    println!("opened, rendered {drawn} pages, closed");
}
