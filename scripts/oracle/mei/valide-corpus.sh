#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 jperon
#
# Ce fichier est original : wrapper fournissant `jing`, absent du shell Nix
# du projet, à l'oracle de validation RelaxNG des corpus MEI.
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

# Sans argument, depuis la racine du dépôt : construit le CLI si besoin,
# exporte les 56 partitions dans les deux modes, puis valide chaque sortie
# avec `jing` (mode par défaut : mei-basic.rng ; mode additif : mei-all.rng).
# Le code de sortie reflète le mode additif, seul utilisé pour la mesure de
# couverture ; les violations du mode par défaut sont affichées mais pas
# masquées (elles viennent de master, dont les octets doivent rester identiques).
#
#   scripts/oracle/mei/valide-corpus.sh
#   scripts/oracle/mei/valide-corpus.sh --out /tmp/mei-rng
#   scripts/oracle/mei/valide-corpus.sh --schematron
#
# `--schematron` ajoute les 249 assertions ISO Schematron intégrées au RNG,
# que `jing` ne vérifie pas. Elles sont compilées en XSLT 3.0 par
# `schematron.py` (code local, aucun stylesheet téléchargé) et exécutées avec
# Saxon HE 11. Les avertissements sont informatifs, les erreurs font échouer
# l'oracle. L'option impose `--out` pour retrouver les fichiers exportés.

set -euo pipefail

DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$DIR/../../.." && pwd)
cd "$ROOT"

ARGS=()
SCHEMATRON=0
for arg in "$@"; do
  if [[ "$arg" == "--schematron" ]]; then
    SCHEMATRON=1
  else
    ARGS+=("$arg")
  fi
done
if [[ $SCHEMATRON == 1 ]] && ! printf '%s\n' "${ARGS[@]}" | grep -qx -- '--out'; then
  ARGS+=(--out "${TMPDIR:-/tmp}/scoretk-mei-schematron")
fi

TARGET=${CARGO_TARGET_DIR:-$ROOT/target}
CLI="$TARGET/debug/scoretk"
if [[ ! -x "$CLI" ]]; then
  printf 'Construction du CLI : %s\n' "$CLI" >&2
  CARGO_TARGET_DIR="$TARGET" CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2} \
    nice -n 10 nix develop -c cargo build -p scoretk-cli
fi

if [[ $SCHEMATRON == 0 ]]; then
  exec nix shell nixpkgs#jing --command \
    python3 "$DIR/valide-corpus.py" --cli "$CLI" "${ARGS[@]}"
fi

OUT=""
previous=""
for arg in "${ARGS[@]}"; do
  if [[ "$previous" == "--out" ]]; then OUT="$arg"; fi
  previous="$arg"
done

nix shell nixpkgs#jing --command \
  python3 -I "$DIR/valide-corpus.py" --cli "$CLI" "${ARGS[@]}"

# `nix shell nixpkgs#saxon_11-he` realises the jar; it is then found in the
# store. The version is pinned by nixpkgs, not by a downloaded binary.
JAR=$(find /nix/store -maxdepth 4 -name 'saxon-he-11*.jar' 2>/dev/null | head -1 || true)
if [[ ! -f "$JAR" ]]; then
  printf 'Saxon HE introuvable (nixpkgs#saxon_11-he)\n' >&2
  exit 1
fi
SPECS=${SCORETK_SPECS_DIR:-/home/jacques/programmes/musique/specs}
python3 -I "$DIR/schematron.py" --selftest
exec nix shell nixpkgs#jre --command python3 -I "$DIR/schematron.py" \
  --rng "$SPECS/mei/mei-all-5.1.rng" --jar "$JAR" \
  --mei "$OUT"/full/*.mei
