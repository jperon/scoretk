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

"""Dump the worst element-level differences of an engraving oracle run.

Unmatched elements are reported with an infinite delta so that missing
noteheads, stems and bar lines are not silently dropped.

Usage: python3 -I scripts/oracle/engrave/worst.py <out> [count] [kind]
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import compare as C


def main() -> int:
    if len(sys.argv) not in (2, 3, 4):
        sys.exit("usage: worst.py <out> [count] [kind]")
    out = Path(sys.argv[1])
    count = int(sys.argv[2]) if len(sys.argv) >= 3 else 20
    only = sys.argv[3] if len(sys.argv) == 4 else None
    kinds = ("Note", "Stem", "BarLine") if only is None else (only,)
    rows: list[tuple[float, str, str, str]] = []
    for source in (out / "lots/corpus.txt").read_text(encoding="utf-8").splitlines():
        if not source.strip():
            continue
        identifier = Path(source).stem
        refs = sorted((out / "refs").glob(f"{identifier}-*.svg"))
        ours = out / "ours" / f"{identifier}.svg"
        if not refs or not ours.exists():
            continue
        reference, ours_svg = C.parse_svg(refs[0]), C.parse_svg(ours)
        for kind in kinds:
            pairs = C.pair_by_nearest(
                reference[kind], ours_svg[kind],
                lambda a, b: abs(a[1] - b[1]) if kind == "Note"
                else (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2,
            )
            for a, b in pairs:
                if a is None or b is None:
                    rows.append((float("inf"), identifier, f"{kind} unmatched", ""))
                elif kind == "Note":
                    delta = abs(reference[kind][a][1] - ours_svg[kind][b][1])
                    rows.append((delta, identifier, kind,
                                 f"ref y={reference[kind][a][1]:.2f}, "
                                 f"ours y={ours_svg[kind][b][1]:.2f}"))
                elif kind == "Stem":
                    ra = reference["stem_lengths"][a]
                    rb = ours_svg["stem_lengths"][b]
                    rows.append((abs(ra - rb), identifier, "Stem length",
                                 f"ref={ra:.2f}, ours={rb:.2f}"))
                else:
                    ra, rb = reference[kind][a], ours_svg[kind][b]
                    delta = max(abs(ra[0] - rb[0]), abs(ra[1] - rb[1]))
                    rows.append((delta, identifier, kind,
                                 f"ref={ra[0]:.2f},{ra[1]:.2f}, "
                                 f"ours={rb[0]:.2f},{rb[1]:.2f}"))
    rows.sort(key=lambda row: row[0], reverse=True)
    for delta, identifier, kind, detail in rows[:count]:
        print(f"{delta:8.2f}  {identifier:28s} {kind:18s} {detail}")
    return 0


if __name__ == "__main__":
    main()
