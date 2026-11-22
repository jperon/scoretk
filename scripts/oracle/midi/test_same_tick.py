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

"""Extinctions sur une autre piste et unissons de la comparaison d'écriture.

MuseScore écrit une note d'unisson écartée comme un restrike de durée nulle
sur la piste de l'attaque précédente (`exportmidi.cpp:304-323`), et
l'extinction réelle sur la piste de la dernière attaque
(`EventsHolder::fixupMIDI`, `engraving/compat/midi/event.cpp:263-306`) : une
note peut donc commencer sur une piste et finir sur une autre. Un lecteur
piste par piste laissait la note ouverte jusqu'à une extinction lointaine.
`read_midi` fusionne désormais les pistes par canal, dans l'ordre du fichier.

`normaliser_ecriture` retire ensuite les restrikes (durée nulle) et fusionne
les unissons simultanés en gardant la durée la plus longue, comme MuseScore
(`fixupMIDI`) et LilyPond (`lily/midi-walker.cc:94-115`).
"""

import os
import sys
import tempfile

import mido

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from midi_notes import normaliser_ecriture, read_midi  # noqa: E402


def ajouter_notes(track, evenements):
    for type_, note, temps, velocite in evenements:
        track.append(
            mido.Message(type_, channel=0, note=note, velocity=velocite, time=temps)
        )


def ecrire(path, pistes):
    mid = mido.MidiFile()
    mid.ticks_per_beat = 480
    for evenements in pistes:
        track = mido.MidiTrack()
        mid.tracks.append(track)
        ajouter_notes(track, evenements)
    mid.save(path)


def test_meme_tick_sans_ordre():
    """Deux ordres d'un zéro et d'une note doivent donner la même écriture.

    `(on, off, on)` : la note nulle est écartée par la normalisation ; le
    second on dure jusqu'à l'extinction suivante. `(off, on, on)` : l'off
    initial ne ferme rien, les deux on s'ouvrent et le premier est fermé.
    """
    with tempfile.TemporaryDirectory() as tmp:
        a = os.path.join(tmp, "on-first.mid")
        b = os.path.join(tmp, "off-first.mid")
        ecrire(a, [[("note_on", 60, 0, 80), ("note_off", 60, 0, 0),
                    ("note_on", 60, 0, 80), ("note_off", 60, 480, 0)]])
        ecrire(b, [[("note_off", 60, 0, 0), ("note_on", 60, 0, 80),
                    ("note_on", 60, 0, 80), ("note_off", 60, 480, 0)]])
        notes_a = normaliser_ecriture(read_midi(a))
        notes_b = normaliser_ecriture(read_midi(b))
        assert notes_a == notes_b == [(60, 0.0, 1.0, 0)], (notes_a, notes_b)


def test_extinction_sur_une_autre_piste():
    """Un unisson sur deux pistes du même canal, comme l'export MuseScore.

    Les deux notes commencent au tick 0. `fixupMIDI` écrit le restrike de la
    première sur **sa** piste (tick 0, après l'attaque) et garde l'extinction
    réelle sur la piste de la dernière attaque. Un lecteur piste par piste
    avec les extinctions triées avant les attaques laissait la première note
    ouverte jusqu'à une extinction lointaine ; lue dans l'ordre du fichier,
    elle dure zéro et l'unisson se ramène à une seule note de 2 noires.
    """
    with tempfile.TemporaryDirectory() as tmp:
        chemin = os.path.join(tmp, "unisson.mid")
        ecrire(
            chemin,
            [
                [
                    ("note_on", 60, 0, 80),
                    ("note_off", 60, 0, 0),  # restrike d'unisson, après l'attaque
                ],
                [
                    ("note_on", 60, 0, 80),
                    ("note_off", 60, 960, 0),  # extinction réelle (2 noires)
                ],
            ],
        )
        notes = read_midi(chemin)
        assert [(n[0], n[1], n[2]) for n in notes] == [
            (60, 0.0, 0.0),
            (60, 0.0, 2.0),
        ], notes
        # Le restrike (durée nulle) disparaît, l'unisson garde sa durée.
        assert normaliser_ecriture(notes) == [(60, 0.0, 2.0, 1)]


def main() -> None:
    test_meme_tick_sans_ordre()
    test_extinction_sur_une_autre_piste()
    print("ok")


if __name__ == "__main__":
    main()
