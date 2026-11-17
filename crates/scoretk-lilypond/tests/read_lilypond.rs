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

//! Petites sources LilyPond écrites à la main.
//!
//! Le corpus `tests/corpus/ly/` est produit par l'écrivain (donc en mode
//! `\fixed` et en anglais). Ces tests couvrent ce que l'écrivain n'émet pas :
//! notation relative, langue néerlandaise par défaut, variables, reprises,
//! transposition, paroles et commandes ignorées.

use mscz2xml::model::{
    ArticulationType, BarLineType, ChordRestKind, ClefType, DurationType, GraceType, KeyMode,
    LyricsSyllabic, SegmentItem, SegmentType, Track,
};
use scoretk_lilypond::{read_lilypond, read_lilypond_reporting};

/// Lit une musique dans une portée unique, en mode `\fixed c'`.
///
/// Le mode fixe place les noms non marqués au do central, comme le corpus ;
/// les tests d'octave pure appellent [`read_lilypond`] directement.
fn lire(music: &str) -> mscz2xml::model::Score {
    let source = format!(
        "\\version \"2.24.0\"\n\
         \\language \"english\"\n\
         \\score {{ \\new Staff {{ \\fixed c' {{ {music} }} }} }}\n"
    );
    read_lilypond(&source).unwrap_or_else(|error| panic!("lecture : {error}"))
}

/// Hauteurs des notes de l'arène, dans l'ordre.
fn hauteurs(score: &mscz2xml::model::Score) -> Vec<u8> {
    score.notes.iter().map(|note| note.pitch).collect()
}

#[test]
fn relative_et_octaves() {
    let score = lire("\\relative c' { c4 d e f g a b c }");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67, 69, 71, 72]);
    assert_eq!(score.measures.len(), 2);
}

#[test]
fn mode_fixe_et_absolu() {
    // `\fixed c'` : un nom sans marque vaut le do central.
    let score = lire("\\fixed c' { c4 c' c, }");
    assert_eq!(hauteurs(&score), vec![60, 72, 48]);
    // Notation absolue : `c` vaut deux octaves sous le do central.
    let score = read_lilypond("\\score { \\new Staff { c4 c' } }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![48, 60]);
}

#[test]
fn notation_neerlandaise_par_defaut() {
    // Sans `\language`, LilyPond lit le néerlandais : `cis`, `des`, `es`.
    let score = read_lilypond("\\score { \\new Staff { cis4 des4 es4 } }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![49, 49, 51]);
    let tpc: Vec<i8> = score.notes.iter().map(|note| note.tpc).collect();
    assert_eq!(
        tpc,
        vec![
            mscz2xml::model::tpc::C_SHARP,
            mscz2xml::model::tpc::D_FLAT,
            mscz2xml::model::tpc::E_FLAT
        ]
    );
}

#[test]
fn variables_et_reprise() {
    let score = lire("m = { c1 d1 } \\repeat volta 2 { \\m } e1");
    // Deux mesures de reprise, puis une mesure.
    assert_eq!(score.measures.len(), 3);
    assert_eq!(score.measures[0].start_bar_line, BarLineType::StartRepeat);
    assert_eq!(score.measures[1].end_bar_line, BarLineType::EndRepeat);
    assert_eq!(score.measures[1].repeat_count, 2);
    assert_eq!(hauteurs(&score), vec![60, 62, 64]);
}

#[test]
fn accords_articulations_et_liaison() {
    let score = lire("<c e g>4-. c4~ c4");
    assert_eq!(hauteurs(&score), vec![60, 64, 67, 60, 60]);
    let mesure = &score.measures[0];
    let segment = mesure
        .segments
        .iter()
        .find(|segment| segment.kind == SegmentType::ChordRest)
        .expect("segment");
    let Some(SegmentItem::ChordRest(chord_rest)) = segment.element(Track::from_raw(0)) else {
        panic!("accord attendu");
    };
    let chord = chord_rest.chord_ref().expect("accord");
    assert_eq!(chord.articulations.len(), 1);
    assert_eq!(chord.articulations[0].kind, ArticulationType::Staccato);
}

#[test]
fn note_relative_et_paroles() {
    let score = read_lilypond(
        r#"\score { \new Staff << \new Voice = "v" { \fixed c' { a4 b4 c'4 d'4 e'4 } } \addlyrics { A -- ve Ma -- ri -- a } >> }"#,
    )
    .expect("lecture");
    let syllabes: Vec<(LyricsSyllabic, String)> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
        .flat_map(|chord_rest| &chord_rest.lyrics)
        .map(|lyrics| (lyrics.syllabic, lyrics.text.clone()))
        .collect();
    assert_eq!(
        syllabes,
        vec![
            (LyricsSyllabic::Begin, "A".to_owned()),
            (LyricsSyllabic::End, "ve".to_owned()),
            (LyricsSyllabic::Begin, "Ma".to_owned()),
            (LyricsSyllabic::Middle, "ri".to_owned()),
            (LyricsSyllabic::End, "a".to_owned()),
        ]
    );
}

#[test]
fn transposition() {
    let score = lire("\\transpose c d { c4 }");
    let note = &score.notes[0];
    assert_eq!(note.pitch, 62);
    assert_eq!(note.tpc, mscz2xml::model::tpc::D);
    assert_eq!(note.tpc_transposed, mscz2xml::model::tpc::C);
}

#[test]
fn armure_clef_et_mesure() {
    let score = lire("\\clef bass \\key bf \\major \\time 3/4 R2. \\bar \"|.\"");
    assert_eq!(score.measures.len(), 1);
    assert_eq!(score.measures[0].len, scoretk_lilypond::Fraction::new(3, 4));
    assert_eq!(score.measures[0].end_bar_line, BarLineType::End);
    let segment = score.measures[0]
        .segments
        .iter()
        .find(|segment| segment.kind == SegmentType::HeaderClef)
        .expect("clef d'en-tête");
    let Some(SegmentItem::Clef(clef)) = segment.element(Track::from_raw(0)) else {
        panic!("clef attendue");
    };
    assert_eq!(clef.concert, ClefType::F);
    let segment = score.measures[0]
        .segments
        .iter()
        .find(|segment| segment.kind == SegmentType::KeySig)
        .expect("armure");
    let Some(SegmentItem::KeySig(key)) = segment.element(Track::from_raw(0)) else {
        panic!("armure attendue");
    };
    assert_eq!(key.concert_key.fifths(), -2);
    assert_eq!(key.mode, KeyMode::Unknown);
    // Silence de mesure entière.
    let Some(SegmentItem::ChordRest(chord_rest)) = score.measures[0]
        .segments
        .iter()
        .find(|segment| segment.kind == SegmentType::ChordRest)
        .and_then(|segment| segment.element(Track::from_raw(0)))
    else {
        panic!("silence attendu");
    };
    assert!(chord_rest.is_full_measure_rest());
}

#[test]
fn levees_et_n_olets() {
    let score = lire("\\partial 4 c4 | \\tuplet 3/2 { d8 e8 f8 } g2");
    assert_eq!(score.measures.len(), 2);
    assert_eq!(score.measures[0].len, scoretk_lilypond::Fraction::QUARTER);
    assert_eq!(score.measures[1].len, scoretk_lilypond::Fraction::ONE);
    assert!(score.measures[0].is_irregular());
    assert_eq!(score.tuplets.len(), 1);
    let tuplet = &score.tuplets[0];
    assert_eq!(tuplet.ratio, scoretk_lilypond::Fraction::new(3, 2));
    assert_eq!(tuplet.base_len, scoretk_lilypond::Fraction::new(1, 8));
}

#[test]
fn ornements() {
    let score = lire("\\grace { b8 } c4 \\afterGrace d4 { e16 }");
    let mut graces = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            if let Some(SegmentItem::ChordRest(chord_rest)) = segment.element(Track::from_raw(0)) {
                if let ChordRestKind::Chord(chord) = &chord_rest.kind {
                    graces.extend(
                        chord
                            .grace_chords
                            .iter()
                            .map(|grace| (grace.chord.grace, grace.duration.kind)),
                    );
                }
            }
        }
    }
    assert_eq!(
        graces,
        vec![
            (Some(GraceType::Appoggiatura), DurationType::Eighth),
            (Some(GraceType::Grace8After), DurationType::Sixteenth)
        ]
    );
}

#[test]
fn commandes_ignorees_avec_avertissement() {
    let source = "\\version \"2.24.0\"\n\
                  \\score { \\new Staff { c4 \\uneCommandeInconnue d4 #(ly:make-moment 1/4) e4 } }";
    let (score, warnings) = read_lilypond_reporting(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![48, 50, 52]);
    assert!(
        !warnings.is_empty(),
        "les commandes ignorées doivent être signalées"
    );
}
