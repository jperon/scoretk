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

# mesure.sh <liste|-> <out> [gregorio-binary] [mesure.py options...]
# Gregorio is the only reference. Use '-' for the manifest with --selftest.
set -u
D=$(
  dirname "$0"
)
L=$1
O=$2
G=${3:-/home/jacques/programmes/musique/specs/gabc/gregorio-buildtree/src/gregorio-6_2_0}
case $G in
  --*)
    G=/home/jacques/programmes/musique/specs/gabc/gregorio-buildtree/src/gregorio-6_2_0
    shift 2 || true
    ;;
  *) shift 3 || true ;;
esac
mkdir -p "$O"
if [ "$L" = "-" ]; then
  exec python3 "$D/mesure.py" --out "$O" --gregorio "$G" "$@"
fi
exec python3 "$D/mesure.py" --liste "$L" --out "$O" --gregorio "$G" "$@"
