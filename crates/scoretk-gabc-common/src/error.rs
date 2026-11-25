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

//! Error and result shared by the GABC reader and writer.

/// Failure of a GABC conversion.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Input is not UTF-8 text (a `.gabc` file is text, `Gabc.tex:36-64`).
    #[error("GABC input is not UTF-8: {0}")]
    Utf8(String),
    /// The two sections or a required header are missing
    /// (`Gabc.tex:51,58,79-82`).
    #[error("invalid GABC: {0}")]
    Syntax(String),
}

/// Result of a GABC conversion.
pub type Result<T> = std::result::Result<T, Error>;
