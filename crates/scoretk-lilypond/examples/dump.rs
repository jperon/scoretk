// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
fn main() {
    let path = std::env::args().nth(1).expect("chemin d'un .mscx");
    let bytes = std::fs::read(path).expect("lecture");
    let container = mscz2xml::MsczContainer::from_bytes(&bytes).expect("conteneur");
    let score = mscz2xml::read::read_score(&container).expect("lecture");
    print!("{}", scoretk_lilypond::write_lilypond(&score).expect("écriture"));
}
