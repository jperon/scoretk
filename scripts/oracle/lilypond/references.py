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
    scripts/oracle/lilypond/references.py --manifest ext.txt --out refs-meme \\
        --lot ext --jobs 1 --sandbox --sandbox-dir /path/to/writable \\
        --inject-midi --same-score

Le statut de chaque fichier va dans ``<out>/<lot>/status.jsonl`` ; un fichier
dont LilyPond échoue n'a pas de ``ly.midi`` et sera exclu par l'oracle comme
référence inutilisable. Aucune source n'est copiée : LilyPond résout les
``\\include`` relatifs au fichier source.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import re
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


def _masque_code(texte: str) -> str:
    """Same length as ``texte``, with comments and string bodies blanked.

    Brace matching must not see the braces of a Scheme expression, a comment
    or a markup string; only LilyPond's syntactic braces count.
    """
    out = list(texte)
    i, n = 0, len(texte)
    while i < n:
        c = texte[i]
        if c == "%":
            if i + 1 < n and texte[i + 1] == "{":
                fin = texte.find("%}", i + 2)
                fin = n if fin < 0 else fin + 2
            else:
                fin = texte.find("\n", i)
                fin = n if fin < 0 else fin
            for j in range(i, fin):
                out[j] = " "
            i = fin
        elif c == '"':
            j = i + 1
            while j < n:
                if texte[j] == "\\":
                    j += 2
                    continue
                if texte[j] == '"':
                    j += 1
                    break
                j += 1
            for k in range(i + 1, min(j - 1, n)):
                out[k] = " "
            i = j
        else:
            i += 1
    return "".join(out)


def _fin_accolade(masque: str, ouverture: int) -> int:
    """Index of the ``}`` closing the brace at ``ouverture`` (-1 if none)."""
    profondeur = 0
    for i in range(ouverture, len(masque)):
        if masque[i] == "{":
            profondeur += 1
        elif masque[i] == "}":
            profondeur -= 1
            if profondeur == 0:
                return i
    return -1


def blocs_score(texte: str) -> list[tuple[int, int, bool]]:
    """Every ``\\score { … }`` as ``(start, closing_brace, has_midi)``."""
    masque = _masque_code(texte)
    blocs = []
    for match in re.finditer(r"\\score\b", masque):
        ouverture = masque.find("{", match.end())
        if ouverture < 0:
            continue
        fin = _fin_accolade(masque, ouverture)
        if fin < 0:
            continue
        blocs.append((match.start(), fin,
                      bool(re.search(r"\\midi\b", masque[ouverture:fin]))))
    return blocs


def _retire_midi(texte: str, debut: int, fin: int) -> str:
    """Remove ``\\midi { … }`` inside one score body, leaving text elsewhere."""
    masque = _masque_code(texte)
    for match in re.finditer(r"\\midi\b", masque[debut:fin]):
        pos = debut + match.start()
        j = debut + match.end()
        while j < fin and masque[j].isspace():
            j += 1
        if j < fin and masque[j] == "{":
            j = _fin_accolade(masque, j)
            if j < 0:
                continue
            return texte[:pos] + " " * (j + 1 - pos) + texte[j + 1:]
        return texte[:pos] + " " * (match.end() - match.start()) + texte[pos + (match.end() - match.start()):]
    return texte


def pour_meme_score(texte: str, index_cible: int | None) -> str:
    """Inject ``\\midi { }`` in the target score, strip it from the others.

    ``index_cible`` is the rank of the score the reader imports inside this
    file, or ``None`` for a file holding only other scores.  The choice is made
    on the copy of the whole source tree (main file first, then included
    files), so LilyPond emits exactly one MIDI: the one of that score.
    """
    blocs = blocs_score(texte)
    if not blocs:
        return texte
    resultat = texte
    # Work from the end: edits never shift earlier offsets.
    for index in range(len(blocs) - 1, -1, -1):
        debut, fin, has_midi = blocs[index]
        if index != index_cible:
            resultat = _retire_midi(resultat, debut, fin)
        elif not has_midi:
            resultat = resultat[:fin] + "\n  \\midi { }\n" + resultat[fin:]
    return resultat


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


def _fichier_score_lu(source: Path, dossier: Path) -> tuple[Path, int] | None:
    """The file holding the score expression the reader imports.

    Reader selection (`crates/scoretk-lilypond-import/src/parser.rs`,
    `Parser::document`, l. 506-569): explicit ``\\score`` blocks are collected
    everywhere (``Parser::toplevel_item``, l. 637-650, recursing through
    ``\\book`` in `Parser::parse_book`, l. 739-748), implicit top-level music
    expressions are the fallback (`Parser::parse_toplevel_music`, l. 783-806),
    and the **first block carrying ``\\midi`` wins** (l. 549-553).  The main
    file is visited before the files it includes, in textual include order;
    a source with no ``\\score`` at all keeps the raw injection below.
    """
    # Only the files the source actually includes (transitively): every
    # directory of a repository holds the other pieces' includes, and taking
    # the first score-bearing file by path would select the wrong score.
    fichiers: list[Path] = []
    vus: set[Path] = set()

    def visiter(fichier: Path) -> None:
        resolu = fichier.resolve()
        if resolu in vus or not fichier.is_file():
            return
        vus.add(resolu)
        fichiers.append(fichier)
        texte = fichier.read_text(encoding="utf-8", errors="replace")
        for nom in re.findall(r'\\include\s*"([^"]+)"', texte):
            visiter(fichier.parent / nom)

    visiter(dossier / source.name)
    for fichier in fichiers:
        if not fichier.is_file():
            continue
        blocs = blocs_score(fichier.read_text(encoding="utf-8", errors="replace"))
        for index, (_, _, has_midi) in enumerate(blocs):
            if has_midi:
                return fichier, index
    for fichier in fichiers:
        if fichier.is_file():
            blocs = blocs_score(fichier.read_text(encoding="utf-8", errors="replace"))
            if blocs:
                return fichier, 0
    return None


def compiler_injecte(
    source: Path, destination: Path, lilypond: str, timeout: int, sandbox: Path,
    same_score: bool = False, raw_ref: Path | None = None,
) -> dict:
    """Compile une copie sandboxed avec ``\\midi { }`` injecté (regen-refs.py).

    Les lots externes n'ont pas de ``\\midi`` dans leurs sources : la
    référence est construite exactement comme `regen-refs.py` le fait, sur une
    copie locale (les ``\\include`` restent relatifs) et dans le bac à sable.
    Avec ``same_score``, l'injection vise le score que le lecteur importe et
    les ``\\midi`` des autres scores sont retirés, pour que LilyPond n'écrive
    qu'un seul MIDI : celui de ce score.
    """
    debut = time.monotonic()
    if same_score and raw_ref is not None and raw_ref.is_file():
        cible = _fichier_score_lu(source, source.parent)
        blocs_main = blocs_score(source.read_text(encoding="utf-8", errors="replace"))
        identique = cible is None or (
            cible[0].name == source.name
            and (any(has_midi for _, _, has_midi in blocs_main) or len(blocs_main) <= 1)
        )
        if identique:
            # No score to retarget, or the raw injection already plays the
            # reader's score: regen-refs.py injects `\midi` in the last score,
            # and LilyPond writes the first MIDI of the first score carrying
            # `\midi`, which is the score the reader selects.  The same-score
            # reference is then exactly the raw one.
            destination.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(raw_ref, destination / "ly.midi")
            return {
                "fichier": str(source), "ok": True, "code": 0, "converti": False,
                "injecte": True, "same_score": True, "copie_brute": True,
                "temps": round(time.monotonic() - debut, 3), "erreur": "",
            }
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

        cible = _fichier_score_lu(source, dossier) if same_score else None
        include_args = [] if cible is not None else prepare_includes(source, dossier)
        texte_original = (dossier / source.name).read_text(encoding="utf-8",
                                                           errors="replace")
        bruts = candidats_avec_midi(texte_original) or [texte_original]
        if same_score and cible is not None:
            for fichier in sorted(dossier.rglob("*")):
                if fichier.suffix not in (".ly", ".ily") or not fichier.is_file():
                    continue
                texte = fichier.read_text(encoding="utf-8", errors="replace")
                modifie = pour_meme_score(texte, cible[1] if fichier == cible[0] else None)
                if modifie != texte:
                    fichier.write_text(modifie, encoding="utf-8")
            # The targeted copy is tried first.  If the selected score lives
            # in an include that is never engraved, LilyPond writes no MIDI;
            # the raw injection is then the honest fallback.
            candidats = [(dossier / source.name).read_text(encoding="utf-8",
                                                           errors="replace")]
            repli = bruts
        else:
            candidats = bruts
            repli = []
        sortie = base / "ly"
        ok = False
        journal = "aucun candidat n'a compilé"
        essais = [(c, False) for c in candidats] + [(c, True) for c in repli]
        for candidat, restaurer in essais:
            if restaurer:
                # Restore pristine includes before the raw fallback.
                shutil.rmtree(dossier)
                shutil.copytree(source.parent, dossier)
                if include_args:
                    prepare_includes(source, dossier)
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
        "same_score": same_score,
        "temps": round(time.monotonic() - debut, 3),
        "erreur": "" if ok else journal[-400:],
    }


def selftest() -> None:
    """The same-score transformation must leave exactly one playable score."""
    source = (
        "\\score { { c'1 } }\n"
        "\\score { \\unfoldRepeats { c'2 d'2 } \\midi { \\tempo 4=60 } }\n"
    )
    blocs = blocs_score(source)
    assert len(blocs) == 2 and not blocs[0][2] and blocs[1][2]
    premier = pour_meme_score(source, 0)
    second = pour_meme_score(source, 1)
    assert "\\midi" in premier.split("\\score")[1]
    assert "\\midi" not in premier.split("\\score")[2]
    assert "\\midi" not in second.split("\\score")[1]
    assert "\\midi" in second.split("\\score")[2]
    # Comments and strings must not create or hide score blocks.
    assert len(blocs_score("% \\score { c'1 }\n\"\\score { d'1 }\"\n\\score { e'1 }")) == 1
    print("selftest références : OK")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--selftest", action="store_true", help="check the same-score transformation")
    p.add_argument("--manifest", help="une source .ly par ligne")
    p.add_argument("--out", default="", help="racine des références")
    p.add_argument("--lot", default="", help="nom du lot (mut, ra, mus)")
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
    p.add_argument("--same-score", action="store_true",
                   help="with --inject-midi, target the score expression the reader imports "
                        "(first \\midi score, else first explicit score, else first "
                        "top-level music) and strip \\midi from every other score")
    p.add_argument("--raw-refs", default="",
                   help="raw reference tree; with --same-score, reuse a raw MIDI when "
                        "there is no score block to retarget")
    args = p.parse_args()
    if args.selftest:
        selftest()
        return
    if not args.manifest or not args.out or not args.lot:
        p.error("--manifest, --out and --lot are required")
    if args.same_score and not args.inject_midi:
        p.error("--same-score requires --inject-midi")

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
            futurs = []
            for source, destination in a_faire:
                brut = (
                    Path(args.raw_refs) / args.lot / nom_court(str(source)) / "ly.midi"
                    if args.raw_refs else None
                )
                futurs.append(pool.submit(
                    compiler_injecte, source, destination,
                    args.lilypond, args.timeout, sandbox, args.same_score, brut,
                ))
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
