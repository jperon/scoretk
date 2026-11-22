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

//! `\transposition` d'une portée dépliée : une seule partie, un `<transpose>`,
//! et la hauteur **écrite** conservée.

#![cfg(all(feature = "lilypond-import", feature = "musicxml-export"))]

use scoretk::{read, write, Format, Options};

#[test]
fn transposition_de_portee_depliee_garde_la_hauteur_ecrite() {
    let source = "\\score { \\context Staff = \"g\" << \
        \\context Staff = \"g\" { \\transposition c } { c''4 } { e''4 } >> }";
    let score = read(source.as_bytes(), Format::Ly).expect("lecture");
    let octets = write(&score, Format::MusicXml, &Options::default()).expect("écriture");
    let texte = String::from_utf8(octets).expect("UTF-8");
    assert!(texte.contains("<transpose>"), "{texte}");
    assert!(
        texte.contains("<step>C</step>\n          <octave>5</octave>"),
        "hauteur écrite C5 attendue :\n{texte}"
    );
}
