#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon

"""Two SMFs with the same note events but a different order of note-on/off
at the SAME tick must yield the same note list. MuseScore 4.7.4 writes the
note-off before the note-on for a zero-length note (Adios_Muchachos.mscz,
tick 32, pitch 60); the FIFO in read_midi dropped the off when it came first,
giving a 1.75/2.0 pair instead of 0.0/1.75. Pairing must be order-independent
at equal tick.
"""

import os
import sys
import tempfile

import mido

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from midi_notes import read_midi  # noqa: E402


def make(path: str, order: str) -> None:
    mid = mido.MidiFile()
    mid.ticks_per_beat = 480
    track = mido.MidiTrack()
    mid.tracks.append(track)
    on = mido.Message("note_on", channel=0, note=60, velocity=80, time=0)
    off = mido.Message("note_off", channel=0, note=60, velocity=0, time=0)
    for msg in order:
        track.append(msg.copy())
    off2 = mido.Message("note_off", channel=0, note=60, velocity=0, time=480)
    track.append(off2)
    mid.save(path)


def main() -> None:
    on = mido.Message("note_on", channel=0, note=60, velocity=80, time=0)
    off = mido.Message("note_off", channel=0, note=60, velocity=0, time=0)
    with tempfile.TemporaryDirectory() as tmp:
        a = os.path.join(tmp, "on-first.mid")
        b = os.path.join(tmp, "off-first.mid")
        make(a, (on, off, on))
        make(b, (off, on, on))
        notes_a = read_midi(a)
        notes_b = read_midi(b)
        print("on-first ", notes_a)
        print("off-first", notes_b)
        assert sorted(notes_a) == sorted(notes_b), "same tick pairing is order-dependent"


if __name__ == "__main__":
    main()
