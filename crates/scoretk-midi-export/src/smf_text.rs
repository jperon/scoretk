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

//! Texts carried by a Standard MIDI File (`MidiOptions::smf_text`).
//!
//! `[SMF §Meta-Events]` defines `FF 01` as a "Text event" (types `01`–`0F`
//! are reserved for text), `FF 02` as a copyright notice, `FF 03` as a track
//! name, `FF 04` as an instrument name and `FF 06`/`FF 07` as marker and cue
//! point. MuseScore 4.7.4 writes none of the generic text metas, so this is
//! an **additive** extension: the default path is byte-identical to the
//! reference exports.
//!
//! The `@ST:` / `@SY:` / `@TK:` prefixes are private semantics stored inside
//! the standard `FF 01` payload, exactly as ABC's `%%scoretk` directives are
//! private directives inside a standard comment (`docs/agents/abc.md`). They
//! let the reader map a text back to the right model field without adding a
//! new meta-event type. Other sequencers simply display the text.

use scoretk_core::model::{Annotation, Score, StaffId};

use super::repeat_list::UNITS_PER_WHOLE;

/// Prefix of a staff text in `FF 01`.
pub(crate) const STAFF_PREFIX: &str = "@ST:";
/// Prefix of a system text in `FF 01`.
pub(crate) const SYSTEM_PREFIX: &str = "@SY:";
/// Prefix of a score metadata field in `FF 01` (`@TK:<field>:<value>`).
pub(crate) const TITLE_PREFIX: &str = "@TK:";

/// `(tick, text)` of the staff texts of one staff.
///
/// `private = true` keeps the historical `@ST:`/`@SY:` payloads of the
/// secondary `smf_text` mode. `private = false` writes the text itself in a
/// standard `FF 01` of the staff's track; system texts then go to a separate
/// `FF 07` (`system_text_events`), so no private prefix is needed.
pub(crate) fn staff_text_events(
    score: &Score,
    staff: StaffId,
    private: bool,
) -> Vec<(i64, String)> {
    let mut result = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for entry in &segment.annotations {
                if entry.track.staff() != staff {
                    continue;
                }
                match &entry.annotation {
                    Annotation::StaffText(value) | Annotation::Expression(value) => {
                        let plain = value.text.plain();
                        result.push((
                            segment.tick.ticks(UNITS_PER_WHOLE),
                            if private {
                                format!("{STAFF_PREFIX}{plain}")
                            } else {
                                plain.to_owned()
                            },
                        ));
                    }
                    Annotation::SystemText(value) if private => result.push((
                        segment.tick.ticks(UNITS_PER_WHOLE),
                        format!("{SYSTEM_PREFIX}{}", value.text.plain()),
                    )),
                    _ => {}
                }
            }
        }
    }
    result
}

/// `(tick, text)` of the system texts of one staff, written as the standard
/// `FF 07` cue point (a text meta at system scope, `[SMF §Meta-Events]`).
pub(crate) fn system_text_events(score: &Score, staff: StaffId) -> Vec<(i64, String)> {
    let mut result = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for entry in &segment.annotations {
                if entry.track.staff() != staff {
                    continue;
                }
                if let Annotation::SystemText(value) = &entry.annotation {
                    result.push((
                        segment.tick.ticks(UNITS_PER_WHOLE),
                        value.text.plain().to_owned(),
                    ));
                }
            }
        }
    }
    result
}

/// `(tick, text)` of the rehearsal marks of one staff (`FF 06`).
pub(crate) fn marker_events(score: &Score, staff: StaffId) -> Vec<(i64, String)> {
    let mut result = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for entry in &segment.annotations {
                if entry.track.staff() != staff {
                    continue;
                }
                if let Annotation::RehearsalMark(value) = &entry.annotation {
                    result.push((
                        segment.tick.ticks(UNITS_PER_WHOLE),
                        value.text.plain().to_owned(),
                    ));
                }
            }
        }
    }
    result
}

/// Metadata fields that SMF text can carry. `copyright` is written as the
/// standard `FF 02`, `work_title` as the sequence name `FF 03` of track 0
/// (`[SMF §Meta-Events]`), so neither is repeated as an `@TK:` field.
type MetadataField = (
    &'static str,
    for<'a> fn(&'a scoretk_core::model::ScoreMetadata) -> &'a str,
);

pub(crate) const METADATA_FIELDS: &[MetadataField] = &[
    ("subtitle", |meta| &meta.subtitle),
    ("composer", |meta| &meta.composer),
    ("lyricist", |meta| &meta.lyricist),
    ("poet", |meta| &meta.poet),
    ("translator", |meta| &meta.translator),
    ("arranger", |meta| &meta.arranger),
    ("source", |meta| &meta.source),
    ("movement_title", |meta| &meta.movement_title),
    ("movement_number", |meta| &meta.movement_number),
    ("work_number", |meta| &meta.work_number),
];

/// `(tick, text)` of the score metadata, all at tick 0 of the first track.
///
/// The standard mode uses RP-026 song-information tags (`{#NAME=value}`),
/// whose generic `{#}` form is defined by MMA RP-026. `work_title` and
/// `copyright` are not repeated here: they use `FF 03` and `FF 02`.
pub(crate) fn metadata_events(score: &Score, private: bool) -> Vec<(i64, String)> {
    METADATA_FIELDS
        .iter()
        .filter_map(|(name, get)| {
            let value = get(&score.metadata);
            (!value.is_empty()).then(|| {
                (
                    0,
                    if private {
                        format!("{TITLE_PREFIX}{name}:{value}")
                    } else {
                        format!("{{#{}={value}}}", name.to_ascii_uppercase())
                    },
                )
            })
        })
        .collect()
}
