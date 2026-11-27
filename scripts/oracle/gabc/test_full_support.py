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

"""Tests of the GABC full-support instrument: dump carriers are not facts.

Run the unit tests alone with::

    python3 -m unittest scripts.oracle.gabc.test_full_support

and add the real FactusEst check with a built CLI::

    SCORETK_CLI=/path/to/scoretk python3 -m unittest \
        scripts.oracle.gabc.test_full_support -v
"""

from __future__ import annotations

import os
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import full_support as fs  # noqa: E402


FACTUS = (
    Path(__file__).resolve().parents[3]
    / "tests" / "corpus" / "gabc" / "gregorio_FactusEst.gabc"
)


class DumpCarrierTests(unittest.TestCase):
    """A generic `key: value` carrier is ignored, never counted."""

    def test_musicxml_labels_are_stripped_before_measurement(self) -> None:
        xml = (b'<score-partwise><note><notations print-object="no">'
               b'<technical><other-technical print-object="no">letter: e'
               b'</other-technical><other-technical print-object="no">'
               b'glyph: virga augmentum</other-technical>'
               b'<other-technical print-object="no">gabc:note:l=e|g=w'
               b'</other-technical></technical></notations></note></score-partwise>')
        self.assertIsNotNone(fs.private_payload("musicxml", xml))
        stripped = fs.strip_private("musicxml", xml)
        self.assertNotIn(b"letter:", stripped)
        self.assertNotIn(b"glyph:", stripped)
        self.assertNotIn(b"gabc", stripped)
        self.assertIsNone(fs.private_payload("musicxml", stripped))

    def test_native_musicxml_carriers_survive(self) -> None:
        xml = (b'<score-partwise><note><pitch><step>E</step><octave>4</octave></pitch>'
               b'<lyric number="1"><syllabic>single</syllabic><text>FAc</text></lyric>'
               b'<notations><other-notation type="single" smufl="chantQuilisma"/>'
               b'</notations></note></score-partwise>')
        stripped = fs.strip_private("musicxml", xml)
        self.assertIn(b"<lyric", stripped)
        self.assertIn(b"chantQuilisma", stripped)
        self.assertIsNone(fs.private_payload("musicxml", stripped))

    def test_abc_annotations_are_stripped_but_w_lines_survive(self) -> None:
        abc = (b'X:1\nT:k\nK:C\nI:chant=gregorian\nI:clef=c3\n'
               b'N:language=latin\nw: FAc\n"^text: FAc"\ng, |\n'
               b'"^glyph: "\nd^"^glyph: virga" |\n')
        stripped = fs.strip_private("abc", abc)
        self.assertIn(b"w: FAc", stripped)
        self.assertNotIn(b"^text:", stripped)
        self.assertNotIn(b"^glyph:", stripped)
        self.assertNotIn(b"I:clef=", stripped)
        self.assertIsNone(fs.private_payload("abc", stripped))

    def test_lilypond_annotations_are_stripped_but_lyrics_survive(self) -> None:
        ly = (b'\\new VaticanaVoice = "cantus" { g,^"glyph: " } '
              b'\\new VaticanaLyrics \\lyricsto "cantus" { "FAc" _ }')
        stripped = fs.strip_private("ly", ly)
        self.assertIn(b"\\lyricsto", stripped)
        self.assertIn(b'"FAc"', stripped)
        self.assertNotIn(b'^"glyph:', stripped)
        self.assertIsNone(fs.private_payload("ly", stripped))

    def test_empty_annotation_fails_loudly_when_not_stripped(self) -> None:
        self.assertIsNotNone(fs.private_payload("ly", b'g,^"glyph: "'))
        self.assertIsNotNone(fs.private_payload("abc", b'""\ng, |'))


@unittest.skipUnless(os.environ.get("SCORETK_CLI"), "SCORETK_CLI is not set")
class FactusEstTests(unittest.TestCase):
    """The reviewer's file: no dump carrier in any measured destination."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.cli = Path(os.environ["SCORETK_CLI"]).expanduser()

    def test_every_destination_uses_a_native_lyric_carrier(self) -> None:
        source = FACTUS.read_bytes()
        native = {
            "musicxml": (b"<lyric", (b"letter: ", b"glyph: ", b"syllable-text: ")),
            "abc": (b"\nw:", (b'"^',)),
            "ly": (b"\\lyricsto", (b'^"',)),
            "mei": (b"<syl", (b"chantNote",)),
        }
        for destination, (marker, forbidden) in native.items():
            with self.subTest(destination=destination):
                written, error = fs.convert_from(
                    self.cli, source, "gabc", destination,
                    fs.FULL_SUPPORT_EXTRA[destination],
                )
                self.assertEqual(error, "", f"{destination}: {error}")
                self.assertTrue(written, f"{destination}: empty output")
                self.assertIn(marker, written, f"{destination}: lyric carrier missing")
                # The historical private `gabc:` fields of `master` are still
                # written (default bytes must stay identical); the instrument
                # strips them. No *new* readable-label dump may be written.
                for dump in forbidden:
                    self.assertNotIn(dump, written, f"{destination}: {dump!r} dump")
                stripped = fs.strip_private(destination, written)
                self.assertIsNone(
                    fs.private_payload(destination, stripped),
                    f"{destination}: dump carrier in the measured output",
                )


if __name__ == "__main__":
    unittest.main()
