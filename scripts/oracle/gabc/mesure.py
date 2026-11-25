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

"""GABC oracle: Gregorio is the only reference.

GABC separates *sections*, not metrical measures: Gregorio's syntax defines
no time signature, no measure numbering and no fixed note duration
(`doc/Gabc.tex:742-845`). MIDI/XML onset comparisons therefore cannot be a
sanity test. The truth is defined by :mod:`canonical`: pitch **with the clef
applied**, syllable text and separation bars; rhythmic signs are optional N2.

Two measurements are made, both against Gregorio's own parser:

* Gregorio against itself (two runs, SHA-256 of the dumps) — must be 100 %;
* our own reader of the source against ``gregorio -f gabc -F dump`` — the
  remaining divergences are ours to fix, classified by failing check.

The downloaded source is data; only the recorded Gregorio binary is run,
in its own build directory. Nothing here is a converter under test yet.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import canonical  # noqa: E402

SELFGABC = """name: Selftest;
language: latin;
staff-lines: 4;
%%

(c4) DE(g)us(h) in(g) adjutórium(fg.) (;) me(g)um(h) (::)
"""


def lire_liste(path: Path) -> list[tuple[str, Path]]:
    """Read ``id<TAB>path``, skipping blanks and comments."""
    out = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) != 2:
            raise ValueError(f"manifest line invalid: {line!r}")
        out.append((parts[0].strip(), Path(parts[1].strip())))
    return out


def _sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else ""


def _run(cmd: list[str], timeout: int = 120) -> tuple[int, str]:
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return 124, "timeout"
    return proc.returncode, (proc.stderr or proc.stdout)[-400:]


def _last_error(err: str) -> str:
    lines = [l.strip() for l in err.splitlines() if l.strip()]
    return " | ".join(lines[-2:])[:260] if lines else ""


def gregorio_ref(binary: Path, src: Path, work: Path, name: str = "ref") -> dict:
    """Run Gregorio's parser once and return its canonical dump."""
    work.mkdir(parents=True, exist_ok=True)
    dump = work / f"{name}.dump"
    code, err = _run(
        [str(binary), "-f", "gabc", "-F", "dump", str(src), "-o", str(dump)]
    )
    if code != 0 or not dump.exists():
        return {"ok": False, "code": code, "erreur": _last_error(err)}
    return {"ok": True, "dump": dump, "sha": _sha(dump)}


def measure_one(item: tuple[str, Path], args) -> dict:
    ident, src = item
    work = args.out / "travail" / ident
    releve = {"id": ident, "fichier": str(src)}

    ref = gregorio_ref(args.gregorio, src, work)
    if not ref["ok"]:
        releve.update(ref)
        return releve
    releve["sha_dump"] = ref["sha"]
    canon_ref = canonical.parse_dump(ref["dump"].read_text(encoding="utf-8", errors="replace"))

    if args.verifier_determinisme:
        again = gregorio_ref(args.gregorio, src, work, "again")
        releve["gregorio_deterministe"] = bool(again.get("ok") and again["sha"] == ref["sha"])

    source = canonical.parse_source(src.read_text(encoding="utf-8", errors="replace"))
    releve["cmp_source"] = canonical.compare(source, canon_ref)
    releve["divergences"] = [
        k for k, ok in releve["cmp_source"].items()
        if k in ("pitch_multiset", "pitch_sequence", "bars", "text", "signs") and not ok
    ]

    # Optional converter measurement: run scoretk (GABC -> model -> GABC) and
    # let Gregorio parse the result. The reference stays Gregorio's own dump
    # of the *original* file; comparing two dumps avoids measuring our Python
    # source reader's own limitations. Only our built CLI is executed here.
    if args.writer:
        converted = work / "scoretk.gabc"
        code, err = _run([
            str(args.writer), str(src), "-o", str(converted),
            "--from", "gabc", "--to", "gabc",
        ])
        if code != 0 or not converted.exists():
            releve["erreur_writer"] = _last_error(err)
            return releve
        writer_ref = gregorio_ref(args.gregorio, converted, work, "writer")
        if not writer_ref["ok"]:
            releve["erreur_writer"] = writer_ref["erreur"]
            return releve
        writer_canon = canonical.parse_dump(
            writer_ref["dump"].read_text(encoding="utf-8", errors="replace")
        )
        releve["cmp_writer"] = canonical.compare(canon_ref, writer_canon)
        releve["writer_dump_identique"] = writer_ref["sha"] == ref["sha"]
    return releve


def selftest(args) -> None:
    tmp = args.out / "selftest"
    tmp.mkdir(parents=True, exist_ok=True)
    src = tmp / "selftest.gabc"
    src.write_text(SELFGABC, encoding="utf-8")
    ref = gregorio_ref(args.gregorio, src, tmp / "ref")
    assert ref["ok"], f"Gregorio failed selftest: {ref}"
    a = canonical.parse_dump(ref["dump"].read_text(encoding="utf-8"))
    b = canonical.parse_source(SELFGABC)
    result = canonical.compare(b, a)
    assert result["core_exact"], result
    # A Dominican bar must not disappear from the canonical form: its digit
    # is the only difference between `;` and `;1` (`src/struct.h:158`).
    dominican = canonical.parse_dump("bar 6 (B_DIVISIO_MINOR_D1)\n")
    assert dominican["bars"] == ["DIVISIO_MINOR_D1"], dominican
    # A custos has a pitch in the dump but is not a sung note.
    custos = canonical.parse_dump(
        "type 8 (GRE_CUSTOS)\npitch a\n---\ntype 1 (GRE_NOTE)\npitch g\n"
    )
    assert [note["letter"] for note in custos["notes"]] == ["g"], custos
    again = gregorio_ref(args.gregorio, src, tmp / "again")
    assert again.get("ok") and again["sha"] == ref["sha"], "Gregorio is not deterministic"
    print(f"selftest : OK (core={result['core_exact']}, signs={result['signs']}, deterministic=True)")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--liste", type=Path)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--gregorio", type=Path, required=True, help="built gregorio binary")
    p.add_argument("--verifier-determinisme", action="store_true")
    p.add_argument("--jobs", type=int, default=min(2, os.cpu_count() or 1))
    p.add_argument("--selftest", action="store_true")
    p.add_argument(
        "--writer", type=Path,
        help="optional scoretk CLI binary to measure GABC -> model -> GABC",
    )
    args = p.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)

    if args.selftest:
        selftest(args)
        return
    if not args.liste:
        p.error("--liste is required unless --selftest is given")

    entrees = lire_liste(args.liste)
    with ThreadPoolExecutor(max_workers=max(1, min(args.jobs, 2))) as pool:
        releves = list(pool.map(lambda it: measure_one(it, args), entrees))

    with (args.out / "mesure.jsonl").open("w", encoding="utf-8") as handle:
        for releve in releves:
            handle.write(json.dumps(releve, sort_keys=True, ensure_ascii=False, default=str) + "\n")

    valides = [r for r in releves if "sha_dump" in r]
    deterministes = sum(1 for r in valides if r.get("gregorio_deterministe"))
    exacts = sum(1 for r in valides if r.get("cmp_source", {}).get("core_exact"))
    written = [r for r in valides if "cmp_writer" in r]
    writer_exacts = sum(1 for r in written if r["cmp_writer"]["core_exact"])
    writer_identical = sum(1 for r in written if r.get("writer_dump_identique"))
    resume = [
        "# GABC oracle — Gregorio reference",
        "",
        f"Files: {len(releves)} | Gregorio refs: {len(valides)} | "
        f"Gregorio deterministic: {deterministes}/{len(valides)} | "
        f"our reader core-exact: {exacts}/{len(valides)}",
        f"scoretk writer: core-exact {writer_exacts}/{len(written)} | "
        f"Gregorio dump identical {writer_identical}/{len(written)}",
        "",
        "Truth: pitch with clef applied, syllable text and bars "
        "(`canonical.py`); signs are optional N2. Gregorio is the only reference.",
        "",
        "| id | gregorio | deterministic | source core-exact | writer core-exact "
        "| writer dump identical | first divergence | signs |",
        "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for r in releves:
        c = r.get("cmp_source", {})
        w = r.get("cmp_writer", {})
        resume.append(
            f"| {r['id']} | {'ok' if 'sha_dump' in r else 'FAIL'} | "
            f"{'yes' if r.get('gregorio_deterministe') else 'no'} | "
            f"{'yes' if c.get('core_exact') else 'no'} | "
            f"{'yes' if w.get('core_exact') else ('no' if w else '—')} | "
            f"{'yes' if r.get('writer_dump_identique') else ('no' if w else '—')} | "
            f"{c.get('premier_divergence', r.get('erreur', ''))} | "
            f"{'yes' if c.get('signs') else 'no'} |"
        )
    (args.out / "resume.md").write_text("\n".join(resume) + "\n", encoding="utf-8")
    print(
        f"files={len(releves)} gregorio={len(valides)} "
        f"deterministic={deterministes}/{len(valides)} source_core={exacts}/{len(valides)} "
        f"writer_core={writer_exacts}/{len(written)} "
        f"writer_dump_identical={writer_identical}/{len(written)}"
    )


if __name__ == "__main__":
    main()
