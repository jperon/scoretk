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
