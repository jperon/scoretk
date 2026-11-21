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

# Régénère la référence de l'oracle de non-régression à partir de la CLI du
# dépôt. La référence est un dossier hors dépôt ; seuls ce script et sa
# documentation sont versionnés. La procédure est celle de `sorties.sh` :
# chaque `.mscx` et `.ref.musicxml` du corpus vers musicxml/mscx/mscz/mxl/ly/
# abc/midi. Le remplacement est atomique et n'a lieu que si toutes les
# conversions ont réussi.
#
# Usage : scripts/oracle/refresh-reference.sh [dossier-de-référence]

set -euo pipefail

REF=${1:-${SCORETK_ORACLE_REFERENCE:-/tmp/nix-shell.7XCuVq/claude-1000/-home-jacques-programmes-musique-musescore/2ae55baa-d7cc-4f59-831d-f77ca3cc7ebd/scratchpad/oracle/ref}}
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)

(cd "$ROOT" && nix develop -c cargo build -q -p scoretk-cli)
B="$ROOT/target/debug/scoretk"
[ -x "$B" ] || { echo "CLI absente : $B" >&2; exit 2; }

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP"

for f in "$ROOT"/tests/corpus/v3/*.mscx "$ROOT"/tests/corpus/v4/*.mscx \
         "$ROOT"/tests/corpus/v3/*.ref.musicxml "$ROOT"/tests/corpus/v4/*.ref.musicxml; do
  n=$(basename "$(dirname "$f")")_$(basename "$f")
  for t in musicxml mscx mscz mxl ly abc midi; do
    if ! "$B" "$f" --to "$t" -o "$TMP/$n.$t" 2>"$TMP/$n.$t.err"; then
      echo "ÉCHEC : $f -> $t" >&2
      exit 1
    fi
    if [ -s "$TMP/$n.$t.err" ]; then
      echo "Avertissement pour $f -> $t :" >&2
      cat "$TMP/$n.$t.err" >&2
    fi
  done
done

OLD=""
[ -d "$REF" ] && OLD="$REF.old.$$" && mv "$REF" "$OLD"
mv "$TMP" "$REF"
[ -n "$OLD" ] && rm -rf "$OLD"
trap - EXIT
echo "Référence régénérée : $REF"
