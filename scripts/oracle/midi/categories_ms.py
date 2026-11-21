#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 jperon
"""Mesure du lot LilyPond contre l'import MIDI de MuseScore 4.7.4."""
from __future__ import annotations
import argparse, collections, json, sys
from pathlib import Path
import xml.etree.ElementTree as ET
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import common  # noqa: E402
from import_mesure import notes, score as note_score

CATS = ("pitch", "onset", "duration", "voice", "staff split", "tuplet", "clef", "key")


def partiel(ref, got, dim):
    if not ref:
        return 1.0
    rem = collections.Counter(n[dim] for n in got)
    found = 0
    for value in (n[dim] for n in ref):
        if rem[value]:
            rem[value] -= 1
            found += 1
    return found / len(ref)


def attrs(path):
    root = ET.parse(path).getroot()
    return {
        "tuplet": len(root.findall(".//time-modification")),
        "staff": len(root.findall("part")),
        "clef": len(root.findall(".//clef")),
        "key": len(root.findall(".//fifths")),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mesure", required=True)
    ap.add_argument("--ms", required=True)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    src, msdir, dst = Path(args.mesure), Path(args.ms), Path(args.out)
    dst.mkdir(parents=True, exist_ok=True)
    lignes = []
    for row in map(json.loads, (src / "import.jsonl").read_text().splitlines()):
        if "erreur" in row or common.est_exclu(row) or not row["id"].startswith("ly-"):
            continue
        ident = row["id"]
        safe = ident.replace("/", "_").replace(" ", "_")
        ref_path = msdir / f"{safe}.musicxml"
        got_path = src / "travail" / ident / "got.musicxml"
        if not ref_path.exists():
            continue
        ref = notes(ref_path)
        got = notes(got_path)
        a, b = attrs(ref_path), attrs(got_path)
        scores = {
            "pitch": partiel(ref, got, 0), "onset": partiel(ref, got, 1),
            "duration": partiel(ref, got, 2), "voice": partiel(ref, got, 3),
        }
        for cle, nom in [("staff", "staff split"), ("clef", "clef"), ("key", "key")]:
            scores[nom] = 1.0 if bool(a[cle]) == bool(b[cle]) else 0.0
        scores["tuplet"] = 1.0 if bool(a["tuplet"]) == bool(b["tuplet"]) else 0.0
        pire = min(CATS, key=lambda c: scores[c])
        lignes.append({
            "id": ident[3:], "ref": len(ref), "got": len(got),
            "score": round(note_score(ref, got), 4), "pire": pire,
            "scores": {k: round(v, 4) for k, v in scores.items()},
            "tup_ref": a["tuplet"], "tup_got": b["tuplet"],
        })
    (dst / "categories_ms.jsonl").write_text(
        "".join(json.dumps(l, ensure_ascii=False) + "\n" for l in lignes), encoding="utf-8")
    total = sum(l["ref"] * (1 - l["score"]) for l in lignes)
    weighted = sum(l["ref"] * l["score"] for l in lignes)
    notes_total = sum(l["ref"] for l in lignes)
    blocs = [
        f"# Lot LilyPond contre MuseScore 4.7.4 — {len(lignes)} fichiers",
        f"# N pondéré par les notes : {weighted / notes_total:.4f}",
        f"# Pertes : {total:.1f} notes sur {notes_total}",
        "", "| Catégorie | Fichiers | Pertes pondérées (notes) |",
        "| --- | ---: | ---: |",
    ]
    for cat in sorted(CATS, key=lambda c: -sum(
            l["ref"] * (1 - l["score"]) for l in lignes if l["pire"] == c)):
        subset = [l for l in lignes if l["pire"] == cat]
        loss = sum(l["ref"] * (1 - l["score"]) for l in subset)
        blocs.append(f"| {cat} | {len(subset)} | {loss:.1f} |")
    blocs += ["", "## 15 pires fichiers", "",
              "| Fichier | Notes MS | Notes nous | Score | Pire | pitch | onset | durée | voix |",
              "| --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: |"]
    for l in sorted(lignes, key=lambda l: l["ref"] * (1 - l["score"]), reverse=True)[:15]:
        s = l["scores"]
        blocs.append(
            f"| {l['id']} | {l['ref']} | {l['got']} | {l['score']:.4f} | {l['pire']} | "
            f"{s['pitch']:.2f} | {s['onset']:.2f} | {s['duration']:.2f} | {s['voice']:.2f} |")
    (dst / "categories_ms.md").write_text("\n".join(blocs) + "\n", encoding="utf-8")
    print("\n".join(blocs[:9]))


if __name__ == "__main__":
    main()
