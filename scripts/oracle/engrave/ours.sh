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

# Grave les mêmes fichiers avec scoretk (page 0), au plus deux en parallèle.
# La CLI est compilée avec la feature `engrave`.
#
# Usage : scripts/oracle/engrave/ours.sh <out> <binaire-scoretk>

set -euo pipefail

OUT=${1:?dossier de sortie exigé}
SCORETK=${2:?binaire scoretk exigé}
PARALLEL=${PARALLEL:-2}
mkdir -p "$OUT/ours" "$OUT/progress"
: > "$OUT/progress/ours.txt"
export OUT SCORETK

convert_one() {
  local src=$1 id
  id=$(basename "$src" .mscx)
  local dest="$OUT/ours/$id.svg"
  if [[ -s "$dest" ]]; then
    echo "SKIP $id" >> "$OUT/progress/ours.txt"
    return 0
  fi
  if timeout 10 prlimit --as=2000000000 "$SCORETK" "$src" -o "$dest" \
      --page 0 >/dev/null 2>&1 && [[ -s "$dest" ]]; then
    echo "OK $id" >> "$OUT/progress/ours.txt"
  else
    rm -f "$dest"
    echo "FAIL $id" >> "$OUT/progress/ours.txt"
  fi
}
export -f convert_one

cat "$OUT/lots/corpus.txt" | xargs -P "$PARALLEL" -d '\n' -I{} bash -c 'convert_one "$1"' _ {}
printf 'notre gravure : %s OK, %s échecs\n' \
  "$(grep -c '^OK' "$OUT/progress/ours.txt" || true)" \
  "$(grep -c '^FAIL' "$OUT/progress/ours.txt" || true)"
