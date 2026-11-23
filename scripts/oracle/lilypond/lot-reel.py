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
# this program. If not, see <https://www.gnu.org/licenses/gpl-3.0.html>.

r"""Construit le lot « réel » (`rw`) : Mutopia + openLilyLib + LSR.

Les données ne sont **pas** copiées dans le dépôt : ce script les télécharge
dans un dossier de travail (défaut : /tmp/rw-corpus) et écrit un manifeste de
chemins absolus, dans le format des autres lots (`lots.sh`). Il ne lit ni
n'exécute aucun fichier téléchargé ; la génération des références MIDI est un
pas séparé (`regen-refs.py`, qui compile les sources avec LilyPond).

Sources et licences (à recopier dans `docs/agents/lilypond-scheme.md`) :

* Mutopia — chaque pièce porte sa licence dans son `\header` (domaine public,
  CC BY-SA, CC BY, GPL…). Le lot réutilise le corpus Mutopia déjà présent
  localement (`mut.txt`), aucun fichier n'est téléchargé.
* openLilyLib — GPL-3.0-or-later (dépôt `openlilylib/openlilylib`), commit
  épinglé ci-dessous.
* LSR (LilyPond Snippet Repository) — les extraits de
  `Documentation/snippets` du dépôt LilyPond, GPL-3.0-or-later / GFDL selon
  le fichier ; commit épinglé ci-dessous.

Usage :
  python3 lot-reel.py --work /tmp/rw-corpus --out /tmp/ly-import-lots/manifestes/rw.txt
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import urllib.request
from pathlib import Path

# Épinglés le 2026-11-23 (voir le rapport de mesure).
OPENLILY_COMMIT = "3d6a145ab094a1d6e7f3c52e0a7f44da9dd56a34"
LILYPOND_COMMIT = "master"
# Nombre d'extraits LSR ; pris dans l'ordre alphabétique pour être reproductible.
LSR_COUNT = 40
OPENLILY_REPO = "https://github.com/openlilylib/openlilylib.git"
LILYPOND_RAW = "https://raw.githubusercontent.com/lilypond/lilypond/{commit}/Documentation/snippets/{name}"
MUT_MANIFEST = "/tmp/ly-import-lots/manifestes/mut.txt"


def telecharger(url: str, cible: Path) -> None:
    cible.parent.mkdir(parents=True, exist_ok=True)
    if cible.exists():
        return
    with urllib.request.urlopen(url, timeout=60) as reponse:
        cible.write_bytes(reponse.read())


def clone_openlily(work: Path) -> Path:
    cible = work / "openlilylib"
    if not (cible / ".git").exists():
        subprocess.run(
            ["git", "clone", "--depth", "1", "-q", OPENLILY_REPO, str(cible)],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(cible), "fetch", "-q", "--depth", "1", "origin", OPENLILY_COMMIT],
            check=False,
        )
        subprocess.run(
            ["git", "-C", str(cible), "checkout", "-q", OPENLILY_COMMIT],
            check=False,
        )
    return cible


def extraits_lsr(work: Path) -> list[Path]:
    """Télécharge les `LSR_COUNT` premiers extraits du dépôt LilyPond."""
    api = (
        "https://api.github.com/repos/lilypond/lilypond/contents/"
        f"Documentation/snippets?ref={LILYPOND_COMMIT}"
    )
    with urllib.request.urlopen(api, timeout=60) as reponse:
        entrees = json.load(reponse)
    noms = sorted(
        entree["name"]
        for entree in entrees
        if entree.get("type") == "file" and entree["name"].endswith(".ly")
    )
    fichiers = []
    for nom in noms[:LSR_COUNT]:
        cible = work / "lsr" / nom
        telecharger(LILYPOND_RAW.format(commit=LILYPOND_COMMIT, name=nom), cible)
        fichiers.append(cible)
    return fichiers


def mutopia() -> list[Path]:
    manifeste = Path(MUT_MANIFEST)
    if not manifeste.exists():
        return []
    return [
        Path(ligne.strip())
        for ligne in manifeste.read_text(encoding="utf-8").splitlines()
        if ligne.strip()
    ]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work", default="/tmp/rw-corpus")
    parser.add_argument("--out", default="/tmp/ly-import-lots/manifestes/rw.txt")
    args = parser.parse_args()

    work = Path(args.work)
    work.mkdir(parents=True, exist_ok=True)
    openlily = clone_openlily(work)
    # Fichiers d'exemple qui portent un `\score` : les seuls qui peuvent donner
    # un MIDI une fois `\midi { }` injecté (comme pour les lots existants).
    exemples = sorted(
        chemin
        for chemin in openlily.rglob("*.ly")
        if "\\score" in chemin.read_text(encoding="utf-8", errors="replace")
    )
    lsr = extraits_lsr(work)
    mut = mutopia()
    chemins = mut + exemples + lsr
    sortie = Path(args.out)
    sortie.parent.mkdir(parents=True, exist_ok=True)
    sortie.write_text("\n".join(str(chemin) for chemin in chemins) + "\n", encoding="utf-8")
    print(
        f"{len(chemins)} fichiers : {len(mut)} Mutopia, {len(exemples)} openLilyLib, "
        f"{len(lsr)} LSR -> {sortie}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
