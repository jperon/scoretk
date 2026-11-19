#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon
#
# This program is free software: you can redistribute it and/or modify it
# under the terms of the GNU General Public License version 3 as published by
# the Free Software Foundation.
#
# Same FIFO pairing check as `scripts/oracle/midi/test_overlap.py`, for the
# MIDI reference reader of this oracle (`ly_notes.py::read_midi`).

import os
import tempfile

import mido

from ly_notes import read_midi


def test_overlap():
    mid = mido.MidiFile(type=0)
    mid.ticks_per_beat = 480
    track = mido.MidiTrack()
    for delta, kind, note, velocity in [
        (0, "note_on", 60, 90),
        (240, "note_on", 60, 90),
        (240, "note_off", 60, 0),
        (240, "note_off", 60, 0),
    ]:
        track.append(mido.Message(kind, note=note, velocity=velocity, channel=0, time=delta))
    mid.tracks.append(track)
    handle, path = tempfile.mkstemp(suffix=".mid")
    os.close(handle)
    mid.save(path)
    try:
        assert sorted(read_midi(path)) == [
            (60, 0.0, 1.0, 0),
            (60, 0.5, 1.0, 0),
        ], read_midi(path)
    finally:
        os.unlink(path)


if __name__ == "__main__":
    test_overlap()
    print("ly_notes test_overlap : OK")
