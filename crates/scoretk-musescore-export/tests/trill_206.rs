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

//! Trille 2.x lu puis réécrit comme spanner 4.x.

use scoretk_core::model::{Score, SpannerType};
use scoretk_musescore_import::MsczContainer;

fn partition(avec_trille: bool) -> Score {
    let debut = if avec_trille {
        r#"<Trill id="1"><subtype>trill</subtype><lineWidth>0.15</lineWidth></Trill>"#
    } else {
        ""
    };
    let fin = if avec_trille {
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
    {debut}
    <voice><Chord><durationType>whole</durationType>
      <Note><pitch>60</pitch><tpc>14</tpc></Note></Chord></voice>
    </Measure>
  <Measure>
    {fin}
    <voice><Chord><durationType>whole</durationType>
      <Note><pitch>62</pitch><tpc>16</tpc></Note></Chord></voice>
    </Measure>
  </Staff>
</Score></museScore>"#
    );
    let container = MsczContainer::from_bytes(xml.as_bytes()).expect("conteneur 2.x");
    scoretk_musescore_import::read::read_score(&container).expect("lecture 2.x")
}

fn trilles(score: &Score) -> usize {
    score
        .spanners
        .iter()
        .filter(|spanner| matches!(spanner.kind, SpannerType::Trill))
        .count()
}

/// `Read206::readTrill206` (`read206.cpp:2362-2376`) lit le `<Trill id>`
/// 2.x ; `readMeasure206` (`:2871-2892`) l'apparie à son `<endSpanner id>`.
/// `TWrite::write(const Trill*)` (`write/twrite.cpp:3490-3494`) réécrit un
/// `<Spanner type="Trill">`. Sans lecture, MuseScore perd la note trillée à
/// la réouverture (11 trilles sur mscz-33).
#[test]
fn trille_2x_ecrit_comme_spanner() {
    let score = partition(true);
    assert_eq!(trilles(&score), 1, "le lecteur 2.x doit lire le trille");
    let ecrit = scoretk_musescore_export::write_mscx(&score).expect("écriture");
    assert!(
        ecrit.contains("<Spanner type=\"Trill\">"),
        "le trille doit être réécrit comme spanner 4.x"
    );
    let relu = {
        let container = MsczContainer::from_bytes(ecrit.as_bytes()).expect("conteneur 4.x");
        scoretk_musescore_import::read::read_score(&container).expect("relecture 4.x")
    };
    assert_eq!(trilles(&relu), 1);
    assert_eq!(relu.measures.len(), score.measures.len());
    assert_eq!(relu.notes.len(), score.notes.len());

    // Cas non concerné : sans trille, aucun spanner n'est inventé.
    let nu = partition(false);
    assert_eq!(trilles(&nu), 0);
    let ecrit_nu = scoretk_musescore_export::write_mscx(&nu).expect("écriture");
    assert!(!ecrit_nu.contains("<Spanner type=\"Trill\">"));
}
