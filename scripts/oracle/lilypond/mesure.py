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
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import common  # noqa: E402
from ly_notes import (
    GRID, TOL, a_des_repetitions, compare, lire_graces, nombre_portees,
    read_midi, read_xml,
)

TIMEOUT = 90
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


def texte_source(src: str) -> str:
    """Source ``.ly`` et ses inclusions directes, pour repérer les reprises.

    Les fichiers Mutopia mettent souvent chaque voix dans un ``\\include`` :
    ne lire que le fichier principal manquerait la majorité des ``\\repeat``.
    Les inclusions sont lues à un niveau (itinéraire borné, pas de cycle).
    """
    try:
        texte = Path(src).read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""
    dossier = Path(src).parent
    for nom in re.findall(r'\\include\s+"([^"]+)"', texte)[:20]:
        inclusion = dossier / nom
        if inclusion.is_file():
            try:
                texte += "\n" + inclusion.read_text(encoding="utf-8", errors="replace")
            except OSError:
                pass
    return texte


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


def base_releve(lot: str, outil: str, src: str, repetitions: bool = False) -> dict:
    return {
        "lot": lot, "outil": outil, "fichier": src, "repeat": repetitions,
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
    if args.refs_only:
        return "référence MIDI absente (--refs-only)"
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
            repetitions = a_des_repetitions(texte_source(src))
            work = Path(args.out) / "travail" / lot / str(i)
            work.mkdir(parents=True, exist_ok=True)
            ref_path = work / "ref.mid"
            err_ref = reference(args, lot, src, str(ref_path))
            texte_src = texte_source(src)
            if err_ref or not ref_path.is_file():
                for outil in outils(args):
                    r = base_releve(lot, outil, src, repetitions)
                    r["erreur"] = err_ref or "référence MIDI absente"
                    marquer_echec(r)
                    # Une référence inutilisable n'est pas un défaut du
                    # lecteur : exclure des deux dénominateurs (comme code 3).
                    common.marquer_exclu(r, src)
                    r["raison_exclusion"] = common.raison_hors_lecteur(r, texte_src)
                    ecrire(out, r)
                out.flush()
                continue
            # Source obsolète que `convert-ly` réécrirait : son MIDI de
            # référence est troué, la comparer pénaliserait le lecteur à tort.
            raison_obsolete = common.raison_hors_lecteur({"code": 0}, texte_src)
            if raison_obsolete.startswith("source obsolète"):
                for outil in outils(args):
                    r = base_releve(lot, outil, src, repetitions)
                    r["erreur"] = raison_obsolete
                    common.marquer_exclu(r, src)
                    r["raison_exclusion"] = raison_obsolete
                    ecrire(out, r)
                out.flush()
                continue
            try:
                ref = read_midi(str(ref_path))
            except Exception as exc:
                for outil in outils(args):
                    r = base_releve(lot, outil, src, repetitions)
                    r["erreur"] = f"MIDI illisible : {exc}"
                    marquer_echec(r)
                    ecrire(out, r)
                out.flush()
                continue
            for outil in outils(args):
                dst = work / ("tk.musicxml" if outil == "scoretk"
                              else "ply.musicxml" if outil == "python-ly" else "m21.musicxml")
                commande = commandes(args, outil, src, str(dst))
                r = base_releve(lot, outil, src, repetitions)
                code, duree, erreur = lancer(commande)
                r["code"], r["temps"] = code, round(duree, 3)
                # `scoretk` est le premier candidat : une entrée non prise en
                # charge exclut le fichier avant qu'aucune ligne d'alternative
                # ne soit écrite, donc tout le lot reste comparable.
                if outil == "scoretk" and common.est_non_pris_en_charge(code):
                    common.marquer_exclu(r, src)
                    r["erreur"] = f"entrée non prise en charge (code {code})"
                    ecrire(out, r)
                    break
                r["taille"] = dst.stat().st_size if dst.is_file() else 0
                r["ok"] = code == 0 and r["taille"] > 0
                if not r["ok"]:
                    r["erreur"] = erreur or "sortie vide ou absente"
                    marquer_echec(r)
                    ecrire(out, r)
                    continue
                try:
                    # Le MIDI de référence a une piste par contexte ; le
                    # MusicXML peut replier plusieurs pistes sur une portée
                    # (lecteur ou source sans structure explicite). Dans ce
                    # cas, fusionner les unissons de voix effacerait des notes
                    # distinctes : on ne le fait que si le XML sépare au moins
                    # autant de portées que la référence a de pistes.
                    fusion = nombre_portees(str(dst)) >= len({n[3] for n in ref})
                    test = read_xml(str(dst), with_grace=True, merge_unisons=fusion)
                except Exception as exc:
                    r["erreur"] = f"XML illisible : {exc}"
                    marquer_echec(r)
                    ecrire(out, r)
                    continue
                # Les graces du MIDI sont appariées à l'attaque de leur note
                # principale (§2.6.1, grace-iterator.cc) ; N2 reste inchangé,
                # n2_grace est publié à côté pour les deux règles de lecture.
                # On relit le XML avec le marqueur de grâce pour que `compare`
                # retire aussi ces graces de l'intersection des notes principales.
                graces = lire_graces(test)
                r.update(compare(ref, test, args.grid, args.tol, graces=graces))
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
    exclus = common.noms_exclus(releves)
    # Une entrée non prise en charge sort du numérateur et du dénominateur.
    releves = common.filtrer(releves)
    groupes: dict[tuple, list[dict]] = {}
    for r in releves:
        groupes.setdefault((r["lot"], r["outil"]), []).append(r)
    lignes = [
        "# Relevé LilyPond — résumé",
        "",
        "Grille : 1/16 de noire (Q16) ; N2 est l'intersection des multiensembles",
        "(hauteur, attaque quantifiée), sans appariement arbitraire.",
        "N1 porte sur tous les fichiers ; N2 **uniquement sur les fichiers sans",
        "reprise** (le MIDI de LilyPond déplie les reprises, pas le MusicXML).",
        "N2-grace applique la même règle en appariant les graces du MIDI à",
        "l'attaque de leur note principale (§2.6.1, grace-iterator.cc).",
        "",
        "| Lot | Outil | Fichiers | Reprises | Réussis | Échecs | Délais | Vides | Exacts | N1 % (tous) | N2 % (sans reprise) | N2-grace % (sans reprise) | Temps max |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for (lot, outil), rs in sorted(groupes.items()):
        n = len(rs)
        reprises = sum(r.get("repeat", False) for r in rs)
        reussis = sum(r["ok"] for r in rs)
        echecs = sum(not r["ok"] and r.get("code") != 124 for r in rs)
        delais = sum(r.get("code") == 124 for r in rs)
        vides = sum("sortie_vide" in r.get("drapeaux", []) for r in rs)
        exacts = sum(r.get("exact", False) for r in rs)
        notes = [r for r in rs if r.get("ref", 0)]
        sans = [r for r in notes if not r.get("repeat", False)]
        n1 = sum(r["ref"] * r.get("pct_n1", 0) for r in notes)
        n2 = sum(r["ref"] * r.get("pct_n2", 0) for r in sans)
        n2g = sum(r["ref"] * r.get("pct_n2_grace", 0) for r in sans)
        tot = sum(r["ref"] for r in notes)
        tot_sans = sum(r["ref"] for r in sans)
        tmax = max((r.get("temps", 0) for r in rs), default=0)
        lignes.append(
            f"| {lot} | {outil} | {n} | {reprises} | {reussis} | {echecs} | {delais} | {vides} | "
            f"{exacts} ({100*exacts/n:.1f} %) | {n1/tot if tot else 0:.2f} | "
            f"{n2/tot_sans if tot_sans else 0:.2f} | "
            f"{n2g/tot_sans if tot_sans else 0:.2f} | {tmax:.2f} s |"
        )
    lignes += ["", common.ligne_resume(exclus)]
    causes: dict[str, list[str]] = {}
    for r in releves:
        if r["outil"] == "scoretk" and r.get("drapeaux"):
            causes.setdefault(r["drapeaux"][0], []).append(r["fichier"])
    lignes += ["", "## Causes d'écart scoretk (les plus fréquentes)", "",
               "| Cause | Fichiers | Exemple |", "| --- | ---: | --- |"]
    for cause, fichiers in sorted(causes.items(), key=lambda kv: -len(kv[1]))[:10]:
        lignes.append(f"| {cause} | {len(fichiers)} | `{fichiers[0]}` |")
    common.avertir(exclus)
    return "\n".join(lignes) + "\n"


def selftest() -> None:
    """Autotest de l'agrégation : N2 ignore les fichiers avec reprises.

    Un fichier propre de 10 notes à 100 % et un fichier avec reprise de 100
    notes à 0 % : l'ancien agrégat donnait 9,09 %, le nouveau doit donner
    100 % en ne comptant que le fichier sans reprise.
    """
    base = {"lot": "mus", "outil": "scoretk", "ok": True, "temps": 0.1,
            "drapeaux": [], "exact": False}
    ex = [
        {**base, "fichier": "propre.ly", "repeat": False,
         "ref": 10, "pct_n1": 100.0, "pct_n2": 100.0,
         "pct_n2_grace": 90.0, "exact": True},
        {**base, "fichier": "reprise.ly", "repeat": True,
         "ref": 100, "pct_n1": 100.0, "pct_n2": 0.0, "pct_n2_grace": 0.0},
    ]
    texte = resume(ex)
    # N2 et N2-grace sont deux colonnes distinctes ; la reprise est exclue des
    # deux agrégats, sinon le fichier propre à 10 notes donnerait 9,09 %.
    assert "| 100.00 | 100.00 | 90.00 |" in texte, texte
    assert "9.09" not in texte, texte

    # Détection dans une source et son inclusion directe (voix incluses).
    import os
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        src = os.path.join(tmp, "principal.ly")
        inc = os.path.join(tmp, "voix.ly")
        with open(src, "w", encoding="utf-8") as f:
            f.write('\\version "2.24"\n\\include "voix.ly"\n')
        with open(inc, "w", encoding="utf-8") as f:
            f.write("musique = { \\repeat volta 2 { c4 d4 } }\n")
        assert a_des_repetitions(texte_source(src))
        with open(inc, "w", encoding="utf-8") as f:
            f.write("musique = { c4 d4 e4 f4 }\n")
        assert not a_des_repetitions(texte_source(src))
    print("selftest mesure : OK")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--out", default="", help="dossier des sorties brutes (hors dépôt)")
    p.add_argument("--refs", default="", help="racine des ly.midi (préfixe + lot + md5_nom)")
    p.add_argument(
        "--refs-only",
        action="store_true",
        help="ne pas régénérer une référence absente : le fichier est exclu",
    )
    p.add_argument("--refs-prefix", default="out3", help="préfixe des dossiers de lot (out3mus, …)")
    p.add_argument("--scoretk", default="", help="binaire scoretk")
    p.add_argument("--python-ly", default="ly", help="commande python-ly")
    p.add_argument("--music21", default="", help="interpréteur Python muni de music21")
    p.add_argument("--lot", action="append", default=[], metavar="NOM=MANIFESTE")
    p.add_argument("--stride", type=int, default=1, help="échantillon 1/N déterministe")
    p.add_argument("--limit", type=int, default=0, help="nombre max de fichiers par lot")
    p.add_argument("--grid", type=float, default=GRID)
    p.add_argument("--tol", type=float, default=TOL)
    p.add_argument("--resume", action="store_true", help="régénère seulement le résumé")
    p.add_argument("--selftest", action="store_true", help="autotest de l'agrégation (sans corpus)")
    args = p.parse_args()
    if args.selftest:
        selftest()
        return
    if not args.out or not args.scoretk:
        p.error("--out et --scoretk sont requis sauf avec --selftest")
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
