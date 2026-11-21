// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Le texte d'un champ `Q:` ne doit pas être réécrit en annotation `"^…"` :
//! le champ porte déjà le texte, et le faire deux fois rend l'aller-retour
//! instable (le texte est dupliqué à chaque lecture). `xml2abc.py::doDirection`
//! écrit `Q:` depuis `<metronome>` et l'annotation depuis `<words>`, mais le
//! modèle pivot fusionne les deux en un seul `Annotation::Tempo`.

#![cfg(all(feature = "abc-import", feature = "abc-export"))]

#[test]
fn texte_tempo_pas_ecrit_deux_fois() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\nQ:\"Tempo text\" 1/4=120\nC D E F|\n";
    let first = scoretk_abc_import::read_abc(source).expect("lecture");
    let abc = scoretk_abc_export::write_abc(&first).expect("écriture");
    assert_eq!(
        abc.matches("Tempo text").count(),
        1,
        "texte de tempo dupliqué :\n{abc}"
    );
}
