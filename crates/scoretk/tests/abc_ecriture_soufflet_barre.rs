// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Un soufflet qui se termine au début de la mesure suivante doit écrire son
//! token de fin sur la première note de cette mesure, pas sur la dernière de
//! la précédente (où il se retrouverait collé au token de début).

#![cfg(all(feature = "abc-import", feature = "abc-export"))]

#[test]
fn soufflet_a_cheval_sur_barre_stable() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\n!>(!C2- | !>)!D2|\n";
    let first = scoretk_abc_import::read_abc(source).expect("lecture");
    let abc = scoretk_abc_export::write_abc(&first).expect("écriture");
    assert!(
        abc.contains("!>(!(C4)") && abc.contains("!>)!D4"),
        "fin de soufflet mal placée :\n{abc}"
    );
    let second = scoretk_abc_import::read_abc(&abc).expect("relecture");
    let abc2 = scoretk_abc_export::write_abc(&second).expect("réécriture");
    assert_eq!(abc, abc2, "aller-retour instable :\n{abc}\n---\n{abc2}");
}
