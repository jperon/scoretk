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

//! Export MusicXML des points d'orgue et des respirations.
//!
//! Les formes de point d'orgue suivent `ExportMusicXml::fermata`
//! (`exportmusicxml.cpp`) ; les respirations suivent `writeBreathMark`
//! (`exportmusicxml.cpp:3398-3458`) : `caesura` avec `curved`/`short`/
//! `thick`/`single`, `breath-mark` avec `tick`/`upbow`/`salzedo`/`comma`.
//! Une respiration au-dessous porte `placement="below"`.
//!
//! La parité au corpus est vérifiée par `crates/scoretk/tests/notations_corpus.rs`
//! (`v3/layout_elements` pour le `tick`, `v4/copypaste_parts` pour la césure
//! courbe) ; ce test couvre les variantes absentes du corpus.

use scoretk_core::model::{
    Annotation, Breath, BreathType, ChordRest, Duration, DurationType, Fermata, FermataType,
    Fraction, Instrument, Key, KeySig, Measure, Note, Part, Placement, Score, Segment, SegmentItem,
    SegmentType, StaffId, TimeSig, Track,
};
use scoretk_musicxml_export::export_musicxml;

fn piano(score: &mut Score) -> StaffId {
    let part_id = score.add_part(Part::new("Piano", "Pno."));
    score.part_mut(part_id).instrument = Instrument {
        id: "keyboard.piano".to_owned(),
        long_name: "Piano".to_owned(),
        short_name: "Pno.".to_owned(),
        track_name: "Piano".to_owned(),
        transpose: None,
        midi_channel: None,
        midi_port: None,
        midi_program: Some(0),
        volume: None,
        pan: None,
        midi_articulations: Vec::new(),
    };
    score.add_staff(part_id)
}

fn measure(score: &mut Score, staff: StaffId, start: Fraction, len: Fraction) -> Measure {
    let mut measure = Measure::new(0, start, len);
    let track = Track::new(staff, 0);
    let mut timesig = Segment::new(SegmentType::TimeSig, start, score.staves.len());
    timesig.set_element(track, SegmentItem::TimeSig(TimeSig::new(4, 4)));
    measure.add_segment(timesig);
    let mut keysig = Segment::new(SegmentType::KeySig, start, score.staves.len());
    keysig.set_element(track, SegmentItem::KeySig(KeySig::new(Key::new(0))));
    measure.add_segment(keysig);
    measure
}

fn add_chord(
    score: &mut Score,
    measure: &mut Measure,
    staff: StaffId,
    tick: Fraction,
    duration: Duration,
) {
    let note = score.add_note(Note::new(60, 14));
    let mut chord_rest = ChordRest::chord(duration);
    chord_rest.chord_mut().expect("accord").notes.push(note);
    let mut segment = Segment::new(SegmentType::ChordRest, tick, score.staves.len());
    segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord_rest));
    measure.add_segment(segment);
}

fn add_fermata(measure: &mut Measure, staff: StaffId, fermata: Fermata) {
    let track = Track::new(staff, 0);
    let tick = measure.start_tick;
    for segment in &mut measure.segments {
        if segment.kind == SegmentType::ChordRest && segment.tick == tick {
            segment.add_annotation(track, Annotation::Fermata(fermata));
            return;
        }
    }
    panic!("aucun accord au début de la mesure");
}

fn add_breath(measure: &mut Measure, score: &Score, staff: StaffId, breath: Breath) {
    let tick = measure.end_tick();
    let mut segment = Segment::new(SegmentType::Breath, tick, score.staves.len());
    segment.set_element(Track::new(staff, 0), SegmentItem::Breath(breath));
    measure.add_segment(segment);
}

fn export(score: &Score) -> String {
    export_musicxml(score).expect("export MusicXML")
}

/// Formes de point d'orgue et placement ; la forme normale n'a pas de corps.
#[test]
fn exporte_les_points_d_orgue() {
    let mut score = Score::new();
    let staff = piano(&mut score);
    let whole = Fraction::ONE;
    for (number, (kind, placement)) in [
        (FermataType::Normal, Placement::Above),
        (FermataType::Long, Placement::Below),
        (FermataType::VeryShort, Placement::Above),
        (FermataType::LongHenze, Placement::Above),
        (FermataType::Curlew, Placement::Below),
    ]
    .into_iter()
    .enumerate()
    {
        let start = whole * Fraction::from_integer(number as i64);
        let mut measure = measure(&mut score, staff, start, whole);
        add_chord(
            &mut score,
            &mut measure,
            staff,
            start,
            Duration::new(DurationType::Whole),
        );
        add_fermata(
            &mut measure,
            staff,
            Fermata {
                kind,
                placement,
                visible: true,
            },
        );
        score.add_measure(measure);
    }

    let xml = export(&score);
    assert!(xml.contains("<fermata type=\"upright\"/>"), "{xml}");
    assert!(
        xml.contains("<fermata type=\"inverted\">square</fermata>"),
        "{xml}"
    );
    assert!(
        xml.contains("<fermata type=\"upright\">double-angled</fermata>"),
        "{xml}"
    );
    assert!(
        xml.contains("<fermata type=\"upright\">double-dot</fermata>"),
        "{xml}"
    );
    assert!(
        xml.contains("<fermata type=\"inverted\">curlew</fermata>"),
        "{xml}"
    );
}

/// Variantes de respiration absentes du corpus.
#[test]
fn exporte_les_respirations() {
    let mut score = Score::new();
    let staff = piano(&mut score);
    let whole = Fraction::ONE;
    for (number, (kind, placement)) in [
        (BreathType::Salzedo, Placement::Below),
        (BreathType::Upbow, Placement::Above),
        (BreathType::Tick, Placement::Above),
        (BreathType::CaesuraThick, Placement::Below),
        (BreathType::CaesuraSingle, Placement::Above),
        (BreathType::Comma, Placement::Above),
    ]
    .into_iter()
    .enumerate()
    {
        let start = whole * Fraction::from_integer(number as i64);
        let mut measure = measure(&mut score, staff, start, whole);
        add_chord(
            &mut score,
            &mut measure,
            staff,
            start,
            Duration::new(DurationType::Whole),
        );
        add_breath(&mut measure, &score, staff, Breath { kind, placement });
        score.add_measure(measure);
    }

    let xml = export(&score);
    assert!(
        xml.contains("<breath-mark placement=\"below\">salzedo</breath-mark>"),
        "{xml}"
    );
    assert!(xml.contains("<breath-mark>upbow</breath-mark>"), "{xml}");
    assert!(xml.contains("<breath-mark>tick</breath-mark>"), "{xml}");
    assert!(
        xml.contains("<caesura placement=\"below\">thick</caesura>"),
        "{xml}"
    );
    assert!(xml.contains("<caesura>single</caesura>"), "{xml}");
    assert!(xml.contains("<breath-mark>comma</breath-mark>"), "{xml}");
}
