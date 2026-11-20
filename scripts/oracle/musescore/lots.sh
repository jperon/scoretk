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

# Construit les manifestes de l'oracle d'écriture MuseScore en réutilisant
# exactement ceux des oracles d'écriture LilyPond et ABC : corpus (56 .mscx),
# mscz (174 fichiers de ~/Musique), xml (vrais MusicXML de ~/Musique),
# ly (1/5 déterministe) et abc (1/5 déterministe). Aucune donnée tierce n'est
# copiée dans le dépôt : seuls des chemins absolus sont écrits.
#
# Usage : scripts/oracle/musescore/lots.sh <dossier-de-sortie>

set -euo pipefail

OUT=${1:?dossier de sortie exigé}
DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
MUSIQUE=${MUSIQUE:-$HOME/Musique}
SAMPLE=${SAMPLE:-5}
mkdir -p "$OUT/manifestes"

bash "$DIR/../lilypond/lots-ecriture.sh" "$OUT" >/dev/null
find "$MUSIQUE" -name '*.abc' -type f | sort | awk -v n="$SAMPLE" 'NR % n == 1' \
  > "$OUT/manifestes/abc.txt"

for f in "$OUT"/manifestes/*.txt; do
  printf '%-10s %6d\n' "$(basename "$f")" "$(wc -l < "$f")"
done
