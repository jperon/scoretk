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
from midi_notes import (compare_evenements, evenements_audibles,  # noqa: E402
                        evenements_par_tick, read_midi)
from test_same_tick import ecrire  # noqa: E402


def test_ordre_egal_au_tick():
    with tempfile.TemporaryDirectory() as tmp:
        a = os.path.join(tmp, "on-off.mid")
        b = os.path.join(tmp, "off-on.mid")
        c = os.path.join(tmp, "un-tick-plus-tard.mid")
        ecrire(a, [[("note_on", 60, 0, 80), ("note_on", 60, 240, 0),
                    ("note_on", 60, 0, 80), ("note_off", 60, 480, 0)]])
        ecrire(b, [[("note_on", 60, 0, 80), ("note_on", 60, 240, 80),
                    ("note_on", 60, 0, 0), ("note_off", 60, 480, 0)]])
        ecrire(c, [[("note_on", 60, 0, 80), ("note_on", 60, 240, 0),
                    ("note_on", 60, 0, 80), ("note_off", 60, 481, 0)]])
        egale, premier, _, _ = compare_evenements(a, b)
        assert egale and premier is None, (egale, premier)
        # Les multiusets bruts sont égaux à chaque tick : c'est l'ordre des
        # messages qui diffère, pas le contenu audible.
        assert evenements_par_tick(a) == evenements_par_tick(b)
        egale2, premier2, _, _ = compare_evenements(a, c)
        assert not egale2 and premier2 is not None, (egale2, premier2)


def test_off_orphelin_sans_effet():
    """Un off sans note ouverte ne change pas ce qui sonne."""
    with tempfile.TemporaryDirectory() as tmp:
        a = os.path.join(tmp, "base.mid")
        b = os.path.join(tmp, "orphelin.mid")
        ecrire(a, [[("note_on", 60, 0, 80), ("note_off", 60, 480, 0)]])
        ecrire(b, [[("note_on", 60, 0, 80), ("note_off", 61, 0, 0),
                    ("note_off", 60, 480, 0)]])
        egale, premier, ref_orphelins, test_orphelins = compare_evenements(a, b)
        assert egale and premier is None, (egale, premier)
        assert ref_orphelins == 0 and test_orphelins == 1, (
            ref_orphelins, test_orphelins)


def test_off_coupe_une_note_sonnante():
    """Un off qui écourte une note reste une différence audible."""
    with tempfile.TemporaryDirectory() as tmp:
        a = os.path.join(tmp, "long.mid")
        b = os.path.join(tmp, "court.mid")
        ecrire(a, [[("note_on", 60, 0, 80), ("note_off", 60, 480, 0)]])
        ecrire(b, [[("note_on", 60, 0, 80), ("note_off", 60, 240, 0)]])
        egale, premier, _, _ = compare_evenements(a, b)
        assert not egale and premier == 240, (egale, premier)


def test_velocity_et_canal_comptent():
    with tempfile.TemporaryDirectory() as tmp:
        a = os.path.join(tmp, "a.mid")
        b = os.path.join(tmp, "b.mid")
        ecrire(a, [[("note_on", 60, 0, 80), ("note_off", 60, 480, 0)]])
        ecrire(b, [[("note_on", 60, 0, 81), ("note_off", 60, 480, 0)]])
        # velocity > 0 est un booléen : 80 et 81 sonnent différemment mais le
        # niveau événements ne retient que « on ».
        assert compare_evenements(a, b)[0]
        mid = mido.MidiFile(a)
        mid.tracks[0][0].channel = 1
        d = os.path.join(tmp, "channel.mid")
        mid.save(d)
        assert not compare_evenements(a, d)[0]
        # La magnitude de vélocité est volontairement ignorée (booléen on),
        # mais un changement de durée décale le tick de l'extinction audible.
        assert evenements_audibles(a)[0] == evenements_audibles(b)[0]
        c = os.path.join(tmp, "duree.mid")
        ecrire(c, [[("note_on", 60, 0, 80), ("note_off", 60, 600, 0)]])
        assert evenements_audibles(a)[0] != evenements_audibles(c)[0]
