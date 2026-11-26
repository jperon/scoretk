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
# Build the CLI first, then (both modes are validated in one run):
#   scripts/oracle/musicxml/valide-corpus.sh --cli target/debug/scoretk
#   scripts/oracle/musicxml/valide-corpus.sh --cli target/debug/scoretk --out /tmp/xml-xsd
# (a) default export = MuseScore 4.7.4 parity; (b) --xml-full-support = XSD only.

set -euo pipefail

DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
exec nix-shell -p libxml2 --run \
  "exec python3 '$DIR/valide-corpus.py' $(printf '%q ' "$@")"
