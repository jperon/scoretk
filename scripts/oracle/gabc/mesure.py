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

"""GABC oracle: Gregorio is the reference, gabctk is characterised against it.

GABC has no fixed durations (`docs/agents/format-ref/gabc.md` §7), so comparing
gabctk's MusicXML (derived from ABC) with its MIDI (``int(duree)`` truncation)
cannot be a sanity test: their onsets differ by construction. The truth is
defined by :mod:`canonical`: pitch **with the clef applied**, syllable text,
neume grouping and separation bars; rhythmic signs are an optional N2.

Primary reference: **Gregorio's own parser**, ``gregorio -f gabc -F dump``.
Secondary reference: our own reader of the source (sanity control).
Candidate: gabctk's re-emitted notation (``-m`` plus ``-e`` for the lyrics).
Where they disagree, Gregorio prevails and the divergence is classified by
the failing check in :func:`canonical.compare`.

The downloaded sources are data; only the two recorded executables run, each
in its own build/venv directory. gabctk is invoked with ``python3 -I``.
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


def gabctk_ref(python: str, gabctk: Path, src: Path, work: Path) -> dict:
    """Run gabctk once for its re-emitted notation and lyric text."""
    work.mkdir(parents=True, exist_ok=True)
    notation, lyrics = work / "cand.gabc", work / "cand.txt"
    code, err = _run(
        [python, "-I", str(gabctk), "-i", str(src), "-m", str(notation), "-e", str(lyrics)]
    )
    if code != 0 or not notation.exists():
        return {"ok": False, "code": code, "erreur": _last_error(err)}
    return {"ok": True, "notation": notation, "lyrics": lyrics}


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

    cand = gabctk_ref(args.python, args.gabctk, src, work)
    if not cand["ok"]:
        releve.update(cand)
        return releve
    canon_cand = canonical.parse_gabctk(
        cand["notation"].read_text(encoding="utf-8", errors="replace"),
        cand["lyrics"].read_text(encoding="utf-8", errors="replace"),
    )
    releve["cmp_grouping"] = canonical.compare_grouping(source, canon_cand)
    releve["cmp_gabctk"] = canonical.compare(canon_cand, canon_ref)
    releve["gabctk_defauts"] = [
        k for k, ok in releve["cmp_gabctk"].items()
        if k in ("pitch_multiset", "pitch_sequence", "bars", "text", "signs") and not ok
    ]
    return releve


def selftest(args) -> None:
    tmp = args.out / "selftest"
    tmp.mkdir(parents=True, exist_ok=True)
    src = tmp / "selftest.gabc"
    src.write_text(SELFGABC, encoding="utf-8")
    ref = gregorio_ref(args.gregorio, src, tmp / "ref")
    assert ref["ok"], f"Gregorio failed selftest: {ref}"
    cand = gabctk_ref(args.python, args.gabctk, src, tmp / "cand")
    assert cand["ok"], f"gabctk failed selftest: {cand}"
    a = canonical.parse_dump(ref["dump"].read_text(encoding="utf-8"))
    b = canonical.parse_gabctk(
        cand["notation"].read_text(encoding="utf-8"),
        cand["lyrics"].read_text(encoding="utf-8"),
    )
    result = canonical.compare(b, a)
    assert result["core_exact"], result
    assert canonical.compare(canonical.parse_source(SELFGABC), a)["core_exact"]
    print(f"selftest : OK ({result['core_exact']=}, signs={result['signs']})")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--liste", type=Path)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--gabctk", type=Path, required=True)
    p.add_argument("--gregorio", type=Path, required=True, help="built gregorio binary")
    p.add_argument("--python", default=sys.executable, help="Python for -I (venv)")
    p.add_argument("--verifier-determinisme", action="store_true")
    p.add_argument("--jobs", type=int, default=min(2, os.cpu_count() or 1))
    p.add_argument("--selftest", action="store_true")
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

    ok = [r for r in releves if r.get("cmp_gabctk")]
    source_exact = sum(1 for r in releves if r.get("cmp_source", {}).get("core_exact"))
    core = sum(1 for r in ok if r["cmp_gabctk"]["core_exact"])
    signs = sum(1 for r in ok if r["cmp_gabctk"]["signs"])
    grouping = sum(1 for r in releves if r.get("cmp_grouping", {}).get("neume_grouping"))
    resume = [
        "# GABC oracle — Gregorio reference",
        "",
        f"Files: {len(releves)} | Gregorio refs: {sum(1 for r in releves if 'sha_dump' in r)} | "
        f"our reader == Gregorio: {source_exact} | "
        f"gabctk core == Gregorio: {core} | gabctk signs == Gregorio: {signs} | "
        f"gabctk grouping == source: {grouping}",
        "",
        "Truth: pitch with clef applied, syllable text, neume grouping, bars "
        "(`canonical.py`); signs are optional N2. MIDI/MusicXML are not truth.",
        "",
        "| id | source==greg | gabctk==greg | grouping | first divergence | gabctk defects |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for r in releves:
        c = r.get("cmp_gabctk", {})
        resume.append(
            f"| {r['id']} | {'yes' if r.get('cmp_source', {}).get('core_exact') else 'no'} | "
            f"{'yes' if c.get('core_exact') else 'no'} | "
            f"{'yes' if r.get('cmp_grouping', {}).get('neume_grouping') else 'no'} | "
            f"{c.get('premier_divergence', r.get('erreur', ''))} | "
            f"{','.join(r.get('gabctk_defauts', []))} |"
        )
    (args.out / "resume.md").write_text("\n".join(resume) + "\n", encoding="utf-8")
    print(
        f"files={len(releves)} source_ok={source_exact} "
        f"gabctk_core={core}/{len(ok)} gabctk_signs={signs}/{len(ok)} grouping={grouping}/{len(releves)}"
    )


if __name__ == "__main__":
    main()
