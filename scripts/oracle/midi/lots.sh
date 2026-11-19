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

# Construit les manifestes des lots (listes de chemins, jamais de données
# tierces versionnées). Usage : scripts/oracle/midi/lots.sh <dossier-sortie>
#
# Les racines peuvent être surchargées : MUSIQUE, MUTOPIA, LAKH.

set -euo pipefail

OUT=${1:?dossier de sortie exigé}
RACINE=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
MUSIQUE=${MUSIQUE:-/home/jacques/Musique}
MUTOPIA=${MUTOPIA:-/home/jacques/programmes/musique/corpus/mutopia}
LAKH=${LAKH:-/home/jacques/programmes/musique/corpus/lakh-midi-clean}
mkdir -p "$OUT/manifestes"
M="$OUT/manifestes"

# Lecture : MIDI LilyPond dont on a la source .ly à côté -> oracle N1/N2.
find "$MUSIQUE" -type f -name '*.midi' -print0 |
    while IFS= read -r -d '' f; do
        if [[ -f "${f%.midi}.ly" ]]; then printf '%s\n' "$f"; fi
    done | sort > "$M/lecture_musique.txt"

# Lecture : MIDI du corpus Mutopia (source LilyPond dans l'arbre, non appariée
# par nom) -> oracle N1/N2 sur le MIDI lui-même.
find "$MUTOPIA" -type f -name '*.mid' -print | sort > "$M/lecture_mutopia.txt"

# Écriture depuis .ly : référence = MIDI produit par LilyPond.
find "$MUSIQUE" -type f -name '*.midi' -print0 |
    while IFS= read -r -d '' f; do
        if [[ -f "${f%.midi}.ly" ]]; then printf '%s\t%s\n' "${f%.midi}.ly" "$f"; fi
    done | sort > "$M/ecriture_ly.txt"

# Écriture depuis .mscx du corpus : référence = MIDI exporté par MuseScore 4.7.4.
: > "$M/ecriture_mscz_corpus.txt"
for version in v3 v4; do
    for ref in "$RACINE"/tests/corpus/midi/"$version"/*.ref.mid; do
        [[ -f "$ref" ]] || continue
        nom=$(basename "$ref" .ref.mid)
        src="$RACINE/tests/corpus/$version/$nom.mscx"
        if [[ -f "$src" ]]; then printf '%s\t%s\n' "$src" "$ref" >> "$M/ecriture_mscz_corpus.txt"; fi
    done
done

# Écriture depuis .mscz de ~/Musique : référence = MIDI MuseScore à côté (8 cas).
find "$MUSIQUE" -type f -name '*.mscz' -print0 |
    while IFS= read -r -d '' f; do
        if [[ -f "${f%.mscz}.midi" ]]; then printf '%s\t%s\n' "$f" "${f%.mscz}.midi"; fi
    done | sort > "$M/ecriture_mscz_musique.txt"

# Robustesse seule : corpus tiers Lakh, sans source appariée. LAKH_STRIDE
# permet un échantillon déterministe : ce corpus contient des fichiers très
# denses pour lesquels l'import met plus de 10 s (borne de la campagne).
LAKH_STRIDE=${LAKH_STRIDE:-1}
find "$LAKH" -type f -name '*.mid' -print | sort |
    awk -v pas="$LAKH_STRIDE" 'NR % pas == 1' > "$M/robustesse_lakh.txt"

for f in "$M"/*.txt; do printf '%-28s %6d\n' "$(basename "$f")" "$(wc -l < "$f")"; done
