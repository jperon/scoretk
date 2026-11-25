#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon
#
# Ce fichier est original : validation RelaxNG des sorties MEI 5.1 Neumes.
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
#
# Usage : scripts/check-mei-neumes.sh [binaire scoretk] [fichier.mei …]
#
# Sans argument de fichier, convertit les neuf exemples Gregorio de
# tests/corpus/gabc en MEI avec le binaire (par défaut target/debug/scoretk),
# puis valide chaque sortie avec `jing` contre `mei-all-5.1.rng` (MEI 5.1,
# ECL-2.0). Les fichiers MEI passés en argument sont validés en plus des
# sorties générées. Le schéma est lu dans $SCORETK_SPECS_DIR/mei/ (par défaut
# /home/jacques/programmes/musique/specs), comme dans docs/agents/mei.md §9.

set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
specs=${SCORETK_SPECS_DIR:-/home/jacques/programmes/musique/specs}
schema="$specs/mei/mei-all-5.1.rng"
cli=${1:-"$root/target/debug/scoretk"}
if [[ $# -gt 0 ]]; then
  shift
fi

if [[ ! -f "$schema" ]]; then
  echo "schéma MEI introuvable : $schema" >&2
  exit 2
fi
if [[ ! -x "$cli" ]]; then
  echo "binaire scoretk introuvable : $cli (nix develop -c cargo build -p scoretk-cli)" >&2
  exit 2
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/scoretk-mei-neumes.XXXXXX")
trap 'rm -rf "$work"' EXIT

count=0
for source in "$root"/tests/corpus/gabc/*.gabc; do
  name=$(basename "$source" .gabc)
  "$cli" "$source" -o "$work/$name.mei"
  nix shell nixpkgs#jing --command jing "$schema" "$work/$name.mei"
  count=$((count + 1))
done

for extra in "$@"; do
  nix shell nixpkgs#jing --command jing "$schema" "$extra"
  count=$((count + 1))
done

echo "MEI 5.1 neumes : $count fichier(s) valide(s) contre mei-all-5.1.rng"
