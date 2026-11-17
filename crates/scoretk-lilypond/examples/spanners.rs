// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
fn main() {
    let path = std::env::args().nth(1).expect("chemin");
    let bytes = std::fs::read(path).expect("lecture");
    let container = mscz2xml::MsczContainer::from_bytes(&bytes).expect("conteneur");
    let score = mscz2xml::read::read_score(&container).expect("lecture");
    for (index, spanner) in score.spanners.iter().enumerate() {
        println!("{index}: {:?} start={:?}/{:?} end={:?}/{:?} visible={} text={:?}", spanner.kind, spanner.start.tick, spanner.start.track.raw(), spanner.end.tick, spanner.end.track.raw(), spanner.visible, spanner.text);
    }
    for (index, measure) in score.measures.iter().enumerate() {
        println!("mesure {index} tick={:?} len={:?} nominal={:?} startbar={:?} endbar={:?} repeat={}", measure.start_tick, measure.len, measure.nominal_len, measure.start_bar_line, measure.end_bar_line, measure.repeat_count);
        for segment in &measure.segments {
            for (track, item) in segment.iter_elements() {
                match item {
                    mscz2xml::model::SegmentItem::ChordRest(cr) => println!("  seg {:?} t={:?} track={} cr={:?} ticks={:?}", segment.kind, segment.tick, track.raw(), cr.duration, cr.ticks),
                    other => println!("  seg {:?} t={:?} track={} item={:?}", segment.kind, segment.tick, track.raw(), other),
                }
            }
            for ann in &segment.annotations {
                println!("  ann t={:?} track={} {:?}", segment.tick, ann.track.raw(), ann.annotation);
            }
        }
    }
}
