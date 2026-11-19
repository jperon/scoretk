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

# Enveloppe : fournit `mido` (absent de l'environnement Nix du projet) et
# transmet les arguments à mesure.py. La CLI scoretk doit être compilée :
#   nix develop -c cargo build -p scoretk-cli
#
# Usage :
#   scripts/oracle/midi/lots.sh <dossier-sortie>
#   scripts/oracle/midi/mesure.sh <dossier-sortie> <binaire-scoretk> [mscore] [--limit N]

set -euo pipefail

DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
OUT=${1:?dossier de sortie exigé}
BIN=${2:?binaire scoretk exigé}
MSCORE=${3:-}
shift 2 || true
if [[ -n "$MSCORE" && "$MSCORE" != --* ]]; then shift; else MSCORE=""; fi
M="$OUT/manifestes"

ARGS=(--out "$OUT" --scoretk "$BIN")
[[ -n "$MSCORE" ]] && ARGS+=(--mscore "$MSCORE")
ARGS+=(--lecture "lecture_musique=$M/lecture_musique.txt")
ARGS+=(--lecture "lecture_mutopia=$M/lecture_mutopia.txt")
ARGS+=(--ecriture "ecriture_ly=$M/ecriture_ly.txt")
ARGS+=(--ecriture "ecriture_mscz_corpus=$M/ecriture_mscz_corpus.txt")
ARGS+=(--ecriture "ecriture_mscz_musique=$M/ecriture_mscz_musique.txt")
ARGS+=(--robustesse "robustesse_lakh=$M/robustesse_lakh.txt")
ARGS+=("$@")

exec nix-shell -p python3Packages.mido --run "python3 '$DIR/mesure.py' $(printf '%q ' "${ARGS[@]}")"
