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

"""Test de non-régression des identifiants du lot LilyPond.

Deux partitions de même nom existent dans des dossiers différents
(`SanctePieX` grégorien et `SanctePieX` des cantiques, trois `AStMichel`).
Avec un identifiant fondé sur le seul nom de fichier, la seconde conversion
et la référence MuseScore écrasaient la première : deux lignes étaient
comparées au même fichier, et la perte de l'une était comptée deux fois
(`ly-SanctePieX`, 82 notes fantômes). Chaque fichier doit donc recevoir un
identifiant unique, stable et déterministe.
"""

from pathlib import Path
from import_mesure import identifiants_lilypond


def main() -> None:
    chemins = [
        Path("/a/SanctePieX/SanctePieX.midi"),
        Path("/b/AStMichel/AStMichel.midi"),
        Path("/c/lily-abc.midi"),
        Path("/d/AStMichel/AStMichel.midi"),
        Path("/e/AStMichel/AStMichel.midi"),
    ]
    attendu = [
        ("ly-SanctePieX", chemins[0]),
        ("ly-AStMichel", chemins[1]),
        ("ly-lily-abc", chemins[2]),
        ("ly-AStMichel-2", chemins[3]),
        ("ly-AStMichel-3", chemins[4]),
    ]
    obtenu = identifiants_lilypond(chemins)
    assert obtenu == attendu, obtenu
    # Le premier identifiant reste celui des relevés antérieurs (référence
    # MuseScore déjà générée), et l'ordre ne dépend que de la liste d'entrée.
    assert obtenu == identifiants_lilypond(list(chemins))
    assert len({ident for ident, _ in obtenu}) == len(obtenu)
    print("identifiants LilyPond : OK")


if __name__ == "__main__":
    main()
