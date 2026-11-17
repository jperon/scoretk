// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/compat/midi/compatmidirender.cpp`,
// `engraving/compat/midi/compatmidirenderinternal.cpp`,
// `engraving/compat/midi/velocitymap.cpp` et
// `importexport/midi/internal/midiexport/exportmidi.cpp` de MuseScore Studio,
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

//! Rendu de la partition en évènements MIDI.
//!
//! Portage de `CompatMidiRenderer` : parcours des mesures déroulées par
//! [`RepeatList`], collecte des notes (liaisons fusionnées, n-olets, notes
//! d'ornement, articulations), des nuances et soufflets (vélocités), puis des
//! méta-évènements de tempo, de mesure et d'armure.
//!
//! Une piste MIDI est produite par portée, avec le canal et le programme de la
//! partie. Les évènements sont datés en ticks MuseScore (480 par noire).

use std::collections::BTreeMap;

use mscz2xml::model::{
    Annotation, ArticulationType, Chord, ChordRest, ChordRestKind, Fraction, GraceType, Instrument,
    KeySig, Measure, Note, PartId, Score, SegmentType, SpannerType, StaffId, TimeSig, Track,
    TremoloKind, VOICES,
};

use super::error::Result;
use super::pause_map::PauseMap;
use super::repeat_list::{RepeatList, DIVISION, UNITS_PER_WHOLE};
use super::velocity::{ChangeDirection, ChangeMethod, VelocityMaps};

/// Valeurs des articulations intégrées (`s_builtInArticulationsValues`).
const BUILTIN_ARTICULATIONS: &[(&str, f64, i32)] = &[
    ("staccatissimo", 1.0, 30),
    ("staccato", 1.0, 50),
    ("portato", 1.0, 67),
    ("tenuto", 1.0, 100),
    ("accent", 1.2, 100),
    ("marcato", 1.44, 100),
    ("sforzato", 1.69, 100),
];

/// Évènement MIDI daté.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Event {
    /// Tick absolu d'exécution.
    pub tick: i64,
    /// Contenu de l'évènement.
    pub kind: EventKind,
}

/// Contenu d'un évènement MIDI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EventKind {
    TrackName(String),
    TimeSignature(u8, u8, u8, u8),
    KeySignature(i8),
    Tempo(u32),
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    ProgramChange {
        channel: u8,
        program: u8,
    },
    NoteOn {
        channel: u8,
        key: u8,
        velocity: u8,
    },
    NoteOff {
        channel: u8,
        key: u8,
    },
}

/// Piste MIDI.
#[derive(Clone, Debug, Default)]
pub(crate) struct TrackData {
    pub events: Vec<Event>,
}

/// Résultat du rendu : une piste par portée.
#[derive(Clone, Debug, Default)]
pub(crate) struct Performance {
    pub tracks: Vec<TrackData>,
}

/// Rend la partition en évènements MIDI (`CompatMidiRender::renderScore` puis
/// `ExportMidi::writeHeader`).
pub(crate) fn render(score: &Score, repeat_list: &RepeatList) -> Result<Performance> {
    let channels = channels(score);
    let velocities = build_velocities(score);
    let time_sigs = time_signature_events(score);
    let key_sigs = key_signature_events(score);
    let pause_map = PauseMap::build(score, repeat_list, &time_sigs);
    let tempo = tempo_events(score, repeat_list, &pause_map);
    let mut performance = Performance {
        tracks: vec![TrackData::default(); score.staves.len()],
    };

    for (staff_index, staff) in score.staves.iter().enumerate() {
        let part = &score.parts[staff.part.index()];
        let channel = channels[staff.part.index()];
        let is_top = part.staves.first() == Some(&staff.id);
        let track = &mut performance.tracks[staff_index];

        // Nom de piste (`writeHeader`) : le nom de la partie, identique sur
        // toutes ses portées.
        let name = if part.name.is_empty() {
            part.long_name.clone()
        } else {
            part.name.clone()
        };
        track.events.push(Event {
            tick: 0,
            kind: EventKind::TrackName(name),
        });

        // Indications de mesure : une seule fois, sur la première piste.
        if staff_index == 0 {
            push_unwound(
                track,
                repeat_list,
                &pause_map,
                &time_sigs,
                |tick, time_sig: &TimeSig| {
                    let exponent = match time_sig.denominator {
                        1 => 0,
                        2 => 1,
                        4 => 2,
                        8 => 3,
                        16 => 4,
                        32 => 5,
                        _ => 2,
                    };
                    Event {
                        tick,
                        kind: EventKind::TimeSignature(
                            time_sig.numerator.clamp(0, 255) as u8,
                            exponent,
                            24,
                            8,
                        ),
                    }
                },
            );
        }

        // Armures, portée par portée.
        if let Some(entries) = key_sigs
            .get(staff_index)
            .and_then(|entries| entries.as_ref())
        {
            push_unwound(track, repeat_list, &pause_map, entries, |tick, key: &i8| {
                Event {
                    tick,
                    kind: EventKind::KeySignature(*key),
                }
            });
        }

        // Tempo, une seule fois (les pauses ont déjà produit leurs
        // évènements dans `tempo`).
        if staff_index == 0 {
            for (tick, value) in &tempo {
                track.events.push(Event {
                    tick: *tick,
                    kind: EventKind::Tempo(*value),
                });
            }
        }

        // Réglages de canal et programme, sur la première portée de la partie
        // (`track.insert(0, …)` dans `writeHeader`).
        if is_top {
            push_channel_setup(track, &part.instrument, channel);
        }

        // Note events.
        let mut notes = Vec::new();
        let mut prev_chords = [0i64; VOICES];
        for segment in &repeat_list.segments {
            let offset = segment.utick - segment.tick;
            for measure_index in &segment.measures {
                let measure = &score.measures[*measure_index];
                render_measure(
                    score,
                    measure,
                    staff,
                    channel,
                    offset,
                    &pause_map,
                    &velocities,
                    &mut notes,
                    &mut prev_chords,
                );
            }
        }
        track.events.extend(notes);
    }

    Ok(performance)
}

/// Canal MIDI de chaque partie : canal déclaré, sinon rang de la partie.
fn channels(score: &Score) -> Vec<u8> {
    score
        .parts
        .iter()
        .enumerate()
        .map(|(index, part)| part.instrument.midi_channel.unwrap_or(index as u8).min(15))
        .collect()
}

/// Contrôleurs initiaux d'un canal (`writeHeader`, branche `staff->isTop()`).
fn push_channel_setup(track: &mut TrackData, instrument: &Instrument, channel: u8) {
    // Ordre final du fichier de MuseScore : reset, RPN, programme, volume,
    // panoramique, réverbération, chœur.
    let controllers = if channel == 9 {
        vec![(121, 0)]
    } else {
        vec![
            (121, 0),
            (100, 0),
            (101, 0),
            (6, 12),
            (100, 127),
            (101, 127),
        ]
    };
    for (controller, value) in controllers {
        track.events.push(Event {
            tick: 0,
            kind: EventKind::ControlChange {
                channel,
                controller,
                value,
            },
        });
    }
    if let Some(program) = instrument.midi_program {
        track.events.push(Event {
            tick: 0,
            kind: EventKind::ProgramChange { channel, program },
        });
    }
    for (controller, value) in [
        (7, instrument.volume.unwrap_or(100)),
        (10, instrument.pan.unwrap_or(64)),
        (91, 0),
        (93, 0),
    ] {
        track.events.push(Event {
            tick: 0,
            kind: EventKind::ControlChange {
                channel,
                controller,
                value,
            },
        });
    }
}

/// Recopie des évènements sources déroulés par les répétitions
/// (`writeHeader`, boucles sur `repeatList`).
fn push_unwound<T, F>(
    track: &mut TrackData,
    repeat_list: &RepeatList,
    pause_map: &PauseMap,
    entries: &[(i64, T)],
    make: F,
) where
    F: Fn(i64, &T) -> Event,
{
    for segment in &repeat_list.segments {
        let end = repeat_list.segment_end(segment);
        let offset = segment.utick - segment.tick;
        for (tick, value) in entries {
            if *tick >= segment.tick && *tick < end {
                let utick = pause_map.tick_with_pauses(tick + offset);
                track.events.push(make(utick, value));
            }
        }
    }
}

/// Indications de mesure, dédoublonnées par tick.
fn time_signature_events(score: &Score) -> Vec<(i64, TimeSig)> {
    let mut events: BTreeMap<i64, TimeSig> = BTreeMap::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            if segment.kind != SegmentType::TimeSig {
                continue;
            }
            for (_, item) in segment.iter_elements() {
                if let mscz2xml::model::SegmentItem::TimeSig(time_sig) = item {
                    events.insert(segment_tick(segment.tick), time_sig.clone());
                    break;
                }
            }
        }
    }
    events.into_iter().collect()
}

/// Armures de chaque portée, dédoublonnées par tick.
fn key_signature_events(score: &Score) -> Vec<Option<Vec<(i64, i8)>>> {
    let mut result = Vec::with_capacity(score.staves.len());
    for staff in &score.staves {
        let mut events: BTreeMap<i64, i8> = BTreeMap::new();
        for measure in &score.measures {
            for segment in &measure.segments {
                if !segment.kind.is_key_sig_type() {
                    continue;
                }
                for (track, item) in segment.iter_elements() {
                    if track.staff() != staff.id {
                        continue;
                    }
                    if let mscz2xml::model::SegmentItem::KeySig(KeySig { concert_key, .. }) = item {
                        events.insert(segment_tick(segment.tick), concert_key.fifths());
                    }
                }
            }
        }
        let mut entries: Vec<(i64, i8)> = events.into_iter().collect();
        if entries.first().is_none_or(|(tick, _)| *tick != 0) {
            // `writeHeader` écrit une armure de do majeur quand le fichier n'en
            // donne aucune au début.
            entries.insert(0, (0, 0));
        }
        result.push(Some(entries));
    }
    result
}

/// Tick d'un segment en ticks MuseScore.
fn segment_tick(tick: Fraction) -> i64 {
    tick.ticks(UNITS_PER_WHOLE)
}

/// Tempo initial et changements, en microsecondes par noire.
///
/// Les changements sont déroulés comme les autres évènements, puis les pauses
/// de section ajoutent leur tempo et le retour au tempo en vigueur.
fn tempo_events(score: &Score, repeat_list: &RepeatList, pause_map: &PauseMap) -> Vec<(i64, u32)> {
    // MuseScore initialise toujours le tempo par défaut à 2 battements par
    // seconde (500 000 µs) quand aucune indication n'ouvre la partition.
    let mut source: BTreeMap<i64, f64> = BTreeMap::new();
    source.insert(0, 2.0);
    for measure in &score.measures {
        for segment in &measure.segments {
            for entry in &segment.annotations {
                if let Annotation::Tempo(tempo) = &entry.annotation {
                    if let Some(beats_per_second) = tempo.beats_per_second {
                        source.insert(segment_tick(segment.tick), beats_per_second);
                    }
                }
            }
        }
    }

    let to_micros = |beats_per_second: f64| {
        let micros = (1.0 / beats_per_second * 1_000_000.0).round();
        micros.clamp(1.0, 0x00ff_ffff as f64) as u32
    };

    let mut events: BTreeMap<i64, u32> = BTreeMap::new();
    for segment in &repeat_list.segments {
        let end = repeat_list.segment_end(segment);
        let offset = segment.utick - segment.tick;
        for (tick, beats_per_second) in &source {
            if *tick >= segment.tick && *tick < end {
                let utick = pause_map.tick_with_pauses(tick + offset);
                events.insert(utick, to_micros(*beats_per_second));
            }
        }
    }
    for (utick, beats_per_second) in &pause_map.tempo_events {
        events.insert(*utick, to_micros(*beats_per_second));
    }
    events.into_iter().collect()
}

/// Carte des vélocités depuis les nuances et les soufflets
/// (`fillScoreVelocities`).
fn build_velocities(score: &Score) -> VelocityMaps {
    let mut maps = VelocityMaps::default();
    for measure in &score.measures {
        for segment in &measure.segments {
            for entry in &segment.annotations {
                let Annotation::Dynamic(dynamic) = &entry.annotation else {
                    continue;
                };
                let Some(velocity) = dynamic.velocity else {
                    continue;
                };
                if velocity < 1 {
                    continue;
                }
                let value = i32::from(velocity).clamp(1, 127);
                let staff = entry.track.staff();
                for voice in 0..VOICES {
                    maps.map_mut(track_raw(staff, voice))
                        .add_dynamic(segment_tick(segment.tick), value);
                }
            }
        }
    }
    for spanner in &score.spanners {
        let SpannerType::Hairpin(kind) = spanner.kind else {
            continue;
        };
        let direction = match kind {
            mscz2xml::model::HairpinType::Crescendo => ChangeDirection::Increasing,
            mscz2xml::model::HairpinType::Diminuendo => ChangeDirection::Decreasing,
        };
        let start = segment_tick(spanner.start.tick);
        let end = segment_tick(spanner.end.tick);
        if end <= start {
            continue;
        }
        let staff = spanner.start.track.staff();
        for voice in 0..VOICES {
            maps.map_mut(track_raw(staff, voice)).add_hairpin(
                start,
                end,
                0,
                ChangeMethod::Normal,
                direction,
            );
        }
    }
    maps.setup();
    maps
}

fn track_raw(staff: StaffId, voice: usize) -> u32 {
    Track::new(staff, voice as u8).raw()
}

/// Parcourt les segments d'une mesure pour une portée
/// (`CompatMidiRendererInternal::doCollectMeasureEvents`).
#[allow(clippy::too_many_arguments)]
fn render_measure(
    score: &Score,
    measure: &Measure,
    staff: &mscz2xml::model::Staff,
    channel: u8,
    offset: i64,
    pause_map: &PauseMap,
    velocities: &VelocityMaps,
    out: &mut Vec<Event>,
    prev_chords: &mut [i64; VOICES],
) {
    for (segment_index, segment) in measure.segments.iter().enumerate() {
        if !segment.kind.is_chord_rest_type() {
            continue;
        }
        for (voice, previous) in prev_chords.iter_mut().enumerate() {
            let track = Track::new(staff.id, voice as u8);
            let Some(chord_rest) = segment.chord_rest(track) else {
                *previous = 0;
                continue;
            };
            let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                *previous = 0;
                continue;
            };
            let source_tick = segment_tick(segment.tick);
            let utick = pause_map.tick_with_pauses(source_tick + offset);
            let ticks = actual_ticks(score, staff, chord_rest, measure);
            if ticks <= 0 {
                continue;
            }

            // Trémolo à deux notes : le premier accord rend les deux, le
            // second est muet (`TremoloFirstChord`/`TremoloSecondChord`).
            if chord.tremolo_two_chord {
                match chord.tremolo.map(|tremolo| tremolo.kind) {
                    Some(TremoloKind::Start) => {
                        let next =
                            measure.segments[segment_index + 1..]
                                .iter()
                                .find_map(|segment| {
                                    segment.chord_rest(track).and_then(|chord_rest| {
                                        matches!(chord_rest.kind, ChordRestKind::Chord(_))
                                            .then_some(chord_rest)
                                    })
                                });
                        if let Some(next) = next {
                            let next_ticks = actual_ticks(score, staff, next, measure);
                            render_two_chord_tremolo(
                                score,
                                staff,
                                chord,
                                chord_rest.duration,
                                next,
                                next_ticks,
                                utick,
                                ticks,
                                channel,
                                track_raw(staff.id, voice),
                                source_tick,
                                velocities,
                                out,
                            );
                        }
                    }
                    Some(TremoloKind::Stop | TremoloKind::Single)
                    | Some(TremoloKind::Unmeasured)
                    | None => {}
                }
                *previous = ticks;
                continue;
            }

            render_chord(
                score,
                staff,
                chord,
                chord_rest.duration,
                utick,
                ticks,
                *previous,
                channel,
                track_raw(staff.id, voice),
                source_tick,
                velocities,
                out,
            );
            *previous = ticks;
        }
    }
}

/// Rend un trémolo à deux accords (`renderTremolo`, `TremoloFirstChord`).
#[allow(clippy::too_many_arguments)]
fn render_two_chord_tremolo(
    score: &Score,
    staff: &mscz2xml::model::Staff,
    first: &Chord,
    duration: mscz2xml::model::Duration,
    second: &ChordRest,
    second_ticks: i64,
    utick: i64,
    ticks: i64,
    channel: u8,
    track: u32,
    source_tick: i64,
    velocities: &VelocityMaps,
    out: &mut Vec<Event>,
) {
    let _ = second_ticks;
    let Some(tremolo) = first.tremolo else {
        return;
    };
    let ChordRestKind::Chord(second_chord) = &second.kind else {
        return;
    };
    let instrument = instrument_for(score, staff.part, source_tick);
    let chord_velocity = chord_velocity_multiplier(first, instrument);
    let gate = gate_time(score, staff, instrument, first, source_tick);

    let exponent = u32::from(tremolo.count) + u32::from(duration.kind.hooks());
    let step = (DIVISION >> exponent.min(31)).max(1) as i64;
    // `tticks` couvre les deux accords, `l` est la durée d'un évènement en
    // millièmes de la durée du premier accord.
    let total_ticks = ticks * 2;
    let events = total_ticks / step / 2;
    if events <= 0 {
        return;
    }
    let length = 2000 * step / total_ticks;
    let velocity_base = velocities.val(track, source_tick) as f64 * chord_velocity;
    let velocity = (velocity_base.round() as i32).clamp(1, 127) as u8;

    for index in 0..events {
        let ontime = length * index * 2;
        let second_ontime = ontime + length;
        let first_key = first.notes.first().map(|id| score.notes[id.index()].pitch);
        let second_key = second_chord
            .notes
            .first()
            .map(|id| score.notes[id.index()].pitch);
        let Some(first_key) = first_key else {
            continue;
        };
        let key = first_key.min(127);
        out.push(Event {
            tick: utick + ticks * ontime / 1000,
            kind: EventKind::NoteOn {
                channel,
                key,
                velocity,
            },
        });
        out.push(Event {
            tick: utick + ticks * ontime / 1000 + ticks * (length * gate / 100) / 1000 - 1,
            kind: EventKind::NoteOff { channel, key },
        });
        if let Some(second_key) = second_key {
            let key = second_key.min(127);
            let start = utick + ticks * second_ontime / 1000;
            out.push(Event {
                tick: start,
                kind: EventKind::NoteOn {
                    channel,
                    key,
                    velocity,
                },
            });
            out.push(Event {
                tick: start + ticks * (length * gate / 100) / 1000 - 1,
                kind: EventKind::NoteOff { channel, key },
            });
        }
    }
}

/// Durée effective d'un accord ou silence en ticks MuseScore
/// (`DurationElement::actualTicks`).
fn actual_ticks(
    score: &Score,
    staff: &mscz2xml::model::Staff,
    chord_rest: &ChordRest,
    measure: &Measure,
) -> i64 {
    let mut ticks = if chord_rest.is_full_measure_rest() {
        measure.len
    } else {
        chord_rest
            .ticks
            .unwrap_or_else(|| chord_rest.duration.ticks())
    };

    // `Staff::timeStretch` (indication de mesure locale).
    if !staff.time_stretch.is_zero() {
        ticks /= staff.time_stretch;
    }

    // N-olets, du plus interne au plus externe.
    let mut tuplet = chord_rest.tuplet;
    while let Some(id) = tuplet {
        let value = &score.tuplets[id.index()];
        ticks /= value.ratio;
        tuplet = value.parent;
    }
    ticks.ticks(UNITS_PER_WHOLE)
}

/// Rend un accord : notes d'ornement avant, note principale, ornements après
/// (`collectGraceBeforeChordEvents`, `collectNote`, `collectGraceNotesAfter`).
#[allow(clippy::too_many_arguments)]
fn render_chord(
    score: &Score,
    staff: &mscz2xml::model::Staff,
    chord: &Chord,
    duration: mscz2xml::model::Duration,
    utick: i64,
    ticks: i64,
    previous_chord_ticks: i64,
    channel: u8,
    track: u32,
    source_tick: i64,
    velocities: &VelocityMaps,
    out: &mut Vec<Event>,
) {
    let instrument = instrument_for(score, staff.part, source_tick);
    let chord_velocity = chord_velocity_multiplier(chord, instrument);
    let gate = gate_time(score, staff, instrument, chord, source_tick);

    let mut before = Vec::new();
    let mut after = Vec::new();
    for grace in &chord.grace_chords {
        match grace.chord.grace {
            Some(GraceType::Grace8After | GraceType::Grace16After | GraceType::Grace32After) => {
                after.push(grace)
            }
            _ => before.push(grace),
        }
    }

    let mut ontime = 0i64;
    let mut trailtime = 0i64;

    let acciaccatura = before
        .first()
        .is_some_and(|grace| grace.chord.grace == Some(GraceType::Acciaccatura));
    if acciaccatura {
        // `collectGraceBeforeChordEvents` : les acciaccaturas occupent au plus
        // la moitié de l'accord précédent, avant le temps.
        let grace_ticks = before
            .first()
            .map_or(0, |grace| grace.duration.ticks().ticks(UNITS_PER_WHOLE));
        let mut sum = grace_ticks;
        if previous_chord_ticks > 0 {
            sum = sum.min(previous_chord_ticks / 2);
        }
        let offset = sum / before.len().max(1) as i64;
        for (index, grace) in before.iter().enumerate() {
            let offset_on = sum - offset * index as i64;
            let offset_off = sum - offset * (index as i64 + 1);
            let on = utick - offset_on;
            let off = utick - 1 - offset_off;
            render_grace_notes(
                score,
                staff,
                &grace.chord,
                on,
                off,
                channel,
                track,
                &chord_velocity,
                velocities,
                source_tick,
                out,
            );
        }
    } else if !before.is_empty() {
        // Appoggiatures : au plus la moitié de l'accord principal, les unes
        // après les autres en prenant le temps de l'accord.
        let grace_ticks = before
            .first()
            .map_or(0, |grace| grace.duration.ticks().ticks(UNITS_PER_WHOLE));
        ontime = (grace_ticks * 1000 / ticks.max(1)).min(500);
        trailtime += ontime;
        let each = ontime / before.len().max(1) as i64;
        for (index, grace) in before.iter().enumerate() {
            let on = utick + ticks * each * index as i64 / 1000;
            let off = on + ticks * each / 1000 - 1;
            render_grace_notes(
                score,
                staff,
                &grace.chord,
                on,
                off,
                channel,
                track,
                &chord_velocity,
                velocities,
                source_tick,
                out,
            );
        }
    }

    if !after.is_empty() {
        // Notes d'ornement après l'accord (`createGraceNotesPlayEvents`).
        let weight = after.len() as f64 / (before.len() + after.len()) as f64;
        trailtime = match duration.dots {
            1 => (667.0 * weight).floor() as i64,
            2 => (571.0 * weight).floor() as i64,
            _ => (500.0 * weight).floor() as i64,
        };
        let each = trailtime / after.len() as i64;
        let mut on = 1000 - trailtime;
        for grace in &after {
            let start = utick + ticks * on / 1000;
            let off = start + ticks * each / 1000 - 1;
            render_grace_notes(
                score,
                staff,
                &grace.chord,
                start,
                off,
                channel,
                track,
                &chord_velocity,
                velocities,
                source_tick,
                out,
            );
            on += each;
        }
    }

    let length = (1000 - trailtime).max(0) * gate / 100;
    let on = utick + ticks * ontime / 1000;
    let velocity_base = velocities.val(track, source_tick);

    // Trémolo sur une note (`renderTremolo`, `TremoloSingle`) : la note est
    // répétée à la figure du trémolo, en millièmes de la durée de l'accord.
    let tremolo = match chord.tremolo {
        Some(tremolo) if tremolo.kind == TremoloKind::Single => {
            let exponent = u32::from(tremolo.count) + u32::from(duration.kind.hooks());
            let step = (DIVISION >> exponent.min(31)).max(1) as i64;
            let events = ticks / step;
            (events > 0).then(|| (events, 1000 / events.max(1)))
        }
        _ => None,
    };

    for note in &chord.notes {
        let note = &score.notes[note.index()];
        if !note.play {
            continue;
        }
        if note.tie_back.is_some() {
            // Note déjà tenue par le début de la liaison : pas de nouvelle
            // attaque, la durée a été ajoutée à la note de départ.
            continue;
        }
        let mut velocity = velocity_base as f64 * chord_velocity;
        if note.ghost {
            velocity *= 0.6;
        }
        let velocity = (velocity.round() as i32).clamp(1, 127) as u8;
        let key = note.pitch.min(127);
        if let Some((events, step)) = tremolo {
            for index in 0..events {
                let start = on + ticks * (index * step) / 1000;
                let end = start + ticks * (step * gate / 100) / 1000 - 1;
                out.push(Event {
                    tick: start,
                    kind: EventKind::NoteOn {
                        channel,
                        key,
                        velocity,
                    },
                });
                out.push(Event {
                    tick: end.max(start + 1),
                    kind: EventKind::NoteOff { channel, key },
                });
            }
            continue;
        }
        let mut off = on + ticks * length / 1000 - 1;
        // Prolongation des liaisons : chaque note liée ajoute sa durée.
        off += tie_length(score, staff, note);
        out.push(Event {
            tick: on,
            kind: EventKind::NoteOn {
                channel,
                key,
                velocity,
            },
        });
        out.push(Event {
            tick: off.max(on + 1),
            kind: EventKind::NoteOff { channel, key },
        });
    }
}

/// Notes d'un accord d'ornement, rendues telles quelles.
#[allow(clippy::too_many_arguments)]
fn render_grace_notes(
    score: &Score,
    staff: &mscz2xml::model::Staff,
    chord: &Chord,
    on: i64,
    off: i64,
    channel: u8,
    track: u32,
    velocity_multiplier: &f64,
    velocities: &VelocityMaps,
    source_tick: i64,
    out: &mut Vec<Event>,
) {
    let velocity_base = velocities.val(track, source_tick) as f64 * velocity_multiplier;
    let velocity = (velocity_base.round() as i32).clamp(1, 127) as u8;
    for note_id in &chord.notes {
        let note = &score.notes[note_id.index()];
        if !note.play || note.tie_back.is_some() {
            // Note tenue par la note précédente : pas de nouvelle attaque.
            continue;
        }
        let key = note.pitch.min(127);
        let end = off + tie_length(score, staff, note);
        out.push(Event {
            tick: on.max(0),
            kind: EventKind::NoteOn {
                channel,
                key,
                velocity,
            },
        });
        out.push(Event {
            tick: end.max(on + 1),
            kind: EventKind::NoteOff { channel, key },
        });
    }
}

/// Durée ajoutée par une chaîne de liaisons de prolongation
/// (`calculateTieLength`).
fn tie_length(score: &Score, staff: &mscz2xml::model::Staff, note: &Note) -> i64 {
    let mut total = 0;
    let mut tie = note.tie_forward;
    let mut guard = 0;
    while let Some(next_id) = tie {
        if guard > 64 {
            break;
        }
        let Some(next_rest) = find_chord_rest_of_note(score, next_id) else {
            break;
        };
        let next_ticks = actual_ticks_of_chord(score, staff, next_rest);
        if next_ticks <= 0 {
            break;
        }
        // La note liée n'est pas attaquée : seule sa durée effective (réduite
        // par ses propres ornements après) prolonge la note.
        let next_trail = match &next_rest.kind {
            ChordRestKind::Chord(next_chord) => {
                trail_time(next_chord, next_rest.duration, next_ticks)
            }
            ChordRestKind::Rest(_) => 0,
        };
        total += next_ticks * (1000 - next_trail) / 1000;
        guard += 1;
        tie = score.notes[next_id.index()].tie_forward;
    }
    total
}

/// Temps de fin réservé aux notes d'ornement après l'accord, en millièmes de
/// la durée de l'accord (`createGraceNotesPlayEvents`).
fn trail_time(chord: &Chord, duration: mscz2xml::model::Duration, ticks: i64) -> i64 {
    let mut before = 0;
    let mut after = 0;
    for grace in &chord.grace_chords {
        match grace.chord.grace {
            Some(GraceType::Grace8After | GraceType::Grace16After | GraceType::Grace32After) => {
                after += 1;
            }
            _ => before += 1,
        }
    }
    let mut trail = 0;
    if before > 0
        && chord
            .grace_chords
            .first()
            .is_some_and(|grace| grace.chord.grace != Some(GraceType::Acciaccatura))
    {
        let grace_ticks = chord
            .grace_chords
            .first()
            .map_or(0, |grace| grace.duration.ticks().ticks(UNITS_PER_WHOLE));
        trail += (grace_ticks * 1000 / ticks.max(1)).min(500);
    }
    if after > 0 {
        let weight = after as f64 / (before + after) as f64;
        trail = match duration.dots {
            1 => (667.0 * weight).floor() as i64,
            2 => (571.0 * weight).floor() as i64,
            _ => (500.0 * weight).floor() as i64,
        };
    }
    trail
}

/// Instrument en vigueur au tick source (`Part::instrument(tick)`).
fn instrument_for(score: &Score, part: PartId, source_tick: i64) -> &Instrument {
    let tick = Fraction::from_ticks(source_tick, UNITS_PER_WHOLE);
    score.parts[part.index()].instrument_at(tick)
}

/// Accord ou silence qui porte une note (recherche linéaire : les chaînes de
/// liaisons sont courtes).
fn find_chord_rest_of_note(score: &Score, note: mscz2xml::model::NoteId) -> Option<&ChordRest> {
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                if let mscz2xml::model::SegmentItem::ChordRest(chord_rest) = item {
                    if let ChordRestKind::Chord(chord) = &chord_rest.kind {
                        if chord.notes.contains(&note) {
                            return Some(chord_rest);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Durée effective d'un accord isolé.
fn actual_ticks_of_chord(
    score: &Score,
    staff: &mscz2xml::model::Staff,
    chord_rest: &ChordRest,
) -> i64 {
    let mut ticks = chord_rest
        .ticks
        .unwrap_or_else(|| chord_rest.duration.ticks());
    if !staff.time_stretch.is_zero() {
        ticks /= staff.time_stretch;
    }
    let mut tuplet = chord_rest.tuplet;
    while let Some(id) = tuplet {
        let value = &score.tuplets[id.index()];
        ticks /= value.ratio;
        tuplet = value.parent;
    }
    ticks.ticks(UNITS_PER_WHOLE)
}

/// Multiplicateur de vélocité des articulations de l'accord
/// (`chordVelocityMultiplier`).
fn chord_velocity_multiplier(chord: &Chord, instrument: &Instrument) -> f64 {
    let mut multiplier = 1.0;
    for articulation in &chord.articulations {
        let Some(name) = articulation_name(&articulation.kind) else {
            continue;
        };
        multiplier *= velocity_multiplier(instrument, name);
    }
    multiplier
}

/// Durée de tenue de l'accord (`gateTime`, `updateGateTime`).
fn gate_time(
    score: &Score,
    staff: &mscz2xml::model::Staff,
    instrument: &Instrument,
    chord: &Chord,
    source_tick: i64,
) -> i64 {
    // Sous une liaison d'expression, la durée reste pleine.
    let slurred = score.spanners.iter().any(|spanner| {
        matches!(spanner.kind, SpannerType::Slur)
            && spanner.start.track.staff() == staff.id
            && segment_tick(spanner.start.tick) <= source_tick
            && source_tick < segment_tick(spanner.end.tick)
    });
    let mut gate = 100;
    if !slurred {
        if let Some(default) = instrument
            .midi_articulations
            .iter()
            .find(|articulation| articulation.name.is_empty())
        {
            gate = gate.min(i64::from(default.gate_time));
        }
    }
    for articulation in &chord.articulations {
        let Some(name) = articulation_name(&articulation.kind) else {
            continue;
        };
        if let Some(value) = instrument
            .midi_articulations
            .iter()
            .find(|articulation| articulation.name == name)
        {
            gate = gate.min(i64::from(value.gate_time));
        } else if let Some((_, _, builtin_gate)) = BUILTIN_ARTICULATIONS
            .iter()
            .find(|(builtin, _, _)| *builtin == name)
        {
            gate = gate.min(i64::from(*builtin_gate));
        }
    }
    gate
}

/// Multiplicateur de vélocité d'une articulation nommée
/// (`velocityMultiplierByInstrument`).
fn velocity_multiplier(instrument: &Instrument, name: &str) -> f64 {
    if let Some(value) = instrument
        .midi_articulations
        .iter()
        .find(|articulation| articulation.name == name)
    {
        return f64::from(value.velocity) / 100.0;
    }
    if let Some((_, multiplier, _)) = BUILTIN_ARTICULATIONS
        .iter()
        .find(|(builtin, _, _)| *builtin == name)
    {
        return *multiplier;
    }
    1.0
}

/// Nom MuseScore d'une articulation (`Articulation::articulationName`).
fn articulation_name(kind: &ArticulationType) -> Option<&'static str> {
    match kind {
        ArticulationType::Staccato => Some("staccato"),
        ArticulationType::Staccatissimo => Some("staccatissimo"),
        ArticulationType::Portato => Some("portato"),
        ArticulationType::Tenuto => Some("tenuto"),
        ArticulationType::Accent | ArticulationType::SoftAccent => Some("accent"),
        ArticulationType::Marcato => Some("marcato"),
        _ => None,
    }
}
