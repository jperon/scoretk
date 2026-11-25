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
- `crates/scoretk/tests/mei_conformity.rs` — a MusicXML-style normalised
  comparison of scoretk's export with the 4.7.4 reference: element name plus
  a stable set of musical attributes, folding `@accid` from a child element
  into the note. Volatile ids, `@isodate`, `@tstamp` and layout-only `<beam>`
  containers are excluded.

## 3. Results (56 corpus scores)

### Layer coverage (declared L0–L5, 5 879 reference items)

| Row | Essential | Full support (before → after) | Per-layer matched / reference (after) |
| --- | ---: | ---: | --- |
| Round trip (own writer) | **98.88 %** | 73.52 % → **92.35 %** | L1 1007/1085, L2 3181/3314, L3 206/296, L4 106/175, L5 929/1009 |
| Import of MuseScore 4.7.4 `.ref.mei` | **96.76 %** | 81.90 % → **84.18 %** | L1 983/1085, L2 3127/3314, L3 210/296, L4 108/175, L5 521/1009 |

The "before" column is the measurement of commit `b6a13039`; the "after"
column is the measurement after the campaigns of §7 below. The per-layer
counts come from `layer_coverage_table`. The remaining gap of the import row
is dominated by 380 `style` items **absent from MuseScore's own `.ref.mei`
files** (see §7), not by an unported converter.

### Normalised export conformity

| Comparison | Matched / reference events | Conformity |
| --- | ---: | ---: |
| scoretk MEI export vs MuseScore 4.7.4 `.ref.mei` | 3 877 / 4 016 | **96.54 %** |

**The ≥ 95 % target is met for the musical core (essential) and for export
conformity, but not for "full support" as this instrument defines it.** The
gap campaign of §7 closed the barline, stem, `<pgHead>`, style, signature,
tremolo, articulation, ending, `<dynam>`/tempo and note-id anchoring gaps;
the round trip rose from 73.52 % to 92.35 %. The remaining import-row gap is
overwhelmingly a *reference* boundary: MuseScore's own `.ref.mei` files carry
none of the standard MEI page-layout/typography carriers (380 `style` items),
so an honest importer cannot recover them. `mei_musescore_references_carry_no_style`
in `layer_coverage.rs` pins that fact.

Largest remaining round-trip gaps: `dynamic-velocity` 43, `key-sig` mode 37,
`lyric` 30, `note` 26, `tuplet-member` 26, `note-head` 22, `slur` 19,
`credit-position` 18, `credit` 15, `tbox` 30. Largest remaining import gaps:
`style` 380, `note` 55, `dynamic-velocity` 43, `note-head` 41,
`tuplet-member` 39, `key-sig` mode 37. The out-of-reach items are justified
in §7.3 with the MEI 5.1 attribute definitions.

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
  no velocity attribute: `att.dynamic` in `MEI.shared.xml` only carries the
  dynamic's type/placement; `<dynam>` has no `@vel`. `Convert::dynamToMEI`
  (`meiconverter.cpp:1367`) writes `@label` + text only. Nothing standard can
  carry MuseScore's numeric velocity.
- **`key-sig` mode (37 per row).** `att.keySig.log` defines only `@sig`
  (`MEI.shared.xml`); the schema has no `ident="mode"` attribute at all. The
  `KeyMode::Unknown` of the reference cannot be expressed, while `@keysig`
  faithfully carries the fifths count.
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
- **`note-head` (22 round trip / 41 import).** `@head.shape` uses the closed
  `data.HEADSHAPE` enumeration (`MEI.xml`: `quarter`, `half`, `whole`,
  `backslash`, `circle`, `+`, `diamond`, `isotriangle`, `oval`, `piewedge`,
  `rectangle`, `rtriangle`, `semicircle`, `slash`, `square`, `x`). The
  remaining groups (solfège `do`…`ti`, heavy crosses, etc.) have no term and
  are not encoded as private NMTOKENs. MuseScore 4.7.4 writes no
  `@head.shape` at all, hence the import-row figure.
- **`style` (380 on the import row only).** MEI 5.1 *can* express page
  geometry (`att.pages`, `@vu.height`, numeric `@fontsize`), and scoretk
  reads and writes all of them (round trip 380 → 0). The MuseScore 4.7.4
  `.ref.mei` files contain none of these attributes, so the information is
  absent from the oracle, not from the reader. Verified by
  `mei_musescore_references_carry_no_style`.

No scoretk-private carrier was used: the `@type="mscore-*"` values are
MuseScore's own documented extensions and are read/written as such, not
counted here as interoperability gains.
