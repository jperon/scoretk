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

# one.sh <dossier_sorties> <binaire_scoretk> <fichier.abc> [abc2xml.py]
# Mesure un seul fichier ; interface calquée sur l'oracle LilyPond one.sh.
set -u
D=$(dirname "$0")
O=$1
B=$2
F=$3
A=${4:-}
T=$(mktemp)
trap 'rm -f "$T"' EXIT
printf 'un\t%s\n' "$F" > "$T"
exec "$D/mesure.sh" "$T" "$O" "$B" "$A"
