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

//! LilyPond 2.26 §27.2 : `#(set-global-staff-size n` note le spatium ; les
//! tailles des styles de texte, sans champ `\paper` normalisé (§26.6), sont
//! portées par des champs `\header` privés.

use scoretk::model::{
    Annotation, Dynamic, Fraction, Measure, Note, NoteHeadGroup, Part, RichText, Score, Segment,
    SegmentAnnotation, SegmentItem, SegmentType, TboxText, Track,
};
use scoretk::{read, write, Format, Options};

#[test]
fn taille_de_portee_et_fonts_de_titre_vont_et_reviennent_par_ly() {
    let mut source = Score::new();
    let part = source.add_part(Part::new("Piano", "Pno"));
    source.add_staff(part);
    source.add_measure(Measure::new(0, Fraction::ZERO, Fraction::ONE));
    source.engraving.style.spatium_mm = 1.76389;
    source.engraving.style.title_font_size = 24.0;
    source.engraving.style.composer_font_size = 12.0;
    source.engraving.style.lyricist_font_size = 12.0;

    let ly = write(&source, Format::Ly, &Options::default()).expect("écriture LilyPond");
    let text = String::from_utf8(ly.clone()).expect("UTF-8");
    assert!(text.contains("#(set-global-staff-size "), "{text}");
    assert!(
        text.contains("scoretkTitleFontSize = \"24.00000\""),
        "{text}"
    );

    let back = read(&ly, Format::Ly).expect("lecture LilyPond");
    assert!(
        (back.engraving.style.spatium_mm - 1.76389).abs() < 1e-4,
        "spatium lu = {}, texte = {text}",
        back.engraving.style.spatium_mm
    );
    assert_eq!(back.engraving.style.title_font_size, 24.0);
    assert_eq!(back.engraving.style.composer_font_size, 12.0);
    assert_eq!(back.engraving.style.lyricist_font_size, 12.0);
}

#[test]
fn credits_et_textes_de_cadre_vont_et_reviennent_par_ly() {
    let mut source = Score::new();
    let part = source.add_part(Part::new("Piano", "Pno"));
    source.add_staff(part);
    source.add_measure(Measure::new(0, Fraction::ZERO, Fraction::ONE));
    source.add_credit(scoretk::model::Credit::new(
        RichText::new("Crédit de cadre"),
        scoretk::model::CreditStyle::Title,
    ));
    source.add_tbox_text(TboxText::new(RichText::new("Encart"), 0));

    let ly = write(&source, Format::Ly, &Options::default()).expect("écriture LilyPond");
    let text = String::from_utf8(ly.clone()).expect("UTF-8");
    assert!(
        text.contains("scoretkCreditA = \"Crédit de cadre\""),
        "{text}"
    );
    assert!(text.contains("scoretkTboxA = \"0|Encart\""), "{text}");
    let back = read(&ly, Format::Ly).expect("lecture LilyPond");
    assert_eq!(back.engraving.credits.len(), 1);
    assert_eq!(back.engraving.credits[0].text.plain(), "Crédit de cadre");
    assert_eq!(back.engraving.tbox_texts.len(), 1);
    assert_eq!(back.engraving.tbox_texts[0].text.plain(), "Encart");
}

#[test]
fn groupes_de_tetes_de_note_vont_et_reviennent_par_ly() {
    let mut source = Score::new();
    let part = source.add_part(Part::new("Piano", "Pno"));
    let staff = source.add_staff(part);
    // Seuls les groupes pourvus d'un style LilyPond standard (§B.9) sont
    // transportés sans markup privé ; `HeavyCrossHat` est documenté hors
    // déclaration (§52.3).
    for (index, group) in [NoteHeadGroup::Cross, NoteHeadGroup::Do]
        .into_iter()
        .enumerate()
    {
        let mut note = Note::new(60 + index as u8, 14);
        note.head_group = group;
        let id = source.add_note(note);
        let mut rest = scoretk::model::ChordRest::chord(scoretk::model::Duration::whole());
        if let scoretk::model::ChordRestKind::Chord(chord) = &mut rest.kind {
            chord.notes.push(id);
        }
        let mut segment = Segment::new(SegmentType::ChordRest, Fraction::new(index as i64, 2), 1);
        segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(rest));
        let mut measure = Measure::new(index, Fraction::new(index as i64, 2), Fraction::ONE);
        measure.add_segment(segment);
        source.add_measure(measure);
    }

    let ly = write(&source, Format::Ly, &Options::default()).expect("écriture LilyPond");
    let back = read(&ly, Format::Ly).expect("lecture LilyPond");
    let groups: Vec<NoteHeadGroup> = back
        .notes
        .iter()
        .map(|note| note.head_group.clone())
        .collect();
    assert_eq!(groups, [NoteHeadGroup::Cross, NoteHeadGroup::Do]);
}

#[test]
fn velocite_de_nuance_vont_et_revient_par_ly() {
    let mut source = Score::new();
    let part = source.add_part(Part::new("Piano", "Pno"));
    let staff = source.add_staff(part);
    let note = source.add_note(scoretk::model::Note::new(60, 14));
    let mut rest = scoretk::model::ChordRest::chord(scoretk::model::Duration::whole());
    if let scoretk::model::ChordRestKind::Chord(chord) = &mut rest.kind {
        chord.notes.push(note);
    }
    let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
    segment.set_element(
        Track::new(staff, 0),
        scoretk::model::SegmentItem::ChordRest(rest),
    );
    let mut dynamic = Dynamic::new("p");
    dynamic.velocity = Some(96);
    segment.annotations.push(SegmentAnnotation::new(
        Track::new(staff, 0),
        Annotation::Dynamic(dynamic),
    ));
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    measure.add_segment(segment);
    source.add_measure(measure);

    let ly = write(&source, Format::Ly, &Options::default()).expect("écriture LilyPond");
    let text = String::from_utf8(ly.clone()).expect("UTF-8");
    assert!(
        text.contains("Staff.midiMinimumVolume = #0.755906"),
        "{text}"
    );
    let back = read(&ly, Format::Ly).expect("lecture LilyPond");
    let velocities: Vec<Option<u8>> = back.measures[0]
        .segments
        .iter()
        .flat_map(|segment| segment.annotations.iter())
        .filter_map(|entry| match &entry.annotation {
            Annotation::Dynamic(dynamic) => Some(dynamic.velocity),
            _ => None,
        })
        .collect();
    assert_eq!(velocities, [Some(96)]);
}
