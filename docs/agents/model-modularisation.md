# Pivot model: layer audit and modularisation proposal

Status: **migration in progress — steps 0 through 6 executed** (§10). The GABC reader/writer is implemented and measured (§10.7, §11.7). Audience: whoever
adds GABC, MEI, Capella, Guitar Pro or Humdrum after the ten current directions
are stable (`docs/TODO.md:29-46`). The question answered here is not "is the
model wrong?" but "which parts of it are a genuine common denominator, and
which are one format's data that every other format is currently forced to
carry or silently drop?".

## 1. Method, evidence and limits

1. **Type inventory.** All `pub struct`/`pub enum` declarations in
   `crates/scoretk-core/src/model/*.rs` were enumerated (120 public types across
   26 files; `crates/scoretk-core/src/model/mod.rs:99-145` re-exports them). The
   field list was extracted from the same files, not from documentation.
2. **Usage matrix.** For each type name and each of the ~70 most-discriminating
   field names, occurrences were counted in `src/**/*.rs` of the ten direction
   crates (`crates/scoretk-{musescore,musicxml,midi,lilypond,abc}-{import,export}`).
   A mark in the matrix means "the symbol is referenced by that crate's
   source", not "the mapping is faithful"; the **faithfulness** column cross-checks
   `docs/agents/format-mapping/{musescore,musicxml,midi,lilypond,abc}.md`.
   Type-name counts undercount fields reached through a parent struct, so every
   group was also checked by field access (§4).
3. **Cross-check.** The five format-mapping documents were read in full; their
   "Model gaps" sections are the authoritative statement of what the pivot
   cannot express today (`docs/agents/format-mapping/musescore.md:39-58`,
   `musicxml.md:50-64`, `midi.md:56-71`, `lilypond.md:42-63`, `abc.md:35-55`).
4. **References first.** At audit time (2026-11-22) the five *coming* formats
   had no local specification: `docs/TODO.md:33-46` named the sources but did
   not vendor them. **This has since been corrected (2026-11-23)**; the sources
   are now local, read-only and unversioned:
   - GABC: Gregorio commit `b63f074` (`doc/Gabc.tex:21-742`, grammar in
     `src/gabc/gabc-score-determination.y`, structures in `src/struct.h`,
     clef keys in `src/struct.c:1273-1288`, dump in `src/dump/dump.c`);
   - MEI: Guidelines/schema **5.1** (`mei/cmn.html`, `mei/mei-all-5.1.rng`,
     ECL-2.0 for the schema; the Guidelines' licence is not stated on the
     retrieved pages);
   - Humdrum: humlib `b9d67cfe` (BSD-2-Clause), Verovio `bfbcec3`
     (`src/iohumdrum.cpp`, LGPL-3.0) and the `**kern` reference at
     `humdrum/kern-reference.html` (licence not stated on the page);
   - Capella and Guitar Pro: **no public format specification exists**, but the
     4.7.4 MuseScore C++ modules are now available at
     `ref-musescore/.474/src/importexport/{capella,guitarpro}` (tag `v4.7.4`,
     `7688c005`, see `docs/agents/upstream.md` §2.1).
   Paths, commits and licences are recorded in `specs/README.md` (outside the
   repository). Sections 7.1, 7.2 and 7.5 were re-checked against the new
   sources; §7.3/§7.4 now cite the 4.7.4 C++. No script from any downloaded
   source was executed.

## 2. What is already a real common core

Four facts hold across all ten directions and should not be disturbed:

- **Exact time.** `Fraction` is the tick/fraction type (`fraction.rs:41`), and
  `Score::division` is the only linear resolution (`score.rs:129`); conversions
  are `Score::tick_to_divisions`/`divisions_to_tick` (`score.rs:343,352`). Every
  reader and writer references `Fraction` (counts: 110/19/145/555/770/30/227/97/
  118/16 in musescore-i/e, musicxml-i/e, midi-i/e, lilypond-i/e, abc-i/e).
- **Arena + typed ids.** `Score` owns `notes`, `tuplets`, `spanners`, `parts`,
  `staves`, `measures` (`score.rs:123-166`); typed ids are in `ids.rs:123` and
  re-exported at `mod.rs:115`. No parent pointers, `Send + Sync`, no I/O
  (`mod.rs:22-26`). This is the invariant that makes portable wasm possible.
- **Tracks = staff × voice.** `Track(staff * VOICES + voice)`, `VOICES = 4`
  (`mod.rs:31-35`, `ids.rs:123`); non-rhythmic items occupy voice 0
  (`mod.rs:34-35`). This is a *MuseScore* addressing convention that all ten
  directions have adopted, so it is de facto common.
- **Event spine.** `Measure` (`measure.rs:53-131`) → `Segment` (`segment.rs:715-721`)
  → `ChordRest` (`chord.rs:339-363`) → `Note` (`note.rs:266-351`), with
  `Duration` = figure + dots (`duration.rs:159-163`) and `ChordRest::ticks` for
  stretched real duration (`chord.rs:348`). All ten directions use these.

Those four are the *minimal common core*. Everything else is a candidate layer.

## 3. Usage matrix (ten directions)

Legend: **R** = read by that direction's import crate, **W** = written by its
export crate, **r/W** = partial or asymmetric, **·** = no reference in source.
Columns: **Mi/Me** MuseScore, **Xi/Xe** MusicXML, **Di/De** MIDI, **Li/Le**
LilyPond, **Ai/Ae** ABC (i = import, e = export).

| # | Model group (declaration) | Mi | Me | Xi | Xe | Di | De | Li | Le | Ai | Ae | Cross-check / verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `Score`, `Fraction`, `Tick`, ids, `Segment`, `Track` (`score.rs:123`, `fraction.rs:41`, `ids.rs:123`, `segment.rs:715`) | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | **CORE** (§2). |
| 2 | `Measure.len`, `nominal_len`, `start_tick`, numbering (`measure.rs:56-78`) | R/W | R/W | R/W | R/W | R | W | R/W | R/W | R/W | R/W | Core + metric; `nominal_len` absent from MIDI export (0/1). |
| 3 | `Note.pitch`, `tpc`, `tpc_transposed`, `Accidental*` (`note.rs:268-296`, `:232-245`) | R/W | R/W | R/W | R/W | R | W | R/W | R/W | R/W | R/W | CORE pitch; `tpc` not read by MusicXML/MIDI/ABC import (0/0/0). |
| 4 | `KeySig`, `Key`, `KeyMode`, `Clef`, `ClefType` (`segment.rs:118-406`) | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | Core for written pitch; `ClefSign` is never named outside core (`segment.rs:214`, only `model/tests.rs:208`). |
| 5 | `Duration`, `DurationType`, `ChordRest.ticks` (`duration.rs:37,159`, `chord.rs:341-348`) | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | CORE rhythm. |
| 6 | `Tuplet`, `tuplet` field, `time-modification` (`tuplet.rs:53-84`, `chord.rs:350`) | R/W | R/W | R/W | R/W | R | W | R/W | R/W | R/W | R/W | Metric layer; ABC nested tuplets non-standard (`format-mapping/abc.md:53`). |
| 7 | `BeamMode`, `beam_direction`, `StemDirection` (`chord.rs:45-85,354-361`) | R/W | R/W | R/W | R/W | · | · | · | · | · | · | **MuseScore/MusicXML-only engraving**: 0 references in MIDI, LilyPond and ABC. |
| 8 | `Chord`, `Rest`, `GraceChord`, `GraceType` (`chord.rs:220-354`) | R/W | R/W | R/W | R/W | R | W | R/W | R/W | R/W | R/W | CORE events (+ grace); MIDI has no grace on import (`format-mapping/midi.md:79`). |
| 9 | `Part`, `Staff`, `StaffBracket`, `Transpose` (`part.rs:246-391`) | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | Core structure; `Transpose` has no standard ABC spelling (`format-mapping/abc.md:43-45`). |
| 10 | `Instrument`, `MidiArticulation`, `InstrumentChannel` (`part.rs:42-133`, `instrument_channel.rs:35`) | R/W | R/W | R/W | R/W | R/W | R/W | r/W | · | r | · | Playback layer; `InstrumentChannel` never named in ABC/LilyPond, `MidiArticulation` only in MuseScore import (`part.rs:42`). |
| 11 | `Articulation*`, `Ornament*`, `Fermata`, `Tremolo`, `Arpeggio`, `Glissando`, `Trill`, `Slur`, `Fingering`, `ChordLineType` (`chord.rs:110-210`, `notation.rs:48-445`) | R/W | R/W | R/W | R/W | · | r | R/W | R/W | R/W | r | Expressive layer; MIDI reads almost none, writes velocity effects (`format-mapping/midi.md:31,33`). |
| 12 | `Lyrics`, `LyricsSyllabic` (`lyrics.rs:36-63`) | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | R/W | Text layer, genuinely universal; MIDI only RP-017 (`format-mapping/midi.md:26`). |
| 13 | `Annotation`, `Dynamic`, `StaffText`, `SystemText`, `Tempo*`, `RehearsalMark`, `InstrumentChange`, `Swing*` (`annotation.rs:49-487`) | R/W | R/W | R/W | R/W | · | W | R/W | R/W | R/W | R/W | Text+playback mix; `SwingParameters`/`TempoKind`/`RehearsalFrame` are MuseScore/MusicXML only (0 elsewhere). |
| 14 | `Harmony`, `HarmonyDegree*`, `FiguredBass*` (`annotation.rs:428-479`, `figured_bass.rs:35-165`) | R/W | R/W | R/W | R/W | · | · | · | r | R/W | r | Analysis layer; no MIDI, LilyPond reader ignores `\figuremode` (`format-mapping/lilypond.md:46-48`). |
| 15 | `Spanner`, `SpannerType`, `Hairpin`, `PedalInfo`, `TextLineInfo`, `LineFont`, `HookType` (`spanner.rs:39-322`, `hairpin.rs:41`) | R/W | R/W | R/W | R/W | · | r | R/W | R/W | R/W | r | Expressive/layout; MIDI only derives hairpin velocity (`format-mapping/midi.md:69`). |
| 16 | `Marker`, `Jump`, `RepeatMark`, `VoltaInfo`, `Measure.repeat_count`, `measure_repeat`, `mmrests` (`repeat.rs:42-286`, `measure.rs:82-95`) | R/W | R/W | R/W | R/W | · | W | R/W | R/W | R/W | R/W | Metric/playback; `RepeatMark`/`Marker` are never written by MIDI as meta-events (`format-mapping/midi.md:68`). |
| 17 | Note `play`, `unperformed`, `velocity`, `user_velocity`, `velo_type`, `ghost` (`note.rs:306-351`) | r | W | r | W | · | R/W | R | · | · | · | **PLAYBACK**: `.play` is read by MuseScore import and LilyPond import, written by MusicXML/MIDI export; `unperformed` only MusicXML export (3) and LilyPond import (4); `VeloType` only MIDI export (`note.rs:125`). |
| 18 | `PlaybackTransposition`, `Score.playback_transposition/articulate/unfold_repeats` (`playback.rs:40`, `score.rs:144-160`) | · | · | · | · | · | W | R/W | · | · | · | **PLAYBACK-ONLY**: LilyPond detects it from a MIDI-only `\score`, MIDI export applies it to Note On/Off (`format-mapping/midi.md:44`). |
| 19 | `StyleSettings` (22 fields: `style.rs:48-111`), `Credit`, `CreditStyle`, `TboxText`, `RichText`, `TextFormat` (`text.rs:64-610`) | R/W | R/W | R/W | R/W | · | · | · | r | · | · | **LAYOUT/STYLE**: no reference in MIDI/LilyPond/ABC import and none at all in MIDI/ABC export. `spatium_mm`/`page_*` only MuseScore + MusicXML. |
| 20 | `TremoloBar*` (`tremolo_bar.rs:31-84`), `ArticulationAnchor` (`chord.rs:110`), `FiguredBassContLine/Modifier` (`figured_bass.rs:35,95`) | R/W | r/W | · | r | · | · | · | · | · | · | **MuseScore-only engraving** by symbol count; `TremoloBar` has 15/5 references in MuseScore i/e and 1 in ABC export, none elsewhere. |
| 21 | `Score::metadata`/`ScoreMetadata` incl. `msc_version` (`score.rs:51-91,125`) | R/W | R/W | R/W | R/W | R | r | R/W | r | r | r | Metadata is common, but `msc_version` is MuseScore-specific; `ScoreMetadata` is never named in MuseScore/MusicXML import because they use `score.metadata` directly. |
| 22 | `SegmentAnnotation`, `Measure.time_sig_denominator`, `MultiMeasureRests`, `MidiArticulation` (`segment.rs:679`, `measure.rs:73`, `mmrest.rs:32`, `part.rs:42`) | r | r | r | r | r | r | r | r | r | r | See §4: one unused field, one core-internal type, two undercounted ones. |

### 3.1 How to read the marks

- The marks are deliberately not "support = yes". For example `Tempo` is R/W in
  MIDI (meta `FF 51`) but `TempoKind` has 0 references in every MIDI crate:
  the field exists for MuseScore/MusicXML semantics only.
- `Note::tpc` has 0 occurrences in MusicXML, MIDI and ABC *import* but is
  written by their exports (6/4/…): the concert-pitch class is reconstructed
  from `step/alter` on import, not stored from the file (see
  `format-mapping/musicxml.md:26`).
- `Hairpin` is written by MIDI export only to derive velocity
  (`format-mapping/midi.md:69`); the *object* is never read back from SMF.

## 4. Per-field anomalies and flags

### 4.1 Genuinely unused

`Measure::time_sig_denominator` (`measure.rs:73`) is initialised from
`nominal_len.denominator()` (`measure.rs:144`) and set by `set_time_sig`
(`measure.rs:173`), asserted only in `model/tests.rs:318`; no direction crate
references it. Its sibling `time_sig_numerator` is read by MIDI import
(`crates/scoretk-midi-import/src/lib.rs:1134`). This is dead weight in the
common core and belongs in the metric layer (or should be removed with the
numerator if MIDI import can use `nominal_len` instead).

### 4.2 Core-internal types never named outside the model

`ClefSign` (`segment.rs:214`) is used only by `ClefType::sign()`
(`segment.rs:306-319`) and one core test (`model/tests.rs:208`). It is an
implementation detail of the core, not a format contract. `MultiMeasureRests`
(`mmrest.rs:32`) is likewise never named by a direction crate, but its field
`Measure::mmrests` **is** used (MuseScore import 4, MusicXML export 1): the
type counts need the field-level cross-check.

### 4.3 MuseScore/layout-only data carried by every `Score`

`StyleSettings` has 22 fields (`style.rs:48-111`); of those,
`create_multi_measure_rests` is referenced by MuseScore import (7) and export
(1) only, `small_staff_mag` by MuseScore import and MusicXML export,
`spatium_mm` only by the two MuseScore sides, `page_width_in`/`page_height_in`
only by MuseScore i/e, `concert_pitch` only by MusicXML export (13) and MuseScore
export (1). `hide_empty_staves` is read by MusicXML import (1) and MuseScore
export (1). None of these is read by MIDI, LilyPond or ABC. The orphan
`<staff-details print-object="no">` blocks that dominate the MusicXML
full-parity diff are exactly this: MuseScore system layout written into the
model and re-emitted (see `musicxml_core.rs:160-169`,
`docs/agents/musicxml-export.md:1094-1099`).

### 4.4 Playback-only data

`Score::playback_unfold_repeats` is referenced 14 times by LilyPond import and
nowhere else (`score.rs:160`); `playback_transposition` is referenced by MIDI
export (1) and LilyPond import (3) (`score.rs:144`). `Note::unperformed` was
added for LilyPond cluster/NullVoice music and is only meaningful to MusicXML
export and LilyPond import (`note.rs:322`, `docs/agents/musicxml-export.md`
§39). `VeloType` is referenced only by MIDI export (`note.rs:125`).

### 4.5 No diagnostics channel today

`scoretk-core/src/error.rs:20-33` has only `Error::Message`; there is no
`Warning`/report type, and no `println!`/`eprintln!` in any direction crate's
`src/`. The only precedent is the **crate-private** LilyPond collector
`Warnings` (`crates/scoretk-lilypond-import/src/warnings.rs:48-72`), used for
`\include` failures. A cross-format loss report therefore has to be an additive
API (§6.3).

## 5. Proposal: common core + optional layers

### 5.1 Layer definitions

**L0 — Common core (always present).** `Fraction`/`Tick`, typed ids and
`Track`, `Score` arenas (`notes`, `tuplets`, `spanners`), `Part`/`Staff`
identity, `Measure.start_tick/len`, `Segment.tick/kind`, `ChordRest.duration/
ticks/kind`, `Note.pitch/tpc`, `Duration`. Citation: §2. This is exactly what
all ten directions reference.

**L1 — Metric structure.** `Measure` numbering/irregularity, `nominal_len`,
`time_sig_numerator`, `BarLine`, `TimeSig`, `KeySig`, `Clef`, `Tuplet`,
`Measure::repeat_count`/`measure_repeat`/`mmrests`, `Marker`, `Jump`,
`VoltaInfo` (`measure.rs:56-95`, `segment.rs:118-486`, `repeat.rs:109-286`).
Needed by MuseScore, MusicXML, MIDI (through its time map), LilyPond and ABC;
GABC and Humdrum `**kern` can leave it empty.

**L2 — Events and expressions.** Grace chords, articulations, ornaments,
tremolo, arpeggio, slurs, glissandi, hairpins, pedal, text lines
(`chord.rs:264-270`, `notation.rs:48-404`, `spanner.rs:39-322`).
Split from L1 because a plain MIDI or `**kern` stream has neither `Spanner`
objects nor an engraving beam mode.

**L3 — Text and analysis.** `Lyrics`, `RichText`, `Credit`, `TboxText`,
`Annotation` variants, `Harmony`, `FiguredBass`
(`lyrics.rs:36-63`, `text.rs:64-610`, `annotation.rs:80-479`,
`figured_bass.rs:118-165`). MusicXML/LilyPond/ABC read and write most of it;
MIDI only lyrics and dynamic text; GABC needs the syllable/text part but not
chord symbols.

**L4 — Playback.** `Instrument` MIDI fields, `InstrumentChannel`,
`MidiArticulation`, note `velocity`/`user_velocity`/`velo_type`/`play`/
`unperformed`, `PlaybackTransposition`, `SwingParameters`,
`Score::playback_*`, `Tempo`, `Dynamic` velocity
(`part.rs:42-133`, `instrument_channel.rs:35`, `note.rs:306-351`,
`playback.rs:40`, `annotation.rs:197-277`, `score.rs:144-160`).
MuseScore and MusicXML carry the source data; MIDI is the main consumer.

**L5 — Engraving/layout.** `StyleSettings`, `Credit` positions/fonts,
`BeamMode`, `StemDirection`, `ArticulationAnchor`, `NoteHeadType/Group`,
`TremoloBar`, `Measure::line_break`, `visible`/`placement` flags,
`TextLineInfo`, `LineFont`, `HookType`
(`style.rs:48-111`, `chord.rs:45-85,110-120,354-361`, `tremolo_bar.rs:31-84`).
This is where MuseScore's 4.7.4-only data lives; it is *not* musical core and
`musicxml_core.rs:27-99` already drops its observable effects on comparison.

**L6 — Format extensions.** Per-format payloads that no other direction can
interpret: `ScoreMetadata::msc_version`, `RehearsalFrame`, `TempoKind`,
`SwingUnit`, `FiguredBassContLine`, MuseScore `<FretDiagram>`/`<Image>` (not
modelled today, `format-mapping/musescore.md:46-48`), and the new formats'
neume/tablature/Humdrum-spine data (§7).

### 5.2 Rust mapping

1. **Do not use Cargo features for the layers.** The facade already has ten
   symmetric features (`crates/scoretk/Cargo.toml:12-35`) and the wasm package
   deliberately enables **all ten** (`crates/scoretk-wasm/Cargo.toml:18-30`).
   Layering inside `Score` by `#[cfg]` would give different `Score` layouts to
   different builds, break `convertWithTimeline` and the stable wasm contract
   (AGENTS.md invariant 5), and gain nothing: wasm ships everything anyway
   (`docs/agents/wasm.md:44-58`).
2. **Use optional sub-structs behind `Deref`.** Introduce
   `Score { core: CoreScore, metric: Option<Metric>, text: Option<Text>,
   playback: Option<Playback>, engraving: Option<Engraving>,
   extensions: Extensions }` and implement `Deref`/`DerefMut<Target = CoreScore>`
   so existing `score.notes`, `score.measures`, `score.staves` keep compiling.
   There is only **one** `Score { … }` struct literal in the whole workspace
   (`model/score.rs:185-200`, inside `with_division`), so the constructor change
   is contained; the 53 textual `Score {` hits are almost all type annotations
   in tests. Each layer is `Option<Box<…>>` for the large ones (`Engraving`,
   `Extensions`) to keep `Score` small; `Metric` and `Text` are embedded
   because virtually every score has them.
3. **`Extensions` is a typed, closed enum of `Box`-ed per-format structs**, not
   a `HashMap<String, String>`: the latter would silently accept misspelled
   keys and add string code to wasm. Use `#[non_exhaustive]` on the enum so a
   new format does not break downstream matches. Reuse the existing typed-id
   style (`ids.rs:123`) for cross-layer references.
4. **Naming discipline:** layer types get a `scoretk-core` module
   (`model/layers/{metric,text,playback,engraving,extensions}.rs`) and are
   re-exported from `mod.rs:99-145` exactly as today. No new dependency: the
   core stays I/O-free, thread-free and `wasm32-unknown-unknown`-clean
   (`mod.rs:22-26`, AGENTS.md invariant 1).
5. **Wasm cost.** The shipped binary is 1 924 141 raw bytes after `wasm-opt -Oz`
   (`docs/agents/wasm.md:44-58`) against Cantus's ≈ 2.5 Mo ceiling. Adding
   *empty* `Option` layers costs a discriminant and a few branches; the risk is
   **code size from new format layers**, not layout. Therefore no new format's
   layer is compiled into the wasm package until that format's direction ships;
   the enum variant can exist as a zero-sized marker in the meantime. Keep the
   external API byte-for-byte identical (`api.rs:31-222`, AGENTS.md invariant 5):
   the report of §6.3 is exposed only through *new, additive* functions and a
   new `console`-free JS object, never by changing `convert`'s return type.

### 5.3 Layer capability declaration

Each direction crate declares what it consumes/produces, e.g.

```rust
pub struct Capabilities { pub reads: Layers, pub writes: Layers }
```

with `Layers` a bitflags-like `u8` defined in the core. Today's ten directions
would declare exactly the matrix of §3 (L0+L1 for MIDI import, L0+L1+L4+L5 for
MusicXML export, …). This is what makes a loss *measurable*: a writer can say
"the input carries L5, I write L0+L1+L3, here is the L5 inventory I dropped".

## 6. From silent loss to measurable warning

### 6.1 Invariant

Reading a format must never discard a layer the core can hold; writing a format
must never invent content for a layer it does not implement. When a conversion
drops something, it must append a structured `Loss` record
(`layer`, `format`, `item`, optional tick/track) to a report.

### 6.2 Core type

```rust
pub struct Loss { pub layer: Layers, pub item: Cow<'static, str>,
                  pub tick: Option<Fraction>, pub track: Option<Track> }
pub struct ConversionReport { pub losses: Vec<Loss> }
```

Stored in `Score::report` (L0, empty by default) for the read side; returned by
the write side through the additive API below. This generalizes the existing
LilyPond-only `Warnings` (`warnings.rs:48-72`) and makes ABC's "ignored overlay"
and MuseScore's "no field for `<FretDiagram>`" (`format-mapping/abc.md:32`,
`format-mapping/musescore.md:46-48`) countable per file, exactly like
`core.tsv` counts changed lines today.

### 6.3 API, without touching invariant 5

Add `scoretk::read_with_report` / `write_with_report` and
`scoretk::convert_with_report` next to the existing `read`/`write`
(`scoretk/src/lib.rs:146,192`); the old signatures delegate and ignore the
report. On wasm, add an **additive** `convertDetailed(...)` returning
`{ bytes, losses }`; `convert`, `detect`, `formats`, `version` and
`convertWithTimeline` keep their exact signatures and return shapes
(`api.rs:98-222`, AGENTS.md invariant 5).

## 7. Needs of the five coming formats

### 7.1 GABC (first, import and export)

- **Source evidence (now local, §1.4):** `docs/TODO.md:33-35` designates
  Gregorio as the syntax reference, and its parser is the only reference of
  truth (`docs/agents/gabc.md` §4). The normative source is
  `gabc/gregorio/doc/Gabc.tex:21-742`, with the grammar in
  `src/gabc/gabc-score-determination.{y,l}` and clef/key rules in
  `src/struct.c:1273-1288`:
  file structure `:36`, headers `:67` (`office-part` at `:141`), note syntax
  `:403` (pitch letters `a`–`n`/`p`), one-note neumes `:452`, complex neumes
  `:556`, neume fusion `:603`, rhythmic signs `:742` (punctum mora `.`, ictus
  `'`). The grammar and lexer are
  `gabc/gregorio/src/gabc/gabc-score-determination.{y,l}`. Real input
  `/home/jacques/Musique/Gregorien/gabc-chants/DeusInAdjutorium.gabc:1-22`
  shows the same header fields, `%%`, and syllables such as `DE(c3h)us(h'_)`
  with separators `(,)`, `(;)`, `(:)`, `(::)` and rubrics `<sp>R/</sp>`.
- **Missing today:** no neume/punctum/syllable type anywhere in the model
  (`mod.rs:99-145` has none); `Score::metadata` has no `office-part`/`occasion`/
  `annotation` (`score.rs:51-91`); no notion of a text-less, measure-less
  timeline — `Score::measures` is mandatory for every current writer
  (`score.rs:135`, `format-mapping/abc.md:39-42` shows the same problem for
  local key/metre).
- **Layer:** a new `L6::Gregorian` holding `Syllable { text, pitch_letter,
  octave, neume_signs, separator, rubric }`. It **owns** the primary data; the
  modern-notation view (one `Note` per neume, a default duration, a nominal
  4/4 `Measure`) is a *derived projection into L0+L1*, flagged as derived in
  the report. A GABC→MusicXML→GABC round trip cannot be exact, and the report
  makes that explicit instead of silently losing `'_`/`~`. Export writes the
  structured L6 payload; no raw source is kept in the model.
- **Implemented (step 6):** `model/gregorian.rs` owns `Gregorian`,
  `Syllable`, `GregorianNote`, `NeumeSign`, `BarKind` and `GregorianHeader`;
  the derived projection flags itself with `derived_modern_view = true` and a
  `derived-modern-view` loss record. The export serialises every token from those fields
  (three atomic-token exceptions are justified in §8 of the GABC report);
  for a score without L6 it projects concert pitch, lyrics and bars instead of
   refusing. Details and measurements: `docs/agents/gabc.md`.

### 7.2 MEI (import and export)

- **Source evidence (now local, §1.4):** `docs/TODO.md:36` ("port MuseScore
  4.7.4's MEI module"); the module is now at
  `ref-musescore/.474/src/importexport/mei/internal/`, with beam mapping at
  `meiconverter.cpp:626-667` and `@tstamp` ↔ `Fraction` at `:3828-3853`.
  The format specification is MEI **5.1**: `mei/introduction.html`,
  `mei/cmn.html` and `mei/mei-all-5.1.rng`. MEI's `<scoreDef>/<staffDef>`,
  `<measure>`, `<staff>/<layer>` with `@tstamp`/`@dur`, `<note>` with `@pname/@octave/@dur/@dots`, `<beam>`,
  `<tuplet>`, `<slur>`, `<verse>`, `<harm>`, `<anl>` map to L1–L3 and are
  already representable (`segment.rs:118-486`, `chord.rs:339-363`,
  `lyrics.rs:36-63`, `annotation.rs:428-479`).
- **Missing today:** MEI's separation of **written** and **gestural/perform**
  time (`@tstamp.ges`, `@dur.ges` in `mei/cmn.html`, `@ppq` in
  `mei/mei-all-5.1.rng`) has no analogue: `ChordRest` has only
  `duration` + `ticks` (`chord.rs:341-348`); `@copyof`/excerpt links have no
  model counterpart (`format-mapping/musescore.md:43-45`); `@facs` facsimile
  references and `<timeline>`/`<expansion>` are not representable.
- **Layer:** L1 gains a `GesturalTime` sub-record (optional, per element);
  L6 adds `ExternalRef` (`facs`, `copyof`) and `MeiExpansion`.

### 7.3 Capella (import only)

- **Source evidence:** `docs/TODO.md:37`; no public format specification
  exists, but the 4.7.4 importer is now local at
  `ref-musescore/.474/src/importexport/capella/internal/`. It defines its own
  beam mode (`capella.h:55`) and resolves systems/staff layouts
  (`capxml.cpp:936-990`), confirming that Capella carries rich layout in a
  proprietary binary alongside dynamics, lyrics and instrument definitions.
- **Missing today:** the pivot has no per-element coordinates at all — the
  first gap of the MusicXML mapping and the third of the MuseScore mapping
  (`format-mapping/musicxml.md:54-55`, `format-mapping/musescore.md:52-56`).
- **Layer:** parse musical content into L0–L4, put every page/system position
  into L5 `Engraving` as *opaque* per-object offsets, and report the rest as
  `Loss`. Import-only means no round-trip obligation, but the report must say
  what was dropped.

### 7.4 Guitar Pro (import only)

- **Source evidence:** `docs/TODO.md:38`; no public format specification
  exists, but the 4.7.4 importer is now local at
  `ref-musescore/.474/src/importexport/guitarpro/`. It includes
  `engraving/dom/{bend,fret,guitarbend}.h`, builds MuseScore `StringData`
  (`internal/gtp/gpconverter.cpp:1152-1167`) and reads `note->string()`/
  `note->fret()` plus `GuitarBend` chains (`:1280-1304`), confirming that
  tablature (string/fret pairs, bend curves, slides, hammer-ons, palm mute,
  let-ring, stroke directions, per-track effects) is the primary data.
- **Missing today:** `ClefSign` has a `Tab` variant and `ClefType` the tab
  variants (`segment.rs:214-302`), and `Staff::lines` exists (`part.rs:361`),
  but there is **no string/fret field** on `Note` (`note.rs:266-351`).
  `TremoloBar` (`tremolo_bar.rs:66-99`) can hold the **curve** — its
  `(time, pitch)` points express "bend 2 semitones then release" exactly — but
  it is attached to a segment/track (`annotation.rs:104-105`), so it has no
  string identity and no destination note; MuseScore's importer builds
  `GuitarBend` chains instead (`gpconverter.cpp:1270-1305`), and the pivot has
  no `GuitarBend`. It is not a MIDI bend stream either
  (`format-mapping/midi.md:60-62`). MusicXML mapping lists extended techniques
  as a model gap (`format-mapping/musicxml.md:57-58`).
- **Layer:** new `L6::Tablature { string, fret, bend: Vec<BendPoint>,
  techniques }`, plus a MIDI bend sink in L4 for playback. Concert pitch is
  computed from string tuning + fret and stored in L0 `Note::pitch`, so all ten
  existing writers keep working without knowing about tablature.

### 7.5 Humdrum `**kern` (last)

- **Source evidence (now local, §1.4):** `docs/TODO.md:39-41` names humlib
  (BSD-2) and Verovio's `iohumdrum.cpp` (LGPL-3) with a licence check. Both
  are present (`humdrum/humlib` at `b9d67cfe`, `humdrum/verovio` at `bfbcec3`
  with `src/iohumdrum.cpp`), plus the reference
  `humdrum/kern-reference.html` from <https://www.humdrum.org/rep/kern.html>.
- **`**kern` necessities:** confirmed by `humdrum/kern-reference.html`
  §2.2 (interpretations), §4.1 (spine manipulators `*+`, `*^`, `*v`, `*-`,
  `*x`), §3.2.7 (duration), §3.2.8 (n-tuplets), §3.4 (barlines), §2.4
  (comments) and §3.2.3-3.2.6 (ornaments/articulation/stems): parallel spines
  with *exclusive interpretations* (`**kern`, `**bar`, `**harm`, `**silbe`,
  `**dyn`), durations encoded in the token (`4`, `8.`, `3` for a quarter
  triplet, `0` for a breve), no required key or time signature, and `!` local
  / `!!` global comments.
- **Missing today:** `Track` is staff × voice with `VOICES = 4`
  (`ids.rs:123`, `mod.rs:31-35`), which cannot represent an arbitrary number of
  parallel spines with different interpretations; `Measure` requires
  `len`/`nominal_len` and a numbering (`measure.rs:56-65`) where `**kern`
  needs none; there is no generic "stream of interpretation-tagged tokens".
- **Layer:** an `L6::Humdrum { spines: Vec<Spine> }`, where each spine declares
  its interpretation and references L0 events. L1 stays empty for
  unsignatured passages; the report records every spine interpretation the
  pivot cannot map (as ABC already does for the overlay,
  `format-mapping/abc.md:32`).

### 7.6 Ordering rationale

GABC first is not arbitrary: it is the only coming format whose **primary data
is not in L0 at all**, so it is the cheapest test of "optional layer + derived
projection + loss report". MEI/Capella/Guitar Pro/Humdrum then reuse the same
extension slot; `docs/TODO.md:29-31` sets the same architecture
(`scoretk-<fmt>-{common,import,export}` and symmetric facade features) for each.

## 8. Migration plan (behaviour-preserving, oracle-gated)

**Rule for every step:** freeze the binary *before* the step, run the same five
oracles with the same manifest/refs and at most two workers, require **0 lost
file and 0 changed byte in every direction**, then commit. Commands (from
`scripts/oracle/*/README.md`, e.g. `musicxml/README.md:29-37`,
`lilypond/README.md:175-192`, `midi/README.md:27-35`,
`abc/README.md:72-80`, `musescore/README.md:69-83`):

```sh
nix develop -c env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 nice -n 10 cargo test --workspace
nix develop -c env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 nice -n 10 cargo clippy --workspace --all-targets -- -D warnings
nix develop -c env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 nice -n 10 cargo fmt --all -- --check
scripts/oracle/{musescore,musicxml,midi,lilypond,abc}/mesure.sh   # 5 oracles = 10 directions
nix flake check && nix build .#wasm && cat result/SIZE
```

| Step | Change | Verification specific to the step | Risk |
| --- | --- | --- | --- |
| 0 | Freeze baselines of all five oracles and the wasm size (`result/SIZE`, `docs/agents/wasm.md:44-58`). | Re-run one oracle twice: byte-identical outputs. | None. |
| 1 | Add `Layers`, `Loss`, `ConversionReport`, `Score::report`, and empty `Metric/Text/Playback/Engraving/Extensions` sub-structs; keep `Score`'s existing fields via `Deref`. No reader/writer change. | All five oracles 0 loss; `cargo test --workspace`; wasm raw size delta < 2 % (the existing threshold in `docs/agents/wasm.md:73-76`). | **Low** — no data moves yet; only `Score` layout and `Deref` dispatch. |
| 2 | Move `StyleSettings`, `Credit`, `TboxText` and the `BeamMode`/`StemDirection` flags into `Engraving`; keep field access through `Deref`. | MusicXML full-parity `staff-details` counts unchanged; MuseScore `.mscx` round trip 26/26 v4 + 30/30 v3; all five oracles 0 loss. | **Medium** — most mutations are in export writers; `DerefMut` must forward. |
| 3 | Move note `velocity`/`user_velocity`/`velo_type`/`play`/`unperformed` and `PlaybackTransposition` into `Playback`; add `read_with_report`/`write_with_report` and the additive wasm `convertDetailed`, leaving `convert` untouched. | MIDI reference 21/21 unchanged; `convertWithTimeline` note/tick arrays byte-identical (AGENTS.md invariant 5); clippy `-D warnings`; node smoke test. | **Medium-high** — touches the wasm contract boundary; mitigated by purely additive functions. |
| 4 | Move `Measure::time_sig_denominator` out (unused, `measure.rs:73`), give `BarLine`/`TimeSig`/`KeySig`/`Clef`/`Tuplet` their `Metric` home; record dropped L5 blocks in `Score::report`. | `core.tsv` line counts unchanged in all directions; `ly_notes.py selftest`; all five oracles 0 loss. | **Medium** — mechanical but wide; one field is genuinely dead so its removal is safe. |
| 5 | Add empty `L6::Gregorian` variant and `Extensions` slot; **no reader yet**. | Oracles 0 loss; wasm raw size < 2 % growth; ABI unchanged. | **Low**. |
| 6 | Implement GABC import (L6 → L0/L1 projection + report), then export. | New GABC oracle from real chants (§7.1) plus the five existing oracles 0 loss; report counts neume signs lost in a MusicXML round trip. | **High** (new direction), but isolated in `scoretk-gabc-*`. |
| 7 | MEI → Capella → Guitar Pro → Humdrum, each adding only its L6 slot and the capability declaration. | Each step: its own oracle + all five existing oracles 0 loss; licence check before vendoring humlib/Verovio (`docs/TODO.md:39-41`). | **High** per format, **low** for the core (no layer changes after step 6). |

Step 6 is executed and reported in §10.7 and §11.7: the five existing
oracles were not re-run byte-for-byte for this additive step, but the full
workspace suite, all snapshots and `e2e_corpus` are unchanged, and the GABC
round trip is measured on its own corpus (§9).

Step 1 is deliberately "structure only": it proves that `Deref` keeps ten
directions green before any data moves. Steps 2–4 each move one layer, so a
regression can be attributed to a single crate group. No step changes a file
format or the wasm API.

## 9. Open questions / remaining reference gaps

- **Resolved since the audit (2026-11-23):** the GABC grammar/docs, MEI 5.1
  guidelines/schema, humlib, Verovio `iohumdrum.cpp`, the `**kern` reference
  and the 4.7.4 MuseScore MEI/Capella/Guitar Pro modules are now local
  (§1.4, `specs/README.md`, `docs/agents/upstream.md` §2.1). Section 7 was
  re-checked against them; the "no local reference" conclusion no longer
  holds except where stated below.
- **Still open:** Capella and Guitar Pro have no published format
  specification; the sole reference is the 4.7.4 C++ import source. The
  licence column of `specs/README.md` is **unverified** for the MEI Guidelines
  HTML and the `**kern` reference page — neither retrieved file states a
  licence, so those two are private reference copies until confirmed. Verovio
  is LGPL-3.0: a clean-room rewrite of the format mapping is compatible with
  GPL-3.0-only, but no library source may be copied verbatim.
- `Note::unperformed` (`note.rs:322`) is a deliberate non-MuseScore extension
  added in the current session (`docs/agents/musicxml-export.md` §39). It is
  the first L6-style field in the common model and the concrete precedent for
  §5.1 L6: a format-specific need (LilyPond cluster notes) that must not change
  MuseScore-faithful output.

## 10. Migration report (steps 0–2)

### 10.1 Step 0 — frozen baselines

**Re-frozen on 2026-11-24 before step 3.** Master had received three LilyPond
import fixes since `d5159339` (`ef6dc9e5`, itself merged from `modular-1`); the
old baselines no longer described the starting point. All five oracles were
re-run with a CLI built from the clean `ef6dc9e5` tree (sha256 of the frozen
binary `c346b310b09bc07a82a524aa66dca3edf75fc352f8a17b0ab612ca65fade6e9e`)
and with the Rust core helper `oracle_core` running in that same clean tree.
The first attempt was discarded: it ran the helper from the dirty migration
worktree, so the MusicXML report showed a compile error and 0/173 core-identical
scores instead of 171/173. The complete re-freeze gives 638 MuseScore rows,
174 MusicXML rows, 2 230 MIDI rows, 1 824 LilyPond rows and 26 542 ABC
measurements. Frozen wasm: **2 327 520 raw bytes / 911 434 gzip** (`SIZE-master`).
All step 3–5 comparisons below use this new baseline, not the `d5159339` one.

The original step-0 measurements below are kept for provenance. They were made outside the repository in
`/home/jacques/tmp/scoretk/baselines-mod/` (binary, wasm size, lots, oracle raw
outputs, comparison tool `compare.py`). Source commit: `d5159339`. Frozen
release CLI: sha256 `2a0fd508eafbea524…`. Frozen wasm: **2 305 575 raw bytes /
904 058 gzip** (`result/SIZE`).

| Oracle | Invocation (deterministic subset) | Rows | Raw artefacts |
| --- | --- | ---: | ---: |
| musescore | `mesure.sh` with the five standard lots (corpus 56, mscz 174, xml 32, ly 375, abc 1) | 638 | 3 586 |
| musicxml | `mesure.sh --manifest …/mscz.txt` (174 scores, 1 unsupported excluded) | 174 | 698 |
| midi | `mesure.sh … --alt-stride 5`, `LAKH_STRIDE=50` (1 020 reading, 1 020 writing `.ly`, 122 Mutopia, 29 `.mscz`, 39 Lakh) | 2 459 | 2 397 |
| lilypond | `mesure.sh --refs …/ly95lect-refs-fresh --stride 5` (mus 290, ra 346, mut 265) | 1 824 | 2 587 |
| abc | `mesure.sh liste.txt … abc2xml.py` (1 434 files, 26 542 tune measurements) | 26 542 TSV rows | 53 076 |

The MusicXML oracle was replayed once with the frozen binary: 698/698 artefacts
byte-identical after normalising the run directory, the encoding date and
wall-clock fields, and 174/174 measurement rows identical. The comparison tool
ignores wall-clock columns (`temps`), dates, panic PIDs, alternative-tool
artefacts (`ms.*`, `alt.*`, LilyPond `ref.pdf`) and, for the MIDI oracle, rows of
tools other than scoretk. Two borderline Lakh/Mutopia conversions are
10-second timeout cases under load: a second full MIDI baseline with the frozen
binary was used for step 1 so that the comparison is not polluted by scheduling.

### 10.2 Step 1 — layers and report, no data moved

Commit `49d179e8`. New `model/layers.rs` defines the hand-written `Layers(u8)`
bit set (L0–L6), `Loss { layer, item, tick, track }`, `ConversionReport` with
`record`/`lost`, and the five empty layer structs `Metric`, `Text`, `Playback`,
`Engraving`, `Extensions`. `Score` now wraps `CoreScore` (all pre-existing
fields) and dereferences to it, so `score.notes`, `score.style`, … keep
compiling unchanged; `report`, `metric`, `text`, `playback`, `engraving` and
`extensions` are new additive fields. No reader or writer output changed.

**Adjustment to §5.2:** `Deref` borrows the whole `Score`, not one field, so
`score.measures.get_mut(..)` plus `score.staves.len()` in the same expression no
longer compiles. Three call sites (`gap_rests.rs`, MIDI import, a MusicXML
export test) were changed to hoist the second read or to address
`score.core` directly; `core` is `pub(crate)` inside `scoretk-core`, with public
`core()`, `core_mut()` and `into_core()` (the last one lets a caller move an
arena out without cloning, which `Deref` cannot do). This is a small, local
deviation from "field access kept via Deref" and is preferable to cloning or to
a second, chained `Deref` on every element type.

Verification: `cargo test --workspace`, clippy `-D warnings`, fmt and all eight
`wasm32-unknown-unknown` builds pass. Wasm raw size **2 305 712 bytes**, i.e.
**+137 bytes (+0.006 %)** versus step 0, far below the 2 % threshold. The five
oracles report 0 lost file and 0 changed output byte (rows: musescore 638/638,
musicxml 174/174, midi 2 459/2 459, lilypond 1 824/1 824, abc 26 542/26 542).

### 10.3 Step 2 — engraving data moved to L5

Commit `2d70af9c`. `Engraving` now owns `style: StyleSettings`,
`credits: Vec<Credit>` and `tbox_texts: Vec<TboxText>`; two new small structs
own the per-element flags previously scattered in the event spine:
`ChordRestEngraving { beam_mode, beam_direction }` and
`ChordEngraving { stem }`. `ChordRest` and `Chord` expose them as an `engraving`
field. All readers and writers still produce the same bytes; 36 source and test
files were updated to the explicit paths (`score.engraving.style`,
`chord_rest.engraving.beam_mode`, `chord.engraving.stem`, …).

**Adjustment to §5.2 / step 2:** the plan suggested `Option<Box<Engraving>>` and
"field access through `Deref`". A `Deref` chain `Score → CoreScore → Engraving`
(and the same trick on every `ChordRest`/`Chord`) would keep `score.style`
spelling, but it hides the layer boundary, makes disjoint borrows worse than
step 1's single `Deref`, and cannot express an absent layer (`Deref` targets a
value, not an `Option`). `Engraving` is therefore **embedded by value** and
accessed explicitly; the other layers stay `Option` because nothing writes
them yet. This changes no output and keeps the wasm contract untouched.

Verification: `cargo test --workspace`, clippy `-D warnings`, fmt, all eight
`wasm32-unknown-unknown` builds and `ly_notes.py selftest` pass. Wasm raw size
**2 305 411 bytes**, i.e. **−164 bytes (−0.007 %)** versus step 0. The five
oracles again report 0 lost file and 0 changed output byte (same row counts as
step 1). In particular, the MusicXML full-parity `hidden staff-details` counts
and the MuseScore round trips are unchanged.

### 10.4 Step 3 — playback layer and additive report API

`NotePlayback { play, unperformed, velocity, user_velocity, velo_type }` owns the
five per-note L4 fields and `Playback { transposition, articulate,
unfold_repeats }` owns the score-wide ones; all 25 reader/writer/test call sites
were changed to `note.playback.*` / `score.playback.*`. The facade gains
`read_with_report`, `write_with_report` and `convert_with_report`; `read`,
`write` and `convert` now delegate to them and ignore the report, so their
signatures and bytes are unchanged. `write_with_report` appends L5 losses for
credits and TBox texts that a writer cannot emit. On wasm, `convertDetailed`
returns `{ bytes, losses }`; `convert`, `convertWithTimeline`, `detect`,
`formats` and `version` are untouched, and `classic/scoretk.js` merely forwards
the new function. The node smoke test asserts that `convertDetailed().bytes`
equals `convert()` byte for byte.

**Adjustments to §5.2/§6.3:** `Note::new` is `const fn`, so it cannot call
`NotePlayback::default()`; the layer exposes `NotePlayback::DEFAULT` as a
`const` and `Default` delegates to it. The write-side report is *not* a copy of
`Score::report`: it starts with it and appends writer-specific losses, so the
old test assumption "report == score.report" was replaced by a prefix check.
These are local consequences of making the API additive, not design changes.

Verification against the re-frozen `ef6dc9e5` baseline: `cargo test --workspace`,
clippy `-D warnings`, fmt, all eight `wasm32-unknown-unknown` builds,
`ly_notes.py selftest` and the Node smoke test pass. Wasm raw size **2 331 467
bytes**, i.e. **+3 947 bytes (+0.170 %)** versus the new baseline, far below the
2 % threshold. The five oracles report **0 lost file and 0 changed artefact
byte**: MuseScore 638/638 rows (1 850/1 850 scoretk artefacts), MusicXML
174/174 (352/352), MIDI 2 230/2 230 (2 197/2 197), LilyPond 1 824/1 824
(2 586/2 586), ABC 26 542 measurements (53 076/53 076 artefacts). Two Lakh
rows in the MIDI robustness lot flapped from success to the 10 s timeout under
concurrent load (9.49 s and 5.87 s in the baseline); replaying exactly those
two files without the load gives identical SHA-256 in both binaries
(`852bf893…`, `b66941b0…`, 3 890 089 and 3 871 157 bytes). No output changed.

### 10.5 Step 4 — metric layer, dead field and read-side losses

`layers/metric.rs` now owns `Metric`, `Key`, `KeySig`, `KeyMode`, `TimeSig`,
`TimeSigType`, `Clef`, `ClefType`, `ClefSign`, `BarLine`, `BarLineType`,
`Tuplet` and `TupletNumberType`; `segment.rs` and `tuplet.rs` re-export them at
their old paths and `model/mod.rs` re-exports them from `layers::metric`, so no
reader or writer path changed. `Measure::time_sig_denominator` (initialised but
never read, §4.1) is gone; `with_raw_timesig` takes only the raw numerator and
the two MIDI import call sites were updated. The 3.x and 4.x MuseScore readers
now append an `ENGRAVING` `Loss` to `Score::report` for `<Symbol>`, `<Image>`,
`<StaffState>`, `<PlayTechAnnotation>`, `<Sticking>` and `<HarpPedalDiagram>`
instead of ignoring them silently.

**Adjustment to §8 step 4:** "give BarLine/TimeSig/… their `Metric` home" is
implemented as a physical move plus re-exports, not as a `Deref` chain: as in
step 2, `Deref` would obscure the layer boundary and worsen borrows. The
report records *read-side* L5 drops only in the two MuseScore readers; the
other directions had no explicit "ignored layout block" site to hook.

Verification: `cargo test --workspace`, clippy `-D warnings`, fmt, all eight
`wasm32-unknown-unknown` builds, `ly_notes.py selftest` and the Node smoke test
pass. Wasm raw size **2 331 934 bytes**, i.e. **+4 414 bytes (+0.190 %)** versus
the re-frozen baseline, far below the 2 % threshold. MuseScore (638/638 rows),
MusicXML (174/174) and ABC (26 542 measurements) are byte-identical to the
baseline. The first MIDI and LilyPond runs lost external *inputs*, not outputs:
`wt-mod/tests/corpus/midi/v3/*.ref.mid` (21 files) and 66 `lyluatex/tmp-ly/*.ly`
files were removed by other worktrees during the campaign. Both oracles were
re-run with stable manifests (MIDI refs pointed at the scratch worktree, the 13
affected `ra` files filtered out) for **both** binaries: 2 197/2 197 MIDI
artefacts and 2 544/2 544 LilyPond artefacts byte-identical, 0 lost file.
Scheduled 10 s Lakh timeouts flapped on eight rows under concurrent load;
replaying those eight files without load produces identical SHA-256 on both
binaries. No output byte changed in any of the five directions.

### 10.6 Step 5 — empty L6 slot, no reader

`Extensions { format: Option<Extension> }` replaces the empty struct;
`Extension` is `#[non_exhaustive]` and has one variant, `Gregorian(Gregorian)`,
where `Gregorian` is a zero-sized marker. Nothing constructs a payload outside
one unit test, no reader or writer references L6, and the wasm ABI is
unchanged. The GABC reader (step 6) will own the syllable/neume data here.

Verification: `cargo test --workspace`, clippy `-D warnings`, fmt, all eight
`wasm32-unknown-unknown` builds, `ly_notes.py selftest` and the Node smoke test
pass. Wasm raw size **2 331 942 bytes**, i.e. **+4 422 bytes (+0.190 %)** versus
the re-frozen master baseline and **+8 bytes** versus step 4, far below the 2 %
threshold. MusicXML (174/174) and ABC (26 542 measurements) are byte-identical
to the baseline. The scratch worktree `wt-mod`, whose paths the frozen lot
manifests used, was deleted by another task during this step: 56 MuseScore
corpus rows failed to find their inputs. The corpus lot was re-run for **both**
binaries with paths pointing at the migration worktree: 172/172 artefacts
byte-identical, 0 changed rows. MIDI and LilyPond were likewise re-run with
stable manifests for both binaries (2 197/2 197 and 2 544/2 544 artefacts
identical); one more Lakh 10 s timeout flapped and was replayed byte-identically
(`23efbd9f…`, 6 626 353 bytes). The MuseScore `ly` lot's only row difference is
an *alternative tool* (MuseScore itself) timing out, not scoretk output.
No output byte changed in any direction and no contract element moved.

### 10.7 Step 6 — GABC import and export

Three new portable crates (`scoretk-gabc-common`, `-import`, `-export`) hold
the Gregorian L6 payload and the derived L0/L1 projection. The facade gains
`Format::Gabc`, `.gabc` extension detection, `name:`/`%%` content detection,
the symmetric features `gabc-import`/`gabc-export` (on by default), an error
variant, and CLI/wasm wiring; `formats()` now reports eight formats. The
stable wasm API signatures are untouched, `convert`'s default `to` is still
`"musicxml"`, and no existing writer path was modified.

Verification: `cargo test --workspace`, clippy `-D warnings`, `fmt --check`,
the three `wasm32-unknown-unknown` builds of the new crates and `scoretk`
pass. The nine Gregorio examples round trip through the *structured fields*, not
byte-for-byte (`tests/corpus/gabc/`); `e2e_corpus` and all existing snapshots
are unchanged. After the correction of the self-comparison, the Gregorio
oracle on the full 202-file lot gives **193/194 core-exact and 0/194
dump-identical** (5 files invalid for Gregorio itself, 3 more rejected after
re-serialisation), layer coverage is **100 % / 100 %** on the direct row and
**83.00 % essential / 59.07 % full support** on the GABC → MusicXML → GABC
cross row (3 435 items). Wasm raw size is 2 403 979 bytes versus 2 379 470 at
the parent commit `a4053795`, i.e. **+24 509 (+1.03 %)**, gzip +13 301
(+1.43 %): below the +2 % budget and the ≈ 2.5 MB cap. Full tables,
citations and limits: `docs/agents/gabc.md` §7-§11.

## 11. Layer coverage: declared capabilities and first measurement

This section records the measurement campaign announced in §5.3: every reader
and writer must eventually implement **all the layers its format can carry**,
and the result is reported as two percentages per direction. This first pass
builds the instrument only; it changes no reader, no writer and no output byte.

### 11.1 Exact definitions

- **Item.** One comparable fact of the model, stored in `model::InventoryItem`
  as `(layer, tick, staff, kind, value)`: `layer` is exactly one of L0–L6,
  `tick` is an exact `Fraction` when the item is positioned, `staff` is the
  staff index when the item belongs to a staff, `kind` is a stable ASCII label
  (`"note"`, `"measure"`, `"key-sig"`, `"dynamic"`, `"style"`, …) and `value`
  is its canonical value (pitch, label, formatted fraction…). `Score →
  LayerInventory` performs an in-memory traversal of measures → segments →
  tracks and of the arenas; no I/O enters the core (invariant 1).
- **Item kinds.** L1: `measure`, `time-sig`, `key-sig`, `clef`, `barline*`,
  `repeat-count`, `multi-measure-rest`, `measure-repeat`, `section-break`,
  `marker`, `jump`, `tuplet`. L2: `note`, `rest`, `grace*`, `tie`, `tuplet-member`,
  `articulation`, `ornament`, `tremolo`, `arpeggio`, `chord-line`, `fermata`,
  `slur`, `hairpin`, `ottava`, `pedal`, `text-line`, `trill`, `glissando`,
  `spanner`, `breath`. L3: `lyric`, `dynamic`, `tempo`, `staff-text`,
  `system-text`, `rehearsal-mark`, `instrument-change`, `harmony`,
  `figured-bass`, `annotation`, `title`, `credit`, `tbox-text`. L4:
  `instrument`, `program`, `channel`, `volume`, `pan`,
  `velocity`, `user-velocity`, `dynamic-velocity`, `articulate`,
  `playback-transposition`. L5: `style`, `credit-position`, `tbox-position`,
  `beam`, `stem`, `note-head`, `line-break`, `tuplet-bracket`. L6:
  `midi-articulation`, `style-internal`. L0 has no separate item kind (it is
  the event spine used to reach the others), so it is neutral in the ratios;
  L6 counts only for MuseScore, which declares it. The `tuplet-bracket`
  value is the rendered decision (`bracket`) only: `bracket_explicit` records
  how the file encodes it, not a second engraving fact. Style floats are
  normalised to five decimals, MuseScore's writer using six significant digits:
  comparing six decimals would turn `0.7874016` → `0.787402` into a false loss.
- **Matching.** Two inventories are multisets of item keys. An item in the
  candidate matches a reference item only if **all five** components are
  equal; duplicates are counted, so two identical attacks are two items.
  `matched = Σ min(reference_count, candidate_count)` per key. A changed
  tick, staff or value is a loss plus an extra item, not a silent match.
- **Essential %** = matched `note` + `measure` items / reference items of
  those two kinds. This is today's musical-core notion of compliance.
- **Full-support %** = matched items / reference items, restricted to the
  layers the format **declares**, at the whole-corpus level.
- **Per-layer %** = the same ratio for one declared layer. A layer with zero
  reference items is 100 % by convention and is not printed.
- **Reference model** = `tests/corpus/{v3,v4}/*.mscx` read by MuseScore import.
  The round-trip row is `reference → write X → read X → compare`. The
  import-alone row reads an existing reference file with the format's reader
  and compares it with the same model, restricted to the layers that reader
  declares.

`Capabilities { reads, writes }` (§5.3) lives in the core; each of the ten
direction crates exports a `CAPABILITIES` constant. An import crate declares
`reads = format layers, writes = NONE`; an export crate the reverse. The
declared layers are those of the **format's specification**, not those the
code handles today — the difference is the gap being measured.

### 11.2 Declared matrix

| Direction crate | Format layers (reads ∪ writes) | Citation (spec, not code) |
| --- | --- | --- |
| `scoretk-musescore-{import,export}` | L0+L1+L2+L3+L4+L5+L6 | `format-ref/musescore.md` § « Section index »: §4 parts/clefs, §5 measures, §7 chords/beams, §10 key/time/barlines, §12 slurs/hairpins/voltas, §13 lyrics/dynamics, §15 excerpts, §18 frames; `format-mapping/musescore.md` lines 14–37. L6 covers excerpts and graphical objects. |
| `scoretk-musicxml-{import,export}` | L0+L1+L2+L3+L4+L5 | `format-ref/musicxml.md` « Document shape » (L3/L5), « Pitch, accidentals, key », « Clef, time, staves, parts », « Notes, rests, chords, beams », « Ties, slurs, tuplets, grace », « Repeats, jumps, directions, lyrics »; `format-mapping/musicxml.md` lines 16–47. `<defaults>`/`<credit>`/`<beam>` justify L5, `<midi-instrument>`/`<play>` justify L4; no L6 payload. |
| `scoretk-midi-{import,export}` | L0+L1+L2+L3+L4 | `format-ref/midi.md` « Events and time », « Meta-event types » (`FF 58`, `FF 59`, `FF 51`, text metas), « Instruments, parts, repeats », « Lyrics: RP-017 and RP-026 »; `format-mapping/midi.md` lines 15–36. L1 is declared for the time-signature/key/marker metas even though SMF has no measure element; L5/L6 do not exist in SMF. |
| `scoretk-lilypond-{import,export}` | L0+L1+L2+L3+L4+L5 | `format-ref/lilypond.md` « Durations, ticks, bars », « Pitch, key, clef, accidentals », « Parts, staves, voices, chords », « Ties, slurs, tuplets, grace », articulations/ornaments (§3.1–3.3), « Repeats and lyrics », `\paper`/`\layout`; the second MIDI `\score` carries L4 (`format-mapping/lilypond.md` line 38; rows 16–40). |
| `scoretk-abc-{import,export}` | L0+L1+L2+L3+L4 | `format-ref/abc.md` §3.1 headers, §4.3–4.5 durations/repeats, §4.8–4.10 barlines/endings, §4.11–4.17 ties/grace/tuplets/chords, §5 lyrics, §6.2 `%%MIDI` (L4), §7 voices; `format-mapping/abc.md` lines 11–33. **L5 is not declared**: ABC 2.1 has no page style, no per-element coordinates and only implicit beaming (§4.7, mapping « Model gaps » n°4). |
| `scoretk-mei-{import,export}` | L0+L1+L2+L3+L4+L5 | `format-ref/mei.md` §2–§13 and `docs/agents/mei.md` §3: `<measure>`/`<staff>`/`<layer>` (§5–§6, L0/L1), `<note>`/`<rest>` (§7–§8, L2), `<dir>`/`<dynam>`/`<harm>` with `<lb/>`/`<rend>` (§10–§11, L3), `<instrDef @midi.instrnum>` and `@dur.ges`/`@tstamp.ges` (§4, §12, L4), `<beam>`/`<pb>`/`<sb>`/`<pgHead>` and `@color` (§9, §13, L5). L6/facsimile data (`@facs`, `<facsimile>`) is not read (§13). |
| `scoretk-gabc-{import,export}` | L0+L1+L2+L3+L6 | `format-ref/gabc.md` §1 file structure, §2 headers, §3 syllables, §4 pitches, §5–§6 neumes and signs (L2), §7 bars (L1), §8 clefs; `Gabc.tex:228-343,408-443,452-870`; the L6 slot is §7.1. **L4/L5 are not declared**: no tempo/velocity header (`:67-154`) and no page geometry (`:801-845`). |

MEI now declares L0–L5 (`scoretk-mei-{import,export}`, citation:
`docs/agents/format-ref/mei.md` §2–§13 and `docs/agents/mei.md` §3), and GABC
(§7.1) now exports `CAPABILITIES` from both direction crates (L0+L1+L2+L3+L6,
citation in the table above). The remaining coming formats (Capella §7.3,
Guitar Pro §7.4, Humdrum §7.5) have no crate yet, so they have no declaration
to export. Their mapping documents already assign layers and will reuse the
same contract once their crates exist.

### 11.3 First measured table

Command:
`nix develop -c env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 nice -n 10 cargo test -p scoretk --test layer_coverage -- --nocapture`.
The test only panics on a broken setup; it never asserts on a percentage.
Corpus: 56 `.mscx` scores (v3 + v4); 21 MIDI references; 56 LilyPond and 56
ABC snapshot files. No file was skipped.

Round trip (`model → write X → read X`):

| Direction | Declared | Essential | Full support | L1 | L2 | L3 | L4 | L5 | Items |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| mscx | L0–L6 | 100.00 % | 95.36 % | 100.00 | 100.00 | 99.66 | 100.00 | 72.79 | 6310 |
| mscz | L0–L6 | 100.00 % | 95.36 % | 100.00 | 100.00 | 99.66 | 100.00 | 72.79 | 6310 |
| musicxml | L0–L5 | 99.83 % | 78.32 % | 90.97 | 99.25 | 88.18 | 31.55 | 21.81 | 6310 |
| mxl | L0–L5 | 99.83 % | 78.32 % | 90.97 | 99.25 | 88.18 | 31.55 | 21.81 | 6310 |
| midi | L0–L4 | 83.41 % | 63.61 % | 62.67 | 76.28 | 7.77 | 18.45 | — | 5237 |
| lilypond | L0–L5 | 99.27 % | 79.32 % | 85.71 | 97.19 | 65.54 | 21.59 | 50.61 | 6310 |
| abc | L0–L4 | 98.92 % | 81.08 % | 83.41 | 96.32 | 50.34 | 0.00 | — | 5237 |

Import alone (existing reference file → reader → compare). The **Source**
column is part of the measurement: it says what the row is evidence *about*.

| Format | Declared | Essential | Full support | L1 | L2 | L3 | L4 | L5 | Files / items | Source |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| musicxml | L0–L5 | 99.83 % | 78.32 % | 90.97 | 99.25 | 88.18 | 31.55 | 21.81 | 56 / 6310 | MuseScore 4.7.4 `*.ref.musicxml` |
| mxl | L0–L5 | 99.83 % | 78.32 % | 90.97 | 99.25 | 88.18 | 31.55 | 21.81 | 56 / 6310 | MuseScore 4.7.4 `*.ref.musicxml` |
| midi | L0–L4 | 71.04 % | 32.52 % | 43.95 | 43.31 | 2.47 | 16.67 | — | 21 / 781 | MuseScore 4.7.4 `*.ref.mid` |
| lilypond | L0–L5 | 99.27 % | 79.32 % | 85.71 | 97.19 | 65.54 | 21.59 | 50.61 | 56 / 6310 | **scoretk snapshot — self-consistency** |
| abc | L0–L4 | 98.92 % | 81.08 % | 83.41 | 96.32 | 50.34 | 0.00 | — | 56 / 5237 | **scoretk snapshot — self-consistency** |

The `mxl` import-alone row reads the same `.ref.musicxml` files as `musicxml`
(the reader accepts both containers), so both rows are expected to be equal.
The LilyPond and ABC snapshots under `tests/corpus/{ly,abc}/` were produced by
scoretk's own writers, not by an independent tool: their import-alone rows
are *self-consistency* measurements, not external compliance evidence. They
repeat the round trip and must never be quoted as a second, independent
measurement. Only the MusicXML and MIDI references come from MuseScore 4.7.4.
`reference_sources_distinguish_self_written_snapshots` in
`crates/scoretk/tests/layer_coverage.rs` asserts that the test prints the same
distinction.

The MusicXML import-alone row is **not** a re-read of scoretk's round-trip
output: `reference_file()` opens `tests/corpus/{v3,v4}/<name>.ref.musicxml`, and
`musicxml_import_alone_reads_musescore_474_references` checks for all 56 files
that the path ends in `.ref.musicxml` and that the file carries
`<software>MuseScore Studio 4.7.4</software>`. Its equality with the round-trip
row to two decimals is therefore a meaningful result: scoretk's writer and
reader reproduce exactly the comparable items that the MuseScore 4.7.4
reference and scoretk's reader agree on (the only difference is one `extra`
item, 135 instead of 134, from the round-trip writer's layout).


### 11.4 Ranked per-layer gaps (round trip)

Missing item counts, highest first; they are the queue for the next tasks.

| Rank / direction | Layer | Item kind | Missing |
| --- | --- | --- | ---: |
| 1 mscx / mscz | L5 engraving | `tuplet-bracket` | 262 |
| 2 mscx / mscz | L5 engraving | `style` | 30 |
| 3 mscx / mscz | L3 text | `title` | 1 |
| 1 musicxml / mxl | L5 engraving | `style` | 434 |
| 2 musicxml / mxl | L4 playback | `midi-articulation` | 367 |
| 3 musicxml / mxl | L5 engraving | `tuplet-bracket` | 283 |
| 4 musicxml / mxl | L1 metric | `barline` | 85 |
| 5 musicxml / mxl | L5 engraving | `credit-position` | 63 |
| 1 midi | L4 playback | `midi-articulation` | 367 |
| 2 midi | L2 events | `note` | 284 |
| 3 midi | L2 events | `rest` | 200 |
| 4 midi | L1 metric | `measure` | 100 |
| 5 midi | L1 metric | `barline` | 85 |
| 1 lilypond | L5 engraving | `style` | 444 |
| 2 lilypond | L4 playback | `midi-articulation` | 367 |
| 3 lilypond | L1 metric | `barline` | 85 |
| 4 lilypond | L4 playback | `dynamic-velocity` | 43 |
| 5 lilypond | L5 engraving | `note-head` | 41 |
| 1 abc | L4 playback | `midi-articulation` | 367 |
| 2 abc | L1 metric | `barline` | 85 |
| 3 abc | L3 text | `credit` | 67 |
| 4 abc | L4 playback | `instrument` | 63 |
| 5 abc | L4 playback | `program` | 63 |

Import-alone MIDI has a different profile: `midi-articulation` 143 (L4),
`note` 53 and `rest` 35 (L2), `credit` 28 (L3), `measure` 22 (L1). The
MuseScore 4.7.4 references unfold and quantise differently from scoretk's
writer, so its essential score (71.04 %) is lower than the round-trip one.

Reading the table:

1. **`midi-articulation` (was L4, now L6; 367 items) was the largest cross-format gap.**
   MuseScore files carry a per-instrument articulation/velocity table; no
   other declared format writes it back. It has been moved out of L4 into
   L6 (MuseScore-only) in the inventory: MusicXML's `<midi-instrument>` only
   has channel/program/volume/pan (`format-ref/musicxml.md` §"Clef, time,
   staves, parts"), SMF has channel voice and meta-events but no articulation
   table (`format-ref/midi.md` §"Events and time", §"Instruments, parts,
   repeats"), ABC's `%%MIDI` directives cannot reconstruct it
   (`format-mapping/abc.md` §6.2 and "Model gaps" n°5), and LilyPond has no
   equivalent. The formats' effects (velocity, gate time) remain in L4; the
   table itself is a MuseScore instrument definition, not a format item.
   `midi_articulation_table_is_a_musescore_extension` in the core asserts the
   reclassification. The historical table above is left unchanged; the new
   ratios are in §11.6.
2. **`style` (L5, 434–444 items)** is *partly* a genuine format item and
   partly a MuseScore-internal default. Page geometry (`spatium_mm`, page
   width/height and margins) and credit font sizes are carried by MusicXML
   `<defaults><scaling>/<page-layout>/<page-margins>` and `<credit-words
   font-size>` (`format-ref/musicxml.md` §"Document shape") and by LilyPond
   `\paper` (`format-mapping/lilypond.md`, `\paper` row): they stay in L5 and
   the writers must be fixed. `hairpinHeight`, `smallStaffMag`,
   `dontHideStavesInFirstSystem`, `concertPitch`, `createMultiMeasureRests`,
   `pageCount` and `minEmptyMeasures` have no counterpart in those specs:
   they are now `style-internal` (L6, MuseScore only) with a core test
   (`muse_score_only_style_defaults_are_extensions`). ABC and MIDI do not
   declare L5, so those rows are unaffected by the split.
3. **`tuplet-bracket` (L5, 262 items)** was lost by the MuseScore round trip
   itself: the 3.x reader records the bracket decided by the 3.x rule, the
   writer emits a 4.70 file and the 4.x reader re-infers a different value.
   Fixed in the writer: `TWrite::write(const Tuplet*)` calls
   `writeStyledProperties` (`engraving/rw/write/twrite.cpp:3321-3345`, tag
   v4.7.4) and `BRACKET_TYPE` is a styled property (`engraving/dom/tuplet.cpp:
   48-60`); scoretk now writes `SHOW_NO_BRACKET` only for short beamed n-olets
   whose decision would otherwise be lost. The inventory compares the rendered
   decision `bracket`, not the `bracket_explicit` encoding flag. Two tests
   (`crochet_implicite_pas_de_propriete`,
   `crochet_court_implicite_preserve_au_aller_retour`) cover both sides.
4. **`barline` (L1, 85 items)** is an `EndBarLine` segment object that the
   MusicXML and ABC readers did not restore (only the measure fields were
   set). Fixed by recreating the segment, including the implicit `Normal`
   barlines MuseScore keeps (`src/barline.rs`, `src/pass2.rs`,
   `reader/build.rs`; tests `barre_de_fin_est_un_element_du_segment` and
   `barre_implicite_en_fin_de_mesure`). LilyPond gained the same implicit bar
   line (`builder.rs::create_measures`, test
   `barre_normale_est_un_element_de_segment`) and a `\paper` page-geometry
   round trip (`writer.rs::write_paper`, `builder.rs::apply_paper`, test
   `paper_porte_la_geometrie_de_page`). `key-sig` (37 in ABC) remains a metric
   item lost by the ABC writer (ABC has one global `K:` field, §3.1).
5. **MIDI L3 (7.77 % round trip, 2.47 % import alone)** is dominated by
   credits and titles: SMF carries track names and text metas, but the
   mapping to `Score::metadata`/`Credit` is only partial.

### 11.5 What this measurement does not yet do

- A zero denominator means "nothing comparable", reported as 100 %; it must
  not be read as a perfect implementation of the layer.
- The percentages are corpus-weighted item ratios, not byte equality and not
  a perceptual judgement. An item that moves to a different tick counts as
  lost even when the music sounds the same; this is intentional because the
  layers are addressed by tick.
- The declared matrix is coarse: a format may carry only part of a layer
  (ABC's implicit beams, MIDI's time-signature metas without measures). The
  declaration is per layer, as §5.3 specifies; finer per-item declarations
  would be a later refinement.
- No reader or writer was changed by this campaign. §11.4 is the input to the
  next tasks, which must close those gaps one layer at a time, re-running
  this table after each change.

### 11.6 Final table after the two gap campaigns (2026-11-25)

Definitions changed during §11.0–§11.5: `midi-articulation` and the
MuseScore-only style defaults are L6 (§11.4), the `tuplet-bracket` item
compares the rendered decision, and style floats are normalised to five
decimals. A second, finer declaration was added in this campaign:
`Capabilities::with_kinds` restricts a direction to the item kinds the
format specification can carry (§5.3), with a core test and a format test
`midi_declaration_follows_smf_spec`. The "before" column below is the
previous §11.6 table, not a re-measurement with the old definitions.

Round trip (`reference → write X → read X`):

| Direction | Declared | Before (previous §11.6) | After | Target 95 % |
| --- | --- | ---: | ---: | --- |
| mscx / mscz | L0–L6 | 99.51 % | **99.51 %** | reached |
| musicxml / mxl | L0–L5 | 95.44 % | **95.44 %** | reached |
| midi | L0–L4, 14 kinds | 68.40 % | 73.51 % | **not reached — proven out of reach** |
| lilypond | L0–L5 | 90.58 % | **95.15 %** | reached |
| abc | L0–L4 | 89.75 % | **95.24 %** | reached |
| gabc (direct, own corpus) | L0+L1+L2+L3+L6 | — (absent from the previous table) | **100.00 %** | reached on its own corpus |
| gabc → xml → gabc (cross) | L0+L1+L2+L3+L6 | — (absent from the previous table) | 59.07 % | not reached |
| mei | L0–L5 | — (absent from the previous table) | 92.35 % (was 73.52 %) | not reached |

Import alone (`reference file → reader`, same redefined inventory):

| Format | Source | Before (previous §11.6) | After |
| --- | --- | ---: | ---: |
| musicxml / mxl | MuseScore 4.7.4 `*.ref.musicxml` | 95.44 % | **95.44 %** |
| midi | MuseScore 4.7.4 `*.ref.mid` | 39.81 % | 49.84 % |
| lilypond | scoretk snapshot — self-consistency | 90.58 % | **95.15 %** |
| abc | scoretk snapshot — self-consistency | 89.75 % | **95.24 %** |
| gabc → model | Gregorio examples (own corpus, direct) | — (absent from the previous table) | **100.00 %** |
| gabc → xml → gabc → model | Gregorio examples (own corpus, cross) | — (absent from the previous table) | 59.07 % |
| mei | MuseScore 4.7.4 `*.ref.mei` | — (absent from the previous table) | 84.18 % (was 81.90 %) |

The MEI and GABC rows were measured after the rebase on master with
`cargo test -p scoretk --test layer_coverage -- --nocapture`: MEI uses the
same 56 MuseScore scores and 56 MuseScore 4.7.4 `*.ref.mei` references (5 879
items, 0 skipped); GABC uses the nine Gregorio examples of
`tests/corpus/gabc/` (3 435 items, 0 skipped) because it has no MuseScore
corpus reference. GABC's two rows measure different things: a structured
round trip and a real cross conversion through MusicXML (§11.7). Neither
MEI row reaches 95 % after the gap campaign of `docs/agents/mei.md` §7:
the largest remaining round-trip gaps are `dynamic-velocity` (43, no MEI 5.1
velocity attribute), `key-sig` mode (37, no MEI `@mode`), `lyric` (30),
`note` (26), `tuplet-member` (26), `note-head` (22) and `slur` (19); on the
import row the 380 `style` items are absent from MuseScore's own `.ref.mei`
files and therefore unrecoverable (proved by
`mei_musescore_references_carry_no_style`). No format declaration was
weakened to raise a percentage, and no private carrier was counted as a gain.

`e2e_corpus` stays at 8/8 passing with no changed discrepancy, as do
`cargo test --workspace`, clippy `-D warnings` and `fmt --check` after each
commit. The LilyPond/ABC snapshots were regenerated for each intentional
format change; every snapshot diff is accounted for by one of the changes
below (private directives, local keys, note-head styles, font sizes and
velocities).

#### What closed the gaps

- **ABC 89.75 → 95.24 %.** `%%scoretk instrument` (ABC 2.1 §11, unknown
  directives must be ignored) carries the MuseScore instrument id alongside
  `V: … name=`/`snm=` (§7.1); the previous claim that ABC cannot carry an
  instrument identity was wrong. Inline `[K:…]` fields (§3.2/§7.3) are
  voice-local: the writer now emits them on the staff that owns the change,
  and a mode-less `K:` is read as `KeyMode::Unknown`, as MuseScore 4.7.4
  writes it. `W:` (§3.1) carries credits, `%%scoretk tbox`/`meta` the frame
  texts and remaining metadata, `!scoretk-velo-N!` (§4.14/§11) the numeric
  velocity of `!p!`, and `!/!`/`!scoretk-tremolo-…!` the tremolo kind. The
  end-bar line is now also a segment item, as MuseScore keeps it.
- **LilyPond 90.58 → 95.15 %.** `#(set-global-staff-size n)` (Notation §27.2)
  round-trips `Spatium`; private `\header` fields carry text-style sizes.
  `\set Staff.midiMinimumVolume`/`midiMaximumVolume` (§24.4.2) carry the
  numeric velocity of a dynamic. The shared `noteheads.rs` table maps every
  `NoteHeadGroup` to a compilable LilyPond style plus a private markup for
  exact groups; `\bar` end kinds are copied to the end-bar segment; private
  `\header` fields carry credits and frame texts; a private markup carries
  the two-note tremolo kind (§4.2.2). LilyPond still compiles all 56 outputs
  without warnings.

#### Why MIDI cannot reach 95 % under a faithful SMF declaration

The SMF has no measure or bar-line object: the delta-time track chunk has
"no absolute timestamps and no bar/measure/staff/voice fields"
(`format-ref/midi.md` §"Container layout"), and §"Instruments, parts,
repeats" states that repeats must be expanded and that there are no ties,
tuplets or slurs. Consequently the earlier per-layer declaration counted
hundreds of items that *no SMF can encode* (`measure`, `barline`, `clef`,
`tuplet`, `slur`, `credit`, …). The honest declaration now lists the 14 kinds
the specification does carry (`FF 58` time signature, `FF 59` key signature,
`FF 06` marker, `FF 05` lyrics, `FF 51` tempo, `FF 03`/`FF 04` names, channel
voice messages); every listed kind is tested in
`midi_declaration_follows_smf_spec`, and the denominator drops from 4 870 to
2 522 items.

The remaining 668 losses are dominated by 284 `note` and 200 `rest` items
plus 60 `title` items. They are **not** missing objects: pitches and
velocities are written as Note On/Off and survive. The inventory is keyed by
the *written* tick, whereas an SMF stores a **performance**: unfolding a
repeat moves every later attack (`format-ref/midi.md` §"Instruments, parts,
repeats"). Two measurements prove that this is not an unimplemented writer
feature:

1. `midi.expandRepeats = false` (flat performance) gains only 27 notes
   (68.40 → 69.01 % under the old declaration);
2. projecting the reference inventory through `RepeatList::unwind` (the very
   unfolding MuseScore performs) and comparing the same performance events
   gains only 44 matches (73.51 → 73.95 % under the new declaration).

The residual tick differences come from the importer's mandatory quantisation
to a sixteenth and its simplified tuplet detection — both explicitly
documented as faithful ports of MuseScore's importer
(`docs/agents/midi.md` §"Measured limits", `format-mapping/midi.md` rows
29–30). Rewriting them to recover written ticks would break the 4.7.4
fidelity invariant (AGENTS.md §2) and the 21/21 MIDI reference exports.
Therefore the remaining MIDI gap is a property of tick-addressed comparison
against a performance format, not an unimplemented layer: with the
spec-cited declaration, 95 % is provably unreachable without changing the
definition of "matched".

No definition was changed to gain points: each reclassification above is
backed by a format specification section and by a test, and the `style`
items kept in L5 (page geometry, scaling, credit fonts) are exactly those
that MusicXML `<defaults>` and LilyPond `\paper` can carry.

### 11.7 GABC coverage (step 6, corrected measurement)

GABC is not measured on the MuseScore corpus: its primary data is the L6
syllable/neume stream, and its L0/L1 view is a derived projection with
invented durations (§7.1). `crates/scoretk/tests/layer_coverage.rs` adds two
rows over `tests/corpus/gabc/` (nine Gregorio examples, GPL-3.0-or-later):
the direct `GABC → model → GABC → model` and a **cross** conversion
`GABC → MusicXML → GABC → model`. The writer now serialises the structured
payload instead of copying the source, so a byte-identical round trip is
neither expected nor measured.

| Row | Declared | Essential | Full support | Files | Items | Source |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| gabc (direct) | L0+L1+L2+L3+L6 | **100.00 %** | **100.00 %** | 9 | 3 435 | structured fields re-parsed, **not** a byte comparison |
| gabc → xml → gabc (cross) | L0+L1+L2+L3+L6 | **83.00 %** | **59.07 %** | 9 | 3 435 | 9/9 outputs differ from the source; real conversion |

Per declared layer of the cross row: L1 45.03 % (154/342), L2 95.74 %
(989/1 033), L3 95.78 % (295/308), L6 33.73 % (591/1 752); the direct row
matches all 3 435 items. Two real defects were fixed during this measurement:
the derived projection used measure-relative segment ticks (stacking MIDI
onsets) and dropped syllable text; with absolute ticks and first-note lyrics
the L2/L3 rows rose from 27.11 %/58.06 % to the figures above. The remaining
loss is structural: MusicXML has no neume stream, so the second GABC is
projected from concert pitch, lyrics and bars (`from_score.rs`), losing
signs, shape groups, custos and typography; GABC measures are not MusicXML
measures and have no metre (`Gabc.tex:801-845`).

Gregorio's independent oracle on the full 202-file lot scores the writer
**193/194 = 99.48 % core-exact but 0/194 byte-identical dumps**; see
`docs/agents/gabc.md` §9–§10 for the per-file causes (Dominican bars,
verbatim `<v>`, custos, and the one residual `@[…]` fusion file).

### 11.8 Honest portability: without the scoretk-private extensions

The 95 % rows of §11.6 are **scoretk → scoretk** measurements: the writers
emit private syntax that only scoretk's readers interpret. Other ABC and
LilyPond tools ignore it. ABC 2.1 §11.0.1 explains that `%%` pseudo-comments
were introduced so that "software that didn't recognise them would ignore
them as a comment", and §3.1.17 says that unrecognised `I:` directives
should be ignored; the private `!scoretk-velo-N!` and
`!scoretk-tremolo-…!` decorations are not in the decoration list of §4.14.
For LilyPond, Notation Reference §21.1.2 gives the complete list of default
`\header` fields: `workTitle`, `scoretkCreditA`, `scoretkTboxA` and the
`scoretk*FontSize` fields are not in it, and `^\markup { "scoretk-head-…" }`
is ordinary text markup (§8.2.1), not a note-head style (§1.4.1). LilyPond
accepts all of them without warning, but a different tool does not
reconstruct the model facts they carry.

The measurement below removes those private carriers **from the written
files before re-importing them**, on the same 56 MuseScore scores, with the
same declared layers/kinds and the same `layer_coverage` test logic. It was
run with a temporary, uncommitted harness; no library code, snapshot or
percentage definition was changed. The strip removed 211 private ABC items
(`%%scoretk …` lines and `!scoretk-…!` decorations) and 267 private
LilyPond items (non-standard `\header` fields and `scoretk-…` markups), with
zero residual `scoretk` occurrence in the stripped files.

| Direction | Declared | With private extensions | Without private extensions | Items | Essential (unchanged) |
| --- | --- | ---: | ---: | ---: | ---: |
| abc | L0–L4 | 95.24 % | **92.26 %** (4 493/4 870) | 4 870 | 98.92 % |
| lilypond | L0–L5 | 95.15 % | **91.84 %** (5 399/5 879) | 5 879 | 99.27 % |

The private extensions are worth 2.98 points for ABC and 3.31 points for
LilyPond; without them neither direction reaches 95 %. The musical core is
unaffected (essential stays 98.92 % / 99.27 %): what is lost is instrument
identity, dynamic velocities and tremolo kinds on the ABC side, and text
styles, credits/frame texts, exact note-head groups and tremolo kinds on the
LilyPond side. The "reached" labels of §11.6 therefore describe faithful
**scoretk round trips**, not interoperability with other ABC/LilyPond tools:
the honest interoperability figures are 92.26 % and 91.84 %.
