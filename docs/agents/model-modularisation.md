# Pivot model: layer audit and modularisation proposal

Status: **analysis only — no code change in this commit**. Audience: whoever
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
     `src/gabc/gabc-score-determination.y`) and gabctk tag `v240107`
     (`40b58b62`, GPL-2.0-or-later);
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

- **Source evidence (now local, §1.4):** `docs/TODO.md:33-35` chooses
  `jperon/gabctk` for the mapping and Gregorio's grammar for syntax. Both are
  now present: `gabc/gabctk` at tag `v240107` (`40b58b62`) and Gregorio at
  `b63f074`. The normative syntax is `gabc/gregorio/doc/Gabc.tex:21-742`:
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
  L6 payload back verbatim when present.

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
