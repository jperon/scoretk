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

"""Oracle d'écriture MusicXML : validité XSD et fidélité mesurée.

Pour chaque `.mscz` du manifeste on produit quatre sorties :

* ``tk.musicxml`` : scoretk, cible de la mesure ;
* ``ms.musicxml`` : MuseScore 4.7.4, référence de fidélité ;
* ``tk.mid`` / ``ms.mid`` : ce que chaque outil fait jouer au fichier.

On mesure alors (a) la validité XSD 4.0 des deux MusicXML, avec comptage des
messages d'erreur, (b) l'écart de noyau musical via la normalisation partagée
avec `e2e_corpus` (`crates/scoretk/tests/oracle_core.rs`), (c) N1/N2 sur les MIDI
avec `scripts/oracle/midi/midi_notes.py`. Aucune donnée tierce n'est versionnée.

Tout lancement de binaire passe par ``timeout`` et ``prlimit --as=2000000000``.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time

DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(DIR))
sys.path.insert(0, str(DIR.parent))
sys.path.insert(0, str(DIR.parent / "midi"))

import common  # noqa: E402
from midi_notes import compare, read_midi  # noqa: E402
from validite import preparer_xsd, valider  # noqa: E402

LIMITE_MSCORE = 30


def lancer(commande: list[str], env: dict | None = None) -> tuple[int, float, str]:
    debut = time.monotonic()
    try:
        p = subprocess.run(
            ["timeout", str(LIMITE_MSCORE), "prlimit", "--as=2000000000", *commande],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            env=env,
            check=False,
            timeout=LIMITE_MSCORE + 5,
        )
        return p.returncode, time.monotonic() - debut, p.stderr.decode("utf-8", "replace")[-300:]
    except subprocess.TimeoutExpired:
        return 124, time.monotonic() - debut, "dépassement de la borne de temps"


def cmd_scoretk(binaire: str, src: str, dst: str, sortie: str) -> list[str]:
    return [binaire, src, "--to", sortie, "-o", dst]


def cmd_mscore(binaire: str, src: str, dst: str) -> tuple[list[str], dict]:
    env = dict(os.environ)
    env.pop("LD_LIBRARY_PATH", None)  # wrapper Nix hostile aux bibliothèques Qt
    env["QT_QPA_PLATFORM"] = "offscreen"
    return [binaire, "-o", dst, src], env


def lire_manifest(chemin: str, limite: int) -> list[str]:
    with open(chemin, encoding="utf-8") as f:
        lignes = [l.strip() for l in f if l.strip() and not l.startswith("#")]
    return lignes[:limite] if limite else lignes


def ecrire(fichier, releve: dict) -> None:
    fichier.write(json.dumps(releve, ensure_ascii=False) + "\n")


def mesure(args, schema, env_xsd, paires, fichier) -> None:
    for i, src in enumerate(lire_manifest(args.manifest, args.limit)):
        travail = Path(args.out) / "travail" / str(i)
        travail.mkdir(parents=True, exist_ok=True)
        r = {"fichier": src, "ok": False, "erreur": "", "temps": 0.0}
        tkxml, msxml = travail / "tk.musicxml", travail / "ms.musicxml"
        tkmid, msmid = travail / "tk.mid", travail / "ms.mid"

        etapes = [
            ("scoretk_xml", cmd_scoretk(args.scoretk, src, str(tkxml), "musicxml"), None),
            ("mscore_xml", *cmd_mscore(args.mscore, src, str(msxml))),
            ("scoretk_midi", cmd_scoretk(args.scoretk, src, str(tkmid), "midi"), None),
            ("mscore_midi", *cmd_mscore(args.mscore, src, str(msmid))),
        ]
        for nom, commande, env in etapes:
            code, duree, erreur = lancer(commande, env)
            r["temps"] += round(duree, 3)
            # Entrée délibérément non prise en charge : exclue de la mesure,
            # jamais comptée comme un échec de conversion.
            if nom.startswith("scoretk") and common.est_non_pris_en_charge(code):
                common.marquer_exclu(r, src)
                r["erreur"] = f"{nom} : entrée non prise en charge (code {code})"
                ecrire(fichier, r)
                fichier.flush()
                break
            if code != 0:
                r["erreur"] = f"{nom} : code {code} {erreur}"
                break
        if common.est_exclu(r):
            continue
        cibles = [tkxml, msxml, tkmid, msmid]
        if r["erreur"] or any(not p.exists() or p.stat().st_size == 0 for p in cibles):
            r["erreur"] = r["erreur"] or "sortie absente ou vide"
            ecrire(fichier, r)
            fichier.flush()
            continue

        r["ok"] = True
        for cle, chemin in (("xsd_tk", tkxml), ("xsd_ms", msxml)):
            v = valider(schema, env_xsd, chemin)
            r[cle] = {
                "valide": v["valide"], "erreurs": v["erreurs"],
                "messages": v["messages"],
            }
        try:
            ref, test = read_midi(str(msmid)), read_midi(str(tkmid))
            r.update(compare(ref, test, args.grid, args.tol))
        except Exception as exc:  # MIDI illisible : noté, pas masqué
            r["erreur_midi"] = str(exc)[:200]
        paires.write(f"{i}\t{msxml}\t{tkxml}\n")
        ecrire(fichier, r)
        fichier.flush()


def fusionner_core(args) -> None:
    """Lance l'aide Rust une fois, puis fusionne ses résultats dans le relevé."""
    manifeste = Path(args.out) / "paires.tsv"
    sortie = Path(args.out) / "core.tsv"
    env = dict(os.environ)
    env["SCORETK_ORACLE_MANIFEST"] = str(manifeste)
    env["SCORETK_ORACLE_OUTPUT"] = str(sortie)
    p = subprocess.run(
        ["nix", "develop", "-c", "cargo", "test", "-q", "-p", "scoretk",
         "--test", "oracle_core", "--", "--ignored"],
        cwd=str(DIR.parent.parent.parent), env=env, check=False,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=600,
    )
    if p.returncode != 0:
        print("ÉCHEC aide core :", p.stdout.decode("utf-8", "replace")[-1000:])
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
    chemin = Path(args.out) / "mesure.jsonl"
    lignes = []
    for index, ligne in enumerate(chemin.read_text(encoding="utf-8").splitlines()):
        r = json.loads(ligne)
        if str(index) in core:
            r["core"] = core[str(index)]
        lignes.append(r)
    with chemin.open("w", encoding="utf-8") as f:
        for r in lignes:
            ecrire(f, r)


FAMILLES = {
    "note": "notes", "pitch": "notes", "staff": "notes", "voice": "notes",
    "duration": "durations", "type": "durations", "dot": "durations",
    "time-modification": "durations", "backup": "durations", "forward": "durations",
    "measure": "measures", "attributes": "measures",
    "key": "keys", "fifths": "keys", "time": "times", "beats": "times",
    "clef": "clefs", "sign": "clefs",
    "tie": "ties", "tied": "ties",
    "lyric": "lyrics", "text": "lyrics", "syllabic": "lyrics",
    "direction": "directions", "direction-type": "directions",
    "words": "directions", "sound": "directions", "dynamics": "directions",
}


def resume(args) -> str:
    rows = []
    chemin = Path(args.out) / "mesure.jsonl"
    with chemin.open(encoding="utf-8") as f:
        toutes = [json.loads(l) for l in f if l.strip()]
    exclus = common.noms_exclus(toutes)
    # Une entrée non prise en charge sort du numérateur et du dénominateur.
    rows = common.filtrer(toutes)
    n = len(rows)
    ok = sum(r["ok"] for r in rows)
    invalides_tk = sum(r.get("xsd_tk", {}).get("erreurs", 0) for r in rows)
    invalides_ms = sum(r.get("xsd_ms", {}).get("erreurs", 0) for r in rows)
    fichiers_invalides = sum(
        1 for r in rows if "xsd_tk" in r and not r["xsd_tk"]["valide"]
    )
    notes_ref = sum(r.get("ref", 0) for r in rows)
    n1 = sum(r.get("n1", 0) for r in rows)
    n2 = sum(r.get("n2", 0) for r in rows)
    core_ok = sum(1 for r in rows if r.get("core", {}).get("noyau_lignes") == 0)
    familles: dict[str, int] = {}
    messages: dict[str, int] = {}
    for r in rows:
        for cle, v in (("xsd_tk", "scoretk"), ("xsd_ms", "musescore")):
            for message, count in r.get(cle, {}).get("messages", {}).items():
                messages[f"{v}: {message}"] = messages.get(f"{v}: {message}", 0) + count
        for tag, count in r.get("core", {}).get("tags", {}).items():
            f = FAMILLES.get(tag, "autre")
            familles[f] = familles.get(f, 0) + int(count)

    lignes = [
        "# MusicXML export oracle — baseline",
        "",
        f"Scores: {n} | converted: {ok} | XSD-invalid (scoretk): {fichiers_invalides}",
        f"XSD errors — scoretk: {invalides_tk} | MuseScore 4.7.4: {invalides_ms}",
        f"Core-identical to MuseScore: {core_ok}/{ok}",
        f"N1 (pitch multiset): {n1/notes_ref if notes_ref else 0:.4f} | "
        f"N2 (pitch+attack): {n2/notes_ref if notes_ref else 0:.4f}",
        common.ligne_resume(exclus),
        "",
        "## XSD error messages (count)",
        "",
        "| Count | Message |", "| ---: | --- |",
    ]
    for message, count in sorted(messages.items(), key=lambda kv: -kv[1])[:20]:
        lignes.append(f"| {count} | {message} |")
    lignes += ["", "## Core differences by family (changed lines)", "",
               "| Family | Lines |", "| --- | ---: |"]
    for f, count in sorted(familles.items(), key=lambda kv: -kv[1]):
        lignes.append(f"| {f} | {count} |")
    common.avertir(exclus)
    return "\n".join(lignes) + "\n"


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--out", required=True, help="dossier des sorties brutes (hors dépôt)")
    p.add_argument("--manifest", required=True, help="manifeste .mscz")
    p.add_argument("--scoretk", default="scoretk", help="binaire scoretk")
    p.add_argument("--mscore", default="mscore", help="MuseScore 4.7.4")
    p.add_argument("--limit", type=int, default=0, help="nombre max de fichiers (0 = tous)")
    p.add_argument("--grid", type=float, default=0.25)
    p.add_argument("--tol", type=float, default=0.25)
    p.add_argument("--resume", action="store_true", help="ne refait que le résumé")
    p.add_argument("--skip-core", action="store_true")
    args = p.parse_args()
    Path(args.out).mkdir(parents=True, exist_ok=True)
    if not args.resume:
        schema, env = preparer_xsd(Path(args.out) / "xsd")
        with (Path(args.out) / "paires.tsv").open("w", encoding="utf-8") as paires:
            with (Path(args.out) / "mesure.jsonl").open("w", encoding="utf-8") as f:
                mesure(args, schema, env, paires, f)
        if not args.skip_core:
            fusionner_core(args)
    texte = resume(args)
    (Path(args.out) / "resume.md").write_text(texte, encoding="utf-8")
    print(texte)


if __name__ == "__main__":
    main()
