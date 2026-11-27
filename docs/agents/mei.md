# MEI 5.1 — port, measurements and limits

This document records the MEI directions added in the t44 series: a faithful
Rust rewrite of MuseScore Studio **4.7.4**'s MEI importer and exporter,
wrapped in three crates (`scoretk-mei-common`, `scoretk-mei-import`,
`scoretk-mei-export`) and wired into the facade, CLI and wasm `formats()`.

Read first: [`format-ref/mei.md`](format-ref/mei.md) (MEI 5.1 syntax) and
[`format-mapping/mei.md`](format-mapping/mei.md) (model mapping). The C++
reference is `ref-musescore/.474/src/importexport/mei/internal/`
(`meiimporter.cpp`, `meiexporter.cpp`, `meiconverter.cpp`), recreated as
described in [`upstream.md`](upstream.md) §2.1.

## 1. What was ported

| Rust file | MuseScore 4.7.4 C++ |
| --- | --- |
| `crates/scoretk-xml-common/src/tree.rs` | in-memory replacement of `muse::XmlStreamReader` (shared with MusicXML) |
| `crates/scoretk-xml-common/src/writer.rs` | `engraving/rw/xmlwriter.cpp` (shared with MusicXML) |
| `crates/scoretk-mei-common/src/convert.rs` | `meiconverter.cpp` (`durFromMEI`, `pitchFromMEI`, `tstampToFraction`, `measureFromMEI`, `staffGrpFromMEI`, …) |
| `crates/scoretk-mei-common/src/ids.rs` | `meiconverter.h` § `@type` prefixes (`mscore-beam-`, `mscore-repeat-`, …) |
| `crates/scoretk-mei-import/src/reader.rs` | `meiimporter.cpp` (`readScore`, `readScoreDef`, `readMeasure`, `readStaves`, `readLayers`, `readElements`, `addChordRest`, `readTuplet`) |
| `crates/scoretk-mei-import/src/header.rs` | `readMeiHead`, `readPgHead`, `buildScoreParts`, `readStaffDefs`, `readStaffGrps`, `readInstrDef` |
| `crates/scoretk-mei-import/src/controls.rs` | `readControlEvents`, `addAnnotation`, `addSpanner`, `findStart`, `readVerses`, `addSpannerEnds` |
| `crates/scoretk-mei-export/src/header.rs` | `writeHeader`, `writeScoreDef`, `writeStaffDef`, `writeLabel`, `writeInstrDef` |
| `crates/scoretk-mei-export/src/writer.rs` | `writeScore`, `writeMeasure`, `writeStaff`, `writeLayer`, `writeChord`, `writeNote`, `writeRest`, control events |

Format detection is by `.mei` extension or by the MEI namespace
(`http://www.music-encoding.org/ns/mei`). The facade features are
`mei-import` / `mei-export`, both on by default; the CLI gets `mei`
automatically through `Format::ALL` and the wasm `formats()` array has eight
entries. `Capabilities` declares L0–L5 for both directions, citing MEI's
`scoreDef`, CMN events, control events and layout elements.

Unlike a strict RelaxNG validator, the reader is a best-effort parser like
MuseScore's: unknown elements are skipped and recorded in `Score::report`
(`control-event`, `layer-beyond-voices`, `nested-tuplet`, `mei-head-verbatim`).

## 2. References and oracles

```sh
scripts/gen-mei-references.sh          # 56 *.ref.mei, two mscore processes at most
```

Command recorded by the script:
`env -u LD_LIBRARY_PATH QT_QPA_PLATFORM=offscreen nix shell nixpkgs#musescore -c mscore -o <out.ref.mei> <in.mscx>`.
The binary reports `MuseScore4 4.7.4`. The corpus files are the same
`.mscx` used for the MusicXML references. `@isodate` in the references is the
generation time; the comparison normalises it away.

Two comparisons are versioned:

- `crates/scoretk/tests/layer_coverage.rs` — the MEI rows compare the item
  inventories (key = layer, tick, staff, kind, value) of
  `reference .mscx → scoretk MEI → scoretk MEI` and of
  `MuseScore *.ref.mei → scoretk MEI reader`.
- `crates/scoretk/tests/layer_coverage.rs` also prints
  `mei_core_loss_diagnostic` (per file/tick/staff), `mei_full_loss_table`
  (per kind) and `mei_import_figure_without_absent_carriers` (the second,
  additive import figure of §8.3).
- `scripts/oracle/mei/valide-corpus.sh` — no-argument `jing` validation of
  both 56-score exports (see §12.3).
- `crates/scoretk/tests/mei_conformity.rs` — a MusicXML-style normalised
  comparison of scoretk's export with the 4.7.4 reference: element name plus
  a stable set of musical attributes, folding `@accid` from a child element
  into the note. Volatile ids, `@isodate`, `@tstamp` and layout-only `<beam>`
  containers are excluded.

## 3. Results (56 corpus scores)

### Layer coverage (declared L0–L5, 5 879 reference items)

| Row | Essential | Full support (before → after) | Per-layer matched / reference (after) |
| --- | ---: | ---: | --- |
| Round trip (own writer) | **99.96 %** | 73.52 % → **95.14 %** | L1 1059/1085, L2 3243/3314, L3 206/296, L4 106/175, L5 973/1009 |
| Import of MuseScore 4.7.4 `.ref.mei` | **98.57 %** | 81.90 % → **87.16 %** | L1 1051/1085, L2 3207/3314, L3 218/296, L4 108/175, L5 537/1009 |

The "before" column is the measurement of commit `b6a13039`; the "after"
column is the measurement after the campaigns of §7 below. The per-layer
counts come from `layer_coverage_table`. The remaining gap of the import row
is dominated by 380 `style` items **absent from MuseScore's own `.ref.mei`
files** (see §7). A second import figure that excludes only the item kinds
whose carrier MuseScore never writes is **95.66 %** (5 251 reference items);
it is printed by `mei_import_figure_without_absent_carriers` and never
replaces the 87.16 % main figure.

### Normalised export conformity

| Comparison | Matched / reference events | Conformity |
| --- | ---: | ---: |
| scoretk MEI export vs MuseScore 4.7.4 `.ref.mei` | 3 877 / 4 016 | **96.54 %** |

**The ≥ 95 % target is now met for the round trip (95.14 %), the musical core
(essential), export conformity, and the reference-bound import figure
(95.66 %).** The campaigns of §7 and §8 closed the barline, stem, `<pgHead>`,
style, signature, tremolo, articulation, ending, `<dynam>`/tempo, note id,
multi-measure rest, nested tuplet, key signature, SMuFL note-head, slur-free
note anchoring and articulation anchoring gaps. The main import row stays
below 95 % because MuseScore's own `.ref.mei` files carry none of the
page-layout/typography carriers (380 `style` items) and none of the note-head
or velocity carriers; these are pinned by
`mei_musescore_references_carry_no_style` and documented in §7.3/§8.3.

Largest remaining round-trip gaps: `dynamic-velocity` 43, `lyric` 30,
`slur` 19, `credit-position` 18, `credit` 15, `tbox-text` 15,
`tbox-position` 15, `instrument` 15, `clef` 13. Largest remaining import gaps:
`style` 380, `dynamic-velocity` 43, `note-head` 41, `note` 33, `lyric` 18,
`credit-position` 18, `tempo` 11. The out-of-reach items are justified in
§7.3 and §8.3 with the MEI 5.1 attribute definitions and the MuseScore C++
lines.

## 4. Known port decisions

- `@dur` is a denominator; middle C is `c`/`4` in MuseScore's MEI (the
  `pitchFromMEI` formula is `(oct + 1) * 12`). The format reference notes a
  conflicting `oct=3` convention for other MEI tools: the 4.7.4 behaviour
  prevails here (invariant 2 of `AGENTS.md`).
- `@tstamp` is one-based in *beats*, recovered from the unreduced
  time-signature denominator (`Measure::time_sig_numerator` plus the reduced
  `nominal_len`) and approximated by continued fractions as in 4.7.4.
- Nested tuplets are accepted but reported as an L6 loss (MuseScore 4.7.4
  rejects them); more than four layers per staff is likewise a reported loss.
- MuseScore 4.7.4 can emit nested `<measure>` elements for nested tuplets;
  the reader follows them so the reference model keeps every measure.
- The writer emits stable generated ids (`n0`, `m1`, …) because MEI control
  events reference notes by `xml:id`. They are not MuseScore's hash ids.
- `@type="mscore-*"` values are MuseScore extensions, not standard MEI;
  they are read and written but reported as such when they cannot be mapped.
- No model layer was added. The `GesturalTime` / `ExternalRef` / `MeiExpansion`
  slots proposed in `model-modularisation.md` §7.2 were **not** introduced:
  MuseScore 4.7.4 itself does not read `@dur.ges`, `@tstamp.ges`, `@copyof`,
  `@facs`, `<timeline>` or `<expansion>`, so no measurement against the oracle
  justified a non-additive model change. `inventory.rs` and `layers.rs` were
  left untouched, which keeps the parallel GABC work easy to rebase.

## 5. Source upgrade

1. Recreate/refresh `ref-musescore/.474` with `upstream.md` §2.1; verify
   `git describe --tags --exact-match HEAD` is `v4.7.4`.
2. List changes with
   `git -C ref-musescore/.474 log --oneline <old>..<new> -- src/importexport/mei`
   and read the diff of `meiimporter.cpp`, `meiexporter.cpp` and
   `meiconverter.cpp`; port file by file, keeping the C++ function names in
   comments and the « Portage de … » headers.
3. Regenerate the references with `scripts/gen-mei-references.sh` only if the
   behaviour target deliberately changes; record the new version in `NOTICE`
   and in this document.
4. Re-run `cargo test --workspace`, the `layer_coverage` table,
   `mei_conformity`, clippy, fmt and the wasm build. No percentage may drop
   without a written explanation here.

## 6. Wasm size

`nix build .#wasm` (size settings in `[profile.wasm]`, `opt-level = "z"`;
they were in `release` when this was measured): the ESM wasm grew from
**2 379 470** to **2 453 424** bytes, i.e. **+73 954 bytes (+3.11 %)**.
The stable contract (`convert`, `detect`, `formats`, `version`,
`convertWithTimeline`) is unchanged; `formats()` now reports `mei` as the
eighth format. The later gap campaign changes code only inside the MEI
crates; `nix build .#wasm` was not re-run for the final size measurement.

## 7. Gap campaign (2026-11-25)

### 7.1 Method

One step per commit, each with unit tests and the full validation suite
(`fmt --check`, `clippy -D warnings`, `test --workspace`, `e2e_corpus` 8/8,
`cargo build -p scoretk --target wasm32-unknown-unknown`). The per-kind loss
table is produced by `layer_coverage::mei_full_loss_table` with `--nocapture`.
No declaration was narrowed and no private attribute was used as a carrier.

### 7.2 What was closed

| Kind (before → after, round trip) | Fix |
| --- | --- |
| `barline` 85 → 1 | `barlines.rs` recreates MuseScore's implicit `EndBarLine` segment from `Measure::end_bar_line` / `@right`. |
| `stem` 163 → 6 | `@stem.dir` written from `Chord::stem_direction` (`Convert::stemToMEI`, `meiexporter.cpp:2297`). Six residual losses are tuplet ticks rounded by `@tstamp`. |
| `credit` 67 → 15, `credit-position` 67 → 18 | `writePgHead` (`meiexporter.cpp:386`) writes the nine alignment cells; only the first frame is representable. |
| `style` 380 → 0 | `att.pages` (`page.width`, `page.height`, `page.leftmar`, `page.rightmar`, `page.topmar`, `page.botmar`), `@vu.height`, `@optimize` on `<scoreDef>` and numeric `@fontsize` on `<rend>` (§7.3). |
| `note` 70 → 26, `measure` 25 → 0, `rest` 19 → 0 | `@dur.ges` carries `actualTicks()`; mid-score `<scoreDef>` changes and signatures at the measure's absolute tick. |
| `line-break` 51 → 4 | `<sb>` written before the measure that follows the break, as `writeScore` does. |
| `slur` 65 → 19, `hairpin` 52 → 6 | Control events anchored by `@startid`/`@endid` on note `xml:id`s, like MuseScore. |
| `trill` 1 → 1, `tremolo` 24 → 0 | `<bTrem>`/`<fTrem>` containers written; the reader tracks `Start`/`Stop` across the two chords. |
| `articulation` 29 → 8 | `<artic>` children of `<note>`/`<chord>` with `Convert::articToMEI` names. |
| `volta` 17 → 1 | `<ending n/type/label>` around the measures of a volta. |
| `tempo` 20 → 1 | `@midi.bpm` divided by 60 (`Convert::tempoFromMEI`, `meiconverter.cpp:3343`) and SMuFL `<rend>` read before the plain text. |
| `key-sig` 37 (tick part) | `<scoreDef>` written before a measure when the key/time signature changes; signature segments inserted at the measure's absolute tick. |
| `marker` 11 → 4, `jump` 8 → 2 | `<repeatMark>` with `@type="mscore-marker-*"`/`"mscore-jump-*"` read and written. |
| `time-sig` 15 → 1, `clef` 14 → 13 | `<meterSig>`/`<clef>` with `@tstamp` inside a layer for mid-measure changes. |
| `fermata` 8 → 4 | `<fermata>` written from segment annotations. |

Import row: 81.90 % → 84.18 %. The gains come from the same reader fixes
(barlines, articulations, tremolos, endings, signatures, tempo, repeat marks);
`style` cannot improve because the references omit the carriers.

### 7.3 Items declared out of reach, with MEI 5.1 citations

These are not declaration changes: the kinds stay in L1–L5, and the
percentage is reported with them in the denominator.

- **`dynamic-velocity` (43 per row).** The MEI 5.1 attribute classes contain
  no velocity attribute on `<dynam>`: `att.dynamic` in `MEI.shared.xml` only
  carries the dynamic's type/placement. `Convert::dynamToMEI`
  (`meiconverter.cpp:1341`) writes `@label` + text only. `@vel` exists in
  `att.midi` for a *note*, not for a `<dynam>` annotation. Nothing standard
  can carry MuseScore's numeric dynamic velocity.
- **`key-sig` mode (37 per row before §8).** `att.keySig.log` defines only
  `@sig` (`MEI.shared.xml`); the schema has no `ident="mode"` attribute.
  This is *not* a loss any more: `Convert::keyFromMEI` (meiconverter.cpp:2066)
  builds an `engraving::Key` (fifths only), so the reader now leaves
  `KeyMode::Unknown`, exactly as the `.mscx` reference model does. The fifths
  count is carried by `@keysig`/`@sig`.
- **`credit`/`credit-position` (15/18 per row).** `<pgHead func="first">` is
  the *first-page* header (`meiexporter.cpp:386-459`); MuseScore writes no
  equivalent for later frames, and the multi-set comparison includes the
  credit index. Empty `<rend>` elements carry the text-style sizes but not
  the later pages' credit order.
- **`tbox-text`/`tbox-position` (15/15 per row).** MEI 5.1 has `<pgHead>` for
  a page header and `<dir>` for a staff-anchored direction, but no
  measure-anchored text-frame element corresponding to MuseScore `TBox`;
  `writeScoreDef` only converts the first `VBox`. Writing a `<dir>` would
  invent a carrier MuseScore 4.7.4 never emits.
- **`note-head` (22 round trip / 41 import before §8).** `@head.shape` uses
  the closed `data.HEADSHAPE` enumeration (`MEI.xml`: `quarter`, `half`,
  `whole`, `circle`, `+`, `diamond`, `isotriangle`, `oval`, `piewedge`,
  `slash`, `square`, `x`, …). The groups without a term are now carried by
  `@glyph.name` + `@glyph.auth="smufl"` (MEI 5.1 `att.noteHeads`), ported from
  `noteHeads` (`engraving/dom/note.cpp:89`) and the SMuFL names in
  `engraving/types/symnames.cpp`: round-trip `note-head` fell 22 → 2.
  `MeiExporter::writeNote` (meiexporter.cpp:1370) writes neither attribute,
  so the 41 import items stay a *reference* boundary (§8.3).
- **`style` (380 on the import row only).** MEI 5.1 *can* express page
  geometry (`att.pages`, `@vu.height`, numeric `@fontsize`), and scoretk
  reads and writes all of them (round trip 380 → 0). The MuseScore 4.7.4
  `.ref.mei` files contain none of these attributes, so the information is
  absent from the oracle, not from the reader. Verified by
  `mei_musescore_references_carry_no_style`.

No scoretk-private carrier was used: the `@type="mscore-*"` values are
MuseScore's own documented extensions and are read/written as such, not
counted here as interoperability gains. `@dur.ges` (scoretk round-trip
extension, MEI 5.1 `att.gestural.log`) is likewise not counted as an
interoperability gain.

## 8. Second gap campaign (2026-11-25, branch `mei2`)

### 8.1 Method

One commit per fix, with unit tests and the full suite after each commit
(`fmt --check`, `clippy -D warnings`, `test --workspace`, `e2e_corpus` 8/8,
`cargo build -p scoretk --target wasm32-unknown-unknown`, all under
`nix develop -c env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 nice -n 10`).
`mei_core_loss_diagnostic` prints the missing reference items file by file
with tick and staff, and `mei_full_loss_table` ranks the per-kind losses. No
declaration was narrowed, and no item was removed from the main figures.

### 8.2 Core musical losses: file-by-file causes and fixes

The diagnostic attributed every core loss to a concrete file:

| Kind (import) | Files / measures | Cause and fix |
| --- | --- | --- |
| `measure` 20, `rest` 23 | `v4/mmrest` (16 measures, 16 rests), `v3/testTboxWords1` (1 measure, 3 rests) | `Convert::measureToMEI` (meiconverter.cpp:2254) writes **one** `<measure>` per layout measure and `writeScore` (meiexporter.cpp:292) iterates `system->measures()`: the covered measures are absent from the reference. The reader now recreates them from the `@n` gap/jump, with a full-measure rest each (`expand_mmrest`). |
| `note` 55 | `v3/testTuplets8` 15, `v4/voiceswitching` 10, `v4/timesig-09` 10, `v3/testMultiInstrumentPart3` 8, `v3/layout_elements` 6, `v3/moonlight` 1, `v4/copypaste_parts` 4, `v3/testTboxWords1` 1 | Nested tuplets (15+10), missing gestural duration (10+6+1+5), mid-score transposition (8), measure offset (5). The reader now stacks nested ratios (`tuplet.parent`), applies `@n`-based measure offsets and reads `<scoreDef>` inside `<ending>`; the rest are reference boundaries (§8.3). |
| `tuplet-member` 39 | `v4/voiceswitching` 24, `v3/testTuplets8` 15 | Same nested-ratio fix, plus innermost tuplet membership on each event. |
| `key-sig` 37 | `v3/keysig_78216` 3, `v3/layout_elements` 4, `v3/voices` 8, etc. | `KeyMode::Unknown` from `Convert::keyFromMEI`, and `@keysig` on a mid-score `<scoreDef>` now applied to every staff; implicit C added on the first measure. |

Round-trip losses were closed by `tuplet_chain` in the writer (nested `<tuplet>`
is legal MEI 5.1 even though MuseScore's `writeTuplet`, meiexporter.cpp:1554,
says « nested tuplet export not fully supported »), by writing the initial
clef and key signature on `<staffDef>` (`writeStaffDef`,
meiexporter.cpp:684-716), by SMuFL note-head glyphs, and by exact articulation
and repeat-mark anchoring. Round trip: 92.35 % → **95.03 %**. Main import:
84.18 % → **87.11 %**.

### 8.3 Items absent from MuseScore's own references (not reader bugs)

These stay in both main denominators. Each has a MuseScore C++ line proving
the information is never written:

- **`@dur.ges` (note 33, tremolo 12, some rests).** `MeiExporter::writeChord`
  (meiexporter.cpp:1241-1270) writes `@dur`/`@dots` only; there is no
  `dur.ges` anywhere in the 4.7.4 MEI module. A tremolo chord with
  `duration=1/2` is therefore written as `dur="1"` in `v4/timesig-09`, so
  MuseScore's own importer also loses its gestural tick. scoretk's writer
  emits `att.gestural.log/@dur.ges` for its own round trips, but cannot read
  what the reference does not carry.
- **Mid-score transposition (`v3/testMultiInstrumentPart3`, 8 notes).**
  `Convert::staffToMEI` (meiconverter.cpp:3099) takes `@trans.semi` from
  `part()->instrument()->transpose()` when writing the *initial* `staffDef`.
  An instrument change mid-score is never written again, so the concert
  pitches of the second clarinet cannot be recovered.
- **Outer tuplet in beam (`v3/testTuplets8` 11, `v4/voiceswitching` 6).**
  `writeTuplet` is called once per chord with `chordRest->tuplet()`
  (meiexporter.cpp:1092, :1109, :1544): an outer n-olet with no direct note
  is not written. MEI 5.1 allows nesting, but the reference omits it.
- **`style` 380, `dynamic-velocity` 43, `note-head` 41, `credit` 15,
  `credit-position` 18, `tbox` 15+15.** See §7.3 and the carrier assertions in
  `mei_musescore_references_carry_no_style`: no `att.pages`, no `@vel`, no
  note-head attributes, only the first `VBox` (`writeScoreDef`,
  meiexporter.cpp:326-331).
- **`tempo` 11.** `Convert::tempoFromMEI` (meiconverter.cpp:3343) divides
  `@midi.bpm` by 60; the `.mscx` reference stores values such as `6.66667` or
  `1.53333` rounded to five decimals while the MEI reference only has the
  integer BPM `400`/`92`. The precision is absent from the oracle.
- **`marker` 6, `jump` 5.** The MuseScore `label` (`coda`, `segno1`) and
  jump target are not the text written in `<repeatMark>` (`To Coda`, `1`);
  `@type="mscore-*"` carries only the kind (`Convert::markerToMEI`,
  meiconverter.cpp:2158).

The second import figure excludes exactly the seven kinds of the second
bullet group (`style`, `dynamic-velocity`, `note-head`, `credit`,
`credit-position`, `tbox-text`, `tbox-position`): **95.60 %** on 5 251
reference items, printed by `mei_import_figure_without_absent_carriers`.
Partially absent kinds (`note`, `tempo`, tuplets) are *not* excluded.

### 8.4 State

No new model layer, no changed wasm contract, no new licence or NOTICE entry:
all changes are inside the three MEI crates and two integration tests. The
wasm build of `scoretk` passes after each commit; `nix build .#wasm` was not
re-run, so §6's size measurement is unchanged.

## 9. Measurement against what MEI can express (2026-11-25, branch `mei2`)

The 4.7.4 references measure **MuseScore's exporter**, not the reader's
coverage of MEI. Their 93.5 % ceiling (§3) is a property of the oracle:
`writeScoreDef` emits no page geometry, `writeNote` no note-head attribute,
`dynamToMEI` no velocity (§7.3/§8.3). The second, independent oracle is the
official **MEI Sample Encodings** repository, which is not produced by
MuseScore.

### 9.1 Source, licence, pin

- Origin: <https://github.com/music-encoding/sample-encodings>, directory
  `MEI_5.1`, commit `f3f1baba02e32279b25dad660b82c834bac034b4`.
- Licence: **ECL-2.0**, Copyright (c) Music Encoding Initiative (MEI) Board
  (upstream `LICENSE`); ECL-2.0 permits redistribution with attribution and a
  copy of the licence, and is GPL-3 compatible. Recorded in `NOTICE` and
  `docs/licences-tierces.md`.
- The files are **not vendored** (32 MiB of XML). `scripts/fetch-mei-samples.sh`
  fetches the pinned commit to `$SCORETK_MEI_SAMPLES` (default
  `$HOME/.cache/scoretk/mei-samples`, outside the repo and the build paths)
  and verifies every SHA-256 of `tests/corpus/mei-samples/MANIFEST.tsv`.
  They are untrusted data, only parsed as XML, never executed.

### 9.2 Lot selection

All 167 `.mei` files of `MEI_5.1` validate against MEI 5.1's
`mei-all_anyStart.rng` (`jing`, schema repo `music-encoding/schema`, `5.1`).
The MEI 5.1 folder contains **no** neume or mensural encoding (checked for
`<neume>`, `<neumeGrp>`, `<nounc>`, `<syllable>`, `<mensuration>`,
`<proport>`, `<ligatura>`): no separate basket is needed, and excluding them
would have been vacuous. The lot is the 130 files with a `<score>`; the 37
others are header-only or document-start fragments with no music events and
are listed with their reason in `EXCLUDED.tsv`. The manifests live under
`tests/corpus/`, hence inside the flake `tests/corpus` filter.

### 9.3 Two measurements (2026-11-25, baseline)

`crates/scoretk/tests/mei_samples.rs` contains three tests:

- `mei_samples_round_trip_table`: for every file,
  `MEI → model → MEI → model`, compared by `(layer, tick, staff, kind, value)`
  with the declared MEI kinds (`compare_with_kinds`), plus a full per-kind
  loss table printed with `--nocapture`;
- `mei_samples_import_completeness_table`: `scoretk-mei-import`'s `audit_mei`
  walks the source tree and reports, per element and per `(element, attribute)`,
  how often it occurs and how often the reader does **not** map it;
- `mei_samples_never_panic`: reads all 167 files (lot and excluded fragments);
  an error is acceptable for a fragment with no `<score>`, a panic is not.

Baseline before any reader fix (130 files, 2 files with a non-`mei` root still
rejected, 128 measured):

| Measurement | Result |
| --- | ---: |
| Round trip, full support (declared L0–L5) | **77.64 %** |
| Elements present / ignored by the reader | 463 375 / 19 681 (4.25 %) |
| Attributes present / ignored by the reader | 1 306 898 / 65 552 (5.02 %) |

Largest round-trip losses by kind: `note` 33 732, `stem` 16 406,
`lyric` 14 957, `rest` 4 373, `tie` 1 173, `slur` 1 038, `time-sig` 956,
`articulation` 559. Largest ignored musical elements: `p` 1 674,
`tupletSpan` 1 467, `change`/`changeDesc` 1 392 each (header revision
metadata), `contentItem` 190. Largest ignored attributes: `@curvedir` on
`<slur>` 9 870, `@tie` on `<note>` 6 754, `@tuplet` on `<note>` 4 946,
`@dur.ppq` 3 265, `@width` on `<measure>` 3 008, `tupletSpan/@num.visible`
1 445, `syl/@wordpos` 1 436.

The row is deliberately not compared with the 95.14 % of §3: that figure uses
MuseScore's own exporter as oracle, while this one starts from external MEI
that uses many standard constructs MuseScore never writes.

### 9.4 First fix: staff index from sorted `staff@n` (commit after `d42791d1`)

The diagnostic attributed 33 732 lost `note`, 16 406 `stem`, 4 373 `rest`,
1 173 `tie` and most `slur`/`lyric` items to a **single** cause: the reader
indexed the staves by the document order of `<staffDef>` (which scoretk's own
writer groups by part), whereas MuseScore 4.7.4's
`MeiImporter::buildStaffLayerMap` (`meiimporter.cpp:3309`) collects
`//staff/@n` into a sorted `std::set` and `getStaffIndex` (`:211`) returns the
position in that set. A MEI file whose parts are not in staff-number order
(e.g. `Das_Veilchen_0Parameters.mei`: Voice = staff 1, Piano = staves 2–3)
therefore placed every event on the wrong staff after a round trip.

`build_staff_layer_map` now follows the C++ (`BTreeSet` of `staff@n` from
`//staff`, index = rank), with a unit test that reverses the `<staffDef>`
order. No declaration was narrowed.

| Kind (round trip) | Before | After |
| --- | ---: | ---: |
| `note` | 33 732 | 9 518 |
| `stem` | 16 406 | 219 |
| `rest` | 4 373 | 114 |
| `tie` | 1 173 | 0 |
| `slur` | 1 038 | 267 |
| Full support | 77.64 % | **91.77 %** |

Remaining leaders: `lyric` 14 934, `note` 9 518, `time-sig` 956,
`key-sig` 406, `articulation` 376.

### 9.5 Second fix: lyrics on the chord/rest segment

Two independent defects dropped 14 886 of the 14 934 missing `lyric` items:

1. **Writer.** `write_note` emitted `<note/>` then `<verse>` as a *sibling* in
   the layer. MEI 5.1 defines `<verse>` as a child of `<note>`/`<chord>`
   (`model.noteLike`), and `MeiExporter::writeVerse`
   (meiexporter.cpp:1594) attaches it to the currently open note node. The
   writer now opens `<note>`, writes articulations and verses, and closes it;
   for a chord, each `<note>` is closed before `<chord>`'s own verses.
2. **Reader.** `last_chord_rest_mut` looked for the last segment of a track
   having *any* element. At the first tick of a measure the initial
   clef/key/time-signature segments share that tick, so the lookup returned a
   signature and no chord rest: lyrics (and grace chords, articulations,
   tremolos) attached to the first event were dropped. The lookup now
   explicitly requires a `SegmentItem::ChordRest`.

| Kind (round trip) | Before | After |
| --- | ---: | ---: |
| `lyric` | 14 934 | 48 |
| Full support | 91.77 % | **96.24 %** |

Remaining leaders: `note` 9 518, `time-sig` 956, `key-sig` 406,
`articulation` 376, `slur` 267, `stem` 219, `dynamic` 192.

### 9.6 Third fix: transposing instruments

The remaining `note` losses were concentrated in `Handel_Messias` (3 779),
`Berlioz_Symphony_Op25` (2 895) and `Hummel_Concerto` (2 257): all three use
transposing parts. The reader applies `@trans.semi` in
`Convert::pitchFromMEI` (meiconverter.cpp:2765) to get the concert pitch, but
the writer called `Convert::pitchToMEI` (`:2820`) with `transpose = 0`, and
emitted no `@trans.diat`/`@trans.semi` on `<staffDef>` although
`Convert::staffToMEI` (`:3099`) writes both. The second read therefore
transposed a second time.

The writer now takes `part.instrument.transpose.chromatic` from the note's
staff (MuseScore uses `staff->part()->instrument()->transpose()`), writes both
`@trans.diat` and `@trans.semi` on every `<staffDef>`, and a unit test checks
that a concert E4 in a +2 part is written as written D4 and read back as E4.

| Kind (round trip) | Before | After |
| --- | ---: | ---: |
| `note` | 9 518 | 329 |
| Full support | 96.24 % | **98.98 %** |

Remaining leaders: `time-sig` 956, `key-sig` 406, `articulation` 376,
`slur` 267, `stem` 219, `dynamic` 192, `rest` 114.

### 9.7 Fourth fix: layout-only `<scoreDef>` must not repeat signatures

`Hummel_Concerto_for_trumpet_E-major.mei` contains 104 mid-score
`<scoreDef page.height=… page.width=…>` elements between systems. The reader
`set signatures_pending = true` unconditionally at the end of `readScoreDef`,
so each of those inserted a full `<TimeSig>` and `<KeySig>` segment in the
following measure: 686 spurious `time-sig` items and 400 `key-sig` items.
MuseScore's `readScoreDef` (meiimporter.cpp:1052) only updates `m_timeSigs` and
`m_keySigs` when `@meter.*` / `@keysig` are present. The port now snapshots
both maps and sets `signatures_pending` only when they actually change (or for
the initial `scoreDef`), with a unit test.

| Kind (round trip) | Before | After |
| --- | ---: | ---: |
| `time-sig` | 956 | 41 |
| `key-sig` | 406 | 4 |
| `dynamic` | 192 | 85 |
| Full support | 98.98 % | **99.43 %** |

Remaining leaders: `articulation` 376, `note` 329, `slur` 267, `stem` 219,
`rest` 114, `tuplet-member` 114.

### 9.8 Fifth fix: MEI `anyStart` documents

Two of the 130 validated files (`docStarts/Doc_starts_with_meiHead.mei`,
`Doc_starts_with_music.mei`) were schema-valid MEI 5.1 but rejected because
`read_mei` required root `<mei>`. The RelaxNG schema `mei-all_anyStart.rng`
(and its Schematron rule at line 21588) allows `mei`, `meiHead`, `meiCorpus`
and `music` as document roots. The reader now accepts these four roots (or
`@meiversion="5.1+anyStart"`) and `readMeiHead` uses the root itself when it is
`<meiHead>`. This is a measured extension beyond MuseScore 4.7.4, whose
importer starts at `<mei>` only.

The measurement tests now assert **130 files read, 0 skipped, 0 unreadable**;
the full support stays at **99.43 %**. The 37 header fragments remain in
`EXCLUDED.tsv` (no `<score>`) and are still read by `mei_samples_never_panic`.

### 9.9 Sixth fix: `@curvedir` from the import-completeness audit

The audit's top ignored attribute was `@curvedir` on `<slur>` (9 870) and
`<tie>` (3 835): MEI 5.1 `att.curvature` expresses an explicit direction that
the pivot can hold (`Slur::placement`, `Note::tie_up`). MuseScore 4.7.4 reads
it with `Convert::curvedirFromMEI` (meiconverter.cpp:1112) and writes it with
`curvedirToMEI` (`:1123`). The port now maps `above`/`below` on slurs and ties
in both directions, including slurs whose `@endid` is resolved at the end of
the score, and the audit marks the attribute as mapped.

| Import completeness (130 files) | Before | After |
| --- | ---: | ---: |
| Attributes ignored | 65 552 (5.02 %) | 51 838 (3.97 %) |
| Elements ignored | 19 681 (4.25 %) | 19 681 (unchanged) |

The round-trip figure stays at 99.43 % (the inventory does not key slur/tie
placement), but the reader now consumes an additional 13 714 MEI attributes
that were previously ignored. Unit tests cover a tie and a slur with opposite
`@curvedir` values.

### 9.10 Where the campaign stopped

After six measured fixes the round trip on the 130 external encodings is
**99.43 %** (128 files at the first two steps, 130 after `anyStart`), against
77.64 % at the baseline. On the MuseScore corpus the same fixes raised the
round trip from 95.03 % to **95.14 %** and the import from 87.11 % to
**87.16 %** (reference-bound 95.60 % → **95.66 %**): no figure regressed.
The remaining per-kind losses are `articulation` 376,
`note` 329, `slur` 269, `stem` 219, `rest` 114 and `tuplet-member` 114; they
are spread across many small files and are not a single dominant cause as the
first six were. The largest ignored *elements* are header prose (`p` 1 674,
`change`/`changeDesc` 1 392 each), `tupletSpan` 1 467, and metadata containers;
mapping those would require a model slot for editorial markup or a full
span-based tuplet reader, neither of which MuseScore 4.7.4 has. No declaration
was narrowed and no item was removed from the denominator at any step.

## 10. Additive MEI 5.1 carriers (2026-11-26, branch `mei2`)

### 10.1 Option and method

The 4.7.4 exporter writes only a subset of what MEI 5.1 can carry. This
campaign adds the missing carriers behind a single **opt-in** switch:
`Options::mei_full_support` in the facade, `MeiOptions::full_support` in
`scoretk-mei-export`. **Default is `false`**: the bytes written with
`Options::default()` stay 4.7.4-faithful (`mei_conformity` is unchanged at
3 913/4 016 = 97.44 %), and `mei_full_support_residual` /
`mei_samples_full_support_residual` measure the same round trips with the
switch on. Each step was one commit with unit tests, followed by
`fmt --check`, `clippy -D warnings`, `test --workspace --no-fail-fast`,
`e2e_corpus` 8/8 and the `scoretk` wasm build. No declaration was narrowed
and no item was removed from a denominator.

The switch only changes emitted bytes; the reader always accepts the new
carriers, so importing a 4.7.4 reference is unaffected except for genuine
reader fixes (below).

### 10.2 Results

| Measurement | Before (§9.10) | After, default 4.7.4 | After, `mei_full_support` |
| --- | ---: | ---: | ---: |
| MuseScore corpus round trip (56 scores, 5 879 items) | 95.14 % | **95.58 %** | **99.06 %** (55 missing) |
| Official MEI 5.1 samples round trip (130 files) | 99.43 % | **99.75 %** | **99.87 %** |
| Import of MuseScore `*.ref.mei` (main figure) | 87.16 % | **87.53 %** | unchanged (reference-bound) |
| Import, carriers MuseScore never writes excluded | 95.66 % | **96.08 %** | unchanged |

The 99.9 % target is reached **neither** row: the external samples are
closest at 99.87 % with the option, the corpus at 99.06 %. Both import
figures are kept: 87.53 % is the honest main figure against MuseScore's own
references, 96.08 % the additive figure that excludes only the 628 reference
items whose carrier is absent from those references (`style` 380,
`dynamic-velocity` 43, `note-head` 41, `credit` 15, `credit-position` 18,
`tbox-text` 15, `tbox-position` 15).

### 10.3 Per-kind corpus residual, before → after (`mei_full_support`)

| Kind | Before | After | Fix |
| --- | ---: | ---: | --- |
| `dynamic-velocity` | 43 | 0 | `<dynam @val>` (Guidelines §"Dynamics": `<dynam val="84">f</dynam>`; `att.dynam.ges` → `att.midiValue`), control events anchored on `xml:id`. |
| `note` | 25 | 0 | `@trans.diat`/`@trans.semi` on a mid-score `<scoreDef>`; written pitch class derived from concert pitch − transposition. |
| `slur` | 19 | 0 | `@startid`/`@endid` on note ids, including notes inside `<chord>` (registered in `id_events`). |
| `credit` / `credit-position` | 15 / 18 | 2 / 0 | Ordered `<rend>` with `@n`, standard `@halign`/`@valign`/`@fontsize`, `@label="mscore-frame-…"` for page/frame height. |
| `tbox-text` / `tbox-position` | 15 / 15 | 4 / 0 | `<dir type="mscore-tbox-…">` with `@label` carrying measure/placement/font size. |
| `instrument` / `program` / `volume` / `channel` / `pan` | 15 / 5 / 3 / 2 / 1 | 0 | `<instrDef @midi.instrnum/@midi.channel/@midi.volume/@midi.pan>` and `@type="mscore-instrument-…"`. |
| `grace` / `grace-note` | 8 / 8 | 0 / 4 | `<graceGrp @grace @dur type="mscore-grace-…">` with child note pitches. |
| `text-line` | 7 | 0 | `<dir type="mscore-text-line">` with `@startid`/`@endid`. |
| `hairpin` | 6 | 2 | Spanners anchored on the `xml:id` of rests as well as notes. |
| `arpeggio` / `ornament` / `breath` | 5 / 4 / 2 | 0 | `<arpeg>`, `<ornam type="mscore-ornament-…">`, `<breath>/<caesura type="mscore-breath-…">`. |
| `title` (`work_number`) | 3 | 0 | `<title type="work-number">`. |
| `line-break` | 3 | 0 | Trailing `<sb>`/`<pb>` after the final measure. |
| `clef` | 13 | 13 | see §10.5. |
| `multi-measure-rest` | 6 | 6 | see §10.5. |
| `figured-bass` / `harmony` | 4 / 4 | 4 / 4 | see §10.5. |

The remaining 55 items are listed by `mei_full_support_loss_diagnostic`;
the largest groups are `clef` 13, `multi-measure-rest` 6, `figured-bass` 4,
`harmony` 4, `tbox-text` 4, `tie` 3 and 17 singleton kinds.

### 10.4 Per-kind sample residual, before → after (`mei_full_support`)

| Kind | Before (§9.7) | After, default | After, option |
| --- | ---: | ---: | ---: |
| `articulation` | 376 | 443 | 0 |
| `note` | 329 | 18 | 18 |
| `slur` | 269 | 6 | 4 |
| `stem` | 219 | 0 | 0 |
| `rest` | 114 | 0 | 0 |
| `tuplet-member` | 114 | 96 | 96 |
| `lyric` | 48 | 0 | 0 |
| `time-sig` | 41 | 36 | 36 |
| `dynamic` | 85 | 85 | 75 |
| `staff-text` | 17 | 11 | 73 |

The dominant cause of the 2026-11-25 residuals (`stem`, `rest`, `note`,
`lyric`) was one structural error: several official encodings put the initial
`<scoreDef>` **inside a `<section>`**. The port now treats the first
`<scoreDef>` seen inside a section as initial when no staff has been built
yet. `articulation` required two fixes: `att.articulation/@artic` on
`<note>`/`<chord>` and an explicit `@startid` on `<artic>` (an emitted
`<chord>` has no `xml:id`, so the reader otherwise anchored the annotation on
the wrong staff). The `staff-text`, `dynamic`, `fermata` and
`rehearsal-mark` losses on the extended row are current reader/anchor
differences, not exclusions.

### 10.5 What remains, and why nothing was excluded

No kind was removed from the declaration: every residual kind in §10.3/§10.4
is a **current implementation or model limitation**, not a claim that MEI
5.1 lacks a carrier. In particular:

- The 13 `clef` items are *duplicate* clef segments at the same tick and
  staff (a MuseScore default clef and the actual clef). MEI can write both
  (one in `<staffDef>`, one inside `<layer>`), but the pivot's `Segment`
  multiset holds one `Clef` per tick/track, so the second fact cannot be
  represented in the model today. This is a model gap, not a format gap.
- `figured-bass`, `harmony`, `tbox-text`, `tie` and `multi-measure-rest`
  residuals are anchoring/serialisation gaps in the current port; their
  MEI 5.1 carriers (`<harm>`, `<dir>`, `<tie @startid/@endid>`,
  `<multiRest>`) exist and are already read from other files.
- The `@type="mscore-*"` values used above are documented MuseScore-style
  extensions of standard elements, not a private binary payload: numeric
  data uses standard attributes (`@val`, `@midi.*`, `@fontsize`, `@halign`,
  `@valign`, `@dur`, `@n`).

Only items MEI 5.1 genuinely cannot represent may ever be removed from the
declaration (with an RNG/Guidelines citation); none were removed here.
## 11. The NEUMES 5.1 module for Gregorian chant (branch `meineume`)

MEI 5.1 is complete only with its **Neume Notation** module (*Guidelines*
5.1, ch. 6 "Repertoire: Neume Notation", read online on 2026-11-26 at
`https://music-encoding.org/guidelines/v5/content/neumes.html`: the local
copy under `specs/mei/` contains only the index, the introduction and the CMN;
the normative local schema is `mei-all-5.1.rng`, ECL-2.0, not `mei-basic`).
MuseScore 4.7.4 neither writes nor reads this module: the preceding sections
measure its **CMN**. The neume module is an additive extension of the port,
with no change to the CMN path.

### 11.1 Standard carriers, by RNG element

| GABC fact (L6) | MEI 5.1 carrier | Citation |
| --- | --- | --- |
| Syllable, words and glyph groups | `<section>/<staff>/<layer>/<syllable>(<syl>, <neume>/<nc>)`; one `<neume>` per GABC group (cuts `/`, `//`, `/0`, `!`) and `@type` = neume name (`punctum`, `pes`, `clivis`, `torculus`, …) | *Guidelines* §6.1, §6.1.1, §6.4.10; RNG `att.neumeType/@type` (20 suggested values + NMTOKEN) |
| Pitch, case and stroke shape | `<nc @pname @oct>`, `@tilt` (stroke direction), `@curve`, `@con` = `g`/`l`/`e`, `@intm`; the inclinatum (uppercase GABC letter) is `@tilt="se"` and `@type="punctum-inclinatum"` | §6.4.2 (`att.nc.log`, `att.ncForm`, `att.curvatureDirection`); RNG `att.ncForm.attribute.con` (`g` gapped, `l` looped, `e` extended) |
| Oriscus, quilisma, liquescent, strophicus | children of `<nc>`; variants via `@looped`/`@curve` on `<liquescent>`, `@waves` on `<quilisma>`, `@n` on `<strophicus>` | §6.4.1, §6.4.5, §6.4.7; RNG `mei_liquescent`, `mei_quilisma`, `mei_strophicus` |
| Horizontal/vertical episema, ictus, mora, virga, cavum, alteration, line | `<episema form="h\|v" place>` for `_`/`'`; `<signifLet type="PunctumMora\|Virga\|Cavum\|Accent\|Alteration\|Linea" n place>` for signs without a dedicated element | §6.4.4, §6.4.8, §6.4.9 ("other articulation marks"); RNG `att.episema.vis/@place` (`data.EVENTREL`) |
| Significant NABC letters (`su1`, `st2`, `ta`, …) and placement digits | text of `<signifLet>` with `@type="gabc-nabc-letter"` and `@place`, on the **second staff** | §6.4.1, §6.4.8 ("significant letter(s) attached to a neume or a nc"); RNG `mei_signifLet` (free text content); full table in `format-mapping/mei.md`, implementation in §13 |
| Separation bars | `<divLine form>`: `virgula`, `caesura`, `minima`, `minor`, `maior`, `finalis` and additional NMTOKENs (`virgula-ledger`, `minima-ledger`, `maior-dotted`, `dominican-1`…`dominican-8`) | §6.4.1; RNG `att.divLine.log/@form` (six suggested values **and** `data.NMTOKEN`) |
| Clef, bass clef, line count | `<staffDef @lines>`, `<clef shape="C\|F" line="n">`, GABC flat clef `cbN` → `@type="clef-flat"` | §4.2.2, §6.4.10; RNG `mei_clef` (`att.clef.log`, `att.typed/@type`) |
| Custos | `<custos @pname @oct>` (not emitted: the L6 payload has no custos field, `gabc.md` §10) | §6.4.3 (`att.custos`) |
| Headers | `name` → `fileDesc/titleStmt/title`; `author`/`transcriber` → `respStmt/persName @role`; `office-part`/`book` → `workList/work/title @type`; `mode`/`annotation`/`occasion`/`style` → `classification/termList/term @type`; the others → `sourceDesc/source/bibl/rend @type`. `@n` keeps the GABC field order. | *Guidelines* ch. 3; RNG `mei_fileDesc`, `mei_titleStmt`, `mei_respStmt`, `mei_workList`, `mei_classification`, `mei_sourceDesc` |
| End of word, elision, text style | `<syl @wordpos @con>` (`s` = space, `t` = elision tilde); `<rend fontstyle="italic"\|fontweight="bold"\|color>` for `<i>`, `<b>`, `<c>` etc. | §6.1.1, §6.4.10 (example `<syl n="initial"><rend color="red">O</rend></syl>`); RNG `att.syl.log/@wordpos`, `@con`, `mei_rend` |

The punctum mora, the virga, the cavum, unsung alteration (`x`, `#`,
`y`, `Gabc.tex:512-540`) and placement digits have no element in the neume
module: they are encoded as *significant letters* (`<signifLet>`), which §6.4.9
explicitly provides for marks that are not neumes. No `gabc-*` annotation is
needed to read them: standard attributes (`@type`, `@n`, `@place`, `@form`,
`@curve`) are converted back to GABC tokens by `neumes::glyph_parts` /
`part_from_mei`.

### 11.2 Files and wiring

| File | Role |
| --- | --- |
| `crates/scoretk-mei-common/src/neumes.rs` | clef/letter/bar tables, decomposition of glyphs into MEI elements, inverse, `<rend>` for styles |
| `crates/scoretk-mei-export/src/neumes.rs` | MEI 5.1 `mei-all` writer of the L6 payload, **without annotation by default** |
| `crates/scoretk-mei-import/src/neumes.rs` | reads the neume module, rebuilds the payload and the derived view |
| `crates/scoretk/tests/gabc_mei_neumes.rs` | structural tests, absence of private annotation, unchanged CMN |
| `crates/scoretk/tests/layer_coverage.rs` | standard measurement line, line "with private extension" second |
| `scripts/check-mei-neumes.sh` | RelaxNG validation of the nine outputs with `jing` against `mei-all-5.1.rng` |

`MeiOptions::gabc_tokens` (false by default) enables the `<annot type="gabc-*">`
of the previous version: they duplicated every atomic token (glyph, sign, bar,
clef, header, source offset) and no longer help the standard reader. CMN wiring
is unchanged: `mod neumes;` in both crates, early return in `writer::write` for
`Extension::Gregorian` and in `read_mei` for documents containing `<syllable>`/
`<neume>`. Corpus `.mscx` files still produce a `5.1+basic` MEI with `<measure>`
(test `cmn_scores_still_use_the_unchanged_mei_basic_path`).

### 11.3 Measurements (nine Gregorio examples, 3 322 items)

Commands: `cargo test -p scoretk --test layer_coverage gabc_mei -- --nocapture`
and `cargo test -p scoretk --test gabc_mei_neumes`. The **first** line of the
printed table is the standard figure: the MEI is re-imported as-is, without
private annotation. The second only measures what the extension used to add.

| Line | Essential | Full support | Detail |
| --- | ---: | ---: | --- |
| **Before** (no module, derived CMN view) | 94.14 % | **66.35 %** | L1 73.33 % (99/135), L2 91.93 % (1 036/1 127), L3 74.68 % (230/308), L6 47.89 % (839/1 752) |
| **After, without `gabc-*` (retained figure)** | **100.00 %** | **100.00 %** | L1 135/135, L2 1 127/1 127, L3 308/308, L6 1 752/1 752, 0 extra; 9/9 outputs differ from their source |
| **After, with private extension** (secondary) | 100.00 % | 100.00 % | same coverage, but 35 headers, 1 033 glyphs, 290 signs and 61 bars are still duplicated in `<annot type="gabc-*">`: this figure is not an interoperability figure |
| Intermediate step (neume module without standard carriers, 2026-11-25) | 27.54 % | 49.37 % | L6 72.66 %, L1 19.26 %, L2 26.97 %, L3 12.01 %; this is the **49.37 %** of the verdict, now superseded |

The test `standard_mei_neumes_carry_the_payload_without_any_private_annotation`
checks the nine files for the absence of `<annot type="gabc-` and the
field-by-field equality of the L6 payload (headers, clefs, syllables, letters,
glyphs, signs, bars, spaces), except for source UTF-16 offsets.
`gabc_crosses_mei_with_the_whole_structured_payload` checks **total** equality,
offsets included, when the extension is enabled.
`gabc_mei_round_trip_is_measured` forbids copying the source.

Exact source layout (`Gregorian::opening_separator`,
`Syllable::separator_after`) and the NABC cut resumption use the standard
`@label` attribute (`att.labelled/@label`, `data.string`) on `<layer>`, `<syl>`
and `<signifLet type="gabc-nabc">` respectively, encoded as
`layout:<kind>:<S|L|T|R|UXXXX|E>` by
`scoretk-mei-common::neumes::{layout_label, layout_from_label}`. No private
`<annot type="gabc-*">` is involved. Without that attribute, the reader falls
back to `wordpos` for the separator and leaves the other two fields empty,
which reproduces the pre-branch defaults. See `gabc.md` §19.

#### What still has no MEI 5.1 carrier (by kind)

| Kind / fact | Standard carrier | Cited limit |
| --- | --- | --- |
| `gregorian-note` (letter + glyph) | `<nc @pname @oct @tilt @curve @type>` + `<liquescent>`, `<oriscus>`, `<quilisma>`, `<strophicus>`, `<signifLet>`, `<episema>` | No loss on the nine examples; §6.3 notes that regional styles do not reduce to a single notation, so reversibility is specific to the documented GABC↔MEI convention, not a universal guarantee |
| `gregorian-sign` (`.`, `..`, `_502`, `r6`, `x?`, `>`…) | `<signifLet @type @n @place>` / `<episema @form @place @n>` | The module defines no `<punctumMora>` or `<cavum>` element; §6.4.9 refers to articulation marks and `glyph.*`, we use `<signifLet>` |
| `gregorian-bar` (`` ` ``, `,`, `;`, `:`, `::`, `;1`…`;8`) | `<divLine @form>` + NMTOKEN | RNG: only six suggested values, free NMTOKEN for variants; the exact variant name is therefore a convention of this port |
| `gregorian-header` (35 fields) | `titleStmt`, `respStmt`, `workList`, `classification`, `sourceDesc/bibl/rend` | `<bibl>` does not accept `<p>` (RNG `mei_bibl`): free fields go through `<rend @type>` |
| `gregorian-clef` (`c3`, `f3`, `cb3`) | `<clef @shape @line @type="clef-flat">` | — |
| `gregorian-clef` double `c1@c4` | **none** | `<clef>` is unique; RNG `mei_clefGrp` is not allowed in `<staffDef>`: only the first clef is written, the other remains in the private extension |
| `gregorian-above-text`, `gregorian-rubric` | `<signifLet type="gabc-above" place="above">`; rubric kept in the text of `<syl>` | — |
| `space_after`, elisions | `<syl @wordpos @con>` | RNG `att.syl.log/@con` (`s` space, `t` tilde) |
| Styles `<b>`, `<i>`, `<c>`, `<sc>`, `<tt>`, `<ul>` | `<rend>` + `@fontstyle`/`@fontweight`/`@color`/`@fontfam` | Only these reversible styles are converted; `<sp>`, `<v>`, `<e>`, `<clear>`, `<pr>` remain exact text in `<syl>` (§6.4.10 shows `<rend>` but defines no GABC mapping) |
| `custos`, `[…]` layout, `z`/`Z` line breaks | **none in the L6 payload** | `gabc.md` §10: these facts are already absent from the model (MEI has `<custos>`, §6.4.3, but nothing to feed it). NABC cuts *are* carried since §13: `Syllable::nabc` + a second staff. |
| UTF-16 offsets `source_offset`/`source_end` | **none** | Tracking attribute for the wasm client (`timemap.md`), not a musical MEI fact; kept only by the private extension and excluded from the compared inventory |

No element was removed from the denominator and no layer declaration was
reduced to obtain the standard figure.

### 11.4 RelaxNG validation

```sh
nix develop -c cargo build -p scoretk-cli
scripts/check-mei-neumes.sh
# MEI 5.1 neumes : 9 fichier(s) valide(s) contre mei-all-5.1.rng
```

`jing` comes from `nix shell nixpkgs#jing`; the schema is the local copy at
`specs/mei/mei-all-5.1.rng` (ECL-2.0). No CMN MEI changed: the conformity
figures of §3, §7 and §9 remain those of the 4.7.4 path.

Wasm package size (`nix build .#wasm`, `result/SIZE`): **2 740 634 raw bytes /
1 068 351 gzip**. Commit `3ea3dd73` (initial neume module) was at 2 714 783 /
1 058 298, i.e. **+25 851 raw (+0.95 %)** and **+10 053 gzip (+0.95 %)** for the
standard carriers and the `<rend>` styles; compared with the last figure
documented before the module (2 558 465 / 1 006 608), the module adds a total
of +182 169 raw / +61 743 gzip.

## 12. Full-support coverage campaign (2026-11-26, branch `mei3`)

### 12.1 Method and headline figures

`Options::mei_full_support` / `MeiOptions::full_support` stays opt-in and
**false** by default. The headline is measured on MEI 5.1-valid output only:
after the rebase on `master` and the consolidation below, the 56 additive
exports validate **56/56** against `mei-all-5.1.rng` with `jing`, and the full
support round trip remains **100.00 %** (0/5 879 missing). A coverage figure
obtained from schema-invalid MEI does not count.

The default path was brought back to **byte identity with `master`**: two
carriers of this campaign (the `<fb><f>` figure and the tremolo-bar `<dir>`)
had leaked into the default writer and made `v3/layout_elements` differ from
`master`. They are now behind `full_support`, as they were meant to be. The
default round trip consequently reads **96.26 %** (not 96.34 % — the old figure
relied on those extra bytes). The default bytes are unchanged from `master`,
verified by exporting all 56 scores with both binaries and diffing.

| Measurement | Before this pass | After this pass | MEI 5.1 validity |
| --- | ---: | ---: | --- |
| Round trip, default export (56 scores, 5 879 items) | 95.63 % | **96.26 %** (bytes identical to `master`) | 0/56 against its declared `mei-basic` (all inherited from `master`, see §12.3) |
| Round trip, `mei_full_support` | 99.51 % (29 missing) | **100.00 %** (0/5 879) | **56/56** against `mei-all-5.1.rng` |
| Import of MuseScore `*.ref.mei` (main figure) | 87.58 % | **87.80 %** | references, not measured here |
| Import, carriers MuseScore never writes excluded | 96.13 % | **96.38 %** | references, not measured here |

The reader fixes (grace duration on the child note, `@wordpos="m"`,
`@startid`-free `<artic>`, exact control-event anchoring, `<staffGrp>` around
mid-score `<staffDef>`, `<dir>`-anchored two-chord tremolos) also apply to
`master`-style references and raise both import figures slightly. No kind was
removed from the declaration, no denominator was reduced and no private
`<annot>` was introduced; each carrier is a standard MEI 5.1 element or
attribute, cited in §12.3.

### 12.2 Per-kind residual, before → after (`mei_full_support`)

| Kind | Before | After | Standard carrier and citation |
| --- | ---: | ---: | --- |
| `clef` | 13 | 0 | `<staffDef>/<clef>` and a second `<clef>` inside `<layer>` at the same tick: RNG `mei_staffDef`, `mei_clef`, `att.clef.log` (Guidelines §4.2.2). The pivot keeps the two facts in two segments; the reader no longer overwrites one with the other. |
| `tie`, `note`, `articulation` (grace/id anchoring) | 3 + 1 + 1 | 0 | `xml:id` (`att.common`) on grace-note `<note>` and `<tie @startid/@endid>` (RNG `att.startEndId`, Guidelines §10): grace notes now carry the model note id. |
| `note` (breath at the same tick) | 1 | 0 | `<breath>`/`<caesura>` is a separate control event (`mei_breath`, Guidelines §10, `att.timestamp`): the reader adds its own segment instead of replacing the chord rest. |
| `circle`/`stamp` rounding (`clef`, `rest`, `tremolo-bar`, `mRpt`) | 6 | 0 | `att.timestamp/@tstamp` is `data.NUMBER`; nine decimals let `Convert::tstampToFraction`'s continued-fraction reader recover exact thirds, and the additive writer uses the same denominator as that reader. |
| `credit`, `tbox-text`, `title` (trailing spaces) | 6 | 0 | `<rend @n>`, `<title type="work-number">`, `<dir type="mscore-tbox-*">`: RNG `att.common/@n`, `att.typed/@type`, `att.labels.addr/@label`; the additive text reader keeps the trailing whitespace that the pivot stores. |
| `multi-measure-rest`, `measure-repeat` | 6 + 1 | 0 | `<multiRest @n>` and `<mRpt @n>`: RNG `att.numbered` (`mei_multiRest`, `mei_mRpt`); unique `xml:id` per track, `@tstamp` for the symbol at measure start. |
| `barline`, `barline-end` | 2 | 0 | `<barLine @form="dashed">`, RNG `mei_barLine` + `data.BARRENDITION` (§5): disambiguates MuseScore's lossy `@right="dotted"`. |
| `jump`, `marker` | 2 | 0 | `att.staff/@staff` on `<repeatMark>` (§10, `mei_repeatMark`): the marker and jump of each staff keep their own anchor. |
| `time-sig` (local signature) | 1 | 0 | `<scoreDef><staffGrp><staffDef @meter.count/@meter.unit>` (RNG `mei_staffDef`, Guidelines §4.2.2), plus global `scoreDef` precedence over the initial `staffDef`. |
| `chord-line`, `tremolo-bar` | 2 | 0 | `<dir>` control event (§10, RNG `mei_dir`, `att.typed/@type`): `mscore-chordline-*` and `mscore-tremolo-bar-*` variants, the curve points in `<dir>` text. |
| `figured-bass` | 4 | 0 | `<harm><fb><f>` (§10, RNG `mei_harm`, `mei_fb`, `mei_f`; `mei_fb` is only admissible as a child of `mei_harm`): `prefix/digit/suffix/continuation/brackets` per item, `;` between items, in the text of `<f>`. |
| `articulation`, `note`, `rest`, `chord-line`, `tremolo-bar` (reader only) | 5 | 0 | `att.startEndId` anchors and inline measure control events: the reader now reads the additive carriers written by this port and no longer erases an event at the same tick. |

No residual coverage item remains. The `mscore-*` values are documented
MuseScore extensions of *standard* attributes/elements, not a private binary
payload; all numeric data uses standard attributes (`@val`, `@n`, `@staff`,
`@midi.*`, `@fontsize`, `@tstamp`, `@form`).

The default residual after restoring `master`'s bytes is 220 missing items out
of 5 879. `dynamic-velocity` 43 is a full-support-only carrier; the remaining
default figure is dominated by `note` 25, `credit-position` 18, `credit` 15,
`tbox-text` 15, `instrument` 15, `tbox-position` 15, `grace`/`grace-note` 8+8,
`multi-measure-rest` 6, `clef` 5, `arpeggio` 5, `program` 5, `ornament` 4,
`figured-bass` 4 and `instrument-change` 3 (plus small singletons). These are
the fidelity limits of the 4.7.4 path, printed by `mei_full_loss_table`; none
of them is hidden by changing a declaration.

### 12.3 Reproducible RelaxNG validation (both modes)

From the repository root, with no arguments:

```sh
scripts/oracle/mei/valide-corpus.sh
```

The wrapper supplies `jing` (`nix shell nixpkgs#jing`), builds the CLI if
needed, exports the 56 `.mscx` scores in both modes, and calls
`scripts/oracle/mei/valide-corpus.py`. The schemas are the local MEI 5.1 RNG
copies in `$SCORETK_SPECS_DIR/mei/` (default
`/home/jacques/programmes/musique/specs`): `mei-basic-5.1.rng` for the default
mode, which declares `meiversion="5.1+basic"`, and `mei-all-5.1.rng` for
`mei_full_support`, which declares `meiversion="5.1"`. Neither schema is
vendored; the basic customization is fetched from
`https://music-encoding.org/schema/5.1/mei-basic.rng` into
`$SCORETK_SPECS_DIR/mei/mei-basic-5.1.rng` (ECL-2.0), exactly as the all schema
is fetched in §9. The wrapper exits non-zero only when the headline mode
fails.

| Mode | Result |
| --- | ---: |
| Default, byte-identical to `master`, against `mei-basic-5.1.rng` | **0/56 valid** (56 invalid) |
| `mei_full_support`, against `mei-all-5.1.rng` | **56/56 valid** (0 invalid) |

### 12.3.1 Schematron (the assertions embedded in the RNG)

`jing` checks only the RelaxNG grammar. The normative
`mei-all-5.1.rng` also embeds **249 ISO Schematron assertions**
(184 rule contexts, 248 `sch:assert` and one `sch:report`) which it does
not evaluate. They can be run: `scripts/oracle/mei/schematron.py` extracts
them from the local RNG, generates a self-contained XSLT 3.0 checker
(**no downloaded stylesheet is executed**), and applies it with Saxon HE 11
(`nixpkgs#saxon_11-he`, a jar, plus `nixpkgs#jre`). Add `--schematron` to
`scripts/oracle/mei/valide-corpus.sh` (which supplies `--out`) to run both
levels; warnings are advisory and errors fail the oracle. `--selftest`
checks the extraction without Java.

The first NABC MWE of §13 declared staff 2 as a `<staffDef>` directly under
`<scoreDef>` with no `@lines`. It passed `jing` but raised exactly **one**
Schematron error: *"Either @lines must be present or a preceding staffDef with
the same value for @n and @lines must exist"* (rule
`mei:staffDef[not(ancestor::mei:staff)]`, RNG :28068–28077). The tension is
real: the RNG types `@lines` as `positiveInteger`
(`mei_att.staffDef.log.attribute.lines`, :22738), so `lines="0"` is invalid;
the *Guidelines* define `@lines` as "Indicates the number of staff lines" and
`@lines.visible` as "Records whether all staff lines are visible"
(<https://music-encoding.org/guidelines/v5/elements/staffDef.html>). Three
valid carriers were investigated on the three NABC outputs:

1. **`lines="1" lines.visible="false"`** on the `<scoreDef>` `<staffDef>`
   (`mei_att.staffDef.vis.attribute.lines.visible`, :31111–31117) passes both
   validators (3/3 RNG, 0 errors, 0 warnings), but declares a notional
   one-line staff and then hides its line. The *Guidelines* treat `@lines` as
   a count, not a placeholder, so this invents a geometry that the
   adiastematic manuscript does not have.
2. **Declare the manuscript staff in place**: `<staff n="2"><staffDef n="2"
   notationtype="neume" label="manuscript"/><layer …>`. RNG `mei_staff`
   (:28007) documents a `<staff>` as covering "both diastematic and
   non-diastematic signs" and admits `mei_model.staffDefLike` as a child;
   Schematron `mei-all-staff-checkStaff_n` (:28025–28031) explicitly lists
   "a staffDef child element" as an acceptable staff declaration, and rule
   169 (:28084) only requires the child's `@n` to match its `<staff>`. The
   *Guidelines* §4.2.2 "Re-definition of Score Parameters" lists the usual
   `<staffGrp>` location and adds: "It is also possible to include `scoreDef`
   and `staffDef` in staves and layers when the MEI All schema is in use;
   however, this practice is not recommended for the CMN repertoire"
   (<https://music-encoding.org/guidelines/v5/content/cmn.html#cmnReDef>).
   A St. Gall/Laon manuscript layer is precisely not CMN, so this sanctioned
   MEI All carrier applies; no `@lines` and no notional geometry.
3. **The Guidelines' own examples** cannot supply a full-document carrier:
   the adiastematic/St. Gall examples of §6.3 Listings 244–247 and 253
   (<https://music-encoding.org/guidelines/v5/content/neumes.html#neumesNotation>)
   are `<neume>/<nc>` fragments with unpitched `<nc>` and `@intm`, but no
   `<staff>` or `<staffDef>` at all, so there is no staff declaration for a
   Schematron rule to check. §6.4.10 even says of its "basic MEI skeleton"
   that "Information about the staff has been omitted for brevity"
   (same page, `#neumesExamples`). They justify the unpitched `<nc>` used
   here, not an invalid document.

Carrier 2 is adopted: the writer emits the staff-level `<staffDef>` and no
`@lines`. It measures **3/3 RNG valid, 0 Schematron errors, 0 warnings**;
the 56 CMN full-support exports remain at 56/56 RNG and 0 Schematron errors.
The reader never consults `@lines` for NABC, so it stays tolerant of the old
MWE form (no `@lines`), of third-party `lines="1" lines.visible="false"`
staves and of other declared line counts; unit tests cover all three. There
is no longer a declared exception: **0 RNG + 0 Schematron errors on all 59
`mei-all` outputs** (3 NABC + 56 CMN full-support). The default mode remains
byte-identical to `master` and keeps its 56 declared `mei-basic` violations
(reported below), which are outside this fix.

The default result is reported, not hidden: it is unchanged from `master`'s
output and is measured with `master`'s binary too. Its 56 failures are
4.7.4-faithful attributes outside `mei-basic` (`page.*`/`@vu.height` on the
mid-score `<scoreDef>`, `@tstamp` on `<note>`/`<chord>` children of `<tuplet>`,
`head.shape` on `<note>` inside a chord, `<fTrem>` with one child, `<pedal>`
without `@dir`); the byte-identity requirement forbids repairing them here.
The earlier figure of "44/56 valid" in the additive mode is superseded: 12
files were invalid, all now fixed while keeping information on a valid carrier.

| File (previously invalid) | jing error and RNG citation | Fix in the additive path |
| --- | --- | --- |
| `v3/layout_elements`, `v4/changevisibility`, `v4/copypaste_parts`, `v4/linked-ties` | `@dur` on `<graceGrp>`: RNG `mei_graceGrp` admits only `att.graceGrp`, not `att.duration.musical` | `@dur`/`@dots` written on the child `<note>`; `read_grace_group` falls back to the first note's duration |
| `v3/moonlight`, `v3/testWedge4` | `@visible="false"` on `<rest>`: RNG `mei_rest` has no `att.visibility` | `<space>` (`mei_space`, Guidelines "Rest"), read back as an invisible rest; default keeps `@visible` |
| `v3/layout_elements`, `v4/testExcludeInvisibleElements` | `<syl con="m">`: RNG `data.SYLLABIC` = `s d u t c v i b` (no `m`) | `@wordpos="m"` (RNG `att.syl.log/@wordpos`, medial syllable); the reader maps it to `LyricsSyllabic::Middle` |
| `v3/layout_elements`, `v4/copypaste_parts`, `v4/testExcludeInvisibleElements` | `<arpeg>`/`<ornam>` inside `<note>`/`<chord>`: `mei_arpeg`/`mei_ornam` are `model.controlEventLike`, not children of an event | measure-level control events with `att.startId`/`att.startEndId`; the reader anchors them exactly on the note id |
| `v3/timesig-05`, `v3/voices`, `v4/changevisibility`, `v4/copypaste_parts`, `v4/tenuto_accent`, `v4/testExcludeInvisibleElements` | `@startid` on `<artic>`: RNG `mei_artic` has no `att.startEndId` | the parent event's `xml:id` (or the first note of a `<chord>`) anchors the articulation |
| `v3/layout_elements`, `v4/copypaste_parts`, `v4/testExcludeInvisibleElements` | `<pedal>` without `@dir`: RNG `att.pedal.log/@dir` is required (`down`/`up`/`half`/`bounce`) | `<pedal dir="down" … endid="…">` in the additive mode |
| `v3/testMultiInstrumentPart3` | `<staffDef>` directly under `<scoreDef>`: RNG `mei_scoreDef` only admits `<staffGrp>`; `@trans.*` on `<dir>` belongs to `att.staffDef.transposition` | `<scoreDef><staffGrp><staffDef @trans.diat/@trans.semi>` before the measure; the `<dir>` keeps text, `@type` and `@label` |
| `v4/copypaste_parts`, `v4/timesig-09` | `<fTrem>` with one child: RNG `mei_fTrem` requires two `chord`/`note` children | `<dir type="mscore-tremolo-two-chord-Start\|Stop" @startid>` control event; the reader restores `TremoloKind` and `tremolo_two_chord` |

Every one of those 12 files now validates, and the full-support round trip is
still 100.00 % on that valid output: no coverage item was sacrificed to gain
validity. The `mscore-*` values remain documented MuseScore extensions of
standard elements/attributes, not private payloads. `nix build .#wasm` after
the consolidation gives **2 825 964 raw bytes / 1 103 570 gzip** (§11's figure
predates the full-support campaigns).

## 13. NABC manuscript neumes as a second staff (2026-11-26, branch `meinabc`)

### 13.1 Encoding

Gregorio's NABC snippets (`Gabc.tex:129`, `GregorioNabcRef.tex:174-199`,
`gabc-score-determination.y:870-907`) are the *manuscript* voice of a
syllable (St. Gall or Laon adiastematic neumes), parallel to the square
notes. They are no longer opaque text in the square staff: with
`MeiOptions::full_support` **only**, the writer emits a second, parallel,
unpitched `<staff n="2">` as validated against `mei-all-5.1.rng` with `jing`:

```xml
<scoreDef><staffGrp>
  <staffDef n="1" lines="4" notationtype="neume" label="square"><clef shape="C" line="3"/></staffDef>
</staffGrp></scoreDef>
<section>
  <staff n="1"><layer n="1"><syllable xml:id="s1a" corresp="#s1b">
    <syl>FAc</syl><neume type="punctum"><nc pname="g" oct="3"/></neume>
  </syllable></layer></staff>
  <staff n="2">
    <staffDef n="2" notationtype="neume" label="manuscript"/>
    <layer n="1"><syllable xml:id="s1b" corresp="#s1a">
      <syl>FAc</syl><neume type="tractulus"><nc/></neume>
      <signifLet type="gabc-nabc" n="1">talsi8</signifLet>
    </syllable></layer>
  </staff>
</section>
```

Staff 2 is declared **inside its `<staff>`** (Guidelines §4.2.2, MEI All) with
**no `@lines`**: the RNG types `@lines` as `positiveInteger` (so `lines="0"`
is invalid) and Schematron requires a `staffDef` for the staff, which the
in-place declaration supplies without inventing a line count (§12.3.1). Its
`<nc>` have no `@pname`/`@oct`: this is the MEI 5.1 adiastematic convention
of Guidelines §6.3 Listings 244–247 and 253 (unpitched `<nc>` with
`@tilt`/`@intm`). Complex NABC glyph descriptors
become `<neume type="virga|punctum|tractulus|...">` with the glyph's note
count of `<nc>`; modifiers `-`, `>` and `~` become `<episema>`, `<liquescent>`
and `@intm` carries the direction between successive `hX` pitch descriptors;
significant letters become `<signifLet @place>` (§6.4.8). The complete table,
with one citation per row, is in
[`format-mapping/mei.md`](format-mapping/mei.md) §"NABC (manuscript neumes)
↔ MEI 5.1". `@xml:id`/`@corresp` link each pair of syllables (RNG :21213).

The exact NABC snippet text always stays in `<signifLet type="gabc-nabc">`,
with `@n` = the number of square notes before the cut. That carrier is what
makes the conversion reversible independently of any semiological
interpretation; a third-party MEI tool sees the real `<neume>`/`<nc>`
structure and can ignore the carrier. Where MEI has no equivalent
(`S`/`G`/`M` variants, spacing, Laon position 5 inside the neume), only the
carrier keeps the information, and the mapping table says so explicitly.

### 13.2 Files and wiring

| File | Role |
| --- | --- |
| `crates/scoretk-core/src/model/gregorian.rs` | `NabcSnippet { notes_before, text }`, `Syllable::nabc` (L6) |
| `crates/scoretk-gabc-import/src/read.rs` | `|` cut with `nabc-lines` state (`y:870-907`); square snippets stay notes, NABC snippets become atomic text |
| `crates/scoretk-gabc-export/src/lib.rs` | re-emits square/NABC snippets in source order, including the cut back to square state |
| `crates/scoretk-mei-common/src/nabc.rs` | Gregorio NABC → MEI 5.1 table, parser and inverse base codes |
| `crates/scoretk-mei-export/src/nabc.rs` | second staff, `<neume>`/`<nc>`, exact-text carrier, `@corresp` |
| `crates/scoretk-mei-import/src/nabc.rs` | reads the carrier back, falls back to structured neumes for third-party files |
| `crates/scoretk/tests/gabc_nabc.rs` | round trip, structure, default-byte invariance, coverage measurement |
| `scripts/check-mei-nabc.sh` | `jing` + Schematron validation and measured report |

The 56 `.mscx` corpus exports remain byte-identical to `master` (0/56 diffs
measured), whatever the options: the CMN writer is untouched. The nine GABC
exports now differ from `master` by design: since §16 the three NABC files
keep their manuscript snippets instead of singing NABC glyph letters as square
notes, and since `gabc.md` §19 every GABC export carries the exact body layout
and cut resumption in standard `@label` attributes. Without `full_support` the
second manuscript staff is still absent; only the layout carrier changes.

### 13.3 Measured results

| Measurement | Command | Result |
| --- | --- | ---: |
| NABC files carried in L6 | `cargo test -p scoretk --test gabc_nabc` | 3/3 Gregorio examples |
| NABC snippets crossing MEI byte-for-byte | same | **144/144** |
| GABC → MEI → GABC vs today's direct bytes | same (`assert_eq!(gabc, baseline)`) | **3/3 identical** |
| MEI 5.1 RelaxNG validity of the 2-staff outputs | `scripts/check-mei-nabc.sh` (`jing`) | **3/3** |
| Embedded Schematron on those outputs | `scripts/oracle/mei/schematron.py` | **0 errors, 0 warnings** on 3/3 files (§12.3.1) |
| Default MEI bytes vs `master`, 56 `.mscx` | CLI + `cmp` | **0/56 diffs** |
| Structural share (descriptor with a `<neume>`) | `nabc_structural_vs_textual_coverage_is_measured` | **178/193 = 92.23 %** |
| Text-only share (no MEI equivalent) | same | 15/193 = 7.77 % |
| Modifiers with a dedicated element | same | 53 episemata, 22 liquescences |
| Significant letters with `@place` | same | 48/50 (2 Laon "inside", no MEI value) |

#### GABC → GABC against the source file (master binary → branch binary)

The CLI was run with `--from gabc --to gabc` on each versioned source;
"before" is commit `master` (`87fd3b85`), "after" is this branch. Comments,
blank lines and the single-line body layout are the writer's normalisation,
so the byte comparison is made on the source body after removing comments and
joining lines; the semantic comparison converts both outputs back to
MusicXML with a fixed `--encoding-date` and normalises the source offsets
(`o=…|e=…`) in the private `gabc:note:` carriers.

| Source file | Source bytes | Before / After bytes | Body exact after | Semantic vs source (before → after) | MusicXML `<note>` (before → after) |
| --- | ---: | ---: | --- | --- | ---: |
| `gregorio_FactusEst.gabc` | 1 297 | 963 / 1 110 | yes | no → **yes** | 281 → 116 |
| `gregorio_omnes.gabc` | 606 | 562 / 606 | yes | no → **yes** | 158 → 61 |
| `gregorio_veni.gabc` | 1 206 | 1 076 / 1 206 | no¹ | no → **yes** | 312 → 176 |

¹ One `,` bar sign inside a square run between two manuscript cuts is not
re-emitted. This pre-existing writer limitation is present in `master` too,
is not a regression from the reader fix, and affects layout only; all 144
NABC snippet texts and all sung notes are preserved byte for byte.

#### GABC → MusicXML / MIDI / LilyPond / ABC / MEI, before vs after

Direct conversion of each source, with the same CLI options and a fixed
encoding date. All 15 before/after outputs differ: `master` sang the NABC
glyph letters as square notes. To isolate the fix, the same three sources
were also converted with every `|…|` manuscript snippet removed: on those
square-only inputs the two binaries are **byte-identical in all 15 cases**
(3 files × 5 formats), and the branch's output from the full source equals
its output from the square-only source (MusicXML modulo the source offsets
above, the other four formats byte for byte, 15/15). Hence the only change
in each direction is the removal of notes that were never sung.

| Direction | Full-source outputs differing (3 files) | Square-only: master vs branch bytes | Square-only vs full source, branch | `<note>` reduction, Factus / omnes / veni |
| --- | ---: | --- | --- | --- |
| MusicXML | 3/3 | **3/3 identical** | 3/3 equal after offset normalisation | 281→116, 158→61, 312→176 |
| MIDI | 3/3 | **3/3 identical** | **3/3 byte-identical** | — |
| LilyPond | 3/3 | **3/3 identical** | **3/3 byte-identical** | — |
| ABC | 3/3 | **3/3 identical** | **3/3 byte-identical** | — |
| MEI (default) | 3/3 | **3/3 identical** | **3/3 byte-identical** | — |

These are the historical measurements of the §16 campaign. Since `gabc.md`
§19 the standard `@label` layout carrier changes the GABC → MEI bytes by
design (also on square-only inputs), so the last MEI row no longer describes
the current tree; the four other rows are unchanged. The 56 `.mscx` CMN
exports still have 0/56 diffs against `master`.

Limits: the `S`/`G`/`M` variant digits and horizontal spacing have no MEI
element (RNG/Guidelines §6.4.9), the two Laon "inside" letters have no
`data.EVENTREL` value (RNG :6466), and a third-party file without the
`<signifLet>` carrier can only be read back approximately. Exactness of the
GABC round trip never depends on those interpretations. In that campaign no
default output changed, no declaration was narrowed and no new wasm API was
introduced; §19 adds a standard layout carrier to GABC → MEI only.

## Chant carriers in MEI full-support mode

With `--mei-full-support`, a `.gabc` source is written with the normative
`<nc @pname @oct>`, NEUMES modifiers, `<custos>`, divisiones and `<syl>`; the
reader reconstructs the payload from those elements. A `<annot type="chantNote">`
whose text is `letter: e | glyph: virga augmentum | attribute: [oh:h]` is a
**dump of the source** under a generic element: no MEI consumer reads those
labels as facts. It has been removed from the writer and the reader, and the
oracle strips it (failing loudly if it reappears). The exact staff letter, the
multi-character glyph spelling and bracketed attributes are counted as defects
when no normative MEI element carries them. Without the option no private
annotation is emitted; the 56/56 MuseScore 4.7.4 MEI references stay
byte-identical. The honest figure is in `docs/agents/gabc.md` §26.
