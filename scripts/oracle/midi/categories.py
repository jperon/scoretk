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

"""Ventilation des écarts d'import MIDI par catégorie musicale.

Relit les sorties de `import_mesure.py` (aucune conversion) et attribue à
chaque fichier la catégorie la plus déficiente : pitch, onset, duration,
voice, tuplet, staff split, clef, key ou time sig. Les scores partiels sont
des correspondances de multisets (une note de référence ne peut être
appariée qu'une fois), ce qui permet de classer les causes sans pondérer
par la longueur du morceau.
"""

from __future__ import annotations

import argparse
import collections
from pathlib import Path
import xml.etree.ElementTree as ET

from import_mesure import GRID, TOL, mscx_notes, notes

CATEGORIES = ("pitch", "onset", "duration", "voice",
              "tuplet", "staff split", "clef", "key", "time sig")


def partiel(ref: list[tuple], got: list[tuple], dimension: int) -> float:
    """Part des valeurs de `ref` retrouvées dans `got` pour une dimension."""
    if not ref:
        return 1.0
    remaining = collections.Counter(n[dimension] for n in got)
    found = 0
    for value in (n[dimension] for n in ref):
        if remaining[value]:
            remaining[value] -= 1
            found += 1
    return found / len(ref)


def attributs(path: Path, mscx: bool) -> dict[str, object]:
    """Attributs musicaux d'un MusicXML ou d'un `.mscx` attendu.

    Une balise absente vaut la valeur par défaut de MuseScore : clef de sol,
    armure de do, 4/4. Sans cela, presque tous les fichiers seraient classés
    « clef » alors que la référence n'écrit simplement jamais cette valeur.
    """
    root = ET.parse(path).getroot()
    if mscx:
        tup = len(root.findall(".//Tuplet"))
        staff = len(root.findall(".//Part"))
        clef = "G2"
        for element in root.findall(".//concertClefType"):
            clef = (element.text or "G").strip()[:2]
            break
        key = 0
        for element in root.findall(".//KeySig"):
            value = element.findtext("accidental")
            if value is not None:
                key = int(value)
                break
        timesig = (4, 4)
        for element in root.findall(".//TimeSig"):
            n, d = element.findtext("sigN"), element.findtext("sigD")
            if n and d:
                timesig = (int(n), int(d))
                break
    else:
        tup = len(root.findall(".//time-modification"))
        staff = len(root.findall("part"))
        clef = "G2"
        for element in root.findall(".//clef"):
            sign, line = element.findtext("sign", "G"), element.findtext("line", "2")
            clef = f"{sign}{line}"
            break
        key = 0
        for element in root.findall(".//fifths"):
            key = int(element.text or "0")
            break
        timesig = (4, 4)
        for element in root.findall(".//time"):
            b, bt = element.findtext("beats"), element.findtext("beat-type")
            if b and bt:
                timesig = (int(b), int(bt))
                break
    return {"tuplet": tup, "staff": staff, "clef": clef, "key": key, "time": timesig}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--mesure", required=True, help="dossier de sortie de import_mesure.py")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    src, dst = Path(args.mesure), Path(args.out)
    dst.mkdir(parents=True, exist_ok=True)
    upstream = Path("/home/jacques/programmes/musique/musescore/"
                    "ref-musescore/src/importexport/midi/tests/midiimport_data")
    lignes = []
    for row in map(__import__("json").loads, (src / "import.jsonl").read_text().splitlines()):
        if "erreur" in row or not row["id"].startswith("up-"):
            continue
        ident = row["id"][3:]
        d = src / "travail" / row["id"]
        ref = mscx_notes(upstream / f"{ident}-ref.mscx")
        got = notes(d / "got.musicxml")
        a_ref = attributs(upstream / f"{ident}-ref.mscx", True)
        a_got = attributs(d / "got.musicxml", False)
        scores = {
            "pitch": partiel(ref, got, 0),
            "onset": partiel(ref, got, 1),
            "duration": partiel(ref, got, 2),
            "voice": partiel(ref, got, 3),
        }
        # Attributs structuraux : égalité des valeurs (avec defaults).
        for nom, cle in (("staff split", "staff"), ("clef", "clef"),
                         ("key", "key"), ("time sig", "time")):
            scores[nom] = 1.0 if a_ref[cle] == a_got[cle] else 0.0
        scores["tuplet"] = (
            1.0 if bool(a_ref["tuplet"]) == bool(a_got["tuplet"]) else 0.0)
        pire = min(CATEGORIES, key=lambda c: scores[c])
        lignes.append({"id": ident, "ref_notes": len(ref), "score": row["score"],
                       "pire": pire, "scores": {k: round(v, 4) for k, v in scores.items()},
                       "tuplets_ref": a_ref["tuplet"], "tuplets_got": a_got["tuplet"]})

    (dst / "categories.jsonl").write_text(
        "".join(__import__("json").dumps(l, ensure_ascii=False) + "\n" for l in lignes),
        encoding="utf-8")
    pertes = collections.Counter(l["pire"] for l in lignes)
    total = sum(l["ref_notes"] * (1 - l["score"]) for l in lignes)
    blocs = ["# Ventilation des pertes d'import MIDI (upstream)",
             "",
             "| Catégorie | Fichiers | Pertes pondérées (notes) |",
             "| --- | ---: | ---: |"]
    for cat in sorted(CATEGORIES, key=lambda c: -sum(
            l["ref_notes"] * (1 - l["score"]) for l in lignes if l["pire"] == c)):
        n = pertes[cat]
        p = sum(l["ref_notes"] * (1 - l["score"]) for l in lignes if l["pire"] == cat)
        blocs.append(f"| {cat} | {n} | {p:.1f} |")
    blocs += ["", "## Fichiers les plus déficients", "",
              "| Fichier | Notes | Score | Pire | pitch | onset | durée | voix | n-olets ref/got |",
              "| --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: |"]
    for l in sorted(lignes, key=lambda l: l["ref_notes"] * (1 - l["score"]), reverse=True)[:30]:
        s = l["scores"]
        blocs.append(f"| {l['id']} | {l['ref_notes']} | {l['score']:.4f} | {l['pire']} | "
                     f"{s['pitch']:.2f} | {s['onset']:.2f} | {s['duration']:.2f} | "
                     f"{s['voice']:.2f} | {l['tuplets_ref']}/{l['tuplets_got']} |")
    (dst / "categories.md").write_text("\n".join(blocs) + "\n", encoding="utf-8")
    print("\n".join(blocs[:9]))
    print(f"pertes totales ≈ {total:.1f} notes")


if __name__ == "__main__":
    main()
