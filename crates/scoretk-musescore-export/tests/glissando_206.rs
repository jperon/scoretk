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

//! Glissando 2.x lu puis réécrit comme spanner 4.x.

use scoretk_core::model::{Score, SpannerType};
use scoretk_musescore_import::MsczContainer;

fn partition(avec_glissando: bool) -> Score {
    let debut = if avec_glissando {
        r#"<Glissando id="1"><text>gliss.</text><subtype>1</subtype><diagonal>1</diagonal></Glissando>"#
    } else {
        ""
    };
    let fin = if avec_glissando {
        r#"<endSpanner id="1"/>"#
    } else {
        ""
    };
    let xml = format!(
        r#"<museScore version="2.06"><Score>
<Division>480</Division>
<Part><Staff id="1"/><trackName>Flute</trackName>
  <Instrument id="wind.flutes.flute"><longName>Flute</longName>
    <trackName>Flute</trackName><instrumentId>wind.flutes.flute</instrumentId>
    <Channel><program value="73"/></Channel></Instrument></Part>
<Staff id="1">
  <Measure>
    <Chord><durationType>whole</durationType>
      <Note><pitch>60</pitch><tpc>14</tpc>{debut}</Note></Chord>
    </Measure>
  <Measure>
    {fin}
    <Chord><durationType>whole</durationType>
      <Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
    </Measure>
  </Staff>
</Score></museScore>"#
    );
    let container = MsczContainer::from_bytes(xml.as_bytes()).expect("conteneur 2.x");
    scoretk_musescore_import::read::read_score(&container).expect("lecture 2.x")
}

fn glissandi(score: &Score) -> usize {
    score
        .spanners
        .iter()
        .filter(|spanner| matches!(spanner.kind, SpannerType::Glissando))
        .count()
}

/// Le lecteur 2.x du C++ reconnaît `<Glissando id>` refermé par
/// `<endSpanner id>` (`read206.cpp:2871-2892`) ; le portage 2.x ne le lisait
/// pas, donc le glissando disparaissait du `.mscx` écrit et MuseScore ne
/// jouait pas ses notes intermédiaires (mscz-8).
#[test]
fn glissando_2x_ecrit_comme_spanner() {
    let score = partition(true);
    assert_eq!(
        glissandi(&score),
        1,
        "le lecteur 2.x doit lire le glissando"
    );
    let ecrit = scoretk_musescore_export::write_mscx(&score).expect("écriture");
    assert!(
        ecrit.contains("<Spanner type=\"Glissando\">"),
        "le glissando doit être réécrit comme spanner 4.x"
    );
    let relu = {
        let container = MsczContainer::from_bytes(ecrit.as_bytes()).expect("conteneur 4.x");
        scoretk_musescore_import::read::read_score(&container).expect("relecture 4.x")
    };
    assert_eq!(glissandi(&relu), 1);
    assert_eq!(relu.measures.len(), score.measures.len());
    assert_eq!(relu.notes.len(), score.notes.len());

    // Cas non concerné : sans glissando, aucun spanner n'est inventé.
    let nu = partition(false);
    assert_eq!(glissandi(&nu), 0);
    let ecrit_nu = scoretk_musescore_export::write_mscx(&nu).expect("écriture");
    assert!(!ecrit_nu.contains("<Spanner type=\"Glissando\">"));
}
