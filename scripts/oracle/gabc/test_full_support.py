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

"""Tests of the GABC full-support instrument: dumps are errors, not facts.

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
import private_carriers as pc  # noqa: E402


FACTUS = (
    Path(__file__).resolve().parents[3]
    / "tests" / "corpus" / "gabc" / "gregorio_FactusEst.gabc"
)


class DumpCarrierTests(unittest.TestCase):
    """A source dump inside a full-support output fails the instrument."""

    def test_musicxml_private_fields_fail_loudly(self) -> None:
        xml = (b'<score-partwise><note><notations print-object="no">'
               b'<technical><other-technical print-object="no">letter: e'
               b'</other-technical><other-technical print-object="no">'
               b'glyph: virga augmentum</other-technical>'
               b'<other-technical print-object="no">gabc:note:l=e|g=w'
               b'</other-technical></technical></notations></note></score-partwise>')
        self.assertIsNotNone(fs.private_payload("musicxml", xml))
        self.assertTrue(pc.violations("musicxml", xml))

    def test_hidden_musicxml_text_fails_loudly(self) -> None:
        xml = (b'<score-partwise><direction print-object="no"><direction-type>'
               b'<other-direction print-object="no">plain hidden text'
               b'</other-direction></direction-type></direction></score-partwise>')
        self.assertTrue(pc.violations("musicxml", xml))

    def test_native_musicxml_carriers_survive(self) -> None:
        xml = (b'<score-partwise><identification><miscellaneous-field '
               b'name="mode">viii</miscellaneous-field></identification>'
               b'<note><pitch><step>E</step><octave>4</octave></pitch>'
               b'<lyric number="1"><syllabic>single</syllabic><text>FAc</text></lyric>'
               b'<notations><other-notation type="single" smufl="chantQuilisma"/>'
               b'</notations></note></score-partwise>')
        self.assertIsNone(fs.private_payload("musicxml", xml))

    def test_words_duplicating_a_lyric_fail_loudly(self) -> None:
        xml = (b'<score-partwise><measure><direction><direction-type>'
               b'<words font-style="italic" color="#FF0000">FAc</words>'
               b'</direction-type></direction>'
               b'<note><lyric><text>FAc</text></lyric></note>'
               b'</measure></score-partwise>')
        self.assertIsNotNone(fs.private_payload("musicxml", xml))

    def test_blank_words_fail_loudly(self) -> None:
        xml = (b'<score-partwise><measure><direction><direction-type>'
               b'<words font-style="italic" color="#0000FF"> </words>'
               b'</direction-type></direction></measure></score-partwise>')
        self.assertIsNotNone(fs.private_payload("musicxml", xml))

    def test_color_as_the_only_difference_between_carriers_fails_loudly(self) -> None:
        xml = (b'<score-partwise><measure><direction><direction-type>'
               b'<words font-style="italic" color="#FF0000">FAc</words>'
               b'<words font-style="italic" color="#0000FF"> </words>'
               b'</direction-type></direction></measure></score-partwise>')
        self.assertIn("couleur", fs.private_payload("musicxml", xml) or "")

    def test_printed_rubric_words_survive(self) -> None:
        # `<alt>` text really is printed above the staff: `<words>` is its
        # native XSD 4.0 carrier, with the printed value and no flag colour.
        xml = (b'<score-partwise><measure><direction placement="above">'
               b'<direction-type><words>Cantus</words></direction-type>'
               b'</direction><note><lyric><text>FAc</text></lyric></note>'
               b'</measure></score-partwise>')
        self.assertIsNone(fs.private_payload("musicxml", xml))

    def test_abc_annotations_fail_loudly(self) -> None:
        abc = (b'X:1\nT:k\nK:C\nI:chant=gregorian\nI:clef=c3\n'
               b'N:language=latin\nw: FAc\n"^text: FAc"\ng, |\n'
               b'"^glyph: "\nd^"^glyph: virga" |\n')
        self.assertTrue(pc.violations("abc", abc))

    def test_native_abc_lyrics_survive(self) -> None:
        abc = b'X:1\nT:k\nK:C clef=C3\nw: FAc\ng, |\n'
        self.assertIsNone(fs.private_payload("abc", abc))

    def test_lilypond_annotations_fail_loudly(self) -> None:
        ly = (b'\\new VaticanaVoice = "cantus" { g,^"glyph: " } '
              b'\\new VaticanaLyrics \\lyricsto "cantus" { "FAc" _ }')
        self.assertTrue(pc.violations("ly", ly))

    def test_native_lilypond_lyrics_survive(self) -> None:
        ly = (b'\\new VaticanaVoice = "cantus" { g, } '
              b'\\new VaticanaLyrics \\lyricsto "cantus" { "FAc" _ }')
        self.assertIsNone(fs.private_payload("ly", ly))

    def test_mei_gabc_labels_fail_loudly(self) -> None:
        mei = (b'<mei><meiHead label="gabc:layout:4332"/><music><syl '
               b'label="layout:sep:S|gabc:detail:4c">Po</syl></music></mei>')
        self.assertTrue(pc.violations("mei", mei))

    def test_smf_private_prefix_fails_loudly(self) -> None:
        smf = b'MThd\x00\x00\x00\x06\x00\x00\x00\x01\x00\x60MTrk\x00\x00\x00\x0b\x00\xff\x01\x08@ST:hello'
        self.assertTrue(pc.violations("midi", smf))


@unittest.skipUnless(os.environ.get("SCORETK_CLI"), "SCORETK_CLI is not set")
class FactusEstTests(unittest.TestCase):
    """The reviewer's file: no dump carrier in any measured destination."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.cli = Path(os.environ["SCORETK_CLI"]).expanduser()

    def test_every_destination_uses_a_native_lyric_carrier(self) -> None:
        source = FACTUS.read_bytes()
        native = {
            "musicxml": b"<lyric",
            "abc": b"\nw:",
            "ly": b"\\lyricsto",
            "mei": b"<syl",
            "midi": b"\xff\x05",
        }
        for destination, marker in native.items():
            with self.subTest(destination=destination):
                written, error = fs.convert_from(
                    self.cli, source, "gabc", destination,
                    fs.FULL_SUPPORT_EXTRA[destination],
                )
                self.assertEqual(error, "", f"{destination}: {error}")
                self.assertTrue(written, f"{destination}: empty output")
                self.assertIn(marker, written, f"{destination}: lyric carrier missing")
                self.assertIsNone(
                    fs.private_payload(destination, written),
                    f"{destination}: private carrier in the measured output",
                )

    def test_factus_est_has_no_disguised_words_carrier(self) -> None:
        # The reviewer found one italic red `<words>` per syllable in the
        # rejected c3c6919f carrier, plus italic blue blanks for separators.
        # FactusEst has no rubric for those syllables: the syllable spelling
        # is either a `<lyric>` fact or a defect, never a second printed text.
        written, error = fs.convert_from(
            self.cli, FACTUS.read_bytes(), "gabc", "musicxml",
            fs.FULL_SUPPORT_EXTRA["musicxml"],
        )
        self.assertEqual(error, "", error)
        self.assertIn(b"<lyric", written)
        self.assertNotIn(b'font-style="italic"', written)
        self.assertNotIn(b'color="#FF0000"', written)
        self.assertNotIn(b'color="#0000FF"', written)
        self.assertIsNone(fs.private_payload("musicxml", written))


if __name__ == "__main__":
    unittest.main()
