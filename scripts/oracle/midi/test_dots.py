#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 jperon

"""Reduced test: mscx_notes must honor <dots> in durations.

Without this, a dotted quarter rest counts as one quarter and every later
onset in a compound meter is shifted.
"""
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from import_mesure import mscx_notes  # noqa: E402


MSCX = """<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.70"><Score><Staff id="1"><Measure>
  <voice>
    <Rest><dots>1</dots><durationType>quarter</durationType></Rest>
    <Chord><dots>1</dots><durationType>eighth</durationType>
      <Note><pitch>60</pitch></Note></Chord>
  </voice>
</Measure></Staff></Score></museScore>
"""


class DotsTest(unittest.TestCase):
    def test_pointed_durations(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "dot.mscx"
            path.write_text(MSCX, encoding="utf-8")
            notes = mscx_notes(path)
        self.assertEqual(len(notes), 1)
        pitch, onset, duration, voice = notes[0]
        self.assertEqual(pitch, 60)
        self.assertAlmostEqual(onset, 1.5)   # dotted quarter rest
        self.assertAlmostEqual(duration, 0.75)  # dotted eighth
        self.assertEqual(voice, 1)


if __name__ == "__main__":
    unittest.main()
