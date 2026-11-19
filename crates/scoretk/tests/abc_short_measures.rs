// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce test est une réécriture Rust originale : aucune ligne de `abc2xml.py`
// n'est copiée. L'air réduit provient du corpus `abcjs-tests` (voir
// `docs/agents/abc.md` § 16.3/§ 16.4).
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

//! Mesures courtes : c'est le modèle qui pilote le `<forward>` de l'export.
//!
//! Quand la mesure du modèle garde sa longueur écrite, `move_to_tick` n'a
//! rien à combler : `abc2xml.py` non plus n'écrit aucun silence de remplissage
//! pour une mesure courte (grammaire `measure`, lignes 428-436).

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

use scoretk::{read, write, Format, Options};

/// X:5 de `abcjs-tests/extracted-abc/tablature.test__bracketPlacement.abc`.
const TABLATURE_X5: &str = "\
X:5
L:1/8
%%staves {RH LH}
V: RH clef=treble
V: LH clef=bass
K:C
[V: RH]
|: c
[V: LH]
|: G,
";

/// Air témoin, sans mesure courte : la sortie doit rester identique.
const AIR_PLEIN: &str = "\
X:1
M:4/4
L:1/8
K:C
C D E F G A B c | C D E F G A B c |
";

fn options() -> Options {
    Options {
        encoding_date: "2026-11-17".to_owned(),
        ..Options::default()
    }
}

fn musicxml(abc: &str) -> String {
    let score = read(abc.as_bytes(), Format::Abc).expect("lecture ABC");
    let bytes = write(&score, Format::MusicXml, &options()).expect("écriture MusicXML");
    String::from_utf8(bytes).expect("UTF-8")
}

#[test]
fn mesure_courte_sans_forward_de_remplissage() {
    let xml = musicxml(TABLATURE_X5);
    assert_eq!(xml.matches("<measure ").count(), 2, "deux mesures écrites");
    assert_eq!(
        xml.matches("<note>").count() + xml.matches("<note ").count(),
        2
    );
    // abc2xml.py n'écrit aucun silence : le <forward> de l'ancien lecteur
    // (7 croches pour atteindre 4/4) décalait tous les attacks suivants.
    assert!(
        !xml.contains("<forward"),
        "aucun <forward> de remplissage attendu, got:\n{xml}"
    );
    assert!(xml.contains("<step>C</step>"), "{xml}");
    assert!(xml.contains("<step>G</step>"), "{xml}");
    // Deux notes d'un croche, chacune à une division MusicXML.
    let durations: Vec<_> = xml
        .match_indices("<duration>")
        .map(|(index, _)| {
            let rest = &xml[index + "<duration>".len()..];
            rest[..rest.find('<').expect("fin de duration")].to_owned()
        })
        .collect();
    assert_eq!(durations, vec!["1", "1"], "deux croches, divisions=2");
}

#[test]
fn air_plein_inchange() {
    let xml = musicxml(AIR_PLEIN);
    assert_eq!(xml.matches("<measure ").count(), 2);
    assert_eq!(
        xml.matches("<note>").count() + xml.matches("<note ").count(),
        16
    );
    assert!(
        !xml.contains("<forward"),
        "une mesure pleine ne doit pas produire de <forward>:\n{xml}"
    );
}
