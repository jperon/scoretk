#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon
#
# This program is free software: you can redistribute it and/or modify it
# under the terms of the GNU General Public License version 3 as published by
# the Free Software Foundation.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
# FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
# more details.
#
# You should have received a copy of the GNU General Public License along with
# this program. If not, see the LICENSE file at the root of this repository or
# <https://www.gnu.org/licenses/gpl-3.0.html>.

"""Canonical GABC truth: pitch with clef, syllables, neumes, bars, signs.

MusicXML and MIDI are **not** the truth for GABC. Separation bars divide
*sections*, not metrical measures: Gregorio's syntax defines no time
signature, no measure numbering and no fixed note duration
(`doc/Gabc.tex:742-845`); the only rhythmic information is neume shape,
punctum mora, episemata and bar weight. What the format actually carries is:

1. a sequence of pitch positions **resolved by the clef**;
2. the syllable text each neume is attached to;
3. the grouping of pitches into neumes;
4. the separation bars (``,``, ``;``, ``:``, ``::`` …);
5. rhythmic signs (punctum mora, ictus, episema, quilisma, liquescents) —
   compared as an optional second level.

The diatonic step index follows Gregorio's own source: staff heights start
at ``LOWEST_PITCH = 3`` (`src/struct.h:809`), the letter ``o`` is skipped
(`src/dump/dump.c:78-88`), and a clef's key is ``2*line-1`` (C) or ``2*line-4``
(F) (`src/struct.c:1273-1288`). With these, ``step = height - key - 5`` gives
C = 0 for both C and F clefs (verified on Gregorio's dump line
``initial_key 5 (c3)`` and on ``f3``). The absolute origin is arbitrary: only
equality between references matters.

Two readers of the same canonical form are provided:

* :func:`parse_source` — our own reader of the original ``.gabc``;
* :func:`parse_dump` — Gregorio's own parser, via ``gregorio -F dump``, the
  reference implementation of the format.

Only downloaded *text* is parsed here; no downloaded script is imported.
"""

from __future__ import annotations

import re
from collections import Counter

LETTERS = "abcdefghijklmnp"
PITCH = re.compile(r"[a-npA-NP]")
CLEF = re.compile(r"[cf]b?[1-9]")
BAR = re.compile(r"::|:[?]?|;1?|\^\d*|`0?|,0?|[`;:]")
SIGNS = set("'_ .w~<>=rRsvVqO".replace(" ", ""))
MARKUP = re.compile(r"<[^>]*>|\$.")
BAR_NAMES = {
    "`": "VIRGULA", ",": "DIVISIO_MINIMA", ";": "DIVISIO_MINOR",
    ":": "DIVISIO_MAIOR", "::": "DIVISIO_FINALIS", "^": "DIVISIO_MINIMIS",
    ":?": "DIVISIO_MAIOR_DOT",
}
SIGN_NAMES = {
    "'": "V_EPISEMA", "_": "H_EPISEMA", ".": "PUNCTUM_MORA",
    "w": "QUILISMA", "~": "LIQUESCENTIA", "<": "AUCTUM", ">": "DIMINUTUM",
    "o": "ORISCUS", "v": "VIRGA", "V": "VIRGA_REVERSA", "s": "STROPHA",
    "r": "CAVUM", "R": "QUADRATUM_LINEA", "=": "LINEA", "q": "QUADRATUM",
    "O": "ORISCUS_SCAPUS",
}


BRACKET = re.compile(r"\[[^\[\]]*\]")


def hauteur(letter: str) -> int:
    """Staff height of a GABC letter (``a`` = 3, ``o`` skipped)."""
    low = letter.lower()
    base = ord(low) - ord("a") + 3
    return base + 1 if low > "o" else base


def cle_key(token: str) -> int:
    """Gregorio key of a clef token ``c3``/``f4`` (struct.c:1273-1288)."""
    line = int(token[-1])
    return 2 * line - 1 if token[0] == "c" else 2 * line - 4


def step_of(letter: str, clef: str) -> int:
    """Diatonic step with C = 0, clef applied."""
    return hauteur(letter) - cle_key(clef) - 5


def _clean_text(text: str) -> str:
    text = re.sub(r"\\[A-Za-z]+\{[^}]*\}", "", text)
    text = MARKUP.sub("", text).replace("{", "").replace("}", "")
    return re.sub(r"~+", "~", re.sub(r"\s+", "", text))


def canon(notes, bars, syllables, signs, words=None) -> dict:
    return {
        "notes": notes,
        "bars": bars,
        "syllables": syllables,
        "text": re.sub(r"\s+", "", words[0]) if isinstance(words, list) and words else
                re.sub(r"\s+", "", words or "".join(syllables)),
        "signs": signs,
        "neumes": len({n["neume"] for n in notes}),
    }


def parse_source(text: str) -> dict:
    """Parse the original GABC with our own reader (independent sanity ref)."""
    sep = re.search(r"^\s*%%\s*$", text, re.M)
    body = text[sep.end():] if sep else text
    body = re.sub(r"^[^\n]*%.*$", "", body, flags=re.M)
    notes, bars, syllables, signs = [], [], [], []
    clef, neume = "c4", 0
    parts = re.split(r"\(([^()]*)\)", body)
    for index in range(0, len(parts), 2):
        if index + 1 < len(parts):
            syllables.append(_clean_text(parts[index]))
        code = parts[index + 1] if index + 1 < len(parts) else ""
        code = BRACKET.sub("", code)
        for found in CLEF.finditer(code):
            clef = found.group(0)
        code = CLEF.sub("", code)
        for found in BAR.finditer(code):
            bare = re.sub(r"0$", "", found.group(0))
            bars.append(BAR_NAMES.get(bare, bare))
        code = BAR.sub("", code)
        code = re.sub(r"[zZ][+-]?0?", "", code)
        if not PITCH.search(code):
            continue
        neume += 1
        for char in code:
            if PITCH.fullmatch(char):
                notes.append({"step": step_of(char, clef), "letter": char.lower(), "neume": neume})
            elif char in SIGNS and char != "_":
                # `_` is an automatic episema: Gregorio resolves its placement
                # in layout and does not dump it as an explicit sign.
                signs.append(SIGN_NAMES[char])
    return canon(notes, bars, [s for s in syllables if s], signs)


def parse_dump(text: str) -> dict:
    """Parse ``gregorio -f gabc -F dump`` into the same canonical form."""
    notes, bars, syllables, signs = [], [], [], []
    clef = "c4"
    key = cle_key(clef)
    neume, syllable = 0, 0
    text_parts: list[str] = []
    custos = False
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("type "):
            # A `GRE_CUSTOS` has a `pitch` too, but it is a layout mark, not a
            # sung note (`Gabc.tex:872-884`): counting it would shift the
            # canonical pitch sequence at every clef change.
            custos = "GRE_CUSTOS" in stripped
        if stripped.startswith("initial_key"):
            match = re.search(r"\(([cf])(\d)\)", stripped)
            if match:
                clef = match.group(1) + match.group(2)
                key = cle_key(clef)
            continue
        if stripped.startswith("clef "):
            match = re.search(r"\(([cf])(\d)\)", stripped)
            if match:
                clef = match.group(1) + match.group(2)
                key = cle_key(clef)
            continue
        if stripped.startswith("type ") and "GRE_SYLLABLE" in stripped:
            if text_parts:
                syllables.append(_clean_text("".join(text_parts)))
                text_parts = []
            syllable += 1
            continue
        if stripped.startswith("character "):
            text_parts.append(stripped.split(None, 1)[1])
            continue
        if stripped.startswith("glyph_type"):
            neume += 1
            continue
        if stripped.startswith("pitch "):
            value = stripped.split(None, 1)[1].strip()
            if custos:
                continue
            if re.fullmatch(r"[a-np]", value):
                notes.append({"step": hauteur(value) - key - 5, "letter": value, "neume": neume, "syllable": syllable})
            continue
        if stripped.startswith("liquescentia ") and "L_" in stripped and line.startswith(" " * 9):
            signs.append("LIQUESCENTIA")
            continue
        if stripped.startswith("bar "):
            # Dominican bars carry a digit (`B_DIVISIO_MINOR_D1`,
            # `src/struct.h:158`); dropping them silently would make a wrong
            # `;` spelling look equal to `;1`.
            match = re.search(r"\((B_[A-Z0-9_]+)\)", stripped)
            if match:
                bars.append(match.group(1).removeprefix("B_"))
            continue
        if stripped.startswith("signs "):
            match = re.search(r"\((_[A-Z_]+)\)", stripped)
            if match:
                signs.append(match.group(1).removeprefix("_"))
    if text_parts:
        syllables.append(_clean_text("".join(text_parts)))
    return canon(notes, bars, [s for s in syllables if s], signs)


def _steps(c: dict) -> list[int]:
    return [n["step"] for n in c["notes"]]


def compare(a: dict, b: dict) -> dict:
    """Compare two canonical forms; the first divergence is classified."""
    sa, sb = _steps(a), _steps(b)
    ca, cb = Counter(sa), Counter(sb)
    checks = {
        "pitch_multiset": ca == cb,
        "pitch_sequence": sa == sb,
        "bars": a["bars"] == b["bars"],
        "text": (not a["text"] or not b["text"]) or a["text"] == b["text"],
        "signs": Counter(a["signs"]) == Counter(b["signs"]),
    }
    # Gregorio dumps layout glyphs, which are a different granularity from
    # GABC's parenthesised neume groups; grouping is not part of this check.
    core = all(checks[k] for k in ("pitch_multiset", "pitch_sequence", "bars", "text"))
    premier = next((k for k, ok in checks.items() if not ok), "")
    return {
        **checks,
        "core_exact": core,
        "exact": core and checks["signs"],
        "n_pitch": sum((ca & cb).values()),
        "ref_notes": len(sa),
        "test_notes": len(sb),
        "premier_divergence": premier,
    }
