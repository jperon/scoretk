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

//! Peak-heap measurement of one open + three pages + close cycle.
//!
//! ```sh
//! cargo run -p scoretk-engrave-serve --features memory-report \
//!   --example heap_cycle -- FILE [WIDTH HEIGHT PAGES]
//! ```
//!
//! dhat writes `dhat-heap.json` into the current directory: run from a
//! throwaway directory. The allocator is feature-gated, so the delivered
//! binary never links dhat.

use scoretk_engrave_serve::cycle;

#[cfg(feature = "memory-report")]
#[global_allocator]
static ALLOCATOR: dhat::Alloc = dhat::Alloc;

fn main() {
    #[cfg(feature = "memory-report")]
    let _profiler = dhat::Profiler::new_heap();

    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: heap_cycle FILE [WIDTH HEIGHT PAGES]");
    let width: u32 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(600);
    let height: u32 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(800);
    let pages: usize = args.next().and_then(|value| value.parse().ok()).unwrap_or(3);

    cycle(&path, width, height, pages).expect("open/render/close cycle failed");
    println!("opened, rendered {pages} pages, closed");
}
