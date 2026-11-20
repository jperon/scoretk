# MusicXML import: port state and measured discrepancies

`crates/scoretk-musicxml-import` reads a `partwise` MusicXML document
(`.musicxml`, `.xml`, or a `.mxl` archive) and returns the pivot model of
`scoretk-core` (`scoretk_core::model::Score`). The port follows MuseScore
Studio's import, `src/importexport/musicxml/internal/import/`
(`importmusicxmlpass1.cpp`, `importmusicxmlpass2.cpp`,
`importmusicxmlnoteduration.cpp`, `importmusicxmlnotepitch.cpp`,
`musicxmlpart.cpp`, `musicxmltupletstate.cpp`, `musicxmlvoicedesc.cpp`).

## 1. Entry point and organisation

```rust
use scoretk_musicxml_import::{read_musicxml, read_mxl};
let score = read_musicxml(bytes)?;            // bytes = .musicxml, .xml or .mxl
let score = read_mxl(bytes)?;                 // bytes = .mxl archive only
let xml = scoretk_musicxml_export::export_musicxml(&score)?;
```

| File | Role |
| --- | --- |
| `src/lib.rs` | `read_musicxml`, `read_mxl`, passes 1 and 2, `gap_rests::fill` |
| `src/container.rs` | choice between bare XML document and archive (`.mxl`) |
| `src/mxl.rs` | `.mxl` (ZIP) → root file via `META-INF/container.xml` |
| `src/xml.rs` | minimal XML tree (local name, unescaped attributes, text) |
| `src/pass1.rs` | structure: `part-list`, measure lengths, voices, overlaps |
| `src/pass2.rs` | model: segments, notes, n-tuplets, ties, barlines, grace notes |
| `src/direction.rs` | `<direction>`: dynamics, hairpins, texts, tempos, repeats |
| `src/barline.rs` | `<barline>`: styles, `<repeat>`, volatas, barline fermatas |
| `src/harmony.rs` | `<harmony>` and `<figured-bass>` |
| `src/notations.rs` | articulations, ornaments, fermatas, arpeggios, slurs, glissandos, trills, fingerings |
| `src/credits.rs` | `<credit>`: title, subtitle, composer, rights |
| `src/noteduration.rs` | `<type>`/`<dot>`/`<duration>`/`<time-modification>` |
| `src/error.rs` | `Error` (XML, ZIP, unknown root, missing `part-list`) |

The library stays portable: no disk access, no clock, no
threads; it compiles for `wasm32-unknown-unknown`. The `.mxl` ZIP is decoded
in pure Rust (`zip` without default features).

### `.mxl` container (t43)

`src/mxl.rs` ports `extractRootfile` from `importmusicxml.cpp`:

1. read `META-INF/container.xml` and take the first `<rootfile full-path>`
   (the C++'s descent `container` → `rootfiles` → `rootfile`, with tolerance
   of a namespace prefix);
2. read the entry thus designated; absent → `Error::RootFileNotFound`.

Two deliberate deviations from the C++:

- **fallback** when `container.xml` is missing: first `.musicxml`/`.xml` outside
  `META-INF`, in archive order (otherwise `Error::NoMusicXmlInMxl`);
  MuseScore, for its part, fails with an empty root file;
- **explicit errors** instead of a silently empty import for a missing
  entry.

`read_mxl` rejects a bare MusicXML document (`Error::Zip`): it is
`read_musicxml` that accepts both. On the facade side, `Format::Mxl` is read with the
`musicxml-import` feature (`hooks::read_mxl`), and content-based detection
recognises a ZIP containing `META-INF/container.xml` **after** having ruled out a
`.mscz` (a `.mscz` also contains a `container.xml`).

## 2. Fidelity measurement: round-trip test

`crates/scoretk-musicxml-import/tests/aller_retour.rs` reads each
`tests/corpus/{v3,v4}/*.ref.musicxml`, re-exports it with the export of
`scoretk-core`, and compares the two documents with the normalisation of
`crates/scoretk/tests/e2e_corpus.rs`, at **two levels**:

- **musical core** (`Level::Core`): blocking level, identical to
  `mscx → MusicXML` compliance;
- **full parity** (`Level::Full`): directions, articulations, spanners and
  credits are compared too; it measures reader coverage and fails
  only on `EXPECTED_FULL_V3` / `EXPECTED_FULL_V4`.

```sh
nix develop -c cargo test -p scoretk-musicxml-import --test aller_retour -- --nocapture
```

Result as of 2026-11-17:

| Level | v3 | v4 |
| --- | ---: | ---: |
| musical core | **26/30** | **26/26** |
| full parity | **23/30** | **21/26** |

Any regression on a file listed in `EXPECTED_V3`, `EXPECTED_V4`,
`EXPECTED_FULL_V3` or `EXPECTED_FULL_V4` fails the test.

### `.mxl` round trip

`crates/scoretk-musicxml-import/tests/mxl.rs` checks over the whole corpus that
`.mscx` → `export_mxl` → `read_mxl` → MusicXML export is **byte for byte
identical** to the equivalent `.musicxml` exported then read back (container detection,
therefore independently of the reader). It also covers the fallback without
`container.xml`, the invalid ZIP archive, the archive without MusicXML and the
missing `rootfile`. Result: **56/56** identical files.

```sh
nix develop -c cargo test -p scoretk-musicxml-import --test mxl -- --nocapture
```

### MusicXML → MIDI chain

`crates/scoretk-midi-export/tests/musicxml_midi.rs` reads the references
`tests/corpus/{v3,v4}/X.ref.musicxml`, writes the MIDI with `write_midi` and
compares notes (track, channel, pitch, start, end, velocity) and tempos to
`tests/corpus/midi/{v3,v4}/X.ref.mid`, like `corpus_midi.rs`. Result:
**7/21 identical** (`v4/dynamics`, `hairpin_simple`, `linked-ties`, `mmrest`,
`repeat68`, `repeat69`, `tenuto_accent`). This is the verification that the
dynamics and tempos read from `<direction>` produce the correct velocities.

## 3. What is read

- parts (`part-list`: name, abbreviation, `-display` variants,
  `score-instrument`, `instrument-sound`, MIDI channel/program/volume/pan),
  `<part-group>` (braces, brackets, common barline);
- staves (`<staves>`, `<staff-details>/<staff-lines>`), transposition
  (`<transpose>`);
- measures: actual length (maximum over parts, 1/128 correction),
  nominal length (time signature, local stretching of directions
  per staff), section break inferred from a measure number that goes backwards;
- key signatures (`fifths`, `mode`, `print-object`), time signatures
  (`symbol="common"`/`"cut"`), clefs (including octave clefs and
  changes at the end of the previous measure);
- chords, rests (whole measure, invisible), notes (pitch, `tpc`,
  displayed accidental and its role, notehead and notehead group, including SMuFL);
- durations `<type>`/`<dot>`/`<duration>`, two-note tremolos (actual duration
  half, `<time-modification>` 2:1);
- voices: `backup`/`forward`, mapping of MusicXML voices to MuseScore voices
  (`allocateStaves`/`allocateVoices`), cross-staff overlap detection
  (`VoiceOverlapDetector`) and `staffMove`;
- n-tuplets: `<tuplet>` (bracket, number, placement), ratio of
  `<time-modification>` (combined for nesting), base duration read from
  `<tuplet-type>`, length `baseLen × normalNotes` as in
  `TRead::read(Tuplet*)`;
- beams: `<beam>` (`begin`/`continue`/`end`/hooks), stem direction
  of the group taken from the `<stem>` of the first chord;
- tied notes (`<tie>`, `<tied>`, orientation);
- lyrics (`<lyric>`: verse, syllabification, placement, extension);
- grace notes: grace chords before **and after** the chord
  (`Chord::toGraceAfter`), appoggiatura/acciaccatura;
- directions (`<direction>`): dynamics `<dynamics>` (velocity from
  `<sound dynamics>`), hairpins `<wedge>` (start/stop, niente, start and end texts, including inserted dynamics), `<words>` (staff or
  system text, italic/bold/underline/font formats), `<rehearsal>`, metronome
  `<metronome>` and `<sound tempo>` (visible and invisible tempos), `<offset>`,
  `<staff>`, pedals, ottavas, `<dashes>`/`<bracket>` lines, segno/coda/fine
  and D.C./D.S. jumps;
- notations (`<notations>`): articulations, ornaments (`<turn>`, mordents,
  `trill-mark`, `wavy-line`), fermatas, arpeggios (`arpeggiate`,
  `non-arpeggiate`), `<slur>` (numbers, orientation), `<slide>`/`<glissando>`,
  fingerings `<technical><fingering>`, chord lines (`falloff`, `doit`,
  `plop`, `scoop`) and breaths/caesuras (`<caesura>` `normal`, `curved`,
  `short`, `thick`, `single`; `<breath-mark>` `comma`, `tick`, `upbow`,
  `salzedo`, with their `placement`);
- repeats (`<repeat>` forward/backward, `times`), volatas (`<ending>`), multi-measure
  rests (`<multiple-rest>`), repeated measures (`<measure-repeat>`);
- harmonies (`<harmony>`: `root`, `kind`/`text`, `bass`, `numeral`,
  `function`, `degree`, `offset`) and figured bass (`<figured-bass>`);
- instrument changes (`<sound><instrument-change>`, `<transpose>` at
  the change tick, associated `<words>` text);
- credits (`<credit>`: several `<credit-words>`, type, page, alignments,
  font), copyright derived from `<rights>` or the `rights` credit;
- filler rests: gaps of the first voice (`fillGapsInFirstVoices`
  of pass 2) then gaps of all voices via `gap_rests::fill`
  (MuseScore's `Measure::checkMeasure`) — the origin of
  MuseScore's split `<forward>`/`<backup>`.

## 4. Remaining discrepancies at core level (4 v3 files, 0 in v4)

| File | Lines | Cause |
| --- | ---: | --- |
| `v3/barline01` | 6 | `<staff-details print-object="no"/>` of hidden staves: **layout** information absent from MusicXML (MuseScore recomputes it) |
| `v3/layout_elements` | 26 | harmonies attached to a different voice than MuseScore (order of `<direction>`/`<harmony>` after `backup`), unmodelled `<technical><bend>` bend |
| `v3/testTuplets8` | 31 | file of **incomplete n-tuplets**: MuseScore inserts filling rests (`MusicXmlTupletState::missingTupletDuration`) that `<type>` alone cannot reconstruct; extra `<normal-type>` and computed `<duration>`s |
| `v3/voices` | 42 | **voice renumbering**: MuseScore renumbers MusicXML voices (1,2,3,5 → 1,2,3,4), the original `.mscx` kept 1,2,3,4; round tripping through MusicXML loses this numbering |

Two discrepancies are **irreducible** without information absent from the format: hidden
staves (layout) and voice renumbering (MuseScore does it on import too). The other two are unported features.

## 5. Remaining discrepancies at full level (7 v3 files, 5 in v4)

v3: `barline01`, `layout_elements`, `moonlight` (6 `<stem>`s of cross
beams, 3 multi-line `<credit-words>`), `testTuplets8`, `testWedge4`
(simultaneous hairpins numbered 1/2), `voices` (voices), and the files
above.

v4: `expression-1` (`font-style="normal"` reset between two
`<words>`), `repeat65`, `repeat66`, `repeat68` (empty `<words></words>` of a
textless frame, explicit `font-family`/`font-size` equal to the style),
`voiceswitching` (multi-block lyrics, stems).

Not read (or without an equivalent in the model): `<technical><bend>` and
other unlisted techniques, `<harmony><frame>` (fretboard diagrams),
extended `<lyric>` formats, `<defaults>`/`<print>` (layout),
multiple `ending` `<barline>`s on the same measure, `<sound><swing>`
(partially).

## 6. Pitfalls encountered

- **`Fraction` always reduced**: the C++ measure-length correction
  (`measureDurationAsFraction`) rewrites `4/4` as `8/8` without changing the value;
  with a reduced fraction, the C++ loop does not terminate. The import
  therefore keeps the value and documents the representation discrepancy.
- **Nested n-tuplets**: `<time-modification>` gives the *combined* ratio;
  the inner ratio is the quotient by the parent ratios. The length is
  computed as `baseLen × normalNotes`, not as the extent of the children.
- **Overlapping voices**: the same MusicXML voice can change staff; MuseScore's
  `VoiceOverlapDetector` then marks the voice as overlapping,
  which changes voice assignment and `staffMove`.
- **Filler rests**: without `Measure::checkMeasure`, the
  `<forward>`/`<backup>` do not match and notes go missing.
- **Grace notes after the chord**: a grace note carrying `<slur type="stop">` without
  a start is a grace note *after* (`mustStopGraceAfter`); its note
  type becomes `GRACE8_AFTER`/`GRACE16_AFTER`/`GRACE32_AFTER` at attachment.
- **Ottava**: MusicXML pitches are **written** (shifted by
  `octave-shift`); the import removes the shift (`xmlSetPitch`, `pitch += 12 ×
  octaveShift`) to store the sounding pitch, the export reapplies it.
- **Transposition**: a `<transpose>` following an instrument change
  belongs to that change; each note's pitch is computed with
  the instrument in force at **its tick** (`Part::instrument_at`), not with the
  last `<transpose>` read.
- **Beam stem direction**: MusicXML does not carry the direction
  of the group; it is taken from the `<stem>` of the first chord (`beam_direction`),
  otherwise the export recomputes and gets cross beams wrong.
- **Multiple `<notations>` groups**: the same note can have several
  `<notations>` (invisible tied then visible fingering); all must be traversed,
  and `print-object="no"` is inherited by each of their children.
- **Annotation order**: harmonies, directions and spanners are
  written in file order; the order of voices processed by the export can
  move an annotation from one voice to another (`v3/layout_elements`).
