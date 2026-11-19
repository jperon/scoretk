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

import mido

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
    """
    mid = mido.MidiFile(path)
    tpb = mid.ticks_per_beat or 480
    notes: list[tuple[int, float, float, int]] = []
    for index, track in enumerate(mid.tracks):
        tick = 0
        ouvertes: dict[tuple[int, int], deque[int]] = defaultdict(deque)
        for msg in track:
            tick += msg.time
            if msg.is_meta:
                continue
            cle = (msg.channel, msg.note) if hasattr(msg, "note") else None
            if msg.type == "note_on" and msg.velocity > 0:
                ouvertes[cle].append(tick)
            elif msg.type == "note_off" or (msg.type == "note_on" and msg.velocity == 0):
                file_attentes = ouvertes.get(cle)
                if file_attentes:
                    debut = file_attentes.popleft()
                    notes.append((msg.note, debut / tpb, (tick - debut) / tpb, index))
    return notes


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
