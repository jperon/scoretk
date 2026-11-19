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

"""Appariement des notes superposées de même hauteur dans `read_midi`.

Le MIDI autorise plusieurs notes simultanées sur le même canal et la même
hauteur (`docs/agents/format-ref/midi.md`, §Channel Voice : les messages
`0x8`/`0x9` forment des paires note-on/note-off). Une table
``{ (canal, hauteur) : tick }`` écrase le note-on précédent et perd une note ;
le lecteur de MuseScore avance au contraire au premier note-off encore libre
(`midishared/midifile.cpp`, boucle après « no note-off for note »), soit un
appariement FIFO. Les tests ci-dessous construisent de petits SMF en mémoire.
"""

import os
import tempfile

import mido

from midi_notes import read_midi


def ecrire(messages: list[tuple[int, str, int, int]]) -> str:
    """Écrit un SMF format 0 ; les tuples sont (delta, type, note, vélocité)."""
    mid = mido.MidiFile(type=0)
    mid.ticks_per_beat = 480
    track = mido.MidiTrack()
    mid.tracks.append(track)
    for delta, kind, note, velocity in messages:
        track.append(
            mido.Message(kind, note=note, velocity=velocity, channel=0, time=delta)
        )
    handle, path = tempfile.mkstemp(suffix=".mid")
    os.close(handle)
    mid.save(path)
    return path


def test_superposition_meme_hauteur():
    # Deux notes C4 qui se chevauchent : 0→480 et 240→720 (noires 0→1 et
    # 0.5→1.5). Le premier note-off doit fermer la première note, pas la
    # seconde : sans FIFO, une seule note subsiste.
    path = ecrire([
        (0, "note_on", 60, 90),
        (240, "note_on", 60, 90),
        (240, "note_off", 60, 0),
        (240, "note_off", 60, 0),
    ])
    try:
        notes = read_midi(path)
    finally:
        os.unlink(path)
    assert sorted(notes) == [(60, 0.0, 1.0, 0), (60, 0.5, 1.0, 0)], notes


def test_non_concerne():
    # Deux notes successives de même hauteur, deux hauteurs simultanées et un
    # note-on de vélocité nulle : le comportement doit rester identique.
    path = ecrire([
        (0, "note_on", 60, 90),
        (240, "note_off", 60, 0),
        (240, "note_on", 60, 0),  # note-off déguisé, sans note ouverte
        (0, "note_on", 60, 90),
        (0, "note_on", 64, 80),
        (240, "note_on", 60, 0),
        (0, "note_off", 64, 0),
    ])
    try:
        notes = read_midi(path)
    finally:
        os.unlink(path)
    assert sorted(notes) == [(60, 0.0, 0.5, 0), (60, 1.0, 0.5, 0), (64, 1.0, 0.5, 0)], notes


def test_trois_notes_superposees():
    path = ecrire([
        (0, "note_on", 64, 90),
        (0, "note_on", 60, 90),
        (120, "note_on", 64, 90),
        (120, "note_off", 64, 0),
        (120, "note_off", 64, 0),
        (120, "note_on", 60, 0),
    ])
    try:
        notes = read_midi(path)
    finally:
        os.unlink(path)
    assert sorted(notes) == [
        (60, 0.0, 1.0, 0),
        (64, 0.0, 0.5, 0),
        (64, 0.25, 0.5, 0),
    ], notes


if __name__ == "__main__":
    test_superposition_meme_hauteur()
    test_non_concerne()
    test_trois_notes_superposees()
    print("test_overlap : OK")
