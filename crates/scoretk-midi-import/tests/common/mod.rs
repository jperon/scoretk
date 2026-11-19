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

//! Small in-memory SMF builder shared by the robustness tests. No external
//! corpus and no process launch: every test stays in the millisecond range.

use midly::num::{u15, u24, u28, u4, u7};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

/// Event of a test SMF, described in absolute ticks.
/// Some variants are only used by one of the test binaries sharing this module.
#[allow(dead_code)]
pub enum Ev {
    TrackName(&'static [u8]),
    Lyric(&'static [u8]),
    Tempo(u32),
    TimeSig(u8, u8),
    /// Armure SMF `FF 59 02 sf mi` (nombre d'altérations, drapeau mineur).
    KeySig(i8, bool),
    Program(u8, u8),
    NoteOn(u8, u8, u8),
    NoteOff(u8, u8),
}

impl Ev {
    fn kind(&self) -> TrackEventKind<'static> {
        match *self {
            Ev::TrackName(name) => TrackEventKind::Meta(MetaMessage::TrackName(name)),
            Ev::Lyric(text) => TrackEventKind::Meta(MetaMessage::Lyric(text)),
            Ev::Tempo(value) => TrackEventKind::Meta(MetaMessage::Tempo(u24::new(value))),
            Ev::TimeSig(numerator, denominator) => {
                TrackEventKind::Meta(MetaMessage::TimeSignature(numerator, denominator, 24, 8))
            }
            Ev::KeySig(sharps, minor) => {
                TrackEventKind::Meta(MetaMessage::KeySignature(sharps, minor))
            }
            Ev::Program(channel, program) => TrackEventKind::Midi {
                channel: u4::new(channel),
                message: MidiMessage::ProgramChange {
                    program: u7::new(program),
                },
            },
            Ev::NoteOn(channel, pitch, velocity) => TrackEventKind::Midi {
                channel: u4::new(channel),
                message: MidiMessage::NoteOn {
                    key: u7::new(pitch),
                    vel: u7::new(velocity),
                },
            },
            Ev::NoteOff(channel, pitch) => TrackEventKind::Midi {
                channel: u4::new(channel),
                message: MidiMessage::NoteOff {
                    key: u7::new(pitch),
                    vel: u7::new(0),
                },
            },
        }
    }
}

/// Encodes a format-1 SMF from tracks of `(absolute_tick, event)` pairs.
#[allow(dead_code)]
pub fn smf(tracks: &[Vec<(u32, Ev)>]) -> Vec<u8> {
    smf_with(Format::Parallel, Timing::Metrical(u15::new(480)), tracks)
}

/// Generalised builder: lets a test choose the SMF format and the division
/// (metrical or SMPTE) while keeping the same `(absolute_tick, event)` input.
#[allow(dead_code)]
pub fn smf_with(format: Format, timing: Timing, tracks: &[Vec<(u32, Ev)>]) -> Vec<u8> {
    let mut result = Smf::new(Header::new(format, timing));
    for events in tracks {
        let mut encoded = Vec::with_capacity(events.len() + 1);
        let mut previous = 0;
        for (tick, event) in events {
            let delta = tick.saturating_sub(previous);
            previous = *tick;
            encoded.push(TrackEvent {
                delta: u28::new(delta),
                kind: event.kind(),
            });
        }
        encoded.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        });
        result.tracks.push(encoded);
    }
    let mut bytes = Vec::new();
    result.write(&mut bytes).expect("SMF encoding");
    bytes
}
