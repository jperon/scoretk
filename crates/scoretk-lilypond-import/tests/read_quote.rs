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

//! `\quoteDuring` / `\cueDuring` : citation au temps absolu de la partition.
//!
//! `Quote_iterator::process` (`lily/quote-iterator.cc:161-211`) calcule
//! `start_mom = now_mom()` et ne joue que les événements de la citation dans
//! `[start_mom, start_mom + durée de l'argument principal)`. Une citation
//! placée après une ronde ne reprend donc pas la citation à son début.
//! Réduction de `input/regression/quote-cue-during.ly`.

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
fn citation_au_temps_absolu_de_la_partition() {
    // La citation commence à la mesure 2 (4 noires) : elle joue les notes de
    // la citation à 4, 5, 6 et 7 noires (g, a, b, c), pas son début.
    let source = "\\version \"2.24.0\"\n\
        quote = \\relative { c'4 d e f g a b c }\n\
        \\addQuote \"Q\" \\quote\n\
        \\score { \\relative { r1 \\quoteDuring \"Q\" { s1 } } }";
    let score = scoretk_lilypond_import::read_lilypond(source).expect("lecture");
    assert_eq!(notes(&score), vec![67, 69, 71, 72]);
}

#[test]
fn cue_during_suit_le_meme_temps_absolu() {
    let source = "\\version \"2.24.0\"\n\
        quote = \\relative { c'4 d e f g a b c }\n\
        \\addQuote \"Q\" \\quote\n\
        \\score { \\relative { r1 \\cueDuring \"Q\" #UP { s1 } } }";
    let score = scoretk_lilypond_import::read_lilypond(source).expect("lecture");
    assert_eq!(notes(&score), vec![67, 69, 71, 72]);
}

#[test]
fn cue_during_garde_la_musique_de_l_argument_principal() {
    // `Quote_iterator` ajoute les notes citées dans une `CueVoice` : les
    // notes écrites dans l'argument principal restent jouées. Réduction de
    // `adding-orchestral-cues-to-a-vocal-score.ly` (LSR), où `\cueWhile`
    // enveloppe `\cueDuring` et passe `c4. g8` comme musique principale.
    let source = "\\version \"2.24.0\"\n\
        quote = \\relative { c'4 d e f }\n\
        \\addQuote \"Q\" \\quote\n\
        \\score { \\relative { \\cueDuring \"Q\" #UP { g'4 a'4 } } }";
    let score = scoretk_lilypond_import::read_lilypond(source).expect("lecture");
    let mut resultat = notes(&score);
    resultat.sort_unstable();
    assert_eq!(resultat, vec![60, 62, 67, 81]);
}
