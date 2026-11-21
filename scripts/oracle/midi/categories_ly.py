#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 jperon
"""Ventilation des pertes du lot LilyPond (deux exports MusicXML du modèle)."""
from __future__ import annotations
import argparse, collections, json
from pathlib import Path
import xml.etree.ElementTree as ET
from import_mesure import GRID, TOL, notes

CATS = ("voice", "duration", "onset", "pitch", "staff split", "tuplet", "clef", "key")

def partiel(ref, got, dim):
    if not ref: return 1.0
    rem = collections.Counter(n[dim] for n in got); found = 0
    for v in (n[dim] for n in ref):
        if rem[v]: rem[v] -= 1; found += 1
    return found / len(ref)

def attrs(path):
    root = ET.parse(path).getroot()
    out = {}
    out['tuplet'] = len(root.findall('.//time-modification'))
    out['staff'] = len(root.findall('part'))
    out['clef'] = len(root.findall('.//clef'))
    out['key'] = len(root.findall('.//fifths'))
    return out

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--mesure', required=True); ap.add_argument('--out', required=True)
    args = ap.parse_args(); src, dst = Path(args.mesure), Path(args.out)
    dst.mkdir(parents=True, exist_ok=True)
    lignes = []
    for row in map(json.loads, (src/'import.jsonl').read_text().splitlines()):
        if 'erreur' in row or not row['id'].startswith('ly-'): continue
        d = src/'travail'/row['id']
        ref = notes(d/'ref.musicxml'); got = notes(d/'got.musicxml')
        a, b = attrs(d/'ref.musicxml'), attrs(d/'got.musicxml')
        s = {'pitch': partiel(ref, got, 0), 'onset': partiel(ref, got, 1),
             'duration': partiel(ref, got, 2), 'voice': partiel(ref, got, 3)}
        for cle, nom in [('staff', 'staff split'), ('clef', 'clef'), ('key', 'key')]:
            s[nom] = 1.0 if bool(a[cle]) == bool(b[cle]) else 0.0
        s['tuplet'] = 1.0 if bool(a['tuplet']) == bool(b['tuplet']) else 0.0
        pire = min(CATS, key=lambda c: s[c])
        lignes.append({'id': row['id'][3:], 'ref': len(ref), 'score': row['score'],
                       'pire': pire, 'scores': {k: round(v,4) for k,v in s.items()},
                       'tup_ref': a['tuplet'], 'tup_got': b['tuplet']})
    (dst/'categories_ly.jsonl').write_text(''.join(json.dumps(l, ensure_ascii=False)+'\n' for l in lignes))
    total = sum(l['ref']*(1-l['score']) for l in lignes)
    blocs = ['# Ventilation des pertes du lot LilyPond', '',
             '| Catégorie | Fichiers | Pertes pondérées (notes) |', '| --- | ---: | ---: |']
    for cat in sorted(CATS, key=lambda c: -sum(l['ref']*(1-l['score']) for l in lignes if l['pire']==c)):
        fs = [l for l in lignes if l['pire']==cat]
        p = sum(l['ref']*(1-l['score']) for l in fs)
        blocs.append(f'| {cat} | {len(fs)} | {p:.1f} |')
    blocs += ['', '## 15 pires fichiers', '',
              '| Fichier | Notes | Score | Pire | pitch | onset | durée | voix | n-olets ref/got |',
              '| --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: |']
    for l in sorted(lignes, key=lambda l: l['ref']*(1-l['score']), reverse=True)[:15]:
        s = l['scores']
        blocs.append(f"| {l['id']} | {l['ref']} | {l['score']:.4f} | {l['pire']} | "
                     f"{s['pitch']:.2f} | {s['onset']:.2f} | {s['duration']:.2f} | {s['voice']:.2f} | "
                     f"{l['tup_ref']}/{l['tup_got']} |")
    (dst/'categories_ly.md').write_text('\n'.join(blocs)+'\n')
    print('\n'.join(blocs[:9])); print('total', round(total,1))

if __name__ == '__main__':
    main()
