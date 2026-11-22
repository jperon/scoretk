#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 jperon

"""Reduced tests for the oracle's written-note preprocessing."""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mesure  # noqa: E402


def test_trille_apres_une_note():
    # ABC 2.1 §4.14: T est un agrément, jamais une hauteur ; abc2xml.py 269
    # `ornMap`/`uSyms` (lignes 1096-1097, 1115-1116) le mappe à `trill`.
    assert mesure.sans_ornements(["EATA>G|EATA>B|G6|"])[0] == "EAA>G|EAA>B|G6|"


def test_trille_en_debut_de_groupe():
    assert mesure.sans_ornements(["(gTfe) d2|"])[0] == "(gfe) d2|"


def test_trille_avec_coup_darchet():
    # `TvG3` : T est le trille, v le coup d'archet ; abc2midi réalise le trille.
    assert mesure.sans_ornements(["TvG3 BGB|"])[0] == "vG3 BGB|"


def test_champ_non_touche():
    assert mesure.sans_ornements(["M:4/4", "T:Titre", "TAc|"])[:2] == ["M:4/4", "T:Titre"]


if __name__ == '__main__':
    for name, function in sorted(globals().items()):
        if name.startswith('test_') and callable(function):
            function()
            print('OK', name)
