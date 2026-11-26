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

//! Raw GregoBase body (no header, no `%%`) through the facade, to MusicXML.

use scoretk::{read, write, Format, Options};

const BODY: &str = "(c4) Ky(f)ri(g)e(h) e(g)lé(f)i(e)son.(f) (::)\n";

#[test]
fn a_body_only_gabc_is_detected_and_converted_to_musicxml() {
    assert_eq!(Format::detect(BODY.as_bytes()), Some(Format::Gabc));
    let score = read(BODY.as_bytes(), Format::Gabc).expect("body-only GABC import");
    assert_eq!(score.notes.len(), 7);

    let xml = String::from_utf8(
        write(&score, Format::MusicXml, &Options::default()).expect("MusicXML export"),
    )
    .expect("UTF-8 MusicXML");
    assert!(xml.contains("<score-partwise"), "{xml}");
    assert_eq!(xml.matches("<pitch>").count(), 7, "{xml}");
    assert!(xml.contains("<senza-misura"), "{xml}");
    assert!(xml.contains("<bar-style>light-heavy</bar-style>"), "{xml}");
    assert!(xml.contains("<lyric"), "{xml}");

    // A raw body has no header to restore: the writer must not invent
    // `name: ;` or a `%%` marker (`docs/agents/gabc.md` §15, §17). Other
    // whitespace is still normalised by the structured writer (§8).
    let written =
        String::from_utf8(write(&score, Format::Gabc, &Options::default()).expect("GABC export"))
            .expect("UTF-8 GABC");
    assert!(!written.contains("%%"), "{written}");
    assert!(!written.contains("name:"), "{written}");
    assert!(written.starts_with("(c4)"), "{written}");
    assert_eq!(
        read(written.as_bytes(), Format::Gabc)
            .expect("second import")
            .notes
            .len(),
        7
    );
}

#[test]
fn a_headerless_gregobase_group_keeps_its_alternating_cuts() {
    // GregoBase 18205 (private SQL dump, no header): with one inferred line
    // every second segment is square notation and must be sung, while the
    // other seven are verbatim manuscript snippets. The cut-back between
    // them is structural and the writer restores it (`docs/agents/gabc.md` §17).
    const BODY: &str =
        "(c4) nos,(hg|cl|h|st|jj|ds-hi|jvHG|ci|hh|ds-|/giHG|pesu2|hf/fe|cl!pi) (:)\n";
    let score = read(BODY.as_bytes(), Format::Gabc).expect("NABC body import");
    assert_eq!(score.notes.len(), 18);
    let written =
        String::from_utf8(write(&score, Format::Gabc, &Options::default()).expect("GABC export"))
            .expect("UTF-8 GABC");
    assert!(!written.contains("nabc-lines:"), "{written}");
    assert!(!written.contains("%%"), "{written}");
    // Every cut-back is restored: the seven snippets stay at their original
    // places. The slash starting the `/giHG` square segment is a pre-existing
    // glyph normalisation — the reader attaches it to the preceding `h` note
    // whatever the cut — not a lost separator.
    assert_eq!(
        written,
        "(c4)\nnos,(hg|cl|h|st|jj|ds-hi|jvHG|ci|hh/|ds-|giHG|pesu2|hf/fe|cl!pi) (:)"
    );
    let again = read(written.as_bytes(), Format::Gabc).expect("second import");
    assert_eq!(again.notes.len(), 18);
    let Some(scoretk::model::Extension::Gregorian(before)) = &score.extensions.format else {
        panic!("first L6 payload");
    };
    let Some(scoretk::model::Extension::Gregorian(after)) = &again.extensions.format else {
        panic!("second L6 payload");
    };
    assert_eq!(
        before
            .syllables
            .iter()
            .map(|syllable| syllable.nabc.clone())
            .collect::<Vec<_>>(),
        after
            .syllables
            .iter()
            .map(|syllable| syllable.nabc.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_headerless_body_with_seven_manuscript_voices_round_trips() {
    const RAW: &str = "(f3) (ii//|bv-|gh!ivHG//|vi-hhppu2su1sut1|fhg/|to|eef.|pt) (,)\n";
    let score = read(RAW.as_bytes(), Format::Gabc).expect("NABC body import");
    let written =
        String::from_utf8(write(&score, Format::Gabc, &Options::default()).expect("GABC export"))
            .expect("UTF-8 GABC");
    assert!(!written.contains("%%"), "{written}");
    assert!(!written.contains("name:"), "{written}");
    assert_eq!(
        written,
        "(f3)\n(ii//|bv-|gh!ivHG//|vi-hhppu2su1sut1|fhg/|to|eef.|pt) (,)"
    );
    let again = read(written.as_bytes(), Format::Gabc).expect("second import");
    assert_eq!(again.notes.len(), score.notes.len());
    let Some(scoretk::model::Extension::Gregorian(before)) = &score.extensions.format else {
        panic!("first L6 payload");
    };
    let Some(scoretk::model::Extension::Gregorian(after)) = &again.extensions.format else {
        panic!("second L6 payload");
    };
    assert_eq!(
        before
            .syllables
            .iter()
            .map(|syllable| syllable.nabc.clone())
            .collect::<Vec<_>>(),
        after
            .syllables
            .iter()
            .map(|syllable| syllable.nabc.clone())
            .collect::<Vec<_>>()
    );
}
