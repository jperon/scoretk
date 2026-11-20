# Musical-core discrepancies on the v4 corpus

This document inventories the discrepancies reported by the end-to-end test
`crates/scoretk/tests/e2e_corpus.rs` between the conversion `X.mscx` →
MusicXML and the `X.ref.musicxml` reference exported by MuseScore 4.7.4.

Method: both documents are normalised (metadata, layout,
`<direction>`, `<stem>`, `<beam>`, `<print>`, `<notations>` except `tied`/`tuplet`,
`<barline>` except `<repeat>` removed), then compared line by line with `similar`.
The test `crates/scoretk/tests/noyau_corpus.rs` additionally compares, file by
file and measure by measure, only the elements of the core family (pitches,
`octave-shift`, n-tuplets, `measure-style`, part names, volume/pan, ties,
`sound tempo`, `repeat times`), with an explicit expected discrepancy count.
The table below gives, for each file, the number of differing lines after
normalisation; files marked **identical** constitute the test's
`EXPECTED_CONFORMANT` list.

Discrepancies sorted by frequency (number of affected files), then by number
of occurrences. Causes are classified as "reader", "model" or "export".
Discrepancies 1 to 14 are fixed (see each section), as are discrepancy 16
and those of the 3.x corpus listed below.

## Compliance table

| File | Differing lines |
| --- | ---: |
| beamStemDir | **identical** |
| changevisibility | **identical** |
| copypaste_parts | **identical** |
| copypastesymbollist-lyrics | **identical** |
| dynamics | **identical** |
| dynamics_niente | **identical** |
| expression-1 | **identical** |
| hairpin_dynamic_inside | **identical** |
| hairpin_dynamic_jump | **identical** |
| hairpin_simple | **identical** |
| hairpins_and_repeats | **identical** |
| linked-ties | **identical** |
| mmrest | **identical** |
| part-textlines | **identical** |
| partExclusion | **identical** |
| repeat65 | **identical** |
| repeat66 | **identical** |
| repeat68 | **identical** |
| repeat69 | **identical** |
| tenuto_accent | **identical** |
| testExcludeInvisibleElements | **identical** |
| testLocalTimesig1 | **identical** |
| testNoteheads2 | **identical** |
| testTempoPrecision | **identical** |
| timesig-09 | **identical** |
| voiceswitching | **identical** |


## 1. Missing default clef — 14 files

**Fixed** (reader/model/export): `Staff::default_clef` reads
`<defaultClef>` (or its concert/transposing variants) and export writes a
clef of the staff's default type when no `<Clef>` opens the score,
as MuseScore's layout does.

Files: beamStemDir, changevisibility, copypaste_parts, dynamics,
hairpin_dynamic_inside, linked-ties, mmrest, part-textlines, repeat65,
tenuto_accent, testExcludeInvisibleElements, testLocalTimesig1, timesig-09,
voiceswitching.

Diff (beamStemDir):

```diff
-        <clef number="1">
-          <sign>G</sign>
-          <line>2</line>
-        </clef>
-        <clef number="2">
-          <sign>F</sign>
-          <line>4</line>
-        </clef>
```

These `.mscx` contain no `<Clef>` element: the clef comes from
`<defaultClef>F</defaultClef>` in `<Staff id="2">` (multi-staff parts) or from
the default "treble" clef of a single staff. **Cause: reader/model.**
`read4xx` inserts a `HeaderClef` segment only when it encounters
a `<Clef>`; `<defaultClef>` is not read and the model has no per-staff
default clef. Export therefore cannot write any clef (its "treble"
fallback assumes an existing `HeaderClef` segment), whereas MuseScore creates
that clef on reading (`read400`).

## 2. `<instrument-name>` taken from the long name instead of the track name — 3 files

**Fixed** (reader/model/export): `Instrument::track_name` reads
`<trackName>` and `<instrument-name>` writes it as-is, like
`MScoreTextToMusicXml::toPlainText(instr->trackName())`; the long name is no
longer a fallback.

Files: hairpin_dynamic_inside, part-textlines, testLocalTimesig1.

Diffs:

```diff
-        <instrument-name></instrument-name>
+        <instrument-name>Steel Guitar</instrument-name>
```

```diff
-        <instrument-name>B♭ Clarinet</instrument-name>
+        <instrument-name>Bb Clarinet</instrument-name>
```

```diff
-        <instrument-name>Piano</instrument-name>
+        <instrument-name>Piano 1</instrument-name>
```

MuseScore writes the instrument's `<trackName>` into `<instrument-name>`, not
its `<longName>` (hence the empty string for `hairpin_dynamic_inside`, whose
`<trackName>` is empty, and "Piano" for `testLocalTimesig1`, whose
`<longName>` is "Piano 1"). **Cause: reader/model.** `read_instrument`
does not store the track name (explicit TODO); export falls back to
`instrument.long_name`. The same flaw explains "Bb" instead of "B♭"
(the long name is already stripped of the flat by `plain_text`).

## 3. Noteheads: wrong name, named heads lost — 3 files

**Fixed** (reader/model/export): `Note` now distinguishes the head
group (`head_group`, `NoteHeadGroup`) from the type (`head`), and `visible`.
Export reproduces `writeNotehead`: MusicXML value of the group (`x`, `cross`,
`circle-x`, `slashed`, `triangle`, `inverted triangle`, solfège…), SMuFL
name with `other` for groups without an equivalent, `none` for an invisible note,
and the `filled` attribute from `<headType>`.

Files: testNoteheads2 (conformant), voiceswitching, testExcludeInvisibleElements.

Diff (testNoteheads2):

```diff
-        <notehead>x</notehead>
+        <notehead>cross</notehead>
...
-        <notehead>circle-x</notehead>
+        <notehead>x</notehead>
...
-        <notehead>slashed</notehead>
+        <notehead>slash</notehead>
...
-        <notehead>so</notehead>
```

Two problems compound:

- **reader/model**: `names::note_head` folds distinct MuseScore heads
  (`cross`, `xcircle`, `withx`, `slashed1/2`, `diamond-old`, `large-arrow`,
  `plus`, `circled`, solfège heads `do/ré/mi/fa/sol/la/ti`, `none`…) into
  a few model variants; `NoteHeadType::Other(u8)` does not keep the
  original name;
- **export**: `write_notehead` rewrites these variants with approximate MusicXML
  names (`Cross` → `cross` instead of `x`, `X` → `x` instead of
  `circle-x`, `Slash` → `slash` instead of `slashed`) and emits nothing for
  `Other`, `none` or named heads.

## 4. Actual duration (`<duration>`) ignored: duration, divisions, `time-modification`, `backup` — 2 files

**Fixed** (reader/model/export): `ChordRest::ticks` keeps the
explicit `<duration>`, `Staff::time_stretch` the local direction stretch,
and `Chord::tremolo_two_chord` the two-note tremolo (matching the
following chord in the same voice, like `Measure::connectTremolo`). Export
writes `<duration>` from the real ticks, advances position with
`actualTicks` (ticks / stretch), counts half a tremolo in
`calcDivisions` and adds `<time-modification>` 2:1 for a tremolo.

Files: timesig-09 (conformant), copypaste_parts (only discrepancy 5 remains).

Initial diff (timesig-09):

```diff
-        <duration>2</duration>
+        <duration>4</duration>
         <voice>1</voice>
         <type>whole</type>
-        <time-modification>
-          <actual-notes>2</actual-notes>
-          <normal-notes>1</normal-notes>
-        </time-modification>
       </note>
+      <backup>
+        <duration>2</duration>
+      </backup>
```

These chords carry `<durationType>whole</durationType>` **and**
`<duration>1/2</duration>` (a whole note stretched to a double whole). `read_chord`
reads `<duration>` into `ParsedChord::ticks` and advances the cursor
accordingly; the model kept only `duration` (the note type) and export
recomputed the duration with `duration.ticks()`, hence the missing
`<time-modification>` 2:1, incorrect `backup`s and wrong divisions.

## 5. `Ottava` spanner not applied to pitch — 2 files

**Fixed** (reader/export): `read4xx` pairs the two halves of
`<Spanner type="Ottava">` (new `read/read4xx/ottava.rs`) and stores them in
`Score::spanners`; `read302` already did so. Export shifts the written pitch
by `ppitch - pitch` (one or two octaves) like `pitch2xml`, and writes
`<direction><direction-type><octave-shift>` at the start and the end, including
for an invisible ottava (the line disappears, the pitch stays shifted).

Files: copypaste_parts, partExclusion, testExcludeInvisibleElements.

## 6. `bracket` of n-tuplets — 1 file, 9 occurrences

**Fixed** (reader): readers compute the bracket in the absence of modelled
beams, following `Tuplet::calcHasBracket` and the corpus:

- 3.x: a simple n-tuplet has no bracket, an n-tuplet containing a nested
  n-tuplet has one, and the shortest inner level does not;
- 4.x: bracket by default, except for a nested n-tuplet with a base shorter
  than an eighth, or eighths with a reduced ratio.

File: voiceswitching (conformant). Same rule applied to
`testTuplets8`, `copypaste_tuplet_01` and `layout_elements` (v3, conformant).

## 7. Displayed numbers of reduced nested n-tuplets — 1 file, 3 occurrences

**Fixed** (model/readers/export): `Tuplet` carries
`actual_notes`/`normal_notes` (raw numbers from the file) in addition to the
reduced `ratio`; `<tuplet-actual>`/`<tuplet-normal>` write them as-is
(`Tuplet::displayed_numbers`). A `4:4` remains `4`/`4`.

File: voiceswitching (conformant).

## 8. Repeat count of an end barline — 1 file

**Fixed** (reader/model/export): `Measure::repeat_count` receives
`<endRepeat>N</endRepeat>` in both readers and `<repeat>` writes
`times="N"` beyond two plays. (Already fixed with repeats, see
`export/repeats.rs`.)

File: repeat65 (conformant).

## 9. Multi-measure rests (`<measure-style>`) — 1 file

**Fixed** (reader/model/export): `Measure::multi_measure_rest` receives the
counter of the `<multiMeasureRest>N</multiMeasureRest>` marker on the measure that
opens the group (markers follow their first measure in the `.mscx`).
When the `<createMultiMeasureRests>` style is active and the whole score
consists of empty measures, an isolated empty measure receives `1` (mmrest.mscx).
Export writes `<attributes><measure-style><multiple-rest>N`.

File: mmrest (conformant).

## 10. `<part-name-display>` / `<part-abbreviation-display>` — 1 file

**Fixed** (export): `writeDisplayName` splits names containing `♭`/`♯`
into `<display-text>` / `<accidental-text>`, for the long name and the abbreviation.
`<part-name>` stays plain text (`Bb Clarinet`).

File: part-textlines (conformant).

## 11. MIDI channel settings (volume) — 1 file

**Fixed** (reader/model/export): `Instrument::volume`/`pan` read
controllers 7 and 10 of `<Channel>`; export computes
`volume/127*100` (`79.5276` for 101) and
`int((pan-63.5)/63.5*90)`, with 100/64 by default.

Files: hairpin_dynamic_inside, hairpin_dynamic_jump, hairpin_simple
(conformant).

## 12. `<sound tempo>` of an invisible tempo — 1 file

**Fixed** (reader/model/export): `Tempo::visible` is read and an invisible
tempo exports only its standalone `<sound tempo>`, outside `<direction>`.

File: testExcludeInvisibleElements (conformant).

## 13. Ties: `<measures>` of `<location>` ignored — 1 file

**Fixed** (reader): `register_tie` keeps the complete `<location>`
(`<measures>`, `<fractions>`, `<staves>`, `<voices>`) and matching computes
the absolute position as `Location::toAbsolute`: relative position in the
target measure, shifted by `<measures>`.

Files: testExcludeInvisibleElements, dynamics_niente (conformant).

## 14. Per-staff local metre — 1 file

**Fixed** (reader/model/export): see discrepancy 4 (explicit `<duration>` and
`Staff::time_stretch`). testLocalTimesig1 is conformant.

## 15. Lyrics above the staff and credits without layout — fixed

**Fixed** (model, 3.x and 4.x readers, export):

- `Lyrics::placement` defaults to `below`, like `lyricsPlacement` in
  MuseScore (`engraving/dom/lyrics.cpp`, `propertyDefault(Pid::PLACEMENT)`).
  Both readers read `<Style><lyricsPlacement>` and take a syllable's
  `<placement>` only if present; export writes
  `placement="above"` only for an explicitly upper placement.
  Previously, every `<Lyrics>` without `<placement>` was read as "above".
- `StyleSettings` (new `model/style.rs`) keeps the page format,
  margins, credit font sizes and page count. Readers
  read `<Style>` (`Spatium`, `pageWidth`/`pageHeight`, `page*Margin`,
  `pageTwosided`, `lyricsPlacement`, `*FontSize`); missing values
  take MuseScore's defaults for the file's version (margins 10/20 mm
  in 3.x, 15 mm in 4.x, title 24 then 22 pt).
- Export writes `<defaults>` (`<scaling>`, `<page-layout>` and its
  `<page-margins>`) and positions `<credit>`s like `textAsCreditWords`:
  `default-x` from alignment and margins, `default-y` from the
  page height, the top margin, the subtitle offset (10 mm) and the
  `VBox` height for bottom alignments. Copyright is repeated on
  each page deduced from `<LayoutBreak>`s of type `page`.

Tests: `crates/scoretk/tests/credits_corpus.rs` compares credits, page
format and lyrics of the v3 and v4 corpora.

Remaining discrepancies, listed in this test:

- `<defaults>` does not yet write `<appearance>`, `<music-font>`,
  `<word-font>` or `<lyric-font>`: these blocks need about thirty style
  values (line widths, fonts) absent from the model.
- `moonlight.mscx`: **fixed** (text family) — text fragments
  are written raw (`XmlWriter::tag_raw`), so named entities (`&quot;`)
  are preserved as in MuseScore.
- `testBreaksPage.mscx`: a `VBox` placed after two systems is not at
  the top of a page; without a layout engine, its computed `default-y` is
  that of a frame at the top of a page (1626.67 instead of 1446.67, i.e. 18 sp).
- Page-format numbers are rendered to six significant digits like
  MuseScore; v3 corpus margins sometimes differ by 0.0001 tenth
  (`56.6893` instead of `56.6894`) from rounding of the style value.

## 16. Numbering after a section break — 2 files (v3 and v4)

**Fixed** (model, 3.x and 4.x readers, export): `Measure::section_break_before`
receives a `<LayoutBreak><subtype>section</subtype></LayoutBreak>` found in
the measure or in a frame (`HBox`/`VBox`). Export then restarts numbers
at `1` and rewrites section-start `<attributes>` (including `<divisions>`),
like `MeasureNumberStateHandler`.

Files: repeat66 (v4), keysig_78216 (v3).

## Discrepancies of the v3 corpus reported by `e2e_corpus.rs`

The end-to-end test covers `v3/` with the same normalisation. Conformant
files constitute `EXPECTED_CONFORMANT_V3` (Beam-CrossM1, barlinedelete,
change, clef-1, copypaste_tuplet_01, grace, keysig_78216, lyricsline01,
measure-4, repeat01, repeat06, repeat07, repeat11, repeat20, repeat39,
selectionfilter1, testBreaksPage, testChordSymbols, testHairpinDynamics,
testHarmony6, testMultiInstrumentPart3, testTboxWords1, testTuplets8,
testWedge4, timesig-05, undoResequenceMeasure, voices).

Points 1 to 5 and 9 to 12 are fixed; points 6 to 8 and 13 remain.

1. **`<instrument-sound>` and `<midi-program>` missing — 13 files.**
   **Fixed** (reader/model): `read/instruments.rs` carries MuseScore's
   instruments table (`share/instruments/instruments.xml`, generated by
   `scripts/gen-instruments-table.py`). For a file earlier than 3.6,
   `updateInstrumentId` is reproduced: recognition by track name then
   long/short names (weights 4/2/1), otherwise by MIDI program, otherwise
   `keyboard.piano`; the default program comes from the same instrument.
   `<midi-channel>` is read when the file sets it.
2. **Instrument changes — 3 files.** **Fixed** (model, readers,
   export, with the text family): `Part` carries successive instruments,
   `<InstrumentChange>` registers its own at the segment tick (`Segment::add`),
   and export writes one `<score-instrument>`/`<midi-instrument>` per instrument,
   `<instruments>` at the first measure, `<instrument>` per note, MIDI channels
   in order (including the "harmony" channel) and written pitch with the
   transposition in force. `writeInstrumentChange` (text family) writes
   `<print><part-name-display>`, `<transpose>` and `<sound><instrument-change>`.
   `change` and `testMultiInstrumentPart3` are conformant.
3. **3.x harmonies — 4 files.** **Fixed** (model/reader/export):
   `Harmony` keeps the suffix (`<name>`), the type (`normal`/`roman`/`nashville`),
   the function and the `<degree>`s. `export/harmony.rs` parses the suffix as
   `ParsedChord::parse` (common qualities and extensions), writes
   `<root>`/`<kind>` (+ text, symbols, parentheses, degrees),
   `<function>`/`<numeral>` of roman and Nashville figures, the bass and
   `<offset>`. A chord without a note is attached to the previous segment with its
   offset, like `harmonies()`.
4. **Clef of the second staff — 2 files.** **Fixed** (reader):
   `readVoice` places a voice's first clef at the start of measure in a
   `HeaderClef` segment even if another staff has already set the metre.
   `clef-1` and `moonlight` are conformant for this point.
5. **Numbering after a section break — 1 file.** **Fixed**, see
   discrepancy 16.
6. **Tie orientation — 1 file, 7 occurrences.** **Open**:
   `moonlight` writes `<tied orientation="over|under">` from the `TieSegment`
   computed by layout; the model has no such direction (it is not
   in the `.mscx`). Orientation is therefore absent from the output.
7. **Staff and filler details — 1 file.** **Partially fixed**:
   `<staffMove>` places cross-staff notes on the other staff (correct `<staff>`)
   and a user `<role>` on an accidental produces `cautionary="yes"
   parentheses="no"` (or `editorial` with brackets). **Remaining** are
   split `<forward>`/`<backup>` at the end of measures (`36` instead of `12`+`24`),
   which come from segment splitting by layout.
8. **`<measure-repeat>` — 1 file.** **Open**: `layout_elements` copies
   repeated measures instead of writing
   `<measure-style><measure-repeat>`.
9. **`<key number="1">` — 1 file.** **Fixed** (export): a key is
   common (without `number`) only if all staves of the part have the same
   one (`keysigTimesig`).
10. **`<part-group>` — 1 file.** **Fixed** (model/readers/export):
    `<bracket>` and `<barLineSpan>` are read; `part-group` opens and closes
    groups like `partGroupStart`, filtering the implicit brace of a
    multi-staff part.
11. **Cautionary accidentals — `moonlight`.** **Fixed**, see point 7.
12. **Cross-staff notes — `moonlight`.** **Fixed**, see point 7.
13. **`<staff-details/>` of hidden staves — `barline01`.** **Open**:
    measures 5 and 7 hide an empty staff (`hideEmptyStaves`), which MuseScore
    signals with `<staff-details print-object="no"/>`. The model does not read
    `<hideEmptyStaves>` and the decision depends on layout (zero
    distance between staves).

Remaining `layout_elements` discrepancies, outside the instruments and chords families
(see discrepancies 8 and 18): `<figured-bass>`, `<notehead parentheses="yes">` and
`<measure-repeat>`.

## Dedicated tests

- `crates/scoretk/tests/instruments_corpus.rs` compares, for each file
  of the v3 and v4 corpora, the `<score-instrument>`/`<midi-instrument>`,
  `<part-group>`, measure numbers, `<attributes>` (`instruments`, `key`,
  `clef`, `transpose`, `staff-details`), `<harmony>`, `<instrument>` and
  notes' `<staff>`, and accidentals. The remaining discrepancies are listed there
  (instrument-change `transpose`, `staff-details`) and filtered.
- `crates/scoretk-musicxml-export/src/tests.rs` covers hand-built scores with
  multi-instrument parts, numbering after a section break
  and cautionary accidentals.
- `crates/scoretk/tests/texts_corpus.rs` and
  `crates/scoretk/tests/notations_corpus.rs` cover the text and
  notation families (see discrepancies 17 and 18).

## 17. Line texts, `TBox` frames, instrument changes, swing — text family

**Fixed** (reader/model/export) and covered by
`crates/scoretk/tests/texts_corpus.rs` on the **v3 and v4** corpora (the v4 e2e
ignores `<direction>` and `<print>`):

- **line texts** (`<Spanner type="TextLine">`): `TextLineInfo` carries
  `lineVisible`, `beginText`/`endText`, line style, hooks, hook
  heights, fonts and `system`. Export reproduces `ExportMusicXml::textLine`
  and `writeHairpinText`: start/end `<words>`, `<bracket type=… number=…
  line-end=… line-type=… end-length=…>`, `<dashes>` for a line without
  hooks, and the negative `<offset sound="yes">` of invisible line ends
  (the C++'s sentinel position `Fraction(-1,1)`);
- **`TBox` texts** (`tboxTextAsWords`): read by both readers,
  exported as `<words valign="top">` at the start of the measure, above or
  below according to the frame (3.x style defaults: 12 points);
- **instrument changes** (`writeInstrumentChange`):
  `<print><part-name-display>`/`<part-abbreviation-display>` (♭/♯ as
  `<accidental-text>`), `<transpose>` in `<attributes>` and
  `<sound><instrument-change id="Pn-Im"><instrument-sound>`. The MusicXML
  identifier absent from 3.x `.mscx` files is recovered by name or MIDI program
  (`recognize_musicxml_id`);
- **tempo**: `<type>aTempo</type>`/`<type>tempoPrimo</type>` take the
  last normal tempo (`tempoSound`);
- **swing**: a `SystemText`'s `<swing unit=… ratio=…>` exported as
  `<sound><swing>` (`straight`, `first`/`second`/`swing-type`), including
  the offset for an invisible text;
- **3.x credits**: `VBox`es are read (3.x styles: title 24, subtitle 14,
  composer/lyricist 12, `frame` 12, `instrument_excerpt` 18, copyright 8),
  with page numbering and copyright repetition; font sizes follow
  `QString::arg(double)` (6 significant digits) instead of rounding to
  two decimals.

Remaining discrepancy, documented and filtered in `texts_corpus.rs`: the `<words>` and
`<dashes>` of **hairpins** (`HairPin` with `beginText`/`endText`), whose
formatting belongs to `writeHairpinText` in the dynamics family
(`testHairpinDynamics`, `voices`). The "A" label re-exported from
`layout_elements` comes from a measure repeat (`RepeatMeasure`), outside
the text family.

## 18. Notations: residual discrepancies of a now-covered family

The notation family (articulations, fermatas, expression ties,
ornaments, trills, tremolos, techniques, arpeggios, glissandos, breaths and
chord lines) is covered by a dedicated test,
`crates/scoretk/tests/notations_corpus.rs`, which compares the `<notations>` of
these elements to the reference, measure by measure and in order. The
end-to-end tests above still remove `<notations>` except `tied`/`tuplet`:
the discrepancies below therefore do not appear in their tables.

Fixed in this pass:

- **3.x and 4.x readers**: articulations (including splitting of
  compound articulations into `staccato`/`tenuto`/`accent`…), fermatas,
  ornaments and their accidentals, single and two-note tremolos, arpeggios,
  chord lines, fingerings, breaths/caesuras, and the `Slur`,
  `Trill`, `Glissando` spanners with their properties (direction, text, line style,
  visibility);
- **export**: `<articulations>`, `<fermata>`, `<slur>` with number and
  `orientation`/`placement`, `<ornaments>` (`trill-mark`, `turn`, `mordent`,
  `wavy-line`, `tremolo`, `accidental-mark`), `<technical>` (`up-bow`,
  `down-bow`, `harmonic`, `open`, `stopped`, `snap-pizzicato`, `fingering`,
  `pluck`), `<arpeggiate>`/`<non-arpeggiate>`, `<glissando>`/`<slide>`,
  `<breath-mark>`/`<caesura>` and `falloff`/`doit`/`plop`/`scoop`;
- **breaths**: both readers follow the `SymNames` names
  (`symnames.cpp:798-822`) — `breathMarkTick`, `breathMarkUpbow`,
  `breathMarkSalzedo`, `caesura*`, `chantCaesura` — and not the truncated names
  `breathTick`/`stringsUpBow`; numeric `<subtype>` is still read as an
  obsolete form (2 = curved caesura, 3 = caesura). `<placement>` prevails over the
  placement deduced from the voice (0 above, 1 below), which the C++ sets
  before reading the tag. `.mscx` writing and MusicXML export follow the
  same names, including `breathMarkSalzedo`;
- `print-object="no"` on a `<notations>` whose element is invisible,
  reopening of a `<notations>` when visibility changes.

Fixed during the full-parity pass (`ECARTS_CONNUS` is now
empty, the test requires exact parity):

- **v3/layout_elements**: the measure repeat is unfolded on reading
  (`read/measure_repeat.rs`: marks `MeasureRepeat`, copies chords,
  chord segment annotations and the source measure's spanners);
  `<measure-repeat>` is written by `measureRepeat` on export. The
  `TremoloBar` (guitar bend) are read by both readers and written as
  `<technical><bend>` by `writeGuitarBend`. **Cause: reader/model/export.**
- **v4/copypaste_parts**: MuseScore 4.7 fills its `TrillHash` with
  `Trill::endElement()`, computed once by `Score::addSpanner` when
  `tick2 == tick` and never recomputed: the `wavy-line type="stop"` therefore comes
  out on the starting chord, with `type="start"`. Export reproduces this
  attachment (`write_trills`). **Cause: export.**
- **v4/testExcludeInvisibleElements**: `Note` carries the visibility of the tie's
  start and end (`<Tie><visible>`, read by both readers); the invisible
  `<tied>` therefore opens a `<notations print-object="no">` and the
  notations grouping matches MuseScore's again. **Cause:
  reader/model/export.**

## 19. Full parity: stems, beams, barlines, metadata

The end-to-end test has a second level, `Level::Full` (`corpus_v3_complet`,
`corpus_v4_complet`, `ecarts_complets_document`), which removes only pure
layout (attributes `default-x`/`default-y`/`relative-x`/`relative-y`/`width`,
control points `bezier-x`/`bezier-y`, `<defaults>` and `<print>` blocks,
`<system-layout>`, `<staff-layout>`, `<encoding-date>`) and produces
`docs/agents/ecarts-complets.md`.

Fixed in this pass:

- **stems**: always written (`stem.ts`), direction computed as
  `Chord::up()` (`export/stems.rs`): explicit direction outside beams,
  beam, tremolo, multiple voices (track parity), grace note, cross-staff chord,
  then pitches (`computeAutoStemDirection`, `Tpc` → `relStep` with the clef in
  force and the written pitch of transposing instruments). Result:
  no stem discrepancy outside `v3/moonlight` (9 cross-staff notes whose beam
  imposes a direction the model does not restore).
- **beams**: `export/beams.rs` rebuilds `BeamLayout::createBeams` and
  `Groups::baseBeamMode`/`actualBeamMode` (the `noteGroups` table, time subdivisions,
  anacrusis, n-tuplets, cross-measure beams), then writes
  `begin`/`continue`/`end`/`forward hook`/`backward hook` per level like
  `writeBeam`, including grace notes (`beamGraceNotes`). `BeamMode` gains
  `begin16`/`begin32` and `ChordRest::beam_direction` carries the
  `<Beam><StemDirection>` read. No `<beam>` discrepancy in either corpus.
- **barlines**: a fermata placed at the end of a measure is written in
  `<barline location="right">` like `barlineRight`, not in the rest's notations.
- **cross-staff chords**: `Chord::staff_move` (reading both formats) shifts
  `<staff>` and stem direction; `v3/moonlight` goes from 213 to 5 lines
  of `<staff>` discrepancy.
- **metadata**: `<miscellaneous>` takes `creationDate`, `mscVersion`,
  `platform` and `subtitle` (platform `Linux` and default encoding date,
  like `MasterScore`).
- **hairpins**: `spread` comes from `hairpinHeight` (12 in 3.x, 11.5 in 4.x).
- **invisible rests**: `<note print-object="no">` for a hidden rest.
- **n-tuplets**: `Tuplet::placement` reads `<direction>` and writes
  `placement="above|below"` on `<tuplet type="start">`; the files
  `testTuplets8` and `voiceswitching` become conformant.
- **direction order**: spanner starts follow the order of
  spanners (`spannerStart`), not an order by family: hairpin before
  line of text, pedal before ottava (`part-textlines` conformant).
- **pedals**: new `PedalInfo` (line, texts, hooks, placement) read
  by both readers (new `read/read4xx/pedals.rs`) and exported like
  `ExportMusicXml::pedal` (`start`/`stop`, `line`, `sign`, `<staff>`).
- **hairpin texts**: the font (`beginFontFace`/`Size`/`Style`, default
  FreeSerif 12 italic) is carried by `Hairpin` and written on the `<words>`.
- **rehearsal marks**: `justify="center"` (style `rehearsalMarkAlign`).
- **invisible key and metre**: `KeySig::visible` / `TimeSig::visible`
  write `print-object="no"`.
- **parenthesised noteheads**: `Note::parentheses` (symbols
  `noteheadParenthesisLeft/Right`) writes `parentheses="yes"`.
- **invisible hairpins**: `Spanner::visible` is read in `<HairPin>` and an
  invisible hairpin is not exported (`canWrite`).

Fixed in this pass:

- **figured bass**: `model/figured_bass.rs` (items, prefix/suffix,
  continuation line, duration), read by both readers
  (`read4xx/figures.rs`, `read302/annotations.rs`) and exported like
  `ExportMusicXml::figuredBass` (`export/figured_bass.rs`: `<figure>`,
  `<figure-number>`, `<extend>`, `<duration>`, continuation over following chords).
- **courtesy barlines**: `Measure::line_break` takes the
  `<LayoutBreak>` `line`/`page` from both readers and `end_bar_line_type`
  writes `light-light` before a key or metre change at the start of a
  system, like the `BarLine` `DOUBLE` that MuseScore's layout adds
  then. Without an explicit break in the file (system cut by page
  width), the rule cannot apply: this is the only remaining case, in
  `v3/voices`.
- **`<measure-repeat>`**: see discrepancy 18.
- **`TBox` "below"**: a frame text below a measure covered by a
  multi-measure rest group is attached to the measure that replaces the
  group (`mmrest_group_start`), like `coveringMMRestOrThis`.
- **regrouping of `<notations>` around an invisible `tied`**: see
  discrepancy 18.

Fixed in the `t31` parity pass:

- **"gap" rests**: `read/gap_rests.rs` ports `Measure::checkMeasure` and
  `Measure::fillGap` (`check.cpp`), as well as the rhythmic splitting of
  `toRhythmicDurationList` (`durationtype.cpp`, `sig.cpp`). Gaps in a
  voice become invisible rests whose position advances the
  export cursor: the origin of MuseScore's split `<forward>`/`<backup>`
  (`moonlight`).
- **tie orientation**: the `.mscx` carries the explicit direction
  in `<Tie><up>up|down</up>` (`Pid::SLUR_DIRECTION`); both
  readers keep it (`Note::tie_up`) and export writes
  `orientation`/`placement` like `slurTieLineStyle` (`moonlight`).
- **hidden staves**: `Measure::line_break`/`section_break_before` give
  explicit systems; `export/systems.rs` applies
  `SystemLayout::hideEmptyStaves` and export writes
  `<staff-details print-object="no"/>` at the first measure of the system
  (`barline01`).
- **total duration of n-tuplets**: `TRead::read(Tuplet*)` computes
  `baseLen * normalNotes` with the **unreduced** ratio; both readers
  used the reduced denominator (a `4:4` lasted one note value instead of
  four).

Remaining full discrepancies (detail and frequency in `ecarts-complets.md`):
cross-beam stem direction (9 notes of `moonlight`, computed
by `BeamLayout::layout1` from beam grouping and geometry) and courtesy barlines
of systems automatically cut by page
width (2 barlines of `v3/voices`, without `<LayoutBreak>` in the
file).

## 20. MusicXML export — schema validity and measured fidelity (English)

### Frozen baseline (before the fixes of this campaign)

- Frozen binary: `/tmp/scoretk-base`, built from commit `5ef2686`, sha256 starting
  `4084ce2dc32f6377`. The fixed binary is built from the same worktree after the
  the two fixes below; the frozen one stays available for A/B measurement.
- Lot: the 174 `.mscz` of `~/Musique`, manifest built by
  `scripts/oracle/musicxml/lots.sh` (same list as
  `scripts/oracle/lilypond/lots-ecriture.sh`). No corpus file or third-party
  script is versioned.
- Oracle: `scripts/oracle/musicxml/mesure.sh` (wraps `mesure.py` with
  `python3Packages.mido` and `libxml2`). For each score: scoretk → MusicXML,
  MuseScore 4.7.4 → MusicXML (fidelity reference), both → MIDI (what each tool
  plays). Every binary runs under `timeout` and `prlimit --as=2000000000`.
  Core comparison reuses `core_norm` through
  `crates/scoretk/tests/oracle_core.rs`; N1/N2 reuse `midi_notes.py` at the
  Q16 grid (±1/16 quarter). music21 is not installed in this environment, so
  the only alternative measured is MuseScore 4.7.4.
- XSD: MusicXML 4.0 (`specs/musicxml/musicxml-4.0.xsd`), its `xml.xsd` and
  `xlink.xsd` imports resolved locally through a catalog. Errors are counted by
  message. XSD text: `xsd:<type>`; narrative text: `ref:<element>` in
  `docs/agents/format-ref/musicxml.md`.

| Metric | Baseline (frozen) |
| --- | ---: |
| Scores | 174 |
| Converted | 152 |
| XSD-invalid scoretk outputs | 2 |
| XSD errors, scoretk / MuseScore 4.7.4 | 3 / 2 |
| Core-identical to MuseScore | 24 / 152 |
| N1 pitch multiset | 0.9791 |
| N2 pitch + attack | 0.9739 |

### XSD errors by cause

1. `<measure-style number="0">` — 2 occurrences, scoretk only
   (`Waltz_No._2.mscz`). `xsd:measure-style` declares `number` as
   `xsd:staff-number`, a restriction of `xs:positiveInteger`
   (`musicxml-4.0.xsd:780-785`); the spec says the number selects the staff from
   the top, 1 being the first (`ref:measure-style`, format-ref § "Clef, time,
   staves, parts"). Cause: export. `write_measure_repeat` used `Some(0)` as a
   sentinel for the closing `<measure-repeat type="stop">`. Port of
   `measureRepeat()` (`exportmusicxml.cpp:6534-6571`), which builds the
   `number` attribute once per staff in the loop and reuses it for both the
   start and the stop. Fixed by passing the 0-based staff position and adding
   one inside `write_measure_repeat`. Reduced test:
   `repetition_de_mesure_stop_numero_de_porte_valide`.

2. `<metronome>` with only `<beat-unit>` — 1 occurrence, scoretk **and** 1 in
   MuseScore 4.7.4 (`Magnificat__Prt.mscz`). `xsd:metronome`
   (`musicxml-4.0.xsd:3795-3825`) is a choice whose first branch is a sequence
   `<beat-unit>` then either `<per-minute>` or another `<beat-unit>`; the spec
   describes the same regular/metric-modulation forms (`ref:metronome`,
   format-ref § "Repeats, jumps, directions, lyrics"). Cause: upstream export,
   faithfully reproduced by scoretk. `findMetronome()`
   (`exportmusicxml.cpp:4809-4884`) does not test `posEq == npos`: when the text
   is `♩ ≈ 90` (no `=`), it returns true with `metroRight` empty and
   `wordsMetronome()` writes the invalid element followed by `<words> ≈ 90</words>`.
   Fixed by returning `None` when neither `per_minute` nor `second_beat` was
   found, so the full text becomes `<words>` (valid and keeps the `≈`).
   Deliberate divergence from MuseScore 4.7.4, which is invalid here: this is a
   validity fix, not a fidelity regression. Only formatting changes on that
   file (the symbol moves into a `<words>` and the bold style is no longer
   attached to the number); pitches, durations and the `<sound tempo="90"/>`
   are unchanged. Reduced test: `tempo_sans_equation_reste_en_words`.

3. `<accidental-mark cautionary="no">` — 1 occurrence, MuseScore 4.7.4 only.
   `xsd:accidental-mark` has no `cautionary` attribute; scoretk already emits a
   valid element. No change needed.

### Out of scope: MuseScore 2.x sources

22 of the 174 files cannot be read by scoretk and are therefore not converted:
20 at version `2.06`, one at `1.14` (`Canon_in_D_Pachelbel_-_arranged_for_accordion.mscz`)
and one reported as `2` (`Autumn_Leaves_Frank_Marocco.mscz`). They are all under
`Partitions/Accordeon`, `Partitions/Chorales` and
`Partitions/Piano/Vingt_danses_enfantines_pour_piano.mscz`; the exact absolute
paths are in the `erreur` field of `/tmp/oracle-xml-full/mesure.jsonl` (raw
measurement, not versioned). The reader supports 3.x and 4.x/5.x only: adding
2.x support is a separate task and is out of scope here.

### Final measurement against the frozen baseline

Fixed binary `/tmp/scoretk-fix`, sha256 starting `b95222eff66c7169`; same 174-file
manifest and oracle as above.

| Metric | Frozen baseline | After fixes | Δ |
| --- | ---: | ---: | ---: |
| Converted | 152 | 152 | 0 |
| XSD-invalid scoretk outputs | 2 | **0** | −2 |
| XSD errors, scoretk | 3 | **0** | −3 |
| XSD errors, MuseScore 4.7.4 (unchanged reference) | 2 | 2 | 0 |
| Core-identical to MuseScore | 24 | **25** | +1 |
| N1 pitch multiset | 0.9791 | 0.9791 | 0 |
| N2 pitch + attack | 0.9739 | 0.9739 | 0 |

No per-file loss: across all 174 rows, `ok`, `ref`, `n1`, `n2` and `n2d` are
identical to the frozen baseline. The only core change is
`Waltz_No._2.mscz` (4 → 0 differing lines); `Magnificat__Prt.mscz` keeps its 39
core lines because `<direction>` is removed by `core_norm`, but its XSD error is
gone. The `directions` family gains 5 changed lines (the `♩ ≈ 90` text is now
two `<words>` instead of one `<metronome>` plus one `<words>`), which is the
documented validity fix and not a pitch, duration or tempo change (`<sound
tempo="90"/>` is preserved). `EXPECTED_CONFORMANT` in `e2e_corpus.rs` is
unchanged.

### Regression guard (oracle diff)

`compare.sh` reports DIFFs on this worktree, but none is caused by the two
fixes. After normalising the encoding date (the reference outputs were frozen
on 2026-11-18, this run was made on 2026-11-20) **every** `.musicxml`, `.mxl`
and `.mscz` output is byte-identical to the reference. The remaining differences
are 28 `.abc`, 14 `.midi` and 14 `.ly` outputs, i.e. formats untouched by this
campaign: they come from the pre-existing branch commit `4d78e80` (ABC export)
against the older `master` reference. In particular `EXPECTED_CONFORMANT` in
`e2e_corpus.rs` is unchanged and `cargo test -p scoretk --test e2e_corpus` still
passes (5/5).

### Fidelity campaign (English)

#### Frozen baseline and grouping

The binary was frozen from `master` before any fix (`/tmp/scoretk-base3`,
sha256 `7944c0d7e7598157`); the 174 `.mscz` of `~/Musique` were measured with
`scripts/oracle/musicxml/mesure.sh`. With the new 1.15–2.07 reader, 173 files
convert (the 22 files of the previous campaign are no longer failures).

| Metric | Frozen baseline | Final | Δ |
| --- | ---: | ---: | ---: |
| Converted | 173 | 173 | 0 |
| Core-identical to MuseScore 4.7.4 | 27 | **39** | +12 |
| N1 pitch multiset | 0.9779 | 0.9778 | −0.0001 |
| N2 pitch + attack | 0.9086 | 0.9086 | 0 |
| XSD errors, scoretk / MuseScore | 0 / 2 | 0 / 2 | 0 |

`groupes.py` groups the 146 non-conformant baseline files by **first differing
core family** (the first changed canonical line, in document order, extracted
by `oracle_core.rs`). Counts and reduced examples:

| First differing family | Files | Reduced example (ref → scoretk) |
| --- | ---: | --- |
| instruments / transposition | 77 | `<midi-channel>3</midi-channel>` → `2` |
| barlines / repeats (measure number) | 18 | `<measure implicit="yes" number="0">` → `number="1"` |
| structure / part-list | 16 | `<group-symbol>square</group-symbol>` → `line` |
| notes / pitch | 7 | `<accidental>sharp</accidental>` → absent |
| tuplets | 6 | `<tuplet bracket="no">` → `bracket="yes"` |
| voices / staves | 5 | `<staff-details number="1">` → absent |
| lyrics | 5 | absent → `<extend/>` |
| metadata (layout artefact) | 4 | `<movement-number>8</movement-number>` → `<measure number="1">` |
| ties | 3 | `<tie type="start"/>` duplicated vs missing half |
| harmony / chords | 2 | `<harmony print-frame="no">` → same tag, different children |
| directions | 2 | absent → `<sound tempo="160"/>` |
| duration / rhythm | 1 | `<backup>` → `<forward>` |

#### Fixes, one commit per cause

1. **MIDI port, 52 files** — `Instrument::midi_port` read from
   `<Channel><midiPort>` by `read302` (therefore `read206`) and `read4xx`, then
   written as `<midi-device port>` by `score-instruments`. Port of
   `ExportMusicXml::partList` (`exportmusicxml.cpp:7939-7947`,
   `midiMapping(channel)->port() + 1`). Test `port_midi_de_l_instrument`.
2. **Pickup measure numbering, 18 files** — `Measure::exclude_from_numbering`
   and a port of `MeasureNumberStateHandler::updateForMeasure`
   (`exportmusicxml.cpp:8272-8299`). Test `anacrouse_implicite_et_numeration`.
   A follow-up commit restricts the flag to `<irregular>1</irregular>`: a bare
   `@len` makes a measure irregular but **does not** remove its number
   (`Buenos_dias` vs `L'Aigle_mexicain`). Test `irregular_et_len_sont_distincts`.
3. **Part-group symbol, 16 files** — `bracket_symbol` mapped `BracketType`
   (`src/engraving/types/types.h:1289`: `NORMAL`, `BRACE`, `SQUARE`, `LINE`,
   `GROUP`); types 2 and 3 were swapped. `partGroupStart`
   (`exportmusicxml.cpp:4742-4770`) confirms the names. Test
   `symbole_de_groupe_carre_et_ligne`.
4. **Legacy accidental names, 7 files** — 2.x `<subtype>sharp|natural|flat`
   was not recognised by `TConv::fromXml` (`read302/tables.rs`); the written
   `<accidental>` was missing. Test `alteration_explicite_2x`.
5. **Null melisma, 5 files** — `write_lyric` emitted `<extend/>` for
   `Some(Fraction::ZERO)`, read from `<ticks_f>0/16</ticks_f>`. Port of
   `ExportMusicXml::lyrics` (`exportmusicxml.cpp:6130`, `ticks().isNotZero()`).
   Test `melisma_nul_n_ecrit_pas_extend`.
6. **2.x ties, 3 files** — the note ordinal used to pair `<Tie id>` with
   `<endSpanner id>` was reset at each chord, so all first notes of chords got
   rank 0 and `fix_ties` discarded intra-measure ties (`from == to`). The
   counter is now per voice over the whole measure. Test
   `liaison_2x_entre_deux_accords_separeres`.

#### Causes deliberately left unchanged (layout artefacts, not music)

- **Tuplet brackets, 6 files** — `bracket` is computed by
  `Tuplet::calcHasBracket` (`src/engraving/dom/tuplet.cpp:204-274`) from beams,
  head side and staff crossing, none of which is in the pivot model. The
  heuristic reproduces the versioned corpus but cannot match both directions
  observed on real files (4 files want `no`, 2 want `yes`). Changing it would
  break `voiceswitching`; no export change.
- **`<staff-details>`, 5 files** — the reference writes `<staff-size scaling=
  "70">70</staff-size>` (small staff) or `print-object="no"` (hidden empty
  staff), both layout decisions absent from the model (`writeStaffDetails`,
  `exportmusicxml.rs:2256`). No export change.
- **`<movement-number>` / bezier points, 4 files** — `core_norm` keeps
  `<movement-number>` and the `bezier-x`/`bezier-y` attributes that
  `Level::Full` drops. These files differ only by metadata and layout geometry
  (slurs), not by notes, durations or structure.
- **`<sound tempo>` / `<sound segno>`, 2 files** — invisible tempo marks and a
  jump's `sound` attribute depend on export context (`tempoSound`,
  `exportmusicxml.cpp:5006-5016`); the first file has 3 extra `<sound tempo>`
  out of 59 invisible tempos, with identical pitches.
- **`<backup>` vs `<forward>`, 1 file** — an equivalent timeline encoding of a
  gap rest (`fillGap`); the resulting cursor and notes are unchanged.

#### Per-file changes and corpus

No file went from core-identical to non-identical. The 11 changed rows are all
2.x files: 6 N1 and 4 N2 variations come from the tie fix, which now merges
tied notes in scoretk's MIDI as MuseScore does (fewer note-ons, different
multiset, same sounding music); one core increase (`ZORBA_THE_GREEK`, 35 → 56)
comes from preserving explicit 2.x accidentals that MuseScore 4.7.4 suppresses
as redundant against the key, while pitches and durations are unchanged.
The versioned corpus stays at **54/56**; `barline01` was added to
`EXPECTED_CONFORMANT_V3` now that it is conformant. `moonlight` (18 lines) and
`voices` (6 lines) keep their documented layout differences.

### Final re-measurement (after rebase and concert pitch)

The binary was rebuilt from the rebased tree (2.x spanners, per-voice tie
ordinals, style-driven concert pitch) and measured against the same frozen
`/tmp/oracle-base3` baseline. The 2.x spanner port and the concert-pitch fix
are the only behavioural changes since the previous table.

| Metric | Frozen baseline | Final | Δ |
| --- | ---: | ---: | ---: |
| Converted | 173 | 173 | 0 |
| Core-identical to MuseScore 4.7.4 | 27 | **39** | +12 |
| N1 pitch multiset | 0.9779 | 0.9776 | −0.0003 |
| N2 pitch + attack | 0.9086 | **0.9257** | +0.0171 |
| XSD errors, scoretk / MuseScore | 0 / 2 | 0 / 2 | 0 |

#### N2 split by score version

| Group | Files | Reference notes | N1 | N2 |
| --- | ---: | ---: | ---: | ---: |
| 3.x / 4.x | 152 | 317 317 | 0.9791 | **0.9739** |
| 1.x / 2.x | 22 (21 converted) | 60 809 | 0.9700 | **0.6742** |

The 3.x/4.x figure is unchanged from the frozen baseline. The 2.x figure rose
from 0.5680 to 0.6742; it remains low for one reason, not a general reader
failure: `Vingt_danses_enfantines_pour_piano.mscz` alone carries 26 477 of the
60 809 reference notes and has N2 = 0.3106. The other converted 2.x scores are
mostly at 0.97–1.00 (`La_Partida` 0.779, `Czardas` 0.801, `ZORBA` 0.790 and
`Triumphal_March` 0.676 are the four remaining outliers).

Reduced cause for the piano file (raw MIDI in `/tmp/oracle-final4/travail/173`):
- repeat/jump expansion diverges. Matching notes show attack offsets of +6,
  +4.5, +13 and +3 quarters (4 241, 2 427, 1 756 and 1 716 notes); the score
  has 30 `<Volta>`, `<endRepeat>` and `<Jump>` markers. Both tools expand
  repeats, but MuseScore's `RepeatList` and scoretk's walker do not make the
  same passes, so most attacks in the second half fall outside the ±1/16
  tolerance and N2 collapses while N1 stays at 0.9896.
- ottava playback: the first pitch divergence is at tick 856, reference 88
  (E6) vs scoretk 76 (E5), i.e. one octave; 271 notes are affected. scoretk's
  MIDI writer does not apply `<Ottava>` to playback, unlike MuseScore.
- Disabling scoretk's repeat expansion makes N2 worse (0.0706), confirming
  that the reference does expand repeats.

Both effects are layout/playback-policy differences of the MIDI oracle, not
lost notes: N1 is 0.9896 and the MusicXML core diff on that file is 403 lines
of layout and ottava placement. No export change is made here: faithfully
reproducing MuseScore's 2.x jump passes would require porting `RepeatList`
beyond the versioned corpus, with a high regression risk.

#### Concert pitch

`Ad_te_Suspiramus` (Piccolo, `Sid::concertPitch` in `<Style>`) went from 797 to
56 differing core lines: `<concert-score/>` is now emitted, pitches are written
at concert pitch (D5 instead of D4) and the transposition becomes
`<for-part><part-transpose>`. N1 and N2 were already 1.0000 (MIDI uses sounding
pitch on both sides) and remain so. The wasm `concertPitch` option still maps
to `Some(value)` and overrides the style; the JS contract is unchanged.

#### Per-file changes

The 16 rows that changed are all 2.x files. N2 **increased** for 7 of them
(Czardas 1 272 → 1 734, La_foule 450 → 813, La_Partida 431 → 557, ZORBA 795 →
1 335, …) thanks to the 2.x spanner port; the 8 small N1 decreases and the
piano N2 drop are the tie-merge and repeat-expansion effects described above.
`ZORBA_THE_GREEK` gains 21 core lines because explicit 2.x accidentals are now
preserved even where MuseScore 4.7.4 suppresses them as redundant. No file
went from core-identical to non-identical, and the versioned corpus remains
54/56 with `EXPECTED_CONFORMANT` unchanged.

### Second fidelity campaign — reader information loss and part-list fidelity (English)

#### Frozen baseline and regrouping

Baseline binary: `/tmp/scoretk-mxl-base`, sha256 starting `1b086523`, built
from commit `842da94` before any fix of this campaign. Same 174-file manifest
(`scripts/oracle/musicxml/lots.sh`) and same oracle
(`scripts/oracle/musicxml/mesure.sh`) as above. The frozen binary stays
available for A/B measurement; all scores run under `timeout` and
`prlimit --as=2000000000`.

`groupes.py` groups the 134 non-conformant baseline scores by **first differing
core family** (the canonical line extracted by `oracle_core.rs`, in document
order). Layout-only tags (`<staff-details>`, tuplet brackets) and metadata
artefacts (`movement-number`, measure numbering across Vivaldi movements) are
kept in the table but deliberately not fixed.

| First differing family | Baseline files | Final files |
| --- | ---: | ---: |
| instruments / transposition | 60 | 6 |
| voices / staves (hidden/small staves) | 21 | 72 |
| tuplets (bracket layout) | 12 | 14 |
| other (part-group order, layout artefact) | 11 | — |
| barlines / repeats (measure-style, layout) | 10 | 11 |
| structure / part-list | 6 | 14 |
| metadata (artefact) | 4 | 4 |
| notes / pitch | 3 | 3 |
| harmony / chords | 2 | 2 |
| ties | 2 | 2 |
| directions | 2 | 5 |
| duration / rhythm | 1 | 1 |

By first differing **tag**, the baseline had `part-name` 50, `staff-details`
21, `tuplet` 12, `part-group` 11, `measure` 8, `attributes` 6,
`midi-channel` 4, `movement-number` 4, `volume` 3, `instrument-sound` 3. The
five fixed causes below remove all 67 files whose first difference was
`part-name`, `part-group`, `instrument-sound` or `volume`; the remaining first
tags are layout (`staff-details`, `tuplet`) or documented artefacts.

#### Fixes, one commit per cause

1. **`<br/>` in instrument names, 50 files** — `TRead::readLegacyStaffName`
   (`read500/tread.cpp:4200`) and `lineBreakFromTag` (`:3812`) replace `<br/>`
   with a newline; the MusicXML writer's `toPlainText`
   (`musicxmlfonthandler.cpp:56`) keeps that newline. The 3.x/4.x readers
   concatenated the text and dropped the `<br/>`, so MuseScore exported
   `<part-name>S\nA</part-name>` while scoretk wrote `SA`. Fixed in
   `read302/xml.rs`, `read302/part.rs`, `read4xx/dom.rs` and `read4xx/mod.rs`.
   MusicXML 4.0 is silent on how a MuseScore legacy name is decoded; MuseScore
   4.7.4 behaviour is followed (`ref:part-name`). Reduced tests:
   `sauts_de_ligne_dans_les_noms_de_partie`,
   `nom_de_partie_3x_avec_saut_de_ligne`.
2. **Part-group closing order, 11 files** — `partList`
   (`exportmusicxml.cpp:7953-7960`) closes `partGroupEnd` from the last group
   down to zero (a stack); scoretk closed groups in ascending order. The XSD
   leaves the order free, so the reference tool decides. Reduced test:
   `fermeture_des_groupes_de_parties_en_ordre_decroissant`.
3. **Instrument ID not recognised for 2.x, 3 files** —
   `TRead::read(Instrument*)` (`read400/tread.cpp:803`) always calls
   `updateInstrumentId`. `read206` delegates with version 302, and the reader
   skipped recognition for that version, so `<instrument-sound>` was missing
   from `<score-instrument>`. Reduced test:
   `identifiant_d_instrument_reconnu_sans_id_dans_le_fichier`.
4. **Volume/pan from the last channel, 2 files** — `midiInstrument`
   (`exportmusicxml.cpp:7341-7355`) reads `instr->channel(0)`. The 3.x reader
   overwrote the controller values with each later `<Channel>`. Reduced test:
   `volume_et_pan_du_premier_canal_seulement`.
5. **Volume decimal formatting, 1 file** — `XmlWriter::tag` writes doubles
   with six significant digits (`3.93701` for a controller at 5); scoretk
   used four decimal places (`3.937`). Reduced test:
   `volume_en_chiffres_significatifs`.

A sixth cause, **MIDI channel allocation** (6 files, 42 lines), was
implemented then reverted: reserving one channel per `<Channel>` requires a
new pivot-model field, and the MuseScore writer emits a single `<Channel>`,
so the `.mscz` round-trip test (`mscz_est_un_conteneur_musescore`) lost the
count. Porting the full channel map through the writer is a separate change;
no partial fix was kept. `<midi-channel>` remains the first difference of 6
files.

#### Final measurement against the frozen baseline

Final binary `/tmp/oracle-mxl-campagne` rebuild, sha256 starting `0f03299f`;
same manifest and oracle.

| Metric | Frozen baseline | Final | Δ |
| --- | ---: | ---: | ---: |
| Scores | 174 | 174 | 0 |
| Converted | 173 | 173 | 0 |
| XSD-invalid scoretk outputs | 0 | 0 | 0 |
| XSD errors, scoretk / MuseScore | 0 / 2 | 0 / 2 | 0 |
| Core-identical to MuseScore | 39 | 39 | 0 |
| N1 pitch multiset | 0.9783 | 0.9783 | 0 |
| N2 pitch + attack | 0.9314 | 0.9314 | 0 |
| Total differing core lines | 42 599 | 42 107 | −492 |
| Files with fewer differing core lines | — | 68 | — |

No per-file loss: across all 174 rows, `ok`, `ref`, `n1`, `n2`, `n2d` and the
reference (`ms`) outputs are unchanged; no file went from core-identical to
non-identical and no file's core line count increased. 68 files improved by
1 to 12 lines each. The 39 already-conformant files stay conformant; none of
the improved files becomes fully conformant because it still has a layout
difference (`<staff-details>`, tuplet brackets, or `<sound>` placement).
`EXPECTED_CONFORMANT` is unchanged and
`cargo test -p scoretk --test e2e_corpus` still passes (5/5).

#### Causes deliberately left unchanged (layout or metadata)

- **`<staff-details>`, now 72 files** — `print-object="no"` on an empty staff
  and `<staff-size>` come from MuseScore's layout decisions, absent from the
  pivot model (`writeStaffDetails`, `exportmusicxml.rs:2256`).
- **Tuplet brackets, 14 files** — `bracket` is computed by
  `Tuplet::calcHasBracket` from beams and staff crossing; no export change.
- **`<measure-style>` (multiple rests), 6 files** — MuseScore creates them in
  the layout engine from `<createMultiMeasureRests>`; reproducing that pass
  is a layout change, not a musical gap.
- **Vivaldi measure numbering / `<movement-number>`, 13 files** — continuous
  numbering across movements and the `Level::Full` metadata artefact, both
  documented above.
- **`<sound tempo>`/`<sound segno>`, 5 files** — invisible tempo marks and
  jump playback context depend on the export pass (`tempoSound`,
  `exportmusicxml.cpp:5006-5016`).
- **`<midi-channel>`, 6 files** — see the reverted cause above.
