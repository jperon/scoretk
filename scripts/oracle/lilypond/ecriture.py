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

"""Oracle de l'écriture LilyPond : scoretk contre musicxml2ly.

Pour chaque fichier source ``S`` d'un lot, on produit deux ``.ly`` :

* celui de **scoretk**, directement depuis ``S`` (``scoretk S --to ly``) ;
* celui de **musicxml2ly**, depuis le MusicXML de référence de ``S`` quand il
  existe (``*.ref.musicxml`` du corpus, ou export MuseScore d'un ``.mscz``),
  sinon depuis le MusicXML de scoretk. L'origine est notée ``xml_source`` dans
  chaque ligne, pour que le relevé dise toujours quel MusicXML a été fourni à
  l'alternative.

Les deux ``.ly`` sont compilés par LilyPond 2.26 (``-dmidi-extension=midi``),
après injection dans une copie d'un bloc ``\\midi { }`` et d'un
``\\unfoldRepeats`` autour de la musique : sans cela ni scoretk ni
musicxml2ly ne déplieraient les reprises dans le MIDI, et la comparaison aux
notes de référence (elles-mêmes dépliées) serait biaisée. Le MIDI compilé est
comparé aux notes de référence de ``S`` avec N1/N2/N2d de ``midi_notes.py`` :
le MIDI de MuseScore pour ``.mscx``/``.mscz``/``.xml``, le MIDI de LilyPond de
la source pour les aller-retours ``.ly``. On relève aussi la réussite de la
compilation, le nombre d'erreurs/avertissements LilyPond et la taille du
``.ly``.

Tous les lancements sont bornés (``timeout`` + ``prlimit --as=2000000000``) ;
les références et sorties brutes vont dans ``--out`` (hors dépôt). Aucune
donnée tierce n'est copiée dans le dépôt. L'oracle est un instrument de
mesure : il ne définit pas le comportement de scoretk.
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

# Réutilisation des outils de mesure existants, sans script ad hoc.
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "midi"))

from ly_notes import a_des_repetitions  # noqa: E402
from midi_notes import GRID, TOL, compare, read_midi  # noqa: E402

TIMEOUT = 10
TIMEOUT_LY = 20

# Erreurs/avertissements LilyPond (messages localisés FR et EN).
RE_ERREUR = re.compile(r"\b(?:erreur|error)\b", re.IGNORECASE)
RE_AVERT = re.compile(r"\b(?:avertissement|warning)\b", re.IGNORECASE)


def lire_manifest(path: str) -> list[str]:
    """Liste de chemins absolus, un par ligne (comme mesure.py)."""
    with open(path, encoding="utf-8") as f:
        return [l.strip() for l in f if l.strip() and not l.startswith("#")]


def borne(commande: list[str], budget: int = TIMEOUT) -> list[str]:
    """Préfixe ``timeout``/``prlimit`` d'une commande à lancer."""
    return ["timeout", str(budget), "prlimit", "--as=2000000000"] + commande


def lancer(commande: list[str], budget: int = TIMEOUT) -> tuple[int, float, str]:
    """Exécute une commande bornée ; renvoie (code, secondes, stderr)."""
    debut = time.monotonic()
    try:
        p = subprocess.run(
            commande,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            timeout=budget + 3,
            check=False,
        )
        return p.returncode, time.monotonic() - debut, p.stderr.decode("utf-8", "replace")
    except subprocess.TimeoutExpired:
        return 124, time.monotonic() - debut, "dépassement de la borne de temps"


def hash_source(src: str) -> str:
    """Clé de cache stable d'un chemin source (le newline compte, comme dans ly_notes)."""
    return hashlib.md5((src + "\n").encode("utf-8")).hexdigest()[:12]


def texte_source(src: str) -> str:
    """Source et ses inclusions directes, pour détecter les reprises (ly_notes)."""
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


def injecter_midi(texte: str) -> str | None:
    """Ajoute un ``\\midi`` et déplie les reprises dans le dernier ``\\score``.

    On insère ``\\unfoldRepeats { … }`` autour de la musique du ``\\score``,
    puis ``\\midi { }`` après ``\\layout``. Le comptage des accolades ignore les
    chaînes et les commentaires (``%``, ``%{ %}``) pour ne pas confondre une
    accolade de texte avec la fin du bloc ``\\score``.
    """
    debut = texte.rfind("\\score")
    if debut < 0:
        return None
    ouverture = texte.find("{", debut)
    if ouverture < 0:
        return None
    profondeur = 0
    chaine = False
    commentaire_ligne = False
    commentaire_bloc = False
    i = ouverture
    while i < len(texte):
        c = texte[i]
        suivant = texte[i + 1] if i + 1 < len(texte) else ""
        if commentaire_ligne:
            if c == "\n":
                commentaire_ligne = False
        elif commentaire_bloc:
            if c == "%" and suivant == "}":
                commentaire_bloc = False
                i += 1
        elif chaine:
            if c == "\\":
                i += 1
            elif c == '"':
                chaine = False
        elif c == "%" and suivant == "{":
            commentaire_bloc = True
            i += 1
        elif c == "%":
            commentaire_ligne = True
        elif c == '"':
            chaine = True
        elif c == "{":
            profondeur += 1
        elif c == "}":
            profondeur -= 1
            if profondeur == 0:
                break
        i += 1
    else:
        return None
    corps = texte[ouverture + 1 : i]
    # Le \\layout global précède le \\score ; on ne regarde que son corps.
    layout = re.search(r"\n[ \t]*\\layout\b", corps)
    if layout:
        nouveau = "\\unfoldRepeats {\n" + corps[: layout.start()] + "\n}\n" + corps[layout.start() :]
    else:
        nouveau = "\\unfoldRepeats {\n" + corps + "\n}"
    nouveau += "\n\\midi { }\n"
    return texte[: ouverture + 1] + nouveau + texte[i:]


def compiler_lilypond(args, ly: Path, sortie: Path) -> dict:
    """Compile un ``.ly`` ; renvoie état, erreurs/avertissements, MIDI."""
    injecte = injecter_midi(ly.read_text(encoding="utf-8", errors="replace"))
    resultat = {
        "compile_ok": False, "code_ly": None, "erreurs": 0, "avertissements": 0,
        "erreur": "", "taille_ly": ly.stat().st_size if ly.is_file() else 0,
    }
    if injecte is None:
        resultat["erreur"] = "aucun \\score trouvé"
        return resultat
    copie = sortie.with_suffix(".inject.ly")
    copie.write_text(injecte, encoding="utf-8")
    code, _, err = lancer(
        [args.lilypond, "-dmidi-extension=midi", "-o", str(sortie.with_suffix("")), str(copie)],
        TIMEOUT_LY,
    )
    resultat["code_ly"] = code
    resultat["erreurs"] = len(RE_ERREUR.findall(err))
    resultat["avertissements"] = len(RE_AVERT.findall(err))
    midi = sortie.with_suffix(".midi")
    if not midi.is_file():
        midi = sortie.with_suffix(".mid")
    if midi.is_file():
        shutil.move(str(midi), str(sortie))
    resultat["compile_ok"] = code == 0 and sortie.is_file()
    if not resultat["compile_ok"]:
        resultat["erreur"] = err[-500:] or f"code {code}, MIDI absent"
    return resultat


def convertir_scoretk(args, src: str, dst: Path, to: str) -> tuple[bool, str]:
    """Lance scoretk sous timeout/prlimit ; renvoie (succès, message)."""
    code, _, err = lancer(borne([args.scoretk, src, "--to", to, "-o", str(dst)]))
    ok = code == 0 and dst.is_file() and dst.stat().st_size > 0
    return ok, "" if ok else (err[-300:] or f"code {code}, sortie absente")


def reference_midi(args, src: str, dest: Path, caches: Path, extensible: bool) -> str:
    """MIDI de référence de S (MuseScore pour les partitions, LilyPond pour .ly)."""
    if dest.is_file():
        return ""
    kind = Path(src).suffix.lower().lstrip(".")
    if kind == "ly":
        code, _, err = lancer(
            [args.lilypond, "-dmidi-extension=midi", "-o", str(dest.with_suffix("")), src],
            TIMEOUT_LY,
        )
        for candidat in (dest.with_suffix(".midi"), dest.with_suffix(".mid")):
            if candidat.is_file():
                shutil.move(str(candidat), str(dest))
                return ""
        return f"lilypond référence : code {code} {err[-300:]}"
    code, _, err = lancer(
        borne([args.mscore, "-o", str(dest), src], 30),
        30,
    )
    if dest.is_file():
        return ""
    return f"mscore référence : code {code} {err[-300:]}"


def reference_xml(args, src: str, dest: Path, tk_xml: Path, caches: Path) -> str:
    """MusicXML fourni à musicxml2ly ; renvoie l'origine (notée dans le relevé)."""
    kind = Path(src).suffix.lower().lstrip(".")
    if kind == "ly":
        if tk_xml.is_file():
            shutil.copyfile(tk_xml, dest)
            return "scoretk"
        return "absent"
    # Corpus : référence MuseScore versionnée à côté du .mscx.
    if kind == "mscx":
        ref = Path(src).with_suffix(".ref.musicxml")
        if ref.is_file():
            shutil.copyfile(ref, dest)
            return "ref.musicxml"
    if kind in ("xml", "musicxml", "mxl"):
        shutil.copyfile(src, dest)
        return "source"
    # .mscz (et .mscx sans référence) : export MuseScore 4.7.4.
    code, _, err = lancer(borne([args.mscore, "-o", str(dest), src], 30), 30)
    if dest.is_file() and dest.stat().st_size > 0:
        return "mscore"
    if tk_xml.is_file():
        shutil.copyfile(tk_xml, dest)
        return "scoretk (repli)"
    return f"absent (mscore code {code} {err[-200:]})"


def base_releve(lot: str, outil: str, src: str, repetition: bool) -> dict:
    return {
        "lot": lot, "outil": outil, "fichier": src, "repeat": repetition,
        "xml_source": "", "ok": False, "code": None, "temps": 0.0, "erreur": "",
    }


def ecrire(fichier, r: dict) -> None:
    fichier.write(json.dumps(r, ensure_ascii=False) + "\n")


def mesurer_lot(args, lot: str, manifeste: str, sortie: Path) -> None:
    fichiers = lire_manifest(manifeste)
    if args.stride > 1:
        fichiers = fichiers[:: args.stride]
    if args.limit:
        fichiers = fichiers[: args.limit]
    caches = Path(args.out) / "refs"
    caches.mkdir(parents=True, exist_ok=True)
    with sortie.open("w", encoding="utf-8") as out:
        for i, src in enumerate(fichiers):
            h = hash_source(src)
            repetition = a_des_repetitions(texte_source(src)) if src.endswith(".ly") else False
            work = Path(args.out) / "travail" / lot / str(i)
            work.mkdir(parents=True, exist_ok=True)
            ref_mid = caches / f"{h}.mid"
            err_ref = reference_midi(args, src, ref_mid, caches, not src.endswith(".ly"))
            if err_ref or not ref_mid.is_file():
                for outil in ("scoretk", "musicxml2ly"):
                    r = base_releve(lot, outil, src, repetition)
                    r["erreur"] = err_ref or "référence MIDI absente"
                    r["drapeaux"] = ["echec"]
                    ecrire(out, r)
                out.flush()
                continue
            try:
                ref = read_midi(str(ref_mid))
            except Exception as exc:
                for outil in ("scoretk", "musicxml2ly"):
                    r = base_releve(lot, outil, src, repetition)
                    r["erreur"] = f"MIDI illisible : {exc}"
                    r["drapeaux"] = ["echec"]
                    ecrire(out, r)
                out.flush()
                continue

            tk_ly = work / "tk.ly"
            tk_xml = work / "tk.musicxml"
            ok_ly, err_ly = convertir_scoretk(args, src, tk_ly, "ly")
            ok_xml, _ = convertir_scoretk(args, src, tk_xml, "musicxml")
            ref_xml = caches / f"{h}.musicxml"
            origine = reference_xml(args, src, ref_xml, tk_xml, caches) if ok_xml or Path(src).suffix.lower() in (".mscz", ".mscx", ".xml", ".musicxml", ".mxl") else "absent"
            m2ly = work / "m2ly.ly"

            for outil in ("scoretk", "musicxml2ly"):
                r = base_releve(lot, outil, src, repetition)
                r["xml_source"] = origine if outil == "musicxml2ly" else ""
                debut = time.monotonic()
                if outil == "musicxml2ly":
                    if "absent" in origine or not ref_xml.is_file():
                        r["erreur"] = origine or "MusicXML de référence absent"
                        r["drapeaux"] = ["echec"]
                        ecrire(out, r)
                        continue
                    code, _, err = lancer(
                        borne([args.musicxml2ly, f"--output={m2ly}", str(ref_xml)], 20), 20
                    )
                    r["code"] = code
                    r["ok"] = code == 0 and m2ly.is_file() and m2ly.stat().st_size > 0
                    if not r["ok"]:
                        r["erreur"] = err[-300:] or f"code {code}"
                        r["drapeaux"] = ["echec"]
                        r["temps"] = round(time.monotonic() - debut, 3)
                        ecrire(out, r)
                        continue
                    ly = m2ly
                else:
                    if not ok_ly:
                        r["erreur"] = err_ly
                        r["drapeaux"] = ["echec"]
                        ecrire(out, r)
                        continue
                    ly = tk_ly
                r.update(compiler_lilypond(args, ly, work / f"{outil}.mid"))
                r["temps"] = round(time.monotonic() - debut, 3)
                # Un échec de compilation est enregistré, mais si LilyPond a
                # quand même écrit un MIDI on le mesure : les deux informations
                # sont complémentaires, pas exclusives.
                if not r["compile_ok"]:
                    r["drapeaux"] = ["compilation"]
                if not (work / f"{outil}.mid").is_file():
                    ecrire(out, r)
                    continue
                try:
                    test = read_midi(str(work / f"{outil}.mid"))
                except Exception as exc:
                    r["erreur"] = f"MIDI illisible : {exc}"
                    r["drapeaux"] = ["echec"]
                    ecrire(out, r)
                    continue
                r.update(compare(ref, test, args.grid, args.tol))
                if r["ref"] and not r["test"]:
                    r["drapeaux"] = ["sortie_vide"]
                if not r["compile_ok"] and "compilation" not in r["drapeaux"]:
                    r["drapeaux"].insert(0, "compilation")
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
        "# Relevé écriture LilyPond — résumé",
        "",
        "Grille 1/16 de noire (Q16). N1/N2/N2d comparent le MIDI du `.ly` compilé",
        "par LilyPond 2.26 au MIDI de référence (MuseScore pour les partitions,",
        "LilyPond pour les aller-retours `.ly`). `xml_source` indique le MusicXML",
        "donné à musicxml2ly : `ref.musicxml`, `mscore`, `source`, `scoretk`.",
        "Un `\\unfoldRepeats` est injecté dans les deux candidats avant compilation.",
        "",
        "| Lot | Outil | Fichiers | Compilés OK | Exacts | N1 % | N2 % | N2d % | Erreurs | Avert. | Taille .ly moy. | Temps max |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for (lot, outil), rs in sorted(groupes.items()):
        n = len(rs)
        ok = sum(r.get("compile_ok", False) for r in rs)
        exacts = sum(r.get("exact", False) for r in rs)
        notes = [r for r in rs if r.get("ref", 0)]
        tot = sum(r["ref"] for r in notes)
        def pct(cle):
            return sum(r["ref"] * r.get(cle, 0) for r in notes) / tot if tot else 0.0
        errs = sum(r.get("erreurs", 0) for r in rs)
        av = sum(r.get("avertissements", 0) for r in rs)
        tailles = [r["taille_ly"] for r in rs if r.get("taille_ly")]
        tmax = max((r.get("temps", 0) for r in rs), default=0)
        lignes.append(
            f"| {lot} | {outil} | {n} | {ok} | {exacts} ({100*exacts/n:.1f} %) | "
            f"{pct('pct_n1'):.2f} | {pct('pct_n2'):.2f} | {pct('pct_n2d'):.2f} | "
            f"{errs} | {av} | {(sum(tailles)/len(tailles)) if tailles else 0:.0f} | {tmax:.2f} s |"
        )
    # Comparaison fichier par fichier : pertes de scoretk face à musicxml2ly.
    paires: dict[tuple, dict] = {}
    for r in releves:
        paires.setdefault((r["lot"], r["fichier"]), {})[r["outil"]] = r
    lignes += ["", "## scoretk contre musicxml2ly (par fichier)", "",
               "| Lot | Comparés | scoretk > m2ly | scoretk = m2ly | scoretk < m2ly |",
               "| --- | ---: | ---: | ---: | ---: |"]
    for lot in sorted({lot for lot, _ in paires}):
        gagnes = perdus = egaux = 0
        for (l, _), paire in paires.items():
            if l != lot or "scoretk" not in paire or "musicxml2ly" not in paire:
                continue
            a, b = paire["scoretk"], paire["musicxml2ly"]
            if not a.get("exact", False) and not b.get("exact", False):
                continue
            sa = a.get("pct_n2d", 0)
            sb = b.get("pct_n2d", 0)
            if sa > sb:
                gagnes += 1
            elif sa < sb:
                perdus += 1
            else:
                egaux += 1
        lignes.append(f"| {lot} | {gagnes+perdus+egaux} | {gagnes} | {egaux} | {perdus} |")
    causes: dict[str, list[str]] = {}
    for r in releves:
        if r["outil"] == "scoretk" and r.get("drapeaux"):
            causes.setdefault(r["drapeaux"][0], []).append(r["fichier"])
    lignes += ["", "## Causes d'écart scoretk (les plus fréquentes)", "",
               "| Cause | Fichiers | Exemple |", "| --- | ---: | --- |"]
    for cause, fichiers in sorted(causes.items(), key=lambda kv: -len(kv[1]))[:10]:
        lignes.append(f"| {cause} | {len(fichiers)} | `{fichiers[0]}` |")
    return "\n".join(lignes) + "\n"


def selftest() -> None:
    """Autotests sans corpus : injection et agrégation."""
    src = '\\version "2.24"\n\\score {\n  << c\'1 >>\n  \\layout { }\n}\n'
    injecte = injecter_midi(src)
    assert injecte and "\\unfoldRepeats {" in injecte and "\\midi { }" in injecte, injecte
    # Une accolade dans un commentaire ou une chaîne ne doit pas tromper le comptage.
    src2 = '\\score {\n  << c\'1 >> % }\n  \\markup { "}" }\n  \\layout { }\n}\n'
    assert injecter_midi(src2), "accolade de commentaire mal comptée"
    # L'agrégation pondère bien par le nombre de notes de référence.
    base = {"lot": "corpus", "outil": "scoretk", "ok": True, "taille_ly": 100,
            "erreurs": 0, "avertissements": 0, "temps": 0.1, "drapeaux": []}
    ex = [
        {**base, "fichier": "a", "ref": 10, "pct_n1": 100, "pct_n2": 100, "pct_n2d": 100, "exact": True},
        {**base, "fichier": "b", "ref": 90, "pct_n1": 50, "pct_n2": 50, "pct_n2d": 50, "exact": False},
    ]
    texte = resume(ex)
    assert "| 55.00 | 55.00 | 55.00 |" in texte, texte
    print("selftest ecriture : OK")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--out", default="", help="dossier des sorties brutes (hors dépôt)")
    p.add_argument("--scoretk", default="target/debug/scoretk", help="binaire scoretk")
    p.add_argument("--lilypond", default="lilypond", help="LilyPond 2.26")
    p.add_argument("--musicxml2ly", default="musicxml2ly")
    p.add_argument("--mscore", default="mscore", help="MuseScore 4.7.4 (références)")
    p.add_argument("--lot", action="append", default=[], metavar="NOM=MANIFESTE")
    p.add_argument("--stride", type=int, default=1, help="échantillon 1/N déterministe")
    p.add_argument("--limit", type=int, default=0)
    p.add_argument("--grid", type=float, default=GRID)
    p.add_argument("--tol", type=float, default=TOL)
    p.add_argument("--resume", action="store_true")
    p.add_argument("--selftest", action="store_true")
    args = p.parse_args()
    if args.selftest:
        selftest()
        return
    if not args.out:
        p.error("--out est requis sauf avec --selftest")
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
