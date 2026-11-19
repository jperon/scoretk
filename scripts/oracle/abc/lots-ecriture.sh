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

# Construit les manifestes de l'oracle d'écriture ABC en réutilisant ceux de
# l'oracle LilyPond (les trois premiers lots sont les mêmes). Aucune donnée
# tierce n'est copiée dans le dépôt : seuls des chemins absolus sont écrits.
#
#   corpus : 56 .mscx versionnés (avec leurs *.ref.musicxml)
#   mscz   : les 174 .mscz de ~/Musique
#   xml    : les 32 vrais fichiers MusicXML de ~/Musique
#
# Usage : scripts/oracle/abc/lots-ecriture.sh <dossier-de-sortie>

set -euo pipefail

OUT=${1:?dossier de sortie exigé}
DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
"$DIR/../lilypond/lots-ecriture.sh" "$OUT"
