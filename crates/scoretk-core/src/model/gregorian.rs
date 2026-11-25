// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de la syntaxe GABC de Gregorio (GPL-3.0-or-later) : `doc/Gabc.tex`,
// `src/gabc/gabc-score-determination.{y,l}` et `src/struct.h`, modifié
// (réécrit en Rust) en 2026. Chaque règle porte sa référence de section.
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

//! L6 payload for Gregorian chant (GABC).
//!
//! The syllable/neume stream is the **primary** data of GABC: the modern
//! projection into L0/L1 (one `Note` per pitch letter, an invented duration
//! and nominal measures) is derived and flagged as such
//! (`docs/agents/model-modularisation.md` §7.1). GABC has no metre, no measure
//! numbering and no fixed durations (`Gabc.tex:801-845`): separation bars
//! divide *sections*, not metric measures.
//!
//! This module is pure data: no I/O, no clock, no dependency, so the core
//! stays portable to `wasm32-unknown-unknown`.

use super::{
    BarLineType, Chord, ChordRest, ChordRestKind, Clef, ClefType, Duration, Fraction, Layers,
    Lyrics, LyricsSyllabic, Measure, Note, Part, Score, Segment, SegmentItem, SegmentType, Track,
};

/// One header field, in source order (`Gabc.tex:67-154`).
///
/// Multi-line values keep their embedded newlines; the terminating `;;`
/// (`:69-74`) is not part of the value.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct GregorianHeader {
    /// Header name (`name`, `language`, `mode`, `office-part`, …).
    pub name: String,
    /// Header value as written between `:` and the final `;`.
    pub value: String,
}

/// A separation bar between sections (`Gabc.tex:801-821`).
///
/// `::` is the divisio finalis, not two divisio maiores (`:815-817`,
/// `src/dump/dump.c:805-821`): the variants are therefore parsed longest
/// first by [`BarKind::parse_prefix`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BarKind {
    /// Virgula `` ` `` (`:808`).
    Virgula,
    /// Virgula on the ledger line above the staff `` `0 `` (`:809`).
    VirgulaLedger,
    /// Divisio minimis `^` / `^0` (`:810-811`).
    DivisioMinimis,
    /// Divisio minima `,` (`:812`).
    DivisioMinima,
    /// Divisio minima on the ledger line above `,0` (`:813`).
    DivisioMinimaLedger,
    /// Divisio minor `;` (`:814`).
    DivisioMinor,
    /// Divisio maior `:` (`:815`).
    DivisioMaior,
    /// Dotted divisio maior `:?` (`:816`).
    DivisioMaiorDot,
    /// Divisio finalis `::` (`:817`).
    DivisioFinalis,
    /// Dominican bar `;1` … `;8` (`:818-820`).
    Dominican(u8),
}

impl BarKind {
    /// Parses one bar at the beginning of `code`, returning the bar and the
    /// number of source characters it occupies.
    ///
    /// Two-character forms are tried before one-character forms so that `::`
    /// is one final bar and `;1` one Dominican bar (`Gabc.tex:815-820`).
    /// The optional `'` (vertical episema) and `_` (bar brace) decorations
    /// (`:823-830`) are handled by the caller, not by the bar token itself.
    pub fn parse_prefix(code: &str) -> Option<(Self, usize)> {
        if code.starts_with("::") {
            return Some((Self::DivisioFinalis, 2));
        }
        if code.starts_with(":?") {
            return Some((Self::DivisioMaiorDot, 2));
        }
        if code.starts_with("`0") {
            return Some((Self::VirgulaLedger, 2));
        }
        if code.starts_with(",0") {
            return Some((Self::DivisioMinimaLedger, 2));
        }
        if code.starts_with("^0") {
            return Some((Self::DivisioMinimis, 2));
        }
        let bytes = code.as_bytes();
        // Gregorio's lexer accepts `[,;][1-8]` as a Dominican bar
        // (`src/gabc/gabc-notes-determination.l:1246-1248`), although
        // `Gabc.tex:818-820` documents only `;n`.
        if bytes.len() >= 2 && matches!(bytes[0], b';' | b',') && (b'1'..=b'8').contains(&bytes[1])
        {
            return Some((Self::Dominican(bytes[1] - b'0'), 2));
        }
        match bytes.first().copied()? {
            b'`' => Some((Self::Virgula, 1)),
            b'^' => Some((Self::DivisioMinimis, 1)),
            b',' => Some((Self::DivisioMinima, 1)),
            b';' => Some((Self::DivisioMinor, 1)),
            b':' => Some((Self::DivisioMaior, 1)),
            _ => None,
        }
    }

    /// Canonical GABC spelling of the bar, used as the inventory value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Virgula => "`",
            Self::VirgulaLedger => "`0",
            Self::DivisioMinimis => "^",
            Self::DivisioMinima => ",",
            Self::DivisioMinimaLedger => ",0",
            Self::DivisioMinor => ";",
            Self::DivisioMaior => ":",
            Self::DivisioMaiorDot => ":?",
            Self::DivisioFinalis => "::",
            Self::Dominican(1) => ";1",
            Self::Dominican(2) => ";2",
            Self::Dominican(3) => ";3",
            Self::Dominican(4) => ";4",
            Self::Dominican(5) => ";5",
            Self::Dominican(6) => ";6",
            Self::Dominican(7) => ";7",
            Self::Dominican(8) => ";8",
            Self::Dominican(_) => ";",
        }
    }
}

/// Family of a symbol appended to a note or a bar (`Gabc.tex:702-797`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SignKind {
    /// Punctum mora `.` / `.0` / `.1` / `..` (`:742-755`).
    PunctumMora,
    /// Vertical episema / ictus `'` / `'0` / `'1` (`:757-768`).
    Ictus,
    /// Horizontal episema `_` with numeric modifiers (`:769-797`).
    HorizontalEpisema,
    /// Oriscus `o` / `O` (`:469,567`).
    Oriscus,
    /// Quilisma `w` (`:470`).
    Quilisma,
    /// Virga `v` / virga reversa `V` (`:471-472`).
    Virga,
    /// Stropha `s` (`:473`).
    Stropha,
    /// Liquescent deminutus `~`, augmented `<`, diminished `>` (`:474-476`).
    Liquescentia,
    /// Linea `=` (`:477`).
    Linea,
    /// Cavum `r` / `r0` (`:478-480`).
    Cavum,
    /// Accent, circle or musica ficta `r1` … `r8` (`:717-724`).
    Accent,
    /// Flat `x` / `X`, sharp `#` / `##`, natural `y` / `Y` (`:481-489`).
    Alteration,
    /// Any other shape character, kept as raw glyph (`:556-579`).
    Other,
}

/// One neume sign attached to a note or a bar.
///
/// The `raw` spelling is kept: placement digits and fusion are part of the
/// source and a lossy projection must be able to report them (`§7.1` of the
/// modularisation plan).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeumeSign {
    /// Semantic family, for reporting and inventory.
    pub kind: SignKind,
    /// Exact source characters of the sign.
    pub raw: String,
}

impl NeumeSign {
    /// Builds a sign from a family and its source spelling.
    pub fn new(kind: SignKind, raw: impl Into<String>) -> Self {
        Self {
            kind,
            raw: raw.into(),
        }
    }
}

/// One pitch letter of a neume (`Gabc.tex:408-443`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GregorianNote {
    /// Typographical staff position `a`–`n` or `p`, **not** a concert pitch
    /// name (`:410-413`); upper case is a punctum inclinatum (`:454-468`).
    pub letter: char,
    /// Neume glyphs appended to the letter (`:452-579,603-700`), including
    /// shape hints and fusion markers. Empty for a plain punctum.
    pub glyph: String,
    /// Rhythmic and shape signs attached to this note (`:702-797`).
    pub signs: Vec<NeumeSign>,
}

/// One syllable and the neumes it carries (`Gabc.tex:228-317`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Syllable {
    /// Text outside parentheses, markup included (`:241-245,254-309`).
    pub text: String,
    /// Clef token opening this group, when one occurs here
    /// (`:847-870`). `Gregorian::clefs` keeps the source-order list; this
    /// per-syllable slot is what lets the writer place a *clef change* at the
    /// right point instead of inventing one.
    pub clef: Option<String>,
    /// Special-character markup such as `<sp>R/</sp>` (`:297-309,345-378`).
    pub rubric: Option<String>,
    /// Pitch letters of the parenthesised groups, in source order.
    pub notes: Vec<GregorianNote>,
    /// Signs attached to the groups rather than to one note.
    pub signs: Vec<NeumeSign>,
    /// Separation bars found in the groups (`:801-845`).
    pub bars: Vec<BarKind>,
    /// A space or an end of line followed the last `)` (`:60-61,241-245`).
    ///
    /// Spaces are music in GABC: this flag is part of the primary data even
    /// though the derived modern view ignores it.
    pub space_after: bool,
}

/// L6 payload of a GABC score.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Gregorian {
    /// Header fields in source order (`Gabc.tex:67-154`).
    pub headers: Vec<GregorianHeader>,
    /// Number of staff lines, 2–5, default 4 (`:128,410-443`).
    pub staff_lines: u8,
    /// Clef tokens in source order, e.g. `c3`, `f4`, `cb3`, `c1@c4`
    /// (`:847-870`).
    pub clefs: Vec<String>,
    /// Primary data: syllables, their neumes and their separation bars.
    ///
    /// This is the **only** source the writer uses. No raw header or notation
    /// text is kept: every emitted token is rebuilt from these fields (an
    /// atomic GABC spelling such as a sign's `raw` or a clef token is the one
    /// deliberate exception, justified in `docs/agents/gabc.md` §8).
    pub syllables: Vec<Syllable>,
    /// True when the L0/L1 event spine was generated from this payload.
    ///
    /// GABC carries neither metrical measures nor fixed durations
    /// (`:801-845`); one `Note` per pitch letter and nominal measures are a
    /// *derived projection*, never source data (plan §7.1).
    pub derived_modern_view: bool,
}

impl Gregorian {
    /// Rebuilds the derived modern-notation view (L0/L1) from this payload.
    ///
    /// GABC has no metre, measure numbering or fixed durations
    /// (`Gabc.tex:801-845`); the one-quarter-per-pitch-letter spine is a
    /// deliberate projection, not source data. The core owns this projection
    /// so both the GABC reader and the MusicXML reader can rebuild the *same*
    /// view when they carry the L6 payload. It is flagged in `Score::report`
    /// and in [`Gregorian::derived_modern_view`].
    ///
    /// Portage de `src/struct.c:1273-1288` (`initial_key`) et
    /// `src/struct.h:809-813` (hauteurs), modifié (réécrit en Rust) en 2026.
    #[must_use]
    pub fn build_derived_score(&self) -> Score {
        let mut score = Score::new();
        let clef = self.clefs.first().map_or("c4", String::as_str);
        let part = score.add_part(Part::new("Gregorian", "Gre"));
        let staff = score.add_staff(part);
        score.staves[staff.index()].lines = self.staff_lines;
        let track = Track::new(staff, 0);

        let mut start = Fraction::ZERO;
        for (index, syllable) in self.syllables.iter().enumerate() {
            let count = syllable.notes.len();
            // Even a bar-only group occupies one quarter: a zero-length
            // measure would put the next syllable at the same absolute tick.
            let len = Fraction::new(count.max(1) as i64, 4);
            let mut measure = Measure::new(index, start, Fraction::ONE).with_len(len);
            if let Some(bar) = syllable.bars.last() {
                measure.end_bar_line = derived_bar_line(*bar);
            }
            for (note_index, note) in syllable.notes.iter().enumerate() {
                let step = derived_step(note.letter, clef);
                let id = score.add_note(Note::new(derived_pitch(step), derived_tpc(step)));
                let mut chord_rest = ChordRest::chord(Duration::quarter());
                chord_rest.kind = ChordRestKind::Chord(Chord {
                    notes: vec![id],
                    ..Chord::default()
                });
                // Segment ticks are absolute in the pivot (`pass2.rs:495-518`),
                // not relative to the measure.
                let tick = start + Fraction::new(note_index as i64, 4);
                if note_index == 0 {
                    let text = plain_lyric(&syllable.text);
                    if !text.is_empty() {
                        chord_rest
                            .lyrics
                            .push(Lyrics::new(0, LyricsSyllabic::Single, text));
                    }
                }
                let mut segment = Segment::new(SegmentType::ChordRest, tick, 1);
                segment.set_element(track, SegmentItem::ChordRest(chord_rest));
                measure.add_segment(segment);
            }
            if index == 0 {
                let mut segment = Segment::new(SegmentType::Clef, Fraction::ZERO, 1);
                segment.set_element(track, SegmentItem::Clef(Clef::new(derived_clef_type(clef))));
                measure.add_segment(segment);
            }
            start = measure.end_tick();
            score.add_measure(measure);
        }

        score.report.record(
            Layers::COMMON,
            "derived-modern-view: one note per pitch letter, quarter duration, nominal 4/4",
            None,
            None,
        );
        score
    }
}

/// Removes GABC typography to get the sung word
/// (`Gabc.tex:247-309,314-343`).
fn plain_lyric(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut stack: Vec<String> = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '<' => {
                if let Some(end) = chars[index..].iter().position(|char| *char == '>') {
                    let end = index + end;
                    let tag: String = chars[index + 1..end].iter().collect();
                    let name = tag
                        .trim_start_matches('/')
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if tag.starts_with('/') {
                        stack.pop();
                    } else {
                        stack.push(name);
                    }
                    index = end + 1;
                    continue;
                }
                index += 1;
            }
            '$' => {
                index += 1;
                if index < chars.len() {
                    out.push(chars[index]);
                    index += 1;
                }
            }
            '{' | '}' => index += 1,
            char => {
                if stack.iter().all(|tag| {
                    matches!(
                        tag.as_str(),
                        "b" | "c" | "i" | "sc" | "tt" | "ul" | "e" | "eu" | "nlba"
                    )
                }) {
                    out.push(char);
                }
                index += 1;
            }
        }
    }
    out.trim().to_owned()
}

/// Clef key of a GABC token (`src/struct.c:1273-1288`).
fn derived_clef_key(token: &str) -> i32 {
    let first = token.split('@').next().unwrap_or(token);
    let line = i32::from(
        first
            .bytes()
            .rev()
            .find(|byte| byte.is_ascii_digit())
            .unwrap_or(b'4')
            - b'0',
    );
    if first.starts_with('f') {
        2 * line - 4
    } else {
        2 * line - 1
    }
}

/// Staff height of a letter, `a` = 3 (`src/struct.h:809-813`; `dump.c:78-88`).
fn derived_height(letter: char) -> i32 {
    let low = letter.to_ascii_lowercase();
    let base = (low as i32) - ('a' as i32) + 3;
    if low > 'o' {
        base + 1
    } else {
        base
    }
}

/// Diatonic step (C = 0) of a pitch letter under a clef.
fn derived_step(letter: char, clef: &str) -> i32 {
    derived_height(letter) - derived_clef_key(clef) - 5
}

/// Concert MIDI pitch of a derived step (step 0 = middle C).
fn derived_pitch(step: i32) -> u8 {
    const SEMITONES: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
    (60 + SEMITONES[step.rem_euclid(7) as usize] + 12 * step.div_euclid(7)).clamp(0, 127) as u8
}

/// Tonal pitch class of a derived step (C = 14, D = 16, …).
fn derived_tpc(step: i32) -> i8 {
    const TPC: [i8; 7] = [14, 16, 18, 13, 15, 17, 19];
    TPC[step.rem_euclid(7) as usize]
}

/// Closest modern bar line to a separation bar
/// (`Gabc.tex:801-845`); the rich vocabulary stays in the payload.
fn derived_bar_line(bar: BarKind) -> BarLineType {
    match bar {
        BarKind::DivisioFinalis => BarLineType::End,
        BarKind::DivisioMaior | BarKind::DivisioMaiorDot => BarLineType::Double,
        BarKind::Dominican(_) => BarLineType::Dotted,
        _ => BarLineType::Normal,
    }
}

/// Closest modern clef to a GABC clef token (`Gabc.tex:847-870`).
fn derived_clef_type(token: &str) -> ClefType {
    let first = token.split('@').next().unwrap_or(token);
    let line = first
        .bytes()
        .find(|byte| byte.is_ascii_digit())
        .unwrap_or(b'4')
        - b'0';
    if first.starts_with('f') {
        match line {
            3 => ClefType::FBaritone,
            5 => ClefType::FSubbass,
            _ => ClefType::F,
        }
    } else {
        match line {
            1 => ClefType::C1,
            2 => ClefType::C2,
            3 => ClefType::C3,
            5 => ClefType::C5,
            _ => ClefType::C4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typography_is_not_a_lyric() {
        assert_eq!(plain_lyric("<sc>tus</sc>"), "tus");
        assert_eq!(plain_lyric("{DE}us"), "DEus");
        assert_eq!(plain_lyric("$(a)"), "(a)");
        assert_eq!(plain_lyric("<sp>V/</sp>."), ".");
    }

    #[test]
    fn derived_view_marks_itself_as_a_projection() {
        let payload = Gregorian {
            clefs: vec!["c3".to_owned()],
            ..Gregorian::default()
        };
        let score = payload.build_derived_score();
        assert!(score.report.losses.iter().any(|loss| {
            loss.layer == Layers::COMMON && loss.item.contains("derived-modern-view")
        }));
    }

    #[test]
    fn finalis_is_one_bar_not_two_maiores() {
        assert_eq!(
            BarKind::parse_prefix("::"),
            Some((BarKind::DivisioFinalis, 2))
        );
        assert_eq!(
            BarKind::parse_prefix(":?"),
            Some((BarKind::DivisioMaiorDot, 2))
        );
        assert_eq!(BarKind::parse_prefix(":"), Some((BarKind::DivisioMaior, 1)));
    }

    #[test]
    fn bars_cover_the_specified_vocabulary() {
        for (token, expected) in [
            ("`", BarKind::Virgula),
            ("`0", BarKind::VirgulaLedger),
            ("^", BarKind::DivisioMinimis),
            ("^0", BarKind::DivisioMinimis),
            (",", BarKind::DivisioMinima),
            (",0", BarKind::DivisioMinimaLedger),
            (";", BarKind::DivisioMinor),
            (":", BarKind::DivisioMaior),
            ("::", BarKind::DivisioFinalis),
            (";1", BarKind::Dominican(1)),
            (";8", BarKind::Dominican(8)),
        ] {
            let (kind, used) = BarKind::parse_prefix(token).expect("a specified bar");
            assert_eq!(kind, expected, "{token}");
            assert_eq!(used, token.len(), "{token}");
        }
        // Out-of-range Dominican digits fall back to a divisio minor plus
        // an unparsed trailing character, as in the source grammar.
        assert_eq!(
            BarKind::parse_prefix(",1"),
            Some((BarKind::Dominican(1), 2))
        );
        assert_eq!(
            BarKind::parse_prefix(";3"),
            Some((BarKind::Dominican(3), 2))
        );
        assert_eq!(
            BarKind::parse_prefix(";9"),
            Some((BarKind::DivisioMinor, 1))
        );
        assert_eq!(BarKind::parse_prefix("g"), None);
    }

    #[test]
    fn payload_stays_an_in_memory_value() {
        let mut gregorian = Gregorian::default();
        assert!(!gregorian.derived_modern_view);
        gregorian.headers.push(GregorianHeader {
            name: "name".to_owned(),
            value: "Veni Sancte Spiritus".to_owned(),
        });
        gregorian.syllables.push(Syllable {
            text: "DE(g)us".to_owned(),
            clef: None,
            rubric: Some("<sp>R/</sp>".to_owned()),
            notes: vec![GregorianNote {
                letter: 'g',
                glyph: "h_".to_owned(),
                signs: vec![NeumeSign::new(SignKind::HorizontalEpisema, "_")],
            }],
            signs: Vec::new(),
            bars: vec![BarKind::DivisioFinalis],
            space_after: true,
        });
        assert_eq!(gregorian.headers[0].value, "Veni Sancte Spiritus");
        assert_eq!(gregorian.syllables[0].notes[0].letter, 'g');
    }
}
