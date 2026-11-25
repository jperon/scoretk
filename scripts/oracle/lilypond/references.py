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

    # Upgrade path of LilyPond itself, on a sandboxed copy of older sources:
    scripts/oracle/lilypond/references.py --manifest mut.txt --out refs-convertis \\
        --lot mut --jobs 2 --sandbox --sandbox-dir /path/to/writable \\
        --inject-midi --same-score --convert-ly

Le statut de chaque fichier va dans ``<out>/<lot>/status.jsonl`` ; un fichier
dont LilyPond échoue n'a pas de ``ly.midi`` et sera exclu par l'oracle comme
référence inutilisable. Sans ``--convert-ly`` aucune source n'est copiée :
LilyPond résout les ``\\include`` relatifs au fichier source. Avec
``--convert-ly``, seule une copie dans le dossier temporaire est réécrite et
le statut porte ``converti`` / ``modifie``.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
from functools import lru_cache
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


_RE_VERSION = re.compile(r'(?m)^\s*\\version\s*"(\d+(?:\.\d+)*)"')


def version_source(texte: str) -> tuple[int, ...] | None:
    """Declared ``\\version``, padded to three components (``None`` if absent).

    A commented-out declaration (``% \\version "…"``) does not match, since
    the command must start its line.  Includes may declare another version;
    the main file is the one LilyPond's ``convert-ly`` reads first.
    """
    m = _RE_VERSION.search(texte)
    if m is None:
        return None
    parties = [int(x) for x in m.group(1).split(".")]
    return tuple((parties + [0, 0, 0])[:3])


@lru_cache(maxsize=8)
def version_installee(lilypond: str) -> tuple[int, ...]:
    """Version reported by the very LilyPond that will compile the source."""
    p = subprocess.run([lilypond, "--version"], stdout=subprocess.PIPE,
                       stderr=subprocess.DEVNULL, check=False, timeout=30)
    m = re.search(r"(\d+)\.(\d+)(?:\.(\d+))?", p.stdout.decode("utf-8", "replace"))
    if m is None:
        raise RuntimeError(f"version LilyPond illisible : {lilypond}")
    return tuple(int(x or 0) for x in m.groups())


def doit_convertir(texte: str, lilypond: str | tuple[int, ...]) -> bool:
    """True when the source declares a version older than this LilyPond.

    A tuple is accepted for the selftest (no compiler subprocess needed).
    """
    declaree = version_source(texte)
    installee = version_installee(lilypond) if isinstance(lilypond, str) else lilypond
    return declaree is not None and declaree < installee


def fichiers_a_convertir(source: Path) -> list[Path]:
    """Main file and its transitive relative ``\\include``s, without cycles.

    A regression directory holds thousands of unrelated `.ly` files: running
    `convert-ly` on all of them for every source would be both slow and wrong.
    Only the files LilyPond actually loads from this copy need upgrading.
    """
    trouves: list[Path] = []
    vus: set[Path] = set()

    def visiter(fichier: Path) -> None:
        resolu = fichier.resolve()
        if resolu in vus or not fichier.is_file():
            return
        vus.add(resolu)
        trouves.append(fichier)
        texte = fichier.read_text(encoding="utf-8", errors="replace")
        for nom in re.findall(r'\\include\s*"([^"]+)"', texte):
            visiter(fichier.parent / nom)

    visiter(source)
    return trouves


def convertir_copie(
    dossier: Path, sandbox: Path, convert_ly: str = "",
    fichiers: list[Path] | None = None,
    version: tuple[int, ...] | None = None,
) -> dict:
    """Run the installed ``convert-ly`` on a writable *copy* of the sources.

    The upgrade is LilyPond's own path (`python/convertrules.py`), never a
    comparator change.  The corpus is read-only: only files under ``dossier``
    are rewritten, and ``convert-ly`` itself runs under the same bubblewrap
    sandbox as the compiler.  A failed conversion is rolled back to the
    original bytes so it cannot silently corrupt a reference.
    """
    if not convert_ly:
        convert_ly = shutil.which("convert-ly") or ""
    resultat = {"applique": False, "modifie": False, "fichiers": 0, "echecs": 0}
    if not convert_ly:
        return resultat
    cibles = fichiers if fichiers is not None else sorted(dossier.rglob("*"))
    for fichier in cibles:
        if not fichier.is_file() or fichier.suffix not in (".ly", ".ily"):
            continue
        avant = fichier.read_bytes()
        options: list[str] = []
        # Un `\include` n'a souvent pas de `\version` : LilyPond lit alors
        # celui du fichier principal. `convert-ly` ne le sait pas et prendrait
        # 2.27, ce qui laisserait un `\relative {` ancien non réécrit dans
        # l'inclusion (`sonate.ly`). On lui donne donc explicitement la version
        # déclarée par le fichier principal, sans ajouter de `\version` à la
        # copie (`--no-version`).
        if version is not None and version_source(avant.decode("utf-8", "replace")) is None:
            depuis = ".".join(str(x) for x in version)
            options = [f"--from={depuis}", "--no-version"]
        commande = ["timeout", "60", *bwrap_prefix(sandbox), convert_ly,
                    *options, "-e", str(fichier)]
        p = subprocess.run(commande, stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL, check=False, timeout=90)
        apres = fichier.read_bytes()
        if p.returncode != 0:
            if apres != avant:
                fichier.write_bytes(avant)
            resultat["echecs"] += 1
            continue
        resultat["applique"] = True
        if apres != avant:
            resultat["modifie"] = True
            resultat["fichiers"] += 1
    return resultat


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


def _convertir(source: Path, racine: Path) -> tuple[Path | None, bool]:
    """Copie le dossier de ``source`` et le passe par ``convert-ly``.

    Repli quand LilyPond 2.27 refuse une commande supprimée (son rattrapage
    insère des silences et lance une référence fausse). `convertir_copie`
    applique les règles de `python/convertrules.py` à une **copie** du dossier,
    sous bwrap ; les `\\include` restent relatifs.
    """
    copie = racine / "src-recup"
    shutil.copytree(source.parent, copie, dirs_exist_ok=True)
    resultat = convertir_copie(
        copie, racine, fichiers=fichiers_a_convertir(copie / source.name),
        version=version_source(source.read_text(encoding="utf-8", errors="replace")),
    )
    if not resultat["applique"]:
        return None, False
    converti = copie / source.name
    return (converti if converti.exists() else None), resultat["modifie"]


def compiler(
    source: Path, destination: Path, lilypond: str, timeout: int,
    sandbox: Path | None = None, convert_ly: bool = False,
) -> dict:
    """Compile une source et copie son MIDI ; renvoie une ligne de statut.

    Avec ``convert_ly``, une source qui déclare un ``\\version`` plus ancien
    que le compilateur installé est d'abord convertie par le ``convert-ly`` de
    **la même installation** (LilyPond 2.27), sur une copie dans le dossier
    temporaire, puis la copie convertie est compilée. Le repli sur erreur
    « commande inconnue » conserve le même chemin. Les champs ``converti`` et
    ``modifie`` du statut disent si `convert-ly` a été appliqué et s'il a
    réellement changé le fichier ; le corpus n'est jamais écrit.
    """
    debut = time.monotonic()
    converti = False
    modifie = False
    with tempfile.TemporaryDirectory(dir=sandbox) as tmp:
        base = Path(tmp)
        sortie = base / "ly"
        travail = source
        texte = source.read_text(encoding="utf-8", errors="replace")
        if convert_ly and doit_convertir(texte, lilypond):
            copie = base / "src"
            shutil.copytree(source.parent, copie)
            resultat = convertir_copie(
                copie, base, fichiers=fichiers_a_convertir(copie / source.name),
                version=version_source(texte),
            )
            converti, modifie = resultat["applique"], resultat["modifie"]
            travail = copie / source.name
        code, journal, midi = _lancer(travail, sortie, lilypond, timeout,
                                      base if sandbox else None)
        if not modifie and ("commande inconnue" in journal or "unknown command" in journal):
            converti_source, change = _convertir(source, base)
            if converti_source is not None:
                if midi.exists():
                    midi.unlink()
                code, journal, midi = _lancer(
                    converti_source, sortie, lilypond, timeout,
                    base if sandbox else None,
                )
                if midi.exists() and midi.stat().st_size > 0:
                    converti, modifie = True, change
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
        "modifie": modifie,
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
    convert_ly: bool = False,
) -> dict:
    """Compile une copie sandboxed avec ``\\midi { }`` injecté (regen-refs.py).

    Les lots externes n'ont pas de ``\\midi`` dans leurs sources : la
    référence est construite exactement comme `regen-refs.py` le fait, sur une
    copie locale (les ``\\include`` restent relatifs) et dans le bac à sable.
    Avec ``same_score``, l'injection vise le score que le lecteur importe et
    les ``\\midi`` des autres scores sont retirés, pour que LilyPond n'écrive
    qu'un seul MIDI : celui de ce score. Avec ``convert_ly``, une source dont
    le ``\\version`` est antérieur à LilyPond est d'abord passée par le
    ``convert-ly`` de la même installation, sur la copie et dans le même
    sandbox ; un MIDI brut ne peut alors plus être réutilisé.
    """
    debut = time.monotonic()
    texte_source = source.read_text(encoding="utf-8", errors="replace")
    conversion = convert_ly and doit_convertir(texte_source, lilypond)
    if not conversion and same_score and raw_ref is not None and raw_ref.is_file():
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
                "modifie": False,
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
        converti = False
        modifie = False
        if conversion:
            resultat = convertir_copie(
                dossier, base, fichiers=fichiers_a_convertir(dossier / source.name),
                version=version_source(texte_source),
            )
            converti, modifie = resultat["applique"], resultat["modifie"]
            if same_score:
                # convert-ly may rewrite a \score header; find the target again
                # on the converted copy, never in the corpus.
                cible = _fichier_score_lu(source, dossier)
        # Frozen model of the copy (conversion and include injection included):
        # the raw-injection fallback must restore this, not the obsolete source.
        modele = base / "src-modele"
        shutil.copytree(dossier, modele)
        texte_original = (dossier / source.name).read_text(encoding="utf-8",
                                                           errors="replace")
        bruts = candidats_avec_midi(texte_original) or [texte_original]
        if same_score and cible is not None:
            for fichier in sorted(dossier.rglob("*")):
                if fichier.suffix not in (".ly", ".ily") or not fichier.is_file():
                    continue
                texte = fichier.read_text(encoding="utf-8", errors="replace")
                texte_meme_score = pour_meme_score(
                    texte, cible[1] if fichier == cible[0] else None,
                )
                if texte_meme_score != texte:
                    fichier.write_text(texte_meme_score, encoding="utf-8")
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
                # Restore the converted/injected copy before the raw fallback.
                shutil.rmtree(dossier)
                shutil.copytree(modele, dossier)
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
        "converti": converti,
        "modifie": modifie,
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
    # --convert-ly: only a declared older \version triggers the upgrade.
    assert version_source('\\version "2.10.0"\n{ c4 }') == (2, 10, 0)
    assert version_source('\\version "2.27"\n{ c4 }') == (2, 27, 0)
    assert version_source('% \\version "2.10.0"\n{ c4 }') is None
    assert version_source('{ c4 }') is None
    # (2, 27, 0) is the LilyPond of this campaign; the production path reads
    # the version from the very compiler passed on the command line.
    assert doit_convertir('\\version "2.10.0"', (2, 27, 0))
    assert doit_convertir('\\version "2.27.0"', (2, 27, 0)) is False
    assert doit_convertir('\\version "2.28.0"', (2, 27, 0)) is False
    assert doit_convertir('{ c4 }', (2, 27, 0)) is False
    # Real convert-ly, same installation, inside the same bwrap sandbox, on a
    # copy only: obsolete command rewritten, version header updated, source
    # outside the copy untouched.
    if shutil.which("convert-ly") and shutil.which("bwrap"):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            dossier = base / "src"
            dossier.mkdir()
            vieux = '\\version "2.10.0"\n{ \\compressFullBarRests c4 }\n'
            (dossier / "vieux.ly").write_text(vieux, encoding="utf-8")
            original = base / "corpus.ly"
            original.write_text(vieux, encoding="utf-8")
            (dossier / "autre.ly").write_text(vieux, encoding="utf-8")
            # An include without \version inherits the main file's version.
            (dossier / "inc.ily").write_text("\\relative { c4 }\n", encoding="utf-8")
            (dossier / "vieux.ly").write_text(
                vieux + '\\include "inc.ily"\n', encoding="utf-8")
            cibles = fichiers_a_convertir(dossier / "vieux.ly")
            assert [f.name for f in cibles] == ["vieux.ly", "inc.ily"], cibles
            resultat = convertir_copie(dossier, base, fichiers=cibles,
                                       version=(2, 10, 0))
            assert resultat["applique"] and resultat["modifie"], resultat
            assert "\\compressEmptyMeasures" in (dossier / "vieux.ly").read_text(
                encoding="utf-8")
            assert "\\relative c'" in (dossier / "inc.ily").read_text(
                encoding="utf-8")
            assert "\\compressFullBarRests" in (dossier / "autre.ly").read_text(
                encoding="utf-8")
            converti = (dossier / "vieux.ly").read_text(encoding="utf-8")
            assert "\\compressEmptyMeasures" in converti and '\\version "2.27.0"' in converti
            assert original.read_text(encoding="utf-8") == vieux
            avant = converti
            assert convertir_copie(dossier, base, fichiers=cibles)["applique"]
            assert (dossier / "vieux.ly").read_text(encoding="utf-8") == avant
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
    p.add_argument("--convert-ly", action="store_true",
                   help="when a source declares a \\version older than this LilyPond, "
                        "run the installed convert-ly on a sandboxed copy before "
                        "compiling (LilyPond's own upgrade path, not a comparator change)")
    args = p.parse_args()
    if args.selftest:
        selftest()
        return
    if not args.manifest or not args.out or not args.lot:
        p.error("--manifest, --out and --lot are required")
    if args.same_score and not args.inject_midi:
        p.error("--same-score requires --inject-midi")
    if args.convert_ly and not args.sandbox:
        p.error("--convert-ly requires --sandbox: convert-ly rewrites an untrusted copy")

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
                    args.convert_ly,
                ))
        else:
            futurs = [
                pool.submit(compiler, source, destination, args.lilypond, args.timeout,
                            sandbox, args.convert_ly)
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
