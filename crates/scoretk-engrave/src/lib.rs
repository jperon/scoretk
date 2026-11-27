// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port of `src/engraving/rendering/score/scorepageviewlayout.cpp:199-229`
// (`ScorePageViewLayout::doLayout`) and `scorerenderer.cpp:49-52`
// (`ScoreRenderer::layoutScore`) of MuseScore Studio 4.7.4 (tag v4.7.4),
// Copyright (C) MuseScore Limited and others, modified (rewritten in Rust)
// in 2026: atoms are computed once, a page is engraved on demand, and the
// whole-document `System*`/`Page*` graph is never built.
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

//! Fixed-point engraving of a single staff into a page-local display list.
//!
//! `no_std` + `alloc`, no I/O. Geometry is i32 in 1/1024 staff space: the same
//! bytes are produced on the host, on wasm and on the Kobo's Cortex-A8.
//!
//! ```
//! use scoretk_engrave::{Engraver, PageSpec};
//! use scoretk_core::model::{Part, Score};
//!
//! let mut score = Score::new();
//! let part = score.add_part(Part::new("Piano", "Pno."));
//! score.add_staff(part);
//! let engraver = Engraver::new(&score, PageSpec::default())?;
//! assert_eq!(engraver.page_count(), 1);
//! assert!(engraver.page(0).items.is_empty()); // no measure, no staff lines yet
//! # Ok::<(), scoretk_engrave::EngraveError>(())
//! ```
//!
//! P1 deliberately handles **one staff**; multiple staves, parts and systems
//! with brackets arrive in P5.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt;

use scoretk_core::model::Score;

pub mod atoms;
pub mod breaking;
pub mod display;
pub mod layout;
pub mod style;

pub use atoms::{Atom, AtomKind, BarCode, ChordAtom, MeasureAtom, NoteAtom};
pub use breaking::SystemPlan;
pub use display::{DisplayItem, DisplayList, Geometry, ItemKind};
pub use layout::PageSpec;
pub use style::SP;

/// Why engraving is not possible with a given score.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngraveError {
    /// The pivot model has no staff to engrave.
    NoStaff,
    /// P1 handles a single staff only.
    MultipleStaves(usize),
}

impl fmt::Display for EngraveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::NoStaff => f.write_str("no staff to engrave"),
            Self::MultipleStaves(count) => {
                write!(f, "P1 engraves a single staff, score has {count}")
            }
        }
    }
}

/// The engraver: per-measure atoms computed once, pages engraved on demand.
///
/// `page(n)` reads only the atoms and the page spec; it never reads the
/// `Score` again and never recomputes an atom. The host owns the page cache
/// window of plan §4b.
#[derive(Debug)]
pub struct Engraver {
    atoms: Vec<MeasureAtom>,
    systems: Vec<SystemPlan>,
    spec: PageSpec,
}

impl Engraver {
    /// Computes the atoms of the whole score (one O(n) pass).
    pub fn new(score: &Score, spec: PageSpec) -> Result<Self, EngraveError> {
        match score.staves.len() {
            0 => Err(EngraveError::NoStaff),
            1 => {
                let atoms = atoms::compute(score);
                let (systems, atoms) = breaking::compute_with_atoms(&atoms, &spec);
                Ok(Self {
                    atoms,
                    systems,
                    spec,
                })
            }
            count => Err(EngraveError::MultipleStaves(count)),
        }
    }

    /// Builds an engraver whose page geometry comes from the score's
    /// `<Style>` (page size, margins, spatium). The facade uses this entry
    /// point; `new` remains available for an explicit page geometry.
    pub fn from_score(score: &Score) -> Result<Self, EngraveError> {
        Self::new(score, layout::PageSpec::from_score(score))
    }

    /// Engraves one page from the cached atoms and breaking plan.
    pub fn page(&self, page: usize) -> DisplayList {
        layout::build_page(&self.atoms, &self.spec, &self.systems, page)
    }

    /// Total number of pages.
    pub fn page_count(&self) -> usize {
        breaking::page_count(&self.systems)
    }

    /// The cached system breaks (read-only), one record per system.
    pub fn systems(&self) -> &[SystemPlan] {
        &self.systems
    }

    /// The cached atoms (read-only), for diagnostics and the oracle.
    pub fn atoms(&self) -> &[MeasureAtom] {
        &self.atoms
    }

    pub fn spec(&self) -> &PageSpec {
        &self.spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{
        Chord, ChordRest, Clef, ClefType, Duration, Fraction, KeySig, Measure, Note, Part, Segment,
        SegmentItem, SegmentType, Staff, TimeSig, Track,
    };

    fn one_staff_score() -> Score {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno."));
        score.add_staff_with(part, Staff::new(part));
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::new(1, 1));
        let mut seg = Segment::new(SegmentType::HeaderClef, Fraction::ZERO, 1);
        seg.set_element(
            Track::from_raw(0),
            SegmentItem::Clef(Clef::new(ClefType::G)),
        );
        measure.add_segment(seg);
        let mut seg = Segment::new(SegmentType::KeySig, Fraction::ZERO, 1);
        seg.set_element(
            Track::from_raw(0),
            SegmentItem::KeySig(KeySig::new(scoretk_core::model::Key::G)),
        );
        measure.add_segment(seg);
        let mut seg = Segment::new(SegmentType::TimeSig, Fraction::ZERO, 1);
        seg.set_element(Track::from_raw(0), SegmentItem::TimeSig(TimeSig::new(4, 4)));
        measure.add_segment(seg);
        for (tick, pitch, duration) in [
            (Fraction::ZERO, 67, Duration::quarter()),
            (Fraction::new(1, 4), 76, Duration::quarter()),
            (Fraction::new(1, 2), 60, Duration::quarter()),
            (Fraction::new(3, 4), 53, Duration::quarter()),
        ] {
            let note = score.add_note(Note::new(pitch, 14));
            let mut rest = ChordRest::chord(duration);
            let mut chord = Chord::default();
            chord.notes.push(note);
            rest.kind = scoretk_core::model::ChordRestKind::Chord(chord);
            let mut seg = Segment::new(SegmentType::ChordRest, tick, 1);
            seg.set_element(Track::from_raw(0), SegmentItem::ChordRest(rest));
            measure.add_segment(seg);
        }
        score.add_measure(measure);
        score
    }

    #[test]
    fn engraves_a_one_staff_score() {
        let score = one_staff_score();
        let engraver = Engraver::new(&score, PageSpec::default()).unwrap();
        assert_eq!(engraver.atoms().len(), 1);
        let page = engraver.page(0);
        assert_eq!(page.count(ItemKind::Notehead), 4);
        assert_eq!(page.count(ItemKind::Stem), 4);
        assert_eq!(page.count(ItemKind::Clef), 1);
        assert_eq!(page.count(ItemKind::Accidental), 1);
        assert!(page.count(ItemKind::TimeSig) >= 2);
    }

    #[test]
    fn page_does_not_recompute_atoms() {
        let score = one_staff_score();
        let engraver = Engraver::new(&score, PageSpec::default()).unwrap();
        let atoms_before = engraver.atoms().to_vec();
        let first = engraver.page(0);
        let second = engraver.page(0);
        assert_eq!(first, second);
        assert_eq!(engraver.atoms(), atoms_before.as_slice());
    }

    #[test]
    fn rejects_empty_and_multi_staff_scores() {
        assert_eq!(
            Engraver::new(&Score::new(), PageSpec::default()).unwrap_err(),
            EngraveError::NoStaff
        );
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno."));
        score.add_staff(part);
        score.add_staff(part);
        assert_eq!(
            Engraver::new(&score, PageSpec::default()).unwrap_err(),
            EngraveError::MultipleStaves(2)
        );
    }
}
