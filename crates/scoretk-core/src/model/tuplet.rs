// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/tuplet.h` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! N-olets.
//!
//! Portage de `Tuplet` (`engraving/dom/tuplet.h`). Le ratio suit la convention
//! de MuseScore : un triolet de croches a `ratio = 3/2`, `base_len = 1/8`, et
//! chaque croche du triolet dure `base_len * denominator / numerator`.
//!
//! Les n-olets imbriqués sont représentés par [`Tuplet::parent`] ; l'accord ou
//! le silence ne référence que le n-olet le plus interne via
//! [`super::ChordRest::tuplet`].

// L1 metric types live in `layers/metric.rs`; re-exported here so that
// `model::tuplet::{Tuplet, TupletNumberType}` keeps working.
pub use super::layers::metric::{Tuplet, TupletNumberType};
