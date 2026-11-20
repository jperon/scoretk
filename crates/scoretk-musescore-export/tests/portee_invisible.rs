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

//! Round trip de la visibilité de portée par l'écrivain `.mscx`.
//!
//! `TWrite::write(Staff*)` (`write/twrite.cpp:2945-2947`) écrit
//! `<isStaffVisible>0</isStaffVisible>` pour une portée invisible. Sans cela,
//! la visibilité perdue à l'écriture ne survit pas à la relecture.

use std::fs;
use std::path::Path;

use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> scoretk_core::model::Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

#[test]
fn portee_invisible_survit_a_lecture_ecriture_relecture() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v4")
        .join("dynamics.mscx");
    let mut partition = lire(&fs::read(racine).expect("corpus"));
    let staff = partition.parts[0].staves[0];
    partition.staff_mut(staff).visible = false;

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<isStaffVisible>0</isStaffVisible>"),
        "visibilité perdue à l'écriture :\n{ecrit}"
    );
    let relu = lire(ecrit.as_bytes());
    assert!(
        !relu.staff(relu.parts[0].staves[0]).visible,
        "visibilité perdue à la relecture"
    );
}
