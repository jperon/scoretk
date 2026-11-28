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

/// Rows of the page that carry a continuous staff line (at least 60 % of the
/// width), in increasing order.
fn staff_line_rows(bytes: &[u8]) -> Vec<usize> {
    bytes
        .as_chunks::<WIDTH>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, row)| longest_dark_run(row.as_slice()) >= WIDTH * 3 / 5)
        .map(|(row, _)| row)
        .collect()
}

/// Smallest vertical gap between two distinct staff lines: the pixels per
/// staff space used by the rasteriser.
fn staff_pitch(rows: &[usize]) -> usize {
    rows.windows(2)
        .map(|pair| pair[1] - pair[0])
        .filter(|gap| *gap > 1)
        .min()
        .unwrap_or(0)
}

#[test]
fn invalid_set_commands_are_rejected_without_changing_the_setting() {
    let mut peer = Peer::start();
    for command in [
        "set",
        "set scale",
        "set nonsense 1.0",
        "set scale nope",
        "set scale 0.06",
        "set text 0.09",
        "set text 3.1",
        "set text NaN",
    ] {
        peer.send(command);
        let answer = peer.line();
        assert!(answer.starts_with("err "), "{command}: {answer}");
    }
    // The process is still usable and the default stays in force.
    peer.send("set scale 2.0");
    assert_eq!(peer.line(), "ok");
    peer.send("version");
    assert!(peer.line().starts_with("ok "));
    peer.send("quit");
    assert!(peer.finish().success());
}

#[test]
fn the_smallest_scales_open_and_render_without_panicking() {
    // Floors of `set scale` (0.07) and `set text` (0.1). On this 600 × 800
    // screen the auto staff size is 14 px/sp, so 14 * 0.07 rounds down to the
    // 1 px/sp floor of `screen_staff_px_at`: the tiniest grid the renderer
    // accepts. Opening must not divide by a zero pixel size, panic (the
    // protocol then replies `err internal error`) or drop every page.
    let file = corpus("v3/moonlight.mscx");
    let path = file.to_str().unwrap();
    let mut peer = Peer::start();

    peer.send("set scale 0.07");
    assert_eq!(peer.line(), "ok");
    peer.send(&format!("open {WIDTH} {HEIGHT} {path}"));
    let answer = peer.line();
    assert!(answer.starts_with("ok "), "scale 0.07: {answer}");
    let pages: usize = answer[3..].parse().unwrap();
    assert!(pages >= 1, "scale 0.07 must still produce pages");
    for page in 0..pages {
        assert_eq!(
            peer.page(page).len(),
            WIDTH * HEIGHT,
            "scale 0.07: page {page} of {pages} is incomplete"
        );
    }
    peer.send("close");
    assert_eq!(peer.line(), "ok");

    // `set text` reaches 0.1 on top of a normal staff.
    peer.send("set scale 1.0");
    assert_eq!(peer.line(), "ok");
    peer.send("set text 0.1");
    assert_eq!(peer.line(), "ok");
    peer.send(&format!("open {WIDTH} {HEIGHT} {path}"));
    let answer = peer.line();
    assert!(answer.starts_with("ok "), "text 0.1: {answer}");
    assert_eq!(peer.page(0).len(), WIDTH * HEIGHT);

    peer.send("quit");
    assert!(peer.finish().success());
}

#[test]
fn a_larger_staff_scale_fits_fewer_systems_per_page() {
    let file = corpus("v3/moonlight.mscx");
    let path = file.to_str().unwrap();
    let mut peer = Peer::start();

    peer.send(&format!("open {WIDTH} {HEIGHT} {path}"));
    let plain: usize = peer.line()[3..].parse().unwrap();
    let plain_pitch = staff_pitch(&staff_line_rows(&peer.page(0)));
    peer.send("close");
    assert_eq!(peer.line(), "ok");

    peer.send("set scale 2.0");
    assert_eq!(peer.line(), "ok");
    peer.send(&format!("open {WIDTH} {HEIGHT} {path}"));
    let zoomed: usize = peer.line()[3..].parse().unwrap();
    let zoomed_pitch = staff_pitch(&staff_line_rows(&peer.page(0)));

    assert!(
        plain > 0 && zoomed > plain,
        "scale 2.0 must need more pages than 1.0 (plain {plain}, zoomed {zoomed})"
    );
    // The document's stored staff size is the effective one: the rasteriser
    // draws twice as many pixels per staff space.
    assert_eq!(
        zoomed_pitch,
        2 * plain_pitch,
        "scale 2.0 did not double the staff pitch ({plain_pitch} -> {zoomed_pitch})"
    );

    peer.send("quit");
    assert!(peer.finish().success());
}

/// Opens `path` at `scale` and returns the page count after checking that
/// every page carries ink.
fn pages_at(peer: &mut Peer, path: &str, scale: &str) -> usize {
    peer.send(&format!("set scale {scale}"));
    assert_eq!(peer.line(), "ok", "set scale {scale}");
    peer.send(&format!("open {WIDTH} {HEIGHT} {path}"));
    let answer = peer.line();
    assert!(answer.starts_with("ok "), "scale {scale}: {answer}");
    let pages: usize = answer[3..].parse().unwrap();
    for page in 0..pages {
        let bytes = peer.page(page);
        assert!(
            bytes.iter().any(|value| *value < 128),
            "scale {scale}: page {page} of {pages} is blank"
        );
    }
    pages
}

#[test]
fn zooming_in_does_not_multiply_the_page_count() {
    // Regression: the first system of a page used to pay the system distance
    // on top of the top border, and the screen geometry added the print
    // page's 7 sp staff borders to its own margins. Both errors are a fixed
    // number of staff spaces, so they only became fatal once the page held
    // few staff spaces: at 1.5 this two-page score needed seven pages, most
    // of them a single system with a large void below.
    let file = corpus("v4/voiceswitching.mscx");
    let path = file.to_str().unwrap();
    let mut peer = Peer::start();

    let plain = pages_at(&mut peer, path, "1.0");
    assert!(plain >= 2, "the sample must span several pages");
    let narrow = pages_at(&mut peer, path, "0.7");
    let zoom = pages_at(&mut peer, path, "1.5");
    let double = pages_at(&mut peer, path, "2.0");

    assert!(
        narrow <= plain,
        "0.7 must not need more pages than 1.0 ({narrow} > {plain})"
    );
    assert!(
        zoom <= 2 * plain,
        "1.5 must stay within twice the pages of 1.0 ({zoom} > 2 x {plain})"
    );
    assert!(
        double <= 3 * plain,
        "2.0 must stay within three times the pages of 1.0 ({double} > 3 x {plain})"
    );

    peer.send("quit");
    assert!(peer.finish().success());
}

#[test]
fn the_text_scale_does_not_change_the_pixels_per_staff_space() {
    let file = corpus("v3/moonlight.mscx");
    let path = file.to_str().unwrap();
    let mut peer = Peer::start();

    peer.send(&format!("open {WIDTH} {HEIGHT} {path}"));
    assert!(peer.line().starts_with("ok "));
    let plain_bytes = peer.page(0);
    let plain = staff_pitch(&staff_line_rows(&plain_bytes));
    assert!(plain > 1, "no staff line found");

    peer.send("close");
    assert_eq!(peer.line(), "ok");
    peer.send("set text 2.0");
    assert_eq!(peer.line(), "ok");
    peer.send(&format!("open {WIDTH} {HEIGHT} {path}"));
    assert!(peer.line().starts_with("ok "));
    let scaled_bytes = peer.page(0);
    let scaled = staff_pitch(&staff_line_rows(&scaled_bytes));
    assert_eq!(
        scaled, plain,
        "text 2.0 changed the staff pitch ({plain} -> {scaled})"
    );
    assert_ne!(scaled_bytes, plain_bytes, "text 2.0 drew the same page");

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
