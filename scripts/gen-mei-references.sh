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

# Exporte le corpus `.mscx` en MEI avec MuseScore 4.7.4 (oracle externe).
#
# Usage : scripts/gen-mei-references.sh [fichier.mscx ...]
#         (sans argument : tout le corpus)
#
# Deux processus mscore au plus (mémoire limitée). Mode headless, jamais la
# session graphique de l'utilisateur. Le LD_LIBRARY_PATH étranger éventuel est
# neutralisé, comme dans gen-references.sh.

set -uo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
CORPUS="$ROOT/tests/corpus"
LOG=$(mktemp)
trap 'rm -f "$LOG"' EXIT

run_clean() {
    env -u LD_LIBRARY_PATH QT_QPA_PLATFORM=offscreen "$@"
}

BIN=$(run_clean nix shell nixpkgs#musescore -c mscore --version 2>/dev/null | head -1)
echo "Oracle : $BIN"
echo "Commande : mscore -o <sortie.ref.mei> <entrée.mscx>"

export_one() {
    local src="$1"
    local out="${src%.*}.ref.mei"
    if run_clean nix shell nixpkgs#musescore -c mscore -o "$out" "$src" >"$LOG" 2>&1 \
        && [[ -s "$out" ]] && grep -q '<mei ' "$out"; then
        printf 'ok %s (%s octets)\n' "$(basename "$src")" "$(wc -c <"$out")"
    else
        printf 'ÉCHEC %s\n' "$(basename "$src")"
        rm -f "$out"
        return 1
    fi
}
export -f run_clean export_one
export LOG

if [[ $# -gt 0 ]]; then
    printf '%s\n' "$@"
else
    printf '%s\n' "$CORPUS"/v3/*.mscx "$CORPUS"/v4/*.mscx
fi | xargs -P 2 -I {} bash -c 'export_one "$@"' _ {}
