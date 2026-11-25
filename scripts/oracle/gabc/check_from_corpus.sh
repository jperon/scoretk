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

# check_from_corpus.sh <scoretk-cli> <scratch-dir> [gregorio-binary]
# Proves that GABC written from a score with no L6 payload is accepted by
# Gregorio's own parser, not merely by scoretk's reader.

set -eu
CLI=$1
OUT=$2
G=${3:-/home/jacques/programmes/musique/specs/gabc/gregorio-buildtree/src/gregorio-6_2_0}
ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
SRC="$ROOT/tests/corpus/v3/lyricsline01.mscx"
mkdir -p "$OUT"
"$CLI" "$SRC" -o "$OUT/from-corpus.gabc" --from mscx --to gabc
"$G" -f gabc -F dump "$OUT/from-corpus.gabc" -o "$OUT/from-corpus.dump"
grep -q 'initial_key' "$OUT/from-corpus.dump"
echo "Gregorio accepted GABC written from $SRC"
grep -E 'initial_key|bar ' "$OUT/from-corpus.dump" | head -5
