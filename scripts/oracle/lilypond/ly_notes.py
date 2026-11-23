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
import sys
from pathlib import Path
import xml.etree.ElementTree as ET

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import common  # noqa: E402

GRID = 0.25  # pas de quantification : une double croche = 1/16 de noire
TOL = 0.25  # tolérance de rapprochement : ±1/16 de noire
STEPS = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


def quant(x: float, grid: float = GRID) -> float:
    """Ramène une position à la grille de quantification.

    Les attaques MusicXML sont des sommes de ``duration / divisions`` : un
    triolet vaut 1/6 de noire, non représentable exactement, et la somme peut
    donner 6.625000000000001 là où le MIDI LilyPond porte le tick exact 6.625.
    Un ``round`` direct sur un demi-pas (26,5 cases) basculerait alors la note
    vers la case suivante. On ramène d'abord le quotient à 9 décimales, ce qui
    ne change pas le résultat des valeurs exactes (arrondi au pair inchangé)
    et rend la quantification insensible au bruit flottant.
    """
    return round(round(x / grid, 9)) * grid


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
    from collections import defaultdict, deque

    mid = common.lire_midi(path)
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


def nombre_portees(path: str) -> int:
    """Nombre de couples (partie, portée) qui portent au moins une note.

    Sert à décider si les unissons de voix peuvent être fusionnés : si le
    MusicXML sépare moins de portées que le MIDI de référence n'a de pistes,
    plusieurs pistes ont été repliées sur une même portée et la fusion
    masquerait des notes distinctes.
    """
    racine = ET.parse(path).getroot()
    portees: set[tuple[int, str]] = set()
    for numero_part, part in enumerate(racine.findall("part")):
        for measure in part.findall("measure"):
            for elem in measure:
                if elem.tag != "note":
                    continue
                if elem.find("pitch") is None and elem.find("unpitched") is None:
                    continue
                portees.add((numero_part, elem.findtext("staff", "1")))
    return len(portees)


def read_xml(
    path: str, with_grace: bool = False, merge_unisons: bool = True
) -> list[tuple]:
    """Notes d'un MusicXML partwise : hauteur concert, attaque, durée, partie.

    Avec ``with_grace=True``, chaque note porte un cinquième élément indiquant
    si elle est une grâce (``<grace/>``). Le format par défaut reste inchangé
    pour les autres appelants.

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
    meta: list[tuple[int, str, str, bool]] = []  # (part, portée, voix, grâce)
    # Rang de chaque grâce depuis la note principale qu'elle orne, par voix :
    # le MusicXML pose toutes les grâces à l'attaque logique de cette note,
    # alors que LilyPond les joue à des instants réels distincts, dans l'ordre
    # de la voix (`lily/grace-iterator.cc`, §2.6.1). Deux grâces de même
    # hauteur et de même rang (deux voix qui ornent le même accord,
    # `Troldtog` mesure 111) tombent au même instant réel ; `midi-walker.cc`
    # les fusionne alors. Le rang est calculé au fil du document, y compris
    # quand la note principale est une continuation de liaison écartée plus
    # bas.
    rangs: list[int] = []
    attente: dict[tuple[int, str, str], list[int]] = {}
    for numero_part, part in enumerate(racine.findall("part")):
        divisions = 1
        transposition = 0
        octave_shifts: dict[str, int] = {}
        # `<octave-shift>` est un événement de direction : il vaut à partir de
        # sa position dans la mesure, pas à partir de sa position dans l'ordre
        # du document. L'écrivain émet la direction dans le flux de la voix qui
        # la porte ; une voix écrite plus loin (après un `<backup>`) peut avoir
        # des notes antérieures à la direction (Sa_Taille : la fin de
        # l'ottava, écrite après la voix 1, ne doit pas annuler le décalage des
        # notes de la voix 3 du même tick). On collecte (tick, portée, valeur)
        # et on applique après lecture, par tick.
        octave_events: list[tuple[float, str, int]] = []
        debut_notes_part = len(notes)
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
                if elem.tag == "direction":
                    # `<octave-shift>` porte sur la hauteur *jouée* de la
                    # portée indiquée par `<staff>` : un changement sur la
                    # portée 1 ne doit pas décaler la portée 2 du même part.
                    staff = elem.findtext("staff", "1")
                    for type_element in elem.findall("direction-type"):
                        shift = type_element.find("octave-shift")
                        if shift is None:
                            continue
                        # `size` est un attribut de `<octave-shift>`
                        # (`musicxml-4.0.xsd:3925-3931`, défaut 8) : le lire
                        # comme un élément enfant ramenait 15ma à 12.
                        size = shift.get("size", "8")
                        amount = 24 if size == "15" else 12
                        kind = shift.get("type")
                        courant = octave_shifts.get(staff, 0)
                        if kind == "down":
                            courant -= amount
                        elif kind == "up":
                            courant += amount
                        elif kind == "stop":
                            courant = 0
                        octave_shifts[staff] = courant
                        octave_events.append((curseur, staff, courant))
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
                # `<cue/>` : la note est gravée mais non jouée
                # (ClusterNoteEvent). Le curseur avance quand même ; on ne
                # l'ajoute pas à la comparaison avec le MIDI.
                joue = elem.find("cue") is None
                est_grace = elem.find("grace") is not None
                est_accord = elem.find("chord") is not None
                staff_note = elem.findtext("staff", "1")
                voix_note = elem.findtext("voice", "1")
                cle_voix = (numero_part, staff_note, voix_note)
                if not est_grace and cle_voix in attente:
                    file = attente.pop(cle_voix)
                    total = len(file)
                    for position, indice in enumerate(file):
                        rangs[indice] = total - position
                attaque = attaque_precedente if est_accord else curseur
                if not est_accord:
                    attaque_precedente = attaque
                    # Une grâce ne consomme pas de temps sur la trame.
                    if not est_grace:
                        curseur = attaque + duree
                        extension = max(extension, curseur - debut_mesure)
                if any(t.get("type") == "stop" for t in elem.findall("tie")):
                    continue
                # `<offset>` décale la lecture sans déplacer le curseur
                # logique. L'écrivain s'en sert pour un ornement après
                # l'accord (`\afterGrace`), joué à 3/4 de la note principale.
                # MusicXML 4.0 : l'attribut `attack` d'une note décale sa
                # lecture (en divisions, positif ou négatif). L'ancien
                # `<offset>` enfant n'est pas valide sur une note ; il reste lu
                # pour les sorties d'autres outils qui l'utilisent.
                attack_texte = elem.get("attack")
                offset_texte = elem.findtext("offset")
                decalage = attack_texte if attack_texte is not None else offset_texte
                offset = int(float(decalage)) / divisions if decalage else 0.0
                jouee = attaque + offset
                # Genre de la note : 0 = principale, 1 = grâce avant l'attaque,
                # 2 = grâce après (`\afterGrace`, exportée avec `attack`/`offset`
                # positif ; `ly/music-functions-init.ly:86-112`). Les grâces
                # après sonnent à leur tour réel (`main + fraction × durée`),
                # donc leur attaque MusicXML est déjà le temps réel.
                genre = 0
                if est_grace:
                    genre = 2 if decalage and offset > 0 else 1
                staff = elem.findtext("staff", "1")
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
                            notes.append((hauteur_u + transposition, jouee,
                                          0.0 if est_grace else duree, numero_part,
                                          genre) if with_grace else
                                         (hauteur_u + transposition, jouee,
                                          0.0 if est_grace else duree, numero_part))
                            if not joue:
                                continue
                            meta.append((numero_part, elem.findtext("staff", "1"),
                                         elem.findtext("voice", "1"), est_grace))
                            rangs.append(0)
                            if est_grace:
                                attente.setdefault(cle_voix, []).append(len(notes) - 1)
                    continue  # silence : le curseur a déjà avancé
                if not joue:
                    continue
                step = pitch.findtext("step")
                octave_txt = pitch.findtext("octave")
                if step not in STEPS or octave_txt is None:
                    continue
                alter = pitch.findtext("alter", "0")
                hauteur = 12 * (int(octave_txt) + 1) + STEPS[step] + int(float(alter))
                notes.append((hauteur + transposition, jouee,
                              0.0 if est_grace else duree, numero_part, genre)
                             if with_grace else
                             (hauteur + transposition, jouee,
                              0.0 if est_grace else duree, numero_part))
                meta.append((numero_part, elem.findtext("staff", "1"),
                             elem.findtext("voice", "1"), est_grace))
                rangs.append(0)
                if est_grace:
                    attente.setdefault(cle_voix, []).append(len(notes) - 1)
        # Chaque partie a ses propres directions : les appliquer à ses notes
        # avant de passer à la suivante (les variables du début de boucle sont
        # réinitialisées à chaque partie).
        _appliquer_octave(notes, meta, debut_notes_part, octave_events)
    for file in attente.values():
        total = len(file)
        for position, indice in enumerate(file):
            rangs[indice] = total - position
    if merge_unisons:
        return _fusionner_unissons(notes, meta, rangs)
    return notes


def _appliquer_octave(
    notes: list[tuple], meta: list[tuple], debut: int,
    evenements: list[tuple[float, str, int]],
) -> None:
    """Applique les `<octave-shift>` par tick, pas dans l'ordre du document.

    Une direction vaut à partir de sa position dans la mesure
    (MusicXML, `<direction>`) ; l'écrivain l'émet dans le flux de la voix qui
    la porte, si bien que les notes d'une autre voix écrites après un
    `<backup>` peuvent la précéder dans le document tout en étant avant elle
    dans le temps. On prend donc, pour chaque note, le dernier événement de sa
    portée dont le tick est antérieur ou égal à son attaque.
    """
    if not evenements:
        return
    par_portee: dict[str, list[tuple[float, int]]] = {}
    for tick, staff, valeur in evenements:
        par_portee.setdefault(staff, []).append((tick, valeur))
    for liste in par_portee.values():
        liste.sort(key=lambda evenement: evenement[0])
    for index in range(debut, len(notes)):
        note = notes[index]
        if index >= len(meta):
            continue
        portee = meta[index][1]
        attaque = note[1]
        valeur = 0
        for tick, candidate in par_portee.get(portee, ()):
            if tick <= attaque + 1e-9:
                valeur = candidate
            else:
                break
        if valeur:
            notes[index] = (note[0] + valeur, *note[1:])


def _fusionner_unissons(
    notes: list[tuple],
    meta: list[tuple[int, str, str, bool]],
    rangs: list[int],
) -> list[tuple]:
    """Fusionne toutes les attaques de même hauteur au même tick d'une piste.

    ``Midi_walker::do_start_note`` (``lily/midi-walker.cc``, branche
    ``now_ticks == queued_ticks``) fusionne deux notes de même hauteur qui
    commencent au même tick **d'une même piste** : il garde une attaque et la
    plus longue extinction. Cela vaut pour deux voix d'une portée
    (``midiChannelMapping = #'staff``) comme pour deux têtes d'un accord
    ``<c' c'>``. Vérifié sur LilyPond 2.27 : ``<c' c'>4 <c' e' c'>4`` joue 60
    puis 60/64, et ``monochords.ly`` a 14 attaques d'une note, jamais deux
    exécutions simultanées de la même hauteur.

    Le MusicXML, lui, garde les deux voix et les deux têtes : c'est une
    gravure légitime. L'oracle compare le **son**, pas la gravure. La règle ne
    regarde que le **candidat** (partie, portée, hauteur, tick, grâce) : elle
    ne consulte pas la référence. Deux portées différentes restent deux pistes
    MIDI distinctes et ne sont jamais fusionnées.
    """
    groupes: dict[tuple, list[int]] = {}
    for index, note in enumerate(notes):
        if index >= len(meta):
            continue
        numero_part, portee, _voix, grace = meta[index]
        rang = rangs[index] if index < len(rangs) else 0
        cle = (numero_part, portee, note[0], round(note[1], 6), grace, rang)
        groupes.setdefault(cle, []).append(index)
    supprimer: set[int] = set()
    for indices in groupes.values():
        supprimer.update(indices[1:])
    if not supprimer:
        return notes
    return [note for index, note in enumerate(notes) if index not in supprimer]


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


def lire_graces(notes: list[tuple]) -> list[tuple[int, float, bool]]:
    """Graces du candidat : (hauteur, attaque, après la note principale ?).

    `lily/grace-iterator.cc` place les graces dans la *partie grace* du moment
    musical et `lily/midi-walker.cc` note que leur MIDI peut commencer à un
    temps négatif ; la Notation Reference §1.3.1 rappelle qu'elles ne prennent
    aucun temps logique. Côté MusicXML, `<grace/>` est placé à l'attaque
    courante, celle de la note principale qui suit — sauf `\afterGrace`, dont
    la grâce sonne **après** la note principale
    (`ly/music-functions-init.ly:86-112`) et dont l'attaque MusicXML est le
    temps réel (`attack`/`offset` positif).
    """
    return [
        (note[0], note[1], note[4] == 2)
        for note in notes
        if len(note) >= 5 and note[4]
    ]


def recaler_graces_initiales(
    reference: list[tuple], test: list[tuple], graces: list[tuple[int, float]],
    tolerance: float = 1e-6,
) -> tuple[list[tuple], float]:
    """Décale la référence du temps des grâces placées avant le premier temps.

    Le SMF ne peut pas écrire un temps négatif : les grâces qui précèdent le
    premier accord décalent tout le morceau de leur durée totale
    (`lily/midi-walker.cc` écrit des deltas ; LilyPond, lui, place ces grâces
    dans la partie « grâce » du moment, avant zéro). Le MusicXML les pose à
    l'attaque de la note principale, sans avancer le curseur. On ne compense
    que ce cas : le candidat commence par les grâces à l'attaque 0, et les
    `k` premières notes MIDI sont exactement ces grâces ; la suivante est la
    première note principale, son attaque est le décalage.

    Renvoie la référence décalée et le décalage appliqué (0.0 sinon).
    """
    principales = [n for n in test if not (len(n) >= 5 and n[4])]
    if not principales or not graces:
        return reference, 0.0
    premiere = min(n[1] for n in principales)
    if abs(premiere) > tolerance:
        return reference, 0.0
    entete = [
        hauteur
        for hauteur, attaque, apres in graces
        if not apres and abs(attaque - premiere) <= tolerance
    ]
    if not entete:
        return reference, 0.0
    notes = sorted(reference, key=lambda n: (n[1], n[0]))
    if len(notes) <= len(entete):
        return reference, 0.0
    if Counter(n[0] for n in notes[: len(entete)]) != Counter(entete):
        return reference, 0.0
    decalage = notes[len(entete)][1] - premiere
    if decalage <= tolerance:
        return reference, 0.0
    return [(n[0], n[1] - decalage, *n[2:]) for n in reference], decalage


def decalage_graces_reference(
    reference: list[tuple], graces: list[tuple[int, float]], grid: float = GRID,
    window: float = 1.0,
) -> list[tuple[float, float]]:
    """Pas de temps inséré par les grâces, calculé sur le MIDI seul.

    `lily/grace-iterator.cc:25-40` place la grâce dans la partie « grâce »
    du moment ; `lily/audio-item.cc:118-129` convertit ce moment en temps
    réel (`main + 9/40 × grace`), et `lily/audio-column.cc:32-34` l'utilise
    pour l'horodatage de chaque colonne audio. Une grâce écrite à l'attaque
    d'une note principale occupe donc un temps MIDI réel qui décale toutes
    les colonnes suivantes (`lily/midi-walker.cc:92-95`).

    On retire à chaque grâce la durée nominale qu'elle remplace (une
    double-croche, GRID/4 : c'est la valeur volée dans les tests `flags-*`) :
    seul l'excédent est du temps inséré. Le résultat est une fonction en escalier
    S(t) : somme des excédents des grâces d'attaque <= t. Les attaques du
    candidat ne servent qu'à *identifier* les grâces du MIDI (par hauteur et
    fenêtre), jamais à calculer le décalage.
    """
    utilises: set[int] = set()
    intervalles: list[tuple[float, float]] = []
    for hauteur, attaque, apres in graces:
        borne = attaque + grid / 2 if apres else attaque
        candidats = [
            i for i, note in enumerate(reference)
            if i not in utilises and note[0] == hauteur
            and attaque - window <= note[1] < borne
            # Une grâce est jouée brièvement dans le MIDI (audio-item.cc:122) :
            # ne pas confondre une note longue de même hauteur dans la fenêtre.
            and len(note) >= 3 and note[2] <= grid / 2
        ]
        if not candidats:
            continue
        i = max(candidats, key=lambda j: (reference[j][1], -j))
        utilises.add(i)
        note = reference[i]
        # Une grâce ne décale la mesure que si elle partage son attaque
        # avec une note principale qu'elle ampute (cas `flags-*` : les deux
        # note-on au tick 336). Une grâce jouée seule avant sa note ne
        # décale pas le temps principal.
        coupes = [
            other for j, other in enumerate(reference)
            if j != i and other[1] == note[1] and len(other) >= 3
            and other[2] < grid / 4
        ]
        if not coupes:
            continue
        coupe = min(coupes, key=lambda n: n[2])
        # Durée nominale de la note amputée : mode des durées courtes de
        # même hauteur dans le MIDI, c'est-à-dire celle qu'aurait eue la
        # note sans la grâce (lily/midi-walker.cc:92-95 raccourcit la note
        # au profit de la grâce).
        courtes = [
            n[2] for n in reference
            if n[0] == coupe[0] and grid / 4 <= n[2] <= grid / 2
        ]
        nominal = Counter(round(d, 6) for d in courtes).most_common(1)[0][0] if courtes else grid / 4
        vole = max(0.0, nominal - coupe[2])
        excedent = max(0.0, note[2] - vole)
        if excedent:
            intervalles.append((note[1], excedent))
    intervalles.sort()
    cumul = 0.0
    paliers: list[tuple[float, float]] = []
    for att, excedent in intervalles:
        cumul += excedent
        paliers.append((att, cumul))
    return paliers


def retirer_temps_graces(
    reference: list[tuple], paliers: list[tuple[float, float]],
) -> list[tuple]:
    """Retranche S(t) de chaque attaque de référence."""
    if not paliers:
        return reference
    resultat: list[tuple] = []
    for note in reference:
        decal = 0.0
        for att, cumul in paliers:
            if note[1] < att:
                break
            decal = cumul
        resultat.append((note[0], round(note[1] - decal, 6), *note[2:]))
    return resultat


def apparier_graces(
    reference: list[tuple], graces: list[tuple[int, float]], grid: float = GRID,
    window: float = 1.0,
) -> tuple[list[tuple], int, int, int]:
    """Retire du MIDI les graces appariées et les compte à part.

    Une grâce du candidat `(hauteur, attaque principale)` est appariée à la
    note MIDI **de même hauteur** la plus proche **strictement avant**
    l'attaque principale, dans la fenêtre `[attaque - window, attaque)`, qui
    n'a pas déjà été utilisée. `lily/audio-item.cc:118-129` place en effet une
    grâce à `main + 9/40 × grace_part`, avec un `grace_part` négatif
    (`lily/grace-iterator.cc:24-27`) : une grâce sonne **avant** l'attaque de
    la note qu'elle orne, jamais dessus. La borne haute `attaque + grid/2`
    volait la note principale quand la grâce avait la même hauteur
    (`grace-auto-beam-engraver` : les 28 `a1` étaient pris pour leurs grâces).
    L'appariement est déterministe (une seule note par grâce, la plus tardive
    en cas de doublon) et ne touche pas aux notes principales. Renvoie
    `(référence sans graces, graces MIDI, graces XML, graces appariées)`.
    """
    restants = list(reference)
    utilises: set[int] = set()
    apparies = 0
    for hauteur, attaque, apres in graces:
        # Grâce avant : strictement avant l'attaque (elle sonne dans la partie
        # « grâce » négative). Grâce après (`\afterGrace`) : son attaque
        # MusicXML est le temps réel, et LilyPond la place juste avant ce
        # temps (`main + 9/40 × grace_part`), donc dans `[attaque - window,
        # attaque + grid/2]`.
        borne = attaque + grid / 2 if apres else attaque
        candidats = [
            i for i, note in enumerate(restants)
            if i not in utilises and note[0] == hauteur
            and attaque - window <= note[1] < borne
        ]
        if candidats:
            utilises.add(max(candidats, key=lambda i: (restants[i][1], -i)))
            apparies += 1
    return ([note for i, note in enumerate(restants) if i not in utilises],
            len(utilises), len(graces), apparies)


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
    reference: list[tuple], test: list[tuple], grid: float = GRID, tol: float = TOL,
    graces: list[tuple[int, float]] | None = None,
) -> dict:
    """Compare deux listes de notes et renvoie un relevé sérialisable.

    ``tol`` est conservé pour la compatibilité de l'interface CLI : la
    correspondance se fait exactement sur la grille de quantification
    (``grid``), sans tolérance flottante ni appariement arbitraire.

    Quand ``graces`` est fourni (graces du candidat, voir `lire_graces`), on
    calcule aussi ``n2_grace`` : les graces du MIDI sont appariées par hauteur
    dans la fenêtre de la note principale et comparées sur cette attaque
    logique, au lieu d'être quantifiées à leur temps de lecture (§2.6.1 de la
    Notation Reference, `grace-iterator.cc`, `midi-walker.cc`). Les notes
    principales gardent exactement la même règle que pour ``n2``.
    """
    if graces is not None:
        reference, _ = recaler_graces_initiales(reference, test, graces)
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
    if graces is not None:
        paliers = decalage_graces_reference(reference, graces, grid)
        reference_sans_temps_grace = retirer_temps_graces(reference, paliers)
        ref_main, n_gr_ref, n_gr_test, n_gr_match = apparier_graces(
            reference_sans_temps_grace, graces, grid
        )
        test_main = [note for note in test if not (len(note) >= 5 and note[4])]
        n2_grace = compter_paires(ref_main, test_main, grid) + n_gr_match
        resultat.update({
            "n2_grace": n2_grace,
            "pct_n2_grace": round(100.0 * n2_grace / r, 2) if r else 0.0,
            "graces_ref": n_gr_ref,
            "graces_test": n_gr_test,
            "graces_match": n_gr_match,
        })
    # Exactitude : la grille ordinaire (Q16) ou la grille « graces appariées ».
    # LilyPond écrit chaque grâce à un temps réel distinct *avant* l'attaque
    # logique (`grace-iterator.cc`, `midi-walker.cc`) ; un MusicXML ne peut pas
    # exprimer ce décalage (la grâce ne prend pas de temps logique). La règle
    # n2_grace (attaque logique de la note principale) est donc l'autre modèle
    # fidèle : un fichier parfait sous l'un des deux est exact. Les deux
    # exigent le même nombre de notes et le même multiset de hauteurs.
    exact = n2 == r
    if graces is not None:
        exact = exact or resultat.get("n2_grace") == r
    resultat["exact"] = bool(r and r == t == n1 and exact)
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
        # Le même passage avec le marqueur de grâce : le B4 est bien signalé.
        lu_graces = read_xml(p, with_grace=True)
        assert [note[4] for note in lu_graces] == [False] * 4 + [True]
        assert lire_graces(lu_graces) == [(61, 2.0, False)]

        # `<octave-shift>` : l'écriture est décalée d'une octave, la hauteur
        # jouée (comparée au MIDI) ne l'est pas.
        ottava = """<score-partwise version="4.0"><part id="P1"><measure number="1">
          <attributes><divisions>1</divisions></attributes>
          <direction><direction-type><octave-shift type="down" size="8"/>
            </direction-type></direction>
          <note><pitch><step>D</step><octave>5</octave></pitch><duration>1</duration>
            <voice>1</voice></note>
          <direction><direction-type><octave-shift type="stop" size="8"/>
            </direction-type></direction>
          <note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration>
            <voice>1</voice></note>
        </measure></part></score-partwise>"""
        p2 = os.path.join(tmp, "ottava.musicxml")
        with open(p2, "w", encoding="utf-8") as f:
            f.write(ottava)
        assert read_xml(p2) == [(62, 0.0, 1.0, 0), (67, 1.0, 1.0, 0)]

        # Quantification : une attaque à un demi-pas de la grille doit être
        # insensible au bruit flottant des sommes de durées MusicXML. Le
        # triolet 1/6 + croches donne 6.625000000000001 côté XML et 6.625
        # côté MIDI ; sans arrondi préalable du quotient, l'un tombe sur 6,5
        # et l'autre sur 6,75.
        assert quant(6.625) == quant(6.625000000000001) == 6.5
        assert quant(6.875) == quant(6.875000000000001) == 7.0
        assert quant(0.49999999999999994) == 0.5

        # Cas « non concerné » : XML lisible, hauteur et attaque exactes.
        relevant = compare(att, lu)
        assert relevant["exact"] and relevant["n1"] == 5 and relevant["n2"] == 5

        # `<octave-shift>` par portée : un décalage sur la portée 1 ne doit
        # pas atteindre la portée 2 du même part (piano, Czerny).
        deux_portees = """<score-partwise version="4.0"><part id="P1"><measure number="1">
          <attributes><divisions>1</divisions></attributes>
          <direction><direction-type><octave-shift type="down" size="8"/>
            </direction-type><staff>1</staff></direction>
          <note><pitch><step>C</step><octave>6</octave></pitch><duration>1</duration>
            <voice>1</voice><staff>1</staff></note>
          <backup><duration>1</duration></backup>
          <note><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration>
            <voice>2</voice><staff>2</staff></note>
        </measure></part></score-partwise>"""
        with open(p, "w", encoding="utf-8") as f:
            f.write(deux_portees)
        assert read_xml(p) == [(72, 0.0, 1.0, 0), (72, 0.0, 1.0, 0)], read_xml(p)

        # `<octave-shift>` est une direction datée : l'écrivain l'émet dans le
        # flux de la voix qui la porte, si bien que les notes d'une voix écrite
        # après un `<backup>` peuvent la précéder dans le document tout en
        # étant avant elle dans le temps (Sa_Taille). Elles gardent le
        # décalage de leur tick.
        ordre = """<score-partwise version="4.0"><part id="P1"><measure number="1">
          <attributes><divisions>1</divisions></attributes>
          <direction><direction-type><octave-shift type="down" size="8"/>
            </direction-type><staff>1</staff></direction>
          <note><pitch><step>C</step><octave>6</octave></pitch><duration>1</duration>
            <voice>1</voice><staff>1</staff></note>
          <direction><direction-type><octave-shift type="stop" size="8"/>
            </direction-type><staff>1</staff></direction>
          <backup><duration>1</duration></backup>
          <note><pitch><step>D</step><octave>6</octave></pitch><duration>1</duration>
            <voice>2</voice><staff>1</staff></note>
        </measure></part></score-partwise>"""
        with open(p, "w", encoding="utf-8") as f:
            f.write(ordre)
        assert read_xml(p) == [(72, 0.0, 1.0, 0), (74, 0.0, 1.0, 0)], read_xml(p)

        # Correction 1 — grâce absente du candidat : LilyPond la joue dans son
        # MIDI, N1 et N2 doivent chuter. Le relevé précédent l'écartait à tort.
        sans_grace = [n for n in att if n[0] != 61]
        manque = compare(att, sans_grace)
        assert manque["n1"] == 4 and manque["n2"] == 4, manque

        # Règle des graces : le MIDI de LilyPond les joue *avant* l'attaque
        # logique (`grace-iterator.cc`, `midi-walker.cc`), le MusicXML les place
        # *à* cette attaque. La règle habituelle perd la première grâce, la
        # règle "appariée" la retrouve par hauteur dans la fenêtre de la note
        # principale, sans toucher aux notes ordinaires.
        ref_g = [(62, 0.0, 0.0625, 0), (60, 0.25, 1.0, 0),
                 (62, 1.0, 0.0625, 0), (64, 1.25, 1.0, 0)]
        test_g = [(62, 0.25, 0.0, 0), (60, 0.25, 1.0, 0),
                  (62, 1.25, 0.0, 0), (64, 1.25, 1.0, 0)]
        graces_g = [(62, 0.25, False), (62, 1.25, False)]
        brut = compare(ref_g, test_g)
        rule = compare(ref_g, test_g, graces=graces_g)
        assert brut["n2"] == 2, brut
        assert rule["n2"] == 2 and rule["graces_match"] == 2, rule
        assert rule["n2_grace"] == 4 and rule["pct_n2_grace"] == 100.0, rule
        assert rule["exact"], rule
        # Une grâce absente du candidat ne doit pas être inventée.
        absent = compare(ref_g, [(60, 0.25, 1.0, 0), (64, 1.25, 1.0, 0)],
                         graces=[])
        assert absent["n2_grace"] == 2 and absent["graces_match"] == 0, absent
        # Grâces initiales : le SMF décale tout le morceau de leur durée
        # totale (pas de temps négatif) ; on recale la référence sur la
        # première note principale avant de comparer.
        ref_lead = [(79, 0.0, 0.05, 0), (81, 0.05, 0.05, 0),
                    (83, 0.1, 0.05, 0), (84, 0.15, 1.0, 0),
                    (86, 1.15, 1.0, 0)]
        test_lead = [(79, 0.0, 0.0, 0, True), (81, 0.0, 0.0, 0, True),
                     (83, 0.0, 0.0, 0, True), (84, 0.0, 1.0, 0, False),
                     (86, 1.0, 1.0, 0, False)]
        lead = compare(ref_lead, test_lead,
                       graces=[(79, 0.0, False), (81, 0.0, False),
                               (83, 0.0, False)])
        assert lead["n2_grace"] == 5 and lead["exact"], lead
        # Le temps inséré par une grâce est calculé sur le MIDI seul :
        # une note principale déplacée d'une croche doit rester inexacte.
        ref_retard = [(62, 0.0, 0.1375, 0), (60, 0.5, 1.0, 0),
                      (64, 1.5, 1.0, 0)]
        graces_retard = [(62, 0.5, False)]
        juste = [(62, 0.5, 0.0, 0, True), (60, 0.5, 1.0, 0, False),
                 (64, 1.5, 1.0, 0, False)]
        faux = [(62, 0.5, 0.0, 0, True), (60, 1.0, 1.0, 0, False),
                (64, 1.5, 1.0, 0, False)]
        assert compare(ref_retard, juste, graces=graces_retard)["exact"]
        faux_releve = compare(ref_retard, faux, graces=graces_retard)
        assert not faux_releve["exact"] and faux_releve["n2_grace"] < 3, faux_releve

        # Sans graces déclarées, le relevé n'a pas de champ supplémentaire.
        assert "n2_grace" not in brut, brut

        # `<offset>` d'un ornement après l'accord : le curseur est à la fin de
        # la note principale (1,0), l'ornement est écrit après elle mais joué à
        # 3/4 de sa durée (−1/4 depuis la fin). Sans l'offset, il serait lu à
        # l'attaque de la note suivante.
        apres = """<score-partwise version="4.0"><part id="P1"><measure number="1">
          <attributes><divisions>4</divisions></attributes>
          <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration>
            <voice>1</voice></note>
          <note attack="-1"><grace/><pitch><step>D</step><octave>4</octave></pitch>
            <voice>1</voice></note>
          <note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration>
            <voice>1</voice></note>
        </measure></part></score-partwise>"""
        with open(p, "w", encoding="utf-8") as f:
            f.write(apres)
        assert read_xml(p) == [(60, 0.0, 1.0, 0), (62, 0.75, 0.0, 0),
                               (64, 1.0, 1.0, 0)], read_xml(p)

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

        # Unissons d'une même piste : `lily/midi-walker.cc` n'émet qu'une
        # attaque par hauteur et par tick sur une piste, que les doublons
        # viennent de deux voix ou de la même voix ; le MusicXML écrit les deux.
        # L'oracle compare le son : une seule note. Deux portées restent deux
        # pistes et ne sont jamais fusionnées.
        unisson = """<score-partwise version="4.0">
          <part id="P1">
            <measure number="1">
              <attributes><divisions>1</divisions></attributes>
              <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
                <voice>1</voice><staff>1</staff></note>
              <backup><duration>1</duration></backup>
              <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
                <voice>2</voice><staff>1</staff></note>
              <backup><duration>1</duration></backup>
              <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
                <voice>1</voice><staff>2</staff></note>
            </measure>
            <measure number="2">
              <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
                <voice>1</voice><staff>1</staff></note>
              <backup><duration>1</duration></backup>
              <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
                <voice>1</voice><staff>1</staff></note>
            </measure>
          </part></score-partwise>"""
        with open(p, "w", encoding="utf-8") as f:
            f.write(unisson)
        # Mesure 1 : voix 1+2 de la portée 1 fusionnent (1 attaque), la
        # portée 2 reste une piste distincte (2 attaques au total).
        # Mesure 2 : les deux C de la même voix fusionnent (1 attaque).
        attendu_unisson = [(60, 0.0, 1.0, 0), (60, 0.0, 1.0, 0),
                           (60, 1.0, 1.0, 0)]
        assert read_xml(p) == attendu_unisson, read_xml(p)

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

        # Grâces : `lily/audio-item.cc:118-129` place une grâce à
        # `main + 9/40 × grace_part` (`grace_part` négatif), donc strictement
        # avant l'attaque principale. L'appariement ne doit jamais prendre la
        # note principale de même hauteur.
        reference = [(60, 0.0, 1.0, 0), (60, -0.1, 0.0, 0)]
        restants, n_ref, n_test, matches = apparier_graces(
            reference, [(60, 0.0, False)]
        )
        assert matches == 1, (restants, matches)
        assert (60, 0.0, 1.0, 0) in restants, restants
        assert (60, -0.1, 0.0, 0) not in restants, restants
        assert n_ref == n_test == 1
        # Un candidat qui omet la grâce (une seule note principale) ne doit pas
        # devenir exact : la grâce manque réellement.
        manquant = compare(
            [(60, 0.0, 1.0, 0), (60, -0.1, 0.0, 0)],
            [(60, 0.0, 1.0, 0, False)],
            graces=[],
        )
        assert not manquant["exact"], manquant
        # `\afterGrace` : la grâce sonne après la note principale. Son attaque
        # MusicXML est le temps réel ; l'appariement doit prendre la note MIDI
        # de ce temps, pas la principale.
        reference = [(60, 0.0, 1.0, 0), (60, 0.9, 0.0, 0)]
        restants, _, _, matches = apparier_graces(
            reference, [(60, 0.9, True)]
        )
        assert matches == 1, (restants, matches)
        assert (60, 0.0, 1.0, 0) in restants, restants
        assert (60, 0.9, 0.0, 0) not in restants, restants

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
