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

//! Dialogue tests of the `scoretk-engrave-serve` process protocol.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};

const WIDTH: usize = 600;
const HEIGHT: usize = 800;

fn corpus(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(relative)
}

/// Longest continuous run of dark pixels in one image row.
fn longest_dark_run(row: &[u8]) -> usize {
    let mut run = 0;
    let mut longest = 0;
    for value in row {
        if *value < 128 {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    longest
}

struct Peer {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Peer {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_scoretk-engrave-serve"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start scoretk-engrave-serve");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, command: &str) {
        writeln!(self.stdin, "{command}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn line(&mut self) -> String {
        let mut line = String::new();
        assert!(
            self.stdout.read_line(&mut line).unwrap() > 0,
            "EOF on stdout"
        );
        line.trim_end_matches(['\n', '\r']).to_owned()
    }

    fn page(&mut self, index: usize) -> Vec<u8> {
        self.send(&format!("page {index}"));
        let header = self.line();
        assert_eq!(
            header,
            format!("ok {}", WIDTH * HEIGHT),
            "unexpected {header:?}"
        );
        let mut bytes = vec![0u8; WIDTH * HEIGHT];
        self.stdout.read_exact(&mut bytes).unwrap();
        bytes
    }

    fn finish(mut self) -> ExitStatus {
        drop(self.stdin);
        self.child.wait().unwrap()
    }
}

#[test]
fn version_and_unknown_command() {
    let mut peer = Peer::start();
    peer.send("version");
    assert_eq!(peer.line(), format!("ok {}", env!("CARGO_PKG_VERSION")));
    peer.send("nonsense");
    assert_eq!(peer.line(), "err unknown command");
    peer.send("quit");
    assert!(peer.finish().success());
}

#[test]
fn page_zero_has_staff_lines_stems_and_margins() {
    let mut peer = Peer::start();
    let path = corpus("abc/v3_measure-4.abc");
    peer.send(&format!("open {WIDTH} {HEIGHT} {}", path.to_str().unwrap()));
    let answer = peer.line();
    assert!(answer.starts_with("ok "), "{answer}");
    let bytes = peer.page(0);

    // The screen geometry leaves room for tall clefs instead of clipping
    // the first system or letting it touch the left edge.
    for x in 0..WIDTH {
        assert_eq!(bytes[x], 255, "ink on the first row at x={x}");
        assert_eq!(
            bytes[(HEIGHT - 1) * WIDTH + x],
            255,
            "ink on the last row at x={x}"
        );
    }
    for y in 0..HEIGHT {
        assert_eq!(bytes[y * WIDTH], 255, "ink on the first column at y={y}");
        assert_eq!(
            bytes[y * WIDTH + WIDTH - 1],
            255,
            "ink on the last column at y={y}"
        );
    }

    // Five staff lines, each a continuous horizontal run of at least 60 %
    // of the page width. Rectangles used to be passed in 1/256-px units to
    // `fill_rect` (which expects whole pixels) and were entirely clipped.
    let staff_rows = bytes
        .as_chunks::<WIDTH>()
        .0
        .iter()
        .filter(|row| longest_dark_run(*row) >= WIDTH * 3 / 5)
        .count();
    assert!(
        staff_rows >= 5,
        "expected at least five continuous staff lines, found {staff_rows}"
    );

    // Stems and bar lines are vertical strokes through the staff, distinct
    // from the five thin staff lines themselves.
    let stem_columns = (0..WIDTH)
        .filter(|x| (0..HEIGHT).filter(|y| bytes[y * WIDTH + x] < 128).count() >= 16)
        .count();
    assert!(
        stem_columns >= 10,
        "expected stems and bar lines, found {stem_columns} tall columns"
    );

    peer.send("quit");
    assert!(peer.finish().success());
}

#[test]
fn moonlight_page_zero_has_a_title_frame_and_no_clipped_credits() {
    let mut peer = Peer::start();
    let path = corpus("v3/moonlight.mscx");
    peer.send(&format!("open {WIDTH} {HEIGHT} {}", path.to_str().unwrap()));
    let answer = peer.line();
    assert!(answer.starts_with("ok "), "{answer}");
    let bytes = peer.page(0);

    // A scaled-down title fits inside the screen: no ink on the outermost
    // rows or columns, even though the 3.x VBox holds long credit texts.
    for x in 0..WIDTH {
        assert_eq!(bytes[x], 255, "ink on the first row at x={x}");
        assert_eq!(bytes[(HEIGHT - 1) * WIDTH + x], 255, "ink on the last row");
    }
    for y in 0..HEIGHT {
        assert_eq!(bytes[y * WIDTH], 255, "ink on the first column at y={y}");
        assert_eq!(bytes[y * WIDTH + WIDTH - 1], 255, "ink on the last column");
    }

    // The VBox pushes the first system below the credits: the staff lines
    // can no longer run through the title.
    let first_staff_row = bytes
        .as_chunks::<WIDTH>()
        .0
        .iter()
        .position(|row| longest_dark_run(row) >= WIDTH * 3 / 5)
        .expect("at least one staff line");
    assert!(
        first_staff_row > 200,
        "first staff line at row {first_staff_row}: title frame was ignored"
    );

    peer.send("quit");
    assert!(peer.finish().success());
}

#[test]
fn opens_every_corpus_format_and_renders_page_zero() {
    let mut peer = Peer::start();
    peer.send("version");
    assert!(peer.line().starts_with("ok "));

    // One file per extension available under tests/corpus: MuseScore,
    // MusicXML, LilyPond, MEI, ABC, MIDI and GABC. A single process opens
    // them in turn, so each `open` also exercises replacing the old document.
    for file in [
        "v3/moonlight.mscx",
        "v3/lyricsline01.ref.musicxml",
        "ly/v4_linked-ties.ly",
        "v4/repeat68.ref.mei",
        "abc/v3_measure-4.abc",
        "midi/v4/dynamics.ref.mid",
        "gabc/gregorio_pitches2.gabc",
    ] {
        let path = corpus(file);
        peer.send(&format!("open {WIDTH} {HEIGHT} {}", path.to_str().unwrap()));
        let answer = peer.line();
        assert!(answer.starts_with("ok "), "{file}: {answer}");
        let pages: usize = answer[3..].parse().unwrap();
        assert!(pages > 0, "{file}: no page");

        let bytes = peer.page(0);
        assert_eq!(bytes.len(), WIDTH * HEIGHT);
        assert!(
            bytes.iter().any(|value| *value < 128),
            "{file}: no dark pixel"
        );

        peer.send(&format!("page {pages}"));
        assert!(peer.line().starts_with("err "), "{file}: out-of-range page");
    }

    peer.send("close");
    assert_eq!(peer.line(), "ok");
    peer.send("close");
    assert_eq!(peer.line(), "ok");
    peer.send("quit");
    assert!(peer.finish().success());
}

#[test]
fn errors_do_not_kill_the_process() {
    let mut peer = Peer::start();
    peer.send("page 0");
    assert_eq!(peer.line(), "err no document open");

    peer.send("open 600 800 /nonexistent/scoretk-serve-test.mscx");
    let answer = peer.line();
    assert!(answer.starts_with("err cannot read"), "{answer}");
    peer.send("version");
    assert!(peer.line().starts_with("ok "));

    peer.send("open 0 800 /nonexistent/file.mscx");
    assert!(peer.line().starts_with("err invalid page size"));
    peer.send("page nope");
    assert_eq!(peer.line(), "err invalid page index");

    let path = corpus("v4/voiceswitching.mscx");
    peer.send(&format!("open {WIDTH} {HEIGHT} {}", path.to_str().unwrap()));
    let answer = peer.line();
    assert!(answer.starts_with("ok "), "{answer}");
    peer.send(&format!("page {}", usize::MAX));
    assert!(peer.line().starts_with("err page"));

    peer.send("quit");
    assert!(peer.finish().success());
}

#[test]
fn end_of_stdin_exits_zero() {
    let peer = Peer::start();
    assert!(peer.finish().success());
}

#[test]
fn paths_with_spaces_are_accepted() {
    let mut peer = Peer::start();
    // Create a copy with a space in its name under a temporary directory
    // provided by the caller; no score is copied into the repository.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tmp/serve-space");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("moon light.mscx");
    std::fs::copy(corpus("v3/lyricsline01.mscx"), &path).unwrap();
    peer.send(&format!("open {WIDTH} {HEIGHT} {}", path.to_str().unwrap()));
    assert!(peer.line().starts_with("ok "));
    let _ = peer.page(0);
    peer.send("quit");
    assert!(peer.finish().success());
    std::fs::remove_dir_all(&dir).unwrap();
}
