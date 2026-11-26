#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon
#
# Ce fichier est original : validation des sorties MEI 5.1 à deux portées
# (GABC NABC en neumes manuscrits) contre la RNG et les règles Schematron
# intégrées, puis mesure du round trip GABC → MEI → GABC.
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

# Usage : scripts/check-mei-nabc.sh
#
# Depuis la racine du dépôt : exporte les trois exemples Gregorio pourvus de
# NABC en MEI 5.1 `mei_full_support`, valide chaque fichier avec `jing`
# contre `mei-all-5.1.rng`, exécute les 249 assertions Schematron intégrées
# au RNG avec Saxon HE, puis affiche les mesures de couverture du test
# `gabc_nabc`. Schéma : $SCORETK_SPECS_DIR/mei/ (ECL-2.0).

set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
specs=${SCORETK_SPECS_DIR:-/home/jacques/programmes/musique/specs}
schema="$specs/mei/mei-all-5.1.rng"
work=$(mktemp -d "${TMPDIR:-/tmp}/scoretk-mei-nabc.XXXXXX")
trap 'rm -rf "$work"' EXIT

if [[ ! -f "$schema" ]]; then
  echo "schéma MEI introuvable : $schema" >&2
  exit 2
fi

export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$root/target-meinabc}
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
export SCORETK_MEI_NABC_DUMP_DIR="$work"

nice -n 10 nix develop -c env CARGO_BUILD_JOBS="$CARGO_BUILD_JOBS" \
  cargo test -p scoretk --test gabc_nabc -- --nocapture

files=("$work"/*.gabc)
if [[ ${#files[@]} -ne 3 ]]; then
  echo "attendu 3 sorties NABC, trouvé ${#files[@]}" >&2
  exit 1
fi
nix shell nixpkgs#jing --command jing "$schema" "${files[@]}"
echo "RelaxNG: ${#files[@]}/${#files[@]} sorties NABC valides contre mei-all-5.1.rng"

jar=$(find /nix/store -maxdepth 4 -name 'saxon-he-11*.jar' 2>/dev/null | head -1 || true)
if [[ -n "$jar" ]]; then
  nix shell nixpkgs#jre --command python3 -I \
    scripts/oracle/mei/schematron.py --rng "$schema" --jar "$jar" --mei "${files[@]}"
else
  echo "Saxon HE absent : Schematron non exécuté (nix shell nixpkgs#saxon_11-he)" >&2
fi
