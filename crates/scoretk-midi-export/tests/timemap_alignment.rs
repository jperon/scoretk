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

//! Alignement de la carte des temps ([`scoretk_midi_export::Timemap`]) sur le fichier
//! MIDI réellement écrit.
//!
//! Les tests vérifient quatre propriétés :
//!
//! - **invariance octet** : [`scoretk_midi_export::write_midi_with_timemap`] produit
//!   exactement les mêmes octets que [`scoretk_midi_export::write_midi`] ;
//! - **ré-analyse** : chaque `NoteOn` brut du SMF correspond à une
//!   [`NoteSync`] de même tick et de même temps (bijection sur le
//!   multiensemble), et les méta `Tempo` bruts à [`Timemap::tempo`] ;
//! - **monotonie** : mesures et changements de tempo strictement croissants ;
//! - **reprises et pauses** : passages multiples d'une même mesure et saut de
//!   trois secondes à une barre de section.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};
use scoretk_core::model::Score;
use scoretk_musescore_import::{read, MsczContainer};

use scoretk_midi_export::{
    write_midi, write_midi_with, write_midi_with_timemap, write_midi_with_timemap_and, MidiOptions,
    Timemap,
};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .canonicalize()
        .expect("corpus introuvable")
}

fn load(name: &str) -> Score {
    let source = corpus_dir().join(format!("{name}.mscx"));
    let bytes = fs::read(&source)
        .unwrap_or_else(|error| panic!("lecture de {} impossible : {error}", source.display()));
    let container = MsczContainer::from_bytes(&bytes)
        .unwrap_or_else(|error| panic!("{} : conteneur invalide : {error}", source.display()));
    read::read_score(&container)
        .unwrap_or_else(|error| panic!("{} : lecture impossible : {error}", source.display()))
}

/// Fichiers du corpus utilisés pour l'invariance à l'octet.
const BYTE_FILES: &[&str] = &[
    "v3/grace",
    "v3/repeat01",
    "v3/testTuplets8",
    "v4/dynamics",
    "v4/linked-ties",
    "v4/repeat66",
    "v4/repeat68",
    "v4/repeat69",
];

#[test]
fn bytes_are_unchanged() {
    for name in BYTE_FILES {
        let score = load(name);
        let expected = write_midi(&score)
            .unwrap_or_else(|error| panic!("{name} : écriture MIDI impossible : {error}"));
        let (produced, timemap) = write_midi_with_timemap(&score)
            .unwrap_or_else(|error| panic!("{name} : écriture MIDI impossible : {error}"));
        assert_eq!(
            produced, expected,
            "{name} : la carte des temps change les octets du MIDI"
        );
        // Les options explicites passent par le même rendu.
        let (again, same_map) = write_midi_with_timemap_and(&score, MidiOptions::default())
            .unwrap_or_else(|error| panic!("{name} : écriture MIDI impossible : {error}"));
        assert_eq!(again, expected, "{name} : options explicites différentes");
        assert_eq!(same_map, timemap, "{name} : carte non reproductible");
        assert_eq!(
            write_midi_with(&score, MidiOptions::default())
                .unwrap_or_else(|error| panic!("{name} : écriture MIDI impossible : {error}")),
            expected,
            "{name} : `write_midi_with` diverge"
        );
    }
}

/// Attaques brutes d'un SMF : tick et touche.
type RawNotes = Vec<(i64, u8)>;
/// Tempos bruts d'un SMF : tick et microsecondes par noire.
type RawTempos = Vec<(i64, u32)>;

/// Attaques brutes et tempos d'un SMF.
fn parse(bytes: &[u8]) -> (RawNotes, RawTempos) {
    let smf = Smf::parse(bytes).expect("MIDI produit illisible");
    let mut notes = Vec::new();
    let mut tempos = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0i64;
        for event in track {
            tick += i64::from(event.delta.as_int());
            match event.kind {
                TrackEventKind::Midi { message, .. } => match message {
                    MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                        notes.push((tick, key.as_int()));
                    }
                    _ => {}
                },
                TrackEventKind::Meta(MetaMessage::Tempo(value)) => {
                    tempos.push((tick, value.as_int()));
                }
                _ => {}
            }
        }
    }
    notes.sort();
    tempos.sort();
    (notes, tempos)
}

fn note_counts(entries: impl IntoIterator<Item = (i64, i64, u8)>) -> BTreeMap<(i64, i64, u8), i64> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry).or_insert(0) += 1;
    }
    counts
}

fn check_alignment(name: &str, _score: &Score, timemap: &Timemap, bytes: &[u8]) {
    let (raw_notes, raw_tempos) = parse(bytes);

    let expected = note_counts(
        raw_notes
            .iter()
            .map(|(tick, key)| (*tick, timemap.time_us_at(*tick), *key)),
    );
    let produced = note_counts(
        timemap
            .notes
            .iter()
            .map(|note| (note.tick, note.time_us, note.pitch)),
    );
    assert_eq!(
        produced, expected,
        "{name} : les attaques de la carte ne correspondent pas au SMF"
    );

    let tempo: Vec<(i64, u32)> = timemap
        .tempo
        .iter()
        .map(|change| (change.tick, change.micros_per_quarter))
        .collect();
    assert_eq!(
        tempo, raw_tempos,
        "{name} : les tempos de la carte ne correspondent pas au SMF"
    );

    // Réciprocité et temps recalculé : chaque note de la carte doit exister
    // dans le SMF (le multiensemble ci-dessus le garantit) et tomber dans la
    // performance.
    for note in &timemap.notes {
        assert!(
            note.tick >= 0 && note.tick < timemap.ticks,
            "{name} : attaque hors performance : {note:?}"
        );
        assert_eq!(
            note.time_us,
            timemap.time_us_at(note.tick),
            "{name} : temps"
        );
    }
}

#[test]
fn notes_and_tempos_match_smf() {
    for name in ["v3/grace", "v3/testTuplets8", "v4/dynamics", "v4/repeat69"] {
        let score = load(name);
        let (bytes, timemap) = write_midi_with_timemap(&score)
            .unwrap_or_else(|error| panic!("{name} : écriture MIDI impossible : {error}"));
        check_alignment(name, &score, &timemap, &bytes);
    }
}

#[test]
fn measures_and_tempos_are_monotonic() {
    for name in BYTE_FILES {
        let score = load(name);
        let (_bytes, timemap) = write_midi_with_timemap(&score)
            .unwrap_or_else(|error| panic!("{name} : écriture MIDI impossible : {error}"));
        for pair in timemap.measures.windows(2) {
            assert!(
                pair[0].tick < pair[1].tick,
                "{name} : ticks de mesures non croissants : {pair:?}"
            );
            assert!(
                pair[0].time_us < pair[1].time_us,
                "{name} : temps de mesures non croissants : {pair:?}"
            );
            assert!(
                pair[0].end_tick <= pair[1].tick,
                "{name} : mesures qui se chevauchent : {pair:?}"
            );
        }
        for pair in timemap.tempo.windows(2) {
            assert!(
                pair[0].tick < pair[1].tick,
                "{name} : ticks de tempo non croissants : {pair:?}"
            );
            assert!(
                pair[0].time_us <= pair[1].time_us,
                "{name} : temps de tempo non croissants : {pair:?}"
            );
        }
        for note in &timemap.notes {
            assert!(
                timemap.measure_at(note.tick).is_some(),
                "{name} : attaque hors mesure à {}",
                note.tick
            );
        }
    }
}

/// Nombre d'occurrences de chaque mesure, avec les passages associés.
fn occurrences(timemap: &Timemap) -> BTreeMap<usize, Vec<(i64, i32)>> {
    let mut map: BTreeMap<usize, Vec<(i64, i32)>> = BTreeMap::new();
    for measure in &timemap.measures {
        map.entry(measure.measure)
            .or_default()
            .push((measure.tick, measure.pass));
    }
    map
}

#[test]
fn repeats_repeat_measures() {
    // `repeat68` est le cas limite du corpus : sa seule mesure porte une volta
    // « 2. (skip) » et n'est donc jamais jouée ; `repeat69` (D.S. au segno) et
    // `repeat65` (deux voltas et un saut) rejouent réellement des mesures.
    for name in ["v4/repeat68", "v4/repeat69", "v4/repeat65"] {
        let score = load(name);
        let (bytes, timemap) = write_midi_with_timemap(&score)
            .unwrap_or_else(|error| panic!("{name} : écriture MIDI impossible : {error}"));
        check_alignment(name, &score, &timemap, &bytes);

        let map = occurrences(&timemap);
        let repeated: Vec<_> = map
            .iter()
            .filter(|(_, occurrences)| occurrences.len() > 1)
            .collect();
        if name != "v4/repeat68" {
            assert!(
                !repeated.is_empty(),
                "{name} : aucune mesure jouée plusieurs fois"
            );
        }
        let mut distinct_passes = 0usize;
        for (measure, occurrences) in &repeated {
            let ticks: Vec<i64> = occurrences.iter().map(|(tick, _)| *tick).collect();
            assert!(
                ticks.windows(2).all(|pair| pair[0] < pair[1]),
                "{name} : mesure {measure} avec des ticks non croissants : {ticks:?}"
            );
            // Un compteur `times` (repeat65) change de passage ; un saut
            // D.S. rejoue la même mesure dans des segments de même
            // `playback_count`, donc les passages ne sont pas toujours
            // distincts. On exige la distinction quand elle existe dans le
            // modèle.
            let passes: std::collections::BTreeSet<i32> =
                occurrences.iter().map(|(_, pass)| *pass).collect();
            if passes.len() > 1 {
                distinct_passes += 1;
            }
        }
        if name == "v4/repeat65" {
            assert!(
                distinct_passes > 0,
                "{name} : aucune mesure rejouée avec des passages distincts"
            );
        }
        println!(
            "{name} : {} mesures, {} attaques, {} mesures répétées (ex. {:?})",
            timemap.measures.len(),
            timemap.notes.len(),
            repeated.len(),
            repeated
                .first()
                .map(|(measure, occurrences)| (**measure, occurrences.len()))
        );

        let (_flat_bytes, flat) = write_midi_with_timemap_and(
            &score,
            MidiOptions {
                expand_repeats: false,
                ..Default::default()
            },
        )
        .unwrap_or_else(|error| panic!("{name} : écriture MIDI impossible : {error}"));
        assert_eq!(
            flat.measures.len(),
            score.measures.len(),
            "{name} : mesures déroulées malgré `expand_repeats = false`"
        );
        for (measure, occurrences) in occurrences(&flat) {
            assert_eq!(
                occurrences.len(),
                1,
                "{name} : mesure {measure} rejouée sans reprises"
            );
        }
    }
}

#[test]
fn section_pause_inserts_three_seconds() {
    let mut checked = 0;
    for name in ["v4/repeat66", "v3/keysig_78216"] {
        let score = load(name);
        let (bytes, timemap) = write_midi_with_timemap(&score)
            .unwrap_or_else(|error| panic!("{name} : écriture MIDI impossible : {error}"));
        check_alignment(name, &score, &timemap, &bytes);
        if timemap.pauses.is_empty() {
            continue;
        }
        checked += 1;
        let pause = timemap
            .pauses
            .iter()
            .find(|pause| (pause.seconds - 3.0).abs() < 0.01)
            .unwrap_or_else(|| panic!("{name} : aucune pause de 3 s : {:?}", timemap.pauses));
        let elapsed = pause.end_time_us - pause.time_us;
        assert!(
            (elapsed - 3_000_000).abs() < 500,
            "{name} : pause de {elapsed} µs au lieu de 3 s"
        );
        // La mesure suivante commence après la pause, la précédente finit
        // avant : le temps saute bien de la durée de la pause.
        let after = timemap
            .measures
            .iter()
            .find(|measure| measure.tick == pause.end_tick)
            .unwrap_or_else(|| {
                panic!(
                    "{name} : aucune mesure au tick {} (fin de pause {pause:?})",
                    pause.end_tick
                )
            });
        assert!(after.pause_before, "{name} : pause non signalée");
        let before = timemap
            .measures
            .iter()
            .rev()
            .find(|measure| measure.end_tick <= pause.tick)
            .unwrap_or_else(|| panic!("{name} : aucune mesure avant la pause"));
        assert_eq!(
            before.end_time_us, pause.time_us,
            "{name} : la pause ne commence pas à la fin de la mesure"
        );
        assert!(
            after.time_us >= pause.end_time_us,
            "{name} : la pause chevauche la mesure suivante"
        );
        println!(
            "{name} : {} pauses, {} mesures, saut de {} µs à {}",
            timemap.pauses.len(),
            timemap.measures.len(),
            elapsed,
            pause.tick
        );
    }
    assert!(checked > 0, "aucun fichier de pause trouvé dans le corpus");
}

/// Identifiants d'un MusicXML stable : mesures, notes, et `<note>` sans
/// attaque MIDI.
///
/// Les `<note>` sont comptées en multiensemble (une même note de trémolo peut
/// partager son `id`). Les `<note>` sans attaque sont séparées :
/// - les silences, qui n'ont pas de [`NoteSync`] ;
/// - les notes de prolongation (`<tie type="stop">` / `<tied type="stop">`),
///   dont le rendu MIDI ne réattaque pas la note : la chaîne liée n'a qu'une
///   [`NoteSync`], celle de la première note.
fn xml_ids(
    xml: &str,
) -> (
    std::collections::BTreeSet<String>,
    std::collections::BTreeMap<String, i64>,
    std::collections::BTreeMap<String, i64>,
) {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut measures = std::collections::BTreeSet::new();
    let mut notes: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
    let mut sans_attaque: std::collections::BTreeMap<String, i64> =
        std::collections::BTreeMap::new();
    let mut current_note: Option<(String, bool, bool)> = None;
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(event @ (Event::Start(_) | Event::Empty(_))) => {
                let empty = matches!(event, Event::Empty(_));
                let element = match event {
                    Event::Start(element) | Event::Empty(element) => element,
                    _ => unreachable!(),
                };
                let name = element.name();
                let name = name.as_ref();
                if name == "measure" {
                    if let Some(id) = element
                        .attributes()
                        .flatten()
                        .find(|attribute| attribute.key.as_ref() == "id")
                        .map(|attribute| attribute.value.to_string())
                    {
                        measures.insert(id);
                    }
                } else if name == "note" {
                    if let Some(id) = element
                        .attributes()
                        .flatten()
                        .find(|attribute| attribute.key.as_ref() == "id")
                        .map(|attribute| attribute.value.to_string())
                    {
                        if empty {
                            *notes.entry(id).or_insert(0) += 1;
                        } else {
                            current_note = Some((id, false, false));
                        }
                    }
                } else if name == "rest" {
                    if let Some((_, is_rest, _)) = current_note.as_mut() {
                        *is_rest = true;
                    }
                } else if name == "tie" || name == "tied" {
                    if let Some((_, _, tie_stop)) = current_note.as_mut() {
                        let stop = element.attributes().flatten().any(|attribute| {
                            attribute.key.as_ref() == "type" && attribute.value.as_ref() == "stop"
                        });
                        *tie_stop |= stop;
                    }
                }
            }
            Ok(Event::End(element)) => {
                if element.name().as_ref() == "note" {
                    if let Some((id, is_rest, tie_stop)) = current_note.take() {
                        *notes.entry(id.clone()).or_insert(0) += 1;
                        if is_rest || tie_stop {
                            *sans_attaque.entry(id).or_insert(0) += 1;
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("XML invalide : {error}"),
        }
        buffer.clear();
    }
    (measures, notes, sans_attaque)
}

#[test]
fn note_ids_match_stable_musicxml() {
    use std::collections::BTreeMap;

    let mut graces = 0usize;
    // Couverture : ornements (v3/grace), accords verticaux (v3/voices),
    // silences (v4/dynamics), liens (v4/linked-ties) et multi-parties
    // (v4/testLocalTimesig1).
    for name in [
        "v3/grace",
        "v3/voices",
        "v4/linked-ties",
        "v4/dynamics",
        "v4/testLocalTimesig1",
    ] {
        let score = load(name);
        let (_bytes, timemap) = write_midi_with_timemap(&score)
            .unwrap_or_else(|error| panic!("{name} : écriture MIDI impossible : {error}"));
        let xml = scoretk_musicxml_export::export_musicxml_with(
            &score,
            &scoretk_musicxml_export::ExportOptions {
                stable_ids: true,
                ..scoretk_musicxml_export::ExportOptions::default()
            },
        )
        .unwrap_or_else(|error| panic!("{name} : export MusicXML impossible : {error}"));
        let (measures, exported_notes, sans_attaque) = xml_ids(&xml);
        for measure in &timemap.measures {
            let id = measure.xml_id(0);
            assert!(
                measures.contains(&id),
                "{name} : identifiant de mesure absent du MusicXML : {id}"
            );
        }

        // Multiensemble exact des notes rendues, après soustraction explicite
        // des silences (qui n'ont pas de `NoteSync`).
        let produced: BTreeMap<String, i64> =
            timemap.notes.iter().fold(BTreeMap::new(), |mut map, note| {
                if note.grace_index.is_some() {
                    graces += 1;
                }
                *map.entry(note.xml_id()).or_insert(0) += 1;
                map
            });
        let mut expected = exported_notes;
        for (id, count) in sans_attaque {
            if let Some(remaining) = expected.get_mut(&id) {
                *remaining -= count;
                if *remaining == 0 {
                    expected.remove(&id);
                }
            }
        }
        let seulement_carte: Vec<&String> = produced
            .keys()
            .filter(|id| !expected.contains_key(*id))
            .collect();
        let seulement_xml: Vec<&String> = expected
            .keys()
            .filter(|id| !produced.contains_key(*id))
            .collect();
        assert_eq!(
            produced, expected,
            "{name} : les identifiants de notes de la carte ne sont pas exactement \
             les <note> du MusicXML exporté (silences et prolongations \
             déduits) ; carte seule : {seulement_carte:?}, XML seul : {seulement_xml:?}"
        );
    }
    assert!(graces > 0, "aucun ornement dans les fichiers couverts");
}

/// Ticks de tous les évènements d'un SMF, toutes pistes confondues.
fn event_ticks(bytes: &[u8]) -> std::collections::BTreeSet<i64> {
    let smf = Smf::parse(bytes).expect("MIDI produit illisible");
    let mut ticks = std::collections::BTreeSet::new();
    for track in &smf.tracks {
        let mut tick = 0i64;
        for event in track {
            tick += i64::from(event.delta.as_int());
            ticks.insert(tick);
        }
    }
    ticks
}

/// Les notes de la carte sont triées par tick, puis par clé stable.
#[test]
fn notes_are_sorted_by_tick() {
    for name in ["v4/repeat65", "v3/grace"] {
        let score = load(name);
        let (_bytes, timemap) = write_midi_with_timemap(&score)
            .unwrap_or_else(|error| panic!("{name} : écriture MIDI impossible : {error}"));
        for pair in timemap.notes.windows(2) {
            assert!(
                pair[0].tick <= pair[1].tick,
                "{name} : ticks de notes non croissants : {:?} puis {:?}",
                pair[0],
                pair[1]
            );
            if pair[0].tick == pair[1].tick {
                let left = (
                    pair[0].measure,
                    pair[0].staff,
                    pair[0].voice,
                    pair[0].chord,
                    pair[0].note,
                    pair[0].pitch,
                );
                let right = (
                    pair[1].measure,
                    pair[1].staff,
                    pair[1].voice,
                    pair[1].chord,
                    pair[1].note,
                    pair[1].pitch,
                );
                assert!(
                    left <= right,
                    "{name} : ordre instable à tick égal : {left:?} puis {right:?}"
                );
            }
        }
    }
}

/// Chaque début de mesure tombe sur un tick d'évènement réellement présent
/// dans le SMF (ou à zéro, début de performance).
#[test]
fn measure_ticks_land_on_smf_events() {
    for name in ["v4/repeat65", "v4/dynamics"] {
        let score = load(name);
        let (bytes, timemap) = write_midi_with_timemap(&score)
            .unwrap_or_else(|error| panic!("{name} : écriture MIDI impossible : {error}"));
        let ticks = event_ticks(&bytes);
        for measure in &timemap.measures {
            assert!(
                measure.tick == 0 || ticks.contains(&measure.tick),
                "{name} : début de mesure au tick {} absent des évènements SMF",
                measure.tick
            );
        }
    }
}
