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

//! ABC 2.1 §4.6 : un `clef=treble-8` en tête de mesure déplace la lecture
//! d'un octave. Réduit de `Sancta_Maria_…` (mesure 48, voix 3) : la note au
//! tick 0 de la mesure où la clef change est écrite une octave trop bas.
//! `xml2abc.py::doAttr` (lignes 1329-1331) et `mkNote` (1154) décalent la
//! hauteur écrite de `-clef-octave-change`.

use scoretk_core::model::*;

fn pitches(score: &Score) -> Vec<u8> {
    score
        .measures
        .iter()
        .flat_map(|m| m.segments.iter())
        .flat_map(|s| s.iter_elements())
        .filter_map(|(_, it)| match it {
            SegmentItem::ChordRest(cr) => match &cr.kind {
                ChordRestKind::Chord(ch) => ch.notes.first().map(|id| score.note(*id).pitch),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn mesure(score: &mut Score, index: i64, pitch: u8, clef: Option<ClefType>, track: Track) {
    let mut measure = Measure::new(
        index as usize,
        Fraction::from_integer(index),
        Fraction::from_integer(index + 1),
    );
    let note = score.add_note(Note::new(pitch, scoretk_core::model::tpc::E));
    let mut cr = ChordRest::chord(Duration::whole());
    cr.chord_mut().unwrap().notes.push(note);
    let mut seg = Segment::new(SegmentType::ChordRest, measure.start_tick, 1);
    seg.set_element(track, SegmentItem::ChordRest(cr));
    measure.add_segment(seg);
    if let Some(clef) = clef {
        let mut clef_seg = Segment::new(SegmentType::Clef, measure.start_tick, 1);
        clef_seg.set_element(track, SegmentItem::Clef(Clef::new(clef)));
        measure.add_segment(clef_seg);
    }
    score.add_measure(measure);
}

/// MuseScore range le changement de clef dans les segments de la mesure
/// **précédente**, au tick de sa barre finale. Le writer ne doit pas l'écrire
/// avant la dernière note de cette mesure, sinon l'aller-retour transpose
/// cette note d'un octave (`Sancta_Maria`, mesure 48).
#[test]
fn clef_octave_en_fin_de_mesure_precedente() {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(score.staff(staff).id, 0);
    mesure(&mut score, 0, 52, Some(ClefType::F), track);
    let clef = measure_clef_at_end(1, ClefType::G8Vb, track);
    mesure(&mut score, 1, 52, None, track);
    add_end_clef(&mut score, clef);

    let abc = String::from_utf8(
        scoretk::write(&score, scoretk::Format::Abc, &scoretk::Options::default()).unwrap(),
    )
    .unwrap();
    let back = scoretk::read(abc.as_bytes(), scoretk::Format::Abc).unwrap();
    assert_eq!(
        pitches(&back),
        vec![52, 52],
        "la dernière note de la mesure 1 ne doit pas changer d'octave :\n{abc}"
    );
}

/// Construit le segment de clef à la fin de la mesure `index`.
fn measure_clef_at_end(index: i64, clef: ClefType, track: Track) -> Segment {
    let end = Fraction::from_integer(index);
    let mut segment = Segment::new(SegmentType::Clef, end, 1);
    segment.set_element(track, SegmentItem::Clef(Clef::new(clef)));
    segment
}

fn add_end_clef(score: &mut Score, segment: Segment) {
    score.measures[0].add_segment(segment);
}

#[test]
fn clef_octave_au_debut_de_mesure() {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(score.staff(staff).id, 0);
    mesure(&mut score, 0, 52, Some(ClefType::F), track);
    mesure(&mut score, 1, 52, Some(ClefType::G8Va), track);

    let abc = String::from_utf8(
        scoretk::write(&score, scoretk::Format::Abc, &scoretk::Options::default()).unwrap(),
    )
    .unwrap();
    let back = scoretk::read(abc.as_bytes(), scoretk::Format::Abc).unwrap();
    assert_eq!(
        pitches(&back),
        vec![52, 52],
        "la note au changement de clef perd un octave :\n{abc}"
    );
}
