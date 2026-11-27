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

//! Fixed-point engraving of the main score's visible staves into a page-local
//! display list.
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
//! Hidden staves and hidden parts are filtered out once in
//! [`Engraver::new`], like MuseScore's `System::staves`; excerpts/parts of a
//! `.mscz` are not engraved as independent scores.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt;

use scoretk_core::model::{Credit, Score};

pub mod atoms;
pub mod breaking;
pub mod decor;
pub mod display;
pub mod identity;
pub mod layout;
pub mod layout_data;
pub mod options;
pub mod screen;
pub mod skyline;
pub mod spacing;
pub mod staffs;
pub mod style;
pub mod vertical;

pub use atoms::{Atom, AtomKind, BarCode, ChordAtom, MeasureAtom, NoteAtom};
pub use breaking::SystemPlan;
pub use display::{DisplayItem, DisplayList, Geometry, ItemKind};
pub use identity::{GraceIdentity, NoteIdentity, NoteSourceRange, SourceRef};
pub use layout::PageSpec;
pub use layout_data::{BBox, MeasureLayout, NoteLayout, PageLayout, SystemLayout};
pub use options::EngraveOptions;
pub use screen::screen_staff_px;
pub use staffs::StaffInfo;
pub use style::SP;

/// Why engraving is not possible with a given score.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngraveError {
    /// The pivot model has no visible staff to engrave.
    NoStaff,
}

impl fmt::Display for EngraveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::NoStaff => f.write_str("no staff to engrave"),
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
    /// One atom sequence per visible staff, indexed like `staffs`.
    atoms: Vec<Vec<MeasureAtom>>,
    systems: Vec<SystemPlan>,
    spec: PageSpec,
    staffs: Vec<StaffInfo>,
    /// Title-frame texts copied once: `page(n)` never borrows the `Score`.
    credits: Vec<Credit>,
}

impl Engraver {
    /// Computes the atoms of the whole score (one O(n) pass).
    pub fn new(score: &Score, spec: PageSpec) -> Result<Self, EngraveError> {
        Self::new_with_options(score, spec, &EngraveOptions::default())
    }

    /// Builds an engraver with explicit rendering options. The score is only
    /// read during construction; `page(n)` never needs it again.
    pub fn new_with_options(
        score: &Score,
        spec: PageSpec,
        options: &EngraveOptions,
    ) -> Result<Self, EngraveError> {
        let staffs = StaffInfo::from_score_with_options(score, options);
        if staffs.is_empty() {
            return Err(EngraveError::NoStaff);
        }
        let atoms = atoms::compute_with_options(score, options);
        let credits = score.engraving.credits.clone();
        let (_, height) = staffs::system_geometry(&staffs, &spec);
        // One height per system: spacers are not modelled yet, but the
        // skylines below can widen each staff and system independently.
        // With `condense`, a measure's height only counts its sounding staves
        // (the first measure keeps them all, like `condenseFirstPage: false`).
        let measure_count = atoms.first().map_or(0, Vec::len);
        let heights: Vec<i32> = if spec.hide_empty_staves {
            (0..measure_count)
                .map(|measure| {
                    let visible: Vec<usize> = (0..staffs.len())
                        .filter(|&index| {
                            measure == 0
                                || atoms[index].get(measure).is_some_and(|atom| {
                                    atom.events.iter().any(|event| {
                                        event
                                            .chord
                                            .as_ref()
                                            .is_some_and(|chord| !chord.notes.is_empty())
                                    })
                                })
                        })
                        .collect();
                    let subset: Vec<StaffInfo> = visible
                        .iter()
                        .filter_map(|&index| staffs.get(index).cloned())
                        .collect();
                    if subset.is_empty() {
                        height
                    } else {
                        let positions: Vec<usize> = (0..subset.len()).collect();
                        let offsets = crate::staffs::system_offsets(&subset, &positions, &spec);
                        offsets.last().copied().unwrap_or(0)
                            + subset
                                .last()
                                .map_or(crate::style::STAFF_HEIGHT, StaffInfo::height)
                    }
                })
                .collect()
        } else {
            alloc::vec![height; measure_count]
        };
        let (mut systems, atoms) = breaking::compute_with_atoms(&atoms, &spec, &staffs, &heights);
        let mut spec = spec;
        // A preliminary layout gives the shapes of every drawn item; their
        // skylines then set the minimum staff and system distances before
        // the final page is laid out (`systemlayout.cpp:2485-2581, 2900-2989`).
        let skylines = layout::all_skylines(&atoms, &spec, &systems, &staffs, &credits);
        vertical::apply_skylines(&mut systems, &skylines, &staffs, &spec);
        breaking::repaginate(&mut systems, &spec);
        vertical::distribute_all(&mut systems, &staffs, &spec);
        if options.one_system_per_page {
            breaking::one_system_per_page(&mut systems, &spec);
        }
        if options.adjust_page_height {
            spec.page_height = breaking::content_height(&systems, &spec)
                .clamp(spec.margin_top * 2, spec.page_height);
        }
        Ok(Self {
            atoms,
            systems,
            spec,
            staffs,
            credits,
        })
    }

    /// Builds an engraver whose page geometry comes from the score's
    /// `<Style>` (page size, margins, spatium). The facade uses this entry
    /// point; `new` remains available for an explicit page geometry.
    pub fn from_score(score: &Score) -> Result<Self, EngraveError> {
        Self::from_score_with_options(score, &EngraveOptions::default())
    }

    /// Page geometry and options derived from the score and the caller's
    /// settings. // Port of the style-driven geometry of `pagestyle.cpp:26-41`.
    pub fn from_score_with_options(
        score: &Score,
        options: &EngraveOptions,
    ) -> Result<Self, EngraveError> {
        let spec = layout::PageSpec::from_score_with_options(score, options);
        Self::new_with_options(score, spec, options)
    }

    /// Engraves one page from the cached atoms and breaking plan.
    pub fn page(&self, page: usize) -> DisplayList {
        layout::build_page(
            &self.atoms,
            &self.spec,
            &self.systems,
            &self.staffs,
            &self.credits,
            page,
        )
    }

    /// The visible staves engraved by this instance.
    pub fn staffs(&self) -> &[StaffInfo] {
        &self.staffs
    }

    /// Total number of pages.
    pub fn page_count(&self) -> usize {
        breaking::page_count(&self.systems)
    }

    /// The cached system breaks (read-only), one record per system.
    pub fn systems(&self) -> &[SystemPlan] {
        &self.systems
    }

    /// The cached atoms of one staff, for diagnostics and the oracle.
    pub fn atoms(&self, staff: usize) -> &[MeasureAtom] {
        &self.atoms[staff]
    }

    pub fn spec(&self) -> &PageSpec {
        &self.spec
    }

    /// Compact layout description of one page (boxes in 1/1024 staff space).
    pub fn page_layout(&self, page: usize) -> layout_data::PageLayout {
        layout_data::PageLayout::from_list(&self.page(page), &self.spec)
    }

    /// Page containing a stable note id, or `None` if it is not engraved.
    /// Equivalent of Verovio's `getPageWithElement`.
    pub fn page_of_element(&self, id: &str) -> Option<usize> {
        layout_data::page_of_element(self, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{
        Chord, ChordRest, Clef, ClefType, Credit, CreditStyle, Duration, Fraction, KeySig, Measure,
        Note, Part, RichText, Segment, SegmentItem, SegmentType, Staff, TimeSig, Track,
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

    /// A 70 % staff has 70 % noteheads (and stems, ledgers and lyrics),
    /// as `Staff::staffMag` scales the staff's spatium (`staff.cpp:977-1004`)
    /// and `EngravingItem::mag` its drawn items (`engravingitem.cpp:2911`).
    #[test]
    fn a_seventy_percent_staff_draws_seventy_percent_noteheads() {
        let mut score = one_staff_score();
        let second_part = score.add_part(Part::new("Bass", "B."));
        let second_staff = score.add_staff(second_part);
        let track = Track::new(second_staff, 0);
        for segment in &mut score.measures[0].segments {
            let copy = segment
                .iter_elements()
                .find(|(source, _)| source.voice() == 0)
                .map(|(_, item)| item.clone());
            if matches!(copy, Some(SegmentItem::ChordRest(_))) {
                segment.set_element(track, copy.unwrap());
            }
        }
        score.staves[1].user_mag_percent = 70;
        let engraver = Engraver::from_score(&score).unwrap();
        let page = engraver.page(0);
        let head_width = |staff: usize| -> i32 {
            let item = page
                .items
                .iter()
                .find(|item| item.kind == ItemKind::Notehead && item.source.staff() == Some(staff))
                .expect("a notehead");
            let (x0, _, x1, _) = item.bbox(&page).expect("a notehead box");
            x1 - x0
        };
        let full = head_width(0);
        let small = head_width(1);
        let expected = full * EngraveOptions::mag_fixed(70) / 1024;
        assert!(
            (small - expected).abs() <= 2,
            "head widths: full {full}, small {small}, expected {expected}"
        );
        assert!(page.count(ItemKind::StaffLine) == 10);
    }

    /// A runtime part override replaces `Staff::user_mag_percent` without
    /// touching the model: cantus can change a staff size at a new layout.
    #[test]
    fn a_part_mag_override_scales_that_part_only() {
        let mut score = one_staff_score();
        let second_part = score.add_part(Part::new("Bass", "B."));
        let second_staff = score.add_staff(second_part);
        let track = Track::new(second_staff, 0);
        for segment in &mut score.measures[0].segments {
            let copy = segment
                .iter_elements()
                .find(|(source, _)| source.voice() == 0)
                .map(|(_, item)| item.clone());
            if matches!(copy, Some(SegmentItem::ChordRest(_))) {
                segment.set_element(track, copy.unwrap());
            }
        }
        let options = EngraveOptions {
            part_mag_percent: alloc::vec![(1, 70)],
            ..EngraveOptions::default()
        };
        let engraver = Engraver::from_score_with_options(&score, &options).unwrap();
        assert_eq!(score.staves[1].user_mag_percent, 100, "model unchanged");
        assert_eq!(engraver.staffs()[0].mag, 1024);
        assert_eq!(engraver.staffs()[1].mag, EngraveOptions::mag_fixed(70));
        let page = engraver.page(0);
        let heads: Vec<&crate::display::DisplayItem> = page
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Notehead)
            .collect();
        assert!(heads.iter().any(|item| matches!(
            item.geometry,
            crate::display::Geometry::Path { mag, .. } if mag == EngraveOptions::mag_fixed(70)
        )));
    }

    #[test]
    fn zoom_and_page_pixels_change_the_spec_not_the_default() {
        let score = one_staff_score();
        let print = Engraver::from_score(&score).unwrap();
        let options = EngraveOptions {
            scale: 2.0,
            page_width_px: Some(800),
            page_height_px: Some(600),
            ..EngraveOptions::default()
        };
        let zoomed = Engraver::from_score_with_options(&score, &options).unwrap();
        assert_eq!(print.spec().spatium_mm, 1.75);
        assert!(zoomed.spec().spatium_mm > print.spec().spatium_mm);
        assert_eq!(zoomed.page_count(), print.page_count());
        // The default page still derives from the score style.
        assert!(print.spec().printable_width > 0 && print.spec().page_height > 0);
    }

    #[test]
    fn lyric_size_scales_only_the_lyric_text() {
        let score = one_staff_score();
        let normal = Engraver::from_score(&score).unwrap();
        let options = EngraveOptions {
            lyric_size: 9.0, // twice Verovio's 4.5 default
            ..EngraveOptions::default()
        };
        let bigger = Engraver::from_score_with_options(&score, &options).unwrap();
        assert_eq!(normal.spec().lyric_font_size, 10.0);
        assert_eq!(bigger.spec().lyric_font_size, 20.0);
        let em = |engraver: &Engraver| {
            crate::layout::text_em(engraver.spec(), engraver.spec().lyric_font_size)
        };
        assert_eq!(em(&bigger), 2 * em(&normal));
        // Note glyphs use their SMuFL size, not the text style.
        assert_eq!(bigger.staffs()[0].mag, normal.staffs()[0].mag);
    }

    #[test]
    fn adjust_page_height_shrinks_the_viewbox_to_the_content() {
        let score = one_staff_score();
        let print = Engraver::from_score(&score).unwrap();
        let options = EngraveOptions {
            adjust_page_height: true,
            ..EngraveOptions::default()
        };
        let adjusted = Engraver::from_score_with_options(&score, &options).unwrap();
        assert!(adjusted.spec().page_height < print.spec().page_height);
        assert!(adjusted.spec().page_height >= 2 * adjusted.spec().margin_top);
        assert_eq!(adjusted.page_count(), print.page_count());
    }

    fn two_staff_score_with_an_empty_second_system() -> Score {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno."));
        let first = score.add_staff_with(part, Staff::new(part));
        let second = score.add_staff(part);
        for measure_index in 0..2 {
            let mut measure = Measure::new(
                measure_index,
                Fraction::new(measure_index as i64, 1),
                Fraction::new(1, 1),
            );
            for staff in [first, second] {
                let track = Track::new(staff, 0);
                let mut clef = Segment::new(SegmentType::HeaderClef, Fraction::ZERO, 1);
                clef.set_element(track, SegmentItem::Clef(Clef::new(ClefType::G)));
                measure.add_segment(clef);
                let mut time = Segment::new(SegmentType::TimeSig, Fraction::ZERO, 1);
                time.set_element(track, SegmentItem::TimeSig(TimeSig::new(4, 4)));
                measure.add_segment(time);
            }
            for voice in 0..2 {
                let track = Track::new(if voice == 0 { first } else { second }, 0);
                let kind = if voice == 0 || measure_index == 0 {
                    let note = score.add_note(Note::new(67, 14));
                    let mut chord = Chord::default();
                    chord.notes.push(note);
                    scoretk_core::model::ChordRestKind::Chord(chord)
                } else {
                    scoretk_core::model::ChordRestKind::Rest(scoretk_core::model::Rest::default())
                };
                let mut rest = ChordRest::chord(Duration::whole());
                rest.kind = kind;
                let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
                segment.set_element(track, SegmentItem::ChordRest(rest));
                measure.add_segment(segment);
            }
            if measure_index == 0 {
                measure.line_break = true;
            }
            score.add_measure(measure);
        }
        score
    }

    #[test]
    fn an_empty_staff_is_hidden_on_later_systems_but_not_the_first() {
        let score = two_staff_score_with_an_empty_second_system();
        let plain = Engraver::from_score(&score).unwrap();
        let options = EngraveOptions {
            hide_empty_staves: true,
            ..EngraveOptions::default()
        };
        let condensed = Engraver::from_score_with_options(&score, &options).unwrap();
        let staff_lines = |engraver: &Engraver| {
            (0..engraver.page_count())
                .flat_map(|page| engraver.page(page).items)
                .filter(|item| item.kind == ItemKind::StaffLine)
                .count()
        };
        assert_eq!(staff_lines(&plain), 4 * 5, "two staves on two systems");
        assert_eq!(
            staff_lines(&condensed),
            3 * 5,
            "the first system keeps both staves, the second keeps one"
        );
    }

    #[test]
    fn engraves_a_one_staff_score() {
        let score = one_staff_score();
        let engraver = Engraver::new(&score, PageSpec::default()).unwrap();
        assert_eq!(engraver.atoms(0).len(), 1);
        let page = engraver.page(0);
        assert_eq!(page.count(ItemKind::Notehead), 4);
        assert_eq!(page.count(ItemKind::Stem), 4);
        assert_eq!(page.count(ItemKind::Clef), 1);
        assert_eq!(page.count(ItemKind::Accidental), 1);
        assert!(page.count(ItemKind::TimeSig) >= 2);
    }

    #[test]
    fn title_frame_shifts_the_first_system_and_draws_text() {
        let mut score = one_staff_score();
        let mut credit = Credit::new(RichText::new("A title"), CreditStyle::Title);
        credit.vbox_height = 10.0;
        score.engraving.credits.push(credit);
        let engraver = Engraver::from_score(&score).unwrap();
        assert_eq!(
            engraver.systems()[0].y,
            engraver.spec().margin_top + engraver.spec().title_frame_height
        );
        assert!(engraver.spec().title_frame_height >= 17 * SP);
        let page = engraver.page(0);
        assert!(page.count(ItemKind::Text) > 0);
    }

    #[test]
    fn page_does_not_recompute_atoms() {
        let score = one_staff_score();
        let engraver = Engraver::new(&score, PageSpec::default()).unwrap();
        let atoms_before = engraver.atoms(0).to_vec();
        let first = engraver.page(0);
        let second = engraver.page(0);
        assert_eq!(first, second);
        assert_eq!(engraver.atoms(0), atoms_before.as_slice());
    }

    #[test]
    fn rejects_an_empty_score_and_engraves_multiple_staves() {
        assert_eq!(
            Engraver::new(&Score::new(), PageSpec::default()).unwrap_err(),
            EngraveError::NoStaff
        );
        let mut score = one_staff_score();
        let part = score.parts[0].id;
        score.add_staff(part);
        let engraver = Engraver::new(&score, PageSpec::default()).unwrap();
        assert_eq!(engraver.atoms(0).len(), 1);
        assert_eq!(engraver.staffs().len(), 2);
        let page = engraver.page(0);
        assert_eq!(page.count(ItemKind::StaffLine), 10);
    }

    #[test]
    fn two_staves_keep_their_own_clef_and_share_segment_x() {
        let mut score = one_staff_score();
        let second_part = score.add_part(Part::new("Bass", "B."));
        let second_staff = score.add_staff(second_part);
        let track = Track::new(second_staff, 0);
        for segment in &mut score.measures[0].segments {
            let copy = segment
                .iter_elements()
                .find(|(source, _)| source.voice() == 0)
                .map(|(_, item)| item.clone());
            if let Some(SegmentItem::ChordRest(_)) = &copy {
                segment.set_element(track, copy.unwrap());
            }
        }
        // A bass clef on the second staff: its written notes must sit on a
        // different part of the staff than the treble notes of the first.
        score.measures[0].segments[0].set_element(track, SegmentItem::Clef(Clef::new(ClefType::F)));
        let engraver = Engraver::from_score(&score).unwrap();
        assert_eq!(engraver.staffs().len(), 2);
        assert_eq!(engraver.atoms(0).len(), 1);
        assert_eq!(engraver.atoms(1).len(), 1);
        let page = engraver.page(0);
        assert_eq!(page.count(ItemKind::Notehead), 8);
        assert_eq!(page.count(ItemKind::StaffLine), 10);
        let heads: Vec<(i32, i32)> = page
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Notehead)
            .map(|item| item.origin())
            .collect();
        // Four notes on each staff, aligned on the same segment x.
        assert_eq!(heads.len(), 8);
        for (treble, bass) in heads[..4].iter().zip(&heads[4..]) {
            assert_eq!(treble.0, bass.0);
        }
    }

    #[test]
    fn first_measure_clef_sets_the_head_vertical_origin() {
        // A clef on the first tick is the staff's clef, even when the
        // part's `default_clef` is still G (a 3.x piano's left hand).
        let mut score = one_staff_score();
        score.measures[0].segments[0].set_element(
            Track::from_raw(0),
            SegmentItem::Clef(Clef::new(ClefType::F)),
        );
        let clef_name = |page: &crate::display::DisplayList| {
            let glyph = page
                .items
                .iter()
                .find_map(|item| match item.geometry {
                    crate::display::Geometry::Glyph { glyph, .. }
                        if item.kind == ItemKind::Clef =>
                    {
                        Some(glyph)
                    }
                    _ => None,
                })
                .expect("a clef");
            scoretk_smufl::GLYPHS[usize::from(glyph)].name
        };
        let first_head_y = |page: &crate::display::DisplayList| {
            page.items
                .iter()
                .find(|item| item.kind == ItemKind::Notehead)
                .map(|item| item.origin().1)
                .expect("a notehead")
        };
        let treble = Engraver::new(&one_staff_score(), PageSpec::default())
            .unwrap()
            .page(0);
        let bass = Engraver::new(&score, PageSpec::default()).unwrap().page(0);
        assert_eq!(clef_name(&treble), "gClef");
        assert_eq!(clef_name(&bass), "fClef");
        // The same written G sits on the treble staff's middle line but
        // three ledger lines above the bass staff.
        assert!(first_head_y(&bass) < first_head_y(&treble));
    }

    #[test]
    fn different_rhythms_share_segment_and_barline_x() {
        // A Segment is shared by all staves of a system: one x per segment,
        // its width the maximum over the staves (`horizontalspacing.cpp:
        // 47-89`, `:262-327`). Give the second staff two half notes where the
        // first one has four quarters; their segment x and bar line x must
        // still agree.
        let mut score = one_staff_score();
        let second_part = score.add_part(Part::new("Bass", "B."));
        let second_staff = score.add_staff(second_part);
        let track = Track::new(second_staff, 0);
        for (segment_index, pitch) in [(3, 67), (5, 67)] {
            let note = score.add_note(Note::new(pitch, 14));
            let mut chord = Chord::default();
            chord.notes.push(note);
            let mut rest = ChordRest::chord(Duration::half());
            rest.kind = scoretk_core::model::ChordRestKind::Chord(chord);
            score.measures[0].segments[segment_index]
                .set_element(track, SegmentItem::ChordRest(rest));
        }
        let engraver = Engraver::from_score(&score).unwrap();
        assert_eq!(engraver.staffs().len(), 2);
        let page = engraver.page(0);
        let bar_x: Vec<(i32, i32)> = page
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::BarLine)
            .map(|item| item.origin())
            .collect();
        assert!(bar_x.len() >= 2, "each staff must end with a bar line");
        // A final bar line draws a thin stroke and a thick one one
        // `barlineSeparation` apart; both staves draw both at the same x.
        let mut xs: Vec<i32> = bar_x.iter().map(|bar| bar.0).collect();
        xs.sort_unstable();
        assert_eq!(xs.len(), 4, "two strokes on each of two staves: {bar_x:?}");
        assert_eq!(xs[0], xs[1], "thin strokes must share x: {bar_x:?}");
        assert_eq!(xs[2], xs[3], "thick strokes must share x: {bar_x:?}");
        let heads: Vec<(i32, i32)> = page
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Notehead)
            .map(|item| item.origin())
            .collect();
        assert_eq!(heads.len(), 6); // four quarters, two halves
        let bass_y = heads.iter().map(|head| head.1).max().unwrap();
        let bass: Vec<i32> = heads
            .iter()
            .filter(|head| head.1 == bass_y)
            .map(|head| head.0)
            .collect();
        let treble: Vec<i32> = heads
            .iter()
            .filter(|head| head.1 != bass_y)
            .map(|head| head.0)
            .collect();
        assert_eq!(bass.len(), 2, "heads={heads:?}");
        assert_eq!(treble.len(), 4);
        assert_eq!(bass[0], treble[0], "tick 0 must share its x");
        assert_eq!(bass[1], treble[2], "tick 1/2 must share its x");
    }
}
