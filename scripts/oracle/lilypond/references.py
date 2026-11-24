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

"""Régénère les références MIDI de l'oracle LilyPond.

L'oracle d'import compare la sortie de scoretk au MIDI produit par le
LilyPond **installé** sur les mêmes sources. Une référence doit donc être
régénérable : un ``ly.midi`` absent, produit par une autre version ou par un
autre choix de ``\\score`` est inutilisable. Ce script compile chaque source
du manifeste et range le MIDI sous ``<out>/<lot>/<nom_court>/ly.midi``, la
disposition attendue par ``mesure.py --refs`` avec un préfixe vide.

Usage :
    scripts/oracle/lilypond/references.py --manifest mut.txt --out refs \\
        --lot mut [--jobs 8] [--timeout 60] [--limit N] [--resume]
    scripts/oracle/lilypond/references.py --manifest ext.txt --out refs \\
        --lot ext --jobs 1 --sandbox --sandbox-dir /path/to/writable \\
        --inject-midi   # untrusted third-party .ly, one worker

Le statut de chaque fichier va dans ``<out>/<lot>/status.jsonl`` ; un fichier
dont LilyPond échoue n'a pas de ``ly.midi`` et sera exclu par l'oracle comme
référence inutilisable. Aucune source n'est copiée : LilyPond résout les
``\\include`` relatifs au fichier source.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))

from mesure import nom_court  # noqa: E402


def _midi_produit(sortie: Path) -> Path:
    """LilyPond names one MIDI per score (``ly-1-….midi``, ``ly-2-…``)."""
    direct = sortie.with_suffix(".midi")
    if direct.is_file():
        return direct
    candidats = sorted(sortie.parent.glob("*.midi"))
    return candidats[0] if candidats else direct


def bwrap_prefix(dossier: Path) -> list[str]:
    """Sandbox LilyPond: no network, source read-only, only ``dossier`` writable.

    Compiling a `.ly` executes its embedded Scheme.  The sandbox is therefore
    mandatory for untrusted third-party sources.  The Nix store and the source
    tree stay visible through the read-only root bind; ``dossier`` is the only
    writable path (output, font cache, injected copies of the source).
    """
    d = str(dossier)
    return [
        "bwrap", "--unshare-all", "--die-with-parent",
        "--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc",
        "--tmpfs", "/tmp", "--bind", d, d,
        "--setenv", "HOME", d, "--setenv", "XDG_CACHE_HOME", d + "/cache",
        "--chdir", d, "--",
    ]


def _lancer(
    source: Path, sortie: Path, lilypond: str, timeout: int,
    sandbox: Path | None = None, extra: list[str] | None = None,
) -> tuple:
    """Compile ``source`` vers ``sortie`` ; (code, erreur, midi)."""
    commande = [
        lilypond, *(extra or []), "-dno-print-pages", "-dmidi-extension=midi",
        "-o", str(sortie), str(source),
    ]
    if sandbox is not None:
        commande = bwrap_prefix(sandbox) + commande
    commande = ["timeout", str(timeout), "prlimit", "--as=4000000000", *commande]
    try:
        p = subprocess.run(
            commande,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            timeout=timeout + 5,
            check=False,
        )
        return p.returncode, p.stderr.decode("utf-8", "replace"), _midi_produit(sortie)
    except subprocess.TimeoutExpired:
        return 124, "dépassement de la borne de temps", sortie.with_suffix(".midi")


def _convertir(source: Path, racine: Path) -> Path | None:
    """Copie le dossier de ``source`` et le passe par ``convert-ly``.

    LilyPond 2.27 refuse les commandes supprimées après 2.10/2.16
    (`\\compressMusic`, `\\compressFullBarRests`, `\\setTextCresc`…) et son
    rattrapage insère des silences : la référence MIDI serait fausse. Le
    ``convert-ly`` de la même installation applique les règles de
    `python/convertrules.py` (par exemple l. 2765 et l. 4239) à une **copie**
    du dossier (les `\\include` restent relatifs), puis on compile la copie.
    """
    convert_ly = shutil.which("convert-ly")
    if not convert_ly:
        return None
    copie = racine / "src"
    shutil.copytree(source.parent, copie, dirs_exist_ok=True)
    for fichier in sorted(copie.rglob("*")):
        if fichier.suffix not in (".ly", ".ily"):
            continue
        subprocess.run(
            [convert_ly, "-e", str(fichier)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=60,
            check=False,
        )
    converti = copie / source.name
    return converti if converti.exists() else None


def compiler(
    source: Path, destination: Path, lilypond: str, timeout: int,
    sandbox: Path | None = None,
) -> dict:
    """Compile une source et copie son MIDI ; renvoie une ligne de statut.

    Si LilyPond signale une commande inconnue (syntaxe supprimée que
    ``convert-ly`` sait réécrire), la source est convertie dans une copie et
    recompilée : la référence reste alors celle du LilyPond installé, mais sur
    le source que `convert-ly` produirait, au lieu d'être jetée.
    """
    debut = time.monotonic()
    converti = False
    with tempfile.TemporaryDirectory(dir=sandbox) as tmp:
        sortie = Path(tmp) / "ly"
        code, journal, midi = _lancer(source, sortie, lilypond, timeout, Path(tmp) if sandbox else None)
        if "commande inconnue" in journal or "unknown command" in journal:
            converti_source = _convertir(source, Path(tmp))
            if converti_source is not None:
                if midi.exists():
                    midi.unlink()
                code, journal, midi = _lancer(
                    converti_source, sortie, lilypond, timeout,
                    Path(tmp) if sandbox else None,
                )
                if midi.exists() and midi.stat().st_size > 0:
                    converti = True
        if midi.exists() and midi.stat().st_size > 0:
            destination.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(midi, destination / "ly.midi")
            ok = True
        else:
            ok = False
    return {
        "fichier": str(source),
        "ok": ok,
        "code": code,
        "converti": converti,
        "temps": round(time.monotonic() - debut, 3),
        "erreur": "" if ok else journal[-400:],
    }


def compiler_injecte(
    source: Path, destination: Path, lilypond: str, timeout: int, sandbox: Path
) -> dict:
    """Compile une copie sandboxed avec ``\\midi { }`` injecté (regen-refs.py).

    Les lots externes n'ont pas de ``\\midi`` dans leurs sources : la
    référence est construite exactement comme `regen-refs.py` le fait, sur une
    copie locale (les ``\\include`` restent relatifs) et dans le bac à sable.
    """
    debut = time.monotonic()
    with tempfile.TemporaryDirectory(dir=sandbox) as tmp:
        base = Path(tmp)
        dossier = base / "src"
        shutil.copytree(source.parent, dossier)
        # `regen-refs.py` has a hyphen in its name: load it by path rather than
        # duplicating its MIDI-injection logic here.
        import importlib.util
        spec = importlib.util.spec_from_file_location(
            "regen_refs", Path(__file__).resolve().parent / "regen-refs.py"
        )
        regen = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(regen)
        candidats_avec_midi = regen.candidats_avec_midi
        prepare_includes = regen.prepare_includes

        texte = (dossier / source.name).read_text(encoding="utf-8", errors="replace")
        candidats = candidats_avec_midi(texte) or [texte]
        include_args = prepare_includes(source, dossier)
        sortie = base / "ly"
        ok = False
        journal = "aucun candidat n'a compilé"
        for candidat in candidats:
            (dossier / source.name).write_text(candidat, encoding="utf-8")
            code, journal, midi = _lancer(
                dossier / source.name, sortie, lilypond, timeout,
                sandbox=base, extra=include_args,
            )
            if midi.exists() and midi.stat().st_size > 0:
                ok = True
                break
        if ok:
            destination.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(midi, destination / "ly.midi")
    return {
        "fichier": str(source),
        "ok": ok,
        "code": 0 if ok else code,
        "converti": False,
        "injecte": True,
        "temps": round(time.monotonic() - debut, 3),
        "erreur": "" if ok else journal[-400:],
    }


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--manifest", required=True, help="une source .ly par ligne")
    p.add_argument("--out", required=True, help="racine des références")
    p.add_argument("--lot", required=True, help="nom du lot (mut, ra, mus)")
    p.add_argument("--lilypond", default="lilypond")
    p.add_argument("--jobs", type=int, default=8)
    p.add_argument("--timeout", type=int, default=60)
    p.add_argument("--limit", type=int, default=0)
    p.add_argument(
        "--resume",
        action="store_true",
        help="ne pas recompiler un fichier dont le ly.midi existe déjà",
    )
    p.add_argument("--sandbox", action="store_true",
                   help="compile under bubblewrap (mandatory for untrusted sources)")
    p.add_argument("--sandbox-dir", default="",
                   help="writable parent for sandbox temporary directories")
    p.add_argument("--inject-midi", action="store_true",
                   help="inject \\midi { } into a copy, as regen-refs.py does")
    args = p.parse_args()

    sandbox = None
    if args.sandbox:
        sandbox = Path(args.sandbox_dir) if args.sandbox_dir else Path(args.out).parent / "sandbox"
        sandbox.mkdir(parents=True, exist_ok=True)

    sources = [
        Path(ligne.strip())
        for ligne in Path(args.manifest).read_text(encoding="utf-8").splitlines()
        if ligne.strip()
    ]
    if args.limit:
        sources = sources[: args.limit]
    racine = Path(args.out) / args.lot
    racine.mkdir(parents=True, exist_ok=True)
    statut = racine / "status.jsonl"

    a_faire = []
    for source in sources:
        destination = racine / nom_court(str(source))
        if args.resume and (destination / "ly.midi").exists():
            continue
        a_faire.append((source, destination))

    with statut.open("w", encoding="utf-8") as journal, ThreadPoolExecutor(
        max_workers=args.jobs
    ) as pool:
        if args.inject_midi:
            if sandbox is None:
                p.error("--inject-midi requires --sandbox for untrusted sources")
            futurs = [
                pool.submit(compiler_injecte, source, destination,
                            args.lilypond, args.timeout, sandbox)
                for source, destination in a_faire
            ]
        else:
            futurs = [
                pool.submit(compiler, source, destination, args.lilypond, args.timeout,
                            sandbox)
                for source, destination in a_faire
            ]
        for index, futur in enumerate(futurs, 1):
            ligne = futur.result()
            journal.write(json.dumps(ligne, ensure_ascii=False) + "\n")
            journal.flush()
            if index % 100 == 0:
                print(f"{index}/{len(futurs)}", file=sys.stderr)

    lignes = [json.loads(l) for l in statut.read_text(encoding="utf-8").splitlines()]
    ok = sum(1 for l in lignes if l["ok"])
    print(f"{args.lot} : {ok}/{len(lignes)} références régénérées")


if __name__ == "__main__":
    main()
