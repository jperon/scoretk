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
placées à l'attaque courante, sans avancer le curseur, ce qui correspond à
leur durée empruntée.

Le temps MusicXML est représenté par **un seul curseur par partie** : les
notes sans ``<chord/>`` l'avancent de leur durée, les accords partagent
l'attaque précédente, ``<backup>``/``<forward>`` le déplacent, et chaque
nouvelle mesure repart de l'extension maximale de la précédente. Ce modèle
remplace l'ancien compteur par voix (puis par couple voix/portée), qui
décalait les voix réutilisées sur plusieurs portées. Voir ``README.md`` pour
le détail, les exemples minimaux et la convention de comparaison des
percussions ``<unpitched>``.
"""

from __future__ import annotations

import argparse
from collections import Counter
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

    Comme dans `scripts/oracle/midi/midi_notes.py`, une **file** par couple
    (canal, hauteur) : plusieurs notes peuvent se chevaucher sur la même
    hauteur (`docs/agents/format-ref/midi.md`, §Channel Voice ; MuseScore
    `midishared/midifile.cpp` apparie chaque note-on au premier note-off
    encore libre). Une table simple écraserait l'attaque précédente.
    """
    import mido
    from collections import defaultdict, deque

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
    """Notes d'un MusicXML partwise : hauteur concert, attaque, durée, partie.

    MusicXML définit le temps **par partie avec un seul curseur**, jamais par
    voix ni par couple (voix, portée). Le curseur est tenu en noires :

    * une note sans ``<chord/>`` est placée au curseur et l'avance de sa
      ``<duration>`` convertie par ``<divisions>`` ;
    * une note avec ``<chord/>`` partage l'attaque de la note précédente ;
    * ``<backup>`` recule le curseur, ``<forward>`` l'avance ;
    * une nouvelle mesure commence à l'attache de la précédente augmentée de
      l'**extension maximale** atteinte dans celle-ci (le plus grand curseur
      vu, et non sa position finale, souvent reculée par un ``<backup>``) ;
    * une note de grâce (``<grace>``) est comptée sans durée : elle est placée
      à l'attaque courante et ne fait pas avancer le curseur.

    Les ``<divisions>`` changent en cours de mesure via ``<attributes>`` et
    s'appliquent aux durées suivantes, y compris celles des
    ``<backup>``/``<forward>``. Les prolongations de liaison
    (``<tie type="stop"/>``) ne sont pas des attaques et sont écartées, mais
    elles occupent leur durée sur le curseur. La transposition
    ``<chromatic>``/``<octave-change>`` est appliquée aux hauteurs, comme dans
    l'oracle historique. Enfin, les notes non accordées (``<unpitched>``) des
    batteries sont lues comme des hauteurs MIDI : voir le chapitre dédié du
    ``README.md`` pour la convention de comparaison des percussions.
    """
    racine = ET.parse(path).getroot()
    notes: list[tuple[int, float, float, int]] = []
    for numero_part, part in enumerate(racine.findall("part")):
        divisions = 1
        transposition = 0
        curseur = 0.0             # position courante de la partie, en noires
        debut_mesure = 0.0        # attache de la mesure en cours
        extension = 0.0           # extension maximale de la mesure en cours
        attaque_precedente = 0.0  # attache de la dernière note (pour <chord/>)
        for measure in part.findall("measure"):
            debut_mesure += extension
            curseur = debut_mesure
            extension = 0.0
            attaque_precedente = curseur
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
                    valeur = elem.findtext("duration")
                    if not valeur:
                        continue  # argument absent : on ne bouge pas le curseur
                    delta = int(float(valeur)) / divisions
                    curseur += delta if elem.tag == "forward" else -delta
                    extension = max(extension, curseur - debut_mesure)
                    continue
                if elem.tag != "note":
                    continue
                duree_texte = elem.findtext("duration")
                duree = int(float(duree_texte)) / divisions if duree_texte else 0.0
                est_grace = elem.find("grace") is not None
                est_accord = elem.find("chord") is not None
                attaque = attaque_precedente if est_accord else curseur
                if not est_accord:
                    attaque_precedente = attaque
                    # Une grâce ne consomme pas de temps sur la trame.
                    if not est_grace:
                        curseur = attaque + duree
                        extension = max(extension, curseur - debut_mesure)
                if any(t.get("type") == "stop" for t in elem.findall("tie")):
                    continue
                pitch = elem.find("pitch")
                if pitch is None:
                    # Percussion `<unpitched>` : les écritures qui ne portent
                    # pas de hauteur MIDI sont comparées par leur position de
                    # tête (display-step/display-octave). scoretk écrit ses
                    # fûts comme des `<pitch>` MIDI (cf. README.md) : ils sont
                    # donc comparés directement au MIDI de LilyPond.
                    unpitched = elem.find("unpitched")
                    if unpitched is not None:
                        step_u = unpitched.findtext("display-step")
                        octave_u = unpitched.findtext("display-octave")
                        if step_u in STEPS and octave_u is not None:
                            hauteur_u = 12 * (int(octave_u) + 1) + STEPS[step_u]
                            notes.append((hauteur_u + transposition, attaque,
                                          0.0 if est_grace else duree, numero_part))
                    continue  # silence : le curseur a déjà avancé
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
        <backup><duration>12</duration></backup>
        <note><rest/><duration>4</duration><voice>1</voice><staff>2</staff><type>quarter</type></note>
        <note><pitch><step>A</step><octave>3</octave></pitch><duration>4</duration>
          <voice>1</voice><staff>2</staff><type>quarter</type></note>
        <note><grace cue="/"/><pitch><step>B</step><octave>4</octave></pitch>
          <duration>1</duration><voice>1</voice><staff>2</staff></note>
      </measure></part>
    </score-partwise>"""
    # Hauteurs attendues : C4+2-12 = 50, E4 -> 54, G4 -> 57, A3+2-12 = 47,
    # B4 (grâce) -> 61. La prolongation de liaison (tie stop) n'est pas une
    # attaque : écartée. La grâce est comptée, sans avancer le curseur.
    # La portée 2 réutilise la **voix 1** (cas du piano) : le <backup> doit
    # ramener le curseur à zéro, sinon A3 et la grâce sont décalés de 3
    # noires. Un compteur par voix échoue ici — c'est le test qui échoue
    # avant le passage au curseur unique par partie.
    att = [
        (50, 0.0, 1.0, 0), (54, 0.0, 1.0, 0), (57, 1.0, 1.0, 0),
        (47, 1.0, 1.0, 0), (61, 2.0, 0.0, 0),
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

        # Le même saut après une première note est un déplacement réel du
        # curseur : C4 occupe 0→1, le backup ramène à 0, le forward avance à 2.
        # F5 et A5 partent donc de 2,0 et 3,0 (plus de compteur par voix pour
        # l'ignorer).
        flux = levee.replace(
            "<forward><duration>2</duration></forward>",
            "<note><pitch><step>C</step><octave>4</octave></pitch>"
            "<duration>1</duration><voice>1</voice><staff>1</staff></note>"
            "<backup><duration>1</duration></backup><forward><duration>2</duration></forward>",
        )
        with open(p, "w", encoding="utf-8") as f:
            f.write(flux)
        assert read_xml(p)[-2:] == [(77, 2.0, 1.0, 0), (81, 3.0, 1.0, 0)], read_xml(p)

        # Correction 6 — changement de <divisions>, accord et extension de
        # mesure. La mesure 1 avance le curseur à 3 noires (forward 2 + note
        # 1) ; la mesure 2 change de divisions (2 par noire) et son backup
        # d'une noire place A4 sur l'attaque de G4. L'extension maximale de
        # la mesure 1 (3, pas la position finale après backup) fixe le début
        # de la mesure 2.
        div = """<score-partwise version="4.0">
          <part id="P1">
            <measure number="1">
              <attributes><divisions>1</divisions></attributes>
              <forward><duration>2</duration></forward>
              <note><pitch><step>F</step><octave>4</octave></pitch><duration>1</duration>
                <voice>1</voice><staff>1</staff></note>
            </measure>
            <measure number="2">
              <attributes><divisions>2</divisions></attributes>
              <note><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration>
                <voice>1</voice><staff>1</staff></note>
              <note><chord/><pitch><step>B</step><octave>4</octave></pitch><duration>2</duration>
                <voice>1</voice><staff>1</staff></note>
              <backup><duration>2</duration></backup>
              <note><pitch><step>A</step><octave>4</octave></pitch><duration>2</duration>
                <voice>1</voice><staff>1</staff></note>
            </measure>
          </part></score-partwise>"""
        with open(p, "w", encoding="utf-8") as f:
            f.write(div)
        attendu_div = [
            (65, 2.0, 1.0, 0), (67, 3.0, 1.0, 0),
            (71, 3.0, 1.0, 0), (69, 3.0, 1.0, 0),
        ]
        assert read_xml(p) == attendu_div, read_xml(p)

        # Percussions `<unpitched>` : une tête B4 vaut 71 ; sa durée et son
        # attaque suivent le même curseur que les notes accordées. Le cas
        # « non concerné » est le `<pitch>` ordinaire, inchangé.
        perc = """<score-partwise version="4.0">
          <part id="P1"><measure number="1">
            <attributes><divisions>1</divisions></attributes>
            <note><unpitched><display-step>B</display-step><display-octave>4</display-octave>
              </unpitched><duration>1</duration><voice>1</voice><staff>1</staff></note>
            <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
              <voice>1</voice><staff>1</staff></note>
          </measure></part></score-partwise>"""
        with open(p, "w", encoding="utf-8") as f:
            f.write(perc)
        assert read_xml(p) == [(71, 0.0, 1.0, 0), (60, 1.0, 1.0, 0)], read_xml(p)

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

        # Correction 4 — voix traversant plusieurs portées (`\change Staff`).
        # Une voix garde sa chronologie quand `<staff>` change : le compteur
        # d'attaque est propre à la voix, pas au couple (voix, portée). Ici la
        # voix 1 alterne portée 1 et 2 : les attaques sont 0,1,2,3 et non
        # 0,0,1,1 (le compteur ne doit pas repartir à chaque changement).
        change = """<score-partwise version="4.0">
          <part id="P1"><measure number="1">
            <attributes><divisions>1</divisions></attributes>
            <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
              <voice>1</voice><staff>1</staff></note>
            <note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration>
              <voice>1</voice><staff>2</staff></note>
            <note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration>
              <voice>1</voice><staff>1</staff></note>
            <note><pitch><step>F</step><octave>4</octave></pitch><duration>1</duration>
              <voice>1</voice><staff>2</staff></note>
          </measure></part></score-partwise>"""
        with open(p, "w", encoding="utf-8") as f:
            f.write(change)
        lu_change = read_xml(p)
        assert [(n[1], n[3]) for n in lu_change] == [
            (0.0, 0), (1.0, 0), (2.0, 0), (3.0, 0)
        ], lu_change
        # Le même XML sans changement de portée doit rester identique : cas
        # « non concerné » qui ne doit pas régresser.
        same = change.replace("<staff>2</staff>", "<staff>1</staff>")
        with open(p, "w", encoding="utf-8") as f:
            f.write(same)
        assert [n[1] for n in read_xml(p)] == [0.0, 1.0, 2.0, 3.0]

        # Correction 5 — `<forward>` de mesure partielle en fin de mesure.
        # Le saut est le dernier élément de la mesure 1 ; la note qui suit est
        # dans la mesure 2. Le compteur de la voix doit avancer de 2 noires,
        # sinon toute la deuxième mesure est décalée (knjze12monferA.ly).
        partiel = """<score-partwise version="4.0">
          <part id="P1">
            <measure number="1">
              <attributes><divisions>1</divisions></attributes>
              <forward><duration>2</duration></forward>
            </measure>
            <measure number="2">
              <note><pitch><step>F</step><octave>4</octave></pitch><duration>1</duration>
                <voice>1</voice><staff>1</staff></note>
              <note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration>
                <voice>1</voice><staff>1</staff></note>
            </measure>
          </part></score-partwise>"""
        with open(p, "w", encoding="utf-8") as f:
            f.write(partiel)
        assert read_xml(p) == [(65, 2.0, 1.0, 0), (67, 3.0, 1.0, 0)], read_xml(p)

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
