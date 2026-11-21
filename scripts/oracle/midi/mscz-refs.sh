#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 jperon
#
# Références MIDI du lot `.mscz` (écriture) : MuseScore 4.7.4 exporte chaque
# `.mscz` en MIDI, en parallèle, avec cache hors dépôt et reprise. Écrit
# `$OUT/manifest.tsv` (source<TAB>référence) et `$OUT/done.txt`, condition
# d'attente pour l'orchestrateur.
#
# Usage : scripts/oracle/midi/mscz-refs.sh <dossier-sortie> [racine]
#   racine par défaut : $MUSIQUE ou /home/jacques/Musique.

set -euo pipefail

OUT=${1:?dossier de sortie exigé}
RACINE=${2:-${MUSIQUE:-/home/jacques/Musique}}
PARALLEL=${PARALLEL:-$(nproc)}
REFS="$OUT/refs"
mkdir -p "$REFS"

# Le nom de la référence est le md5 du chemin source : stable d'une campagne à
# l'autre et sûr pour les espaces et les accents des partitions personnelles.
: > "$OUT/manifest.tsv"
find "$RACINE" -type f -name '*.mscz' -print0 | sort -z |
    while IFS= read -r -d '' src; do
        hash=$(printf '%s' "$src" | md5sum | cut -d' ' -f1 | cut -c1-12)
        printf '%s\t%s\n' "$src" "$REFS/$hash.mid" >> "$OUT/manifest.tsv"
    done

convert_one() {
    local src="$1" dest="$2"
    if [[ -s "$dest" ]]; then
        echo "SKIP $src" >> "$OUT/progress.txt"
        return 0
    fi
    if timeout 240 env -u LD_LIBRARY_PATH QT_QPA_PLATFORM=offscreen \
        mscore -o "$dest" "$src" >/dev/null 2>&1 && [[ -s "$dest" ]]; then
        echo "OK $src" >> "$OUT/progress.txt"
    else
        rm -f "$dest"
        echo "FAIL $src" >> "$OUT/progress.txt"
    fi
}
export -f convert_one
export OUT

: > "$OUT/progress.txt"
rm -f "$OUT/done.txt"
awk -F '\t' '{ printf "%s\t%s\n", $1, $2 }' "$OUT/manifest.tsv" |
    xargs -P "$PARALLEL" -d '\n' -I{} bash -c \
        'IFS=$'"'"'\t'"'"' read -r a b <<< "{}"; convert_one "$a" "$b"'

printf 'DONE %s conversions\n' "$(wc -l < "$OUT/manifest.tsv")" > "$OUT/done.txt"
