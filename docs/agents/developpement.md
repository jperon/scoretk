# Development: conventions, portability and tooling

How-to for writing code in `scoretk`. The core (invariant rules,
commands, index) is in `AGENTS.md`; this document gives the
details and tooling pitfalls.

## 1. Layout and responsibilities

| Path | Role |
| --- | --- |
| `crates/scoretk-core/` | portable, generic core: model, error, `gap_rests`, stable identifiers. **No I/O, no threads, no clock, no format.** |
| `crates/scoretk-musescore-import/src/container.rs` | `.mscz` (ZIP) → `.mscx`, version detection |
| `crates/scoretk-musescore-import/src/read/` | `read302` (3.x) and `read4xx` (4.x/5.x) → model |
| `crates/scoretk-core/src/model/` | in-memory score model, pivot of readers and export |
| `crates/scoretk-musicxml-export/src/` | model → `partwise` MusicXML / `.mxl` |
| `crates/scoretk-wasm/` | wasm-bindgen wrapper of the facade (`convert`, `detect`, `formats`, `version`), the only wasm package (`nix build .#wasm`) |
| `crates/scoretk-musicxml-import/` | MusicXML reader → model (`read_musicxml`), portable library |
| `crates/scoretk-musescore-export/` | MuseScore writing (`.mscx` 4.x, `.mscz`), no I/O |
| `crates/scoretk-midi-common/` | error and result shared by both directions, no conversion |
| `crates/scoretk-midi-import/` | MIDI reading into the model, no I/O |
| `crates/scoretk-midi-export/` | SMF format 1 writing and repeat unfolding, no I/O |
| `crates/scoretk-lilypond-common/` | tables shared by LilyPond reading and writing (error, `tpc`, clefs), **no conversion logic** |
| `crates/scoretk-lilypond-import/` | reading a subset of LilyPond into the model, no I/O |
| `crates/scoretk-lilypond-export/` | LilyPond writing from the model (voices in `\fixed c'`), no I/O |
| `crates/scoretk-abc-common/` | ABC tables and helpers shared by both directions (pitches, key signatures, clefs, durations, ornaments, lyrics), no conversion |
| `crates/scoretk-abc-import/` | ABC reading into the model, no I/O |
| `crates/scoretk-abc-export/` | ABC writing from the model, no I/O |
| `crates/scoretk/` | multi-format facade: `Format`, detection, `read`/`write` into the pivot model; ten symmetric features `<format>-import` / `<format>-export`, one per direction crate, wired in `src/hooks.rs` |
| `crates/scoretk-cli/` | `scoretk` binary: disk I/O for the facade, formats inferred from extensions |
| `crates/scoretk/tests/` | round-trip tests (reading then export) and compliance e2e tests |
| `crates/scoretk-musescore-import/tests/` | reading-only tests by family |
| `crates/scoretk-musicxml-export/tests/` | export-only tests by family |
| `tests/corpus/{v3,v4}/` | `.mscx` and `*.ref.musicxml` references |
| `scripts/` | generation of references and instruments table, licence check |
| `flake.nix` | dev shell, checks, wasm package |

`tests/corpus/` must remain included in the source of the Nix checks: the
filter in `flake.nix` combines `craneLib.filterCargoSources` with a
`tests/corpus` prefix, otherwise the `tests` check fails (« No such file or directory »).
Do not remove this rule.

## 2. Code conventions

- **Faithful port, not reinvention.** The code follows the C++ closely: same
  method names (`write_measure`, `calc_divisions`, `move_to_tick`), same
  breakdowns, references to the upstream file and function in the
  comments. If in doubt, read `ref-musescore/` (see
  `docs/agents/upstream.md`) and the upstream tests, never code from memory.
- **New code in new files, by family.** One Rust file per
  functional family (3.x reader, 4.x reader, model, export); existing
  files receive only wiring (`mod.rs`, re-export) or
  a localised fix. This limits conflicts between parallel agents and
  keeps diffs reviewable.
- **Rule for `-common` crates.** Factorise intelligently: what is useful
  or necessary to all formats goes into a common crate (e.g. a future
  XML utility crate shared by all readers/writers); what
  serves both directions of the same format goes into that format's `-common`; what
  serves only one function (one direction) stays in the
  direction-specific file. No single-direction module in a `-common`, no
  format-specific logic in a common crate. Cross-cutting generic code
  goes into `scoretk-core`.
- **Comments, error messages and documentation in English.** Type
  and function names follow upstream or Rust usage.
- **Reproducibility.** No clock: `encoding_date` is an option, with
  a default constant. No iteration over `HashMap` when output
  order matters: use `BTreeMap`/`BTreeSet`.
- **Tests.** Each family has its integration test (`nuances_corpus.rs`,
  `notations_corpus.rs`, `texts_corpus.rs`, `repeats_corpus.rs`, …) and its
  unit tests in the module. A parity fix updates
  `EXPECTED_CONFORMANT` and the corresponding discrepancy document.
- **Generated files, do not edit by hand**:
  `crates/scoretk-musescore-import/src/read/instruments.rs` (by
  `scripts/gen-instruments-table.py`) and `docs/agents/ecarts-complets.md`
  (output of the `ecarts_complets_document` test). Regenerate, then review the diff.

## 3. Licence headers (mandatory)

Every new or modified source file carries, at the top, the SPDX identifier, the
copyright and the GPL-3.0-only text. For a ported file, add the
modification notice required by GPL §5(a) and cite the original C++ files.

Template for a ported Rust file (adapt the source list):

```rust
// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/xxx.h` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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
```

Same header (with `#` comments) for a Python or shell script, without the
« Portage de » part if the file is original. A new data file or a
new dependency additionally requires an update to `NOTICE` and, for a
third-party crate, to `docs/licences-tierces.md`; `scripts/check-licences.py`
rejects any licence outside its whitelist.

## 4. Library portability

The library must compile and run on `wasm32-unknown-unknown` (without
WASI), Android, iOS, Linux, macOS and Windows. Checklist before adding
code to the library crates (`scoretk-core`, `scoretk-musescore-import`,
`scoretk-musescore-export`, `scoretk-musicxml-import`, `scoretk-musicxml-export`,
`scoretk-midi-common`, `scoretk-midi-import`, `scoretk-midi-export`,
`scoretk-lilypond-common`, `scoretk-lilypond-import`, `scoretk-lilypond-export`,
`scoretk-abc-common`, `scoretk-abc-import`, `scoretk-abc-export`):

- no `std::fs`, `std::env`, `std::net`, `std::process`;
- no `std::thread`, `std::sync::mpsc`, `rayon`;
- no clock (`SystemTime`, `Instant`) nor non-deterministic random
  generation;
- inputs and outputs only as **in-memory bytes**;
- no C dependency, no `build.rs` with native compilation: the ZIP is
  decoded in pure Rust (`zip` without default features, `miniz_oxide`);
- tests may use the filesystem (they run on the host),
  the library may not;
- check the target: `nix develop -c cargo build -p scoretk-core --target
  wasm32-unknown-unknown` (and likewise for each library crate).

The CLIs are the only ones to read and write files; the wasm crate merely
wraps the library API.

## 5. Stable wasm contract (cantus)

The package produced by `nix build .#wasm` is consumed by the **cantus** PWA.
Do not change the file names, the package name or the
signatures without coordination:

| Element | Stable value |
| --- | --- |
| Package | `package.json` `name: "scoretk"`, `type: "module"` |
| ESM | `esm/scoretk_wasm.js`, `esm/scoretk_wasm_bg.wasm`, `esm/scoretk_wasm.d.ts` |
| Classic script | `classic/scoretk_wasm.js`, `classic/scoretk_wasm_bg.wasm`, `classic/scoretk_wasm.d.ts` |
| Worker wrapper | `classic/scoretk.js` → `self.scoretk = { ready, convert, convertWithTimeline, version, detect, formats, baseUrl }` |
| Conversion | `convert(bytes: Uint8Array, options?: { from?, to?, encodingDate?, software?, concertPitch?, expandRepeats? }) => Uint8Array` (`to` = `"musicxml"` by default) |
| Reading + timeline | `convertWithTimeline(bytes, options?) => { musicxml: Uint8Array, midi: Uint8Array, timeline: Timeline }` (additive, `to` ignored) |
| Detection | `detect(bytes: Uint8Array) => string \| null` |
| Formats | `formats() => FormatInfo[]` |
| Version | `version() => string` |

The `includes?: Record<string, Uint8Array>` option is additive on both `convert`
and `convertWithTimeline`: for LilyPond input it supplies the other files of a
folder by relative path so that `\include` can be resolved in memory (including
file's directory first, then root). It is ignored by other formats and changes
nothing when absent.

`convert`, `convertWithTimeline`, `detect`, `formats` and `version` of the wrapper
are synchronous once `ready` is resolved and throw an `Error` otherwise. Rust errors become
JavaScript `Error`s. Details (initialisation, base URL, size,
web examples, Node test, migration) are in `docs/agents/wasm.md`.

This is a deliberate break with the old single-format contract (which returned
text and exposed a dedicated global): Cantus now decodes the
`Uint8Array` with `TextDecoder` (details: `docs/agents/wasm.md` § 9).

## 6. Commands

Always go through `nix develop -c`: the `cargo` on `PATH` outside the Nix shell is
a rustup shim whose component installation conflicts in this
environment (`detected conflict: lib/rustlib/manifest-rust-std-…`).

```sh
nix develop -c cargo test --workspace
nix develop -c cargo clippy --workspace --all-targets -- -D warnings
nix develop -c cargo fmt --all -- --check
nix develop -c cargo build -p scoretk-core --target wasm32-unknown-unknown

nix flake check          # clippy, tests, fmt, licences, wasm32, Node smoke
nix build .#wasm         # result/{esm,classic,package.json,…}

# Compliance: both levels (core, then full parity)
nix develop -c cargo test -p scoretk --test e2e_corpus -- --nocapture

# Derived data
scripts/gen-references.sh
python3 scripts/gen-instruments-table.py \
  ref-musescore/share/instruments/instruments.xml \
  instruments-4.7.4.xml importmidi_instrument.cpp \
  crates/scoretk-core/src/instruments.rs

# Already-built wasm package
nix develop -c node crates/scoretk-wasm/tests/node-smoke.mjs
```

`nix flake check` works offline (crane vendors the crates from
`Cargo.lock`, `CARGO_NET_OFFLINE`): any dependency change requires an up-to-date
`Cargo.lock` and a documented entry in `docs/licences-tierces.md`.

## 7. Compliance tests: two levels

`crates/scoretk/tests/e2e_corpus.rs` compares each `.mscx` to its
4.7.4 reference after normalisation, at two levels:

1. **Musical core** (`Level::Core`): removes `<identification>`, `<defaults>`,
   `<credit>`, `<direction>`, `<stem>`, `<beam>`, `<print>`, formatting and
   position attributes (`default-x`, `placement`, `font-*`,
   `print-object`, …), `<notations>` except `tied`/`tuplet`, `<barline>` except
   `repeat`. Only the files in `EXPECTED_CONFORMANT` must be
   identical; a discrepancy on these files fails the test.
2. **Full parity** (`Level::Full`): removes only pure layout
   (`default-x/y`, `relative-x/y`, `width`, `bezier-x/y`, `<defaults>`,
   `<print>`, `<system-layout>`, `<staff-layout>`, `<encoding-date>`). The test
   is non-blocking and produces `docs/agents/ecarts-complets.md` (markers
   `---DEBUT DOC---` / `---FIN DOC---` in the output).

Known discrepancies and their causes are in `docs/agents/ecarts-noyau.md`.
Read the table before choosing what to fix; a file not listed in
`EXPECTED_CONFORMANT` can regress without failing the suite.

## 8. Tooling and porting pitfalls

Those that produce no visible error:

- **References regenerated with another version of MuseScore**: the compliance
  target changes silently. `scripts/gen-references.sh` uses
  `nix shell nixpkgs#musescore` (4.7.4 at the time of writing); check the
  version printed at startup and change it only explicitly (see
  `docs/agents/upstream.md`).
- **`wasm-bindgen` pinned**: the crate is at `=0.2.130`, equal to the
  flake's `wasm-bindgen-cli`. Updating the flake without changing
  the pin makes `nix build .#wasm` fail (« schema version mismatch »).
- **e2e tests and corpus**: they read `tests/corpus` via
  `CARGO_MANIFEST_DIR/../../tests/corpus`. Running the tests from any
  directory works, but the corpus must be present (see § 1).
- **Node smoke**: it loads `result/esm`; run `nix build .#wasm` first,
  or point `SCORETK_PKG_DIR` / `SCORETK_CORPUS` elsewhere.
- **`ref-musescore/` missing**: the repository does not version it. Headers
  cite paths that do not exist in a fresh checkout; recreate
  the copy with `docs/agents/upstream.md`.
- **Generated files**: `instruments.rs` and `ecarts-complets.md` are
  script/test outputs. A manual edit is lost on
  regeneration.
- **Output order**: the order of `<direction>`, spanners and
  `<credit>` is significant for parity; do not "tidy up" a
  collection without checking the C++ and the reference.
- **Silent ignores**: an unmodelled element is ignored without a message
  (`// TODO(parité)` in the export). An e2e discrepancy that does not move after a
  fix often comes from an unread upstream field, not from the export.
- **`<tick>` and durations**: see the format pitfalls in
  `docs/agents/mscx-format.md` § 17 (implicit tick, `actualTicks`, order of
  `<dots>`, positional voices, 3.x/4.x vs 5.0 ties, 5.0 spanners).

## 9. Continuous integration and releases

Two GitHub Actions workflows live in `.github/workflows/` and build on
Nix (same toolchain as `nix develop -c`):

- `ci.yml` — on push and pull request to `master`: `cargo fmt --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, the `wasm32-unknown-unknown` builds of the list in
  § Commands of `AGENTS.md`, `nix build .#wasm` then the Node test
  `node-smoke.mjs`. The
  `tests/corpus` corpus is versioned; the only test reading a file outside the repository
  (`change_dans_beam_cross_staff` of `scoretk-lilypond-import`) silently returns
  when that file is absent, hence has no effect on CI.
- `release.yml` — see below.

Both files can be validated locally with `actionlint` (`act` is not
installed in the dev shell):

```sh
nix-shell -p actionlint --run 'actionlint .github/workflows/*.yml'
```

### Publishing a version

1. Update `version` in `[workspace.package]` of `Cargo.toml` (and, if
   needed, `version` in `flake.nix`) so that `scoretk --version` and `version()`
   of the wasm coincide with the tag.
2. Let CI pass on `master`, then:

   ```sh
   git tag v0.2.0 && git push origin v0.2.0
   ```

3. The `v*` trigger creates the GitHub release named after the tag, with
   automatically generated notes. Do not create it by hand.

### Rolling « nightly » release

Every push to `master` produces a **prerelease** on the moving
`nightly` tag: the tag is moved to the latest commit and old
assets are deleted before the new ones are uploaded (concurrency cancels
a rolling run still in progress). It is always at the same place:

- <https://github.com/jperon/scoretk/releases/tag/nightly>

`releases/latest` still designates the latest `v*` release, not the
rolling one. Archives carry the computed version: tag `vX.Y.Z` for
a normal release, `0.1.0-nightly.AAAAAMMJJ.<sha court>` for the rolling
release (base version read from `Cargo.toml`). The CLI binary is compiled with the
`Cargo.toml` version: the suffix is not rewritten in the Rust code,
it appears in archive names and in the npm package version. For
a nightly to show its suffix in `scoretk --version`, the code would have to be changed:
that is not done here.

### Artefact contents

For each target (`x86_64-unknown-linux-musl`, `aarch64-apple-darwin`,
`x86_64-apple-darwin`, `x86_64-pc-windows-msvc`), an archive
`scoretk-<version>-<cible>.tar.gz` (`.zip` on Windows) contains the binary
`scoretk`, `LICENSE`, `NOTICE`, `README.md` and `SOURCES.txt`. The wasm package
(output of `nix build .#wasm`, packaged by `npm pack`) gives
`scoretk-<version>-wasm.tgz` with the same notices. `SOURCES.txt` carries the exact URL
of the corresponding source (tag or commit of the nightly), as
required by GPL §6 (see `docs/agents/licence.md`). A global `SHA256SUMS`
file is attached to each release.

The workflow publishes with `GITHUB_TOKEN` and `permissions: contents: write`
only in the publication job; the other jobs stay read-only. No tag or release is created from the workstation.
