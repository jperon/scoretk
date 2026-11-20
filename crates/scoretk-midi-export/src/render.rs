// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/compat/midi/compatmidirender.cpp`,
// `engraving/compat/midi/compatmidirenderinternal.cpp`,
// `engraving/compat/midi/velocitymap.cpp`,
// `importexport/midi/internal/midiexport/exportmidi.cpp`, `engraving/dom/note.cpp`
// (`customizeVelocity`) et `engraving/types/types.h` (`VeloType`) de MuseScore
// Studio, Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust)
// en 2026.
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

use scoretk_core::model::{
    Annotation, ArticulationType, Chord, ChordRest, ChordRestKind, Fraction, GraceType, Instrument,
    KeyMode, KeySig, LyricsSyllabic, Measure, Note, OrnamentType, OttavaType, PartId, Score,
    SegmentItem, SegmentType, SpannerType, StaffId, TimeSig, Track, TremoloKind, VeloType, VOICES,
};

use super::error::Result;
use super::harmony;
use super::ornaments;
use super::pause_map::PauseMap;
use super::repeat_list::{RepeatList, DIVISION, UNITS_PER_WHOLE};
use super::timemap::{Attack, Builder, Timemap};
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
    /// Altérations et drapeau mineur (`mi`) du méta `FF 59 02 sf mi`.
    KeySignature(i8, bool),
    /// Parole `FF 05`, avec l'espace final des syllabes terminales déjà posé.
    Lyric(String),
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

/// Résultat du rendu : la performance et sa carte des temps.
#[derive(Clone, Debug, Default)]
pub(crate) struct Rendered {
    pub performance: Performance,
    pub timemap: Timemap,
}

/// Rend la partition en évènements MIDI (`CompatMidiRender::renderScore` puis
/// `ExportMidi::writeHeader`).
///
/// `capture_timemap` faux évite de construire la carte des temps (et les
/// allocations de mesures/attaques) quand seul le MIDI est demandé.
pub(crate) fn render(
    score: &Score,
    repeat_list: &RepeatList,
    capture_timemap: bool,
) -> Result<Rendered> {
    let channels = channels(score);
    let velocities = build_velocities(score);
    let pitch_offsets = pitch_offsets(score);
    let time_sigs = time_signature_events(score);
    let key_sigs = key_signature_events(score);
    let pause_map = PauseMap::build(score, repeat_list, &time_sigs);
    let tempo = tempo_events(score, repeat_list, &pause_map);
    // La carte est remplie aux mêmes points que les évènements MIDI : les ticks
    // qu'elle contient sont exactement ceux du fichier écrit.
    let mut timemap = if capture_timemap {
        Builder::new(score, repeat_list, &pause_map, &tempo)
    } else {
        Builder::disabled(score, repeat_list, &pause_map)
    };
    let mut performance = Performance {
        tracks: vec![TrackData::default(); score.staves.len()],
    };

    for (staff_index, staff) in score.staves.iter().enumerate() {
        let part = &score.parts[staff.part.index()];
        let channel = channels[staff.part.index()];
        let is_top = part.staves.first() == Some(&staff.id);
        let offsets = &pitch_offsets[staff_index];
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
            push_unwound(
                track,
                repeat_list,
                &pause_map,
                entries,
                |tick, key: &(i8, bool)| Event {
                    tick,
                    kind: EventKind::KeySignature(key.0, key.1),
                },
            );
        }

        // Paroles de la portée (`exportmidi.cpp` : « Export lyrics … as Meta
        // events »). Le déroulement des reprises est le même que pour les notes.
        let lyrics = lyric_events(score, staff.id);
        push_unwound(
            track,
            repeat_list,
            &pause_map,
            &lyrics,
            |tick, text: &String| Event {
                tick,
                kind: EventKind::Lyric(text.clone()),
            },
        );

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

        // Accords chiffrés : `renderHarmony`
        // (`compatmidirenderinternal.cpp:1021-1078`) joue les hauteurs
        // réalisées du symbole courant jusqu'au symbole suivant. Leur
        // déroulement suit les mêmes segments que les notes.
        for (tick, duration, pitches) in harmony::events(score, staff.id) {
            for segment in &repeat_list.segments {
                let end = repeat_list.segment_end(segment);
                if tick < segment.tick || tick >= end {
                    continue;
                }
                let offset = segment.utick - segment.tick;
                let on = pause_map.tick_with_pauses(tick + offset);
                let off = pause_map.tick_with_pauses(tick + duration + offset);
                for key in &pitches {
                    track.events.push(Event {
                        tick: on,
                        kind: EventKind::NoteOn {
                            channel,
                            key: *key,
                            velocity: 80,
                        },
                    });
                    track.events.push(Event {
                        tick: off,
                        kind: EventKind::NoteOff { channel, key: *key },
                    });
                }
            }
        }

        // Note events.
        let mut notes = Vec::new();
        let mut prev_chords = [0i64; VOICES];
        for (segment_index, segment) in repeat_list.segments.iter().enumerate() {
            let offset = segment.utick - segment.tick;
            for measure_index in &segment.measures {
                // Les mesures sont communes aux portées : une seule occurrence
                // par passage, enregistrée sur la première portée. Les bornes
                // sont figées ici, au moment où la mesure est parcourue.
                if staff_index == 0 {
                    let start_utick = repeat_list.measure_start(*measure_index) + offset;
                    let end_utick = repeat_list.measure_end(*measure_index) + offset;
                    let tick = pause_map.tick_with_pauses(start_utick);
                    // Variante non inclusive : la pause qui ouvre la mesure
                    // suivante n'est pas comptée dans la fin de celle-ci.
                    let end_tick = pause_map.tick_with_pauses_before(end_utick);
                    let pause_before = pause_map
                        .spans()
                        .iter()
                        .any(|span| span.base_utick == start_utick);
                    timemap.measure(
                        tick,
                        end_tick,
                        *measure_index,
                        segment_index,
                        segment.playback_count,
                        pause_before,
                    );
                }
                let measure = &score.measures[*measure_index];
                render_measure(
                    score,
                    measure,
                    *measure_index,
                    segment.playback_count,
                    staff,
                    channel,
                    offset,
                    &pause_map,
                    &velocities,
                    offsets,
                    &mut notes,
                    &mut prev_chords,
                    &mut timemap,
                );
            }
        }
        track.events.extend(notes);
    }

    Ok(Rendered {
        performance,
        timemap: timemap.finish(),
    })
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
                if let scoretk_core::model::SegmentItem::TimeSig(time_sig) = item {
                    events.insert(segment_tick(segment.tick), time_sig.clone());
                    break;
                }
            }
        }
    }
    events.into_iter().collect()
}

/// Paroles d'une portée, prêtes à l'écriture (`tick, texte`).
fn lyric_events(score: &Score, staff: StaffId) -> Vec<(i64, String)> {
    let mut result = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (track, item) in segment.iter_elements() {
                if track.staff() != staff {
                    continue;
                }
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                for lyric in &chord_rest.lyrics {
                    // MuseScore ajoute un espace final aux syllabes `Single`
                    // et `End` (fin de mot), pas à `Begin`/`Middle`.
                    let mut text = lyric.text.clone();
                    if matches!(lyric.syllabic, LyricsSyllabic::Single | LyricsSyllabic::End)
                        && !text.ends_with(' ')
                    {
                        text.push(' ');
                    }
                    result.push((segment_tick(segment.tick), text));
                }
            }
        }
    }
    result
}

/// Armures d'une portée : `(tick, (altérations, mineur))`.
type KeySigEntries = Vec<(i64, (i8, bool))>;

/// Armures de chaque portée, dédoublonnées par tick.
fn key_signature_events(score: &Score) -> Vec<Option<KeySigEntries>> {
    let mut result = Vec::with_capacity(score.staves.len());
    for staff in &score.staves {
        let mut events: BTreeMap<i64, (i8, bool)> = BTreeMap::new();
        for measure in &score.measures {
            for segment in &measure.segments {
                if !segment.kind.is_key_sig_type() {
                    continue;
                }
                for (track, item) in segment.iter_elements() {
                    if track.staff() != staff.id {
                        continue;
                    }
                    if let scoretk_core::model::SegmentItem::KeySig(KeySig {
                        concert_key,
                        mode,
                        ..
                    }) = item
                    {
                        events.insert(
                            segment_tick(segment.tick),
                            (concert_key.fifths(), *mode == KeyMode::Minor),
                        );
                    }
                }
            }
        }
        let mut entries: KeySigEntries = events.into_iter().collect();
        if entries.first().is_none_or(|(tick, _)| *tick != 0) {
            // `writeHeader` écrit une armure de do majeur quand le fichier n'en
            // donne aucune au début.
            entries.insert(0, (0, (0, false)));
        }
        result.push(Some(entries));
    }
    result
}

/// Tick d'un segment en ticks MuseScore.
fn segment_tick(tick: Fraction) -> i64 {
    tick.ticks(UNITS_PER_WHOLE)
}

/// Décalage de hauteur jouée d'une ottava, en demi-tons
/// (`Ottava::pitchShift`, `engraving/dom/ottava.h:67-74`).
fn ottava_shift(kind: OttavaType) -> i32 {
    match kind {
        OttavaType::OctaveUp => 12,
        OttavaType::OctaveDown => -12,
        OttavaType::FifteenthUp => 24,
        OttavaType::FifteenthDown => -24,
    }
}

/// Carte des décalages de hauteur par portée
/// (`Staff::updateOttava`, `engraving/dom/staff.cpp:1279-1301`).
///
/// La norme SMF n'a pas d'ottava : c'est un réglage de rendu de MuseScore.
/// Comme dans le C++, chaque ottava inscrit son `pitchShift` au tick de début
/// et `0` au tick de fin ; `insert` reproduit `insert_or_assign` en écrasant
/// une valeur déjà présente au même tick (la dernière ottava gagne).
fn pitch_offsets(score: &Score) -> Vec<BTreeMap<i64, i32>> {
    let mut result = vec![BTreeMap::new(); score.staves.len()];
    for spanner in &score.spanners {
        let SpannerType::Ottava(kind) = &spanner.kind else {
            continue;
        };
        // `Staff::updateOttava` (`staff.cpp:1285`) ne retient que les
        // `playSpanner()` : l'ottava LilyPond n'est qu'une convention d'écriture.
        if !spanner.play {
            continue;
        }
        let Some(map) = result.get_mut(spanner.start.track.staff().index()) else {
            continue;
        };
        map.insert(segment_tick(spanner.start.tick), ottava_shift(*kind));
        map.insert(segment_tick(spanner.end.tick), 0);
    }
    result
}

/// Dernier décalage inscrit à un tick ≤ `tick` (`Staff::pitchOffset`,
/// `engraving/dom/staff.cpp:1293-1301` : `findLessOrEqual`).
fn pitch_offset(map: &BTreeMap<i64, i32>, tick: i64) -> i32 {
    map.range(..=tick)
        .next_back()
        .map_or(0, |(_, shift)| *shift)
}

/// Temps en battements par seconde au tick source (annotations `<Tempo>`).
/// Sert à borner la fréquence du trille comme `multipliedTempo` dans
/// `renderNoteArticulation` (`compatmidirender.cpp:705-720`).
fn beats_per_second_at(score: &Score, tick: i64) -> f64 {
    let mut tempo = 2.0;
    for measure in &score.measures {
        for segment in &measure.segments {
            if segment.tick.ticks(UNITS_PER_WHOLE) > tick {
                continue;
            }
            for entry in &segment.annotations {
                if let Annotation::Tempo(value) = &entry.annotation {
                    if let Some(beats) = value.beats_per_second {
                        tempo = beats;
                    }
                }
            }
        }
    }
    tempo
}

/// Tempo initial et changements, en microsecondes par noire.
///
/// Les changements sont déroulés comme les autres évènements, puis les pauses
/// de section ajoutent leur tempo et le retour au tempo en vigueur.
fn tempo_events(
    _score: &Score,
    _repeat_list: &RepeatList,
    pause_map: &PauseMap,
) -> Vec<(i64, u32)> {
    // `PauseMap::calculate` (`pausemap.cpp:47-76`) tient **tous** les
    // changements de tempo dans `m_tempoEvents`, y compris les indications
    // écrites et les pauses, aux ticks après insertion. On ne reconstruit donc
    // rien ici : la chronologie déroulée et fusionnée est la seule source.
    let to_micros = |beats_per_second: f64| {
        let micros = (1.0 / beats_per_second * 1_000_000.0).round();
        micros.clamp(1.0, 0x00ff_ffff as f64) as u32
    };
    pause_map
        .tempo_events
        .iter()
        .map(|(utick, beats_per_second)| (*utick, to_micros(*beats_per_second)))
        .collect()
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
            scoretk_core::model::HairpinType::Crescendo => ChangeDirection::Increasing,
            scoretk_core::model::HairpinType::Diminuendo => ChangeDirection::Decreasing,
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
    measure_index: usize,
    pass: i32,
    staff: &scoretk_core::model::Staff,
    channel: u8,
    offset: i64,
    pause_map: &PauseMap,
    velocities: &VelocityMaps,
    offsets: &BTreeMap<i64, i32>,
    out: &mut Vec<Event>,
    prev_chords: &mut [i64; VOICES],
    timemap: &mut Builder<'_>,
) {
    // Rang du `ChordRest` dans la voix et la mesure, silences compris : il
    // identifie l'élément du modèle, pas l'attaque.
    let mut ranks = [0usize; VOICES];
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
            let chord_rank = ranks[voice];
            ranks[voice] += 1;
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
                        let next = measure.segments[segment_index + 1..]
                            .iter()
                            .enumerate()
                            .find_map(|(offset, segment)| {
                                segment.chord_rest(track).and_then(|chord_rest| {
                                    matches!(chord_rest.kind, ChordRestKind::Chord(_))
                                        .then_some((segment_index + 1 + offset, chord_rest))
                                })
                            });
                        if let Some((next_segment, next)) = next {
                            let next_ticks = actual_ticks(score, staff, next, measure);
                            // Rang du second accord : les `ChordRest` présents
                            // pour cette voix entre les deux segments.
                            let second_rank = ranks[voice]
                                + measure.segments[segment_index + 1..next_segment]
                                    .iter()
                                    .filter(|segment| segment.chord_rest(track).is_some())
                                    .count();
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
                                offsets,
                                out,
                                &mut AttackContext {
                                    measure: measure_index,
                                    staff: staff.id.raw(),
                                    voice: voice as u8,
                                    chord: chord_rank,
                                    second_chord: second_rank,
                                    pass,
                                    timemap,
                                },
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
                offsets,
                out,
                &mut AttackContext {
                    measure: measure_index,
                    staff: staff.id.raw(),
                    voice: voice as u8,
                    chord: chord_rank,
                    second_chord: chord_rank,
                    pass,
                    timemap,
                },
            );
            *previous = ticks;
        }
    }
}

/// Contexte d'enregistrement des attaques dans la carte des temps.
struct AttackContext<'a, 'b> {
    measure: usize,
    staff: u32,
    voice: u8,
    /// Rang du `ChordRest` principal.
    chord: usize,
    /// Rang du second accord d'un trémolo à deux notes.
    second_chord: usize,
    pass: i32,
    timemap: &'a mut Builder<'b>,
}

/// Note visée par une attaque.
#[derive(Clone, Copy, Debug)]
struct AttackNote {
    chord: usize,
    note: usize,
    pitch: u8,
    grace: bool,
    /// Position dans `Chord::grace_chords` (`None` hors ornement).
    grace_index: Option<usize>,
    /// Vrai quand l'ornement précède l'accord principal.
    grace_before: bool,
    tremolo: bool,
}

impl AttackContext<'_, '_> {
    fn attack(&mut self, tick: i64, duration_ticks: i64, note: AttackNote) {
        self.timemap.attack(Attack {
            tick,
            duration_ticks,
            measure: self.measure,
            staff: self.staff,
            voice: self.voice,
            chord: note.chord,
            note: note.note,
            pitch: note.pitch,
            pass: self.pass,
            grace: note.grace,
            grace_index: note.grace_index,
            grace_before: note.grace_before,
            tremolo: note.tremolo,
        });
    }
}

/// Rend un trémolo à deux accords (`renderTremolo`, `TremoloFirstChord`).
#[allow(clippy::too_many_arguments)]
fn render_two_chord_tremolo(
    score: &Score,
    staff: &scoretk_core::model::Staff,
    first: &Chord,
    duration: scoretk_core::model::Duration,
    second: &ChordRest,
    second_ticks: i64,
    utick: i64,
    ticks: i64,
    channel: u8,
    track: u32,
    source_tick: i64,
    velocities: &VelocityMaps,
    offsets: &BTreeMap<i64, i32>,
    out: &mut Vec<Event>,
    context: &mut AttackContext<'_, '_>,
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
    let velocity_base = (velocities.val(track, source_tick) as f64 * chord_velocity).round() as i32;
    // `playNote` applique `customizeVelocity` par note, après le clamp de la
    // vélocité calculée (`compatmidirenderinternal.cpp:227-229`). Le trémolo à
    // deux accords n'attaque que la première note de chaque accord : c'est
    // donc sa vélocité utilisateur qui s'applique.
    let velocity = match first.notes.first() {
        Some(id) => customize_velocity(velocity_base.clamp(1, 127), &score.notes[id.index()]),
        None => velocity_base.clamp(1, 127),
    } as u8;

    for index in 0..events {
        let ontime = length * index * 2;
        let second_ontime = ontime + length;
        let first_key = first
            .notes
            .first()
            .map(|id| played_pitch(score, &score.notes[id.index()], source_tick, offsets));
        let second_key = second_chord
            .notes
            .first()
            .map(|id| played_pitch(score, &score.notes[id.index()], source_tick, offsets));
        let Some(first_key) = first_key else {
            continue;
        };
        let key = first_key.min(127);
        let first_on = utick + ticks * ontime / 1000;
        let first_off = first_on + ticks * (length * gate / 100) / 1000 - 1;
        out.push(Event {
            tick: first_on,
            kind: EventKind::NoteOn {
                channel,
                key,
                velocity,
            },
        });
        context.attack(
            first_on,
            first_off - first_on,
            AttackNote {
                chord: context.chord,
                note: 0,
                pitch: key,
                grace: false,
                grace_index: None,
                grace_before: false,
                tremolo: true,
            },
        );
        out.push(Event {
            tick: first_off,
            kind: EventKind::NoteOff { channel, key },
        });
        if let Some(second_key) = second_key {
            let key = second_key.min(127);
            let start = utick + ticks * second_ontime / 1000;
            let end = start + ticks * (length * gate / 100) / 1000 - 1;
            out.push(Event {
                tick: start,
                kind: EventKind::NoteOn {
                    channel,
                    key,
                    velocity,
                },
            });
            context.attack(
                start,
                end - start,
                AttackNote {
                    chord: context.second_chord,
                    note: 0,
                    pitch: key,
                    grace: false,
                    grace_index: None,
                    grace_before: false,
                    tremolo: true,
                },
            );
            out.push(Event {
                tick: end,
                kind: EventKind::NoteOff { channel, key },
            });
        }
    }
}

/// Durée effective d'un accord ou silence en ticks MuseScore
/// (`DurationElement::actualTicks`).
fn actual_ticks(
    score: &Score,
    staff: &scoretk_core::model::Staff,
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

/// Hauteur MIDI effectivement jouée d'une note du modèle.
///
/// `Note::ppitch` (`engraving/dom/note.cpp:2668-2690`) ajoute à la hauteur
/// écrite le décalage d'ottava de la portée au tick du segment. La norme SMF
/// (§Channel Voice) ne définit pas l'ottava : ce décalage est propre au rendu
/// de MuseScore, porté ici par [`pitch_offset`]. Le tick utilisé est le tick
/// source du segment, pas le tick d'exécution `utick` : dans le C++, la note
/// provient du segment original, même quand les reprises sont déroulées.
///
/// La transposition de lecture trouvée dans un second `\score` LilyPond
/// (`\transpose` sur la partition MIDI, Notation Reference §1.2.2/§24.3) ne
/// s'applique qu'ici, aux numéros de note des messages Note On/Off (MIDI 1.0
/// §Channel Voice). Le méta `FF 59` (`Key Signature`, SMF §Meta-Events) reste
/// celui de la partition gravée : c'est une donnée d'affichage/analyse, elle ne
/// change aucun numéro de note, et les autres écrivains gardent l'armure écrite.
fn played_pitch(score: &Score, note: &Note, source_tick: i64, offsets: &BTreeMap<i64, i32>) -> u8 {
    let shift = score
        .playback_transposition
        .map_or(0, |transposition| transposition.chromatic);
    (i32::from(note.pitch) + shift + pitch_offset(offsets, source_tick)).clamp(0, 127) as u8
}

/// Rend un accord : notes d'ornement avant, note principale, ornements après
/// (`collectGraceBeforeChordEvents`, `collectNote`, `collectGraceNotesAfter`).
#[allow(clippy::too_many_arguments)]
fn render_chord(
    score: &Score,
    staff: &scoretk_core::model::Staff,
    chord: &Chord,
    duration: scoretk_core::model::Duration,
    utick: i64,
    ticks: i64,
    previous_chord_ticks: i64,
    channel: u8,
    track: u32,
    source_tick: i64,
    velocities: &VelocityMaps,
    offsets: &BTreeMap<i64, i32>,
    out: &mut Vec<Event>,
    context: &mut AttackContext<'_, '_>,
) {
    let instrument = instrument_for(score, staff.part, source_tick);
    let chord_velocity = chord_velocity_multiplier(chord, instrument);
    let gate = gate_time(score, staff, instrument, chord, source_tick);

    // Indices dans `Chord::grace_chords` : l'identifiant MusicXML d'un
    // ornement dépend de cette position, pas de son rang dans le groupe.
    let mut before: Vec<usize> = Vec::new();
    let mut after: Vec<usize> = Vec::new();
    for (index, grace) in chord.grace_chords.iter().enumerate() {
        match grace.chord.grace {
            Some(GraceType::Grace8After | GraceType::Grace16After | GraceType::Grace32After) => {
                after.push(index)
            }
            _ => before.push(index),
        }
    }

    let mut ontime = 0i64;
    let mut trailtime = 0i64;

    let acciaccatura = before.first().is_some_and(|index| {
        chord.grace_chords[*index].chord.grace == Some(GraceType::Acciaccatura)
    });
    if acciaccatura {
        // `collectGraceBeforeChordEvents` : les acciaccaturas occupent au plus
        // la moitié de l'accord précédent, avant le temps.
        let grace_ticks = before.first().map_or(0, |index| {
            chord.grace_chords[*index]
                .duration
                .ticks()
                .ticks(UNITS_PER_WHOLE)
        });
        let mut sum = grace_ticks;
        if previous_chord_ticks > 0 {
            sum = sum.min(previous_chord_ticks / 2);
        }
        let offset = sum / before.len().max(1) as i64;
        for (position, index) in before.iter().enumerate() {
            let offset_on = sum - offset * position as i64;
            let offset_off = sum - offset * (position as i64 + 1);
            let on = utick - offset_on;
            let off = utick - 1 - offset_off;
            render_grace_notes(
                score,
                staff,
                &chord.grace_chords[*index].chord,
                *index,
                true,
                on,
                off,
                channel,
                track,
                &chord_velocity,
                velocities,
                source_tick,
                offsets,
                out,
                context,
            );
        }
    } else if !before.is_empty() {
        // Appoggiatures : au plus la moitié de l'accord principal, les unes
        // après les autres en prenant le temps de l'accord.
        let grace_ticks = before.first().map_or(0, |index| {
            chord.grace_chords[*index]
                .duration
                .ticks()
                .ticks(UNITS_PER_WHOLE)
        });
        ontime = (grace_ticks * 1000 / ticks.max(1)).min(500);
        trailtime += ontime;
        let each = ontime / before.len().max(1) as i64;
        for (position, index) in before.iter().enumerate() {
            let on = utick + ticks * each * position as i64 / 1000;
            let off = on + ticks * each / 1000 - 1;
            render_grace_notes(
                score,
                staff,
                &chord.grace_chords[*index].chord,
                *index,
                true,
                on,
                off,
                channel,
                track,
                &chord_velocity,
                velocities,
                source_tick,
                offsets,
                out,
                context,
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
        for index in &after {
            let start = utick + ticks * on / 1000;
            let off = start + ticks * each / 1000 - 1;
            render_grace_notes(
                score,
                staff,
                &chord.grace_chords[*index].chord,
                *index,
                false,
                start,
                off,
                channel,
                track,
                &chord_velocity,
                velocities,
                source_tick,
                offsets,
                out,
                context,
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

    for (note_index, note) in chord.notes.iter().enumerate() {
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
        // Le C++ clampe la vélocité calculée avant de la personnaliser par
        // note (`compatmidirenderinternal.cpp:887-893`, puis `playNote`).
        let velocity = customize_velocity((velocity.round() as i32).clamp(1, 127), note) as u8;
        let key = played_pitch(score, note, source_tick, offsets);
        // Trille : `renderNoteArticulation` (`compatmidirender.cpp:668-810`)
        // remplace la note tenue par l'excitation de l'ornement. Les grâces
        // attachées à un trille sont déjà fusionnées dans l'ornement
        // (`graceNotesMerged`, `compatmidirenderinternal.cpp:1736-1754`).
        if chord
            .ornaments
            .iter()
            .any(|ornament| ornament.kind == OrnamentType::TrillMark)
            && chord.grace_chords.is_empty()
            && tremolo.is_none()
        {
            for (offset, trill_key, length) in
                ornaments::trill(note, ticks, beats_per_second_at(score, source_tick))
            {
                let start = on + offset;
                let end = start + length.max(1);
                out.push(Event {
                    tick: start,
                    kind: EventKind::NoteOn {
                        channel,
                        key: trill_key,
                        velocity,
                    },
                });
                context.attack(
                    start,
                    end - start,
                    AttackNote {
                        chord: context.chord,
                        note: note_index,
                        pitch: trill_key,
                        grace: true,
                        grace_index: None,
                        grace_before: true,
                        tremolo: false,
                    },
                );
                out.push(Event {
                    tick: end,
                    kind: EventKind::NoteOff {
                        channel,
                        key: trill_key,
                    },
                });
            }
            continue;
        }
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
                let end = end.max(start + 1);
                context.attack(
                    start,
                    end - start,
                    AttackNote {
                        chord: context.chord,
                        note: note_index,
                        pitch: key,
                        grace: false,
                        grace_index: None,
                        grace_before: false,
                        tremolo: true,
                    },
                );
                out.push(Event {
                    tick: end,
                    kind: EventKind::NoteOff { channel, key },
                });
            }
            continue;
        }
        let mut off = on + ticks * length / 1000 - 1;
        // Prolongation des liaisons : chaque note liée ajoute sa durée.
        off += tie_length(score, staff, note, instrument, source_tick);
        out.push(Event {
            tick: on,
            kind: EventKind::NoteOn {
                channel,
                key,
                velocity,
            },
        });
        let end = off.max(on + 1);
        context.attack(
            on,
            end - on,
            AttackNote {
                chord: context.chord,
                note: note_index,
                pitch: key,
                grace: false,
                grace_index: None,
                grace_before: false,
                tremolo: false,
            },
        );
        out.push(Event {
            tick: end,
            kind: EventKind::NoteOff { channel, key },
        });
    }
}

/// Notes d'un accord d'ornement, rendues telles quelles.
#[allow(clippy::too_many_arguments)]
fn render_grace_notes(
    score: &Score,
    staff: &scoretk_core::model::Staff,
    chord: &Chord,
    grace_index: usize,
    grace_before: bool,
    on: i64,
    off: i64,
    channel: u8,
    track: u32,
    velocity_multiplier: &f64,
    velocities: &VelocityMaps,
    source_tick: i64,
    offsets: &BTreeMap<i64, i32>,
    out: &mut Vec<Event>,
    context: &mut AttackContext<'_, '_>,
) {
    let velocity_base =
        (velocities.val(track, source_tick) as f64 * velocity_multiplier).round() as i32;
    for (note_index, note_id) in chord.notes.iter().enumerate() {
        let note = &score.notes[note_id.index()];
        if !note.play || note.tie_back.is_some() {
            // Note tenue par la note précédente : pas de nouvelle attaque.
            continue;
        }
        // La personnalisation est par note, comme `playNote` dans le C++.
        let velocity = customize_velocity(velocity_base.clamp(1, 127), note) as u8;
        let key = played_pitch(score, note, source_tick, offsets);
        let instrument = instrument_for(score, staff.part, source_tick);
        let end = off + tie_length(score, staff, note, instrument, source_tick);
        let start = on.max(0);
        // Un ornement placé avant le tout début peut avoir `on < 0` : la fin
        // reste alors strictement après le début effectif.
        let end_tick = end.max(start + 1);
        out.push(Event {
            tick: start,
            kind: EventKind::NoteOn {
                channel,
                key,
                velocity,
            },
        });
        context.attack(
            start,
            end_tick - start,
            AttackNote {
                chord: context.chord,
                note: note_index,
                pitch: key,
                grace: true,
                grace_index: Some(grace_index),
                grace_before,
                tremolo: false,
            },
        );
        out.push(Event {
            tick: end_tick,
            kind: EventKind::NoteOff { channel, key },
        });
    }
}

/// Durée ajoutée par une chaîne de liaisons de prolongation
/// (`calculateTieLength`, `compatmidirenderinternal.cpp:771-786`).
///
/// Le C++ lit `nel[0].len()` de chaque note liée : cet évènement de jeu a déjà
/// été multiplié par le `gateTime` de l'instrument dans `renderChord`
/// (`compatmidirender.cpp:263`, `e.setLen(e.len() * gateTime / 100)`). La durée
/// d'une liaison doit donc porter le même facteur que la note elle-même, sans
/// quoi un instrument à tenue courte (flûte à 95 %) joue ses notes liées trop
/// longuement.
fn tie_length(
    score: &Score,
    staff: &scoretk_core::model::Staff,
    note: &Note,
    instrument: &Instrument,
    source_tick: i64,
) -> i64 {
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
        // La note liée n'est pas attaquée : seule la durée de son évènement de
        // jeu (ornements après et `gateTime` déjà appliqués) prolonge la note.
        let (next_trail, next_gate) = match &next_rest.kind {
            ChordRestKind::Chord(next_chord) => (
                trail_time(next_chord, next_rest.duration, next_ticks),
                gate_time(score, staff, instrument, next_chord, source_tick),
            ),
            ChordRestKind::Rest(_) => (0, 100),
        };
        let next_len = (1000 - next_trail) * next_gate / 100;
        total += next_len * next_ticks / 1000;
        guard += 1;
        tie = score.notes[next_id.index()].tie_forward;
    }
    total
}

/// Temps de fin réservé aux notes d'ornement après l'accord, en millièmes de
/// la durée de l'accord (`createGraceNotesPlayEvents`).
fn trail_time(chord: &Chord, duration: scoretk_core::model::Duration, ticks: i64) -> i64 {
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
fn find_chord_rest_of_note(score: &Score, note: scoretk_core::model::NoteId) -> Option<&ChordRest> {
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                if let scoretk_core::model::SegmentItem::ChordRest(chord_rest) = item {
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
    staff: &scoretk_core::model::Staff,
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
    staff: &scoretk_core::model::Staff,
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

/// Vélocité effective d'une note (`Note::customizeVelocity`).
///
/// Portage de `engraving/dom/note.cpp:2753-2760` : en mode `OFFSET_VAL`,
/// `velo = velo + (velo * userVelocity()) / 100` ; en mode `USER_VAL`,
/// `velo = userVelocity()` ; puis `clamp(velo, 1, 127)`. La division entière
/// de Rust tronque vers zéro comme celle du C++.
///
/// Appelée par note avec la vélocité déjà calculée (nuances, soufflets,
/// articulations, note fantôme) et déjà clampée, exactement comme
/// `playNote` (`compatmidirenderinternal.cpp:227-229`) reçoit `params.velo`.
fn customize_velocity(velocity: i32, note: &Note) -> i32 {
    let Some(user_velocity) = note.user_velocity else {
        return velocity.clamp(1, 127);
    };
    // `playNote` ne personnalise que si `userVelocity() != 0`
    // (`compatmidirenderinternal.cpp:227`).
    if user_velocity == 0 {
        return velocity.clamp(1, 127);
    }
    // Calcul en `i64` : une valeur `<velocity>` extrême ne doit pas déborder
    // en `i32` avant le clamp final.
    let customized: i64 = match note.velo_type.unwrap_or(VeloType::UserVal) {
        VeloType::OffsetVal => {
            i64::from(velocity) + (i64::from(velocity) * i64::from(user_velocity)) / 100
        }
        VeloType::UserVal => i64::from(user_velocity),
    };
    customized.clamp(1, 127) as i32
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
