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

"""`sans_unfold` juge la première partition, pas le fichier entier.

`ATiTeCantamos` définit des `\\book` de répétition avec `\\unfoldRepeats` après
sa partition principale : le MIDI de référence (partition principale) ne
déplie pas la reprise, et l'oracle doit passer `--no-expand-repeats`. Un
`\\unfoldRepeats` dans la **première** partition, lui, doit empêcher ce
drapeau (le MIDI de référence déplie alors).
"""

import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mesure import premier_score, sans_unfold  # noqa: E402


def ecrire(dossier: str, nom: str, texte: str) -> str:
    chemin = os.path.join(dossier, nom)
    with open(chemin, "w", encoding="utf-8") as fichier:
        fichier.write(texte)
    return chemin


def main() -> None:
    with tempfile.TemporaryDirectory() as dossier:
        # `\unfoldRepeats` seulement dans un `\book` postérieur : non déplié.
        apres = ecrire(
            dossier,
            "apres.ly",
            "\\score { \\new Staff { \\repeat volta 2 { c'4 d' } } }\n"
            "\\book { \\score { \\unfoldRepeats \\new Staff { c'4 } } }\n",
        )
        assert sans_unfold(apres), "un fold d'un autre \\score ne compte pas"
        assert "\\unfoldRepeats" not in (premier_score(open(apres).read()) or "")

        # `\unfoldRepeats` dans la première partition : déplié.
        avant = ecrire(
            dossier,
            "avant.ly",
            "\\score { \\unfoldRepeats \\new Staff { \\repeat volta 2 { c'4 d' } } }\n"
            "\\book { \\score { \\new Staff { c'4 } } }\n",
        )
        assert not sans_unfold(avant), "le fold de la première partition compte"

        # Aucun `\score` : repli sur le fichier entier.
        sans = ecrire(dossier, "sans.ly", "musique = { c'4 }\n")
        assert sans_unfold(sans), "aucun fold dans le fichier"
    print("test_sans_unfold : OK")


if __name__ == "__main__":
    main()
