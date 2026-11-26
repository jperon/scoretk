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

"""Validate the MusicXML export of the 56 MuseScore corpus scores against
the normative MusicXML 4.0 XSD.

Two modes are checked in one run, with the CLI flag ``--xml-full-support``
for the second:

* (a) ``Options::default()`` -- byte-faithful to MuseScore 4.7.4;
* (b) ``--xml-full-support`` -- standard XSD carriers only (no private
  ``scoretk-*`` carrier). This is the reported coverage mode.

Validation uses ``xmllint --schema`` with the normative MusicXML 4.0 XSD and
its local XML catalog (shared with ``validite.py``). No third-party file is
copied into the repository.

Usage (see ``valide-corpus.sh`` for the nix-shell wrapper):
  valide-corpus.py --cli target/debug/scoretk --out /tmp/xml-xsd
"""

from __future__ import annotations

import argparse
from collections import Counter
from pathlib import Path
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from validite import preparer_xsd, valider  # noqa: E402

ROOT = Path(__file__).resolve().parents[3]


def corpus_scores() -> list[Path]:
    scores: list[Path] = []
    for version in ("v3", "v4"):
        scores.extend(sorted((ROOT / "tests" / "corpus" / version).glob("*.mscx")))
    return scores


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
        temporary = tempfile.TemporaryDirectory(prefix="scoretk-xsd-")
        out = Path(temporary.name)
    schema, env = preparer_xsd(out / "xsd")
    xml_dir = out / "musicxml"
    xml_dir.mkdir(parents=True, exist_ok=True)

    scores = corpus_scores()
    modes = (
        ("(a) default export, MuseScore 4.7.4 parity", []),
        ("(b) xml_full_support, standard XSD only", ["--xml-full-support"]),
    )
    tout_valide = True
    for label, extra in modes:
        valides = 0
        messages: Counter[str] = Counter()
        invalides: list[str] = []
        echecs: list[str] = []
        for score in scores:
            written = xml_dir / f"{score.parent.name}_{score.stem}.musicxml"
            result = subprocess.run(
                [str(cli), str(score), "-o", str(written), *extra],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            if result.returncode != 0 or not written.is_file():
                echecs.append(f"{score.parent.name}/{score.stem}")
                continue
            report = valider(schema, env, written)
            if report["valide"]:
                valides += 1
            else:
                invalides.append(f"{score.parent.name}/{score.stem}")
                for message, count in report["messages"].items():
                    messages[message] += count
                if not report["messages"]:
                    messages[report["brut"].strip()[-300:]] += 1

        print(f"{label}: {valides}/{len(scores)} valid, "
              f"{len(invalides)} invalid, {len(echecs)} export failures")
        for name in invalides:
            print(f"  INVALID  {name}")
        for name in echecs:
            print(f"  FAILED   {name}")
        for message, count in sorted(messages.items(), key=lambda pair: -pair[1]):
            print(f"  {count:5d}  {message}")
        tout_valide &= not invalides and not echecs and valides == len(scores)

    print(f"Schema: {schema}")
    print(f"Working directory: {out}")
    if temporary is not None:
        temporary.cleanup()
    sys.exit(0 if tout_valide else 1)


if __name__ == "__main__":
    main()
