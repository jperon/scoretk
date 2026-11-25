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
        if bytes.len() >= 2 && bytes[0] == b';' && (b'1'..=b'8').contains(&bytes[1]) {
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
#[derive(Clone, Debug, PartialEq, Eq)]
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
    pub syllables: Vec<Syllable>,
    /// Exact header section source, so the writer can emit it verbatim.
    pub header_source: String,
    /// Exact notation section source, so the writer can emit it verbatim.
    ///
    /// The modularisation plan (§7.1) requires the export to write the L6
    /// payload back verbatim when present; the structured fields above are
    /// what the inventory and the loss report measure.
    pub notation: String,
    /// True when the L0/L1 event spine was generated from this payload.
    ///
    /// GABC carries neither metrical measures nor fixed durations
    /// (`:801-845`); one `Note` per pitch letter and nominal measures are a
    /// *derived projection*, never source data (plan §7.1).
    pub derived_modern_view: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

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
