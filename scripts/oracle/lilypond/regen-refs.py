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

"""Régénère un sous-ensemble de références MIDI avec le LilyPond installé.

Un `ly.midi` produit par une autre version n'est pas un oracle : par exemple
les références `\new ChordNames` de `ra` ne contiennent qu'une piste, alors
que LilyPond 2.26 en produit deux (grille + portée). Ce script recompile les
sources listées en injectant un `\\midi { }` dans un `\\score` (les sources du
corpus n'en ont souvent pas), avec le compilateur courant.

Usage :
  python3 regen-refs.py --manifest /tmp/ly95-chordnames.txt \\
      --refs /tmp/ly95lect-refs-fresh --lot ra --out /tmp/ly95lect-refs-fixed \\
      --jobs 2
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from mesure import nom_court  # noqa: E402


def candidats_avec_midi(source: str) -> list[str]:
    r"""Positions possibles d'un `\score { … \midi { } }` pour une source.

    Les sources du corpus sont souvent des scores implicites avec des
    définitions et des blocs `\paper`/`\layout` : selon le fichier, la
    première expression musicale se trouve à des endroits variés. On propose
    plusieurs points de coupe et `regenerer` garde celui qui compile.
    """
    candidats: list[str] = []
    if re.search(r"\\score\b", source):
        debut = source.rfind("\\score")
        ouverture = source.find("{", debut)
        if ouverture >= 0:
            profondeur = 0
            for i in range(ouverture, len(source)):
                if source[i] == "{":
                    profondeur += 1
                elif source[i] == "}":
                    profondeur -= 1
                    if profondeur == 0:
                        candidats.append(source[:i] + "\n  \\midi { }\n" + source[i:])
                        break
        return candidats
    lignes = source.splitlines(keepends=True)
    tetes = re.compile(
        r"\\(?:new|context|relative|fixed|chordmode|drummode|transpose|"
        r"times|repeat|override|set|once|partial|time|key|clef|tempo|mark)\b|^\s*(<<|\{)"
    )
    positions: list[int] = []
    profondeur = 0
    for i, ligne in enumerate(lignes):
        nettoyee = ligne.split("%", 1)[0].strip()
        if profondeur == 0 and tetes.search(nettoyee) and "=" not in nettoyee:
            positions.append(i)
        profondeur += ligne.count("{") - ligne.count("}")
    # Essayer d'abord les positions les plus tardives (le cœur du score),
    # puis les plus tôt (quand le score englobe tout).
    for i in reversed(positions):
        candidats.append(
            "".join(lignes[:i])
            + "\n\\score {\n"
            + "".join(lignes[i:])
            + "\n\\midi { }\n}\n"
        )
    return candidats


def regenerer(tache: tuple) -> dict:
    src, refs, lot, out = tache
    source = Path(src)
    if not source.is_file():
        return {"fichier": src, "ok": False, "erreur": "source absente"}
    try:
        candidats = candidats_avec_midi(source.read_text(encoding="utf-8", errors="replace"))
    except OSError as exc:
        return {"fichier": src, "ok": False, "erreur": str(exc)[:100]}
    if not candidats:
        return {"fichier": src, "ok": False, "erreur": "pas de score injectable"}
    destination = Path(out) / lot / nom_court(src)
    destination.mkdir(parents=True, exist_ok=True)
    midi = destination / "ly.midi"
    original = Path(refs) / lot / nom_court(src) / "ly.midi"
    ancien = original.read_bytes() if original.is_file() else b""
    # La copie est faite par liens durs : détacher la cible avant d'écrire,
    # sinon on modifierait aussi l'arbre de références d'origine.
    midi.unlink(missing_ok=True)
    tmp = destination / "_regen.ly"
    erreur = "aucun candidat n'a compilé"
    for texte in candidats:
        tmp.write_text(texte, encoding="utf-8")
        midi.unlink(missing_ok=True)
        cp = subprocess.run(
            ["lilypond", "-dno-print-pages", "-dmidi-extension=midi",
             "-o", str(destination / "ly"), str(tmp)],
            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=120, check=False,
        )
        if midi.is_file() and midi.stat().st_size > 0:
            (destination / "ly.midi.orig").write_bytes(ancien)
            tmp.unlink(missing_ok=True)
            for reste in destination.glob("ly.*"):
                if reste.suffix not in (".midi", ".orig"):
                    reste.unlink()
            return {"fichier": src, "ok": True, "erreur": ""}
        erreur = cp.stderr.decode("utf-8", "replace")[-200:]
    tmp.unlink(missing_ok=True)
    return {"fichier": src, "ok": False, "erreur": erreur}


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--manifest", required=True)
    p.add_argument("--refs", required=True, help="racine des références d'origine")
    p.add_argument("--out", required=True, help="racine de la copie corrigée")
    p.add_argument("--lot", default="ra")
    p.add_argument("--jobs", type=int, default=min(2, os.cpu_count() or 1))
    args = p.parse_args()
    srcs = [l.strip() for l in Path(args.manifest).read_text().splitlines() if l.strip()]
    # Copie en liens durs : seul le sous-ensemble régénéré est remplacé.
    original = Path(args.refs) / args.lot
    copie = Path(args.out) / args.lot
    if not copie.exists():
        shutil.copytree(original, copie, copy_function=os.link)
    taches = [(src, args.refs, args.lot, args.out) for src in srcs]
    with ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        resultats = list(pool.map(regenerer, taches))
    for r in resultats:
        print(("OK  " if r["ok"] else "ECHEC ") + Path(r["fichier"]).name + " " + r["erreur"])
    return 0 if all(r["ok"] for r in resultats) else 1


if __name__ == "__main__":
    raise SystemExit(main())
