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

"""Oracle d'import MIDI : relevé par fichier, comparaison des notes du modèle.

Deux lots sans donnée versionnée :

* les tests amont de MuseScore (`importexport/midi/tests/midiimport_data`) :
  un `.mid` et son `.mscx` attendu (lecteur C++) ;
* les MIDI LilyPond de `~/Musique` dont la source `.ly` est lue : le `.ly`
  fournit la quantification idéale (hauteurs, attaques, durées).

Pour chaque paire, scoretk lit le MIDI et le `.mscx`/`.ly` de référence, et on
compare les notes `(hauteur, attaque, durée, voix)` de deux exports MusicXML
sur une grille de 1/16 de noire (+/- 1/16 de tolérance), comme `midi_notes.py`.
Seules les mesures sont lues : aucun fichier tiers n'est copié dans le dépôt.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import xml.etree.ElementTree as ET

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import common  # noqa: E402
import midi_notes  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "lilypond"))
import ly_notes  # noqa: E402

GRID = 0.25
TOL = 0.25
LIMITE = 15


def notes(
    path: Path, to_concert: bool = False, graces: list | None = None
) -> list[tuple[int, float, float, int]]:
    """Notes d'un MusicXML partwise : hauteur, attaque, durée, voix (noires).

    `to_concert` ramène les hauteurs écrites aux hauteurs concert en
    annulant `<transpose>`. La comparaison se fait **toujours** en concert :
    le MIDI ne porte que la hauteur entendue, et l'import invente un
    instrument (donc une transposition) à partir du programme General MIDI ;
    comparer la hauteur écrite des deux côtés ferait apparaître un décalage
    qui n'existe pas dans le fichier (une référence `.ly` avec
    `\transposition` face à un import qui choisit un autre instrument). Le
    `.mscx` de référence stocke déjà la hauteur concert.

    Quand ``graces`` est une liste, les notes ``<grace>`` y sont ajoutées
    (hauteur, attaque de la note principale) et retirées de la comparaison :
    un MIDI n'a pas de grâce, l'import ne peut pas la noter
    (`importmidi_chord.cpp` n'a pas de module de grâces) et elle devient une
    note normale avant le temps.

    Les chaînes de liaisons sont fusionnées en **une** note dont la durée est
    la somme des fragments (`<tie type="start"/>` … `<tie type="stop"/>`), des
    deux côtés de la comparaison : une note tenue qui franchit un temps est
    écrite par MuseScore comme deux notes liées, et compter les fragments
    pénaliserait tout import qui écrit une seule note. Les chaînes sont
    distinguées par (part, portée, voix, hauteur).
    """
    root = ET.parse(path).getroot()
    raw: list[list] = []  # [hauteur, attaque, durée, voix], mutables
    open_ties: dict[tuple[int, int, int, int], list] = {}
    for part_index, part in enumerate(root.findall("part")):
        transpose = 0
        if to_concert:
            for element in part.findall(".//attributes/transpose"):
                chromatic = int(element.findtext("chromatic", "0"))
                octave = int(element.findtext("octave-change", "0"))
                transpose = chromatic + 12 * octave
                break
        divisions = 1
        tick = 0.0
        for measure in part.findall("measure"):
            offset = 0.0
            for child in measure:
                if child.tag == "attributes":
                    value = child.findtext("divisions")
                    if value:
                        divisions = int(value)
                elif child.tag in ("backup", "forward"):
                    delta = int(child.findtext("duration", "0")) / divisions
                    offset += -delta if child.tag == "backup" else delta
                elif child.tag == "note":
                    dur = int(child.findtext("duration", "0")) / divisions
                    is_chord_note = child.find("chord") is not None
                    # Une note avec <chord/> partage l'attaque de la précédente :
                    # elle est comptée à `offset - dur` et n'avance pas le curseur.
                    onset = tick + offset - dur if is_chord_note else tick + offset
                    if child.find("rest") is None:
                        pitch = child.find("pitch")
                        if pitch is not None:
                            step = pitch.findtext("step", "C")
                            semi = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}[step]
                            semi += int(pitch.findtext("alter", "0"))
                            midi = 12 * (int(pitch.findtext("octave", "0")) + 1) + semi + transpose
                            raw_voice = int(child.findtext("voice", "1"))
                            staff = int(child.findtext("staff", "1"))
                            # La sortie MusicXML numérote les voix globalement
                            # `(staff - 1) * VOICES + voice` ; la référence
                            # `.mscx` utilise une voix locale par portée.
                            voice = raw_voice - (staff - 1) * 4
                            if child.find("grace") is not None:
                                # Grâce : pas de durée, l'attaque est celle de
                                # la note principale. Un MIDI n'en a pas.
                                if graces is not None:
                                    graces.append((midi, onset))
                                if not is_chord_note:
                                    offset += dur
                                continue
                            tie_types = [tie.get("type") for tie in child.findall("tie")]
                            tie_stop = "stop" in tie_types
                            tie_start = "start" in tie_types
                            cle = (part_index, staff, raw_voice, midi)
                            if tie_stop and cle in open_ties:
                                # Continuation : la durée s'ajoute au premier
                                # fragment, sans nouvelle attaque.
                                open_ties[cle][2] += dur
                                if not tie_start:
                                    del open_ties[cle]
                            else:
                                note = [midi, onset, dur, voice]
                                raw.append(note)
                                if tie_start:
                                    open_ties[cle] = note
                    if not is_chord_note:
                        offset += dur
            tick += offset
    return midi_notes.fusionner_unissons(
        [(pitch, round(onset / GRID) * GRID, round(dur / GRID) * GRID, voice)
         for pitch, onset, dur, voice in raw]
    )


DUREES = {
    "whole": 4.0, "half": 2.0, "quarter": 1.0, "eighth": 0.5, "16th": 0.25,
    "32nd": 0.125, "64th": 0.0625, "128th": 0.03125, "measure": 4.0,
}


def mscx_notes(path: Path) -> list[tuple[int, float, float, int]]:
    """Notes d'un `.mscx` attendu, en noires, sans passer par notre exporteur.

    Le `.mscx` stocke la hauteur réelle : le convertir en MusicXML appliquerait
    la transposition de l'instrument et décalerait la référence d'une octave.

    Les liaisons sont fusionnées comme dans `notes()` : une note continuée
    porte un `<Tie>` dont `<startElement>` est l'`<eid>` de la note précédente
    de la chaîne. La durée de chaque continuation est reportée sur le premier
    fragment, et la continuation n'est pas comptée comme une attaque.
    """
    root = ET.parse(path).getroot()
    score_el = root.find("Score")
    if score_el is None:
        return []
    raw: list[list] = []  # [hauteur, attaque, durée, voix, eid, eid précédent]
    for staff in score_el.findall("Staff"):
        if not staff.findall("Measure"):
            continue  # déclaration de portée sous <Part>
        tick = 0.0
        current_sig = 4.0
        for measure in staff.findall("Measure"):
            for timesig in measure.findall(".//TimeSig"):
                num, den = timesig.findtext("sigN"), timesig.findtext("sigD")
                if num and den:
                    current_sig = float(num) / float(den) * 4.0
                    break
            mesure_len = current_sig
            len_txt = measure.get("len")
            if len_txt:
                num, _, den = len_txt.partition("/")
                mesure_len = float(num) / float(den or 1) * 4.0
            for voice_index, voice in enumerate(measure.findall("voice"), start=1):
                cursor = tick
                # Un <Tuplet> réduit la durée réelle des accords suivants :
                # `<normalNotes>2</normalNotes><actualNotes>3</actualNotes>`
                # sur des croches donne 2/3 de croche par note, pas 1/2.
                tuplet: tuple[float, float] | None = None  # (ratio, reste)
                for child in voice:
                    if child.tag == "location":
                        frac = child.findtext("fractions", "0/1")
                        num, _, den = frac.partition("/")
                        cursor += int(num) / int(den or 1)
                    elif child.tag == "Tuplet":
                        normal = int(child.findtext("normalNotes", "1"))
                        actual = int(child.findtext("actualNotes", "1"))
                        base = DUREES.get(child.findtext("baseNote", "eighth"), 0.5)
                        if normal > 0 and actual > 0:
                            tuplet = (normal / actual, normal * base)
                    elif child.tag in ("Chord", "Rest"):
                        duree = child.findtext("duration")
                        if duree:
                            num, _, den = duree.partition("/")
                            length = float(num) / float(den or 1) * 4.0
                        else:
                            length = DUREES.get(child.findtext("durationType", "quarter"), 1.0)
                            # `<dots>` change la durée réelle, pas seulement
                            # la notation : un croche pointé vaut 3/4 de
                            # noire. Les mesures composées (12/8) du lot amont
                            # en sont pleines.
                            dots = int(child.findtext("dots", "0") or 0)
                            if dots > 0:
                                length *= 2 - 2 ** (-dots)
                        if tuplet is not None:
                            ratio, remainder = tuplet
                            length *= ratio
                            remainder -= length
                            tuplet = (ratio, remainder) if remainder > GRID / 2 else None
                        if child.tag == "Chord":
                            for note in child.findall("Note"):
                                pitch = note.findtext("pitch")
                                if pitch is not None:
                                    tie = note.find("Tie")
                                    previous = (tie.findtext("startElement")
                                                if tie is not None else None)
                                    raw.append([int(pitch), cursor, length, voice_index,
                                                note.findtext("eid"), previous])
                        cursor += length
            tick += mesure_len

    by_eid = {note[4]: index for index, note in enumerate(raw) if note[4]}
    racine = list(range(len(raw)))

    def racine_de(index: int) -> int:
        while racine[index] != index:
            index = racine[index]
        return index

    continuations = set()
    for index, note in enumerate(raw):
        previous = note[5]
        if previous is None or previous not in by_eid:
            continue
        debut = racine_de(by_eid[previous])
        racine[index] = debut
        raw[debut][2] += note[2]
        continuations.add(index)
    return midi_notes.fusionner_unissons(
        [(note[0], round(note[1] / GRID) * GRID, round(note[2] / GRID) * GRID, note[3])
         for index, note in enumerate(raw) if index not in continuations]
    )


def score(ref: list[tuple], got: list[tuple]) -> float:
    """Part des notes de référence retrouvées (hauteur, attaque, durée, voix)."""
    if not ref:
        return 1.0
    remaining = list(got)
    found = 0
    for pitch, onset, dur, voice in ref:
        for index, candidate in enumerate(remaining):
            cp, co, cd, cv = candidate
            if cp == pitch and cv == voice and abs(co - onset) <= TOL and abs(cd - dur) <= TOL:
                remaining.pop(index)
                found += 1
                break
    return found / len(ref)


def run(binary: str, src: Path, dst: Path, extra: list[str] | None = None) -> int:
    """Lit ``src`` avec la CLI ; renvoie son code (3 = non prise en charge)."""
    cmd = ["timeout", str(LIMITE), "prlimit", "--as=2000000000",
           binary, str(src), "--to", "musicxml", "-o", str(dst)]
    cmd += extra or []
    return subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode


def upstream_test_options(tests_dir: Path) -> dict[str, list[str]]:
    """Options CLI reproduisant la configuration de chaque test amont.

    Les helpers de `midiimport_tests.cpp` verrouillent certaines valeurs :
    `dontSimplify` (V1, sans simplification), `staffSplit` (V1, sans
    simplification, séparation forcée), `voiceSeparation` (V4, sans
    simplification), `simplification` (V1, simplification active).
    """
    source = tests_dir / "midiimport_tests.cpp"
    if not source.exists():
        return {}
    texte = source.read_text(encoding="utf-8")
    result: dict[str, list[str]] = {}
    for helper, options in (
        ("dontSimplify", ["--midi-no-simplify", "--midi-voices", "1"]),
        ("staffSplit", ["--midi-no-simplify", "--midi-voices", "1"]),
        ("voiceSeparation", ["--midi-no-simplify"]),
        ("simplification", ["--midi-voices", "1"]),
    ):
        for nom in re.findall(rf'{helper}\(\s*"([^"]+)"\s*\)', texte):
            result[nom] = list(options)
    if "changeClef.setDefaultValue(false, false)" in texte:
        result["tuplet_3_5_7_tuplets"] = ["--midi-no-simplify"]
    for nom, options in staff_split_tests(tests_dir).items():
        result.setdefault(nom, []).append(
            "--midi-staff-split" if options else "--midi-no-staff-split"
        )
    # Certains tests posent leurs options **dans le corps du TEST_F**, hors des
    # helpers ci-dessus : `quantValue` (`Q_4`, `Q_32`…), `changeClef`,
    # `doStaffSplit`, `simplifyDurations`, `maxVoiceCount`
    # (`midiimport_tests.cpp:230-243, 349-379`). Sans elles, le test mesure une
    # autre configuration d'import que sa référence.
    for bloc_match in re.finditer(r"TEST_F\(\s*MidiImportTests\s*,\s*\w+\s*\)\s*\{", texte):
        debut = bloc_match.end()
        profondeur = 1
        index = debut
        while index < len(texte) and profondeur:
            if texte[index] == "{":
                profondeur += 1
            elif texte[index] == "}":
                profondeur -= 1
            index += 1
        bloc = texte[debut:index]
        nom = re.search(r'midiFile(?:Path)?\(\s*(?:u)?"([^"]+)"', bloc) or re.search(
            r'String\s+midiFile\s*\(\s*(?:u)?"([^"]+)"', bloc
        )
        if not nom:
            continue
        options = result.setdefault(nom.group(1), [])
        # Helpers appelés avec une **variable** (`dontSimplify(midiFile…)`) :
        # les options du helper s'ajoutent à celles du corps.
        if re.search(r"dontSimplify\(", bloc):
            options += ["--midi-no-simplify", "--midi-voices", "1", "--midi-no-staff-split"]
        elif re.search(r"simplification\(", bloc):
            options += ["--midi-no-staff-split", "--midi-voices", "1"]
        elif re.search(r"voiceSeparation\(", bloc):
            options += ["--midi-no-staff-split"]
        for quant in re.findall(r"QuantValue::Q_(\d+)", bloc):
            options += ["--midi-quant", quant]
        if re.search(r"changeClef\.setDefaultValue\(\s*false", bloc):
            options.append("--midi-no-change-clef")
        if re.search(r"doStaffSplit\.setDefaultValue\(\s*false", bloc):
            options.append("--midi-no-staff-split")
        if re.search(r"doStaffSplit\.setDefaultValue\(\s*true", bloc):
            options.append("--midi-staff-split")
        if re.search(r"simplifyDurations\.setDefaultValue\(\s*false", bloc):
            options.append("--midi-no-simplify")
        for voix in re.findall(
            r"maxVoiceCount\.setDefaultValue\(\s*MidiOperations::VoiceCount::V_(\d)", bloc
        ):
            options += ["--midi-voices", voix]
    return result


def dont_simplify_tests(tests_dir: Path) -> set[str]:
    """Tests amont compilés avec ``dontSimplify`` (``midiimport_tests.cpp``).

    Le helper ``dontSimplify`` de MuseScore pose ``simplifyDurations = false``,
    ``maxVoiceCount = V_1``, ``doStaffSplit = false`` et
    ``showTempoText = false`` (``midiimport_tests.cpp:79-92``) : la référence
    ``.mscx`` attendue décrit donc une configuration d'import différente de
    celle de la CLI. On rejoue ces tests avec les mêmes options, sans quoi leur
    score mesure un choix d'import, pas une fidélité.
    """
    source = tests_dir / "midiimport_tests.cpp"
    if not source.exists():
        return set()
    return set(re.findall(r'dontSimplify\(\s*"([^"]+)"\s*\)', source.read_text(encoding="utf-8")))


def staff_split_tests(tests_dir: Path) -> dict[str, bool]:
    """Tests amont qui **verrouillent** `doStaffSplit` (`canRedefine=false`).

    `voiceSeparation`, `dontSimplify` et `simplification` posent la valeur
    `false` et interdisent à `setLeftRightHandSplit` de la redéfinir ;
    `staffSplit` pose `true`. La valeur verrouillée s'applique alors à toutes
    les pistes (`midiimport.cpp:1076-1099`).
    """
    source = tests_dir / "midiimport_tests.cpp"
    if not source.exists():
        return {}
    texte = source.read_text(encoding="utf-8")
    result: dict[str, bool] = {}
    for helper, value in (("voiceSeparation", False), ("dontSimplify", False),
                          ("simplification", False), ("staffSplit", True)):
        for nom in re.findall(rf'{helper}\(\s*"([^"]+)"\s*\)', texte):
            result[nom] = value
    return result


def paires_upstream(data: Path) -> list[tuple[str, Path, Path]]:
    return [(f"up-{p.stem}", p, p.with_name(p.stem + "-ref.mscx"))
            for p in sorted(data.glob("*.mid")) if p.with_name(p.stem + "-ref.mscx").exists()]


def identifiants_lilypond(chemins: list[Path]) -> list[tuple[str, Path]]:
    """Associe un identifiant **unique** à chaque MIDI LilyPond.

    Deux partitions de même nom vivent dans des dossiers différents
    (`SanctePieX` grégorien et `SanctePieX` des cantiques, trois `AStMichel`).
    Un identifiant fondé sur le seul nom de fichier faisait écrire les deux
    conversions et la même référence MuseScore au même endroit : la seconde
    écrasait la première, et les deux lignes étaient comparées au même
    fichier. La première occurrence garde l'identifiant historique (dont la
    référence existe déjà), les suivantes reçoivent `-2`, `-3`… dans l'ordre
    trié des chemins, donc de façon déterministe.
    """
    vus: dict[str, int] = {}
    out = []
    for mid in chemins:
        ident = f"ly-{mid.stem}"
        if ident in vus:
            vus[ident] += 1
            ident = f"{ident}-{vus[ident]}"
        else:
            vus[ident] = 1
        out.append((ident, mid))
    return out


def a_des_repetitions(ly: Path) -> bool:
    """Vrai si la source `.ly` (et ses inclusions directes) contient une reprise.

    Le MIDI de LilyPond est **déplié**, la partition écrite non : comparer les
    notes d'une source à reprise compterait chaque passage repris comme des
    notes supplémentaires. `ly_notes.a_des_repetitions` porte la règle, comme
    pour l'oracle LilyPond ; on lui concatène les inclusions directes du
    dossier (meilleur effort, sans résolution LilyPond).
    """
    try:
        texte = ly.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return False
    for nom in re.findall(r'\\include\s+"([^"]+)"', texte):
        inclus = ly.parent / nom
        if inclus.exists():
            try:
                texte += inclus.read_text(encoding="utf-8", errors="ignore")
            except OSError:
                pass
    return ly_notes.a_des_repetitions(texte)


def paires_lilypond() -> list[tuple[str, Path, Path]]:
    """Les partitions LilyPond écrivent `.midi`, pas `.mid`.

    La référence est le `.ly` lu par **notre propre lecteur LilyPond** (98 % de
    notes exact contre le MIDI produit par LilyPond), pas un parseur Python :
    les deux côtés passent par la CLI, donc on compare modèle à modèle.
    """
    chemins = [mid for mid in sorted(Path("/home/jacques/Musique").rglob("*.midi"))
               if mid.with_suffix(".ly").exists()]
    return [(ident, mid, mid.with_suffix(".ly"))
            for ident, mid in identifiants_lilypond(chemins)]


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--binary", required=True)
    ap.add_argument("--out", required=True, help="dossier des sorties (hors dépôt)")
    ap.add_argument("--upstream", default="")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument(
        "--extra-arg",
        action="append",
        default=[],
        help="argument ajouté à chaque conversion scoretk (reproductible)",
    )
    ap.add_argument(
        "--ly-arg",
        action="append",
        default=[],
        help=(
            "argument ajouté seulement aux conversions dont la référence est "
            "un .ly (les notes de la source ne sont pas simplifiées : "
            "--midi-no-simplify compare la notation écrite)"
        ),
    )
    args = ap.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    paires = []
    test_options: dict[str, list[str]] = {}
    if args.upstream:
        upstream = Path(args.upstream)
        paires += paires_upstream(upstream)
        test_options = upstream_test_options(upstream.parent)
    paires += paires_lilypond()
    if args.limit:
        paires = paires[:args.limit]
    travail = out / "travail"
    travail.mkdir(exist_ok=True)
    with (out / "import.jsonl").open("w", encoding="utf-8") as fic:
        for ident, mid, ref in paires:
            d = travail / ident
            d.mkdir(exist_ok=True)
            got_xml, ref_xml = d / "got.musicxml", d / "ref.musicxml"
            row = {"id": ident, "mid": str(mid), "ref": str(ref), "score": 0.0}
            exclu = False
            if ref.suffix == ".ly" and a_des_repetitions(ref):
                # MIDI déplié, source écrite une fois : hors comparaison note
                # à note (comme N2 de l'oracle LilyPond).
                common.marquer_exclu(row, str(ref))
                row["raison_exclusion"] = "reprise"
                exclu = True
            graces: list[tuple[int, float]] = []
            if ref.suffix == ".mscx":
                try:
                    a = mscx_notes(ref)
                except Exception as exc:
                    a = []
                    row["erreur"] = f"reference : {exc}"[:200]
            else:
                code = run(args.binary, ref, ref_xml)
                if common.est_non_pris_en_charge(code):
                    common.marquer_exclu(row, str(ref))
                    a, exclu = [], True
                elif code != 0:
                    a = []
                    row["erreur"] = "conversion reference"
                else:
                    try:
                        a = notes(ref_xml, to_concert=True, graces=graces)
                    except Exception as exc:
                        a = []
                        row["erreur"] = f"reference : {exc}"[:200]
            if not exclu:
                extra = list(args.extra_arg)
                if ref.suffix == ".ly":
                    extra += args.ly_arg
                nom = ident[3:] if ident.startswith("up-") else ""
                if nom in test_options:
                    # Un test peut rejouer une option déjà passée par
                    # `--extra-arg` (`--midi-no-simplify`) : le CLI refuse le
                    # doublon, on déduplique en gardant l'ordre.
                    extra = list(dict.fromkeys(extra + test_options[nom]))
                extra = extra or None
                code_mid = run(args.binary, mid, got_xml, extra)
                if common.est_non_pris_en_charge(code_mid):
                    common.marquer_exclu(row, str(mid))
                elif code_mid == 0:
                    try:
                        b = notes(got_xml, to_concert=True)
                        releve = midi_notes.releve_import(
                            a, b,
                            avec_voix=ref.suffix == ".mscx",
                            avec_duree=ref.suffix == ".mscx",
                            graces=graces,
                        )
                        row.update(
                            ref_notes=len(a),
                            got_notes=len(b),
                            score=round(releve["n2v"] / len(a), 4) if a else 1.0,
                            n1=releve["n1"],
                            n2=releve["n2"],
                            n2d=releve["n2d"],
                            n2v=releve["n2v"],
                            exact=releve["exact"],
                            tick=releve["tick"],
                            cause=releve["cause"],
                        )
                    except Exception as exc:
                        row["erreur"] = str(exc)[:200]
                else:
                    row["erreur"] = "conversion"
            fic.write(json.dumps(row, ensure_ascii=False) + "\n")
            fic.flush()
    rows = [json.loads(l) for l in (out / "import.jsonl").read_text().splitlines()]
    exclus = common.noms_exclus(rows)
    # Une entrée non prise en charge sort du numérateur et du dénominateur.
    rows = common.filtrer(rows)
    ok = [r for r in rows if "erreur" not in r]
    total = sum(r.get("ref_notes", 0) for r in ok)
    found = sum(r.get("n2v", r.get("ref_notes", 0) * r["score"]) for r in ok)
    exact = [r for r in ok if r.get("exact")]
    causes: dict[str, list[dict]] = {}
    for r in ok:
        if r.get("exact"):
            continue
        causes.setdefault(r.get("cause", "?"), []).append(r)
    resume = [f"# MIDI import oracle — {len(ok)}/{len(rows)} files",
              f"# Exact (pitch, onset, duration, voice): {len(exact)}/{len(ok)}"
              f" ({100.0*len(exact)/len(ok):.1f} %)" if ok else "",
              f"# N (pitch+onset+duration+voice, Q{int(1/GRID)}, tol {TOL}): "
              f"{found/total:.4f}" if total else "",
              f"# {common.ligne_resume(exclus)}",
              "", "| Première divergence | Fichiers | Exemple |", "| --- | ---: | --- |"]
    for cause, fichiers in sorted(causes.items(), key=lambda item: (-len(item[1]), item[0])):
        resume.append(f"| {cause} | {len(fichiers)} | `{fichiers[0]['id']}` |")
    resume += ["", "| File | Ref notes | Got notes | N2d | Cause |", "| --- | ---: | ---: | ---: | --- |"]
    for r in sorted(rows, key=lambda r: (r.get("exact", False), r["score"])):
        resume.append(f"| {r['id']} | {r.get('ref_notes', 0)} | {r.get('got_notes', 0)} | "
                      f"{r.get('n2d', 0)} | {r.get('cause', '')} |")
    (out / "resume.md").write_text("\n".join(resume) + "\n", encoding="utf-8")
    common.avertir(exclus)
    print("\n".join(l for l in resume if l.startswith("#")))
    for cause, fichiers in sorted(causes.items(), key=lambda item: (-len(item[1]), item[0])):
        print(f"| {cause} | {len(fichiers)} | `{fichiers[0]['id']}` |")


if __name__ == "__main__":
    main()
