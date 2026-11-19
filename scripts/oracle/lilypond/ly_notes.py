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
* N2 : multiset des couples (hauteur, attaque quantifiée sur la grille Q16).
  L'appariement est une **intersection de multiensembles**, sans choix
  arbitraire de la paire : deux notes de même hauteur et d'attaques qui se
  recouvrent sur la grille comptent une fois.

Les reprises sont la difficulté propre à LilyPond : son MIDI déplie les
``\\repeat`` alors que le MusicXML peut les écrire une seule fois. L'ancien
N2 appariait « les attaques les plus anciennes de chaque hauteur » ; cet
appariement arbitraire désynchronise tout ce qui suit le premier bloc déplié
et gonfle ou écrase les résultats selon le hasard des doublons. Il a été
supprimé : N2 n'est fiable que sur les fichiers **sans reprise**, détectés par
``a_des_repetitions()`` ; les autres sont signalés et écartés de l'agrégat N2.

Les notes de grâce sont comptées des deux côtés : LilyPond les joue dans son
MIDI, et le MusicXML les porte comme éléments ``<grace>``. Côté XML elles sont
placées à l'attaque courante (celle de la note suivante), sans avancer le
compteur, ce qui correspond à leur durée empruntée. Un ``<forward>`` qui
précède la première note d'une voix avance son compteur (levée, silence
d'espaceur) ; les ``<backup>``/``<forward>`` ultérieurs sont des artefacts de
positionnement de flux et sont ignorés, le compteur étant propre à chaque
voix. Voir ``README.md`` pour le détail et les exemples minimaux.
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import json
import re
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
    silences avancent le compteur, et les prolongations de liaison
    (``<tie type="stop"/>``) ne sont pas des attaques et sont écartées.

    Les notes de grâce (``<grace>``) sont **comptées** : LilyPond les joue dans
    son MIDI, les écarter d'un seul côté fausserait N1 et N2. Elles sont
    placées à l'attaque courante (celle de la note qui suit) et n'avancent pas
    le compteur, leur durée étant empruntée à cette note. Un ``<forward>`` qui
    précède la **première** note d'une voix avance son compteur : c'est le cas
    d'une levée ou d'un silence d'espaceur écrit sous forme de saut. Les
    ``<backup>``/``<forward>`` suivants sont des artefacts de positionnement de
    flux des exportateurs ; le compteur par voix les rend inutiles et les
    honorer décalerait les attaques (vérifié sur des fichiers réels).

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
        voix_vues: set[tuple[str, str]] = set()
        for measure in part.findall("measure"):
            enfants = list(measure)
            for index, elem in enumerate(enfants):
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
                if elem.tag == "forward":
                    # Un saut avant la première note d'une voix décale son
                    # entrée (levée / espaceur). Ensuite, c'est un artefact de
                    # flux : le compteur propre à la voix suffit.
                    valeur = elem.findtext("duration")
                    if not valeur:
                        continue
                    suivante = next((n for n in enfants[index + 1:] if n.tag == "note"), None)
                    if suivante is None:
                        continue
                    cle_suivante = (suivante.findtext("voice") or "1",
                                    suivante.findtext("staff") or "1")
                    if cle_suivante not in voix_vues:
                        position[cle_suivante] += int(float(valeur)) / divisions
                    continue
                if elem.tag != "note":
                    continue
                duree_texte = elem.findtext("duration")
                duree = int(float(duree_texte)) / divisions if duree_texte else 0.0
                voix = elem.findtext("voice") or "1"
                portee = elem.findtext("staff") or "1"
                cle = (voix, portee)
                est_grace = elem.find("grace") is not None
                est_accord = elem.find("chord") is not None
                attaque = derniere_attaque.get(cle, 0.0) if est_accord else position[cle]
                if not est_accord:
                    derniere_attaque[cle] = attaque
                    # Une grâce ne consomme pas de temps sur la trame.
                    if not est_grace:
                        position[cle] = attaque + duree
                voix_vues.add(cle)
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
                notes.append((hauteur + transposition, attaque,
                              0.0 if est_grace else duree, numero_part))
    return notes


def compter_paires(
    reference: list[tuple], test: list[tuple], grid: float = GRID
) -> int:
    """Intersection de multiensembles de couples (hauteur, attaque quantifiée).

    Chaque attaque est ramenée à la grille Q16 puis on compte
    ``sum((ref & test).values())``. Aucun appariement « le plus tôt possible »
    n'est fait : avec des doublons (reprises, notes répétées) il associerait des
    événements différents et dépendrait de l'ordre des listes. L'intersection
    de multiensembles est symétrique, déterministe et ``O(n)``.
    """
    ref = Counter((note[0], quant(note[1], grid)) for note in reference)
    essai = Counter((note[0], quant(note[1], grid)) for note in test)
    return sum((ref & essai).values())


def a_des_repetitions(texte: str) -> bool:
    """Vrai si la source LilyPond contient une construction de reprise.

    ``\\repeat`` (tous les modes), ``\\unfoldRepeats`` et les marqueurs de
    ``volta``/``alternative`` sont détectés. Le ``.ly`` fourni peut inclure ses
    voix : l'appelant lui concatène les inclusions directes avant cet appel.
    N2 n'est fiable que lorsque cette fonction est fausse : le MIDI de
    LilyPond déplie les reprises, le MusicXML les écrit souvent une seule fois.
    """
    return bool(re.search(r"\\(?:repeat|unfoldRepeats|volta|alternative)\b", texte))


def compare(
    reference: list[tuple], test: list[tuple], grid: float = GRID, tol: float = TOL
) -> dict:
    """Compare deux listes de notes et renvoie un relevé sérialisable.

    ``tol`` est conservé pour la compatibilité de l'interface CLI : la
    correspondance se fait désormais exactement sur la grille de quantification
    (``grid``), sans tolérance flottante ni appariement arbitraire.
    """
    r = len(reference)
    t = len(test)
    cr = Counter(n[0] for n in reference)
    ct = Counter(n[0] for n in test)
    n1 = sum((cr & ct).values())
    n2 = compter_paires(reference, test, grid)

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
        <note><grace cue="/"/><pitch><step>B</step><octave>4</octave></pitch>
          <duration>1</duration><voice>1</voice><staff>1</staff></note>
      </measure></part>
    </score-partwise>"""
    # Hauteurs attendues : C4+2-12 = 50, E4 -> 54, G4 -> 57, A3+2-12 = 47,
    # B4 (grâce) -> 61. La prolongation de liaison (tie stop) n'est pas une
    # attaque : écartée. La grâce est comptée, sans avancer le compteur.
    att = [
        (50, 0.0, 1.0, 0), (54, 0.0, 1.0, 0), (57, 1.0, 1.0, 0),
        (47, 1.0, 1.0, 0), (61, 3.0, 0.0, 0),
    ]
    with tempfile.TemporaryDirectory() as tmp:
        p = os.path.join(tmp, "musique.musicxml")
        with open(p, "w", encoding="utf-8") as f:
            f.write(xml)
        lu = read_xml(p)
        assert lu == att, (lu, att)

        # Cas « non concerné » : XML lisible, hauteur et attaque exactes.
        relevant = compare(att, lu)
        assert relevant["exact"] and relevant["n1"] == 5 and relevant["n2"] == 5

        # Correction 1 — grâce absente du candidat : LilyPond la joue dans son
        # MIDI, N1 et N2 doivent chuter. Le relevé précédent l'écartait à tort.
        sans_grace = [n for n in att if n[0] != 61]
        manque = compare(att, sans_grace)
        assert manque["n1"] == 4 and manque["n2"] == 4, manque

        # Correction 2 — <forward> avant la première note d'une voix (levée).
        # Le silence de deux noires décale l'entrée : F5 passe de 0,0 à 2,0.
        levee = """<score-partwise version="4.0">
          <part id="P1"><measure number="1">
            <attributes><divisions>1</divisions></attributes>
            <forward><duration>2</duration></forward>
            <note><pitch><step>F</step><octave>5</octave></pitch><duration>1</duration>
              <voice>1</voice><staff>1</staff></note>
            <note><pitch><step>A</step><octave>5</octave></pitch><duration>1</duration>
              <voice>1</voice><staff>1</staff></note>
          </measure></part></score-partwise>"""
        with open(p, "w", encoding="utf-8") as f:
            f.write(levee)
        assert read_xml(p) == [(77, 2.0, 1.0, 0), (81, 3.0, 1.0, 0)], read_xml(p)

        # Cas « non concerné » : après une première note, backup/forward sont
        # des artefacts de flux et ne doivent PAS avancer le compteur de voix.
        flux = levee.replace(
            "<forward><duration>2</duration></forward>",
            "<note><pitch><step>C</step><octave>4</octave></pitch>"
            "<duration>1</duration><voice>1</voice><staff>1</staff></note>"
            "<backup><duration>1</duration></backup><forward><duration>2</duration></forward>",
        )
        with open(p, "w", encoding="utf-8") as f:
            f.write(flux)
        # F5 et A5 restent à 1,0 et 2,0 : le saut est bien ignoré ici.
        assert read_xml(p)[-2:] == [(77, 1.0, 1.0, 0), (81, 2.0, 1.0, 0)], read_xml(p)

        # Correction 3 — reprises : l'ancien appariement « attaques les plus
        # anciennes par hauteur » consommait un couple sur la copie surnumé-
        # raire et ratait les deux notes pourtant bien datées. L'intersection
        # de multiensembles les compte.
        ref_rep = [(50, 0.0, 1.0, 0), (50, 1.0, 1.0, 0), (50, 2.0, 1.0, 0)]
        test_rep = [(50, 1.0, 1.0, 0), (50, 2.0, 1.0, 0)]
        rep = compare(ref_rep, test_rep)
        assert rep["n1"] == 2 and rep["n2"] == 2, rep

        # Détection des reprises, y compris dans les marqueurs de volta.
        assert a_des_repetitions("\\repeat volta 2 { c4 d4 }")
        assert a_des_repetitions("\\unfoldRepeats \\alternative { { c4 } }")
        assert not a_des_repetitions("\\relative c' { c4 d4 e4 f4 }")

        # Attaque décalée sur toute la musique : N1 reste exact, N2 s'effondre.
        decale = [(h, a + 1.0, d, piste) for h, a, d, piste in att]
        dec = compare(att, decale)
        assert dec["n1"] == 5 and dec["n2"] == 0, dec
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
