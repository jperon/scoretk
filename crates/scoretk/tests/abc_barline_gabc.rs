// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : conventions ABC des barres de séparation
// grégoriennes, d'après gabctk (`gabctk.py:932-933`), décision de
// l'utilisateur du 2026-11-25.
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

//! `,` (divisio minima, style MusicXML `tick`) et `;` (divisio minor, style
//! MusicXML `short`) sont écrits en ABC comme dans gabctk.

use scoretk::{read, write, Format, Options};

const GABC: &str = "name: Barres;\n%%\n(c3)De(g,)(us;)(et:)(iam::)\n";

#[test]
fn barres_gregoriennes_suivent_gabctk_en_abc() {
    let score = read(GABC.as_bytes(), Format::Gabc).expect("lecture GABC");
    let abc = String::from_utf8(write(&score, Format::Abc, &Options::default()).unwrap())
        .expect("ABC UTF-8");
    assert!(
        abc.contains("!shortphrase![|]"),
        "divisio minima `,` : {abc}"
    );
    assert!(
        abc.contains("!mediumphrase![|]"),
        "divisio minor `;` : {abc}"
    );
    // Les deux autres barres gardent leurs symboles ordinaires.
    assert!(abc.contains("||"), "divisio maior : {abc}");
}
