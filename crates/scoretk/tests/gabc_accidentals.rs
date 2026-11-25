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

//! GABC `x`/`y`/`#` → modèle → MusicXML : `<alter>` **et** `<accidental>`.
//!
//! XSD 4.0 distingue `<pitch>/<alter>` (hauteur sonore) de `<accidental>`
//! (signe réellement noté). Verovio ne dessine le bémol que si ce dernier est
//! présent. Les altérations GABC sont des glyphes non chantés
//! (`Gabc.tex:512-540`) : le signe visible est reporté sur la première note
//! chantée suivante de la même lettre, les notes suivantes sous la portée
//! gardant `<alter>` sans signe.

use scoretk::{read, write, write_playback, Format, Options};
use scoretk_xml_common::Element;

#[derive(Debug, PartialEq, Eq)]
struct XmlNote {
    step: Option<String>,
    alter: Option<i64>,
    accidental: Option<String>,
    parentheses: bool,
}

fn collect(element: &Element, notes: &mut Vec<XmlNote>) {
    if element.name == "note" {
        let pitch = element.child("pitch");
        let accidental = element.child("accidental");
        notes.push(XmlNote {
            step: pitch
                .and_then(|pitch| pitch.child_text("step"))
                .map(str::to_owned),
            alter: pitch.and_then(|pitch| pitch.child_int("alter")),
            accidental: accidental.map(|sign| sign.text_trimmed().to_owned()),
            parentheses: accidental.and_then(|sign| sign.attribute("parentheses")) == Some("yes"),
        });
    }
    for child in &element.children {
        collect(child, notes);
    }
}

fn xml_notes(xml: &str) -> Vec<XmlNote> {
    let root = scoretk_xml_common::parse(xml.as_bytes()).expect("XML valide");
    let mut notes = Vec::new();
    collect(&root, &mut notes);
    notes
}

fn export(source: &str, modern: bool) -> String {
    let score = read(source.as_bytes(), Format::Gabc).expect("import GABC");
    let options = Options {
        gabc_modern_clef: modern,
        ..Options::default()
    };
    String::from_utf8(write(&score, Format::MusicXml, &options).expect("export MusicXML"))
        .expect("UTF-8")
}

/// `(c3) lé(gxg)i(f)` : le glyphe `gx` précède la note chantée `g`, qui doit
/// porter bémol visible et `<alter>-1</alter>` ; `i` et `f` restent naturels.
#[test]
fn standalone_flat_glyph_draws_flat_on_the_following_note() {
    const SOURCE: &str = "name:t;\n%%\n(c3) lé(gxg)i(f) (::)\n";
    let score = read(SOURCE.as_bytes(), Format::Gabc).expect("import GABC");
    assert_eq!(score.notes.len(), 2, "a GABC alteration is not a note");
    use scoretk::model::{Accidental, AccidentalType};
    assert_eq!(
        score.notes[0].accidental,
        Some(Accidental::new(AccidentalType::Flat))
    );
    assert_eq!(score.notes[1].accidental, None);

    for modern in [false, true] {
        let xml = export(SOURCE, modern);
        let notes = xml_notes(&xml);
        // `i(f)` est la syllabe `i` sur un `f`, pas une note `i`.
        assert_eq!(notes.len(), 2, "{xml}");
        assert_eq!(notes[0].alter, Some(-1), "{xml}");
        assert_eq!(notes[0].accidental.as_deref(), Some("flat"), "{xml}");
        assert_eq!(notes[1].alter, None);
        assert_eq!(notes[1].accidental, None, "{xml}");
    }
}

/// Les trois repros remontés par Cantus, en `convert` ordinaire (clé d'ut)
/// comme avec `gabcModernClef` (clé de sol) : un seul bémol dessiné par cas.
#[test]
fn cantus_repros_draw_flat_with_and_without_the_modern_clef() {
    const REPROS: [&str; 3] = [
        "name:t;\n%%\n(c3) lé(gxg)i(f) (::)\n",
        "name:t;\n%%\n(c4) lé(ixi)i(h) (::)\n",
        "name:t;\n%%\n(c3) lé(gx)son(g)h(g) (::)\n",
    ];
    for source in REPROS {
        for modern in [false, true] {
            let xml = export(source, modern);
            let notes = xml_notes(&xml);
            let flats: Vec<_> = notes
                .iter()
                .filter(|note| note.accidental.as_deref() == Some("flat"))
                .collect();
            assert_eq!(flats.len(), 1, "one notated flat per repro: {xml}");
            assert_eq!(flats[0].alter, Some(-1), "{xml}");
            // Toutes les hauteurs bémolisées portent `<alter>`, mais une seule
            // note porte le signe : les suivantes sont sous la même portée.
            let altered: Vec<_> = notes.iter().filter(|note| note.alter == Some(-1)).collect();
            assert!(!altered.is_empty(), "{xml}");
            assert_eq!(altered[0].accidental.as_deref(), Some("flat"));
            for note in altered.iter().skip(1) {
                assert_eq!(note.accidental, None, "no reminder inside the scope: {xml}");
            }
        }
    }
}

/// `y`/`Y` est un bécarre visible qui annule la portée du bémol ; la
/// deuxième note encore sous le bémol n'a pas de signe affiché.
#[test]
fn natural_glyph_draws_natural_and_cancels_the_flat() {
    const SOURCE: &str = "name:t;\n%%\n(c4) a(gxgggYgg) (::)\n";
    let xml = export(SOURCE, false);
    let notes = xml_notes(&xml);
    assert_eq!(
        notes
            .iter()
            .map(|note| (note.alter, note.accidental.as_deref()))
            .collect::<Vec<_>>(),
        [
            (Some(-1), Some("flat")),
            (Some(-1), None),
            (None, Some("natural")),
            (None, None),
        ],
        "{xml}"
    );
}

/// `#` est un dièse visible ; la forme parenthesée `#?` garde
/// `parentheses="yes"` en MusicXML (`Gabc.tex:482,486`).
#[test]
fn sharp_and_parenthesized_forms_draw_their_signs() {
    for (code, sign, parentheses) in [("gg#g", "sharp", false), ("ggx?g", "flat", true)] {
        let source = format!("name:t;\n%%\n(c4) a({code}) (::)\n");
        let xml = export(&source, false);
        let notes = xml_notes(&xml);
        assert_eq!(notes.len(), 2, "{xml}");
        assert_eq!(notes[1].accidental.as_deref(), Some(sign), "{xml}");
        assert_eq!(notes[0].accidental, None, "{xml}");
        assert_eq!(notes[1].parentheses, parentheses, "{xml}");
    }
    let xml = export("name:t;\n%%\n(c4) a(gg##g) (::)\n", false);
    assert!(xml.contains("<accidental>sharp</accidental>"), "{xml}");
}

/// Portée : fin de mot ou barre de séparation (`docs/agents/gabc.md` §7).
/// Le signe n'est répété que si le glyphe l'est ; le naturel qui suit une
/// barre n'a ni `<alter>` ni `<accidental>` hérités.
#[test]
fn scope_ends_at_word_and_bar_without_inherited_signs() {
    const SOURCE: &str = "name:t;\n%%\n(c3) lé(gx)son(g)h(g) (,) k(g)\n";
    let xml = export(SOURCE, false);
    let notes = xml_notes(&xml);
    assert_eq!(
        notes
            .iter()
            .map(|note| (note.alter, note.accidental.as_deref()))
            .collect::<Vec<_>>(),
        [(Some(-1), Some("flat")), (Some(-1), None), (None, None)],
        "{xml}"
    );

    // Un glyphe suivi d'un espace ne porte pas au-delà du mot.
    let xml = export("name:t;\n%%\n(c3) A(gx) B(g) (::)\n", false);
    let notes = xml_notes(&xml);
    assert_eq!(notes.len(), 1, "{xml}");
    assert!(notes.iter().all(|note| note.accidental.is_none()), "{xml}");
    assert!(notes.iter().all(|note| note.alter.is_none()), "{xml}");
}

/// `convertWithTimeline` exporte le même MusicXML identifié que `convert` :
/// le chemin `write_playback` doit donc écrire le signe visible aussi.
#[test]
fn timeline_export_draws_the_flat_too() {
    const SOURCE: &str = "name:t;\n%%\n(c3) lé(gxg)i(f) (::)\n";
    let score = read(SOURCE.as_bytes(), Format::Gabc).expect("import GABC");
    for modern in [false, true] {
        let options = Options {
            gabc_modern_clef: modern,
            ..Options::default()
        };
        let playback = write_playback(&score, &options).expect("playback");
        let xml = String::from_utf8(playback.musicxml).expect("UTF-8");
        let notes = xml_notes(&xml);
        assert_eq!(notes[0].accidental.as_deref(), Some("flat"), "{xml}");
        assert_eq!(notes[0].alter, Some(-1), "{xml}");
        assert_eq!(playback.timemap.notes.len(), score.notes.len());
    }
}

/// Ajouter le signe affiché ne doit rien changer à l'aller-retour L6 :
/// le MusicXML relu reconstruit le même inventaire et le glyphe reste `gixi`.
#[test]
fn gabc_musicxml_gabc_round_trip_is_unchanged() {
    const SOURCE: &str = "name:t;\n%%\n(c4) A(gixi)ve(h_g.) (::)\n";
    let score = read(SOURCE.as_bytes(), Format::Gabc).expect("import GABC");
    let xml = export(SOURCE, false);
    assert!(xml.contains("<accidental>flat</accidental>"), "{xml}");
    let from_xml = read(xml.as_bytes(), Format::MusicXml).expect("import MusicXML");
    let gabc = String::from_utf8(
        write(&from_xml, Format::Gabc, &Options::default()).expect("export GABC"),
    )
    .expect("UTF-8 GABC");
    assert!(gabc.contains("gixi"), "{gabc}");
    let again = read(gabc.as_bytes(), Format::Gabc).expect("second import");
    let reference = scoretk::model::inventory(&score);
    let candidate = scoretk::model::inventory(&again);
    let coverage = scoretk::model::compare(&reference, &candidate, scoretk::model::Layers::ALL);
    assert_eq!(
        (
            coverage.total.reference,
            coverage.total.matched,
            coverage.total.extra
        ),
        (reference.items.len(), reference.items.len(), 0),
        "the structured inventory changed"
    );
}
