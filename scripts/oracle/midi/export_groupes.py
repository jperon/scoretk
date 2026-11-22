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

"""Groupes d'écart de l'export MIDI par première divergence.

Lit un relevé `mesure.py --ecriture` (JSONL) et les MIDI produits
(`travail/<lot>/<index>/tk.mid`), puis classe chaque fichier non exact par la
première note qui diverge de la référence : `note manquante`, `note sup`,
`durée`, `attaque`, `octave` (même attaque, hauteur à ±12/±24), ou `aucune`
(référence vide). La comparaison est celle de `midi_notes.releve_import` sans
voix : hauteur + attaque + durée sur la grille Q16, exactement le critère N2d
de l'oracle d'écriture.

Aucune donnée tierce n'est copiée : tout est lu dans le dossier de mesure.
"""

from __future__ import annotations

import argparse
import json
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import midi_notes  # noqa: E402


def charger_manifeste(path: Path) -> dict[str, tuple[int, str]]:
    """Index et référence de chaque source (ordre de `mesure.py`).

    Le manifeste d'écriture a deux colonnes : source puis MIDI de référence.
    """
    index: dict[str, tuple[int, str]] = {}
    for position, ligne in enumerate(path.read_text(encoding="utf-8").splitlines()):
        if not ligne.strip():
            continue
        colonnes = ligne.split("\t")
        if len(colonnes) < 2:
            continue
        index.setdefault(colonnes[0], (position, colonnes[1]))
    return index


def analyse(args: tuple[Path, dict, Path, str]) -> dict:
    ref_path, manifeste, travail, lot = args
    lignes = [
        json.loads(ligne)
        for ligne in ref_path.read_text(encoding="utf-8").splitlines()
        if ligne.strip()
    ]
    lignes = [
        ligne
        for ligne in lignes
        if ligne.get("outil") == "scoretk" and ligne.get("ok") and not ligne.get("exclu")
    ]
    resultats = []
    for ligne in lignes:
        if ligne.get("exact") or not ligne.get("ref"):
            continue
        entree = manifeste.get(ligne["fichier"])
        if entree is None:
            continue
        position, reference = entree
        sortie = travail / lot / str(position) / "tk.mid"
        if not sortie.exists() or not reference:
            continue
        try:
            ours = midi_notes.read_midi(str(sortie))
            theirs = midi_notes.read_midi(str(reference))
        except Exception as exc:  # noqa: BLE001 - relevé, pas de conversion
            resultats.append({"fichier": ligne["fichier"], "cause": f"lecture : {exc}"})
            continue
        releve = midi_notes.releve_import(theirs, ours, avec_voix=False)
        resultats.append(
            {
                "fichier": ligne["fichier"],
                "cause": releve["cause"],
                "tick": releve["tick"],
                "n1": releve["n1"],
                "n2": releve["n2"],
                "n2d": releve["n2d"],
                "ref": releve["ref"],
            }
        )
    return {"lot": lot, "resultats": resultats}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--releve", action="append", required=True, metavar="LOT=JSONL")
    ap.add_argument("--manifeste", action="append", required=True, metavar="LOT=MANIFESTE")
    ap.add_argument("--travail", required=True, help="dossier travail/ de la mesure")
    ap.add_argument("--json", help="où écrire le détail")
    ap.add_argument(
        "--jobs",
        type=int,
        default=min(2, __import__("os").cpu_count() or 1),
        help="workers (défaut min(2, cpu))",
    )
    args = ap.parse_args()
    releves = dict(valeur.split("=", 1) for valeur in args.releve)
    manifestes = {
        nom: charger_manifeste(Path(chemin))
        for nom, chemin in (valeur.split("=", 1) for valeur in args.manifeste)
    }
    taches = []
    for lot, chemin in releves.items():
        manifeste = manifestes.get(lot)
        if manifeste is None:
            continue
        taches.append((Path(chemin), manifeste, Path(args.travail), lot))
    with ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        rapports = list(pool.map(analyse, taches))
    detail = {rapport["lot"]: rapport["resultats"] for rapport in rapports}
    if args.json:
        Path(args.json).write_text(json.dumps(detail, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print("| Lot | Première divergence | Fichiers |")
    print("| --- | --- | ---: |")
    for rapport in rapports:
        compte: dict[str, int] = {}
        for resultat in rapport["resultats"]:
            compte[resultat["cause"]] = compte.get(resultat["cause"], 0) + 1
        for cause, nombre in sorted(compte.items(), key=lambda item: (-item[1], item[0])):
            print(f"| {rapport['lot']} | {cause} | {nombre} |")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
