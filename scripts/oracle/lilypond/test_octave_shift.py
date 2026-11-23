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
# this program. If not, see <https://www.gnu.org/licenses/gpl-3.0.html>.

# `size` de `<octave-shift>` est un attribut (`musicxml-4.0.xsd:3925-3931`,
# défaut 8) : une 15ma doit décaler la hauteur jouée de deux octaves, pas
# d'une. `ottava.ly` et `ottavation-markups.ly` en dépendent.

import os
import tempfile

from ly_notes import read_xml

DOCUMENT = """<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes><divisions>1</divisions></attributes>
      <direction><direction-type><octave-shift type="down" size="15"/></direction-type></direction>
      <note><pitch><step>A</step><octave>7</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>
      <direction><direction-type><octave-shift type="stop" size="15"/></direction-type></direction>
    </measure>
  </part>
</score-partwise>
"""


def test_octave_shift_size_15():
    handle, path = tempfile.mkstemp(suffix=".musicxml")
    os.close(handle)
    try:
        with open(path, "w", encoding="utf-8") as fichier:
            fichier.write(DOCUMENT)
        # A7 = 105 ; la 15ma écrite au-dessus se lit `down` (le `<pitch>` est
        # la hauteur écrite, la hauteur jouée est deux octaves plus bas).
        assert read_xml(path) == [(81, 0.0, 1.0, 0)], read_xml(path)
    finally:
        os.unlink(path)


if __name__ == "__main__":
    test_octave_shift_size_15()
    print("ly_notes test_octave_shift : OK")
