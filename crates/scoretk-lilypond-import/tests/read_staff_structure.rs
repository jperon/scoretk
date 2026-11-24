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

//! Structure des portées d'un groupe et durées nues.
//!
//! Réductions de `ra`/`mut` : éléments simultanés d'un `StaffGroup`
//! (`lily/parser.yy` `context_prefix`), `\context ChordNames` enfant d'un
//! `Staff` (`ly/engraver-init.ly` l. 492), déclaration `\context Lyrics = nom`
//! (`Abendlied`), `SemiChoirStaff` défini par `\layout`, et `\breve`/`\longa`
//! employés comme durées nues (`monochords`).

use scoretk_core::model::{ChordRestKind, Score, SegmentItem};

fn notes(score: &Score) -> Vec<u8> {
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

fn layout(score: &Score) -> Vec<(usize, u8)> {
    let mut resultat = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (track, item) in segment.iter_elements() {
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                if !chord.notes.is_empty() {
                    resultat.push((track.staff().index(), track.voice()));
                }
            }
        }
    }
    resultat.sort();
    resultat.dedup();
    resultat
}

#[test]
fn nouveau_staff_dans_un_sim_devient_une_portee_soeur() {
    // `\new Staff` dans le `<< … >>` d'une portée : LilyPond 2.27 joue trois
    // attaques (deux sur la portée d'origine, une sur la nouvelle portée),
    // dont deux unissons au même tick sur des pistes distinctes. Le lecteur
    // les réunissait en une seule portée et n'en gardait qu'une.
    let source = "\\version \"2.24.0\"\n\
        \\score { << \\new Staff { c'1 << { c'1 } \\new Staff { c'1 } >> } >> }";
    let score = scoretk_lilypond_import::read_lilypond(source).expect("lecture");
    assert_eq!(notes(&score), vec![60, 60, 60]);
    let dispositions = layout(&score);
    assert!(
        dispositions.iter().any(|(staff, _)| *staff == 1),
        "la nouvelle portée doit porter sa propre piste : {dispositions:?}"
    );
}

#[test]
fn staff_group_bare_items_ouvrent_une_portee_chacun() {
    // `\new StaffGroup << { c'1 } { e'1 } >>` : le groupe accepte des
    // `Staff`, chaque élément simultané sans contexte en crée un. LilyPond
    // 2.27 écrit deux pistes MIDI.
    let source = "\\version \"2.24.0\"\n\
        \\score { \\new StaffGroup << { c'1 } { e'1 } >> }";
    let score = scoretk_lilypond_import::read_lilypond(source).expect("lecture");
    assert_eq!(score.parts.len(), 1, "une partie (le StaffGroup)");
    assert_eq!(score.staves.len(), 2, "deux portées");
    assert_eq!(notes(&score), vec![60, 64]);
}

#[test]
fn double_antislash_dans_un_groupe_ne_fait_qu_une_portee() {
    // `\\` (VoiceSeparator) crée deux `Voice` dans la portée courante, comme
    // au niveau du score : une piste MIDI sur LilyPond 2.27.
    let source = "\\version \"2.24.0\"\n\
        \\score { \\new StaffGroup << { c'1 } \\\\ { e'1 } >> }";
    let score = scoretk_lilypond_import::read_lilypond(source).expect("lecture");
    assert_eq!(score.parts.len(), 1);
    assert_eq!(score.staves.len(), 1, "une seule portée");
    assert_eq!(notes(&score), vec![60, 64]);
}

#[test]
fn chord_names_dans_un_staff_est_une_portee_soeur() {
    // `\context Staff { << { \context ChordNames \grille } { \notes } >> }` :
    // `Staff` accepte `ChordNames` (`ly/engraver-init.ly` l. 492) ; c'est une
    // piste MIDI distincte, comme dans `LullabyBrahms-C`.
    let source = "\\version \"2.24.0\"\n\
        grille = \\chordmode { d1:m7 }\n\
        \\score { \\context Staff << { \\context ChordNames \\grille } { c'1 } >> }";
    let score = scoretk_lilypond_import::read_lilypond(source).expect("lecture");
    assert_eq!(score.parts.len(), 1);
    assert_eq!(score.staves.len(), 2, "la grille et la voix");
    let portees: Vec<usize> = layout(&score).iter().map(|(staff, _)| *staff).collect();
    assert!(portees.contains(&0) && portees.contains(&1), "{portees:?}");
}

#[test]
fn declaration_de_paroles_ne_mange_pas_la_portee_suivante() {
    // `\context Lyrics = SopranX` sans paroles, puis `\context Staff = women` :
    // `Abendlied`. Les paroles déclarées ne doivent pas avaler le contexte
    // suivant ; deux portées, deux voix chacune.
    let source = "\\version \"2.24.0\"\n\
        \\score { \\context ChoirStaff <<\n\
          \\context Lyrics = SopranX\n\
          \\context Staff = women << \\context Voice = S { \\voiceOne { c''1 } }\n\
            \\context Voice = A { \\voiceTwo { e'1 } } >>\n\
          \\context Staff = men << \\context Voice = T { \\voiceOne { g1 } }\n\
            \\context Voice = B { \\voiceTwo { c1 } } >>\n\
        >> }";
    let score = scoretk_lilypond_import::read_lilypond(source).expect("lecture");
    assert_eq!(score.parts.len(), 1);
    assert_eq!(score.staves.len(), 2, "women et men");
    assert_eq!(notes(&score), vec![72, 64, 55, 48]);
}

#[test]
fn semichoirstaff_est_un_groupe_de_portees() {
    // `SemiChoirStaff` est un `ChoirStaff` renommé par
    // `\layout { \context { \ChoirStaff \name "SemiChoirStaff" } }`
    // (`Nunc-dimittis`) : ses `\new Staff` sont des portées, pas des voix.
    let source = "\\version \"2.24.0\"\n\
        \\layout { \\context { \\ChoirStaff \\name \"SemiChoirStaff\" } }\n\
        \\score { \\new SemiChoirStaff << \\new Staff { c'1 } \\new Staff { e'1 } >> }";
    let score = scoretk_lilypond_import::read_lilypond(source).expect("lecture");
    assert_eq!(score.parts.len(), 1);
    assert_eq!(score.staves.len(), 2, "deux portées");
    assert_eq!(notes(&score), vec![60, 64]);
}
