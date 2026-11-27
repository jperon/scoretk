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

//! Canonical Note On / Note Off pairs for `MidiOptions::full_support`.
//!
//! `[MIDI §Channel Voice; Table II]` carries a note as a Note On plus a Note
//! Off (or a velocity-0 Note On). With one note per channel and pitch, MuseScore
//! 4.7.4's `fixupMIDI` writes a velocity-0 Note On ("restrike") at the next
//! attack and hoists the surviving note-off to the later attack's track. That
//! leaves two legal pairings for the same bytes: the first attack can be read
//! as a one-tick grace or as a note ending one tick before the restrike.
//!
//! Full support writes the **canonical** form instead: one Note On per
//! `(channel, pitch, tick)`, a real Note Off at the played end minus one tick,
//! and no velocity-0 Note On. All three are standard SMF messages; the option
//! is additive and off by default, so the 4.7.4 byte references are untouched.
//!
//! Pairing follows the same FIFO rule as a synthesiser (and as the independent
//! SMF oracle in `crates/scoretk/tests/support/midi_smf.rs`), so the events
//! that remain in the file are exactly the attacks the importer reconstructs.

use std::collections::{BTreeMap, BTreeSet};

use super::render::{Event, EventKind, Performance};

#[derive(Clone, Copy, Debug)]
struct NoteOn {
    tick: i64,
    channel: u8,
    key: u8,
    velocity: u8,
}

fn close(
    open: &mut BTreeMap<(u8, u8), Vec<NoteOn>>,
    channel: u8,
    key: u8,
    off: i64,
    finished: &mut Vec<(NoteOn, i64)>,
) {
    if let Some(notes) = open.get_mut(&(channel, key)) {
        if !notes.is_empty() {
            let note = notes.remove(0);
            finished.push((note, off));
        }
    }
}

/// Rewrites the note events of every track in canonical Note On/Off pairs.
pub(crate) fn canonicalize_notes(performance: &mut Performance) {
    for track in &mut performance.tracks {
        let mut events = std::mem::take(&mut track.events);
        // `fixupMIDI` appends restrikes after the ordered stream: restore the
        // chronology first, keeping the collection order at equal ticks.
        events.sort_by_key(|event| event.tick);
        let last_tick = events.last().map_or(0, |event| event.tick);

        let mut kept: Vec<Event> = Vec::with_capacity(events.len());
        let mut open: BTreeMap<(u8, u8), Vec<NoteOn>> = BTreeMap::new();
        let mut finished: Vec<(NoteOn, i64)> = Vec::new();

        for event in &events {
            match event.kind {
                EventKind::NoteOn {
                    channel,
                    key,
                    velocity,
                } if velocity > 0 => {
                    open.entry((channel, key)).or_default().push(NoteOn {
                        tick: event.tick,
                        channel,
                        key,
                        velocity,
                    });
                }
                EventKind::NoteOn { channel, key, .. } => {
                    // Velocity zero is a note-off one tick before the new
                    // attack, so the two notes do not overlap.
                    close(
                        &mut open,
                        channel,
                        key,
                        (event.tick - 1).max(0),
                        &mut finished,
                    );
                }
                EventKind::NoteOff { channel, key }
                | EventKind::NoteOffRestrike { channel, key } => {
                    close(&mut open, channel, key, event.tick, &mut finished);
                }
                _ => kept.push(event.clone()),
            }
        }
        for notes in open.values_mut() {
            for note in notes.drain(..) {
                finished.push((note, last_tick.max(note.tick + 1)));
            }
        }

        // A note-off at the exact tick of the next same-pitch attack overlaps
        // that attack: write it one tick earlier, as `fixupMIDI`'s restrike
        // does. A reader with one voice per channel cannot keep both events at
        // the same tick, and `[MIDI §Channel Voice]` gives no priority rule.
        let onsets: BTreeSet<(i64, u8, u8)> = finished
            .iter()
            .filter(|(note, off)| *off > note.tick)
            .map(|(note, _)| (note.tick, note.channel, note.key))
            .collect();
        for (note, off) in &mut finished {
            if onsets.contains(&(*off, note.channel, note.key)) {
                *off -= 1;
            }
        }
        // At most one attack per (tick, channel, pitch): when a restrike
        // duplicates an attack, the longest played duration wins.
        finished
            .sort_by_key(|(note, off)| (note.tick, note.channel, note.key, -*off, note.velocity));
        let mut written: BTreeMap<(i64, u8, u8), usize> = BTreeMap::new();
        for (note, off) in finished {
            let key = (note.tick, note.channel, note.key);
            if off <= note.tick || written.contains_key(&key) {
                continue;
            }
            written.insert(key, off as usize);
            kept.push(Event {
                tick: note.tick,
                kind: EventKind::NoteOn {
                    channel: note.channel,
                    key: note.key,
                    velocity: note.velocity,
                },
            });
            kept.push(Event {
                tick: off,
                kind: EventKind::NoteOff {
                    channel: note.channel,
                    key: note.key,
                },
            });
        }
        track.events = kept;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::TrackData;

    fn on(tick: i64, channel: u8, key: u8, velocity: u8) -> Event {
        Event {
            tick,
            kind: EventKind::NoteOn {
                channel,
                key,
                velocity,
            },
        }
    }
    fn off(tick: i64, channel: u8, key: u8) -> Event {
        Event {
            tick,
            kind: EventKind::NoteOff { channel, key },
        }
    }
    fn restrike(tick: i64, channel: u8, key: u8) -> Event {
        Event {
            tick,
            kind: EventKind::NoteOffRestrike { channel, key },
        }
    }
    /// `(onset, note-off, channel, key)` of every attack left in the file.
    fn pairs(events: &[Event]) -> Vec<(i64, i64, u8, u8)> {
        let mut result = Vec::new();
        for (index, event) in events.iter().enumerate() {
            let EventKind::NoteOn { channel, key, .. } = event.kind else {
                continue;
            };
            let off = events[index + 1..]
                .iter()
                .find_map(|next| match next.kind {
                    EventKind::NoteOff {
                        channel: other_channel,
                        key: other_key,
                    } if other_channel == channel && other_key == key => Some(next.tick),
                    _ => None,
                })
                .unwrap_or(event.tick);
            result.push((event.tick, off, channel, key));
        }
        result.sort();
        result
    }

    #[test]
    fn duplicate_attack_keeps_the_long_gate() {
        let mut performance = Performance {
            tracks: vec![TrackData {
                events: vec![
                    on(100, 0, 60, 80),
                    on(100, 0, 60, 80),
                    on(100, 0, 60, 0),
                    off(400, 0, 60),
                ],
            }],
        };
        canonicalize_notes(&mut performance);
        assert_eq!(
            pairs(&performance.tracks[0].events),
            vec![(100, 400, 0, 60)]
        );
        assert!(!performance.tracks[0]
            .events
            .iter()
            .any(|event| matches!(event.kind, EventKind::NoteOn { velocity: 0, .. })));
    }

    #[test]
    fn restrike_ends_one_tick_before_the_next_attack() {
        let mut performance = Performance {
            tracks: vec![TrackData {
                events: vec![on(0, 0, 64, 80), restrike(200, 0, 64), on(200, 0, 64, 80)],
            }],
        };
        canonicalize_notes(&mut performance);
        assert_eq!(
            pairs(&performance.tracks[0].events),
            vec![(0, 199, 0, 64), (200, 201, 0, 64)]
        );
    }

    #[test]
    fn regular_note_off_is_unchanged() {
        let mut performance = Performance {
            tracks: vec![TrackData {
                events: vec![on(0, 0, 60, 90), off(479, 0, 60)],
            }],
        };
        canonicalize_notes(&mut performance);
        assert_eq!(pairs(&performance.tracks[0].events), vec![(0, 479, 0, 60)]);
    }
}
