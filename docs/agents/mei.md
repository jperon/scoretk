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

| Row | Essential | Full support | Per-layer matched / reference |
| --- | ---: | ---: | --- |
| Round trip (own writer) | **95.89 %** | 73.52 % | L1 852/1085, L2 2958/3314, L3 129/296, L4 106/175, L5 277/1009 |
| Import of MuseScore 4.7.4 `.ref.mei` | **96.76 %** | 81.90 % | L1 881/1085, L2 3098/3314, L3 207/296, L4 108/175, L5 521/1009 |

### Normalised export conformity

| Comparison | Matched / reference events | Conformity |
| --- | ---: | ---: |
| scoretk MEI export vs MuseScore 4.7.4 `.ref.mei` | 3 877 / 4 016 | **96.54 %** |

**The ≥ 95 % target is met for the musical core (essential) and for export
conformity, but not for "full support" as this instrument defines it.**
The full-support denominator includes layout items that MEI Basic cannot
carry: `style` (380 items: MuseScore page margins, spatium, font sizes),
`credit-position` (67), `barline` segment objects (85, distinct from
`@left`/`@right` barline renditions), and the generated `xml:id`/header blob.
These are exactly the items that MuseScore's own MEI re-import also drops:
using MuseScore's MEI→mscx output as the oracle on the 49 files it can
re-import gives 82.86 % full support, so the remaining gap is a format
boundary, not an unported converter. Round-trip L2 is additionally limited
by slurs/hairpins whose ends sit in another measure (the writer currently
anchors them with `@tstamp`), and L3 by title-frame texts (MuseScore writes
`<pgHead>` only when a frame exists).

Largest remaining import gaps: `style` 380, `barline` 85, `note` 55,
`dynamic-velocity` 43 (MEI `dynam` has no velocity), `note-head` 41,
`tuplet-member` 39, `key-sig` mode 37, `articulation` 29.

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
eighth format.
