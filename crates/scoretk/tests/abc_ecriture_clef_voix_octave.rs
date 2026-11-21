// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Un suffixe d'octave sur la clef d'un champ `V:` (`clef=treble-8`) décale
//! les hauteurs comme celui d'un `K:` (ABC 2.1 § 4.6). Le lecteur ne
//! l'appliquait pas : la hauteur lue était un octave trop haut, puis
//! l'écrivain compensait et la hauteur montait d'un octave à chaque
//! aller-retour (`G4` → `g4` → `g'4`).

#![cfg(all(feature = "abc-import", feature = "abc-export"))]

#[test]
fn octave_clef_champ_v_stable() {
    let source = "X:1\nM:4/4\nL:1/8\nK:Eb\n%%score 1 2 3\nV:1 name=\"S\" clef=treble\n|: E4 |\nV:2 name=\"A\" clef=treble\n|: F4 |\nV:3 name=\"T\" clef=treble-8\n|: G4B4 |\n";
    let first = scoretk_abc_import::read_abc(source).expect("lecture");
    let pitches: Vec<u8> = first.notes.iter().map(|n| n.pitch).collect();
    assert_eq!(
        &pitches[2..],
        &[55, 58],
        "octave de la clef V: non appliquée : {pitches:?}"
    );
    let abc = scoretk_abc_export::write_abc(&first).expect("écriture");
    assert!(
        abc.contains("G4B4"),
        "hauteur écrite avec un octave de trop :\n{abc}"
    );
    let second = scoretk_abc_import::read_abc(&abc).expect("relecture");
    let abc2 = scoretk_abc_export::write_abc(&second).expect("réécriture");
    assert_eq!(abc, abc2, "aller-retour instable :\n{abc}\n---\n{abc2}");
}
