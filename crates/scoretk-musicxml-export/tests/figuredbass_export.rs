// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `FiguredBass::parenthesesMode()`
// (`engraving/dom/figuredbass.cpp:1216-1224`) et `writeMusicXml(const FiguredBass*)`
// (`importexport/musicxml/internal/export/exportmusicxml.cpp:6832-6834`).

use scoretk_core::model::{
    tpc, Annotation, ChordRest, Duration, FiguredBass, FiguredBassItem, Fraction, Measure, Note,
    Part, Score, Segment, SegmentItem, SegmentType, Track,
};
use scoretk_musicxml_export::{export_musicxml, export_musicxml_with, ExportOptions};

/// Partition minimale : une noire sur une portée, une mesure de 4/4.
fn partition_avec_chiffrage(brackets: [u8; 5]) -> Score {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Continuo", "C."));
    let staff_id = score.add_staff(part_id);
    let track = Track::new(staff_id, 0);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, score.staves.len());
    let mut chord = ChordRest::chord(Duration::whole());
    chord
        .chord_mut()
        .expect("accord")
        .notes
        .push(score.add_note(Note::new(48, tpc::C)));
    segment.set_element(track, SegmentItem::ChordRest(chord));
    let mut bass = FiguredBass::new();
    bass.ticks = Fraction::ONE;
    bass.items.push(FiguredBassItem {
        digit: Some(6),
        brackets,
        ..FiguredBassItem::default()
    });
    segment.add_annotation(track, Annotation::FiguredBass(bass));
    measure.add_segment(segment);
    score.add_measure(measure);
    score
}

#[test]
fn chiffrage_crochets_ecrit_parentheses() {
    // `parenthesesMode()` regarde l'encadrement de la partie affichée : un
    // crochet carré sur le chiffre (b1 = 3, b2 = 4) suffit à écrire
    // `parentheses="yes"`.
    let xml = export_musicxml(&partition_avec_chiffrage([0, 3, 4, 0, 0])).expect("export");
    assert!(
        xml.contains("<figured-bass parentheses=\"yes\">"),
        "encadrement non exporté :\n{xml}"
    );
}

/// Un chiffrage vide (`items.is_empty()`) est omis par MuseScore 4.7.4.
/// `<figured-bass><figure/></figured-bass>` est valide dans la XSD 4.0
/// (`xsd:figured-bass`, `xsd:figure`) et l'écrit dans `full_support` et dans
/// le mode secondaire ; le défaut garde les octets 4.7.4.
#[test]
fn chiffrage_vide_preserve_en_mode_complet() {
    let mut score = partition_avec_chiffrage([0; 5]);
    let segment = &mut score.measures[0].segments[0];
    let entry = segment
        .annotations
        .iter_mut()
        .find(|entry| matches!(entry.annotation, Annotation::FiguredBass(_)))
        .expect("chiffrage");
    if let Annotation::FiguredBass(bass) = &mut entry.annotation {
        bass.items.clear();
    }
    let plain = export_musicxml(&score).expect("export 4.7.4");
    assert!(!plain.contains("<figured-bass"), "{plain}");
    let options = ExportOptions {
        lossless_round_trip: true,
        ..ExportOptions::default()
    };
    let full = export_musicxml_with(&score, &options).expect("export secondaire");
    assert!(full.contains("<figured-bass>"));
    assert!(full.contains("<figure/>"), "{full}");
    let xsd = export_musicxml_with(
        &score,
        &ExportOptions {
            full_support: true,
            ..ExportOptions::default()
        },
    )
    .expect("export XSD");
    assert!(xsd.contains("<figured-bass>"), "{xsd}");
    assert!(xsd.contains("<figure/>"), "{xsd}");
}

#[test]
fn chiffrage_sans_crochets_reste_sans_parentheses() {
    let xml = export_musicxml(&partition_avec_chiffrage([0; 5])).expect("export");
    assert!(
        xml.contains("<figured-bass>"),
        "figured-bass absent :\n{xml}"
    );
    assert!(!xml.contains("<figured-bass parentheses=\"yes\">"));
}
