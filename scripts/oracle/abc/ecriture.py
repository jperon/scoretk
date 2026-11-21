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

"""Oracle de l'écriture ABC : scoretk contre xml2abc.py.

Pour chaque fichier source ``S`` d'un lot on produit deux ``.abc`` :

* celui de **scoretk**, directement depuis ``S`` (``scoretk S -o tk.abc``) ;
* celui de **xml2abc.py** (version 177, hors dépôt), depuis le MusicXML de
  référence de ``S`` quand il existe (``*.ref.musicxml`` du corpus, export
  MuseScore d'un ``.mscz``), sinon depuis le MusicXML de la source (lots XML)
  ou, en dernier recours, celui de scoretk. L'origine est notée ``xml_source``
  dans chaque relevé, pour que le rapport dise toujours quel MusicXML a été
  fourni à l'alternative.

Les deux ``.abc`` sont joués par ``abc2midi`` avec les mêmes options que
l'oracle de lecture (``-silent -NGRA -NGUI``), après application à une copie
des mêmes nettoyages que lui (``mesure.sans_reprises`` et
``mesure.sans_ornements``) : on mesure les notes ÉCRITES, une seule fois
chacune, pas les notes rejouées par les reprises ni la réalisation des
ornements. Le MIDI obtenu est comparé au MIDI de référence de ``S`` avec
N1/N2/N2d de ``midi_notes.py`` (grille 1/16 de noire). La référence est le MIDI
de MuseScore 4.7.4 pour ``.mscx``/``.mscz``, et le MIDI que scoretk tire de la
source pour les fichiers MusicXML. On relève aussi les erreurs/avertissements
d'``abc2midi`` et la taille du ``.abc``.

Les helpers sont ceux des oracles existants, sans duplication : ``mesure.py``
(nettoyages, lancement borné) et ``midi_notes.py`` (lecture MIDI et N1/N2/N2d).
Tous les lancements sont bornés par ``timeout 10`` et ``prlimit``. Les sorties
brutes vont dans ``--out`` (hors dépôt). L'oracle est un instrument de mesure :
il ne définit pas le comportement de scoretk.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys
import time

HERE = Path(__file__).resolve().parent
# L'oracle ABC a son propre mesure.py ; il doit précéder celui de l'oracle MIDI
# (qui porte le même nom) dans le chemin de recherche.
sys.path.insert(0, str(HERE.parent / "midi"))  # midi_notes.py
sys.path.insert(0, str(HERE))                  # mesure.py, cmp.py

import mesure  # noqa: E402
from midi_notes import GRID, TOL, compare, read_midi  # noqa: E402

TIMEOUT = 10
RE_ERREUR = re.compile(r"\b(?:erreur|error)\b", re.IGNORECASE)
RE_AVERT = re.compile(r"\b(?:avertissement|warning)\b", re.IGNORECASE)


def borne(commande: list[str]) -> list[str]:
    """Préfixe ``timeout``/``prlimit`` d'une commande à lancer."""
    return ["timeout", str(TIMEOUT), "prlimit", "--as=2000000000"] + commande


def lancer(commande: list[str], log: Path) -> tuple[int, float, str]:
    """Lance une commande bornée ; renvoie (code, secondes, message)."""
    debut = time.monotonic()
    code, message = mesure.lance(borne(commande), str(log))
    return code, time.monotonic() - debut, message


def hash_source(src: str) -> str:
    """Clé de cache stable d'un chemin source (comme mesure.py)."""
    return hashlib.md5((src + "\n").encode("utf-8")).hexdigest()[:12]


def lire_manifest(path: str) -> list[str]:
    """Liste de chemins absolus, un par ligne (comme ecriture.py)."""
    with open(path, encoding="utf-8") as handle:
        return [l.strip() for l in handle if l.strip() and not l.startswith("#")]


def base_releve(lot: str, outil: str, src: str, xml_source: str = "") -> dict:
    return {
        "lot": lot, "outil": outil, "fichier": src, "xml_source": xml_source,
        "ok": False, "code": None, "erreurs": 0, "avertissements": 0,
        "taille_abc": 0, "temps": 0.0, "erreur": "",
        "rt_ok": False, "rt_stable": False,
    }


def ecrire(fichier, releve: dict) -> None:
    fichier.write(json.dumps(releve, ensure_ascii=False) + "\n")


def reference_midi(args, src: str, dest: Path, log: Path) -> str:
    """MIDI de référence de S (MuseScore pour les partitions, scoretk pour XML)."""
    if dest.is_file() and dest.stat().st_size:
        return ""
    kind = Path(src).suffix.lower().lstrip(".")
    if kind in ("mscx", "mscz"):
        commande = [args.mscore, "-o", str(dest), src]
    else:
        # Un fichier MusicXML est sa propre référence : on lui demande le MIDI
        # de scoretk, qui définit le contenu musical mesuré pour ce lot.
        commande = [args.scoretk, src, "-o", str(dest)]
    code, _, message = lancer(commande, log)
    if dest.is_file() and dest.stat().st_size:
        return ""
    return f"référence MIDI absente (code {code}) {message[-200:]}"


def reference_xml(args, src: str, dest: Path, tk_xml: Path, log: Path) -> str:
    """MusicXML fourni à xml2abc.py ; renvoie l'origine notée au relevé."""
    kind = Path(src).suffix.lower().lstrip(".")
    if kind == "mscx":
        ref = Path(src).with_suffix(".ref.musicxml")
        if ref.is_file():
            shutil.copyfile(ref, dest)
            return "ref.musicxml"
    if kind in ("xml", "musicxml", "mxl"):
        if kind == "mxl":
            # Un .mxl est un ZIP : xml2abc.py le lit selon son extension. On
            # extrait le premier membre .musicxml/.xml dans un fichier nu.
            import zipfile

            try:
                with zipfile.ZipFile(src) as archive:
                    noms = [
                        n for n in archive.namelist()
                        if n.lower().endswith((".musicxml", ".xml"))
                        and "meta-inf" not in n.lower()
                    ]
                    # Le premier membre `.xml` peut être un `container.xml`
                    # hors META-INF : xml2abc.py n'y trouve aucun `<part>` et
                    # échoue. On choisit le premier membre qui contient
                    # réellement une partition partwise.
                    membres = [
                        n for n in noms
                        if b"<score-partwise" in archive.read(n)[:8192]
                        or b"<score-timewise" in archive.read(n)[:8192]
                    ]
                    if not membres and noms:
                        membres = [noms[0]]
                    if not membres:
                        return "absent (mxl vide)"
                    dest.write_bytes(archive.read(membres[0]))
            except (OSError, zipfile.BadZipFile) as exc:
                return f"absent (mxl illisible : {exc})"
        else:
            shutil.copyfile(src, dest)
        return "source"
    if kind == "mscz":
        code, _, _ = lancer([args.mscore, "-o", str(dest), src], log)
        if dest.is_file() and dest.stat().st_size:
            return "mscore"
        if code not in (0, None):
            pass  # le repli scoretk est tenté ci-dessous
        if code == 124:
            return "absent (mscore délai dépassé)"
    if tk_xml.is_file():
        shutil.copyfile(tk_xml, dest)
        return "scoretk (repli)"
    return "absent"


def ecrire_abc_scoretk(args, src: str, dst: Path, log: Path) -> tuple[bool, str]:
    """Écrit le ``.abc`` de scoretk sous timeout/prlimit."""
    code, _, message = lancer([args.scoretk, src, "-o", str(dst)], log)
    ok = code == 0 and dst.is_file() and dst.stat().st_size > 0
    return ok, "" if ok else (message[-300:] or f"code {code}, sortie absente")


def verifier_aller_retour(args, abc: Path, work: Path, log: Path) -> dict:
    """Relit l'ABC écrit, l'écrit à nouveau et vérifie qu'il est stable.

    ``.abc → scoretk → .abc → scoretk`` : deux écritures successives depuis
    le modèle relu doivent donner des octets identiques (idempotence de
    l'écrivain). La taille du second fichier est aussi relevée pour détecter
    un gonflement silencieux.
    """
    rt1 = work / "rt1.abc"
    rt2 = work / "rt2.abc"
    ok1, _ = ecrire_abc_scoretk(args, str(abc), rt1, log)
    if not ok1:
        return {"rt_ok": False, "rt_stable": False}
    ok2, _ = ecrire_abc_scoretk(args, str(rt1), rt2, log)
    if not ok2:
        return {"rt_ok": False, "rt_stable": False}
    stable = rt1.read_bytes() == rt2.read_bytes()
    return {"rt_ok": True, "rt_stable": stable}


def ecrire_abc_alt(args, musicxml: Path, dossier: Path, log: Path) -> tuple[bool, str, Path]:
    """Écrit le ``.abc`` de xml2abc.py ; renvoie (succès, message, chemin)."""
    if dossier.is_dir():
        for ancien in dossier.glob("*"):
            if ancien.is_file():
                ancien.unlink()
    dossier.mkdir(parents=True, exist_ok=True)
    code, _, message = lancer(
        ["python3", args.xml2abc, str(musicxml), "-o", str(dossier)], log
    )
    fichiers = sorted(dossier.glob("*.abc")) if dossier.is_dir() else []
    if not fichiers:
        return False, (message[-300:] or f"code {code}, aucun .abc"), Path()
    return True, "", fichiers[0]


def jouer_abc(abc: Path, midi: Path, log: Path) -> dict:
    """Joue un ``.abc`` avec abc2midi (options de l'oracle de lecture).

    Les nettoyages de ``mesure.py`` sont appliqués à une copie : reprises
    remplacées par des barres simples, ornements réalisés retirés. Un code de
    retour non nul est accepté si un MIDI non vide a été écrit, comme dans
    l'oracle de lecture.
    """
    lignes = abc.read_text(encoding="utf-8", errors="replace").splitlines()
    plain = midi.with_suffix(".plain.abc")
    mesure.ecrit(str(plain), mesure.sans_ornements(mesure.sans_reprises(lignes)))
    code, secondes, message = lancer(
        ["abc2midi", str(plain), "-o", str(midi), "-silent", "-NGRA", "-NGUI"], log
    )
    midi_ok = midi.is_file() and midi.stat().st_size > 0
    return {
        "code": code,
        "erreurs": len(RE_ERREUR.findall(message)),
        "avertissements": len(RE_AVERT.findall(message)),
        "temps_abc2midi": round(secondes, 3),
        "midi_ok": midi_ok,
        "message_abc2midi": message[-300:] if not midi_ok else "",
    }


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
            work = Path(args.out) / "travail" / lot / str(i)
            work.mkdir(parents=True, exist_ok=True)
            log = work / "journal.log"
            ref_mid = caches / f"{h}.mid"
            err_ref = reference_midi(args, src, ref_mid, log)
            if err_ref or not ref_mid.is_file():
                for outil in ("scoretk", "xml2abc"):
                    r = base_releve(lot, outil, src)
                    r["erreur"] = err_ref or "référence MIDI absente"
                    r["drapeaux"] = ["echec"]
                    ecrire(out, r)
                out.flush()
                continue
            try:
                ref = read_midi(str(ref_mid))
            except Exception as exc:
                for outil in ("scoretk", "xml2abc"):
                    r = base_releve(lot, outil, src)
                    r["erreur"] = f"MIDI illisible : {exc}"
                    r["drapeaux"] = ["echec"]
                    ecrire(out, r)
                out.flush()
                continue

            debut = time.monotonic()
            tk_abc = work / "tk.abc"
            tk_xml = work / "tk.musicxml"
            ok_tk, err_tk = ecrire_abc_scoretk(args, src, tk_abc, log)
            ok_xml, _ = ecrire_abc_scoretk(args, src, tk_xml, log)
            ref_xml = caches / f"{h}.musicxml"
            origine = reference_xml(args, src, ref_xml, tk_xml, log) if Path(src).suffix.lower() in (
                ".mscx", ".mscz", ".xml", ".musicxml", ".mxl"
            ) else "absent"
            alt_abc = work / "alt"
            ok_alt, err_alt, chemin_alt = (False, "MusicXML de référence absent", Path())
            if "absent" not in origine and ref_xml.is_file():
                ok_alt, err_alt, chemin_alt = ecrire_abc_alt(args, ref_xml, alt_abc, log)
            else:
                chemin_alt = Path()

            for outil in ("scoretk", "xml2abc"):
                r = base_releve(lot, outil, src, origine if outil == "xml2abc" else "")
                if outil == "scoretk":
                    abc, ok, erreur = tk_abc, ok_tk, err_tk
                else:
                    abc, ok, erreur = chemin_alt, ok_alt, err_alt
                if not ok:
                    r["erreur"] = erreur or "ABC absent"
                    r["drapeaux"] = ["echec"]
                    r["temps"] = round(time.monotonic() - debut, 3)
                    ecrire(out, r)
                    continue
                r["ok"] = True
                r["taille_abc"] = abc.stat().st_size
                if outil == "scoretk":
                    r.update(verifier_aller_retour(args, abc, work, log))
                r.update(jouer_abc(abc, work / f"{outil}.mid", log))
                r["temps"] = round(time.monotonic() - debut, 3)
                if not r["midi_ok"]:
                    r["erreur"] = r.get("message_abc2midi") or "MIDI abc2midi absent"
                    r["drapeaux"] = ["echec"]
                    ecrire(out, r)
                    continue
                try:
                    test = read_midi(str(work / f"{outil}.mid"))
                except Exception as exc:
                    r["erreur"] = f"MIDI illisible : {exc}"
                    r["drapeaux"] = ["echec"]
                    ecrire(out, r)
                    continue
                if r["code"] not in (0, None) and r["midi_ok"]:
                    # Comme dans l'oracle de lecture : le MIDI produit fait foi.
                    r["drapeaux"] = ["avertissement_abc2midi"]
                r.update(compare(ref, test, args.grid, args.tol))
                if r.get("ref") and not r.get("test"):
                    r["drapeaux"] = ["sortie_vide"]
                ecrire(out, r)
            out.flush()


def charger(out: str) -> list[dict]:
    releves: list[dict] = []
    for chemin in sorted(Path(out).glob("*.jsonl")):
        with chemin.open(encoding="utf-8") as handle:
            for ligne in handle:
                if ligne.strip():
                    releves.append(json.loads(ligne))
    return releves


def resume(releves: list[dict]) -> str:
    groupes: dict[tuple, list[dict]] = {}
    for r in releves:
        groupes.setdefault((r["lot"], r["outil"]), []).append(r)
    lignes = [
        "# Relevé écriture ABC — résumé",
        "",
        "Comparaison MIDI ↔ MIDI sur la grille 1/16 de noire (Q16) : N1",
        "(multiset des hauteurs), N2 (hauteur + attaque à ±1/16), N2d (+ durée).",
        "Le MIDI de référence vient de MuseScore 4.7.4 pour `.mscx`/`.mscz`,",
        "de scoretk pour les fichiers MusicXML. `xml_source` indique le MusicXML",
        "donné à xml2abc.py : `ref.musicxml`, `mscore`, `source`, `scoretk`.",
        "Les deux `.abc` sont joués par abc2midi après le même nettoyage que",
        "l'oracle de lecture (reprises et ornements retirés sur une copie).",
        "",
        "| Lot | Outil | Fichiers | ABC OK | Exacts | N1 % | N2 % | N2d % | Erreurs | Avert. | Taille .abc moy. | Temps max |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for (lot, outil), rs in sorted(groupes.items()):
        n = len(rs)
        ok = sum(r.get("ok", False) for r in rs)
        exacts = sum(r.get("exact", False) for r in rs)
        notes = [r for r in rs if r.get("ref", 0)]
        tot = sum(r["ref"] for r in notes)

        def pct(cle):
            return sum(r["ref"] * r.get(cle, 0) for r in notes) / tot if tot else 0.0

        tailles = [r["taille_abc"] for r in rs if r.get("taille_abc")]
        tmax = max((r.get("temps", 0) for r in rs), default=0)
        lignes.append(
            f"| {lot} | {outil} | {n} | {ok} | {exacts} ({100*exacts/n:.1f} %) | "
            f"{pct('pct_n1'):.2f} | {pct('pct_n2'):.2f} | {pct('pct_n2d'):.2f} | "
            f"{sum(r.get('erreurs', 0) for r in rs)} | {sum(r.get('avertissements', 0) for r in rs)} | "
            f"{(sum(tailles)/len(tailles)) if tailles else 0:.0f} | {tmax:.2f} s |"
        )
    lignes += ["", "## Aller-retour `.abc → scoretk → .abc → scoretk`", "",
               "| Lot | Testés | Relus | Stables | Instables |",
               "| --- | ---: | ---: | ---: | ---: |"]
    for lot in sorted({lot for lot, _ in groupes}):
        rs = [r for r in releves if r["lot"] == lot and r["outil"] == "scoretk" and r.get("ok")]
        tests = [r for r in rs if r.get("rt_ok")]
        stables = [r for r in tests if r.get("rt_stable")]
        lignes.append(
            f"| {lot} | {len(rs)} | {len(tests)} | {len(stables)} | {len(tests)-len(stables)} |"
        )
    paires: dict[tuple, dict] = {}
    for r in releves:
        paires.setdefault((r["lot"], r["fichier"]), {})[r["outil"]] = r
    lignes += ["", "## scoretk contre xml2abc (par fichier)", "",
               "| Lot | Comparés | scoretk > xml2abc | scoretk = xml2abc | scoretk < xml2abc |",
               "| --- | ---: | ---: | ---: | ---: |"]
    for lot in sorted({lot for lot, _ in paires}):
        gagnes = perdus = egaux = 0
        for (l, _), paire in paires.items():
            if l != lot or "scoretk" not in paire or "xml2abc" not in paire:
                continue
            a, b = paire["scoretk"], paire["xml2abc"]
            if not a.get("ok") or not b.get("ok"):
                continue
            sa, sb = a.get("pct_n2d", 0), b.get("pct_n2d", 0)
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
    """Autotests sans corpus : nettoyages partagés et agrégation."""
    air = ["X:1", "M:4/4", "L:1/8", "K:C", "|: A2 B2 :| [1 c2 d2 :|[2 e2 f2 |]"]
    nettoye = mesure.sans_ornements(mesure.sans_reprises(air))
    assert all("|:" not in l and ":|" not in l for l in nettoye), nettoye
    assert nettoye[-1] == "| A2 B2 | | c2 d2 || e2 f2 |]", nettoye
    base = {"lot": "corpus", "outil": "scoretk", "ok": True, "taille_abc": 100,
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
    p.add_argument("--xml2abc", required=False, default="", help="xml2abc.py (hors dépôt)")
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
    if not args.xml2abc:
        p.error("--xml2abc est requis sauf avec --selftest")
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
