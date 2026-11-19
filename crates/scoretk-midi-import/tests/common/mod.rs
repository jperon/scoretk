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
use scoretk_core::model::{ChordRestKind, SegmentItem, StaffId, Track, VOICES};

/// Event of a test SMF, described in absolute ticks.
pub enum Ev {
    TrackName(&'static [u8]),
    Lyric(&'static [u8]),
    Tempo(u32),
    TimeSig(u8, u8),
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
pub fn smf(tracks: &[Vec<(u32, Ev)>]) -> Vec<u8> {
    let mut result = Smf::new(Header::new(
        Format::Parallel,
        Timing::Metrical(u15::new(480)),
    ));
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

/// Collects `(pitches, duration in ticks of 1/1920 whole)` from every chord.
pub fn chords(score: &scoretk_core::model::Score) -> Vec<(Vec<u8>, i64)> {
    let mut result = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for staff in 0..score.staves.len() {
                for voice in 0..VOICES {
                    let Some(SegmentItem::ChordRest(chord_rest)) =
                        segment.element(Track::new(StaffId::from_index(staff), voice as u8))
                    else {
                        continue;
                    };
                    let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                        continue;
                    };
                    let ticks = chord_rest
                        .ticks
                        .unwrap_or_else(|| chord_rest.duration.ticks())
                        .ticks(1920);
                    let pitches: Vec<u8> = chord
                        .notes
                        .iter()
                        .map(|note| score.notes[note.index()].pitch)
                        .collect();
                    result.push((pitches, ticks));
                }
            }
        }
    }
    result
}
