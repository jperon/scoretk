// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de scoretk
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

//! Tests de construction manuelle d'une partition.
//!
//! Ils servent de spécification vivante pour les lecteurs `.mscx` et pour
//! l'export MusicXML : une partie, deux mesures, un accord, un silence, un
//! triolet, une liaison de prolongation.

use super::*;

fn construire_partition() -> Score {
    let mut score = Score::new();

    let part_id = score.add_part(Part::new("Piano", "Pno"));
    let staff_id = score.add_staff(part_id);
    let staff_count = score.staves.len();

    // --- Mesure 1 : noire, noire de silence, triolet de croches. -----------
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    measure.len = Fraction::new(3, 4); // mesure irrégulière de 3/4

    let mut chord_segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, staff_count);
    let note_id = score.add_note(Note::new(60, tpc::C));
    let mut chord = ChordRest::chord(Duration::quarter());
    chord
        .chord_mut()
        .expect("l'élément doit être un accord")
        .notes
        .push(note_id);
    chord
        .lyrics
        .push(Lyrics::new(0, LyricsSyllabic::Single, "Do"));
    chord_segment.set_element(Track::new(staff_id, 0), SegmentItem::ChordRest(chord));
    chord_segment.add_annotation(
        Track::new(staff_id, 0),
        Annotation::Dynamic(Dynamic::new("mf")),
    );
    measure.add_segment(chord_segment);

    let mut rest_segment = Segment::new(SegmentType::ChordRest, Fraction::QUARTER, staff_count);
    rest_segment.set_element(
        Track::new(staff_id, 0),
        SegmentItem::ChordRest(ChordRest::rest(Duration::quarter())),
    );
    measure.add_segment(rest_segment);

    let tuplet_id = score.add_tuplet(Tuplet::new(
        Fraction::new(3, 2),
        Fraction::new(1, 8),
        Fraction::new(1, 2),
        Fraction::new(1, 4),
    ));

    let mut triolet_notes = Vec::new();
    for index in 0..3_i64 {
        let tick = Fraction::new(1, 2) + Fraction::new(index, 12);
        let mut segment = Segment::new(SegmentType::ChordRest, tick, staff_count);
        let note_id = score.add_note(Note::new(62 + index as u8, tpc::D));
        triolet_notes.push(note_id);
        let mut chord = ChordRest::chord(Duration::eighth());
        chord.tuplet = Some(tuplet_id);
        chord
            .chord_mut()
            .expect("l'élément doit être un accord")
            .notes
            .push(note_id);
        segment.set_element(Track::new(staff_id, 0), SegmentItem::ChordRest(chord));
        measure.add_segment(segment);
    }

    score.add_measure(measure);

    // --- Mesure 2 : silence de mesure entière. -----------------------------
    let mut measure = Measure::new(1, Fraction::ONE, Fraction::ONE);
    let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ONE, staff_count);
    segment.set_element(
        Track::new(staff_id, 0),
        SegmentItem::ChordRest(ChordRest::full_measure_rest()),
    );
    measure.add_segment(segment);
    score.add_measure(measure);

    // Liaison de prolongation entre la première note et la première note du
    // triolet, via des identifiants et sans pointeur cyclique.
    score.note_mut(note_id).tie_forward = Some(triolet_notes[0]);
    score.note_mut(triolet_notes[0]).tie_back = Some(note_id);

    score
}

#[test]
fn construction_partition_minimale() {
    let score = construire_partition();

    assert_eq!(score.parts.len(), 1);
    assert_eq!(score.staves.len(), 1);
    assert_eq!(score.measures.len(), 2);
    assert_eq!(score.notes.len(), 4);
    assert_eq!(score.tuplets.len(), 1);

    let part = &score.parts[0];
    assert_eq!(part.long_name, "Piano");
    assert_eq!(part.short_name, "Pno");
    assert_eq!(part.staves, vec![score.staves[0].id]);
    assert_eq!(score.staves[0].part, part.id);
    assert_eq!(score.staves[0].lines, 5);

    assert_eq!(score.measures[0].number, 0);
    assert!(score.measures[0].is_irregular());
    assert_eq!(score.measures[0].len, Fraction::new(3, 4));
    assert_eq!(score.measures[0].nominal_len, Fraction::ONE);
    assert_eq!(score.measures[0].segments.len(), 5);
    assert_eq!(score.measures[1].number, 1);
    assert!(!score.measures[1].is_irregular());
    assert_eq!(score.end_tick(), Fraction::from_integer(2));

    let first_note = score.note(NoteId::from_index(0));
    assert_eq!(first_note.pitch, 60);
    assert_eq!(first_note.tpc, tpc::C);
    assert!(first_note.is_tie_start());
    assert!(!first_note.is_tie_stop());
    assert!(score.note(NoteId::from_index(1)).is_tie_stop());

    let chord_rest = score.measures[0].segments[0]
        .chord_rest(Track::new(score.staves[0].id, 0))
        .expect("le premier segment doit contenir un accord");
    assert!(chord_rest.is_chord());
    assert_eq!(
        chord_rest.chord_ref().expect("accord").notes,
        vec![NoteId::from_index(0)]
    );
    assert_eq!(chord_rest.lyrics[0].number(), 1);
    assert_eq!(chord_rest.lyrics[0].text, "Do");
    assert_eq!(score.measures[0].segments[0].annotations[0].text(), "mf");

    let rest = score.measures[1].segments[0]
        .chord_rest(Track::new(score.staves[0].id, 0))
        .expect("la seconde mesure doit contenir un silence");
    assert!(rest.is_rest());
    assert!(rest.is_full_measure_rest());
    assert_eq!(rest.ticks_in_measure(Fraction::ONE), Fraction::ONE);

    let tuplet = score.tuplet(TupletId::from_index(0));
    assert_eq!(tuplet.ratio, Fraction::new(3, 2));
    assert_eq!(tuplet.base_len, Fraction::new(1, 8));
    assert_eq!(tuplet.number(), 3);
    assert_eq!(tuplet.end_tick(), Fraction::new(3, 4));
    assert_eq!(tuplet.parent, None);

    for index in 2..5 {
        let chord_rest = score.measures[0].segments[index]
            .chord_rest(Track::new(score.staves[0].id, 0))
            .expect("le triolet doit contenir des accords");
        assert_eq!(chord_rest.tuplet, Some(TupletId::from_index(0)));
        assert_eq!(chord_rest.duration, Duration::eighth());
    }
}

#[test]
fn conversions_divisions_musicxml() {
    let score = Score::new();
    assert_eq!(score.division, 480);
    assert_eq!(score.tick_to_divisions(Fraction::QUARTER), 480);
    assert_eq!(score.tick_to_divisions(Fraction::ONE), 1920);
    assert_eq!(score.divisions_to_tick(480), Fraction::QUARTER);
    assert_eq!(score.divisions_to_tick(1920), Fraction::ONE);
}

#[test]
fn signatures_clefs_barres_et_pistes() {
    let time_sig = TimeSig::new(6, 8);
    assert_eq!(time_sig.measure_len(), Fraction::new(3, 4));
    assert!(time_sig.is_valid());

    let mut segment = Segment::new(SegmentType::TimeSig, Fraction::ZERO, 2);
    let track = Track::new(StaffId::from_index(1), 0);
    assert_eq!(track.raw(), 4);
    segment.set_element(track, SegmentItem::TimeSig(time_sig));
    let stored = segment
        .element(track)
        .expect("l'indication de mesure doit être sur la piste 4");
    assert!(matches!(stored, SegmentItem::TimeSig(_)));
    assert!(segment
        .element(Track::new(StaffId::from_index(0), 0))
        .is_none());

    let key_sig = KeySig::new(Key::F);
    assert_eq!(key_sig.concert_key.fifths(), -1);
    assert_eq!(key_sig.transposed_key, key_sig.concert_key);
    assert_eq!(key_sig.mode, KeyMode::Major);

    let clef = Clef::new(ClefType::F8Vb);
    assert_eq!(clef.concert.sign(), ClefSign::F);
    assert_eq!(clef.concert.line(), 4);
    assert_eq!(clef.concert.octave_change(), -1);
    assert!(!clef.concert.is_tab());

    let repeat_end = BarLine::new(BarLineType::EndRepeat);
    assert!(repeat_end.kind.is_repeat_end());
    assert!(!repeat_end.kind.is_repeat_start());

    let spanner = Spanner::new(
        SpannerType::Hairpin(HairpinType::Crescendo),
        SpannerAnchor::new(Fraction::ZERO, track),
        SpannerAnchor::new(Fraction::ONE, track),
    );
    assert_eq!(spanner.start.tick, Fraction::ZERO);
    assert_eq!(spanner.end.tick, Fraction::ONE);
    assert_eq!(spanner.end.track, track);
}

#[test]
fn accords_d_agrement() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Violon", "Vln"));
    let staff_id = score.add_staff(part_id);

    let main_note = score.add_note(Note::new(69, tpc::A));
    let appoggiatura_note = score.add_note(Note::new(67, tpc::G));
    let after_grace_note = score.add_note(Note::new(71, tpc::B));

    let mut chord_rest = ChordRest::chord(Duration::quarter());
    let main_chord = chord_rest
        .chord_mut()
        .expect("l'élément doit être un accord");
    main_chord.notes.push(main_note);

    let mut appoggiatura = GraceChord::new(GraceType::Appoggiatura, Duration::eighth());
    appoggiatura.chord_mut().notes.push(appoggiatura_note);

    let mut after_grace = GraceChord::new(GraceType::Grace8After, Duration::eighth());
    after_grace.chord_mut().notes.push(after_grace_note);

    main_chord.grace_chords.push(appoggiatura);
    main_chord.grace_chords.push(after_grace);

    let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, score.staves.len());
    segment.set_element(Track::new(staff_id, 0), SegmentItem::ChordRest(chord_rest));

    let chord = segment
        .chord_rest(Track::new(staff_id, 0))
        .expect("le segment doit contenir l'accord principal")
        .chord_ref()
        .expect("l'élément doit être un accord");
    assert_eq!(chord.grace_chords.len(), 2);
    assert_eq!(chord.grace_chords[0].duration, Duration::eighth());
    assert_eq!(
        chord.grace_chords[0].chord.grace,
        Some(GraceType::Appoggiatura)
    );
    assert_eq!(chord.grace_chords[0].chord.notes, vec![appoggiatura_note]);
    assert_eq!(
        score.note(appoggiatura_note).pitch,
        67,
        "la note d'agrément reste dans l'arène Score::notes"
    );
    assert_eq!(
        chord.grace_chords[1].chord.grace,
        Some(GraceType::Grace8After)
    );
    assert_eq!(chord.grace_chords[1].chord.notes, vec![after_grace_note]);
}

#[test]
fn annotations_avec_piste() {
    let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 2);
    let first_staff_voice_0 = Track::new(StaffId::from_index(0), 0);
    let second_staff_voice_2 = Track::new(StaffId::from_index(1), 2);

    segment.add_annotation(first_staff_voice_0, Annotation::Dynamic(Dynamic::new("p")));
    segment.add_annotation(
        second_staff_voice_2,
        Annotation::StaffText(StaffText::new("solo")),
    );

    assert_eq!(segment.annotations.len(), 2);
    assert_eq!(segment.annotations[0].track, first_staff_voice_0);
    assert_eq!(segment.annotations[0].text(), "p");
    assert_eq!(segment.annotations[1].track, second_staff_voice_2);
    assert_eq!(segment.annotations[1].text(), "solo");

    let on_second_staff: Vec<&str> = segment
        .annotations_on(second_staff_voice_2)
        .map(Annotation::text)
        .collect();
    assert_eq!(on_second_staff, vec!["solo"]);
    assert_eq!(segment.annotations_on(first_staff_voice_0).count(), 1);
}

#[test]
fn modele_send_et_sync() {
    fn verifie<T: Send + Sync>() {}
    verifie::<Score>();
    verifie::<Segment>();
    verifie::<Spanner>();
}
