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

# Build the GABC reference parser in its own directory, outside the repository.
# Only the upstream *build* is executed; no downloaded script is sourced/run
# for measurement. Uses at most two parallel jobs.
set -eu
SRC=${1:-/home/jacques/programmes/musique/specs/gabc/gregorio}
DST=${2:-/home/jacques/programmes/musique/specs/gabc/gregorio-buildtree}

if [ ! -d "$SRC/.git" ] && [ ! -f "$SRC/configure.ac" ]; then
  echo "missing Gregorio checkout: $SRC" >&2
  exit 2
fi
if [ ! -f "$SRC/configure" ]; then
  nix-shell -p autoconf automake flex bison --run "autoreconf -i '$SRC'"
fi
# In-tree build in a copy: the VPATH build misses generated vowel.h, and
# build.sh also wants a TeX/PDF toolchain we do not need for -F dump.
TREE=$DST
if [ ! -f "$TREE/src/Makefile" ]; then
  rm -rf "$TREE"
  cp -a "$SRC" "$TREE"
  (cd "$TREE" && nix-shell -p autoconf automake flex bison --run \
    "./configure --prefix='$DST/install' >/dev/null && make -j2 >/dev/null")
fi
BIN=$(find "$TREE/src" -maxdepth 1 -type f -name 'gregorio-*' -perm -u+x | head -1)
if [ -z "$BIN" ]; then
  echo "Gregorio binary not built in $TREE/src" >&2
  exit 3
fi
echo "$BIN"
"$BIN" --version | head -1
