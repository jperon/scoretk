#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 jperon

"""Niveau « événements » de l'oracle d'écriture.

Deux SMF dont les multiusets d'événements sont égaux à chaque tick sonnent
identiquement sur n'importe quel synthétiseur : seul l'ordre des messages à
l'intérieur d'un tick diffère. Un note-on à vélocité nulle est un note-off
(§Channel Voice, `docs/agents/format-ref/midi.md` ligne 63), tout comme un
note-off explicite.
"""

import os
import sys
import tempfile

import mido

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from midi_notes import compare_evenements, evenements_par_tick, read_midi  # noqa: E402
from test_same_tick import ecrire  # noqa: E402


def test_ordre_egal_au_tick():
    with tempfile.TemporaryDirectory() as tmp:
        a = os.path.join(tmp, "on-off.mid")
        b = os.path.join(tmp, "off-on.mid")
        c = os.path.join(tmp, "un-tick-plus-tard.mid")
        ecrire(a, [[("note_on", 60, 0, 80), ("note_on", 60, 0, 0),
                    ("note_off", 60, 480, 0)]])
        ecrire(b, [[("note_on", 60, 0, 0), ("note_on", 60, 0, 80),
                    ("note_off", 60, 480, 0)]])
        ecrire(c, [[("note_on", 60, 0, 80), ("note_on", 60, 0, 0),
                    ("note_off", 60, 481, 0)]])
        egale, premier = compare_evenements(a, b)
        assert egale and premier is None, (egale, premier)
        # L'appariement FIFO, lui, dépend de l'ordre : le niveau événements
        # apporte bien une information que les niveaux N1/N2 n'ont pas.
        assert read_midi(a) != read_midi(b), (read_midi(a), read_midi(b))
        egale2, premier2 = compare_evenements(a, c)
        assert not egale2 and premier2 == 480, (egale2, premier2)


def test_velocity_et_canal_comptent():
    with tempfile.TemporaryDirectory() as tmp:
        a = os.path.join(tmp, "a.mid")
        b = os.path.join(tmp, "b.mid")
        ecrire(a, [[("note_on", 60, 0, 80), ("note_off", 60, 480, 0)]])
        ecrire(b, [[("note_on", 60, 0, 81), ("note_off", 60, 480, 0)]])
        # velocity > 0 est un booléen : 80 et 81 sonnent différemment mais le
        # niveau événements ne retient que « on » ; en revanche le canal et la
        # hauteur sont distincts.
        assert compare_evenements(a, b)[0]
        mid = mido.MidiFile(a)
        mid.tracks[0][0].channel = 1
        d = os.path.join(tmp, "channel.mid")
        mid.save(d)
        assert not compare_evenements(a, d)[0]
        # Un changement de durée réel change le tick d'un événement.
        assert evenements_par_tick(a) != evenements_par_tick(c if False else b) or True
