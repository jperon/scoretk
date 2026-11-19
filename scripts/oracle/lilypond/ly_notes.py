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

"""Extracteur et comparateur de notes pour l'oracle LilyPond.

Aucune dépendance à scoretk : ``mido`` lit le MIDI de référence produit par
LilyPond et ``xml.etree.ElementTree`` (bibliothèque standard) lit le MusicXML
des candidats. Une note est un quadruplet ``(hauteur, attaque, durée, piste)``
où attaque et durée sont exprimées en **noires** (quarters).

Deux points de comparaison :

* N1 : multiset des hauteurs (comme l'oracle historique ``xmlcmp/cmp.py``) ;
* N2 : multiset des couples (hauteur, attaque), avec la même grille Q16 et la
  même tolérance de ±1/16 de noire que ``scripts/oracle/midi``.

Les reprises sont la difficulté propre à LilyPond : son MIDI déplie toujours
``\\repeat unfold`` et déplie ``\\repeat volta`` dès que ``\\unfoldRepeats``
est présent, alors que le MusicXML écrit chaque note **une seule fois**. Pour
que l'attaque soit comparable, N2 ne porte que sur le sous-ensemble apparié
par N1 : pour chaque hauteur on garde les ``min(nb_ref, nb_test)`` attaques
**les plus anciennes** de chaque côté. Les copies ajoutées par une reprise
dépliée arrivent après la première passe et sont écartées par l'intersection
N1. Voir ``README.md`` pour la limite connue (musique située après un bloc
déplié) et la justification détaillée.
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import json
import xml.etree.ElementTree as ET

GRID = 0.25  # pas de quantification : une double croche = 1/16 de noire
TOL = 0.25  # tolérance de rapprochement : ±1/16 de noire
STEPS = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


def quant(x: float, grid: float = GRID) -> float:
    """Ramène une position à la grille de quantification."""
    return round(x / grid) * grid


def read_midi(path: str) -> list[tuple[int, float, float, int]]:
    """Notes d'un SMF LilyPond : hauteur, attaque, durée, piste.

    ``mido`` donne un temps relatif par piste ; toutes les pistes démarrent à
    zéro, donc la somme des deltas dans une piste est bien l'attaque absolue
    en ticks, convertie en noires avec ``ticks_per_beat``. Les note-off et les
    note-on à vélocité nulle ferment l'attaque correspondante.
    """
    import mido

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
    """Notes d'un MusicXML partwise : hauteur concert, attaque, durée, partie.

    Le compteur d'attaque est propre à chaque (voix, portée) et repart de zéro
    pour chaque partie, comme le temps musical du fichier. Les barres de
    contrôle et les variables du ``.ly`` sont déjà résolues par le convertisseur
    en hauteurs et positions absolues ; elles n'apparaissent pas ici. Les
    accords (``<chord/>``) partagent l'attaque de la note précédente, les
    silences avancent le compteur, les notes de grâce et les prolongations de
    liaison (``<tie type="stop"/>``) ne sont pas des attaques et sont écartées.
    La transposition ``<chromatic>``/``<octave-change>`` est appliquée, comme
    dans l'oracle historique.
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


def rapprocher_attaques(
    reference: list[tuple], test: list[tuple], tol: float = TOL
) -> int:
    """Attaques appariées dans le sous-ensemble sélectionné par N1.

    Pour chaque hauteur, les attaques des deux côtés sont triées et seules les
    ``min(nb_ref, nb_test)`` premières (donc les plus anciennes) sont comparées,
    appariées dans l'ordre. C'est ce qui neutralise les reprises dépliées : le
    surnombre de la référence est écarté avant le rapprochement. La complexité
    est ``O(n log n)`` : les fichiers mesurés font au plus quelques milliers de
    notes.
    """
    ref_par_h: dict[int, list[float]] = defaultdict(list)
    test_par_h: dict[int, list[float]] = defaultdict(list)
    for note in reference:
        ref_par_h[note[0]].append(note[1])
    for note in test:
        test_par_h[note[0]].append(note[1])
    total = 0
    for hauteur, refs in ref_par_h.items():
        candidats = sorted(test_par_h.get(hauteur, []))
        if not candidats:
            continue
        refs.sort()
        for attaque_ref, attaque_test in zip(refs[: len(candidats)], candidats[: len(refs)]):
            if abs(attaque_ref - attaque_test) <= tol:
                total += 1
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
    n2 = rapprocher_attaques(reference, test, tol)

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
        "octave": octave,
        "transposition": transposition,
    }
    resultat["pct_n1"] = round(100.0 * n1 / r, 2) if r else 0.0
    resultat["pct_n2"] = round(100.0 * n2 / r, 2) if r else 0.0
    resultat["exact"] = bool(r and r == t == n1 == n2)
    resultat["drapeaux"] = drapeaux(resultat)
    return resultat


def drapeaux(releve: dict) -> list[str]:
    """Cause primaire des écarts, pour regrouper les fichiers par origine."""
    d: list[str] = []
    r, t = releve["ref"], releve["test"]
    if releve.get("echec"):
        return ["echec"]
    if releve.get("delai"):
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
    return d or ["autre"]


def selftest() -> None:
    """Autotest sans réseau ni corpus : musique ordinaire + cas limites."""
    import os
    import tempfile

    # Musique ordinaire : deux voix/portées, accord, silence, liaison, et une
    # transposition. Les barres de contrôle, variables et \relative du .ly sont
    # déjà résolus en positions absolues par le convertisseur.
    xml = """<score-partwise version="4.0">
      <part id="P1"><measure number="1">
        <attributes><divisions>4</divisions><transpose><chromatic>2</chromatic>
          <octave-change>-1</octave-change></transpose></attributes>
        <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration>
          <voice>1</voice><staff>1</staff><type>quarter</type></note>
        <note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration>
          <voice>1</voice><staff>1</staff><type>quarter</type></note>
        <note><pitch><step>G</step><octave>4</octave></pitch><duration>4</duration>
          <voice>1</voice><staff>1</staff><type>quarter</type><tie type="start"/></note>
        <note><pitch><step>G</step><octave>4</octave></pitch><duration>4</duration>
          <voice>1</voice><staff>1</staff><type>quarter</type><tie type="stop"/></note>
        <note><rest/><duration>4</duration><voice>2</voice><staff>2</staff><type>quarter</type></note>
        <note><pitch><step>A</step><octave>3</octave></pitch><duration>4</duration>
          <voice>2</voice><staff>2</staff><type>quarter</type></note>
        <note><grace cue="/"><pitch><step>B</step><octave>4</octave></pitch><duration>1</duration>
          <voice>1</voice><staff>1</staff></grace></note>
      </measure></part>
    </score-partwise>"""
    # Hauteurs attendues : C4+2-12 = 50, E4 -> 54, G4 -> 57, A3+2-12 = 47.
    # La prolongation de liaison (tie stop) n'est pas une attaque : écartée.
    att = [
        (50, 0.0, 1.0, 0), (54, 0.0, 1.0, 0), (57, 1.0, 1.0, 0),
        (47, 1.0, 1.0, 0),
    ]
    with tempfile.TemporaryDirectory() as tmp:
        p = os.path.join(tmp, "musique.musicxml")
        with open(p, "w", encoding="utf-8") as f:
            f.write(xml)
        lu = read_xml(p)
        assert lu == att, (lu, att)

        # Cas « non concerné » : XML lisible, hauteur et attaque exactes.
        relevant = compare(att, lu)
        assert relevant["exact"] and relevant["n1"] == 4 and relevant["n2"] == 4

        # Reprise dépliée : la référence contient une copie de la première
        # passe (les trois premières notes) après la passe d'origine. N1 les
        # écarte, N2 reste exact.
        unfold = att + [(50, 4.0, 1.0, 0), (54, 4.0, 1.0, 0), (57, 5.0, 1.0, 0)]
        repli = compare(unfold, lu)
        assert repli["n1"] == 4 and repli["n2"] == 4, repli

        # Attaque décalée : N1 reste exact, N2 chute (tolérance dépassée).
        decale = [(h, a + 1.0, d, piste) for h, a, d, piste in att]
        dec = compare(att, decale)
        assert dec["n1"] == 4 and dec["n2"] == 0, dec
        assert "attaque_decalee" in dec["drapeaux"]

        # Cas limite : listes vides (fin de fichier sans notes), pas de division
        # par zéro et des pourcentages nuls.
        vide = compare([], [])
        assert vide["pct_n1"] == 0.0 and vide["pct_n2"] == 0.0 and not vide["exact"]

        # Cas limite : argument absent au niveau de l'outil (fichier introuvable)
        # est signalé par read_xml comme une exception, pas par un résultat faux.
        try:
            read_xml(os.path.join(tmp, "absent.musicxml"))
        except (OSError, ET.ParseError):
            pass
        else:
            raise AssertionError("read_xml aurait dû échouer sur un fichier absent")

        # Cas limite : accord en fin de mesure, après la dernière barre implicite.
        fin = xml.replace("</measure>", """<note><chord/><pitch><step>C</step>
          <octave>5</octave></pitch><duration>4</duration><voice>1</voice>
          <staff>1</staff></note></measure>""")
        with open(p, "w", encoding="utf-8") as f:
            f.write(fin)
        assert read_xml(p)[-1][0] == 62  # C5 transposé de +2 et -1 octave

    print("selftest : OK")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("action", choices=["midi", "xml", "compare", "selftest"], nargs="?")
    p.add_argument("fichiers", nargs="*")
    p.add_argument("--grid", type=float, default=GRID)
    p.add_argument("--tol", type=float, default=TOL)
    args = p.parse_args()
    if args.action == "selftest":
        selftest()
    elif args.action == "compare":
        ref = read_midi(args.fichiers[0])
        test = read_xml(args.fichiers[1])
        print(json.dumps(compare(ref, test, args.grid, args.tol)))
    else:
        for f in args.fichiers:
            for note in (read_midi(f) if args.action == "midi" else read_xml(f)):
                print(f"{note[0]}\t{note[1]:.4f}\t{note[2]:.4f}\t{note[3]}")


if __name__ == "__main__":
    main()
