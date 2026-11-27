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

# Pilote de l'oracle de gravure : lots → références MuseScore → notre SVG →
# comparaison. Les conversions externes sont limitées à deux en parallèle.
#
# Usage :
#   scripts/oracle/engrave/mesure.sh <out> <binaire-scoretk>
#   scripts/oracle/engrave/mesure.sh --resume <out>

set -euo pipefail

DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
if [[ ${1:-} == "--resume" ]]; then
  OUT=${2:?dossier de sortie exigé}
else
  OUT=${1:?dossier de sortie exigé}
  SCORETK=${2:?binaire scoretk exigé}
  bash "$DIR/lots.sh" "$OUT"
  bash "$DIR/refs.sh" "$OUT"
  bash "$DIR/ours.sh" "$OUT" "$SCORETK"
fi
python3 -I "$DIR/compare.py" "$OUT"
