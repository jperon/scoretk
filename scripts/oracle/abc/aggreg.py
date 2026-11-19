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

"""Agrège mesures.tsv en tableau de relevé et top des causes d'écart."""

import argparse
import collections
import csv


def resume(rows, key):
    group = [r for r in rows if key is None or r["lot"] == key]
    return {
        "fichiers": len({r["fichier"] for r in group}),
        "morceaux": len(group),
        "exacts": sum(r["statut"] == "ok" and r["cause"] == "OK" for r in group),
        "ref": sum(int(r["ref"]) for r in group),
        "justes": sum(int(r["com1"]) for r in group),
        "échecs": sum(r["statut"] != "ok" for r in group),
        "vides": sum(r["statut"] == "ok" and int(r["cand"]) == 0 for r in group),
    }


def affiche(rows, lots):
    print("lot\tcandidat\tfichiers\tmorceaux\texacts\tnotes\tcauses justes\t%\téchecs\tvides")
    for lot in lots + [(None, "TOTAL")]:
        name, label = lot
        for candidate in ("scoretk", "alternatif"):
            s = resume([r for r in rows if r["candidat"] == candidate], name)
            pct = 100 * s["justes"] / s["ref"] if s["ref"] else 0.0
            print(f"{label}\t{candidate}\t{s['fichiers']}\t{s['morceaux']}\t"
                  f"{s['exacts']}\t{s['ref']}\t{s['justes']}\t{pct:.1f}\t"
                  f"{s['échecs']}\t{s['vides']}")
    print()
    print("causes\tscoretk\talternatif\texemple scoretk")
    counts = {}
    examples = {}
    for candidate in ("scoretk", "alternatif"):
        for row in rows:
            if row["candidat"] != candidate or row["cause"] == "OK":
                continue
            cause = row["cause"]
            counts.setdefault(cause, collections.Counter())[candidate] += 1
            examples.setdefault((cause, candidate), f"{row['fichier']} X:{row['x']}")
    for cause, counter in sorted(counts.items(), key=lambda kv: -sum(kv[1].values())):
        sample = examples.get((cause, "scoretk"),
                              examples.get((cause, "alternatif"), "-"))
        print(f"{cause}\t{counter['scoretk']}\t{counter['alternatif']}\t{sample}")


def principal():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tsv")
    args = parser.parse_args()
    with open(args.tsv, newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle, delimiter="\t"))
    lots = [(lot, lot) for lot in sorted({r["lot"] for r in rows})]
    affiche(rows, lots)


if __name__ == "__main__":
    principal()
