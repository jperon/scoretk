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

"""Comparateur MIDI (oracle abc2midi) / MusicXML (scoretk ou abc2xml.py).

N1 = multiset des hauteurs (comme cmp.py de l'oracle LilyPond).
N2 = multiset des couples (hauteur, attaque), attaque quantifiée au 1/24 de
noire. Les notes de grâce, les notes liées en fin de liaison et les notes
sans hauteur (percussion) sont écartées des deux côtés. Les transpositions
MusicXML <chromatic>/<octave-change> sont appliquées, comme dans cmp.py.

Aucune dépendance tierce : SMF et MusicXML sont analysés avec la bibliothèque
standard.
"""

import collections
import struct
import xml.etree.ElementTree as ET

STEP = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
QUANT = 24  # résolution des attaques : 1/24 de noire


def _varlen(data, pos):
    """Lit un entier de longueur variable SMF et renvoie (valeur, position)."""
    value = 0
    while True:
        byte = data[pos]
        pos += 1
        value = (value << 7) | (byte & 0x7F)
        if not byte & 0x80:
            return value, pos


def midi_notes(path):
    """Suite ordonnée des notes non liées : (tick, hauteur). Partagée avec
    ``sequence.py`` pour le diff de suite, sans dupliquer le parseur SMF."""
    data = open(path, "rb").read()
    if data[:4] != b"MThd":
        return []
    hlen = struct.unpack_from(">I", data, 4)[0]
    division = struct.unpack_from(">H", data, 12)[0]
    tpq = division if division > 0 else 480
    pos = 8 + hlen
    notes = []
    while data[pos : pos + 4] == b"MTrk":
        length = struct.unpack_from(">I", data, pos + 4)[0]
        p = pos + 8
        end = p + length
        tick = 0
        running = None
        while p < end:
            delta, p = _varlen(data, p)
            tick += delta
            byte = data[p]
            if byte < 0x80:
                status = running
            else:
                status = byte
                p += 1
                running = status if status < 0xF0 else None
            if status is None:
                break
            kind = status & 0xF0
            if status == 0xFF:
                p += 1
                length, p = _varlen(data, p)
                p += length
                continue
            if kind in (0xF0, 0xF7):
                length, p = _varlen(data, p)
                p += length
                continue
            if kind == 0x90 and data[p + 1] > 0:
                notes.append((tick, data[p]))
            p += 1 if kind in (0xC0, 0xD0) else 2
        pos = end
    return notes


def midi(path):
    """Renvoie (compteurs hauteur, compteur (hauteur, attaque), nombre de pistes)."""
    pitches = collections.Counter()
    attacks = collections.Counter()
    for tick, note in midi_notes(path):
        pitches[note] += 1
        attacks[(note, round(tick / tpq_global(path) * QUANT))] += 1
    return pitches, attacks, _midi_tracks(path)


def tpq_global(path):
    """Résolution SMF (mutualisée avec ``midi_notes``)."""
    data = open(path, "rb").read()
    division = struct.unpack_from(">H", data, 12)[0]
    return division if division > 0 else 480


def _midi_tracks(path):
    data = open(path, "rb").read()
    if data[:4] != b"MThd":
        return 0
    hlen = struct.unpack_from(">I", data, 4)[0]
    pos = 8 + hlen
    count = 0
    while data[pos : pos + 4] == b"MTrk":
        length = struct.unpack_from(">I", data, pos + 4)[0]
        pos += 8 + length
        count += 1
    return count


def musicxml_notes(path):
    """Suite ordonnée (temps, hauteur, numéro de mesure) jouée par le même
    parcours que ``musicxml`` ; utilisée par ``sequence.py``."""
    root = ET.parse(path).getroot()
    notes = []
    for part in root.iter("part"):
        divisions = 1
        transpose = 0
        time = 0.0
        for measure in part.findall("measure"):
            number = measure.get("number", "?")
            for element in measure:
                tag = element.tag
                if tag == "attributes":
                    value = element.findtext("divisions")
                    if value:
                        divisions = int(float(value))
                    node = element.find("transpose")
                    if node is not None:
                        transpose = int(float(node.findtext("chromatic", "0"))) + 12 * int(
                            float(node.findtext("octave-change", "0"))
                        )
                elif tag in ("backup", "forward"):
                    delta = int(float(element.findtext("duration", "0"))) / divisions
                    time += -delta if tag == "backup" else delta
                elif tag == "note":
                    if element.find("grace") is not None:
                        continue
                    tied_stop = any(t.get("type") == "stop" for t in element.findall("tie"))
                    duration = int(float(element.findtext("duration", "0"))) / divisions
                    node = element.find("pitch")
                    if not tied_stop and node is not None:
                        step = node.findtext("step")
                        octave = node.findtext("octave")
                        if step in STEP and octave is not None:
                            try:
                                pitch = (
                                    12 * (int(octave) + 1)
                                    + STEP[step]
                                    + round(float(node.findtext("alter", "0")))
                                    + transpose
                                )
                            except ValueError:
                                pitch = None
                            if pitch is not None:
                                notes.append((time, pitch, number))
                    if element.find("chord") is None:
                        time += duration
    return notes


def musicxml(path):
    """Renvoie les mêmes compteurs pour un MusicXML partwise."""
    pitches = collections.Counter()
    attacks = collections.Counter()
    for time, pitch, _measure in musicxml_notes(path):
        pitches[pitch] += 1
        attacks[(pitch, round(time * QUANT))] += 1
    return pitches, attacks


def compare(ref_midi, candidate_xml):
    """Mesure N1/N2 d'un candidat MusicXML contre un MIDI de référence."""
    ref_p, ref_a, tracks = midi(ref_midi)
    cand_p, cand_a = musicxml(candidate_xml)
    n_ref, n_cand = sum(ref_p.values()), sum(cand_p.values())
    common1 = sum((ref_p & cand_p).values())
    common2 = sum((ref_a & cand_a).values())
    # Correspondances à une octave près, pour distinguer octave et fausse note.
    octave = 0
    for pitch, count in (cand_p - ref_p).items():
        for shift in (12, -12, 24, -24):
            octave += min(count, ref_p.get(pitch + shift, 0))
    # Décalage chromatique dominant (détecte une transposition systématique).
    offsets = collections.Counter()
    for pitch, count in ref_p.items():
        for shift in range(-12, 13):
            other = cand_p.get(pitch + shift, 0)
            if other:
                offsets[shift] += min(count, other)
    shift, shift_notes = offsets.most_common(1)[0] if offsets else (0, 0)
    return {
        "ref": n_ref,
        "cand": n_cand,
        "com1": common1,
        "oct": octave,
        "shift": shift,
        "shift_notes": shift_notes,
        "n1": common1 / n_ref if n_ref else 0.0,
        "com2": common2,
        "n2": common2 / n_ref if n_ref else 0.0,
        "tracks": tracks,
        "exact1": n_ref > 0 and n_ref == n_cand and common1 == n_ref,
        "exact2": n_ref > 0 and n_ref == n_cand and common2 == n_ref,
    }
