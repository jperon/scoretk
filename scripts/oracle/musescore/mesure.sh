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

# Enveloppe : fournit `mido` (lecture SMF, absent du shell Nix du projet) et
# transmet les arguments à mesure.py. La CLI scoretk doit être compilée :
#   nix develop -c cargo build -p scoretk-cli
#
# Usage :
#   scripts/oracle/musescore/lots.sh /tmp/oracle-mscz/lots
#   scripts/oracle/musescore/mesure.sh --out /tmp/oracle-mscz/base \
#     --scoretk /tmp/scoretk-mscz-base --mscore mscore \
#     --lot mscz=/tmp/oracle-mscz/lots/manifestes/mscz.txt
#   scripts/oracle/musescore/mesure.sh --out /tmp/oracle-mscz/base --resume

set -euo pipefail

DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
exec nix-shell -p python3Packages.mido --run \
  "exec python3 '$DIR/mesure.py' $(printf '%q ' "$@")"
