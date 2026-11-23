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

//! `\killCues` : retire les notes de cue de `\cueDuring`, garde les citations
//! de `\quoteDuring` et le temps occupé.
//!
//! `ly/music-functions-init.ly:955-964` : `killCues` remplace les citations
//! dont `quoted-context-id` vaut `"cue"` par leur argument principal (des
//! silences) ; une citation ordinaire (`\quoteDuring`) n'est pas retirée.
//! Réduction de `input/regression/quote-kill-cues.ly`.

use scoretk_core::model::{ChordRestKind, SegmentItem};

fn notes(score: &scoretk_core::model::Score) -> Vec<u8> {
    let mut resultat = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_track, item) in segment.iter_elements() {
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                for id in &chord.notes {
                    resultat.push(score.note(*id).pitch);
                }
            }
        }
    }
    resultat
}

#[test]
fn kill_cues_retire_les_cues_et_garde_les_citations() {
    // Citation de quatre noires ; la cue commence après d2 et joue donc les
    // notes de la citation aux temps 2 et 4 (c, c), au temps absolu de la
    // partition (`lily/quote-iterator.cc`).
    let sans = "\\version \"2.24.0\"\n\
        mus = \\relative { c'2 c c c }\n\
        \\addQuote \"M\" \\mus\n\
        quot = \\relative { d'2 \\cueDuring \"M\" #UP { s1 } e2 }\n\
        \\score { << \\quot >> }";
    let score = scoretk_lilypond_import::read_lilypond(sans).expect("lecture");
    assert_eq!(notes(&score), vec![62, 60, 60, 64]);

    // `\killCues` retire les deux notes de cue, en gardant leur durée : e2
    // reste après le silence de quatre noires.
    let tue = "\\version \"2.24.0\"\n\
        mus = \\relative { c'2 c c c }\n\
        \\addQuote \"M\" \\mus\n\
        quot = \\relative { d'2 \\cueDuring \"M\" #UP { s1 } e2 }\n\
        \\score { << \\killCues \\quot >> }";
    let score = scoretk_lilypond_import::read_lilypond(tue).expect("lecture");
    assert_eq!(notes(&score), vec![62, 64]);

    // Une citation de `\quoteDuring` n'est pas une note de cue : elle reste.
    let quote = "\\version \"2.24.0\"\n\
        mus = \\relative { c'2 c c c }\n\
        \\addQuote \"M\" \\mus\n\
        quot = \\relative { d'2 \\quoteDuring \"M\" { s1 } e2 }\n\
        \\score { << \\killCues \\quot >> }";
    let score = scoretk_lilypond_import::read_lilypond(quote).expect("lecture");
    assert_eq!(notes(&score), vec![62, 60, 60, 64]);
}
