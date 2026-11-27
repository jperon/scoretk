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

//! Line protocol of the persistent engraving process.
//!
//! Commands: `version`, `open <width> <height> <path>`, `page <n>`,
//! `close`, `quit`. Replies are `ok …` or `err <one-line message>`; a `page`
//! reply is followed by exactly `width × height` raw grey bytes. Every reply
//! is flushed. Nothing is written to stdout except replies.

use std::any::Any;
use std::io::{self, BufRead, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::Document;

/// The line protocol, owning one optional document at a time.
pub struct Server<W: Write> {
    out: W,
    doc: Option<Document>,
}

impl<W: Write> Server<W> {
    pub fn new(out: W) -> Self {
        Self { out, doc: None }
    }

    /// Handles one line; returns `false` when the caller should exit.
    pub fn handle_line(&mut self, line: &str) -> io::Result<bool> {
        let line = line.trim_end_matches(['\n', '\r']);
        let (command, rest) = match line.split_once(' ') {
            Some((command, rest)) => (command, rest),
            None => (line, ""),
        };
        match command {
            "version" => Self::reply(&mut self.out, &format!("ok {}", env!("CARGO_PKG_VERSION")))?,
            "open" => self.open(rest)?,
            "page" => self.page(rest)?,
            "close" => {
                self.doc = None;
                Self::reply(&mut self.out, "ok")?;
            }
            "quit" => return Ok(false),
            _ => Self::reply(&mut self.out, "err unknown command")?,
        }
        Ok(true)
    }

    /// Reads commands until EOF or `quit`. EOF is a normal exit.
    pub fn serve<R: BufRead>(&mut self, mut input: R) -> io::Result<()> {
        let mut line = String::new();
        loop {
            line.clear();
            if input.read_line(&mut line)? == 0 {
                return Ok(());
            }
            if !self.handle_line(&line)? {
                return Ok(());
            }
        }
    }

    fn open(&mut self, rest: &str) -> io::Result<()> {
        // A new document always replaces (and therefore frees) the old one.
        self.doc = None;
        let mut fields = rest.splitn(3, ' ');
        let (Some(width), Some(height), Some(path)) = (fields.next(), fields.next(), fields.next())
        else {
            return Self::reply(
                &mut self.out,
                "err invalid open arguments (expected: open <width> <height> <path>)",
            );
        };
        let Ok(width) = width.trim().parse::<u32>() else {
            return Self::reply(&mut self.out, "err invalid width");
        };
        let Ok(height) = height.trim().parse::<u32>() else {
            return Self::reply(&mut self.out, "err invalid height");
        };
        let path = path.trim_start();
        let outcome = catch_unwind(AssertUnwindSafe(|| Document::open(path, width, height)));
        match outcome {
            Ok(Ok(document)) => {
                let pages = document.page_count();
                self.doc = Some(document);
                Self::reply(&mut self.out, &format!("ok {pages}"))
            }
            Ok(Err(message)) => Self::reply(&mut self.out, &format!("err {}", one_line(&message))),
            Err(payload) => {
                self.doc = None;
                Self::reply(&mut self.out, &format!("err {}", internal_error(payload)))
            }
        }
    }

    fn page(&mut self, rest: &str) -> io::Result<()> {
        let Ok(index) = rest.trim().parse::<usize>() else {
            return Self::reply(&mut self.out, "err invalid page index");
        };
        let Some(document) = self.doc.as_mut() else {
            return Self::reply(&mut self.out, "err no document open");
        };
        let out = &mut self.out;
        // The reply is emitted inside the guarded closure so that the page
        // buffer can be borrowed straight into the caller's writer: no copy
        // of 480 KB is made per page.
        let outcome = catch_unwind(AssertUnwindSafe(|| match document.page(index) {
            Ok(bytes) => {
                let count = bytes.len();
                Self::write_page(out, count, bytes)
            }
            Err(message) => Self::reply(out, &format!("err {}", one_line(&message))),
        }));
        match outcome {
            Ok(result) => result,
            Err(payload) => Self::reply(out, &format!("err {}", internal_error(payload))),
        }
    }

    fn reply(out: &mut W, text: &str) -> io::Result<()> {
        writeln!(out, "{}", one_line(text))?;
        out.flush()
    }

    fn write_page(out: &mut W, count: usize, bytes: &[u8]) -> io::Result<()> {
        writeln!(out, "ok {count}")?;
        out.write_all(bytes)?;
        out.flush()
    }
}

/// Protocol replies must be single lines, even when an error contains one.
fn one_line(text: &str) -> String {
    text.chars()
        .map(|ch| if ch == '\n' || ch == '\r' { ' ' } else { ch })
        .collect()
}

fn internal_error(payload: Box<dyn Any + Send>) -> String {
    let message = if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "unknown panic".to_owned()
    };
    format!("internal error: {}", one_line(&message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_replies_never_contain_a_newline() {
        assert_eq!(one_line("a\nb\r c"), "a b  c");
    }
}
