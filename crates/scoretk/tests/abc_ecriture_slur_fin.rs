// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Un `)` placé après une note clôt la liaison sur CETTE note, pas sur la
//! suivante : sinon l'extrémité avance d'une note à chaque aller-retour
//! (`(cde)f` → `(cdef)g` → `(cdefg)`). ABC 2.1 § 4.11.

#![cfg(all(feature = "abc-import", feature = "abc-export"))]

#[test]
fn fin_de_slur_sur_la_note_precedente() {
    let source = "X:1\nM:4/4\nL:1/8\nK:G\n(cde)f g|\n";
    let first = scoretk_abc_import::read_abc(source).expect("lecture");
    let abc = scoretk_abc_export::write_abc(&first).expect("écriture");
    assert!(abc.contains("(cde)f"), "slur trop long :\n{abc}");
    let second = scoretk_abc_import::read_abc(&abc).expect("relecture");
    let abc2 = scoretk_abc_export::write_abc(&second).expect("réécriture");
    assert_eq!(abc, abc2, "aller-retour instable :\n{abc}\n---\n{abc2}");
}
