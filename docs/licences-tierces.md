# Licences of third-party dependencies

This document lists the third-party crates **actually linked** in the binaries
produced by the workspace (`scoretk-core`, `scoretk` and its CLI, plus the
wasm and MIDI crates), with their licence, and checks their compatibility with the
project's licence, **GPL-3.0-only**.

## Scope and method

```console
$ cargo tree --offline --workspace --edges normal,build --prefix none \
       --format '{p}|{l}'
```

The command makes no network access and includes neither development
dependencies (`--edges normal,build`) nor packages present in
`Cargo.lock` for other targets or for optional features
(see below); development dependencies, including those of the
`criterion` performance benchmark, are listed separately (§ "Development
dependencies"). Result as of 2026-11-17: **46 third-party crates**, all
compatible.

No third-party crate was added for the MEI directions: the
`scoretk-mei-*` and `scoretk-xml-common` crates reuse the already listed
`quick-xml` and `thiserror`.

## Reused specifications outside Cargo

The GABC crates implement Gregorio's syntax (`doc/Gabc.tex`,
`src/gabc/gabc-score-determination.{y,l}`, `src/struct.{c,h}`), commit
`b63f074`. Gregorio is **GPL-3.0-or-later**, Copyright (C) The Gregorio
Project, which permits redistribution under this project's GPL-3.0-only
(`NOTICE`, `docs/agents/gabc.md`). No Gregorio source file is vendored and no
new third-party Cargo dependency was introduced by the GABC direction: the
three new crates are workspace members using only `scoretk-core` and the
already listed `thiserror`.

### MEI Sample Encodings (measurement data, not linked)

`tests/corpus/mei-samples/` version only two manifests: the official encodings
it describes are fetched by `scripts/fetch-mei-samples.sh` at commit
`f3f1baba02e32279b25dad660b82c834bac034b4` into `$SCORETK_MEI_SAMPLES`
(outside the repository). Origin:
<https://github.com/music-encoding/sample-encodings>, directory `MEI_5.1`,
Copyright (c) Music Encoding Initiative (MEI) Board, licence **ECL-2.0**
(upstream `LICENSE`: the Apache-2.0 terms with a modified patent grant). This
is the same licence as the MEI 5.1 RelaxNG schemas already used as a format
reference, which this document previously classified as GPL-3 compatible.
ECL-2.0 §4 requires retaining the copyright/attribution notice and providing a
copy of the licence to recipients; `NOTICE` carries the attribution and this
section records the licence. The files are read-only measurement data: no code
is executed and no artefact links them. No Cargo dependency was added.

### Leland font data (SIL OFL 1.1, generated subset)

`crates/scoretk-smufl/fonts/leland/` vendors the Leland font data shipped by
MuseScore **4.7.4** in `fonts/leland/` (tag `v4.7.4`, commit
`7688c005ad963ba09ee715aae53dbc7e8c9d5ef8`): `Leland.otf`,
`leland_metadata.json`, `LICENSE.txt`, `OFL-FAQ.txt`, `FONTLOG.txt` and
`README.md`. Leland is Copyright (C) MuseScore BVBA and Klaus Rettinghaus and
is licensed under the **SIL Open Font License 1.1**; the licence text, FAQ and
font log are kept next to the font data (OFL §1/§2). The MuseScore 4.7.4 OTF
and metadata are byte-identical to the Leland 0.80 copy in Verovio, but the
MuseScore originals are the ones used and declared in `NOTICE`.

`scripts/gen-smufl-subset.py` reads these files and writes the generated
`crates/scoretk-smufl/src/glyphs.rs`: outlines converted from CFF cubics to
quadratics, advances, bounding boxes, SMuFL anchors and `engravingDefaults`
for the notation subset. The script uses only the Python standard library and
is run with `python3 -I`; the font data is never executed. The generated
constants are compiled into `scoretk-smufl` (GPL-3.0-only code); the OFL-1.1
font data is *not* linked as a crate and introduces no Cargo dependency. OFL
1.1 is GPL-3 compatible, so the combined work remains GPL-3.0-only.

### Edwin font data (SIL OFL 1.1, generated text subset)

`crates/scoretk-smufl/fonts/edwin/` vendors `Edwin-Roman.otf`,
`Edwin-Bold.otf`, `LICENSE.txt`, `README.md` and `FONTLOG.txt` from MuseScore
**4.7.4** `fonts/edwin/` (tag `v4.7.4`, commit
`7688c005ad963ba09ee715aae53dbc7e8c9d5ef8`). Edwin is Copyright (C) 2021
MuseScore Limited, reserved font name « Edwin », licensed under the **SIL
Open Font License 1.1**; its licence, README and font log are kept alongside
the data (OFL §1/§2).

`scripts/gen-edwin-subset.py` reuses the dependency-free CFF reader of
`gen-smufl-subset.py` and writes the generated
`crates/scoretk-smufl/src/text.rs`: 167 codepoints per style (ASCII, Latin-1
and the typographic signs used by the corpus), real advance widths, bounding
boxes and quadratic outlines. It runs with `python3 -I` and never executes
the font. The generated constants are compiled into `scoretk-smufl` (GPL-3.0
code); the OFL font files are not linked and add no Cargo dependency. OFL 1.1
is GPL-3 compatible, so the combined work remains GPL-3.0-only. The text
subset is smaller than notation glyph data: about 93 KB of raw quadratic
edges for both styles, well inside the font-data budget of
`docs/agents/engraving.md` §1.

## Table

| Crate | Version | Declared licence | Role |
| --- | --- | --- | --- |
| `adler2` | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | core (via miniz_oxide) |
| `anstream` | 1.0.0 | MIT OR Apache-2.0 | CLI (via clap) |
| `anstyle` | 1.0.14 | MIT OR Apache-2.0 | CLI (via clap) |
| `anstyle-parse` | 1.0.0 | MIT OR Apache-2.0 | CLI (via anstream) |
| `anstyle-query` | 1.1.5 | MIT OR Apache-2.0 | CLI (via anstream) |
| `bumpalo` | 3.20.4 | MIT OR Apache-2.0 | core and wasm (via zopfli, wasm-bindgen-macro-support) |
| `cfg-if` | 1.0.5 | MIT OR Apache-2.0 | core (via zip, js-sys) |
| `clap` | 4.6.7 | MIT OR Apache-2.0 | CLI (arguments) |
| `clap_builder` | 4.6.7 | MIT OR Apache-2.0 | CLI (via clap) |
| `clap_derive` | 4.6.7 | MIT OR Apache-2.0 | CLI (proc-macro) |
| `clap_lex` | 1.1.1 | MIT OR Apache-2.0 | CLI (via clap) |
| `colorchoice` | 1.0.5 | MIT OR Apache-2.0 | CLI (via anstream) |
| `crc32fast` | 1.5.5 | MIT OR Apache-2.0 | core (via zip, flate2, zopfli) |
| `equivalent` | 1.0.3 | Apache-2.0 OR MIT | core (via indexmap) |
| `flate2` | 1.1.11 | MIT OR Apache-2.0 | core (via zip) |
| `futures-core` | 0.3.34 | MIT OR Apache-2.0 | wasm (via js-sys) |
| `futures-task` | 0.3.34 | MIT OR Apache-2.0 | wasm (via futures-util) |
| `futures-util` | 0.3.34 | MIT OR Apache-2.0 | wasm (via js-sys) |
| `hashbrown` | 0.17.1 | MIT OR Apache-2.0 | core (via indexmap) |
| `heck` | 0.5.0 | MIT OR Apache-2.0 | CLI (proc-macro, via clap_derive) |
| `indexmap` | 2.14.2 | Apache-2.0 OR MIT | core (via zip) |
| `is_terminal_polyfill` | 1.70.2 | MIT OR Apache-2.0 | CLI (via anstream) |
| `js-sys` | 0.3.107 | MIT OR Apache-2.0 | wasm (JS bindings) |
| `log` | 0.4.35 | MIT OR Apache-2.0 | core and wasm (via zopfli, wasm-bindgen) |
| `memchr` | 2.8.3 | Unlicense OR MIT | core (via quick-xml, zip) |
| `midly` | 0.5.3 | Unlicense | MIDI (file format, `scoretk-midi-import` and `scoretk-midi-export`) |
| `miniz_oxide` | 0.9.1 | MIT OR Zlib OR Apache-2.0 | core (via flate2) |
| `pin-project-lite` | 0.2.17 | Apache-2.0 OR MIT | wasm (via futures-util) |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 | proc-macro |
| `quick-xml` | 0.42.0 | MIT | core (XML reading); tests of `scoretk-lilypond-import` (`read_corpus`) |
| `quote` | 1.0.47 | MIT OR Apache-2.0 | proc-macro |
| `rustversion` | 1.0.23 | MIT OR Apache-2.0 | wasm (build, via wasm-bindgen) |
| `simd-adler32` | 0.3.10 | MIT | core (via miniz_oxide, zopfli) |
| `slab` | 0.4.12 | MIT | wasm (via futures-util) |
| `strsim` | 0.11.1 | MIT | CLI (via clap) |
| `syn` | 3.0.10 | MIT OR Apache-2.0 | proc-macro |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 | core (errors) |
| `thiserror-impl` | 2.0.21 | MIT OR Apache-2.0 | core (proc-macro) |
| `unicode-ident` | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | proc-macro |
| `utf8parse` | 0.2.2 | Apache-2.0 OR MIT | CLI (via anstyle-parse) |
| `wasm-bindgen` | 0.2.130 | MIT OR Apache-2.0 | wasm (bindings) |
| `wasm-bindgen-macro` | 0.2.130 | MIT OR Apache-2.0 | wasm (proc-macro) |
| `wasm-bindgen-macro-support` | 0.2.130 | MIT OR Apache-2.0 | wasm (proc-macro) |
| `wasm-bindgen-shared` | 0.2.130 | MIT OR Apache-2.0 | wasm (via wasm-bindgen) |
| `zip` | 2.6.1 | MIT | core (.mscz container) |

## Compatibility with GPL-3.0-only

All the licences found are **permissive** and compatible with
GPL-3.0-only, which can absorb permissively licensed code and
redistribute it under the GPL:

- **MIT**, **ISC**, **0BSD**, **Zlib**, **X11**, **BSD-2-Clause**,
  **BSD-3-Clause**, **CC0-1.0**, **Unlicense**: permissive, compatible
  with all versions of the GPL. The "advertising" clauses of BSD-3 and
  X11 are satisfied by the `NOTICE` file (project origin, no
  additional contributor list to display).
- **Apache-2.0** (alone or as an alternative): compatible with GPL **v3**
  (the FSF classifies it as GPLv3-compatible; the known incompatibility concerns
  GPLv2). Apache-2.0's patent clause and notice obligations
  apply in addition: the project's `NOTICE` file and this document
  serve as them for the whole.
- **Unicode-3.0** (via `unicode-ident`): permissive Unicode data
  licence, compatible.
- The `A OR B` expressions only require respecting the chosen branch;
  all the alternatives offered are in the whitelist.

No dependency is under GPL, LGPL, AGPL, MPL, EUPL, SSPL or an unidentified
licence: no relicensing question arises.

## Crates present in `Cargo.lock` but not linked

These packages do not appear in the `normal,build` graph above:

| Package | Reason | Known licence |
| --- | --- | --- |
| `similar` | **development** dependency of `scoretk-core` and `scoretk-midi-import` (tests) | Apache-2.0 |
| `arbitrary`, `derive_arbitrary` | optional `zip` feature, not enabled | MIT OR Apache-2.0 |
| `anstyle-wincon`, `windows-sys`, `windows-link` | Windows target only (terminal colours) | MIT OR Apache-2.0 |
| `once_cell_polyfill` | Windows chain of `clap`/`anstream` | MIT OR Apache-2.0 |
| `serde_core`, `serde_derive` | optional (via `zip`/`bstr`), not enabled | MIT OR Apache-2.0 |
| `bstr` | optional, not enabled | MIT OR Apache-2.0 |
| `crossbeam-utils` | optional, not enabled | MIT OR Apache-2.0 |

**Windows** builds would link `anstyle-wincon`, `windows-sys` and
`windows-link`, all under MIT OR Apache-2.0: compatible as well.

### Development dependencies: performance benchmark (`criterion`)

`crates/scoretk` declares `criterion` 0.8.2 as a **development dependency**,
with `default-features = false` and the single feature
`cargo_bench_support` (neither `plotters`, nor `html_reports`, nor `rayon`), for
`crates/scoretk/benches/conversion.rs` (see `docs/agents/performance.md`).
These crates are compiled only by `cargo bench` and `cargo test`: they
do not enter the `normal,build` graph above — which remains at
**46 crates** — and are therefore not linked in any delivered binary. Surveyed on
2026-11-18 with:

```console
$ cargo tree -p scoretk --edges normal,dev --prefix none --format '{p}|{l}'
```

| Crate | Version | Declared licence | Role |
| --- | --- | --- | --- |
| `alloca` | 0.4.0 | MIT | criterion (allocation-free measurement during timing) |
| `anes` | 0.1.6 | MIT OR Apache-2.0 | criterion (statistics) |
| `cast` | 0.3.0 | MIT OR Apache-2.0 | criterion (numeric conversions) |
| `ciborium` | 0.2.2 | Apache-2.0 | criterion (result storage) |
| `ciborium-io` | 0.2.2 | Apache-2.0 | criterion (via ciborium) |
| `ciborium-ll` | 0.2.2 | Apache-2.0 | criterion (via ciborium) |
| `criterion` | 0.8.2 | Apache-2.0 OR MIT | performance benchmark |
| `criterion-plot` | 0.8.2 | Apache-2.0 OR MIT | criterion (hard dependency, plots disabled) |
| `either` | 1.19.0 | MIT OR Apache-2.0 | criterion (via itertools) |
| `half` | 2.7.1 | MIT OR Apache-2.0 | criterion (via ciborium) |
| `itertools` | 0.13.0 | MIT OR Apache-2.0 | criterion |
| `itoa` | 1.0.19 | MIT OR Apache-2.0 | criterion (via serde_json) |
| `libc` | 0.2.191 | MIT OR Apache-2.0 | criterion (via page_size) |
| `num-traits` | 0.2.19 | MIT OR Apache-2.0 | criterion (via half) |
| `oorandom` | 11.1.5 | MIT | criterion (random sampling) |
| `page_size` | 0.6.0 | MIT/Apache-2.0 | criterion (alloca) |
| `regex` | 1.13.2 | MIT OR Apache-2.0 | criterion |
| `regex-automata` | 0.4.19 | MIT OR Apache-2.0 | criterion (via regex) |
| `regex-syntax` | 0.8.12 | MIT OR Apache-2.0 | criterion (via regex) |
| `same-file` | 1.0.6 | Unlicense/MIT | criterion (via walkdir) |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | criterion (results) |
| `serde_core` | 1.0.229 | MIT OR Apache-2.0 | criterion (via serde) |
| `serde_derive` | 1.0.229 | MIT OR Apache-2.0 | criterion (proc-macro, via serde) |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 | criterion (results) |
| `syn` | 2.0.119 | MIT OR Apache-2.0 | proc-macro (via serde_derive) |
| `tinytemplate` | 1.2.1 | Apache-2.0 OR MIT | criterion (text reports) |
| `walkdir` | 2.5.0 | Unlicense/MIT | criterion |
| `zerocopy` | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | criterion (via half) |
| `zerocopy-derive` | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | proc-macro (via zerocopy) |
| `zmij` | 1.0.23 | MIT | criterion (via serde_json) |

All these licences are already in the whitelist of
`scripts/check-licences.py` (MIT, Apache-2.0, BSD-2-Clause, Unlicense) and
compatible with GPL-3.0-only. `page_size` declares `MIT/Apache-2.0` (slash,
a non-SPDX form): both branches are permitted, and the crate stays
outside the checked graph since it is pulled only by the benchmark.

### Development dependencies: raster spike (`tiny-skia`)

`crates/scoretk-render-raster` declares `tiny-skia` 0.11.4 as a **development
dependency** for `benches/spike.rs` only (phase P0 comparison, see
`docs/agents/engraving.md` § 9.3). It is not linked into any delivered library,
binary or wasm package and does not change the 46-crate `normal,build` graph
above. Surveyed on 2026-11-27 with:

```console
$ cargo tree -p scoretk-render-raster --edges normal,dev --prefix none --format '{p}|{l}'
```

| Crate | Version | Declared licence | Role |
| --- | --- | --- | --- |
| `tiny-skia` | 0.11.4 | BSD-3-Clause | reference rasteriser in the benchmark |
| `tiny-skia-path` | 0.11.4 | BSD-3-Clause | path representation (via tiny-skia) |
| `arrayref` | 0.3.9 | BSD-2-Clause | tiny-skia internals |
| `arrayvec` | 0.7.8 | MIT OR Apache-2.0 | tiny-skia internals |
| `bytemuck` | 1.25.2 | Zlib OR Apache-2.0 OR MIT | tiny-skia internals |
| `strict-num` | 0.1.1 | MIT | tiny-skia-path internals |
| `miniz_oxide` | 0.8.9 | MIT OR Zlib OR Apache-2.0 | png (via tiny-skia); the linked graph uses 0.9.1 |
| `png`, `fdeflate`, `flate2`, `crc32fast`, `cfg-if`, `log`, `bitflags`, `simd-adler32`, `adler2` | see main table | MIT, Apache-2.0, 0BSD | pulled by tiny-skia's PNG feature, already listed above |

BSD-3-Clause was already in the whitelist of `scripts/check-licences.py`; no
script change is needed. The in-house rasteriser itself has no dependency.

Build tools outside Cargo (not linked in the delivered binaries):
`wasm-bindgen-cli` (MIT OR Apache-2.0), `binaryen` (Apache-2.0),
`nodejs` (MIT) and the Rust toolchain (MIT OR Apache-2.0).

## Automated check

`scripts/check-licences.py` runs the command above, evaluates SPDX
expressions (`AND` binds more tightly than `OR`, parentheses handled) and fails
if a licence is outside the whitelist or missing. The flake check
runs it offline on the vendored sources:

```console
$ nix flake check            # includes the « licences » check
$ nix build .#checks.<système>.licences
$ nix develop -c python3 scripts/check-licences.py   # in development
```

Any new dependency must be added here **and**, if its licence is genuinely
new, to `LICENCES_PERMISES` in `scripts/check-licences.py`
after a GPL-3.0-only compatibility check.
