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

# Build the KOReader helper as a static musl executable for the Kobo Mini.
# The device runs glibc 2.11.1 / kernel 2.6.35: a glibc-linked Rust cdylib
# cannot be loaded there, while a static musl process works.
#
# Usage:
#   nice -n 19 ionice -c3 scripts/build-engrave-serve-kobo.sh
#
# The `kobo` profile (Cargo.toml) inherits `dist`: LTO, opt-level = "s",
# panic = "unwind", strip = true. The target is already installed in rustup;
# this script never modifies the toolchain.

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RUSTUP_AUTO_INSTALL=0
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_LINKER=rust-lld
export RUSTFLAGS="-C link-self-contained=yes -C target-feature=+crt-static"

cargo +stable build --profile kobo -p scoretk-engrave-serve \
  --target armv7-unknown-linux-musleabihf

binary=target/armv7-unknown-linux-musleabihf/kobo/scoretk-engrave-serve
[[ -x "$binary" ]] || { echo "binary not found: $binary" >&2; exit 1; }
file "$binary"
stat -c 'stripped ARM executable: %s bytes' "$binary"
