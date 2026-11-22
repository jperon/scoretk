#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 jperon

"""First pitch-sequence divergence for the ABC *writing* oracle (model mode).

The writing oracle keeps, for each source file, `tk.musicxml` (the model as
exported directly, the reference) and `scoretk.musicxml` (the model after an
ABC round trip, the candidate).
Both are read with `cmp.musicxml_notes`; the first pitch difference is found
by `sequence.first_difference`. A second TSV `<sortie>.events.tsv` uses
`cmp.musicxml_events` and `sequence.first_event_difference` to localise the
first divergence of attack or duration. No parser is duplicated.

Usage: export_sequence.py <releve.jsonl> <dossier_travail> <lot> [sortie.tsv]
"""

import csv
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cmp  # noqa: E402
import sequence  # noqa: E402


def main():
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    releve, travail, lot = sys.argv[1:4]
    out = sys.argv[4] if len(sys.argv) > 4 else None
    rows = [json.loads(l) for l in open(releve, encoding='utf-8') if l.strip()]
    dest = open(out, 'w', encoding='utf-8') if out else sys.stdout
    dest.write('fichier\tindex\tref\tcand\tmesure\tcontexte\n')
    for index, row in enumerate(rows):
        if row.get('outil') != 'scoretk' or row.get('exact'):
            continue
        folder = os.path.join(travail, lot, str(index // 2))
        ref = os.path.join(folder, 'tk.musicxml')
        cand = os.path.join(folder, 'scoretk.musicxml')
        if not (os.path.exists(ref) and os.path.exists(cand)):
            continue
        a = cmp.musicxml_notes(ref)
        b = cmp.musicxml_notes(cand)
        diff = sequence.first_difference(a, b)
        if diff is None:
            continue
        i, j, va, vb = diff
        measure = b[j][2] if j < len(b) else '?'
        context = ' '.join(str(x[1]) for x in a[max(0, i - 2):i + 3])
        dest.write('\t'.join(str(v) for v in (
            os.path.basename(row['fichier']), index, va, vb, measure, context)) + '\n')
    if out:
        dest.close()
        events_path = out + '.events.tsv'
        with open(events_path, 'w', encoding='utf-8') as events:
            events.write('fichier\tindex\tgenre\tref\tcand\tmesure\tcontexte\n')
            for index, row in enumerate(rows):
                if row.get('outil') != 'scoretk' or row.get('exact'):
                    continue
                folder = os.path.join(travail, lot, str(index // 2))
                ref = os.path.join(folder, 'tk.musicxml')
                cand = os.path.join(folder, 'scoretk.musicxml')
                if not (os.path.exists(ref) and os.path.exists(cand)):
                    continue
                a = cmp.musicxml_events(ref)
                b = cmp.musicxml_events(cand)
                diff = sequence.first_event_difference(a, b)
                if diff is None:
                    continue
                i, kind, va, vb = diff
                measure = b[i][3] if i < len(b) else '?'
                context = ' '.join(str(x) for x in a[max(0, i - 2):i + 3])
                events.write('\t'.join(str(v) for v in (
                    os.path.basename(row['fichier']), index, kind, va, vb, measure, context)) + '\n')


if __name__ == '__main__':
    main()
