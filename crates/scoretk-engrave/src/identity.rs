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

//! Compact source identity of a drawn element.
//!
//! The identifiers returned by `scoretk_core::model::stable_ids` are the sole
//! anchoring keys shared with `write_playback`'s MusicXML and time map. An
//! engraved page keeps them as integers, never as one `String` per item:
//! [`NoteIdentity`] is the full (atom-side) record and [`SourceRef`] is its
//! packed form stored in every `DisplayItem`. The string identifier is built
//! only by [`SourceRef::xml_id`], i.e. at serialisation time.

use alloc::string::String;

use scoretk_core::model::stable_ids::{
    stable_grace_note_id_with_source, stable_note_id_with_source,
};
use scoretk_core::model::NoteSource;

/// UTF-16 offsets in the textual source of a note (GABC only), when present.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoteSourceRange {
    pub offset: u32,
    pub end: u32,
}

impl NoteSourceRange {
    pub fn to_model(self) -> NoteSource {
        NoteSource {
            offset: self.offset as usize,
            end: self.end as usize,
            ..NoteSource::default()
        }
    }
}

/// Identity of an ornement: its index in the grace-chord list, the note's
/// index in that chord and whether it is played before the main chord.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GraceIdentity {
    pub index: u32,
    pub note: u32,
    pub before: bool,
}

/// Full note identity, identical to the one used by the MusicXML exporter
/// (`docs/agents/timemap.md` §3). Indices are model indices: `measure` is the
/// ordinal in `Score::measures` (the time map's `measures[].ordinal`), `staff`
/// is `StaffId::raw()`, `voice` is 0..3, `chord` is the rank of the
/// `ChordRest` in its voice (rests included) and `note` its index in the
/// chord.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoteIdentity {
    pub measure: u32,
    pub staff: u32,
    pub voice: u8,
    pub chord: u32,
    pub note: u32,
    pub grace: Option<GraceIdentity>,
    pub source: Option<NoteSourceRange>,
}

impl NoteIdentity {
    /// A plain note or rest (`stable_note_id`).
    pub fn note(measure: usize, staff: u32, voice: u8, chord: usize, note: usize) -> Self {
        Self {
            measure: measure as u32,
            staff,
            voice,
            chord: chord as u32,
            note: note as u32,
            grace: None,
            source: None,
        }
    }

    /// An ornament (`stable_grace_note_id`).
    #[allow(clippy::too_many_arguments)]
    pub fn grace(
        measure: usize,
        staff: u32,
        voice: u8,
        chord: usize,
        index: usize,
        note: usize,
        before: bool,
    ) -> Self {
        Self {
            measure: measure as u32,
            staff,
            voice,
            chord: chord as u32,
            note: note as u32,
            grace: Some(GraceIdentity {
                index: index as u32,
                note: note as u32,
                before,
            }),
            source: None,
        }
    }

    /// The string identifier, built only when a backend serialises the item.
    pub fn xml_id(&self) -> String {
        match self.grace {
            Some(grace) => stable_grace_note_id_with_source(
                self.measure as usize,
                self.staff,
                self.voice,
                self.chord as usize,
                grace.index as usize,
                grace.note as usize,
                grace.before,
                self.source.map(NoteSourceRange::to_model),
            ),
            None => stable_note_id_with_source(
                self.measure as usize,
                self.staff,
                self.voice,
                self.chord as usize,
                self.note as usize,
                self.source.map(NoteSourceRange::to_model),
            ),
        }
    }
}

/// Packed source reference carried by every display item (two `u64` words).
///
/// `place` holds the system (10 bits), measure ordinal (20 bits), staff
/// (10 bits) and part (8 bits). `note` holds the identity kind (2 bits:
/// 0 = none, 1 = note, 2 = grace), voice (2), chord (14), note (10), grace
/// index (10), the before/after bit and an index into the page's compact
/// table of textual source ranges (11 bits, 0 = none). Values beyond a
/// field's range are clamped: the identifier stays deterministic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceRef {
    place: u64,
    note: u64,
}

const NONE_20: u64 = (1 << 20) - 1;
const NONE_10: u64 = (1 << 10) - 1;
const NONE_8: u64 = (1 << 8) - 1;

impl SourceRef {
    /// No source (title frame, or an item not yet given a placement).
    pub const fn none() -> Self {
        Self { place: 0, note: 0 }
    }

    /// Placement of items that belong to no measure (title, system padding).
    pub fn system(system: usize) -> Self {
        Self {
            place: (system.min(1023) as u64) | (NONE_20 << 10) | (NONE_10 << 30) | (NONE_8 << 40),
            note: 0,
        }
    }

    /// Full placement of an item drawn on `staff` of `part` in a measure.
    pub fn placement(system: usize, measure: usize, staff: usize, part: usize) -> Self {
        Self {
            place: (system.min(1023) as u64)
                | ((measure.min(1_048_574) as u64) << 10)
                | ((staff.min(1023) as u64) << 30)
                | ((part.min(255) as u64) << 40),
            note: 0,
        }
    }

    /// Placement of a staff before its measures are visited.
    pub fn staff_placement(system: usize, staff: usize, part: usize) -> Self {
        Self {
            place: (system.min(1023) as u64)
                | (NONE_20 << 10)
                | ((staff.min(1023) as u64) << 30)
                | ((part.min(255) as u64) << 40),
            note: 0,
        }
    }

    /// Replaces the measure (and part) of a placement already carrying its
    /// system and staff.
    pub fn with_measure(self, measure: usize, part: usize) -> Self {
        Self {
            place: (self.place & !(NONE_20 << 10) & !(NONE_8 << 40))
                | ((measure.min(1_048_574) as u64) << 10)
                | ((part.min(255) as u64) << 40),
            note: 0,
        }
    }

    /// Attaches a packed note identity, interning a textual source range if
    /// needed. This is done when the item is pushed, not when it is created.
    pub(crate) fn set_note(
        &mut self,
        id: &NoteIdentity,
        sources: &mut alloc::vec::Vec<NoteSourceRange>,
    ) {
        let grace = id.grace.unwrap_or_default();
        let source_index = match id.source {
            Some(range) => if let Some(index) = sources.iter().position(|entry| *entry == range) {
                index + 1
            } else {
                sources.push(range);
                sources.len()
            }
            .min(2047) as u64,
            None => 0,
        };
        self.note = (if id.grace.is_some() { 2 } else { 1 })
            | ((u64::from(id.voice & 3)) << 2)
            | ((id.chord.min(0x3fff) as u64) << 4)
            | ((id.note.min(0x3ff) as u64) << 18)
            | ((grace.index.min(0x3ff) as u64) << 28)
            | ((grace.note.min(0x3ff) as u64) << 38)
            | ((u64::from(grace.before)) << 48)
            | (source_index << 49);
        // The first two bits encode the kind: 1 for a note, 2 for a grace.
        debug_assert_eq!(self.note & 3, if id.grace.is_some() { 2 } else { 1 });
    }

    pub fn system_index(self) -> Option<usize> {
        let value = self.place & NONE_10;
        (value != NONE_10).then_some(value as usize)
    }

    pub fn measure_ordinal(self) -> Option<usize> {
        let value = (self.place >> 10) & NONE_20;
        (value != NONE_20).then_some(value as usize)
    }

    pub fn staff(self) -> Option<usize> {
        let value = (self.place >> 30) & NONE_10;
        (value != NONE_10).then_some(value as usize)
    }

    pub fn part(self) -> Option<usize> {
        let value = (self.place >> 40) & NONE_8;
        (value != NONE_8).then_some(value as usize)
    }

    pub fn has_note(self) -> bool {
        self.note & 3 != 0
    }

    /// Derives the string identifier; `None` for items with no note source.
    /// No allocation happens for the common (non-serialising) path.
    pub fn xml_id(self, sources: &[NoteSourceRange]) -> Option<String> {
        let kind = self.note & 3;
        if kind == 0 {
            return None;
        }
        let measure = self.measure_ordinal().unwrap_or(0);
        let staff = self.staff().unwrap_or(0) as u32;
        let voice = ((self.note >> 2) & 3) as u8;
        let chord = ((self.note >> 4) & 0x3fff) as usize;
        let note = ((self.note >> 18) & 0x3ff) as usize;
        let source_index = ((self.note >> 49) & 0x7ff) as usize;
        let source = source_index
            .checked_sub(1)
            .and_then(|index| sources.get(index))
            .copied()
            .map(NoteSourceRange::to_model);
        if kind == 2 {
            let grace_index = ((self.note >> 28) & 0x3ff) as usize;
            let grace_note = ((self.note >> 38) & 0x3ff) as usize;
            let before = (self.note >> 48) & 1 != 0;
            Some(stable_grace_note_id_with_source(
                measure,
                staff,
                voice,
                chord,
                grace_index,
                grace_note,
                before,
                source,
            ))
        } else {
            Some(stable_note_id_with_source(
                measure, staff, voice, chord, note, source,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identite_note_et_grace() {
        assert_eq!(NoteIdentity::note(3, 5, 2, 9, 1).xml_id(), "m3-s5-v2-c9-n1");
        assert_eq!(
            NoteIdentity::grace(1, 0, 0, 2, 3, 1, false).xml_id(),
            "m1-s0-v0-c2-g3an1"
        );
    }

    #[test]
    fn reference_compacte_restaure_identifiant_et_placement() {
        let mut sources = alloc::vec::Vec::new();
        let mut reference = SourceRef::placement(2, 17, 5, 1);
        reference.set_note(&NoteIdentity::note(17, 5, 3, 42, 2), &mut sources);
        assert_eq!(reference.system_index(), Some(2));
        assert_eq!(reference.measure_ordinal(), Some(17));
        assert_eq!(reference.staff(), Some(5));
        assert_eq!(reference.part(), Some(1));
        assert_eq!(
            reference.xml_id(&sources).as_deref(),
            Some("m17-s5-v3-c42-n2")
        );
        assert!(sources.is_empty());
    }

    #[test]
    fn position_source_internee_une_seule_fois() {
        let mut sources = alloc::vec::Vec::new();
        let mut id = NoteIdentity::note(0, 0, 0, 0, 0);
        id.source = Some(NoteSourceRange {
            offset: 18,
            end: 19,
        });
        let mut first = SourceRef::placement(0, 0, 0, 0);
        first.set_note(&id, &mut sources);
        let mut second = SourceRef::placement(0, 0, 0, 0);
        second.set_note(&id, &mut sources);
        assert_eq!(sources.len(), 1);
        assert_eq!(
            first.xml_id(&sources).as_deref(),
            Some("m0-s0-v0-c0-n0-o18-e19")
        );
    }
}
