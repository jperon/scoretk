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

"""Test de non-régression des liaisons dans l'oracle d'import MIDI.

MuseScore découpe une note tenue qui franchit un temps en fragments liés
(`<tie type="start"/>` … `<tie type="stop"/>`), et son `.mscx` attendu porte
un `<Tie><startElement>` sur chaque continuation. L'oracle doit fusionner la
chaîne en une seule note des deux côtés : sinon tout import qui écrit une
note unique perd une note par fragment, et la catégorie « pitch » compte du
découpage de notation au lieu d'erreurs de hauteur.
"""

import tempfile
from pathlib import Path

from import_mesure import mscx_notes, notes

MUSICXML = """<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Test</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes><divisions>1</divisions></attributes>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
        <tie type="start"/><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
        <tie type="stop"/><tie type="start"/><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
        <tie type="stop"/><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration>
        <voice>1</voice><type>quarter</type></note>
    </measure>
  </part>
</score-partwise>
"""

MSCX = """<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.00">
  <Score>
    <Part><Staff id="1"/></Part>
    <Staff id="1">
      <Measure>
        <voice>
          <Chord><durationType>quarter</durationType><Note><eid>A</eid><pitch>60</pitch></Note></Chord>
          <Chord><durationType>quarter</durationType><Note><eid>B</eid>
            <Tie><startElement>A</startElement><endElement>B</endElement></Tie>
            <pitch>60</pitch></Note></Chord>
          <Chord><durationType>quarter</durationType><Note><eid>C</eid>
            <Tie><startElement>B</startElement><endElement>C</endElement></Tie>
            <pitch>60</pitch></Note></Chord>
          <Chord><durationType>quarter</durationType><Note><eid>D</eid><pitch>62</pitch></Note></Chord>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"""

ATTENDU = [(60, 0.0, 3.0, 1), (62, 3.0, 1.0, 1)]


def main() -> None:
    with tempfile.TemporaryDirectory() as dossier:
        dossier = Path(dossier)
        xml = dossier / "test.musicxml"
        xml.write_text(MUSICXML, encoding="utf-8")
        mscx = dossier / "test.mscx"
        mscx.write_text(MSCX, encoding="utf-8")
        obtenu_xml = notes(xml)
        obtenu_mscx = mscx_notes(mscx)
    assert obtenu_xml == ATTENDU, obtenu_xml
    assert obtenu_mscx == ATTENDU, obtenu_mscx
    print("liaisons de l'oracle d'import : OK")


if __name__ == "__main__":
    main()
