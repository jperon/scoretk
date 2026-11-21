# Writing MuseScore: `.mscx` and `.mscz`

The `crates/scoretk-musescore-export` crate, wired into the `scoretk` facade by
the `musescore-export` feature (`Format::Mscx` and `Format::Mscz` for writing).
The project core and its invariants are in `AGENTS.md`; the C++ → Rust mapping is
in `docs/agents/upstream.md`.

## 1. Role and API

```rust
pub fn write_mscx(score: &Score) -> Result<String, Error>; // <museScore version="4.70">
pub fn write_mscz(score: &Score) -> Result<Vec<u8>, Error>; // ZIP archive
```

The writer is a port of `engraving/rw/write/` (`Writer::writeScore`,
`StaffWrite::writeStaff`, `MeasureWrite::writeMeasure`, `TWrite::writeItem`,
`ConnectorInfoWriter`, `WriteContext`) and of
`engraving/infrastructure/mscwriter.cpp` for the container. Function names and
breakdown follow the C++: `write/writer.rs`, `write/staffwrite.rs`,
`write/measurewrite.rs`, `write/twrite.rs`, `write/connectorinfowriter.rs`,
`write/context.rs`.

The library is portable (no `std::fs`, threads or clock) and compiles for
`wasm32-unknown-unknown`; the compliance test runs on the host.

## 2. Contents of the `.mscz`

`MscSaver::writeMscz` writes the style separately and the rest in the archive.
The crate produces, in this order and deterministically (deflate, frozen ZIP
timestamp):

| Entry | Role |
| --- | --- |
| `META-INF/container.xml` | `rootfile` to `score_style.mss` then `score.mscx`, as in MuseScore 4.7 |
| `score_style.mss` | `<Style>` of the model (same values as in the `.mscx`) |
| `score.mscx` | complete score |
| `audiosettings.json`, `viewsettings.json` | `{}` |

The thumbnail (`Thumbnails/thumbnail.png`) and media are not produced: the
library does not render. MuseScore recreates them when saving.

## 3. Measured fidelity: round trip

The test `crates/scoretk-musescore-export/tests/roundtrip.rs` replays, for each
corpus file, the chain

```text
read (.mscx 3.x or 4.x) → write .mscx → re-read with the 4.x reader
→ export MusicXML
```

and compares the resulting MusicXML with the **direct** export of the original
model, using the exact normalisation of `crates/scoretk/tests/e2e_corpus.rs`, at
both levels (musical core and full parity). Result as of 2026-11-17:

| Corpus | Identical `.mscx` round trip | Identical `.mscz` round trip |
| --- | --- | --- |
| `tests/corpus/v4` | **26/26** (core and full) | **26/26** |
| `tests/corpus/v3` (read as 3.x, written as 4.x) | **30/30** | **30/30** |

Both containers are measured for each file.

All these files are listed in `EXPECTED_ROUNDTRIP_V4` and
`EXPECTED_ROUNDTRIP_V3`: if any one of them ceases to be identical, the test
fails. The list is therefore non-regressive, like `EXPECTED_CONFORMANT` of the
e2e test.

The `archive_mscz_relue` test additionally checks the `.mscz` container on one
file; the `scoretk` CLI has its own tests (`.mscx` and `.mscz` written then
re-read).

## 4. What is preserved, and how

The model stores only what the MusicXML export needs; the writer produces the
elements that the reader re-reads to reconstruct that same model:

- **score**: `<Division>`, complete `<Style>` (page format, margins,
  `lyricsPlacement`, font sizes, `hairpinHeight`, empty staves), `<metaTag>`
  (all fields, even empty: an empty `<metaTag name="platform">` is not the
  same as "absent");
- **parts and staves**: `<Part>` (names, `<Instrument>` with transposition,
  program, MIDI channel, volume, pan, MIDI articulations), `<Staff>`
  declarations (lines, `barLineSpan`, default clef, brackets);
- **measures**: `len` of irregular measures, `<startRepeat>`/`<endRepeat>`,
  `<multiMeasureRest>` (layout measure after the one that opens the group),
  `<LayoutBreak>` (`section` on the previous one, `line` on the one that ends
  the system), `<Marker>`/`<Jump>`, `MeasureRepeat` (with
  `measureRepeatCount`, `<durationType>measure</durationType>` and its
  duration: MuseScore 4.7 rejects the file without these two children; the
  content is copied by the reader, not rewritten);
- **voices and rhythm**: positional `<voice>` blocks, exact positions via
  `<location><fractions>` (no rounding), `<durationType>`, `<dots>`, explicit
  `<duration>`, `<Beam>`/`<BeamMode>`, `<Tuplet>` (with `<bracket>`), grace
  chords as separate `<Chord>`s, `gap` rests, lyrics, fingerings;
- **notations**: key signatures (`concertKey`/`actualKey`, mode, courtesy),
  time signatures (subtype, stretch, courtesy), clefs (`isHeader`), barlines
  (extension), articulations, ornaments, fermatas, tremolos, arpeggios, chord
  lines, accidentals (`role`/`parens`), tie prolongation (two halves with
  `<location>`), slurs, trills, glissandos, hairpins, ottavas, pedals, voltas,
  text lines;
- **annotations**: dynamics, staff/expression/system texts (MuseScore markup
  reconstructed, `&quot;` entities preserved, `<sym>` restored), tempos,
  rehearsal marks, instrument changes, chord symbols (`harmonyType`, degrees),
  figured bass, vibrato bars;
- **frames**: credits (`<VBox>` per page, height, style, alignment), `TBox`
  texts (above or below the measure, font size).

Positions are written as exact fractions (`<location><fractions>`), which
avoids any loss on n-tuplets (3, 5, 7…), unlike an integer `<tick>`.

## 5. Known limits

- **Unmodelled layout**: fine positions (`default-x/y`, stems, computed beams,
  system widths) are not in the model; the output is musically identical, not
  graphically. This is the same limit as the MusicXML export.
- **Copied `spanners` order**: a spanner entirely contained in a repeated
  measure is not written; the reader recreates it when expanding the source
  measure.
- **Fields without a `.mscx` equivalent**: `Accidental.user` goes through
  `<role>`, parentheses through `<parens>`; hand-built `Dimension`/`BarLine`
  can lose `EndStartRepeat` (written as `<endRepeat>` + `<startRepeat>`),
  which the MusicXML export does not distinguish.
- **3.x format**: the output is always 4.70; there is no 3.x writer. Elements
  read in 3.x but with no 4.x equivalent (for example old `<subtype>` key
  encodings) are converted by the 3.x reader, not by the writer.
- **Dates**: `<programVersion>` is frozen at `4.7.4` and `<programRevision>`
  is empty; no clock is read (reproducible output).

## 6. Checks

```sh
nix develop -c cargo test -p scoretk-musescore-export
nix develop -c cargo build -p scoretk-musescore-export --target wasm32-unknown-unknown
# Does MuseScore 4.7.4 reopen the outputs? (headless, outside the graphical session)
scripts/check-mscx-rewrite.sh
```

### Non-regression oracle

`compare.sh <root>` rebuilds the current CLI, regenerates its outputs and
compares them with master's reference. That reference must be refreshed with
the master binary and the **same procedure** whenever a fix legitimately
changes outputs; otherwise the oracle measures nothing (498 `DIFF` against a
stale reference). `scripts/oracle/refresh-reference.sh [<dir>]` does exactly
that: it builds the repository CLI, converts every corpus `.mscx` and
`.ref.musicxml` to the seven output formats, then replaces the reference
directory atomically. Only the script and this note are versioned; the
generated data stays outside the repository, like the rest of the oracle.

`scripts/check-mscx-rewrite.sh` rewrites each corpus file as `.mscx` and
`.mscz`, then asks MuseScore 4.7.4 (`QT_QPA_PLATFORM=offscreen`) to open and
export it. It checks opening, not graphical parity: as of 2026-11-17,
**56/56 files** (the 26 v4 and the 30 v3, in both `.mscx` and `.mscz`) are
reopened without `--force`.

## 7. 4.x reader adjustments made for the round trip

A few properties that the writer must preserve were not re-read by the 4.x
reader (they did not change the corpus references, which do not contain them);
they were added in `read4xx`, with the same semantics as `read302` and `TRead`:

| Property | `.mscx` element | Effect on export |
| --- | --- | --- |
| n-tuplet bracket | `<Tuplet><bracket>` | `<tuplet bracket="no">` |
| chord writing | `<Harmony><harmonyType>` | `<numeral>`/`<function>` branch instead of `<root>` |
| chord degrees | `<Harmony><degree>` | MusicXML `<degree>` |
| accidental role | `<Accidental><role>` | `cautionary="yes"` |
| accidental parentheses | `<Accidental><parens>` | `parentheses="yes"` |
| MIDI channel | `<Channel><midiChannel>` | `<midi-channel>` and free channels |
| instrument change | `<InstrumentChange><Instrument>` | `Part::instruments()`, `<score-instrument>` |
| frame below the measure | `<TBox><Text><offset y>` | text target and placement |
| frame font size | `<TBox><Text><size>` | `font-size` of `<words>` |

The corpus e2e test (4.7.4 references) remains unchanged by these additions:
only `v3/moonlight` (18 lines) and `v3/voices` (6 lines) still differ from the
references, for the layout reasons already documented in
`docs/agents/ecarts-noyau.md`.

## 8. Measured fidelity after reopening in MuseScore 4.7.4

### Frozen baseline and oracle

There is no published specification of `.mscz` (see
`docs/agents/format-ref/musescore.md`): the behaviour of MuseScore 4.7.4 is the
reference. The round-trip test of §3 only uses scoretk's *own* reader, which
can hide symmetric errors; the oracle below uses MuseScore itself.

- Frozen binary: `/tmp/scoretk-mscz-base`, built from commit `497f9e3`, sha256
  starting `04033ff2586c5090`. The fixed binary `/tmp/scoretk-mscz-final`
  (sha256 starting `3611bacac5460a50`) is built from the same worktree after
  the four fixes below, so both runs can be compared file by file.
- Lots: the 56 corpus `.mscx`, the 174 `.mscz` of `~/Musique`, the 32 genuine
  MusicXML files of `~/Musique`, a deterministic 1/5 sample of the 1 872 `.ly`
  files (375) and of the `.abc` files (1). Manifests are built by
  `scripts/oracle/musescore/lots.sh`, which reuses the lists of the LilyPond
  and ABC writing oracles. No third-party file is versioned.
- Oracle: `scripts/oracle/musescore/mesure.sh`. For each source, scoretk writes
  a `.mscz`, MuseScore 4.7.4 reopens it and exports MusicXML and MIDI, and the
  result is compared to scoretk's *direct* MusicXML and MIDI. Core comparison
  reuses `core_norm` through `crates/scoretk/tests/oracle_core.rs` (no
  duplicated normalisation), N1/N2 reuse `scripts/oracle/midi/midi_notes.py`
  at the Q16 grid (±1/16 quarter). Every binary runs under `timeout` and
  `prlimit --as=2000000000`.
- **Alternative to beat:** MuseScore's own import of the same source (its own
  `.mscz` for an mscz source, MusicXML/LilyPond/ABC import otherwise). A file
  is counted as a loss when its round-trip has more changed core lines than
  that alternative.

### Baseline table (frozen binary)

| Lot | Scores | `.mscz` written | Opened | Core-identical | N1 | N2 | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | 38/56 | 0.9834 | 0.9817 | 0 | 56/56 |
| mscz | 174 | 171 | 172 | 9/171 | 0.9823 | 0.9783 | 0 | 39/171 |
| xml | 32 | 27 | 27 | 0/27 | 0.9127 | 0.3063 | 0 | 0/27 |
| ly | 375 | 3 | 3 | 1/3 | 1.0000 | 1.0000 | 0 | 0/0 |
| abc | 1 | 0 | 0 | 0/0 | — | — | 0 | 0/0 |
| **Total** | **638** | **257** | **258** | **48/257** | **0.9695** | **0.8542** | **0** | **95/254** |

No MuseScore load error or warning was reported on the files it did open; the
three mscz failures (one unreadable 1.14 file, one MIDI export timeout) and
the 372 unopened `.ly`/`.abc` files are the limits detailed below.

### Baseline gaps by first differing core family

154 files are worse than MuseScore's own import. The first differing core
family (`scripts/oracle/musicxml/groupes.py`, same columns) is:

| Family (changed tag) | Files | Extra changed lines |
| --- | ---: | ---: |
| `midi-device` | 46 | 30 556 |
| `sign` (clef) | 26 | 2 554 |
| `measure-style` | 14 | 3 026 |
| `measure` | 11 | 1 971 |
| `part-abbreviation` | 10 | 706 |
| `note` | 9 | 5 018 |
| `tuplet` | 8 | 1 392 |
| `clef-octave-change` | 7 | 785 |
| `harmony` | 4 | 205 |
| `accidental` | 4 | 153 |
| others (2 or fewer each) | 17 | — |

### Final table (after the four fixes)

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | 45/56 | 0.9700 | 0.9682 | 0 | 56/56 |
| mscz | 174 | 171 | 172 | 28/171 | 0.9781 | 0.9732 | 0 | 39/171 |
| xml | 32 | 27 | 27 | 0/27 | 0.9103 | 0.3055 | 0 | 0/27 |
| ly | 375 | 297 | 297 | 1/297 | 0.9998 | 0.9966 | 0 | 0/0 |
| abc | 1 | 0 | 0 | 0/0 | — | — | 0 | 0/0 |
| **Total** | **638** | **551** | **552** | **74/551** | **0.9677** | **0.8594** | **0** | **95/254** |

171 files improve, 3 files gain core lines (mscz-33: +51, mscz-117: +12,
mscz-115: +2). All three keep exactly the same N1/N2/N2d (4 196/4 196 notes
for mscz-33, 2 856/2 856 for the other two): the corrected `<midi-device>` and
clef lines change the alignment of the line diff, exposing the unchanged
`note`/`forward`, anacrusis and `<attributes>` differences that were already
further down in the same file. There is no musical loss.

### Fixes, in decreasing count

1. **`<midi-device port>` — 46 files, up to 1 975 changed lines.** The writer
   never emitted `<Channel><midiPort>`, so MuseScore recomputed the port from
   the MIDI channel. Port of `TWrite::write(const InstrChannel*)`
   (`write/twrite.cpp:2111-2114`), which writes `midiPort` and `midiChannel`
   together. Reduced test: `port_midi_du_canal_ecrit_et_relu`.
2. **Clef `<sign>` — 26 files.** The writer omitted `<transposingClefType>`
   when it equalled the concert clef; the reader (`read400/tread.cpp:2551`)
   then forces G. Port of `TWrite::write(const Clef*)`
   (`write/twrite.cpp:1281-1299`). Reduced test:
   `clef_basse_ecrit_son_type_transposant`.
3. **Harmony — 4 files first, but up to 196 lines on the worst one.** The
   writer used the pre-4.6 `<root>`/`<name>` children of `<Harmony>`; since
   4.6 the reader (`read460/tread.cpp:2868`) only accepts `<harmonyInfo>`.
   Port of `writeHarmonyInfo` (`write/twrite.cpp:1786-1826`), with the direct
   children kept for scoretk's own reader. Reduced test:
   `accords_chiffres_relus_par_musescore_46`.
4. **Empty `<Instrument/>` — 372 files that MuseScore refused to open.** A
   scoretk score built from LilyPond/ABC has no instrument; MuseScore 4.7.4
   exits with code 1 (no message) on `<Instrument/>`. The writer now emits
   the minimal default `<Instrument><Channel><program value="0"/></Channel>`
   as MuseScore's own writer always does (`TWrite::write(const Instrument*)`,
   `write/twrite.cpp:1947`). Reduced test:
   `instrument_vide_ecrit_un_instrument_valide`.

### Out of scope (unchanged)

The remaining families are not simple mis-serialisations of a stored value:
`measure-style`/`measure` concern the anacrusis number and multi-measure-rest
range, `part-abbreviation` the part-group/newline handling,
`clef-octave-change`/`accidental` the interaction between clef octave and
transposition and courtesy accidentals, `tuplet` the bracket encoding read back
by MuseScore, and `note`/`forward` the padded voice timeline. They need model
or reader work in addition to the writer, and are left unchanged here. The
`v3/moonlight` and `v3/voices` reference differences of §7 are likewise
layout-only.

### Second pass — the 86 files still unopened

The frozen baseline of this pass is the previous final run itself: the binary
`/tmp/scoretk-mscz-pass2-base` rebuilt from the merged master is byte-identical
(sha256 `3611bacac5460a50…`) to `/tmp/scoretk-mscz-final`, so the table above
is the baseline for the comparison below.

MuseScore 4.7.4 writes a detailed log under
`~/.local/share/MuseScore/MuseScore4/logs/` even though the CLI prints nothing;
grouping the 86 unopened files by its first error gives:

| Cause (MuseScore log / scoretk error) | Files | Owner |
| --- | ---: | --- |
| `Incomplete measure … Found: 3/8. Expected: 1/8` (and 30 other Found/Expected pairs) | 63 | reader: `\partial 8*5` and `*4` multipliers in the LilyPond/ABC source produce a measure whose `len` disagrees with its chord durations |
| `PlaybackSetupDataResolver: Unable to resolve setup data for instrument, id: S.` then SIGSEGV | 6 | **writer**: an `<Instrument>` with no `<Channel>` |
| scoretk `source LilyPond illisible : aucune musique dans la source` | 16 | reader: `.ly` include snippets with no music |
| scoretk `version MuseScore non prise en charge` (1.14) | 1 | reader: `read114` is deliberately not ported |

The one writer cause is fixed by porting `TWrite::write(const Instrument*)`
(`write/twrite.cpp:1947`): every instrument now gets at least an empty
`<Channel>`, as MuseScore's own writer always does. Reduced test:
`instrument_sans_programme_ecrit_un_canal`. The patch was first verified by hand
on `03_01_Miguel_Les-grains-de-mais_livret.ly`: adding `<Channel/>` to the
written `.mscz` changes MuseScore's exit from SIGSEGV to success.

### Second pass — remaining non-identical files by first core family

On the current binary, 477 opened files still differ from scoretk's direct
MusicXML. Counting files *worse than MuseScore's own import of the same source*
(rightmost column) is the meaningful measure; the first differing tag alone is
often a diff-alignment artefact (e.g. `part-group` prints an identical line).

| First tag | Files | Changed core lines | Worse than alternative |
| --- | ---: | ---: | ---: |
| `instrument-sound` | 299 | 11 482 | 0 |
| `measure-style` | 29 | 11 144 | 24 |
| `measure` | 24 | 27 809 | 24 |
| `note` | 22 | 5 953 | 18 |
| `attributes` | 17 | 9 296 | 8 |
| `part-group` | 17 | 4 613 | 13 |
| `tuplet` | 15 | 8 785 | 6 |
| `part-abbreviation` | 14 | 756 | 9 |
| `accidental` | 9 | 298 | 6 |
| others (8 or fewer each) | 30 | — | 14 |

`instrument-sound` is the known consequence of the fallback instrument: a
LilyPond source has no instrument in the pivot model, so scoretk's direct
MusicXML writes none while MuseScore assigns `keyboard.piano.grand`. MuseScore's
own import of the same `.ly` fails outright, so there is no alternative to beat
on those files.

`measure-style`/`measure` come from the reader's anacrusis and multi-measure
rest model (a 1/8 `len` holding a dotted quarter, or a single mmRest measure
where MuseScore builds a range); `note`/`forward` from scoretk's explicit
timeline padding in the MusicXML exporter; `tuplet` from the reader's bracket
heuristic; `part-abbreviation`/`part-group` from part metadata. None is a
mis-serialised value in the writer: a faithful port of
`MeasureWrite::writeMeasure`, `TWrite::write(const Tuplet*)` and
`TWrite::writeProperties(const ChordRest*)` already reproduces the model. They
are left for reader/model work rather than patched with a writer-side fudge.

### Second pass — final table

Binary `/tmp/scoretk-mscz-pass2-final` (sha256 `5e1d046bb209beb6…`). Same 638 scores, same oracle, same tolerance.

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | 45/56 | 0.9700 | 0.9682 | 0 | 56/56 |
| mscz | 174 | 171 | 172 | 28/171 | 0.9781 | 0.9732 | 0 | 39/171 |
| xml | 32 | 27 | 27 | 0/27 | 0.9103 | 0.3055 | 0 | 0/27 |
| ly | 375 | 301 | 301 | 1/301 | 0.9998 | 0.9967 | 0 | 0/0 |
| abc | 1 | 1 | 1 | 0/1 | 1.0000 | 1.0000 | 0 | 0/0 |
| **Total** | **638** | **556** | **557** | **74/556** | **0.9679** | **0.8603** | **0** | **95/254** |

Compared with the frozen baseline: **5 more scores open** (301 `.ly` instead of 297, and the single `.abc`), N2 rises from 0.8594 to 0.8603, and **no file loses any core line** (0 regressions, 0 improvements on files that already opened). The 1 still-failing mscore conversion is a reader-caused incomplete measure; the 17 scoretk errors are unchanged. On the `.ly` and `.abc` lots MuseScore's own import fails outright, so the alternative cannot be beaten there; on corpus/mscz/xml the alternative stays at 56/56, 39/171 and 0/27 core-identical, unchanged.

### Third pass — corpus fidelity after reopening (English)

The baseline of this pass is pass 2 (master `1008616`): corpus 45/56,
mscz 28/171, xml N2 0.3055. The 11 corpus scores whose scoretk-written `.mscz`
did not round-trip like MuseScore's own import were reduced by their first
differing core line; three writer causes were found and fixed.

- **`<markerType>` of a repeat marker** — `repeat69` reopened as a `FINE`
  marker (`<sound fine="yes"/>`) because the writer emitted only `<label>`.
  Port of `TWrite::write(const Marker*)` (`write/twrite.cpp:2401-2409`), which
  writes `Pid::MARKER_TYPE` (`<markerType>`, table `typesconv.cpp:3254`);
  without it `Marker::Marker` (`dom/marker.cpp:79`) defaults to `FINE`.
  Reduced test: `marqueur_segno_ecrit_son_type`.
- **`<parentheses>` of a notehead** — `layout_elements` lost the parenthesised
  head: the single `<Symbol name="noteheadParenthesis">` is not the 4.6+
  property. Port of `TWrite::writeProperties(const Note*)`
  (`write/twrite.cpp:690-697`), which writes `Pid::HAS_PARENTHESES`
  (`<parentheses>both</parentheses>`, table `typesconv.cpp:952-958`); the
  MusicXML exporter of MuseScore (`importmusicxml.cpp:3883-3902`) only reads
  that property. The `<Symbol>` is kept for scoretk's own reader. Reduced
  test: `parenthèses_de_tête_ecrites_pour_musescore`.
- **`<dead>` instead of `<ghost>`** — `testNoteheads2` and `voiceswitching`
  gained `<notehead parentheses="yes">`: the pivot collapses MuseScore's
  separate `Pid::GHOST` and `Pid::DEAD` into `Note::ghost`, and `<ghost>` makes
  `ghost()` (`engravingitem.cpp:1688`) add parentheses where the direct export
  does not. Port of `TWrite::write(const Note*)`
  (`write/twrite.cpp:2500-2505`): write `<dead>` for the collapsed field.
  Reduced test: `note_dead_ecrite_comme_dead_pas_comme_ghost`.

Each fix was verified in ordinary music (bars, voices, chords, rests, ties)
and on a non-concerned score, and confirmed by reopening the written `.mscz`
in MuseScore 4.7.4: all four corpus files are now core-identical.

| Cause | Corpus | First differing core line | Owner |
| --- | --- | --- | --- |
| `<markerType>` | repeat69 | `<sound fine="yes"/>` vs `<sound segno="segno"/>` | writer |
| `<parentheses>` | layout_elements | missing `<notehead parentheses="yes">normal</notehead>` | writer |
| `<dead>`/`<ghost>` | testNoteheads2, voiceswitching | `<notehead parentheses="yes">x</notehead>` vs `<notehead>x</notehead>` | writer |

The 7 remaining corpus files are not simple mis-serialisations: `mmrest` and
`testTboxWords1` keep the multi-measure-rest count only through MuseScore's
layout (`createMultiMeasureRests` and page-break frames are not in the pivot
style); `moonlight` is 618 core lines of tuplet brackets in a dense 3.x score;
`testHarmony6` relies on `<function>`, dropped from the 4.6+ harmony encoding;
`changevisibility` attaches an accidental to a trill ornament;
`linked-ties` needs excerpt metadata absent from the pivot; and
`testExcludeInvisibleElements` mixes ottava and invisible elements. They need
model, reader or layout work and are left unchanged.

### Third pass — xml N2 (0.31) reduced

The xml N2 collapse is dominated by three `Vingt danses enfantines` scores
(N1 0.89, N2 0.13). The divergence was reduced to `red_137.musicxml`
(36 measures) and then to `red_1_160.musicxml` (160 measures). Re-reading both
`.mscz` with scoretk gives an identical MusicalXML core (0 changed line), so
the writer preserves the model, the three `<startRepeat>`/`<endRepeat>` pairs
and the six volta spanners. The difference is in MuseScore's playback: its
own re-import of the same source plays 1308 notes over 588.5 quarters, the
scoretk-written `.mscz` 1104 notes over 492.5 quarters — the repeat expansion
is shorter. Targeted encodings were tested one at a time without restoring
parity: grace-note `<Events><Event><len>0</len></Event></Events>`, volta
`<beginText>`, empty versus explicit tie `<location><fractions>`, stray
`<location>` positioning, `<eid>`, `<Division>`, `<Part>` and `score_style.mss`.
This is a MuseScore reader/MIDI-expansion semantic difference, not a
mis-serialised stored value; no speculative writer fudge was applied and the
xml N2 stays at 0.3055.

### Third pass — final table

Binary `/tmp/scoretk-mscz-pass3-final` (sha256 `7ccc09798f0a774d1d0d8a24…`).
Same 638 scores, same oracle, same tolerance. Baseline = pass 2 (§ above),
frozen from master.

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | **49/56** | 0.9700 | 0.9682 | 0 | 56/56 |
| mscz | 174 | 171 | 172 | **30/171** | 0.9781 | 0.9732 | 0 | 39/171 |
| xml | 32 | 27 | 27 | 0/27 | 0.9103 | 0.3055 | 0 | 0/27 |
| ly | 375 | 301 | 301 | 1/301 | 0.9998 | 0.9967 | 0 | 0/0 |
| abc | 1 | 1 | 1 | 0/1 | 1.0000 | 1.0000 | 0 | 0/0 |
| **Total** | **638** | **556** | **557** | **80/556** | **0.9679** | **0.8603** | **0** | **95/254** |

Compared with the frozen baseline, **10 files improve and none loses a core
line**: corpus 8, 48, 52 and 55 (the four causes above, now core-identical);
mscz 20 (20 to 19 changed lines), 46 and 170 (both core-identical, both
`<sound fine="yes"/>` from an unrecognised marker, fixed by `<markerType>`);
and the three `Vingt danses` xml scores each lose 3 core lines (`coda`,
`segno`, `sound`) from the same marker fix. N1 and N2 are unchanged on every
lot, so there is no playback regression. On corpus/mscz/xml the alternative
stays at 56/56, 39/171 and 0/27 core-identical: the remaining gap is
reader/model work out of scope here.

### Fourth pass — fresh baseline (English)

The baseline of this pass is a **fresh, full measurement** (`mesure.sh` without
`--resume`) of the same 638 scores with the current master binary
`/tmp/scoretk-mscz-pass4-base` (commit `47fc6d4`, sha256 `e07c1ae9…`). Since
pass 3, master gained the LilyPond `\partial` multiplier fix and several
MuseScore reader / MusicXML export fixes; the table below is the new baseline,
not a copy of the pass-3 table.

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | 49/56 | 0.9700 | 0.9682 | 0 | 56/56 |
| mscz | 174 | 171 | 172 | 31/171 | 0.9781 | 0.9732 | 0 | 39/171 |
| xml | 32 | 27 | 27 | 0/27 | 0.9103 | 0.3055 | 0 | 0/27 |
| ly | 375 | 340 | 340 | 1/340 | 0.9998 | 0.9970 | 0 | 0/0 |
| abc | 1 | 1 | 1 | 0/1 | 1.0000 | 1.0000 | 0 | 0/0 |
| **Total** | **638** | **595** | **596** | **81/595** | **0.9682** | **0.8612** | **0** | **95/254** |

The 43 rows MuseScore did not open group as follows. Its CLI prints nothing;
the messages come from `~/.local/share/MuseScore/MuseScore4/logs/`, one
invocation per log.

| Cause | Rows | Owner |
| --- | ---: | --- |
| `Incomplete measure … Found: x Expected: y` | 23 | reader/model: a measure `len` disagrees with the durations written in it (anacrusis, padded voices) |
| scoretk `source LilyPond illisible : aucune musique dans la source` | 16 | reader: `.ly` include snippets with no music |
| SIGSEGV (ly-29 `SanctePieX.ly`) | 1 | not reduced to a writer value; MuseScore's own import of the source also fails (rc 40), so there is no alternative to beat |
| `Voice too long … Found: 66/8. Expected: 8` (ly-97) | 1 | reader: LilyPond voice overflow |
| scoretk `version MuseScore non prise en charge` (1.14, mscz-7) | 1 | reader: `read114` is deliberately not ported |
| MuseScore MIDI export timeout (mscz-55) | 1 | playback, not the writer |

For the SIGSEGV, adding a `<Channel>`, instrument names, or removing the
`<Spanner>`, `<Lyrics>`, `<VBox>` and `<BarLine>` elements of the written
`.mscx` one at a time did not change the crash: it is not a missing writer
child of the kind fixed in pass 2.

### Fourth pass — corpus/mscz gaps reduced

For every score where MuseScore's own import is core-identical (rightmost
column of the baseline table) and scoretk's written `.mscz` is not, the first
differing core line was reduced. None of the 25 corpus/mscz files is gained or
lost by the fix below except corpus-11.

| Cause / owner | Scores | First differing core line |
| --- | --- | --- |
| **tuplet `numberType` — writer, fixed** | corpus-11 | `<tuplet bracket="no" type="start"/>` vs `… show-number="none" …` |
| mmRest count — model/style | corpus-24, 42; mscz-34, 35, 36, 38, 39, 42 | `<measure-style><multiple-rest>N</multiple-rest>` written by scoretk only |
| anacrusis measure number — reader/model | mscz-1, 2, 11, 28, 41, 47 | `<measure number="1">` vs `<measure implicit="yes" number="0">` |
| accidental attached to an ornament/padded voice — reader/model | corpus-31; mscz-10, 44, 56, 142 | `<accidental>natural</accidental>` / `<accidental>flat</accidental>` |
| explicit timeline padding — reader | mscz-12, 17, 154 | `<note>` vs `<forward>` |
| Nashville `<function>` — reader | corpus-22 | `<function>m</function>` vs `<numeral>` |
| excerpt/tie metadata — model | corpus-41 | `<tie type="start"/>` written by scoretk only |
| ottava and invisible elements — reader | corpus-50 | `<octave>5</octave>` vs `<octave>4</octave>` |

The mmRest group is not a missing writer attribute: `createMultiMeasureRests`
is in `score_style.mss`, outside the pivot `StyleSettings`. Adding the flag to
the written `.mscx` and `.mss` of corpus-24 by hand makes MuseScore export
three `<multiple-rest>1</multiple-rest>` where scoretk writes one
`<multiple-rest>2</multiple-rest>`: it needs model work, not a writer fudge.
The Nashville group needs `function2Tpc` in `TRead::read(HarmonyInfo*)`
(upstream `read410/tread.cpp:2839`) to fill `rootTpc`, which scoretk's 3.x
reader does not do.

**`<numberType>` of a tuplet — corpus-11 (`moonlight`), 618 core lines.** The
writer emitted `<Number><text/></Number>` but not `Pid::NUMBER_TYPE`, so
MuseScore's 4.6+ reader fell back to `SHOW_NUMBER`. Port of
`TWrite::write(const Tuplet*)` (`write/twrite.cpp:3500-3525`) through
`writeStyledProperties` (`:644-647`), writing `numberType` 1 (`SHOW_RELATION`)
and 2 (`NO_TEXT`) from `dom/property.cpp:119` and `dom/types.h:1600`; the 4.6+
reader (`read460/tread.cpp:1530`) and MusicXML exporter
(`exportmusicxml.cpp:2781-2784`) use only that property. The 4.x reader of
scoretk now reads it too, keeping `<Number><text>` as fallback for older
files. Reduced test: `type_nombre_nolet_ecrit_et_relu`, in ordinary music
(`testTuplets8`: bars, chords, rests, voices and n-olets), with a
non-concerned case. On `moonlight` reopened by MuseScore 4.7.4, the re-export
now carries the same 264 `show-number="none"` as scoretk's direct export
(0 before), over the same 536 tuplets: no pitch, duration or measure changed.
Its core diff drops from 618 to 98 changed lines, but it does **not** become
core-identical: the first remaining difference is `<note>` vs `<forward>`
(scoretk's explicit voice padding), which is reader/MusicXML-export work.
Format note: `.mscx` has no published specification, so this follows the
MuseScore 4.7.4 behaviour and the upstream C++ cited above.

### Fourth pass — final table

Binary `/tmp/scoretk-mscz-pass4-final` (sha256 `069b8afe…`). Same 638 scores,
same oracle, same tolerance. The final run hit the 1-hour shell timeout after
264 of the 375 `.ly` files; the remaining 111 `.ly` and the single `.abc` were
measured with the same frozen binary into a second output directory, then
their rows and work files were merged before recomputing the table from all
638 rows (`mesure.sh --resume` only recomputes the core comparison).

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | 49/56 | 0.9700 | 0.9682 | 0 | 56/56 |
| mscz | 174 | 171 | 172 | 31/171 | 0.9781 | 0.9732 | 0 | 39/171 |
| xml | 32 | 27 | 27 | 0/27 | 0.9103 | 0.3055 | 0 | 0/27 |
| ly | 375 | 340 | 340 | 1/340 | 0.9998 | 0.9970 | 0 | 0/0 |
| abc | 1 | 1 | 1 | 0/1 | 1.0000 | 1.0000 | 0 | 0/0 |
| **Total** | **638** | **595** | **596** | **81/595** | **0.9682** | **0.8612** | **0** | **95/254** |

Compared with the fresh baseline, **no file loses a core line** and two files
improve: corpus-11 `moonlight` (618 → 98 changed core lines) and mscz-167
`Vivaldi, DeTorrente` (366 → 104, same `<tuplet show-number="none">` cause).
N1/N2 are unchanged on every lot. No file becomes core-identical: the first
remaining difference on both is `<note>` vs `<forward>` (scoretk's explicit
voice padding), and on mscz-167 then the tuplet-bracket heuristic — both
reader/MusicXML-export work, not mis-serialised writer values. The 43 unopened
rows have the same grouping and count as in the baseline.

### Fifth pass — translation and writer causes (English)

Sections 1–7 of this document are now in English; no section number, path or
table was changed. This pass re-examined the 49/56 corpus scores and the
31/171 `.mscz` scores against the C++ writer of MuseScore 4.7.4. There is no
published specification of `.mscx`/`.mscz`, so every cause below cites the
upstream `engraving/rw/write/*.cpp` behaviour.

**Corpus — first core difference and the C++ writer of that element** (the
`scoretk` column is the side of the comparison written by scoretk):

| Score | First core difference | C++ writer | Cause / action |
| --- | --- | --- | --- |
| `moonlight` | `<note>` vs `<forward>` | `TWrite::write(const ChordRest*)` (`write/twrite.cpp:1090-1105`) | reader/MusicXML: scoretk pads voices with explicit `<forward>`; 98 core lines remain after the `<numberType>` fix. Not a MuseScore writer value. |
| `testHarmony6` | `<function>m</function>` vs `<numeral>` | `TWrite::write(const Harmony*)` (`write/twrite.cpp:1834-1868`), `writeHarmonyInfo` (`:1786-1826`) | **writer, fixed**: the pivot keeps `<function>` but no `rootTpc`; MuseScore 4.6+ only reads `<root>` in `<harmonyInfo>`. |
| `testTboxWords1` | `<measure-style><multiple-rest>` only in scoretk | `MeasureWrite::writeMeasure` (`write/measurewrite.cpp:45-56`) | model: mmRest count/range; enabling `createMultiMeasureRests` and adding the voice rest by hand did not restore parity. |
| `changevisibility` | `<accidental>natural</accidental>` only in scoretk | `TWrite::write(const Note*)` (`write/twrite.cpp:2456-2462`), `write(const Accidental*)` (`:707-715`) | reader: the accidental of a grace note inside an ornament is collapsed onto the main note. MuseScore's own writer emits the same `<Accidental>` element, so the writer is faithful. |
| `linked-ties` | `<tie type="start"/>` only in scoretk | `TWrite::write(const Tie*)` (`write/twrite.cpp:3388-3393`) | reader: the corpus ties a grace note (`<location><grace>0</grace>`) to the main note; the pivot has no grace-tie anchor. |
| `mmrest` | `<measure-style>` only in scoretk | `MeasureWrite::writeMeasure` (`write/measurewrite.cpp:45-56`), `StaffWrite::writeStaff` (`:50-54`) | model: MuseScore rebuilds mmRests from consecutive measures when style `createMultiMeasureRests` is enabled (`style/styledef.cpp:571`); not a mis-serialised writer value. |
| `testExcludeInvisibleElements` | `<octave>4</octave>` vs `<octave>5</octave>` | `TWrite::write(const Ottava*)` (`write/twrite.cpp:2564-2574`), `SpannerWriter::fillSpannerPosition` (`write/connectorinfowriter.cpp:82-96`) | **writer, fixed**: a whole-measure offset was written as `<fractions>1/1</fractions>` instead of `<measures>1</measures>`; MuseScore shortened the ottava by one measure. |

**`<root>` from `<function>` — `testHarmony6`.** `writeHarmonyInfo` writes
`<root>` from `rootTpc`, and MuseScore's own reader converts a 3.x `<function>`
to `rootTpc` (`read410/tread.cpp:2839`) via `function2Tpc`
(`dom/pitchspelling.cpp:726-754`) and `tpcInterval` (`:658-669`). The writer now
does the same conversion using the key signature in force at the harmony.
Reduced test: `fonction_nashville_ecrit_une_racine`, in ordinary music (bars,
chords, lyrics, time signature), with a non-concerned Roman harmony that must
not gain a root. Reopened in MuseScore 4.7.4, `testHarmony6` is core-identical.

**`<measures>` of a spanner — `testExcludeInvisibleElements`.**
`SpannerWriter::fillSpannerPosition` stores `Location::measure()` and
`Location::frac()` separately, and `TWrite::write(const Location*)`
(`write/twrite.cpp:2355-2368`) writes `<measures>` and `<fractions>` as two
distinct tags. scoretk converted the whole tick delta to a fraction, so the
ottava ended one measure early (`ConnectorInfoReader`,
`read460/connectorinforeader.cpp:346`). The writer now derives the measure
index and the within-measure fraction of each endpoint. Reduced test:
`ottava_ecrit_ses_mesures_et_non_pas_une_fraction`, with a non-concerned
intra-measure tie that must stay in `<fractions>`. Reopened in MuseScore 4.7.4,
the score is core-identical.

**`.mscz` ranking.** Across the 171 written `.mscz`, the most frequent changed
core tags are `notations`/`slur` (149/145 files, layout), `measure` (35),
`measure-style`/`multiple-rest` (63), `note`/`forward` (82), `beam` (50),
`stem` (93) and `tuplet` (36). Restricting to files where MuseScore's own
import is core-identical and ours is not, the first differing lines group as
mmRest ranges (7 files), anacrusis measure numbers (6), explicit voice padding
(3), accidentals (5), Nashville harmony, clef octave, MIDI channel and tie
metadata. The two writer causes above apply to the same element families in
that lot; the rest need model or reader work (mmRest layout, anacrusis number,
voice padding), as documented in pass 3.

**Incomplete measures.** Reduced example `ly-3`
(`02_01_Miguel_Humberto_Sous-le-toit-la-tourterelle_partition.ly`, 715 bytes):
MuseScore reports `Incomplete measure, measure 32, staff 1. Found: 5/4.
Expected: 4/4` (and the same on most measures). The written `.mscx` starts
that voice with two `<location><fractions>1/8</fractions></location>` before
the first quarter chord; the model has the first chord at tick `1/4` with no
rest for the missing `1/4`. scoretk's MusicXML export makes that gap explicit
as `<forward>`, but the MuseScore writer follows `TWrite::writeVoiceMove`
(`write/twrite.cpp:3546-3576`) and writes only `<location>`. Replacing those
locations with a `gap` rest by hand does not restore parity (the other
locations are still counted), so the wrong object is the *model*: the reader
leaves a voice gap where MuseScore would have a rest. This is a reader cause
(`\partial`/voice padding in `scoretk-lilypond-import`), not a writer value;
no writer change was applied. The same mechanism explains the rest of the 23
`Incomplete measure` / 1 `Voice too long` rows.

### Fifth pass — final table

Binary `/tmp/scoretk-mscz-pass5-final` (sha256 `b957fb91…`). Same 638 scores,
same oracle, same tolerance. Note that master gained LilyPond reader fixes
between the previous table and this one; the *reference* MusicXML/MIDI of the
`.ly` lot therefore changes independently of the MuseScore writer.

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | **51/56** | 0.9702 | 0.9694 | 0 | 56/56 |
| mscz | 174 | 172 | 172 | **34/172** | 0.9763 | 0.9762 | 0 | 39/172 |
| xml | 32 | 27 | 27 | 0/27 | **0.9943** | **0.9868** | 0 | 0/27 |
| ly | 375 | 345 | 345 | 1/345 | 0.9985 | 0.9957 | 0 | 0/0 |
| abc | 1 | 1 | 1 | 0/1 | 1.0000 | 1.0000 | 0 | 0/0 |
| **Total** | **638** | **601** | **601** | **86/601** | **0.9808** | **0.9794** | **0** | **95/255** |

Compared with the previous table: **five files become core-identical**
(`testHarmony6`, `testExcludeInvisibleElements`, mscz-10, mscz-44, mscz-142),
16 files lose changed core lines (the three `Vingt danses` XML scores drop
from 169 to 53 each; the xml N2 rises from 0.3055 to 0.9868 because the
corrected spanner endpoints restore MuseScore's repeat/playback expansion),
and no file stops opening. Five files gain core lines: mscz-12 (+4, first
difference unchanged `<note>` vs `<forward>` — diff alignment after the
spanner rewrite, its direct MusicXML is byte-identical) and four `.ly`
(ly-14 +7, ly-166 +34, ly-255 +2, ly-316 +2) whose *direct* MusicXML also
changed between the two binaries because of the master reader fixes, so they
are not caused by this writer. The two N2 movements on ly-127/171
(180 → 174 notes) come from the same reader changes: N1 = N2 = ref on both
runs, i.e. still a perfect playback match. The 43 unopened rows keep the same
grouping; no writer-only failure was found.

### Sixth pass — reader/model causes and the anacrusis writer fix (English)

**Corpus — the five remaining files.** Each was reduced against the C++
reader; none is a mis-serialised writer value, so no writer change was made
for them (except the anacrusis property below, which concerns the `.mscz`
lot):

| Score | Remaining gap | C++ reader / model | Owner |
| --- | --- | --- | --- |
| `moonlight` | `<note>` vs `<forward>` (98 core lines) | `Measure::checkMeasure`/`fillGap` (`engraving/dom/check.cpp`), ported in `crates/scoretk-core/src/gap_rests.rs` | model: gap-rest durations differ from MuseScore's, so scoretk's export cursor inserts `<forward>` where MuseScore writes a rest |
| `testTboxWords1` | `<measure-style><multiple-rest>` only in scoretk | `TRead::read(Measure*)` (`read460/measureread.cpp:174-177`), `Measure::mmRestLast()` (`dom/measure.cpp:2875-2885`) | model: MuseScore rebuilds the mmRest from consecutive measures when style `createMultiMeasureRests` is set (`style/styledef.cpp:571`); the pivot stores one count on one measure |
| `changevisibility` | `<accidental>natural</accidental>` only in scoretk | `TRead::read(Note*)` (`read460/tread.cpp`), `TRead::read(Accidental*)` (`read460/tread.cpp`, `Pid::ACCIDENTAL_TYPE`) | reader: the grace chord inside `<Ornament>` is dropped while its accidental stays on the main note; MuseScore's own writer emits the identical `<Accidental>` element |
| `linked-ties` | `<tie type="start"/>` only in scoretk | `TRead::read(Tie*)` (`read460/tread.cpp`), `ConnectorInfoReader::readEndpointLocation` (`read460/connectorinforeader.cpp:120-130`) | model: the corpus ties a grace note (`<location><grace>0</grace>`) to the main note; the pivot has no grace-tie anchor |
| `mmrest` | `<measure-style>` only in scoretk | `TRead::read(Measure*)` (`read460/measureread.cpp:174-177`) | model: same mmRest range issue as `testTboxWords1`; layout style is not in the pivot |

For `testTboxWords1`, hand-enabling `createMultiMeasureRests` in both
`score.mscx` and `score_style.mss` makes MuseScore export three
`<multiple-rest>1</multiple-rest>` where scoretk writes one
`<multiple-rest>2</multiple-rest>`: the fix needs the mmRest range model, not
a writer attribute.

**Incomplete measures — reduced example.** `ly-3`
(`02_01_Miguel_Humberto_Sous-le-toit-la-tourterelle_partition.ly`, 715 bytes)
is read from LilyPond, not from `.mscx`. MuseScore reports `Incomplete
measure, measure 32, staff 1. Found: 5/4. Expected: 4/4`. The written voice
starts with two `<location><fractions>1/8</fractions></location>` before the
first chord: the model has a 1/4 voice gap with no rest (`s8*2` in the
`\chordmode` staff). `scoretk-core::gap_rests::fill` is called by the MuseScore
and MusicXML importers (`read/mod.rs:87`, `musicxml-import/src/lib.rs:116`)
but not by `scoretk-lilypond-import`; this is the precise cause to hand to the
LilyPond agent. It is already recorded in `docs/agents/lilypond.md` under
"The '+3.5 quarters' shift" (full-measure rest written with the pickup
measure length, NR §2.2.3/§2.3.3). No LilyPond crate was touched here.

**`.mscz` ranking and writer fix.** Across the 172 written `.mscz`, the most
frequent changed core tags remain `notations`/`slur` (layout), `measure` (24
files), `measure-style`/`multiple-rest` (63), `note`/`forward` (82) and
`tuplet` (36). Among files where MuseScore's own import is core-identical and
ours is not, the first difference is an anacrusis measure number on six files
(`<measure number="1">` vs `<measure implicit="yes" number="0">`), mmRest
ranges on six, voice padding on four, accidentals on three.

`MeasureWrite::writeMeasure` (`write/measurewrite.cpp:76`) writes
`Pid::EXCLUDE_FROM_NUMBERING` as `<irregular>`; the 4.6+ reader
(`read460/measureread.cpp:124-125`) restores `Measure::excludeFromNumbering()`.
The writer only emitted the `len` attribute, so MuseScore lost the anacrusis
flag on reopening. It now writes `<irregular>1</irregular>` whenever the pivot
sets it. Reduced test: `mesure_irreguliere_ecrit_sa_propriete`, in ordinary
music (voices, chords, rests, bars), with a non-concerned ordinary measure.
A hand patch of `tk.mscz` for mscz-1 confirmed that adding
`<irregular>1</irregular>` changes MuseScore's re-export from
`<measure number="1">` to `<measure number="0" implicit="yes">`.

### Sixth pass — final table

Binary `/tmp/scoretk-mscz-pass6-final` (sha256 `3d77a839…`). Same 638 scores,
same oracle, same tolerance. Master gained LilyPond reader commits between the
previous binary and this one (`6c5d873`, `ba4b690`, `60659a1`), so the
reference MIDI/MusicXML of the `.ly` lot changes independently of this writer.

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | 51/56 | 0.9702 | 0.9694 | 0 | 56/56 |
| mscz | 174 | 172 | 172 | **40/172** | 0.9763 | 0.9762 | 0 | 39/172 |
| xml | 32 | 27 | 27 | 0/27 | 0.9943 | 0.9868 | 0 | 0/27 |
| ly | 375 | 345 | 345 | 1/345 | 0.9822 | 0.9490 | 0 | 0/0 |
| abc | 1 | 1 | 1 | 0/1 | 1.0000 | 1.0000 | 0 | 0/0 |
| **Total** | **638** | **601** | **601** | **92/601** | **0.9796** | **0.9760** | **0** | **95/255** |

Compared with the previous table, **six `.mscz` become core-identical**
(mscz-1, 2, 11, 28, 41, 47 — all the anacrusis `<irregular>` cases) and 27
`.mscz` scores lose changed core lines. No score gains core lines and none
stops opening. The `.ly` N2 drop (0.9957 → 0.9490) is entirely one file,
`Angels_Carol.ly` (ly-255, 1617 → 20 matched notes): its *direct* MIDI from
scoretk differs between the two binaries because of the master LilyPond
reader commits above, which are not ancestors of the previous binary.
N1 = N2 on both runs; no MuseScore replay regression from this writer.

### Seventh pass — ornament intervals and grace-tie anchors (English)

Two of the reader/model causes left in §6 are now fixed; both are faithful
ports of the 4.7.4 code, not writer fudges.

**`changevisibility` — `Pid::INTERVAL_ABOVE`/`Pid::INTERVAL_BELOW`.**
`TRead::readProperties(Ornament*)` (`read460/tread.cpp:1868-1872`) reads the
turn's two intervals; the 3.x reader (`read302/notations.rs`) and the 4.x
reader (`read4xx/notations.rs`) ignored them, so the writer had nothing to
put back. `TWrite::writeProperties(Ornament*)` (`write/twrite.cpp`) writes
them as `<intervalAbove>`/`<intervalBelow>`. The pivot `Ornament` now carries
both optional strings, the two readers fill them and the writer emits them.
A hand patch adding only `<intervalAbove>second,major</intervalAbove>` and
`<intervalBelow>second,minor</intervalBelow>` to the written `.mscz` made
MuseScore restore the missing `<accidental>natural</accidental>`, which
identified the cause. Reduced test: `intervalles_dornement_lus_et_ecrits`,
in ordinary music (bars, chords, rests, lyrics, two ornaments), with the
trill's single interval as the non-concerned case. On `changevisibility`
reopened by MuseScore 4.7.4, the core diff is now 0.

**`linked-ties` — `Pid::LOCATION_GRACE`.**
`TWrite::write(const Location*)` (`write/twrite.cpp:2355-2368`) writes
`<grace>` from `Location::graceIndex()` (`dom/location.cpp:196-207`), which
`ConnectorInfoReader` uses to find the grace chord at the tie endpoint
(`read460/connectorinforeader.cpp:346-360`). scoretk resolved the tie to the
correct note IDs but wrote a location without `<grace>`, so MuseScore lost
the four tie tags on reopening. `WriteContext` now indexes the grace index of
each grace-chord note, and the tie writer emits `<grace>N</grace>` when the
other endpoint is a grace note. Reduced test:
`liaison_depuis_note_dagrement_ecrit_grace`, on the corpus plus ordinary ties
from `copypaste_parts` as the non-concerned case. On `linked-ties` reopened by
MuseScore 4.7.4, the core diff is now 0.

**`.mscz` re-ranking on the current binary.** After the `<irregular>` fix, the
`measure` first-difference group disappeared. The remaining first core
differences group as `attributes` (37 files, 1 worse than MuseScore's own
import), `measure-style`/`multiple-rest` (35, 6 worse), `note`/`forward`
(25, 4 worse), `tuplet` (18, 0 worse), `tie` (6, 0 worse), `sound` (3),
`accidental` (3) and singletons. By total changed lines, `notations`/`slur`
still dominate (about 70k/35k lines) but are identical to MuseScore's own
import on the same source: they are layout, not a serialisation error. The
actionable families — mmRest ranges, voice padding and the remaining
`attributes` case — need model work (mmRest range, gap-rest durations), as
recorded in §6; no further writer value was found by the C++ comparison.

### Seventh pass — final table

Binary `/tmp/scoretk-mscz-pass7-final` (sha256 `86571d73…`). Same 638 scores,
same oracle, same tolerance, no master change between the two binaries.

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | **53/56** | 0.9711 | 0.9702 | 0 | 56/56 |
| mscz | 174 | 172 | 172 | 40/172 | 0.9763 | 0.9762 | 0 | 39/172 |
| xml | 32 | 27 | 27 | 0/27 | 0.9943 | 0.9868 | 0 | 0/0 |
| ly | 375 | 345 | 345 | 1/345 | 0.9822 | 0.9490 | 0 | 0/0 |
| abc | 1 | 1 | 1 | 0/1 | 1.0000 | 1.0000 | 0 | 0/0 |
| **Total** | **638** | **601** | **601** | **94/601** | **0.9796** | **0.9760** | **0** | **95/255** |

Compared with the previous table, `changevisibility` and `linked-ties` become
core-identical (corpus 51 → 53, total 92 → 94). Twelve `.mscz` and three `.ly`
scores also lose changed core lines (four lines each for the `.mscz` grace-tie
group, larger gains for three `.ly` with ornament intervals). No file gains a
core line, no file stops opening, and N1/N2 are unchanged on every lot.

### Eighth pass — full ranking of the 132 non-identical `.mscz`

On the pass-7 binary, 40 of 172 written `.mscz` are core-identical after
reopening in MuseScore 4.7.4. The table below classifies **all 132** remaining
files by their *first* differing core line (element and attribute), not just
by the most frequent tag. Categories are mutually exclusive; "first" means
the first change in the shared core normalisation, so a file can have other
differences further down.

| Files | First differing core category (MuseScore re-export vs scoretk direct) |
| ---: | --- |
| 37 | `<attributes>` (key/time/clef change placement) |
| 35 | `<measure-style><multiple-rest>` (mmRest range) |
| 25 | voice gap: `<note>` vs `<forward>`/`<backup>` |
| 21 | `<tuplet bracket="…">` |
| 3 | `<sound tempo="…">` |
| 3 | `<accidental>` attributes (cautionary/parentheses vs bracket/editorial) |
| 3 | tie anchor (`<note>` vs `<tie type="start">`) |
| 1 | `<harmony>` |
| 1 | `<actual-notes>` |
| 1 | `<clef-octave-change>` vs `<transpose>` |
| 1 | `<midi-channel>` |
| 1 | `<staff-details>` |
| **132** | **total** |

The two largest actionable categories are the mmRest range and the tuplet
bracket; the `<attributes>` and voice-gap families are model/gap-rest work.

### Eighth pass — mmRest style and tuplet bracket type (English)

**`score_style.mss` — `createMultiMeasureRests` (35 files in the ranking).**
`MscLoader::read` (`rw/mscloader.cpp:86-94`) calls `MscReader::readStyleFile()`
(`infrastructure/mscreader.cpp:127-132`) and applies `score_style.mss` before
the score. The pivot kept only the inline `<Style>`, so the flag was lost on
`.mscz` input; `Measure::coveringMMRestOrThis()` (`dom/measure.cpp:2899`) then
refused to rebuild the mmRests and `measureStyle()` (`importmusicxml.cpp:6587`)
wrote no `<multiple-rest>`. The container now keeps `score_style.mss`, the
reader applies its `<Style>` through `StyleSettings::set_value`, and the writer
emits `<createMultiMeasureRests>1</createMultiMeasureRests>` when set
(`style/styledef.cpp:571`, default `false`). Reduced test:
`style_multi_repos_lu_et_ecrit`, on `mmrest` with `testNoteheads2` as the
non-concerned case. On mscz-34 reopened by MuseScore 4.7.4, the core diff is
now 0 (the only remaining differences in the local diff tool are its own
`<notations/>` rendering artefacts). The flag also removes the hidden
`<multiple-rest>` differences behind several files ranked under
`<attributes>`.

**`<bracketType>` of a tuplet (21 files in the ranking).** The C++ writer
`TWrite::write(const Tuplet*)` (`write/twrite.cpp:3500-3525`) emits the styled
property `Pid::BRACKET_TYPE` as `<bracketType>`: `SHOW_BRACKET = 1`,
`SHOW_NO_BRACKET = 2` (`types.h:1604-1607`). MuseScore 4.6+ ignores the 3.x
`<bracket>` tag and takes the bracket from `Tuplet::calcHasBracket()`
(`dom/tuplet.cpp:204-208`). scoretk wrote only `<bracket>`, so MuseScore
recomputed a different value. The writer now emits both tags and the 4.x
reader accepts `<bracketType>`. Reduced test: `type_crochet_nolet_ecrit_et_relu`,
on `testTuplets8` with a no-tuplet score as the non-concerned case. On mscz-0
reopened by MuseScore 4.7.4, the core diff is now 0.

The remaining top categories — `<attributes>` placement, voice gaps
(`<note>` vs `<forward>`) and the tuplet-bracket *heuristic* where no explicit
type existed — still need model work (gap-rest durations, beam-aware
`calcHasBracket`) and are not writer mis-serialisations.

### Eighth pass — final table

Binary `/tmp/scoretk-mscz-pass8b-final` (sha256 `1897ab76…`; the pass-8 run
timed out with six `.ly` left, which were measured with the same binary and
merged before recomputing the core table). Same 638 scores, same oracle, same
tolerance. Master also gained MIDI-only and LilyPond reader commits during
this pass, which change some *reference* outputs independently of the writer.

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | **54/56** | 0.9882 | 0.9873 | 0 | 56/56 |
| mscz | 174 | 172 | 172 | **52/172** | 0.9935 | 0.9934 | 0 | 39/172 |
| xml | 32 | 27 | 27 | 0/27 | 0.9981 | 0.9907 | 0 | 0/27 |
| ly | 375 | 345 | 345 | 1/345 | 0.9995 | 0.9968 | 0 | 0/0 |
| abc | 1 | 1 | 1 | 0/1 | 1.0000 | 1.0000 | 0 | 0/0 |
| **Total** | **638** | **601** | **601** | **107/601** | **0.9947** | **0.9932** | **0** | **95/255** |

Compared with pass 7: corpus 53 → 54 (`mmrest` becomes core-identical), mscz
40 → 52 (the mmRest and explicit tuplet-bracket files), total 94 → 107. No
file stops opening and no N1/N2 decreases. Three files gain changed core
lines: corpus-24 and mscz-149 (5 → 10 and 33 → 58) because the restored
`createMultiMeasureRests` style makes MuseScore build mmRests that the pivot
still does not model as `<multiple-rest>`; mscz-70 (661 → 979) because the
tuplet tag disappears from the diff and shifts its alignment, exposing the
unchanged `forward`/`slur` differences (same counts as before). The eleven
`.ly` regressions are caused by master's `a269f08`/`c31b250` LilyPond reader
commits, which change those scores' *direct* MusicXML; they are not produced
by this writer.

The first broad version of the `.mss` merge applied every style value and
regressed 68 `.mscz` files (e.g. mscz-57, 63 → 117 lines); it was corrected
before the final measurement to carry only `createMultiMeasureRests`, so the
inline `<Style>` keeps precedence as in `MscLoader::read`.

### Ninth pass — writer-only N2 in the oracle (English)

`mesure.py` now also asks MuseScore 4.7.4 to export MIDI from the *source*
(`alt.mid`) and compares it with its MIDI from our written `.mscz` (`ms.mid`).
The new N1w/N2w columns therefore measure only what our writer loses:
playback differences between scoretk and MuseScore on the same source are
excluded. `resume.md` additionally lists every file whose writer-only N2 is
below 1.0 with a short cause (octave, transposition, missing notes, shifted
attacks); MuseScore cannot import the LilyPond source, so N1w/N2w are `—` on
the `.ly` lot.

On the pass-8b rows (written before the current master fixes): corpus
N2w 0.9733 (3 files below 1.0, all octave), mscz N2w 0.9936 (12 files),
xml N2w 0.9276 (3 files), total 0.9871. The writer is thus much closer to
MuseScore's own playback than N2 (0.9932) suggested: most of the remaining
N2 gap is scoretk's own source interpretation, not a loss in `.mscz`.

**Categories re-examined in this pass.** The `<attributes>` first difference
is, on inspection, mostly a hidden `<measure-style><multiple-rest>`: extending
`assign_multiple_rests` to every isolated empty measure (read4xx/read302) was
tried with a reduced test (`multi_repos_isole`) and hand-checked on mscz-36/39
and 62-65; it improved mscz-36 but worsened four other scores, so it was
reverted. Writing gap rests as `<note print-object="no">` (instead of
`<forward>`, `importmusicxml.cpp:4571-4650`) was also tried with a reduced test
(`silences_gap`); it made mscz-6 core-identical but regressed the MuseScore
corpus reference test `elements_instruments_et_harmonies`, so it was reverted
too. Neither category has a safe fix yet; both need the model to distinguish
MuseScore-created gap rests and mmRest ranges from ordinary rests, which is
out of scope for a localised writer/reader patch.

### Tenth pass — style `concertPitch` and `startWithMeasureOne` (English)

Two writer/reader omissions found while investigating the writer-only N2
losses were fixed; both have a reduced test written first and cite the 4.7.4
code.

**`<concertPitch>` (style).** `Sid::concertPitch` (`style/styledef.cpp:567`,
default `false`) belongs to `<Style>`, which `MscWriter::writeStyleFile`
(`infrastructure/mscwriter.cpp:135`) writes; the pivot already read it through
`StyleSettings::set_value` but the writer never emitted it. On mscz-165, whose
source has `<concertPitch>1</concertPitch>`, adding that tag by hand to the
written `.mscz` reduced the core diff from 121 to 89 lines. Reduced test:
`style_concert_pitch_lu_et_ecrit`, on `voices` with the unset case as the
non-concerned control.

**`<startWithMeasureOne>` of a section break.** `Pid::START_WITH_MEASURE_ONE`
(`dom/property.cpp:468`) is read by `TRead::read(LayoutBreak*)`
(`read460/tread.cpp:2971`) and used by `MeasureBase` to restart numbering at 1
(`dom/measurebase.cpp:332`). scoretk's reader ignored it and its writer emitted
only `<subtype>section</subtype>`. `Measure` now carries the flag on the
measure the break applies to; the reader fills it and the writer emits it.
Reduced test: `saut_de_numero_mesure`, with a plain section break as control.
Three `.mscz` scores (mscz-31, 76, 77) become core-identical.

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | N1w | N2w | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | 54/56 | 0.9882 | 0.9873 | 0.9836 | 0.9733 | 0 | 56/56 |
| mscz | 174 | 172 | 172 | 49/172 | 0.9974 | 0.9973 | 0.9987 | 0.9936 | 0 | 40/172 |
| xml | 32 | 27 | 27 | 0/27 | 0.9983 | 0.9908 | 0.9066 | 0.8992 | 0 | 0/27 |
| ly | 375 | 345 | 345 | 1/345 | 0.9997 | 0.9971 | — | — | 0 | 0/0 |
| abc | 1 | 1 | 1 | 0/1 | 1.0000 | 1.0000 | — | — | 0 | 0/0 |
| **Total** | **638** | **601** | **601** | **104/601** | **0.9977** | **0.9963** | **0.9816** | **0.9761** | **0** | **96/255** |

Against the previous table, N1/N2 improve strongly (total N2 0.9932 →
0.9963) because master gained MIDI fixes; writer-only N2w is unchanged on
corpus and `.mscz` (0.9733 and 0.9936) and drops on `xml` (0.9276 → 0.8992)
because master also changed the MusicXML reader, not because of these two
writer fixes. Three `.mscz` scores lose core-identical status (mscz-93, 94,
157): their first difference is `<staff-details>` written by scoretk after
master's `Staff::visible` fix and not reproduced by MuseScore's re-export —
an external change, not a regression from `concertPitch` or
`startWithMeasureOne` (neither tag is present in those three sources).

### Eleventh pass — bisecting the reported regressions (English)

The two regressions reported after pass 10 were traced before changing code.

**Three `.mscz` losing core identity (mscz-93/94/157).** Bisected with the
oracle as the test (`direct has <staff-details> AND MuseScore's re-export has
none`) between `e62e557` and the rebased master. The first bad commit was
`926b862b088fc6324485195b5fd2f5ae9d9cb1b3` ("Visibilité de portée : round trip
.mscx et lecture MusicXML"), which added the `<isStaffVisible>0</isStaffVisible>`
writer (`TWrite::write(Staff*)`, `write/twrite.cpp:2945-2947`). On the current
rebased tree the criterion no longer holds: writing the `.mscz` for mscz-93 and
reopening it yields 24 `<staff-details>` as in the good revision. The pass-10
binary had been built before those master fixes; the "regression" was an
outdated binary, not a defect in the merged code.

**`xml` N2w 0.9276 → 0.8992.** No file's N2w changed between the two runs
(file-by-file comparison is identical). The count of measured files went from
25 to 27: two more MusicXML scores became importable, among them the very long
`Vingt danses` files whose writer-only N2 is 0.8738. Their inclusion lowers the
lot average; it is increased coverage, not a playback regression.

**Octave-classified losses.** The `octave` label from `midi_notes` is a pitch
multiset heuristic, not proof of an octave bug: on corpus-8/32/50 and mscz-33
the actual missing notes are fast glissando/trill notes (0.035–0.12 s) and
notes whose duration was written as one long instead of many short values.
Adding `<diagonal>1</diagonal>` to the written `<Glissando>` (the `SLine`
property written by `TWrite::writeProperties`, `write/twrite.cpp:1619`) by hand
did not change playback, so no speculative writer change was made for it.
The one genuine octave-classified cause already fixed is
`<concertPitch>` (`Sid::concertPitch`, `style/styledef.cpp:567`,
`MscWriter::writeStyleFile`, `infrastructure/mscwriter.cpp:135`): mscz-165 has
`<concertPitch>1</concertPitch>` in its source, and the fix (commit b16d51b)
reduced its core diff from 121 to 89 lines in a hand check.

### Eleventh pass — final table

Binary `/tmp/scoretk-mscz-pass11-final` (sha256 `c4e3ff8e…`), full 638-score
run on the rebased tree. The table is identical to pass 10 on every fidelity
column, confirming that the reported xml N2w and `<staff-details>` regressions
were artifacts of the older binary and of counting two additional MusicXML
files, not losses introduced by the two writer fixes.

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | N1w | N2w | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | 54/56 | 0.9882 | 0.9873 | 0.9836 | 0.9733 | 0 | 56/56 |
| mscz | 174 | 172 | 172 | 49/172 | 0.9974 | 0.9973 | 0.9987 | 0.9936 | 0 | 42/172 |
| xml | 32 | 27 | 27 | 0/27 | 0.9983 | 0.9908 | 0.9066 | 0.8992 | 0 | 0/27 |
| ly | 375 | 345 | 345 | 1/345 | 0.9997 | 0.9971 | — | — | 0 | 0/0 |
| abc | 1 | 1 | 1 | 0/1 | 1.0000 | 1.0000 | — | — | 0 | 0/0 |
| **Total** | **638** | **601** | **601** | **104/601** | **0.9977** | **0.9963** | **0.9816** | **0.9761** | **0** | **98/255** |

The only change is the alternative column (96 → 98 core-identical), from the
master staff-visibility fixes. No file loses a core line or writer-only N2
compared with pass 10. The remaining octave-classified writer-only losses
need the model to distinguish MuseScore's playback of glissando/trill notes
from the long written values; no speculative writer change was applied.

### Twelfth pass — element inventory of the writer-only N2 losses (English)

Rather than trusting the `octave` label of `midi_notes`, the element inventory
of the source `.mscx` and of our written `.mscx` was compared for every file
with writer-only N2 below 1.0 (first `<Score>` only, since some sources contain
linked excerpts; mscz-166 was excluded as its source has nine linked scores).
Elements lost by the rewrite, by count and number of files: `Beam` 2334 (9),
`Note`/`Chord` about 1400 each (mostly the excluded multi-score file), `Tuplet`
222 (3), `Slur` 146 (3), `Symbol` 53 (3), `Articulation` 9 (2), `Trill` 11,
`Glissando` 4, `Ornament` 1. Beams and symbols do not change playback; the
playable elements actually missing were legacy 2.x `<Trill>` and `<Glissando>`
spanners, which the 2.x reader ignored.

**Legacy `<Trill id>` (mscz-33).** `Read206::readTrill206`
(`read206.cpp:2362-2376`) and `readMeasure206` (`:2871-2892`) pair a 2.x
`<Trill id>` with its `<endSpanner id>`. scoretk's 2.x reader had no `Trill`
spanner family, so all 11 trills of Hallelujah disappeared from the written
`.mscx` and MuseScore could not play their notes. `SpanFamily::Trill` now
reads the subtype (`notation_names::trill_type`) and `fix_spanners` builds a
`SpannerType::Trill` with `SpannerNotation::Trill`; `TWrite::write(const Trill*)`
(`write/twrite.cpp:3490-3494`) is then reproduced by the writer. Reduced test:
`trill_206`, with a score without trill as the control. On mscz-33 reopened by
MuseScore 4.7.4, writer-only N2 goes from 4204/4321 (0.9729) to 4321/4321
(1.0000).

**Legacy `<Glissando id>` (mscz-8).** The same 2.x coupling applies to
`<Glissando id>` (`read206.cpp:2871-2892`); it can be carried by a `<Note>`
(`readNoteProperties206`, `:1221`), as in the corpus. The 2.x reader now
collects the subtype, line style and text and builds a `SpannerType::Glissando`
with `SpannerNotation::Glissando`; the writer emits the corresponding
`<Spanner type="Glissando">`. Reduced test: `glissando_206`, with a score
without glissando as the control. The rewritten mscz-8 now carries four
glissando spanners instead of zero (its writer-only N2 stays at 0.9852: the
remaining loss is the glissando note-generation itself, not the spanner).

### Twelfth pass — final table

Binary `/tmp/scoretk-mscz-pass12-final` (sha256 `80df43de…`), full 638-score
run. Writer-only N2 improves on mscz-33 (0.9729 → 1.0000) and no file loses
writer-only N2.

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | N1w | N2w | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | 54/56 | 0.9882 | 0.9873 | 0.9836 | 0.9733 | 0 | 56/56 |
| mscz | 174 | 172 | 172 | 42/172 | 0.9967 | 0.9967 | 0.9990 | 0.9939 | 0 | 56/172 |
| xml | 32 | 27 | 27 | 0/27 | 0.9983 | 0.9908 | 0.9066 | 0.8992 | 0 | 0/27 |
| ly | 375 | 358 | 358 | 1/358 | 0.9913 | 0.9889 | — | — | 0 | 0/0 |
| abc | 1 | 1 | 1 | 0/1 | 1.0000 | 1.0000 | — | — | 0 | 0/0 |
| **Total** | **638** | **614** | **614** | **97/614** | **0.9965** | **0.9951** | **0.9819** | **0.9763** | **0** | **112/255** |

The core-identical count drops from 49 to 42 on the `.mscz` lot, with seven
scores gaining 2–36 core lines whose first difference is the tuplet bracket.
Between the two binaries master merged `8c539b1` ("MusicXML : crochet des
n-olets d'après calcHasBracket"), which changed our *direct* MusicXML bracket
values independently of the two 2.x reader fixes; none of those seven sources
has a legacy `<Trill>` or `<Glissando>`. The `.ly` lot gains 13 openable scores
(345 → 358) from master's LilyPond reader fixes, which also explains its
slightly lower N1/N2 averages (more, harder files are now measured). Neither
change is caused by the trill/glissando reader commits. The oracle reference
was regenerated with `scripts/oracle/refresh-reference.sh` before these code
changes and must be refreshed again afterwards so `compare.sh` remains
meaningful.

### Thirteenth pass — the seven lost `.mscz` are a round-trip bracket bug (English)

A fresh full measurement on current master (binary `9e8ff1c2…`) confirmed that
the seven `.mscz` scores were still lost: 42/172 core-identical, mscz-18 first
difference `<tuplet bracket="no">` (MuseScore's re-export) vs
`<tuplet bracket="yes">` (scoretk's direct MusicXML). Master's `8c539b1`
introduced `Tuplet::bracket_explicit` and made the MusicXML exporter compute a
non-explicit bracket with `tuplet_bracket::has_bracket` (`tuplet.cpp:204-208`
via the C++ port). The MuseScore writer, however, always wrote `<bracket>` and
`<bracketType>`, turning an implicit bracket into an explicit one in our
`.mscz`. MuseScore then kept our value instead of recalculating it: on mscz-18
it re-exported 12 `bracket="no"` where both the source and scoretk's direct
export have 12 `bracket="yes"`.

Fix: `TWrite::write(const Tuplet*)` (`write/twrite.cpp:3500-3525`) emits
`Pid::BRACKET_TYPE` only when the property is explicit; the writer now does the
same. Reduced test: `crochet_nolet_implicite`, on `testTuplets8` with an
explicit-bracket score as control; the older `type_crochet_nolet` test was
updated to set `bracket_explicit` on the two tuplets it edits. After the fix,
mscz-18's re-export has 12 `bracket="yes"` matching its direct export.

The element inventory of the remaining writer-only N2 losses was continued:
after the 2.x trill/glissando fixes, the lost playable elements are mostly
notes/tuplets of linked excerpts (mscz-166, out of scope: the pivot writes one
score), plus `Slur` (3 files), `Articulation` (2) and `Symbol` (3, layout only).
The next actionable writer cause is not a missing element but the bracket
round trip above.

### Fourteenth pass — final table

Binary `/tmp/scoretk-mscz-pass14-final` (sha256 `892cf991…`), full 638-score
run with the implicit-bracket fix.

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | N1w | N2w | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | 54/56 | 0.9882 | 0.9873 | 0.9836 | 0.9733 | 0 | 56/56 |
| mscz | 174 | 172 | 172 | **50/172** | 0.9967 | 0.9967 | 0.9990 | 0.9939 | 0 | 58/172 |
| xml | 32 | 27 | 27 | 0/27 | 0.9983 | 0.9908 | 0.9066 | 0.8992 | 0 | 0/27 |
| ly | 375 | 358 | 358 | 1/358 | 0.9913 | 0.9889 | — | — | 0 | 0/0 |
| abc | 1 | 1 | 1 | 0/1 | 1.0000 | 1.0000 | — | — | 0 | 0/0 |
| **Total** | **638** | **614** | **614** | **105/614** | **0.9965** | **0.9951** | **0.9819** | **0.9763** | **0** | **114/255** |

Compared with the fresh master baseline (42/172), **eight `.mscz` scores
become core-identical** and 21 lose changed core lines; no writer-only N2
decreases on any lot and no file stops opening. Two scores gain 4 and 14 core
lines: mscz-150 (first difference moves from `<staff-details>` to a tuplet
bracket, i.e. a diff-alignment shift after the bracket fix) and mscz-167
(measure numbering `1` vs `454`, a pre-existing section-numbering difference
unchanged by the fix). Neither is a musical loss.

### Fifteenth pass — inventory of the remaining writer-only N2 losses (English)

On the pass-14 binary, 19 files still have writer-only N2 below 1.0. The
element inventory of the source and of the written `.mscx` (first `<Score>`
only) gives, biggest first:

| Files | Lost / altered element in the written file |
| --- | --- |
| 3 `Vingt danses` `.mxl`, `MarieJeanneGabrielle.mxl`, `bovet_fanfare.xml` | no element lost; MuseScore's repeat/playback expansion differs (long source notes become short written notes) — reader/playback model, not a writer element |
| `mscz-6`, `mscz-8`, `mscz-10` | 2.x legacy `<Tuplet id>` references (definitions are preserved), slurs and beams; the fast notes are generated by MuseScore's playback of ornaments, not by a missing spanner |
| `mscz-165`, `corpus-50` | 7 and 2 `<Articulation><subtype>ornamentTrill>` rewritten as `<Ornament><subtype>ornamentTrill>`; hand-restoring `<Articulation>` did not change N2w (0.4071 and 0.7867 unchanged) |
| `mscz-9`, `mscz-54`, `mscz-55` | no element lost; see the direct-path check below — the apparent three-quarter shift is not a writer offset |
| `corpus-8`, `corpus-32` | `Symbol` (layout), one `Ornament`, fast notes generated by a `prall` ornament in MuseScore's playback |

**Direct-path check for `mscz-9`, `mscz-54`, `mscz-55` (2026-11-21).** On the
read → MusicXML path, `N1 = N2 = ref` for all three (1161, 339 and 755 notes):
there is no three-quarter shift in our reader or in the direct export. The
shift only appears in `N2w` after MuseScore plays our rewritten `.mscz`.
Comparing the two playback MIDIs note by note, the pitches and onsets match;
the differences are sub-quarter durations and missing/short grace notes (for
example `mscz-9`: 0.4979 quarter in the reference against 0.4729 in ours at
the same onset, repeated throughout). This is MuseScore's playback of
durations and ornaments, not a misplaced jump or repeat, so there is no safe
local writer fix here; it belongs to the reader/playback campaign, not the
`.mscx` writer. No writer change was attempted for these three files.

Two writer/reader experiments were tried with failing tests first and then
reverted because they produced **no measured gain**: writing the gap rests as
`<note print-object="no">` (breaks `instruments_corpus`) and emitting
`<diagonal>` for `<Glissando>` (N2w of mscz-8 stays at 0.9852). The remaining
losses need MuseScore's playback model (ornament/grace generation, repeat
expansion, hidden-stave layout), not another local writer property. The
pass-14 table is therefore still the current baseline; no further writer
cause with a measured gain could be identified in this pass.
