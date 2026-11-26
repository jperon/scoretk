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

# Wrapper providing `xmllint` (libxml2), absent from the project's nix shell.
# With no argument, the workspace-built `target/debug/scoretk` is used
# (`$CARGO_TARGET_DIR` is honoured); with no binary, usage is printed and the
# script exits non-zero. Build the CLI first, then (both modes are validated
# in one run):
#   scripts/oracle/musicxml/valide-corpus.sh
#   scripts/oracle/musicxml/valide-corpus.sh --cli target/debug/scoretk --out /tmp/xml-xsd
# (a) default export = MuseScore 4.7.4 parity; (b) --xml-full-support = XSD only.

set -euo pipefail

DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

# Sans argument, utiliser le binaire construit dans l'espace de travail.
# On ne laisse jamais Python produire une erreur d'argument manquant : faute
# de binaire, on affiche l'usage et on échoue explicitement.
if [ "$#" -eq 0 ]; then
  ROOT=$(cd "$DIR/../../.." && pwd)
  TARGET=${CARGO_TARGET_DIR:-$ROOT/target}
  CLI="$TARGET/debug/scoretk"
  if [ ! -x "$CLI" ]; then
    printf 'Usage: %s --cli <scoretk-binary> [--out <dir>]\n' "$0" >&2
    printf 'Build the CLI first: nix develop -c cargo build -p scoretk-cli\n' >&2
    printf 'Expected binary: %s\n' "$CLI" >&2
    exit 2
  fi
  set -- --cli "$CLI"
fi

exec nix-shell -p libxml2 --run \
  "exec python3 '$DIR/valide-corpus.py' $(printf '%q ' "$@")"
