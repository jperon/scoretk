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

//! Silences des mesures surchargées d'Anerio, Byrd, Perosi et Vivaldi.
//!
//! Référence : `xml2abc.py` 177 `doNote` (lignes 1208, 1229) lit la `<duration>`
//! du MusicXML telle quelle, `abcdur` (lignes 501-521) la convertit en
//! multiplicateur ABC sans la borner à `M:`, et `sortMeasure` (lignes 714-769)
//! pose les événements à leur temps absolu, écarte un silence chevauchant une
//! note (ligne 740) et raccourcit un silence chevauché (lignes 742-750).
//! ABC 2.1 §4.3/§4.5 : `x`/`z` acceptent un multiplicateur arbitraire.

use scoretk_core::model::*;

fn write_and_read(score: &Score) -> (String, Score) {
    let abc = String::from_utf8(
        scoretk::write(score, scoretk::Format::Abc, &scoretk::Options::default()).unwrap(),
    )
    .unwrap();
    let back = scoretk::read(abc.as_bytes(), scoretk::Format::Abc).unwrap();
    (abc, back)
}

fn first_note_tick(score: &Score) -> Fraction {
    score
        .measures
        .iter()
        .flat_map(|measure| {
            measure.segments.iter().filter_map(move |segment| {
                segment
                    .chord_rest(Track::new(score.staves[0].id, 0))
                    .filter(|cr| cr.chord_ref().is_some())
                    // Le lecteur ABC range des ticks absolus dans les segments.
                    .map(|_| segment.tick)
            })
        })
        .next()
        .expect("aucune note après l'aller-retour")
}

fn rest(visible: bool, len: Fraction) -> ChordRest {
    let mut cr = ChordRest::rest(Duration::whole());
    if let ChordRestKind::Rest(r) = &mut cr.kind {
        r.visible = visible;
    }
    cr.ticks = Some(len);
    cr
}

fn note(score: &mut Score, pitch: u8) -> ChordRest {
    let id = score.add_note(Note::new(pitch, scoretk_core::model::tpc::C));
    let mut cr = ChordRest::chord(Duration::quarter());
    cr.chord_mut().unwrap().notes.push(id);
    cr
}

#[test]
fn silence_invisible_suivie_d_une_note_garde_sa_duree() {
    // Perosi, voix 7, mesure 7 : un silence invisible d'une ronde (4 temps)
    // dans une mesure 3/4, puis un ré qui commence après ce silence. Le
    // tronquer à `x6` avance ce ré de 2 temps et décale toutes les voix.
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(score.staff(staff).id, 0);
    let mut m = Measure::new(0, Fraction::ZERO, Fraction::new(3, 4));
    let mut seg = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
    seg.set_element(track, SegmentItem::ChordRest(rest(false, Fraction::ONE)));
    m.add_segment(seg);
    let mut seg = Segment::new(SegmentType::ChordRest, Fraction::ONE, 2);
    seg.set_element(track, SegmentItem::ChordRest(note(&mut score, 62)));
    m.add_segment(seg);
    score.add_measure(m);
    let mut m = Measure::new(1, Fraction::new(3, 4), Fraction::ONE);
    let mut seg = Segment::new(SegmentType::ChordRest, Fraction::new(3, 4), 1);
    seg.set_element(track, SegmentItem::ChordRest(note(&mut score, 64)));
    m.add_segment(seg);
    score.add_measure(m);

    let (abc, back) = write_and_read(&score);
    assert!(abc.contains("x8"), "silence tronqué :\n{abc}");
    assert_eq!(
        first_note_tick(&back),
        Fraction::ONE,
        "attaque avancée :\n{abc}"
    );
}

#[test]
fn pause_de_mesure_seule_garde_la_longueur_de_la_barre() {
    // Anerio, voix 2, mesure 1 : une pause de mesure dont le modèle porte deux
    // rondes dans une mesure 4/4. L'écrire `z16` allongerait la barre et
    // décalerait toutes les autres voix de 4 temps (xml2abc.py::abcdur ne borne
    // pas la durée, mais le pivot a ici des horloges de parties distinctes).
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(score.staff(staff).id, 0);
    let mut m = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    let mut cr = ChordRest::full_measure_rest();
    cr.ticks = Some(Fraction::from_integer(2));
    let mut seg = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
    seg.set_element(track, SegmentItem::ChordRest(cr));
    m.add_segment(seg);
    score.add_measure(m);
    let mut m = Measure::new(1, Fraction::ONE, Fraction::ONE);
    let mut seg = Segment::new(SegmentType::ChordRest, Fraction::ONE, 1);
    seg.set_element(track, SegmentItem::ChordRest(note(&mut score, 60)));
    m.add_segment(seg);
    score.add_measure(m);

    let (abc, back) = write_and_read(&score);
    assert!(abc.contains("z8 |"), "barre allongée :\n{abc}");
    assert_eq!(first_note_tick(&back), Fraction::ONE, "\n{abc}");
}

#[test]
fn silence_chevauchant_une_note_est_ecarte() {
    // Vivaldi, voix 7 : une note de 4 temps et une pause invisible de 12 temps
    // qui la chevauche. `sortMeasure` (ligne 740) écarte la pause ; l'écrire
    // `x24` allonge la mesure de 12 temps et décale toutes les voix.
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(score.staff(staff).id, 0);
    let mut m = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    let mut seg = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
    seg.set_element(track, SegmentItem::ChordRest(note(&mut score, 61)));
    m.add_segment(seg);
    let mut seg = Segment::new(SegmentType::ChordRest, Fraction::new(1, 2), 2);
    seg.set_element(
        track,
        SegmentItem::ChordRest(rest(false, Fraction::from_integer(1))),
    );
    m.add_segment(seg);
    score.add_measure(m);
    let mut m = Measure::new(1, Fraction::ONE, Fraction::ONE);
    let mut seg = Segment::new(SegmentType::ChordRest, Fraction::ONE, 1);
    seg.set_element(track, SegmentItem::ChordRest(note(&mut score, 62)));
    m.add_segment(seg);
    score.add_measure(m);

    let (abc, back) = write_and_read(&score);
    assert!(!abc.contains("x8"), "pause chevauchante écrite :\n{abc}");
    assert_eq!(first_note_tick(&back), Fraction::ZERO, "\n{abc}");
    assert_eq!(back.measures[0].len, Fraction::ONE, "\n{abc}");
}

#[test]
fn silence_chevauchee_par_une_note_est_raccourcie() {
    // Byrd, voix 1, mesure 1 : deux silences invisibles d'un temps, le second
    // commençant à 1/2 temps. `sortMeasure` (lignes 742-750) raccourcit le
    // premier à 1/2 temps ; l'écrire deux fois `x2` décale la mesure suivante.
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(score.staff(staff).id, 0);
    let mut m = Measure::new(0, Fraction::ZERO, Fraction::new(3, 2));
    m.nominal_len = Fraction::ONE;
    for (index, tick) in [Fraction::ZERO, Fraction::new(1, 2)]
        .into_iter()
        .enumerate()
    {
        let mut seg = Segment::new(SegmentType::ChordRest, tick, index + 1);
        seg.set_element(
            track,
            SegmentItem::ChordRest(rest(false, Fraction::new(1, 1))),
        );
        m.add_segment(seg);
    }
    score.add_measure(m);
    let mut m = Measure::new(1, Fraction::new(3, 2), Fraction::ONE);
    let mut seg = Segment::new(SegmentType::ChordRest, Fraction::new(3, 2), 1);
    seg.set_element(track, SegmentItem::ChordRest(note(&mut score, 71)));
    m.add_segment(seg);
    score.add_measure(m);

    let (abc, back) = write_and_read(&score);
    assert_eq!(first_note_tick(&back), Fraction::new(3, 2), "\n{abc}");
}
