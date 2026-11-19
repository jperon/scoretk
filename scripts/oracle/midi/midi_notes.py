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
from collections import Counter, defaultdict
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
    """
    mid = mido.MidiFile(path)
    tpb = mid.ticks_per_beat or 480
    notes: list[tuple[int, float, float, int]] = []
    for index, track in enumerate(mid.tracks):
        tick = 0
        ouvertes: dict[tuple[int, int], int] = {}
        for msg in track:
            tick += msg.time
            if msg.is_meta:
                continue
            cle = (msg.channel, msg.note) if hasattr(msg, "note") else None
            if msg.type == "note_on" and msg.velocity > 0:
                ouvertes[cle] = tick
            elif msg.type == "note_off" or (msg.type == "note_on" and msg.velocity == 0):
                debut = ouvertes.pop(cle, None)
                if debut is not None:
                    notes.append((msg.note, debut / tpb, (tick - debut) / tpb, index))
    return notes


def read_xml(path: str) -> list[tuple[int, float, float, int]]:
    """Notes d'un MusicXML partwise : hauteur concert, attaque, durée, part.

    Le compteur d'attaques est propre à chaque (voix, portée) et repart de
    zéro pour chaque partie, comme le temps musical du fichier. Les accords
    (``<chord/>``) partagent l'attaque de la note précédente ; les silences
    avancent le compteur ; les notes de grâce et les prolongations de liaison
    (``<tie type="stop"/>``) ne sont pas des attaques et sont écartées.
    """
    racine = ET.parse(path).getroot()
    notes: list[tuple[int, float, float, int]] = []
    for numero_part, part in enumerate(racine.findall("part")):
        divisions = 1
        transposition = 0
        position: dict[tuple[str, str], float] = defaultdict(float)
        derniere_attaque: dict[tuple[str, str], float] = {}
        for measure in part.findall("measure"):
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
                if elem.tag != "note":
                    continue
                duree_texte = elem.findtext("duration")
                duree = int(float(duree_texte)) / divisions if duree_texte else 0.0
                voix = elem.findtext("voice") or "1"
                portee = elem.findtext("staff") or "1"
                cle = (voix, portee)
                est_accord = elem.find("chord") is not None
                attaque = derniere_attaque.get(cle, 0.0) if est_accord else position[cle]
                if not est_accord:
                    derniere_attaque[cle] = attaque
                    position[cle] = attaque + duree
                if elem.find("grace") is not None:
                    continue
                if any(t.get("type") == "stop" for t in elem.findall("tie")):
                    continue
                pitch = elem.find("pitch")
                if pitch is None:
                    continue  # silence : il a déjà avancé le compteur
                step = pitch.findtext("step")
                octave_txt = pitch.findtext("octave")
                if step not in STEPS or octave_txt is None:
                    continue
                alter = pitch.findtext("alter", "0")
                hauteur = 12 * (int(octave_txt) + 1) + STEPS[step] + int(float(alter))
                notes.append((hauteur + transposition, attaque, duree, numero_part))
    return notes


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
