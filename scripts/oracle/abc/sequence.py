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
# FITNESS FOR A PARTICULAR PURPOSE. See the LICENSE file at the root of this
# repository or <https://www.gnu.org/licenses/gpl-3.0.html>.

"""Diff de suite de hauteurs pour l'oracle ABC.

L'oracle N1/N2 de ``cmp.py`` compare des *multisets* : il dit combien de notes
manquent, pas où. Ce script réutilise ses parseurs (``midi_notes``,
``musicxml_notes``) pour_aligner_ les suites ordonnées, puis rapporte la
première divergence avec le numéro de mesure et la ligne ABC correspondante.

Usage:
  sequence.py <releve.tsv> <dossier_brut> <cause|*> [sortie.tsv]

Une ligne par air : `id\tfichier\tx\tindex_ref\tindex_cand\tref\tcand\tmesure\tligne\tcontexte`.
Les airs sans divergence de suite (compteurs égaux mais ordre différent, ou
erreur de lecture) sont marqués `ALIGNE`/`INDISPONIBLE`.
"""

import collections
import csv
import difflib
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cmp  # noqa: E402  — parseurs mutualisés


def abc_lines(air):
    """Renvoie (lignes de musique, numéros) après le premier `K:` du corps."""
    lines = open(air, encoding="utf-8", errors="replace").read().splitlines()
    start = 0
    for index, line in enumerate(lines):
        if line.startswith("K:"):
            start = index + 1
            break
    return [(index + 1, line) for index, line in enumerate(lines[start:], start)
            if line.strip() and not line.lstrip().startswith('%')
            and not line.startswith(('X:', 'T:', 'C:', 'M:', 'L:', 'Q:', 'P:',
                                     'N:', 'Z:', 'V:', 'I:', 'U:', '%%', 'K:'))]


def abc_note_lines(air):
    """(numéro de ligne, index de note) pour chaque tête écrite du corps ABC.

    Analyse légère : compte les lettres A-G/a-g hors noms de décoration
    `!…!`, ce qui suffit pour localiser une note dans sa ligne source.
    """
    result = []
    for number, line in abc_lines(air):
        skip = False
        for char in line:
            if char == '!':
                skip = not skip
                continue
            if not skip and char in 'ABCDEFGabcdefg':
                result.append((number, len(result)))
    return result


def abc_token_at(air, index):
    """Renvoie (ligne, extrait) autour de la `index`-ième tête du corps."""
    for number, line in abc_lines(air):
        positions = [i for i, char in enumerate(line)
                     if char in 'ABCDEFGabcdefg']
        for pos in positions:
            if abc_token_at.seen < index:
                abc_token_at.seen += 1
                continue
            return number, line[max(0, pos - 8):pos + 12]
    return 0, ''


abc_token_at.seen = 0


def bar_lines(air):
    """Associe chaque numéro de mesure à la ligne ABC où il commence."""
    result = {}
    bar = 1
    for number, line in abc_lines(air):
        result[bar] = (number, line)
        bar += sum(line.count(marker) for marker in ('|', ':')) // 2
    return result


def first_difference(reference, candidate):
    """Premier bloc modifié entre deux suites de hauteurs."""
    a = [item[1] for item in reference]
    b = [item[1] for item in candidate]
    matcher = difflib.SequenceMatcher(None, a, b, autojunk=False)
    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == 'equal':
            continue
        # Un bloc `replace` peut commencer par deux notes égales : avancer
        # jusqu'à la première hauteur réellement différente.
        for offset in range(max(i2 - i1, j2 - j1)):
            ia, ib = i1 + offset, j1 + offset
            va = a[ia] if ia < i2 else None
            vb = b[ib] if ib < j2 else None
            if va != vb:
                return ia, ib, va, vb
    return None


def main():
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    tsv, brut, cause = sys.argv[1:4]
    out = sys.argv[4] if len(sys.argv) > 4 else None
    rows = []
    with open(tsv, encoding="utf-8") as handle:
        for row in csv.DictReader(handle, delimiter='\t'):
            if row['candidat'] != 'scoretk':
                continue
            if cause != '*' and row['cause'] != cause:
                continue
            folder = os.path.join(brut, row['id'])
            air = os.path.join(folder, 'air.abc')
            ref = os.path.join(folder, 'ref.mid')
            xml = os.path.join(folder, 'score.musicxml')
            if not all(os.path.exists(p) for p in (air, ref, xml)):
                continue
            reference = cmp.midi_notes(ref)
            candidate = cmp.musicxml_notes(xml)
            diff = first_difference(reference, candidate)
            if diff is None:
                rows.append((row['id'], row['fichier'], row['x'], 'ALIGNE',
                             '', '', '', '', '', ''))
                continue
            i, j, rp, cp = diff
            number = candidate[j][2] if j < len(candidate) else '?'
            lines = bar_lines(air)
            wanted = int(number) if str(number).isdigit() else 1
            key = max((k for k in lines if k <= wanted), default=1)
            source_line, text = lines.get(key, (0, ''))
            abc_token_at.seen = 0
            source_line, context = abc_token_at(air, j)
            rows.append((row['id'], row['fichier'], row['x'], i, j, rp, cp,
                         number, source_line, context))
    dest = open(out, 'w', encoding='utf-8') if out else sys.stdout
    dest.write('id\tfichier\tx\tindex_ref\tindex_cand\tref\tcand\tmesure\tligne\tcontexte\n')
    for row in rows:
        dest.write('\t'.join(str(v) for v in row) + '\n')
    if out:
        dest.close()


if __name__ == '__main__':
    main()


def first_event_difference(reference, candidate):
    """Première divergence d'attaque, de durée ou de hauteur entre deux suites.

    Les suites sont des tuples ``(temps, durée, hauteur, mesure)`` produits par
    :func:`cmp.musicxml_events`. Renvoie ``(index, genre, ref, cand)`` ou None.
    """
    for index in range(min(len(reference), len(candidate))):
        a, b = reference[index], candidate[index]
        if a[0] != b[0]:
            return index, 'attaque', a[0], b[0]
        if a[1] != b[1]:
            return index, 'duree', a[1], b[1]
        if a[2] != b[2]:
            return index, 'hauteur', a[2], b[2]
    if len(reference) != len(candidate):
        return min(len(reference), len(candidate)), 'compte', len(reference), len(candidate)
    return None
