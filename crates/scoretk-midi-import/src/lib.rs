// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de l'import MIDI de MuseScore Studio
// (`importexport/midi/internal/midiimport/`, principalement `importmidi.cpp`),
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026.
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

//! Lecture MIDI : d'un Standard MIDI File au modèle de `scoretk_core`.
//!
//! Portage de `convertMidi` (`importmidi.cpp`) et de ses modules : pistes,
//! accords, quantification, mesure, n-olets, voix, armures, clefs, tempo,
//! séparation des mains et paroles. Le découpage suit le C++ :
//!
//! | Module | C++ amont |
//! | --- | --- |
//! | [`chord`] | `importmidi_chord.cpp` |
//! | [`clef`] | `importmidi_clef.cpp` |
//! | [`drum`] | `importmidi_drum.cpp` |
//! | [`instrument`] | `importmidi_instrument.cpp`, `generalmidi.cpp` |
//! | [`key`] | `importmidi_key.cpp` |
//! | [`lrhand`] | `importmidi_lrhand.cpp` |
//! | [`lyrics`] | `importmidi_lyrics.cpp` |
//! | [`meter`] | `importmidi_meter.cpp`, `engraving/dom/durationtype.cpp` |
//! | `midifile` | `midishared/midifile.cpp` |
//! | [`operations`] | `importmidi_operations.h` |
//! | [`quant`] | `importmidi_quant.cpp` |
//! | [`tempo`] | `importmidi_tempo.cpp` |
//! | [`timesigmap`] | `engraving/dom/sig.cpp` |
//! | [`tuplet`] | `importmidi_tuplet*.cpp` |
//! | [`voice`] | `importmidi_voice.cpp` |
//!
//! Les options sont celles de MuseScore par défaut (quantification à la double
//! croche, recherche des 3/5/7/9-olets, points, staccato, quatre voix, clefs
//! adaptées) ; la logique de panneau d'options n'est pas portée.

pub mod midifile;

pub mod beat_locator;
pub mod beatroot;
pub mod chord;
pub mod clef;
pub mod drum;
pub mod instrument;
pub mod key;
pub mod lrhand;
pub mod lyrics;
pub mod mechanical;
pub mod meter;
pub mod operations;
pub mod quant;
pub mod simplify;
pub mod tempo;
pub mod timesigmap;
pub mod tuplet;
pub mod voice;

use std::collections::BTreeMap;

use scoretk_core::model::{
    Chord, ChordRest, Duration, Fraction, Instrument, Key, KeyMode, Measure, Note, Part, Score,
    Segment, SegmentItem, SegmentType, TimeSig, Track, Tuplet, VOICES,
};

use chord::{
    collect_chords, find_min_duration, max_note_off_time,
    merge_chords_with_equal_on_time_and_voice, remove_overlapping_notes, split_unequal_chords,
    MTrack, MidiChord, MidiNote,
};
use meter::{meter_to_duration_list_with_sig, DurationKind};
use midifile::{decode_meta_text, meta, EventKind, MidiFile, UNITS_PER_WHOLE};
use timesigmap::{fraction_from_ticks, TimeSigMap, Timesig};

/// Options d'import MIDI (`MidiOperations`) et nombre de voix.
pub use operations::{ImportOptions, QuantValue, VoiceCount};
pub use scoretk_midi_common::Error;

/// Lit un fichier MIDI et renvoie le modèle pivot, avec les options par défaut
/// de MuseScore.
pub fn read_midi(bytes: &[u8]) -> Result<Score, Error> {
    read_midi_with(bytes, &ImportOptions::default())
}

/// Lit un fichier MIDI avec des options explicites.
pub fn read_midi_with(bytes: &[u8], options: &ImportOptions) -> Result<Score, Error> {
    let mut midi = MidiFile::read(bytes).map_err(Error::MidiParse)?;
    load_midi_data(&mut midi);
    let mut score = Score::new();
    convert_midi(&mut score, &midi, options)?;
    Ok(score)
}

/// Prépare les données MIDI (`loadMidiData`) : canaux séparés, notes fusionnées.
fn load_midi_data(midi: &mut MidiFile) {
    midi.separate_channel();
    midi.merge_note_on_off();
}

/// Convertit le fichier en partition (`convertMidi`).
fn convert_midi(score: &mut Score, midi: &MidiFile, options: &ImportOptions) -> Result<(), Error> {
    let mut sigmap = TimeSigMap::new();
    let mut tracks = create_mtrack_list(&mut sigmap, midi, options);
    lengthen_too_short_notes(&mut tracks);

    // Détection d'une performance humaine, comme `Quantize::setIfHumanPerformance`.
    let (is_human, quant_value, max_voice_count, beat_data) =
        detect_human_performance(midi, &tracks, &sigmap, options);
    // Extension délibérée (docs/agents/midi.md) : un MIDI dont toutes les
    // attaques ET fins de note sont déjà sur la grille est mécanique (par
    // exemple gravé par LilyPond). Les heuristiques de `lengthenNote` et
    // `minimizeNumberOfRests` (importmidi_simplify.cpp:56-165, :211-278)
    // ont été conçues pour des performances humaines : elles altèrent des
    // durées déjà justes. Le chemin MuseScore reste inchangé sinon.
    let mechanical =
        mechanical::is_mechanical_file(&tracks, quant::quant_value_to_fraction(quant_value));
    // `MidiBeat::findBeatLocations` remplace la mesure du fichier par la
    // signature retenue (`setTimeSig`, importmidi_beat.cpp:305-309).
    if let Some(data) = &beat_data {
        sigmap = TimeSigMap::new();
        sigmap.add(0, data.time_sig);
    }

    for track in &mut tracks {
        collect_chords(
            track,
            Fraction::from_integer(2),
            Fraction::from_integer(1) / Fraction::from_integer(2),
            is_human,
        );
    }
    // `MidiBeat::adjustChordsToBeats` juste après `collectChords`
    // (importmidi.cpp:1169-1170).
    if let Some(data) = &beat_data {
        beat_locator::adjust_chords_to_beats(&mut tracks, data);
    }
    for track in &mut tracks {
        merge_chords_with_equal_on_time_and_voice(track);
    }

    // `setLeftRightHandSplit` puis `LRHand::splitIntoLeftRightHands`.
    let split_flags = lrhand::find_splits(&tracks, options);
    lrhand::split_into_left_right_hands(&mut tracks, &split_flags);
    drum::split_drum_voices(&mut tracks);
    drum::split_drum_tracks(&mut tracks);

    let last_tick = find_last_chord_tick(&tracks);
    let detected_tuplets = quantize_all_tracks(
        &mut tracks,
        &sigmap,
        last_tick,
        quant_value,
        options,
        is_human,
    );

    for track in &mut tracks {
        remove_overlapping_notes(track);
    }
    // `Simplify::simplifyDurations` teste l'option `simplifyDurations` de la
    // piste (`importmidi_simplify.cpp:298`) : les tests amont `dontSimplify`
    // l'ont à faux (`midiimport_tests.cpp:86`).
    if options.simplify_durations && !mechanical {
        simplify::simplify_durations_not_drums(&mut tracks, &sigmap, is_human, options.use_dots);
    }
    if voice::separate_voices(&mut tracks, &sigmap, max_voice_count, options.use_dots)
        && options.simplify_durations
        && !mechanical
    {
        simplify::simplify_durations_not_drums(&mut tracks, &sigmap, is_human, options.use_dots);
    }
    if options.simplify_durations && !mechanical {
        simplify::simplify_durations_for_drums(&mut tracks, &sigmap, is_human, options.use_dots);
    }
    drum::split_unequal_drum_chords(&mut tracks);
    for track in &mut tracks {
        split_unequal_chords(track);
    }

    // N-olets détectés sur les accords bruts puis insérés après les étapes de
    // durée : le filtre complet est porté, mais la détection précoce invente
    // encore 182 n-olets dans `v4/timesig-09` (aucun dans la source).
    if options.tuplet_notation {
        for (index, track) in tracks.iter_mut().enumerate() {
            if let Some(candidates) = detected_tuplets.get(index) {
                tuplet::apply_tuplets(
                    track,
                    &sigmap,
                    candidates,
                    quant::quant_value_to_fraction(quant_value),
                );
            }
        }
    }
    let mut track_list = prepare_track_list(tracks);
    instrument::set_grand_staff_program(&mut track_list);
    create_instruments(score, &mut track_list, options);
    drum::set_staff_bracket_for_drums(score, &track_list);

    let first_tick = find_first_chord_tick(&track_list);
    let last_tick = create_measures(score, &sigmap, first_tick, last_tick);
    process_non_lyric_meta(score, &mut track_list, midi, &sigmap);
    set_track_info(score, &mut track_list);
    create_keys(score, &mut track_list);
    key::recognize_main_key_sig(score, &mut track_list);
    create_notes(score, &mut track_list, last_tick, options);
    process_lyric_meta(score, &mut track_list, midi, &sigmap);
    clef::create_clefs(score, &track_list, options);
    create_time_signatures(score, &sigmap);
    lyrics::set_lyrics_to_score(score, &track_list);
    tempo::set_tempo(score, midi, &sigmap, options);

    Ok(())
}

/// Construit la liste des pistes à partir du fichier
/// (`createMTrackList`).
///
/// Les indications de mesure de toutes les pistes alimentent la carte
/// partagée ; seules les pistes qui portent des notes deviennent des pistes
/// d'import, dans l'ordre du fichier.
fn create_mtrack_list(
    sigmap: &mut TimeSigMap,
    midi: &MidiFile,
    options: &ImportOptions,
) -> Vec<MTrack> {
    sigmap.add(0, Timesig::default());
    let mut tracks = Vec::new();

    for (midi_index, midi_track) in midi.tracks.iter().enumerate() {
        let mut track = MTrack {
            index: midi_index,
            index_of_operation: 0,
            program: 0,
            drum_track: midi_track.drum_track,
            out_channel: midi_track.out_channel.max(0),
            division: midi.division,
            is_division_in_tps: midi.is_division_in_tps,
            events: midi_track.events.clone(),
            ..MTrack::default()
        };
        let mut has_notes = false;

        for (tick, raw) in &midi_track.events {
            let tick = midifile::to_muse_score_ticks(*tick, midi.division, midi.is_division_in_tps);
            match &raw.kind {
                EventKind::Meta { kind, data } if *kind == meta::TIME_SIGNATURE => {
                    let Some(timesig) = meta_time_signature(data) else {
                        continue;
                    };
                    let (bars, _, _) = sigmap.tick_values(tick.ticks(UNITS_PER_WHOLE as u32));
                    sigmap.add(sigmap.bar2tick(bars, 0), timesig);
                }
                EventKind::Note {
                    pitch,
                    velocity,
                    len,
                } => {
                    has_notes = true;
                    let length =
                        midifile::to_muse_score_ticks(*len, midi.division, midi.is_division_in_tps);
                    let note =
                        MidiNote::new(i32::from(*pitch), i32::from(*velocity), tick + length, tick);
                    let chord = MidiChord {
                        notes: vec![note],
                        ..MidiChord::default()
                    };
                    track.chords.insert(tick, chord);
                }
                EventKind::Program(program) => {
                    track.program = *program;
                }
                EventKind::Controller { controller, value }
                    if *controller == midifile::ctrl::VOLUME =>
                {
                    track.volumes.insert(tick, *value);
                }
                EventKind::Controller { controller, value }
                    if *controller == midifile::ctrl::PANPOT =>
                {
                    track.pan = Some(*value);
                }
                _ => {}
            }
        }

        if has_notes {
            track.had_initial_notes = true;
            track.index_of_operation = tracks.len();
            tracks.push(track);
        }
    }
    let _ = options;
    tracks
}

/// Indication de mesure d'un méta-évènement (`metaTimeSignature`).
fn meta_time_signature(data: &[u8]) -> Option<Timesig> {
    if data.len() < 2 {
        return None;
    }
    let numerator = i64::from(data[0]);
    let denominator = 1_i64.checked_shl(u32::from(data[1]))?;
    if numerator <= 0 || denominator <= 0 {
        return None;
    }
    Some(Timesig::new(numerator, denominator))
}

/// Allonge les notes plus courtes que la durée minimale
/// (`lengthenTooShortNotes`).
fn lengthen_too_short_notes(tracks: &mut [MTrack]) {
    let minimum = chord::min_allowed_duration();
    for track in tracks {
        for (on_time, chord) in track.chords.iter_mut() {
            for note in &mut chord.notes {
                if note.off_time - on_time < minimum {
                    note.off_time = on_time + minimum;
                }
            }
        }
    }
}

/// Détecte une performance humaine et en déduit les options
/// (`Quantize::setIfHumanPerformance`).
fn detect_human_performance(
    midi: &MidiFile,
    tracks: &[MTrack],
    sigmap: &TimeSigMap,
    options: &ImportOptions,
) -> (
    bool,
    operations::QuantValue,
    operations::VoiceCount,
    Option<beat_locator::BeatData>,
) {
    let mut all = MTrack::default();
    for track in tracks {
        for (on_time, chord) in track.chords.iter() {
            all.chords.insert(on_time, chord.clone());
        }
    }
    collect_chords(&mut all, Fraction::from_integer(2), Fraction::HALF, false);
    let is_human = quant::is_human_performance(&all, sigmap);
    if is_human {
        // `Quantize::setIfHumanPerformance` appelle `MidiBeat::findBeatLocations`
        // avec la piste agrégée construite ci-dessus (`importmidi_quant.cpp:474`).
        let beat_data = beat_locator::find_beat_locations(
            &all,
            sigmap,
            beat_locator::ticks_per_second(midi, tracks),
        );
        (
            true,
            operations::QuantValue::Q8,
            operations::VoiceCount::V2,
            beat_data,
        )
    } else {
        (false, options.quant_value, options.max_voice_count, None)
    }
}

/// Quantifie toutes les pistes (`quantizeAllTracks`).
fn quantize_all_tracks(
    tracks: &mut [MTrack],
    sigmap: &TimeSigMap,
    last_tick: Fraction,
    quant_value: operations::QuantValue,
    options: &ImportOptions,
    is_human: bool,
) -> Vec<Vec<tuplet::TupletInfo>> {
    let basic_quant = quant::quant_value_to_fraction(quant_value);
    let mut detected_tuplets: Vec<Vec<tuplet::TupletInfo>> = Vec::with_capacity(tracks.len());
    for track in tracks {
        if track.chords.is_empty() {
            detected_tuplets.push(Vec::new());
            continue;
        }
        chord::set_bar_indexes(track, basic_quant, last_tick, sigmap);
        // `importmidi.cpp:274` : tolérance 4/3 juste avant la quantification.
        // Ici, avant l'attribution simplifiée des voix, sinon les notes d'un
        // accord échelonné portent déjà des voix distinctes et ne fusionnent plus.
        collect_chords(
            track,
            Fraction::from_integer(2),
            Fraction::new(4, 3),
            is_human,
        );
        voice::assign_initial_voices(track, options.max_voice_count.limit());
        if options.tuplet_notation && (options.search_tuplets || track.drum_track) {
            tuplet::detect_and_assign_early(track, sigmap, basic_quant, options, is_human);
            detected_tuplets.push(Vec::new());
        } else if track.drum_track {
            drum::find_all_tuplets_for_drums(track, sigmap, basic_quant);
        } else if options.search_tuplets {
            tuplet::find_all_tuplets(track, sigmap, basic_quant);
            detected_tuplets.push(Vec::new());
        } else {
            detected_tuplets.push(Vec::new());
        }
        quant::quantize_chords(track, basic_quant);
        if track.tuplets.is_empty() {
            voice::assign_initial_voices_quantized(
                track,
                options.max_voice_count.limit(),
                basic_quant,
            );
        }
        tuplet::remove_empty_tuplets(track);
        // `MidiTuplet::assignVoices` : les accords hors n-olet repartent sur la
        // voix la plus proche des n-olets (voix 0 sans n-olet), avant
        // `MidiVoice::separateVoices`.
        voice::assign_non_tuplet_voices(
            track,
            options.max_voice_count.limit(),
            basic_quant,
            sigmap,
        );
    }
    detected_tuplets
}

/// Tick du premier accord (`findFirstChordTick`).
fn find_first_chord_tick(tracks: &[MTrack]) -> Fraction {
    let mut first = Fraction::ZERO;
    for track in tracks {
        if let Some((on_time, _)) = track.chords.iter().next() {
            if first == Fraction::ZERO || on_time < first {
                first = on_time;
            }
        }
    }
    first
}

/// Tick de la dernière fin de note (`findLastChordTick`).
fn find_last_chord_tick(tracks: &[MTrack]) -> Fraction {
    let mut last = Fraction::ZERO;
    for track in tracks {
        for (_, chord) in track.chords.iter() {
            let off_time = max_note_off_time(&chord.notes);
            if off_time > last {
                last = off_time;
            }
        }
    }
    last
}

/// Pistes à importer (`prepareTrackList`).
fn prepare_track_list(tracks: Vec<MTrack>) -> Vec<MTrack> {
    tracks
        .into_iter()
        .filter(|track| track.had_initial_notes)
        .collect()
}

/// Crée les parties et portées (`MidiInstr::createInstruments`).
///
/// Une partie regroupe deux portées quand deux pistes successives forment un
/// grand staff (`isGrandStaff` : même canal, programme de piano ou équivalent,
/// aucune percussion).
fn create_instruments(score: &mut Score, tracks: &mut [MTrack], options: &ImportOptions) {
    let mut index = 0;
    while index < tracks.len() {
        let grand_staff = index + 1 < tracks.len()
            && instrument::is_grand_staff(&tracks[index], &tracks[index + 1]);
        let name = instrument::track_instrument_name(&tracks[index]);
        let long_name = instrument::concatenate_with_comma(&name, &tracks[index].name);
        let mut part = Part::new(long_name.clone(), String::new());
        part.name = long_name;
        part.instrument = Instrument::new(name.clone(), name.clone(), String::new());
        part.instrument.id =
            scoretk_core::model::recognize_musicxml_id(&name, Some(tracks[index].program))
                .unwrap_or("")
                .to_owned();
        part.instrument.track_name = name.clone();
        if let Some(template) = instrument::template_for_track(&tracks[index]) {
            part.instrument.transpose = if template.transpose_chromatic == 0 {
                None
            } else {
                Some(scoretk_core::model::Transpose::new(
                    template.transpose_diatonic,
                    template.transpose_chromatic,
                ))
            };
            if !template.music_xml_id.is_empty() {
                part.instrument.id = template.music_xml_id.to_owned();
            }
        }
        part.instrument.midi_program = Some(if tracks[index].drum_track {
            0
        } else {
            tracks[index].program
        });
        part.instrument.midi_channel = Some((tracks[index].out_channel.max(0) as u8).min(15));
        part.instrument.volume = Some(
            tracks[index]
                .volumes
                .values()
                .next()
                .copied()
                .unwrap_or(100),
        );
        part.instrument.pan = tracks[index].pan.or(Some(64));
        let part_id = score.add_part(part);
        let first_staff = score.add_staff(part_id);
        tracks[index].staff = Some(first_staff);
        if grand_staff {
            // Le programme de la première piste vaut pour les deux.
            tracks[index + 1].program = tracks[index].program;
            let second_staff = score.add_staff(part_id);
            tracks[index + 1].staff = Some(second_staff);
            index += 2;
        } else {
            index += 1;
        }
    }
    let _ = options;
}

/// Crée les mesures (`createMeasures`).
/// Crée les mesures et renvoie le nouveau tick de fin de partition
/// (`createMeasures` met à jour `lastTick` avec la fin de la dernière mesure).
fn create_measures(
    score: &mut Score,
    sigmap: &TimeSigMap,
    first_tick: Fraction,
    last_tick: Fraction,
) -> Fraction {
    let (mut bar_count, beat, tick) = sigmap.tick_values(last_tick.ticks(UNITS_PER_WHOLE as u32));
    if beat > 0 || tick > 0 {
        bar_count += 1;
    }
    let bar_count = bar_count.max(1) as usize;

    let first_bar_start = sigmap.bar2tick(0, 0);
    let second_bar_start = sigmap.bar2tick(1, 0);
    let first_timesig = sigmap.timesig(first_bar_start).measure_len();
    let second_timesig = sigmap.timesig(second_bar_start).measure_len();

    let mut begin = 0;
    // `isPickupWithLessTimeSig` : première mesure plus courte que la suivante.
    let _ = first_tick;
    if bar_count > 1 && first_timesig < second_timesig {
        let mut measure = Measure::new(0, Fraction::ZERO, second_timesig)
            .with_raw_timesig(first_timesig.numerator(), first_timesig.denominator());
        measure.len = first_timesig;
        score.add_measure(measure);
        begin = 1;
    }

    for index in begin..bar_count {
        let start_ticks = sigmap.bar2tick(index as i64, 0);
        let timesig = sigmap.timesig(start_ticks);
        let measure = Measure::new(
            index,
            fraction_from_ticks(start_ticks),
            timesig.measure_len(),
        )
        .with_raw_timesig(timesig.numerator, timesig.denominator);
        score.add_measure(measure);
    }
    score.end_tick()
}

/// Indication de mesure d'une mesure (nominale), pour un tick donné.
fn measure_index_for_tick(score: &Score, tick: Fraction) -> Option<usize> {
    if let Some(index) = score
        .measures
        .iter()
        .position(|measure| measure.start_tick <= tick && tick < measure.end_tick())
    {
        return Some(index);
    }
    if tick.is_negative() {
        return None;
    }
    score.measures.len().checked_sub(1)
}

/// Posée des méta-évènements non lyriques (`processNonLyricMeta`).
///
/// Le nom de piste, l'armure et les textes d'œuvre sont traités ; le tempo
/// l'est plus tard, une fois les mesures et les notes en place.
fn process_non_lyric_meta(
    score: &mut Score,
    tracks: &mut [MTrack],
    _midi: &MidiFile,
    _sigmap: &TimeSigMap,
) {
    for track in tracks.iter_mut() {
        let events = track.events.clone();
        let mut key_list = key::KeyList::new();
        for (raw_tick, raw) in &events {
            let tick =
                midifile::to_muse_score_ticks(*raw_tick, track.division, track.is_division_in_tps);
            let EventKind::Meta { kind, data } = &raw.kind else {
                continue;
            };
            match *kind {
                meta::TRACK_NAME => {
                    let text = decode_meta_text(data);
                    if track.name.is_empty() {
                        track.name = text;
                    }
                }
                meta::KEY_SIGNATURE => {
                    if data.is_empty() {
                        continue;
                    }
                    let fifths = data[0] as i8;
                    if !(-7..=7).contains(&fifths) {
                        continue;
                    }
                    let value = Key::new(fifths);
                    // `mi` (FF 59 02 sf mi) : 0 = majeur, 1 = mineur. Le mode
                    // est stocké comme dans l'import MusicXML, sans changer le
                    // nombre d'altérations du `Key`.
                    let mode = if data.get(1).is_some_and(|minor| *minor != 0) {
                        KeyMode::Minor
                    } else {
                        KeyMode::Major
                    };
                    key_list.set_key(
                        tick,
                        key::TrackKey {
                            concert_key: value,
                            key: value,
                            mode,
                        },
                    );
                    track.has_key = true;
                }
                meta::COPYRIGHT => {
                    score.metadata.copyright = decode_meta_text(data);
                }
                meta::TITLE | meta::SUBTITLE | meta::COMPOSER | meta::TRANSLATOR | meta::POET => {
                    let text = decode_meta_text(data);
                    match *kind {
                        meta::TITLE => score.metadata.work_title = text,
                        meta::SUBTITLE => score.metadata.subtitle = text,
                        meta::COMPOSER => score.metadata.composer = text,
                        meta::TRANSLATOR => score.metadata.arranger = text,
                        meta::POET => score.metadata.lyricist = text,
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        track.key_list = key_list;
    }
    // Applique les armures lues, comme `MTrack::processMeta` le fait sur la
    // portée au moment du parcours.
    for track in tracks.iter() {
        if let Some(staff) = track.staff {
            key::assign_key_list_to_staff(score, staff, &track.key_list);
        }
    }
}

/// Renseigne le nom affiché (`setTrackInfo`).
///
/// Le nom de partie est `concaténation(nom d'instrument, nom de piste)`, comme
/// `Part::setLongName(XmlWriter::xmlString(MidiInstr::concatenateWithComma(...)))`.
fn set_track_info(score: &mut Score, tracks: &mut [MTrack]) {
    for track in tracks.iter_mut() {
        let name = instrument::track_instrument_name(track);
        if track.name.is_empty() && !name.is_empty() {
            track.name = name;
        }
        let Some(staff) = track.staff else {
            continue;
        };
        let Some(part_id) = score.staves.get(staff.index()).map(|staff| staff.part) else {
            continue;
        };
        // Seule la portée du haut nomme la partie.
        let is_top = score.parts[part_id.index()]
            .staves
            .first()
            .is_some_and(|first| *first == staff);
        if !is_top {
            continue;
        }
        let long_name = instrument::concatenate_with_comma(
            &instrument::track_instrument_name(track),
            &track.name,
        );
        let part = &mut score.parts[part_id.index()];
        part.long_name = long_name.clone();
        part.name = long_name;
        part.instrument.track_name = instrument::track_instrument_name(track);
    }
}

/// Crée les armures des pistes qui n'en portent pas (`createKeys`).
fn create_keys(score: &mut Score, tracks: &mut [MTrack]) {
    let mut all_keys = key::KeyList::new();
    for track in tracks.iter() {
        if track.has_key {
            for (tick, value) in track.key_list.iter() {
                all_keys.set_key(tick, value);
            }
        }
    }
    let has_any = tracks.iter().any(|track| track.has_key);
    for track in tracks.iter_mut() {
        if track.drum_track || track.has_key {
            continue;
        }
        let staff = track.staff.expect("portée créée");
        if has_any {
            track.has_key = true;
            let list = all_keys.clone();
            track.key_list = list.clone();
            key::assign_key_list_to_staff(score, staff, &list);
        } else {
            let mut list = key::KeyList::new();
            list.set_key(
                Fraction::ZERO,
                key::TrackKey {
                    concert_key: Key::C,
                    key: Key::C,
                    mode: KeyMode::Major,
                },
            );
            track.key_list = list;
            // MuseScore n'écrit pas d'armure de do majeur : la liste vide
            // suffit.
        }
    }
}

/// Crée les notes et les silences (`createNotes`).
fn create_notes(
    score: &mut Score,
    tracks: &mut [MTrack],
    last_tick: Fraction,
    options: &ImportOptions,
) {
    for track in tracks.iter_mut() {
        create_track_notes(score, track, last_tick, options);
    }
}

/// Crée les notes d'une piste, toutes voix confondues.
fn create_track_notes(
    score: &mut Score,
    track: &mut MTrack,
    last_tick: Fraction,
    options: &ImportOptions,
) {
    let staff = track.staff.expect("portée créée");
    // N-olets du modèle, créés avant les notes.
    let mut tuplet_ids: BTreeMap<tuplet::TupletKey, scoretk_core::model::TupletId> =
        BTreeMap::new();
    for (key, data) in track.tuplets.iter() {
        let ratio = tuplet::tuplet_limits(data.tuplet_number).ratio;
        let base_len = data.len / Fraction::from_integer(ratio.denominator());
        let tuplet = Tuplet::new(ratio, base_len, data.on_time, data.len);
        tuplet_ids.insert(*key, score.add_tuplet(tuplet));
    }

    for voice in 0..options.max_voice_count.limit().min(VOICES) {
        let keys: Vec<chord::ChordKey> = track
            .chords
            .keys()
            .into_iter()
            .filter(|key| {
                track
                    .chords
                    .get(*key)
                    .map(|chord| chord.voice == voice)
                    .unwrap_or(false)
            })
            .collect();
        let mut start_chord_tick = Fraction::ZERO;
        let mut pending: Vec<MidiChord> = Vec::new();
        let mut position = 0;
        while position < keys.len() {
            let next_tick = keys[position].0;
            process_pending_notes(
                score,
                track,
                staff,
                voice,
                &mut pending,
                start_chord_tick,
                next_tick,
                &tuplet_ids,
                options,
            );
            start_chord_tick = next_tick;
            while position < keys.len() && keys[position].0 == start_chord_tick {
                if let Some(chord) = track.chords.get(keys[position]) {
                    pending.push(chord.clone());
                }
                position += 1;
            }
        }
        process_pending_notes(
            score,
            track,
            staff,
            voice,
            &mut pending,
            start_chord_tick,
            last_tick,
            &tuplet_ids,
            options,
        );
    }
}

/// Convertit les accords en attente en notes et remplit le reste de silences
/// (`MTrack::processPendingNotes`).
#[allow(clippy::too_many_arguments)]
fn process_pending_notes(
    score: &mut Score,
    track: &MTrack,
    staff: scoretk_core::model::StaffId,
    voice: usize,
    pending: &mut Vec<MidiChord>,
    start_tick: Fraction,
    next_chord_tick: Fraction,
    tuplet_ids: &BTreeMap<tuplet::TupletKey, scoretk_core::model::TupletId>,
    options: &ImportOptions,
) {
    let track_id = Track::new(staff, voice as u8);
    let transpose = score.part(score.staff(staff).part).instrument.transpose;
    let mut tick = start_tick;
    while !pending.is_empty() {
        let mut len = next_chord_tick - tick;
        if len <= Fraction::ZERO {
            break;
        }
        let refs: Vec<&MidiChord> = pending.iter().collect();
        len = find_min_duration(tick, &refs, len);
        let Some(measure_index) = measure_index_for_tick(score, tick) else {
            break;
        };
        let measure_end = score.measures[measure_index].end_tick();
        if tick + len > measure_end {
            len = measure_end - tick;
        }
        let durations = duration_list(
            score,
            track,
            measure_index,
            voice,
            tick,
            len,
            DurationKind::Note,
            options,
        );
        let Some((ratio, figure)) = durations.first().copied() else {
            break;
        };
        let len = figure.ticks() / ratio;
        let mut chord_rest = ChordRest::chord(figure);
        chord_rest.ticks = Some(figure.ticks());
        chord_rest.tuplet = tuplet::tuplet_for_element(track, voice, tick, len)
            .and_then(|key| tuplet_ids.get(&key).copied());
        if options.show_staccato
            && tick == start_tick
            && pending.first().is_some_and(MidiChord::is_staccato)
        {
            if let Some(chord) = chord_rest.chord_mut() {
                chord
                    .articulations
                    .push(scoretk_core::model::Articulation::new(
                        scoretk_core::model::ArticulationType::Staccato,
                    ));
            }
        }
        {
            let chord_model = chord_rest.chord_mut().expect("accord construit");
            let mut finished: Vec<usize> = Vec::new();
            for (index, midi_chord) in pending.iter_mut().enumerate() {
                let mut done = !midi_chord.notes.is_empty();
                for note_index in 0..midi_chord.notes.len() {
                    let note = midi_chord.notes[note_index].clone();
                    let note_id = add_note(score, chord_model, &note, tick, transpose);
                    if note.off_time - tick <= len {
                        // Dernière note de la chaîne de liaisons.
                    } else {
                        // La note continue : la note suivante s'y liera.
                        midi_chord.notes[note_index].tie = Some(note_id.index());
                        done = false;
                    }
                }
                if done {
                    finished.push(index);
                }
            }
            for index in finished.into_iter().rev() {
                pending.remove(index);
            }
        }
        let segment = get_or_create_chord_rest_segment(score, measure_index, tick);
        segment.set_element(track_id, SegmentItem::ChordRest(chord_rest));
        tick += len;
    }
    fill_gap_with_rests(
        score,
        track,
        staff,
        voice,
        tick,
        next_chord_tick - tick,
        tuplet_ids,
        options,
    );
}

/// Ajoute une note MIDI à un accord du modèle, en gérant la liaison de
/// continuation (`setMusicNotesFromMidi`).
fn add_note(
    score: &mut Score,
    chord: &mut Chord,
    midi_note: &MidiNote,
    tick: Fraction,
    transpose: Option<scoretk_core::model::Transpose>,
) -> scoretk_core::model::NoteId {
    let key = chord_key_at(score, tick);
    let tpc = pitch_to_tpc(midi_note.pitch, key);
    let mut note = Note::new(midi_note.pitch.clamp(0, 127) as u8, tpc);
    // `Part::initFromInstrTemplate` transpose le Tpc de chaque note d'un
    // instrument transpositeur ; la hauteur concert reste inchangée.
    if let Some(interval) = transpose {
        // Une transposition d'octave pure ne change pas le nom de la note :
        // seul `written_pitch` décale l'octave. Les autres transpositions
        // écrivent le Tpc transposé (`write_pitch` soustrait
        // `transpose_chromatic`, donc l'intervalle est inversé ici).
        if interval.chromatic.rem_euclid(12) != 0 {
            note.tpc_transposed = scoretk_core::model::transpose_tpc(
                tpc,
                scoretk_core::model::Transpose::new(-interval.diatonic, -interval.chromatic),
            );
        }
    }
    note.velocity = u8::try_from(midi_note.velocity.clamp(0, 127)).ok();
    note.tie_back = midi_note.tie.map(scoretk_core::model::NoteId::from_index);
    let note_id = score.add_note(note);
    if let Some(previous) = midi_note.tie {
        let previous = scoretk_core::model::NoteId::from_index(previous);
        if let Some(previous_note) = score.notes.get_mut(previous.index()) {
            previous_note.tie_forward = Some(note_id);
        }
    }
    chord.notes.push(note_id);
    note_id
}

/// Armure en vigueur à un tick, pour le calcul du `tpc` (`pitch2tpc`).
fn chord_key_at(score: &Score, tick: Fraction) -> Key {
    for measure in score.measures.iter().rev() {
        if measure.start_tick > tick {
            continue;
        }
        for segment in measure.segments.iter().rev() {
            if segment.tick > tick {
                continue;
            }
            if !segment.kind.is_key_sig_type() {
                continue;
            }
            for (_, item) in segment.iter_elements() {
                if let SegmentItem::KeySig(keysig) = item {
                    return keysig.concert_key;
                }
            }
        }
    }
    Key::C
}

/// Calcule le `tpc` d'une hauteur dans une armure (`pitch2tpc`).
///
/// Reprend la formule de `engraving/dom/pitchspelling.cpp` avec
/// `Prefer::NEAREST` (11).
pub fn pitch_to_tpc(pitch: i32, key: Key) -> i8 {
    let prefer = 11_i32;
    let value = (pitch * 7 + 26 - (prefer + i32::from(key.fifths()))).rem_euclid(12)
        + (prefer + i32::from(key.fifths()));
    value.clamp(i8::MIN as i32, i8::MAX as i32) as i8
}

/// Segment d'accord/silence d'un tick, créé au besoin
/// (`Measure::getSegment`).
pub(crate) fn get_or_create_chord_rest_segment(
    score: &mut Score,
    measure_index: usize,
    tick: Fraction,
) -> &mut Segment {
    let staff_count = score.staves.len();
    let measure = &mut score.measures[measure_index];
    let position = measure
        .segments
        .iter()
        .position(|segment| segment.kind == SegmentType::ChordRest && segment.tick == tick);
    let position = match position {
        Some(position) => position,
        None => {
            measure.add_segment(Segment::new(SegmentType::ChordRest, tick, staff_count));
            measure
                .segments
                .iter()
                .position(|segment| segment.kind == SegmentType::ChordRest && segment.tick == tick)
                .expect("segment créé")
        }
    };
    &mut score.measures[measure_index].segments[position]
}

/// Remplit un intervalle de silences (`MTrack::fillGapWithRests`).
#[allow(clippy::too_many_arguments)]
fn fill_gap_with_rests(
    score: &mut Score,
    track: &MTrack,
    staff: scoretk_core::model::StaffId,
    voice: usize,
    start_tick: Fraction,
    rest_len: Fraction,
    tuplet_ids: &BTreeMap<tuplet::TupletKey, scoretk_core::model::TupletId>,
    options: &ImportOptions,
) {
    let track_id = Track::new(staff, voice as u8);
    let mut tick = start_tick;
    let mut remaining = rest_len;
    while remaining > Fraction::ZERO {
        let Some(measure_index) = measure_index_for_tick(score, tick) else {
            break;
        };
        let measure_len = score.measures[measure_index].len;
        if tick >= score.measures[measure_index].end_tick() {
            break;
        }
        let mut len = remaining;
        let measure_end = score.measures[measure_index].end_tick();
        if tick + len > measure_end {
            len = measure_end - tick;
        }
        if len >= measure_len {
            len = measure_len;
            if voice == 0 {
                let segment = get_or_create_chord_rest_segment(score, measure_index, tick);
                let mut rest = ChordRest::full_measure_rest();
                rest.ticks = Some(measure_len);
                segment.set_element(track_id, SegmentItem::ChordRest(rest));
            }
            remaining -= len;
            tick += len;
            continue;
        }
        let durations = duration_list(
            score,
            track,
            measure_index,
            voice,
            tick,
            len,
            DurationKind::Rest,
            options,
        );
        if durations.is_empty() {
            break;
        }
        for (ratio, figure) in durations {
            let len = figure.ticks() / ratio;
            if len <= Fraction::ZERO {
                continue;
            }
            let segment = get_or_create_chord_rest_segment(score, measure_index, tick);
            let mut rest = ChordRest::rest(figure);
            rest.ticks = Some(figure.ticks());
            rest.tuplet = tuplet::tuplet_for_element(track, voice, tick, len)
                .and_then(|key| tuplet_ids.get(&key).copied());
            segment.set_element(track_id, SegmentItem::ChordRest(rest));
            remaining -= len;
            tick += len;
            if remaining <= Fraction::ZERO {
                break;
            }
        }
    }
}

/// Convertit une durée en figures pour une mesure (`MTrack::toDurationList`).
#[allow(clippy::too_many_arguments)]
fn duration_list(
    score: &Score,
    track: &MTrack,
    measure_index: usize,
    voice: usize,
    start_tick: Fraction,
    len: Fraction,
    kind: DurationKind,
    options: &ImportOptions,
) -> Vec<(Fraction, Duration)> {
    let measure = &score.measures[measure_index];
    let bar_tick = measure.start_tick;
    // En mode notation seule, les n-olets ne découpent pas les durées.
    // Durées **notées** de MuseScore pour les notes de n-olet : le découpage
    // utilise le ratio du n-olet (`toDurationList`, importmidi.cpp:553-611).
    let tuplets = tuplet::find_tuplets_in_bar_for_duration(track, voice, bar_tick, start_tick, len);
    meter_to_duration_list_with_sig(
        start_tick - bar_tick,
        start_tick - bar_tick + len,
        measure.nominal_len,
        measure.time_sig_numerator,
        &tuplets,
        kind,
        options.use_dots,
    )
}

/// Crée les indications de mesure (`createTimeSignatures`).
fn create_time_signatures(score: &mut Score, sigmap: &TimeSigMap) {
    let events: Vec<(i64, Timesig)> = sigmap
        .iter()
        .map(|(tick, event)| (tick, event.timesig))
        .collect();
    let mut previous: Option<Timesig> = None;
    for (tick, timesig) in events {
        // MuseScore n'écrit pas un changement d'indication de mesure identique
        // au précédent ; le fichier MIDI peut en contenir plusieurs.
        if previous == Some(timesig) {
            continue;
        }
        previous = Some(timesig);
        let tick = fraction_from_ticks(tick);
        let Some(measure_index) = measure_index_for_tick(score, tick) else {
            continue;
        };
        let segment = {
            let staff_count = score.staves.len();
            let measure = &mut score.measures[measure_index];
            let position = measure
                .segments
                .iter()
                .position(|segment| segment.kind == SegmentType::TimeSig && segment.tick == tick);
            let position = match position {
                Some(position) => position,
                None => {
                    measure.add_segment(Segment::new(SegmentType::TimeSig, tick, staff_count));
                    measure
                        .segments
                        .iter()
                        .position(|segment| {
                            segment.kind == SegmentType::TimeSig && segment.tick == tick
                        })
                        .expect("segment créé")
                }
            };
            &mut score.measures[measure_index].segments[position]
        };
        for staff in 0..score.staves.len() {
            let staff_id = scoretk_core::model::StaffId::from_index(staff);
            let track_id = Track::new(staff_id, 0);
            segment.set_element(
                track_id,
                SegmentItem::TimeSig(TimeSig::new(
                    timesig.numerator as i32,
                    timesig.denominator as i32,
                )),
            );
        }
    }
}
/// Pose les paroles (`processLyricMeta`).
fn process_lyric_meta(
    score: &mut Score,
    tracks: &mut [MTrack],
    midi: &MidiFile,
    _sigmap: &TimeSigMap,
) {
    for track in tracks.iter_mut() {
        let mut lyrics = Vec::new();
        for (midi_index, (tick, raw)) in midi.tracks[track.index].events.iter().enumerate() {
            let _ = midi_index;
            let EventKind::Meta { kind, data } = &raw.kind else {
                continue;
            };
            if *kind != meta::LYRIC && *kind != meta::TEXT {
                continue;
            }
            // Un texte vide est conservé : RP-017 en fait un mélisme (ou un
            // drapeau « on » au tick 0), décodé par `parse_rp017`.
            let text = decode_meta_text(data);
            let tick =
                midifile::to_muse_score_ticks(*tick, track.division, track.is_division_in_tps);
            lyrics.push((tick, text));
        }
        let parsed = lyrics::parse_rp017(&lyrics);
        lyrics::set_lyrics(score, track, &parsed);
    }
}
