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

"""Test de non-régression de la lecture MusicXML de l'oracle : liaisons.

Une note tenue sur plusieurs mesures est écrite en MusicXML comme une chaîne
de notes liées (``<tie type="start"/>`` … ``<tie type="stop"/>``), une par
fragment de mesure. Le MIDI de référence, lui, ne connaît qu'une seule attaque
dont la durée couvre toute la tenue. L'ancien lecteur écartait les notes
``tie type="stop"`` : la durée n'était donc comptée que pour le premier
fragment, d'où les écarts ``duree_seule``. Le lecteur doit aux contraire
fusionner chaque chaîne en UNE note dont la durée est la somme des fragments,
en distinguant les chaînes par hauteur, voix et portée.
"""

import tempfile
from pathlib import Path

from midi_notes import read_xml


ATTRIBUTS = (
    '<attributes><divisions>1</divisions>'
    '<time><beats>4</beats><beat-type>4</beat-type></time></attributes>'
)


def _lire(xml: str):
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "ties.xml"
        path.write_text(xml, encoding="utf-8")
        return read_xml(str(path))


CHAINE = """<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">""" + ATTRIBUTS + """
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><tie type="start"/><voice>1</voice></note>
      <note><rest/><duration>1</duration><voice>1</voice></note>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice></note>
    </measure>
    <measure number="2">
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><tie type="stop"/><voice>1</voice></note>
      <note><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice></note>
    </measure>
  </part>
</score-partwise>
"""


def test_chaine_sur_barre_et_hauteur_non_concernee():
    """Une chaîne de deux fragments = une note de durée 4, pas 2.

    Le E4 non lié et le G4 de la seconde mesure gardent leur durée et leur
    attaque : une fusion trop large serait détectée.
    """
    assert _lire(CHAINE) == [
        (60, 0.0, 4.0, 0),  # C4 : attaque du premier fragment, durée 2 + 2
        (64, 3.0, 1.0, 0),  # E4 non lié
        (67, 6.0, 2.0, 0),  # G4, mesure 2
    ], _lire(CHAINE)


CHAINE_TROIS = """<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">""" + ATTRIBUTS + """
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><tie type="start"/><voice>1</voice></note>
      <note><rest/><duration>2</duration><voice>1</voice></note>
    </measure>
    <measure number="2">
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><tie type="stop"/><tie type="start"/><voice>1</voice></note>
      <note><rest/><duration>2</duration><voice>1</voice></note>
    </measure>
    <measure number="3">
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><tie type="stop"/><voice>1</voice></note>
      <note><rest/><duration>2</duration><voice>1</voice></note>
    </measure>
  </part>
</score-partwise>
"""


def test_chaine_de_trois_fragments():
    """Milieu de chaîne (stop + start) : une seule note de durée 6."""
    assert _lire(CHAINE_TROIS) == [(62, 0.0, 6.0, 0)], _lire(CHAINE_TROIS)


VOIX = """<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">""" + ATTRIBUTS + """
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><tie type="start"/><voice>1</voice></note>
      <backup><duration>4</duration></backup>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><tie type="start"/><voice>2</voice><staff>2</staff></note>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><voice>2</voice><staff>2</staff></note>
    </measure>
    <measure number="2">
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><tie type="stop"/><voice>1</voice></note>
      <backup><duration>4</duration></backup>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><tie type="stop"/><voice>2</voice><staff>2</staff></note>
      <note><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><voice>2</voice><staff>2</staff></note>
    </measure>
  </part>
</score-partwise>
"""


def test_chaines_paralleles_voix_et_portee():
    """Deux chaînes de même hauteur mais de voix/portées distinctes ne fusionnent pas."""
    notes = _lire(VOIX)
    assert notes == [
        (60, 0.0, 8.0, 0),  # voix 1 : C4 sur deux mesures
        (60, 0.0, 4.0, 0),  # voix 2 / portée 2 : C4 sur deux mesures
        (64, 2.0, 2.0, 0),
        (67, 6.0, 2.0, 0),
    ], notes


LIMITE = """<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">""" + ATTRIBUTS + """
      <note><pitch><step>A</step><octave>3</octave></pitch><duration>2</duration><voice>1</voice></note>
      <note><pitch><step>A</step><octave>3</octave></pitch><duration>2</duration><voice>1</voice></note>
    </measure>
    <measure number="2">
      <note><pitch><step>B</step><octave>3</octave></pitch><duration>4</duration><tie type="start"/><voice>1</voice></note>
    </measure>
  </part>
</score-partwise>
"""


def test_limites_repetition_sans_tie_et_tie_sans_fin():
    """Deux notes de même hauteur sans liaison restent distinctes ; un tie
    orphelin en fin de fichier garde sa propre durée."""
    notes = _lire(LIMITE)
    assert notes == [
        (57, 0.0, 2.0, 0),
        (57, 2.0, 2.0, 0),
        (59, 4.0, 4.0, 0),  # tie start sans stop : conservé tel quel
    ], notes


if __name__ == "__main__":
    test_chaine_sur_barre_et_hauteur_non_concernee()
    test_chaine_de_trois_fragments()
    test_chaines_paralleles_voix_et_portee()
    test_limites_repetition_sans_tie_et_tie_sans_fin()
    print("test_ties : OK")
