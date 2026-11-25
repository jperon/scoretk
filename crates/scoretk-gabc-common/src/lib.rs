// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Syntax reimplemented from Gregorio's GABC specification (`doc/Gabc.tex`,
// `src/gabc/gabc-score-determination.{y,l}`, `src/struct.h`, `src/struct.c`),
// Copyright (C) Gregorio Project, GPL-3.0-or-later, rewritten in Rust in 2026.
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

//! Shared GABC syntax for both directions.
//!
//! Only what the reader **and** the writer both use lives here: the error type
//! and the staff-position/clef tables (`Gabc.tex:408-443,847-870`). The
//! syllable parser stays in `scoretk-gabc-import` and the emitter in
//! `scoretk-gabc-export` (AGENTS.md, rule for `-common` crates).

pub mod error;
pub mod syntax;

pub use error::{Error, Result};
