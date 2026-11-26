#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon
#
# Ce fichier est original : validation RelaxNG des 56 exportations MEI du
# corpus MuseScore, dans les deux modes du writer.
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

"""Validate both MEI export modes of the 56 MuseScore corpus scores with jing.

* default: bytes byte-identical to ``master``, declared ``mei-basic.rng``;
* ``mei_full_support``: standard MEI 5.1 carriers, declared ``mei-all.rng``,
  which is the mode the reliability figure is measured on.

The default export is validated against its *declared* customization
(``mei-basic``). It is reported even when it is invalid: those violations are
MuseScore 4.7.4 output faithfully reproduced by ``master`` and cannot be
changed without breaking the byte-identity requirement. The exit status
therefore tracks the headline (``mei_full_support``) mode only.

Usage (see ``valide-corpus.sh`` for the nix-shell wrapper):
  valide-corpus.py --cli target/debug/scoretk --out /tmp/mei-xsd
"""

from __future__ import annotations

import argparse
from collections import Counter
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
SPECS = Path(
    os.environ.get("SCORETK_SPECS_DIR", "/home/jacques/programmes/musique/specs")
)


def corpus_scores() -> list[Path]:
    scores: list[Path] = []
    for version in ("v3", "v4"):
        scores.extend(sorted((ROOT / "tests" / "corpus" / version).glob("*.mscx")))
    return scores


def validate(schema: Path, paths: list[Path]) -> tuple[int, list[str], Counter]:
    valid = 0
    invalid: list[str] = []
    messages: Counter[str] = Counter()
    for path in paths:
        result = subprocess.run(
            ["jing", str(schema), str(path)],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=120,
        )
        report = result.stdout.decode("utf-8", "replace")
        if result.returncode == 0 and not report.strip():
            valid += 1
            continue
        invalid.append(path.stem)
        lines = [line.split(": error: ", 1)[-1].strip() for line in report.splitlines()]
        messages.update(lines[:6])
    return valid, invalid, messages


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, help="path to the scoretk binary")
    parser.add_argument(
        "--out",
        default="",
        help="working directory (default: temporary directory, removed on exit)",
    )
    args = parser.parse_args()
    cli = Path(args.cli).resolve()
    if not cli.is_file():
        parser.error(f"scoretk binary not found: {cli}")

    temporary = None
    if args.out:
        out = Path(args.out).resolve()
        out.mkdir(parents=True, exist_ok=True)
    else:
        temporary = tempfile.TemporaryDirectory(prefix="scoretk-mei-rng-")
        out = Path(temporary.name)
    (out / "default").mkdir(exist_ok=True)
    (out / "full").mkdir(exist_ok=True)

    schema_basic = SPECS / "mei" / "mei-basic-5.1.rng"
    schema_all = SPECS / "mei" / "mei-all-5.1.rng"
    for schema in (schema_basic, schema_all):
        if not schema.is_file():
            parser.error(f"MEI 5.1 schema not found: {schema}")

    scores = corpus_scores()
    for score in scores:
        written = out / "default" / f"{score.parent.name}_{score.stem}.mei"
        result = subprocess.run(
            [
                str(cli),
                str(score),
                "-o",
                str(written),
                "--from",
                "mscx",
                "--to",
                "mei",
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
        )
        if result.returncode != 0 or not written.is_file():
            parser.error(f"MEI export failed for {score}: {result.stdout.decode()}")

    env = dict(os.environ)
    env["SCORETK_MEI_DUMP_DIR"] = str(out / "full")
    result = subprocess.run(
        [
            "nix",
            "develop",
            "-c",
            "env",
            "CARGO_BUILD_JOBS=2",
            "RUST_TEST_THREADS=2",
            "cargo",
            "test",
            "-p",
            "scoretk",
            "--test",
            "mei_full_support_dump",
            "dump_mei_full_support_corpus",
        ],
        cwd=ROOT,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if result.returncode != 0:
        parser.error(f"full-support dump failed: {result.stdout.decode()}")

    default_paths = sorted((out / "default").glob("*.mei"))
    full_paths = sorted((out / "full").glob("*.mei"))
    labels = (
        ("(a) default, byte-identical to master, mei-basic.rng", schema_basic, default_paths),
        ("(b) mei_full_support, mei-all-5.1.rng (headline)", schema_all, full_paths),
    )
    headline_valid = True
    for label, schema, paths in labels:
        valid, invalid, messages = validate(schema, paths)
        print(f"{label}: {valid}/{len(paths)} valid, {len(invalid)} invalid")
        for name in invalid:
            print(f"  INVALID  {name}")
        for message, count in sorted(messages.items(), key=lambda pair: -pair[1]):
            print(f"  {count:5d}  {message}")
        if "headline" in label:
            headline_valid = not invalid and valid == len(scores)
        else:
            print("  (default violations are inherited from master: bytes are fixed)")

    print(f"Schemas: {schema_basic}, {schema_all}")
    print(f"Working directory: {out}")
    if temporary is not None:
        temporary.cleanup()
    sys.exit(0 if headline_valid else 1)


if __name__ == "__main__":
    main()
