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

"""Une armure hors norme ne doit pas empêcher la lecture des notes.

`FF 59 02 sf mi` admet −7…+7 dièses/bémols (`docs/agents/format-ref/midi.md`,
§Meta-events). LilyPond écrit parfois 8 dièses (`Egmont`) ; `mido` lève
`KeySignatureError` à la lecture, ce qui faisait exclure le fichier de
l'oracle alors que l'armure ne sert pas à extraire les notes.
`common.lire_midi` complète la table de décodage et laisse les notes intactes.
"""

from __future__ import annotations

import struct
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import common  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent))

from midi_notes import read_midi  # noqa: E402


def _vlq(value: int) -> bytes:
    data = [value & 0x7F]
    value >>= 7
    while value:
        data.append(0x80 | (value & 0x7F))
        value >>= 7
    return bytes(reversed(data))


def _smf(key_signature: bytes, note: int = 60) -> bytes:
    track = b"".join(
        (
            b"\x00\xff\x59\x02" + key_signature,          # armure (8 dièses, mineur)
            b"\x00\x90" + bytes((note, 100)),             # note-on
            _vlq(480) + b"\x80" + bytes((note, 0)),       # note-off, une noire
            b"\x00\xff\x2f\x00",                          # fin de piste
        )
    )
    header = b"MThd" + struct.pack(">IHHH", 6, 0, 1, 480)
    return header + b"MTrk" + struct.pack(">I", len(track)) + track


def main() -> int:
    with tempfile.NamedTemporaryFile(suffix=".mid", delete=False) as handle:
        handle.write(_smf(b"\x08\x01"))
        chemin = handle.name
    try:
        notes = read_midi(chemin)
    finally:
        Path(chemin).unlink()
    print("notes lues :", notes)
    assert notes == [(60, 0.0, 1.0, 0)], notes

    # Une armure valide garde son décodage habituel.
    with tempfile.NamedTemporaryFile(suffix=".mid", delete=False) as handle:
        handle.write(_smf(b"\x00\x00"))
        chemin = handle.name
    try:
        notes = read_midi(chemin)
    finally:
        Path(chemin).unlink()
    assert notes == [(60, 0.0, 1.0, 0)], notes
    print("ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
