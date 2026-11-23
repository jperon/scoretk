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

"""GABC oracle: characterise gabctk, then measure a candidate reader/writer.

For each ``.gabc`` of the manifest the reference is produced by **gabctk**
(GPL-2.0-or-later, tag ``v240107``), run in an isolated interpreter
(``python3 -I``) so no ambient ``PYTHONPATH`` or user site can change its
behaviour. The downloaded source is data: this module never imports it, only
executes the recorded ``gabctk.py`` entry point in a subprocess.

Two references are produced from the same source and compared with the shared
``midi_notes`` extractor:

* ``ref.xml`` — gabctk ``-x`` (MusicXML, itself derived from ABC through the
  bundled ``abc2xml.py``: see ``gabctk.py:1390-1410``);
* ``ref.mid`` — gabctk ``-o`` (SMF, durations truncated with ``int()`` at
  ``gabctk.py:1443-1454``).

Their agreement (``cmp_ref``) is a **characterisation of the reference**:
``n1`` (pitch multiset) should be 1.0000 and ``n2`` (pitch + onset) records
the two exporters' duration/quantisation divergence. ``n2 < 1`` is expected
and is never attributed to the converter under test.

When ``--scoretk`` is supplied the same source is also converted by scoretk
and compared to each reference (import direction). With ``--roundtrip`` the
manifest holds scoretk-*exported* GABC files, compared with references from
``--refs`` (export direction). Phase 0 has neither converter: the default run
measures gabctk against itself.
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
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE.parent / "midi"))

import midi_notes as notes  # noqa: E402

SELFGABC = """name: Selftest;
language: latin;
staff-lines: 4;
%%

(c4) Test(g) de(f) se(g)lf(h)test.(g) (::)
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
    return proc.returncode, (proc.stderr or proc.stdout)[-800:]


def _last_error(err: str) -> str:
    """Keep the exception line and its source frame, not the whole traceback."""
    lines = [l.strip() for l in err.splitlines() if l.strip()]
    frames = [l for l in lines if "gabctk.py" in l and ", in " in l]
    return " | ".join((frames[-1:] + lines[-1:]))[:300] if lines else ""


def gabctk_refs(python: str, gabctk: Path, src: Path, work: Path) -> dict:
    """Run gabctk once; write ``ref.mid``, ``ref.xml``, ``ref.abc``."""
    work.mkdir(parents=True, exist_ok=True)
    mid, xml, abc = work / "ref.mid", work / "ref.xml", work / "ref.abc"
    code, err = _run(
        [
            python, "-I", str(gabctk), "-i", str(src),
            "-o", str(mid), "-x", str(xml), "-c", str(abc),
        ]
    )
    if code != 0 or not mid.exists() or not xml.exists():
        return {"ok": False, "code": code, "erreur": _last_error(err)}
    return {
        "ok": True,
        "code": code,
        "mid": str(mid),
        "xml": str(xml),
        "abc": str(abc),
        "sha_mid": _sha(mid),
        "sha_xml": _sha(xml),
    }


def _compare(xml: str, mid: str) -> dict:
    """Shared N1/N2/N2d comparison; empty notes are kept visible in the row."""
    try:
        return notes.compare(notes.read_xml(xml), notes.read_midi(mid))
    except Exception as exc:  # untrusted data: record, never crash the batch
        return {"erreur": f"{type(exc).__name__}: {exc}"[:300], "exact": False}


def measure_one(item: tuple[str, Path], args) -> dict:
    ident, src = item
    work = args.out / "travail" / ident
    releve = {"id": ident, "fichier": str(src)}
    refs = gabctk_refs(args.python, args.gabctk, src, work)
    if not refs["ok"]:
        releve.update(refs)
        return releve
    releve["ref_ok"] = True
    releve["cmp_ref"] = _compare(refs["xml"], refs["mid"])
    releve["sha_mid"] = refs["sha_mid"]
    releve["sha_xml"] = refs["sha_xml"]

    if args.verifier_determinisme and ident in args._deterministes:
        again = work / "again"
        second = gabctk_refs(args.python, args.gabctk, src, again)
        releve["deterministe"] = bool(
            second.get("ok")
            and second["sha_mid"] == refs["sha_mid"]
            and second["sha_xml"] == refs["sha_xml"]
        )

    if args.roundtrip:
        # The manifest holds candidate GABC exported by scoretk; compare it
        # with the reference produced from the *original* source in --refs.
        ref_dir = args.refs / ident
        if not ref_dir.is_dir():
            releve["erreur"] = f"missing round-trip references: {ref_dir}"
            return releve
        releve["cmp_export_xml"] = _compare(
            refs["xml"], str(ref_dir / "ref.xml")
        ) if (ref_dir / "ref.xml").exists() else {"erreur": "no ref.xml"}
        releve["cmp_export_mid"] = _compare(
            refs["mid"], str(ref_dir / "ref.mid")
        ) if (ref_dir / "ref.mid").exists() else {"erreur": "no ref.mid"}
        return releve

    if args.scoretk:
        tk_xml, tk_mid = work / "tk.musicxml", work / "tk.mid"
        for to, dest in (("musicxml", tk_xml), ("midi", tk_mid)):
            code, err = _run([args.scoretk, str(src), "--to", to, "-o", str(dest)])
            if code != 0:
                releve[f"scoretk_{to}_erreur"] = err.strip()[:300]
        if tk_xml.exists():
            releve["cmp_import_xml"] = _compare(refs["xml"], str(tk_xml))
        if tk_mid.exists():
            releve["cmp_import_mid"] = _compare(refs["mid"], str(tk_mid))
    return releve


def selftest(args) -> None:
    """Small, deterministic checks that do not need the full lot."""
    tmp = args.out / "selftest"
    tmp.mkdir(parents=True, exist_ok=True)
    src = tmp / "selftest.gabc"
    src.write_text(SELFGABC, encoding="utf-8")
    refs = gabctk_refs(args.python, args.gabctk, src, tmp / "run")
    assert refs["ok"], f"gabctk failed selftest: {refs}"
    cmp_self = _compare(refs["xml"], refs["mid"])
    assert cmp_self.get("n1") == cmp_self.get("ref", 0), cmp_self
    a = [(60, 0.0, 1.0, 1), (62, 1.0, 1.0, 1)]
    assert notes.compare(a, a)["exact"] is True
    assert notes.compare(a, [(60, 0.5, 1.0, 1), (62, 1.0, 1.0, 1)])["n1"] == 2
    manifest = tmp / "lot.tsv"
    manifest.write_text(f"un\t{src}\n", encoding="utf-8")
    assert lire_liste(manifest) == [("un", src)]
    print(f"selftest : OK (gabctk n1={cmp_self.get('n1')}/{cmp_self.get('ref')})")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--liste", type=Path, help="manifest id<TAB>path")
    p.add_argument("--out", type=Path, required=True, help="output directory")
    p.add_argument("--gabctk", type=Path, required=True, help="gabctk.py path")
    p.add_argument("--python", default=sys.executable, help="Python for -I (venv)")
    p.add_argument("--scoretk", help="optional scoretk CLI for the import direction")
    p.add_argument("--roundtrip", action="store_true", help="manifest = exported GABC")
    p.add_argument("--refs", type=Path, help="references for --roundtrip")
    p.add_argument("--verifier-determinisme", action="store_true")
    p.add_argument("--jobs", type=int, default=min(2, os.cpu_count() or 1))
    p.add_argument("--selftest", action="store_true")
    args = p.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    if args.roundtrip and not args.refs:
        p.error("--roundtrip requires --refs")

    if args.selftest:
        selftest(args)
        return
    if not args.liste:
        p.error("--liste is required unless --selftest is given")

    entrees = lire_liste(args.liste)
    args._deterministes = {ident for ident, _ in entrees[:3]}
    # At most two workers: shared machine, and gabctk is single-threaded.
    with ThreadPoolExecutor(max_workers=max(1, min(args.jobs, 2))) as pool:
        releves = list(pool.map(lambda it: measure_one(it, args), entrees))

    with (args.out / "mesure.jsonl").open("w", encoding="utf-8") as handle:
        for releve in releves:
            handle.write(json.dumps(releve, sort_keys=True, ensure_ascii=False) + "\n")

    ok = [r for r in releves if r.get("ref_ok")]
    exact = sum(1 for r in ok if r.get("cmp_ref", {}).get("exact"))
    parfait_n1 = sum(
        1 for r in ok
        if r.get("cmp_ref", {}).get("n1") == r.get("cmp_ref", {}).get("ref")
    )
    resume = [
        "# GABC oracle",
        "",
        f"Files: {len(releves)} | reference produced: {len(ok)} | "
        f"reference n1 exact: {parfait_n1} | reference fully exact: {exact}",
        "",
        "The reference comparison is gabctk MusicXML vs gabctk MIDI: it "
        "characterises the two gabctk exporters, not scoretk.",
        "",
        "| id | status | n1 | n2 | note |",
        "| --- | --- | ---: | ---: | --- |",
    ]
    for r in releves:
        c = r.get("cmp_ref", {})
        resume.append(
            f"| {r['id']} | {'ok' if r.get('ref_ok') else 'FAIL'} | "
            f"{c.get('n1', '-')} | {c.get('n2', '-')} | "
            f"{(r.get('erreur') or c.get('erreur') or '')[:70]} |"
        )
    (args.out / "resume.md").write_text("\n".join(resume) + "\n", encoding="utf-8")
    print(f"files={len(releves)} refs_ok={len(ok)} n1_exact={parfait_n1} full_exact={exact}")


if __name__ == "__main__":
    main()
