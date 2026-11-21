#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon
#
# This program is free software: you can redistribute it and/or modify it
# under the terms of the GNU General Public License version 3 as published by
# the Free Software Foundation.

"""N3 : comparaison des **fonctions musicales** de deux exports MusicXML.

N1 (hauteurs) et N2 (attaques) mesurent les notes ; N3 mesure ce que le lecteur
ABC doit porter en plus : paroles `w:`, liaisons, slurs, triolets, notes de
grâce, ornements, symboles d'accord, reprises/voltas, armures, indications de
mesure, clefs et voix. La référence est l'`abc2xml.py` de la campagne, pas
`abc2midi` (qui ne represents pas ces éléments).

Usage : n3.py <dossier-sortie> [mesures.tsv]
Sortie : n3.tsv (lot, id, fichier, X:, catégorie, scoretk, alternatif)
"""

import argparse
import collections
import csv
import os
import sys
import xml.etree.ElementTree as ET

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import common  # noqa: E402

CATEGORIES = (
    "lyrics", "ties", "slurs", "tuplets", "graces", "decorations",
    "harmony", "repeats", "voltas", "keys", "times", "clefs", "voices",
)


def features(path):
    """Compteurs des catégories N3 d'un document MusicXML partwise."""
    out = collections.Counter()
    try:
        root = ET.parse(path).getroot()
    except (ET.ParseError, OSError):
        return out
    for part in root.iter("part"):
        voices = set()
        for element in part.iter():
            tag = element.tag
            if tag == "lyric":
                out["lyrics"] += 1
            elif tag == "tie" and element.get("type") == "start":
                out["ties"] += 1
            elif tag == "slur" and element.get("type") == "start":
                out["slurs"] += 1
            elif tag == "time-modification":
                out["tuplets"] += 1
            elif tag == "grace":
                out["graces"] += 1
            # `<notations>` est un conteneur générique (slurs, ties…) : le
            # compter comme ornement fausse la catégorie decorations dès que
            # l'export écrit des slurs.
            elif tag in ("words", "other-technical", "ornaments", "fermata"):
                out["decorations"] += 1
            elif tag == "harmony":
                out["harmony"] += 1
            elif tag == "repeat":
                out["repeats"] += 1
            elif tag == "ending":
                out["voltas"] += 1
            elif tag == "key":
                out["keys"] += 1
            elif tag == "time":
                out["times"] += 1
            elif tag == "clef":
                out["clefs"] += 1
            elif tag == "voice" and element.text:
                voices.add(element.text.strip())
        out["voices"] += len(voices)
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("out")
    parser.add_argument("mesures", nargs="?")
    args = parser.parse_args()
    mesures = args.mesures or os.path.join(args.out, "mesures.tsv")
    rows = [r for r in csv.DictReader(open(mesures, encoding="utf-8"), delimiter="\t")
            if r["candidat"] == "scoretk" and r.get("statut") != common.STATUT]
    target = os.path.join(args.out, "n3.tsv")
    with open(target, "w", encoding="utf-8", newline="") as handle:
        writer = csv.writer(handle, delimiter="\t")
        writer.writerow(("lot", "id", "fichier", "x", "categorie", "scoretk", "alternatif"))
        for row in rows:
            base = os.path.join(args.out, "brut", row["id"])
            tk = features(os.path.join(base, "score.musicxml"))
            alt = features(os.path.join(base, "alt", "air.xml"))
            for category in CATEGORIES:
                if tk[category] or alt[category]:
                    writer.writerow((row["lot"], row["id"], row["fichier"], row["x"],
                                     category, tk[category], alt[category]))
    print(f"N3 : {target}")


if __name__ == "__main__":
    main()
