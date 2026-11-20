# AGENTS.md — core instructions

This file is read on **every request**: it contains only the core. Details live
in `docs/agents/` (index at the end of the file).

## The project

**scoretk** (GPL-3.0-only) is a score conversion toolkit. A Rust port of the
converters of **MuseScore Studio**: a generic core, reading `.mscz`/`.mscx`,
`partwise` MusicXML export, then MuseScore, MusicXML, LilyPond, ABC and MIDI
(see `docs/TODO.md`).

Map of crates and modules (17 members: one core, one facade, its CLI and its
wasm package, plus the formats split by direction):

```
crates/
  scoretk-core              pivot model, no I/O
  scoretk-musescore-import  .mscz/.mscx → model (3.x and 4.x/5.x)
  scoretk-musescore-export  model → .mscx / .mscz
  scoretk-musicxml-import   .musicxml/.xml/.mxl → model
  scoretk-musicxml-export   model → partwise MusicXML / .mxl
  scoretk-midi-common       shared error, nothing more
  scoretk-midi-import       MIDI → model
  scoretk-midi-export       model → MIDI (repeats, time map)
  scoretk-lilypond-common   error, tpc, clefs, shared by both directions
  scoretk-lilypond-import   LilyPond subset → model
  scoretk-lilypond-export   model → .ly
  scoretk-abc-common        ABC tables and helpers shared by both directions
  scoretk-abc-import        .abc → model
  scoretk-abc-export        model → .abc
  scoretk                   facade: Format, read/write/convert, hooks
  scoretk-cli               `scoretk` binary (disk I/O)
  scoretk-wasm              wasm-bindgen wrapper
```

- `crates/scoretk-core` — portable, generic core, with no I/O and no format:
  `model/` (in-memory model), `error.rs`, `gap_rests.rs` and
  `model/stable_ids.rs` (element addressing).
- `crates/scoretk-musescore-import` — MuseScore reading: `container.rs` (ZIP
  `.mscz` → `.mscx`, version), `read/read206` (1.15–2.07), `read/read302`
  (3.x) and `read/read4xx` (4.x/5.x) → model.
- `crates/scoretk-musescore-export` — MuseScore writer: `write_mscx`
  (4.x XML) and `write_mscz` (ZIP), ported from `engraving/rw/write/`.
- `crates/scoretk-musicxml-import` — MusicXML reader (`read_musicxml`) into the
  model: `.musicxml`/`.xml`/`.mxl` → `Score`.
- `crates/scoretk-musicxml-export` — MusicXML export: model → `partwise` /
  `.mxl`, with `ExportOptions` and the `DEFAULT_*` constants.
- `crates/scoretk-midi-common` — error and result shared by both
  directions, no I/O; nothing else (SMF reading is in `-import`).
- `crates/scoretk-midi-import` — MIDI reading (`read_midi`, ported from
  MuseScore's import): `midifile.rs` (SMF format, via `midly`), quantisation,
  voices, `timesigmap`, `tuplet`…, no I/O.
- `crates/scoretk-midi-export` — MIDI writing (SMF format 1) with repeat
  unfolding (`repeat_list.rs`) and a time map (`timemap.rs`), no I/O.
- `crates/scoretk-lilypond-common` — what LilyPond reading and writing
  share: error, circle of fifths (`tpc`), clef table. No conversion logic.
- `crates/scoretk-lilypond-import` — reading a subset of LilyPond
  by porting `lily/parser.yy`/`lexer.ll` (t40), no I/O.
- `crates/scoretk-lilypond-export` — LilyPond writing (`.ly`) from the
  model (t36, voices in `\fixed c'`), no I/O.
- `crates/scoretk-abc-common` — ABC tables and helpers shared by both
  directions (pitches, key signatures, clefs, durations, ornaments, lyrics); no I/O.
- `crates/scoretk-abc-import` — ABC reading (`.abc`) into the model, standard 2.1
  and the behaviour of `abc2xml.py`, no I/O (t41).
- `crates/scoretk-abc-export` — ABC writing (`.abc`) from the model,
  the behaviour of `xml2abc.py`, no I/O (t41).
- `crates/scoretk` — multi-format facade: `Format`, detection by extension
  or content, `read`/`write`/`convert` into the pivot model; ten symmetric
  `<format>-import` / `<format>-export` features (`musescore`, `musicxml`,
  `midi`, `lilypond`, `abc`), all enabled by default, wired up in
  `src/hooks.rs` and with no mixed alias.
- `crates/scoretk-cli` — `scoretk` binary (`input -o output`, `--from`,
  `--to`, `-` for stdin/stdout): disk I/O for the facade.
- `crates/scoretk-wasm` — wasm-bindgen wrapper of the facade (`convert`,
  `detect`, `formats`, `version`); the only wasm package in the repository
  (`nix build .#wasm`).
- `tests/corpus/{v3,v4}/` — upstream `.mscx` + `*.ref.musicxml` references;
  `tests/corpus/midi/` — `*.ref.mid` references; `crates/scoretk/tests/`
  (round trips, compliance e2e tests and facade wiring); tests
  per direction live in that direction's crate (`crates/scoretk-*-import/tests/`,
  `crates/scoretk-*-export/tests/`).
- `scripts/` — corpus references, instruments table, licences;
  `flake.nix` — shell, checks, wasm package.

## Invariants

1. **Portable library.** No `std::fs`/`env`/`net`/`process`, no
   threads, no clock, no C dependency in `crates/scoretk-core`: it must
   compile for `wasm32-unknown-unknown` (without WASI), Android and iOS.
   Inputs/outputs are in-memory bytes. The CLI and tests, however, may
   use the disk.
2. **Fidelity to MuseScore 4.7.4.** Compliance is measured against the
   `*.ref.musicxml` references produced by MuseScore 4.7.4. The ported C++
   source is a 5.0.0 development upstream snapshot: in case of divergence,
   the 4.7.4 behaviour prevails (unless there is an explicit decision to change
   the reference, see `docs/agents/upstream.md`). Reproducible output: no
   clock, optional encoding date with a default constant.
3. **New code in new files, by family** (3.x reader,
   4.x reader, model, export). An existing file receives only the
   wiring (`mod.rs`) or a localised fix. Faithful port: follow the
   names and breakdowns of the C++ (`TRead::read(Chord*)`, `write_measure`,
   `calc_divisions`), with the upstream reference in a comment; do not code from
   memory.
4. **GPL-3.0-only licence.** Every new or modified source file carries
   the SPDX identifier, the copyright, the GPL-3.0-only text and, if it is ported,
   the modification notice « Portage de `<chemin C++>` …, modifié (réécrit en
   Rust) en 2026 ». Every new upstream source or datum is declared in
   `NOTICE`; every new third-party crate in `docs/licences-tierces.md`
   (checked by `scripts/check-licences.py`). Header template:
   `docs/agents/developpement.md`.
5. **Stable wasm contract for cantus.** Do not change without coordination:
   `package.json` package `name: "scoretk"` ; `esm/scoretk_wasm.js` +
   `esm/scoretk_wasm_bg.wasm` + `esm/scoretk_wasm.d.ts` ;
   `classic/scoretk.js` exposing
   `self.scoretk = { ready, convert, convertWithTimeline, version, detect,
   formats, baseUrl }` ;
   `convert(Uint8Array, { from?, to?, encodingDate?, software?, concertPitch?,
   expandRepeats? }) → Uint8Array` (`to` defaults to `"musicxml"`) ;
   `detect(bytes) → string | null` ; `formats() → FormatInfo[]` ;
   `version() → string` ; `convertWithTimeline(bytes, options?) →
   { musicxml, midi, timeline }` (additive: identified MusicXML and time map,
   see `docs/agents/timemap.md`; `timeline.measures[].ordinal` is
   the 0-based measure index and the only anchoring key, `timeline.notes`
   is sorted by increasing `tick` then by a stable key). `convert` returns
   `Uint8Array`s that the consumer decodes with `TextDecoder`
   (see `docs/agents/wasm.md`).
6. **No distributed upstream C++.** The port is a rewrite; the
   reference copy `ref-musescore/` is read-only and not versioned.

## Commands

```sh
nix develop -c cargo test --workspace
nix develop -c cargo clippy --workspace --all-targets -- -D warnings
nix develop -c cargo fmt --all -- --check
nix develop -c cargo build -p scoretk-core --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-midi-common -p scoretk-midi-import -p scoretk-midi-export --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-lilypond-common -p scoretk-lilypond-import -p scoretk-lilypond-export --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-abc-common -p scoretk-abc-import -p scoretk-abc-export --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-musescore-export --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-musicxml-import --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk --target wasm32-unknown-unknown
nix flake check
nix build .#wasm
nix develop -c cargo build -p scoretk-wasm --target wasm32-unknown-unknown

# Compliance, two levels (musical core, then full parity)
nix develop -c cargo test -p scoretk --test e2e_corpus -- --nocapture
```

Derived data is regenerated with `scripts/gen-references.sh` and
`scripts/gen-instruments-table.py`; the wasm package is tested with
`crates/scoretk-wasm/tests/node-smoke.mjs` (after `nix build .#wasm`).

## Pitfalls that fail silently

- **Outside `nix develop -c`, the `cargo` on PATH is a conflicting rustup shim**
  (`rust-std`): always go through the Nix shell.
- **Two e2e levels, and only the core is blocking for
  `EXPECTED_CONFORMANT`**: an unlisted file can regress silently;
  a layout difference is not a musical discrepancy, do not
  "fix" the export for a position attribute.
- **Do not regenerate the references with another version of MuseScore** without
  an explicit decision: the compliance target changes silently
  (`NOTICE`, `SOURCES.md`, `DEFAULT_SOFTWARE`, updated discrepancy documents).
- **`wasm-bindgen` is pinned to `=0.2.130`**, equal to the flake's CLI; otherwise
  « schema version mismatch ».
- **The corpus must stay in the source of the Nix checks** (the
  `tests/corpus` filter in `flake.nix`), otherwise the `tests` check fails.
- **Rule for `-common` crates (factorise intelligently):** what serves
  all formats goes into a common crate (e.g. shared XML utilities),
  what serves both directions of a format goes into its `-common`, what serves
  only one direction stays in its own file; never a single-direction module
  in a `-common`, nor format logic in a common crate
  (full rule in `docs/agents/developpement.md`).
- **Generated files**: `read/instruments.rs` and
  `docs/agents/ecarts-complets.md` are regenerated, never hand-edited.
- **Order of `<direction>`, spanners and `<credit>` is significant** for
  parity: do not sort a collection without checking the C++ and the reference.
- **`ref-musescore/` is absent from a fresh checkout**: recreate it with
  `docs/agents/upstream.md` before reading the C++.
- **Format reading**: implicit tick, `actualTicks()` ≠ `ticks()`, order
  `<dots>` before `<durationType>`, positional voices, absolute `<tick>` vs
  relative `<location>`, 3.x/4.x ties in two halves vs EID-based `<Tie>` in
  5.0, 5.0 spanners outside measures, first `.mscx` `rootfile` (not `.mss`).
  Details: `docs/agents/mscx-format.md` § 17.

## Index `docs/agents/`

One line per document; read the one matching the subject before coding.

- `docs/agents/upstream.md` — ported version, `ref-musescore` clone, C++ → Rust
  table, upstream tracking: read before any porting or upgrade.
- `docs/agents/developpement.md` — conventions, licence headers,
  portability, wasm contract, tooling: read before writing code.
- `docs/agents/mscx-format.md` — `.mscx` 3.x/4.x/5.x format reference and
  reading pitfalls: read before touching a reader.
- `docs/agents/ecarts-noyau.md` — musical-core compliance discrepancies, file by
  file and their causes: read to choose the next fix.
- `docs/agents/ecarts-complets.md` — **generated** report of full parity:
  read to measure, not to decide a fix on its own.
- `docs/agents/wasm.md` — the single wasm package (scoretk facade): build,
  sizes, API (convert/detect/formats/version), options, ESM/classic usage,
  Node tests: read before touching the
  `scoretk-wasm` crate.
- `docs/agents/midi.md` — MIDI reading and writing, `RepeatList`, rests,
  velocities, references and discrepancies: read before touching the
  `scoretk-midi-*` crates.
- `docs/agents/timemap.md` — time map (`Timemap`): Rust and
  JS structures, stable MusicXML identifiers, repeat passages, rests, client-side
  usage and limits: read before touching `write_playback` or
  `convertWithTimeline`.
- `docs/agents/scoretk.md` — multi-format facade: API, detection, wiring points
  t34/t35/t36/t38, CLI and tests: read before touching `scoretk`.
- `docs/agents/licence.md` — GPL-3.0-only obligations of a consumer:
  read before any distribution or integration.
- `docs/agents/musicxml-export.md` — MusicXML export reliability campaign,
  file-by-file gains and remaining causes: read before touching the writer.
- `docs/agents/musicxml-import.md` — MusicXML import port, result
  of the round-trip test and remaining discrepancies: read before touching the
  MusicXML reader.
- `docs/agents/lilypond.md` — LilyPond: writing (model →
  `.ly` mapping, `\fixed` mode, snapshots), reading (subset of
  `parser.yy`/`lexer.ll`, measured round trip), limits and upgrade of the
  sources: read before touching the `scoretk-lilypond-*` crates.
- `docs/agents/abc.md` — ABC reading and writing: split into three crates,
  subset, voices, snapshots, measured round trip, upgrade of
  `xml2abc`/`abc2xml`: read before touching the `scoretk-abc-*` crates.
- `docs/agents/mscx-write.md` — `.mscx`/`.mscz` writer
  (`scoretk-musescore-export`):
  round-trip method, measured results and limits: read before
  touching MuseScore writing.
- `docs/agents/performance.md` — native criterion benchmark (`benches/conversion.rs`),
  times per direction on the corpus, `perf`/Callgrind profiling, allocations and
  quantified leads: read before optimising a conversion.
- `docs/agents/plan-fiabilite.md` — reliability plan for the ten directions:
  reference batches, oracles, phases: read before a measurement campaign.
- docs/agents/format-mapping/ — format ↔ model tables per format and model
  gaps: read before extending a reader or a writer.
- docs/agents/format-ref/ — synthetic syntax references (in English) for
  agents, one per format, with section numbers and pitfalls: read before writing
  or reviewing a reader/writer.
