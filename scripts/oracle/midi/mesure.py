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

"""Pilote de mesure des deux sens de conversion MIDI.

Tous les lancements de scoretk passent par ``timeout 10 prlimit`` (bornes
mémoire et temps), conformément aux consignes du projet. MuseScore, utilisé
comme alternative sur la lecture, est lancé de la même façon en mode headless.

Les manifestes sont des fichiers texte, un chemin par ligne ; les paires
source/référence de l'écriture sont séparées par une tabulation. Aucune donnée
tierce n'est versionnée : les manifestes et les sorties brutes vont dans le
dossier ``--out`` (hors dépôt).
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import time

from midi_notes import GRID, TOL, compare, read_midi, read_xml

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import common  # noqa: E402

TIMEOUT_SCORETK = 10
TIMEOUT_MSCORE = 20


def lire_manifest(path: str) -> list[str]:
    with open(path, encoding="utf-8") as f:
        return [ligne.strip() for ligne in f if ligne.strip() and not ligne.startswith("#")]


def lancer(commande: list[str], env: dict | None = None, budget: int = TIMEOUT_SCORETK):
    """Exécute une conversion bornée ; renvoie (code, secondes, message)."""
    debut = time.monotonic()
    try:
        p = subprocess.run(
            commande,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            env=env,
            timeout=budget + 3,
            check=False,
        )
        return p.returncode, time.monotonic() - debut, p.stderr.decode("utf-8", "replace")[-400:]
    except subprocess.TimeoutExpired:
        return 124, time.monotonic() - debut, "dépassement de la borne de temps"


def cmd_scoretk(
    binaire: str, src: str, dst: str, entree: str, sortie: str, extra: list[str] | None = None
) -> list[str]:
    return [
        "timeout", str(TIMEOUT_SCORETK), "prlimit", "--as=2000000000",
        binaire, src, "--from", entree, "--to", sortie, "-o", dst,
        *(extra or []),
    ]


def _sans_lyricmode(text: str) -> str:
    """Retire les blocs `\\lyricmode { … }` (un `\\unfoldRepeats` y est inerte)."""
    morceaux = []
    i = 0
    while True:
        j = text.find("\\lyricmode", i)
        if j < 0:
            morceaux.append(text[i:])
            break
        morceaux.append(text[i:j])
        k = text.find("{", j)
        if k < 0:
            break
        profondeur = 0
        while k < len(text):
            if text[k] == "{":
                profondeur += 1
            elif text[k] == "}":
                profondeur -= 1
                if profondeur == 0:
                    k += 1
                    break
            k += 1
        i = k
    return "".join(morceaux)


def sans_unfold(src: str) -> bool:
    r"""Vrai pour un `.ly` sans `\unfoldRepeats` (MIDI de LilyPond non déplié).

    LilyPond ne déplie `\repeat volta` que sous `\unfoldRepeats` ; l'oracle
    doit donc demander `--no-expand-repeats` pour ces sources. Le drapeau est
    cherché dans la source et ses `\include` directs (même convention que
    `scripts/oracle/lilypond/ly_notes.py`).
    """
    if not src.endswith(".ly"):
        return False
    try:
        text = Path(src).read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return False
    text = _sans_lyricmode(text)
    if "\\unfoldRepeats" in text:
        return False
    base = Path(src).parent
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("\\include"):
            continue
        debut = line.find('"')
        fin = line.find('"', debut + 1)
        if debut == -1 or fin == -1:
            continue
        try:
            include = (base / line[debut + 1:fin]).read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        if "\\unfoldRepeats" in include:
            return False
    return True


def cmd_mscore(binaire: str, src: str, dst: str) -> tuple[list[str], dict]:
    env = dict(os.environ)
    env.pop("LD_LIBRARY_PATH", None)  # wrapper Nix hostile aux bibliothèques Qt
    env["QT_QPA_PLATFORM"] = "offscreen"
    commande = [
        "timeout", str(TIMEOUT_MSCORE), "prlimit", "--as=2000000000",
        binaire, "-o", dst, src,
    ]
    return commande, env


def chaines_midi2ly(src: str, work: Path) -> list[tuple[list[str], dict | None]]:
    """midi2ly (MIDI → .ly) puis lilypond (ly → MIDI), bornés comme scoretk."""
    ly = work / "m2l.ly"
    base = work / "m2l"
    borne = ["timeout", str(TIMEOUT_MSCORE), "prlimit", "--as=2000000000"]
    return [
        (borne + ["midi2ly", "-o", str(ly), src], None),
        (borne + ["env", "-u", "LD_LIBRARY_PATH", "lilypond", "-s",
                  "-o", str(base), str(ly)], None),
    ]


def cmd_music21(src: str, dst: str) -> list[str]:
    """music21.converter.parse → MusicXML (si le module est disponible)."""
    script = (
        "from music21 import converter; "
        f"s = converter.parse({src!r}); "
        f"s.write('musicxml', fp={dst!r})"
    )
    return [
        "timeout", str(TIMEOUT_MSCORE), "prlimit", "--as=2000000000",
        "python3", "-c", script,
    ]


def lancer_chaine(chaine: list[tuple[list[str], dict | None]], budget: int):
    """Lance plusieurs commandes en séquence ; s'arrête au premier échec."""
    total = 0.0
    for commande, env in chaine:
        code, duree, erreur = lancer(commande, env, budget)
        total += duree
        if code != 0:
            return code, total, erreur
    return 0, total, ""


def ecrire_ligne(fichier, releve: dict) -> None:
    fichier.write(json.dumps(releve, ensure_ascii=False) + "\n")


def base_releve(lot: str, sens: str, outil: str, fichier: str) -> dict:
    return {
        "lot": lot, "sens": sens, "outil": outil, "fichier": fichier,
        "ok": False, "code": None, "temps": 0.0, "taille": 0, "erreur": "",
    }


def marquer_echec(releve: dict) -> None:
    """Classe les échecs pour que le résumé les compte et les regroupe."""
    releve["drapeaux"] = ["delai" if releve.get("code") == 124 else "echec"]


def mesurer_lecture(args, lot: str, manifeste: str, fichier_sortie) -> None:
    """MIDI → MusicXML : oracle = hauteurs/attaques du MIDI lui-même."""
    for i, src in enumerate(lire_manifest(manifeste)[: args.limit or None]):
        try:
            ref = read_midi(src)
        except Exception as exc:  # MIDI illisible : on le note comme échec d'oracle
            r = base_releve(lot, "lecture", "oracle", src)
            r["erreur"] = f"oracle MIDI : {exc}"
            marquer_echec(r)
            ecrire_ligne(fichier_sortie, r)
            continue
        work = Path(args.out) / "travail" / lot / str(i)
        work.mkdir(parents=True, exist_ok=True)
        # Chaque cible = (nom, chaîne de commandes, sortie, lecteur, budget).
        # scoretk est toujours mesuré ; les alternatives sont échantillonnées
        # avec le même pas déterministe, sans réduire le lot de scoretk.
        cibles = [(
            "scoretk",
            [(cmd_scoretk(args.scoretk, src, str(work / "tk.xml"), "midi", "musicxml"), None)],
            work / "tk.xml", read_xml, TIMEOUT_SCORETK,
        )]
        if i % args.alt_stride == 0:
            if args.mscore:
                commande, env = cmd_mscore(args.mscore, src, str(work / "ms.xml"))
                cibles.append(("mscore", [(commande, env)], work / "ms.xml",
                               read_xml, TIMEOUT_MSCORE))
            if args.midi2ly:
                cibles.append(("midi2ly", chaines_midi2ly(src, work),
                               work / "m2l.midi", read_midi, TIMEOUT_MSCORE))
            if args.music21:
                cibles.append(("music21",
                               [(cmd_music21(src, str(work / "m21.musicxml")), None)],
                               work / "m21.musicxml", read_xml, TIMEOUT_MSCORE))
        for outil, chaine, sortie, lire, budget in cibles:
            r = base_releve(lot, "lecture", outil, src)
            code, duree, erreur = lancer_chaine(chaine, budget)
            r["code"], r["temps"] = code, round(duree, 3)
            # `scoretk` est le premier candidat : l'entrée non prise en charge
            # exclut le fichier pour tous les outils du lot.
            if outil == "scoretk" and common.est_non_pris_en_charge(code):
                common.marquer_exclu(r, src)
                r["erreur"] = f"entrée non prise en charge (code {code})"
                ecrire_ligne(fichier_sortie, r)
                break
            r["ok"] = code == 0 and sortie.exists() and sortie.stat().st_size > 0
            r["taille"] = sortie.stat().st_size if sortie.exists() else 0
            if not r["ok"]:
                r["erreur"] = erreur or "sortie vide ou absente"
                marquer_echec(r)
                ecrire_ligne(fichier_sortie, r)
                continue
            try:
                test = lire(str(sortie))
            except Exception as exc:
                r["erreur"] = f"sortie illisible : {exc}"
                r["ok"] = False  # une sortie mal formée n'est pas une réussite
                marquer_echec(r)
                ecrire_ligne(fichier_sortie, r)
                continue
            if not ref and not test:
                common.marquer_vide(r, src)
                ecrire_ligne(fichier_sortie, r)
                continue
            r.update(compare(ref, test, args.grid, args.tol))
            ecrire_ligne(fichier_sortie, r)
        fichier_sortie.flush()


def mesurer_ecriture(args, lot: str, manifeste: str, fichier_sortie) -> None:
    """Partition → MIDI : comparateur MIDI ↔ MIDI commun, référence externe."""
    for i, ligne in enumerate(lire_manifest(manifeste)[: args.limit or None]):
        src, ref_path = ligne.split("\t", 1)
        try:
            ref = read_midi(ref_path)
        except Exception as exc:
            r = base_releve(lot, "ecriture", "oracle", src)
            r["erreur"] = f"oracle MIDI : {exc}"
            marquer_echec(r)
            ecrire_ligne(fichier_sortie, r)
            continue
        work = Path(args.out) / "travail" / lot / str(i)
        work.mkdir(parents=True, exist_ok=True)
        dst = work / "tk.mid"
        r = base_releve(lot, "ecriture", "scoretk", src)
        # A source MusicXML is not LilyPond: the old fallback sent
        # `.musicxml`/`.xml`/`.mxl` through the LilyPond reader, which yielded
        # an empty score (one note in the SMF) for every file of the xml lot.
        lower = src.lower()
        if lower.endswith((".musicxml", ".xml", ".mxl")):
            entree = "musicxml"
        elif lower.endswith(".mscx"):
            entree = "mscx"
        elif lower.endswith(".mscz"):
            entree = "mscz"
        else:
            entree = "ly"
        extra = ["--no-expand-repeats"] if sans_unfold(src) else None
        code, duree, erreur = lancer(
            cmd_scoretk(args.scoretk, src, str(dst), entree, "midi", extra)
        )
        r["code"], r["temps"] = code, round(duree, 3)
        if common.est_non_pris_en_charge(code):
            common.marquer_exclu(r, src)
            r["erreur"] = f"entrée non prise en charge (code {code})"
            ecrire_ligne(fichier_sortie, r)
            continue
        r["ok"] = code == 0 and dst.exists() and dst.stat().st_size > 0
        r["taille"] = dst.stat().st_size if dst.exists() else 0
        if not r["ok"]:
            r["erreur"] = erreur or "sortie vide ou absente"
            ecrire_ligne(fichier_sortie, r)
            continue
        try:
            test = read_midi(str(dst))
        except Exception as exc:
            r["erreur"] = f"MIDI illisible : {exc}"
            marquer_echec(r)
            ecrire_ligne(fichier_sortie, r)
            continue
        if not ref and not test:
            common.marquer_vide(r, src)
            ecrire_ligne(fichier_sortie, r)
            fichier_sortie.flush()
            continue
        r.update(compare(ref, test, args.grid, args.tol))
        ecrire_ligne(fichier_sortie, r)
        fichier_sortie.flush()


def mesurer_robustesse(args, lot: str, manifeste: str, fichier_sortie) -> None:
    """Corpus sans source : échecs, délais, vides et taille de sortie seulement."""
    for i, src in enumerate(lire_manifest(manifeste)[: args.limit or None]):
        work = Path(args.out) / "travail" / lot / str(i)
        work.mkdir(parents=True, exist_ok=True)
        dst = work / "tk.xml"
        r = base_releve(lot, "robustesse", "scoretk", src)
        code, duree, erreur = lancer(
            cmd_scoretk(args.scoretk, src, str(dst), "midi", "musicxml")
        )
        r["code"], r["temps"] = code, round(duree, 3)
        r["taille"] = dst.stat().st_size if dst.exists() else 0
        if common.est_non_pris_en_charge(code):
            common.marquer_exclu(r, src)
            r["erreur"] = f"entrée non prise en charge (code {code})"
            ecrire_ligne(fichier_sortie, r)
            fichier_sortie.flush()
            continue
        r["ok"] = code == 0 and r["taille"] > 0
        if not r["ok"]:
            r["erreur"] = erreur or "sortie vide ou absente"
            marquer_echec(r)
        else:
            try:
                r["notes_test"] = len(read_xml(str(dst)))
                if r["notes_test"] == 0:
                    r["drapeaux"] = ["sortie_vide"]
            except Exception as exc:
                r["erreur"] = f"XML illisible : {exc}"
                r["ok"] = False  # une sortie mal formée n'est pas une réussite
                marquer_echec(r)
        ecrire_ligne(fichier_sortie, r)
        fichier_sortie.flush()
        # Seuls la taille et le compte de notes importent ici : on ne garde pas
        # les XML intermédiaires (le corpus Lakh represente ~2 Go de sorties).
        if dst.exists():
            dst.unlink()


def charger(out: str) -> list[dict]:
    relevés: list[dict] = []
    for chemin in sorted(Path(out).glob("*.jsonl")):
        with chemin.open(encoding="utf-8") as f:
            for ligne in f:
                if ligne.strip():
                    relevés.append(json.loads(ligne))
    return relevés


def resume(relevés: list[dict]) -> str:
    exclus = common.noms_exclus(relevés)
    vides_noms = common.noms_vides(relevés)
    # Unsupported inputs and empty scores leave the numerator and denominator.
    relevés = common.filtrer(relevés)
    groupes: dict[tuple, list[dict]] = {}
    for r in relevés:
        groupes.setdefault((r["lot"], r["sens"], r["outil"]), []).append(r)
    lignes = [
        "# Relevé MIDI — résumé",
        "",
        "Tolérance : ±1/16 de noire sur l'attaque et la durée ; grille de",
        "quantification : 1/16 de noire (Q16, défaut MuseScore).",
        "",
        "| Lot | Sens | Outil | Fichiers | Réussis | Échecs | Délais | Vides | Exacts | N1 % | N2 % | Taille moy. | Temps max |",
        "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for (lot, sens, outil), rs in sorted(groupes.items()):
        n = len(rs)
        reussis = sum(r["ok"] for r in rs)
        echecs = sum(not r["ok"] and r.get("code") not in (124,) for r in rs)
        delais = sum(r.get("code") == 124 for r in rs)
        vides = sum("sortie_vide" in r.get("drapeaux", []) or (r.get("notes_test") == 0 and r["ok"]) for r in rs)
        exacts = sum(r.get("exact", False) for r in rs)
        notes = [r for r in rs if r.get("ref", 0)]
        n1 = sum(r.get("ref", 0) * r.get("pct_n1", 0) for r in notes)
        n2 = sum(r.get("ref", 0) * r.get("pct_n2", 0) for r in notes)
        tot = sum(r.get("ref", 0) for r in notes)
        taille = sum(r.get("taille", 0) for r in rs) / n if n else 0
        tmax = max((r.get("temps", 0) for r in rs), default=0)
        lignes.append(
            f"| {lot} | {sens} | {outil} | {n} | {reussis} | {echecs} | {delais} | "
            f"{vides} | {exacts} ({100*exacts/n:.1f} %) | "
            f"{n1/tot if tot else 0:.2f} | {n2/tot if tot else 0:.2f} | "
            f"{taille:.0f} | {tmax:.2f} s |"
        )

    causes: dict[str, list[str]] = {}
    for r in relevés:
        if r.get("drapeaux"):
            causes.setdefault(r["drapeaux"][0], []).append(r["fichier"])
    lignes += ["", common.ligne_resume(exclus), common.ligne_resume_vides(vides_noms), "",
               "## Causes d'écart (les plus fréquentes)", "", "| Cause | Fichiers | Exemple |", "| --- | ---: | --- |"]
    for cause, fichiers in sorted(causes.items(), key=lambda kv: -len(kv[1]))[:10]:
        lignes.append(f"| {cause} | {len(fichiers)} | `{fichiers[0]}` |")
    common.avertir(exclus)
    return "\n".join(lignes) + "\n"


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--out", required=True, help="dossier des sorties brutes (hors dépôt)")
    p.add_argument("--scoretk", required=True, help="chemin du binaire scoretk")
    p.add_argument("--mscore", default="", help="binaire MuseScore (alternative lecture)")
    p.add_argument("--midi2ly", action="store_true",
                   help="mesurer aussi midi2ly + lilypond (alternative lecture)")
    p.add_argument("--music21", action="store_true",
                   help="mesurer aussi music21.converter (si module disponible)")
    p.add_argument("--alt-stride", type=int, default=1,
                   help="mesurer les alternatives sur 1 fichier sur N (défaut : 1)")
    p.add_argument("--lecture", action="append", default=[], metavar="NOM=MANIFESTE")
    p.add_argument("--ecriture", action="append", default=[], metavar="NOM=MANIFESTE")
    p.add_argument("--robustesse", action="append", default=[], metavar="NOM=MANIFESTE")
    p.add_argument("--limit", type=int, default=0, help="nombre max de fichiers par lot (0 = tous)")
    p.add_argument("--grid", type=float, default=GRID)
    p.add_argument("--tol", type=float, default=TOL)
    p.add_argument("--resume", action="store_true", help="ne fait que régénérer le résumé")
    args = p.parse_args()
    Path(args.out).mkdir(parents=True, exist_ok=True)
    if args.music21 and importlib.util.find_spec("music21") is None:
        print("music21 absent de l'environnement : alternative ignorée")
        args.music21 = False

    if not args.resume:
        for spec in args.lecture:
            lot, manifeste = spec.split("=", 1)
            with open(Path(args.out) / f"{lot}.jsonl", "w", encoding="utf-8") as f:
                mesurer_lecture(args, lot, manifeste, f)
        for spec in args.ecriture:
            lot, manifeste = spec.split("=", 1)
            with open(Path(args.out) / f"{lot}.jsonl", "w", encoding="utf-8") as f:
                mesurer_ecriture(args, lot, manifeste, f)
        for spec in args.robustesse:
            lot, manifeste = spec.split("=", 1)
            with open(Path(args.out) / f"{lot}.jsonl", "w", encoding="utf-8") as f:
                mesurer_robustesse(args, lot, manifeste, f)

    texte = resume(charger(args.out))
    (Path(args.out) / "resume.md").write_text(texte, encoding="utf-8")
    print(texte)


if __name__ == "__main__":
    main()
