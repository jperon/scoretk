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

//! Engagements du contrat de `write_playback` : MusicXML indépendant du
//! déroulement des reprises, reprises conservées par un aller-retour
//! MusicXML/.mxl, et mesures jamais jouées absentes de la carte des temps.

#![allow(dead_code)]

mod common;

use std::collections::BTreeSet;

#[allow(unused_imports)]
use scoretk::{read, write, Format, MidiOptions, Options, Score};

fn load(name: &str) -> Score {
    let path = common::corpus_v4(&format!("{name}.mscx"));
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("{} : {error}", path.display()));
    read(&bytes, Format::Mscx)
        .unwrap_or_else(|error| panic!("{} : lecture impossible : {error}", path.display()))
}

fn options(expand_repeats: bool) -> Options {
    Options {
        midi: MidiOptions {
            expand_repeats,
            ..Default::default()
        },
        ..Options::default()
    }
}

/// Identifiants `id` des `<measure>` d'un MusicXML stable.
fn measure_ids(xml: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for chunk in xml.split("<measure").skip(1) {
        let Some(start) = chunk.find("id=\"") else {
            continue;
        };
        let rest = &chunk[start + 4..];
        let Some(end) = rest.find('"') else {
            continue;
        };
        ids.insert(rest[..end].to_owned());
    }
    ids
}

#[cfg(feature = "midi-export")]
#[test]
fn musicxml_de_playback_ignore_expand_repeats() {
    for name in ["repeat65", "repeat69"] {
        let score = load(name);
        let expanded = scoretk::write_playback(&score, &options(true))
            .unwrap_or_else(|error| panic!("{name} : playback impossible : {error}"));
        let flat = scoretk::write_playback(&score, &options(false))
            .unwrap_or_else(|error| panic!("{name} : playback impossible : {error}"));
        assert_eq!(
            expanded.musicxml, flat.musicxml,
            "{name} : le MusicXML change avec `expand_repeats`"
        );
        assert_ne!(
            expanded.timemap.measures.len(),
            flat.timemap.measures.len(),
            "{name} : même nombre de mesures jouées avec et sans reprises"
        );
    }
}

#[cfg(all(feature = "midi-export", feature = "musicxml-import"))]
fn assert_repeats_survive(score: &Score, label: &str) {
    let expanded = scoretk::write_playback(score, &options(true))
        .unwrap_or_else(|error| panic!("{label} : playback impossible : {error}"));
    let flat = scoretk::write_playback(score, &options(false))
        .unwrap_or_else(|error| panic!("{label} : playback impossible : {error}"));
    assert_ne!(
        expanded.midi, flat.midi,
        "{label} : le MIDI doit différer entre reprises déroulées et non déroulées"
    );
    assert_ne!(
        expanded.timemap.measures.len(),
        flat.timemap.measures.len(),
        "{label} : le nombre de mesures jouées doit différer"
    );
}

#[cfg(all(feature = "midi-export", feature = "musicxml-import"))]
#[test]
fn reprises_survivent_a_un_aller_retour_musicxml() {
    let score = load("repeat65");
    let musicxml = write(&score, Format::MusicXml, &Options::default())
        .expect("export MusicXML avec reprises");
    let relu = read(&musicxml, Format::MusicXml).expect("relecture MusicXML");
    assert_repeats_survive(&relu, "repeat65 via MusicXML");

    let mxl = write(&score, Format::Mxl, &Options::default()).expect("emballage .mxl");
    let relu = read(&mxl, Format::Mxl).expect("relecture .mxl");
    assert_repeats_survive(&relu, "repeat65 via .mxl");
}

#[cfg(feature = "midi-export")]
#[test]
fn mesure_jamais_jouee_absente_de_la_carte() {
    // `repeat68` ne contient qu'une mesure sous une volta « 2. (skip) » :
    // l'export MusicXML la contient, mais le déroulement des reprises ne la
    // joue jamais.
    let score = load("repeat68");
    let playback = scoretk::write_playback(&score, &options(true)).expect("playback de repeat68");
    let exported = {
        let xml = String::from_utf8(playback.musicxml.clone()).expect("MusicXML UTF-8");
        measure_ids(&xml)
    };
    assert!(
        !exported.is_empty(),
        "repeat68 : aucune mesure dans le MusicXML exporté"
    );
    let played: BTreeSet<String> = playback
        .timemap
        .measures
        .iter()
        .map(|measure| measure.xml_id(0))
        .collect();
    let never_played: Vec<&String> = exported.difference(&played).collect();
    assert!(
        !never_played.is_empty(),
        "repeat68 : toutes les mesures exportées ont été jouées, cas non couvert"
    );
    println!(
        "repeat68 : {} mesures exportées, {} jouées, jamais jouées : {:?}",
        exported.len(),
        played.len(),
        never_played
    );
}
