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

# Enveloppe de l'oracle d'écriture ABC : fournit `abcmidi` (`abc2midi`) et
# `mido` (lecture SMF), puis transmet les arguments à ecriture.py. MuseScore
# sert à produire les MIDI/MusicXML de référence ; son paquet nixpkgs courant
# charge un libasound bâti contre une glibc plus récente, d'où le
# LD_LIBRARY_PATH de compatibilité (surchargeable).
#
# La CLI scoretk doit être compilée : nix develop -c cargo build --release -p scoretk-cli
#
# Usage :
#   scripts/oracle/abc/ecriture.sh --out <dossier> \
#       --scoretk /tmp/abc-ecriture/bin/scoretk-base \
#       --xml2abc /tmp/abcchk/xml2abc_177/xml2abc.py \
#       --lot corpus=<manifest> --lot mscz=<manifest> --lot xml=<manifest>
#   scripts/oracle/abc/ecriture.sh --out <dossier> --resume
#   scripts/oracle/abc/ecriture.sh --selftest

set -euo pipefail

DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
if [[ -z "${MSCORE_LIBS:-}" ]]; then
  lib=$(find /nix/store -path '*alsa-lib-1.2.15.3*/lib/libasound.so.2' 2>/dev/null | head -1 || true)
  MSCORE_LIBS=${lib:+$(dirname "$lib")}
fi
export MSCORE_LIBS
exec nix-shell -p abcmidi python3Packages.mido --run \
  "export LD_LIBRARY_PATH=\"\${MSCORE_LIBS:+\$MSCORE_LIBS:}\${LD_LIBRARY_PATH:-}\"; exec python3 '$DIR/ecriture.py' $(printf '%q ' "$@")"
