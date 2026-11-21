// SPDX-License-Identifier: GPL-3.0-only
use scoretk_musescore_import::MsczContainer;
#[test]
fn dump() {
    let path = "/home/jacques/Musique/Partitions/Chorales/Noel/00_Noel_2026/06- Minuit chrétien/00- Minuit chretien - Chant - piano.mscz";
    let bytes = std::fs::read(path).unwrap();
    let container = MsczContainer::from_bytes(&bytes).unwrap();
    let score = scoretk_musescore_import::read::read_score(&container).unwrap();
    for measure in score.measures.iter() {
        for seg in &measure.segments {
            let tick = seg.tick.ticks(1920);
            let q = tick / 480;

            for (tr, item) in seg.iter_elements() {
                if let scoretk_core::model::SegmentItem::ChordRest(cr) = item {
                    if let scoretk_core::model::ChordRestKind::Chord(c) = &cr.kind {
                        let notes: Vec<_> = c
                            .notes
                            .iter()
                            .map(|n| {
                                let note = score.note(*n);
                                (
                                    note.pitch,
                                    note.tie_back.map(|_| 1),
                                    note.tie_forward.map(|_| 1),
                                )
                            })
                            .collect();
                        if tr.staff().raw() == 6 && q >= 508 {
                            println!(
                                "q{} staff {} voice {} notes {:?} orn {:?}",
                                q,
                                tr.staff().raw(),
                                tr.voice(),
                                notes,
                                c.ornaments
                                    .iter()
                                    .map(|o| (format!("{:?}", o.kind), o.play))
                                    .collect::<Vec<_>>()
                            );
                        }
                    }
                }
            }
        }
    }
}
