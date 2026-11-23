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
# this program. If not, see <https://www.gnu.org/licenses/gpl-3.0.html>.

# `lily/audio-item.cc:122` : une grâce sonne à `main + 9/40 × grace_part` ;
# un groupe de plusieurs grâces peut donc insérer plus d'un temps avant sa
# note principale (`grace-auto-beam-engraver.ly` : 1,2 temps).

from ly_notes import (
    apparier_graces,
    decalage_graces_reference,
    lire_graces,
    retirer_temps_graces,
)


def test_groupe_de_graces_plus_long_qu_un_temps():
    reference = [
        (72, 34.80, 0.05, 0),
        (72, 34.90, 0.05, 0),
        (71, 35.00, 0.05, 0),
        (72, 36.00, 4.0, 0),
    ]
    graces = [(72, 36.0, False), (72, 36.0, False), (71, 36.0, False)]
    _, refs, tests, matched = apparier_graces(reference, graces, window=2.0)
    assert (refs, tests, matched) == (3, 3, 3), (refs, tests, matched)


def test_decalage_par_groupe():
    """Le temps inséré se calcule par groupe, depuis les durées écrites.

    `lily/audio-item.cc:118-129` : une grâce de durée écrite `d` sonne
    `9/40 × d` ronde, soit `0.9 × d` noire, avant sa note principale. Elle est
    découpée dans la note précédente, raccourcie ; seul l'excédent quand cette
    note ne suffit pas décale la suite :

        inséré = max(0, 0.9 × Σd − (attaque − début de la note précédente))

    `flags-default` : un 64e (0.0625 noire) précède un accord orné d'une
    croche (0.1125 réelle) -> 0.05 inséré.
    """
    notes = [
        (60, 0.0, 0.5, 0, 0, 0.0),
        (62, 0.5, 0.25, 0, 0, 0.0),
        (60, 0.75, 0.125, 0, 0, 0.0),
        (62, 0.875, 0.0625, 0, 0, 0.0),
        (60, 0.9375, 0.0, 0, 1, 0.125),  # grâce (croche)
        (62, 0.9375, 0.0625, 0, 0, 0.0),
    ]
    graces = lire_graces(notes)
    assert graces == [(60, 0.9375, False, 0.125, 0)], graces
    paliers = decalage_graces_reference([], graces, notes=notes)
    assert len(paliers) == 1, paliers
    seuil, cumul = paliers[0]
    assert abs(cumul - 0.05) < 1e-9, paliers
    assert abs(seuil - 0.9375) < 1e-9, paliers
    # La note précédente garde son attaque ; la note principale et la suite
    # sont ramenées à leur place logique.
    reference = [
        (62, 0.875, 0.0026, 0),
        (60, 0.875, 0.112, 0),
        (62, 0.9896, 0.0625, 0),
        (72, 1.0521, 0.5, 0),
    ]
    corrigee = retirer_temps_graces(reference, paliers)
    assert [round(note[1], 4) for note in corrigee] == [0.875, 0.875, 0.9396, 1.0021]

    # Une note précédente assez longue absorbe la grâce : rien n'est inséré.
    notes = [
        (60, 0.0, 1.0, 0, 0, 0.0),
        (60, 1.0, 0.0, 0, 1, 0.125),
        (62, 1.0, 1.0, 0, 0, 0.0),
    ]
    assert decalage_graces_reference([], lire_graces(notes), notes=notes) == []

    # Deux doubles croches forment un groupe de 0.125 : même total qu'une
    # croche, et la seconde note d'un accord d'agrément (durée écrite nulle,
    # `<chord/>`) ne compte pas deux fois.
    notes = [
        (62, 0.875, 0.0625, 0, 0, 0.0),
        (60, 0.9375, 0.0, 0, 1, 0.0625),
        (64, 0.9375, 0.0, 0, 1, 0.0625),
        (60, 0.9375, 0.0, 0, 1, 0.0),  # seconde tête de l'accord de grâce
        (62, 0.9375, 0.0625, 0, 0, 0.0),
    ]
    paliers = decalage_graces_reference([], lire_graces(notes), notes=notes)
    assert len(paliers) == 1 and abs(paliers[0][1] - 0.05) < 1e-9, paliers

    # Le seuil est un temps de référence : après un premier groupe, la note
    # précédente du second garde son attaque (déjà décalée) et ne reçoit pas
    # le temps inséré par sa propre grâce.
    notes = [
        (62, 0.875, 0.0625, 0, 0, 0.0),
        (60, 0.9375, 0.0, 0, 1, 0.125),
        (62, 0.9375, 0.0625, 0, 0, 0.0),
        (62, 1.875, 0.0625, 0, 0, 0.0),
        (60, 1.9375, 0.0, 0, 1, 0.125),
        (62, 1.9375, 0.0625, 0, 0, 0.0),
    ]
    paliers = decalage_graces_reference([], lire_graces(notes), notes=notes)
    assert len(paliers) == 2, paliers
    assert abs(paliers[1][0] - 1.9875) < 1e-9, paliers
    assert abs(paliers[1][1] - 0.1) < 1e-9, paliers


def test_appariement_par_position_attendue():
    """Chaque grâce est appariée à sa position réelle (`9/40 × durée écrite`).

    Avec deux croches de grâce, positions `main − 0.225` et `main − 0.1125`.
    Une note principale de même hauteur qui précède de peu ne doit pas être
    volée (`flags-default` : l'écart tombe entre la note tronquée et la
    grâce), et une note principale longue de même hauteur non plus
    (`grace-auto-beam-engraver` : 3,77 contre 0,11).
    """
    reference = [
        (60, 3.5, 0.5, 0),       # note principale précédente, même hauteur
        (60, 3.775, 0.1125, 0),  # grâce 1
        (60, 3.8875, 0.1125, 0), # grâce 2
        (60, 4.0, 2.0, 0),       # note principale
    ]
    graces = [(60, 4.0, False, 0.125), (60, 4.0, False, 0.125)]
    restants, refs, tests, matches = apparier_graces(reference, graces, window=2.5)
    assert (refs, tests, matches) == (2, 2, 2), (refs, tests, matches)
    assert (60, 3.5, 0.5, 0) in restants, restants
    assert (60, 4.0, 2.0, 0) in restants, restants


if __name__ == "__main__":
    test_groupe_de_graces_plus_long_qu_un_temps()
    test_decalage_par_groupe()
    test_appariement_par_position_attendue()
    print("ly_notes test_graces_groupe_long : OK")
