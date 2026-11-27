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

//! C ABI of the engraver for the KOReader plugin `score.koplugin`.
//!
//! The header in `include/scoretk_engrave.h` is written to be pasted directly
//! into LuaJIT's `ffi.cdef`. The library holds only the `Engraver`: source
//! bytes and the pivot `Score` are freed at the end of `st_open`, and each
//! page's display list is built and dropped inside `st_render_page`.

/// Static version string, NUL-terminated for `ffi.string()`.
static VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();

/// Returns the library version as a static NUL-terminated C string.
#[no_mangle]
pub extern "C" fn st_version() -> *const core::ffi::c_char {
    VERSION.as_ptr().cast()
}
