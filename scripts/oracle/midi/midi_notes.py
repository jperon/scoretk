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

"""Extracteur et comparateur de notes pour l'oracle MIDI.

Aucune dépendance à scoretk : `mido` lit les fichiers SMF et
`xml.etree.ElementTree` (bibliothèque standard) lit le MusicXML. Une note est
un quadruplet ``(hauteur, attaque, durée, piste)`` où attaque et durée sont
exprimées en **noires** (quarters).

Deux points de comparaison :

* N1 : multiset des hauteurs (comme l'oracle historique ``xmlcmp/cmp.py``) ;
* N2 : multiset des couples (hauteur, attaque quantifiée), avec une
  tolérance documentée de ±1/16 de noire ; N2d ajoute la durée.

La grille de quantification par défaut est 1/16 de noire (0,25), qui est la
valeur par défaut de l'import MIDI de MuseScore (Q16, double croche).
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict, deque
import json
import xml.etree.ElementTree as ET

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import common  # noqa: E402

GRID = 0.25  # pas de quantification : une double croche = 1/16 de noire
TOL = 0.25  # tolérance de rapprochement : ±1/16 de noire
STEPS = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


def quant(x: float, grid: float = GRID) -> float:
    """Ramène une position à la grille de quantification."""
    return round(x / grid) * grid


def read_midi(path: str) -> list[tuple[int, float, float, int]]:
    """Notes d'un SMF : hauteur, attaque (noires), durée (noires), piste.

    Les note-off et les note-on à vélocité nulle ferment l'attaque
    correspondante. Les hauteurs sont absolues (doigté MIDI), sans
    interprétation de batterie : le comparateur est le même dans les deux sens.

    Une file par couple (canal, hauteur), et non une seule attache : le MIDI
    autorise plusieurs notes simultanées de même hauteur (§Channel Voice de
    `docs/agents/format-ref/midi.md`, messages `0x8`/`0x9` appariés) — par
    exemple une appoggiature qui chevauche la note précédente ou deux voix
    d'une même partie. Le lecteur de MuseScore suit la même règle : dans
    `midishared/midifile.cpp`, chaque note-on cherche le premier note-off
    encore libre de cette hauteur et le marque consommé (FIFO). Une table
    simple écrasait l'attaque précédente et perdait la note la plus ancienne.

    Les **pistes sont fusionnées par canal** avant l'appariement, dans l'ordre
    du fichier (tick, piste, position). Un synthétiseur ne connaît que le
    canal : c'est aussi ce que fait l'export MuseScore, dont
    `EventsHolder::fixupMIDI` (`engraving/compat/midi/event.cpp:263-306`)
    répartit les attaques d'unisson sur la piste de la dernière attaque et
    `exportmidi.cpp:304-323` écrit l'extinction sur la piste de l'attaque
    précédente. Lire piste par piste laissait alors la note de la piste
    émettrice ouverte jusqu'à une extinction lointaine d'une autre note, d'où
    des durées de plusieurs mesures ; le fichier doit être lu comme le jeu le
    fait. L'ordre des messages au même tick est celui du fichier (les
    extinctions de restrike sont écrites après les attaques, `exportmidi.cpp`
    les insère dans le multimap).
    """
    mid = common.lire_midi(path)
    tpb = mid.ticks_per_beat or 480
    flux: list[tuple[int, int, int, bool, int, int]] = []
    for index, track in enumerate(mid.tracks):
        tick = 0
        for position, msg in enumerate(track):
            tick += msg.time
            if not msg.is_meta and hasattr(msg, "note"):
                extinction = msg.type == "note_off" or (
                    msg.type == "note_on" and msg.velocity == 0
                )
                flux.append((tick, index, position, extinction, msg.note, msg.channel))
    flux.sort(key=lambda evenement: (evenement[0], evenement[1], evenement[2]))
    ouvertes: dict[tuple[int, int], deque[int]] = defaultdict(deque)
    notes: list[tuple[int, float, float, int]] = []
    for tick, index, _position, extinction, hauteur, canal in flux:
        cle = (canal, hauteur)
        if extinction:
            file_attentes = ouvertes.get(cle)
            if file_attentes:
                debut = file_attentes.popleft()
                notes.append((hauteur, debut / tpb, (tick - debut) / tpb, index))
        else:
            ouvertes[cle].append(tick)
    return notes


def normaliser_ecriture(notes: list[tuple]) -> list[tuple]:
    """Notes d'une écriture, ramenées à ce qui s'entend réellement.

    Deux conventions d'écriture rendent deux MIDI musicalement égaux sans
    être identiques note à note ; la comparaison d'écriture les ramène donc à
    la même forme, symétriquement des deux côtés :

    - **unissons simultanés** : même hauteur et même attaque. MuseScore les
      fusionne par canal interne (`EventsHolder::fixupMIDI`,
      `engraving/compat/midi/event.cpp:263-306`) et écrit l'attaque écartée
      comme un restrike de durée nulle (`exportmidi.cpp:304-323`) ; LilyPond
      les fusionne aussi quand ils tombent sur le même tick d'une même piste,
      en gardant la plus longue extinction (`lily/midi-walker.cc:94-115`,
      propriété `midiMergeUnisons`, `scm/define-context-properties.scm:555`).
      Deux attaques de même hauteur au même instant s'entendent comme une
      seule : on garde la durée la plus longue, comme `fusionner_unissons`
      pour l'import.
    - **durée nulle** : un restrike de MuseScore est inaudible et n'a pas de
      contrepartie chez LilyPond. Une note de durée nulle est écartée.

    La fusion ne touche ni l'attaque ni la hauteur : une note réellement
    différente (autre attaque, autre hauteur, autre durée au-delà de tolérance)
    reste comptée comme un écart.
    """
    index: dict[tuple[int, float], int] = {}
    out: list[tuple] = []
    for note in sorted(notes, key=lambda n: (n[1], n[3])):
        if note[2] <= 0:
            continue
        cle = (note[0], note[1])
        position = index.get(cle)
        if position is None:
            index[cle] = len(out)
            out.append(note)
        elif note[2] > out[position][2]:
            out[position] = note
    return out


def read_xml(path: str) -> list[tuple[int, float, float, int]]:
    """Notes d'un MusicXML partwise : hauteur concert, attaque, durée, part.

    L'attaque est calculée comme dans un vrai lecteur MusicXML : un curseur
    parcourt chaque mesure depuis son début, ``<backup>``/``<forward>`` le
    déplacent, et les accords (``<chord/>``) partagent l'attaque de la note
    précédente sans avancer le curseur. Le début de la mesure suivante est la
    fin de la précédente. L'ancienne version additionnait les durées par
    (voix, portée) sur tout le morceau : une voix qui n'apparaissait qu'en
    cours de route voyait toutes ses attaques décalées vers le début (les
    « attaques décalées » du relevé, en fait un défaut de l'oracle, pas de
    l'import).

    Une note tenue sur plusieurs mesures est écrite comme une chaîne de notes
    liées (``<tie type="start"/>`` … ``<tie type="stop"/>``), une par fragment
    de mesure. Le MIDI de référence ne connaît qu'une seule attaque couvrant
    toute la tenue : l'oracle doit donc fusionner chaque chaîne en UNE note
    dont la durée est la somme des fragments, avec l'attaque du premier. Les
    chaînes sont distinguées par (part, portée, voix, hauteur), car deux voix
    peuvent tenir la même hauteur en parallèle. Les notes de grâce ne sont pas
    des attaques et sont écartées. Ce lecteur est commun à scoretk et à
    MuseScore, donc la correction s'applique équitablement aux deux sorties.
    """
    racine = ET.parse(path).getroot()
    # Listes mutables : une chaîne de liaison agrandit la durée de son
    # premier fragment au fur et à mesure que ses continuations sont lues.
    notes: list[list] = []
    ties_ouvertes: dict[tuple, list] = {}
    for numero_part, part in enumerate(racine.findall("part")):
        divisions = 1
        transposition = 0
        temps_part = 0.0
        for measure in part.findall("measure"):
            curseur = temps_part
            fin_mesure = temps_part
            derniere_attaque = temps_part
            a_des_notes = False
            for elem in measure:
                if elem.tag == "attributes":
                    d = elem.find("divisions")
                    if d is not None and d.text:
                        divisions = int(float(d.text))
                    t = elem.find("transpose")
                    if t is not None:
                        chromatique = t.findtext("chromatic", "0")
                        octave = t.findtext("octave-change", "0")
                        transposition = int(float(chromatique)) + 12 * int(float(octave))
                    continue
                if elem.tag in ("backup", "forward"):
                    duree_texte = elem.findtext("duration")
                    delta = int(float(duree_texte)) / divisions if duree_texte else 0.0
                    curseur += -delta if elem.tag == "backup" else delta
                    continue
                if elem.tag != "note":
                    continue
                duree_texte = elem.findtext("duration")
                duree = int(float(duree_texte)) / divisions if duree_texte else 0.0
                est_accord = elem.find("chord") is not None
                if est_accord:
                    attaque = derniere_attaque
                else:
                    attaque = curseur
                    derniere_attaque = attaque
                    curseur += duree
                a_des_notes = True
                fin_mesure = max(fin_mesure, attaque + duree)
                if elem.find("grace") is not None:
                    continue
                pitch = elem.find("pitch")
                if pitch is None:
                    continue  # silence : il a déjà avancé le curseur
                step = pitch.findtext("step")
                octave_txt = pitch.findtext("octave")
                if step not in STEPS or octave_txt is None:
                    continue
                alter = pitch.findtext("alter", "0")
                hauteur = 12 * (int(octave_txt) + 1) + STEPS[step] + int(float(alter)) + transposition
                types_tie = [t.get("type") for t in elem.findall("tie")]
                tie_stop = "stop" in types_tie
                tie_start = "start" in types_tie
                voix = elem.findtext("voice", "1")
                portee = elem.findtext("staff", "1")
                cle = (numero_part, portee, voix, hauteur)
                if tie_stop and cle in ties_ouvertes:
                    # Continuation d'une chaîne : on ajoute la durée du
                    # fragment au premier fragment, sans nouvelle attaque.
                    ouverte = ties_ouvertes[cle]
                    ouverte[2] += duree
                    fin_mesure = max(fin_mesure, ouverte[1] + ouverte[2])
                    if not tie_start:  # fin de chaîne
                        del ties_ouvertes[cle]
                    continue
                note = [hauteur, attaque, duree, numero_part]
                notes.append(note)
                if tie_start:
                    ties_ouvertes[cle] = note
            if not a_des_notes:
                # Mesure vide : on avance au moins de sa durée nominale pour
                # ne pas écraser la suivante sur le même temps.
                temps_part = max(fin_mesure, temps_part + 4.0)
            else:
                temps_part = fin_mesure
    return [tuple(note) for note in notes]


def _rapprocher(
    reference: list[tuple], test: list[tuple], tol: float, avec_duree: bool
) -> int:
    """Appariement glouton des notes de même hauteur à attaque proche.

    Les listes sont triées par attaque : chaque note de référence prend la
    première note libre dont l'attaque est à moins de ``tol`` (et la durée
    également si demandé). Les fichiers mesurés font au plus quelques milliers
    de notes, ce qui rend le balayage linéaire suffisant.
    """
    ref_par_h: dict[int, list] = defaultdict(list)
    test_par_h: dict[int, list] = defaultdict(list)
    for note in reference:
        ref_par_h[note[0]].append(note)
    for note in test:
        test_par_h[note[0]].append(note)
    total = 0
    for hauteur, refs in ref_par_h.items():
        candidats = sorted(test_par_h.get(hauteur, []), key=lambda n: n[1])
        if not candidats:
            continue
        refs.sort(key=lambda n: n[1])
        pris = [False] * len(candidats)
        for ref in refs:
            for i, cand in enumerate(candidats):
                if pris[i] or abs(ref[1] - cand[1]) > tol:
                    continue
                if avec_duree and abs(ref[2] - cand[2]) > tol:
                    continue
                pris[i] = True
                total += 1
                break
    return total


def fusionner_unissons(notes: list[tuple]) -> list[tuple]:
    """Ramène deux notes de même (hauteur, attaque) à une seule attaque.

    Un MIDI ne permet pas de noter deux fois la même hauteur au même instant
    dans un accord : `MChord::collectChords` fusionne les notes proches et
    `hasNotesWithEqualPitch` **écarte** la seconde
    (`importmidi_chord.cpp:391-465`). MuseScore perd donc les unissons de
    deux voix, et un import fidèle aussi. Comparer un source `.ly` à deux
    voix doublées sans cette normalisation compterait chaque doublure comme
    une note manquante (l'oracle LilyPond applique déjà la même règle :
    `test_same_tick.py`, « unisson = une attaque MIDI »).

    La fusion garde la **durée la plus longue** : `removeOverlappingNotes`
    (`MChord::removeOverlappingNotes(QList<MidiNote>&)`) reporte le plus grand
    `offTime` sur la note conservée avant d'effacer l'autre. La voix retenue
    est celle de la première occurrence dans l'ordre (attaque, voix).
    """
    index: dict[tuple[int, float], int] = {}
    out: list[tuple] = []
    for note in sorted(notes, key=lambda n: (n[1], n[3])):
        cle = (note[0], note[1])
        position = index.get(cle)
        if position is None:
            index[cle] = len(out)
            out.append(note)
        elif note[2] > out[position][2]:
            out[position] = (note[0], note[1], note[2]) + tuple(out[position][3:])
    return out


def apparier_import(
    reference: list[tuple], test: list[tuple], champs: frozenset[int], tol: float = TOL
) -> tuple[list[tuple[int, int]], list[int], list[int]]:
    """Appariement glouton de notes d'import, champ par champ.

    ``champs`` énumère les indices comparés en plus de la hauteur : 1 attaque,
    2 durée, 3 voix. Les notes sont triées par attaque ; chaque note de
    référence prend la première note libre de même hauteur, à moins de ``tol``
    d'attaque, dont les autres champs demandés sont égaux. Renvoie les paires
    d'indices et les restes de chaque côté.
    """
    restes_test = sorted(range(len(test)), key=lambda i: test[i][1])
    restes_ref: list[int] = []
    paires: list[tuple[int, int]] = []
    pris: set[int] = set()
    for i in sorted(range(len(reference)), key=lambda i: reference[i][1]):
        note = reference[i]
        trouve = None
        for j in restes_test:
            if j in pris or test[j][0] != note[0]:
                continue
            candidat = test[j]
            if abs(candidat[1] - note[1]) > tol:
                continue
            if 2 in champs and abs(candidat[2] - note[2]) > tol:
                continue
            if 3 in champs and candidat[3] != note[3]:
                continue
            trouve = j
            break
        if trouve is None:
            restes_ref.append(i)
        else:
            pris.add(trouve)
            paires.append((i, trouve))
    return paires, restes_ref, [j for j in range(len(test)) if j not in pris]


def premiere_divergence_import(
    reference: list[tuple],
    test: list[tuple],
    tol: float = TOL,
    avec_voix: bool = True,
    avec_duree: bool = True,
) -> tuple[float, str]:
    """Première divergence (attaque, cause) entre référence et import.

    L'appariement le plus strict (hauteur, attaque, durée, voix) laisse des
    restes ; on prend le plus tôt et on cherche la différence la plus fine
    avec une note libre de l'autre côté, pour regrouper les fichiers par
    cause : ``voix`` (même note, autre voix), ``durée``, ``octave``,
    ``attaque``, ``note manquante`` ou ``note sup``.
    """
    champs = {1}
    if avec_duree:
        champs.add(2)
    if avec_voix:
        champs.add(3)
    _, restes_ref, restes_test = apparier_import(reference, test, frozenset(champs), tol)
    evenements: list[tuple[float, int, int]] = []
    for i in restes_ref:
        evenements.append((reference[i][1], 0, i))
    for j in restes_test:
        evenements.append((test[j][1], 1, j))
    if not evenements:
        return 0.0, "aucune"
    evenements.sort()
    _, cote, index = evenements[0]
    if cote == 1:
        return test[index][1], "note sup"
    note = reference[index]
    # Même hauteur et même attaque : durée ou voix.
    for cand in test:
        if cand[0] != note[0] or abs(cand[1] - note[1]) > tol:
            continue
        if avec_duree and abs(cand[2] - note[2]) > tol:
            return note[1], "durée"
        if avec_voix and cand[3] != note[3]:
            return note[1], "voix"
    # Même attaque, hauteur à l'octave près.
    for cand in test:
        if abs(cand[1] - note[1]) > tol:
            continue
        if abs(cand[0] - note[0]) in (12, 24):
            return note[1], "octave"
    # Même hauteur, attaque déplacée (dans la noire) : note décalée.
    for cand in test:
        if cand[0] == note[0] and tol < abs(cand[1] - note[1]) <= 4.0:
            return note[1], "attaque"
    return note[1], "note manquante"


def releve_import(
    reference: list[tuple],
    test: list[tuple],
    tol: float = TOL,
    avec_voix: bool = True,
    avec_duree: bool = True,
    graces: list[tuple[int, float]] | None = None,
) -> dict:
    """Relevé d'import : N1, N2, N2d, N2v, exactitude et première divergence.

    ``avec_voix`` compare aussi la voix. Elle n'a de sens que face à une
    référence d'import MuseScore (`.mscx`) : un MIDI ne porte aucune voix, et
    les « notes de la source » d'un `.ly` n'en fixent pas non plus — l'import
    déduit les voix des chevauchements (`importmidi_voice.cpp`), pas de la
    partition d'origine.

    ``avec_duree`` compare la durée. Face à un `.ly`, la durée écrite n'est pas
    une vérité du MIDI : LilyPond écrit des note-off de *jeu* (gate time,
    articulations, mesures irrégulières), et deux voix doublées sur la même
    hauteur y sont indiscernables (`MChord::removeOverlappingNotes`). La
    comparaison « notes de la source » porte donc sur hauteur + attaque ;
    c'est le lot `.mscx` (import de MuseScore lui-même) qui juge les durées et
    les voix.
    """
    r = len(reference)
    t = len(test)
    cr = Counter(n[0] for n in reference)
    ct = Counter(n[0] for n in test)
    n1 = sum((cr & ct).values())
    n2 = r - len(apparier_import(reference, test, frozenset({1}), tol)[1])
    n2d = r - len(apparier_import(reference, test, frozenset({1, 2}), tol)[1])
    champs = {1}
    if avec_duree:
        champs.add(2)
    if avec_voix:
        champs.add(3)
    _, restes_ref, restes_test = apparier_import(reference, test, frozenset(champs), tol)
    n2v = r - len(restes_ref)
    if graces:
        # Les grâces ne sont appariées qu'aux notes **non appariées** de
        # l'import : une grâce de même hauteur qu'une note principale ne doit
        # pas lui voler son appariement.
        libres = list(restes_test)
        consommees = 0
        for pitch, onset in graces:
            meilleur = None
            for index in libres:
                candidat = test[index]
                if candidat[0] != pitch:
                    continue
                if not (onset - 1.0 - tol <= candidat[1] <= onset + tol):
                    continue
                distance = abs(candidat[1] - onset)
                if meilleur is None or distance < meilleur[0]:
                    meilleur = (distance, index)
            if meilleur is not None:
                libres.remove(meilleur[1])
                consommees += 1
        restes_test = libres
        t -= consommees
    tick, cause = premiere_divergence_import(reference, test, tol, avec_voix, avec_duree)
    return {
        "ref": r,
        "test": t,
        "n1": n1,
        "n2": n2,
        "n2d": n2d,
        "n2v": n2v,
        "exact": bool(r and r == t == n1 == n2v),
        "tick": tick,
        "cause": cause,
        "restes_ref": len(restes_ref),
        "restes_test": len(restes_test),
    }


def compare(
    reference: list[tuple], test: list[tuple], grid: float = GRID, tol: float = TOL
) -> dict:
    """Compare deux listes de notes et renvoie un relevé sérialisable."""
    r = len(reference)
    t = len(test)
    cr = Counter(n[0] for n in reference)
    ct = Counter(n[0] for n in test)
    n1 = sum((cr & ct).values())
    n2 = _rapprocher(reference, test, tol, False)
    n2d = _rapprocher(reference, test, tol, True)

    # Erreurs d'octave : hauteur de référence absente mais présente à ±12/±24.
    octave = 0
    for hauteur, count in (cr - ct).items():
        for decalage in (12, -12, 24, -24):
            presents = ct.get(hauteur + decalage, 0)
            if presents:
                octave += min(count, presents)
                break

    # Transposition : décalage constant qui maximise le recouvrement des
    # hauteurs (les listes ne sont pas alignées, on travaille sur les multisets).
    transposition = 0
    if n1 < r:
        meilleur = n1
        for decalage in range(-24, 25):
            if decalage == 0:
                continue
            commun = sum(min(c, ct.get(h + decalage, 0)) for h, c in cr.items())
            if commun > meilleur:
                meilleur, transposition = commun, decalage

    resultat = {
        "ref": r,
        "test": t,
        "n1": n1,
        "n2": n2,
        "n2d": n2d,
        "octave": octave,
        "transposition": transposition,
    }
    resultat["pct_n1"] = round(100.0 * n1 / r, 2) if r else 0.0
    resultat["pct_n2"] = round(100.0 * n2 / r, 2) if r else 0.0
    resultat["pct_n2d"] = round(100.0 * n2d / r, 2) if r else 0.0
    resultat["exact"] = bool(r and r == t == n1 == n2 == n2d)
    resultat["drapeaux"] = drapeaux(resultat)
    return resultat


def drapeaux(releve: dict) -> list[str]:
    """Cause primaire des écarts, pour regrouper les fichiers par origine."""
    d: list[str] = []
    r, t = releve["ref"], releve["test"]
    if releve.get("echec"):
        return ["echec"]
    if releve.get("timeout"):
        return ["delai"]
    if r and not t:
        return ["sortie_vide"]
    if releve["exact"]:
        return []
    if releve["octave"]:
        d.append("octave")
    if releve["transposition"] and releve["pct_n1"] < 90:
        d.append("transposition")
    if releve["n1"] == r and releve["n2"] < r:
        d.append("attaque_decalee")
    if t < r:
        d.append("notes_manquantes")
    elif t > r:
        d.append("notes_supp")
    if releve["n2"] == r == t and releve["n2d"] < r:
        d.append("duree_seule")
    return d or ["autre"]


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("action", choices=["midi", "xml", "compare"])
    p.add_argument("fichiers", nargs="*")
    p.add_argument("--grid", type=float, default=GRID)
    p.add_argument("--tol", type=float, default=TOL)
    args = p.parse_args()
    if args.action == "compare":
        ref = read_midi(args.fichiers[0])
        test = read_midi(args.fichiers[1])
        print(json.dumps(compare(ref, test, args.grid, args.tol)))
    else:
        lire = read_midi if args.action == "midi" else read_xml
        for f in args.fichiers:
            for note in lire(f):
                print(f"{note[0]}\t{note[1]:.4f}\t{note[2]:.4f}\t{note[3]}")


if __name__ == "__main__":
    main()
