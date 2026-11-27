#!/usr/bin/env bash
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

# Reproducible peak-heap / allocation / wall-time / RSS measurement of the
# engraving path (`scoretk <file> -o out.svg`). dhat is behind the
# feature-gated `memory-report` CLI build, never in a delivered binary.
# See docs/agents/engraving.md, "Resource budget (Kobo Mini)".
#
# Usage:
#   CARGO_TARGET_DIR=~/tmp/scoretk/target-engmem nix develop -c \
#     scripts/measure-engrave-memory.sh [--build] [--out DIR] FILE...
#
# The SVG, the dhat JSON and all temporary files live outside the repository;
# only numbers are reported. Private scores are read from their absolute path
# and never copied into the worktree.

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
BUILD=0
OUT=""
BIN="${SCORETK_BIN:-}"

while (($#)); do
  case $1 in
    --build) BUILD=1; shift ;;
    --out) OUT=${2:?--out needs a directory}; shift 2 ;;
    --binary) BIN=${2:?--binary needs a path}; shift 2 ;;
    -h|--help)
      sed -n '17,29p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) break ;;
  esac
done
(($#)) || { echo "at least one input file is required" >&2; exit 2; }

TARGET_DIR=${CARGO_TARGET_DIR:-"$ROOT/target"}
if (( BUILD )); then
  CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2} cargo build --release \
    -p scoretk-cli --features memory-report
fi
if [[ -z "$BIN" ]]; then
  BIN="$TARGET_DIR/release/scoretk"
fi
[[ -x "$BIN" ]] || { echo "binary not found: $BIN (use --build or --binary)" >&2; exit 2; }

# GNU time with `%M` (max RSS, KB); NixOS has no /usr/bin/time.
TIME_BIN=${TIME_BIN:-$(type -P time || true)}
[[ -n "$TIME_BIN" && -x "$TIME_BIN" ]] || {
  echo "GNU time not found; run inside 'nix develop'" >&2; exit 2;
}

WORK=$(mktemp -d "${TMPDIR:-/tmp}/scoretk-engrave-mem.XXXXXX")
trap 'rm -rf "$WORK"' EXIT
if [[ -n "$OUT" ]]; then
  mkdir -p "$OUT"
fi

for file in "$@"; do
  [[ -f "$file" ]] || { echo "not a file: $file" >&2; exit 2; }
  # The measurement runs in WORK (where dhat writes its JSON), so make the
  # input absolute; private scores are read, never copied.
  case $file in
    /*) input=$file ;;
    *) input=$PWD/$file ;;
  esac
  id=$(basename "$file")
  id=${id%.*}
  echo "=== $file"
  rm -f "$WORK/dhat-heap.json" "$WORK/time.txt"
  # dhat writes dhat-heap.json into the current directory: run in WORK.
  if ! (cd "$WORK" && "$TIME_BIN" -f 'wall=%e s user=%U s sys=%S s max_rss=%M KB' \
      -o "$WORK/time.txt" "$BIN" "$input" -o "$WORK/$id.svg" >/dev/null) \
      2> "$WORK/dhat.txt"; then
    cat "$WORK/dhat.txt" >&2
    echo "engraving measurement failed for $file" >&2
    exit 1
  fi
  cat "$WORK/time.txt"
  # dhat prints total allocations, t-gmax peak and allocations still live.
  grep '^dhat:' "$WORK/dhat.txt" || { echo "dhat report missing" >&2; exit 1; }
  if [[ -n "$OUT" ]]; then
    cp "$WORK/dhat-heap.json" "$OUT/$id.dhat-heap.json"
  fi
  python3 -I - "$WORK/dhat-heap.json" <<'PY'
import collections, json, sys
with open(sys.argv[1]) as f:
    d = json.load(f)
frames = d['ftbl']
def label(i):
    text = frames[i - 1]
    return text.split(': ', 1)[-1].split(' (???')[0]
cats = collections.defaultdict(lambda: [0, 0, 0])
for p in d['pps']:
    stack = ' '.join(label(i) for i in p['fs'])
    if ('read4xx::dom' in stack or 'read302::xml' in stack
            or 'MsczContainer' in stack or 'std::fs::read' in stack):
        cat = 'parse (XML tree / container / input bytes)'
    elif 'scoretk_engrave::atoms' in stack:
        cat = 'atoms (POD per measure/event)'
    elif ('scoretk_engrave' in stack or 'render_svg' in stack
          or 'write_svg' in stack):
        cat = 'layout / skylines / display list / SVG'
    elif any(name in stack for name in ('read4xx', 'read302', 'read206')):
        cat = 'pivot model construction'
    elif 'clap' in stack:
        cat = 'CLI parsing'
    else:
        cat = 'other'
    cats[cat][0] += p['gb']
    cats[cat][1] += p['mbk']
    cats[cat][2] = max(cats[cat][2], p['mb'])
print('Allocated bytes by subsystem (dhat JSON, cumulative):')
for cat, (b, blocks, peak) in sorted(cats.items(), key=lambda x: -x[1][0]):
    print(f'  {cat:<44} {b / 2**20:8.2f} MiB  {blocks:>7} blocks  '
          f'peak live {peak / 2**20:7.2f} MiB')
IGNORE = ('[root]', 'dhat::Alloc', 'core::alloc::global', 'alloc::raw_vec',
          'clap_builder', 'scoretk::main', 'spec_from', 'grow_amortized',
          'finish_grow', 'grow_one', 'try_allocate_in', '::new (???:0:0)')
sites = collections.defaultdict(lambda: [0, 0])
for p in d['pps']:
    for i in p['fs']:
        if not any(part in label(i) for part in IGNORE):
            sites[i][0] += p['gb']
            sites[i][1] += p['mbk']
print('Top 10 backtrace frames by cumulative bytes (inclusive, shares overlap):')
for i, (b, blocks) in sorted(sites.items(), key=lambda x: -x[1][0])[:10]:
    print(f'  {b / 2**20:8.2f} MiB {blocks:>7} blocks  {label(i)[:96]}')
PY
done
