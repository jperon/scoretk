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

"""Measure GABC -> GABC byte exactness and cross-format stability.

The private GregoBase lot is data, never copied into the repository. Inputs
are passed on the command line; outputs are kept in a scratch directory.
``--baseline`` makes a second CLI's outputs the reference for every
destination other than GABC, so a fix in the GABC reader/writer cannot silently
change MusicXML, MIDI, LilyPond, ABC or MEI.

Usage::

    python3 scripts/oracle/gabc/roundtrip_bytes.py \\
        --cli /tmp/branch/scoretk --baseline /tmp/master/scoretk \\
        --out /tmp/gabc-roundtrip-bytes \\
        --dir tests/corpus/gabc \\
        --dir /home/jacques/programmes/musique/specs/gabc/lot/corpus
"""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

OTHER_FORMATS = ("musicxml", "midi", "ly", "abc", "mei")
ENCODING_DATE = "2026-01-01"

# Bodies that Gregorio itself rejects even with a header and `%%` prepended.
# Verified with its parser at commit b63f074 in the bwrap sandbox:
# `() (c4) …` → "clef change is not supported on the first syllable"
# (`gabc-score-determination.y` rejects a clef on an empty first syllable).
# Such a body is out of the denominator, reported separately, never silently
# dropped.
INVALID_BODIES = {
    "gb_006036": (
        "Gregorio b63f074: clef change is not supported on the first syllable"
    ),
}


def convert(cli: Path, source: bytes, to: str) -> tuple[bytes, str]:
    result = subprocess.run(
        [str(cli), "-", "-o", "-", "--from", "gabc", "--to", to,
         "--encoding-date", ENCODING_DATE],
        input=source,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    return result.stdout, result.stderr.decode("utf-8", "replace").strip()


def headerless(source: bytes) -> bytes:
    """Body of a GABC file, without its header and its ``%%`` line.

    The marker is a line whose trimmed content is exactly ``%%``
    (`gabc-common::syntax::split_sections`); a ``%%`` inside a comment or a
    header value is not a separator. GregoBase files may repeat a whole header
    block *after* the first marker: the notation starts at the **last**
    marker, since each ``END_OF_DEFINITIONS`` closes a definitions section
    (`Gabc.tex:51,58`; grammar `score: all_definitions syllables`,
    `all_definitions: definitions END_OF_DEFINITIONS`,
    `gabc-score-determination.y:737-749`). Cutting at the first marker would
    feed a header as if it were notation and report a false difference.
    """
    text = source.decode("utf-8")
    lines = text.splitlines(keepends=True)
    marks = [index for index, line in enumerate(lines) if line.strip() == "%%"]
    if not marks:
        return source
    body = "".join(lines[marks[-1] + 1:])
    return (body.lstrip("\r\n").rstrip("\r\n") + "\n").encode("utf-8")


def has_nabc(body: bytes) -> bool:
    return b"|" in body


def first_difference(expected: bytes, actual: bytes) -> str:
    for index, (left, right) in enumerate(zip(expected, actual)):
        if left != right:
            start = max(0, index - 24)
            return (f"offset {index}: source {expected[start:index + 24]!r} "
                    f"vs output {actual[start:index + 24]!r}")
    return f"length {len(expected)} vs {len(actual)}"


def run(cli: Path, baseline: Path | None, directories: list[Path], out: Path) -> dict:
    files: list[tuple[str, Path]] = []
    for directory in directories:
        files.extend((path.stem, path) for path in sorted(directory.glob("*.gabc")))
    report: dict = {"cli": str(cli), "files": len(files), "phases": {}}
    for phase in ("roundtrip", "body", "body_nabc", "other"):
        (out / phase).mkdir(parents=True, exist_ok=True)
    counts = {phase: {"exact": 0, "different": 0, "failed": 0,
                      "excluded_invalid": 0} for phase in
              ("roundtrip", "body", "body_nabc")}
    details: list[dict] = []
    other_counts = {"identical": 0, "different": 0, "failed": 0}
    for name, path in files:
        source = path.read_bytes()
        body = headerless(source)
        cases = [("roundtrip", name, source, source)]
        if body != source:
            cases.append(("body", f"{name}_body", body, body))
            if has_nabc(body):
                cases.append(("body_nabc", f"{name}_body", body, body))
        for phase, stem, payload, expected in cases:
            written, error = convert(cli, payload, "gabc")
            (out / phase / f"{stem}.gabc").write_bytes(written)
            if error or not written:
                exclusion = INVALID_BODIES.get(name) if phase == "body" else None
                if exclusion:
                    counts[phase]["excluded_invalid"] += 1
                    details.append({"phase": phase, "file": name,
                                    "status": "excluded_invalid",
                                    "citation": exclusion, "error": error})
                else:
                    counts[phase]["failed"] += 1
                    details.append({"phase": phase, "file": name, "status": "failed",
                                    "error": error})
            elif written == expected:
                counts[phase]["exact"] += 1
            else:
                counts[phase]["different"] += 1
                details.append({"phase": phase, "file": name, "status": "different",
                                "detail": first_difference(expected, written)})
        if baseline is not None:
            for fmt in OTHER_FORMATS:
                current, error = convert(cli, source, fmt)
                reference, reference_error = convert(baseline, source, fmt)
                (out / "other" / f"{name}.{fmt}").write_bytes(current)
                if error or reference_error:
                    other_counts["failed"] += 1
                    details.append({"phase": "other", "file": name, "format": fmt,
                                    "status": "failed", "error": error or reference_error})
                elif current == reference:
                    other_counts["identical"] += 1
                else:
                    other_counts["different"] += 1
                    details.append({"phase": "other", "file": name, "format": fmt,
                                    "status": "different",
                                    "detail": first_difference(reference, current)})
    report["phases"] = counts
    report["other_vs_baseline"] = other_counts
    report["details"] = details
    (out / "report.json").write_text(json.dumps(report, indent=2, ensure_ascii=False))
    for phase, numbers in counts.items():
        total = sum(numbers.values()) - numbers["excluded_invalid"]
        print(f"{phase}: exact {numbers['exact']}/{total}, "
              f"different {numbers['different']}, failed {numbers['failed']}, "
              f"excluded_invalid {numbers['excluded_invalid']}")
        for detail in details:
            if detail["status"] == "excluded_invalid" and detail["phase"] == phase:
                print(f"    excluded: {detail['file']} — {detail['citation']}")
    print(f"other formats vs baseline: {other_counts}")
    print(f"report: {out / 'report.json'}")
    return report


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--dir", type=Path, action="append", default=[],
                        help="directory of .gabc files (repeatable)")
    args = parser.parse_args()
    run(args.cli, args.baseline, args.dir, args.out)


if __name__ == "__main__":
    main()
