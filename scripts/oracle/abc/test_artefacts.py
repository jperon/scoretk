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

"""Autotest des trois causes d'artefacts du relevé ABC d'import.

`SANS_X` : abc2midi et abc2xml.py lisent un air sans `X:` (ABC 2.1 §2.2,
§3.1.1 : le champ est recommandé, pas obligatoire pour lire un air unique).
`ORACLE_ECHEC` : abc2midi exige une tonique dans le premier `K:` (§3.1.14),
alors que les deux lecteurs prennent do majeur par défaut ; on corrige seule
la copie destinée à l'oracle.
`ORACLE_VIDE` : une référence et un candidat tous deux sans notes sont égaux
par vacuité ; ils restent dans le dénominateur (`plan-fiabilite.md` §1.7).
"""

import os
import struct
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import cmp  # noqa: E402
import mesure  # noqa: E402


SANS_X = "L:1/4\nK:C\nCDEF|GABc|]\n"
SANS_K = "X:1\nT: Sans tonalité\nL:1/4\nCDEF|\n"
K_SANS_TONIQUE = "X:1\nK:cm\n.A.Bcd|\n"


def main() -> None:
    with tempfile.TemporaryDirectory() as dossier:
        dossier = os.path.join(dossier, "air.abc")
        with open(dossier, "w", encoding="utf-8") as handle:
            handle.write(SANS_X)
        header, tunes = mesure.decoupe(dossier)
        assert header == ["X:1"], header
        assert tunes == [("1", SANS_X.splitlines())], tunes

    sans_cle = mesure.cle_pour_oracle(SANS_K.splitlines())
    assert sans_cle == ["X:1", "T: Sans tonalité", "L:1/4", "K:C", "CDEF|"], sans_cle
    assert mesure.cle_pour_oracle(K_SANS_TONIQUE.splitlines())[1] == "K:C cm"
    assert mesure.cle_pour_oracle(["X:1", "K:C bass", "C2|"]) == [
        "X:1",
        "K:C bass",
        "C2|",
    ]
    assert mesure.cle_pour_oracle(["X:1", "K:G", "C2|"]) == ["X:1", "K:G", "C2|"]

    # Fins alternatives composées : la copie oracle ne doit garder que la
    # barre, sinon abc2midi ne joue qu'une partie des notes écrites.
    fins = " C8 |1 D8 :|2 E8 :|3 F8 ||"
    assert mesure.sans_reprises(fins.splitlines()) == [" C8 | D8 | E8 | F8 ||"]

    # Un MIDI sans évènement et un MusicXML sans note : les deux sont vides.
    piste = b"MTrk" + struct.pack(">I", 0)
    midi = b"MThd" + struct.pack(">IHHH", 6, 0, 1, 480) + piste
    xml = """<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Vide</part-name></score-part></part-list>
  <part id="P1"><measure number="1"><note><rest measure="yes"/>
    <duration>4</duration></note></measure></part>
</score-partwise>
"""
    with tempfile.TemporaryDirectory() as dossier:
        chemin_midi = os.path.join(dossier, "ref.mid")
        chemin_xml = os.path.join(dossier, "got.musicxml")
        with open(chemin_midi, "wb") as handle:
            handle.write(midi)
        with open(chemin_xml, "w", encoding="utf-8") as handle:
            handle.write(xml)
        relevé = cmp.compare(chemin_midi, chemin_xml)
    assert relevé["ref"] == relevé["cand"] == 0, relevé
    assert relevé["exact1"] and relevé["exact2"], relevé
    print("artefacts de l'oracle ABC : OK")


if __name__ == "__main__":
    main()
