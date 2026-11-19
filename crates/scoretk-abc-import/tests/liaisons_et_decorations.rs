// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce test est une réécriture Rust originale : aucune ligne de `abc2xml.py`
// n'est copiée. Les airs sont écrits à la main pour l'occasion.
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

//! Liaisons entre hauteurs différentes et récupération d'une décoration
//! non fermée, telles que `abc2xml.py` 269 les traite.

use scoretk_abc_import::{read_abc, read_abc_with, ReadOptions};
use scoretk_core::model::{OrnamentType, Score, StaffId, Track};

/// Hauteurs, durées et drapeaux de liaison d'une voix, dans l'ordre du fichier.
fn voice_content(score: &Score, staff: usize, voice: u8) -> (Vec<u8>, Vec<f64>, Vec<(bool, bool)>) {
    let track = Track::new(StaffId::from_index(staff), voice);
    let mut pitches = Vec::new();
    let mut durations = Vec::new();
    let mut ties = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            if let Some(chord_rest) = segment.chord_rest(track) {
                if let Some(chord) = chord_rest.chord_ref() {
                    for id in &chord.notes {
                        let note = score.note(*id);
                        pitches.push(note.pitch);
                        ties.push((note.tie_back.is_some(), note.tie_forward.is_some()));
                    }
                }
                durations.push(chord_rest.duration.ticks().as_f64());
            }
        }
    }
    (pitches, durations, ties)
}

#[test]
fn liaison_entre_hauteurs_differentes() {
    // Musique ordinaire : deux voix, barres, accords, silence, liaisons
    // valides (c-c, [CE]-[CE]) et liaisons invalides (G-A-G, [DF]-[CE]-[DF]).
    let source = "\
X:1
T:Liaisons
M:4/4
L:1/8
K:C
V:1
G2-A2 G2 c2 | c2-c2 [CE]2-[CE]2 | [DF]2-[CE]2 [DF]2 z2 |]
V:2
C2 D2 E2 F2 | G2 A2 B2 c2 | C2 D2 E2 F2 |]
";
    let score = read_abc(source).expect("lecture");

    assert_eq!(score.measures.len(), 3, "trois mesures dans chaque voix");
    assert_eq!(
        voice_content(&score, 0, 0),
        (
            vec![
                67, 69, 67, 72, // G-A-G-c
                72, 72, 60, 64, 60, 64, // c-c, [CE]-[CE]
                62, 65, 60, 64, 62, 65, // [DF]-[CE]-[DF]
            ],
            vec![0.25; 12],
            vec![
                (false, false), // G : la liaison vers A est abandonnée
                (false, false), // A
                (false, false), // G : ne doit pas hériter de la liaison de G
                (false, false), // c
                (false, true),  // c : début de liaison valide
                (true, false),  // c : fin de liaison valide
                (false, true),  // C de [CE] : début
                (false, true),  // E de [CE] : début
                (true, false),  // C de [CE] : fin
                (true, false),  // E de [CE] : fin
                (false, false), // D de [DF] : liaison vers [CE] abandonnée
                (false, false), // F de [DF]
                (false, false), // C de [CE]
                (false, false), // E de [CE]
                (false, false), // D de [DF] : ne doit pas hériter de la liaison
                (false, false), // F de [DF]
            ]
        )
    );

    // Cas « non concerné » : la deuxième voix n'a aucune liaison et garde
    // exactement ses hauteurs écrites.
    let (pitches, durations, ties) = voice_content(&score, 1, 0);
    assert_eq!(
        pitches,
        vec![60, 62, 64, 65, 67, 69, 71, 72, 60, 62, 64, 65]
    );
    assert_eq!(durations, vec![0.25; 12]);
    assert!(ties.iter().all(|(back, forward)| !back && !forward));
}

#[test]
fn decoration_non_fermee() {
    // Musique ordinaire : deux voix, accord, silence, barres. Le `!` placé
    // devant G2 n'a pas de fermant : abc2xml.py le traite comme saut de ligne
    // (`linebreak = ~decorations + '!'`, ligne 432), pas comme une décoration
    // avalant toute la fin de la ligne.
    let source = "\
X:1
T:Orphelin
M:4/4
L:1/8
K:C
V:1
C2 D2 E2 F2 | !G2 [AB]2 z2 c2 |
V:2
C D E F | G A B c |]
";
    let output = read_abc_with(source, &ReadOptions::default()).expect("lecture");
    let score = &output.score;

    assert_eq!(score.measures.len(), 2);
    let (pitches, durations, _) = voice_content(score, 0, 0);
    assert_eq!(pitches, vec![60, 62, 64, 65, 67, 69, 71, 72]);
    assert_eq!(durations, vec![0.25; 8]);
    // L'autre voix est lue entièrement, elle aussi.
    assert_eq!(
        voice_content(score, 1, 0).0,
        vec![60, 62, 64, 65, 67, 69, 71, 72]
    );
    assert!(
        output
            .warnings
            .iter()
            .any(|warning| warning.message.contains("non fermée")),
        "avertissement absent : {:?}",
        output.warnings
    );

    // Cas limite : `!` en fin de fichier, sans aucune note après.
    let eof = "X:1\nT:Fin\nM:4/4\nL:1/8\nK:C\nC D E !\n";
    let output_eof = read_abc_with(eof, &ReadOptions::default()).expect("lecture fin de fichier");
    assert_eq!(
        output_eof
            .score
            .notes
            .iter()
            .map(|note| note.pitch)
            .collect::<Vec<_>>(),
        vec![60, 62, 64]
    );

    // Cas « non concerné » : une décoration correctement fermée garde son
    // effet et les mêmes hauteurs.
    let closed = source.replace("!G2", "!trill!G2");
    let output_closed = read_abc_with(&closed, &ReadOptions::default()).expect("lecture");
    assert_eq!(
        voice_content(&output_closed.score, 0, 0).0,
        vec![60, 62, 64, 65, 67, 69, 71, 72]
    );
    let trills = output_closed
        .score
        .measures
        .iter()
        .flat_map(|measure| &measure.segments)
        .filter_map(|segment| segment.chord_rest(Track::new(StaffId::from_index(0), 0)))
        .filter_map(|chord_rest| chord_rest.chord_ref())
        .flat_map(|chord| chord.ornaments.iter())
        .filter(|ornament| ornament.kind == OrnamentType::TrillMark)
        .count();
    assert_eq!(trills, 1, "le trill fermé reste une décoration");
}
