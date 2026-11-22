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

"""Classe les fichiers ``notes_supp`` par première attaque en trop.

Pour chaque fichier mesuré portant le drapeau ``notes_supp`` et non exact, le
script repère la première attaque que scoretk ajoute par rapport au MIDI de
LilyPond (plus petit couple (attaque quantifiée, hauteur) en surnombre) et
décrit son voisinage dans le MusicXML du candidat :

* ``dup`` : la hauteur et l'attaque se répètent dans la **même** voix et la
  même portée (doublon interne) ;
* ``other`` : la même hauteur et la même attaque existent sur une autre
  portée, une autre voix ou une autre partie (doublon déplacé) ;
* ``grace`` : l'attaque en trop est une note de grâce écrite ;
* ``end`` : elle tombe sur la dernière attaque de la référence ;
* ``t0`` : elle est au tick 0.

L'attaque et la hauteur viennent de ``ly_notes.read_xml`` (oracle) ; la
portée, la voix et la partie sont relues dans le MusicXML pour situer le
doublon. Sert à choisir la prochaine cause à corriger, pas à décider seul.

Usage : ``BASE=import-mut66 python3 scripts/oracle/lilypond/supp_first.py``.
``PARALLEL`` règle le nombre de fils (2 par défaut : la machine est partagée).
"""

import collections
import json
import os
import re
import sys
import xml.etree.ElementTree as ET
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from ly_notes import quant, read_midi, read_xml  # noqa: E402

STEPS = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
RACINE = Path(__file__).resolve().parents[3]
BASE = os.environ.get("BASE", "import-mut66")
PARALLEL = int(os.environ.get("PARALLEL", min(2, os.cpu_count() or 1)))
MESURES = RACINE / "target/mesures"
MANIFESTE = MESURES / "mut-manifest.txt"


def notes_musicxml(chemin: Path) -> list[tuple]:
    """(hauteur, attaque, partie, portée, voix, grâce) de chaque note."""
    notes = []
    racine = ET.parse(chemin).getroot()
    for partie, part in enumerate(racine.findall("part")):
        divisions = 1
        debut = 0.0
        for measure in part.findall("measure"):
            curseur = debut
            extension = 0.0
            precedente = debut
            for elem in measure:
                if elem.tag == "attributes":
                    d = elem.find("divisions")
                    if d is not None and d.text:
                        divisions = max(1, int(float(d.text)))
                elif elem.tag in ("backup", "forward"):
                    valeur = elem.findtext("duration")
                    if not valeur:
                        continue
                    delta = int(float(valeur)) / divisions
                    curseur += delta if elem.tag == "forward" else -delta
                    extension = max(extension, curseur - debut)
                elif elem.tag == "note":
                    duree = int(float(elem.findtext("duration", "0"))) / divisions
                    grace = elem.find("grace") is not None
                    accord = elem.find("chord") is not None
                    attaque = precedente if accord else curseur
                    if not accord:
                        precedente = attaque
                        if not grace:
                            curseur = attaque + duree
                            extension = max(extension, curseur - debut)
                    pas = elem.findtext("pitch/step")
                    octave = elem.findtext("pitch/octave")
                    if pas and octave:
                        hauteur = 12 * (int(octave) + 1) + STEPS[pas]
                        notes.append(
                            (
                                hauteur,
                                attaque,
                                partie,
                                elem.findtext("staff") or "",
                                elem.findtext("voice") or "",
                                grace,
                            )
                        )
            debut += extension
    return notes


def motifs(source: str) -> list[str]:
    table = [
        ("times", r"\\times\b|\\tuplet\b"),
        ("grace", r"\\(grace|appoggiatura|acciaccatura|afterGrace)\b"),
        ("repeat", r"\\repeat\b"),
        ("tiewait", r"tieWaitForNote"),
        ("change", r"\\change\b"),
        ("ottava", r"\\ottava\b"),
        ("partial", r"\\partial\b"),
        ("transpose", r"\\transpose\b"),
    ]
    return [nom for nom, rx in table if re.search(rx, source)]


def source_et_includes(fichier: str) -> str:
    try:
        texte = Path(fichier).read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return ""
    base = Path(fichier).parent
    for include in re.finditer(r'\\include\s+"([^"]+)"', texte):
        try:
            texte += "\n" + (base / include.group(1)).read_text(
                encoding="utf-8", errors="ignore"
            )
        except OSError:
            pass
    return texte


def classer(ligne: dict, index: dict[str, int]) -> dict | None:
    travail = MESURES / BASE / "travail/mut" / str(index[ligne["fichier"]])
    if not (travail / "tk.musicxml").exists():
        return None
    reference = read_midi(str(travail / "ref.mid"))
    test = read_xml(str(travail / "tk.musicxml"), with_grace=True)
    situees = notes_musicxml(travail / "tk.musicxml")
    attendu = collections.Counter((n[0], quant(n[1])) for n in reference)
    obtenu = collections.Counter((n[0], quant(n[1])) for n in test)
    en_trop = list((obtenu - attendu).items())
    if not en_trop:
        return None
    (hauteur, attaque), _ = min(en_trop, key=lambda x: (x[0][1], x[0][0]))
    meme = [x for x in situees if x[0] == hauteur and quant(x[1]) == attaque]
    meme_voix = (
        len(
            [
                x
                for x in meme
                if (x[2], x[3], x[4]) == (meme[0][2], meme[0][3], meme[0][4])
            ]
        )
        > 1
        if meme
        else False
    )
    ailleurs = (
        any((x[2], x[3], x[4]) != (meme[0][2], meme[0][3], meme[0][4]) for x in meme)
        if meme
        else False
    )
    grace = any(x[3] and x[0] == hauteur and quant(x[1]) == attaque for x in test)
    fin = abs(attaque - max((x[1] for x in reference), default=0)) < 0.01
    return {
        "file": Path(ligne["fichier"]).name,
        "ref": len(reference),
        "tick": attaque,
        "pitch": hauteur,
        "n": sum(c for _, c in en_trop),
        "dup": meme_voix,
        "other": ailleurs,
        "grace": grace,
        "end": fin,
        "t0": attaque == 0.0,
        "tokens": motifs(source_et_includes(ligne["fichier"])),
    }


def main() -> None:
    lignes = [
        json.loads(ligne)
        for ligne in open(MESURES / BASE / "mut.jsonl", encoding="utf-8")
        if json.loads(ligne).get("outil") == "scoretk"
        and json.loads(ligne).get("ok")
        and json.loads(ligne).get("ref")
    ]
    manifeste = [ligne.rstrip("\n") for ligne in open(MANIFESTE, encoding="utf-8")]
    index: dict[str, int] = {}
    for position, chemin in enumerate(manifeste):
        index.setdefault(chemin, position)
    choisis = [
        ligne
        for ligne in lignes
        if "notes_supp" in ligne.get("drapeaux", []) and not ligne["exact"]
    ]
    with ThreadPoolExecutor(max_workers=PARALLEL) as pool:
        resultats = [x for x in pool.map(lambda l: classer(l, index), choisis) if x]
    print("files", len(resultats), "extra", sum(x["n"] for x in resultats))
    print(
        "kind counts: dup_same_voice",
        sum(x["dup"] for x in resultats),
        "other_place",
        sum(x["other"] for x in resultats),
        "grace",
        sum(x["grace"] for x in resultats),
        "end",
        sum(x["end"] for x in resultats),
        "tick0",
        sum(x["t0"] for x in resultats),
    )
    print(
        "tokens",
        collections.Counter(t for x in resultats for t in x["tokens"]).most_common(),
    )
    for x in sorted(resultats, key=lambda x: (x["tick"], x["ref"])):
        print(
            f"{x['file'][:40]:40} ref={x['ref']:4} first=({x['pitch']},{x['tick']})"
            f" n={x['n']:3} dup={int(x['dup'])} other={int(x['other'])}"
            f" grace={int(x['grace'])} end={int(x['end'])} toks={','.join(x['tokens'])}"
        )


if __name__ == "__main__":
    main()
