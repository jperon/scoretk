// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Le décalage d'octave d'une clef (`clef=treble-8`) ne doit pas être cumulé
//! dans le champ `V: transpose=` à chaque aller-retour : la clef porte déjà
//! l'information. `xml2abc.py::doAttr` (lignes 1319-1331) écrit les deux, mais
//! le lecteur scoretk ne doit additionner le suffixe qu'aux hauteurs.

#![cfg(all(feature = "abc-import", feature = "abc-export"))]

#[test]
fn transpose_pas_double_par_clef_octave() {
    let source =
        "X:1\nM:4/4\nL:1/4\nK:C\nV:1 clef=treble transpose=-24\nC D|\n[K:C clef=treble-8] E F|\n";
    let first = scoretk_abc_import::read_abc(source).expect("lecture");
    let abc = scoretk_abc_export::write_abc(&first).expect("écriture");
    assert!(
        abc.contains("transpose=-24"),
        "transpose initial perdu :\n{abc}"
    );
    let second = scoretk_abc_import::read_abc(&abc).expect("relecture");
    let abc2 = scoretk_abc_export::write_abc(&second).expect("réécriture");
    assert_eq!(abc, abc2, "aller-retour instable :\n{abc}\n---\n{abc2}");
}
