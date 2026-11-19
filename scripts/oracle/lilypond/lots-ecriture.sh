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

# Construit les quatre manifestes de l'oracle d'écriture. Aucune donnée tierce
# n'est copiée dans le dépôt : seuls des chemins absolus sont écrits.
#
#   corpus : 56 .mscx versionnés (avec leurs *.ref.musicxml)
#   mscz   : les 174 .mscz de ~/Musique (référence MIDI/MusicXML MuseScore)
#   xml    : les .xml/.musicxml de ~/Musique (eux-mêmes MusicXML de référence)
#   ly     : 1/5 déterministe des .ly de ~/Musique (aller-retour ly → scoretk → ly)
#
# Usage : scripts/oracle/lilypond/lots-ecriture.sh <dossier-de-sortie>

set -euo pipefail

OUT=${1:?dossier de sortie exigé}
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
MUSIQUE=${MUSIQUE:-$HOME/Musique}
SAMPLE=${SAMPLE:-5}
mkdir -p "$OUT/manifestes"

find "$REPO/tests/corpus/v3" "$REPO/tests/corpus/v4" -maxdepth 1 -name '*.mscx' -type f \
  | sort > "$OUT/manifestes/corpus.txt"
find "$MUSIQUE" -name '*.mscz' -type f | sort > "$OUT/manifestes/mscz.txt"
# Seuls les vrais fichiers MusicXML (déclaration <score-partwise> / <opus>),
# pas les .xml de configuration qui traînent dans ~/Musique.
: > "$OUT/manifestes/xml.txt"
set +o pipefail  # `head -c` ferme le tuyau tôt : SIGPIPE attendu, pas une erreur
while IFS= read -r -d '' f; do
  case "$f" in
    *.mxl) head=$(unzip -p "$f" 2>/dev/null | head -c 4096) ;;
    *)     head=$(head -c 4096 "$f") ;;
  esac
  if [[ "$head" == *'<score-partwise'* || "$head" == *'<score-timewise'* || "$head" == *'<opus'* ]]; then
    printf '%s\n' "$f" >> "$OUT/manifestes/xml.txt"
  fi
done < <(find "$MUSIQUE" \( -name '*.xml' -o -name '*.musicxml' -o -name '*.mxl' \) -type f -print0 | sort -z)
set -o pipefail
find "$MUSIQUE" -name '*.ly' -type f | sort | awk -v n="$SAMPLE" 'NR % n == 1' \
  > "$OUT/manifestes/ly.txt"

for f in "$OUT"/manifestes/*.txt; do
  printf '%-10s %6d\n' "$(basename "$f")" "$(wc -l < "$f")"
done
