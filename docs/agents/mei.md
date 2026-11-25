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
- `crates/scoretk/tests/mei_conformity.rs` — a MusicXML-style normalised
  comparison of scoretk's export with the 4.7.4 reference: element name plus
  a stable set of musical attributes, folding `@accid` from a child element
  into the note. Volatile ids, `@isodate`, `@tstamp` and layout-only `<beam>`
  containers are excluded.

## 3. Results (56 corpus scores)

### Layer coverage (declared L0–L5, 5 879 reference items)

| Row | Essential | Full support (before → after) | Per-layer matched / reference (after) |
| --- | ---: | ---: | --- |
| Round trip (own writer) | **99.96 %** | 73.52 % → **95.03 %** | L1 1059/1085, L2 3243/3314, L3 206/296, L4 106/175, L5 973/1009 |
| Import of MuseScore 4.7.4 `.ref.mei` | **98.57 %** | 81.90 % → **87.11 %** | L1 1051/1085, L2 3207/3314, L3 218/296, L4 108/175, L5 537/1009 |

The "before" column is the measurement of commit `b6a13039`; the "after"
column is the measurement after the campaigns of §7 below. The per-layer
counts come from `layer_coverage_table`. The remaining gap of the import row
is dominated by 380 `style` items **absent from MuseScore's own `.ref.mei`
files** (see §7). A second import figure that excludes only the item kinds
whose carrier MuseScore never writes is **95.60 %** (5 251 reference items);
it is printed by `mei_import_figure_without_absent_carriers` and never
replaces the 87.11 % main figure.

### Normalised export conformity

| Comparison | Matched / reference events | Conformity |
| --- | ---: | ---: |
| scoretk MEI export vs MuseScore 4.7.4 `.ref.mei` | 3 877 / 4 016 | **96.54 %** |

**The ≥ 95 % target is now met for the round trip (95.03 %), the musical core
(essential), export conformity, and the reference-bound import figure
(95.60 %).** The campaigns of §7 and §8 closed the barline, stem, `<pgHead>`,
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

`nix build .#wasm` (release, `opt-level = "z"`): the ESM wasm grew from
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
