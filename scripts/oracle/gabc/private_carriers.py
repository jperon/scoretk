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

"""Inventory of private/hidden carriers in GABC full-support outputs.

A carrier is *private* when its value is a copy of a GABC source token that no
consumer of the destination can interpret: a ``gabc:`` field, an invisible
MusicXML text, a ``layout:``/``gabc:`` MEI ``@label``, a hidden annotation, an
``@…`` SMF text prefix.  Carriers defined by the destination specification
(``<miscellaneous-field>``, ``<words>``, ABC ``%%`` directives, LilyPond
``\\header`` fields, RP-017 ``FF 05`` lyrics) are counted separately as
``native`` and are never a violation.

Usage::

    python3 scripts/oracle/gabc/private_carriers.py \\
        --cli /tmp/branch/scoretk --out /tmp/gabc-private \\
        --dir tests/corpus/gabc \\
        --dir /home/jacques/programmes/musique/specs/gabc/lot/corpus

With ``--check`` the process exits non-zero on the first private carrier.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

DESTINATIONS = ("musicxml", "midi", "ly", "abc", "mei")
ENCODING_DATE = "2026-01-01"
# Same full-support modes as `full_support.py`; kept local so the instrument
# can be imported by it without a circular dependency.
FULL_SUPPORT_EXTRA: dict[str, tuple[str, ...]] = {
    "musicxml": ("--xml-full-support",),
    "midi": ("--midi-full-support",),
    "mei": ("--mei-full-support",),
    "ly": ("--ly-gregorian",),
    "abc": ("--abc-gregorian",),
}

# Occurrences of the `gabc` marker in any carrier, including `gabc:` fields,
# `type="gabc-*"`, `gabc:layout:` labels and private `gabcHeader*` variables.
GABC_RE = re.compile(r"(?i)gabc")
# An invisible MusicXML element that actually carries text.
HIDDEN_TEXT_RE = re.compile(
    r"<([A-Za-z][\w:-]*)\b([^>]*\bprint-object\s*=\s*\"no\"[^>]*)>"
    r"([^<]+)</\1>")
# MEI labels encoding source-private layout details (`layout:`, `gabc:`).
PRIVATE_LABEL_RE = re.compile(
    r'\b(?:label|name|type|value)="(?:gabc:|layout:)[^"]*"')
# Generic annotations used in §22-§25 as disguised dumps.
ABC_ANNOTATION_RE = re.compile(r'"\^(?:[A-Za-z]+):[^"\n]*"|""')
LY_ANNOTATION_RE = re.compile(r'\^"(?:[A-Za-z]+:)?[^"\n]*"')
SMF_PRIVATE_PREFIX_RE = re.compile(rb"@(?:ST|SY|TK):")
MIDI_LYRIC_RE = re.compile(rb"\xff\x05")
MIDI_TEXT_RE = re.compile(rb"\xff\x01")
MIDI_TRACK_NAME_RE = re.compile(rb"\xff\x03")

PRIVATE_KINDS = (
    "gabc_prefix", "hidden_text", "private_label", "abc_annotation",
    "ly_annotation", "smf_private_prefix", "hidden_comment",
)
NATIVE_KINDS = (
    "miscellaneous_field", "printed_words", "abc_directive",
    "ly_header_variable", "midi_lyric_meta", "midi_text_meta",
    "midi_track_name_meta",
)


def convert(cli: Path, data: bytes, source_format: str, target_format: str,
            extra: tuple[str, ...]) -> tuple[bytes, str]:
    """Convert in-memory bytes with the destination's full-support options."""
    result = subprocess.run(
        [str(cli), "-", "-o", "-", "--from", source_format, "--to", target_format,
         "--encoding-date", ENCODING_DATE, *extra],
        input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    return result.stdout, result.stderr.decode("utf-8", "replace").strip()


def text_of(destination: str, data: bytes) -> str | None:
    if destination == "midi":
        return None  # binary SMF: scanned byte-wise below
    return data.decode("utf-8", "replace")


def scan(destination: str, data: bytes) -> tuple[Counter[str], list[str]]:
    """Counts per carrier kind and up to four representative samples."""
    counts: Counter[str] = Counter()
    samples: list[str] = []
    if destination == "midi":
        counts["smf_private_prefix"] = len(SMF_PRIVATE_PREFIX_RE.findall(data))
        counts["midi_lyric_meta"] = len(MIDI_LYRIC_RE.findall(data))
        counts["midi_text_meta"] = len(MIDI_TEXT_RE.findall(data))
        counts["midi_track_name_meta"] = len(MIDI_TRACK_NAME_RE.findall(data))
        found = GABC_RE.search(data.decode("latin-1"))
        if found:
            counts["gabc_prefix"] += 1
            samples.append(found.group(0))
        return counts, samples

    text = text_of(destination, data)
    assert text is not None
    matches = GABC_RE.findall(text)
    if matches:
        counts["gabc_prefix"] = len(matches)
        found = GABC_RE.search(text)
        if found:
            samples.append(text[max(0, found.start() - 12):found.start() + 40])
    if destination == "musicxml":
        hidden = HIDDEN_TEXT_RE.findall(text)
        counts["hidden_text"] = len(hidden)
        for _, attributes, value in hidden[:2]:
            samples.append(f"{attributes.strip()} → {value[:48]!r}")
        counts["miscellaneous_field"] = text.count("<miscellaneous-field")
        counts["printed_words"] = text.count("<words")
        counts["hidden_comment"] = text.count("<!--")
    elif destination == "mei":
        counts["private_label"] = len(PRIVATE_LABEL_RE.findall(text))
        counts["hidden_comment"] = text.count("<!--")
        if counts["private_label"]:
            found = PRIVATE_LABEL_RE.search(text)
            if found:
                samples.append(found.group(0)[:64])
    elif destination == "abc":
        counts["abc_annotation"] = len(ABC_ANNOTATION_RE.findall(text))
        directives = re.findall(r"(?m)^%%([^\n]*)", text)
        counts["abc_directive"] = len(directives)
        counts["gabc_prefix"] += sum(1 for directive in directives
                                     if GABC_RE.search(directive))
    elif destination == "ly":
        counts["ly_annotation"] = len(LY_ANNOTATION_RE.findall(text))
        counts["ly_header_variable"] = len(
            re.findall(r'(?m)^\s*[A-Za-z][A-Za-z0-9]*\s*=\s*"', text))
        comments = re.findall(r"(?m)%[^\n]*", text)
        counts["hidden_comment"] = len(comments)
        counts["gabc_prefix"] += sum(1 for comment in comments
                                     if GABC_RE.search(comment))
    return counts, samples


def violations(destination: str, data: bytes) -> list[str]:
    """Describe every private carrier found, or return an empty list."""
    counts, samples = scan(destination, data)
    found = [f"{kind}={counts[kind]}" for kind in PRIVATE_KINDS if counts[kind]]
    if found and samples:
        found.append("exemple: " + " ; ".join(samples[:2]))
    return found


def run(cli: Path, directories: list[Path], out: Path, check: bool) -> dict:
    files = [path for directory in directories
             for path in sorted(directory.glob("*.gabc"))]
    out.mkdir(parents=True, exist_ok=True)
    report: dict = {"cli": str(cli), "files": len(files), "destinations": {}}
    for destination in DESTINATIONS:
        counts: Counter[str] = Counter()
        per_file: Counter[str] = Counter()
        samples: dict[str, list[str]] = {}
        failed = 0
        for path in files:
            source = path.read_bytes()
            written, error = convert(cli, source, "gabc", destination,
                                     FULL_SUPPORT_EXTRA[destination])
            if error or not written:
                failed += 1
                continue
            found, examples = scan(destination, written)
            counts.update(found)
            for kind, count in found.items():
                if count:
                    per_file[kind] += 1
                    if kind in PRIVATE_KINDS:
                        samples.setdefault(kind, [])
                        for example in examples[:1]:
                            if len(samples[kind]) < 3:
                                samples[kind].append(f"{path.stem}: {example}")
            if check and violations(destination, written):
                raise ValueError(f"porteur privé dans {destination} ({path.name}): "
                                 + ", ".join(violations(destination, written)))
        report["destinations"][destination] = {
            "files": len(files), "failed": failed,
            "counts": dict(sorted(counts.items())),
            "files_with": dict(sorted(per_file.items())),
            "samples": samples,
        }
    (out / "private_carriers.json").write_text(
        json.dumps(report, indent=2, ensure_ascii=False))
    return report


def print_report(report: dict) -> None:
    print(f"Private carriers: {report['files']} sources")
    for destination, row in report["destinations"].items():
        print(f"  {destination} ({row['files']} files, {row['failed']} failed)")
        for kind in PRIVATE_KINDS + NATIVE_KINDS:
            count = row["counts"].get(kind, 0)
            if count:
                flag = "PRIVATE" if kind in PRIVATE_KINDS else "native "
                files = row["files_with"].get(kind, 0)
                print(f"    {flag} {kind:<22} {count:>7} occurrences, "
                      f"{files:>3} files")
        for kind, examples in row["samples"].items():
            for example in examples:
                print(f"      e.g. {kind}: {example[:110]}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--dir", type=Path, action="append", default=[])
    parser.add_argument("--check", action="store_true",
                        help="fail on the first private carrier")
    args = parser.parse_args()
    report = run(args.cli, args.dir, args.out, args.check)
    print_report(report)
    print(f"report: {args.out / 'private_carriers.json'}")


if __name__ == "__main__":
    main()
