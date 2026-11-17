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

//! Diagnostic : imprime les liaisons de prolongation du modèle.

use scoretk_musicxml::read_musicxml;

fn main() {
    let path = std::env::args().nth(1).expect("chemin");
    let score = read_musicxml(&std::fs::read(&path).expect("lecture")).expect("analyse");
    for (index, note) in score.notes.iter().enumerate() {
        if note.tie_forward.is_some() || note.tie_back.is_some() {
            println!(
                "{index}: pitch {} back {:?} forward {:?}",
                note.pitch, note.tie_back, note.tie_forward
            );
        }
    }
}
