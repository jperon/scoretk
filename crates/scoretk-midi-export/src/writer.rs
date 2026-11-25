// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/midi/internal/midiexport/exportmidi.cpp` et du
// graveur `MidiFile` de MuseScore Studio, Copyright (C) MuseScore Limited and
// others, modifié (réécrit en Rust) en 2026.
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

//! Assemblage du Standard MIDI File format 1.
//!
//! Le format de fichier est confié à `midly` (`no_std`, `alloc`) : on lui donne
//! les pistes d'évènements datés produits par [`super::render`], il encode les
//! deltas en temps variable et les morceaux `MTrk`.

use midly::num::{u15, u24, u28, u4, u7};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

use super::error::{Error, Result};
use super::render::{Event, EventKind, Performance};
use super::repeat_list::DIVISION;

/// Écrit un fichier MIDI format 1 (une piste par portée).
pub(crate) fn write_smf(performance: &Performance) -> Result<Vec<u8>> {
    let header = Header::new(
        Format::Parallel,
        Timing::Metrical(u15::new(DIVISION as u16)),
    );
    // Les évènements sont clonés et triés avant l'encodage : `TrackEvent`
    // emprunte les noms de piste, donc les clones doivent vivre aussi
    // longtemps que le fichier.
    let sorted: Vec<Vec<Event>> = performance
        .tracks
        .iter()
        .map(|track| {
            let mut events = track.events.clone();
            // Ordre d'écriture : méta et contrôleurs avant les notes à tick
            // égal, et parmi les notes, les fins avant les attaques.
            events.sort_by_key(|event| (event.tick, priority(&event.kind)));
            events
        })
        .collect();
    let mut tracks: Vec<Vec<TrackEvent<'_>>> = Vec::with_capacity(sorted.len());
    for events in &sorted {
        let mut encoded = Vec::with_capacity(events.len() + 1);
        let mut previous = 0i64;
        for event in events {
            let delta = (event.tick - previous).max(0);
            let delta = u32::try_from(delta).map_err(|_| Error::TickOverflow(event.tick))?;
            if delta > 0x0fff_ffff {
                return Err(Error::TickOverflow(event.tick));
            }
            previous = event.tick;
            encoded.push(TrackEvent {
                delta: u28::new(delta),
                kind: kind(event)?,
            });
        }
        // MuseScore termine la piste un tick après le dernier évènement.
        encoded.push(TrackEvent {
            delta: u28::new(1),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        });
        tracks.push(encoded);
    }

    let mut smf = Smf::new(header);
    smf.tracks = tracks;
    let mut bytes = Vec::new();
    smf.write(&mut bytes).map_err(Error::Write)?;
    Ok(bytes)
}

/// Priorité d'écriture à tick égal.
fn priority(kind: &EventKind) -> u8 {
    match kind {
        EventKind::NoteOff { .. } => 1,
        EventKind::NoteOn { .. } => 2,
        // L'extinction de restrike est insérée après les attaques du même
        // tick dans le multimap du C++ (`exportmidi.cpp:304`).
        EventKind::NoteOffRestrike { .. } => 3,
        _ => 0,
    }
}

/// Convertit un évènement du rendu en évènement `midly`.
fn kind(event: &Event) -> Result<TrackEventKind<'_>> {
    let value = |value: u8| u7::new(value.min(127));
    let channel = |channel: u8| u4::new(channel.min(15));
    Ok(match &event.kind {
        EventKind::TrackName(name) => TrackEventKind::Meta(MetaMessage::TrackName(name.as_bytes())),
        EventKind::TimeSignature(numerator, denominator, clocks, thirty_seconds) => {
            TrackEventKind::Meta(MetaMessage::TimeSignature(
                *numerator,
                *denominator,
                *clocks,
                *thirty_seconds,
            ))
        }
        EventKind::KeySignature(sharps, minor) => {
            TrackEventKind::Meta(MetaMessage::KeySignature(*sharps, *minor))
        }
        EventKind::Lyric(text) => TrackEventKind::Meta(MetaMessage::Lyric(text.as_bytes())),
        EventKind::Text(text) => TrackEventKind::Meta(MetaMessage::Text(text.as_bytes())),
        EventKind::Copyright(text) => TrackEventKind::Meta(MetaMessage::Copyright(text.as_bytes())),
        EventKind::Marker(text) => TrackEventKind::Meta(MetaMessage::Marker(text.as_bytes())),
        EventKind::Tempo(microseconds) => TrackEventKind::Meta(MetaMessage::Tempo(u24::new(
            (*microseconds).min(0x00ff_ffff),
        ))),
        EventKind::ControlChange {
            channel: event_channel,
            controller,
            value: controller_value,
        } => TrackEventKind::Midi {
            channel: channel(*event_channel),
            message: MidiMessage::Controller {
                controller: value(*controller),
                value: value(*controller_value),
            },
        },
        EventKind::ProgramChange {
            channel: event_channel,
            program,
        } => TrackEventKind::Midi {
            channel: channel(*event_channel),
            message: MidiMessage::ProgramChange {
                program: value(*program),
            },
        },
        EventKind::NoteOn {
            channel: event_channel,
            key,
            velocity,
        } => TrackEventKind::Midi {
            channel: channel(*event_channel),
            message: MidiMessage::NoteOn {
                key: value(*key),
                vel: value(*velocity),
            },
        },
        EventKind::NoteOff {
            channel: event_channel,
            key,
        }
        | EventKind::NoteOffRestrike {
            channel: event_channel,
            key,
        } => TrackEventKind::Midi {
            channel: channel(*event_channel),
            // MuseScore éteint les notes par un NoteOn de vélocité nulle.
            message: MidiMessage::NoteOn {
                key: value(*key),
                vel: u7::new(0),
            },
        },
    })
}
