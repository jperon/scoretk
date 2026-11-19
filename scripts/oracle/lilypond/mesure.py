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

"""Pilote de mesure de la lecture LilyPond vers MusicXML.

La référence est le MIDI produit par LilyPond (``ly.midi``), fourni par
``--refs`` ou régénéré à la volée. Trois candidats sont mesurés avec le même
oracle : scoretk, ``ly musicxml`` (python-ly) et ``music21.converter.parse``.
music21 ne sait pas lire le ``.ly`` : l'échec est enregistré comme donnée de
mesure, pas masqué.

Tous les lancements passent par ``timeout 10 prlimit --as=2000000000``. Les
manifestes sont des listes de chemins absolus, un par ligne ; aucune donnée
tierce n'est versionnée, les sorties brutes vont dans ``--out`` (hors dépôt).
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import time

from ly_notes import GRID, TOL, compare, read_midi, read_xml

TIMEOUT = 10
TIMEOUT_LY = 20

# music21 n'a pas de parser LilyPond : on tente quand même la lecture afin que
# l'erreur réelle figure dans le relevé. Le code 3 distingue un refus de format
# d'un échec de conversion interne.
M21_CODE = (
    "import sys, music21\n"
    "try:\n"
    "    s = music21.converter.parse(sys.argv[1])\n"
    "except Exception as e:\n"
    "    print(type(e).__name__ + ': ' + str(e), file=sys.stderr)\n"
    "    sys.exit(3)\n"
    "s.write('musicxml', fp=sys.argv[2])\n"
)


def lire_manifest(path: str) -> list[str]:
    with open(path, encoding="utf-8") as f:
        return [l.strip() for l in f if l.strip() and not l.startswith("#")]


def nom_court(src: str) -> str:
    """Nom stable <md5 du chemin>_<basename>, comme l'oracle historique.

    L'oracle historique fait ``echo "$f" | md5sum`` : le newline final fait
    partie du hash, on le reproduit exactement pour retrouver les ly.midi.
    """
    h = hashlib.md5((src + "\n").encode("utf-8")).hexdigest()[:8]
    b = re.sub(r"[^A-Za-z0-9_-]", "_", Path(src).stem)[:30]
    return f"{h}_{b}"


def lancer(commande: list[str], budget: int = TIMEOUT) -> tuple[int, float, str]:
    """Exécute une conversion bornée ; renvoie (code, secondes, message)."""
    debut = time.monotonic()
    try:
        p = subprocess.run(
            commande,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            timeout=budget + 3,
            check=False,
        )
        return p.returncode, time.monotonic() - debut, p.stderr.decode("utf-8", "replace")[-400:]
    except subprocess.TimeoutExpired:
        return 124, time.monotonic() - debut, "dépassement de la borne de temps"


def borne(commande: list[str]) -> list[str]:
    return ["timeout", str(TIMEOUT), "prlimit", "--as=2000000000"] + commande


def outils(args) -> list[str]:
    """Liste ordonnée des candidats, pour les messages d'échec de référence."""
    noms = ["scoretk"]
    if args.python_ly:
        noms.append("python-ly")
    if args.music21:
        noms.append("music21")
    return noms


def commandes(args, outil: str, src: str, dst: str) -> list[str]:
    if outil == "python-ly":
        return borne([args.python_ly, "musicxml", "-o", dst, src])
    if outil == "music21":
        return borne([args.music21, "-c", M21_CODE, src, dst])
    return borne([args.scoretk, src, "--to", "musicxml", "-o", dst])


def base_releve(lot: str, outil: str, src: str) -> dict:
    return {
        "lot": lot, "outil": outil, "fichier": src,
        "ok": False, "code": None, "temps": 0.0, "taille": 0, "erreur": "",
    }


def marquer_echec(r: dict) -> None:
    r["drapeaux"] = ["delai" if r.get("code") == 124 else "echec"]


def ecrire(fichier, r: dict) -> None:
    fichier.write(json.dumps(r, ensure_ascii=False) + "\n")


def reference(args, lot: str, src: str, dest: str) -> str:
    """Copie le MIDI LilyPond fourni, ou le régénère si le dossier est absent."""
    fourni = (Path(args.refs) / f"{args.refs_prefix}{lot}" / nom_court(src) / "ly.midi"
              if args.refs else None)
    if fourni and fourni.is_file():
        shutil.copyfile(fourni, dest)
        return ""
    base = str(Path(dest).with_suffix(""))
    code, _, err = lancer(
        ["timeout", str(TIMEOUT_LY), "prlimit", "--as=2000000000",
         "lilypond", "-dmidi-extension=midi", "-o", base, src],
        TIMEOUT_LY,
    )
    for candidat in (base + ".midi", base + ".mid"):
        if Path(candidat).is_file():
            shutil.move(candidat, dest)
            return ""
    return f"lilypond : code {code} {err}"


def mesurer_lot(args, lot: str, manifeste: str, sortie: Path) -> None:
    fichiers = lire_manifest(manifeste)
    # Échantillon déterministe : un fichier sur N, sans tirage aléatoire.
    if args.stride > 1:
        fichiers = fichiers[:: args.stride]
    if args.limit:
        fichiers = fichiers[: args.limit]
    with sortie.open("w", encoding="utf-8") as out:
        for i, src in enumerate(fichiers):
            work = Path(args.out) / "travail" / lot / str(i)
            work.mkdir(parents=True, exist_ok=True)
            ref_path = work / "ref.mid"
            err_ref = reference(args, lot, src, str(ref_path))
            if err_ref or not ref_path.is_file():
                for outil in outils(args):
                    r = base_releve(lot, outil, src)
                    r["erreur"] = err_ref or "référence MIDI absente"
                    marquer_echec(r)
                    ecrire(out, r)
                out.flush()
                continue
            try:
                ref = read_midi(str(ref_path))
            except Exception as exc:
                for outil in outils(args):
                    r = base_releve(lot, outil, src)
                    r["erreur"] = f"MIDI illisible : {exc}"
                    marquer_echec(r)
                    ecrire(out, r)
                out.flush()
                continue
            for outil in outils(args):
                dst = work / ("tk.musicxml" if outil == "scoretk"
                              else "ply.musicxml" if outil == "python-ly" else "m21.musicxml")
                commande = commandes(args, outil, src, str(dst))
                r = base_releve(lot, outil, src)
                code, duree, erreur = lancer(commande)
                r["code"], r["temps"] = code, round(duree, 3)
                r["taille"] = dst.stat().st_size if dst.is_file() else 0
                r["ok"] = code == 0 and r["taille"] > 0
                if not r["ok"]:
                    r["erreur"] = erreur or "sortie vide ou absente"
                    marquer_echec(r)
                    ecrire(out, r)
                    continue
                try:
                    test = read_xml(str(dst))
                except Exception as exc:
                    r["erreur"] = f"XML illisible : {exc}"
                    marquer_echec(r)
                    ecrire(out, r)
                    continue
                r.update(compare(ref, test, args.grid, args.tol))
                if r["ref"] and not r["test"]:
                    r["drapeaux"] = ["sortie_vide"]
                ecrire(out, r)
            out.flush()


def charger(out: str) -> list[dict]:
    releves: list[dict] = []
    for chemin in sorted(Path(out).glob("*.jsonl")):
        with chemin.open(encoding="utf-8") as f:
            for ligne in f:
                if ligne.strip():
                    releves.append(json.loads(ligne))
    return releves


def resume(releves: list[dict]) -> str:
    groupes: dict[tuple, list[dict]] = {}
    for r in releves:
        groupes.setdefault((r["lot"], r["outil"]), []).append(r)
    lignes = [
        "# Relevé LilyPond — résumé",
        "",
        "Tolérance : ±1/16 de noire sur l'attaque ; grille : 1/16 de noire (Q16).",
        "N2 est calculé sur le sous-ensemble apparié par N1 (reprises dépliées neutralisées).",
        "",
        "| Lot | Outil | Fichiers | Réussis | Échecs | Délais | Vides | Exacts | N1 % | N2 % | Temps max |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for (lot, outil), rs in sorted(groupes.items()):
        n = len(rs)
        reussis = sum(r["ok"] for r in rs)
        echecs = sum(not r["ok"] and r.get("code") != 124 for r in rs)
        delais = sum(r.get("code") == 124 for r in rs)
        vides = sum("sortie_vide" in r.get("drapeaux", []) for r in rs)
        exacts = sum(r.get("exact", False) for r in rs)
        notes = [r for r in rs if r.get("ref", 0)]
        n1 = sum(r["ref"] * r.get("pct_n1", 0) for r in notes)
        n2 = sum(r["ref"] * r.get("pct_n2", 0) for r in notes)
        tot = sum(r["ref"] for r in notes)
        tmax = max((r.get("temps", 0) for r in rs), default=0)
        lignes.append(
            f"| {lot} | {outil} | {n} | {reussis} | {echecs} | {delais} | {vides} | "
            f"{exacts} ({100*exacts/n:.1f} %) | {n1/tot if tot else 0:.2f} | "
            f"{n2/tot if tot else 0:.2f} | {tmax:.2f} s |"
        )
    causes: dict[str, list[str]] = {}
    for r in releves:
        if r["outil"] == "scoretk" and r.get("drapeaux"):
            causes.setdefault(r["drapeaux"][0], []).append(r["fichier"])
    lignes += ["", "## Causes d'écart scoretk (les plus fréquentes)", "",
               "| Cause | Fichiers | Exemple |", "| --- | ---: | --- |"]
    for cause, fichiers in sorted(causes.items(), key=lambda kv: -len(kv[1]))[:10]:
        lignes.append(f"| {cause} | {len(fichiers)} | `{fichiers[0]}` |")
    return "\n".join(lignes) + "\n"


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--out", required=True, help="dossier des sorties brutes (hors dépôt)")
    p.add_argument("--refs", default="", help="racine des ly.midi (préfixe + lot + md5_nom)")
    p.add_argument("--refs-prefix", default="out3", help="préfixe des dossiers de lot (out3mus, …)")
    p.add_argument("--scoretk", required=True, help="binaire scoretk")
    p.add_argument("--python-ly", default="ly", help="commande python-ly")
    p.add_argument("--music21", default="", help="interpréteur Python muni de music21")
    p.add_argument("--lot", action="append", default=[], metavar="NOM=MANIFESTE")
    p.add_argument("--stride", type=int, default=1, help="échantillon 1/N déterministe")
    p.add_argument("--limit", type=int, default=0, help="nombre max de fichiers par lot")
    p.add_argument("--grid", type=float, default=GRID)
    p.add_argument("--tol", type=float, default=TOL)
    p.add_argument("--resume", action="store_true", help="régénère seulement le résumé")
    args = p.parse_args()
    Path(args.out).mkdir(parents=True, exist_ok=True)

    if not args.resume:
        for spec in args.lot:
            lot, manifeste = spec.split("=", 1)
            mesurer_lot(args, lot, manifeste, Path(args.out) / f"{lot}.jsonl")

    texte = resume(charger(args.out))
    (Path(args.out) / "resume.md").write_text(texte, encoding="utf-8")
    print(texte)


if __name__ == "__main__":
    main()
