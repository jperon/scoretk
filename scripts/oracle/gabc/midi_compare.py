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

"""Compare scoretk's GABC → MIDI with the sandboxed landmark references.

Only pitches, their order and words are scored. Durations are *reported* but
never used as a pass/fail: GABC has no fixed durations (`Gabc.tex:801-845`)
and the landmark's note lengths come from its own neume-rhythm convention.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import re
import struct
import subprocess
import unicodedata

TAG = re.compile(r"<[^>]*>")
CLEF = re.compile(r"[cf]b?[1-9]")
SIGNS = set("'._~wvVs")


def vlq(data: bytes, pos: int) -> tuple[int, int]:
    value = 0
    while True:
        byte = data[pos]
        pos += 1
        value = (value << 7) | (byte & 0x7F)
        if not byte & 0x80:
            return value, pos


def read_midi(path: Path) -> dict:
    """Minimal standard MIDI file reader: notes, text metas, division."""
    data = path.read_bytes()
    if data[:4] != b"MThd":
        raise ValueError(f"not an SMF: {path}")
    _, tracks, division = struct.unpack(">HHH", data[8:14])
    pos = 14
    notes: list[tuple[int, int, int]] = []
    texts: list[tuple[int, str]] = []
    for _ in range(tracks):
        if data[pos : pos + 4] != b"MTrk":
            raise ValueError(f"missing MTrk: {path}")
        end = pos + 8 + int.from_bytes(data[pos + 4 : pos + 8], "big")
        pos += 8
        tick = 0
        running = None
        pending: dict[int, int] = {}
        while pos < end:
            delta, pos = vlq(data, pos)
            tick += delta
            status = data[pos]
            if status & 0x80:
                running = status
                pos += 1
            if running is None:
                raise ValueError(f"no running status: {path}")
            kind = running & 0xF0
            if running == 0xFF:
                meta = data[pos]
                pos += 1
                length, pos = vlq(data, pos)
                raw = data[pos : pos + length]
                pos += length
                try:
                    payload = raw.decode("utf-8")
                except UnicodeDecodeError:
                    # midiutil writes text metas in latin-1.
                    payload = raw.decode("latin-1")
                if meta in (1, 5):
                    texts.append((tick, payload))
            elif running in (0xF0, 0xF7):
                length, pos = vlq(data, pos)
                pos += length
            elif kind in (0x80, 0x90, 0xA0, 0xB0, 0xE0):
                note, velocity = data[pos], data[pos + 1]
                pos += 2
                if kind == 0x90 and velocity > 0:
                    pending[note] = tick
                elif kind == 0x80 or (kind == 0x90 and velocity == 0):
                    start = pending.pop(note, None)
                    if start is not None:
                        notes.append((start, tick - start, note))
            elif kind in (0xC0, 0xD0):
                pos += 1
            else:
                raise ValueError(f"unsupported status {running:#x}: {path}")
        pos = end
    notes.sort()
    texts.sort()
    return {"division": division, "notes": notes, "texts": texts}


def norm_word(text: str) -> str:
    text = TAG.sub("", text).replace("$", "")
    text = unicodedata.normalize("NFKD", text)
    text = "".join(char for char in text if not unicodedata.combining(char))
    return re.sub(r"\s+", "", text.strip().rstrip("-").strip().lower())


def first_divergence(a: list, b: list) -> int:
    for index, (left, right) in enumerate(zip(a, b)):
        if left != right:
            return index
    return min(len(a), len(b))


def source_facts(source: Path) -> dict:
    text = source.read_text(encoding="utf-8", errors="replace")
    body = text.split("%%", 1)[1] if "%%" in text else text
    found = CLEF.search(body)
    return {
        "clef": found.group(0) if found else "",
        "signs": sorted({char for char in SIGNS if char in body}),
    }


def compare_one(item: tuple[str, Path], args) -> dict:
    ident, source = item
    reference = args.refs / ident / "ref.mid"
    result = {"id": ident, "fichier": str(source), **source_facts(source)}
    if not reference.is_file():
        result["erreur"] = "missing reference MIDI"
        return result
    try:
        ref = read_midi(reference)
    except (ValueError, IndexError) as error:
        result["erreur"] = f"unreadable reference: {error}"
        return result
    scratch = args.scratch / ident
    scratch.mkdir(parents=True, exist_ok=True)
    output = scratch / "scoretk.mid"
    try:
        proc = subprocess.run(
            [str(args.cli), str(source), "-o", str(output),
             "--from", "gabc", "--to", "midi"],
            capture_output=True, timeout=args.timeout, check=False,
        )
        if proc.returncode != 0 or not output.exists():
            result["erreur"] = proc.stderr.decode("utf-8", "replace")[-300:]
            return result
        candidate = read_midi(output)
    except (subprocess.TimeoutExpired, ValueError, IndexError) as error:
        result["erreur"] = f"scoretk: {error}"
        return result

    ref_pitches = [note[2] for note in ref["notes"]]
    out_pitches = [note[2] for note in candidate["notes"]]
    ref_words = [norm_word(text) for _, text in ref["texts"]]
    out_words = [norm_word(text) for _, text in candidate["texts"]]
    ref_words = [word for word in ref_words if word]
    out_words = [word for word in out_words if word]
    ref_durations = [note[1] for note in ref["notes"]]
    out_durations = [note[1] for note in candidate["notes"]]
    ref_classes = [pitch % 12 for pitch in ref_pitches]
    out_classes = [pitch % 12 for pitch in out_pitches]
    # The landmark auto-transposes to a comfortable tessiture, so absolute
    # pitches are not comparable; consecutive intervals are.
    ref_intervals = [b - a for a, b in zip(ref_pitches, ref_pitches[1:])]
    out_intervals = [b - a for a, b in zip(out_pitches, out_pitches[1:])]
    result.update({
        "intervals_exact": ref_intervals == out_intervals,
        "first_interval_divergence": first_divergence(ref_intervals, out_intervals),
        "pitch_offset": (ref_pitches[0] - out_pitches[0]) if ref_pitches and out_pitches else None,
        "ref_notes": len(ref_pitches),
        "out_notes": len(out_pitches),
        "pitches_exact": ref_pitches == out_pitches,
        "pitch_classes_exact": ref_classes == out_classes,
        "first_pitch_divergence": first_divergence(ref_pitches, out_pitches),
        "first_class_divergence": first_divergence(ref_classes, out_classes),
        "words_exact": ref_words == out_words,
        "ref_words": ref_words[:12],
        "out_words": out_words[:12],
        "first_word_divergence": first_divergence(ref_words, out_words),
        # Informational only: see module docstring.
        "durations_exact": ref_durations == out_durations,
        "ref_division": ref["division"],
        "out_division": candidate["division"],
    })
    return result


def selftest() -> None:
    """The SMF reader must see one note and one (latin-1) lyric meta."""
    track = (
        b"\x00\xff\x05\x03Do-"
        b"\x00\x90\x3c\x64"
        b"\x83\x60\x80\x3c\x00"
        b"\x00\xff\x2f\x00"
    )
    smf = (
        b"MThd\x00\x00\x00\x06\x00\x01\x00\x01\x01\xe0"
        + b"MTrk" + len(track).to_bytes(4, "big") + track
    )
    scratch = Path("/tmp/gabc-midi-selftest.mid")
    scratch.write_bytes(smf)
    parsed = read_midi(scratch)
    assert parsed["notes"] == [(0, 480, 60)], parsed
    assert norm_word(parsed["texts"][0][1]) == "do", parsed
    scratch.unlink()
    print("midi_compare selftest: OK")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--selftest", action="store_true")
    p.add_argument("--manifest", type=Path)
    p.add_argument("--refs", type=Path)
    p.add_argument("--cli", type=Path)
    p.add_argument("--scratch", type=Path)
    p.add_argument("--jobs", type=int, default=2)
    p.add_argument("--timeout", type=int, default=30)
    p.add_argument("--limit", type=int, default=0)
    args = p.parse_args()
    if args.selftest:
        selftest()
        return
    if not args.manifest or not args.refs or not args.cli or not args.scratch:
        p.error("--manifest, --refs, --cli and --scratch are required")
    args.scratch.mkdir(parents=True, exist_ok=True)
    entrees = []
    for line in args.manifest.read_text(encoding="utf-8").splitlines():
        if line.strip() and not line.startswith("#"):
            ident, source = line.split("\t")
            entrees.append((ident.strip(), Path(source.strip())))
    if args.limit:
        entrees = entrees[: args.limit]
    with ThreadPoolExecutor(max_workers=max(1, min(args.jobs, 2))) as pool:
        lignes = list(pool.map(lambda it: compare_one(it, args), entrees))
    with (args.scratch / "comparaison.jsonl").open("w", encoding="utf-8") as handle:
        for ligne in lignes:
            handle.write(json.dumps(ligne, sort_keys=True, ensure_ascii=False) + "\n")
    valides = [l for l in lignes if "erreur" not in l]
    print(
        f"midi comparison: {len(valides)}/{len(lignes)} comparable | "
        f"pitches exact {sum(l['pitches_exact'] for l in valides)}/{len(valides)} | "
        f"pitch classes exact {sum(l['pitch_classes_exact'] for l in valides)}/{len(valides)} | "
        f"intervals exact {sum(l['intervals_exact'] for l in valides)}/{len(valides)} | "
        f"words exact {sum(l['words_exact'] for l in valides)}/{len(valides)} | "
        f"durations exact (informational) {sum(l['durations_exact'] for l in valides)}/{len(valides)}"
    )


if __name__ == "__main__":
    main()
