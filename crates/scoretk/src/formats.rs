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

//! Enumeration of the formats available in the current build.
//!
//! [`Format::ALL`](crate::Format::ALL) keeps every format known to the facade,
//! including formats whose features are all disabled. [`formats`] lists only
//! the formats with at least one compiled direction, so consumers (the wasm
//! `formats()` and the CLI `--list-formats`) never advertise a format that
//! would always fail.

use crate::format::Format;

/// Capabilities of one format in the current build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatInfo {
    /// The format being described.
    pub format: Format,
    /// Whether `read` is wired in this build.
    pub read: bool,
    /// Whether `write` is wired in this build.
    pub write: bool,
}

impl FormatInfo {
    /// Canonical name used by `--from`/`--to`.
    pub const fn name(self) -> &'static str {
        self.format.as_str()
    }

    /// File extensions associated with the format.
    pub const fn extensions(self) -> &'static [&'static str] {
        self.format.extensions()
    }

    /// True if at least one direction is compiled in.
    pub const fn available(self) -> bool {
        self.read || self.write
    }
}

/// Formats this build can actually read or write, in `Format::ALL` order.
pub fn formats() -> Vec<FormatInfo> {
    Format::ALL
        .iter()
        .copied()
        .map(|format| FormatInfo {
            format,
            read: format.supports_read(),
            write: format.supports_write(),
        })
        .filter(|info| info.available())
        .collect()
}
