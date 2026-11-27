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

# Sélectionne **tous** les fichiers `.mscx` du corpus, y compris les partitions
# à plusieurs portées. Depuis P5, le graveur accepte toutes les portées
# visibles de la partition principale ; les fichiers encore en échec sont
# comptés comme tels dans l'oracle, jamais exclus du dénominateur.
#
# Usage : scripts/oracle/engrave/lots.sh <dossier-de-sortie>

set -euo pipefail

OUT=${1:?dossier de sortie exigé}
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
mkdir -p "$OUT/lots"
: > "$OUT/lots/corpus.txt"

for dir in "$ROOT/tests/corpus/v3" "$ROOT/tests/corpus/v4"; do
  [[ -d "$dir" ]] || continue
  for file in "$dir"/*.mscx; do
    [[ -f "$file" ]] || continue
    printf '%s\n' "$file" >> "$OUT/lots/corpus.txt"
  done
done
printf 'lots %s fichiers (toutes portées confondues)\n' "$(wc -l < "$OUT/lots/corpus.txt")"
