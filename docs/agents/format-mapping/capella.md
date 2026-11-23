# Capella (`.cap`, `.capx`) ↔ scoretk model

Reference documents (outside the repository): **none published**.
`specs/README.md` § Capella records the absence; the only reference is the
MuseScore **4.7.4** importer:

- `ref-musescore/.474/src/importexport/capella/internal/capella.h` — the
  importer's complete model of the format;
- `capella.cpp` — `.cap` binary reader + `convertCapella`;
- `capxml.cpp` — `.capx` (ZIP + CapXML 1.0/2.0) reader;
- `capellareader.cpp:36-46` — suffix dispatch; `tests/data/` — 27 inputs and
  4.7.4 `*.mscx` references.

Format reference: [`format-ref/capella.md`](../format-ref/capella.md).
**Import only**, planned crates `scoretk-capella-{common,import}`. The pivot
has no Capella writer, so no round-trip obligation — but every dropped item
must be reported (`model-modularisation.md` §6).

**Status**: neither crate exists yet. The *Model* column is the pivot's
capability (`crates/scoretk-core/src/model`, file:line); **Status** says
whether a faithful port is **direct**, **conversion** (computed mapping or
approximation) or **missing**. Layers: L0 core, L1 metric, L2 events, L3 text,
L4 playback, L5 engraving/layout, L6 extension
(`docs/agents/model-modularisation.md` §5.1).

| Capella property | Model type / field (file:line) | Status | Layer | MuseScore 4.7.4 (C++ file:line) | Notes |
| --- | --- | --- | --- | --- | --- |
| `.cap` / `.capx` container | — (facade detection) | direct | — | `capellareader.cpp:36-46`; `capxml.cpp:1414-1439` | `.capx` is a ZIP whose only entry is `score.xml`. |
| XML encoding (ISO-8859-1 / UTF-8) | — | direct | — | `capxml.cpp:1425-1428` | 1.0 files declare Latin-1, 2.0 files UTF-8. |
| Page size, margins, spacing, fonts | `StyleSettings` page/margins/spatium (`style.rs:48-111`) | conversion | L5 | `capella.cpp:1189-1191` (`spatium`, small staff); `capxml.cpp:1230-1250` | MuseScore keeps spatium and a few distances, drops most of the page geometry. |
| Title / composer / lyricist text frames | `Credit` (`text.rs:586-…`), `ScoreMetadata` (`score.rs:51-91`) | conversion | L3/L5 | `capella.cpp:1293-1335` (`backgroundChord` text by alignment) | Alignment 0/1/2 → lyricist/title/composer in the reference; text position is a user offset. |
| `staffLayout/@description`, `name`, `abbrev` | `Part::name`, `long_name`, `short_name` (`part.rs:251-255`) | direct | L3 | `capella.cpp:1240-1247` | Empty description is normal; it is a key, not a display name. |
| `sound/@instr`, `@volume`, `@pan` | `Instrument::midi_program/volume/pan` (`part.rs:71-97`) | direct | L4 | `capella.cpp:1249-1256` | 0-based program; percussion channel forces program 128 (`:1251-1254`). |
| `transp` (semitones) | `Transpose { diatonic, chromatic }` (`part.rs:135-140`) | conversion | L4 | `capella.cpp:1268-1277` | Diatonic component is **guessed** from the chromatic value by a 23-entry table; document the approximation. |
| `notation/@defaultClef`, `clefSign` | `Staff::default_clef` (`part.rs:391`), `Clef` segment (`segment.rs:377-382`), `ClefType` (`segment.rs:250-302`) | conversion | L1 | `capxml.cpp:233-260`; `capella.cpp:841-880` | Capella names only treble/bass/`G2-`; unknown names default to treble. |
| `keySign/@fifths` | `KeySig` (`segment.rs:118-129`), `Key` (`segment.rs:43-99`) | direct | L1 | `capxml.cpp:262-271`; `capella.cpp:882-901` | Concert/transposed key split computed from `Transpose`. |
| `timeSign`, `staff/@defaultTime` | `TimeSig` (`segment.rs:166-180`) | conversion | L1 | `capxml.cpp:274-340`; `capella.cpp:903-932` | `C` → 4/4, `allaBreve` → 2/2 with sign, `longAllaBreve` → 4/2, `infinite` unsupported. |
| `barline/@type`, `@barMode` | `Measure::start_bar_line/end_bar_line` (`measure.rs:80-83`), `BarLineType` (`segment.rs:413-487`), repeat flags (`measure.rs:87`) | conversion | L1 | `capxml.cpp:198-231`; `capella.cpp:933-999` | No measure elements exist: barlines delimit measures implicitly and an irregular measure is the duration actually accumulated. |
| `barlines/@mode,@from,@to` (staff layout) | `Staff::bar_line_span` (`part.rs:382`) | conversion | L5 | `capella.cpp:1261-1267` | MuseScore only keeps "barline spans this staff" (`mode == 1`), not from/to. |
| Staff brackets | `StaffBracket { kind, span }` (`part.rs:341-347`), `Staff::brackets` (`part.rs:385`) | conversion | L5 | `capella.cpp:1282-1291`; `capxml.cpp:1209-1229` | `curly` → brace, else normal bracket; nesting order lost. |
| Number of staves / parts | `Part`/`Staff` (`part.rs:246-266`, `:354-392`) | conversion | L0 | `capella.cpp:1168-1181` (`needPart`), `:1234-1281` | Consecutive staves sharing a MIDI program become one part; the heuristic is not metadata. |
| `system/@tempo` | `Tempo` annotation (`annotation.rs:336-360`) | missing | L3/L4 | read at `capella.cpp:2665`, **never applied** (`:1424` only calls `setUpTempoMap`) | The 4.7.4 reference drops Capella's system tempo entirely; a port that keeps it is an intentional improvement and must be documented as a divergence. |
| `system/@justified`, `@leftIndent`, `expectedNumberOfSystems`, page breaks | `Measure::line_break`, `section_break_before` (`measure.rs:126-128`), style (`style.rs:48-111`) | partial | L5 | `capella.cpp:1350-1450` | System layout is recomputed; only explicit page breaks survive. |
| Chord duration `base` + `dots` | `DurationType` (`duration.rs:37-99`), `Duration.dots` (`duration.rs:159-164`), `ChordRest::duration/ticks` (`chord.rs:341-348`) | direct | L0 | `capxml.cpp:139-187`; `capella.cpp:1847-1902` (binary); `:650-664` (conversion) | `base` fraction for notes, integer count for multi-measure rests. |
| `noDuration` grace chord | `GraceType` (`chord.rs:87-107`), `GraceChord` (`chord.rs:268-281`) | conversion | L2 | `capella.cpp:659-715`, `:119-155` (`SetCapGraceDuration`) | No explicit grace type: inferred from stem/beam/`postGrace`. |
| Rest `fullMeasures` | `Measure::multi_measure_rest` (`measure.rs:93`), `MultiMeasureRests` (`mmrest.rs:32-60`) | conversion | L1 | `capxml.cpp:550-592`; `capella.cpp:584-620` | Capella's count is expanded by MuseScore into measure rests. |
| Rest `invisible`, `vertShift` | `Rest::visible` (`chord.rs:320`), `Rest::gap` (`chord.rs:318`) | partial | L0/L5 | `capella.cpp:560-580` | Vertical shift has no pivot field; `invisible` + zero ticks = placeholder, dropped. |
| Chord heads `pitch`, `alter`, `shape` | `Note::pitch`, `tpc`, `accidental` (`note.rs:266-296`), `NoteHeadGroup` (`note.rs:138-200`) | conversion | L0/L5 | `capxml.cpp:487-548`; `capella.cpp:760-812` | `.capx` pitch is MIDI with **C5 = middle C**; `.cap` is diatonic relative to clef+key. `shape="none"` → cross head. |
| `explAlteration` (force/suppress accidental) | `Accidental::user`, `visible` (`note.rs:232-246`) | missing | L0 | parsed (`capella.cpp:1994-2016`, `capxml.cpp:511-517`) but **never applied**; only `N.alteration` is added (`:795`) | MuseScore prints the accidental the key implies; the pivot can hide one but has no "explicitly none" flag. A faithful port drops it and reports. |
| Ties (`<tie>` per head, binary flags) | `Note::tie_forward/tie_back` (`note.rs:276-294`) | conversion | L2 | `capxml.cpp:487-548`; `capella.cpp:803-810` | Reference applies the chord's `rightTie` to every note; a faithful port reproduces or documents this. |
| Tuplet `count` + `<bracket noteRange>` | `Tuplet` (`tuplet.rs:53-81`), `ChordRest::tuplet` (`chord.rs:350`) | conversion | L1 | `capxml.cpp:530-547`, `:865-950`; `capella.cpp:382-419` | Reconstructed by counting; `tripartite`/`prolong` change the ratio (`n/3`, doubled). |
| Beam `group="force"` / `AUTO`/`SPLIT` | `ChordRest::beam_mode` (`chord.rs:354-356`), `BeamMode` (`chord.rs:45-69`) | conversion | L5 | `capxml.cpp:407-410`; `capella.cpp:1933-1990`, `:700-706` | `SPLIT` maps to `BeamMode::None` at that chord; there is no "split here" bit. |
| Stem `dir`, `dStemLength` | `Chord::stem` (`chord.rs:233`), `StemDirection` (`chord.rs:72-85`) | conversion | L5 | `capxml.cpp:343-358`; `capella.cpp:700-706` | Stem length has no field (L5 loss). |
| Cross-staff `notationStave` (±1) | `Chord::staff_move` (`chord.rs:238`) | direct | L5 | `capella.cpp:1946-1955` (read), `:700-706` (applied) | Limited to ±1 by the format. |
| Tremolo bars `nTremoloBars` | `Tremolo { kind, count }` (`notation.rs:200-227`) | missing | L2 | read (`capella.cpp:1958`) but used only to distinguish acciaccatura from appoggiatura (`:119-125`); never emitted as a tremolo | Do not port it as a tremolo without deciding against the oracle (`test*` fixtures have no tremolo). |
| Articulation code | `Articulation` (`chord.rs:187-205`), `ArticulationType` (`chord.rs:125-185`) | conversion | L2 | `capxml.cpp:360-386`; `capella.cpp:816-829` | Only staccato/tenuto/staccatissimo/accents converted; codes 7/8 logged. |
| Lyrics `<verse i hyphen>` | `Lyrics { verse, syllabic, text }` (`lyrics.rs:36-63`) | conversion | L3 | `capxml.cpp:465-485`; `capella.cpp:813-816` | `hyphen` → `LyricsSyllabic::Begin`; extender/leftAlign dropped. |
| `<slur>` (draw object) | `SpannerType::Slur` (`spanner.rs:268-330`), `Slur` (`notation.rs:387-403`) | conversion | L2 | `capxml.cpp:650-656`; `capella.cpp:1004-1050` | Span from `noteRange`; CapXML geometry discarded, binary Bézier points unused. |
| `<wedge>` | `SpannerType::Hairpin` (`spanner.rs:268-276`), `Hairpin` (`hairpin.rs:41-60`) | direct | L2 | `capxml.cpp:705-717`; `capella.cpp:1101-1130` | `decrescendo` → `HairpinType::Decrescendo`. |
| `<volta firstNumber lastNumber>` | `SpannerType::Volta` (`spanner.rs:268-276`) + `VoltaInfo` (`repeat.rs:282-287`) | conversion | L1 | `capxml.cpp:660-685`; `capella.cpp:1052-1076` | The span comes from `x1`/`x2`, not from measure numbers. |
| `<trill>` | `SpannerType::Trill` (`spanner.rs:268-276`), `Trill` (`notation.rs:346-383`) | direct | L2 | `capxml.cpp:687-703`; `capella.cpp:1077-1100` | `tr="false"` = wavy line without "tr". |
| `<text>` in a normal font | `StaffText` (`annotation.rs:231-249`) | direct | L3 | `capella.cpp:161-330` | Position/font → user offset + L5. |
| `<text>` in the `capella3` font | `Dynamic` (`annotation.rs:197-228`), `Fermata` (`notation.rs:48-98`), `Arpeggio` (`notation.rs:229-259`), `Breath` (`segment.rs:488-533`) | conversion | L2/L3 | `capella.cpp:161-300` | Glyph → object table (format-ref §7). Everything not in the table is dropped with a log. |
| `<transposable>` | `Harmony` (`annotation.rs:428-478`) | conversion | L3 | `capella.cpp:330-357` | Concatenated variants; `Q/S/R` glyphs → `b`, `#`, `natural`; the pivot's structured fields need a parser. |
| `<bracket noteRange>` (tuplet) | — (consumed) | direct | L1 | `capxml.cpp:718-727`, `:530-547` | Not exported as a bracket; the tuplet's own bracket flag exists (`tuplet.rs:63`). |
| `<guitar>` (chord frame) | — | missing | L6 | skipped (`capxml.cpp:781-783`) | Capella's guitar object is not imported; the pivot has no chord diagram type either (`format-mapping/musescore.md`). |
| `<line>`, `<rectangle>`, `<ellipse>`, `<polygon>`, `<wavyLine>`, `<metafile>`, `<notelines>`, `<group>` | — | missing | L5 | skipped (`capxml.cpp:751-816`) | Pure graphics; report as L5 loss. |
| Percussion `soundMapIn/Out` (128 entries) | `InstrumentChannel` (`instrument_channel.rs:35-55`) is per-channel, not a drum map | missing | L6 | `capella.cpp:2280-2305` (read), `:1249-1254` | MuseScore only sets program 128 for percussion; the maps are not converted. |
| Per-object coordinates (text, lines, slurs) | — | missing | L5 | `capella.cpp:300-316`; `capxml.cpp:595-624` | The pivot has no coordinates at all (§7.3 of `model-modularisation.md`). |
| `encodingSoftware`, author/keywords/comment (`.cap`) | `ScoreMetadata` has no such fields | missing | L6 | `capella.cpp:2756-2758` | Keep in an L6 blob or drop with a record. |

## Gaps in the model, ranked

1. **No coordinates.** Text offsets, slur geometry, line/rectangle objects and
   page layout cannot be stored. This is the gap already named in
   `model-modularisation.md` §7.3; the proposal (opaque per-object offsets in
   L5) is confirmed by the amount of geometry in Capella (slur Bézier points,
   `x1`/`x2` voltas, `relPos` text).
2. **No "suppress accidental" flag.** `CNote::explAlteration == 2` and
   `Accidental::visible` are not the same thing; the pivot can hide an
   accidental but cannot say "explicitly no accidental here".
3. **No drum map.** Capella's per-staff 128-entry sound maps have no
   counterpart; `InstrumentChannel` describes a channel, not a percussion
   table.
4. **No opaque blob / source metadata.** `encodingSoftware`, author,
   keywords, comment (`.cap`) and CapXML's `info` have nowhere to go.
5. **No chord-diagram type** (guitar object), consistent with
   `format-mapping/musescore.md:46-48`.
6. **Tremolo bars read but unused** by the reference: reproducing that is a
   *decision*, not a gap; document it either way.
7. **Beam `SPLIT`** has no dedicated pivot value; `BeamMode::None` on the
   chord is the closest approximation.

No row contradicts `model-modularisation.md` §7.3. The mapping confirms the
"parse music into L0–L4, put positions in L5, report the rest" split; it only
makes the **volume** of opaque layout data explicit (slur Béziers, `x1`/`x2`
voltas, 128-entry percussion sound maps), which is an argument for the L5
proposal, not against it.

## Oracle and reference candidates

- **MuseScore 4.7.4 CLI** (`mscore`, installed at
  `/run/current-system/sw/bin/mscore`) is the only implementation and the
  compliance target: `mscore -o out.musicxml in.cap` / `in.capx`. Its MusicXML
  export is already a trusted, measured direction, so it is the natural
  oracle for the Capella reader.
- **The 27 paired inputs** in
  `ref-musescore/.474/src/importexport/capella/tests/data/`:
  `test1..8.cap`, `test1..8.capx`, `testSlurTie`, `testVolta1`,
  `testBarline`, `testText1`, `testEmptyStaff1/2`, `testPianoG4G5`,
  `testScaleC4C5`, `testTuplet1/2/3`, each with a `*-ref.mscx` produced by
  4.7.4. Licence **GPL-3.0-only** (MuseScore upstream), usable as an
  out-of-repository oracle like the existing corpus.
- No third-party library reads Capella that could serve as a second opinion
  (no public spec, no open reader besides MuseScore); do not invent one.

## Corpus candidates

1. The 27 MuseScore test pairs (no download, GPL-3, exact 4.7.4 behaviour).
2. The `.capx` files extracted from those same archives are themselves small
   hand-checkable CapXML fixtures (one bar, one tuplet, one volta).
3. Do **not** redistribute Capella application files from the wild: their
   licence is unknown and they often contain personal data. If a real-world
   file is needed, keep it out of the repository like `ref-musescore/`.

## Suggested task breakdown

Numbers continue after `t43`; MEI takes t44 (see
`format-mapping/mei.md`), Capella t45.

- **t45.1 — crate skeleton.** `scoretk-capella-{common,import}`, `capella-import`
  facade feature, `.cap`/`.capx` detection by suffix (content sniffing is not
  reliable: the ZIP signature is shared with `.mxl`/`.mscz`).
- **t45.2 — `.capx` reader into a Capella-shaped model.** Port
  `capella.h` + `capxml.cpp`: layout, systems, staff/voice, note objects,
  draw objects. Keep the intermediate model private to the crate, exactly like
  the reference; do not let it leak into the pivot.
- **t45.3 — conversion to the pivot (L0/L1).** Chords/rests/durations,
  clefs/keys/meters, implicit measures and barlines, repeats, multi-measure
  rests, grace notes.
- **t45.4 — conversion (L2/L3).** Slurs, hairpins, voltas, trills, lyrics,
  articulations, `capella3` glyphs → dynamics/fermatas/arpeggios/breaths,
  chord symbols.
- **t45.5 — `.cap` binary reader.** Same intermediate model; test that
  `.cap` and `.capx` versions of the same fixture converge.
- **t45.6 — measurement.** `mscore -o out.musicxml` as oracle over the 24
  pairs, musical-core level first, then full parity; two workers maximum.
- **t45.7 — loss report and docs.** Every skipped draw object, coordinate,
  sound map, `explAlteration` and unsupported clef becomes a `Loss`; write
  `docs/agents/capella.md` with the measured figures.

## Risks

- **No specification.** Every claim must be traced to the C++ or to a fixture;
  a "reasonable" reading of Capella is unverifiable. This document's table is
  the trace.
- **Two pitch conventions** (`.cap` diatonic, `.capx` MIDI) with a `C5`
  middle C in CapXML: a shared conversion path must branch on the container,
  as `convertCapella(..., capxMode)` does.
- **Implicit measures.** A reader that assumes a measure element (as in
  MusicXML/MEI) will mis-time every irregular bar. The measure boundary is a
  barline object, and an irregular measure is defined by the accumulated
  duration.
- **Tuplets reconstructed by counting** are fragile on mixed durations and
  on `tripartite`/`prolong`; the fixture `testTuplet2.capx` (3, 5 and 7) is
  the regression case.
- **`capella3` glyphs.** A font that is not installed still yields the
  character codes in the XML; never render, map by code.
- **Licence.** `capella.h` and the test data are GPL-3.0-only upstream: a
  port is a derivative and must carry the modification notice
  (`docs/agents/developpement.md`); do not copy the fixtures into
  `tests/corpus` without a NOTICE entry.
