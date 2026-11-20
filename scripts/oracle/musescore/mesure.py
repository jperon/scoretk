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

"""Oracle d'écriture MuseScore : fidélité du .mscz après réouverture par 4.7.4.

Pour chaque fichier source que scoretk sait lire on écrit un `.mscz`, on le
fait rouvrir par MuseScore 4.7.4 quiexporte MusicXML et MIDI, puis on compare
ces deux sorties à celles que scoretk produit **directement** depuis le même
source. La comparaison de noyau réutilise la normalisation partagée avec
`e2e_corpus` (l'aide `crates/scoretk/tests/oracle_core.rs`, sans duplication) ;
N1/N2 réutilisent `scripts/oracle/midi/midi_notes.py`. L'alternative mesurée
sur le même lot est l'import direct de la source par MuseScore (son propre
`.mscz` pour les sources mscz, l'import MusicXML/LilyPond/ABC sinon).

Tout lancement de binaire passe par `timeout` et `prlimit --as=2000000000`.
Aucune donnée tierce n'est versionnée : manifestes et sorties brutes vivent
hors du dépôt.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(DIR))
sys.path.insert(0, str(DIR.parent / "midi"))

from midi_notes import compare, read_midi  # noqa: E402

LIMITE = 30  # secondes, pour MuseScore comme pour scoretk
REPO = DIR.parent.parent.parent
Mot_avert = re.compile(r"(?i)(error|warning|erreur|avert|failed|cannot|unable|invalid|not found)")


def lancer(commande: list[str], env: dict | None = None) -> tuple[int, float, str]:
    """Lance un bINARY sous timeout/prlimit ; renvoie (code, durée, stderr)."""
    debut = time.monotonic()
    try:
        p = subprocess.run(
            ["timeout", str(LIMITE), "prlimit", "--as=2000000000", *commande],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            env=env,
            check=False,
            timeout=LIMITE + 5,
        )
        return p.returncode, time.monotonic() - debut, p.stderr.decode("utf-8", "replace")[-2000:]
    except subprocess.TimeoutExpired:
        return 124, time.monotonic() - debut, "dépassement de la borne de temps"


def env_mscore() -> dict:
    """Le wrapper Nix de MuseScore hait le LD_LIBRARY_PATH du shell du projet."""
    env = dict(os.environ)
    env.pop("LD_LIBRARY_PATH", None)
    env["QT_QPA_PLATFORM"] = "offscreen"
    return env


def cmd_mscore(binaire: str, src: str, dst: str) -> list[str]:
    return [binaire, "-o", dst, src]


def cmd_scoretk(binaire: str, src: str, dst: str, sortie: str) -> list[str]:
    return [binaire, src, "--to", sortie, "-o", dst]


def lire_manifest(chemin: str, limite: int) -> list[str]:
    with open(chemin, encoding="utf-8") as f:
        lignes = [l.strip() for l in f if l.strip() and not l.startswith("#")]
    return lignes[:limite] if limite else lignes


def ecrire(fichier, releve: dict) -> None:
    fichier.write(json.dumps(releve, ensure_ascii=False) + "\n")


def mesure_lot(lot: str, args, fichier) -> None:
    """Mesure un lot entier, une ligne JSON par fichier, écrite au fur et à mesure."""
    deja: set[str] = set()
    if args.resume:
        chemin = Path(args.out) / f"{lot}.jsonl"
        if chemin.exists():
            for ligne in chemin.read_text(encoding="utf-8").splitlines():
                r = json.loads(ligne)
                deja.add(f"{r['lot']}-{r['i']}")
    for i, src in enumerate(lire_manifest(getattr(args, "lot_" + lot), args.limit)):
        ident = f"{lot}-{i}"
        if ident in deja:
            continue
        travail = Path(args.out) / "travail" / lot / str(i)
        travail.mkdir(parents=True, exist_ok=True)
        r = {
            "lot": lot, "i": i, "fichier": src, "ok": False, "ouvert": False,
            "erreur": "", "temps": 0.0, "averts": 0, "journal": "",
        }
        tk_mscz = travail / "tk.mscz"
        ms_xml, ms_mid = travail / "ms.musicxml", travail / "ms.mid"
        tk_xml, tk_mid = travail / "tk.musicxml", travail / "tk.mid"
        alt_xml = travail / "alt.musicxml"
        alt_mid = travail / "alt.mid"

        etapes = [
            ("scoretk_mscz", cmd_scoretk(args.scoretk, src, str(tk_mscz), "mscz"), None),
            ("mscore_xml", cmd_mscore(args.mscore, str(tk_mscz), str(ms_xml)), env_mscore()),
            ("mscore_midi", cmd_mscore(args.mscore, str(tk_mscz), str(ms_mid)), env_mscore()),
            ("scoretk_xml", cmd_scoretk(args.scoretk, src, str(tk_xml), "musicxml"), None),
            ("scoretk_midi", cmd_scoretk(args.scoretk, src, str(tk_mid), "midi"), None),
        ]
        for nom, commande, env in etapes:
            code, duree, erreur = lancer(commande, env)
            r["temps"] += round(duree, 3)
            if nom.startswith("mscore"):
                r["journal"] += f"{nom}: rc={code} {erreur[-400:]}\n"
                r["averts"] += len(Mot_avert.findall(erreur))
                if code == 0:
                    r["ouvert"] = True
            if code != 0:
                r["erreur"] = f"{nom} : code {code} {erreur}"
                break
        cibles = [tk_mscz, ms_xml, ms_mid, tk_xml, tk_mid]
        if r["erreur"] or any(not p.exists() or p.stat().st_size == 0 for p in cibles):
            r["erreur"] = r["erreur"] or "sortie absente ou vide"
            ecrire(fichier, r)
            fichier.flush()
            continue

        r["ok"] = True
        try:
            r.update(compare(read_midi(str(ms_mid)), read_midi(str(tk_mid)), args.grid, args.tol))
        except Exception as exc:  # MIDI illisible : noté, pas masqué
            r["erreur_midi"] = str(exc)[:200]
        if not args.skip_alt:
            code, duree, erreur = lancer(cmd_mscore(args.mscore, src, str(alt_xml)), env_mscore())
            r["temps"] += round(duree, 3)
            if not (code == 0 and alt_xml.exists() and alt_xml.stat().st_size):
                r["alt_erreur"] = f"code {code} {erreur[-200:]}"
            # N2 « écrivain seul » : MuseScore joue la source, puis notre
            # `.mscz`. Les écarts entre scoretk et MuseScore sur la *source*
            # ne comptent plus ; seul ce que l'écrivain perd est mesuré.
            code, duree, erreur = lancer(cmd_mscore(args.mscore, src, str(alt_mid)), env_mscore())
            r["temps"] += round(duree, 3)
            if not (code == 0 and alt_mid.exists() and alt_mid.stat().st_size):
                r["alt_midi_erreur"] = f"code {code} {erreur[-200:]}"
            elif ms_mid.exists() and ms_mid.stat().st_size:
                try:
                    writer = compare(
                        read_midi(str(alt_mid)), read_midi(str(ms_mid)), args.grid, args.tol
                    )
                    r["refw"] = writer["ref"]
                    r["n1w"] = writer["n1"]
                    r["n2w"] = writer["n2"]
                    r["n2dw"] = writer["n2d"]
                    r["testw"] = writer["test"]
                    r["octavew"] = writer["octave"]
                    r["transpositionw"] = writer["transposition"]
                except Exception as exc:  # MIDI illisible : noté, pas masqué
                    r["erreur_midiw"] = str(exc)[:200]
        ecrire(fichier, r)
        fichier.flush()


def regenerer_paires(args) -> None:
    """Reconstruit les manifestes de l'aide Rust depuis les relevés écrits.

    Les paires sont dérivées des chemins déterministes de `travail/`, ce qui
    rend `--resume` fiable : un relevé partiel conserve ses comparaisons de
    noyau sans relancer les conversions.
    """
    out = Path(args.out)
    with (out / "paires.tsv").open("w", encoding="utf-8") as paires, (
        out / "alt_paires.tsv"
    ).open("w", encoding="utf-8") as alt_paires:
        for lot in args.lots:
            chemin = out / f"{lot}.jsonl"
            if not chemin.exists():
                continue
            for ligne in chemin.read_text(encoding="utf-8").splitlines():
                r = json.loads(ligne)
                if not r.get("ok"):
                    continue
                travail = out / "travail" / lot / str(r["i"])
                ms_xml, tk_xml = travail / "ms.musicxml", travail / "tk.musicxml"
                alt_xml = travail / "alt.musicxml"
                if ms_xml.exists() and tk_xml.exists():
                    paires.write(f"{lot}-{r['i']}\t{ms_xml}\t{tk_xml}\n")
                if alt_xml.exists() and alt_xml.stat().st_size and tk_xml.exists():
                    alt_paires.write(f"{lot}-{r['i']}\t{alt_xml}\t{tk_xml}\n")


def fusionner_core(args, manifeste: Path, sortie: Path, cle: str) -> None:
    """Réutilise l'aide Rust `oracle_core.rs`, sans réécrire la normalisation."""
    if not manifeste.exists() or not manifeste.stat().st_size:
        return
    env = dict(os.environ)
    env["SCORETK_ORACLE_MANIFEST"] = str(manifeste)
    env["SCORETK_ORACLE_OUTPUT"] = str(sortie)
    p = subprocess.run(
        ["nix", "develop", "-c", "cargo", "test", "-q", "-p", "scoretk",
         "--test", "oracle_core", "--", "--ignored"],
        cwd=str(REPO), env=env, check=False,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=600,
    )
    if p.returncode != 0:
        print(f"ÉCHEC aide core ({cle}) :", p.stdout.decode("utf-8", "replace")[-1000:])
        return
    core = {}
    for ligne in sortie.read_text(encoding="utf-8").splitlines():
        champs = ligne.split("\t")
        if len(champs) >= 6:
            core[champs[0]] = {
                "noyau_lignes": int(champs[1]),
                "plein_lignes": int(champs[2]),
                "tags": dict(t.split(":") for t in champs[3].split(",") if ":" in t),
                "premier_noyau": champs[4],
                "premier_plein": champs[5],
            }
    for lot in args.lots:
        chemin = Path(args.out) / f"{lot}.jsonl"
        lignes = [json.loads(l) for l in chemin.read_text(encoding="utf-8").splitlines()]
        with chemin.open("w", encoding="utf-8") as f:
            for r in lignes:
                ident = f"{r['lot']}-{r['i']}"
                if ident in core:
                    r[cle] = core[ident]
                ecrire(f, r)


def cause_n2w(r: dict) -> str:
    """Cause courte d'un N2 écrivain inférieur à 1."""
    if r.get("octavew", 0):
        return "octave"
    if r.get("transpositionw", 0):
        return f"transposition {r['transpositionw']:+d}"
    if r.get("testw", 0) < r.get("refw", 0):
        return f"notes manquantes ({r.get('refw', 0) - r.get('testw', 0)})"
    return "attaques décalées"


def resume(args) -> str:
    """Tableau récapitulatif par lot, avec l'alternative sur le même lot."""
    lignes = [
        "# MuseScore export oracle — fidelity after reopening in MuseScore 4.7.4",
        "",
        ("| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | "
         "N1w | N2w | Load err/warn | Alt core-identical |"),
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    totaux: dict[str, list[float]] = {}
    for lot in args.lots:
        chemin = Path(args.out) / f"{lot}.jsonl"
        rows = [json.loads(l) for l in chemin.read_text(encoding="utf-8").splitlines()]
        n = len(rows)
        ecrits = sum(1 for r in rows if r.get("ok"))
        ouverts = sum(1 for r in rows if r.get("ouvert"))
        core_ok = sum(1 for r in rows if r.get("core", {}).get("noyau_lignes") == 0)
        notes_ref = sum(r.get("ref", 0) for r in rows)
        n1 = sum(r.get("n1", 0) for r in rows)
        n2 = sum(r.get("n2", 0) for r in rows)
        refw = sum(r.get("refw", 0) for r in rows)
        n1w = sum(r.get("n1w", 0) for r in rows)
        n2w = sum(r.get("n2w", 0) for r in rows)
        avert = sum(r.get("averts", 0) for r in rows)
        alt_ok = sum(1 for r in rows if r.get("alt_core", {}).get("noyau_lignes") == 0)
        alt_n = sum(1 for r in rows if "alt_core" in r)
        pct = lambda a, b: f"{a/b:.4f}" if b else "—"
        lignes.append(
            f"| {lot} | {n} | {ecrits} | {ouverts} | {core_ok}/{ecrits} | "
            f"{pct(n1, notes_ref)} | {pct(n2, notes_ref)} | "
            f"{pct(n1w, refw)} | {pct(n2w, refw)} | {avert} | "
            f"{alt_ok}/{alt_n} |"
        )
        totaux[lot] = [
            n, ecrits, ouverts, core_ok, notes_ref, n1, n2, refw, n1w, n2w,
            avert, alt_ok, alt_n,
        ]
    n = sum(v[0] for v in totaux.values())
    ecrits = sum(v[1] for v in totaux.values())
    ouverts = sum(v[2] for v in totaux.values())
    core_ok = sum(v[3] for v in totaux.values())
    notes_ref = sum(v[4] for v in totaux.values())
    n1 = sum(v[5] for v in totaux.values())
    n2 = sum(v[6] for v in totaux.values())
    refw = sum(v[7] for v in totaux.values())
    n1w = sum(v[8] for v in totaux.values())
    n2w = sum(v[9] for v in totaux.values())
    avert = sum(v[10] for v in totaux.values())
    alt_ok = sum(v[11] for v in totaux.values())
    alt_n = sum(v[12] for v in totaux.values())
    pct = lambda a, b: f"{a/b:.4f}" if b else "—"
    lignes += [
        f"| **Total** | **{n}** | **{ecrits}** | **{ouverts}** | "
        f"**{core_ok}/{ecrits}** | **{pct(n1, notes_ref)}** | "
        f"**{pct(n2, notes_ref)}** | **{pct(n1w, refw)}** | "
        f"**{pct(n2w, refw)}** | **{avert}** | **{alt_ok}/{alt_n}** |",
        "",
        "Core-identical: MuseScore re-export of scoretk's `.mscz` matches "
        "scoretk's direct MusicXML (`core` normalisation, 0 changed line). "
        "N1/N2: scoretk's direct MIDI against MuseScore's playback of its "
        "`.mscz`. N1w/N2w (writer only): MuseScore's playback of the *source* "
        "against MuseScore's playback of our `.mscz`; playback differences "
        "between scoretk and MuseScore are excluded. Alt core-identical: "
        "MuseScore's own import of the source against scoretk's direct "
        "MusicXML, on the same files.",
        "",
        "## Writer-only N2 below 1.0",
        "",
        "| File | Ref notes | N1w | N2w | Cause |",
        "| --- | ---: | ---: | ---: | --- |",
    ]
    for lot in args.lots:
        chemin = Path(args.out) / f"{lot}.jsonl"
        if not chemin.exists():
            continue
        for ligne in chemin.read_text(encoding="utf-8").splitlines():
            r = json.loads(ligne)
            if not r.get("refw") or r.get("n2w", 1.0) >= 1.0:
                continue
            pct_w = r["n2w"] / r["refw"]
            if pct_w >= 1.0:
                continue
            lignes.append(
                f"| {lot}-{r['i']} {Path(r['fichier']).name} | {r['refw']} | "
                f"{pct(r.get('n1w', 0), r['refw'])} | {pct_w:.4f} | "
                f"{cause_n2w(r)} |"
            )
    return "\n".join(lignes) + "\n"


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--out", required=True, help="dossier des sorties brutes (hors dépôt)")
    p.add_argument("--scoretk", default="scoretk", help="binaire scoretk figé")
    p.add_argument("--mscore", default="mscore", help="MuseScore 4.7.4")
    p.add_argument("--lot", action="append", default=[], help="lot=chemin_du_manifeste")
    p.add_argument("--limit", type=int, default=0, help="nombre max de fichiers par lot")
    p.add_argument("--grid", type=float, default=0.25)
    p.add_argument("--tol", type=float, default=0.25)
    p.add_argument("--resume", action="store_true", help="ne refait que les fichiers manquants")
    p.add_argument("--skip-alt", action="store_true", help="ne pas mesurer l'import MuseScore source")
    p.add_argument("--skip-core", action="store_true", help="ne pas lancer l'aide Rust")
    args = p.parse_args()
    args.lots = []
    for entree in args.lot:
        nom, chemin = entree.split("=", 1)
        args.lots.append(nom)
        setattr(args, "lot_" + nom, chemin)
    if not args.lots:
        p.error("au moins un --lot nom=manifest")
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    if not args.resume:
        for lot in args.lots:
            with (out / f"{lot}.jsonl").open("w", encoding="utf-8") as fichier:
                mesure_lot(lot, args, fichier)
    if not args.skip_core:
        regenerer_paires(args)
        fusionner_core(args, out / "paires.tsv", out / "core.tsv", "core")
        fusionner_core(args, out / "alt_paires.tsv", out / "alt_core.tsv", "alt_core")
    texte = resume(args)
    (out / "resume.md").write_text(texte, encoding="utf-8")
    print(texte)


if __name__ == "__main__":
    main()
