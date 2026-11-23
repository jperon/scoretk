# Guitar Pro (`.gp3`–`.gp7`, `.gpx`) ↔ scoretk model

Reference documents (outside the repository): **no published specification**
for GP3–GP5 or GP6/GP7 (`specs/README.md` § Guitar Pro). The only reference is
the MuseScore **4.7.4** importer:

- `ref-musescore/.474/src/importexport/guitarpro/internal/gtp/*.h` — the
  in-memory model of the format;
- `importgtp.cpp` (sniffing and GP3–5 binary), `importgtp-gp{3,4,5}.cpp`,
  `gtp/gp67dombuilder.cpp` (GPIF XML), `gtp/gpconverter.cpp` (→ MuseScore);
- `tests/data/` — ≈250 fixture inputs with `*-ref.mscx` references.

Format reference: [`format-ref/guitarpro.md`](../format-ref/guitarpro.md).
**Import only**, planned crates `scoretk-guitarpro-{common,import}`.

**Status**: no crate exists yet. *Model* is the pivot's capability
(`crates/scoretk-core/src/model`, file:line); **Status** is whether a faithful
port is **direct**, **conversion** or **missing**. Layers: L0 core, L1 metric,
L2 events, L3 text, L4 playback, L5 engraving, L6 extension
(`docs/agents/model-modularisation.md` §5.1).

| Guitar Pro property | Model type / field (file:line) | Status | Layer | MuseScore 4.7.4 (C++ file:line) | Notes |
| --- | --- | --- | --- | --- | --- |
| Container/version sniffing | — (facade detection) | direct | — | `importgtp.cpp:2798-2870` | `ptab`/`PK`/`BCFZ`/`FIC`; suffix is not authoritative. |
| Title, subtitle, artist, album, words, music, copyright | `ScoreMetadata` (`score.rs:51-91`) | conversion | L3 | `addMetaInfo` in `importgtp.cpp` (called `:2860`) | GP splits "words" (lyricist) and "music" (composer); `artist`/`album` have no dedicated field. |
| Tempo automations (bar, position, value, linear) | `Tempo` annotations (`annotation.rs:336-360`), `SwingParameters` (`:272-283`) | conversion | L3/L4 | `collectTempoMap`/`addTempoMap` (`gpconverter.cpp:1179-1185`, `:1442-1521`) | GP tempo map is an automation curve; the pivot has point annotations and a tick. Linear interpolations are approximated. |
| Master bar time signature | `TimeSig` (`segment.rs:166-180`) | direct | L1 | `addTimeSig` (`gpconverter.cpp:726-756`) | Per-bar changes; `Time` is `numerator/denominator`. |
| Master bar key signature (+ useFlats) | `KeySig` (`segment.rs:118-129`), `Key` (`segment.rs:43-99`) | conversion | L1 | `addKeySig` (`gpconverter.cpp:886-987`) | GP has a major/minor mode and a `useFlats` spelling hint; the pivot's `KeyMode` and the `Key` value cover the signature, not the hint. |
| Repeat, alternate endings, double bar | `Measure::start/end_bar_line` (`measure.rs:80-83`), `repeat_count` (`:87`), `SpannerType::Volta` + `VoltaInfo` (`repeat.rs:282-287`) | conversion | L1 | `addRepeat`/`addVolta` `gpconverter.cpp:757-829`; `addBarline` `:448-504` | Endings are master-bar numbers; the volta span is computed. |
| Directions (segno, coda, DC, DS, fine) | `Marker`/`Jump` (`repeat.rs:109-240`), `JumpType`/`MarkerType` (`:42-207`) | conversion | L1 | `addDirection`, `gpconverter.cpp:830-847` | `JumpType`/`MarkerType` set from the GP name; text is regenerated. |
| Section (rehearsal name) | `RehearsalMark` (`annotation.rs:370-386`) | direct | L3 | `addSection`, `gpconverter.cpp:848-871` | GP sections are master-bar metadata. |
| Triplet feel (Triplet8th, Dotted16th, Scottish…) | `SwingParameters { unit, ratio }` (`annotation.rs:257-283`) | conversion | L4 | `addTripletFeel`, `gpconverter.cpp:872-885` | GP's six feel types do not map 1:1 to a swing ratio; document the chosen values. |
| Fermatas (short/medium/long, offset) | `Fermata` (`notation.rs:48-98`), `FermataType` (`:48-71`) | conversion | L2 | `collectFermatas`/`addFermatas`, `gpconverter.cpp:1186-1192`, `:1393-1421` | Type → `FermataType`; the offset is a position, not a duration. |
| Free time (no meter) | `Measure::nominal_len`/`len` (`measure.rs:63-65`), `TimeSig` | conversion | L1 | `GPMasterBar::freeTime`, `gpconverter.cpp:346-362` | No pivot flag for "no meter"; a nominal length must be invented and reported. |
| Track → part and staves | `Part` (`part.rs:246-266`), `Staff` (`part.rs:354-392`) | direct | L0 | `setUpTrack`, `gpconverter.cpp:1065-1177` | `staffCount > 1` gets a brace and barline span. |
| MIDI program, channel, volume, pan | `Instrument::midi_program/midi_channel/volume/pan` (`part.rs:71-97`) | direct | L4 | `setUpTrack`, `gpconverter.cpp:1081-1098` | GP volumes/pans are 0–1 floats scaled to 0–127. |
| Track name, short name, instrument name | `Part::long_name/short_name`, `Instrument::long_name/short_name` (`part.rs:251-264`, `:57-64`) | direct | L3 | `gpconverter.cpp:1071-1076` | GP `instrument` is a sound id (`"a-bass4"`), not a GM name. |
| Tuning (per string, MIDI), string count | — | **missing** | L6 | `setUpTrack` builds `StringData` (`gpconverter.cpp:1140-1160`) | No tuning field in the pivot; no `StringData` at all. Tablature is the primary GP data. |
| Capo fret | — | **missing** | L6 | `setCapoFret`, `gpconverter.cpp:1143-1146` | — |
| Fret count, flats preference | — | **missing** | L6 | `gpconverter.cpp:1144-1163` | Only affects tablature rendering. |
| Track transpose | `Transpose { diatonic, chromatic }` (`part.rs:135-140`) | direct | L4 | `setTranspose`, `gpconverter.cpp:1176` | — |
| Percussion tracks / drumset | — (no drum map) | **missing** | L6 | `setUpTrack`, `gpconverter.cpp:1100-1136`; `calculateDrumPitch` `:2152-2170` | The pivot has no drumset/key-map (`format-mapping/midi.md:63-66`). |
| Chord diagrams (`Diagram`) | — | **missing** | L6 | `addFretDiagram`, `gpconverter.cpp` (called `:615`) | Same gap as MusicXML `<frame>` (`format-mapping/musicxml.md:59-61`). |
| Lyrics per beat | `Lyrics` (`lyrics.rs:36-63`) | conversion | L3 | `addLyrics`, `gpconverter.cpp` (called `:611`) | GP lyrics are per beat and per bar; GP6+ stores pre-arranged lyrics, GP5 needs the line parsed. |
| Bar clef | `Clef` (`segment.rs:377-382`), `ClefType` (`:250-302`) | direct | L1 | `addClef`, `gpconverter.cpp:1621-1685` | GP clefs are `G2/C3/C4/F4/Neutral`. |
| Simile mark | `MeasureRepeatMark` (`measure.rs:42-46`), `Measure::measure_repeat` (`:118`) | direct | L1 | `addSimileMark`, `gpconverter.cpp:1577-1620` | `%` one- or two-bar repeat. |
| Voice → track | `Track` (`ids.rs:118-145`, `VOICES = 4` `:31`) | conversion | L0 | `convertVoices`, `gpconverter.cpp:505-532` | GP voices per bar are unbounded in principle; >4 is a loss. |
| Beat rhythm, dots, tuplet | `DurationType` (`duration.rs:37-99`), `Duration.dots` (`:159-164`), `Tuplet` (`tuplet.rs:53-81`) | conversion | L0/L1 | `addChordRest`, `gpconverter.cpp:1700-1739`; `fillTuplet` `:295-326` | GP tuplet is `(num, denom)` on the rhythm; the pivot needs a container with span. |
| Beat absolute `time` | `Segment::tick` (`segment.rs:715-721`), `Measure::len` | conversion | L0/L1 | `convertBeat`, `gpconverter.cpp:545-648` | GP can place a beat at a bar position; gaps are filled with rests (`fixEmptyMeasures`, `:363-424`). |
| Chord (beat) | `Chord::notes` (`chord.rs:220-262`), `ChordRest` (`chord.rs:339-363`) | direct | L0 | `convertNotes`, `gpconverter.cpp:650-662` | A beat with no notes is a rest (`GPBeat::isRest`). |
| Note MIDI pitch | `Note::pitch`, `tpc` (`note.rs:266-293`) | conversion | L0 | `setPitch`, `gpconverter.cpp:2070-2124`; `setTpc` `:2125-2151` | GP writes both MIDI and `(string, fret)`; the pivot keeps the sounding pitch and `tpc`. |
| **String / fret** | — | **missing** | L6 | `note->string()`/`note->fret()` (`gpconverter.cpp:1270-1305`, `:2126-2151`) | The central tablature gap: `Note` has no `string`/`fret`. |
| Accidental | `Note::accidental` (`note.rs:232-246`, field `:296`), `AccidentalType` (`:201-231`) | direct | L0 | `setTpc`, `gpconverter.cpp:2125-2151` | GP accidental is −2…+2. |
| Tie | `Note::tie_forward/tie_back` (`note.rs:276-294`) | conversion | L2 | `addTie`, `gpconverter.cpp` (called `:698-701`) | GP `Tie @origin @destination` becomes a start/middle/end state; the pivot stores both ends. |
| Bend / pre-bend / release | `Annotation::TremoloBar` (`annotation.rs:104-105`), `TremoloBar { kind, points }` (`tremolo_bar.rs:77-99`) | conversion | L2/L6 | `addBend` (`gpconverter.cpp:1996-2069`), `GuitarBend` chains (`:1270-1305`) | The pivot can hold the **curve** (`TremoloBarPoint` = time %, pitch in cents) but only per segment/track: no string identity, no destination note for a legato bend. `GuitarBend` has no pivot type. |
| Harmonic (natural/artificial/pinch/tap/semi/feedback) | `NoteHeadGroup::Diamond` (`note.rs:138-200`), `ArticulationType::Harmonic` (`chord.rs:125-185`) | conversion | L2/L6 | `addHarmonic`, `gpconverter.cpp:1831-1881`; harmonic marks `:2376-...` | Head group + articulation approximate; `HarmonicMark` (line with subtype) has no pivot type. |
| Let ring | — (`SpannerType::Other("LetRing")` at best) | **missing** | L2 | `addLetRing`, `gpconverter.cpp:2331-2350` | Dedicated `LetRing` spanner absent. |
| Palm mute | — | **missing** | L2 | `addPalmMute`, `gpconverter.cpp:2339-2356` | Same. |
| Dead note | `Note::ghost` (`note.rs:306`) | direct | L0 | `note->setDeadNote`, `gpconverter.cpp:688` | The field doc says "note morte, guitare". |
| Ghost note (head) | `Note::ghost` | direct | L0 | `note->setGhost`, `gpconverter.cpp:687` | Same field as dead note in the pivot; GP distinguishes them. |
| Slide in/out/legato/shift | `SpannerType::Glissando` (`spanner.rs:268-276`), `Glissando` (`notation.rs:307-340`) | conversion | L2 | `addSlide`/`addSingleSlide`/`collectContinuousSlide`, `gpconverter.cpp:1925-1973` | Continuous slides are merged into a chain (`addContinuousSlideHammerOn`, `:1235-1392`). |
| Hammer-on / pull-off | — (no `HammerOnPullOff` type) | **missing** | L2 | `collectHammerOn`, `gpconverter.cpp:1989-1995`; built in `addContinuousSlideHammerOn` | Could be a `Slur` + text, but the pivot has no hammer-on object. |
| Pick scrape | — | **missing** | L2 | `addPickScrape`, `gpconverter.cpp:1974-1988` | — |
| Vibrato (left hand / whammy) | — | **missing** | L2 | `addVibratoLeftHand` (`:1821-1830`), `addVibratoWTremBar` | No vibrato type; `TremoloBar` is a bend curve, not a vibrato. |
| Trill | `SpannerType::Trill` (`spanner.rs:268-276`), `Trill` (`notation.rs:346-383`) | conversion | L2 | `addTrill`, `gpconverter.cpp:1779-1794` | Auxiliary fret is not stored; a trill line is. |
| Mordent / turn / inverted turn | `Ornament` (`notation.rs:158-183`) | direct | L2 | `addOrnament`, `gpconverter.cpp:1795-1820` | — |
| Accents, marcato, sforzato, staccato | `Articulation` (`chord.rs:187-205`), `ArticulationType` (`:125-185`) | direct | L2 | `addAccent`, `gpconverter.cpp:1882-1910` | Four accent bits map to accent/marcato/staccato. |
| Left/right fingering, string number | `Fingering` (`notation.rs:431-445`), `Note::fingerings` (`note.rs:351`) | direct | L3 | `addFingering` (`:1740-1764`), `addStringNumber` (`:1765-1778`) | String number is a circled fingering of style `STRING_NUMBER`; the string value itself is lost with the tablature gap. |
| Right/left hand tapping | — | **missing** | L2 | `addTapping`, `gpconverter.cpp:2458-2469` | MuseScore `Tapping` object absent. |
| Slap / pop | `ArticulationType::Other` (`chord.rs:184`) | conversion | L2 | `addSlapped`/`addPopped`, `gpconverter.cpp:2471-2491` | Text articulation (SLAP/POP) without a pivot enum value. |
| Arpeggio, brush | `ArpeggioType` (`notation.rs:229-259`), `Arpeggio` (`:244`) | direct | L2 | `addArpeggio`; `addBrush` `gpconverter.cpp:2493-2512` | Brush maps to a straight up/down arpeggio. |
| Tremolo (beat) | `Tremolo { kind, count }` (`notation.rs:200-227`) | conversion | L2 | `addTremolo`, `gpconverter.cpp` (called `:598`) | Numerator/denominator → count. |
| Grace notes (on/before beat) | `GraceType` (`chord.rs:87-107`), `GraceChord` (`chord.rs:268-281`) | conversion | L2 | `convertBeat`/`configureGraceChord`, `gpconverter.cpp:573-610`, `:702-725` | On-beat = appoggiatura, before-beat = acciaccatura. |
| Ottava (8va/8vb/15ma/15mb) | `SpannerType::Ottava` (`spanner.rs:268-276`) | conversion | L2 | `addOttava`, `gpconverter.cpp` (called `:626`) | — |
| Dynamic (PPP…FFF) | `Dynamic` (`annotation.rs:197-228`) | direct | L3 | `addDynamic`, `gpconverter.cpp:2157-...` | — |
| Hairpin (crescendo/decrescendo) | `SpannerType::Hairpin` (`spanner.rs:268-276`), `Hairpin` (`hairpin.rs:41-60`) | direct | L2 | `addHairPin`, `gpconverter.cpp` (called `:596`) | — |
| Fade in/out, volume swell | — | **missing** | L2 | `addFadding`, `gpconverter.cpp:2662-2693` | MuseScore uses a guitar-specific `SymId`; the pivot has no such symbol. |
| Wah (open/closed) | `ArticulationType::Other` | conversion | L2 | `addWah`, `gpconverter.cpp:2742-2763` | `Other("wah-open")` preserves the name; no pivot enum. |
| Golpe (finger/thumb) | `ArticulationType::Other` | conversion | L2 | `addGolpe`, `gpconverter.cpp:2765-2784` | Anchor above/below. |
| Rasgueado (18 patterns) | — | **missing** | L2 | `addRasgueado`, `gpconverter.cpp:2370-2374` | A line-only playing technique. |
| Pick stroke (up/down) | `ArticulationType::UpBow/DownBow` (`chord.rs:125-185`) | conversion | L2 | `addPickStroke`, `gpconverter.cpp:2694-2741` | MuseScore reuses `stringsUpBow`/`stringsDownBow`; the pivot's names are bowing-oriented but the symbols match. |
| Barre (fret, string) | — | **missing** | L6 | `addBarre`, `gpconverter.cpp` (called `:619`) | Tablature-only. |
| Timer, free text | `StaffText` (`annotation.rs:231-249`) | conversion | L3 | `addTimer`/`addFreeText`, `gpconverter.cpp` (called `:590-591`) | Timer is a countdown text. |
| Beat user stem orientation, beam mode | `Chord::stem` (`chord.rs:233`), `StemDirection` (`:72-85`), `ChordRest::beam_mode` (`:354-356`) | direct | L5 | `setBeamMode`, `gpconverter.cpp:577-583` | GP beam modes `JOINED/BROKEN` have no exact pivot value; `Begin/End` approximation. |
| Track lyrics offset / line count (percussion) | `Staff::lines` (`part.rs:361`) | conversion | L5 | `GPTrack::lyricsOffset/lineCount` | Only the line count matters for percussion staves. |
| Chord diagram reference (per beat) | — | **missing** | L6 | `addFretDiagram` | Same as diagrams above. |

## Gaps in the model, ranked

1. **String/fret and tuning (`L6::Tablature`).** `Note` has no
   `string`/`fret` and `Instrument` has no tuning; the sounding pitch is the
   only musical content the pivot can hold. This is the gap of
   `model-modularisation.md` §7.4, and it is the **primary** GP data.
2. **Capo, fret count, string count, barre.** All tablature metadata, all
   missing. Without capo, the fret numbers of a capoed score do not even
   correspond to the sounding pitch.
3. **`GuitarBend` spanner.** The pivot has `TremoloBar` (a per-segment curve),
   which can hold a bend **as geometry**, but not the bend's relation to a
   destination note. MuseScore's GP importer builds `GuitarBend` chains
   (`gpconverter.cpp:1270-1305`); a faithful port needs that object (L6) or an
   explicitly documented degradation to `TremoloBar`.
4. **Playing techniques**: let ring, palm mute, hammer-on/pull-off, pick
   scrape, tapping, rasgueado, fade in/out, vibrato. MuseScore has dedicated
   objects/lines; the pivot only has `SpannerType::Other(String)` or an
   articulation `Other(String)`.
5. **Drum map / percussion tracks.** GP percussion resolves element+variation
   to a drumset; the pivot has none (also a MIDI gap,
   `format-mapping/midi.md:63-66`).
6. **Chord diagrams.** No `FretDiagram` type (same as MuseScore/MusicXML,
   `format-mapping/musescore.md:46-48`).
7. **Free time.** No "no meter" flag; a nominal length must be fabricated.
8. **Vibrato / whammy bar.** `TremoloBar` is not a vibrato; there is no
   vibrato type at all.
9. **>4 voices per bar**, because `VOICES = 4` (`ids.rs:31`).

### Contradiction found in `model-modularisation.md` §7.4

§7.4 says `TremoloBar` "cannot express 'bend 2 semitones then release'". That
is **false**: `TremoloBarPoint { time, pitch }` (`tremolo_bar.rs:66-71`) is an
arbitrary polyline — `(0,0), (50,200), (100,0)` *is* "bend two semitones then
release", exactly what MuseScore's `TremoloBar` and the GP bend data describe.
The real gap is (a) attachment: `Annotation::TremoloBar` is attached to a
segment/track (`annotation.rs:104-105`), not to a note/string, so two strings
of one chord cannot bend differently; and (b) no destination-note link, so a
legato bend (MuseScore `GuitarBend`) is not representable. §7.4 has been
corrected accordingly; the `L6::Tablature` proposal (`string`, `fret`,
`bend: Vec<BendPoint>`, `techniques`) remains the right shape.

## Oracle and reference candidates

- **MuseScore 4.7.4 CLI** (`mscore`, installed): `mscore -o out.musicxml
  in.gp5` (and `.gp4`, `.gp3`, `.gpx`, `.gp`), plus `.mscx` output when the
  MusicXML comparison is not enough (tuning, tablature and string data do not
  appear in MusicXML).
- **The ≈250 fixture inputs** in
  `ref-musescore/.474/src/importexport/guitarpro/tests/data/`, each with a
  `*-ref.mscx`: bend, basic-bend, prebend, let-ring, palm-mute, brush,
  arpeggio, tremolo, ottava1–5, volta, repeats, tuplets, keysig, clef,
  high-pitch, ghost-note, dead-note, percussion, timer, directions,
  barre, harmonics, `guitarbendimporter_data/*` (bend splitting), etc.
  Licence **GPL-3.0-only**; out-of-repository oracle, same status as the
  existing corpus.
- No independent open reader to cross-check (TuxGuitar is a separate GPL
  implementation but not local; do not fetch without recording the licence).
  MuseScore is the compliance target anyway.

## Corpus candidates

1. The ≈250 MuseScore fixture inputs (no download, GPL-3, exact 4.7.4 behaviour):
   enough to cover every effect in the tables above.
2. The repository's own `tests/corpus/*.mscx` exported by `mscore` to GP? —
   **not possible**: MuseScore has no GP writer. GP is import-only by design.
3. Real-world `.gp` files: licences and personal data unknown; keep them
   outside the repository and record provenance if used.

## Suggested task breakdown

Numbers continue after `t43`; MEI = t44, Capella = t45, Guitar Pro = t46.

- **t46.1 — crate skeleton.** `scoretk-guitarpro-{common,import}`,
  `guitarpro-import` facade feature, content sniffing (`ptab`/`PK`/`BCFZ`/`FIC`).
- **t46.2 — model + GP3/4/5 binary reader.** Port the `gtp/*.h` model
  (private to the crate) and `importgtp-gp{3,4,5}.cpp`; verify that the same
  fixture in `.gp3/.gp4/.gp5` converges.
- **t46.3 — GP6/GP7 reader.** BCFS decompression (GP6) and ZIP +
  `Content/score.gpif` (GP7), then `gp67dombuilder.cpp`.
- **t46.4 — conversion L0/L1.** Tracks/parts/staves, measures, voices, beats,
  notes, durations/tuplets, clefs/keys/meters, repeats/endings, ties.
- **t46.5 — conversion L2/L3.** Articulations, ornaments, slides, harmonics,
  let ring/palm mute (as `Other` until t46.7), dynamics, hairpins, ottavas,
  lyrics, chord symbols/diagrams (reported).
- **t46.6 — model additions (L6).** `Tablature { string, fret, tuning, capo,
  bend: Vec<BendPoint>, techniques }` and `GuitarBend`; keep `Note::pitch`
  computed from tuning+fret so the ten writers keep working.
- **t46.7 — measurement.** `mscore -o out.musicxml` + `.mscx` comparison over
  the fixtures, musical-core level first; two workers maximum.
- **t46.8 — report.** `docs/agents/guitarpro.md` with per-effect figures and
  the list of `Loss` records.

## Risks

- **Tablature first.** If t46.4 ships without the L6 tablature layer, the
  reader will "succeed" while dropping the primary data (string, fret,
  tuning, capo). Measure with both MusicXML and `.mscx`: MusicXML alone hides
  the loss.
- **String numbering sign** (`strings - 1 - gpString`) and **tuning order**
  (lowest first) are two independent conventions; a unit test with a
  non-symmetric tuning (e.g. drop D) is necessary.
- **Effect bit masks differ by version**; one shared mask table will silently
  misread GP3/4/5.
- **Bend conventions** (GP3–5 `0–60/100-per-semitone` vs GP6/7 percentages)
  must be normalised at parse time, not at conversion time.
- **Grace notes** are synthesised; a reader that also reads them as beats will
  double them.
- **No published spec**: every claim must cite the C++ or a fixture, as this
  table does.
