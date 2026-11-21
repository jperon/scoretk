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

//! Tests d'export : partitions construites à la main sur le modèle.
//!
//! Les comparaisons au corpus (`tests/corpus/v4`) ignorent ce que le modèle ne
//! porte pas : en-tête `<identification>`, `<work>`, `<defaults>`, `<credit>`,
//! blocs `<print>`, attributs de mise en page et hampes calculées.

use scoretk_core::model::{
    tpc, Accidental, AccidentalType, Annotation, Articulation, ArticulationType, BarLineType,
    BeamMode, ChordRest, Clef, ClefType, Credit, CreditStyle, Duration, DurationType, Fraction,
    GraceChord, GraceType, Harmony, Instrument, InstrumentChange, Key, KeyMode, KeySig, Lyrics,
    LyricsSyllabic, Measure, MeasureRepeatMark, Note, NoteHeadGroup, NoteId, Part, PartId,
    Placement, RehearsalFrame, RehearsalMark, RichText, Score, Segment, SegmentItem, SegmentType,
    Spanner, SpannerAnchor, SpannerType, Staff, StaffBracket, StaffId, StaffText, SwingParameters,
    SwingUnit, SystemText, TboxText, Tempo, TempoKind, TextLineInfo, TimeSig, Track, Transpose,
    Tuplet,
};

use super::{export_musicxml, export_musicxml_with, ExportOptions};

// ----------------------------------------------------------------------
// Utilitaires de construction
// ----------------------------------------------------------------------

fn ajouter_mesure(number: usize, start: Fraction, nominal: Fraction, len: Fraction) -> Measure {
    let mut measure = Measure::new(number, start, nominal);
    measure.len = len;
    measure
}

/// Ajoute un accord ou un silence prêt à l'emploi.
fn ajouter_element(
    score: &mut Score,
    measure: &mut Measure,
    track: Track,
    tick: Fraction,
    chord_rest: ChordRest,
) {
    let mut segment = Segment::new(SegmentType::ChordRest, tick, score.staves.len());
    segment.set_element(track, SegmentItem::ChordRest(chord_rest));
    measure.add_segment(segment);
}

/// Ajoute un accord et renvoie les identifiants de ses notes.
fn ajouter_accord(
    score: &mut Score,
    measure: &mut Measure,
    track: Track,
    tick: Fraction,
    duration: Duration,
    notes: &[(u8, i8)],
) -> Vec<NoteId> {
    let ids: Vec<NoteId> = notes
        .iter()
        .map(|(pitch, tpc)| score.add_note(Note::new(*pitch, *tpc)))
        .collect();
    let mut chord_rest = ChordRest::chord(duration);
    chord_rest
        .chord_mut()
        .expect("accord")
        .notes
        .extend(ids.iter().copied());
    ajouter_element(score, measure, track, tick, chord_rest);
    ids
}

fn ajouter_silence(
    score: &mut Score,
    measure: &mut Measure,
    track: Track,
    tick: Fraction,
    duration: Duration,
) {
    ajouter_element(score, measure, track, tick, ChordRest::rest(duration));
}

fn ajouter_silence_mesure(score: &mut Score, measure: &mut Measure, track: Track, tick: Fraction) {
    ajouter_element(score, measure, track, tick, ChordRest::full_measure_rest());
}

fn ajouter_clef(score: &mut Score, measure: &mut Measure, track: Track, clef: ClefType) {
    let mut segment = Segment::new(
        SegmentType::HeaderClef,
        measure.start_tick,
        score.staves.len(),
    );
    segment.set_element(track, SegmentItem::Clef(Clef::new(clef)));
    measure.add_segment(segment);
}

fn ajouter_armure(
    score: &mut Score,
    measure: &mut Measure,
    track: Track,
    fifths: i8,
    mode: KeyMode,
) {
    let mut segment = Segment::new(SegmentType::KeySig, measure.start_tick, score.staves.len());
    let mut keysig = KeySig::new(Key::new(fifths));
    keysig.mode = mode;
    segment.set_element(track, SegmentItem::KeySig(keysig));
    measure.add_segment(segment);
}

fn ajouter_indication_mesure(
    score: &mut Score,
    measure: &mut Measure,
    track: Track,
    numerator: i32,
    denominator: i32,
) {
    let mut segment = Segment::new(SegmentType::TimeSig, measure.start_tick, score.staves.len());
    segment.set_element(
        track,
        SegmentItem::TimeSig(TimeSig::new(numerator, denominator)),
    );
    measure.add_segment(segment);
}

/// Crée une partie piano à une portée avec un instrument complet.
fn piano(score: &mut Score) -> (PartId, StaffId) {
    let part_id = score.add_part(Part::new("Piano", "Pno."));
    score.part_mut(part_id).instrument = piano_instrument();
    let staff_id = score.add_staff(part_id);
    (part_id, staff_id)
}

fn piano_instrument() -> Instrument {
    Instrument {
        id: "keyboard.piano".to_owned(),
        long_name: "Piano".to_owned(),
        short_name: "Pno.".to_owned(),
        track_name: "Piano".to_owned(),
        transpose: None,
        midi_channel: None,
        channel_count: 1,
        midi_port: None,
        midi_program: Some(0),
        volume: None,
        pan: None,
        midi_articulations: Vec::new(),
    }
}

// ----------------------------------------------------------------------
// Tests unitaires
// ----------------------------------------------------------------------

#[test]
fn partition_vide() {
    let score = Score::new();
    let xml = export_musicxml(&score).expect("export");

    assert!(xml.starts_with(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \
         \"http://www.musicxml.org/dtds/partwise.dtd\">\n\
         <score-partwise version=\"4.0\">\n"
    ));
    assert!(xml.contains("<part-list>\n    </part-list>"));
    assert!(xml.ends_with("</score-partwise>\n"));
}

#[test]
fn quatre_noires() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_armure(&mut score, &mut measure, track, 0, KeyMode::Major);
    ajouter_indication_mesure(&mut score, &mut measure, track, 4, 4);
    for (index, (pitch, tpc)) in [(60_u8, tpc::C), (62, tpc::D), (64, tpc::E), (65, tpc::F)]
        .into_iter()
        .enumerate()
    {
        ajouter_accord(
            &mut score,
            &mut measure,
            track,
            Fraction::new(index as i64, 4),
            Duration::quarter(),
            &[(pitch, tpc)],
        );
    }
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");

    assert!(xml.contains("<divisions>1</divisions>"));
    assert!(xml.contains("<fifths>0</fifths>"));
    assert!(xml.contains("<mode>major</mode>"));
    assert!(xml.contains("<beats>4</beats>"));
    assert!(xml.contains("<beat-type>4</beat-type>"));
    assert!(xml.contains("<clef>\n          <sign>G</sign>"));
    assert_eq!(xml.matches("<type>quarter</type>").count(), 4);
    assert_eq!(xml.matches("<duration>1</duration>").count(), 4);
    assert!(xml.contains("<step>C</step>\n          <octave>4</octave>"));
    assert!(xml.contains("<step>F</step>\n          <octave>4</octave>"));
    assert_eq!(xml.matches("<voice>1</voice>").count(), 4);
}

#[test]
fn divisions_et_triolet() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);

    ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        Duration::quarter(),
        &[(60, tpc::C)],
    );

    let tuplet = score.add_tuplet(Tuplet::new(
        Fraction::new(3, 2),
        Fraction::new(1, 8),
        Fraction::QUARTER,
        Fraction::QUARTER,
    ));

    for index in 0..3 {
        let note = score.add_note(Note::new(62 + index as u8, tpc::D));
        let mut chord_rest = ChordRest::chord(Duration::eighth());
        chord_rest.tuplet = Some(tuplet);
        chord_rest.chord_mut().expect("accord").notes.push(note);
        ajouter_element(
            &mut score,
            &mut measure,
            track,
            Fraction::QUARTER + Fraction::new(index, 12),
            chord_rest,
        );
    }
    ajouter_silence_mesure(
        &mut score,
        &mut measure,
        track,
        Fraction::HALF + Fraction::QUARTER,
    );
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");

    // PPCM(4, 12) / 4 = 3.
    assert!(xml.contains("<divisions>3</divisions>"));
    assert!(xml.contains("<actual-notes>3</actual-notes>"));
    assert!(xml.contains("<normal-notes>2</normal-notes>"));
    assert!(xml.contains("<tuplet type=\"start\" bracket=\"no\"/>"));
    assert!(xml.contains("<tuplet type=\"stop\"/>"));
    assert_eq!(xml.matches("<type>eighth</type>").count(), 3);
    assert!(xml.contains("<duration>1</duration>"));
}

#[test]
fn deux_voix_backup() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let first = Track::new(staff, 0);
    let second = Track::new(staff, 1);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, first, ClefType::G);
    ajouter_accord(
        &mut score,
        &mut measure,
        first,
        Fraction::ZERO,
        Duration::half(),
        &[(72, tpc::C)],
    );
    ajouter_silence(
        &mut score,
        &mut measure,
        first,
        Fraction::HALF,
        Duration::half(),
    );
    ajouter_silence_mesure(&mut score, &mut measure, second, Fraction::ZERO);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<backup>\n        <duration>4</duration>\n        </backup>"));
    assert!(xml.contains("<voice>1</voice>"));
    assert!(xml.contains("<voice>2</voice>"));
}

#[test]
fn partie_a_deux_portees() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Piano", "Pno."));
    score.part_mut(part_id).instrument = piano_instrument();
    let first = score.add_staff(part_id);
    let second = score.add_staff_with(part_id, Staff::new(part_id));
    // Accolade implicite du piano, comme l'instrument du corpus.
    score.staff_mut(first).brackets.push(StaffBracket {
        kind: 1,
        span: 2,
        visible: true,
    });
    let first_track = Track::new(first, 0);
    let second_track = Track::new(second, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    let mut clefs = Segment::new(SegmentType::HeaderClef, Fraction::ZERO, score.staves.len());
    clefs.set_element(first_track, SegmentItem::Clef(Clef::new(ClefType::G)));
    clefs.set_element(second_track, SegmentItem::Clef(Clef::new(ClefType::F)));
    measure.add_segment(clefs);
    ajouter_silence_mesure(&mut score, &mut measure, first_track, Fraction::ZERO);
    ajouter_silence_mesure(&mut score, &mut measure, second_track, Fraction::ZERO);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<staves>2</staves>"));
    assert!(xml.contains("<clef number=\"1\">"));
    assert!(xml.contains("<clef number=\"2\">"));
    assert!(xml.contains("<sign>F</sign>"));
    assert!(xml.contains("<staff>1</staff>"));
    assert!(xml.contains("<staff>2</staff>"));
    assert!(xml.contains("<voice>1</voice>"));
    assert!(xml.contains("<voice>5</voice>"));
}

#[test]
fn liaisons_de_prolongation() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    let first = ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        Duration::half(),
        &[(71, tpc::B)],
    );
    let second = ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::HALF,
        Duration::half(),
        &[(71, tpc::B)],
    );
    score.note_mut(first[0]).tie_forward = Some(second[0]);
    score.note_mut(second[0]).tie_back = Some(first[0]);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<tie type=\"start\"/>"));
    assert!(xml.contains("<tie type=\"stop\"/>"));
    assert!(xml.contains("<tied type=\"start\"/>"));
    assert!(xml.contains("<tied type=\"stop\"/>"));
    assert!(xml.contains("<notations>\n          <tied type=\"start\"/>"));
}

#[test]
fn paroles_et_accords_d_agrement() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);

    let main_note = score.add_note(Note::new(69, tpc::A));
    let grace_note = score.add_note(Note::new(67, tpc::G));

    let mut chord_rest = ChordRest::chord(Duration::quarter());
    chord_rest
        .lyrics
        .push(Lyrics::new(0, LyricsSyllabic::Single, "Do"));
    {
        let chord = chord_rest.chord_mut().expect("accord");
        chord.notes.push(main_note);
        let mut grace = GraceChord::new(GraceType::Appoggiatura, Duration::eighth());
        grace.chord_mut().notes.push(grace_note);
        chord.grace_chords.push(grace);
    }
    ajouter_element(&mut score, &mut measure, track, Fraction::ZERO, chord_rest);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<grace/>"));
    assert!(xml.contains("<lyric number=\"1\">"));
    assert!(xml.contains("<syllabic>single</syllabic>"));
    assert!(xml.contains("<text>Do</text>"));
    // L'accord d'agrément n'a pas de durée.
    assert_eq!(xml.matches("<duration>1</duration>").count(), 1);
}

#[test]
fn placement_des_paroles_par_defaut_et_au_dessus() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);

    let first = score.add_note(Note::new(69, tpc::A));
    let second = score.add_note(Note::new(71, tpc::B));
    let mut chord_rest = ChordRest::chord(Duration::half());
    chord_rest
        .lyrics
        .push(Lyrics::new(0, LyricsSyllabic::Single, "Do"));
    let mut above = Lyrics::new(1, LyricsSyllabic::Single, "Ré");
    above.placement = Placement::Above;
    chord_rest.lyrics.push(above);
    {
        let chord = chord_rest.chord_mut().expect("accord");
        chord.notes.push(first);
    }
    ajouter_element(&mut score, &mut measure, track, Fraction::ZERO, chord_rest);

    let mut second_measure = ajouter_mesure(1, Fraction::ONE, Fraction::ONE, Fraction::ONE);
    let mut other = ChordRest::chord(Duration::half());
    other.lyrics.push(Lyrics::new(0, LyricsSyllabic::End, "mi"));
    if let Some(chord) = other.chord_mut() {
        chord.notes.push(second);
    }
    ajouter_element(
        &mut score,
        &mut second_measure,
        track,
        Fraction::ZERO,
        other,
    );
    score.add_measure(measure);
    score.add_measure(second_measure);

    let xml = export_musicxml(&score).expect("export");
    // Le premier couplet est au-dessous (défaut) : aucun attribut placement.
    assert!(xml.contains("<lyric number=\"1\">"));
    assert!(!xml.contains("<lyric number=\"1\" placement"));
    // Le second est explicitement au-dessus.
    assert!(xml.contains("<lyric number=\"2\" placement=\"above\">"));
}

#[test]
fn defaults_de_page_et_credits_positionnes() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);
    score.add_credit(Credit::new(RichText::new("Titre"), CreditStyle::Title));
    score.add_credit(Credit::new(
        RichText::new("Compositrice"),
        CreditStyle::Composer,
    ));
    score.metadata.copyright = "© 2026".to_owned();

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<defaults>\n    <scaling>"));
    assert!(xml.contains("<millimeters>7</millimeters>"));
    assert!(xml.contains("<tenths>40</tenths>"));
    assert!(xml.contains("<page-height>1696.72</page-height>"));
    assert!(xml.contains("<page-width>1200.33</page-width>"));
    assert!(xml.contains(
        "<credit-words default-x=\"600.17\" default-y=\"1611.01\" justify=\"center\" \
         valign=\"top\" font-size=\"22\">Titre</credit-words>"
    ));
    assert!(xml.contains(
        "<credit-words default-x=\"1114.62\" default-y=\"1511.01\" justify=\"right\" \
         valign=\"bottom\">Compositrice</credit-words>"
    ));
    assert!(xml.contains(
        "<credit-words default-x=\"600.17\" default-y=\"85.71\" justify=\"center\" \
         valign=\"bottom\" font-size=\"8\">© 2026</credit-words>"
    ));
}

#[test]
fn mesure_irreguliere_armure_et_reprise() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    // Anacrouse de 3/4 dans une mesure nominale de 4/4.
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::new(3, 4));
    ajouter_clef(&mut score, &mut measure, track, ClefType::F);
    ajouter_armure(&mut score, &mut measure, track, -2, KeyMode::Minor);
    ajouter_indication_mesure(&mut score, &mut measure, track, 4, 4);
    for index in 0..3 {
        ajouter_accord(
            &mut score,
            &mut measure,
            track,
            Fraction::new(index, 4),
            Duration::quarter(),
            &[(43, tpc::G)],
        );
    }
    score.add_measure(measure);

    let mut measure = ajouter_mesure(1, Fraction::new(3, 4), Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::new(3, 4));
    measure.end_bar_line = BarLineType::EndRepeat;
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<fifths>-2</fifths>"));
    assert!(xml.contains("<mode>minor</mode>"));
    assert!(xml.contains("<sign>F</sign>"));
    assert!(xml.contains("<bar-style>light-heavy</bar-style>"));
    assert!(xml.contains("<repeat direction=\"backward\"/>"));
    assert!(xml.contains("<measure number=\"2\">"));
}

#[test]
fn n_olets_imbriques() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::HALF, Fraction::HALF);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);

    let mut outer = Tuplet::new(
        Fraction::new(3, 2),
        Fraction::new(1, 4),
        Fraction::ZERO,
        Fraction::HALF,
    );
    outer.bracket_explicit = true; // ce test porte sur l'imbrication, pas sur le crochet
    let outer = score.add_tuplet(outer);
    let inner = score.add_tuplet({
        let mut tuplet = Tuplet::new(
            Fraction::new(5, 4),
            Fraction::new(1, 16),
            Fraction::ZERO,
            Fraction::QUARTER,
        );
        tuplet.parent = Some(outer);
        tuplet.bracket_explicit = true;
        tuplet
    });

    // Cinq double-croches du n-olet interne : 1/16 / (5/4) / (3/2) = 1/30.
    for index in 0..5 {
        let note = score.add_note(Note::new(60, tpc::C));
        let mut chord_rest = ChordRest::chord(Duration::new(DurationType::Sixteenth));
        chord_rest.tuplet = Some(inner);
        chord_rest.chord_mut().expect("accord").notes.push(note);
        ajouter_element(
            &mut score,
            &mut measure,
            track,
            Fraction::new(index, 30),
            chord_rest,
        );
    }
    // Deux noires du n-olet externe : 1/4 / (3/2) = 1/6.
    for index in 1..=2 {
        let note = score.add_note(Note::new(62, tpc::D));
        let mut chord_rest = ChordRest::chord(Duration::quarter());
        chord_rest.tuplet = Some(outer);
        chord_rest.chord_mut().expect("accord").notes.push(note);
        ajouter_element(
            &mut score,
            &mut measure,
            track,
            Fraction::new(index, 6),
            chord_rest,
        );
    }
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<tuplet type=\"start\" number=\"1\" bracket=\"no\">"));
    assert!(xml.contains("<tuplet type=\"start\" number=\"2\" bracket=\"no\">"));
    assert!(xml.contains("<tuplet-number>3</tuplet-number>"));
    assert!(xml.contains("<tuplet-number>5</tuplet-number>"));
    assert!(xml.contains("<actual-notes>15</actual-notes>"));
    assert!(xml.contains("<normal-notes>8</normal-notes>"));
    assert!(xml.contains("<tuplet type=\"stop\" number=\"2\"/>"));
    assert!(xml.contains("<tuplet type=\"stop\" number=\"1\"/>"));
    assert!(xml.contains("<tuplet-type>16th</tuplet-type>"));
    assert!(xml.contains("<tuplet-type>quarter</tuplet-type>"));
}

#[test]
fn accords_chiffres_nuances_et_textes() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        Duration::whole(),
        &[(60, tpc::C)],
    );
    let segment = measure.segments.last_mut().expect("segment");
    let mut harmony = Harmony::new("Cm7");
    harmony.root_tpc = tpc::C;
    harmony.name = "m7".to_owned();
    harmony.kind = "normal".to_owned();
    segment.add_annotation(track, Annotation::Harmony(harmony));
    let mut dynamic = scoretk_core::model::Dynamic::new("mf");
    dynamic.velocity = Some(80);
    segment.add_annotation(track, Annotation::Dynamic(dynamic));
    segment.add_annotation(
        track,
        Annotation::StaffText(scoretk_core::model::StaffText::new("dolce")),
    );
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<harmony print-frame=\"no\">"));
    assert!(xml.contains("<root-step>C</root-step>"));
    assert!(xml.contains("<kind text=\"m7\">minor-seventh</kind>"));
    assert!(xml.contains("<direction placement=\"below\">"));
    assert!(xml.contains("<mf/>"));
    assert!(xml.contains("<sound dynamics=\"88.89\"/>"));
    assert!(xml.contains("<words>dolce</words>"));
}

#[test]
fn alterations_et_tetes_de_note() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_armure(&mut score, &mut measure, track, 2, KeyMode::Major);

    let ids = ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        Duration::quarter(),
        &[(66, tpc::F_SHARP)],
    );
    score.note_mut(ids[0]).accidental = Some(Accidental::new(AccidentalType::Natural));
    let ids = ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::QUARTER,
        Duration::quarter(),
        &[(65, tpc::F)],
    );
    score.note_mut(ids[0]).head_group = NoteHeadGroup::Cross;

    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<fifths>2</fifths>"));
    assert!(xml.contains("<alter>1</alter>"));
    assert!(xml.contains("<accidental>natural</accidental>"));
    assert!(xml.contains("<notehead>x</notehead>"));
}

#[test]
fn transposition_et_concert_pitch() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Bb Clarinet", "Bb Cl."));
    score.part_mut(part_id).instrument = Instrument {
        id: "wind.reed.clarinet.bflat".to_owned(),
        long_name: "B♭ Clarinet".to_owned(),
        short_name: "Bb Cl.".to_owned(),
        track_name: "B♭ Clarinet".to_owned(),
        transpose: Some(Transpose::new(-1, -2)),
        midi_channel: None,
        channel_count: 1,
        midi_port: None,
        midi_program: Some(71),
        volume: None,
        pan: None,
        midi_articulations: Vec::new(),
    };
    let staff = score.add_staff(part_id);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_armure(&mut score, &mut measure, track, -2, KeyMode::Major);

    let ids = ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        Duration::whole(),
        &[(70, tpc::B_FLAT)],
    );
    // Hauteur écrite : do5 (tpc transposé), sonnant si bémol4.
    score.note_mut(ids[0]).tpc_transposed = tpc::C;
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<transpose>"));
    assert!(xml.contains("<chromatic>-2</chromatic>"));
    assert!(xml.contains("<diatonic>-1</diatonic>"));
    assert!(xml.contains("<step>C</step>\n          <octave>5</octave>"));
    assert!(xml.contains("<part-name print-object=\"no\">Bb Clarinet</part-name>"));

    let concert = ExportOptions {
        concert_pitch: Some(true),
        ..ExportOptions::default()
    };
    let xml = export_musicxml_with(&score, &concert).expect("export");
    assert!(xml.contains("<for-part>"));
    assert!(xml.contains("<part-transpose>"));
    assert!(
        xml.contains("<step>B</step>\n          <alter>-1</alter>\n          <octave>4</octave>")
    );
}

#[test]
fn sortie_deterministe() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        Duration::quarter(),
        &[(60, tpc::C)],
    );
    score.add_measure(measure);

    let first = export_musicxml(&score).expect("export");
    let second = export_musicxml(&score).expect("export");
    assert_eq!(first, second);
}

#[test]
fn archive_mxl() {
    use std::io::Read;

    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        Duration::quarter(),
        &[(60, tpc::C)],
    );
    score.add_measure(measure);

    let bytes = super::export_mxl(&score).expect("export mxl");
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).expect("zip");
    let names: Vec<String> = archive.file_names().map(str::to_owned).collect();
    assert_eq!(names, vec!["META-INF/container.xml", "score.xml"]);

    let mut container = String::new();
    archive
        .by_name("META-INF/container.xml")
        .expect("container")
        .read_to_string(&mut container)
        .expect("lecture");
    assert!(container.contains("<rootfile full-path=\"score.xml\">"));

    let mut musicxml = String::new();
    archive
        .by_name("score.xml")
        .expect("score")
        .read_to_string(&mut musicxml)
        .expect("lecture");
    assert_eq!(musicxml, export_musicxml(&score).expect("export"));

    // Le second export produit exactement les mêmes octets.
    let again = super::export_mxl(&score).expect("export mxl");
    assert_eq!(bytes, again);
}

// ----------------------------------------------------------------------
// Comparaison au corpus MuseScore 4.7
// ----------------------------------------------------------------------

/// Normalise un document MusicXML pour comparaison.
///
/// Supprime les blocs que le modèle ne porte pas (`work`, `identification`,
/// `defaults`, `credit`, `print`), les attributs de mise en page et les
/// hampes calculées.
fn normaliser(xml: &str) -> String {
    const BLOCS: [&str; 5] = ["work", "identification", "defaults", "credit", "print"];
    const ATTRIBUTS: [&str; 6] = [
        " default-x",
        " default-y",
        " relative-x",
        " relative-y",
        " width",
        " justify",
    ];

    let mut result = String::new();
    let mut ignorer: Option<&str> = None;

    for line in xml.lines() {
        let trimmed = line.trim_start();

        if let Some(tag) = ignorer {
            if trimmed.starts_with(&format!("</{tag}")) {
                ignorer = None;
            }
            continue;
        }

        let mut saute = false;
        for tag in BLOCS {
            let open = format!("<{tag}>");
            let open_attributes = format!("<{tag} ");
            if trimmed.starts_with(&open) || trimmed.starts_with(&open_attributes) {
                if !trimmed.contains(&format!("</{tag}>")) {
                    ignorer = Some(tag);
                }
                saute = true;
                break;
            }
        }
        if saute {
            continue;
        }

        if trimmed.starts_with("<stem>") {
            continue;
        }

        let mut line = line.to_owned();
        for attribute in ATTRIBUTS {
            while let Some(position) = line.find(attribute) {
                let value_start = position + attribute.len();
                if !line[value_start..].starts_with("=\"") {
                    break;
                }
                let content_start = value_start + 2;
                let Some(length) = line[content_start..].find('"') else {
                    break;
                };
                line.replace_range(position..content_start + length + 1, "");
            }
        }
        result.push_str(&line);
        result.push('\n');
    }

    result
}

fn comparer_au_corpus(nom: &str, score: &Score) {
    let reference = std::fs::read_to_string(format!(
        "{}/../../tests/corpus/v4/{nom}.ref.musicxml",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("référence du corpus");
    let obtenu = export_musicxml(score).expect("export");
    comparer(nom, &normaliser(&reference), &normaliser(&obtenu));
}

fn comparer(nom: &str, expected: &str, actual: &str) {
    if expected == actual {
        return;
    }
    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    for (index, (left, right)) in expected_lines.iter().zip(actual_lines.iter()).enumerate() {
        if left != right {
            panic!(
                "{nom} : divergence ligne {}\nattendu : {left:?}\nobtenu  : {right:?}",
                index + 1
            );
        }
    }
    panic!(
        "{nom} : nombre de lignes différent ({} attendues, {} obtenues)",
        expected_lines.len(),
        actual_lines.len()
    );
}

#[test]
fn armure_par_defaut_et_forward() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    // Aucune armure explicite : MuseScore écrit do majeur au tick zéro, sans
    // mode. La note commence à la deuxième noire : un <forward> la précède,
    // et un autre comble la fin de mesure.
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::QUARTER,
        Duration::quarter(),
        &[(60, tpc::C)],
    );
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<fifths>0</fifths>"));
    assert!(!xml.contains("<mode>"));
    assert!(xml.contains("<forward>\n        <duration>1</duration>\n        </forward>"));
    assert!(xml.contains("<forward>\n        <duration>2</duration>\n        </forward>"));

    assert_eq!(super::calc_divisions(&score), 1);
}

#[test]
fn reprises_et_barre_mediane() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    measure.start_bar_line = BarLineType::StartRepeat;
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        Duration::half(),
        &[(60, tpc::C)],
    );
    let mut segment = Segment::new(SegmentType::BarLine, Fraction::HALF, score.staves.len());
    segment.set_element(
        track,
        SegmentItem::BarLine(scoretk_core::model::BarLine::new(BarLineType::Double)),
    );
    measure.add_segment(segment);
    ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::HALF,
        Duration::half(),
        &[(62, tpc::D)],
    );
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains(
        "<barline location=\"left\">\n        <bar-style>heavy-light</bar-style>\n        <repeat direction=\"forward\"/>\n        </barline>"
    ));
    assert!(xml.contains(
        "<barline location=\"middle\">\n        <bar-style>light-light</bar-style>\n        </barline>"
    ));
}

#[test]
fn divisions_calculees() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        Duration::half(),
        &[(60, tpc::C)],
    );
    let tuplet = score.add_tuplet(Tuplet::new(
        Fraction::new(3, 2),
        Fraction::new(1, 8),
        Fraction::HALF,
        Fraction::QUARTER,
    ));
    for index in 0..3 {
        let note = score.add_note(Note::new(64, tpc::E));
        let mut chord_rest = ChordRest::chord(Duration::eighth());
        chord_rest.tuplet = Some(tuplet);
        chord_rest.chord_mut().expect("accord").notes.push(note);
        ajouter_element(
            &mut score,
            &mut measure,
            track,
            Fraction::HALF + Fraction::new(index, 12),
            chord_rest,
        );
    }
    ajouter_silence_mesure(
        &mut score,
        &mut measure,
        track,
        Fraction::HALF + Fraction::QUARTER,
    );
    score.add_measure(measure);

    // PPCM(4, 12) / 4 = 3.
    assert_eq!(super::calc_divisions(&score), 3);
}

#[test]
fn deux_parties_et_durees_pointees() {
    let mut score = Score::new();

    let first_part = score.add_part(Part::new("Piano 1", "Pno. 1"));
    score.part_mut(first_part).instrument = piano_instrument();
    let first_staff = score.add_staff(first_part);
    let first_track = Track::new(first_staff, 0);

    let second_part = score.add_part(Part::new("Piano 2", "Pno. 2"));
    score.part_mut(second_part).instrument = piano_instrument();
    let second_staff = score.add_staff(second_part);
    let second_track = Track::new(second_staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, first_track, ClefType::G);
    ajouter_clef(&mut score, &mut measure, second_track, ClefType::F);
    ajouter_armure(&mut score, &mut measure, first_track, 0, KeyMode::Unknown);
    ajouter_armure(&mut score, &mut measure, second_track, -3, KeyMode::Unknown);
    ajouter_indication_mesure(&mut score, &mut measure, first_track, 4, 4);
    ajouter_accord(
        &mut score,
        &mut measure,
        first_track,
        Fraction::ZERO,
        Duration::with_dots(DurationType::Quarter, 1),
        &[(60, tpc::C)],
    );
    ajouter_silence(
        &mut score,
        &mut measure,
        first_track,
        Fraction::new(3, 8),
        Duration::eighth(),
    );
    ajouter_silence(
        &mut score,
        &mut measure,
        first_track,
        Fraction::HALF,
        Duration::half(),
    );
    ajouter_silence_mesure(&mut score, &mut measure, second_track, Fraction::ZERO);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert_eq!(xml.matches("<part id=").count(), 2);
    assert_eq!(xml.matches("<measure number=\"1\">").count(), 2);
    assert!(xml.contains("<fifths>-3</fifths>"));
    // La noire pointée est une noire suivie d'un point.
    assert!(xml.contains("<type>quarter</type>\n        <dot/>"));
    // La croche de la seconde partie est aussi une croche.
    assert_eq!(xml.matches("<type>eighth</type>").count(), 1);

    let first_part_xml = xml
        .split("<part id=\"P2\">")
        .next()
        .expect("première partie");
    let second_part_xml = xml
        .split("<part id=\"P2\">")
        .nth(1)
        .expect("seconde partie");
    assert!(first_part_xml.contains("<fifths>0</fifths>"));
    assert!(!first_part_xml.contains("<fifths>-3</fifths>"));
    assert!(second_part_xml.contains("<fifths>-3</fifths>"));
    assert!(second_part_xml.contains("<rest measure=\"yes\"/>"));
}

#[test]
fn corpus_linked_ties() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Flute", "Fl."));
    score.part_mut(part_id).instrument = Instrument {
        id: "wind.flutes.flute".to_owned(),
        long_name: "Flute".to_owned(),
        short_name: "Fl.".to_owned(),
        track_name: "Flute".to_owned(),
        transpose: None,
        midi_channel: None,
        channel_count: 1,
        midi_port: None,
        midi_program: Some(73),
        volume: None,
        pan: None,
        midi_articulations: Vec::new(),
    };
    let staff = score.add_staff(part_id);
    let track = Track::new(staff, 0);

    // Mesure 1 : si4 noire liée, si4 noire liée, blanche de silence.
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_armure(&mut score, &mut measure, track, 0, KeyMode::Unknown);
    ajouter_indication_mesure(&mut score, &mut measure, track, 4, 4);
    let first = ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        Duration::quarter(),
        &[(71, tpc::B)],
    );
    let second = ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::QUARTER,
        Duration::quarter(),
        &[(71, tpc::B)],
    );
    ajouter_silence(
        &mut score,
        &mut measure,
        track,
        Fraction::HALF,
        Duration::half(),
    );
    score.note_mut(first[0]).tie_forward = Some(second[0]);
    score.note_mut(second[0]).tie_back = Some(first[0]);
    score.add_measure(measure);

    // Mesure 2 : ornement lié, noire, soupir, noire liée, ornement après lié,
    // soupir, barre finale.
    let mut measure = ajouter_mesure(1, Fraction::ONE, Fraction::ONE, Fraction::ONE);

    let main = score.add_note(Note::new(71, tpc::B));
    let grace = score.add_note(Note::new(71, tpc::B));
    let mut chord_rest = ChordRest::chord(Duration::quarter());
    {
        let chord = chord_rest.chord_mut().expect("accord");
        chord.notes.push(main);
        let mut grace_chord = GraceChord::new(GraceType::Appoggiatura, Duration::eighth());
        grace_chord.chord_mut().notes.push(grace);
        chord.grace_chords.push(grace_chord);
    }
    ajouter_element(&mut score, &mut measure, track, Fraction::ONE, chord_rest);
    score.note_mut(grace).tie_forward = Some(main);
    score.note_mut(main).tie_back = Some(grace);

    ajouter_silence(
        &mut score,
        &mut measure,
        track,
        Fraction::new(5, 4),
        Duration::quarter(),
    );

    let third = score.add_note(Note::new(71, tpc::B));
    let after = score.add_note(Note::new(71, tpc::B));
    let mut chord_rest = ChordRest::chord(Duration::quarter());
    {
        let chord = chord_rest.chord_mut().expect("accord");
        chord.notes.push(third);
        let mut grace_chord = GraceChord::new(GraceType::Grace8After, Duration::eighth());
        grace_chord.chord_mut().notes.push(after);
        chord.grace_chords.push(grace_chord);
    }
    ajouter_element(
        &mut score,
        &mut measure,
        track,
        Fraction::new(6, 4),
        chord_rest,
    );
    score.note_mut(third).tie_forward = Some(after);
    score.note_mut(after).tie_back = Some(third);

    ajouter_silence(
        &mut score,
        &mut measure,
        track,
        Fraction::new(7, 4),
        Duration::quarter(),
    );
    measure.end_bar_line = BarLineType::End;
    score.add_measure(measure);

    comparer_au_corpus("linked-ties", &score);
}

#[test]
fn corpus_tenuto_accent() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Piano", "Pno."));
    score.part_mut(part_id).instrument = piano_instrument();
    let first = score.add_staff(part_id);
    let second = score.add_staff_with(part_id, Staff::new(part_id));
    // Accolade implicite du piano, comme l'instrument du corpus.
    score.staff_mut(first).brackets.push(StaffBracket {
        kind: 1,
        span: 2,
        visible: true,
    });
    let first_track = Track::new(first, 0);
    let second_track = Track::new(second, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    let mut clefs = Segment::new(SegmentType::HeaderClef, Fraction::ZERO, score.staves.len());
    clefs.set_element(first_track, SegmentItem::Clef(Clef::new(ClefType::G)));
    clefs.set_element(second_track, SegmentItem::Clef(Clef::new(ClefType::F)));
    measure.add_segment(clefs);
    ajouter_armure(&mut score, &mut measure, first_track, 0, KeyMode::Unknown);
    ajouter_armure(&mut score, &mut measure, second_track, 0, KeyMode::Unknown);
    ajouter_indication_mesure(&mut score, &mut measure, first_track, 4, 4);

    let note = score.add_note(Note::new(65, tpc::F));
    let mut chord_rest = ChordRest::chord(Duration::quarter());
    {
        let chord = chord_rest.chord_mut().expect("accord");
        chord.notes.push(note);
        chord
            .articulations
            .push(Articulation::new(ArticulationType::Tenuto));
        chord
            .articulations
            .push(Articulation::new(ArticulationType::Accent));
    }
    ajouter_element(
        &mut score,
        &mut measure,
        first_track,
        Fraction::ZERO,
        chord_rest,
    );
    ajouter_silence(
        &mut score,
        &mut measure,
        first_track,
        Fraction::QUARTER,
        Duration::quarter(),
    );
    ajouter_silence(
        &mut score,
        &mut measure,
        first_track,
        Fraction::HALF,
        Duration::half(),
    );
    ajouter_silence_mesure(&mut score, &mut measure, second_track, Fraction::ZERO);
    measure.end_bar_line = BarLineType::End;
    score.add_measure(measure);

    comparer_au_corpus("tenuto_accent", &score);
}

#[test]
fn corpus_paroles() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Voice", ""));
    score.part_mut(part_id).instrument = Instrument {
        id: "voice.vocals".to_owned(),
        long_name: "Voice".to_owned(),
        short_name: String::new(),
        track_name: "Voice".to_owned(),
        transpose: None,
        midi_channel: None,
        channel_count: 1,
        midi_port: None,
        midi_program: Some(52),
        volume: None,
        pan: None,
        midi_articulations: Vec::new(),
    };
    let staff = score.add_staff(part_id);
    let track = Track::new(staff, 0);

    struct NoteSpec {
        tick: Fraction,
        duration: Duration,
        pitch: u8,
        tpc: i8,
        lyric: Option<(LyricsSyllabic, &'static str, bool)>,
        beam: BeamMode,
    }

    let specifications = [
        // Mesure 1.
        NoteSpec {
            tick: Fraction::ZERO,
            duration: Duration::half(),
            pitch: 72,
            tpc: tpc::C,
            lyric: Some((LyricsSyllabic::Single, "Nel", false)),
            beam: BeamMode::Auto,
        },
        NoteSpec {
            tick: Fraction::HALF,
            duration: Duration::quarter(),
            pitch: 74,
            tpc: tpc::D,
            lyric: Some((LyricsSyllabic::Begin, "mez", false)),
            beam: BeamMode::Auto,
        },
        NoteSpec {
            tick: Fraction::new(3, 4),
            duration: Duration::quarter(),
            pitch: 76,
            tpc: tpc::E,
            lyric: Some((LyricsSyllabic::End, "zo", false)),
            beam: BeamMode::Auto,
        },
        // Mesure 2.
        NoteSpec {
            tick: Fraction::ONE,
            duration: Duration::half(),
            pitch: 77,
            tpc: tpc::F,
            lyric: Some((LyricsSyllabic::Single, "del", false)),
            beam: BeamMode::Auto,
        },
        NoteSpec {
            tick: Fraction::new(3, 2),
            duration: Duration::quarter(),
            pitch: 76,
            tpc: tpc::E,
            lyric: Some((LyricsSyllabic::Begin, "cam", false)),
            beam: BeamMode::Auto,
        },
        NoteSpec {
            tick: Fraction::new(7, 4),
            duration: Duration::quarter(),
            pitch: 74,
            tpc: tpc::D,
            lyric: None,
            beam: BeamMode::Auto,
        },
        // Mesure 3.
        NoteSpec {
            tick: Fraction::from_integer(2),
            duration: Duration::eighth(),
            pitch: 72,
            tpc: tpc::C,
            lyric: Some((LyricsSyllabic::End, "min", true)),
            beam: BeamMode::Begin,
        },
        NoteSpec {
            tick: Fraction::from_integer(2) + Fraction::new(1, 8),
            duration: Duration::eighth(),
            pitch: 74,
            tpc: tpc::D,
            lyric: None,
            beam: BeamMode::Mid,
        },
        NoteSpec {
            tick: Fraction::from_integer(2) + Fraction::new(2, 8),
            duration: Duration::eighth(),
            pitch: 76,
            tpc: tpc::E,
            lyric: None,
            beam: BeamMode::Mid,
        },
        NoteSpec {
            tick: Fraction::from_integer(2) + Fraction::new(3, 8),
            duration: Duration::eighth(),
            pitch: 77,
            tpc: tpc::F,
            lyric: Some((LyricsSyllabic::Single, "di", false)),
            beam: BeamMode::End,
        },
        NoteSpec {
            tick: Fraction::from_integer(2) + Fraction::new(4, 8),
            duration: Duration::eighth(),
            pitch: 79,
            tpc: tpc::G,
            lyric: Some((LyricsSyllabic::Begin, "no", false)),
            beam: BeamMode::Begin,
        },
        NoteSpec {
            tick: Fraction::from_integer(2) + Fraction::new(5, 8),
            duration: Duration::eighth(),
            pitch: 77,
            tpc: tpc::F,
            lyric: Some((LyricsSyllabic::End, "stra", false)),
            beam: BeamMode::Mid,
        },
        NoteSpec {
            tick: Fraction::from_integer(2) + Fraction::new(6, 8),
            duration: Duration::eighth(),
            pitch: 76,
            tpc: tpc::E,
            lyric: Some((LyricsSyllabic::Begin, "vi", false)),
            beam: BeamMode::Mid,
        },
        NoteSpec {
            tick: Fraction::from_integer(2) + Fraction::new(7, 8),
            duration: Duration::eighth(),
            pitch: 74,
            tpc: tpc::D,
            lyric: None,
            beam: BeamMode::End,
        },
        // Mesure 4.
        NoteSpec {
            tick: Fraction::from_integer(3),
            duration: Duration::whole(),
            pitch: 72,
            tpc: tpc::C,
            lyric: Some((LyricsSyllabic::End, "ta.", false)),
            beam: BeamMode::Auto,
        },
        // Mesure 5.
        NoteSpec {
            tick: Fraction::from_integer(4),
            duration: Duration::quarter(),
            pitch: 72,
            tpc: tpc::C,
            lyric: None,
            beam: BeamMode::Auto,
        },
        NoteSpec {
            tick: Fraction::from_integer(4) + Fraction::QUARTER,
            duration: Duration::quarter(),
            pitch: 74,
            tpc: tpc::D,
            lyric: None,
            beam: BeamMode::Auto,
        },
        NoteSpec {
            tick: Fraction::from_integer(4) + Fraction::HALF,
            duration: Duration::quarter(),
            pitch: 76,
            tpc: tpc::E,
            lyric: None,
            beam: BeamMode::Auto,
        },
        NoteSpec {
            tick: Fraction::from_integer(4) + Fraction::new(3, 4),
            duration: Duration::quarter(),
            pitch: 77,
            tpc: tpc::F,
            lyric: None,
            beam: BeamMode::Auto,
        },
        // Mesure 6.
        NoteSpec {
            tick: Fraction::from_integer(5),
            duration: Duration::eighth(),
            pitch: 79,
            tpc: tpc::G,
            lyric: None,
            beam: BeamMode::Begin,
        },
        NoteSpec {
            tick: Fraction::from_integer(5) + Fraction::new(1, 8),
            duration: Duration::eighth(),
            pitch: 77,
            tpc: tpc::F,
            lyric: None,
            beam: BeamMode::Mid,
        },
        NoteSpec {
            tick: Fraction::from_integer(5) + Fraction::new(2, 8),
            duration: Duration::eighth(),
            pitch: 76,
            tpc: tpc::E,
            lyric: None,
            beam: BeamMode::Mid,
        },
        NoteSpec {
            tick: Fraction::from_integer(5) + Fraction::new(3, 8),
            duration: Duration::eighth(),
            pitch: 74,
            tpc: tpc::D,
            lyric: None,
            beam: BeamMode::End,
        },
        NoteSpec {
            tick: Fraction::from_integer(5) + Fraction::new(4, 8),
            duration: Duration::eighth(),
            pitch: 72,
            tpc: tpc::C,
            lyric: None,
            beam: BeamMode::Begin,
        },
        NoteSpec {
            tick: Fraction::from_integer(5) + Fraction::new(5, 8),
            duration: Duration::eighth(),
            pitch: 74,
            tpc: tpc::D,
            lyric: None,
            beam: BeamMode::Mid,
        },
        NoteSpec {
            tick: Fraction::from_integer(5) + Fraction::new(6, 8),
            duration: Duration::eighth(),
            pitch: 76,
            tpc: tpc::E,
            lyric: None,
            beam: BeamMode::Mid,
        },
        NoteSpec {
            tick: Fraction::from_integer(5) + Fraction::new(7, 8),
            duration: Duration::eighth(),
            pitch: 77,
            tpc: tpc::F,
            lyric: None,
            beam: BeamMode::End,
        },
        // Mesure 7.
        NoteSpec {
            tick: Fraction::from_integer(6),
            duration: Duration::half(),
            pitch: 76,
            tpc: tpc::E,
            lyric: None,
            beam: BeamMode::Auto,
        },
        NoteSpec {
            tick: Fraction::from_integer(6) + Fraction::HALF,
            duration: Duration::half(),
            pitch: 74,
            tpc: tpc::D,
            lyric: None,
            beam: BeamMode::Auto,
        },
        // Mesure 8.
        NoteSpec {
            tick: Fraction::from_integer(7),
            duration: Duration::whole(),
            pitch: 72,
            tpc: tpc::C,
            lyric: None,
            beam: BeamMode::Auto,
        },
    ];

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_armure(&mut score, &mut measure, track, 0, KeyMode::Unknown);
    ajouter_indication_mesure(&mut score, &mut measure, track, 4, 4);
    score.add_measure(measure);

    for number in 1..8 {
        let measure = ajouter_mesure(
            number,
            Fraction::from_integer(number as i64),
            Fraction::ONE,
            Fraction::ONE,
        );
        score.add_measure(measure);
    }

    for specification in &specifications {
        let note = score.add_note(Note::new(specification.pitch, specification.tpc));
        let mut chord_rest = ChordRest::chord(specification.duration);
        chord_rest.beam_mode = specification.beam;
        if let Some((syllabic, text, melisma)) = specification.lyric {
            let mut lyric = Lyrics::new(0, syllabic, text);
            if melisma {
                lyric.melisma = Some(specification.duration.ticks());
            }
            chord_rest.lyrics.push(lyric);
        }
        chord_rest.chord_mut().expect("accord").notes.push(note);

        let measure_index =
            (specification.tick.numerator() / specification.tick.denominator()) as usize;
        let measure = &mut score.measures[measure_index.min(7)];
        let mut segment = Segment::new(
            SegmentType::ChordRest,
            specification.tick,
            score.staves.len(),
        );
        segment.set_element(track, SegmentItem::ChordRest(chord_rest));
        measure.add_segment(segment);
    }

    score.measures[7].end_bar_line = BarLineType::End;

    comparer_au_corpus("copypastesymbollist-lyrics", &score);
}

// ----------------------------------------------------------------------
// Textes : directions, métronome, repères, crédits
// ----------------------------------------------------------------------

/// Ajoute une annotation au segment d'accords du tick demandé.
fn ajouter_annotation(
    measure: &mut Measure,
    staves: usize,
    track: Track,
    tick: Fraction,
    annotation: Annotation,
) {
    // MuseScore partage un segment entre toutes les portées : on le cherche
    // d'abord avec un élément sur la piste de l'annotation.
    let index = measure
        .segments
        .iter()
        .position(|segment| {
            segment.kind == SegmentType::ChordRest
                && segment.tick == tick
                && segment.element(track).is_some()
        })
        .or_else(|| {
            measure
                .segments
                .iter()
                .position(|segment| segment.kind == SegmentType::ChordRest && segment.tick == tick)
        });
    match index {
        Some(index) => measure.segments[index].add_annotation(track, annotation),
        None => {
            let mut segment = Segment::new(SegmentType::ChordRest, tick, staves);
            segment.add_annotation(track, annotation);
            measure.add_segment(segment);
        }
    }
}

#[test]
fn textes_de_portee_et_de_systeme() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Piano", "Pno."));
    score.part_mut(part_id).instrument = piano_instrument();
    let first = score.add_staff(part_id);
    let second = score.add_staff_with(part_id, Staff::new(part_id));
    let first_track = Track::new(first, 0);
    let second_track = Track::new(second, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut measure, first_track, Fraction::ZERO);
    ajouter_silence_mesure(&mut score, &mut measure, second_track, Fraction::ZERO);
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        first_track,
        Fraction::ZERO,
        Annotation::StaffText(StaffText::new("doux")),
    );
    let mut below = StaffText::new("pédale");
    below.placement = Placement::Below;
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        second_track,
        Fraction::ZERO,
        Annotation::StaffText(below),
    );
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        second_track,
        Fraction::ZERO,
        Annotation::SystemText(SystemText::new("Chœur")),
    );
    let mut invisible = StaffText::new("caché");
    invisible.visible = false;
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        first_track,
        Fraction::ZERO,
        Annotation::StaffText(invisible),
    );
    // Texte d'expression et changement d'instrument : eux aussi des `<words>`.
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        first_track,
        Fraction::ZERO,
        Annotation::Expression(StaffText::new("espressivo")),
    );
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        first_track,
        Fraction::ZERO,
        Annotation::InstrumentChange(InstrumentChange::new("pizz.")),
    );
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");

    // Portée 1 : texte au-dessus, rattaché à `<staff>1</staff>`.
    assert!(xml.contains(
        "<direction placement=\"above\">\n        <direction-type>\n          \
         <words>doux</words>\n          </direction-type>\n        <staff>1</staff>"
    ));
    // Portée 2 : texte au-dessous, rattaché à `<staff>2</staff>`.
    assert!(xml.contains(
        "<direction placement=\"below\">\n        <direction-type>\n          \
         <words>pédale</words>\n          </direction-type>\n        <staff>2</staff>"
    ));
    // Texte système : `system="only-top"` sur la direction.
    assert!(xml.contains(
        "<direction placement=\"above\" system=\"only-top\">\n        <direction-type>\n          \
         <words>Chœur</words>"
    ));
    // Un texte invisible n'est pas exporté (`canWrite`).
    assert!(!xml.contains("caché"));
    assert!(xml.contains("<words>espressivo</words>"));
    // Le changement d'instrument reprend le style « Instrument Name » :
    // gras, taille 12.
    assert!(xml.contains("<words font-weight=\"bold\" font-size=\"12\">pizz.</words>"));
}

#[test]
fn tempo_avec_metronome() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);

    let mut tempo = Tempo::new("<sym>metNoteQuarterUp</sym> = 120");
    tempo.beats_per_second = Some(2.0);
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        track,
        Fraction::ZERO,
        Annotation::Tempo(tempo),
    );

    // Formule entourée de texte, avec parenthèses.
    let mut allegro = Tempo::new("Allegro (<sym>metNoteHalfUp</sym> = 60)");
    allegro.beats_per_second = Some(1.0);
    allegro.placement = Placement::Below;
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        track,
        Fraction::ZERO,
        Annotation::Tempo(allegro),
    );

    // Tempo invisible : seul le `<sound tempo>` est exporté.
    let mut invisible = Tempo::new("<sym>metNoteQuarterUp</sym> = 90");
    invisible.beats_per_second = Some(1.5);
    invisible.visible = false;
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        track,
        Fraction::ZERO,
        Annotation::Tempo(invisible),
    );

    score.add_measure(measure);
    let xml = export_musicxml(&score).expect("export");

    assert!(xml.contains(
        "<direction placement=\"above\" system=\"only-top\">\n        <direction-type>\n          \
         <metronome parentheses=\"no\">\n            <beat-unit>quarter</beat-unit>\n            \
         <per-minute>120</per-minute>\n            </metronome>"
    ));
    assert!(xml.contains("<sound tempo=\"120\"/>"));
    // Texte à gauche, parenthèses reconnues.
    assert!(xml.contains(">Allegro </words>"));
    assert!(xml.contains("<metronome parentheses=\"yes\">"));
    assert!(xml.contains("<beat-unit>half</beat-unit>"));
    assert!(xml.contains("<per-minute>60</per-minute>"));
    assert!(xml.contains("<sound tempo=\"60\"/>"));
    assert!(xml.contains("<sound tempo=\"90\"/>"));
    assert!(!xml.contains("<words>\u{ECA5} = 90"));
}

#[test]
fn reperes_de_repetition() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);

    // Cadre du style (rectangle) : pas d'attribut `enclosure`.
    let mut encadre = RehearsalMark::new("A");
    encadre.placement = Placement::Below;
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        track,
        Fraction::ZERO,
        Annotation::RehearsalMark(encadre),
    );

    let mut cercle = RehearsalMark::new("B");
    cercle.frame = RehearsalFrame::Circle;
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        track,
        Fraction::ZERO,
        Annotation::RehearsalMark(cercle),
    );

    let mut sans_cadre = RehearsalMark::new("C");
    sans_cadre.frame = RehearsalFrame::NoFrame;
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        track,
        Fraction::ZERO,
        Annotation::RehearsalMark(sans_cadre),
    );

    score.add_measure(measure);
    let xml = export_musicxml(&score).expect("export");

    assert!(xml.contains(
        "<direction placement=\"below\" system=\"only-top\">\n        <direction-type>\n          \
         <rehearsal justify=\"center\" font-weight=\"bold\" font-size=\"14\">A</rehearsal>"
    ));
    assert!(xml.contains("<rehearsal enclosure=\"circle\" justify=\"center\" font-weight=\"bold\" font-size=\"14\">B"));
    assert!(xml.contains(
        "<rehearsal enclosure=\"none\" justify=\"center\" font-weight=\"bold\" font-size=\"14\">C"
    ));
}

#[test]
fn formats_de_texte_dans_words() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        track,
        Fraction::ZERO,
        Annotation::StaffText(StaffText::new(
            "a<b>b</b><i>c</i><font face=\"FreeSerif\"><font size=\"16\">d</font></font><u>e</u>",
        )),
    );
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");

    assert!(xml.contains("<words>a</words>"));
    assert!(xml.contains("<words font-weight=\"bold\">b</words>"));
    assert!(xml.contains("<words font-weight=\"normal\" font-style=\"italic\">c</words>"));
    assert!(xml.contains(
        "<words font-style=\"normal\" font-family=\"FreeSerif\" font-size=\"16\">d</words>"
    ));
    // `</font>` ne restaure pas le format précédent (comportement MuseScore).
    assert!(xml.contains("<words underline=\"1\">e</words>"));
}

#[test]
fn credits_et_metadonnees_de_page_de_titre() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    score.metadata.work_title = "Œuvre".to_owned();
    score.metadata.work_number = "Op. 1".to_owned();
    score.metadata.movement_title = "Mouvement".to_owned();
    score.metadata.composer = "Compositrice".to_owned();
    score.metadata.lyricist = "Parolier".to_owned();
    score.metadata.copyright = "© 2026".to_owned();
    score.add_credit(Credit::new(RichText::new("Titre"), CreditStyle::Title));
    score.add_credit(Credit::new(
        RichText::new("<b>Sous-titre</b>"),
        CreditStyle::Subtitle,
    ));

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");

    assert!(xml.contains(
        "<work>\n    <work-number>Op. 1</work-number>\n    <work-title>Œuvre</work-title>\n    </work>"
    ));
    assert!(xml.contains("<movement-title>Mouvement</movement-title>"));
    assert!(xml.contains("<creator type=\"composer\">Compositrice</creator>"));
    assert!(xml.contains("<creator type=\"lyricist\">Parolier</creator>"));
    assert!(xml.contains(
        "<credit page=\"1\">\n    <credit-type>title</credit-type>\n    \
         <credit-words default-x=\"600.17\" default-y=\"1611.01\" justify=\"center\" \
         valign=\"top\" font-size=\"22\">Titre</credit-words>"
    ));
    assert!(xml.contains("<credit-type>subtitle</credit-type>"));
    assert!(xml.contains(
        "<credit-words default-x=\"600.17\" default-y=\"1553.86\" justify=\"center\" \
         valign=\"top\" font-weight=\"bold\" font-size=\"14\">Sous-titre</credit-words>"
    ));
    assert!(xml.contains("<credit-type>rights</credit-type>"));
    assert!(xml.contains(
        "<credit-words default-x=\"600.17\" default-y=\"85.71\" justify=\"center\" \
         valign=\"bottom\" font-size=\"8\">© 2026</credit-words>"
    ));
}

// ----------------------------------------------------------------------
// Lignes de texte, cadres, changements d'instrument, swing
// ----------------------------------------------------------------------

/// Partition d'une mesure de silence, avec une ligne de texte de la mesure 0
/// à la fin de la mesure.
fn score_avec_ligne(info: TextLineInfo) -> Score {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);
    score.add_measure(measure);
    let mut spanner = Spanner::new(
        SpannerType::TextLine,
        SpannerAnchor::new(Fraction::ZERO, track),
        SpannerAnchor::new(Fraction::ONE, track),
    );
    spanner.text_line = Some(info);
    score.add_spanner(spanner);
    score
}

#[test]
fn ligne_de_texte_visible_en_bracket() {
    let mut info = TextLineInfo {
        begin_hook: scoretk_core::model::HookType::Hook90,
        end_hook: scoretk_core::model::HookType::Hook90,
        end_hook_height: 1.5,
        ..TextLineInfo::default()
    };
    info.begin_text = Some(RichText::new("Staff/1"));
    let score = score_avec_ligne(info);
    let xml = export_musicxml(&score).expect("export");

    assert!(xml.contains(
        "<direction placement=\"above\">\n        <direction-type>\n          \
         <words>Staff/1</words>\n          </direction-type>"
    ));
    assert!(xml.contains(
        "<bracket type=\"start\" number=\"1\" line-end=\"down\" line-type=\"solid\" end-length=\"15\"/>"
    ));
    assert!(
        xml.contains("<bracket type=\"stop\" number=\"1\" line-end=\"down\" end-length=\"15\"/>")
    );
}

#[test]
fn ligne_de_texte_invisible_avec_texte_de_fin() {
    let mut info = TextLineInfo {
        line_visible: false,
        ..TextLineInfo::default()
    };
    info.end_text = Some(RichText::new("3x"));
    let score = score_avec_ligne(info);
    let xml = export_musicxml(&score).expect("export");

    // `writeHairpinText` écrit toujours la police de l'extrémité.
    assert!(xml.contains("<words font-family=\"Edwin\" font-size=\"10\">3x</words>"));
    assert!(!xml.contains("<bracket"));
    // `spannerStop` passe la position sentinelle -1 : 4 * 1 * (-1 - 1) = -8.
    assert!(xml.contains("<offset sound=\"yes\">-8</offset>"));
}

#[test]
fn texte_de_tbox_en_words() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);
    score.add_measure(measure);
    score.add_tbox_text(TboxText::new(RichText::new("1,2,3,4   2,3,4"), 0));

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains(
        "<direction placement=\"above\">\n        <direction-type>\n          \
         <words valign=\"top\">1,2,3,4   2,3,4</words>"
    ));
}

#[test]
fn changement_d_instrument_complet() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);
    let mut change = InstrumentChange::new("Flûte");
    change.instrument = Some(Instrument {
        id: "wind.flutes.flute".to_owned(),
        long_name: "Flute".to_owned(),
        short_name: "Fl.".to_owned(),
        track_name: "Flute".to_owned(),
        transpose: Some(Transpose::new(-1, -2)),
        midi_program: Some(73),
        ..piano_instrument()
    });
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        track,
        Fraction::ZERO,
        Annotation::InstrumentChange(change),
    );
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains(
        "<print>\n        <part-name-display>\n          \
         <display-text>Flute</display-text>\n          </part-name-display>"
    ));
    assert!(xml.contains("<part-abbreviation-display>\n          <display-text>Fl.</display-text>"));
    assert!(xml.contains(
        "<transpose>\n          <diatonic>-1</diatonic>\n          \
         <chromatic>-2</chromatic>\n          </transpose>"
    ));
    assert!(xml.contains(
        "<sound>\n        <instrument-change id=\"P1-I2\">\n          \
         <instrument-sound>wind.flutes.flute</instrument-sound>"
    ));
    assert!(xml.contains("<words font-weight=\"bold\" font-size=\"12\">Flûte</words>"));
}

#[test]
fn a_tempo_reprend_le_tempo_precedent() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut first = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut first, track, Fraction::ZERO);
    let mut tempo = Tempo::new("Andante");
    tempo.beats_per_second = Some(1.5);
    ajouter_annotation(
        &mut first,
        score.staves.len(),
        track,
        Fraction::ZERO,
        Annotation::Tempo(tempo),
    );
    score.add_measure(first);

    let mut second = ajouter_mesure(1, Fraction::ONE, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut second, track, Fraction::ONE);
    let mut a_tempo = Tempo::new("a tempo");
    a_tempo.beats_per_second = Some(9.9);
    a_tempo.kind = TempoKind::ATempo;
    ajouter_annotation(
        &mut second,
        score.staves.len(),
        track,
        Fraction::ONE,
        Annotation::Tempo(a_tempo),
    );
    score.add_measure(second);

    let xml = export_musicxml(&score).expect("export");
    assert_eq!(xml.matches("<sound tempo=\"90\"/>").count(), 2);
    assert!(!xml.contains("tempo=\"594\""));
}

#[test]
fn swing_de_texte_systeme() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);
    let mut texte = SystemText::new("Swing");
    texte.swing = Some(SwingParameters {
        unit: SwingUnit::Eighth,
        ratio: 60,
    });
    ajouter_annotation(
        &mut measure,
        score.staves.len(),
        track,
        Fraction::ZERO,
        Annotation::SystemText(texte),
    );
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains(
        "<sound>\n          <swing>\n            <first>5</first>\n            \
         <second>3</second>\n            <swing-type>eighth</swing-type>\n            \
         </swing>\n          </sound>"
    ));
}

#[test]
fn parties_multi_instruments() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Flute", "Fl."));
    let mut flute = piano_instrument();
    flute.id = "wind.flutes.flute".to_owned();
    flute.track_name = "Flute".to_owned();
    flute.midi_program = Some(73);
    score.part_mut(part_id).instrument = flute.clone();
    let mut oboe = flute;
    oboe.id = "wind.reed.oboe".to_owned();
    oboe.track_name = "Oboe".to_owned();
    oboe.midi_program = Some(68);
    score
        .part_mut(part_id)
        .set_instrument_at(Fraction::new(1, 2), oboe);
    let staff = score.add_staff(part_id);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        Duration::half(),
        &[(60, tpc::C)],
    );
    ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::new(1, 2),
        Duration::half(),
        &[(62, tpc::D)],
    );
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<score-instrument id=\"P1-I2\">"));
    assert!(xml.contains("<instrument-name>Oboe</instrument-name>"));
    assert!(xml.contains("<instrument-sound>wind.reed.oboe</instrument-sound>"));
    assert!(xml.contains("<midi-instrument id=\"P1-I2\">"));
    assert!(xml.contains("<midi-channel>2</midi-channel>"));
    assert!(xml.contains("<midi-program>69</midi-program>"));
    assert!(xml.contains("<instruments>2</instruments>"));
    assert!(xml.contains("<instrument id=\"P1-I1\"/>"));
    assert!(xml.contains("<instrument id=\"P1-I2\"/>"));
}

#[test]
fn numerotation_apres_saut_de_section() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    for index in 0..3 {
        let start = Fraction::from_integer(index as i64);
        let mut measure = ajouter_mesure(index, start, Fraction::ONE, Fraction::ONE);
        if index == 1 {
            measure.section_break_before = true;
            measure.section_keeps_numbering = false;
        }
        ajouter_accord(
            &mut score,
            &mut measure,
            track,
            start,
            Duration::whole(),
            &[(60, tpc::C)],
        );
        score.add_measure(measure);
    }

    let xml = export_musicxml(&score).expect("export");
    // La deuxième mesure rouvre une section : elle repart à 1 et réécrit les
    // divisions, la troisième suit.
    assert_eq!(xml.matches("<measure number=\"1\">").count(), 2);
    assert!(xml.contains("<measure number=\"2\">"));
    assert_eq!(xml.matches("<divisions>").count(), 2);
}

#[test]
fn alteration_d_avertissement() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);

    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    let ids = ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        Duration::whole(),
        &[(61, tpc::C)],
    );
    let mut accidental = Accidental::new(AccidentalType::Sharp);
    accidental.user = true;
    score.note_mut(ids[0]).accidental = Some(accidental);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<accidental cautionary=\"yes\" parentheses=\"no\">sharp</accidental>"));
}

#[test]
fn repetition_de_mesure_stop_numero_de_porte_valide() {
    let mut score = Score::new();
    let (part_id, staff1) = piano(&mut score);
    let staff2 = score.add_staff(part_id);
    let (piste1, piste2) = (Track::new(staff1, 0), Track::new(staff2, 0));
    let mesure = Fraction::ONE;

    // Trois mesures ordinaires : la deuxième ouvre une répétition sur la
    // portée 2, la troisième la ferme. Le groupe porte donc le numéro 2.
    let mut m0 = ajouter_mesure(0, Fraction::ZERO, mesure, mesure);
    ajouter_accord(
        &mut score,
        &mut m0,
        piste1,
        Fraction::ZERO,
        Duration::whole(),
        &[(60, tpc::C)],
    );
    ajouter_accord(
        &mut score,
        &mut m0,
        piste2,
        Fraction::ZERO,
        Duration::whole(),
        &[(48, tpc::C)],
    );
    score.add_measure(m0);

    let mut m1 = ajouter_mesure(1, mesure, mesure, mesure);
    m1.measure_repeat = Some(MeasureRepeatMark {
        staff: staff2,
        count: 2,
    });
    ajouter_accord(
        &mut score,
        &mut m1,
        piste1,
        Fraction::ZERO,
        Duration::whole(),
        &[(62, tpc::C)],
    );
    ajouter_accord(
        &mut score,
        &mut m1,
        piste2,
        Fraction::ZERO,
        Duration::whole(),
        &[(50, tpc::C)],
    );
    score.add_measure(m1);

    let mut m2 = ajouter_mesure(2, Fraction::new(2, 1), mesure, mesure);
    ajouter_accord(
        &mut score,
        &mut m2,
        piste1,
        Fraction::ZERO,
        Duration::whole(),
        &[(64, tpc::C)],
    );
    ajouter_accord(
        &mut score,
        &mut m2,
        piste2,
        Fraction::ZERO,
        Duration::whole(),
        &[(52, tpc::C)],
    );
    score.add_measure(m2);

    let xml = export_musicxml(&score).expect("export");
    // xsd:staff-number = xs:positiveInteger : « 0 » n'est pas un numéro de portée.
    assert!(
        !xml.contains("number=\"0\""),
        "numéro de portée invalide dans {xml}"
    );
    assert!(
        xml.contains("<measure-style number=\"2\">"),
        "ouverture sur la portée 2 : {xml}"
    );
    assert!(
        xml.contains("<measure-style number=\"2\">\n          <measure-repeat type=\"stop\"")
            || xml.contains("<measure-style number=\"2\">\n        <measure-repeat type=\"stop\""),
        "la fermeture doit porter le numéro de la portée : {xml}"
    );
    assert_eq!(xml.matches("<measure number=").count(), 3);
}

#[test]
fn tempo_sans_equation_reste_en_words() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let piste = Track::new(staff, 0);

    let mut m0 = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    let ids = ajouter_accord(
        &mut score,
        &mut m0,
        piste,
        Fraction::ZERO,
        Duration::quarter(),
        &[(60, tpc::C)],
    );
    let suivants = ajouter_accord(
        &mut score,
        &mut m0,
        piste,
        Fraction::new(1, 4),
        Duration::quarter(),
        &[(60, tpc::C)],
    );
    score.note_mut(ids[0]).tie_forward = Some(suivants[0]);
    score.note_mut(suivants[0]).tie_back = Some(ids[0]);
    ajouter_silence(
        &mut score,
        &mut m0,
        piste,
        Fraction::new(1, 2),
        Duration::half(),
    );
    // « ♩ ≈ 90 » : pas de signe « = », donc pas de formule de métronome.
    let mut tempo = Tempo::new("<sym>metNoteQuarterUp</sym> ≈ 90");
    tempo.beats_per_second = Some(1.5);
    ajouter_annotation(
        &mut m0,
        score.staves.len(),
        piste,
        Fraction::ZERO,
        Annotation::Tempo(tempo),
    );
    score.add_measure(m0);

    let mut m1 = ajouter_mesure(1, Fraction::ONE, Fraction::ONE, Fraction::ONE);
    ajouter_accord(
        &mut score,
        &mut m1,
        piste,
        Fraction::ZERO,
        Duration::whole(),
        &[(62, tpc::C)],
    );
    score.add_measure(m1);

    let xml = export_musicxml(&score).expect("export");
    // xsd:metronome exige <per-minute> ou un second <beat-unit> ; sans « = »
    // MuseScore 4.7.4 écrit un <metronome> invalide, on garde donc des <words>.
    assert!(
        !xml.contains("<metronome"),
        "métrophone sans enfant valide : {xml}"
    );
    assert!(xml.contains("≈ 90"), "le « ≈ » doit rester lisible : {xml}");
    assert!(xml.contains("<sound tempo=\"90\"/>"));
    assert_eq!(xml.matches("<measure number=").count(), 2);
}

#[test]
fn port_midi_de_l_instrument() {
    let mut score = Score::new();
    let (part_id, staff) = piano(&mut score);
    let instrument = Instrument {
        midi_port: Some(1),
        ..score.part(part_id).instrument.clone()
    };
    score.part_mut(part_id).instrument = instrument;
    let piste = Track::new(staff, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut measure, piste, Fraction::ZERO);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    // MuseScore : midi-device port = <Channel><midiPort> + 1
    // (exportmusicxml.cpp:7939-7947). La valeur par défaut reste 1.
    assert!(
        xml.contains("<midi-device id=\"P1-I1\" port=\"2\"></midi-device>"),
        "port de la piste : {xml}"
    );
    assert!(
        !xml.contains("port=\"1\""),
        "aucun port par défaut ici : {xml}"
    );

    // Un instrument sans port explicite garde le port 1 (cas non concerné).
    score.part_mut(part_id).instrument.midi_port = None;
    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<midi-device id=\"P1-I1\" port=\"1\"></midi-device>"));
}

#[test]
fn anacrouse_implicite_et_numeration() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let piste = Track::new(staff, 0);
    let nominal = Fraction::new(12, 8);

    // Première mesure courte, exclue de la numérotation (anacrouse) ; la
    // suivante est la première mesure « réelle ».
    let mut m0 = ajouter_mesure(0, Fraction::ZERO, nominal, Fraction::new(3, 8));
    m0.exclude_from_numbering = true;
    ajouter_accord(
        &mut score,
        &mut m0,
        piste,
        Fraction::ZERO,
        Duration::quarter(),
        &[(60, tpc::C)],
    );
    score.add_measure(m0);

    let mut m1 = ajouter_mesure(1, Fraction::new(3, 8), nominal, nominal);
    ajouter_silence_mesure(&mut score, &mut m1, piste, Fraction::ZERO);
    score.add_measure(m1);

    let xml = export_musicxml(&score).expect("export");
    // MeasureNumberStateHandler (exportmusicxml.cpp:8272-8299).
    assert!(
        xml.contains("<measure number=\"0\" implicit=\"yes\">"),
        "anacrouse : {xml}"
    );
    assert!(
        xml.contains("<measure number=\"1\">"),
        "mesure réelle : {xml}"
    );
}

#[test]
fn symbole_de_groupe_carre_et_ligne() {
    let mut score = Score::new();
    let p1 = score.add_part(Part::new("Bois", "Bois"));
    let s1 = score.add_staff(p1);
    let p2 = score.add_part(Part::new("Cuivres", "Cuiv."));
    let s2 = score.add_staff(p2);
    // BracketType (src/engraving/types/types.h:1289) : 0 NORMAL, 1 BRACE,
    // 2 SQUARE, 3 LINE. Le portage inversait 2 et 3.
    score.staff_mut(s1).brackets.push(StaffBracket {
        kind: 2,
        span: 2,
        visible: true,
    });
    for staff in [s1, s2] {
        let piste = Track::new(staff, 0);
        let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
        ajouter_silence_mesure(&mut score, &mut measure, piste, Fraction::ZERO);
        score.add_measure(measure);
    }
    let xml = export_musicxml(&score).expect("export");
    assert!(
        xml.contains("<group-symbol>square</group-symbol>"),
        "type 2 = SQUARE : {xml}"
    );
    assert!(!xml.contains("<group-symbol>line</group-symbol>"));

    // Cas non concerné : le type 3 reste un trait.
    score.staff_mut(s1).brackets[0].kind = 3;
    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<group-symbol>line</group-symbol>"));
    assert!(!xml.contains("<group-symbol>square</group-symbol>"));
}

#[test]
fn melisma_nul_n_ecrit_pas_extend() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let piste = Track::new(staff, 0);

    let mut m0 = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    let premiers = ajouter_accord(
        &mut score,
        &mut m0,
        piste,
        Fraction::ZERO,
        Duration::quarter(),
        &[(60, tpc::C)],
    );
    let suivants = ajouter_accord(
        &mut score,
        &mut m0,
        piste,
        Fraction::new(1, 4),
        Duration::quarter(),
        &[(60, tpc::C)],
    );
    score.note_mut(premiers[0]).tie_forward = Some(suivants[0]);
    score.note_mut(suivants[0]).tie_back = Some(premiers[0]);
    // Un `<ticks_f>0/16</ticks_f>` est lu comme Some(ZERO) : aucun prolongement.
    let mut nul = Lyrics::new(0, LyricsSyllabic::Begin, "En");
    nul.melisma = Some(Fraction::ZERO);
    let mut cr = ChordRest::chord(Duration::quarter());
    cr.lyrics.push(nul);
    cr.chord_mut().expect("accord").notes.push(premiers[0]);
    ajouter_element(&mut score, &mut m0, piste, Fraction::ZERO, cr);
    // Une syllabe avec un vrai melisma doit garder son <extend/>.
    let mut reel = Lyrics::new(0, LyricsSyllabic::End, "fant");
    reel.melisma = Some(Fraction::new(1, 4));
    let mut cr2 = ChordRest::chord(Duration::quarter());
    cr2.lyrics.push(reel);
    cr2.chord_mut().expect("accord").notes.push(suivants[0]);
    ajouter_element(&mut score, &mut m0, piste, Fraction::new(1, 4), cr2);
    score.add_measure(m0);

    let mut m1 = ajouter_mesure(1, Fraction::ONE, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut m1, piste, Fraction::ZERO);
    score.add_measure(m1);

    let xml = export_musicxml(&score).expect("export");
    // `ExportMusicXml::lyrics` (exportmusicxml.cpp:6130) : ticks().isNotZero().
    assert_eq!(
        xml.matches("<extend/>").count(),
        1,
        "extend nul en trop : {xml}"
    );
    assert!(xml.contains("<text>fant</text>\n          <extend/>"));
    assert_eq!(xml.matches("<measure number=").count(), 2);
}

#[test]
fn concert_pitch_suit_le_style_de_la_partition() {
    let mut score = Score::new();
    let (part_id, staff) = piano(&mut score);
    // Piccolo : +12 chromatique, hauteur sonnante D5 (74), écrite D4.
    score.part_mut(part_id).instrument.transpose = Some(Transpose::new(0, 12));
    score.style.concert_pitch = true;
    let piste = Track::new(staff, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_accord(
        &mut score,
        &mut measure,
        piste,
        Fraction::ZERO,
        Duration::whole(),
        &[(74, tpc::D)],
    );
    score.add_measure(measure);

    // Sans surcharge, le style enregistré décide (exportmusicxml.cpp:1369,
    // 8944 et pitch2xml 1654).
    let xml = export_musicxml(&score).expect("export");
    assert!(
        xml.contains("<concert-score/>"),
        "style concertPitch : {xml}"
    );
    assert!(
        xml.contains("<step>D</step>\n          <octave>5</octave>"),
        "hauteur sonnante : {xml}"
    );
    // En hauteurs réelles, la transposition devient `<for-part><part-transpose>`
    // (`writeInstrumentDetails`), comme dans la référence 4.7.4.
    assert!(xml.contains("<part-transpose>"));

    // Une surcharge explicite de l'appelant gagne toujours sur le style.
    let ecrit = ExportOptions {
        concert_pitch: Some(false),
        ..ExportOptions::default()
    };
    let xml = export_musicxml_with(&score, &ecrit).expect("export");
    assert!(!xml.contains("<concert-score/>"));
    assert!(xml.contains("<transpose>"));
    assert!(
        xml.contains("<step>D</step>\n          <octave>4</octave>"),
        "hauteur écrite : {xml}"
    );
}

/// `partList` (`exportmusicxml.cpp:7953-7960`) ferme les groupes de parties en
/// parcourant `partGroupEnd` de la fin vers le début (pile). Le portage les
/// fermait dans l'ordre croissant : premier écart de 11 partitions réelles.
#[test]
fn fermeture_des_groupes_de_parties_en_ordre_decroissant() {
    let mut score = Score::new();
    let p1 = score.add_part(Part::new("Bois", "Bois"));
    let s1 = score.add_staff(p1);
    let p2 = score.add_part(Part::new("Cuivres", "Cuiv."));
    let s2 = score.add_staff(p2);
    // Deux colonnes d'accolades couvrant les deux parties : deux groupes
    // s'ouvrent et doivent se fermer dans l'ordre inverse.
    score.staff_mut(s1).brackets.push(StaffBracket {
        kind: 0,
        span: 2,
        visible: true,
    });
    score.staff_mut(s1).brackets.push(StaffBracket {
        kind: 2,
        span: 2,
        visible: true,
    });
    for staff in [s1, s2] {
        let piste = Track::new(staff, 0);
        let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
        ajouter_silence_mesure(&mut score, &mut measure, piste, Fraction::ZERO);
        score.add_measure(measure);
    }

    let xml = export_musicxml(&score).expect("export");
    let stops: Vec<&str> = xml
        .lines()
        .filter(|line| line.contains("<part-group type=\"stop\""))
        .collect();
    assert_eq!(stops.len(), 2, "deux groupes à fermer : {xml}");
    assert!(
        stops[0].contains("number=\"2\""),
        "le groupe 2 doit se fermer en premier : {xml}"
    );
    assert!(
        stops[1].contains("number=\"1\""),
        "le groupe 1 doit se fermer en second : {xml}"
    );
}

/// `XmlWriter` de MuseScore écrit les réels avec six chiffres significatifs :
/// `<volume>3.93701</volume>` pour un contrôleur à 5 (`exportmusicxml.cpp:7353`,
/// `volume / 127 * 100`). Quatre décimales fixes perdaient le dernier chiffre.
#[test]
fn volume_en_chiffres_significatifs() {
    let mut score = Score::new();
    let (part_id, staff) = piano(&mut score);
    score.part_mut(part_id).instrument.volume = Some(5);
    let piste = Track::new(staff, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_silence_mesure(&mut score, &mut measure, piste, Fraction::ZERO);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<volume>3.93701</volume>"), "{xml}");

    // Cas non concerné : une valeur entière reste entière (100/127*100 n'est
    // pas entier ; on vérifie la fraction usuelle 78.7402).
    score.part_mut(part_id).instrument.volume = Some(100);
    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<volume>78.7402</volume>"), "{xml}");
}

/// `writeStaffDetails` (`exportmusicxml.cpp:8059-8104`) écrit aussi les
/// portées masquées (`hideEmptyStaves`), y compris à 5 lignes, avec
/// `print-object="no"` et `<staff-lines>`. Le portage ignorait ces portées.
/// Cas réels : 72 fichiers du lot `.mscz` (Marusia, Veni veni Emmanuel…).
#[test]
fn portee_masquee_ecrit_staff_details() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part_id);
    score.staff_mut(staff).visible = false;
    let track = Track::new(score.staff(staff).id, 0);
    let mut measure = ajouter_mesure(1, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(
        xml.contains("<staff-details print-object=\"no\">"),
        "staff-details de la portée masquée absent :\n{xml}"
    );
    assert!(
        xml.contains("<staff-lines>5</staff-lines>"),
        "staff-lines de la portée masquée absent :\n{xml}"
    );
}

/// `MeasureNumberStateHandler::updateForMeasure` (`exportmusicxml.cpp:8288-8300`)
/// ajoute `measureNumberOffset()` au numéro courant. Une partition extraite
/// d'une grande œuvre garde ainsi ses numéros d'origine (`<noOffset>494</noOffset>`
/// dans le `.mscx`, cas réel : les 9 fichiers Vivaldi Dixit Dominus).
#[test]
fn numero_de_mesure_avec_decalage_nooffset() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part_id);
    let track = Track::new(score.staff(staff).id, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    measure.number_offset = 494;
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(
        xml.contains("<measure number=\"495\">"),
        "décalage de numérotation perdu :\n{xml}"
    );
}

/// Un saut de section avec `startWithMeasureOne="0"` ne relance pas la
/// numérotation (`exportmusicxml.cpp:8274-8286`). Cas réel : Vivaldi, RV 594,
/// « Dominus fredipi » (`noOffset` 317 puis 325).
#[test]
fn saut_de_section_sans_reprise_garde_le_numero() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part_id);
    let track = Track::new(score.staff(staff).id, 0);
    for index in 0..2 {
        let mut measure = ajouter_mesure(
            index,
            Fraction::from_integer(index as i64),
            Fraction::ONE,
            Fraction::ONE,
        );
        if index == 0 {
            measure.number_offset = 317;
        } else {
            measure.section_break_before = true;
            measure.section_keeps_numbering = true;
            measure.number_offset = 325;
        }
        ajouter_clef(&mut score, &mut measure, track, ClefType::G);
        ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);
        score.add_measure(measure);
    }

    let xml = export_musicxml(&score).expect("export");
    assert!(
        xml.contains("<measure number=\"644\">"),
        "la numérotation doit continuer après le saut :\n{xml}"
    );
    assert_eq!(
        xml.matches("<measure number=\"318\">").count(),
        1,
        "le saut sans startWithMeasureOne ne doit pas relancer la numérotation"
    );
}

/// `midiInstrument` (`exportmusicxml.cpp:7337-7346`) utilise le canal alloué
/// au **premier** `<Channel>` de l'instrument dans `rebuildMidiMapping`, qui
/// parcourt tous les canaux secondaires (mute, harmony…). Trois canaux par
/// instrument décalent donc le canal exporté de 0, 3, 6… Cas réel :
/// `Amazing Grace - String Quartet` (canaux 1, 4, 7, 11).
#[test]
fn canaux_midi_des_canaux_secondaires() {
    let mut score = Score::new();
    for index in 0..2 {
        let part_id = score.add_part(Part::new(format!("Part{index}"), "P"));
        score.part_mut(part_id).instrument.channel_count = 3;
        let staff = score.add_staff(part_id);
        let track = Track::new(score.staff(staff).id, 0);
        let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
        ajouter_clef(&mut score, &mut measure, track, ClefType::G);
        ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);
        score.add_measure(measure);
    }

    let xml = export_musicxml(&score).expect("export");
    let canaux: Vec<&str> = xml
        .lines()
        .filter_map(|line| line.trim().strip_prefix("<midi-channel>"))
        .map(|line| line.split('<').next().unwrap_or(""))
        .collect();
    assert_eq!(canaux, vec!["1", "4"], "canaux exportés : {canaux:?}");
}

/// Une clef de reprise (`after-barline`) ne doit jamais être écrite dans le
/// même `<attributes>` que `<staff-details>` : la XSD 4.0 exige toutes les
/// clefs avant les `<staff-details>`.Cas réel : Sancta Maria KV 273.
#[test]
fn clef_apres_staff_details_interdite() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part_id);
    score.staff_mut(staff).visible = false;
    let track = Track::new(score.staff(staff).id, 0);

    let mut first = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut first, track, ClefType::G);
    ajouter_silence_mesure(&mut score, &mut first, track, Fraction::ZERO);
    score.add_measure(first);

    let mut second = ajouter_mesure(1, Fraction::ONE, Fraction::ONE, Fraction::ONE);
    second.section_break_before = true;
    let mut segment = Segment::new(SegmentType::Clef, Fraction::ONE, score.staves.len());
    segment.set_element(track, SegmentItem::Clef(Clef::new(ClefType::F)));
    second.add_segment(segment);
    ajouter_silence_mesure(&mut score, &mut second, track, Fraction::ZERO);
    score.add_measure(second);

    let xml = export_musicxml(&score).expect("export");
    for block in xml.split("<attributes>").skip(1) {
        let block = block.split("</attributes>").next().unwrap_or("");
        if let (Some(details), Some(clef)) = (
            block.find("<staff-details"),
            block.find("<clef after-barline"),
        ) {
            assert!(
                clef < details,
                "clef après staff-details dans le même attributes :\n{block}"
            );
        }
    }
}

/// `Attributes::doAttr` garde `<attributes>` ouvert entre `writeStaffDetails`
/// et `measureStyle` : MuseScore écrit `<measure-style>` dans le même bloc que
/// `<staff-details>` (`exportmusicxml.cpp:8691-8710`). Les fichiers Il est né…
/// et Les anges dans nos campagnes… montrent cet ordre (39 fichiers du lot).
#[test]
fn measure_style_dans_le_bloc_staff_details() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part_id);
    score.staff_mut(staff).visible = false;
    let track = Track::new(score.staff(staff).id, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    measure.multi_measure_rest = Some(3);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    let same_block = xml.split("<attributes>").skip(1).any(|block| {
        let block = block.split("</attributes>").next().unwrap_or("");
        block.contains("<staff-details") && block.contains("<measure-style>")
    });
    assert!(
        same_block,
        "measure-style doit rester dans le bloc staff-details :\n{xml}"
    );
}

/// `Tuplet::calcHasBracket` (`tuplet.cpp:204-268`) supprime le crochet d'un
/// n-olet entièrement ligaturé (croches ou plus courtes). Le portage écrivait
/// le crochet par défaut. Cas réels : Buenos_dias (15 fichiers). Un n-olet de
/// noires non ligaturables garde son crochet (7 fichiers).
#[test]
fn crochet_de_nolet_sans_bracket_type() {
    for (duration, expected) in [(Duration::eighth(), "no"), (Duration::quarter(), "yes")] {
        let mut score = Score::new();
        let part_id = score.add_part(Part::new("Voix", "Vx"));
        let staff = score.add_staff(part_id);
        let track = Track::new(score.staff(staff).id, 0);
        let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
        ajouter_clef(&mut score, &mut measure, track, ClefType::G);
        let tuplet = score.add_tuplet(Tuplet::new(
            Fraction::new(3, 2),
            duration.ticks(),
            Fraction::ZERO,
            Fraction::ONE,
        ));
        for index in 0..3 {
            let note = score.add_note(Note::new(62 + index as u8, tpc::D));
            let mut chord_rest = ChordRest::chord(duration);
            chord_rest.tuplet = Some(tuplet);
            chord_rest.chord_mut().expect("accord").notes.push(note);
            ajouter_element(
                &mut score,
                &mut measure,
                track,
                Fraction::new(index as i64, 3),
                chord_rest,
            );
        }
        score.add_measure(measure);

        let xml = export_musicxml(&score).expect("export");
        assert!(
            xml.contains(&format!("<tuplet type=\"start\" bracket=\"{expected}\"")),
            "crochet attendu {expected} pour {duration:?} :\n{xml}"
        );
    }
}

/// `Rest::checkDots()` n'est appelé que par le layout de la portée
/// (`restlayout.cpp:148`) : une portée masquée (`<isStaffVisible>0`) n'a donc
/// aucun `NoteDot`, et `writeRest` (`exportmusicxml.cpp:4655-4669`) n'écrit
/// alors aucun `<dot/>`, même pour une figure pointée. Cas réel : les 155
/// points supprimés de `Minuit chrétien` sont sur des portées masquées.
fn silence_pointee(visible: bool) -> String {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part_id);
    score.staff_mut(staff).visible = visible;
    let track = Track::new(score.staff(staff).id, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_element(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        ChordRest::rest(Duration::with_dots(DurationType::Half, 1)),
    );
    score.add_measure(measure);
    export_musicxml(&score).expect("export")
}

#[test]
fn dot_silence_portee_masquee_supprime() {
    let xml = silence_pointee(false);
    assert!(!xml.contains("<dot/>"), "point sur portée masquée : {xml}");
}

#[test]
fn dot_silence_portee_visible_conserve() {
    let xml = silence_pointee(true);
    assert!(
        xml.contains("<dot/>"),
        "point sur portée visible perdu : {xml}"
    );
}

/// `writeStaffDetails` (`exportmusicxml.cpp:8139-8144`) écrit `<staff-size>`
/// quand le facteur d'agrandissement diffère de 1 : une portée réduite
/// (`<small>1</small>`, mag 0.7) donne `<staff-size scaling="70">70</staff-size>`.
/// Cas réels : Sancta Maria KV 273 (4 occurrences), Gloria Patri de Vivaldi (8).
#[test]
fn staff_size_des_portees_reduites() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part_id);
    score.staff_mut(staff).small = true;
    let track = Track::new(score.staff(staff).id, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    ajouter_silence_mesure(&mut score, &mut measure, track, Fraction::ZERO);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(
        xml.contains("<staff-size scaling=\"70\">70</staff-size>"),
        "staff-size de la portée réduite absent :\n{xml}"
    );
}

/// Un triolet de doubles-croches contenant un silence garde son crochet sur
/// une portée visible (`tuplet.cpp:232-234`) mais pas sur une portée masquée,
/// que la mise en page ne dispose pas (`hasBracket` reste faux). Douce nuit
/// (12 fichiers) montre les deux cas : portée Femmes visible (31 crochets)
/// et portées masquées (31 triolets identiques sans crochet).
fn tuplet_avec_silence(visible: bool) -> String {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part_id);
    score.staff_mut(staff).visible = visible;
    let track = Track::new(score.staff(staff).id, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    ajouter_clef(&mut score, &mut measure, track, ClefType::G);
    let tuplet = score.add_tuplet(Tuplet::new(
        Fraction::new(3, 2),
        Duration::new(DurationType::Sixteenth).ticks(),
        Fraction::ZERO,
        Fraction::ONE,
    ));
    for index in 0..3 {
        let mut chord_rest = if index == 0 {
            ChordRest::rest(Duration::new(DurationType::Sixteenth))
        } else {
            let note = score.add_note(Note::new(62 + index as u8, tpc::D));
            let mut chord = ChordRest::chord(Duration::new(DurationType::Sixteenth));
            chord.chord_mut().expect("accord").notes.push(note);
            chord
        };
        chord_rest.tuplet = Some(tuplet);
        ajouter_element(
            &mut score,
            &mut measure,
            track,
            Fraction::new(index as i64, 24),
            chord_rest,
        );
    }
    score.add_measure(measure);
    export_musicxml(&score).expect("export")
}

#[test]
fn crochet_triolet_silence_portee_visible() {
    let xml = tuplet_avec_silence(true);
    assert!(
        xml.contains("<tuplet type=\"start\" bracket=\"yes\""),
        "crochet attendu sur portée visible :\n{xml}"
    );
}

#[test]
fn crochet_triolet_silence_portee_masquee() {
    let xml = tuplet_avec_silence(false);
    assert!(
        xml.contains("<tuplet type=\"start\" bracket=\"no\""),
        "crochet supprimé attendu sur portée masquée :\n{xml}"
    );
}

/// MuseScore 4.7.4 exporte une altération `<bracket>1</bracket>` avec rôle
/// utilisateur en `cautionary="yes" parentheses="yes"` (références : Ave
/// Verum, Angels Carol). Le C++ 5.0 (`exportmusicxml.cpp:2866-2870`) écrit
/// `editorial`/`bracket`, mais la référence 4.7.4 fait foi.
#[test]
fn alteration_crochetee_et_utilisateur() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    let ids = ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        Duration::whole(),
        &[(61, tpc::C)],
    );
    let mut accidental = Accidental::new(AccidentalType::Sharp);
    accidental.bracket = true;
    accidental.user = true;
    score.note_mut(ids[0]).accidental = Some(accidental);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(
        xml.contains("<accidental cautionary=\"yes\" parentheses=\"yes\">sharp</accidental>"),
        "altération user+crochet mal exportée :\n{xml}"
    );
}

/// `writeAccidental` (`exportmusicxml.cpp:2887-2892`) écrit `size="cue"`
/// quand l'altération est plus petite que la tête de note (`<small>1</small>`).
/// Cas réel : JMHaydn Viderunt omnes (5 altérations réduites).
#[test]
fn alteration_reduite_size_cue() {
    let mut score = Score::new();
    let (_, staff) = piano(&mut score);
    let track = Track::new(staff, 0);
    let mut measure = ajouter_mesure(0, Fraction::ZERO, Fraction::ONE, Fraction::ONE);
    let ids = ajouter_accord(
        &mut score,
        &mut measure,
        track,
        Fraction::ZERO,
        Duration::whole(),
        &[(61, tpc::C)],
    );
    let mut accidental = Accidental::new(AccidentalType::Flat);
    accidental.small = true;
    score.note_mut(ids[0]).accidental = Some(accidental);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert!(
        xml.contains("<accidental size=\"cue\">flat</accidental>"),
        "taille d'altération perdue :\n{xml}"
    );
}
