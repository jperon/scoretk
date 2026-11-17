// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les airs ci-dessous sont des mélodies traditionnelles du domaine public
// (France, Angleterre), transcrites pour le test. Aucune source externe n'est
// copiée.
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

//! Airs traditionnels écrits à la main : lecture, structure et aller-retour.
//!
//! Ces tests ne dépendent pas du corpus MuseScore : ils vérifient le
//! sous-ensemble ABC sur des airs courts, avec paroles, reprises, modes,
//! liaisons, phrasés, accords, agréments, nuances et annotations.

use mscz2xml::model::{KeyMode, SpannerType};

const AU_CLAIR: &str = "\
%abc-2.1
X:1
T:Au clair de la lune
C:Traditionnel
M:4/4
L:1/4
K:C
C C C D | E E D z |
w: Au clair de la lu- ne
C C C D | E E D z |
w: Mon a- mi Pier- rot
D D D D | E E D |
C C C D | E E D |]
";

const GREENSLEEVES: &str = "\
X:1
T:Greensleeves
M:6/8
L:1/8
K:Am
A2A c2d | e2e e2d | c2B A2B | c2B A2G |
A2A c2d | e2e e2d | c2B A2B | A4 z2 |]
";

const SUR_LE_PONT: &str = "\
X:1
T:Sur le pont d'Avignon
M:2/4
L:1/8
K:G
d2 d2 | B2 B2 | A2 A2 | G4 |
d2 d2 | B2 B2 | A2 A2 | G4 |]
";

const DEUX_MORCEAUX: &str = "\
X:1
T:Premier
M:4/4
L:1/8
K:C
C D E F | G4 |
X:2
T:Second
M:3/4
L:1/8
K:G
G A B | c4 |
";

const ORNEMENTS: &str = "\
X:1
T:Ornements
M:4/4
L:1/8
K:D
!p!\"D\" (A2B2) [DFA]2 {=c}B2- | B4 z4 |]
";

fn pitches(score: &mscz2xml::model::Score) -> Vec<u8> {
    score.notes.iter().map(|note| note.pitch).collect()
}

#[test]
fn au_clair_de_la_lune() {
    let score = scoretk_abc::read_abc(AU_CLAIR).expect("lecture");
    assert_eq!(score.metadata.work_title, "Au clair de la lune");
    assert_eq!(score.metadata.composer, "Traditionnel");
    assert_eq!(score.measures.len(), 8);
    assert_eq!(score.measures[0].nominal_len.as_f64(), 1.0);
    let first: Vec<u8> = score.measures[0]
        .segments
        .iter()
        .filter_map(|segment| segment.chord_rest(mscz2xml::model::Track::from_raw(0)))
        .filter_map(|chord_rest| chord_rest.chord_ref())
        .filter_map(|chord| chord.notes.first().copied())
        .map(|id| score.note(id).pitch)
        .collect();
    assert_eq!(first, vec![60, 60, 60, 62]);

    let lyrics: Vec<String> = score.measures[0]
        .segments
        .iter()
        .filter_map(|segment| segment.chord_rest(mscz2xml::model::Track::from_raw(0)))
        .flat_map(|chord_rest| chord_rest.lyrics.iter().map(|lyrics| lyrics.text.clone()))
        .collect();
    assert_eq!(lyrics, ["Au", "clair", "de", "la"]);
}

#[test]
fn greensleeves_en_la_mineur() {
    let score = scoretk_abc::read_abc(GREENSLEEVES).expect("lecture");
    assert_eq!(score.measures.len(), 8);
    let mut found_minor = false;
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                if let mscz2xml::model::SegmentItem::KeySig(key) = item {
                    assert_eq!(key.concert_key.fifths(), 0);
                    assert_eq!(key.mode, KeyMode::Minor);
                    found_minor = true;
                }
            }
        }
    }
    assert!(found_minor, "armure de la mineur absente");
    assert_eq!(pitches(&score).first(), Some(&69), "premier la 4");
}

#[test]
fn sur_le_pont_et_rythme() {
    let score = scoretk_abc::read_abc(SUR_LE_PONT).expect("lecture");
    assert_eq!(score.measures.len(), 8);
    // Mesure 2/4 : une blanche (`G4` avec L:1/8) dure une mesure.
    assert_eq!(score.measures[3].len.as_f64(), 0.5);
    assert_eq!(score.notes.len(), 14);
}

#[test]
fn deux_morceaux() {
    let premier = scoretk_abc::read_abc(DEUX_MORCEAUX).expect("premier");
    assert_eq!(premier.metadata.work_title, "Premier");
    assert_eq!(premier.measures.len(), 2);
    let second = scoretk_abc::read_abc_tune(DEUX_MORCEAUX, 1).expect("second");
    assert_eq!(second.metadata.work_title, "Second");
    assert_eq!(second.measures.len(), 2);
    assert!(scoretk_abc::read_abc_tune(DEUX_MORCEAUX, 2).is_err());
}

#[test]
fn directives_ignorees_avec_avertissement() {
    let source = "X:1\nM:4/4\nL:1/8\n%%MIDI program 1\nK:C\nC D E F |]\n";
    let output =
        scoretk_abc::read_abc_with(source, &scoretk_abc::ReadOptions::default()).expect("lecture");
    assert!(
        output
            .warnings
            .iter()
            .any(|warning| warning.message.to_lowercase().contains("%%midi")),
        "avertissement absent : {:?}",
        output.warnings
    );
}

#[test]
fn ornements_annotations_et_liaisons() {
    let score = scoretk_abc::read_abc(ORNEMENTS).expect("lecture");
    let mut chord = None;
    for segment in &score.measures[0].segments {
        if let Some(chord_rest) = segment.chord_rest(mscz2xml::model::Track::from_raw(0)) {
            if chord_rest.is_chord() {
                chord = Some(chord_rest);
                break;
            }
        }
    }
    let chord_rest = chord.expect("accord");
    assert!(chord_rest
        .chord_ref()
        .is_some_and(|chord| !chord.notes.is_empty()));
    assert!(score.measures[0]
        .segments
        .iter()
        .any(
            |segment| segment.annotations.iter().any(|annotation| matches!(
                annotation.annotation,
                mscz2xml::model::Annotation::Harmony(_)
            ))
        ));
    assert!(score
        .spanners
        .iter()
        .any(|spanner| matches!(spanner.kind, SpannerType::Slur)));
    assert!(score.notes.iter().any(|note| note.tie_forward.is_some()));
}

#[test]
fn aller_retour_abc_sur_les_airs() {
    for source in [AU_CLAIR, GREENSLEEVES, SUR_LE_PONT, ORNEMENTS] {
        let first = scoretk_abc::read_abc(source).expect("lecture");
        let abc = scoretk_abc::write_abc(&first).expect("écriture");
        let second = scoretk_abc::read_abc(&abc).expect("relecture");
        assert_eq!(
            first.measures.len(),
            second.measures.len(),
            "nombre de mesures instable"
        );
        assert_eq!(
            first.notes.len(),
            second.notes.len(),
            "nombre de notes instable\n{abc}"
        );
        let first_pitches: Vec<u8> = first.notes.iter().map(|note| note.pitch).collect();
        let second_pitches: Vec<u8> = second.notes.iter().map(|note| note.pitch).collect();
        assert_eq!(first_pitches, second_pitches, "hauteurs instables\n{abc}");
    }
}
