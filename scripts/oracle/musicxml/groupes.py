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

"""Regroupe les fichiers non conformes par première famille musicale différante.

Lit le relevé produit par ``mesure.py``. La première différence est celle que
`crates/scoretk/tests/oracle_core.rs` a extraite des lignes canoniques (niveau
`core`), donc dans l'ordre du document ; aucune heuristique sur le texte n'est
nécessaire pour choisir la famille. Un seul exemple réduit (le fichier avec le
moins de lignes d'écart) est montré par famille.

Usage : groupes.py <mesure.jsonl> [--sortie <rapport.md>]
"""

from __future__ import annotations

import argparse
from collections import defaultdict
import json
from pathlib import Path
import re

FAMILLE_TAG = {
    # Hauteurs / notes
    "note": "notes/pitch", "chord": "notes/pitch", "pitch": "notes/pitch",
    "step": "notes/pitch", "alter": "notes/pitch", "octave": "notes/pitch",
    "rest": "notes/pitch", "unpitched": "notes/pitch", "display-step": "notes/pitch",
    "grace": "notes/pitch", "cue": "notes/pitch",
    "accidental": "notes/pitch", "notehead": "notes/pitch",
    # Durées / rythme
    "duration": "duration/rythme", "type": "duration/rythme", "dot": "duration/rythme",
    "divisions": "duration/rythme", "backup": "duration/rythme", "forward": "duration/rythme",
    "time-modification": "tuplets", "tuplet": "tuplets", "tuplet-number": "tuplets",
    "tuplet-type": "tuplets", "actual-notes": "tuplets", "normal-notes": "tuplets",
    # Liaisons / spanners
    "tie": "ties", "tied": "ties", "slur": "ties", "notations": "ties",
    # Paroles
    "lyric": "lyrics", "syllabic": "lyrics", "text": "lyrics", "extend": "lyrics",
    "elision": "lyrics",
    # Armures / métriques / clefs
    "key": "key", "fifths": "key", "mode": "key", "cancel": "key",
    "time": "time", "beats": "time", "beat-type": "time",
    "clef": "clef", "sign": "clef", "line": "clef", "clef-octave-change": "clef",
    # Mesures / barres / reprises
    "measure": "barlines/repeats", "barline": "barlines/repeats",
    "bar-style": "barlines/repeats", "repeat": "barlines/repeats",
    "ending": "barlines/repeats", "measure-style": "barlines/repeats",
    "measure-repeat": "barlines/repeats", "multiple-rest": "barlines/repeats",
    "sound": "directions", "direction": "directions", "direction-type": "directions",
    "words": "directions", "metronome": "directions", "dynamics": "directions",
    "wedge": "directions",
    # Instruments / transposition / voix / portées
    "transpose": "instruments/transposition", "instrument": "instruments/transposition",
    "score-instrument": "instruments/transposition",
    "midi-instrument": "instruments/transposition", "part-name": "instruments/transposition",
    "midi-channel": "instruments/transposition", "midi-program": "instruments/transposition",
    "volume": "instruments/transposition", "pan": "instruments/transposition",
    "instrument-name": "instruments/transposition", "part-abbreviation": "instruments/transposition",
    "midi-device": "instruments/transposition", "instrument-sound": "instruments/transposition",
    "staff-details": "voices/staves",
    "group-symbol": "structure/part-list", "group-barline": "structure/part-list",
    "harmony": "harmony/chords", "root": "harmony/chords", "kind": "harmony/chords",
    "movement-number": "metadata (artefact norme)", "work": "metadata (artefact norme)",
    "voice": "voices/staves", "staff": "voices/staves", "staves": "voices/staves",
    "attributes": "structure/part-list", "part": "structure/part-list",
    "score-part": "structure/part-list", "part-list": "structure/part-list",
}

# Certaines lignes canoniques ne commencent pas par une balise reconnue.
FAMILLE_REGEX = [
    (re.compile(r"^<(?:flat|sharp|natural|double)"), "notes/pitch"),
    (re.compile(r"^(?:C|D|E|F|G|A|B)[0-9]?$"), "notes/pitch"),
]


def famille(tag: str, ligne: str) -> str:
    if tag in FAMILLE_TAG:
        return FAMILLE_TAG[tag]
    for motif, nom in FAMILLE_REGEX:
        if motif.search(ligne):
            return nom
    return "autre (artefact/métadonnée)"


def deverifier(texte: str) -> str:
    return texte.replace("\\t", "\t").replace("\\n", "\n").replace("\\\\", "\\")


def premiere(row: dict, cle: str) -> tuple[str, str, str]:
    brut = row.get("core", {}).get(cle, "")
    champs = brut.split("|", 2)
    if len(champs) != 3:
        return "", "", ""
    return champs[0], deverifier(champs[1]), deverifier(champs[2])


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("mesure", help="mesure.jsonl produit par mesure.py")
    p.add_argument("--sortie", help="fichier de rapport (défaut : stdout)")
    args = p.parse_args()
    rows = [json.loads(l) for l in Path(args.mesure).read_text(encoding="utf-8").splitlines()]
    groupes: dict[str, list[tuple[int, str, tuple[str, str, str]]]] = defaultdict(list)
    for row in rows:
        if not row.get("ok") or "core" not in row:
            continue
        n = row["core"].get("noyau_lignes", 0)
        if n <= 0:
            continue
        tag, ref, sortie = premiere(row, "premier_noyau")
        groupes[famille(tag, ref)].append((n, row["fichier"], (tag, ref, sortie)))

    lignes = [
        "# Non-conformes par première famille différante",
        "",
        f"Total : {sum(len(v) for v in groupes.values())} fichiers.",
        "",
        "| Famille | Fichiers | Exemple réduit |",
        "| --- | ---: | --- |",
    ]
    for nom, entrees in sorted(groupes.items(), key=lambda kv: -len(kv[1])):
        n, chemin, _ = min(entrees, key=lambda e: e[0])
        lignes.append(f"| {nom} | {len(entrees)} | `{Path(chemin).name}` ({n} lignes) |")
    lignes += ["", "## Exemples réduits", ""]
    for nom, entrees in sorted(groupes.items(), key=lambda kv: -len(kv[1])):
        n, chemin, (tag, ref, sortie) = min(entrees, key=lambda e: e[0])
        lignes += [
            f"### {nom} — {len(entrees)} fichiers",
            "",
            f"`{chemin}` — {n} lignes de noyau, première balise `{tag}`.",
            "",
            "```diff",
            f"- {ref}",
            f"+ {sortie}",
            "```",
            "",
        ]
    texte = "\n".join(lignes) + "\n"
    if args.sortie:
        Path(args.sortie).write_text(texte, encoding="utf-8")
    print(texte)


if __name__ == "__main__":
    main()
