# Humdrum `**kern` ↔ scoretk model

Reference documents (outside the repository): `specs/README.md` § Humdrum;
`specs/humdrum/kern-reference.html` (the `**kern` syntax text — **no stated
licence**, private: cite sections, do not copy prose);
`specs/humdrum/humlib/` (BSD-2-Clause parser and analyses);
`specs/humdrum/verovio/src/iohumdrum.cpp` (LGPL-3.0-or-later `**kern` → MEI
converter, reimplement — never copy).

Format reference: [`format-ref/humdrum.md`](../format-ref/humdrum.md).
Planned crates `scoretk-humdrum-{common,import,export}`; import first, export
after MEI (the model can already hold most of `**kern`'s musical content).

**Status**: no crate exists yet. *Model* is the pivot's capability
(`crates/scoretk-core/src/model`, file:line); **Status** is whether a faithful
port is **direct**, **conversion** or **missing**. Layers: L0 core, L1 metric,
L2 events, L3 text, L4 playback, L5 engraving, L6 extension
(`docs/agents/model-modularisation.md` §5.1).

| Humdrum property | Model type / field (file:line) | Status | Layer | humlib / Verovio | Notes |
| --- | --- | --- | --- | --- | --- |
| File = text records, tab-separated tokens | — | conversion | — | `HumdrumFileBase`, `HumdrumLine`, `HumdrumToken` (`include/HumdrumFileBase.h`, `HumdrumLine.h`, `HumdrumToken.h`) | No container; the parser must split on tabs only. |
| Spine as a first-class, mutable column | — | **missing** | L6 | `analyzeSpines` (`HumdrumFileBase.h:355`), manipulators (`HumdrumToken.h:52-86`); Verovio `storeStaffLayerTokensForMeasure` `iohumdrum.cpp:10132-10292` | The pivot's `Track` is `staff*4+voice` (`ids.rs:118-145`), fixed and bounded. Spines can appear, split, join, exchange. |
| Exclusive interpretation `**kern` | `Staff`/voice via `Track` (`ids.rs:118-145`) | conversion | L0 | `isExclusiveInterpretation` (`HumdrumToken.h:51`); Verovio `getStaffNumForSpine` `:1067-1086` | One `**kern` spine = one voice; `*staffN` assigns the staff. |
| `**bar` spine | `Measure::number` (`measure.rs:56-59`) | conversion | L1 | — (Verovio also reads `=` records) | Bar numbers can also come from barline labels; the two must agree or be reconciled. |
| `**harm`, `**mxhm`, `**rhrm` | `Harmony` (`annotation.rs:428-478`) | conversion | L3 | `analyzeHarmInterpretations` `iohumdrum.cpp:1211-1321` | Text/degree/roman vocabularies differ; the structured fields need a parser. |
| `**dyn`, `**dynam` | `Dynamic` (`annotation.rs:197-228`) | conversion | L3 | `getAssociatedDynamSpine` `:7995-8021` | A dynamic spine is attached to the staff below/above by position. |
| `**text`, `**silbe` | `Lyrics` (`lyrics.rs:36-63`) | conversion | L3 | `prepareVerses` `:5308-5354` | Syllable text, hyphen and melisma conventions differ from `**kern` tokens. |
| `**fb`, `**fba`, `**Bnum` | `FiguredBass` (`figured_bass.rs:118-165`) | conversion | L3 | `addFiguredBassForMeasure` `:10473-10652` | The pivot's figured bass is structured; Humdrum's is a text spine. |
| `**recip` | durations only | conversion | L1 | `prepareTimeSigDur` `:5355-5425` | A rhythm-only spine has no pitch; it only validates durations. |
| `**mens` | — | **missing** | L6 | `checkForMens` `:1837-1860` | Mensural notation is outside the pivot (and MEI's mensural chapter is not local). |
| `!!!OTL`, `!!!COM`, `!!!YEC`, … | `ScoreMetadata` (`score.rs:51-91`) | conversion | L3 | `createFileDesc` `:2867-3052`, `getAllReferenceItems` `:5156-5292` | Reference records have repeats and language selectors; `work_number`/`movement_*` have no direct key. |
| `*M` meter | `TimeSig` (`segment.rs:166-180`), `Measure::time_sig_*` (`measure.rs:71-73`) | conversion | L1 | `setTimeSig` `:9019-9246`; `analyzeMeter` (`HumdrumFileStructure.h:144`) | Per spine and mid-file; `*M` is optional, so a default must be invented and reported. |
| `*k[...]` key signature | `KeySig` (`segment.rs:118-129`), `Key` (`:43-99`) | conversion | L1 | `setKeySig` `:9415-9547`; `analyzeAccidentals` (`HumdrumFileContent.h:42`) | The signature is informational: notes carry their own accidentals. |
| `*c#:` key (tonic) | `KeySig::mode` (`segment.rs:101-129`), `KeyMode` | conversion | L1 | `setKeySig` `:9548-9568` | Major/minor/modal tonics; the pivot keeps the signature and a mode. |
| `*MM` tempo | `Tempo` (`annotation.rs:336-360`) | conversion | L3/L4 | `addMidiTempo` `:7470-7540`; `handleTempoChange` `:15485-15537` | `*MM96.3` is a decimal BPM; the pivot stores beats/second + text. |
| `*tb` time base | `ChordRest::duration`/`ticks` (`chord.rs:341-348`) | conversion | L1 | `HumdrumToken::getDuration` (`HumdrumToken.h:192-197`) | A time-base change rescales the duration integers; it must be consumed, not stored. |
| `*clef` | `Clef` (`segment.rs:377-382`), `ClefType` (`:250-302`) | conversion | L1 | `setClef` `:9680-9773` | Mid-file clef changes; `*clefG2`, `*clefF4`, `*clefX`… |
| `*staffN` | `StaffId` (`ids.rs:78-85`), `Staff` (`part.rs:354-392`) | conversion | L0 | `checkIfReversedSpineOrder` `:1019-1066`; `getStaffNumForSpine` `:1067-1086` | Decides staff assignment *and* order; without it the order is ambiguous. |
| `*I"`, `*I'` | `Part::long_name`/`short_name` (`part.rs:251-255`) | direct | L3 | `fillStaffInfo` `:7572-7994`; `getInstrumentName` `:5691-5713` | `"` full name, `'` abbreviated name. |
| `*IC` instrument class, `*IG` group | `StaffBracket` (`part.rs:341-347`) only groups staves | partial | L5/L6 | `prepareStaffGroups` `:5445-5517`; `getInstrumentClass` `:6487-6513` | Class/group are semantic taxonomies, not brackets; the pivot cannot hold them. |
| `*ITr` transposition | `Transpose` (`part.rs:135-140`) | conversion | L4 | `setTransposition` `:9643-9659` | Semitone/decimal notation to a diatonic+chromatic interval. |
| `*met(c)`, `*met(O)` | `TimeSig` (`segment.rs:166-180`) | conversion | L1 | `setMensurationSymbol` `:8682-9018` | Mensuration symbols are approximated by a `TimeSigType`; report the loss. |
| Note token pitch (`c`, `cc`, `C`, `C#`, `e-`) | `Note::pitch`, `tpc` (`note.rs:266-293`) | conversion | L0 | `HumdrumToken::isNote` (`HumdrumToken.h:98`); `analyzeAccidentals` (`HumdrumFileContent-accidental.cpp`) | Case/repetition encode the octave; the accidental is explicit. |
| Accidental signifier `#`, `##`, `-`, `--`, `n` | `Note::accidental` (`note.rs:232-246`, field `:296`), `AccidentalType` (`:201-231`) | conversion | L0 | `analyzeAccidentals`; Verovio `analyzeAccidentals` `:767` | Unlike MusicXML/MEI, every note carries its accidental: do not re-derive it from the key. |
| Duration reciprocal `4`, `8.`, `0`, `3`, `6`, `12` | `DurationType` (`duration.rs:37-99`), `Duration.dots` (`:159-164`) | conversion | L0 | `HumdrumToken::getDuration` (`HumdrumToken.h:192`) | Reciprocal value; `3/6/12` are triplet values, not counts; `0` = breve. |
| Tuplet durations | `Tuplet` (`tuplet.rs:53-81`), `ChordRest::tuplet` (`chord.rs:350`) | conversion | L1 | `HumdrumFileStructure::analyzeRhythmStructure` (`HumdrumFileStructure.h:131`); Verovio `getDurAndDots` `:14958-15049` | The ratio must be inferred from the reciprocal and the metric position; Verovio rewrites non-expressible durations (`isExpressibleDuration` `:14868-14929`). |
| Rest `r`, `rr` (whole-rest glyph) | `Rest` (`chord.rs:313-320`), `Rest::full_measure` (`:316`) | conversion | L0/L1 | `HumdrumToken::isRest` (`HumdrumToken.h:97`) | `2.rr` is a whole-rest glyph with a different actual duration; `full_measure` is the closest field. |
| Null token `.` | — (no event) | direct | L0 | `HumdrumToken::isNull` (`HumdrumToken.h:46`) | A sustain, not a rest: skip the token, do not create anything. |
| Tie `[ _ ]` | `Note::tie_forward/tie_back` (`note.rs:276-294`) | conversion | L2 | `analyzeKernTies` (`HumdrumFileContent.h:41`); Verovio tie processing `:1959-2290` | Per note of a multiple stop. |
| Slur `( )` | `SpannerType::Slur` (`spanner.rs:268-276`), `Slur` (`notation.rs:387-403`) | conversion | L2 | `analyzeSlurs` (`HumdrumFileContent.h:37`) | Per multiple stop; nested slurs use `&` and numbers. |
| Phrase `{ }` | `SpannerType::Other("Phrase")` at best | **missing** | L2 | `analyzePhrasings` (`HumdrumFileContent.h`) | No phrase-mark type in the pivot. |
| Beam `L`/`J`/`K`/`k` | `ChordRest::beam_mode` (`chord.rs:354-356`), `BeamMode` (`:45-69`) | conversion | L5 | `analyzeBeams` (`HumdrumFileContent.h:38`); Verovio `handleGroupStarts` `:13334-13663` | `LJ` levels → `Begin`/`Mid`/`End`; `K/k` → `ForwardHook`/`BackwardHook`. |
| Stem `/` `\` | `Chord::stem` (`chord.rs:233`), `StemDirection` (`:72-85`) | direct | L5 | `analyzeKernStemLengths` (commented in Verovio `:764`) | — |
| Ornaments `m M t T S $ O R w W` | `Ornament` (`notation.rs:158-183`), `OrnamentType` (`:100-145`) | conversion | L2 | Verovio token handling in `fillContentsOfLayer` `:13827-14843` | Names map 1:1 except `R` ("ornament ends with turn") which has no pivot flag. |
| Articulations `' \` ~ ^ ; , z s " I o U` | `Articulation` (`chord.rs:187-205`), `ArticulationType` (`:125-185`) | conversion | L2 | Verovio `fillContentsOfLayer` | `z` (sforzando) and `o` (harmonic) have no exact pivot articulation; `Other` or dynamic. |
| Bowing `u`/`v` | `ArticulationType::UpBow/DownBow` | direct | L2 | — | — |
| Glissando `H`/`h` | `SpannerType::Glissando` (`spanner.rs:268-276`), `Glissando` (`notation.rs:307-340`) | direct | L2 | Verovio `fillContentsOfLayer` | A pair across tokens; must be matched by spine and note order. |
| Grace `q` | `GraceType` (`chord.rs:87-107`), `GraceChord` (`chord.rs:268-281`) | conversion | L2 | Verovio `fillContentsOfLayer` | Durationless; attach to the following (or preceding) chord. |
| Groupetto `Q` | — | **missing** | L2 | — | A small note that keeps its duration; the pivot has grace chords (no time) and normal notes, nothing between. |
| Appoggiatura `P`/`p` | `GraceType::Appoggiatura` | conversion | L2 | — | `**kern` already encodes the performed duration; do not re-shorten. |
| Barline `=`, `==`, `=N`, `=-`, `=N-` | `Measure::start/end_bar_line` (`measure.rs:80-83`), `BarLineType` (`segment.rs:413-487`) | conversion | L1 | `analyzeBarlines` (`HumdrumFileContent.h:116`) | A measure is the data between barline records; the pivot requires explicit `Measure` objects. |
| Barline labels `=1a`, `=2b` | `Measure::number`, `number_offset` (`measure.rs:56-59`) | conversion | L1 | — | Alternate measure labels are the only encoding of first/second endings. |
| Repeat signifiers in barlines (`=:\|:`, `=:\|\|:`) | repeat flags (`measure.rs:87`), `SpannerType::Volta` + `VoltaInfo` (`repeat.rs:282-287`) | conversion | L1 | `analyzeBarlines` (`HumdrumFileContent.h:116`) | The printed repeat and the played repeat are distinct in Humdrum; the pivot has only the printed structure. |
| Multiple stop (`4c 4e` in one token) | `Chord::notes` (`chord.rs:220-262`) | direct | L0 | Verovio `hasParallelNote` `:1712-1745` | Same duration by definition. |
| Spine split/join `*^`/`*v` | `Track` (`ids.rs:118-145`) | conversion | L0/L6 | `isSplitInterpretation`/`isExchangeInterpretation` (`HumdrumToken.h:52-56`) | Different-duration chords require splits; the pivot's 4 voices/staff bound is the limit. |
| Editorial `x`, `X`, `y`, `Y`, `?` | — | **missing** | L6 | — | Editorial status (added/implied/questionable) has no pivot field. |
| Local/global comments `!`, `!!` | — | **missing** | L6 | — | Not musical; drop with a record if a `Loss` report is desired. |

## Gaps in the model, ranked

1. **Spine model (`L6::Humdrum`).** The pivot cannot represent an arbitrary
   number of parallel spines with per-spine interpretations, nor a column
   whose identity changes through `*^`/`*v`/`*+`/`*x`/`*-`. `Track` is
   `staff*4+voice` and bounded at 4. This is the gap of
   `model-modularisation.md` §7.5 and it is structural, not cosmetic.
2. **Measure-less and signature-less data.** `Measure` requires
   `start_tick`/`len`/`number`; `**kern` may have no `*M`, no `*k` and no `=`
   at all (or only at the end). A port must either fabricate measures from the
   barlines or hold the raw stream in L6 and project onto L1.
3. **Groupetto (`Q`) and phrase marks (`{ }`).** No pivot type for either.
4. **Editorial signifiers** (`x/X/y/Y/?`): "added", "implied", "questionable"
   have no representation; dropping them silently changes the scholarly
   meaning of a file.
5. **Mensural notation** (`**mens`): entirely outside the pivot.
6. **`**harm` vocabularies** (degrees, roman numerals, `**mxhm`): the pivot's
   `Harmony` is chord-symbol shaped.
7. **Alternate measures via labels** (`=1a`/`=1b`): no explicit volta
   structure; the reader must infer the endings, and any inference must be
   reported.
8. **>4 voices per staff** (shared with MEI).

### Contradiction found in `model-modularisation.md` §7.5

§7.5 cites "§4.1 (spine paths `*>`/`*v`)". The local
`kern-reference.html` §4.1 (`:4237-4395`) documents `*+`, `*^`, `*v`, `*-`
and `*x` — **not** `*>`. Neither humlib (`isSplitInterpretation`,
`isJoinInterpretation`, `isExchangeInterpretation`, `isAddInterpretation`,
`isTerminator`, `HumdrumToken.h:52-86`) nor Verovio mentions `*>`. §7.5 has
been corrected to the five documented manipulators; if `*>` exists in a newer
Humdrum draft, it is not in the local reference and must not be assumed.

## Oracle and reference candidates

- **Verovio** is the only complete `**kern` → notation implementation
  available (LGPL-3.0-or-later, `specs/humdrum/verovio/`, sparse: only
  `iohumdrum.cpp` and headers). Not installed. Build it out-of-repo from a
  full clone at the recorded commit, then `verovio score.krn -o out.mei` is
  the oracle; compare through the MEI reader (t44) or convert the MEI to
  MusicXML with `mscore`.
- **humlib** (BSD-2, local and complete) is the *parsing* oracle: its
  `analyzeRhythmStructure`/`analyzeBarlines`/`analyzeKernTies` define the
  expected tick and barline structure, and `HumdrumToken::getDuration` the
  expected duration. A small out-of-repo harness can dump
  `(spine, token, duration, from-start)` and compare with our reader.
- **hum2xml** (Craig Sapp, separate repository, BSD-2) is a direct
  Humdrum → MusicXML converter; **not local**. Fetch only after recording its
  licence in `specs/README.md`; it would give a second MusicXML oracle.
- **MuseScore cannot read `**kern`**, so it is only usable downstream
  (MEI/MusicXML produced by Verovio → `mscore`).

## Corpus candidates

1. `specs/humdrum/humlib/tests/files/*.krn` (13 tiny files: manipulators,
   spine split, meter change, null tokens, rhythms, text, parameters,
   floating spines) and `tests/test-slurs/slur.krn` — BSD-2, no download,
   ideal unit fixtures.
2. Hand-written fixtures for the traps of
   [`format-ref/humdrum.md`](../format-ref/humdrum.md): null vs rest,
   `2.rr`, `3`/`6`/`12` tuplets, grace/groupetto/appoggiatura, `*tb`,
   editorial signifiers, a 5-spine split.
3. **KernScores** (kern.humdrum.org) and the Bach chorales: large, real, but
   the licence is **per file**; use only after recording it. Music itself is
   public domain, the encoding may not be.
4. Verovio's own test suite (in the full repository, not local): LGPL-3 test
   data, useful once the oracle is built.

## Suggested task breakdown

Numbers continue after `t43`; MEI = t44, Capella = t45, Guitar Pro = t46,
Humdrum = t47 (last, as `docs/TODO.md:39-41` says, because it benefits from
the MEI work).

- **t47.1 — parser and spine model.** Records, tokens, spines, manipulators,
  null tokens, comments, reference records; keep the raw spine stream in a
  crate-private model (mirroring `L6::Humdrum`). Unit-test on the humlib
  fixtures.
- **t47.2 — intervals and time.** Durations (reciprocal, dots, tuplets),
  `*tb`, barline-defined measures, `*M`/`*k` per spine, pickup handling.
- **t47.3 — notes and chords.** Pitch case/octave, explicit accidentals,
  multiple stops, spine splits, rests, grace/groupetto/appoggiatura, ties.
- **t47.4 — notation.** Slurs, phrases (report), beams, stems, ornaments,
  articulations, bowing, glissandi, barlines and repeats.
- **t47.5 — companion spines.** `**harm`, `**dyn`, `**text`/`**silbe`,
  `**fb`, `**bar`, `**recip`; report every unmapped exclusive interpretation.
- **t47.6 — writer (after MEI).** Model → `**kern`: one spine per voice,
  `*staff`, `*M`, `*k`, barlines, ties/slurs, beams; the pivot's engraving
  layer (L5) is not representable and must be reported.
- **t47.7 — measurement.** humlib structural comparison on the fixtures, then
  Verovio on a licensed real corpus; two workers maximum.
- **t47.8 — report.** `docs/agents/humdrum.md` with per-file causes.

## Risks

- **Tabs vs spaces** is the single most destructive trap: a chord is a
  space-separated group inside one token. Unit-test it first.
- **Spine identity**: any model indexed by column instead of spine will pass
  simple files and fail on `*^`/`*v`. The humlib fixture
  `test-simple-spine-split.krn` is the minimum regression.
- **Reciprocal durations**: `3` is a triplet, not three units; a reader that
  stores the integer as a count produces plausible but wrong music.
- **Explicit accidentals**: re-deriving them from the key signature adds
  accidentals; never run the MusicXML accidental algorithm on `**kern`.
- **No measures**: `Measure` is mandatory for the ten writers. A fabricated
  measure grid must be a documented projection, not silent data.
- **Licences**: `kern-reference.html` has no stated licence (private; do not
  copy), Verovio is LGPL-3 (reimplement), humlib is BSD-2 (usable as a
  reference and test oracle with its notice). Keep all downloaded data out of
  the repository; never execute scripts from it.
