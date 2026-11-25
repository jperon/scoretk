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

//! L'identité d'instrument est reconstruite depuis les champs **standard** de
//! l'ABC 2.1 : `V: name=`/`snm=` (§7.1) et `%%MIDI program` (§11.2). Aucune
//! directive `%%scoretk instrument` n'est écrite dans ces airs.

use scoretk_abc_import::read_abc;

fn instrument_id(source: &str) -> String {
    let score = read_abc(source).expect("lecture ABC");
    score.parts[0].instrument.id.clone()
}

#[test]
fn nom_de_voix_et_programme_gm_sans_extension_privee() {
    // Violon : le nom affiché et le programme GM 41 suffisent.
    let violin = "\
X:1
T:Violon
M:4/4
L:1/8
K:C clef=treble
%%MIDI program 41
V:1 name=\"Violin\" snm=\"Vln.\"
C D E F |]
";
    assert_eq!(instrument_id(violin), "strings.violin");

    // Flûte : le nom localisé ne correspond à aucun gabarit, mais le
    // programme GM 74 permet tout de même la reconnaissance.
    let flute = "\
X:1
T:Flute
M:4/4
L:1/8
K:C
%%MIDI program 74
V:1 name=\"Viool\" snm=\"Fl.\"
C D E F |]
";
    assert_eq!(instrument_id(flute), "wind.flutes.flute");
}

#[test]
fn nom_avec_numero_reste_reconnaissable() {
    // `abc2xml.py` écrit volontiers « Piano 1 », « Flute 3 » ; ces noms ne
    // sont pas des gabarits exacts, mais le sous-chaîne « piano » et le
    // programme GM sont des données standard.
    let piano = "\
X:1
T:Piano 1
M:4/4
L:1/8
K:C
%%MIDI program 1
V:1 name=\"Piano 1\" snm=\"Pno. 1\"
C D E F |]
";
    assert_eq!(instrument_id(piano), "keyboard.piano");
}

/// Hors de portée : ABC 2.1 §7.1 (`V: name=`/`snm=`) et §11.2
/// (`%%MIDI program`, numéro GM) ne distinguent pas les identifiants internes
/// de MuseScore partageant un même programme GM. Le catalogue ne contient
/// aucun gabarit dont le nom et le programme reproduisent ces identifiants ;
/// la directive privée reste nécessaire pour eux.
#[test]
fn variantes_de_guitare_restant_hors_de_portee_sans_extension_privee() {
    let steel = "\
X:1
T:Guitar
M:4/4
L:1/8
K:C
%%MIDI program 26
V:1 name=\"Steel Guitar\" snm=\"s.guit.\" clef=treble transpose=-12
C D E F |]
";
    assert_ne!(instrument_id(steel), "pluck.guitar");
    let with_private = steel.replace(
        "C D E F |]",
        "%%scoretk instrument pluck.guitar\nC D E F |]",
    );
    assert_eq!(instrument_id(&with_private), "pluck.guitar");
}

#[test]
fn directive_privee_reste_prioritaire() {
    // Quand elle est présente, la directive privée garde la priorité : elle
    // porte l'identifiant exact des instruments que le GM ne distingue pas.
    let source = "\
X:1
T:Precis
M:4/4
L:1/8
K:C
%%MIDI program 72
V:1 name=\"A Clarinet\" snm=\"Cl.A\"
%%scoretk instrument winds.reed.clarinet.a
C D E F |]
";
    assert_eq!(instrument_id(source), "winds.reed.clarinet.a");
}
