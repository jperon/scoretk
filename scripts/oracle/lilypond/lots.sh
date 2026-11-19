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

# Construit les manifestes des trois lots à partir de la colonne 6 des listes
# de référence hors dépôt. Aucune donnée tierce n'est copiée dans le dépôt.
#
# Usage : BASEDIR=<dossier des *-base.txt> scripts/oracle/lilypond/lots.sh <sortie>
# Le dossier par défaut est l'espace de travail hors dépôt utilisé par la
# campagne de mesure ; le surcharger pour une autre installation.

set -euo pipefail

OUT=${1:?dossier de sortie exigé}
BASEDIR=${BASEDIR:-/tmp/nix-shell.7XCuVq/claude-1000/-home-jacques-programmes-musique-musescore/2ae55baa-d7cc-4f59-831d-f77ca3cc7ebd/scratchpad/xmlcmp}
mkdir -p "$OUT/manifestes"
for lot in mus ra mut; do
  source="$BASEDIR/$lot-base.txt"
  if [[ ! -f "$source" ]]; then
    echo "liste absente : $source (poser BASEDIR)" >&2
    exit 1
  fi
  awk 'NF >= 6 {print $6}' "$source" > "$OUT/manifestes/$lot.txt"
done
for f in "$OUT"/manifestes/*.txt; do
  printf '%-8s %6d\n' "$(basename "$f")" "$(wc -l < "$f")"
done
