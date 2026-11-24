# WebAssembly package of the scoretk facade

Crate `crates/scoretk-wasm`, Nix package `packages.wasm`, check
`checks.wasm-node-smoke`. It is the only wasm package in the repository. The
project core (rules, commands, index) remains in `AGENTS.md`.

## 1. Role

`crates/scoretk-wasm` is a **wasm-bindgen wrapper** of the
`crates/scoretk` facade: no conversion logic, only exposure of
`read`/`write`/`Format` to JavaScript. It enables **all** the features of the
facade (`musicxml-import`, `midi`, `lilypond`, `lilypond-import`, `mscx`,
`abc`), like the `scoretk` CLI: the browser package can therefore read and write
all seven formats.

| Crate | Surface | Nix package |
| --- | --- | --- |
| `crates/scoretk-wasm` | `convert`, `convertWithTimeline`, `convertDetailed` (additive), `detect`, `formats`, `version` | `packages.wasm` (`nix build .#wasm`) |

## 2. Build and size

```sh
nix build .#wasm
cat result/SIZE
ls -la result/esm result/classic
```

`result/` contains:

```
esm/scoretk_wasm.js           ES module (wasm-bindgen --target web)
esm/scoretk_wasm_bg.wasm      optimised binary (wasm-opt -Oz)
esm/scoretk_wasm.d.ts         TypeScript types (custom section: see § 3)
classic/scoretk_wasm.js       classic script (--target no-modules)
classic/scoretk_wasm_bg.wasm  same binary, optimised separately
classic/scoretk.js            wrapper: self.scoretk = { ready, convert, version, detect, formats, baseUrl }
classic/scoretk_wasm.d.ts     types
package.json                  name "scoretk", type "module", exports/types, repository
README.md                     short package how-to
LICENSE                       GPL-3.0-only text
NOTICE                        MuseScore Studio origin, reused data, trademark
SIZE                          raw and gzip measurement of the .wasm
```

Measurement (wasm-bindgen 0.2.130, binaryen 133 / `wasm-opt`, rustc 1.100.0,
`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `wasm-opt -Oz
--strip-debug --converge`), state of commit `fdf22443` plus the `--converge`
pass (2026-11-21):

| Binary | Raw | Gzip -9 | Brotli -q 11 |
| --- | --- | --- | --- |
| cdylib `scoretk_wasm.wasm` (rustc output) | 4 472 499 bytes | — | — |
| `scoretk_wasm_bg.wasm` (after wasm-bindgen) | 4 223 436 bytes | — | — |
| `scoretk_wasm_bg.wasm` (after `wasm-opt -Oz --strip-debug --converge`) | **1 924 141 bytes** | **782 198 bytes** | **597 632 bytes** |

The two `esm` and `classic` variants share the same binary byte for byte.
The package remains **complete, all features enabled**: it stays below
Cantus's limit (≈ 2.5 Mo raw).

The previous reference (1 552 892 raw / 641 472 gzip, commit `a7f690c`) predates
the addition of the ABC and LilyPond readers, the MIDI reader and the ten
directions of the facade: the binary has grown with the features, not through a
build regression (§ "Composition" below).

`panic = "abort"` was **measured then discarded** on an earlier version of the
package: it removed only 20 raw bytes (1 468 917 instead of 1 468 937,
i.e. 0.001 %) and 34 gzip bytes. Re-measured on 2026-11-21: −2 566 raw bytes
(−0.13 %), −1 162 gzip bytes but +117 brotli bytes. `wasm32-unknown-unknown`
cannot unwind, so only the residual cleanup shims disappear and a panic still
traps into a JavaScript `RuntimeError`; the gain stays below the 2 % threshold
(§ "Build settings evaluated on 2026-11-21").

The additions measured since the reference size of 1 472 792 raw bytes
brought the binary to 1 482 794 raw bytes / 621 633 gzip bytes. Three
distinct contributions: +9 555 raw bytes (+0.65 %) for
`convertWithTimeline` (`timemap_js.rs`, note sorting), +414 raw bytes
(+0.03 %) for note velocity (`Note::user_velocity`) and +33 raw bytes
for the `i64` computation of `customize_velocity` (overflow protection), i.e. a
total of +10 002 raw bytes (+0.68 %). The overhead stays below the 2 % threshold.
(Historical figures, before the ABC/LilyPond/MIDI readers.)

### Effect of the crate split

The restructuring into 17 crates (one `-common`, one `-import` and one `-export`
per bidirectional format) is measured with the same pipeline (`nix build .#wasm`):

| State | Raw | Gzip |
| --- | --- | --- |
| Before restructuring (`4767464`) | 1 485 733 | 621 845 |
| Step 1: core split into `scoretk-core` + `-musescore-import` + `-musicxml-export` (`609efad`) | 1 544 160 | 636 991 |
| After integration (LilyPond, ABC, MIDI split; renames) | **1 552 892** | **641 472** |

That is +67 159 raw bytes (+4.52 %) and +19 627 gzip bytes (+3.16 %), of which
+58 427 for the core split alone and +8 732 for the integration, for a
refactor with no logic change. Diagnosis, with figures:

- **No new duplication.** `cargo tree -d` is empty (no crate compiled twice) and the number of identically named functions appearing
  several times is the same before/after (6, of which four `hashbrown`) ;
  `wasm-opt -Oz --duplicate-function-elimination` recovers only 110 bytes.
- **No double path.** `scoretk::convert` is no longer reached by the
  package (the wasm API goes through `read`/`write`/`write_playback`) and its symbol
  has disappeared from the binary: the historical orchestration path costs nothing.
  `write_playback` reuses the same `export_musicxml_with` and the same MIDI
  rendering as `write`.
- **LTO still active.** With the same pipeline, `lto = false` gives
  1 535 725 bytes (baseline) and 1 609 180 (after integration), versus 1 482 791 and
  1 550 324 with `lto = true`: the 53 to 59 ko difference is preserved, so the
  split did not break LTO. `lto = "thin"` ties with `lto = true`
  (less than 300 bytes), so nothing to gain there.
- **What grows: the code, by number of functions.** The code goes from
  3 311 to 3 543 functions (names preserved, `wasm-opt -Oz --debuginfo`), for
  +52 ko of code and +13 ko of static data (`.rodata`). Average
  function sizes remain the same (≈ 380 bytes): it is the number of small
  and medium functions that increases, i.e. helpers that were
  inlined/merged into their caller and now remain full-fledged functions, at crate boundaries.

It is therefore the normal effect of splitting on inter-crate inlining, and not a
code regression: the split costs ≈ 3.2 % of gzip through lost inter-crate
inlining (3 311 → 3 543 functions), without duplication or double path.
This growth is **accepted** (decision made), the package staying under the
Cantus limit. The levers remain documented for information,
**not applied** (none roughly exceeds the 2 % threshold of the
wasm contract): keep `lto = "fat"` (already the case), avoid crate boundaries on
hot paths (read/write a note, a chord, a measure), merge the
two most intertwined crates (`scoretk-musescore-import` +
`scoretk-musicxml-export` were inside `scoretk-core` before step 1 : moving
them back into the same crate would recover the 58 ko of step 1), or enable
`-Z share-generics` (nightly), which would deduplicate generic monomorphisations
across crates.

## Binary composition — measurement of 2026-11-21

State: commit `fdf22443`, delivered binary of **1 924 924 raw / 783 883 gzip /
599 312 brotli** bytes, i.e. **before** the `--converge` pass adopted below
(it removes 783 raw, 1 685 gzip and 1 680 brotli bytes, 0.04 % of the binary).
`.rodata.2837` and `.rodata.1419` figures below come from the raw data section;
twiggy reports 10 bytes more per segment (alignment).

### Reproduction method

```sh
nix build .#wasm
# INPUT_WASM: the cdylib produced by rustc, before wasm-bindgen
nix develop -c cargo build --release -p scoretk-wasm --target wasm32-unknown-unknown
nix develop -c wasm-bindgen --target web --out-dir /tmp/m/esm --out-name scoretk_wasm \
  target/wasm32-unknown-unknown/release/scoretk_wasm.wasm
nix develop -c wasm-opt -Oz --debuginfo /tmp/m/esm/scoretk_wasm_bg.wasm -o /tmp/m/debug.wasm
nix shell nixpkgs#twiggy -c twiggy top -f csv -o /tmp/m/top.csv /tmp/m/debug.wasm
nix shell nixpkgs#twiggy -c twiggy dominators -d 3 /tmp/m/debug.wasm
nix shell nixpkgs#twiggy -c twiggy garbage /tmp/m/debug.wasm
```

The variant with symbols weighs 2 722 922 bytes; its name sections
(`"function names"` 734 629 bytes, `"data names"` 63 294 bytes) are absent from
the delivered binary. Removing them leaves 1 924 953 bytes, within 29 bytes of
the delivered size: percentages below use that as denominator and are therefore
those of the shipped module.

Structural counts: 3 824 functions, 4 303 active data segments, 142 types,
15 imports, 16 exports.

### Aggregation by crate (bytes present in the delivered binary)

| Bucket | Bytes | % |
| --- | ---: | ---: |
| data segments (`.rodata` + active data) | 384 948 | 20,00 |
| `alloc` | 218 075 | 11,33 |
| `core` (outside `core::fmt`/`core::panicking`) | 214 346 | 11,14 |
| `scoretk_lilypond_import` | 165 056 | 8,57 |
| `scoretk_musescore_import` | 151 085 | 7,85 |
| `scoretk_musicxml_import` | 103 062 | 5,35 |
| `scoretk_musicxml_export` | 101 095 | 5,25 |
| `scoretk` (facade) | 100 175 | 5,20 |
| `scoretk_midi_export` | 63 182 | 3,28 |
| wasm-bindgen `convert` multivalue shim | 57 884 | 3,01 |
| `scoretk_midi_import` | 46 587 | 2,42 |
| `zip` | 45 570 | 2,37 |
| `scoretk_musescore_export` | 40 904 | 2,12 |
| `core::fmt` (`Debug`/`Display`, isolated) | 31 983 | 1,66 |
| `scoretk_core` | 30 370 | 1,58 |
| `scoretk_abc_import` | 29 319 | 1,52 |
| other + wasm sections | 20 198 | 1,05 |
| `miniz_oxide` | 19 345 | 1,00 |
| `quick_xml` | 17 958 | 0,93 |
| `scoretk_lilypond_export` | 12 806 | 0,67 |
| `compiler_builtins` | 9 812 | 0,51 |
| `flate2` | 9 192 | 0,48 |
| `dlmalloc` | 7 427 | 0,39 |
| `memchr` | 5 760 | 0,30 |
| `scoretk_abc_common` | 5 512 | 0,29 |
| `std` | 3 515 | 0,18 |
| `indexmap` | 3 182 | 0,17 |
| `hashbrown` | 2 227 | 0,12 |
| `wasm_bindgen` | 2 214 | 0,12 |
| `scoretk_abc_export` | 1 889 | 0,10 |
| `midly` | 1 827 | 0,09 |
| `scoretk_wasm` | 1 630 | 0,08 |
| `core::panicking` | 1 527 | 0,08 |
| `crc32fast` | 1 341 | 0,07 |
| remaining (types, globals, `js_sys`, `scoretk_midi_common`, …) | ~3 100 | 0,16 |
| **total** | **1 924 953** | **100,00** |

Reading:

- **The data section is now the first item (20.0 % of the binary)**, ahead of
  code: 4 303 segments, dominated by generated tables and panic/error messages
  (§ "Largest data segments").
- **`alloc` + `core` = 22.5 %**: `Vec`/`String`/`BTreeMap`/sorting machinery and
  `core` formatting/parsing, not format logic. `core::fmt` alone is 1.66 %
  (31 983 bytes), `core::panicking` 0.08 %: marginal, as before.
- The five readers and five writers are balanced (1.5 % to 8.6 % each);
  `scoretk-lilypond-import` is the heaviest crate (8.57 %) because of its
  parser.
- The wasm-bindgen **multivalue shim for `convert`** alone weighs 57 884 bytes
  (3.01 %): it copies input into linear memory, calls the Rust function and
  builds the returned `(ptr, len, ptr, len)` tuple. `convertWithTimeline` has a
  smaller shim.
- `zip` + `miniz_oxide` + `flate2` + `crc32fast` = 82 448 bytes (4.28 %): the
  pure-Rust cost of `.mscz`/`.mxl` containers. `zopfli` is no longer in the
  graph.

### Largest data segments

| Segment | Twiggy bytes | Content identified |
| --- | ---: | --- |
| `.rodata.1419` | 40 451 | concatenated string literals of the writers (MusicXML element/attribute names, `<sym>` glyph names, directions, harmony tables) |
| `.rodata.14` | 26 801 | panic messages (`assertion failed: …`) and French user-facing error messages |
| `.rodata.2837` | 25 645 | instruments table: names and `wind.flutes.flute.piccolo`-style ids (`read/instruments.rs`, generated) |
| `.rodata.1` | 22 695 | mixed literals: LilyPond templates (`\once \override …`), `staff`/`voice` names, error messages |
| `.rodata.4243` | 20 720 | `miniz_oxide` internal assertion messages (`out_pos + 3 < out_slice.len()`, …) |
| `.rodata.13` | 18 899 | LilyPond reader tables: pitch names, errors, volta/tremolo keywords |
| `.rodata.2833` | 7 743 | `.mscx` element/attribute names (`page-margins`, `defaults`, …) |
| `.rodata.4271` | 7 737 | binary (non-text) table |
| `.rodata.1417` | 7 676 | panic messages of `alloc` (`LeftOrRight::…`, one-time initialisation) and `zip` errors |
| `.rodata.2821` | 5 828 | ABC/MuseScore tables: articulations, clefs, ornament names |
| `.rodata.4295` | 5 284 | binary (non-text) table, weakly compressible |
| `.rodata.2822` | 5 222 | `quick_xml`/`alloc` errors and CDATA/XML literal markers |
| `.rodata.1415` | 4 074 | French error messages (`accord présent`, `armure`) and MIDI instrument names |

The other 4 290 segments are below 4 Ko. `twiggy garbage` reports 282 bytes of
truly dead types; the 320 058 bytes flagged in data segments are false
positives (segments addressed through computed indices).

### Dominators

The root is the `convert` export (and `convertWithTimeline`); it dominates
`scoretk::read` (946 487 retained bytes, 34.76 %), itself dominating the five
readers, of which `musescore_import::read_score` is the largest
(171 697 bytes, 6.31 %). No abnormal cycle.

### Build settings evaluated on 2026-11-21

All candidates below were measured on the same state; each one was kept only if
the conversion outputs stayed **byte-identical** (checked for the ten
directions with `scripts/check-wasm-identity.mjs`, § 6).

| Candidate | Raw | Gzip -9 | Brotli -q 11 | Decision |
| --- | ---: | ---: | ---: | --- |
| `wasm-opt -Oz --strip-debug` (baseline) | 1 924 924 | 783 883 | 599 312 | reference |
| `+ --converge` | **1 924 141** | **782 198** | **597 632** | **kept** (−783 / −1 685 / −1 680) |
| `+ --strip-producers --strip-target-features` (without `--converge`) | 1 924 658 | 783 666 | 599 248 | not kept (−266 raw, loses feature metadata) |
| `+ --converge --strip-producers --strip-target-features` | 1 923 875 | 781 993 | 597 167 | not kept (−266 more, same metadata loss) |
| `--gufa` | 1 926 572 | 784 469 | 599 730 | worse |
| `--gufa --converge` | 1 925 741 | 782 489 | 597 963 | worse than `--converge` |
| `-O3 --converge` | 1 958 850 | 783 492 | 597 751 | worse raw, no compression gain |
| `--closed-world` | 1 924 924 | 783 891 | 599 312 | no effect |
| `wasm-opt --strip-debug` twice (second pass) | — | — | — | no effect (`--converge` already reaches the fixpoint) |

`--converge` is a pure optimisation fixpoint (no semantic flag): it is applied
in `flake.nix` for both variants.

Two profile levers were evaluated with the same local pipeline (cargo +
wasm-bindgen + wasm-opt; the local cdylib is 2 224 bytes smaller than the Nix
one, so only the deltas are meaningful):

| Candidate | Raw | Gzip -9 | Brotli -q 11 | Δ vs local baseline |
| --- | ---: | ---: | ---: | --- |
| local baseline (`opt-level = "z"`, `lto = true`, `codegen-units = 1`) | 1 922 703 | 783 813 | 599 062 | — |
| `panic = "abort"` (`CARGO_PROFILE_RELEASE_PANIC=abort`) | 1 920 137 | 782 651 | 599 179 | −2 566 raw (−0.13 %), −1 162 gzip, **+117 brotli** |
| `opt-level = "s"` | 2 186 037 | 891 997 | 668 537 | +263 334 raw (+13.7 %), much worse |

- **`panic = "abort"` is not kept**: the gain stays far below the 2 % threshold
  of the wasm contract, brotli slightly regresses (the dominant HTTP encoding),
  and applying the profile setting globally would also change the CLI's panic
  behaviour for an internal bug. On a two-line test program, `-C panic=abort`
  changes the cdylib by 2 bytes only: `wasm32-unknown-unknown` cannot unwind, so
  the full-package delta is just the residual cleanup shims. A panic would still trap
  into a JavaScript `RuntimeError` (not a silent failure).
- **`opt-level = "s"` is not kept**: after `wasm-opt -Oz`, `"z"` is
  decisively smaller (13.7 % raw). `lto = "thin"` was already documented as a
  tie with `lto = true`; `codegen-units = 1` is already in
  `[profile.release]`.
- **`strip` gives nothing**: the release cdylib has no name or DWARF sections
  (the 3 824 function names appear only in the `--debuginfo` twiggy variant);
  `--strip-debug` is therefore already the minimal form. The `producers` and
  `target_features` custom sections weigh 266 bytes together; removing them
  would drop the engine-facing feature declaration for a 0.01 % gain, so it was
  rejected.

### Data candidates inspected

- **Instruments table** (`scoretk_core::instruments`, generated,
  `.rodata.2837`): 644 templates, 41 371 bytes of string payload in the source.
  Every field is read by one of the two consumers (MuseScore recognition,
  MIDI import) — verified field by field. Exact duplicate literals are already
  merged by rustc (two identical `&'static str` in a test cdylib leave a single
  copy in the data section), so the remaining duplication is semantic
  (`track_name` vs `long_name` differing only by case or abbreviation) and
  cannot be folded without changing recognition. The compaction attempt
  documented in `docs/agents/performance.md` § 4 already measured the ceiling:
  **−13 ko raw but +189 bytes gzip**, because DEFLATE exploits the repeated
  strings better than an index does. Not applied.
- **Tables shared between crates** are already factored: instruments in
  `scoretk-core` (MuseScore + MIDI), `tpc`/clefs in `scoretk-lilypond-common`
  (both directions), pitches/keys/ornaments/lyrics in `scoretk-abc-common`.
  The remaining large tables (`.rodata.1419`, `.rodata.13`, `.rodata.2821`,
  `.rodata.2833`) belong to one format and one direction each; moving them
  would only add a crate boundary.
- **Panic and assertion messages** (`.rodata.14`, `.rodata.4243`, 47.5 ko
  together) come from `std`/`core` and from `miniz_oxide`'s internal `assert!`s.
  They cannot be removed without changing user-visible error messages or the
  dependency, and they compress to a few kilobytes. Not applied.
- **`core::fmt`** (31 983 bytes, 1.66 %) is dominated by `f64` `Display`
  (14 010 bytes) and `FromStr` (4 063 bytes), reached by the `.mscx` and ABC
  readers/writers that print and parse real-number values. Suppressing it would
  change numeric formatting: forbidden.


## Binary composition (twiggy) — historical measurement (a7f690c)

Measurement made on commit `a7f690c` (`mesure-twiggy` branch), the workspace's
`[profile.release]` profile (`opt-level = "z"`, `lto = true`,
`codegen-units = 1`), with `wasm-bindgen 0.2.130` and `wasm-opt -Oz`.

### Reproduction method

```sh
nix develop -c cargo build --release -p scoretk-wasm --target wasm32-unknown-unknown
WB=/tmp/scratchpad/build-twiggy
mkdir -p "$WB/esm"
nix develop -c wasm-bindgen --target web --out-dir "$WB/esm" --out-name scoretk_wasm \
  target/wasm32-unknown-unknown/release/scoretk_wasm.wasm
# avec symboles (pour twiggy) :
nix develop -c wasm-opt -Oz --debuginfo "$WB/esm/scoretk_wasm_bg.wasm" \
  -o "$WB/esm/scoretk_wasm_bg.debug.wasm"
# sans symboles (cohérence avec la taille livrée) :
nix develop -c wasm-opt -Oz --strip-debug "$WB/esm/scoretk_wasm_bg.wasm" \
  -o "$WB/esm/scoretk_wasm_bg.strip.wasm"
nix shell nixpkgs#twiggy -c twiggy top -n 60 "$WB/esm/scoretk_wasm_bg.debug.wasm"
nix shell nixpkgs#twiggy -c twiggy dominators -d 3 "$WB/esm/scoretk_wasm_bg.debug.wasm"
nix shell nixpkgs#twiggy -c twiggy monos "$WB/esm/scoretk_wasm_bg.debug.wasm"
nix shell nixpkgs#twiggy -c twiggy garbage "$WB/esm/scoretk_wasm_bg.debug.wasm"
```

Measurements obtained:

| Variant | Raw | Gzip |
| --- | --- | --- |
| with symbols (`--debuginfo`) | 2 155 434 | — |
| without symbols (`--strip-debug`) | 1 546 119 | 640 218 |
| documented reference (§ 2) | 1 552 892 | 641 472 |

The 0.4 % difference between the local measurement and the documented reference comes from
the toolchain (rustc 1.100.0 / wasm-bindgen 0.2.130 of the Nix shell); the
orders of magnitude are consistent. The name section (`"function names"`)
weighs 607 871 bytes (28.20 %) in the variant with symbols; it is
absent from the delivered binary.

### Aggregation by crate (shallow bytes, total 2 155 434)

Classification by symbol prefix (`crate[hash]::…`); implementations
of generic traits are attributed to the crate defining the
generic function (`core`, `alloc`). The `core::fmt` and `panics/messages`
buckets are isolated afterwards (see (a)).

| Bucket | Bytes | % |
| --- | ---: | ---: |
| `"function names"` (name section) | 607 871 | 28,20 |
| static data (`.rodata`) | 213 102 | 9,89 |
| `scoretk` (facade) | 196 636 | 9,12 |
| `core` (rust) | 194 787 | 9,04 |
| `alloc` | 181 775 | 8,43 |
| `scoretk_musicxml_import` | 102 234 | 4,74 |
| `scoretk_musicxml_export` | 96 798 | 4,49 |
| wasm-bindgen (shim + runtime) | 56 754 | 2,63 |
| `scoretk_midi_export` | 54 392 | 2,52 |
| `zip` | 47 092 | 2,18 |
| `scoretk_musescore_import` | 43 439 | 2,02 |
| `zopfli` | 42 464 | 1,97 |
| `scoretk_midi_import` | 42 450 | 1,97 |
| `scoretk_lilypond_import` | 40 825 | 1,89 |
| `scoretk_musescore_export` | 36 177 | 1,68 |
| `core::fmt` (isolated, see (a)) | 32 686 | 1,52 |
| `scoretk_core` | 27 331 | 1,27 |
| `miniz_oxide` | 19 525 | 0,91 |
| `quick_xml` | 17 956 | 0,83 |
| `scoretk_abc_import` | 15 101 | 0,70 |
| `scoretk_lilypond_export` | 12 306 | 0,57 |
| `compiler_builtins` | 10 264 | 0,48 |
| `flate2` | 9 192 | 0,43 |
| `dlmalloc` | 7 427 | 0,34 |
| `memchr` | 5 760 | 0,27 |
| `scoretk_abc_common` | 4 943 | 0,23 |
| `indexmap` | 3 182 | 0,15 |
| `std` | 3 138 | 0,15 |
| `hashbrown` | 2 066 | 0,10 |
| `wasm_bindgen` | 1 979 | 0,09 |
| `scoretk_abc_export` | 1 926 | 0,09 |
| `midly` | 1 827 | 0,08 |
| `scoretk_wasm` | 1 630 | 0,08 |
| `panics/messages` (isolated, see (a)) | 1 527 | 0,07 |
| other dependencies + wasm sections | ~3 600 | 0,17 |
| **total** | **2 155 434** | **100,00** |

### (a) `core::fmt` and panics/messages

- `core::fmt` : **32 686 bytes (1.52 %)** — `Debug`/`Display` implementations
  generated for model and error types (including
  `scoretk_core::*` types via `<… as core::fmt::Display>::fmt`). This is the
  formatting item, not the XML rendering engine.
- panics/messages : **1 527 bytes (0.07 %)** — `core::panicking` and associated
  messages. Marginal.

Isolated total: 34 213 bytes (1.59 %). Both are already marginalised by
`panic = "abort"` (measured: −20 raw bytes, discarded).

### (b) Static data (`.rodata`)

Total `.rodata`: **213 102 bytes (9.89 %)**, spread over 115 segments.
The largest:

| Segment | Bytes | % |
| --- | ---: | ---: |
| `.rodata.18` | 53 135 | 2,47 |
| `.rodata.54` | 20 692 | 0,96 |
| `.rodata.1` | 20 059 | 0,93 |
| `.rodata.8` | 9 198 | 0,43 |
| `.rodata.13` | 7 803 | 0,36 |
| `.rodata.82` | 7 737 | 0,36 |

Segment `.rodata.18` contains the strings of the instruments table
(`crates/scoretk-musescore-import/src/read/instruments.rs`, 4 934 lines,
generated file) — we find `piccolo`, `wind.flutes.flute.piccolo`, etc.
Estimate: the instruments table weighs ≈ **53 Ko (2.47 %)** of the binary.
The rest of `.rodata` is dominated by error messages and
mapping tables (clefs, key signatures, ABC decoration).

### (c) The 20 largest monomorphisations

`twiggy monos`: 297 instances, cumulative bloat ≈ 262 399 bytes (12.2 %)
if all redundant instances were merged. Top 20 instances:

```
 4285   0.20%  zopfli::deflate::add_lz77_block::<zip::write::MaybeEncrypted<Cursor<Vec<u8>>>> (×2)
 3382   0.16%  core::slice::sort::unstable::quicksort::<i32>
 3128   0.15%  core::slice::sort::unstable::quicksort::<usize>
 2708   0.13%  zopfli::squeeze::lz77_optimal::<ZopfliLongestMatchCache> (×2)
 2195   0.10%  core::slice::sort::stable::quicksort::<(i64, usize)> (midi)
 2036   0.09%  core::slice::sort::stable::quicksort::<Fraction> (midi tuplets)
 2031   0.09%  core::slice::sort::stable::quicksort::<(&str, &Element)> (musicxml)
 2025   0.09%  core::slice::sort::stable::quicksort::<Volta> (lilypond)
 1946   0.09%  core::slice::sort::stable::quicksort::<(TupletId, (usize, usize))>
 1940   0.09%  core::slice::sort::stable::quicksort::<Fraction> (PartialOrd)
 1878   0.09%  core::slice::sort::stable::quicksort::<VoltaSpan> (midi)
 1843   0.09%  core::slice::sort::stable::quicksort::<Event> (lilypond)
 1809   0.08%  core::slice::sort::stable::quicksort::<Half> (musescore)
 1739   0.08%  core::slice::sort::stable::quicksort::<Leaf> (zopfli)
 1523   0.07%  core::slice::sort::stable::drift::sort::<(i64, usize)>
 1500   0.07%  core::slice::sort::stable::drift::sort::<VoltaSpan>
 1461   0.07%  core::slice::sort::stable::drift::sort::<Volta>
 1371   0.06%  core::slice::sort::stable::drift::sort::<(&str, &Element)>
```

Levers: `-Z share-generics` (nightly) would deduplicate identical instances
across crates; realistic estimated gain 30–80 Ko raw (upper bound 262 Ko).

### (d) Small non-inlined scoretk functions (`#[inline]` candidates)

Selection: non-generic functions of a `scoretk_*` crate, shallow bytes between
8 and 96, sorted by increasing size (top 30 of 91). Verification
by `twiggy paths` on a sample (e.g. `scoretk_core::gap_rests::units_fraction`,
14 bytes) confirms they are called from another crate
(`scoretk_musicxml_import`), hence candidates for `#[inline]` at crate
boundaries.

```
 13  scoretk_abc_common            duration::unit
 13  scoretk_lilypond_common       pitch::valid
 13  scoretk_musescore_import      read::read4xx::notations::accidental_type
 13  scoretk_musescore_import      read::read302::tables::accidental_type
 14  scoretk_core                  gap_rests::units_fraction
 14  scoretk_midi_import           chord::min_allowed_duration
 14  scoretk_midi_import           timesigmap::fraction_from_ticks
 14  scoretk_musescore_import      read::read302::credits::page_break
 15  scoretk_musicxml_export       beams::stems_hooks
 15  scoretk_musicxml_export       stems::clef_pitch_offset
 16  scoretk_midi_import           simplify::simplify_durations_not_drums
 19  scoretk_musescore_import      read::read4xx::textlines::rich_text
 19  scoretk_musicxml_export       exportmusicxml::borrow_attributes
 23  scoretk_musicxml_export       text::write_words
 24  scoretk_musescore_import      read::read302::spanner::read_chord_spanner
 24  scoretk_musicxml_export       repeats::sound
 25  scoretk_musescore_import      read::notation_names::line_style
 25  scoretk_musescore_import      read::read302::tables::stem_direction
 32  scoretk_midi_import           key::find_key::{closure#0}
 34  scoretk_musicxml_export       pitch::tpc_step_name
 34  scoretk_musicxml_export       harmony::is_separator
 37  scoretk_musescore_export      names::duration_type
 37  scoretk_musescore_export      names::clef_type
 37  scoretk_musescore_export      write::twrite::stem_name
 37  scoretk_musicxml_export       repeats::barline_style
 38  scoretk_musicxml_export       exportmusicxml::contains_accidental
 38  scoretk_musicxml_export       exportmusicxml::accidental_name
 39  scoretk_lilypond_export       annotations::text_event
 39  scoretk_midi_import           simplify::minimize_number_of_rests::{closure#1}
 40  scoretk_lilypond_import       builder::lyrics_build
```

Individual gain low (≤ 96 bytes), but these functions remain
non-inlined inter-crate calls with fat LTO at crate boundaries: the real
lever is crate merging (see levers), not the annotation alone.

### Dominators and garbage

`twiggy dominators -d 3`: the root is the `convert` export (and
`convertWithTimeline`), which dominates `scoretk::read` (194 175 bytes, 9.01 %),
then the readers per format. No abnormal domination cycle.

`twiggy garbage`: **282 bytes** of dead types (unused wasm types);
the 168 550 bytes reported in data segments are false positives
(segments referenced via indices computed at runtime).

### Quantified levers (not applied)

| Lever | Estimated raw gain | Estimated gzip gain | Note |
| --- | ---: | ---: | --- |
| Merge `scoretk-musescore-import` + `scoretk-musicxml-export` into `scoretk-core` (restore pre-step-1 state) | 58 427 | 15 146 | measured (§ "Effect of the crate split") |
| Make the instruments table (`instruments.rs`) optional / loaded on demand | ~53 000 | ~22 000 | estimated from `.rodata.18` |
| Replace `zopfli` with `flate2` deflate for `.mscz` writing | ~33 000 | ~13 500 | estimated (42 464 − flate2 already present) |
| Limit `core::fmt` (`Debug`/`Display`) on internal types | 32 686 | ~13 500 | measured |
| `-Z share-generics` (nightly) to deduplicate monomorphisations | 30 000–80 000 | 12 000–33 000 | upper bound 262 399; requires nightly |
| Replace `dlmalloc` with `wee_alloc` | ~7 000 | ~3 000 | estimated |
| Eliminate redundant multivalue wasm-bindgen exports | ~10 000 | ~4 000 | estimated (shim 49 489) |

Gzip gains are estimated with the global ratio 640 218 / 1 546 119 ≈ 0.41.
None of these levers is applied: the package stays under the
Cantus limit (≈ 2.5 Mo raw) and the individual gains stay below the 2 % threshold
of the wasm contract, except crate merging (3.8 %) and `-Z share-generics`.


The build is in two steps:

1. `wasm32Build`: `craneLib.cargoBuild` with
   `CARGO_BUILD_TARGET = "wasm32-unknown-unknown"`, on the already compiled
   dependency artefacts (`wasmCargoArtifacts`); only the `.wasm` of the cdylib
   `scoretk_wasm` is copied to the output.
2. `wasm`: `runCommand` calling `wasm-bindgen` twice (targets `web` and
   `no-modules`), passing each `.wasm` through `wasm-opt`, installing the wrapper
   `classic/scoretk.js`, a `package.json` generated from `version`, and writing
   `SIZE`.

No derivation accesses the network: crane vendors the crates from
`Cargo.lock`, and `--locked` forbids any change to the lock. Adding a
workspace member invalidates `buildDepsOnly` for wasm32: the first
build after such a change rebuilds the dependencies, then
the Nix cache takes over.

**wasm-bindgen version**: the crate is pinned `=0.2.130`, equal to the
`wasm-bindgen-cli` of the Nix shell. When updating the flake, change
the pin in `crates/scoretk-wasm/Cargo.toml`, otherwise the CLI refuses the
`.wasm` (« schema version mismatch »).

## 3. API

The produced `.d.ts` contains the custom TypeScript section: all
functions are marked `skip_typescript` on the Rust side, so that `detect` comes out
as `string | null`, `formats` as `FormatInfo[]` and `convertWithTimeline` as
`PlaybackResult`.

| Function | Signature | Notes |
| --- | --- | --- |
| `convert` | `(input: Uint8Array, options?: ConvertOptions) => Uint8Array` | bytes in both directions, all formats |
| `convertWithTimeline` | `(input: Uint8Array, options?: ConvertOptions) => PlaybackResult` | identified MusicXML + MIDI + time map; `to` ignored |
| `convertDetailed` | `(input: Uint8Array, options?: ConvertOptions) => DetailedResult` | additive: same bytes as `convert` plus `{ bytes, losses }` |
| `detect` | `(input: Uint8Array) => string \| null` | `Format::detect` (content) |
| `formats` | `() => FormatInfo[]` | seven entries, actual capabilities |
| `version` | `() => string` | library version |

```ts
interface ConvertOptions {
  from?: string;        // mscz, mscx, musicxml, xml, mxl, ly, lilypond, abc, mid, midi
  to?: string;          // défaut : "musicxml"
  encodingDate?: string; // <encoding-date> (AAAA-MM-JJ)
  software?: string;    // <software> (MusicXML)
  concertPitch?: boolean; // hauteurs en sons réels (MusicXML)
  expandRepeats?: boolean; // dérouler les reprises MIDI (défaut : true)
  includes?: Record<string, Uint8Array>; // folder files by relative path, for LilyPond \include resolution
}

interface FormatInfo {
  name: string;
  extensions: string[];
  read: boolean;
  write: boolean;
}

interface TimelineTempo { tick: number; timeUs: number; microsPerQuarter: number }
interface TimelineMeasure { measure: number; ordinal: number; segment: number; passage: number; xmlId: string; tick: number; endTick: number; timeUs: number; endTimeUs: number; timeMs: number }
interface TimelineNote { tick: number; timeUs: number; durationTicks: number; measure: number; staff: number; voice: number; chord: number; note: number; pitch: number; passage: number; grace: boolean; tremolo: boolean; xmlId: string }
interface TimelinePause { tick: number; endTick: number; timeUs: number; endTimeUs: number; seconds: number }
interface Timeline { division: number; ticks: number; tempo: TimelineTempo[]; measures: TimelineMeasure[]; notes: TimelineNote[]; pauses: TimelinePause[] }
interface PlaybackResult { musicxml: Uint8Array; midi: Uint8Array; timeline: Timeline }
```

`convertWithTimeline` resolves `from` like `convert` (content detection
if absent) and ignores `to`. The returned MusicXML is exported with stable
identifiers: every `<measure>` and every `<note>` carries `id` **and**
`xml:id` (same value), which are exactly the `xmlId`s of `timeline`.
`timeline.measures[].ordinal` is the 0-based index of the measure in the
full score (equal to `measure`): it is the only anchoring key to
use, `number` not being exposed. `timeline.notes` is sorted by increasing `tick`
(then by a stable key), on the Rust side. The MIDI and the map come from the
same rendering; `expandRepeats: false` produces a "flat" MIDI
and fewer measures in `timeline.measures`.

The Rust type remains `Timemap` (crate `scoretk-midi-export`, re-exported by `scoretk`):
the field-by-field mapping from Rust `snake_case` to JS `camelCase` is
documented in `docs/agents/timemap.md` § 6. `MeasureSync::number`,
`pause_before`, `grace_index` and `grace_before` are not exposed on the JS side.

A missing `from` is detected by content (`Format::detect`); an unrecognised input
raises an explicit error rather than guessing. A missing `to` defaults to
`musicxml`: `mscz → musicxml` remains the zero-option call.

`includes` is additive and only affects LilyPond input (with `from: "ly"` or
`"lilypond"`, or content detection). It maps folder-relative paths to the raw
bytes of the other files of the folder, e.g.
`{ "noaten.ily": new TextEncoder().encode(text), "sub/x.ily": bytes }`. The
wasm wrapper builds an in-memory resolver for
`scoretk::read_lilypond_with_includes`: an `\include` is looked up relative to
the including file's directory first, then at the root (LilyPond's include
search order, `lily/includable-lexer.cc` / `lily/sources.cc`), with `.` and `..`
normalised. An unknown include keeps the previous behaviour: a warning, no
failure. Other formats ignore the option; without it, `convert` and
`convertWithTimeline` behave exactly as before. The classic wrapper forwards
the options object unchanged.

## 4. Errors

All errors become a JavaScript `Error` whose message is that
of the Rust library:

- unrecognised input format from content (when `from` is absent) ;
- unknown `from` or `to` (`Error::FormatInconnu`, message listing the
  accepted names) ;
- format recognised but unsupported in the requested direction
  (`Error::FormatNonPrisEnCharge`) — impossible in this package, which enables
  all features, but the guard remains ;
- conversion errors: `Error::Core`, `MusicXmlImport`, `Midi`,
  `MidiImport`, `LilyPond`, `Mscx`, `Abc`, `Zip`.

No `console_error_panic_hook` is installed: a Rust panic appears
as a `RuntimeError` (« unreachable »), which should only happen on an
internal bug.

## 5. ESM and classic usage

```js
import init, { convert, detect, formats, version } from 'scoretk';

await init();
const midi = convert(bytes, { to: 'midi' });            // Uint8Array
const xml = new TextDecoder().decode(convert(bytes));   // musicxml par défaut
```

```js
// Worker classique
importScripts('scoretk/classic/scoretk.js');
await self.scoretk.ready;
const midi = self.scoretk.convert(bytes, { to: 'midi' });
```

`self.scoretk` is `{ ready, convert, version, detect, formats, baseUrl }` ;
in case of an incorrect base URL, set `self.SCORETK_BASE_URL` before
`importScripts`. The wrapper loads `scoretk_wasm.js` and
`scoretk_wasm_bg.wasm`.

The `.wasm` must be served as `application/wasm` so that the glue uses
`WebAssembly.instantiateStreaming` (otherwise it falls back to an `arrayBuffer`,
correct but slower).

## 6. Tests

`crates/scoretk-wasm/tests/node-smoke.mjs` loads the ESM variant of the package
as produced by `nix build` and covers the following cases (numbered 1 to 25
in the script):

- `version()` equals `package.json` ;
- `formats()`: seven entries, `read`/`write` `true`, MusicXML extensions ;
- `detect`: `.mscx`, `.musicxml`, `.abc`, `.ly`, unknown content (`null`) ;
- `convert`: default MusicXML with `encodingDate`/`software`, `.mscx` → `.mscx`,
  `.mscx` → `.mscz` read back then → MIDI, MusicXML → LilyPond/ABC/MIDI,
  ABC → MusicXML, LilyPond → MusicXML ;
- errors: `to: 'pdf'`, `from: 'pdf'`, unknown input without `from` ;
- `convertWithTimeline` on `v4/repeat65.mscx`: presence of `musicxml`,
  `midi` and `timeline`, `id` **and** `xml:id` attributes in the MusicXML, MIDI
  `MThd`, non-empty measures with first tick/time at zero, `ordinal` equal to
  `measure`, strictly increasing ticks, at least one replayed measure,
  `expandRepeats: false` shorter, and every note `xmlId` present in the
  MusicXML;
- `includes` on the CC0 `tests/corpus/ly/alde-friezen/` lot: `partituer.ly`
  without the option still yields no notes, with `{ "noaten.ily": bytes }` it
  yields notes whose MIDI pitch sequence is exactly that of the LilyPond
  `partituer.ref.mid`, `fideo.ly` yields notes despite its unknown
  `ly2video-events.ily` include, `convertWithTimeline` sees the included notes,
  and a nested include under `sub/` resolves relative to the including file.

Environment variables: `SCORETK_PKG_DIR` (default `result/`) and
`SCORETK_CORPUS` (default `tests/corpus/`). The flake check
`checks.wasm-node-smoke` runs this script on the Nix package.

```sh
nix build .#wasm
node crates/scoretk-wasm/tests/node-smoke.mjs
SCORETK_PKG_DIR=$PWD/result node crates/scoretk-wasm/tests/node-smoke.mjs
```

### Byte identity between two builds

`scripts/check-wasm-identity.mjs` compares two package directories in the same
Node process and fails on the first differing byte. It loads both ESM variants
(`initSync` with a precompiled module), then on a corpus sample:

- compares `detect`, `formats()` and `version()` ;
- converts each sample to every writable format (`.mscx`, `.mscz`,
  `musicxml`, `.mxl`, `midi`, `.ly`, `.abc`), with and without an explicit
  `from`, and compares the output bytes;
- reads `.mscz` and `.mxl` containers built once with the old package;
- compares `convertWithTimeline` output (MusicXML bytes, MIDI bytes and the
  whole time map, with and without `expandRepeats`);
- compares the three user-visible error messages.

Samples cover both MuseScore versions, MusicXML, MIDI, LilyPond, ABC and
repeats; the ten import/export directions are all exercised (the script fails
if one is missing).

```sh
cp -rL result /tmp/scoretk-old
# ... modify the build ...
nix build .#wasm
node scripts/check-wasm-identity.mjs /tmp/scoretk-old result
```


## 7. Licence and sources

The package is distributed under **GPL-3.0-only**, like the library. The
corresponding source within the meaning of GPL §6 is the repository
<https://github.com/jperon/scoretk> (the `repository` field of `package.json`).
The package embeds code ported from MuseScore Studio, Copyright (C) MuseScore
Limited and others, rewritten in Rust in 2026, and code generated by
wasm-bindgen. « MuseScore » is a trademark of MuseScore Limited; the package
is neither affiliated with nor endorsed by it.

## 8. Examples

`examples/web/` contains two minimal pages (chosen or dropped file,
MusicXML displayed in `<pre>`, download, no network dependency):

```sh
nix build .#wasm
python3 -m http.server 8000        # à la racine du dépôt
# http://localhost:8000/examples/web/index.html        (variante esm)
# http://localhost:8000/examples/web/worker-demo.html  (variante classic en Worker)
```

The demo worker loads the package via
`importScripts('../../result/classic/scoretk.js')`.

## 9. Execution times (Node)

### Method

Measured tree: commit `a7f690cf0817fd3f73a3657631e04a231cfc318a` (`mesure-nodebench`
branch), before the addition of the measurement script and this section;
no conversion code (`crates/*/src`) was modified. Package built by
`nix build .#wasm` (`./result`, binary of 1 549 059 bytes / 640 274 gzip,
`cat result/SIZE`), native CLI by
`nix develop -c cargo build --release -p scoretk-cli`
(`./target/release/scoretk`, identical `release` profile: `opt-level = "z"`,
`lto`, `codegen-units = 1`).

- Wasm: `nix shell nixpkgs#nodejs -c node crates/scoretk-wasm/tests/node-bench.mjs ./result`
  (and its `--json` variant, three runs). The script loads the ESM variant
  via `initSync` with a precompiled `WebAssembly.Module`, iterates over the 56
  `.mscx` of `tests/corpus/v3` and `tests/corpus/v4`, and records for each,
  after 3 warm-up iterations, the median of 15 iterations of
  `convert(bytes,{to:"musicxml"})`, `convert(bytes,{to:"midi"})` and
  `convertWithTimeline(bytes)`.
- Native: the same set of 56 files, one CLI invocation per file and
  per direction (`scoretk "$f" --to musicxml|midi -o /dev/null`), 46 timed runs
  per direction (`date +%s%N`); the CLI does not expose
  `convertWithTimeline`, so the "playback" comparison adds its two
  directions. `time` over a complete run gave 0.144 s (MusicXML) and 0.307 s
  (MIDI). The start-up cost of the 56 processes was measured separately: median
  123 ms over 15 runs of 56 `scoretk --list-formats`; it is **included**
  in the native measurements.
- Machine: Intel(R) Processor U300, 6 cores / 6 threads, 4.4 GHz max, 15 GiB
  of RAM, Linux 6.18.60 (NixOS) x86_64, Node v24.22.1 (`lscpu`, `free -h`,
  `node --version`). Average load of 4.6 to 9.0 during the measurements (`uptime`)
  : another session was running a concurrent benchmark, hence significant
  noise. The figures below are the median of three wasm runs
  and 46 native runs; the observed minimum is also given.

### Results

Instantiation (once, outside the per-file measurements, `--json`):
module compilation 5.5 to 10.7 ms, `initSync`/`ready` 0.75 to 4.64 ms,
wasm linear memory 6 160 384 bytes (5.9 Mio). `process.memoryUsage()`:
RSS 58.2 Mo before loading → 63.7 Mo after `initSync` → about 180 Mo at the end
of the campaign; `arrayBuffers` 0.14 → 1.69 → 8.91 Mo.

Five largest corpus files (medians in ms, sum of medians per file):

| File | Bytes | MusicXML | MIDI | Timeline | Total |
| --- | ---: | ---: | ---: | ---: | ---: |
| `v3/moonlight.mscx` | 409 776 | 74,317 | 22,547 | 84,613 | 181,477 |
| `v4/voiceswitching.mscx` | 30 868 | 5,838 | 3,005 | 6,780 | 15,623 |
| `v4/testExcludeInvisibleElements.mscx` | 29 297 | 4,430 | 2,882 | 6,714 | 14,026 |
| `v3/layout_elements.mscx` | 24 855 | 3,501 | 1,569 | 4,933 | 10,003 |
| `v3/voices.mscx` | 24 571 | 2,895 | 1,367 | 4,383 | 8,645 |
| **Total (56 files)** | | **145,568** | **64,740** | **204,010** | **414,318** |

### Wasm/native ratio

| Chain | Wasm (ms) | Native CLI (ms) | Wasm/native ratio |
| --- | ---: | ---: | ---: |
| MusicXML | 145,6 | 277 | 0,53 |
| MIDI | 64,7 | 251 | 0,26 |
| Playback (MusicXML + MIDI) | 204,0 (`timeline`) | 528 | 0,39 |

The native is penalised by 56 process start-ups (123 ms measured). After
subtracting them: 154 ms / 128 ms / 282 ms on the native side, i.e. ratios of 0.95 /
0.51 / 0.72. On the least loaded runs, wasm drops to 128.7 /
55.6 / 183.1 ms versus 138 / 108 / 246 ms on the native side, i.e. 0.93 / 0.51 / 0.74.
The order of magnitude is therefore a wasm equivalent to slightly faster in
MusicXML, about twice as fast in MIDI, and machine noise
prevents precision better than a few tens of percent. The
"playback" ratio remains approximate: the time map has no equivalent in the
CLI, only the MusicXML + MIDI cost.

### Slowest files

`v3/moonlight.mscx` dominates by a wide margin (181.5 ms, about 44 % of the total
wasm, i.e. 74.3 ms MusicXML, 22.5 ms MIDI and 84.6 ms timeline): with 410 Ko,
it is fourteen times larger than the next one. Followed by
`v4/voiceswitching.mscx` (15.6 ms), `v4/testExcludeInvisibleElements.mscx`
(14.0 ms), `v4/timesig-09.mscx` (12.7 ms, only 9.7 Ko but an expensive
timeline) and `v3/layout_elements.mscx` (10.0 ms).

No optimisation lead has been applied: this section only records
measurements.
