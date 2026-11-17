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

//! Diagnostic : lit un MusicXML, le réexporte et imprime le nombre de lignes.
//!
//! Utilisé pour isoler un fichier bloquant : `timeout 10 cargo run --example
//! aller_retour_un -- fichier.ref.musicxml`.

use mscz2xml::export::export_musicxml_with;
use scoretk_musicxml::read_musicxml;

fn main() {
    let path = std::env::args().nth(1).expect("chemin du fichier");
    let bytes = std::fs::read(&path).expect("lecture");
    eprintln!("lecture {path}");
    let score = read_musicxml(&bytes).expect("analyse");
    eprintln!(
        "modèle : {} parties, {} mesures, {} notes, {} n-olets",
        score.parts.len(),
        score.measures.len(),
        score.notes.len(),
        score.tuplets.len()
    );
    let xml = export_musicxml_with(&score, &Default::default()).expect("export");
    println!("{}", xml.lines().count());
}
