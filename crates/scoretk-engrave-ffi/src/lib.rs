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
//! into LuaJIT's `ffi.cdef`. Every entry point catches unwinding: no panic may
//! cross the FFI boundary. A handle owns only the `Engraver`: the source bytes
//! and the pivot `Score` are dropped at the end of [`st_open`], and each page's
//! display list is built and dropped inside [`st_render_page`]. Nothing is
//! cached between pages, so memory does not grow with the number of pages.

mod reader;

use std::ffi::{c_char, c_int, CStr};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

use scoretk_engrave::{screen_staff_px, Engraver, PageSpec};
use scoretk_render_raster::display::draw_display_list;
use scoretk_render_raster::{Gray8, Point};

/// Opaque handle returned by [`st_open`], owned by exactly one caller.
pub struct StDoc {
    engraver: Engraver,
    /// Whole pixels per staff space, used by the raster backend.
    staff_px: i32,
}

/// Error codes of `st_render_page` (documented in the header).
const ERR_NULL_DOC: c_int = -1;
const ERR_PAGE: c_int = -2;
const ERR_BUFFER: c_int = -3;
const ERR_PANIC: c_int = -4;

/// Static version string, NUL-terminated for `ffi.string()`.
static VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();

/// Writes a NUL-terminated UTF-8 message, truncated on a character boundary.
fn set_error(err: *mut c_char, err_len: usize, message: &str) {
    if err.is_null() || err_len == 0 {
        return;
    }
    let limit = err_len - 1;
    let mut end = message.len().min(limit);
    while end > 0 && !message.is_char_boundary(end) {
        end -= 1;
    }
    // SAFETY: the caller supplies a writable buffer of at least `err_len`
    // bytes; `end < err_len`, so the message and its NUL both fit.
    unsafe {
        ptr::copy_nonoverlapping(message.as_ptr().cast::<u8>(), err.cast::<u8>(), end);
        *err.add(end) = 0;
    }
}

fn open_impl(path: *const c_char, width_px: c_int, height_px: c_int) -> Result<Box<StDoc>, String> {
    if path.is_null() {
        return Err("null path".to_owned());
    }
    // SAFETY: a non-null `path` is a NUL-terminated C string provided by the
    // caller and only read here.
    let path = unsafe { CStr::from_ptr(path) }
        .to_str()
        .map_err(|error| format!("path is not UTF-8: {error}"))?;
    if width_px <= 0 || height_px <= 0 {
        return Err(format!(
            "invalid screen size {width_px} x {height_px}: both dimensions must be positive"
        ));
    }
    let (width, height) = (width_px as u32, height_px as u32);
    let (engraver, staff_px) = {
        // Source bytes and the pivot model die here, before the handle is
        // returned: the engraver owns compact atoms, not a score object graph.
        let score = reader::read_score(path)?;
        let staff_px = screen_staff_px(width, height);
        let engraver = Engraver::new(&score, PageSpec::for_screen(width, height))
            .map_err(|error| format!("cannot engrave {path}: {error}"))?;
        (engraver, staff_px)
    };
    Ok(Box::new(StDoc {
        engraver,
        staff_px,
    }))
}

/// Reads a score and engraves it for a `width_px × height_px` screen.
///
/// Returns NULL and writes a truncated UTF-8 message into `err` on failure.
/// The handle must be released with [`st_close`] once, by the same thread.
#[no_mangle]
pub extern "C" fn st_open(
    path: *const c_char,
    width_px: c_int,
    height_px: c_int,
    err: *mut c_char,
    err_len: usize,
) -> *mut StDoc {
    match catch_unwind(AssertUnwindSafe(|| open_impl(path, width_px, height_px))) {
        Ok(Ok(doc)) => Box::into_raw(doc),
        Ok(Err(message)) => {
            set_error(err, err_len, &message);
            ptr::null_mut()
        }
        Err(_) => {
            set_error(err, err_len, "internal error: panic in st_open");
            ptr::null_mut()
        }
    }
}

fn page_count_impl(doc: *const StDoc) -> c_int {
    // SAFETY: a non-null document is a live handle owned by the caller.
    let Some(doc) = (unsafe { doc.as_ref() }) else {
        return ERR_NULL_DOC;
    };
    c_int::try_from(doc.engraver.page_count()).unwrap_or(c_int::MAX)
}

/// Number of engraved pages, or -1 for a NULL handle.
#[no_mangle]
pub extern "C" fn st_page_count(doc: *const StDoc) -> c_int {
    match catch_unwind(AssertUnwindSafe(|| page_count_impl(doc))) {
        Ok(count) => count,
        Err(_) => ERR_PANIC,
    }
}

fn render_impl(
    doc: *mut StDoc,
    page: c_int,
    buf: *mut u8,
    width: c_int,
    height: c_int,
    stride: c_int,
) -> c_int {
    // SAFETY: a non-null document is a live handle; the caller must not call
    // `st_close` concurrently (the plugin is single-threaded).
    let Some(doc) = (unsafe { doc.as_mut() }) else {
        return ERR_NULL_DOC;
    };
    if page < 0 || page as usize >= doc.engraver.page_count() {
        return ERR_PAGE;
    }
    if width <= 0 || height <= 0 || stride < width || buf.is_null() {
        return ERR_BUFFER;
    }
    let (width, height, stride) = (width as usize, height as usize, stride as usize);
    let Some(bytes) = stride.checked_mul(height) else {
        return ERR_BUFFER;
    };
    // SAFETY: `buf` is a writable buffer of at least `stride * height` bytes
    // supplied by the caller (a KOReader BlitBuffer8's data pointer).
    let pixels = unsafe { std::slice::from_raw_parts_mut(buf, bytes) };
    let list = doc.engraver.page(page as usize);
    // SAFETY: `pixels` has at least `stride * height` bytes, stride >= width.
    let Some(mut gray) = Gray8::new_with_stride(pixels, width, height, stride) else {
        return ERR_BUFFER;
    };
    draw_display_list(&list, &mut gray, Point::new(0, 0), doc.staff_px);
    0
}

/// Clears the caller's 8-bit grey buffer to white and draws page `page`.
///
/// `stride` is the row pitch in bytes; padding bytes are left untouched.
/// Returns 0 on success, -1 for a NULL handle, -2 for an out-of-range page,
/// -3 for an invalid buffer or geometry, -4 for an internal error.
#[no_mangle]
pub extern "C" fn st_render_page(
    doc: *mut StDoc,
    page: c_int,
    buf: *mut u8,
    width: c_int,
    height: c_int,
    stride: c_int,
) -> c_int {
    match catch_unwind(AssertUnwindSafe(|| {
        render_impl(doc, page, buf, width, height, stride)
    })) {
        Ok(code) => code,
        Err(_) => ERR_PANIC,
    }
}

/// Releases a handle. NULL is accepted; each handle must be released once.
// SAFETY contract of a C ABI: callers pass a pointer returned by `st_open`;
// making this `unsafe fn` would change the exported symbol's ABI.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn st_close(doc: *mut StDoc) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !doc.is_null() {
            // SAFETY: the handle was returned by `st_open` and is not used
            // after this call; `Box::from_raw` restores the original allocation.
            unsafe { drop(Box::from_raw(doc)) }
        }
    }));
}

/// Returns the library version as a static NUL-terminated C string.
#[no_mangle]
pub extern "C" fn st_version() -> *const c_char {
    VERSION.as_ptr().cast()
}
