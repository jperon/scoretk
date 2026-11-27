// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see the LICENSE file at the root of this repository or
// <https://www.gnu.org/licenses/gpl-3.0.html>.

//! Persistent engraving process spoken to over stdin/stdout by KOReader.

use std::io::{self, BufReader, BufWriter};

use scoretk_engrave_serve::Server;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut server = Server::new(BufWriter::new(stdout.lock()));
    if let Err(error) = server.serve(BufReader::new(stdin.lock())) {
        // stdout is reserved for the protocol; diagnostics go to stderr.
        eprintln!("scoretk-engrave-serve: {error}");
        std::process::exit(1);
    }
}
