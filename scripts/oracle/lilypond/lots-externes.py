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

"""Build independent external LilyPond control lots for the reading oracle.

Nothing is copied into this repository.  Pinned snapshots are downloaded into
``--work`` (default ``/home/jacques/tmp/scoretk/lots-ext``) and the manifests
of absolute paths are written into ``--out`` (default ``<work>/manifestes``).
The data are untrusted: this script only reads text files; it never runs a
script, Makefile or Python file shipped with a source.  MIDI references are a
separate step (``references.py --sandbox --inject-midi``), under bubblewrap.

Licences were audited per source before inclusion (``SOURCES`` and
``REJECTED`` below, with file+line or URL evidence).  Every audit record is
also written to ``<out>/licences.jsonl`` so the decision can be re-checked.
Only sources permitting copying and redistribution of derived test data are
accepted: public domain, CC0, CC BY, CC BY-SA, GPL/LGPL, MIT/Apache.  NC/ND
and rights-ambiguous sources are rejected and listed, never downloaded.

Usage:
    python3 scripts/oracle/lilypond/lots-externes.py
    python3 scripts/oracle/lilypond/lots-externes.py --max-par-source 40
    python3 scripts/oracle/lilypond/lots-externes.py --causes \
        --out /home/jacques/tmp/scoretk/lots-ext/manifestes
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import json
from pathlib import Path
import subprocess
import sys
import urllib.request

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

WORK = Path("/home/jacques/tmp/scoretk/lots-ext")

# Pinned to the branch heads present on 2026-11-24.  `evidence` is
# "<file>:<line>" of the licence statement at that exact commit.
SOURCES = [
    # Ksnortum's personal engraving repositories (CC BY-SA 4.0).
    dict(key="ksnortum-bach15", group="kno", repo="ksnortum/bach-15-sinfonias",
         sha="81ab75c5421db183671405c49a28644b2f8adedd", licence="CC BY-SA 4.0",
         evidence="README.md:24", max_files=40),
    dict(key="ksnortum-inventions", group="kno", repo="ksnortum/bach-15-two-part-inventions",
         sha="2398bbbef2bd0ac8b2dadafd53eed7048d261289", licence="CC BY-SA 4.0",
         evidence="README.md:30", max_files=40),
    dict(key="ksnortum-goldberg", group="kno", repo="ksnortum/bach-goldberg",
         sha="970dc7ccf194639ec97dddfb660fde3d775de35c", licence="CC BY-SA 4.0",
         evidence="README.md:41", max_files=40),
    dict(key="ksnortum-wtc1", group="kno", repo="ksnortum/bach-well-tempered-1",
         sha="cfddd9b9a8f507cf662fe7d2880ca97e0694ca4e", licence="CC BY-SA 4.0",
         evidence="README.md:32", max_files=40),
    dict(key="ksnortum-wtc2", group="kno", repo="ksnortum/bach-well-tempered-2",
         sha="27f5de11ef2458d93238217b10f2e98a1782aa7e", licence="CC BY-SA 4.0",
         evidence="README.md:38", max_files=40),
    dict(key="ksnortum-chopin28", group="kno", repo="ksnortum/chopin-opus-28",
         sha="9c0e38a8431b2483944a4bd7c536017d78118c6a", licence="CC BY-SA 4.0",
         evidence="README.md:46", max_files=40),
    dict(key="ksnortum-chopin25", group="kno", repo="ksnortum/chopin-opus-25",
         sha="e38cb7214425970e716eef33b1eadd3e6fd5e321", licence="CC BY-SA 4.0",
         evidence="README.md:32", max_files=40),
    dict(key="ksnortum-satie", group="kno", repo="ksnortum/satie-trois-valses",
         sha="1a7ff98e69705286d689ec8d68ae79623a90510d", licence="CC BY-SA 4.0",
         evidence="README.md:27", max_files=40),
    dict(key="ksnortum-scarlatti", group="kno", repo="ksnortum/scarlatti-sonatas-k1-k10",
         sha="1b7da6b893b6ceed4a00d8882103d153deb107ac", licence="CC BY-SA 4.0",
         evidence="README.md:29", max_files=40),
    # CPDL editions: repo-level GPL for ace214, per-file CPDL redistribution
    # permission for both (checked by `header_allows_cpdl` below).
    dict(key="cpdl-ace214", group="cpdl", repo="ace214/cpdl",
         sha="5a02f2e5ff4a4d627ce6e7f83940ca9cee8977d5", licence="GPL-3.0 / CPDL edition",
         evidence="LICENSE:1; beethoven/abschiedgesang/Abschiedsgesang.ly:9", max_files=40),
    dict(key="cpdl-wbsoft", group="cpdl", repo="wbsoft/lilymusic",
         sha="ecf7415b6e006ad9f8516e0cac6f5f3683e24827", licence="per-file CPDL licence",
         evidence="arcadelt/o-lord.ly:20-24", max_files=60),
    # Personal/public-domain collections.
    dict(key="juliedigne", group="perso", repo="juliedigne/scores",
         sha="b9d2564f25a55d6cb110d9a3be3b739bb482fefe", licence="public domain",
         evidence="README.md:3", max_files=40),
    dict(key="snapsakademien", group="perso", repo="Snapsakademien/scores-ly",
         sha="5d95654e65815c7a969888b82fd68b0596f441c4", licence="public domain songs",
         evidence="README.md:5", max_files=40),
    dict(key="cellist", group="perso", repo="cellist/Lilypond-Sheet-Music",
         sha="9568b99058a2c8e9c7129f34440f898963315f4e",
         licence="explicit permission to use and share", evidence="README.md:7", max_files=40),
    dict(key="phmccarty", group="perso", repo="phmccarty/compositions",
         sha="90eed300828535b3bb4fda0f3bdbcbc7819aac29", licence="CC BY-SA 4.0 / CC0",
         evidence="README.md:4", max_files=40),
    # Small CC0 / CC BY-SA / MIT collections found by topic search.
    dict(key="allofme", group="cc", repo="jeandeaual/lilypond-allofme",
         sha="a29330dd0e9c88ad8f749106df055ae541f0f3dc", licence="CC0-1.0",
         evidence="LICENSE:1", max_files=40),
    # De âlde Friezen: two scores sharing one included note/lyric file; the
    # scoretk report from the ly2video author motivating this control lot.
    dict(key="alde-friezen", group="af", repo="MacLotsen/alde-friezen",
         sha="bba32e7492d2bfe581b15b5d82179db132937702", licence="CC0-1.0",
         evidence="README.md:68-71; LICENSE:1", max_files=10),
    dict(key="guitar-yawnoc", group="cc", repo="yawnoc/guitar",
         sha="fe48dbba46be760fab453b3a72ef35746f20ea48", licence="CC0-1.0 / public domain",
         evidence="README.md:5-8", max_files=40),
    dict(key="horn-mit", group="cc", repo="nwhetsell/horn-sheet-music",
         sha="51e060ef554e47df229c536f06be212c93e36cec", licence="MIT / public-domain music",
         evidence="LICENSE.md:1-3", max_files=40),
    dict(key="engravings", group="cc", repo="AugmentedFifth/engravings",
         sha="fe85d3d094e33de4820b505a3d3c0f9fdf86156e", licence="CC BY-SA 4.0-or-later",
         evidence="README.md:37-39", max_files=40),
    dict(key="latvian", group="cc", repo="just-a-book-archivist/latvian-music",
         sha="ac3cf707f57969ee667d0a0b37a0304b19679244", licence="CC BY-SA 4.0",
         evidence="README.md:50", max_files=40),
    dict(key="hinariocentral", group="cc", repo="hinariocentral/hinariocentral-core",
         sha="c878675b8e60b4bc58d502feb1255a4394295718", licence="CC BY-SA 4.0",
         evidence="LICENSE:1", max_files=40),
    dict(key="loyola", group="cc", repo="loyola-university-tech-ensemble/in-c-phrases-ly",
         sha="b0e446e847b86cabff166484ccc2afc37c1d902c", licence="CC BY-SA 4.0",
         evidence="LICENSE.md:1", max_files=40),
]

REJECTED = [
    dict(source="BMdataset (BaroqueMusic.it), Zenodo record 18723290, v1.0.0",
         url="https://zenodo.org/api/records/18723290",
         reason="CC BY-NC-ND 4.0 (metadata.license.id): NC forbids the intended reuse, "
                "ND forbids derived data; 826 MB archive not downloaded"),
    dict(source="CSCPadova/lilybert @bd28a4a99a3574e3854aa7082bd856c64889c57e",
         url="https://github.com/CSCPadova/lilybert",
         reason="Apache-2.0 covers the code only; tests/ly/00a01a21-…ly:2 says the scores "
                "come from the NC-ND BMdataset, and line 1 has a @TOPLEVEL_VERSION@ "
                "placeholder. Score data rejected as NC-ambiguous"),
    dict(source="allengarvin/typeset @1fac63f4967839056598403093db3e5dd247590a",
         url="https://github.com/allengarvin/typeset",
         reason="LICENSE.txt:1 Attribution-NonCommercial 4.0 International (NC)"),
    dict(source="ksnortum/openbook @7b9ffb6c766e36e8278820b36d255469e99db9f7",
         url="https://github.com/ksnortum/openbook",
         reason="README.md:77 GPL-3.0 for the project but 'The tunes themselves have "
                "their own copyright holders': underlying rights ambiguous"),
    dict(source="nicovince/scores @656a2d055b9bf086b9dd6d56a23de4513c563133",
         url="https://github.com/nicovince/scores",
         reason="public-domain statement, but the tree duplicates juliedigne/scores "
                "(same files); deduplicated, not measured twice"),
]

FORBIDDEN = ("mutopia", "openlilylib", "regression", "lsr", "all rights reserved")


def git(*args: str, cwd: Path) -> str:
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True,
                          text=True, check=True).stdout.strip()


def clone_pinned(src: dict, work: Path) -> Path:
    """Partial clone at the pinned commit; checkout only text score files."""
    dest = work / src["key"]
    if not (dest / ".git").exists():
        dest.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["git", "clone", "-q", "--filter=blob:none", "--no-checkout",
                        "--depth", "1", f"https://github.com/{src['repo']}.git", str(dest)],
                       check=True)
    if git("rev-parse", "HEAD", cwd=dest) != src["sha"]:
        subprocess.run(["git", "-C", str(dest), "fetch", "-q", "--depth", "1",
                        "origin", src["sha"]], check=True)
        subprocess.run(["git", "-C", str(dest), "checkout", "-q", src["sha"]], check=True)
    paths = git("ls-tree", "-r", "--name-only", "HEAD", cwd=dest).splitlines()
    candidats = sorted(
        p for p in paths
        if p.endswith(".ly") and not p.endswith(".ily")
        and not any(mot in p.lower() for mot in FORBIDDEN)
    )
    maximum = min(src.get("max_files", 40), len(candidats))
    if maximum and maximum < len(candidats):
        # Spread over the alphabetical list rather than taking the first files.
        pas = max(1, len(candidats) // maximum)
        candidats = candidats[::pas][:maximum]
    # Includes live next to the score, one directory below (ksnortum's
    # `includes/`) or one above (shared headers).  Only those are fetched,
    # never all 22 000 .ily of cellist.
    parents = {Path(p).parent for p in candidats}
    includes = [
        p for p in paths
        if p.endswith(".ily")
        and any(
            Path(p).parent in (d, d.parent) or Path(p).parent.parent == d
            for d in parents
        )
    ]
    meta = [p for p in paths if Path(p).name.startswith(("README", "LICENSE", "COPYING"))]
    checkout = candidats + includes + meta
    subprocess.run(["git", "-C", str(dest), "checkout", "-q", "HEAD", "--", *checkout],
                   check=True)
    retenus = []
    for chemin in candidats:
        texte = (dest / chemin).read_text(encoding="utf-8", errors="replace")
        if any(mot in texte.lower() for mot in FORBIDDEN):
            continue
        if src["group"] == "cpdl":
            tete = texte[:4000].lower()
            if not any(mot in tete for mot in ("cpdl", "public domain", "freely", "creative commons")):
                continue
        retenus.append(dest / chemin)
    return retenus


def ecrire_manifestes(work: Path, out: Path, max_par_source: int,
                      seulement: set[str] | None) -> dict[str, list[Path]]:
    groupes: dict[str, list[Path]] = defaultdict(list)
    audit = []
    for src in SOURCES:
        if seulement and src["key"] not in seulement:
            continue
        src = dict(src)
        if max_par_source:
            src["max_files"] = min(src.get("max_files", 40), max_par_source)
        fichiers = clone_pinned(src, work)
        groupes[src["group"]].extend(fichiers)
        groupes["ext"].extend(fichiers)
        audit.append(dict(source=src["key"], url=f"https://github.com/{src['repo']}",
                          commit=src["sha"], licence=src["licence"],
                          evidence=src["evidence"], status="accepted",
                          files=len(fichiers)))
        print(f"{src['key']}: {len(fichiers)} fichiers", file=sys.stderr)
    for rejete in REJECTED:
        if seulement:
            continue
        audit.append({**rejete, "status": "rejected", "files": 0})
    out.mkdir(parents=True, exist_ok=True)
    (out / "licences.jsonl").write_text(
        "\n".join(json.dumps(a, ensure_ascii=False) for a in audit) + "\n",
        encoding="utf-8",
    )
    for nom, fichiers in sorted(groupes.items()):
        chemin = out / f"{nom}.txt"
        chemin.write_text("\n".join(str(f) for f in sorted(set(fichiers))) + "\n",
                          encoding="utf-8")
        print(f"{chemin}: {len(set(fichiers))} fichiers")
    return groupes


def premiere_divergence(ref: list[tuple], test: list[tuple]) -> str:
    """First divergent event, using the tuple that defines exactness.

    ``ly_notes.compare`` defines ``exact`` as ``r == t == n1`` and
    ``n2 == r``: it compares note count, pitch multiset and
    ``(pitch, quantized attack)`` multiset.  Duration is deliberately not
    part of that definition.  A tie chain is the reason: LilyPond's
    ``Tie_performer`` merges tied heads into one MIDI note covering the whole
    chain (``lily/tie-performer.cc:116-185``), whereas MusicXML writes one
    note per tie segment (``<tie type="start"/>`` … ``<tie type="stop"/>``,
    ``scoretk-musicxml-export/src/exportmusicxml.rs:1813-1816``).  ``read_xml``
    skips the stop notes (``ly_notes.py:314``), so the single surviving attack
    carries only the *first* segment's duration.  Sorting on duration would
    therefore report that harmless difference as the "first divergence" and
    mask the missing/extra attack that actually makes the file inexact.
    Classifying on ``(pitch, attack)`` matches the oracle's own semantics.
    """
    if not test:
        return "empty_output"
    if not ref:
        return "empty_reference"
    for i in range(max(len(ref), len(test))):
        a = ref[i] if i < len(ref) else None
        b = test[i] if i < len(test) else None
        if a == b:
            continue
        if a is None:
            return "extra_note"
        if b is None:
            return "missing_note"
        if a[0] != b[0]:
            return "pitch"
        return "attack"
    return "other"


def causes_selftest() -> None:
    """A tie chain must not be reported as a duration divergence."""
    # c'2~ c'1: LilyPond plays one attack of 3 quarters; MusicXML writes a
    # half note tied to a whole note.  ``read_xml`` keeps one attack of 1
    # quarter (the first segment).  With one extra unrelated note in the
    # candidate, the real first divergence is that extra note, not duration.
    ref = [(60, 0), (62, 1)]
    test = [(60, 0), (62, 1), (64, 2)]
    assert premiere_divergence(ref, test) == "extra_note"
    # The legacy (pitch, attack, duration) sort reported duration first:
    legacy_ref = sorted([(60, 0, 3), (62, 1, 1)])
    legacy_test = sorted([(60, 0, 1), (62, 1, 1), (64, 2, 1)])
    assert legacy_ref[0] != legacy_test[0] and legacy_ref[0][:2] == legacy_test[0][:2]
    assert premiere_divergence([(60, 0)], [(60, 0)]) == "other"
    assert premiere_divergence([], [(60, 0)]) == "empty_reference"
    assert premiere_divergence([(60, 0)], []) == "empty_output"
    print("selftest causes : OK")


def analyser_causes(out: Path, lot: str) -> None:
    """Group non-exact files by their first divergent note event."""
    from ly_notes import GRID, read_midi, read_xml

    chemin = out / f"{lot}.jsonl"
    lignes = [json.loads(l) for l in chemin.read_text(encoding="utf-8").splitlines() if l.strip()]
    lignes = [r for r in lignes if r.get("outil") == "scoretk"]
    groupes: dict[str, list[str]] = defaultdict(list)
    for index, r in enumerate(lignes):
        if r.get("exclu") or not r.get("ok") or not r.get("ref") or r.get("exact"):
            continue
        travail = out / "travail" / lot / str(index)
        ref = [(p, round(a / GRID)) for p, a, d, _ in
               read_midi(str(travail / "ref.mid"))]
        test = [(p, round(a / GRID)) for p, a, d, *_ in
                read_xml(str(travail / "tk.musicxml"), with_grace=True)]
        groupes[premiere_divergence(sorted(ref), sorted(test))].append(r["fichier"])
    print(f"# First divergence, {lot}: {sum(map(len, groupes.values()))} non-exact files")
    for cause, fichiers in sorted(groupes.items(), key=lambda kv: -len(kv[1])):
        print(f"| {cause} | {len(fichiers)} | `{Path(fichiers[0]).name}` |")
    for cause, fichiers in sorted(groupes.items(), key=lambda kv: -len(kv[1])):
        print(f"\n## {cause} ({len(fichiers)})")
        for f in fichiers[:12]:
            print(f"  {Path(f).name}")
        if len(fichiers) > 12:
            print(f"  … {len(fichiers) - 12} more")


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--work", default=str(WORK))
    p.add_argument("--out", default="", help="manifest directory (default <work>/manifestes)")
    p.add_argument("--max-par-source", type=int, default=0,
                   help="override the per-source cap (0: keep the declared caps)")
    p.add_argument("--only", default="", help="comma-separated source keys")
    p.add_argument("--causes", action="store_true",
                   help="group first divergences of an existing --out measurement")
    p.add_argument("--causes-selftest", action="store_true",
                   help="check that tie chains do not mask the real divergence")
    p.add_argument("--lot", default="ext", help="lot name for --causes")
    args = p.parse_args()
    if args.causes_selftest:
        causes_selftest()
        return 0
    out = Path(args.out) if args.out else Path(args.work) / "manifestes"
    if args.causes:
        analyser_causes(out, args.lot)
        return 0
    ecrire_manifestes(Path(args.work), out, args.max_par_source,
                      set(args.only.split(",")) if args.only else None)
    return 0


if __name__ == "__main__":
    sys.exit(main())
