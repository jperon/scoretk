// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Un nom de voix entre guillemets peut contenir des espaces (ABC 2.1 § 8.2
//! « Text strings »). Le lecteur le tronquait au premier espace, ce qui
//! rendait l'écriture instable (`name="Voix 1"` → `name="Voix"`).

#![cfg(all(feature = "abc-import", feature = "abc-export"))]

#[test]
fn nom_de_voix_avec_espace_stable() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\nV:1 name=\"Voix 1\" clef=treble\nC D E F|\n";
    let first = scoretk_abc_import::read_abc(source).expect("lecture");
    let abc = scoretk_abc_export::write_abc(&first).expect("écriture");
    assert!(abc.contains("name=\"Voix 1\""), "nom tronqué :\n{abc}");
    let second = scoretk_abc_import::read_abc(&abc).expect("relecture");
    let abc2 = scoretk_abc_export::write_abc(&second).expect("réécriture");
    assert_eq!(abc, abc2, "aller-retour instable :\n{abc}\n---\n{abc2}");
}
