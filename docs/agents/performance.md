# Performance: native benchmark and profile of the full chain

This document records the measurement campaign of the `scoretk` facade — times per
direction and per chain, hot spots, allocation cost — then the
optimisations derived from it and their effect. The sections describing
the measurements (§ 3, § 5, § 6) apply to the state **before** optimisation; § 4
lists the changes and the measured effect of each. The project core (rules,
commands, index) remains in `AGENTS.md`.

All the values below come from commands actually run on
this machine (§ 2); each measurement is dated and accompanied by the conditions of
its execution. No figure is extrapolated from another machine.

## 1. Method

### 1.1 Criterion benchmark

`crates/scoretk/benches/conversion.rs` (criterion 0.8.2 as a development
dependency, `[[bench]] harness = false`, licences in
`docs/licences-tierces.md`) measures eleven groups on six corpus files:

- the two largest `.mscx` in bytes (`v3/moonlight`, 409 776 o ;
  `v4/voiceswitching`, 30 868 o) ;
- four files covering both versions of the format, several staves,
  several voices, repeats and layout
  (`v4/testExcludeInvisibleElements`, `v3/layout_elements`, `v3/voices`,
  `v4/hairpins_and_repeats`).

The eleven groups are: `lecture_mscx`, `lecture_musicxml`, `ecriture_musicxml`,
`ecriture_midi`, `ecriture_mscx`, `ecriture_mscz`, `ecriture_mxl`,
`ecriture_lilypond`, `ecriture_abc`, `chaine_mscx_musicxml` and `playback`
(equivalent of `convertWithTimeline`).

Bytes and models are loaded **outside the measurement** (`OnceLock`, once
per process); only the facade functions are timed, and
dropping the output is included in the measurement (criterion's `iter` semantics).
Each group declares its throughput in **input** bytes (`.mscx`,
or the MusicXML reference for `lecture_musicxml`): throughput therefore reads as
"source score bytes processed per second", not "bytes produced".

```sh
nix develop -c cargo bench -p scoretk --bench conversion
```

Settings: 20 samples, 1 s warm-up, 3 s measurement per data point,
**median** retained, throughput in Mo (10⁶ bytes) per second. At the time of
the campaign the `bench` profile inherited from `[profile.release]`
(`opt-level = "z"`, `lto = true`, `codegen-units = 1`): the figures apply to
the delivered binaries, not to a `debug` build. Since 2026-11-27 the host
`release` profile is `lto = "thin"`, `codegen-units = 16`, `opt-level = 3`
(which `bench` now inherits), while the size settings remain in
`[profile.wasm]`; these figures are historical, not a new baseline.

Run retained for the table: 2026-11-18, 11:55–12:00, `loadavg`
4.3 → 7.8 on 6 logical processors, without core pinning.

### 1.2 Profiling

The profile covers the full `.mscx` → MusicXML chain of the largest file
(`chaine_mscx_musicxml/v3/moonlight`), with the benchmark binary compiled with
`CARGO_PROFILE_BENCH_DEBUG=2` and run in criterion profiling mode
(`--bench --profile-time 15`, or `30` under Valgrind).

- **Real time, self**: perf 7.3.1,
  `perf record -e task-clock -F 999 -- <banc> --bench --profile-time 15 "chaine_mscx_musicxml/v3/moonlight"`
  → 16 699 samples. Call graphs are **unusable here**: with
  `--call-graph fp` or `dwarf`, perf fails to open the event
  (`kernel.perf_event_paranoid = 2` → `Failure to open any events`), hence
  the absence of an inclusive column in this profile.
- **Inclusive**: Valgrind 3.26 / Callgrind,
  `valgrind --tool=callgrind --profile-time 30 …`, in **instructions (Ir)** over
  1 940 637 507 instructions, processed by
  `callgrind_annotate [--inclusive=yes]`. Inclusive costs **overlap** (they are not added up) and Callgrind poorly distributes the inclusive
  cost of recursive functions (`drop_glue` of a tree, `xml::element`) among
  members of the cycle: these are excluded from the table.
- **Allocations**: heaptrack 1.6.80 on the release CLI
  (`./target/release/scoretk tests/corpus/v3/moonlight.mscx -o …`), plus a
  global counter (`GlobalAlloc` delegating to `System`, measurement binary outside
  the repository) compiled with the same profile as the release, which isolates the cost of
  each facade call (file reading outside the measurement).

## 2. Machine and conditions

| Element | Value |
| --- | --- |
| Processor | Intel(R) Processor U300, 6 logical processors, 8 Mio of cache |
| Cores | 2 threads at 4.4 GHz (P-core) + 4 cores at 3.3 GHz (E-cores) |
| Observed frequency | 1.4 to 2.7 GHz during the campaign (`/proc/cpuinfo`) |
| Memory | 15.7 Gio |
| System | NixOS, Linux 6.18.60, x86_64 |
| Rust toolchain | rustc 1.100.0 (0ef1bc386 2026-11-09) |
| Load (`loadavg`) | 4.3 to 11.3 on 6 logical processors |

The machine is **shared** (browser, other agents): medians vary
by a factor 1 to 6 from one run to another and even drift during
a run (§ 7). A trial pinned to an E-core (`taskset -c 3`) gave
medians up to 6 times higher for the same data point (48.3 ms versus 8.1 ms
for `.mscx` reading of moonlight): the retained values are those of a
run without pinning.

## 3. Measured times (before optimisation)

| Measurement | `v3/moonlight` | `v4/voiceswitching` | `v4/testExcludeInvisibleElements` | `v3/layout_elements` | `v3/voices` | `v4/hairpins_and_repeats` |
| --- | --- | --- | --- | --- | --- | --- |
| `.mscx` reading → model | 8,06 ms · 51 Mo/s | 0,75 ms · 41 Mo/s | 1,39 ms · 21 Mo/s | 0,46 ms · 54 Mo/s | 0,81 ms · 30 Mo/s | 0,50 ms · 20 Mo/s |
| MusicXML reading → model | 14,60 ms · 43 Mo/s | 0,87 ms · 39 Mo/s | 1,23 ms · 23 Mo/s | 1,60 ms · 22 Mo/s | 1,20 ms · 26 Mo/s | 0,32 ms · 31 Mo/s |
| model → MusicXML | 28,64 ms · 14 Mo/s | 1,06 ms · 29 Mo/s | 0,49 ms · 60 Mo/s | 0,59 ms · 42 Mo/s | 0,42 ms · 58 Mo/s | 0,17 ms · 59 Mo/s |
| model → MIDI | 1,46 ms · 280 Mo/s | 0,06 ms · 548 Mo/s | 0,06 ms · 470 Mo/s | 0,04 ms · 681 Mo/s | 0,03 ms · 969 Mo/s | 0,03 ms · 316 Mo/s |
| model → `.mscx` | 3,16 ms · 130 Mo/s | 0,13 ms · 243 Mo/s | 0,13 ms · 231 Mo/s | 0,12 ms · 209 Mo/s | 0,19 ms · 131 Mo/s | 0,04 ms · 262 Mo/s |
| model → `.mscz` | 10,09 ms · 41 Mo/s | 0,83 ms · 37 Mo/s | 0,76 ms · 39 Mo/s | 1,00 ms · 25 Mo/s | 0,84 ms · 29 Mo/s | 0,45 ms · 22 Mo/s |
| model → `.mxl` | 19,51 ms · 21 Mo/s | 0,89 ms · 35 Mo/s | 0,98 ms · 30 Mo/s | 0,88 ms · 28 Mo/s | 0,93 ms · 26 Mo/s | 0,25 ms · 40 Mo/s |
| model → LilyPond | 1,67 ms · 245 Mo/s | 0,13 ms · 236 Mo/s | 0,05 ms · 545 Mo/s | 0,05 ms · 500 Mo/s | 0,13 ms · 196 Mo/s | 0,01 ms · 793 Mo/s |
| model → ABC | 1,73 ms · 236 Mo/s | 0,06 ms · 506 Mo/s | 0,06 ms · 468 Mo/s | 0,03 ms · 895 Mo/s | 0,06 ms · 406 Mo/s | 0,01 ms · 774 Mo/s |
| `.mscx` → MusicXML chain | 16,21 ms · 25 Mo/s | 2,14 ms · 14 Mo/s | 2,58 ms · 11 Mo/s | 1,13 ms · 22 Mo/s | 1,03 ms · 24 Mo/s | 0,64 ms · 15 Mo/s |
| `write_playback` | 22,71 ms · 18 Mo/s | 1,74 ms · 18 Mo/s | 1,57 ms · 19 Mo/s | 1,42 ms · 17 Mo/s | 2,05 ms · 12 Mo/s | 0,45 ms · 22 Mo/s |

Input sizes: 409 776 o (moonlight), 30 868, 29 297, 24 855, 24 571 and
9 954 o.

Reading the table:

- order of magnitude: a 410 ko `.mscx` is read in **8 to 13 ms** and exported
  to MusicXML in **13 to 29 ms** (the two extremes observed in the campaign);
  the other writes stay under 3.2 ms on the same model;
- throughputs are around 20 to 60 Mo/s for the XML directions, 130 to 260 Mo/s for
  `.mscx`/LilyPond/ABC and 280 to 1 100 Mo/s for MIDI;
- **the rows do not add up**: here the full chain (16.2 ms)
  comes out faster than MusicXML export alone (28.6 ms), measured three
  groups earlier. The machine sped up during the run (§ 7). Only
  ratios between files within the same group are stable;
- `write_playback` is roughly MusicXML export alone plus MIDI and the
  time map.

## 4. Applied optimisations

Six commits were merged on 2026-11-18, after the initial
measurement campaign. The table of § 3 has **not** been redone since: the machine
stayed loaded (loadavg 8 to 111 during attempts, cf. § 7) and a
rerun would have measured contention, not the code. The effects below
are therefore those measured at the time of each commit — benchmark or profile of § 5 on
the `moonlight` chain —, without extrapolation from one commit to another.

- **`e983fa5` — precomputed table in `tick_to_xml`** (`pitch.rs`): the
  function rebuilt, for each note, the 65 candidates (13
  `DURATION_TYPES` × 5 dots) and normalised a `Fraction` by comparison —
  26 % of export time according to the commit's measurement. A `OnceLock`
  table filled once, in the same order, replaces it; an equivalence
  test compares it with the old algorithm. Measured effect: the
  `tick_to_xml` item (**25.6 %** inclusive) and the `Duration::ticks` it called
  (**29.6 %**, § 5.2) disappear from the profile.
- **`e8fc8ce` and `eff23e7` — `Fraction` arithmetic via a fast `i64` path**:
  GCD and short products stay in `i64` and no longer call the
  128-bit primitives (**18.6 %** of the chain's instructions, § 5.3) ;
  `eff23e7` also fixes negation of `i64::MIN` on this path. Measured effect:
  `normalize` (**12.1 %** self, **30.7 %** inclusive) and
  `__divti3`/`__umodti3`/`u128_div_rem` disappear from the profile; the gain
  applies to both directions, since the arithmetic is also used for reading.
- **`374bd61` — spanner index in `NotationsState`** : `spanner_indices`
  no longer scans all of `score.spanners` at every note and every measure
  (`SpannerType::eq` **4.8 %** self, iterator **1.9 %**, **7.1 %**
  inclusive, § 5.1 and 5.2). The `(type, tick, track) → indices` index is
  built once per measure.
- **`6bd58e6` — `zopfli` removed from the dependency graph**: the
  DEFLATE features of `zip` 2.6.1 are unified on `deflate-flate2` + `flate2`
  (`miniz_oxide`), and level 6 never used zopfli anyway (§ 5.3
  bis). Measured effect on the wasm package: **1 546 119 → 1 474 491 o** raw
  (−71.6 ko) and **640 218 → 616 877 o** gzipped (−23.3 ko).
- **`8b11c78` — 3.x XML tree borrowing the source text**: `read302::xml`
  copied every element name, every attribute and every text node;
  `Node` now carries `Cow<'a, str>` and `parse` borrows the
  parsed text (leads 5 and 6, § 6 ; `push_text` **6.9 %** and `to_vec` **5.9 %**
  inclusive). Measured effect at the commit: `moonlight` `.mscx` reading from
  **6.83 to 4.94 ms**.
- **Tried, not kept — instruments table compaction** (branch
  `perf-instr`) : **−13 ko** of raw package, but **+189 o gzipped**: DEFLATE
  already eliminates the table's duplicate strings, and compaction adds
  indexing. Rejected.

## 5. Hot spots

Profile taken **before** the optimisations of § 4, at the campaign commit:
it is what motivates each of them. The measurements in this section were not
redone afterwards (instruction shares and allocation counts remain
reproducible, unlike times, cf. § 7).

### 5.1 Real-time self (perf, `moonlight` chain)

Share of the full chain's CPU time, top 25 entries:

| Share | Function |
| --- | --- |
| 8,22 % | `__umodti3` (remaining 128-bit) |
| 8,16 % | `scoretk_core::model::fraction::normalize` |
| 5,33 % | `__divti3` (128-bit division) |
| 4,90 % | `compiler_builtins::…::u128_div_rem` |
| 4,82 % | `<SpannerType as PartialEq>::eq` |
| 3,42 % | `_int_malloc` |
| 2,96 % | `quick_xml::reader::Reader::read_event_impl` |
| 2,62 % | `__memmove_avx_unaligned_erms` |
| 2,58 % | `exportmusicxml::Context::write_measure` |
| 2,49 % | `exportmusicxml::Context::new` |
| 1,92 % | iterator of `notations::spanner_indices` (`Map<Filter<Enumerate<Iter<Spanner>>>>`) |
| 1,92 % | `__memcmp_avx2_movbe` |
| 1,86 % | `exportmusicxml::Context::chord` |
| 1,80 % | `Vec<read302::xml::Content>::push` |
| 1,79 % | `_int_free_create_chunk` |
| 1,78 % | `core::str::converts::from_utf8` |
| 1,77 % | `Vec<u8>::append_elements` |
| 1,49 % | `scoretk_musescore_import::read::read_score` |
| 1,45 % | `cfree` |
| 1,35 % | `malloc` |
| 1,32 % | `alloc::string::String::push` |
| 1,19 % | `exportmusicxml::stems::active_clef` |
| 1,18 % | `quick_xml::reader::XmlSource::read_with` |
| 1,15 % | `<Duration>::ticks` |
| 1,06 % | `drop_glue::<read302::xml::Content>` |

The same ranking appears in instructions (Callgrind without `--inclusive`) :
`normalize` 12.1 %, `__divti3` 9.4 %, `u128_div_rem` 5.6 %,
`SpannerType::eq` 4.8 %, `_int_malloc` 3.5 %.

### 5.2 Inclusive cost (Callgrind, instructions)

Top 25 functions, excluding benchmark infrastructure, `main`, raw allocation
and recursive destruction glue:

| Share of instructions | Function |
| --- | --- |
| 63,99 % | `scoretk::write` / `export_musicxml_with` |
| 54,50 % | `exportmusicxml::Context::write_measure` |
| 46,60 % | `exportmusicxml::Context::chord` |
| 35,48 % | `scoretk::read` / `musescore_import::read::read_score` |
| 30,70 % | `scoretk_core::model::fraction::normalize` |
| 29,59 % | `<Duration>::ticks` |
| 28,21 % | `fraction::from_i128` |
| 25,64 % | `musicxml_export::pitch::tick_to_xml` |
| 15,00 % | `__divti3` |
| 10,50 % | `<Fraction as Add>::add` |
| 10,10 % | `<Fraction as AddAssign>::add_assign` |
| 8,82 % | `<Fraction as MulAssign>::mul_assign` |
| 8,65 % | `quick_xml::reader::Reader::read_event_impl` |
| 8,49 % | `<Fraction as Mul>::mul` |
| 8,24 % | `exportmusicxml::Context::new` |
| 7,08 % | `musicxml_export::notations::spanner_indices` |
| 6,94 % | iterator of `spanner_indices` |
| 6,94 % | `read302::xml::push_text` |
| 6,56 % | `read302::Reader::read_score` |
| 6,32 % | `read302::measure::read_voice` |
| 5,87 % | `<[u8]>::to_vec` |
| 5,56 % | `compiler_builtins::…::u128_div_rem` |
| 5,32 % | `Fraction::new` |
| 5,08 % | `__rust_alloc` |
| 4,42 % | `musicxml_export::writer::XmlWriter::tag` |

In other words: **64 % of the chain is MusicXML export, 35 % reading**;
in both directions the same family dominates — exact
fraction arithmetic (`Fraction`, `Duration::ticks`), recomputed for each note and
each position.

### 5.3 XML parsing, `String`, `format!`

**Additive** shares (sum of self costs in instructions, hence without
overlap) over the 1 940 637 507 instructions of the `moonlight` chain:

| Item | Share |
| --- | --- |
| XML parsing: `quick_xml` + `memchr` (5,4 %) and building the `read302::xml` tree (3,3 %) | **8,6 %** |
| 128-bit arithmetic (`__divti3`, `__umodti3`, `u128_div_rem`) | **18,6 %** |
| `fraction` module alone (including `normalize`) | **17,0 %** (12,1 % for `normalize`) |
| `String` (`alloc::string::String`, including `push` 1,5 %) | **2,0 %** |
| `format!` / `write!` (`core::fmt`, `alloc::fmt`) | **0,01 %** |
| Allocator (`malloc`/`free`/libc) | **11,0 %** |
| `from_utf8` / `Utf8Chunks` / `str` | **4,3 %** |
| `memcpy`/`memmove`/`memcmp` | **3,0 %** |

Formatting is therefore **irrelevant**: writers push bytes into a
`Vec<u8>`/`String`, and the rare `format!` calls (identifiers `P<n>`, harmony
alterations) stay below 0.02 % of instructions. UTF-8, on the other hand,
appears twice: validation on XML reading and `lossy` on texts.

### 5.3 bis — Container compression (`.mscz`, `.mxl`)

Originally, the `deflate` feature of `zip` 2.6.1 also pulled in
`deflate-zopfli` (hence the presence of `zopfli` in the graph, cf.
`docs/licences-tierces.md`), but the default level (6) stayed in the
flate2 range and the Callgrind profile shows
`<flate2::mem::Compress>::compress_vec` at 1.16 % of instructions, **no
call to `zopfli`**. Compression therefore already went through **flate2 /
miniz_oxide**, not zopfli ; `6bd58e6` (§ 4) has since removed `zopfli` from the
graph. The `.mscz` (10.09 ms) and `.mxl` (19.51 ms) writing cost
on `moonlight` is dominated by DEFLATE compression of the `.mscx` (≈ 410 ko);
it is of the same order as uncompressed MusicXML writing (20.73 ms).

### 5.4 Allocations

A complete `moonlight.mscx` → MusicXML conversion, measured by the
global counter (release profile):

| Operation | Allocations | Bytes requested |
| --- | --- | --- |
| `.mscx` reading → model | 62 064 | 8,8 Mo |
| model → MusicXML | 23 682 | 2,4 Mo |
| `.mscx` → MusicXML chain | 85 746 | 11,2 Mo |
| MusicXML reading → model | 71 707 | 8,5 Mo |
| `write_playback` | 27 584 | 3,7 Mo |
| model → `.mscx` | 46 438 | 2,7 Mo |
| model → LilyPond | 16 729 | 1,0 Mo |
| model → ABC | 8 610 | 0,4 Mo |
| model → MIDI | 1 346 | 0,7 Mo |

On a small file (`v4/hairpins_and_repeats`, 10 ko), the same chain requests
2 953 allocations: the cost follows the number of XML elements, not the number of
measures.

heaptrack check on the CLI (`scoretk moonlight.mscx -o moonlight.musicxml`) :
85 864 allocator calls, 11 632 temporary allocations, **heap peak
7.03 Mo** (17 × the input `.mscx`), 548 o not freed at process end,
0.14 s under heaptrack (instrumentation overhead included).

Reading concentrates **72 % of allocations** (`push_text` alone: 20 871
allocations of copied text, plus ~10 300 node vectors), while it represents only 35 % of instructions: the intermediate XML tree
(`read302::xml::Content`/`Node`, one `Vec<u8>` per text) is the
dominant allocation item.

## 6. Quantified leads

Each lead gives the measurement motivating it and the theoretical **ceiling** on the
`moonlight` chain (share of instructions or allocations it can touch), not a guaranteed gain. Leads 1 to 4 concern writing,
5 and 6 reading: these are two separate work items. The state of each is
indicated at its head; those that were followed are detailed in § 4, with their
measured effect.

1. **Static durations table in `tick_to_xml`** (`pitch.rs`) — **done**
   (`e983fa5`, § 4): for each note, the function scanned the 13
   `DURATION_TYPES` × 5 dots and built at each attempt a
   `Duration::with_dots(...).ticks()`, hence a `Fraction` normalised by
   comparison. Measured cost: `tick_to_xml` **25.6 %** inclusive,
   `Duration::ticks` **29.6 %**. Ceiling: nearly all of the
   128-bit arithmetic (**18.6 %**).
2. **`Fraction::normalize` without `u128`** — **done** (`e8fc8ce`/`eff23e7`,
   § 4): GCD and products went through `i128`/`u128` although durations
   fit in `i64` (denominators ≤ 1024 in the corpus). Measured cost:
   `normalize` **30.7 %** inclusive, **12.1 %** self, plus
   `__divti3`/`__umodti3`/`u128_div_rem` (**18.6 %** in total). Ceiling:
   up to ~15 % of the chain if the `i64` path avoids 128-bit divisions.
3. **Memoise `Duration::ticks`** — **partially done**: the value is
   pure and the `Duration` is immutable; the export recomputes it for each note
   (`write_measure` 54.5 % inclusive). Ceiling: the share of redundant calls,
   not isolated here — at least half of the **29.6 %** of `ticks`. The table of
   § 4 removes the 65 comparisons per note, but `ticks` is still called once
   per note: memoisation itself is not done.
4. **Index the spanners** — **done** (`374bd61`, § 4): `spanner_indices`
   traversed all of `score.spanners` at each note and each measure
   (`SpannerType::eq` **4.8 %** self, iterator **1.9 %**) ; an index
   `(type, tick, track) → indices` built once per measure replaced
   this scan. Ceiling: **7.1 %** inclusive.
5. **Remove or lighten the intermediate XML tree** — **done** for the
   3.x reader (`8b11c78`, § 4): reading built `Content`/`Node`
   with a `Vec<u8>` per text (**72 %** of allocations), then destroyed it ;
   parsing weighed **8.6 %** of instructions and the allocator **11.0 %**.
   Ceiling: all of the allocator item on the reading side, i.e. ~8 to 10 % of the
   chain.
6. **Borrow XML texts** (`&[u8]` to the source) instead of
   copying them — **done** for the 3.x reader (`8b11c78`, § 4): `push_text`
   **6.9 %** inclusive, `to_vec` **5.9 %**. This was the most direct way to
   lead 5, with the same ceiling.
7. **`String` and `format!`**: **2.0 %** and **0.01 %** of instructions. Nothing to
   gain in the short term; do not tackle them.
8. **Direction priority** — **followed**: MusicXML export represented **64 %**
   of the chain versus **35 %** for reading; leads 1 to 4 were therefore
   handled first, leads 5 and 6 afterwards. The other writes
   (MIDI, `.mscx`, LilyPond, ABC) stay under 3.2 ms on 410 ko: irrelevant
   for now.

## 7. Measurement limits

- **No rerun after optimisation**: on 2026-11-18, the machine
  ran other campaigns in parallel (LilyPond tests, headless browsers,
  builds) and `loadavg` stayed between 8 and 111 during
  attempts; medians taken under these conditions (`.mscx` reading of
  `moonlight`: 7.7 to 15.5 ms from one run to another) do not separate the
  gain of the optimisations from contention. The table of § 3 therefore remains that
  of the initial campaign, and § 4 relies on measurements made at the time of
  each commit.
- **Machine variability**: the same data point (`.mscx` reading of
  `moonlight`) gave 8.1 ms, 12.7 ms and 48.3 ms median depending on the run,
  and drift continues during a run — to the point that the table of § 3
  contains a full chain faster than the export alone measured three
  groups earlier. Absolute times are **not** reproducible to within a few
  percent on this machine; instruction shares (§ 5.2, 5.3) and
  allocation counts (§ 5.4) are.
- A single shared machine, without stable pinning (the 3.3 GHz E-cores
  are 1.3 × slower than the P-core and were occupied by other
  processes) ; no CPU governor setting is possible here.
- The corpus is small (410 ko at most): the measurement describes scores of
  a few tens of thousands of notes, not an orchestral score.
- `opt-level = "z"` and `lto = true` (the delivery profile at the time, now
  the `wasm` profile) change inlining:
  perf and Callgrind see only the functions that remain after
  optimisation, and some are merged (`Context::new` absorbs several
  constructors).
- Callgrind counts instructions, not cycles: it ignores cache misses and
  mispredicted branches ; perf, for its part, cannot produce
  call graphs in this environment.
- Criterion's `iter` measurements include allocation **and** dropping
  the output: this is the full cost of the call, not only the computation.
