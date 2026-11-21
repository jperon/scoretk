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

"""Oracle d'import MIDI : relevé par fichier, comparaison des notes du modèle.

Deux lots sans donnée versionnée :

* les tests amont de MuseScore (`importexport/midi/tests/midiimport_data`) :
  un `.mid` et son `.mscx` attendu (lecteur C++) ;
* les MIDI LilyPond de `~/Musique` dont la source `.ly` est lue : le `.ly`
  fournit la quantification idéale (hauteurs, attaques, durées).

Pour chaque paire, scoretk lit le MIDI et le `.mscx`/`.ly` de référence, et on
compare les notes `(hauteur, attaque, durée, voix)` de deux exports MusicXML
sur une grille de 1/16 de noire (+/- 1/16 de tolérance), comme `midi_notes.py`.
Seules les mesures sont lues : aucun fichier tiers n'est copié dans le dépôt.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import xml.etree.ElementTree as ET

GRID = 0.25
TOL = 0.25
LIMITE = 15


def notes(path: Path) -> list[tuple[int, float, float, int]]:
    """Notes d'un MusicXML partwise : hauteur, attaque, durée, voix (noires)."""
    root = ET.parse(path).getroot()
    out: list[tuple[int, float, float, int]] = []
    for part in root.findall("part"):
        divisions = 1
        tick = 0.0
        for measure in part.findall("measure"):
            offset = 0.0
            for child in measure:
                if child.tag == "attributes":
                    value = child.findtext("divisions")
                    if value:
                        divisions = int(value)
                elif child.tag in ("backup", "forward"):
                    delta = int(child.findtext("duration", "0")) / divisions
                    offset += -delta if child.tag == "backup" else delta
                elif child.tag == "note":
                    dur = int(child.findtext("duration", "0")) / divisions
                    if child.find("rest") is None and child.find("chord") is None:
                        pitch = child.find("pitch")
                        if pitch is not None:
                            step = pitch.findtext("step", "C")
                            semi = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}[step]
                            semi += int(pitch.findtext("alter", "0"))
                            midi = 12 * (int(pitch.findtext("octave", "0")) + 1) + semi
                            voice = int(child.findtext("voice", "1"))
                            out.append((midi, round((tick + offset) / GRID) * GRID,
                                        round(dur / GRID) * GRID, voice))
                    offset += dur
            tick += offset
    return out


DUREES = {
    "whole": 4.0, "half": 2.0, "quarter": 1.0, "eighth": 0.5, "16th": 0.25,
    "32nd": 0.125, "64th": 0.0625, "128th": 0.03125, "measure": 4.0,
}


def mscx_notes(path: Path) -> list[tuple[int, float, float, int]]:
    """Notes d'un `.mscx` attendu, en noires, sans passer par notre exporteur.

    Le `.mscx` stocke la hauteur réelle : le convertir en MusicXML appliquerait
    la transposition de l'instrument et décalerait la référence d'une octave.
    """
    root = ET.parse(path).getroot()
    score_el = root.find("Score")
    if score_el is None:
        return []
    out: list[tuple[int, float, float, int]] = []
    for staff in score_el.findall("Staff"):
        if not staff.findall("Measure"):
            continue  # déclaration de portée sous <Part>
        tick = 0.0
        for measure in staff.findall("Measure"):
            for voice_index, voice in enumerate(measure.findall("voice"), start=1):
                cursor = tick
                for child in voice:
                    if child.tag == "location":
                        frac = child.findtext("fractions", "0/1")
                        num, _, den = frac.partition("/")
                        cursor += int(num) / int(den or 1)
                    elif child.tag in ("Chord", "Rest"):
                        duree = child.findtext("duration")
                        if duree:
                            num, _, den = duree.partition("/")
                            length = float(num) / float(den or 1) * 4.0
                        else:
                            length = DUREES.get(child.findtext("durationType", "quarter"), 1.0)
                        if child.tag == "Chord":
                            for note in child.findall("Note"):
                                pitch = note.findtext("pitch")
                                if pitch is not None:
                                    out.append((int(pitch), round(cursor / GRID) * GRID,
                                                round(length / GRID) * GRID, voice_index))
                        cursor += length
            tick += 4.0  # mesure nominale ; les cas 3/4 sont traités par <len>
            len_txt = measure.get("len")
            if len_txt:
                num, _, den = len_txt.partition("/")
                tick = (tick - 4.0) + float(num) / float(den or 1) * 4.0
    return out


def score(ref: list[tuple], got: list[tuple]) -> float:
    """Part des notes de référence retrouvées (hauteur, attaque, durée, voix)."""
    if not ref:
        return 1.0
    remaining = list(got)
    found = 0
    for pitch, onset, dur, voice in ref:
        for index, candidate in enumerate(remaining):
            cp, co, cd, cv = candidate
            if cp == pitch and cv == voice and abs(co - onset) <= TOL and abs(cd - dur) <= TOL:
                remaining.pop(index)
                found += 1
                break
    return found / len(ref)


def run(binary: str, src: Path, dst: Path) -> bool:
    cmd = ["timeout", str(LIMITE), "prlimit", "--as=2000000000",
           binary, str(src), "--to", "musicxml", "-o", str(dst)]
    return subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0


def paires_upstream(data: Path) -> list[tuple[str, Path, Path]]:
    return [(f"up-{p.stem}", p, p.with_name(p.stem + "-ref.mscx"))
            for p in sorted(data.glob("*.mid")) if p.with_name(p.stem + "-ref.mscx").exists()]


def paires_lilypond() -> list[tuple[str, Path, Path]]:
    out = []
    for ly in sorted(Path("/home/jacques/Musique").rglob("*.ly")):
        mid = ly.with_suffix(".mid")
        if mid.exists():
            out.append((f"ly-{ly.stem}", mid, ly))
    return out


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--binary", required=True)
    ap.add_argument("--out", required=True, help="dossier des sorties (hors dépôt)")
    ap.add_argument("--upstream", default="")
    ap.add_argument("--limit", type=int, default=0)
    args = ap.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    paires = []
    if args.upstream:
        paires += paires_upstream(Path(args.upstream))
    paires += paires_lilypond()
    if args.limit:
        paires = paires[:args.limit]
    travail = out / "travail"
    travail.mkdir(exist_ok=True)
    with (out / "import.jsonl").open("w", encoding="utf-8") as fic:
        for ident, mid, ref in paires:
            d = travail / ident
            d.mkdir(exist_ok=True)
            got_xml = d / "got.musicxml"
            row = {"id": ident, "mid": str(mid), "ref": str(ref), "score": 0.0}
            try:
                a = mscx_notes(ref) if ref.suffix == ".mscx" else notes(ref)
            except Exception as exc:
                a = []
                row["erreur"] = f"reference : {exc}"[:200]
            if run(args.binary, mid, got_xml):
                try:
                    b = notes(got_xml)
                    row.update(ref_notes=len(a), got_notes=len(b), score=round(score(a, b), 4))
                except Exception as exc:
                    row["erreur"] = str(exc)[:200]
            else:
                row["erreur"] = "conversion"
            fic.write(json.dumps(row, ensure_ascii=False) + "\n")
            fic.flush()
    rows = [json.loads(l) for l in (out / "import.jsonl").read_text().splitlines()]
    ok = [r for r in rows if "erreur" not in r]
    total = sum(r.get("ref_notes", 0) for r in ok)
    found = sum(r.get("ref_notes", 0) * r["score"] for r in ok)
    resume = [f"# MIDI import oracle — {len(ok)}/{len(rows)} files",
              f"# N (pitch+onset+duration+voice, Q{int(1/GRID)}, tol {TOL}): "
              f"{found/total:.4f}" if total else "",
              "", "| File | Ref notes | Got notes | Score |", "| --- | ---: | ---: | ---: |"]
    for r in sorted(rows, key=lambda r: r["score"]):
        resume.append(f"| {r['id']} | {r.get('ref_notes', 0)} | {r.get('got_notes', 0)} | "
                      f"{r['score']:.4f} |")
    (out / "resume.md").write_text("\n".join(resume) + "\n", encoding="utf-8")
    print("\n".join(l for l in resume if l.startswith("#")))


if __name__ == "__main__":
    main()
