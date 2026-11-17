#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 les auteurs de scoretk
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

# Génère les références MusicXML du corpus de test avec le vrai MuseScore.
#
# Pour chaque fichier tests/corpus/{v3,v4}/*.mscx, exporte un MusicXML non
# compressé à côté de la source : <nom>.ref.musicxml.
#
# MuseScore est lancé en mode headless (QT_QPA_PLATFORM=offscreen) : jamais
# dans la session graphique de l'utilisateur.
#
# Variables d'environnement :
#   MSCORE   chemin explicite vers le binaire MuseScore (sinon résolution via
#            `nix shell nixpkgs#musescore`, en essayant mscore, musescore,
#            mscore4portable, ...)
#
# Usage : scripts/gen-references.sh [fichier.mscx ...]
#         (sans argument : tout le corpus)

set -uo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
CORPUS="$ROOT/tests/corpus"

# Certains environnements (nix-shell, wrappers) exportent un LD_LIBRARY_PATH
# étranger qui casse les bibliothèques de MuseScore : on le neutralise.
run_clean() {
    env -u LD_LIBRARY_PATH QT_QPA_PLATFORM=offscreen "$@"
}

if [[ -n "${MSCORE:-}" ]]; then
    MSCORE_CMD=("$MSCORE")
elif command -v nix >/dev/null 2>&1; then
    MSCORE_CMD=(nix shell nixpkgs#musescore -c)
else
    MSCORE_CMD=()
fi

resolve_mscore() {
    local candidate
    for candidate in mscore musescore mscore4portable musescore4 MuseScore4 musescore4portable; do
        if run_clean "${MSCORE_CMD[@]}" "$candidate" --version >/dev/null 2>&1; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    return 1
}

if ! BIN=$(resolve_mscore); then
    echo "Erreur : aucun binaire MuseScore utilisable trouvé." >&2
    echo "Indiquez-le avec MSCORE=/chemin/vers/mscore." >&2
    exit 1
fi

echo "MuseScore : $(run_clean "${MSCORE_CMD[@]}" "$BIN" --version 2>/dev/null | head -1)"

if [[ $# -gt 0 ]]; then
    files=("$@")
else
    files=("$CORPUS"/v3/*.mscx "$CORPUS"/v4/*.mscx)
fi

failed=()
log=$(mktemp)
trap 'rm -f "$log"' EXIT

for src in "${files[@]}"; do
    [[ -f "$src" ]] || continue
    out="${src%.*}.ref.musicxml"
    rm -f "$out"
    printf '  %-40s ' "$(basename "$src")"

    if ! run_clean "${MSCORE_CMD[@]}" "$BIN" -o "$out" "$src" >"$log" 2>&1; then
        echo "ÉCHEC (code $?)"
        sed 's/^/      /' "$log" | tail -5
        failed+=("$src")
        rm -f "$out"
        continue
    fi

    if [[ ! -s "$out" ]] || ! grep -q '<score-partwise' "$out"; then
        echo "ÉCHEC (sortie invalide)"
        sed 's/^/      /' "$log" | tail -5
        failed+=("$src")
        rm -f "$out"
        continue
    fi

    echo "ok ($(wc -c <"$out") octets)"
done

echo
if [[ ${#failed[@]} -eq 0 ]]; then
    echo "Toutes les références ont été générées."
    exit 0
fi

echo "Fichiers en échec (${#failed[@]}) :" >&2
printf '  %s\n' "${failed[@]}" >&2
echo "À lister dans tests/corpus/SOURCES.md et à retirer du corpus." >&2
exit 1
