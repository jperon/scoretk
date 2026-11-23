// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `writeMusicXml(const FretDiagram*)`
// (`exportmusicxml.cpp:9049-9122`) : la grille est écrite en `<frame>` dans
// l'élément `<harmony>`.

use scoretk_core::model::{
    tpc, Annotation, ChordRest, Duration, Fraction, FretBarre, FretDiagram, FretDot, FretDotType,
    FretMarker, FretMarkerType, Harmony, Measure, Note, Part, Score, Segment, SegmentItem,
    SegmentType, Track,
};
use scoretk_musicxml_export::export_musicxml;

#[test]
fn grille_d_accord_ecrite_en_frame() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Guitare", "Gt."));
    let staff_id = score.add_staff(part_id);
    let track = Track::new(staff_id, 0);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, score.staves.len());
    let mut chord = ChordRest::chord(Duration::whole());
    chord
        .chord_mut()
        .expect("accord")
        .notes
        .push(score.add_note(Note::new(67, tpc::G)));
    segment.set_element(track, SegmentItem::ChordRest(chord));
    let mut harmony = Harmony::new("G");
    harmony.root_tpc = tpc::G;
    harmony.kind = "normal".to_owned();
    harmony.fret_diagram = Some(FretDiagram {
        strings: 6,
        frets: 4,
        fret_offset: 0,
        show_nut: true,
        dots: vec![
            FretDot {
                string: 0,
                fret: 3,
                kind: FretDotType::Normal,
            },
            FretDot {
                string: 5,
                fret: 3,
                kind: FretDotType::Normal,
            },
        ],
        markers: vec![FretMarker {
            string: 2,
            kind: FretMarkerType::Circle,
        }],
        barres: vec![FretBarre {
            start: 0,
            end: -1,
            fret: 3,
        }],
    });
    segment.add_annotation(track, Annotation::Harmony(harmony));
    measure.add_segment(segment);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<frame>"), "frame absent :\n{xml}");
    assert!(xml.contains("<frame-strings>6</frame-strings>"));
    assert!(xml.contains("<frame-frets>4</frame-frets>"));
    // Corde 6 : point frette 3, début de barre ; corde 4 : marqueur cercle ;
    // corde 1 : fin de barre (`end="-1"`).
    for needle in [
        "<string>6</string>",
        "<fret>3</fret>",
        "<barre type=\"start\"/>",
        "<string>4</string>",
        "<fret>0</fret>",
        "<barre type=\"stop\"/>",
    ] {
        assert!(xml.contains(needle), "{needle} absent :\n{xml}");
    }
}

#[test]
fn grille_avec_decalage_ecrit_first_fret() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Guitare", "Gt."));
    let staff_id = score.add_staff(part_id);
    let track = Track::new(staff_id, 0);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, score.staves.len());
    let mut chord = ChordRest::chord(Duration::whole());
    chord
        .chord_mut()
        .expect("accord")
        .notes
        .push(score.add_note(Note::new(67, tpc::G)));
    segment.set_element(track, SegmentItem::ChordRest(chord));
    let mut harmony = Harmony::new("G");
    harmony.root_tpc = tpc::G;
    let mut diagram = FretDiagram {
        fret_offset: 4,
        ..FretDiagram::default()
    };
    diagram.dots.push(FretDot {
        string: 0,
        fret: 3,
        kind: FretDotType::Normal,
    });
    harmony.fret_diagram = Some(diagram);
    segment.add_annotation(track, Annotation::Harmony(harmony));
    measure.add_segment(segment);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<first-fret>5</first-fret>"), "{xml}");
    assert!(
        xml.contains("<fret>7</fret>"),
        "fretOffset non ajouté : {xml}"
    );
}
