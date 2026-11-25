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

# Récupère les encodages officiels MEI 5.1 hors du dépôt et des chemins de
# build. Les fichiers sont des DONNÉES NON FIABLES : ils ne sont jamais
# exécutés, seulement lus comme du XML par les tests de mesure.
#
# Usage : SCORETK_MEI_SAMPLES=<dir> scripts/fetch-mei-samples.sh
#   <dir> par défaut : $HOME/.cache/scoretk/mei-samples
#   Les fichiers sont déposés dans <dir>/MEI_5.1/ (manifest :
#   tests/corpus/mei-samples/MANIFEST.tsv).
#
# Licence des données : ECL-2.0, Copyright (c) Music Encoding Initiative
# (MEI) Board ; voir NOTICE et docs/licences-tierces.md. Aucune redistribution
# des fichiers n'est nécessaire pour compiler ou tester : ce script les
# télécharge depuis un commit épinglé et vérifie chaque empreinte SHA-256.

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
COMMIT=f3f1baba02e32279b25dad660b82c834bac034b4
DEST=${SCORETK_MEI_SAMPLES:-$HOME/.cache/scoretk/mei-samples}
MANIFEST="$ROOT/tests/corpus/mei-samples/MANIFEST.tsv"
TOP="sample-encodings-$COMMIT"

command -v curl >/dev/null || { echo "curl requis" >&2; exit 2; }
command -v sha256sum >/dev/null || { echo "sha256sum requis" >&2; exit 2; }

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
echo "MEI sample encodings : commit $COMMIT (données non exécutées)"
curl -fsSL "https://codeload.github.com/music-encoding/sample-encodings/tar.gz/$COMMIT" \
    -o "$TMP/samples.tar.gz"
tar -xzf "$TMP/samples.tar.gz" -C "$TMP" --no-same-owner --no-same-permissions \
    "$TOP/MEI_5.1"

mkdir -p "$DEST"
rm -rf "$DEST/MEI_5.1"
mv "$TMP/$TOP/MEI_5.1" "$DEST/MEI_5.1"

# Vérifie chaque fichier du lot avant de le rendre visible aux tests.
cd "$DEST"
awk -F '\t' '!/^#/ { print $2 "  MEI_5.1/" $1 }' "$MANIFEST" | sha256sum --check --quiet
printf '%-s\n' "$DEST/MEI_5.1"
