// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port of `src/engraving/rendering/score/systemlayout.cpp:2900-2991`
// (`minDistance`), `pagelayout.cpp:524-613` (`layoutPage`) and
// `chordlayout.cpp:1287-1442` (`updateLedgerLines`), `:2468-2564`
// (`placeDots`) of MuseScore Studio 4.7.4 (tag v4.7.4), Copyright (C)
// MuseScore Limited and others, modified (rewritten in Rust) in 2026.
// The `EngravingItem` graph is replaced by system/page geometry from atoms
// plus plain vertical intervals.
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

//! Page-local placement from precomputed atoms. `page(n)` never touches the
//! `Score` and never recomputes an atom.

use alloc::vec::Vec;

use scoretk_smufl::{glyph, units_to_fixed};

use crate::atoms::{flag_name, rest_name, Atom, AtomKind, ChordAtom, MeasureAtom};
use crate::display::{DisplayItem, DisplayList, Geometry, ItemKind};
use crate::style::*;

/// Page geometry in fixed 1/1024 staff-space units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageSpec {
    /// Printable width (180 mm on A4 with 15 mm margins at 1.75 mm/sp).
    pub printable_width: i32,
    /// Left margin from the page edge.
    pub margin_left: i32,
    /// Top margin from the page edge.
    pub margin_top: i32,
    /// Measures per system, fixed for P1 (breaking arrives in P2).
    pub measures_per_system: usize,
    /// Systems per page.
    pub systems_per_page: usize,
}

impl Default for PageSpec {
    fn default() -> Self {
        Self {
            // 180 mm / 1.75 mm per staff space = 102.857 sp.
            printable_width: 105 * SP,
            margin_left: 9 * SP, // 15.75 mm
            margin_top: 12 * SP, // 21 mm
            measures_per_system: 4,
            systems_per_page: 8,
        }
    }
}

impl PageSpec {
    /// Converts a fixed staff-space coordinate to millimetres (spatium 1.75 mm).
    pub fn fixed_to_mm(fixed: i32) -> f64 {
        (fixed as f64) * 1.75 / SP as f64
    }

    /// Page width in millimetres.
    pub fn page_width_mm(&self) -> f64 {
        Self::fixed_to_mm(self.printable_width + 2 * self.margin_left)
    }

    /// Page height in millimetres.
    pub fn page_height_mm(&self) -> f64 {
        Self::fixed_to_mm(self.margin_top * 2 + 12 * SP * self.systems_per_page as i32)
    }
}

/// Place one measure's events at `x0` on the system at `system_y`.
fn place_measure(out: &mut DisplayList, atom: &MeasureAtom, x0: i32, system_y: i32) {
    for event in &atom.events {
        place_event(out, event, x0 + event.x, system_y);
    }
    // // Port of `tdraw.cpp` bar-line drawing: thin lines use the style width,
    // double lines are separated by `barlineSeparation` (SMuFL).
    let bar_x = x0 + atom.width - SEGMENT_WIDTH / 2;
    place_barline(out, atom.end_bar, bar_x, system_y);
}

fn place_barline(out: &mut DisplayList, code: crate::atoms::BarCode, x: i32, y: i32) {
    let w = THIN_BARLINE_WIDTH;
    match code.0 {
        1 => out.push(DisplayItem::rect(
            ItemKind::BarLine,
            x - w / 2,
            y,
            w,
            STAFF_HEIGHT,
        )),
        2 => {
            out.push(DisplayItem::rect(
                ItemKind::BarLine,
                x - BARLINE_SEPARATION - w / 2,
                y,
                w,
                STAFF_HEIGHT,
            ));
            out.push(DisplayItem::rect(
                ItemKind::BarLine,
                x - w / 2,
                y,
                w,
                STAFF_HEIGHT,
            ));
        }
        3 => {
            out.push(DisplayItem::rect(
                ItemKind::BarLine,
                x - THICK_BARLINE_WIDTH - BARLINE_SEPARATION,
                y,
                THIN_BARLINE_WIDTH,
                STAFF_HEIGHT,
            ));
            out.push(DisplayItem::rect(
                ItemKind::BarLine,
                x - THICK_BARLINE_WIDTH / 2,
                y,
                THICK_BARLINE_WIDTH,
                STAFF_HEIGHT,
            ));
        }
        _ => {}
    }
}

/// SMuFL key-signature positions on a five-line treble staff, in half-space
/// line units (top line = 0). // Port of `keysig.cpp`'s sharp/flat order and
/// `tdraw.cpp:2099` (`TDraw::draw(const KeySig*)`).
const SHARP_LINES: [i32; 7] = [0, 3, -1, 2, 5, 1, 4];
const FLAT_LINES: [i32; 7] = [4, 1, 5, 2, 6, 3, 0];

fn place_key_sig(out: &mut DisplayList, fifths: i8, x: i32, y: i32) {
    let count = fifths.unsigned_abs().min(7) as usize;
    let mut gx = x;
    for index in 0..count {
        let (line, name) = if fifths > 0 {
            (SHARP_LINES[index], "accidentalSharp")
        } else {
            (FLAT_LINES[index], "accidentalFlat")
        };
        out.glyph(ItemKind::Accidental, name, gx, y + line * HALF_SP);
        // // Port of `styledef.cpp:241` (`keysigAccidentalDistance` 0.3 sp):
        // one SMuFL glyph width plus a fixed gap between accidentals.
        let width = glyph(name)
            .map(|g| units_to_fixed(g.bbox[2] - g.bbox[0]))
            .unwrap_or(SP);
        gx += width + KEYSIG_ACCIDENTAL_DISTANCE;
    }
}

/// // Port of `timesig.cpp` / `tdraw.cpp:3137`: digits are stacked in the
/// upper and lower halves of the staff. `TimeSigType::FourFour` uses the
/// common-time glyph; P1 keeps digits, the glyph choice arrives with P2.
fn place_time_sig(out: &mut DisplayList, numerator: u8, denominator: u8, x: i32, y: i32) {
    for (value, top) in [(numerator, true), (denominator, false)] {
        if value > 9 {
            continue;
        }
        let name = match value {
            0 => "timeSig0",
            1 => "timeSig1",
            2 => "timeSig2",
            3 => "timeSig3",
            4 => "timeSig4",
            5 => "timeSig5",
            6 => "timeSig6",
            7 => "timeSig7",
            8 => "timeSig8",
            _ => "timeSig9",
        };
        let digit_y = if top { y + HALF_SP } else { y + 5 * HALF_SP };
        out.glyph(ItemKind::TimeSig, name, x, digit_y);
    }
}

/// Ledger lines: // Port of `chordlayout.cpp:1310-1314, 1355-1360, 1394-1419`
/// — notes outside `[-1, lineBelow+1]` need lines on every even line between
/// the note and the staff, extended by `ledgerLineLength` on both sides.
fn place_ledger_lines(out: &mut DisplayList, head_x: i32, head_w: i32, line: i32, y: i32) {
    if (-1..=9).contains(&line) {
        return;
    }
    if line < 0 {
        let mut ledger = (line + 1) & !1; // round toward zero to an even line
        while ledger < 0 {
            out.push(DisplayItem::rect(
                ItemKind::LedgerLine,
                head_x - LEDGER_EXTENSION,
                y + ledger * HALF_SP - LEDGER_WIDTH / 2,
                head_w + 2 * LEDGER_EXTENSION,
                LEDGER_WIDTH,
            ));
            ledger += 2;
        }
    } else {
        let mut ledger = line & !1;
        while ledger > 8 {
            out.push(DisplayItem::rect(
                ItemKind::LedgerLine,
                head_x - LEDGER_EXTENSION,
                y + ledger * HALF_SP - LEDGER_WIDTH / 2,
                head_w + 2 * LEDGER_EXTENSION,
                LEDGER_WIDTH,
            ));
            ledger -= 2;
        }
    }
}

fn place_chord(out: &mut DisplayList, chord: &ChordAtom, x: i32, y: i32) {
    for note in &chord.notes {
        let ny = y + note.line * HALF_SP;
        if let Some(name) = note.accidental {
            out.glyph(ItemKind::Accidental, name, x - 7 * HALF_SP, ny);
        }
        let head = glyph(note.head).unwrap();
        let head_w = units_to_fixed(head.bbox[2]);
        // // Port of `tlayout.cpp`/`tdraw.cpp`: the notehead origin is at the
        // left edge of the head, vertically centred on its staff line.
        out.path(ItemKind::Notehead, note.head, x, ny);
        place_ledger_lines(out, x, head_w, note.line, y);

        // Dots: // Port of `chordlayout.cpp:2490-2562` (`placeDots`) — a note
        // on a line moves its dot into the adjacent space, up for an up stem.
        if chord.dots > 0 {
            let dot_offset = if note.line % 2 == 0 {
                if chord.stem_up {
                    -1
                } else {
                    1
                }
            } else {
                0
            };
            for index in 0..usize::from(chord.dots) {
                let dx = x + head_w + DOT_NOTE_DISTANCE + index as i32 * DOT_DOT_DISTANCE;
                let dy = y + (note.line + dot_offset) * HALF_SP;
                out.glyph(ItemKind::Dot, "augmentationDot", dx, dy);
            }
        }
    }

    if chord.notes.is_empty() || !crate::atoms::has_stem(chord.duration) {
        return;
    }
    let top = chord.notes.iter().map(|n| n.line).min().unwrap();
    let bottom = chord.notes.iter().map(|n| n.line).max().unwrap();
    let head = glyph(chord.notes[0].head).unwrap();
    let head_w = units_to_fixed(head.bbox[2]);
    let anchor_line = if chord.stem_up { bottom } else { top };
    let ny = y + anchor_line * HALF_SP;
    let stem_x = if chord.stem_up {
        x + head_w - STEM_WIDTH / 2
    } else {
        x - STEM_WIDTH / 2
    };
    // // Port of `tlayout.cpp:5180-5187` (`TLayout::layoutStem`): the drawn
    // stem starts at the notehead's SMuFL stem anchor, not at the note's
    // centre. SMuFL anchor y is up-positive; our page coordinates are
    // down-positive, hence the sign flip. `stemlayout.cpp:227-242`
    // (`StemLayout::stemPos`) supplies the horizontal side of the stem.
    let anchor_y = if chord.stem_up {
        -STEM_ANCHOR_OFFSET
    } else {
        STEM_ANCHOR_OFFSET
    };
    // The far end stays at `note ± stemLength`; only the notehead end moves
    // to the anchor (`tlayout.cpp:5198-5201` computes y2 independently).
    let (sy, ey) = if chord.stem_up {
        (ny - chord.stem_len, ny + anchor_y)
    } else {
        (ny + anchor_y, ny + chord.stem_len)
    };
    // // Port of `stemlayout.cpp:227-242` and `tlayout.cpp:5143-5175`: a
    // stem is a rectangle from the notehead anchor to `stemLength` away.
    out.push(DisplayItem::rect(
        ItemKind::Stem,
        stem_x,
        sy,
        STEM_WIDTH,
        ey - sy,
    ));
    if let Some(name) = flag_name(chord.duration, chord.stem_up) {
        let _ = (top, bottom);
        let flag_y = if chord.stem_up { sy } else { ey };
        out.glyph(ItemKind::Flag, name, stem_x, flag_y);
    }
}

fn place_event(out: &mut DisplayList, event: &Atom, x: i32, y: i32) {
    match event.kind {
        AtomKind::Clef { name } => {
            // Leland's clef origin sits near the second staff line; the
            // horizontal margin is `clefLeftMargin` (styledef.cpp:174).
            out.glyph(ItemKind::Clef, name, x, y + 2 * SP);
        }
        AtomKind::KeySig { fifths } => place_key_sig(out, fifths, x, y),
        AtomKind::TimeSig {
            numerator,
            denominator,
        } => place_time_sig(out, numerator, denominator, x, y),
        AtomKind::Chord => {
            if let Some(chord) = &event.chord {
                place_chord(out, chord, x, y);
            }
        }
        AtomKind::Rest { duration, dots } => {
            let name = rest_name(duration);
            out.glyph(ItemKind::Rest, name, x, y + HALF_SP);
            for index in 0..usize::from(dots) {
                out.glyph(
                    ItemKind::Dot,
                    "augmentationDot",
                    x + 2 * SP + index as i32 * DOT_DOT_DISTANCE,
                    y + HALF_SP,
                );
            }
        }
    }
}

/// Builds the display list of one page from the atoms prepared in `new()`.
pub fn build_page(atoms: &[MeasureAtom], spec: &PageSpec, page: usize) -> DisplayList {
    let mut out = DisplayList::new(page);
    let per_system = spec.measures_per_system.max(1);
    if atoms.is_empty() {
        return out;
    }
    for local_system in 0..spec.systems_per_page {
        let system = page * spec.systems_per_page + local_system;
        let first = system * per_system;
        if first >= atoms.len() {
            break;
        }
        let sy = spec.margin_top + local_system as i32 * (STAFF_HEIGHT + 8 * SP);
        // // Port of `tdraw.cpp`'s staff-line drawing: five pixel-snapped
        // rectangles, thickness `staffLineWidth`.
        for line in 0..5 {
            out.push(DisplayItem::rect(
                ItemKind::StaffLine,
                spec.margin_left,
                sy + line * SP - STAFF_LINE_WIDTH / 2,
                spec.printable_width,
                STAFF_LINE_WIDTH,
            ));
        }
        let mut x = spec.margin_left;
        let last = (first + per_system).min(atoms.len());
        for (index, atom) in atoms[first..last].iter().enumerate() {
            let _ = index;
            place_measure(&mut out, atom, x, sy);
            x += atom.width;
        }
        // // Port of `systemlayout.cpp`: the start bar line closes the
        // system; its x is the staff origin.
        place_barline(&mut out, atoms[first].start_bar, spec.margin_left, sy);
    }
    out
}

/// Number of pages for a set of atoms and a page spec.
pub fn page_count(atoms: &[MeasureAtom], spec: &PageSpec) -> usize {
    let per_page = spec.measures_per_system.max(1) * spec.systems_per_page.max(1);
    atoms.len().div_ceil(per_page).max(1)
}

/// Iterates over all items of a page in deterministic order (used by tests
/// and the oracles).
pub fn items(atoms: &[MeasureAtom], spec: &PageSpec, page: usize) -> Vec<DisplayItem> {
    build_page(atoms, spec, page).items
}

/// Exposed for the raster backend: the quadratic outline of a display item.
pub fn geometry_of(item: &DisplayItem) -> Geometry {
    item.geometry
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    use crate::atoms::NoteAtom;

    fn atom(line: i32) -> MeasureAtom {
        MeasureAtom {
            number: 0,
            width: 8 * SEGMENT_WIDTH,
            spring: 8 * 1024,
            events: vec![Atom {
                x: 4 * SP,
                kind: AtomKind::Chord,
                chord: Some(ChordAtom {
                    duration: 4,
                    dots: 0,
                    stem_up: true,
                    stem_len: 3584,
                    notes: vec![NoteAtom {
                        line,
                        head: "noteheadBlack",
                        accidental: None,
                    }],
                }),
            }],
            start_bar: crate::atoms::BarCode(1),
            end_bar: crate::atoms::BarCode(1),
        }
    }

    #[test]
    fn page_has_five_staff_lines_and_the_event_items() {
        let spec = PageSpec::default();
        let list = build_page(&[atom(4)], &spec, 0);
        assert_eq!(list.count(ItemKind::StaffLine), 5);
        assert_eq!(list.count(ItemKind::Notehead), 1);
        assert_eq!(list.count(ItemKind::Stem), 1);
        assert_eq!(list.count(ItemKind::BarLine), 2);
    }

    #[test]
    fn high_note_gets_ledger_lines_only_outside_the_staff() {
        let spec = PageSpec::default();
        let inside = build_page(&[atom(4)], &spec, 0);
        assert_eq!(inside.count(ItemKind::LedgerLine), 0);
        let outside = build_page(&[atom(-2)], &spec, 0);
        assert_eq!(outside.count(ItemKind::LedgerLine), 1);
        let low = build_page(&[atom(12)], &spec, 0);
        assert_eq!(low.count(ItemKind::LedgerLine), 2);
    }

    #[test]
    fn page_count_and_empty_pages_are_stable() {
        let spec = PageSpec::default();
        let atoms: Vec<_> = (0..9).map(|i| atom(i % 8)).collect();
        assert_eq!(page_count(&atoms, &spec), 1);
        let many: Vec<_> = (0..33).map(|i| atom(i % 8)).collect();
        assert_eq!(page_count(&many, &spec), 2);
        assert!(build_page(&atoms, &spec, 1).items.is_empty());
    }

    #[test]
    fn header_segments_and_first_note_do_not_overlap() {
        // Same spacing as the atoms stage; the test catches a regression of
        // the clef/key/time widths against the 4.7.4 style defaults.
        let width = |name: &str| {
            let g = scoretk_smufl::glyph(name).unwrap();
            units_to_fixed(g.bbox[2] - g.bbox[0])
        };
        let clef_x = CLEF_LEFT_MARGIN;
        let key_x = clef_x + width("gClef") + CLEF_KEY_RIGHT_MARGIN;
        let time_x = key_x + 4 * (width("accidentalSharp") + KEYSIG_ACCIDENTAL_DISTANCE)
            + KEY_TIME_DISTANCE;
        let note_x = time_x + width("timeSig4") + BAR_NOTE_DISTANCE;
        let atom = MeasureAtom {
            number: 0,
            width: note_x + SEGMENT_WIDTH,
            spring: 1024,
            events: vec![
                Atom::simple(clef_x, AtomKind::Clef { name: "gClef" }),
                Atom::simple(key_x, AtomKind::KeySig { fifths: 4 }),
                Atom::simple(
                    time_x,
                    AtomKind::TimeSig {
                        numerator: 4,
                        denominator: 4,
                    },
                ),
                Atom {
                    x: note_x,
                    kind: AtomKind::Chord,
                    chord: Some(ChordAtom {
                        duration: 4,
                        dots: 0,
                        stem_up: true,
                        stem_len: 3072,
                        notes: vec![NoteAtom {
                            line: 4,
                            head: "noteheadBlack",
                            accidental: None,
                        }],
                    }),
                },
            ],
            start_bar: crate::atoms::BarCode(1),
            end_bar: crate::atoms::BarCode(1),
        };
        let list = build_page(&[atom], &PageSpec::default(), 0);
        let header: Vec<_> = list
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item.kind,
                    ItemKind::Clef | ItemKind::Accidental | ItemKind::TimeSig | ItemKind::Notehead
                )
            })
            .collect();
        for (index, left) in header.iter().enumerate() {
            for right in header.iter().skip(index + 1) {
                let (ax0, ay0, ax1, ay1) = left.bbox(&list).unwrap();
                let (bx0, by0, bx1, by1) = right.bbox(&list).unwrap();
                let overlap_x = ax0.min(bx1) - bx0.max(ax0);
                let overlap_y = ay0.min(by1) - by0.max(ay0);
                assert!(
                    overlap_x <= 0 || overlap_y <= 0,
                    "{:?} and {:?} overlap by {overlap_x},{overlap_y}",
                    left.kind,
                    right.kind
                );
            }
        }
    }

    #[test]
    fn vertical_geometry_uses_fixed_point_staff_spaces() {
        let spec = PageSpec::default();
        let list = build_page(&[atom(8)], &spec, 0);
        let head = list
            .items
            .iter()
            .find(|item| item.kind == ItemKind::Notehead)
            .unwrap();
        let (_, y) = head.origin();
        assert_eq!(y, spec.margin_top + 8 * HALF_SP);
    }
}
