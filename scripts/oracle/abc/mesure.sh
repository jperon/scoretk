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

# mesure.sh <liste> <dossier_sorties> <binaire_scoretk> [abc2xml.py]
# La liste contient une ligne « lot<tabulation>chemin.fichier.abc » par fichier.
# abc2midi est fourni par nixpkgs ; abc2xml.py reste hors du dépôt.
set -u
D=$(dirname "$0")
L=$1
O=$2
B=$3
A=${4:-}
mkdir -p "$O"
if [ -n "$A" ]; then
  exec nix-shell -p abcmidi --run "python3 '$D/mesure.py' --liste '$L' --out '$O' --scoretk '$B' --abc2xml '$A'"
fi
exec nix-shell -p abcmidi --run "python3 '$D/mesure.py' --liste '$L' --out '$O' --scoretk '$B'"
