# scoretk: conversion facade

Crate `crates/scoretk` (library) and `crates/scoretk-cli` (binary
`scoretk`). This document describes the routing, the wiring points of
future converters and the tests. The project core remains in `AGENTS.md`.

## 1. Role

The model of `crates/scoretk-core/src/model` is the **common pivot**: each format
provides a reader into this model and a writer from this model, which avoids
N×N converters. `scoretk` simply chooses the reader or the writer according to the `Format`.

The library stays portable: no `std::fs`/`env`/`net`/`process`, no
threads, no clock; it compiles for `wasm32-unknown-unknown`. The CLI is
the only one to touch the disk.

| Element | Location |
| --- | --- |
| Formats, detection | `crates/scoretk/src/format.rs` |
| Errors | `crates/scoretk/src/error.rs` |
| Write options | `crates/scoretk/src/options.rs` |
| `read`/`write` routing | `crates/scoretk/src/lib.rs` |
| Feature hook points | `crates/scoretk/src/hooks.rs` |
| CLI | `crates/scoretk-cli/src/main.rs` |
| Tests | `crates/scoretk/tests/`, `crates/scoretk-cli/tests/cli.rs` |

## 2. API

```rust
pub enum Format { Mscz, Mscx, MusicXml, Mxl, Ly, Abc, Midi }
pub enum Sens { Lecture, Ecriture }

pub fn read(bytes: &[u8], format: Format) -> Result<Score>;
pub fn write(score: &Score, format: Format, options: &Options) -> Result<Vec<u8>>;

#[cfg(all(feature = "midi-export", feature = "musicxml-export"))]
pub fn write_playback(score: &Score, options: &Options) -> Result<PlaybackOutput>;
```

`Options` contains `software`, `encoding_date`, `concert_pitch` and `midi`.
`MidiOptions { expand_repeats: bool }` (default `true`) controls repeat
unfolding in the MIDI output; the CLI exposes it via `--no-expand-repeats`.
Ordinary MusicXML export sets no identifiers (`stable_ids: false`):
it stays identical to the MuseScore 4.7.4 references.

`write_playback` (features `midi-export` and `musicxml-export`) returns
`PlaybackOutput { musicxml: Vec<u8>, midi: Vec<u8>, timemap: scoretk::Timemap }`.
The MusicXML there is exported with `stable_ids` forced: every `<measure>` and
every `<note>` carries `id` **and** `xml:id` (same value), exactly the
`xmlId`s of the time map. The MIDI is identical to that of `write(...,
Format::Midi, ...)` with the same options. The map, its identifier
formulas and its client-side use are described in
`docs/agents/timemap.md`.

`Format` provides `from_extension`, `from_path` (extension only, no disk
access), `detect` (content), `supports_read`/`supports_write` (actual state,
including features) and `FromStr` (`mscz`, `mscx`, `musicxml`/`xml`, `mxl`,
`ly`/`lilypond`, `abc`, `mid`/`midi`).

### Content-based detection

Order of `Format::detect`:

1. ZIP signature (`.mscz` if it contains a `.mscx`, otherwise `.mxl` if it contains
   `META-INF/container.xml` or a `.xml`/`.musicxml`);
2. `MThd` → MIDI;
3. `<museScore` → `.mscx`;
4. `<score-partwise`/`<score-timewise`, or `<?xml` alone → MusicXML;
5. `\version` at the start → LilyPond;
6. `%abc` or an `X:` header → ABC.

Nothing matches → `None` (the CLI then asks for `--from`).

### Current limits

Each direction has its feature, and all ten are enabled by default; the `scoretk`
CLI and the wasm crate enable them all. `--list-formats` shows the actual state of the
compiled library.

| Format | Reading | Writing |
| --- | --- | --- |
| `.mscz` / `.mscx` | feature `musescore-import` | feature `musescore-export` |
| MusicXML `.musicxml` / `.xml` | feature `musicxml-import` (t34) | feature `musicxml-export` |
| MusicXML `.mxl` | feature `musicxml-import` (t43) | feature `musicxml-export` |
| LilyPond `.ly` | feature `lilypond-import` (t40) | feature `lilypond-export` (t36) |
| ABC `.abc` | feature `abc-import` (t41) | feature `abc-export` (t41) |
| MIDI `.mid` | feature `midi-import` (t42) | feature `midi-export` (t35) |

A format absent in the requested direction returns
`Error::FormatNonPrisEnCharge { format, sens }`, never an empty output.

## 3. Wiring the converters: ten symmetric features

The facade wires each converter through exactly one feature per direction:
`<format>-import` for `read`, `<format>-export` for `write`. All ten are
enabled by default and there are **no mixed aliases** (no
`midi = ["midi-import", "midi-export"]`): a need for both directions lists both
features, as the CLI and the wasm crate do. Each feature wires
exactly one crate:

| Feature | Crate | Functions in `hooks.rs` |
| --- | --- | --- |
| `musescore-import` | `scoretk-musescore-import` | `read_musescore` |
| `musescore-export` | `scoretk-musescore-export` | `write_mscx`, `write_mscz` |
| `musicxml-import` | `scoretk-musicxml-import` | `read_musicxml`, `read_mxl` |
| `musicxml-export` | `scoretk-musicxml-export` | `write_musicxml`, `write_mxl` |
| `midi-import` | `scoretk-midi-import` | `read_midi` |
| `midi-export` | `scoretk-midi-export` | `write_midi`, `write_playback` |
| `lilypond-import` | `scoretk-lilypond-import` | `read_lilypond`, `read_lilypond_with_includes` |
| `lilypond-export` | `scoretk-lilypond-export` | `write_lilypond` |
| `abc-import` | `scoretk-abc-import` | `read_abc` |
| `abc-export` | `scoretk-abc-export` | `write_abc` |

All conditional code is in `hooks.rs`: each function has **two mutually
exclusive definitions**, the real one under `#[cfg(feature = "…")]` and a fallback under
`#[cfg(not(feature = "…"))]` generated by the `non_branche!` macro, which returns
`Error::FormatNonPrisEnCharge { format, sens }`. No global `cfg(not(any(...)))`:
adding a feature amounts to adding a pair of functions.

`error.rs` follows the same rule: variants that carry directly
a crate's error (`MuseScoreImport`, `MusicXmlExport`) exist only if the
corresponding feature is active.

To add a converter:

1. declare the direction crate as an optional dependency and the feature
   `<format>-<sens> = ["dep:…"]`;
2. write the function in `hooks.rs` (real definition + `non_branche!`) ;
3. wire the matching arm of `read`/`write` in `lib.rs` and complete
   `Format::{supports_read,supports_write}`;
4. add the feature to `default`, to the CLI and to the wasm crate.

`write_playback` is the only point requiring two features (`midi-export` for
the MIDI and the time map, `musicxml-export` for the identified MusicXML):
it is absent if either one is missing.

### MuseScore (`.mscz`, `.mscx`) — `musescore-import`, `musescore-export`

Reading (t38) goes through `MsczContainer::from_bytes` then `read::read_score`
(`scoretk-musescore-import`); writing through `write_mscx`/`write_mscz`
(`scoretk-musescore-export`): text `.mscx`, `.mscz` ZIP archive with
`META-INF/container.xml`, `score.mscx` and `score_style.mss`.

### MusicXML (`.musicxml`, `.xml`, `.mxl`) — `musicxml-import`, `musicxml-export`

Reading (t34, t43) calls `read_musicxml`/`read_mxl` from
`scoretk-musicxml-import` and converts the error into `Error::MusicXmlImport`.
Export goes through `scoretk-musicxml-export`, and `.mxl` reuses
`export_mxl_with` (archive `META-INF/container.xml` + `score.xml`, like
MuseScore's `saveMxl`). Details of container extraction and fallbacks:
`docs/agents/musicxml-import.md`.

### MIDI — `midi-import`, `midi-export`

`read_midi` (t42) comes from `scoretk-midi-import` (`Error::MidiImport`) ;
`write_midi` (t35) and `write_playback` from `scoretk-midi-export`
(`Error::Midi`). The import and its limits are described in
`docs/agents/midi.md`, the time map in `docs/agents/timemap.md`.

### LilyPond — `lilypond-import`, `lilypond-export`

`read_lilypond` (t40, `scoretk-lilypond-import`) decodes UTF-8 then reads a
subset of LilyPond; `read_lilypond_with_includes` additionally follows
`\include` via a callback supplied by the caller (no I/O in the
facade, the CLI reads the files); `write_lilypond` (t36,
`scoretk-lilypond-export`) writes in `\fixed c'` voices. Both crates share
`scoretk-lilypond-common` (error, `tpc`, clefs). Subset, limits and
measured round trip: `docs/agents/lilypond.md`.

### ABC — `abc-import`, `abc-export`

`read_abc` and `write_abc` (t41) come from `scoretk-abc-import` and
`scoretk-abc-export` (UTF-8 conversion and errors into `Error::Abc`); the two
share `scoretk-abc-common`. Subset and round trip:
`docs/agents/abc.md`.

## 4. CLI

```sh
scoretk input.mscz -o output.musicxml      # formats inferred from extensions
scoretk input.mscx --to mxl -o -           # standard output
scoretk input.mscx -o copy.mscx           # MuseScore 4.70 rewrite
scoretk input.mscx -o copy.mscz           # MuseScore container
scoretk input.musicxml -o output.mscx      # MusicXML reading (feature)
scoretk input.mxl -o output.mscx           # compressed MusicXML → MuseScore
scoretk input.musicxml -o output.ly        # MusicXML → LilyPond
scoretk input.musicxml -o output.mid       # MusicXML → MIDI
scoretk input.mscx -o output.mid           # MuseScore → MIDI
scoretk input.mid -o output.musicxml       # MIDI → MusicXML (import)
scoretk input.ly -o output.musicxml        # LilyPond (subset) → MusicXML
scoretk input.mscx -o output.abc           # MuseScore → ABC
scoretk input.abc -o output.musicxml       # ABC → MusicXML
cat input.mscx | scoretk - --from mscx --to musicxml
scoretk --list-formats                      # supported formats and directions
scoretk --version                           # version + licence (GPL-3.0-only)
```

- Formats inferred from the extension; if it is missing or unknown, the input is
  guessed from the content.
- `--from`/`--to` force the format; they are mandatory when the input or the
  output is `-`.
- missing `-o` or `-` → standard output; input `-` → standard input.
- `--encoding-date AAAA-MM-JJ` fixes `<encoding-date>` (default: the day's date in
  UTC, like `scoretk-core`).
- `--no-expand-repeats` does not unfold repeats in the MIDI output
  (`Options::midi.expand_repeats = false`); the output is then "flat".

Exit codes (also in `scoretk --help`):

| Code | Meaning |
| ---: | --- |
| 0 | success |
| 1 | conversion failure (unreadable or malformed input, writer error) |
| 2 | invalid usage (clap: missing `--from`/`--to`, unknown option…) |
| 3 | **unsupported input**: version or format deliberately not supported |

Code 3 is what `scoretk::Error::is_unsupported` classifies: a MuseScore file
whose version is outside the ported subset (`UnsupportedVersion`: 1.15–2.07,
3.00–3.99 and 4.00–5.00 are read, below 1.15 `read114` is not ported) and a
format without a converter in the requested direction
(`FormatNonPrisEnCharge`, disabled feature). A
reader bug, a malformed XML/ZIP or an unreadable source keeps code 1: the
distinction exists so that the measurement scripts (`scripts/oracle/*`) can
exclude an unsupported input from their numerator and denominator — shared
helper `scripts/oracle/common.py` — never to hide a regression.

## 5. Flake

- `packages.scoretk` builds the CLI (`cargoExtraArgs = "-p scoretk-cli"`).
- `apps.scoretk` makes it runnable via `nix run .#scoretk -- …`.
- `packages.default` also builds the `scoretk` CLI.
- The `wasm32-unknown-unknown` check compiles the sixteen library crates
  (`-p scoretk-core`, `-p scoretk-musescore-import`, `-p scoretk-musescore-export`,
  `-p scoretk-musicxml-import`, `-p scoretk-musicxml-export`,
  `-p scoretk-midi-common`, `-p scoretk-midi-import`, `-p scoretk-midi-export`,
  `-p scoretk-lilypond-common`, `-p scoretk-lilypond-import`,
  `-p scoretk-lilypond-export`, `-p scoretk-abc-common`, `-p scoretk-abc-import`,
  `-p scoretk-abc-export`, `-p scoretk`, `-p scoretk-wasm`).
- `packages.wasm` is the browser package of the facade (crate
  `scoretk-wasm`); see `docs/agents/wasm.md`.
- `checks.wasm-node-smoke` checks that package under Node.

## 6. Tests

- `crates/scoretk/tests/formats.rs`: extensions, paths, `FromStr`, content
  detection (XML, `.mscz`/`.mxl` ZIP, MIDI, LilyPond, ABC, unknown).
- `crates/scoretk/tests/parite.rs`: `.mscz` → MusicXML **byte-for-byte
  identical** to `scoretk::convert` on the corpus (`dynamics`, `hairpin_simple`,
  `hairpins_and_repeats`), `.mxl` (container + `score.xml`), MIDI reading into
  MusicXML with the feature, explicit errors of unwired formats.
- `crates/scoretk/tests/mxl.rs` (feature `musicxml-import`): `.mxl` read like
  the equivalent `.musicxml`, fallback without `container.xml`, invalid ZIP.
- `crates/scoretk-cli/tests/mxl.rs`: `.mxl` → `.mscx` and `--list-formats`
  with `.mxl` reading active.
- `crates/scoretk/tests/abc.rs` (features `abc-import` and `abc-export`):
  ABC reading, writing, rereading and stable note count ;
  `crates/scoretk-cli/tests/abc.rs`: `.mscx` → `.abc`, `.abc` → MusicXML,
  stdin/stdout, `--list-formats`.
- `crates/scoretk-cli/tests/lilypond.rs`: `\relative` `.ly` → MusicXML and
  `--list-formats` with LilyPond reading active.
- `crates/scoretk/tests/abc_*.rs` (round trips in the facade, since they
  exercise both directions): hand-written tunes (`abc_roundtrip.rs`),
  corpus snapshots (`abc_corpus.rs`), core round trip
  `EXPECTED_ROUNDTRIP` (`abc_aller_retour.rs`), external tools
  (`abc_outils_externes.rs`) ; `crates/scoretk-abc-import/tests/lecture.rs`
  for reading only.
- `crates/scoretk/tests/mscx.rs` (feature `musescore-export`): `.mscx` writing read back
  with an identical model, identical MusicXML output, `.mscz` detected and read back,
  version `4.70`.
- `crates/scoretk/tests/lilypond_read.rs` (feature `lilypond-import`):
  `read(..., Format::Ly)` returns an exportable model, a source without
  `\score` is an error.
- Per-direction tests: `crates/scoretk-musescore-import/tests/`,
  `crates/scoretk-musicxml-import/tests/`,
  `crates/scoretk-musicxml-export/tests/`,
  `crates/scoretk-midi-import/tests/`, `crates/scoretk-midi-export/tests/`
  (`corpus_midi`, `timemap_alignment`), `crates/scoretk-lilypond-import/tests/`,
  `crates/scoretk-lilypond-export/tests/`, `crates/scoretk-musescore-export/tests/`.
- `crates/scoretk-cli/tests/cli.rs`: real binary (`CARGO_BIN_EXE_scoretk`),
  `--list-formats` (feature state: MusicXML reading, MIDI writing),
  `--version`, stdin/stdout, `.mscz` → MusicXML compared to `scoretk::convert`, output
  `.mxl`, `.mscx`/`.mscz` written and read back, `.mscx` → `.ly`, `.musicxml` →
  `.mscx`/`.ly`/`.mid`, `.mscx` → `.mid`, `.mxl` input refused (no
  reader), errors for missing `--from`/`--to` and for an unwired format.
- The corpus is read via `CARGO_MANIFEST_DIR/../../tests/corpus`; it must remain
  in the `flake.nix` filter (see `docs/agents/developpement.md`).

## 7. Known discrepancies

- `MsczContainer::from_bytes` accepts a bare `.mscx`: `read(..., Format::Mscx)`
  and `read(..., Format::Mscz)` therefore follow the same path.
- `detect` classifies any ZIP without a `.mscx` but with `META-INF/container.xml` (or
  an `.xml`/`.musicxml`) as `.mxl`; an arbitrary ZIP without XML, without
  a container or a `.mscx` is not recognised.
- An unknown XML is assumed to be MusicXML (`<?xml` alone): it is the only XML
  format of scoretk today; `--from` remains priority.
- No measured discrepancy with the reading and export crates: the facade
  calls their functions, the test comparison is a strict equality.
