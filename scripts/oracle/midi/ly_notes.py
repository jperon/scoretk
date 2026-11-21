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

r"""Extracteur minimal de notes d'une source LilyPond (oracle MIDI).

Sous-ensemble volontairement limité : noms de notes (anglais et allemand),
chiffres de durée, points, silences, accords `<...>`, liaisons `~`, barres `|`,
octaves relatives `'` / `,` et `\relative`. Les commandes `\...` inconnues sont
ignorées avec leur argument. Cela suffit aux deux fichiers `.mid`/`.ly`
appariés de `~/Musique` et donne la quantification idéale du plan-fiabilite.
"""

from __future__ import annotations

import re

SEMI = {"c": 0, "d": 2, "e": 4, "f": 5, "g": 7, "a": 9, "b": 11, "h": 11}
DUREES = {1: 4.0, 2: 2.0, 4: 1.0, 8: 0.5, 16: 0.25, 32: 0.125, 64: 0.0625}
MODIFICATEURS = {"es": -1, "s": -1, "is": 1, "fis": 1}
TOKEN = re.compile(r"[a-zA-Z]+|<<|>>|[0-9]+|['`,~|<>]|[^\s]")


def nettoie(texte: str) -> str:
    """Retire commentaires, chaînes et expressions Scheme `#(...)`."""
    texte = re.sub(r"%[^\n]*", " ", texte)
    texte = re.sub(r'"(?:[^"\\]|\\.)*"', " ", texte)
    texte = re.sub(r"#\([^)]*\)", " ", texte)
    return texte


def hauteur(nom: str, octave: int) -> int | None:
    """Nom de note LilyPond → hauteur MIDI (octave absolue après `\relative`)."""
    alter = 0
    base = nom
    for mod, delta in MODIFICATEURS.items():
        if base.endswith(mod) and len(base) > len(mod):
            alter = delta
            base = base[: -len(mod)]
            break
    if base not in SEMI:
        return None
    return 12 * (octave + 1) + SEMI[base] + alter


def notes(chemin) -> list[tuple[int, float, float, int]]:
    """Notes `(hauteur, attaque, durée, voix)` en noires, sur la grille Q16."""
    texte = nettoie(chemin.read_text(encoding="utf-8", errors="replace"))
    tokens = [m.group(0) for m in TOKEN.finditer(texte)]
    sorties: list[tuple[int, float, float, int]] = []
    index = 0
    relative = 4
    while index < len(tokens):
        token = tokens[index]
        index += 1
        if token == "\\relative":
            # `\relative c' { ... }` : relève l'octave de référence.
            if index < len(tokens):
                ref = hauteur(tokens[index], 4)
                if ref is not None:
                    relative = ref // 12 - 1
                    if "'" in tokens[index]:
                        relative += 1
                index += 1
            continue
        if token.startswith("\\"):
            # Argument d'une commande inconnue : consommé au plus un token.
            if index < len(tokens) and not tokens[index].startswith("\\") and tokens[index] not in ("{", "}"):
                index += 1
            continue
        if token in ("{", "}"):
            continue
        if token == "|":
            continue
        if token in ("r", "R", "s"):
            if index < len(tokens) and tokens[index].isdigit():
                index += 1
            continue
        if token == "<":
            chord: list[int] = []
            duree = 1.0
            while index < len(tokens) and tokens[index] != ">":
                t = tokens[index]
                index += 1
                p = hauteur(t, relative)
                if p is not None:
                    chord.append(p)
                elif t.isdigit() and int(t) in DUREES:
                    duree = DUREES[int(t)]
            if chord and index < len(tokens) and tokens[index].isdigit():
                duree = DUREES.get(int(tokens[index]), duree)
                index += 1
            for p in chord:
                sorties.append((p, 0.0, duree, 1))
            continue
        p = hauteur(token, relative)
        if p is None:
            continue
        duree = 1.0
        if index < len(tokens) and tokens[index].isdigit():
            duree = DUREES.get(int(tokens[index]), 1.0)
            index += 1
        if index < len(tokens) and tokens[index] == ".":
            duree *= 1.5
            index += 1
        sorties.append((p, 0.0, duree, 1))
    # Les attaques dépendent de l'ordre dans chaque portée : on les reconstruit
    # en cumulatif, en remettant le curseur à zéro à chaque changement de voix.
    resultat: list[tuple[int, float, float, int]] = []
    curseur = 0.0
    for pitch, _, duree, voix in sorties:
        resultat.append((pitch, round(curseur * 4) / 4, round(duree * 4) / 4, voix))
        curseur += duree
    return resultat
