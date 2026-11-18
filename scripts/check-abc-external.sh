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

# Vérifie les sorties ABC avec abcm2ps et abc2midi, hors shell de développement.
#
# Compile le test dans `nix develop`, puis l'exécute dans un `nix shell` qui
# fournit les deux outils (nixpkgs). Aucun accès réseau n'est nécessaire une
# fois les paquets en cache.

set -euo pipefail

racine="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$racine"

nix develop -c cargo test -p scoretk --features abc-import,abc-export --test abc_outils_externes --no-run >/dev/null
binaire="$(find target -type f -executable -name 'abc_outils_externes-*' | head -n 1)"
if [ -z "$binaire" ]; then
    echo "binaire de test abc_outils_externes introuvable" >&2
    exit 1
fi

exec nix shell nixpkgs#abcm2ps nixpkgs#abcmidi -c "$binaire" --ignored --nocapture
