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

"""GABC full-support figure: every L6/L0/L1 fact, every destination.

The reference inventory is read from the *canonical GABC* produced by
scoretk itself from each source (the 261 full files are byte-identical on the
current tree, see `roundtrip_bytes.py`). The same reader is applied to the GABC
obtained by `source -> destination -> GABC`, so a fact is counted as carried
only when a consumer of the destination can reconstruct it. Private carriers
(`gabc:` fields in MusicXML, `gabc:layout:` / `gabc:detail:` MEI labels and
`<annot type="gabc-*">`) are removed before the return conversion: they cannot
be part of the headline figure.

A loss is **inherent** only when the destination's specification has no
carrier for that kind and a citation is recorded in `INHERENT`. A loss with no
citation is a defect and stays in the denominator. The headline is
`carried / (total - inherent)`. Both figures are printed, and every inherent
loss is listed with its citation; nothing is excluded silently.

Usage::

    python3 scripts/oracle/gabc/full_support.py \\
        --cli /tmp/branch/scoretk --out /tmp/gabc-full-support \\
        --dir tests/corpus/gabc \\
        --dir /home/jacques/programmes/musique/specs/gabc/lot/corpus
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from roundtrip_bytes import convert  # noqa: E402


def convert_from(cli: Path, data: bytes, source_format: str,
                 target_format: str, extra: tuple[str, ...] = ()) -> tuple[bytes, str]:
    """Convert in-memory bytes between two explicit formats."""
    result = subprocess.run(
        [str(cli), "-", "-o", "-", "--from", source_format, "--to", target_format,
         "--encoding-date", ENCODING_DATE, *extra],
        input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    return result.stdout, result.stderr.decode("utf-8", "replace").strip()


DESTINATIONS = ("musicxml", "midi", "ly", "abc", "mei")
ENCODING_DATE = "2026-01-01"

# Fact kinds, with the Gregorio rule that makes them a GABC fact.
KIND_SOURCE = {
    "header": "Gabc.tex:67-154 (header definitions other than the title)",
    "title": "Gabc.tex:79 (name: header)",
    "text": "Gabc.tex:228-343 (syllable and rubric text)",
    "letter": "Gabc.tex:408-443 (one sung staff position per letter)",
    "pitch": "Gabc.tex:410-413,452-490 with `src/struct.c:1273-1288` (derived concert pitch of one sung letter)",
    "glyph": "Gabc.tex:452-700 (neume shape, fusion and spacing)",
    "mora": "Gabc.tex:742-755 (punctum mora)",
    "ictus": "Gabc.tex:756-768 (ictus)",
    "episema": "Gabc.tex:769-797 (horizontal episema)",
    "quilisma": "Gabc.tex:470,452-490 (quilisma)",
    "shape": "Gabc.tex:452-700,702-797 (other neume/sign glyphs)",
    "bar": "Gabc.tex:801-845 (separation bars)",
    "clef": "Gabc.tex:847-870 (clef tokens)",
    "custos": "Gabc.tex:872-906 (custos and line breaks)",
    "nabc": "gabc-score-determination.y:870-907 (NABC cuts)",
    "attribute": "Gabc.tex:689-700,908-931,1093-1118 (bracketed attributes)",
    "rubric": "Gabc.tex:297-343,345-378 (<sp>/<alt> rubric markup)",
    "layout": "Gabc.tex:60-64,241-245 (exact inter-group whitespace)",
}

SIGN_CLASSES = {
    ".": "mora",
    "'": "ictus",
    "_": "episema",
    "w": "quilisma",
}
SHAPE_START = "vOsSr~<>=xXy#"
PITCH = set("abcdefghijklmnpABCDEFGHIJKLMNP")


def notation(text: str) -> tuple[list[str], str, str]:
    """Header lines and notation half, split at the *last* `%%` line.

    GregoBase files can contain a second header block after the first marker;
    the notation starts at the last `%%` line (`Gabc.tex:51,58`,
    `gabc-score-determination.y:737-749`). A file with no marker is a raw body
    (`docs/agents/gabc.md` §15).
    """
    lines = text.splitlines(keepends=True)
    marks = [index for index, line in enumerate(lines) if line.strip() == "%%"]
    if not marks:
        return [], "", text
    first, last = marks[0], marks[-1]
    return (lines[:first], "".join(lines[first + 1:last]),
            "".join(lines[last + 1:]))


def groups(body: str) -> list[tuple[str, str]]:
    """`(code, text after the group)` for each notation group.

    The scanner follows the GABC lexer: `<v>…</v>` is verbatim TeX whose
    parentheses are not groups (`Gabc.tex:297-309,1093-1102`) and `%` comments
    only suppress the following space (`Gabc.tex:60-64,241-245`).
    """
    result: list[tuple[str, str]] = []
    index = 0
    while index < len(body):
        if body.startswith("<v>", index):
            end = body.find("</v>", index + 3)
            index = len(body) if end < 0 else end + 4
            continue
        if body[index] == "%":
            end = body.find("\n", index)
            index = len(body) if end < 0 else end + 1
            continue
        if body[index] != "(":
            index += 1
            continue
        end = body.find(")", index + 1)
        if end < 0:
            break
        result.append((body[index + 1:end], body[end + 1:]))
        index = end + 1
    return result


def signs(code: str) -> list[tuple[str, str]]:
    """Raw rhythmic/shape signs of one note glyph (`Gabc.tex:702-797`)."""
    found: list[tuple[str, str]] = []
    index = 0
    while index < len(code):
        char = code[index]
        end = index + 1
        if char == ".":
            while end < len(code) and end - index < 3 and code[end] in ".01":
                end += 1
        elif char == "'" and end < len(code) and code[end] in "01":
            end += 1
        elif char == "_":
            while end < len(code) and code[end] in "012345":
                end += 1
        elif char == "r" and end < len(code) and code[end] in "12345678":
            end += 1
        elif char in "oO" and end < len(code) and code[end] in "012":
            end += 1
        elif char == "#" and code[end:end + 1] == "#":
            end += 1
        elif char in "xXy" and code[end:end + 1] == "?":
            end += 1
        kind = SIGN_CLASSES.get(char, "shape" if char in SHAPE_START else None)
        if kind is not None and char.isalpha():
            # A pitch letter is a sung note, never a sign (Gabc.tex:408-413).
            if char in PITCH and not (
                char in "wvs" and code[index - 1:index] in PITCH | {"("}
            ):
                kind = None
        if kind is not None:
            found.append((kind, code[index:end]))
        index = end
    return found


def facts(gabc_text: str) -> Counter[tuple[str, str]]:
    """Comparable facts of one canonical GABC document."""
    header, prefix, body = notation(gabc_text)
    result: Counter[tuple[str, str]] = Counter()
    for line in prefix.splitlines():
        if line.strip():
            result[("layout", " ".join(line.split()))] += 1
    declared_lines = 0
    for line in header:
        match = re.match(r"\s*nabc-lines:\s*(\d+)", line)
        if match:
            declared_lines = int(match.group(1))
    # A group alternates square/manuscript at each cut when one line is
    # declared (`gabc-score-determination.y:870-907`); headerless bodies imply
    # one line (gabc.md §17). With n lines the state cycles modulo n+1.
    cycle = declared_lines + 1 if declared_lines else 2
    for line in header:
        line = line.strip()
        if not line or line.startswith("%"):
            continue
        name, separator, value = line.partition(":")
        if separator and value.endswith(";"):
            key = f"{name.strip()}={' '.join(value.rstrip(';').split())}"
            result[("title" if name.strip() == "name" else "header", key)] += 1
    all_groups = groups(body)
    # The derived concert pitch uses the first clef only (gabc.md §10;
    # `src/struct.c:1273-1288`): step = height - key - 5, C = 0.
    first_clef = "c4"
    for code, _ in all_groups:
        found = re.search(r"(?<![A-Za-z0-9])([cf]b?[1-9](?:@[cf]b?[1-9])?)", code)
        if found:
            first_clef = found.group(1)
            break
    key = 2 * int(first_clef[-1]) - (4 if first_clef.startswith("f") else 1)
    semitones = (0, 2, 4, 5, 7, 9, 11)

    alterations: dict[str, int] = {}

    def concert(letter: str) -> str:
        low = letter.lower()
        height = ord(low) - ord("a") + 3 + (1 if low > "o" else 0)
        step = height - key - 5
        return str(60 + semitones[step % 7] + 12 * (step // 7)
                   + alterations.get(low, 0))

    prefix = "\n".join(
        line for line in body.split("(", 1)[0].splitlines()
        if not line.lstrip().startswith("%")
    )
    for match in re.finditer(r"<[^>]+>", prefix):
        result[("rubric", match.group(0))] += 1
    lead = re.sub(r"<[^>]+>", "", prefix).strip()
    if lead:
        result[("text", " ".join(lead.split()))] += 1
    previous_tail = ""
    last_letter = ""
    for code, tail in all_groups:
        if previous_tail and previous_tail[0].isspace():
            alterations.clear()  # word end (`Gabc.tex:212-227`)
        previous_tail = tail
        # Attributes are layout/tuning/TeX tokens, not notes (Gabc.tex:689-700).
        for attribute in re.findall(r"\[[^\]]*\]", code):
            result[("attribute", attribute)] += 1
        code = re.sub(r"\[[^\]]*\]", "", code)
        # NABC cuts (`gabc-score-determination.y:870-907`). Only state-0
        # segments are square notes; the others are manuscript snippets.
        segments = code.split("|")
        square = segments[0]
        for state, part in enumerate(segments[1:], start=1):
            if state % cycle == 0:
                square += part
            else:
                result[("nabc", part)] += 1
        code = square
        for clef in re.findall(r"(?<![A-Za-z0-9])([cf]b?[1-9](?:@[cf]b?[1-9])?)", code):
            result[("clef", clef)] += 1
            code = code.replace(clef, " ", 1)
        for bar in re.findall(r"::|[,;][1-8]|:\?|[,;:]", code):
            result[("bar", bar)] += 1
            code = code.replace(bar, " ", 1)
        for custos in re.findall(r"[zZ][0-9+\-]?", code):
            result[("custos", custos)] += 1
            code = code.replace(custos, " ", 1)
        # A glyph is the spelling between two sung letters (one per note,
        # `GregorianNote::glyph`, `Gabc.tex:452-700`). Signs are read from it.
        buffer: list[str] = []
        for char in code:
            if char.isspace():
                continue
            if char in PITCH:
                result[("letter", char)] += 1
                glyph = "".join(buffer)
                if glyph:
                    result[("glyph", glyph)] += 1
                    if last_letter:
                        if "x" in glyph or "X" in glyph:
                            alterations[last_letter.lower()] = -1
                        elif "#" in glyph:
                            alterations[last_letter.lower()] = 1
                        elif "y" in glyph or "Y" in glyph:
                            alterations[last_letter.lower()] = 0
                result[("pitch", concert(char))] += 1
                last_letter = char
                for kind, token in signs(glyph):
                    result[(kind, token)] += 1
                buffer = []
            else:
                buffer.append(char)
        glyph = "".join(buffer)
        if glyph:
            result[("glyph", glyph)] += 1
        for kind, token in signs(glyph):
            result[(kind, token)] += 1
        text = tail.split("(")[0].strip()
        # Rubric markup (`<sp>`, `<alt>`) is counted separately from the sung
        # word (`Gabc.tex:297-343`); other formats carry plain lyric text.
        markup = re.findall(r"<[^>]+>", text)
        for tag in markup:
            result[("rubric", tag)] += 1
        text = re.sub(r"<[^>]+>", "", text)
        # Inter-group separators carry word breaks and verbatim layout
        # (`Gabc.tex:60-64,241-245`); only actual words are lyric facts.
        if text.strip():
            value = " ".join(text.split())
            if any(char.isalnum() for char in value):
                if ("\\" in value or "{" in value or "}" in value
                        or value.startswith(("V/", "R/", "Ant.", "Ps.", "~~", "+"))
                        or re.match(r"^(?:[0-9]+|[ivxl]+)\.$", value)):
                    result[("rubric", value)] += 1  # TeX/<sp> markup
                else:
                    result[("text", value)] += 1
            else:
                result[("glyph", value)] += 1
        separator = tail[:len(tail) - len(tail.lstrip())]
        if separator:
            result[("layout", repr(separator))] += 1
    return result


# Some sign/barrier spellings have no carrier even when the kind does.
# Patterns are matched against the fact value; the first match wins.
INHERENT_VALUES: dict[str, dict[str, list[tuple[str, str]]]] = {
    destination: {
        "mora": [(r"^\.{2,}|^\.[0-9]", "Gabc.tex:742-755: sign placement digits and repeated morae have no separate object in this format")],
        "episema": [(r"^_", "Gabc.tex:769-797: only one tenuto/episema can be attached to a written note in this format; repeated and positioned episemata have no separate carrier")],
        "bar": [(r"^[,;][1-8]$", "Gregorio `gabc-notes-determination.l:1246-1248`: Dominican bar digits have no element in this format (`gabc.md` §19)")],
    }
    for destination in DESTINATIONS
}


def inherent_citation(destination: str, kind: str, value: str) -> str | None:
    for pattern, citation in INHERENT_VALUES.get(destination, {}).get(kind, []):
        if re.match(pattern, value):
            return citation
    return INHERENT.get(destination, {}).get(kind)


# A loss is inherent only with a citation here. `None` / absence means the
# format has a carrier and the loss is a defect.
INHERENT: dict[str, dict[str, str]] = {
    "musicxml": {
        "letter": "XSD 4.0 `pitch/step` names a pitch, not a staff position; `notehead-value` has no punctum inclusum/inclinatum value (format-ref/musicxml.md §Notes)",
        "header": "MusicXML XSD 4.0 `work`/`identification` define a closed header vocabulary (format-ref/musicxml.md §Document shape); GregoBase headers have no standard field",
        "rubric": "XSD 4.0 `<words>` carries plain text only, not GABC `<sp>`/`<alt>` TeX markup (gabc.md §9.1)",
        "glyph": "MusicXML XSD 4.0 `notations` has no element for a GABC glyph spelling; only SMuFL *names* are standard (`xsd:other-notation/@smufl`), see gabc.md §14.4",
        "shape": "XSD 4.0 `ornaments`/`articulations` enumerate a fixed vocabulary; most chant shapes have no element (gabc.md §9.1)",
        "ictus": "XSD 4.0 has no ictus object (`xsd:notations` enumeration)",
        "nabc": "XSD 4.0 `notations` has no manuscript-neume object (gabc.md §16)",
        "custos": "XSD 4.0 `note` offers pitch/rest only; no `<custos>` element",
        "attribute": "XSD 4.0 `direction-type` has no TeX attribute object",
        "layout": "XSD 4.0 `print`/`spacing` describe page geometry, not inter-syllable source whitespace",
    },
    "midi": {
        "letter": "SMF has no staff/clef fields (`format-ref/midi.md` §Container layout); only sounding pitch is carried",
        "header": "SMF meta events have no structured header object (`format-ref/midi.md` §Meta-event types)",
        "rubric": "RP-017 lyric meta carries syllable text only (`format-ref/midi.md` §Lyrics)",
        "title": "SMF `FF 03` is a sequence/track name, not a Gregorian `name:` header; the MuseScore-compatible default export writes no text meta (`format-ref/midi.md` §Meta-event types)",
        "text": "RP-017 carries syllable text but not GABC markup (`format-ref/midi.md` §Lyrics)",
        "glyph": "SMF has no notation objects (`format-ref/midi.md` §Instruments, parts, repeats)",
        "mora": "SMF Note On/Off carries sound, not rhythmic signs (`format-ref/midi.md` §Events and time)",
        "ictus": "SMF has no ictus object (`format-ref/midi.md` §Events and time)",
        "episema": "SMF has no articulation object (`format-ref/midi.md` §Events and time)",
        "quilisma": "SMF renders an ornament as notes, not as a sign (`format-ref/midi.md` §Instruments, parts, repeats)",
        "shape": "SMF has no glyph object (`format-ref/midi.md` §Events and time)",
        "bar": "SMF `MTrk` has no bar/measure fields (`format-ref/midi.md` §Container layout)",
        "clef": "SMF `MTrk` has no staff/clef fields (`format-ref/midi.md` §Container layout)",
        "custos": "SMF has no engraving objects (`format-ref/midi.md` §Instruments, parts, repeats)",
        "nabc": "SMF has no manuscript-neume object (`format-ref/midi.md` §Events and time)",
        "attribute": "SMF has no TeX attribute object (`format-ref/midi.md` §Events and time)",
        "layout": "SMF has no layout object (`format-ref/midi.md` §Container layout)",
    },
    "ly": {
        "quilisma": "The `\\fixed` chant dialect writes a generic `\\prall`; the quilisma glyph requires the vaticana/gregoriano syntax of Notation Reference §17.4 (gabc.md §14.4)",
        "letter": "LilyPond pitches are absolute note names, not GABC staff positions (`format-ref/lilypond.md` §Pitch, key, clef, accidentals)",
        "header": r"LilyPond `\header` has no standard field for GregoBase definitions such as `office-part` or `nabc-lines` (format-ref/lilypond.md §File structure)",
        "rubric": "LilyPond lyrics are plain words with hyphens (`format-ref/lilypond.md` §Repeats and lyrics)",
        "glyph": "The scoretk chant dialect is `\\fixed` with ordinary notes (gabc.md §14.4); no GABC glyph spelling is a token of it",
        "shape": "Same `\\fixed` dialect: only `\\prall`, `_`, `-` and `\\breathe` are emitted; arbitrary chant glyphs are not tokens (`format-ref/lilypond.md` §Pitch, clef, accidentals)",
        "ictus": "No ictus token in the `\\fixed` dialect (`format-ref/lilypond.md` §Articulations ornaments)",
        "custos": "LilyPond has no custos object in the `\\fixed` dialect (`format-ref/lilypond.md` §Pitch, key, clef, accidentals)",
        "nabc": "NABC snippets need the vaticana/gregoriano dialect, not the `\\fixed` output (gabc.md §14.4)",
        "attribute": "TeX layout attributes are not LilyPond syntax tokens",
        "layout": "Exact source whitespace is not an engraving fact of LilyPond input",
    },
    "abc": {
        "quilisma": "ABC 2.1 §4.14 has `P` (upper mordent) but no distinct quilisma decoration",
        "letter": "ABC §4.1/§4.2: only note-letter case and octave marks encode pitch; clef never alters the written letter (`format-ref/abc.md` §Pitch, accidentals, key)",
        "header": "ABC 2.1 §3.1 defines a closed tune-header field list; GregoBase headers have no field (format-ref/abc.md §File and tune structure)",
        "rubric": "ABC 2.1 lyrics are plain words (`format-ref/abc.md` §Lyrics)",
        "glyph": "ABC 2.1 decorations are a fixed vocabulary (`format-ref/abc.md` §4.14); no GABC glyph spelling",
        "shape": "ABC 2.1 §4.14 has no arbitrary chant glyph token",
        "ictus": "ABC 2.1 §4.14 has no ictus decoration",
        "custos": "ABC 2.1 §4.6 clef/ staff syntax has no custos object",
        "nabc": "ABC 2.1 has no manuscript-neume object",
        "attribute": "TeX layout attributes are not ABC tokens",
        "layout": "ABC line breaks are layout, not source whitespace (`format-ref/abc.md` §File and tune structure)",
    },
    "mei": {
        "letter": "MEI `@pname`/`@oct` name a pitch, not a GABC staff position (`format-ref/mei.md` §7); only the sounding pitch is standard",
        "header": "MEI `<meiHead>` defines bibliographic fields, not GregoBase header names (`format-ref/mei.md` §3)",
        "rubric": "MEI `<syl>` carries plain text; GABC rubric markup is not a standard MEI element (`format-ref/mei.md` §11)",
        "attribute": "MEI 5.1 has no element for TeX tuning/attribute tokens (`format-ref/mei.md` §13)",
        "layout": "MEI `@label` carriers are removed here as source-private; no standard element encodes exact GABC whitespace",
    },
}


def strip_private(destination: str, data: bytes) -> bytes:
    """Remove every private carrier before measuring the return conversion."""
    if destination == "musicxml":
        text = data.decode("utf-8")
        out: list[str] = []
        rest = text
        while "gabc:" in rest:
            at = rest.index("gabc:")
            open_at = rest.rfind("<", 0, at)
            if open_at < 0:
                break
            tag = rest[open_at + 1:]
            name_end = len(tag)
            for index, char in enumerate(tag):
                if char.isspace() or char == ">":
                    name_end = index
                    break
            name = tag[:name_end]
            close = f"</{name}>"
            end = rest.find(close, at)
            if end < 0:
                break
            out.append(rest[:open_at])
            rest = rest[end + len(close):]
        out.append(rest)
        return "".join(out).encode("utf-8")
    if destination == "mei":
        text = data.decode("utf-8")
        text = re.sub(r"\s*<annot type=\"gabc-[^\"]*\"[^>]*>.*?</annot>", "", text, flags=re.S)
        text = re.sub(r"\s*<annot type=\"gabc-[^\"]*\"[^>]*/>", "", text)
        text = re.sub(r" label=\"(?:gabc:layout:|layout:)[^\"]*\"", "", text)
        return text.encode("utf-8")
    return data


def convert_back(cli: Path, data: bytes, destination: str) -> tuple[bytes, str]:
    return convert_from(cli, strip_private(destination, data), destination, "gabc")


def run(cli: Path, directories: list[Path], out: Path) -> dict:
    files: list[tuple[str, Path]] = []
    for directory in directories:
        files.extend((path.stem, path) for path in sorted(directory.glob("*.gabc")))
    out.mkdir(parents=True, exist_ok=True)
    totals = {
        destination: {"total": Counter(), "carried": Counter(), "files": 0,
                      "failed": 0}
        for destination in DESTINATIONS
    }
    details: list[dict] = []
    for name, path in files:
        source = path.read_bytes()
        canonical, error = convert(cli, source, "gabc")
        if error or not canonical:
            details.append({"file": name, "status": "canonicalization failed",
                            "error": error})
            continue
        reference = facts(canonical.decode("utf-8"))
        for destination in DESTINATIONS:
            extra = ("--mei-full-support",) if destination == "mei" else ()
            written, error = convert_from(cli, source, "gabc", destination, extra)
            if error or not written:
                totals[destination]["failed"] += 1
                details.append({"file": name, "destination": destination,
                                "status": "forward conversion failed", "error": error})
                continue
            returned, error = convert_back(cli, written, destination)
            if error or not returned:
                totals[destination]["failed"] += 1
                details.append({"file": name, "destination": destination,
                                "status": "return conversion failed", "error": error})
                continue
            candidate = facts(returned.decode("utf-8"))
            totals[destination]["files"] += 1
            for fact, count in reference.items():
                totals[destination]["total"][fact] += count
                found = min(count, candidate[fact])
                totals[destination]["carried"][fact] += found
                if found < count:
                    details.append({"file": name, "destination": destination,
                                    "fact": fact[0], "value": fact[1],
                                    "missing": count - found})
    report = {
        "cli": str(cli), "files": len(files), "destinations": {},
        "details": details,
    }
    for destination in DESTINATIONS:
        total = totals[destination]["total"]
        carried = totals[destination]["carried"]
        inherent = Counter()
        defects = Counter()
        citations: dict[str, str] = {}
        for (kind, _), count in total.items():
            missing = count - carried[(kind, _)]
            if not missing:
                continue
            citation = inherent_citation(destination, kind, _)
            if citation:
                inherent[kind] += missing
                citations[kind] = citation
            else:
                defects[kind] += missing
        all_facts = sum(total.values())
        all_carried = sum(carried.values())
        inherent_total = sum(inherent.values())
        report["destinations"][destination] = {
            "files": totals[destination]["files"],
            "failed": totals[destination]["failed"],
            "all_facts": all_facts,
            "carried": all_carried,
            "inherent": dict(inherent),
            "defects": dict(defects),
            "citations": citations,
            "headline_total": all_facts - inherent_total,
            "headline_percent": (
                100.0 * all_carried / (all_facts - inherent_total)
                if all_facts > inherent_total else 100.0
            ),
            "raw_percent": 100.0 * all_carried / all_facts if all_facts else 100.0,
        }
    (out / "full_support.json").write_text(
        json.dumps(report, indent=2, ensure_ascii=False))
    return report


def print_report(report: dict) -> None:
    print(f"GABC full support: {report['files']} sources")
    print(f"{'destination':<12} {'carried':>10} {'total':>10} {'main %':>9} "
          f"{'raw %':>9} {'inherent':>10} {'defects':>9}")
    for destination in DESTINATIONS:
        row = report["destinations"][destination]
        print(f"{destination:<12} {row['carried']:>10} {row['all_facts']:>10} "
              f"{row['headline_percent']:>9.2f} {row['raw_percent']:>9.2f} "
              f"{sum(row['inherent'].values()):>10} "
              f"{sum(row['defects'].values()):>9}  "
              f"({row['files']} files, {row['failed']} failed)")
        for kind, count in sorted(row["defects"].items(), key=lambda item: -item[1]):
            print(f"    DEFECT {kind}: {count} (no carrier citation)")
        for kind, count in sorted(row["inherent"].items(), key=lambda item: -item[1]):
            print(f"    inherent {kind}: {count} — {row['citations'][kind]}")
    print(f"report: {report.get('out', '(see --out)/full_support.json')}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--dir", type=Path, action="append", default=[])
    args = parser.parse_args()
    report = run(args.cli, args.dir, args.out)
    report["out"] = str(args.out)
    print_report(report)
    (args.out / "full_support.json").write_text(
        json.dumps(report, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
