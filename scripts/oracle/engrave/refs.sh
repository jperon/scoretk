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

# Exporte les références SVG avec MuseScore 4.7.4, au plus deux en parallèle,
# comme les autres oracles (docs/agents/plan-fiabilite.md §2).
#
# Usage : MSCORE=/chemin/mscore scripts/oracle/engrave/refs.sh <out>

set -euo pipefail

OUT=${1:?dossier de sortie exigé}
PARALLEL=${PARALLEL:-2}
mkdir -p "$OUT/refs" "$OUT/progress"
: > "$OUT/progress/refs.txt"

if [[ -n "${MSCORE:-}" ]]; then
  BIN="$MSCORE"
  USE_NIX=0
else
  USE_NIX=1
  BIN=mscore
fi

version() {
  env -u LD_LIBRARY_PATH QT_QPA_PLATFORM=offscreen "$@" --version 2>/dev/null | head -1
}

if [[ $USE_NIX -eq 1 ]]; then
  echo "MuseScore : $(version nix shell nixpkgs#musescore -c "$BIN")"
else
  echo "MuseScore : $(version "$BIN")"
fi

export BIN OUT USE_NIX

convert_one() {
  local src=$1 id dest
  id=$(basename "$src" .mscx)
  dest="$OUT/refs/$id.svg"
  if [[ -s "$OUT/refs/$id-1.svg" ]]; then
    echo "SKIP $id" >> "$OUT/progress/refs.txt"
    return 0
  fi
  local -a cmd
  if [[ $USE_NIX -eq 1 ]]; then
    cmd=(nix shell nixpkgs#musescore -c "$BIN")
  else
    cmd=("$BIN")
  fi
  if timeout 240 env -u LD_LIBRARY_PATH QT_QPA_PLATFORM=offscreen \
      "${cmd[@]}" -o "$dest" "$src" >/dev/null 2>&1 \
      && compgen -G "$OUT/refs/$id-*.svg" >/dev/null; then
    echo "OK $id" >> "$OUT/progress/refs.txt"
  else
    rm -f "$dest"
    echo "FAIL $id" >> "$OUT/progress/refs.txt"
  fi
}
export -f convert_one

cat "$OUT/lots/corpus.txt" | xargs -P "$PARALLEL" -d '\n' -I{} bash -c 'convert_one "$1"' _ {}
printf 'références : %s OK, %s échecs\n' \
  "$(grep -c '^OK' "$OUT/progress/refs.txt" || true)" \
  "$(grep -c '^FAIL' "$OUT/progress/refs.txt" || true)"
