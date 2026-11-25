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

"""Generate the GABC → MIDI landmark references inside a bubblewrap sandbox.

The external tool is downloaded data: it is **executed**, never imported,
with ``python3 -I``. The sandbox has no network, a read-only root, a private
tmpfs and exactly one writable directory per file (its output and caches),
bounded by ``timeout`` and ``prlimit``. Only two files run concurrently.
All divergences are recorded in ``status.jsonl``; nothing is silently skipped.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import subprocess
import time

TOOL = Path("/home/jacques/programmes/musique/specs/gabc/gabctk/gabctk.py")


def lire_liste(path: Path) -> list[tuple[str, Path]]:
    out = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        ident, source = line.split("\t")
        out.append((ident.strip(), Path(source.strip())))
    return out


def sandbox(dossier: Path) -> list[str]:
    """Same shape as the LilyPond third-party-source sandbox."""
    d = str(dossier)
    return [
        "bwrap", "--unshare-all", "--die-with-parent",
        "--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc",
        "--tmpfs", "/tmp", "--bind", d, d,
        "--setenv", "HOME", d, "--setenv", "XDG_CACHE_HOME", d + "/cache",
        "--setenv", "PYTHONDONTWRITEBYTECODE", "1",
        "--chdir", d, "--",
    ]


def generer(item: tuple[str, Path], args) -> dict:
    ident, source = item
    scratch = args.scratch / ident
    scratch.mkdir(parents=True, exist_ok=True)
    sortie = scratch / "ref.mid"
    debut = time.monotonic()
    commande = sandbox(scratch) + [
        "timeout", str(args.timeout), "prlimit", "--as=2000000000",
        "python3", "-I", str(TOOL), "-i", str(source), "-o", str(sortie),
    ]
    try:
        proc = subprocess.run(
            commande, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            timeout=args.timeout + 5, check=False,
        )
        code = proc.returncode
        erreur = proc.stderr.decode("utf-8", "replace")[-400:]
    except subprocess.TimeoutExpired:
        code, erreur = 124, "timeout"
    ok = code == 0 and sortie.is_file() and sortie.stat().st_size > 0
    destination = args.out / ident
    if ok:
        destination.mkdir(parents=True, exist_ok=True)
        (destination / "ref.mid").write_bytes(sortie.read_bytes())
    return {
        "id": ident,
        "fichier": str(source),
        "ok": ok,
        "code": code,
        "erreur": "" if ok else erreur,
        "temps": round(time.monotonic() - debut, 3),
        "sha": hashlib.sha256(sortie.read_bytes()).hexdigest() if ok else "",
    }


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--manifest", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--scratch", type=Path, required=True)
    p.add_argument("--jobs", type=int, default=2)
    p.add_argument("--timeout", type=int, default=30)
    p.add_argument("--limit", type=int, default=0)
    args = p.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    args.scratch.mkdir(parents=True, exist_ok=True)
    entrees = lire_liste(args.manifest)
    if args.limit:
        entrees = entrees[: args.limit]
    with ThreadPoolExecutor(max_workers=max(1, min(args.jobs, 2))) as pool:
        lignes = list(pool.map(lambda it: generer(it, args), entrees))
    with (args.out / "status.jsonl").open("w", encoding="utf-8") as handle:
        for ligne in lignes:
            handle.write(json.dumps(ligne, sort_keys=True, ensure_ascii=False) + "\n")
    ok = sum(1 for ligne in lignes if ligne["ok"])
    print(f"midi refs: {ok}/{len(lignes)} produced in {args.out}")


if __name__ == "__main__":
    main()
