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

//! Persistent engraving helper for the KOReader plugin `score.koplugin`.
//!
//! The binary reads line commands on stdin and writes a response, flushed,
//! on stdout. It is a **process**, not a shared library: the Kobo Mini runs
//! glibc 2.11.1 / kernel 2.6.35, which cannot load a modern Rust `cdylib`.
//! A static musl executable works and returns all layout memory to the
//! system when the plugin closes it.
//!
//! A document owns only an `Engraver` plus one reusable page buffer: source
//! bytes and the pivot `Score` are dropped as soon as the engraver is built,
//! and a page's display list is built and dropped inside each `page` call.
//!
//! The protocol also has `set scale <f>` and `set text <f>` (default 1.0,
//! bounded above to 3.0) to size notes and text of the next documents;
//! `set scale` accepts down to 0.07, `set text` down to 0.1.

mod document;
mod reader;
mod server;

pub use document::{cycle, Document};
pub use server::Server;
