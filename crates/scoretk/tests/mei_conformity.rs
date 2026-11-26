// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : comparaison normalisée des exports MEI de scoretk
// aux références produites par MuseScore 4.7.4.
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

//! MEI export conformity against MuseScore 4.7.4 references.
//!
//! A raw XML diff is meaningless here: MuseScore generates hash-based
//! `xml:id`s, embeds the current time in `@isodate` and writes layout details.
//! Like the MusicXML `core_norm` comparison, this test normalises volatile
//! attributes and keeps only the musical content: element name plus a stable
//! set of attributes (`@dur`, `@dots`, `@pname`, `@oct`, `@accid`,
//! `@accid.ges`, `@num`, `@numbase`, clef/meter/key attributes, barlines).
//! Both files are then compared as multisets of normalised events.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use scoretk::{read, write, Format, Options};
use scoretk_xml_common::Element;

/// Attributes that carry musical information in `mei-basic`.
const KEEP: [&str; 16] = [
    "n",
    "dur",
    "dots",
    "pname",
    "oct",
    "accid",
    "accid.ges",
    "num",
    "numbase",
    "shape",
    "line",
    "meter.count",
    "meter.unit",
    "meter.sym",
    "keysig",
    "right",
];

fn corpus_scores() -> Vec<(String, PathBuf)> {
    let mut scores = Vec::new();
    for version in ["v3", "v4"] {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("corpus")
            .join(version);
        let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
            .expect("corpus directory")
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "mscx")
            })
            .collect();
        paths.sort();
        for path in paths {
            let name = path
                .file_stem()
                .expect("score file name")
                .to_string_lossy()
                .into_owned();
            scores.push((format!("{version}/{name}"), path));
        }
    }
    scores
}

/// Multiset of `(element, relevant attributes)` pairs, in document order.
fn events(root: &Element) -> Vec<String> {
    let keep = |element: &Element| {
        // MuseScore writes accidental attributes on a child `<accid>`;
        // scoretk writes them on the note. Fold both forms together.
        let mut attributes = element
            .attributes
            .iter()
            .chain(
                element
                    .children
                    .iter()
                    .filter(|child| child.name == "accid")
                    .flat_map(|child| child.attributes.iter()),
            )
            .filter(|(name, _)| KEEP.contains(&name.as_str()) || name == "left")
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>();
        attributes.sort();
        format!("{}{{{}}}", element.name, attributes.join(","))
    };
    let mut result = Vec::new();
    fn walk(element: &Element, keep: &impl Fn(&Element) -> String, result: &mut Vec<String>) {
        if matches!(
            element.name.as_str(),
            "measure"
                | "staffDef"
                | "layer"
                | "note"
                | "chord"
                | "rest"
                | "mRest"
                | "mRpt"
                | "tuplet"
                | "clef"
                | "space"
                | "tempo"
                | "dynam"
                | "dir"
                | "hairpin"
                | "slur"
                | "reh"
        ) {
            result.push(keep(element));
        }
        for child in &element.children {
            walk(child, keep, result);
        }
    }
    walk(root, &keep, &mut result);
    result
}

fn counts(events: &[String]) -> BTreeMap<String, usize> {
    let mut result = BTreeMap::new();
    for event in events {
        *result.entry(event.clone()).or_insert(0) += 1;
    }
    result
}

#[test]
fn mei_export_matches_musescore_474_references() {
    let mut total = 0;
    let mut matched = 0;
    let mut worst = Vec::new();
    for (name, path) in corpus_scores() {
        let reference_path = path.with_extension("ref.mei");
        let reference =
            scoretk_xml_common::parse(&fs::read(&reference_path).expect("MEI reference"))
                .expect("parse MEI reference");
        let score = read(&fs::read(&path).expect("mscx"), Format::Mscx).expect("MuseScore import");
        let exported = write(&score, Format::Mei, &Options::default()).expect("MEI export");
        let candidate = scoretk_xml_common::parse(&exported).expect("parse MEI export");
        let expected = counts(&events(&reference));
        let actual = counts(&events(&candidate));
        total += expected.values().sum::<usize>();
        let file_matched = expected
            .iter()
            .map(|(event, count)| {
                let found = actual.get(event).copied().unwrap_or(0).min(*count);
                matched += found;
                found
            })
            .sum::<usize>();
        let file_total = expected.values().sum::<usize>();
        worst.push(format!(
            "{}: {}/{} (miss {})",
            name,
            file_matched,
            file_total,
            file_total - file_matched
        ));
    }
    let percent = matched as f64 * 100.0 / total as f64;
    println!("\nMEI export conformity (normalised events): {matched}/{total} = {percent:.2}%");
    worst.sort_by(|a, b| {
        let miss = |s: &str| {
            s.rsplit("miss ")
                .next()
                .and_then(|v| v.trim_end_matches(')').parse::<usize>().ok())
                .unwrap_or(0)
        };
        miss(b).cmp(&miss(a))
    });
    println!(
        "Largest gaps: {}",
        worst
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    );
    assert!(total > 1000, "the corpus must provide enough MEI events");
    assert!(
        percent >= 90.0,
        "MEI export conformity {percent:.2}% is below 90%"
    );
}

/// `@tstamp` must be read with the same denominator as the one the writer
/// used: the reduced numerator of the measure's time signature
/// (`Measure::time_sig_numerator`, `4/4` → `1`). Without it a hairpin written
/// at `tstamp="2.000"` in a 4/4 measure lands at 1/4 instead of 1/1.
#[test]
fn tstamp_uses_the_reduced_numerator_in_both_directions() {
    let mei = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1">
      <music><body><mdiv><score>
        <scoreDef><staffGrp><staffDef n="1" lines="5" meter.count="4" meter.unit="4"/></staffGrp></scoreDef>
        <section><measure n="1">
          <staff n="1"><layer n="1"><note dur="4" pname="c" oct="4"/></layer></staff>
          <hairpin staff="1" tstamp="2.000" tstamp2="3.000" form="cres"/>
        </measure></section>
      </score></mdiv></body></music></mei>"#;
    let score = read(mei, Format::Mei).expect("read MEI");
    assert_eq!(score.measures[0].time_sig_numerator, 1);
    let hairpin = score
        .spanners
        .iter()
        .find(|spanner| matches!(spanner.kind, scoretk::model::SpannerType::Hairpin(_)))
        .expect("hairpin");
    assert_eq!(hairpin.start.tick, scoretk::model::Fraction::new(1, 1));
    assert_eq!(hairpin.end.tick, scoretk::model::Fraction::new(2, 1));
}

/// `multiRest/@n` et `mRpt/@n` (MEI 5.1 `att.numbered`) portent le nombre de
/// mesures couvertes et le nombre de reprises, sans recourir à un `@n` de
/// mesure truqué ni à une annotation privée.
#[test]
fn mei_numbered_rest_carriers_are_read() {
    let mei = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1">
      <music><body><mdiv><score>
        <scoreDef><staffGrp><staffDef n="1" lines="5"/></staffGrp></scoreDef>
        <section>
          <measure n="1"><staff n="1"><layer n="1"><multiRest n="3"/></layer></staff></measure>
          <measure n="2"><staff n="1"><layer n="1"><mRpt n="2"/></layer></staff></measure>
        </section>
      </score></mdiv></body></music></mei>"#;
    let score = read(mei, Format::Mei).expect("read MEI");
    assert_eq!(score.measures[0].multi_measure_rest, Some(3));
    let repeat = score.measures[1].measure_repeat.expect("measure repeat");
    assert_eq!(repeat.count, 2);
}

/// A `<staffDef>` clef and a layer `<clef>` at the same tick are two distinct
/// facts in MuseScore's model; `mei_clef` places no restriction on either.
#[test]
fn two_clefs_at_one_tick_are_both_read() {
    let mei = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1">
      <music><body><mdiv><score>
        <scoreDef><staffGrp><staffDef n="1" lines="5">
          <clef shape="G" line="2"/>
        </staffDef></staffGrp></scoreDef>
        <section><measure n="1">
          <staff n="1"><layer n="1">
            <clef shape="F" line="4" tstamp="1.000"/>
            <note dur="4" pname="c" oct="4"/>
          </layer></staff>
        </measure></section>
      </score></mdiv></body></music></mei>"#;
    let score = read(mei, Format::Mei).expect("read MEI");
    let clefs: Vec<_> = score.measures[0]
        .segments
        .iter()
        .filter_map(
            |segment| match segment.element(scoretk::model::Track::from_raw(0)) {
                Some(scoretk::model::SegmentItem::Clef(clef)) => Some(clef.concert),
                _ => None,
            },
        )
        .collect();
    assert_eq!(clefs.len(), 2, "both clefs must survive the import");
}

/// The additive text carriers (`<rend @n>` and `<dir type="mscore-tbox-*">`)
/// write the pivot `RichText` verbatim: a trailing space or newline is part
/// of the model and must survive the round trip. A 4.7.4 `<rend>` without
/// `@n` keeps the importer's trimming.
#[test]
fn additive_text_carriers_keep_trailing_whitespace() {
    let mei = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1">
      <music><body><mdiv><score>
        <scoreDef><pgHead func="first">
          <rend type="title" n="1">Sonata &quot;Moonlight&quot; </rend>
          <rend type="subtitle">Subtitle</rend>
        </pgHead><staffGrp><staffDef n="1" lines="5"/></staffGrp></scoreDef>
        <section><measure n="1">
          <staff n="1"><layer n="1"><note dur="4" pname="c" oct="4"/></layer></staff>
          <dir type="mscore-tbox-0">volta
</dir>
        </measure></section>
      </score></mdiv></body></music></mei>"#;
    let score = read(mei, Format::Mei).expect("read MEI");
    assert!(score
        .engraving
        .credits
        .iter()
        .any(|credit| credit.text.plain() == "Subtitle"));
    assert!(score
        .engraving
        .credits
        .iter()
        .any(|credit| credit.text.plain() == "Sonata \"Moonlight\" "));
    assert_eq!(score.engraving.tbox_texts[0].text.plain(), "volta\n");
}
