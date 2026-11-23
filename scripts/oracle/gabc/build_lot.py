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

"""Build a GABC measurement lot from local, read-only reference data.

The downloaded sources are **data**, never executed: this script parses the
GregoBase SQL dump with Python's ``csv`` reader and reads the gabc payload
with ``json``. GregoBase publishes no site-wide licence; the repository's
``gregobase_chants.copyrighted`` flag is therefore used as the only filter,
and only entries with ``copyrighted = 0`` are written. The resulting corpus
is a private reference lot: do not commit it, do not redistribute it. The
Gregorio ``doc/`` and ``examples/`` files (GPL-3.0-or-later) are copied as
parsing exercises, not redistributed either.

Usage::

    python3 scripts/oracle/gabc/build_lot.py \
        --sql /home/jacques/programmes/musique/specs/gabc/gregobase/gregobase_online.sql \
        --gregorio /home/jacques/programmes/musique/specs/gabc/gregorio \
        --out /home/jacques/programmes/musique/specs/gabc/lot --count 200

Writes ``<out>/corpus/*.gabc`` and ``<out>/lot.tsv`` (``id<TAB>path``).
"""

from __future__ import annotations

import argparse
import csv
import io
import json
import shutil
from pathlib import Path


def chants(sql: Path):
    """Yield ``(id, incipit, mode, gabc)`` for non-copyrighted chants.

    Rows are the single-line ``INSERT INTO gregobase_chants … VALUES (…)``
    statements of the dump; the gabc column (index 11) is a JSON array of
    ``[name, value, options]`` triplets. A row with an unparsable payload is
    skipped, never fatal: the lot is a measurement sample, not a validator of
    GregoBase data.
    """
    with sql.open("r", encoding="utf-8", errors="replace") as handle:
        dans_les_chants = False
        for line in handle:
            if line.startswith("INSERT INTO"):
                dans_les_chants = line.startswith("INSERT INTO `gregobase_chants`")
                continue
            if line.startswith(("CREATE TABLE", "UNLOCK TABLES", "LOCK TABLES")):
                dans_les_chants = False
                continue
            if not dans_les_chants or not line.startswith("("):
                continue
            try:
                row = next(
                    csv.reader(
                        io.StringIO(line.strip()[1:-2]),
                        quotechar="'",
                        escapechar="\\",
                        doublequote=False,
                        skipinitialspace=True,
                    )
                )
            except (csv.Error, StopIteration):
                continue
            if len(row) != 17 or row[15] != "0" or row[11] == "NULL":
                continue
            try:
                parts = json.loads(row[11])
                if isinstance(parts, str):
                    # Most rows store the gabc source directly as a SQL string.
                    code = parts
                else:
                    # A minority store a JSON array of [name, value, options].
                    code = next((v for k, v, *_ in parts if k == "gabc"), "")
            except (ValueError, TypeError, StopIteration):
                continue
            if code.strip():
                yield row[0], row[3], row[6], code


def ecrire_gabc(path: Path, incipit: str, mode: str, code: str) -> None:
    """Write a self-contained .gabc file with the headers gabctk needs.

    ``gabctk.py:391-393`` reads only ``name``; ``gabctk.py:1220-1261`` assumes
    a four-line staff and a clef inside the notation, so ``staff-lines: 4``
    is made explicit. ``mode`` is recorded when the database has one.
    """
    head = [f"name: {incipit or 'Inconnu'};", "language: latin;", "staff-lines: 4;"]
    if mode and mode != "NULL":
        head.append(f"mode: {mode};")
    path.write_text("\n".join(head) + "\n\n%%\n\n" + code.rstrip() + "\n", encoding="utf-8")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--sql", type=Path, required=True, help="GregoBase SQL dump")
    p.add_argument("--gregorio", type=Path, required=True, help="Gregorio checkout")
    p.add_argument("--out", type=Path, required=True, help="lot directory (unversioned)")
    p.add_argument("--count", type=int, default=200, help="GregoBase chants to sample")
    p.add_argument("--stride", type=int, default=97, help="deterministic sampling stride")
    args = p.parse_args()

    corpus = args.out / "corpus"
    corpus.mkdir(parents=True, exist_ok=True)
    entrees: list[tuple[str, Path]] = []

    for index, (cid, incipit, mode, code) in enumerate(chants(args.sql)):
        if index % args.stride:
            continue
        if len([x for x in entrees if x[0].startswith("gb_")]) >= args.count:
            break
        dest = corpus / f"gb_{cid:0>6}.gabc"
        ecrire_gabc(dest, incipit, mode, code)
        entrees.append((f"gb{cid}", dest))

    sources = sorted(
        list((args.gregorio / "doc").glob("*.gabc"))
        + list((args.gregorio / "examples").glob("*.gabc"))
    )
    for src in sources:
        dest = corpus / f"gregorio_{src.name}"
        shutil.copyfile(src, dest)
        entrees.append((f"gregorio_{src.stem}", dest))

    manifest = args.out / "lot.tsv"
    with manifest.open("w", encoding="utf-8") as handle:
        for ident, path in entrees:
            handle.write(f"{ident}\t{path}\n")
    print(
        f"lot: {len(entrees)} files -> {manifest}\n"
        "GregoBase: no site-wide licence statement; only copyrighted=0 entries used; "
        "private reference, do not redistribute.\n"
        "Gregorio files: GPL-3.0-or-later (gregorio/COPYING.md)."
    )


if __name__ == "__main__":
    main()
