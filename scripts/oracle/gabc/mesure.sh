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

# mesure.sh <liste|-> <out> <gabctk.py> [venv|-] [mesure.py options...]
# The driver needs mido (nix-shell); gabctk runs in its own venv with -I.
# Use '-' for the manifest together with --selftest.
set -u
D=$(
  dirname "$0"
)
L=$1
O=$2
G=$3
V=${4:-}
shift 4 || true
if [ -z "$V" ] || [ "$V" = "-" ]; then
  V=$O/.venv
fi
mkdir -p "$O"
if [ ! -x "$V/bin/python" ]; then
  python3 -m venv "$V"
fi
LISTE=""
if [ "$L" != "-" ]; then
  LISTE="--liste '$L'"
fi
exec nix-shell -p python3Packages.mido --run \
  "python3 '$D/mesure.py' $LISTE --out '$O' --gabctk '$G' --python '$V/bin/python' $*"
