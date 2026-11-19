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

"""Test de non-régression de la lecture MusicXML de l'oracle.

Une voix n'apparaît parfois qu'après plusieurs mesures (typiquement après
une séparation de voix à l'import MIDI). Le temps d'une note est alors donné
par la position dans sa mesure, remise à zéro par ``<backup>``, et non par la
somme des durées de la voix depuis le début du morceau. L'ancien lecteur
additionnait les durées par voix sur tout le morceau et décalait toutes les
notes de la voix tardive : c'était la cause des « attaques décalées » du
relevé.
"""

import tempfile
from pathlib import Path

from midi_notes import read_xml

XML = """<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice></note>
    </measure>
    <measure number="2">
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice></note>
      <backup><duration>4</duration></backup>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><voice>2</voice></note>
    </measure>
    <measure number="3">
      <note><pitch><step>F</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice></note>
      <backup><duration>4</duration></backup>
      <note><pitch><step>G</step><octave>4</octave></pitch><duration>4</duration><voice>2</voice></note>
    </measure>
  </part>
</score-partwise>
"""


def test_voix_tardive_sur_backup():
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "voix.ly.xml"
        path.write_text(XML, encoding="utf-8")
        notes = read_xml(str(path))
    # (hauteur, attaque, durée, part) — la voix 2 commence à 4, pas à 0.
    assert notes == [
        (60, 0.0, 4.0, 0),  # C4, mesure 1, voix 1
        (62, 4.0, 4.0, 0),  # D4, mesure 2, voix 1
        (64, 4.0, 4.0, 0),  # E4, mesure 2, voix 2 (après backup)
        (65, 8.0, 4.0, 0),  # F4, mesure 3, voix 1
        (67, 8.0, 4.0, 0),  # G4, mesure 3, voix 2
    ], notes


if __name__ == "__main__":
    test_voix_tardive_sur_backup()
    print("test_backup : OK")
