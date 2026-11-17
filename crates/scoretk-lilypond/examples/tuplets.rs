// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
fn main() {
    let path = std::env::args().nth(1).expect("chemin");
    let bytes = std::fs::read(path).expect("lecture");
    let container = mscz2xml::MsczContainer::from_bytes(&bytes).expect("conteneur");
    let score = mscz2xml::read::read_score(&container).expect("lecture");
    for (index, tuplet) in score.tuplets.iter().enumerate() {
        println!("{index}: tick={:?} len={:?} ratio={:?} parent={:?} end={:?}", tuplet.tick, tuplet.len, tuplet.ratio, tuplet.parent, tuplet.end_tick());
    }
    for measure in &score.measures {
        println!("mesure {} tick={:?} len={:?}", measure.number, measure.start_tick, measure.len);
    }
}
