// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de mscz2xml
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

//! Tests d'export du noyau sur des partitions construites à la main.
//!
//! Couvre les éléments ajoutés pour les écarts 5 à 13 de
//! `docs/agents/ecarts-noyau.md` : ottava (hauteur écrite et
//! `<octave-shift>`), nombres affichés des n-olets, silences multi-mesures,
//! noms de partie décomposés, canaux MIDI et compteur de reprise.

use mscz2xml::export::export_musicxml;
use mscz2xml::model::{
    BarLineType, ChordRest, Duration, DurationType, Fraction, Instrument, Key, KeySig, Measure,
    Note, OttavaType, Part, Score, Segment, SegmentItem, SegmentType, Spanner, SpannerAnchor,
    SpannerType, StaffId, TimeSig, Track, Tuplet,
};

fn piano(score: &mut Score) -> StaffId {
    let part_id = score.add_part(Part::new("Piano", "Pno."));
    score.part_mut(part_id).instrument = Instrument {
        id: "keyboard.piano".to_owned(),
        long_name: "Piano".to_owned(),
        short_name: "Pno.".to_owned(),
        track_name: "Piano".to_owned(),
        transpose: None,
        midi_channel: None,
        midi_program: Some(0),
        volume: None,
        pan: None,
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
    pitch: u8,
    tpc: i8,
) -> mscz2xml::model::NoteId {
    let note = score.add_note(Note::new(pitch, tpc));
    let mut chord_rest = ChordRest::chord(duration);
    chord_rest.chord_mut().expect("accord").notes.push(note);
    let mut segment = Segment::new(SegmentType::ChordRest, tick, score.staves.len());
    segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord_rest));
    measure.add_segment(segment);
    note
}

fn export(score: &Score) -> String {
    export_musicxml(score).expect("export MusicXML")
}

#[test]
fn ottava_décale_la_hauteur_écrite_et_écrit_octave_shift() {
    let mut score = Score::new();
    let staff = piano(&mut score);
    let mut measure = measure(&mut score, staff, Fraction::ZERO, Fraction::ONE);
    add_chord(
        &mut score,
        &mut measure,
        staff,
        Fraction::ZERO,
        Duration::new(DurationType::Whole),
        60,
        14,
    );
    let end = measure.end_tick();
    score.add_measure(measure);
    score.add_spanner(Spanner::new(
        SpannerType::Ottava(OttavaType::OctaveUp),
        SpannerAnchor::new(Fraction::ZERO, Track::new(staff, 0)),
        SpannerAnchor::new(end, Track::new(staff, 0)),
    ));

    let xml = export(&score);
    assert!(xml.contains("<octave>5</octave>"), "{xml}");
    assert!(
        xml.contains("<octave-shift type=\"down\" size=\"8\" number=\"1\"/>"),
        "{xml}"
    );
    assert!(
        xml.contains("<octave-shift type=\"stop\" size=\"8\" number=\"1\"/>"),
        "{xml}"
    );
}

#[test]
fn n_olet_imbriqué_exporte_les_nombres_bruts() {
    let mut score = Score::new();
    let staff = piano(&mut score);
    let mut measure = measure(&mut score, staff, Fraction::ZERO, Fraction::new(1, 4));

    let outer = score.add_tuplet(Tuplet::new(
        Fraction::new(3, 2),
        Fraction::new(1, 8),
        Fraction::ZERO,
        Fraction::new(3, 16),
    ));
    let mut inner = Tuplet::new(
        Fraction::ONE,
        Fraction::new(1, 8),
        Fraction::ZERO,
        Fraction::new(1, 2),
    );
    inner.parent = Some(outer);
    inner.actual_notes = 4;
    inner.normal_notes = 4;
    let inner = score.add_tuplet(inner);

    let note = score.add_note(Note::new(60, 14));
    let mut chord_rest = ChordRest::chord(Duration::new(DurationType::Eighth));
    chord_rest.tuplet = Some(inner);
    chord_rest.chord_mut().expect("accord").notes.push(note);
    let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, score.staves.len());
    segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord_rest));
    measure.add_segment(segment);
    score.add_measure(measure);

    let xml = export(&score);
    assert!(xml.contains("<tuplet-number>4</tuplet-number>"), "{xml}");
    assert!(xml.contains("<tuplet-number>3</tuplet-number>"), "{xml}");
    assert!(xml.contains("number=\"2\""), "{xml}");
}

#[test]
fn silence_multi_mesures_exporte_measure_style() {
    let mut score = Score::new();
    let staff = piano(&mut score);
    let mut measure = measure(&mut score, staff, Fraction::ZERO, Fraction::ONE);
    measure.multi_measure_rest = Some(4);
    measure.add_segment({
        let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, score.staves.len());
        segment.set_element(
            Track::new(staff, 0),
            SegmentItem::ChordRest(ChordRest::full_measure_rest()),
        );
        segment
    });
    score.add_measure(measure);

    let xml = export(&score);
    assert!(xml.contains("<measure-style>"), "{xml}");
    assert!(xml.contains("<multiple-rest>4</multiple-rest>"), "{xml}");
}

#[test]
fn nom_de_partie_décomposé_en_fragments() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("B♭ Clarinet", "B♭ Cl."));
    score.part_mut(part_id).instrument.track_name = "B♭ Clarinet".to_owned();
    score.add_staff(part_id);
    let staff = StaffId::from_index(0);
    let measure = measure(&mut score, staff, Fraction::ZERO, Fraction::ONE);
    score.add_measure(measure);

    let xml = export(&score);
    assert!(xml.contains("<part-name-display"), "{xml}");
    assert!(xml.contains("<display-text>B</display-text>"), "{xml}");
    assert!(
        xml.contains("<accidental-text>flat</accidental-text>"),
        "{xml}"
    );
    assert!(
        xml.contains("<display-text> Clarinet</display-text>"),
        "{xml}"
    );
    assert!(xml.contains("<part-abbreviation-display"), "{xml}");
    assert!(xml.contains(">Bb Clarinet</part-name>"), "{xml}");
}

#[test]
fn volume_et_panoramique_du_canal_midi() {
    let mut score = Score::new();
    let staff = piano(&mut score);
    score.parts[0].instrument.volume = Some(101);
    score.parts[0].instrument.pan = Some(63);
    let measure = measure(&mut score, staff, Fraction::ZERO, Fraction::ONE);
    score.add_measure(measure);

    let xml = export(&score);
    assert!(xml.contains("<volume>79.5276</volume>"), "{xml}");
    assert!(xml.contains("<pan>0</pan>"), "{xml}");
}

#[test]
fn reprise_de_fin_exporte_le_nombre_de_lectures() {
    let mut score = Score::new();
    let staff = piano(&mut score);
    let mut measure = measure(&mut score, staff, Fraction::ZERO, Fraction::ONE);
    measure.end_bar_line = BarLineType::EndRepeat;
    measure.repeat_count = 3;
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(
        xml.contains("<repeat direction=\"backward\" times=\"3\"/>"),
        "{xml}"
    );
}
