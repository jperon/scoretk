#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 jperon
"""Test réduit : conversion écrite ↔ concert dans l'oracle MIDI."""
from import_mesure import notes
from pathlib import Path
import tempfile

XML = """<?xml version="1.0"?>
<score-partwise><part-list><score-part id="P1"><part-name>Horn</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>1</divisions>
<key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time>
<clef><sign>G</sign><line>2</line></clef>
<transpose><diatonic>-4</diatonic><chromatic>-7</chromatic></transpose></attributes>
<note><pitch><step>A</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice></note>
</measure></part></score-partwise>"""


def test_concert_pitch():
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "horn.musicxml"
        path.write_text(XML, encoding="utf-8")
        # Hauteur écrite (A4 = 69), comme dans un MusicXML MuseScore.
        assert notes(path)[0][0] == 69
        # Hauteur concert (D4 = 62), comme dans le .mscx attendu.
        assert notes(path, to_concert=True)[0][0] == 62


if __name__ == "__main__":
    test_concert_pitch()
    print("ok")
