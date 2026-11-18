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

# Vérifie que MuseScore 4.7.4 rouvre les fichiers écrits par
# scoretk-musescore-export.
#
# Pour chaque .mscx du corpus : réécriture avec `scoretk` (sortie .mscx et
# .mscz), puis ouverture et export MusicXML par MuseScore headless. Le script
# ne compare pas le MusicXML à la référence (la mise en page n'est pas
# modélisée) : il vérifie que MuseScore relit le fichier et produit un
# `<score-partwise>` non vide.
#
# MuseScore est lancé en mode headless (QT_QPA_PLATFORM=offscreen) : jamais
# dans la session graphique de l'utilisateur.
#
# Variables d'environnement :
#   MSCORE   chemin explicite vers le binaire MuseScore (sinon `musescore`)
#   CARGO    commande d'exécution cargo (défaut : `nix develop -c cargo`)
#
# Usage : scripts/check-mscx-rewrite.sh [fichier.mscx ...]
#         (sans argument : tout le corpus)

set -uo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
CORPUS="$ROOT/tests/corpus"
MSCORE=${MSCORE:-musescore}
if [[ -n "${CARGO:-}" ]]; then
    read -r -a CARGO_CMD <<<"$CARGO"
else
    CARGO_CMD=(nix develop -c cargo)
fi

run_clean() {
    env -u LD_LIBRARY_PATH QT_QPA_PLATFORM=offscreen "$@"
}

echo "MuseScore : $(run_clean "$MSCORE" --version 2>/dev/null | head -1)"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
log="$work/musescore.log"

if [[ $# -gt 0 ]]; then
    files=("$@")
else
    files=("$CORPUS"/v3/*.mscx "$CORPUS"/v4/*.mscx)
fi

failed=()
for src in "${files[@]}"; do
    [[ -f "$src" ]] || continue
    name=$(basename "$src" .mscx)
    printf '  %-32s ' "$name"

    for extension in mscx mscz; do
        out="$work/$name.$extension"
        if ! "${CARGO_CMD[@]}" run -q -p scoretk-cli -- "$src" -o "$out" \
                >"$log" 2>&1; then
            echo "ÉCHEC écriture .$extension"
            sed 's/^/      /' "$log" | tail -3
            failed+=("$src (.$extension)")
            continue 2
        fi
    done

    status=0
    for extension in mscx mscz; do
        musicxml="$work/$name.$extension.musicxml"
        if ! run_clean "$MSCORE" -o "$musicxml" "$work/$name.$extension" \
                >"$log" 2>&1 \
            || [[ ! -s "$musicxml" ]] \
            || ! grep -q '<score-partwise' "$musicxml"; then
            echo "ÉCHEC relecture .$extension"
            sed 's/^/      /' "$log" | tail -3
            status=1
        fi
    done
    if [[ $status -ne 0 ]]; then
        failed+=("$src")
        continue
    fi
    echo "ok"
done

echo
if [[ ${#failed[@]} -eq 0 ]]; then
    echo "MuseScore rouvre toutes les sorties."
    exit 0
fi

echo "Fichiers refusés par MuseScore (${#failed[@]}) :" >&2
printf '  %s\n' "${failed[@]}" >&2
exit 1
