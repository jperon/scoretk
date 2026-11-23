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

"""Autotest de la normalisation symétrique des chaînes de notes contiguës.

Un MIDI ne distingue pas `sib4~ sib8` (une liaison écrite) de deux attaques
successives : il ne contient que des évènements note-on/note-off. L'oracle
doit donc fusionner les deux fragments **des deux côtés** quand l'autre côté
porte une note longue, sans toucher aux vraies répétitions que le MIDI sait
encoder. `test_ties_import.py` couvre les liaisons explicitement marquées.
"""

import tempfile
from pathlib import Path

from import_mesure import GRID, mscx_notes, notes
from midi_notes import reconcilier_chaines


def musicxml(notes_xml: str) -> str:
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Test</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes><divisions>1</divisions></attributes>
      {notes_xml}
    </measure>
  </part>
</score-partwise>
"""


DEUX_ATTAQUES = """<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice></note>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice></note>
<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice></note>"""

UNE_TENUE = """<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice></note>
<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice></note>"""

ATTENDU = [(60, 0.0, 2.0, 1), (62, 2.0, 1.0, 1)]


def mscx(chords: str) -> str:
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.00">
  <Score>
    <Part><Staff id="1"/></Part>
    <Staff id="1">
      <Measure>
        <voice>
          {chords}
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"""


DEUX_CHORDS = """<Chord><durationType>quarter</durationType><Note><pitch>60</pitch></Note></Chord>
<Chord><durationType>quarter</durationType><Note><pitch>60</pitch></Note></Chord>
<Chord><durationType>quarter</durationType><Note><pitch>62</pitch></Note></Chord>"""

UN_CHORD = """<Chord><durationType>half</durationType><Note><pitch>60</pitch></Note></Chord>
<Chord><durationType>quarter</durationType><Note><pitch>62</pitch></Note></Chord>"""


def main() -> None:
    with tempfile.TemporaryDirectory() as dossier:
        dossier = Path(dossier)
        deux = dossier / "deux.musicxml"
        deux.write_text(musicxml(DEUX_ATTAQUES), encoding="utf-8")
        tenue = dossier / "tenue.musicxml"
        tenue.write_text(musicxml(UNE_TENUE), encoding="utf-8")
        ms_deux = dossier / "deux.mscx"
        ms_deux.write_text(mscx(DEUX_CHORDS), encoding="utf-8")
        ms_tenue = dossier / "tenue.mscx"
        ms_tenue.write_text(mscx(UN_CHORD), encoding="utf-8")
        paires = [
            (notes(deux), notes(tenue)),
            (mscx_notes(ms_deux), mscx_notes(ms_tenue)),
        ]
    for reference, importation in paires:
        ref, got = reconcilier_chaines(reference, importation)
        assert ref == ATTENDU, ref
        assert got == ATTENDU, got

    # Deux vraies attaques des deux côtés : la règle ne doit rien fusionner.
    repetees = [(60, 0.0, 1.0, 1), (60, 1.0, 1.0, 1)]
    assert reconcilier_chaines(repetees, repetees) == (repetees, repetees)

    # Un écart de durée d'une tolérance entière n'est pas une tenue : ce sont
    # deux fragments dans deux voix, pas un accord et sa continuation.
    fragments = [(60, 0.0, 0.75, 1), (60, 0.75, 0.25, 2)]
    attention = [(60, 0.0, 0.75, 1), (60, 0.75, 0.25, 1)]
    assert reconcilier_chaines(fragments, attention) == (fragments, attention)

    # Une grâce (durée nulle après quantification) n'est pas une tenue.
    avec_grace = [(60, 0.5, 0.0, 1), (60, 1.0, 1.0, 1)]
    assert reconcilier_chaines(avec_grace, avec_grace) == (avec_grace, avec_grace)

    # Voix distinctes : pas de fusion, même à la même hauteur.
    deux_voix = [(60, 0.0, 1.0, 1), (60, 1.0, 1.0, 2)]
    assert reconcilier_chaines(deux_voix, deux_voix) == (deux_voix, deux_voix)

    assert reconcilier_chaines([(60, 0.0, GRID, 1)], [(60, 0.0, GRID, 1)]) == (
        [(60, 0.0, GRID, 1)],
        [(60, 0.0, GRID, 1)],
    )
    print("normalisation symétrique des chaînes : OK")


if __name__ == "__main__":
    main()
