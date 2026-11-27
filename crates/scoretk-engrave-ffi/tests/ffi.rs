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

//! End-to-end tests of the exported C ABI against one file of every format
//! present in `tests/corpus`.
//!
//! The exported functions are safe Rust functions (they catch unwinding), but
//! each call below still has the C ABI's ownership contract described in the
//! header: one `st_close` per non-NULL handle, no concurrent use.

use std::ffi::{CStr, CString};
use std::path::PathBuf;

use scoretk_engrave_ffi::{st_close, st_open, st_page_count, st_render_page, st_version};

const WIDTH: i32 = 600;
const HEIGHT: i32 = 800;

fn corpus(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(relative)
}

/// Opens a score, checks it has pages and draws page 0 into a strided buffer.
fn open_and_render(relative: &str) {
    let path = corpus(relative);
    let path = CString::new(path.to_str().unwrap().as_bytes()).unwrap();
    let mut error = [0i8; 256];
    let doc = st_open(
        path.as_ptr(),
        WIDTH,
        HEIGHT,
        error.as_mut_ptr(),
        error.len(),
    );
    assert!(!doc.is_null(), "{}", message(&error));

    let pages = st_page_count(doc);
    assert!(pages > 0, "{relative}: page_count = {pages}");
    assert_eq!(
        st_page_count(std::ptr::null()),
        -1,
        "a NULL handle has no pages"
    );

    // KOReader's BlitBuffer8 may pad each row: use a stride larger than the
    // width and verify that the padding is left untouched.
    let stride = WIDTH + 16;
    let mut pixels = vec![0xAAu8; (stride * HEIGHT) as usize];
    let code = st_render_page(doc, 0, pixels.as_mut_ptr(), WIDTH, HEIGHT, stride);
    assert_eq!(code, 0, "{relative}: st_render_page returned {code}");
    assert!(
        pixels
            .chunks_exact(stride as usize)
            .any(|row| row[..WIDTH as usize].iter().any(|value| *value < 128)),
        "{relative}: page 0 has no dark pixel"
    );
    assert!(
        pixels
            .chunks_exact(stride as usize)
            .all(|row| row[WIDTH as usize..].iter().all(|value| *value == 0xAA)),
        "{relative}: the row padding was overwritten"
    );

    assert!(st_render_page(doc, pages, pixels.as_mut_ptr(), WIDTH, HEIGHT, stride) < 0);
    assert!(st_render_page(doc, -1, pixels.as_mut_ptr(), WIDTH, HEIGHT, stride) < 0);
    assert!(st_render_page(doc, 0, std::ptr::null_mut(), WIDTH, HEIGHT, stride) < 0);
    assert!(st_render_page(doc, 0, pixels.as_mut_ptr(), WIDTH, HEIGHT, WIDTH - 1) < 0);

    st_close(doc);
}

fn message(error: &[i8; 256]) -> String {
    // SAFETY: the buffer is NUL-terminated by `st_open` on failure.
    unsafe { CStr::from_ptr(error.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

#[test]
fn opens_every_corpus_format() {
    // One file per extension available under tests/corpus. MuseScore,
    // MusicXML, LilyPond, MEI, ABC, MIDI and GABC.
    for file in [
        "v3/moonlight.mscx",
        "v3/lyricsline01.ref.musicxml",
        "ly/v4_linked-ties.ly",
        "v4/repeat68.ref.mei",
        "abc/v3_measure-4.abc",
        "midi/v4/dynamics.ref.mid",
        "gabc/gregorio_pitches2.gabc",
    ] {
        open_and_render(file);
    }
}

#[test]
fn missing_file_returns_null_and_a_message() {
    let path = CString::new("/nonexistent/scoretk-ffi-test.mscx").unwrap();
    let mut error = [0i8; 256];
    let doc = st_open(
        path.as_ptr(),
        WIDTH,
        HEIGHT,
        error.as_mut_ptr(),
        error.len(),
    );
    assert!(doc.is_null());
    let message = message(&error);
    assert!(message.contains("cannot read"), "{message}");
}

#[test]
fn invalid_screen_size_returns_null_and_a_message() {
    let path = CString::new(corpus("v3/moonlight.mscx").to_str().unwrap()).unwrap();
    let mut error = [0i8; 256];
    let doc = st_open(path.as_ptr(), 0, -1, error.as_mut_ptr(), error.len());
    assert!(doc.is_null());
    assert!(message(&error).contains("invalid screen size"));
}

#[test]
fn error_message_is_truncated_on_a_utf8_boundary() {
    let path = CString::new("/nonexistent/éééééééééééééééééééééééééééééééééééééééé.mscx").unwrap();
    let mut error = [0i8; 8];
    let doc = st_open(
        path.as_ptr(),
        WIDTH,
        HEIGHT,
        error.as_mut_ptr(),
        error.len(),
    );
    assert!(doc.is_null());
    assert_eq!(error[7], 0, "the message is NUL-terminated");
    assert!(std::str::from_utf8(unsafe {
        std::slice::from_raw_parts(error.as_ptr().cast::<u8>(), 7)
    })
    .is_ok());
}

#[test]
fn close_accepts_null_and_version_is_static() {
    st_close(std::ptr::null_mut());
    // SAFETY: `st_version` returns a static NUL-terminated string.
    let version = unsafe { CStr::from_ptr(st_version()) }
        .to_str()
        .unwrap()
        .to_owned();
    assert_eq!(version, env!("CARGO_PKG_VERSION"));
}
