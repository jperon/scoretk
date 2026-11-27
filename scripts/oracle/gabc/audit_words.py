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

"""Audit every MusicXML ``<words>`` written for the 261 GABC sources.

Each carrier is checked against the only acceptable values: printed special
characters (``<sp>``/``<v>``), printed rubric text (verse numbers, ``V/``,
``R/``, ``Ant.``, ``Ps.``, the ``+``/``*`` signs) and ``[alt:…]`` translation
text. Syllable markup such as the vowel-centering braces ``{}``
(`Gabc.tex:314-324`) is never printed and must never appear in ``<words>``.

Usage::

    python3 scripts/oracle/gabc/audit_words.py --cli /path/to/scoretk \\
        --dir tests/corpus/gabc \\
        --dir /home/jacques/programmes/musique/specs/gabc/lot/corpus
"""

from __future__ import annotations

import argparse
import html
import re
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from full_support import FULL_SUPPORT_EXTRA, convert_from  # noqa: E402

WORDS_RE = re.compile(r"<words\b([^>]*)>(.*?)</words>", re.S)
SP_RE = re.compile(r"<sp>(.*?)</sp>", re.S)
V_RE = re.compile(r"<v>(.*?)</v>", re.S)

# Printed value -> source token. The left column is what MusicXML prints;
# the right column is the GABC token the exporter read it from.
PRINTED_SPECIALS = {
    "V/": "<sp>V/</sp>",
    "R/": "<sp>R/</sp>",
    "✶": "<sp>*</sp>",
    "†": "<sp>+</sp>",
    "'æ": "<sp>'ae</sp>",
    "æ": "<sp>ae</sp>",
    "✳": "<v>\\greheightstar</v> / <v>\\textcolor{red}{*}</v>",
    "✦": "<v>\\gresixstar</v>",
    "‡": "<v>\\ddag\\ </v>",
    "V̄": "<v>\\Vbar</v>",
    "★": "<v>$\\star$</v>",
    "¶": "<v>\\P\\ </v>",
    "(": "<v>(</v>",
    ")": "<v>)</v>",
    "A": "<v>\\textcolor{red}{A}</v>",
    "G": "<v>\\textcolor{red}{G}</v>",
    "H": "<v>\\textcolor{red}{H}</v>",
    "M": "<v>\\textcolor{red}{M}</v>",
    "P": "<v>\\textcolor{red}{P}</v>",
}

# Source tokens of printed specials that are not part of a printed value.
PRINTED_GLYPH_OF_TOKEN = {
    "<sp>V/</sp>": "V/ (barred V, Gabc.tex:345-378)",
    "<sp>R/</sp>": "R/ (barred R)",
    "<sp>*</sp>": "✶ (GreStar)",
    "<sp>+</sp>": "† (GreDagger)",
    "<sp>'ae</sp>": "'æ (accented æ ligature)",
    "<sp>ae</sp>": "æ ligature",
    "<v>\\greheightstar</v>": "✳ (StarHeight)",
    "<v>\\textcolor{red}{*}</v>": "✳ (red star sign)",
    "<v>\\gresixstar</v>": "✦ (StarSix)",
    "<v>\\ddag\\ </v>": "‡ (double dagger)",
    "<v>\\Vbar</v>": "V̄ (barred V)",
    "<v>$\\star$</v>": "★ (star)",
    "<v>\\P\\ </v>": "¶ (pilcrow)",
    "<v>(</v>": "(",
    "<v>)</v>": ")",
}


def sources(dirs: list[Path]) -> list[Path]:
    return sorted({f for directory in dirs for f in directory.glob("*.gabc")})


def printed_family(value: str) -> str:
    """Return the source token and the Gregorio printed form of ``value``."""
    for glyph in PRINTED_SPECIALS:
        if glyph == value or value == glyph + "." or value == glyph + ",":
            token = PRINTED_SPECIALS[glyph]
            suffix = value[len(glyph):]
            return f"{token}{suffix}  → prints {PRINTED_GLYPH_OF_TOKEN.get(token, glyph)}{suffix}"
    if value.endswith(".") and value[:-1].isdigit():
        return "plain verse number before a note-less bar → printed verbatim"
    if re.fullmatch(r"\(\d+\)\.", value):
        return "<v>(</v>N<v>)</v>. → printed verbatim as (N)."
    if value in ("Ps.", "Ant."):
        return f"<i>{value}</i> → printed verbatim"
    if value == "Hic genuflectitur.":
        return "[alt:Hic genuflectitur.] → printed above the staff in italics"
    if value == "+":
        return "plain + sign before a note-less bar → Gregorio prints † (GreDagger)"
    return "UNCLASSIFIED — not a printed value"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--dir", type=Path, action="append", default=[])
    args = parser.parse_args()

    counts: Counter[str] = Counter()
    origins: dict[str, Counter[str]] = defaultdict(Counter)
    files = sources(args.dir)
    for source in files:
        xml, _ = convert_from(args.cli, source.read_bytes(), "gabc", "musicxml",
                              FULL_SUPPORT_EXTRA["musicxml"])
        for match in WORDS_RE.finditer(xml.decode("utf-8", "replace")):
            value = html.unescape(match.group(2))
            counts[value] += 1
            origins[value][source.name] += 1

    print(f"GABC sources: {len(files)}; distinct <words> values: {len(counts)}; "
          f"total <words>: {sum(counts.values())}")
    print(f"{'count':>6}  {'value':<22} source token → printed form")
    bad = []
    for value, count in sorted(counts.items(), key=lambda item: (-item[1], item[0])):
        family = printed_family(value)
        print(f"{count:>6}  {value!r:<22} {family}")
        if family.startswith("UNCLASSIFIED") or any(c in value for c in "{}\\<>"):
            bad.append(value)
    if bad:
        print(f"FAIL: {len(bad)} value(s) are not printed text: {bad}", file=sys.stderr)
        raise SystemExit(1)
    print("PASS: every <words> value is text Gregorio prints for its source")


if __name__ == "__main__":
    main()
