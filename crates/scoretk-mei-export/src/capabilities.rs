// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : déclaration des couches que MEI 5.1 peut porter.
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

//! Couches écrites vers MEI 5.1 (§5.3 de `model-modularisation.md`).
//!
//! Citation : `docs/agents/format-ref/mei.md` §2–§13 et
//! `docs/agents/format-mapping/mei.md` lignes 25–90.

use scoretk_core::model::{Capabilities, Layers};

/// Couches que le format MEI peut porter (écriture).
pub const CAPABILITIES: Capabilities = Capabilities::new(
    Layers::NONE,
    Layers::COMMON
        .union(Layers::METRIC)
        .union(Layers::EVENTS)
        .union(Layers::TEXT)
        .union(Layers::PLAYBACK)
        .union(Layers::ENGRAVING),
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mei_writes_l0_to_l5() {
        assert_eq!(
            CAPABILITIES.declared(),
            Layers::COMMON
                .union(Layers::METRIC)
                .union(Layers::EVENTS)
                .union(Layers::TEXT)
                .union(Layers::PLAYBACK)
                .union(Layers::ENGRAVING)
        );
        assert_eq!(CAPABILITIES.reads, Layers::NONE);
    }
}
