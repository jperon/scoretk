# MIDI (`scoretk-midi-common`, `scoretk-midi-import`, `scoretk-midi-export`)

The MIDI format is split across three crates with no cross dependency between the
two directions:

- `scoretk-midi-common`: shared error and result (and nothing else);
  the error is an `Error` that must remain identical in both directions.
- `scoretk-midi-import`: MIDI reading (`read_midi`, port of `midiimport/`),
  with SMF types (`RawEvent`, `MidiTrack`, `MidiFile`, `EventKind`,
  meta-events and controllers) and in-memory reading of an SMF in
  `midifile.rs`. No conversion to the model in `midifile.rs`.
- `scoretk-midi-export`: writing a **Standard MIDI File format 1** from
  the model of `scoretk-core`, rendered as a performance (repeats unfolded, notes,
  velocities, tempo), then encoded.

The two business crates depend on `-common`; neither depends on
its counterpart as a library.

## API

```rust
use scoretk_musescore_import::read;
use scoretk_core::MsczContainer;
use scoretk_midi_export::write_midi;

let container = MsczContainer::from_bytes(&mscx)?;
let score = read::read_score(&container)?;
let midi: Vec<u8> = write_midi(&score)?;
```

- `write_midi(&Score) -> Result<Vec<u8>, Error>`: MuseScore's default
  settings (repeats unfolded).
- `write_midi_with(&Score, MidiOptions)`: `expand_repeats: false` produces a
  "flat" file, without unfolding repeats (`RepeatList::flatten`).
  The option is exposed by the facade (`Options::midi.expand_repeats`) and by
  the CLI (`--no-expand-repeats`).
- `write_midi_with_timemap(&Score)` and
  `write_midi_with_timemap_and(&Score, MidiOptions)` additionally return the
  time map of the same rendering: `(Vec<u8>, Timemap)`. The MIDI bytes are
  identical to those of `write_midi`/`write_midi_with`.
- `RepeatList` / `RepeatSegment` are public: repeat, volta and jump
  unfolding can be reused by a future MIDI reader or a LilyPond/ABC export.
- `Timemap` (`timemap.rs`): `division`, `ticks`, `tempo`, `measures`, `notes`,
  `pauses`, `time_us_at`, `measure_at`. The `xml_id()` identifiers come from
  `scoretk_core::model::stable_ids`, as in MusicXML export. Full structure,
  pass semantics and limits: `docs/agents/timemap.md`.

All three crates are portable like `scoretk-core`: no disk access,
no threads, no clock; they compile for `wasm32-unknown-unknown`.
The only added third-party dependency is `midly` (`no_std`, `alloc`), under
the Unlicense, for the file format (`-import` for reading, `-export` for
writing).

## Modules

### Common (`crates/scoretk-midi-common/src/`)

| Module | Role | Upstream C++ |
| --- | --- | --- |
| `error.rs` | `Error` and `Result` shared by both directions | — |

Nothing else: a `-common` contains only what both directions use
(`docs/agents/developpement.md` § 2). SMF reading is used only by
the import and therefore lives in `scoretk-midi-import` (see below).

### Writing (`crates/scoretk-midi-export/src/`)

| Module | Role | Upstream C++ |
| --- | --- | --- |
| `lib.rs` | entry point and options | `exportmidi.cpp` (`ExportMidi::write`) |
| `repeat_list.rs` | repeat/volta/jump unfolding | `engraving/dom/repeatlist.cpp` |
| `render.rs` | score → time-stamped events | `engraving/compat/midi/compatmidirender*.cpp` |
| `velocity.rs` | dynamics and hairpin velocities | `engraving/compat/midi/velocitymap.cpp` |
| `pause_map.rs` | section pauses (ticks + tempo) | `engraving/compat/midi/pausemap.cpp` |
| `writer.rs` | SMF assembly via `midly` | `MidiFile` writing |
| `timemap.rs` | ticks ↔ times map, measures, notes, pauses | by-product of rendering (no dedicated C++) |

Division: **480 ticks per quarter note** (`Constants::DIVISION`), as in MuseScore.
The model's internal ticks (fraction of a whole note) are converted to
`1920 × fraction`, with the same integer truncation as `Fraction::ticks()`.

## Rendering: what is ported

- **One MIDI track per staff**, with the part name; channel, program,
  volume and pan from the instrument (volume 100, pan 64 by
  default); initial controllers (reset, bend RPN, program, volume,
  pan, reverb, chorus) on the first staff of each part.
- **Repeats**: `RepeatList::unwind` (simple repeats and `times`, voltas with
  endings, D.C./D.S., `start`/`end`/`segno`/`coda`/`fine`/`codab` targets, section
  jumps). A measure after the end of a repeat stays attached to the last
  segment, as in the C++.
- **Section jumps**: a 3 s pause (`Sid::sectionPause`) inserts a
  measure's worth of ticks and two tempo changes, in accordance with `PauseMap`.
- **Tempo**: default tempo 2 beats/s (500 000 µs) then `<Tempo>` directions,
  unfolded per segment; the time signature and key signature are
  written as meta-events, also unfolded.
- **Velocities**: full `VelocityMap` (dynamics, hairpins, collisions,
  interpolation), default value 80.
- **Notes**: concert pitch, velocity from the dynamics and hairpin map,
  × articulation multipliers (0.6 for a ghost note),
  then **`Note::customizeVelocity` per note**: in `offset`
  mode (`<veloType>offset</veloType>`) the computed velocity is increased by
  `userVelocity` percent (`velo + velo * pct / 100`); in `user` mode (the
  default value) `<velocity>` is the absolute velocity; the result is clamped to
  1–127 (`compatmidirenderinternal.cpp:227-229`,
  `dom/note.cpp:2753-2760`). Effective duration divided by `timeStretch` and
  n-tuplet ratios (`DurationElement::actualTicks`), note-off at
  `start + duration - 1`.
- **Merged tied notes**: a single attack, the duration summing the tied
  notes while accounting for their own after-grace notes
  (`calculateTieLength`).
- **Grace notes**: before-appoggiaturas (at most 50% of the chord),
  before-acciaccaturas (at most half the previous chord), after-grace notes
  (50%, 66.7% or 57.1% depending on dots).
- **Articulations**: velocity and hold duration from the instrument's MIDI
  table (`<Instrument><Articulation>`, read into the model) or, as
  a fallback, MuseScore's built-in values (`staccato` 50%,
  `staccatissimo` 30%, `portato`, `tenuto`, `accent` 1.2, `marcato` 1.44,
  `sforzato` 1.69). Under a slur, the default duration remains
  100%.
- **Tremolos**: single-note tremolo (repetition at the tremolo note value) and
  two-note tremolo (alternating the two chords); the unmeasured
  version is ignored, as in MuseScore (`buzz roll` not rendered in MIDI).

## Known discrepancies and limits

Measured on the reference subset (`tests/corpus/midi/`): **21/21
identical files** (notes and effective tempo changes; `v3/repeat06`
and `v4/repeat65/66/68/69`, including voltas and jumps). The following points
remain out of scope, for lack of information in the model:

- **Volta closure**: at `RepeatEnd`, the port now closes **every**
  still-open volta, without condition on its end measure, and a jump closes
  the volta ending before it (`repeatlist.cpp:581-588` and `:525-543`):
  a volta overrunning the `endRepeat` ends on that measure, as in
  the C++. Covered by `repeat_list::tests::volta_debordant_le_repeat_end`.
- **Fermata and breath pauses**: a fermata does not move the tick grid in
  the 4.7.4 MIDI export (its `timeStretch` is a tempo change, not inserted
  ticks; the reduced `fermata_no_stretch` test still holds). A breath **does**
  when it carries `<pause>`: `Score::rebuildTempoAndTimeSigMaps`
  (`score.cpp:531-545`) puts the seconds in the tempomap and `PauseMap`
  inserts `quarterNotesPerMeasure` ticks at `quarterNotesPerMeasure / pause`
  (see the follow-up section below; `El_Choclo` has a 0.5 s caesura). A breath
  without `<pause>` (the reduced test) still adds nothing. The old reading
  that all breath pauses were absent came from the unported playback module;
  the 4.7.4 sources show the tempomap path.
- **Gradual dynamics embedded in dynamics** (`fp`, `sfz`…): the velocity
  decrease after the attack is not modelled; the initial velocity is
  correct.
- **Dynamics rendered as CC** (`singleNoteDynamics`): continuous
  controllers (CC2/CC11) are not emitted; only attack velocities are.
- **Hairpins with explicit `<veloChange>`**: the value is ignored (the
  model does not store it); corpus hairpins rely on the next dynamic.
- **`Note::velocity` (MIDI import) not applied**: MIDI rendering uses the
  velocities from dynamics and hairpins, then MuseScore's user velocity
  (`Note::user_velocity`, `.mscx` `<velocity>`/`<veloType>`), not
  the raw velocity from MIDI import, which serves MusicXML export. The
  time map carries no velocity.
- **Two-chord tremolo**: in `compatmidirender.cpp:364-440`, the
  events of the second pitch are placed in the **first**
  chord's list and `playNote` receives the first chord's note. Only the velocity of
  the **first note of the first chord** drives both attacks; the second
  is never read. The port is faithful to this behaviour.
- **Instrument changes, rehearsal marks, pedals**: not exported.
  Lyrics, however, are written in `FF 05` as in `exportmidi.cpp` (see
  "SMF fidelity" below); markers remain unexported.
- **Percussion instruments**: no drum table (`<Drumset>` not
  modelled) and no automatic channel 9.
- **`playRepeats` of a jump**: not modelled, treated as false (the
  default of MuseScore and of the corpus files).
- **Multi-rest measures**: the content copied by the reader is played; the
  `MeasureRepeat` logic of `renderStaff` is not replayed.
- **Files with linked staves** (`linked-ties`): the model does not duplicate
  linked staves; export follows the actually modelled content.

## SMF fidelity (documented decisions)

These points concern the SMF format itself, not musical rendering. They are
measured by reduced tests building the SMF bytes with `midly` (no
binary fixture added).

- **Minor flag `FF 59`**: the reader reads `mi` (`0` major, `1` minor) and
  stores it in `KeySig::mode`, as in MusicXML import. The writer restores it
  from the model. MuseScore 4.7.4 does otherwise: `processMeta`
  (`importmidi.cpp`) reads only `sf`, and `writeHeader` (`exportmidi.cpp`) always writes
  `0 /* major */`. The port deliberately diverges so as not to lose
  the mode of a read SMF. Verified by `smf_key_signature` (reading) and
  `key_signature_export` (writing).
- **SMPTE division**: `frames/s × ticks/frame` (29.97 rounded for
  30 drop-frame) gives ticks per second. `toMuseScoreTicks`
  (`importmidi_fraction.cpp`) handles them with `ReducedFraction::fromTicks`,
  i.e. a fixed equivalence without a tempo map; the port does
  the same but divides by the declared division (960 MuseScore ticks per second at
  120 BPM). An SMPTE tempo map would be a separate feature.
  Verified by `smf_smpte` (30 and 29.97 frames/s, plus an unchanged metre case).
- **RP-017 lyrics**: `parse_rp017` recognises the empty event as a
  melisma (cumulative duration on the previous syllable), the final hyphen as
  `Begin`/`Middle` then `End`, and ignores the "on" flag at tick 0. MuseScore
  (`importmidi_lyrics.cpp`) sets each text to `Single` and skips empty events:
  the port follows the standard. RP-026 (commands `
`, character sets,
  ruby) remains undecoded. Verified by `smf_lyrics` on two measures with eight
  notes, pitches and durations included.
- **Format 2**: tracks are independent patterns; `MidiFile::read`
  offsets each track by the end of the previous one before conversion. MuseScore
  rejects format 2 (`midifile.cpp`: `midi file format 2 not implemented`);
  the port accepts it. Formats 0 and 1 keep their usual overlapping.
  Verified by `smf_format2` (three patterns, five notes, two measures, plus the
  unchanged format 1 case).
- **Lyrics on writing**: unlike the port's earlier state,
  MuseScore 4.7.4's `exportmidi.cpp` does emit the model's lyrics in
  `FF 05`, with a trailing space added only to `Single`/`End` syllables.
  The port follows this behaviour, including across repeats. Verified by
  `lyrics_export` (four quarter notes, three syllables, pitches and onsets
  unchanged, no `FF 05` without lyrics).

## SMF item declaration audit (2026-11-25)

User decision: MIDI "full support" means 100 % of what the SMF itself can
carry, not 100 % of what a score would like to store. Measures, barlines,
credits and layout stay **out of the denominator** (`Capabilities::with_kinds`,
§11 of `docs/agents/model-modularisation.md`). Every declared kind below is
cited from the local normative text (`format-ref/midi.md` §"Source key"):

| Inventory kind | SMF carrier | Citation |
| --- | --- | --- |
| `time-sig` | `FF 58 04 nn dd cc bb` | `[SMF §Meta-Events]` |
| `key-sig` | `FF 59 02 sf mi` | `[SMF §Meta-Events]` |
| `note` | Note On/Off, note number, velocity | `[MIDI §Channel Voice; Table II]` |
| `rest` | silence between Note Off and the next Note On (no rest object) | `[SMF §Track Chunk]` |
| `lyric` | `FF 05`, one event per syllable | `[SMF §Meta-Events; RP-017]` |
| `staff-text`, `system-text` | `FF 01 len text` ("Text event", types `01`–`0F` reserved for text) | `[SMF §Meta-Events]` |
| `rehearsal-mark` | `FF 06` marker, `FF 07` cue point | `[SMF §Meta-Events]` |
| `tempo` | `FF 51 03 tttttt` | `[SMF §Meta-Events]` |
| `title` | `FF 02` copyright, `FF 03` track name, `FF 04` instrument name, `{#TITLE=}` in RP-026 text | `[SMF §Meta-Events; RP-026]` |
| `instrument` | program change plus track/instrument names (SMF has no MuseScore instrument id) | `[MIDI §Program Change; GM §Sound Set]` |
| `program`, `channel`, `volume`, `pan`, `velocity` | channel status nibble, `C0`, `CC 7`, `CC 10`, note-on data byte | `[MIDI §Channel Voice; GM §Requirements]` |

The previous declaration listed `marker`, but the inventory's `marker` is a
**segno/coda repeat marker** (L1). `FF 06` is a rehearsal mark / section name
(`[SMF §Meta-Events]`), and SMF has no repeats (`[SMF §Instruments, parts,
repeats]`); the declaration now uses `rehearsal-mark` (L3) instead. The audit
also **added the missing `FF 01` text** (`staff-text`, `system-text`) and
`FF 06`/`FF 07` (`rehearsal-mark`). Excluded with a citation: `measure`,
`barline`, `clef`, `volta`, `tuplet`, `tie`, `slur`, `tremolo`, `credit`
(`[SMF §Container layout]`, `[SMF §Instruments, parts, repeats]`) and
`dynamic-velocity` (SMF has attack velocity only). The core test
`midi_declaration_follows_smf_spec` pins both sides of the list, and
`midi_full_loss_table` prints the full per-kind loss table (diagnostic only).

Before the audit (14 kinds) the 56-score round trip scored **73.51 %**;
after adding four spec-carried kinds and correcting `marker` it scores
**72.71 %** over 2 550 items — a lower percentage with a *more honest*
denominator, not a regression of any output byte. Import alone is
51.68 % over the 21 MuseScore reference MIDIs. The complete loss table:

| Kind | Layer | Round trip | Import alone |
| --- | --- | ---: | ---: |
| `note` | L2 | 284 | 53 |
| `rest` | L2 | 200 | 35 |
| `title` | L3 | 60 | 22 |
| `key-sig` | L1 | 37 | 5 |
| `staff-text` (`FF 01`) | L3 | 24 | — |
| `instrument` | L4 | 23 | 7 |
| `time-sig` | L1 | 17 | 7 |
| `tempo` | L3 | 14 | 10 |
| `lyric` | L3 | 13 | — |
| `program` | L4 | 9 | 5 |
| `system-text` (`FF 01`) | L3 | 8 | — |
| `rehearsal-mark` (`FF 06`) | L3 | 7 | — |

The `note`/`rest` losses are addressed by the additive `smf_exact_ticks`
import option (§"Full SMF support measured with options"); the text/marker
losses by the additive `smf_text` export option. Both keep MuseScore 4.7.4
behaviour as the default.

## Full SMF support measured with options

### Additive exact-tick import (`smf_exact_ticks`)

`ImportOptions::smf_exact_ticks` (default `false`) is an additive extension:
`read_midi` and the default facade/CLI/wasm path stay byte-for-byte the
MuseScore 4.7.4 importer. When enabled:

- onsets are quantised on the **source tick grid** (`1/1920` of a whole note,
  i.e. one MIDI tick at 480 PPQ) instead of the mandatory sixteenth grid;
- note-offs are still quantised on a 1/32 grid (a written duration is a
  notation figure, not an SMF field), and the `end-1` tick of MuseScore's
  `exportmidi.cpp` is compensated by one tick;
- n-tuplets are not rewritten and durations are not simplified; a performance
  duration is kept in `ChordRest::ticks` rather than chained into binary
  figures (which would swallow the following attacks);
- human-performance beat detection is disabled.

The option is exposed by `scoretk::read_midi_with` (facade), by the CLI flag
`--midi-exact-ticks` and by the additive wasm `midiExactTicks` property. It is
measured by `midi_exact_ticks_full_support` in `crates/scoretk/tests/layer_coverage.rs`
on the same 56 MuseScore scores, with the same writer, layers and kinds:

| Reader / writer | Essential | Full support | `note` loss | `rest` loss |
| --- | ---: | ---: | ---: | ---: |
| default import, default writer | 83.41 % | 72.71 % | 284 | 200 |
| `smf_exact_ticks`, default writer | **83.66 %** | **73.14 %** | 278 | 195 |
| default import, `expand_repeats = false` | 84.57 % | 73.88 % | 257 | 200 |
| `smf_exact_ticks`, `expand_repeats = false` | **84.83 %** | **74.31 %** | 251 | 195 |
| `quant_value = Q1024` alone (finer grid only) | 83.15 % | 72.39 % | 290 | 202 |

A finer grid alone is not enough: it moves onsets without recovering the
duration chain. The exact-tick path gains 6 `note` and 5 `rest` matches on the
default writer and 6 + 5 on the flat one. Both defaults and the 21/21 MIDI
reference exports, the 21/21 round trip, the 4/21 import corpus and
`e2e_corpus` 8/8 remain unchanged; the option is opt-in.

The remaining gap is not a missing SMF writer feature: a score stores *written*
ticks while an SMF stores a *performance* (repeats unfolded, gate times,
tuplets as tick arithmetic — `[SMF §Instruments, parts, repeats]`). The two
small measurements of §11.6 of `model-modularisation.md` still hold: flat
output gains 27 notes and projecting the reference through `RepeatList::unwind`
gains 44. Exact tick recovery can only close the quantisation part of that
gap; it cannot map a performed attack back to a written segment that SMF does
not identify.

## References and compliance test

`scripts/gen-midi-references.sh` exports a short subset of the corpus with
MuseScore **4.7.4** (headless, `QT_QPA_PLATFORM=offscreen`) into
`tests/corpus/midi/{v3,v4}/<nom>.ref.mid`. The test
`crates/scoretk-midi-export/tests/corpus_midi.rs` reads the `.mscx` source, writes the MIDI
and compares after normalisation:

- notes: track, channel, pitch, start tick, end tick, velocity;
  the order of simultaneous events is neutralised and note-on/note-off
  are paired;
- tempo: effective changes only (MuseScore sometimes rewrites the
  tempo in force at a repeat boundary).

The test prints `MIDI : N/M identiques` and fails only on
`EXPECTED_IDENTICAL`, which must not regress.

```sh
scripts/gen-midi-references.sh            # all references of the subset
scripts/gen-midi-references.sh tests/corpus/v4/dynamics.mscx
nix develop -c cargo test -p scoretk-midi-export --test corpus_midi -- --nocapture
```

Do not regenerate with another version of MuseScore without an explicit decision:
the compliance target would change silently (same rule as the MusicXML
references, see `upstream.md`).

## MusicXML → MIDI chain

`crates/scoretk-midi-export/tests/musicxml_midi.rs` checks the full
MusicXML → model (`scoretk_musicxml_import::read_musicxml`) → MIDI chain: for
each `tests/corpus/{v3,v4}/X.ref.musicxml`, it writes the MIDI and compares it to
`tests/corpus/midi/{v3,v4}/X.ref.mid` with the same normalisation as
`corpus_midi.rs` (notes and effective tempos). This guarantees that the
dynamics and tempos read from `<direction>` produce MuseScore's velocities and
tempo changes.

```sh
nix develop -c cargo test -p scoretk-midi-export --test musicxml_midi -- --nocapture
```

Result as of 2026-11-17: **7/21 identical** (`v4/dynamics`,
`hairpin_simple`, `linked-ties`, `mmrest`, `repeat68`, `repeat69`,
`tenuto_accent`). The remaining discrepancies come from instruments' hold
duration (`gateTime`), which MusicXML does not convey, and
articulations not read for some files.


## MIDI reading (import, t42)

The crate exposes reading a Standard MIDI File into the pivot model:

```rust
let score = scoretk_midi_import::read_midi(&bytes)?;          // MuseScore options
let score = scoretk_midi_import::read_midi_with(&bytes, &options)?; // explicit options
```

The `scoretk` facade wires it with the `midi-import` feature (writing has its
own `midi-export` feature):
`read(bytes, Format::Midi)` and `scoretk input.mid -o output.musicxml`
work. The facade's wasm package exposes them too.

### Modules

Port of `importexport/midi/internal/midiimport/` (same names and breakdown
as the C++, upstream reference in each header):

| Rust (`crates/scoretk-midi-import/src/`) | Upstream C++ |
| --- | --- |
| `lib.rs` | `importmidi.cpp` (`convertMidi`, `createMTrackList`, `createMeasures`, `createNotes`, `processMeta`) |
| `midifile.rs` | `midishared/midifile.cpp`, `midievent.h` (SMF events, `MidiFile::read`, channel separation, note-on/off merging, `to_muse_score_ticks`, by `midly`) |
| `timesigmap.rs` | `engraving/dom/sig.cpp` (`TimeSigMap`) |
| `chord.rs` | `importmidi_chord.cpp` (`MChord`) |
| `quant.rs` | `importmidi_quant.cpp` (`Quantize`) |
| `meter.rs` | `importmidi_meter.cpp`, `engraving/dom/durationtype.cpp` (`toDurationList`) |
| `simplify.rs` | `importmidi_simplify.cpp` (`lengthenNote`, `minimizeNumberOfRests`) |
| `tuplet.rs` | `importmidi_tuplet*.cpp` (simplified detection, see limits) |
| `voice.rs` | `importmidi_voice.cpp` (overlap separation, pitch sorting) |
| `key.rs` | `importmidi_key.cpp`, `engraving/dom/keylist.h` |
| `clef.rs` | `importmidi_clef.cpp` (main clef only) |
| `lrhand.rs` | `importmidi_lrhand.cpp` (simplified hand separation) |
| `drum.rs` | `importmidi_drum.cpp` |
| `lyrics.rs` | `importmidi_lyrics.cpp` |
| `tempo.rs` | `importmidi_tempo.cpp` |
| `instrument.rs` (+ `instrument/gm_names.rs`) | `importmidi_instrument.cpp`, `midishared/generalmidi.cpp` |
| `operations.rs` | `importmidi_operations.h` (default values) |

The pipeline follows `convertMidi`: tracks and time-signature indications, chords
(`collectChords`), merging and splitting, left/right hand separation, percussion,
quantisation (onsets then ends), removal of overlaps,
duration simplification, voice separation, measures, meta-events,
key signatures, notes and rests, model n-tuplets, clefs, time signatures,
lyrics, tempo.

Default options (no options panel): quantisation to the **sixteenth
note** (`QuantValue::Q16`, `defaultQuantValueFromPreferences`), search for
3/5/7/9-tuplets, continuation dots, staccato, four voices, piano hand
separation, clef changes, tempo text, pickup search.
Like MuseScore on an already-quantised performance, onsets stay on
the grid; a human performance is detected (fewer than 60% of onsets on
a beat) and quantised to the eighth note, without beat detection.

The import adds `Note::velocity` to the model (raw MIDI velocity); MusicXML
export derives the note's `dynamics` attribute from it (`velo * 100 / 90`, MuseScore's
scale).

### Measured limits

- **N-tuplets**: detection handles only simple n-tuplets whose grid
  `length / n` is not binary (3, 5, 7, 9), over the measure's metric divisions,
  with no foreign note between two onsets. Nested n-tuplets,
  ones overlapped by another voice or with a binary grid (duplets, quadruplets)
  are not detected: `v3/testTuplets8` and `v4/timesig-09` remain
  partially different from the references (see below).
- **Instrument catalogue**: names come from the General MIDI table
  (`getMidiName`) rather than MuseScore's catalogue search
  (`findSuitableInstruments`). Program 0 is called "Grand Piano" instead of
  "Piano", program 25 "Steel Gtr." instead of "Acoustic Guitar",
  and short names (`Pno.`, `Vln.`, `Guit.`) are not produced. This is the
  cause of most 1-to-7-line discrepancies in the import test.
- **Clefs**: only the main clef (average pitch, threshold 60) is placed;
  the dynamic programming of clef changes is not ported, and octave
  clefs from the catalogue (guitar, double bass) are not inferred.
- **Hand separation**: the split decision is the C++'s
  (chord exceeding an octave, outside an already present grand staff); note
  allocation uses the largest pitch gap, without the
  dynamic programming of `LRHand::findSplits`.
- **Voices**: `MidiVoice::separateVoices` is ported literally (see the
  section at the end); `MidiTuplet::setNonTupletVoices` is approximated (the
  chord interval starts at its on time, `findOnTimeBetweenChords` is not
  ported).
- **Percussion**: the drum table is not embedded; only the
  voice-1 pitches of the standard kit (35, 36, 44) are known, and splitting
  by pitch (`splitDrumTracks`) is not exercised (`doStaffSplit` false).
- **Human performances**: no `MidiBeat` (beats, average tempo);
  only explicit tempos are placed.
- **Pickup**: only the first measure shorter than the next is
  recognised (`isPickupWithLessTimeSig`); the "first measure longer" case
  is not handled.
- **Unequal chords, grace notes, tremolos, swing, chord names**: the modules
  `importmidi_chordname`, `importmidi_swing`, `importmidi_beat`,
  `importmidi_tie`, `importmidi_operations` (interface) are not ported.

### References and compliance

`scripts/gen-midi-import-references.sh` has each
`tests/corpus/midi/{v3,v4}/<nom>.ref.mid` imported by MuseScore **4.7.4** (headless,
`QT_QPA_PLATFORM=offscreen`, `LD_LIBRARY_PATH` neutralised) and exports the
result to `tests/corpus/midi/import/<version>/<nom>.ref.musicxml`.
`crates/scoretk-midi-import/tests/import_corpus.rs` reads the MIDI with `read_midi`,
exports to MusicXML and compares the **musical core** with the same normalisation
as `crates/scoretk/tests/e2e_corpus.rs` (`tests/core_norm.rs`, plus
normalisation of `<divisions>`: the value is an encoding convention,
`<duration>`s are reduced to 480 per quarter note).

Measured result as of 2026-11-17: **4/21 identical** (`v4/mmrest`,
`v4/repeat68`, `v4/repeat69`, `v4/testTempoPrecision`, all files with
no or almost no notes). The other files differ by 1 to 7 lines (part names
and abbreviations, octave clef) except `v3/testTuplets8` (224 lines) and
`v4/timesig-09` (965 lines), because of complex n-tuplets. The
`EXPECTED_IDENTICAL` list must not regress.

The `.mscx` → MIDI → `read_midi` → MIDI round trip is checked by
`crates/scoretk-midi-export/tests/roundtrip_midi.rs` on the 21 sources: **21/21**
with the same pitches, onsets within ±1/16 of a quarter note and durations within ±1/16,
except notes shorter than the grid, which n-tuplet simplification may
lengthen (documented in the test).

```sh
scripts/gen-midi-import-references.sh            # all references
scripts/gen-midi-import-references.sh tests/corpus/midi/v4/dynamics.ref.mid
nix develop -c cargo test -p scoretk-midi-import --test import_corpus -- --nocapture
nix develop -c cargo test -p scoretk-midi-export --test roundtrip_midi -- --nocapture
```

## MIDI import: measured state (2026-11-22)

Measured with `scripts/oracle/midi/import_mesure.py` on the two lots of
phase 4 of `plan-fiabilite.md`:

- **upstream** — the 96 MuseScore tests that have a `.mid` and its expected
  `-ref.mscx` (`importexport/midi/tests/midiimport_data`, 4.7.4 reference);
  each test is replayed with the options `midiimport_tests.cpp` pins
  (`dontSimplify`, `staffSplit`, `voiceSeparation`, `simplification`, and the
  `canRedefine` values of `staff_split_tests`).
- **LilyPond** — the 1 020 MIDI files of `~/Musique` that have their `.ly`
  source; the reference is that source read by **our own LilyPond reader**
  (model against model, the same binary), with `--midi-no-simplify`: the
  written durations are not simplified, and `simplifyDurations` would
  legitimately lengthen them. A source that uses `\repeat`/`\unfoldRepeats`
  is excluded: LilyPond's MIDI is unfolded, the written source is not
  (`ly_notes.a_des_repetitions`, the rule of the LilyPond oracle); 87 pairs
  are excluded for this reason.

Two normalisations make the comparison mean the same thing on both sides:

- **concert pitch**: a MIDI carries only the sounding pitch, and the import
  invents an instrument (hence a `<transpose>`) from the General MIDI program;
  both sides are brought back with `<transpose>` applied
  (`notes(to_concert=True)`), as already done for the `.mscx` references.
- **one attack per (pitch, onset)**: two voices doubling a pitch cannot be
  notated twice in a chord — `MChord::collectChords`/`hasNotesWithEqualPitch`
  drops the second and `removeOverlappingNotes` keeps the longest
  (`importmidi_chord.cpp:176-240, 391-465`). The reference is deduplicated the
  same way, keeping the **longest** duration.

Voice and duration are compared only against the `.mscx` references (an
import of MuseScore itself). Against a `.ly` source they are not a truth of
the MIDI: voices are inferred from overlaps (`importmidi_voice.cpp`) and
LilyPond writes performance note-offs (gate time, articulations, irregular
measures), so the source-notes comparison is pitch + onset.

| Lot | Files | Exact | N1 % | N2 % | N2d % | N2v % |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| upstream `.mscx` | 96 | 95 (99,0 %) | 100,00 | 99,88 | 99,88 | 99,88 |
| LilyPond `.ly` (87 excluded) | 932 | 917 (98,4 %) | 99,98 | 99,89 | — | — |
| all comparable | 1 028 | **1 012 (98,3 %)** | **99,98** | **99,89** | — | — |

Percentages are weighted by the reference notes (100 028). On the `.mscx` lot
the exactness includes voice and duration; on the `.ly` lot it is
pitch + onset (see above). N2d/N2v are not aggregated outside the `.mscx`
lot. The one file with an empty reference (`LaRondeDeNuit`) is not counted
exact. Before the oracle corrections below, the same binary scored
682/1 116 on the raw comparison (voice required against the `.ly` sources,
written pitch, duplicate unisons counted, repeats not excluded): that figure
measured the oracle, not the importer.

### Update 2026-11-23 — normalisation symétrique des liaisons

With the frozen binary of the human-performance fix and the same references,
`import_mesure.py` now measures **1 026/1 029 exact** (upstream 96/96,
LilyPond 930/933), N = 0,9991. The 87 deliberately unsupported pairs stay
outside both numerator and denominator (`plan-fiabilite.md` § 1, rule 7).

A MIDI carries only note-on/note-off pairs: it cannot distinguish a written
tie from two consecutive attacks of the same pitch. Depending on the source,
the same sound is notated either as one long note or as two touching
fragments. `midi_notes.reconcilier_chaines` now applies the **same rule to
both lists before matching**: a pair of same-pitch/same-voice notes whose
second onset is the first note's end is coalesced only when the other side
has a single note covering the whole span **and no attack at the second
onset** (duration match within half the tolerance, to avoid confusing a tie
with a repeated note in another voice). Graces (zero duration after grid
rounding) are never coalesced. Selftest: `scripts/oracle/midi/test_chaines_import.py`.
Before this normalisation the same binary scored 1 014/1 029; the 12 gained
files are the tie-related `lily-*` pairs and `LeCheminDeSaintJacques`, with
no loss.

Three files remain inexact and **stay in the denominator**: `plan-fiabilite.md`
allows exclusion only for deliberately unsupported inputs (exit code 3) and
for unusable `.ly` references, not for a reference that disagrees with its
own MIDI.

- `ly-Dechiffrage`: the `.ly` generates music with Scheme (`$(let …)`) that
  our LilyPond reader does not evaluate (16 written notes against 68 played).
- `ly-Veni, Creator Spiritus_Courtial_v3_SATB`: several `\score` blocks; the
  MIDI is compiled from another one than the source the reader keeps
  (245 against 1 619 notes).
- `ly-LaRondeDeNuit`: empty reference (0 notes), never counted exact by design.

### Non-exact files by first divergence

`import_mesure.py` matches on the strictest field set and reports the earliest
unmatched note with the finest cause: `note sup` (extra note),
`attaque` (same pitch, onset moved), `octave`, `voix`, `durée`,
`note manquante`, `aucune` (empty reference).

| First divergence | Files | Example |
| --- | ---: | --- |
| `attaque` | 7 | `up-human_4-4` |
| `note sup` | 8 | `ly-lily-30ec2d9a` |
| `octave` | 1 | `ly-Lassus_Alleluja._Dies_sanctificatus` |

`ATiTeCantamos` (55 extra notes) is fixed by the **oracle**: the file defines
rehearsal `\book` blocks with `\unfoldRepeats` after its main score, and
`sans_unfold` checked the whole text, so the writer expanded the main score's
`\repeat volta 2` (200 notes) where LilyPond's reference plays it once (145).
`sans_unfold` now judges the **first `\score`** (the one the reader keeps,
`premier_score`); test `test_sans_unfold.py`. 898 → 899 exact.

Two reader fixes of this pass show up here because the `.ly` references are
converted to MusicXML by the same binary: `Kryshachok` (`\chordmode` octave
marks, Cause 43 of `lilypond.md`) and `02_01_CommentLeSaisJe` (`a:7m`, Cause
44) become exact, 1 008 → **1 010/1 028**, no per-file loss and no N1/N2/N2v
regression. `Lassus_Alleluja` stays `octave` (inconsistent `.ly`/MIDI pair,
documented below).

Three causes explain almost all the rest, and two are measurement gaps rather
than importer defects:

- **per-test options** (fixed here): `quant_dotted_4th` runs with
  `QuantValue::Q_4`, `tuplet_2_voices_3_5_tuplets` with `Q_32`
  (`midiimport_tests.cpp:230-243, 349-357`; `Quantize::quantForLen`,
  `reduceQuantIfDottedNote`, `importmidi_quant.cpp:155-190`), and
  `tuplet_3_5_7_tuplets` pins `changeClef = false`, `doStaffSplit = false`
  and `simplifyDurations = false` (`:366-379`). The CLI now exposes
  `--midi-quant` (`quantValue`, re-exported `QuantValue`) and
  `--midi-no-change-clef`; `upstream_test_options` parses the `TEST_F`
  bodies, so `quant_dotted_4th` becomes exact. The two tuplet tests still
  fail (see below).
- **grace notes** (paired): the reference `.ly` writes the ornament as
  `<grace>`, while a MIDI import has no grace module at all
  (`importmidi_chord.cpp` has none) and produces a normal short note before
  the beat. The oracle now pairs each reference grace with the nearest
  unmatched import note of the same pitch before the main note, as the
  LilyPond oracle does.
- **`splitFirstTupletChords`** (fixed, new module `tuplet/split.rs`): the
  first chord of overlapping tuplets is split at the same tick — the earlier
  tuplet keeps the first note, the later the rest — after sorting the notes
  by pitch and the tuplets by average pitch (`importmidi_tuplet.cpp:224-249,
  401-440`, called at `:1116-1119`). **MuseScore 4.7.4 sorts the notes
  descending** (`n1.pitch > n2.pitch`) while the 5.0.0 snapshot sorts them
  ascending: the 4.7.4 order is ported, since it alone reproduces
  `tuplet_2_voices_3_5_tuplets` (67 in the triolet, 64 in the quintuplet).
  That test is now exact, and `voix` drops from 3 to 2 files.
- **tuplet division levels** (fixed): `metricDivisionsOfTuplet`
  (`importmidi_meter.cpp:118-132`) pushes each division with the **current**
  `tupletStartLevel` and decrements afterwards (`tupletStartLevel--`), the
  caller having already pre-decremented (`:169-183`). The port decremented
  once more before the first push, so every tuplet division was one level too
  low. Tested and measured: no file changes on either lot (the level list is
  not read by the duration choice on the measured paths), so the fix is a
  faithful-port correction without a score effect.
- **the « ultra-short durations » were not a real defect**: after the oracle's
  correction, `tuplet_3_5_7_tuplets` and `tuplet_2_voices_tuplet_non` match
  the reference on duration (N2d = 18/18 and 8/8). The MusicXML
  `<duration>` is the sounding value (2/18 of a quarter in a 9:8 tuplet) and
  `<time-modification>` carries the notation, which is correct; the earlier
  « 9 ticks » came from reading `<duration>` as the notated figure.
- **`minimizeOffTimeError`** (fixed, new module `tuplet/off_time.rs`, from
  `importmidi_tuplet.cpp:454-503`): a note of the first chord that belongs to
  an ordinary voice (62 at 0.0 beside the triplet's 67 in
  `tuplet_2_voices_tuplet_non`) is moved out of the tuplet when the regular
  grid gives it a smaller off-time error, then `setNonTupletVoices` puts it
  in another voice. Test `note_hors_nolet_sur_la_meme_attaque_reste_hors_du_nolet`
  (62 was in voice 1 as a triplet eighth; now voice 2, plain eighth). That
  test becomes exact: 1 007 → 1 008 files, no loss.
- **`assignVoices` fidelity**: `chordInterval` (`importmidi_tuplet_voice.cpp:49-67`)
  now uses a faithful port of `findOnTimeBetweenChords`
  (`importmidi_tuplet.cpp:957-1041`, new `tuplet/on_time.rs`, shared with
  `voice.rs` instead of the duplicate approximation). Measured: 1 008 before
  and after, no loss — the corpus does not exercise an on-time collision
  through this path, so the fix is fidelity, not score.
- **`findAllTuplets` / `collectChords` order** (fixed): MuseScore detects the
  tuplets **before** the 4/3 `collectChords` of `quantizeAllTracks`
  (`importmidi.cpp:265-275`: `findAllTuplets`, then
  `collectChords({4,3})`, then `quantizeChords`). The port applied the 4/3
  collection first; on `tuplet_3_5_7_tuplets` it merged the 5:4 and 7:8 notes
  whose attacks are 12 ticks apart (154 and 166) into one chord at
  `193/1920`, so `findBestTuplets` (`importmidi_tuplet_filter.cpp:691-717`)
  saw them as sharing a non-first chord and kept the 9:8 instead of 3:2 + 5:4
  + 7:8. The collection now follows the detection, and the existing faithful
  `detect_tuplets`/`filter_tuplets` are also wired into `find_all_tuplets`
  (drums and the late-insertion path) instead of the old shortcut: the same
  test now finds the three tuplets and spreads the shared first chord over
  three voices. Measured: **1 010 → 1 012/1 028**, no per-file loss and no
  N1/N2/N2v regression; gains `up-tuplet_3_5_7_tuplets` and
  `ly-Ad_te_Suspiramus` (the latter was a Q16 grid artefact, now reproduced
  exactly because the chord is no longer merged before quantisation).
- **`Kryshachok`** is a **LilyPond reader** octave error: the MIDI has
  (46, 50, 53) at 4.0 and the reference written pitch is (58, 62, 65), with
  no `<transpose>` or octave clef between them — the reader resolved the bass
  voice an octave too high.
- **`Lassus_Alleluja._Dies_sanctificatus`** is the same kind of inconsistent
  pair, not an importer loss: at 64.0 the MIDI has three notes (66, 57, 50)
  while the `.ly` reference has five (74, 66, 62, 57, 50); the two extra
  written notes are exactly 12 semitones above MIDI pitches the source's
  octave-clef staves should have produced. The import reproduces the MIDI.
- **`Ad_te_Suspiramus`** (12 of 1 072) reduces to a **grid artefact**: the
  missing attacks are consecutive same-pitch notes 1/6 of a quarter apart
  (81 at 71.833 and 72.0, then 73.833 and 74.0, …). The importer keeps them
  0.125 apart (71.875 and 72.0, its `quantForLen` halving for the short
  note); the reference's written 1/6 positions quantise to 71.75 and 72.0.
  On the oracle's Q16 comparison grid both collapse to 72.0 and
  `fusionner_unissons` keeps one, while the reference keeps two — the import
  is not losing a note.
- **`human_4-4`** is a human performance, where our direct
  `findQuantizedChordOnTime` differs from MuseScore's metric dynamic
  programming (documented in `quant.rs`).

The single `note manquante` file (`CommentLeSaisJe`) and the `note sup`
files `Veni, Creator Spiritus` (1 619 notes against 245) and `Dechiffrage`
(68 against 16) are repeats not caught by `a_des_repetitions` (a `\repeat`
in a deeper include, or a `bar "|:"` written without `\repeat`); they are
oracle artefacts. The seven `attaque` files of tick 3.0 (`lily-ed3284f3` and
its copies, `LeCheminDeSaintJacques`) are the opposite: the source has
successive identical notes whose tie our **LilyPond reader** fails to merge
(the MIDI has a single attack), so the reference is wrong, not the import.

## MIDI export: measured state (2026-11-22)

Measured with `scripts/oracle/midi/mesure.py --ecriture` on the writing lots
of `scripts/oracle/midi/lots.sh`, one reference per source:

- **`ecriture_ly`** — the 1 019 `.ly` of `~/Musique` that have a LilyPond
  `.midi` next to them; the reference is that MIDI. The chain measured is
  `.ly` → our LilyPond reader → model → MIDI writer, so a reader error shows
  up here too (that is the point of this lot).
- **`ecriture_mscz_corpus`** — the 21 `tests/corpus/{v3,v4}/*.mscx` that have
  a `*.ref.mid`, i.e. MuseScore 4.7.4's own MIDI export.
- **`ecriture_mscz_musique`** — the 8 `.mscz` of `~/Musique` with a MuseScore
  MIDI beside them.

Five inputs are excluded as unsupported or empty (a `.ly` with no notes and
four corpus scores whose reference MIDI has no notes: `mmrest`, `repeat68`,
`repeat69`, `testTempoPrecision`). Notes are compared after the oracle's
normalisation (pitch, onset, duration; simultaneous events unordered;
note-on/off paired; Q16 grid, ±1/16 of a quarter), plus the effective tempo
changes.

| Lot | Files | Exact | N1 % | N2 % | N2d % | Ref. notes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `ecriture_mscz_corpus` | 17 | **17 (100,0 %)** | 100,00 | 100,00 | 100,00 | 483 |
| `ecriture_ly` | 1 019 | **899 (88,2 %)** | 97,81 | 97,65 | 96,90 | 131 736 |
| `ecriture_mscz_musique` | 8 | 3 (37,5 %) | 63,87 | 53,63 | 51,26 | 9 325 |

### Comparison rule for a writing lot (oracle)

A writing comparison puts two *performances* face to face, not two scores.
Two of MuseScore's export conventions make them differ note by note without
changing what is heard; the writing oracle now normalises both sides before
`compare` (`midi_notes.normaliser_ecriture`):

- **Cross-track note-offs.** MuseScore writes one SMF track per staff, but
  `EventsHolder::fixupMIDI` (`ref-musescore/src/engraving/compat/midi/event.cpp:263-306`,
  tag `v4.7.4`) counts the notes **per internal channel**: a same-pitch
  attack while another sounds gets `discard = originatingStaff + 1`, and
  `exportmidi.cpp:304-323` writes the restrike (a velocity-0 note-on) on the
  previous staff's track while the surviving note-off is hoisted to the last
  attack's track. A note can therefore start on one track and end on another.
  `midi_notes.read_midi` now merges the tracks **by channel** in file order
  before pairing FIFO, like a synthesiser (and like LilyPond's one channel per
  staff, `lily/staff-performer.cc:165-181`, `midiChannelMapping = #'staff`,
  `ly/performer-init.ly:257`). Reading track by track left the emitted note
  open until a much later note-off and produced durations of several measures
  (e.g. `benoit_ave_maria.ly`: a G4 of 2 quarters measured at 34).
- **Simultaneous unisons.** For the same reason, two same-pitch notes at the
  same onset sound as one. MuseScore keeps one (`fixupMIDI`) and writes the
  other as a zero-length restrike; LilyPond merges them when they start at
  the same tick of a track and keeps the longest stop
  (`lily/midi-walker.cc:94-115`, `midiMergeUnisons`,
  `scm/define-context-properties.scm:555`). The normaliser drops
  zero-duration restrikes and collapses notes with the same `(pitch, onset)`
  to the longest duration, as `fusionner_unissons` already does for the import
  direction (`importmidi_chord.cpp:391-465`).

Measured effect on the three writing lots (same binary, same manifests):
`ecriture_ly` 723 → **894** exact (+171, no file lost), `durée` 149 → 120,
`note sup` 143 → 1; `ecriture_mscz_corpus` 17/17 unchanged;
`ecriture_mscz_musique` 3/8 unchanged. The oracle's own tests
(`scripts/oracle/midi/test_same_tick.py`, `test_overlap.py`, …) pass.

### Non-exact files by first divergence

`scripts/oracle/midi/export_groupes.py` reads the JSONL, the manifests and
the `travail/<lot>/<index>/tk.mid` outputs, and classifies each non-exact
file by the first note where the output differs from the reference, using
`midi_notes.releve_import` on the same normalised lists (pitch + onset +
duration, no voice): `durée`, `note manquante`, `note sup`, `attaque`,
`octave`.

| Lot | First divergence | Files |
| --- | --- | ---: |
| `ecriture_ly` | `durée` | 119 |
| `ecriture_ly` | `attaque` | 1 |
| `ecriture_mscz_musique` | `note sup` | 2 |
| `ecriture_mscz_musique` | `attaque` | 1 |
| `ecriture_mscz_musique` | `durée` | 1 |
| `ecriture_mscz_musique` | `octave` | 1 |

### The five non-exact `.mscz` files of `~/Musique`

Classified with the same normalised sequence diff (pitch + onset + duration):

- **`Wellerman`** (ref 178, ours 704): the score has `<endRepeat>4</endRepeat>`
  and the reference `.midi` beside the `.mscz` was exported **without**
  expanding it (our `--no-expand-repeats` output has 176 notes, expanded 704).
  Export-option mismatch of the reference, not a writer cause; the `.mscz`
  does not record the export option.
- **`Veni, Creator Spiritus_Courtial_v3_SATB`** (ref 3 040, ours 249): the
  reference is a different/stale score — the sibling `.ly` had the same
  3 134-versus-249 pair, already documented in `lilypond.md` §14 as a stale
  MIDI. Reader/reference, not the writer.
- **`Laudate_pueri`** (ref 1 611, ours 1 297, N1 80,5 %): MuseScore 3.2.3
  linked staves/parts, like the rest of the `.mscz` lot; the model is missing
  notes before the writer runs.
- **`Sancta_Maria_Mater_Dei`** (ref 2 370, ours 2 420, N1/N2 100 %, N2d
  2 364): an early interpretation put the value graces *before* the beat
  (`compatmidirender.cpp:559-590` read too narrowly); a fresh MuseScore 4.7.4
  reference disproved it, see §"Value graces" below. The file is now exact.
- **`Sancta_Maria_Lux_tempestiva`** (ref 467, ours 560, N1 44,5 %): MuseScore
  4.0.1 model/reference disagreement before the writer (pitch/voice layout).

The corpus lot is exact everywhere. The 120 remaining `durée` files of
`ecriture_ly` split into two **performance conventions** and one residual
reader class:

- **overlapping same-pitch notes** (423 mismatched notes, 78 files, e.g.
  `01_01_Etudiants.ly` at q10.5: a held A2 and a second voice attacking the
  same A2 at q12). MuseScore cuts the held note at the restrike
  (`fixupMIDI`): ours reports 1.5 + 1.5 quarters; LilyPond keeps two channels
  and reports 3.0 + 0.5. The **union of the sounding intervals is identical**
  (checked on all 149 files: 423 of the mismatched notes have the same union),
  so the performance is the same. Extending each note to its union end in the
  oracle was tried: **exact zero-gap** (touch or overlap only) gains 28 files
  but loses **17** on the 1 019 `.ly` (net 899 → 882). The cause is MuseScore's
  `end-1` note-off: our notes end one tick before the next same-pitch attack,
  so a zero-gap union fuses the reference's touching notes but not ours (and
  the reverse for `\articulate` `7/8` lengths); a small gap tolerance fuses
  ours too but then hides real articulation rests, and a matching tolerance
  (`1/16`) loses 177. The rule is therefore **not** applied: the current
  normalisation (same-onset fusion only) keeps 899, and the restrike split
  stays the 4.7.4-faithful export.
- **articulations without `\articulate`** (261 mismatched notes, 42 files,
  e.g. `LaChansonDuPleinAir.ly`, staccato `-.` and no `\articulate`). LilyPond
  plays the written length (`note-performer.cc` ignores articulations unless
  `\articulate` replaces the notes, `ly/articulate.ly`); MuseScore applies its
  `gateTime` table (staccato 50 %, staccatissimo 30 %, portato 75 %,
  `dom/articulation.cpp`, `compatmidirender.cpp:263`). Ours is
  MuseScore-faithful (ratio 0.5 on those notes). The reference MIDI cannot
  represent the articulation, so the oracle cannot decide which side is
  short; the writer keeps MuseScore's behaviour.
- **reader residue** — one attack, one missing and one extra note; the chain
  `.ly` → reader is measured here on purpose. The two `octave` files
  (`Kryshachok`, `VolgaVolga`) were fixed by Cause 43 of
  `docs/agents/lilypond.md`: `\chordmode` ignored the written octave marks
  (`bes,`) and the duration that follows them.

## Upgrading MuseScore's MIDI import

Same upstream reference as the rest of the port (see `docs/agents/upstream.md`):
commit `2b9e82df847111d673c2051a93deeaaeea3f5a29` (2026-11-16, 5.0.0 development
version), local copy `ref-musescore/` **not versioned**. The
reference behaviour for import outputs remains **MuseScore 4.7.4**: in case of divergence, 4.7.4 prevails.

Where to get the sources (outside the repository):

```sh
git -C ref-musescore fetch origin
git -C ref-musescore checkout 2b9e82df847111d673c2051a93deeaaeea3f5a29
```

Files to watch (under `ref-musescore/src/`):

- `importexport/midi/internal/midiimport/importmidi.cpp` (pipeline),
  `importmidi_chord.cpp`, `importmidi_quant.cpp`, `importmidi_simplify.cpp`,
  `importmidi_meter.cpp`, `importmidi_tuplet*.cpp` (detect/filter/voice/tonotes),
  `importmidi_voice.cpp`, `importmidi_key.cpp`, `importmidi_clef.cpp`,
  `importmidi_lrhand.cpp`, `importmidi_drum.cpp`, `importmidi_lyrics.cpp`,
  `importmidi_tempo.cpp`, `importmidi_instrument.cpp`,
  `importmidi_operations.{h,cpp}` (default values);
- `importexport/midi/internal/midishared/{midifile,midievent,generalmidi}.cpp`
  and `.h` files;
- `engraving/dom/{sig,durationtype}.cpp` (measure map, `toDurationList`).

C++ → Rust correspondence: "Modules" table above. To spot the
relevant changes:

```sh
git -C ref-musescore log --oneline <ancien>..<nouveau> --   src/importexport/midi/internal/midiimport   src/importexport/midi/internal/midishared   src/engraving/dom/sig.cpp src/engraving/dom/durationtype.cpp
git -C ref-musescore diff <ancien>..<nouveau> --   src/importexport/midi/internal/midiimport/importmidi_quant.cpp
```

Sort: interface changes (panel, options) do not concern the
library; changes to quantisation, n-tuplet, simplification and tempo algorithms do. Generated tables (`typesconv.cpp`,
`generalmidi.cpp`) need manual rechecking (`getMidiName`,
`isGrandStaffProgram`).

Regenerate and revalidate:

```sh
scripts/gen-midi-import-references.sh                # with MuseScore 4.7.4
nix develop -c cargo test -p scoretk-midi-import --test import_corpus -- --nocapture
nix develop -c cargo test -p scoretk-midi-export --test roundtrip_midi -- --nocapture
nix develop -c cargo test --workspace
nix flake check
```

Do not change the version of MuseScore producing the references without an explicit
decision (same rule as `docs/agents/upstream.md`): it would change the compliance
target, `NOTICE`, `tests/corpus/SOURCES.md` and the discrepancy documents.

**When to do it**: before a new scoretk release, if upstream fixes
a quantisation or import bug, if a new MuseScore version
changes the references, or if a corpus test regresses without a local change.

## Upgrading midly (SMF format)

`midly` is the only third-party crate for MIDI format reading/writing. Ported
version: **0.5.3** (`Cargo.lock`), Unlicense, without `std`
(`default-features = false`, `features = ["alloc"]`). The project's code
depends only on `Smf::parse`, `TrackEventKind`/`MetaMessage` and `no_std`
writing; the adaptation to C++ is concentrated in
`crates/scoretk-midi-import/src/midifile.rs` (reading) and
`crates/scoretk-midi-export/src/writer.rs` (writing).

Where to get the sources: <https://github.com/kovaxis/midly> (or the
`midly` crate from crates.io, sources in the local Cargo registry). For an update:

1. change the version in `crates/scoretk-midi-import/Cargo.toml` and
   `crates/scoretk-midi-export/Cargo.toml` (and `Cargo.lock`), check that it
   stays `no_std + alloc` and compatible with `wasm32-unknown-unknown`;
2. read the release notes (`CHANGELOG`, diffs of `src/smf.rs`,
   `src/event.rs`, `src/primitive.rs`) for parsing API changes
   (header, `Timing::Metrical`/`Timecode`, meta messages, `u7`/`u4`);
3. update `docs/licences-tierces.md` if the licence changes (it must
   remain Unlicense or an equivalent permissive licence);
4. revalidate:

```sh
nix develop -c cargo test -p scoretk-midi-common -p scoretk-midi-import -p scoretk-midi-export
nix develop -c cargo build -p scoretk-midi-common -p scoretk-midi-import -p scoretk-midi-export --target wasm32-unknown-unknown
nix develop -c cargo test -p scoretk-midi-import --test import_corpus -- --nocapture
nix develop -c cargo test -p scoretk-midi-export --test roundtrip_midi -- --nocapture
nix flake check
```

**When to do it**: during a project dependency update, or if a
MIDI corpus file no longer reads after a MuseScore version bump
(the SMF format itself is stable; it is `midly`'s API that evolves).

## Measurement (reliability plan, phase 0-1)

Survey of 2026-11-19, **without any scoretk fixes**. Tools versioned
in `scripts/oracle/midi/` (`lots.sh`, `mesure.sh`, `mesure.py`,
`midi_notes.py`); manifests (path lists, no third-party data in the
repository) and raw outputs in
`/tmp/nix-shell.7XCuVq/claude-1000/-home-jacques-programmes-musique-musescore/2ae55baa-d7cc-4f59-831d-f77ca3cc7ebd/scratchpad/midi-mesure/`.
The oracle is independent of both directions: `mido` extracts notes from the
reference MIDI, `ElementTree` (standard library) those from the produced MusicXML.
Every launch of scoretk goes through `timeout 10 prlimit --as=2000000000`.

**Measured levels.** N1 = multiset of pitches; N2 = pitch + onset;
N2d = pitch + onset + duration. Onsets and durations are expressed in quarter notes,
brought to the Q16 grid (1/16 of a quarter note, the default setting of MuseScore's
import) with a ±1/16-quarter-note matching tolerance. A file
is "exact" when N1 = N2 = N2d = number of reference notes; a file
with no reference note is not counted as exact. Percentages are weighted
by the number of reference notes, so that a short and a long piece do not carry
the same weight.

**Batches and sampling.** Reading: 1 020 LilyPond MIDIs from `~/Musique`
paired with a `.ly` and 122 Mutopia MIDIs. Writing: the 1 020 paired `.ly`s
(reference = LilyPond MIDI), 21 corpus `.mscx` (reference = MIDI exported by
MuseScore 4.7.4) and 8 `.mscz` from `~/Musique` with a MIDI alongside. Robustness:
Lakh corpus without paired source. MuseScore 4.7.4, the alternative reader, is
measured on a deterministic sample of **1 file in 10** (102 files from
`~/Musique`, 13 from Mutopia; `--alt-stride 10`) and Lakh on **1 file in 20**
(96 files; `LAKH_STRIDE=20`): with the machine's load, MuseScore and
dense Lakh imports made full batches prohibitive. scoretk is
measured on the entirety of the reading and writing batches.

### Results

| Batch | Direction | Tool | Files | Succeeded | Failures | Timeouts | Empty | Exact | N1 % | N2 % | Avg. size | Max time |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| ecriture_ly | writing | scoretk | 1020 | 1020 | 0 | 0 | 0 | 753 (73,8 %) | 90,33 | 83,25 | 984 | 0,10 s |
| ecriture_mscz_corpus | writing | scoretk | 21 | 21 | 0 | 0 | 0 | 17 (81,0 %) | 100,00 | 100,00 | 248 | 0,01 s |
| ecriture_mscz_musique | writing | scoretk | 8 | 7 | 1 | 0 | 0 | 1 (12,5 %) | 66,25 | 52,20 | 6451 | 0,22 s |
| lecture_musique | reading | scoretk | 1020 | 1018 | 0 | 2 | 0 | 245 (24,0 %) | 100,00 | 97,26 | 46 575 | 10,07 s |
| lecture_musique | reading | MuseScore 4.7.4 | 102 | 102 | 0 | 0 | 0 | 20 (19,6 %) | 100,00 | 97,73 | 66 786 | 7,81 s |
| lecture_mutopia | reading | scoretk | 122 | 115 | 0 | 7 | 0 | 11 (9,0 %) | 99,74 | 91,20 | 201 211 | 10,01 s |
| lecture_mutopia | reading | MuseScore 4.7.4 | 13 | 13 | 0 | 0 | 0 | 1 (7,7 %) | 100,00 | 74,38 | 521 281 | 3,32 s |
| robustesse_lakh (1/20) | robustness | scoretk | 96 | 54 | 2 | 40 | 0 | 0 (0,0 %) | — | — | 979 050 | 10,01 s |

Reading: scoretk places **100% of pitches** on the `~/Musique` batch (99.74%
on Mutopia) and beats MuseScore on Mutopia (91.20% of onsets versus
74.38% on 13 files), while being very close to it on `~/Musique`
(97.26% versus 97.73% on 102 files). On writing, the 21 corpus files with
a MuseScore reference
are conformant on notes; the 4 not counted as "exact" (`mmrest`,
`repeat68`, `repeat69`, `testTempoPrecision`) have a reference with no notes.
The `.mscz` batch of `~/Musique` is the weakest: one file is refused
(unsupported MuseScore 2.06 version) and two have a note count very
different from the reference. In robustness (Lakh, 1/20 sample),
**40 files out of 96 exceed the 10 s bound**, 2 produce a malformed
MusicXML and 54 pass, for an average output size of 979 ko: this is the
clearest wear signal of this survey.

### Discrepancy causes

Ten causes observed, by decreasing frequency, with one example file
each. The first eight are flags from the survey's automatic
grouping (all batches combined); the last two are targeted
diagnostics on files from the `.mscz` and Lakh batches:

| Cause | Files | Example |
| --- | ---: | --- |
| Different hold duration (`gateTime`), correct notes and onsets | 724 | `~/Musique/Partitions/Accordeon/Encantada/Encantada.ly` |
| Onsets shifted after quantisation | 341 | `~/Musique/Ecole/2024_Noel_1re/À-Notre-Dame.ly` |
| Clef octave / octave clef | 87 | `~/Musique/Ecole/2206_Pere_Pro/partitions/02_02_Segura_L-aigle-mexicain_partition.ly` |
| Missing notes (voices, n-tuplets) | 55 | `~/Musique/Partitions/ChantsScouts/Fait/0a/lily-8546410f.ly` |
| Exceeding the 10 s bound | 49 | `~/Musique/Partitions/Chorales/Mendelssohn/Laudate_pueri_Op._39_No._2_-_Felix_Mendelssohn.midi` |
| Extra notes (repeats, grace notes) | 35 | `~/Musique/Ecole/Promos/22_StMichel/22_HymneStMichel.ly` |
| Constant pitch offset (transposition, -12 semitones) | 13 | `~/Musique/Ecole/Promos/22_StMichel/22_StMichel.ly` |
| Reference with no notes (not counted exact) | 6 | `~/Musique/Partitions/ChantsScouts/Partitions/LaRondeDeNuit/LaRondeDeNuit.ly` |
| Malformed output MusicXML | 2 | `~/programmes/musique/corpus/lakh-midi-clean/clean_midi/Backstreet Boys/Back to Your Heart.1.mid` |
| Unsupported MuseScore 2.06 version | 1 | `~/Musique/Partitions/Chorales/AdTeSuspiramus/Ad_te_Suspiramus.mscz` |

Timeouts are not worked around by raising the bound: these are loops
or quadratic processing to fix in phase 2. The file
`Dreadlock Holiday.1.mid` (114 ko, Lakh) still exceeds the limit at 25 s in
isolated diagnosis.
### Phase 2 — Lakh robustness fixes (2026-11-19)

The phase-0 findings above were reproduced and fixed in three independent
causes. Profiling used a release build with temporary per-stage instrumentation
(`perf` is not available in the Nix shell) on `Dreadlock Holiday.1.mid`,
`Fernando.2.mid` and `Beautiful Life.mid`. Parsing the SMF and merging note-on /
note-off pairs took less than 15 ms per file; all the time was spent in
`convert_midi`.

**1. Quadratic chord scans (performance).** `MidiTuplet::findAllTuplets`
accounted for roughly 3.5 s of a 3.7 s conversion on `Dreadlock Holiday.1.mid`:
for each candidate start position of each metric span of each bar,
`detect_tuplet` rebuilt its chord list by walking **all** chords of the track,
i.e. `O(bars × spans × positions × chords)`. Upstream
(`importexport/midi/internal/midiimport/importmidi_tuplet.cpp`,
`findAllTuplets`) iterates `[startBarIt, endBarIt)` ranges of the
`std::multimap`, which contain only the chords of the current bar. The port now
mirrors that with `ChordMap::range(start, end)`, a `BTreeMap::range` on
`(tick, rank)`; the original time and `bar_index` filters are kept, so the
selected chords and their order are unchanged. The same pattern was fixed in
two other stages:

* `MChord::minimizeNumberOfRests` looked for the next chord of the same voice by
  scanning the whole table for every chord (`O(n²)`); `ChordMap::next_in_voice`
  now uses an **exclusive** bound at `(tick, u64::MAX)` so chords at the same
  tick remain excluded, exactly as the original `next_on_time > key.0` filter.
* `MidiVoice::separateVoices` rebuilt each bar's key list by scanning the whole
  table once per bar; a single pass now fills a sparse
  `BTreeMap<bar_index, keys>`.

A progress guard was added to the tuplet start loop: a non-positive `step`
(which a malformed bar length could produce) skips the span instead of spinning
forever. None of these upstream algorithms is intrinsically exponential; the
cost came from repeated linear scans over ordered data, so the results are
unchanged and only the time drops.

`tests/dense_bars.rs` covers the correctness of the range queries with 12 bars,
two tracks, 352 sixteenth notes plus a triplet in each track: it checks the
part, measure, chord and note counts, the pitches and durations, and that the
triplet is still detected.

**2. Malformed MusicXML (NUL in track names).** Many Lakh MIDI files terminate
their text meta events with a NUL byte (for example `Back To Your Heart.1.mid`:
`'Back To Your Heart'\0`). `String::from_utf8_lossy` preserves NUL, which XML
1.0 forbids and cannot escape, so the raw byte ended up inside `<part-name>` and
`xml.etree.ElementTree` rejected the output. This is faithful to upstream
`midishared/midifile.cpp`: `MidiFile::writeEvent` writes `len - 1` bytes for text
meta events ("Don't null terminate text meta events") and the import converts
the data with C-string semantics. `midifile::decode_meta_text` now removes the
XML 1.0 forbidden characters (C0 controls other than tab/LF/CR, `U+FFFE`,
`U+FFFF`) at decoding time, and is used for track names, metadata and lyrics.
`tests/meta_text.rs` checks an end-to-end MIDI → MusicXML conversion (no NUL,
parseable XML, exact part/measure/note counts and pitches) plus an unaffected
case with ordinary names and whitespace.

**3. Latent panic in `removeOverlappingNotes`.** Once the timeouts were gone,
`Toto/Don't Chain My Heart.mid` exposed a pre-existing out-of-bounds panic: a
note discarded for being shorter than the minimum duration shrinks its chord,
but the loop kept a cached note count and indexed `notes[0]` on an empty vector
(`chord.rs`). The loop now reads the note at the current index on every
iteration. `tests/overlapping_notes.rs` reproduces the exact Lakh timing
(two same-pitch chords 1/192 apart, below the 1/128 minimum) and checks that
only the later chord survives.

**Remeasurement (`LAKH_STRIDE=20`, same manifests).** The table below replaces
the phase-0 rows; it was produced with `scripts/oracle/midi/mesure.sh` and the
same `timeout 10 prlimit --as=2000000000` bounds.

| Lot | Files | OK | Timeouts | Exact | N1 % | N2 % | Max time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lecture_musique | 1020 | 1020 | 0 | 245 (24.0 %) | 100.00 | 97.08 | 0.25 s |
| lecture_mutopia | 122 | 122 | 0 | 11 (9.0 %) | 99.80 | 90.47 | 0.50 s |
| robustesse_lakh (1/20) | 96 | 96 | 0 | 0 (0.0 %) | — | — | 4.23 s |

The small drop in the aggregate N2/N2d percentages is **not** a regression: the
nine files that used to time out (two in `lecture_musique`, seven in
`lecture_mutopia`) now complete with partial matches below 100 %, which lowers
the weighted average. Restricted to the 1018 + 115 files that already succeeded
in phase 0, the note counts, N1, N2, N2d and the exact flags are **unchanged**.
On Lakh, the 40 timeouts, the two malformed outputs and the latent panic are
gone; all 96 files now produce a non-empty MusicXML output, with a maximum
conversion time of 4.23 s instead of 10 s. The two previously malformed files
(`Back to Your Heart.1.mid`, `Besame Mucho.mid`) and the three slowest files
(`Dreadlock Holiday.1.mid`, `Fernando.2.mid`, `Beautiful Life.mid`) were also
checked individually with `ElementTree`: all parse cleanly in 0.26–1.71 s.

## Phase 3 — alternatives, output size and N2 (2026-11-19)

### Alternatives on the same lots and the same oracle

The oracle (`scripts/oracle/midi/`) now runs the alternatives with option
flags, on the same manifests and with the same N1/N2 comparison as scoretk:
`--mscore` (MuseScore 4.7.4, headless), `--midi2ly` (`midi2ly` → `.ly` →
`lilypond -s` → MIDI, compared MIDI↔MIDI) and `--music21`
(`music21.converter.parse` → MusicXML). **music21 is not packaged in nixpkgs**
as of 2026-11-19, so the flag is skipped with a warning rather than recorded
as a failure. Alternatives are sampled with `--alt-stride 10`; scoretk is always
measured on the whole lot. Every launch uses `timeout` + `prlimit --as=2000000000`.

| Lot | Tool | Files | OK | Fail | Exact | N1 % | N2 % | Avg size | Max time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lecture_musique | scoretk | 1020 | 1020 | 0 | 251 (24.6 %) | **100.00** | **100.00** | 48 384 B | 0.79 s |
| lecture_musique | MuseScore 4.7.4 | 102 | 102 | 0 | 20 (19.6 %) | **100.00** | **100.00** | 66 786 B | 2.08 s |
| lecture_musique | midi2ly 2.26.0 | 102 | 90 | 12 | 87 (85.3 %) | 99.11 | 99.11 | 1 705 B¹ | 5.17 s |
| lecture_mutopia | scoretk | 122 | 122 | 0 | 12 (9.8 %) | 99.80 | **99.80** | 262 149 B | 2.09 s |
| lecture_mutopia | MuseScore 4.7.4 | 13 | 13 | 0 | 1 (7.7 %) | 100.00 | 83.73 | 521 281 B | 1.88 s |
| lecture_mutopia | midi2ly 2.26.0 | 13 | 12 | 1 | 11 (84.6 %) | 97.84 | 96.67 | 7 661 B¹ | 16.46 s |

¹ midi2ly's output is a MIDI file, not MusicXML: its size is not comparable
with the two MusicXML producers. The 12 + 1 failures are `midi2ly`/`lilypond`
rejections of dense files, not scoretk failures.

**Result: scoretk ≥ alternatives.** It ties MuseScore on `~/Musique` and beats
it clearly on Mutopia (N2 99.80 % against 83.73 %), and beats midi2ly on both
lots. scoretk also has zero failures and a much lower maximum conversion time
than midi2ly. On the 96-file Lakh robustness sample, scoretk converts 96/96
files with a 6.30 s maximum on an idle machine (one file hit the 10 s bound
during the loaded campaign at 10.01 s but completes in 5.77 s in isolation).

### Output size — `Dreadlock Holiday.1.mid`

The 114 KB Lakh file yields **6 834 457 B** of MusicXML from scoretk and
**8 652 812 B** from MuseScore 4.7.4: the ratio scoretk/MuseScore is **0.79**.
The source MIDI is single-track format 0, 192 tpq, 15 668 note-ons (14 965
after note-on/off pairing), one 4/4 signature, one tempo (582 524 µs ≈ 103 BPM)
and nine programs. The size is therefore inherent to a dense ~15 000-note
piece, not a runaway conversion. Element counts confirm it:

| Element | scoretk | MuseScore |
| --- | ---: | ---: |
| `<note>` (pitched / rest) | 22 115 (17 616 / 4 499) | 22 364 (13 809 / 5 241) |
| `<chord/>`, ties | 9 420, 4 280 | 8 305, 3 282 |
| `<beam>`, `<stem>` | 13 011, 17 594 | 6 654, 17 123 |
| All elements / attributes | 225 074 / 40 935 | 234 165 / 100 226 |

MuseScore emits three times more layout attributes (`default-x`, `default-y`,
`width`), which is why its file is larger despite fewer notes. scoretk actually
*preserves* more notes: with the fixed oracle, its output matches
14 953 / 14 965 reference attacks (99.92 %) while MuseScore's matches only
1 353 (9.04 %) — MuseScore's performance detection rewrites the piece as 3/4
at 222 BPM with a −12 transposition. Reducing scoretk's size would mean
dropping notes or ties, which would lose N1/N2; no faithful importer change is
warranted. Average size ratios over the lots: **0.72** (`lecture_musique`),
**0.50** (`lecture_mutopia`), and **0.65–0.80** (mean **0.73**) on a nine-file
Lakh sample. scoretk is smaller than MuseScore everywhere measured.

### N2 losses — cause and fix

Before the fix, the flagged N2 losses were almost entirely `attaque_decalee`:
194 files / 3 983 lost attacks on `~/Musique` and 78 / 7 773 on Mutopia, with
N1 and the note count always correct. Grouping by cause showed this was **not
an importer defect**: the oracle's `read_xml` ignored `<backup>` and
accumulated durations per (voice, staff) over the whole part. A voice that
only appears after bar N therefore had all its attacks shifted to the start of
the piece. Reduced demonstration: `test_backup.py` (a two-voice score where
voice 2 starts in bar 2) — it failed before the fix and passes after.

The oracle now uses a per-measure cursor moved by `<backup>`/`<forward>`, with
the next measure starting at the end of the previous one. After re-measuring
the same manifests with the same oracle:

| Lot | Before N2 | After N2 | Files improved | Files dropped |
| --- | ---: | ---: | ---: | ---: |
| lecture_musique | 97.08 % | **100.00 %** | 194 | 0 |
| lecture_mutopia | 90.47 % | **99.80 %** | 79 | 0 |

N1 is unchanged (pitch multisets do not depend on attack placement). The
remaining `duree_seule` flags concern N2d only, not N2: the oracle still skips
`<tie type="stop"/>` notes, so tied-note durations are undercounted (documented
oracle limitation, left unchanged to keep the measurement stable). No importer
change was needed for N2: the import was already faithful, and porting more
MuseScore voice-separation code could only have degraded the result.

## Phase 4 — tied-note oracle fix and N2d causes (2026-11-19)

### Oracle: a tied chain is one note

The `duree_seule` limitation above is now fixed in
`scripts/oracle/midi/midi_notes.py::read_xml`. A sustained note crossing a
barline is written in MusicXML as a chain of tied fragments
(`<tie type="start"/>` … `<tie type="stop"/>`), one per measure, while the
reference MIDI has a single note whose duration covers the whole sustain.
The old reader discarded `<tie type="stop"/>` notes, so only the first
fragment was counted. The reader now merges each chain into **one note** whose
attack is the first fragment's and whose duration is the **sum** of all
fragments. Chains are keyed by `(part, staff, voice, pitch)`, so two voices (or
two staves) sustaining the same pitch in parallel stay separate. Grace notes
are still skipped. `read_xml` is shared by scoretk and MuseScore, so the fix
applies equally to both tools and the comparison stays fair. The reduced
regression test `scripts/oracle/midi/test_ties.py` covers a chain across a
barline, a three-fragment chain (stop + start in the middle), two parallel
chains on different voices/staves, an unterminated tie at end of file and an
untied repeated pitch (which must not merge). `test_backup.py` still passes.

### Re-measurement (same manifests, same oracle, both tools)

| Lot | Tool | Files | Exact before → after | N1 % | N2 % | N2d % |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| lecture_musique | scoretk | 1020 | 24.6 % → **58.4 %** | 100.00 | 100.00 | 97.83 |
| lecture_musique | MuseScore 4.7.4 | 102 | 19.6 % → **46.1 %** | 100.00 | 100.00 | 97.85 |
| lecture_mutopia | scoretk | 122 | 9.8 % → **40.2 %** | 99.80 | 99.80 | 96.08 |
| lecture_mutopia | MuseScore 4.7.4 | 13 | 7.7 % → **23.1 %** | 100.00 | 83.73 | 75.38 |

N1 and N2 are unchanged by the tie fix (they do not depend on the duration of
a note). The exact rate jumps because N2d now matches the reference MIDI for
sustained notes. On the Lakh robustness sample (1/20, 96 files) scoretk still
converts 96/96 files, with a 7.02 s maximum and no timeout, under the standing
`timeout 10 prlimit --as=2000000000` bound. In this worktree the
`ecriture_mscz_corpus` manifest still points to a scratch worktree that no
longer exists, so its 21 rows are oracle-side failures (missing reference
MIDI), not scoretk failures; the other writing lots are unchanged from
phase 3.

### Remaining N2d mismatches grouped by cause

Causes were counted note by note on the files sampled for both tools (every
10th file, 102 on `~/Musique` and 13 on Mutopia), after pairing each reference
note with the test note of the same pitch and an attack within ±1/16 quarter.
Only pairs whose duration differs by more than that tolerance are counted.
A *fair* subset was also built, restricted to files where both tools match
every reference attack (N2 = reference), so the duration comparison is not
confounded by MuseScore's attack shifts.

| Cause | `~/Musique` tk / MuseScore | Mutopia tk / MuseScore | Verdict |
| --- | ---: | ---: | --- |
| Staccato/legato length (short note lengthened by one grid step) | 230 / 239 | 567 / 1058 | scoretk better |
| Last note of the piece (one extra grid step at the end) | 27 / 40 | 7 / 14 | scoretk better |
| Note-off quantisation (off-grid reference offsets) | 1 / 15 | 116 / 24 | see below |
| Overlapping notes in one voice | 0 / 0 | 0 / 0 | equivalent |
| Grace notes | 0 / 0 | 0 / 0 | equivalent |

On the fair subset the aggregate is also in scoretk's favour: N2d
**98.09 % vs 97.85 %** on `~/Musique` and **95.29 % vs 90.42 %** on Mutopia;
the exact-file counts are 57 vs 47 and 6 vs 3. The aggregate N2d over all
files (97.83 % vs 97.85 % on `~/Musique`) is a tie to within 0.02 percentage
points, and scoretk wins clearly on Mutopia (96.08 % vs 75.38 %).

The note-off-quantisation line is the only one where scoretk has more local
mismatches, and it is an artefact of the comparison, not a defect worth a
port: all 116 Mutopia cases come from three human-performance files
(`KV331_2_1_menuetto`, `SchubertF-D899-2-Impromptu`, `KingCotton`), where the
reference has offsets such as 191/384 or 1/3 quarter and MuseScore's own
attack placement is wrong (for example N2 = 18.53 % on `KV331_2_1_menuetto`,
against 100.00 % for scoretk). On those files the two tools are not aligning
the same notes, so the duration counts are not comparable. Restricted to the
fair subset, scoretk matches more note-offs than MuseScore on `~/Musique`
(1 vs 15). The remaining local files where scoretk is lower on N2d
(`Fait/04/lily-47b4b301`, `Fait/32/lily-a931e49d`, `Encantada`, `Etude1`) all
belong to the staccato/legato and last-note causes above, where the aggregate
still favours scoretk.

### No importer change

For every cause above scoretk equals or beats MuseScore, so per the stated
rule the importer is left untouched: no port from
`ref-musescore/src/importexport/midi/` was committed, and no file was dropped.
One candidate was investigated and rejected: upstream
`Simplify::minimizeNumberOfRests` skips a next chord that starts before the
current note's off-time (`next->first < note.offTime`), whereas the port's
`ChordMap::next_in_voice` does not. Adding the equivalent
`next_in_voice_after` filter and rebuilding the CLI produced **identical**
N2d on the affected files (92.48 %, 96.67 % and 94.05 % respectively), so the
change was reverted rather than committed as a no-op with a misleading test.
The other upstream difference, the `isHumanPerformance` flag that
`lengthenNote` uses to relax its accuracy guard, is not threaded into
`simplify.rs`, but it has no effect on these grid-aligned lots (both tools
detect the same human performances, and the fair subset still favours
scoretk). `MidiBeat::adjustChordsToBeats`, which aligns human performances to
recognised beats, remains a documented limitation (§ "Limites mesurées"): it
would be a separate feature port with a real risk to the 100 % N1/N2 baseline,
not a localised N2d fix, so it is deliberately out of scope here. No
conversion in this phase was run without `timeout` + `prlimit --as=2000000000`.

## Phase 5 — MIDI writing: baseline, alternatives and the tie-gate fix (2026-11-19)

### Frozen baseline and MuseScore on the same lots

The CLI was built once, copied aside as a frozen binary, and the three writing
lots were measured with `scripts/oracle/midi/lots.sh` + `mesure.py` (same
manifests, same oracle, `timeout 10 prlimit --as=2000000000` on every launch).
MuseScore 4.7.4 itself was then exported on the two `.mscz` lots with the same
oracle. All numbers are "files exact" / N1 / N2 percentages.

| Lot | Tool | Files | Exact | N1 % | N2 % |
| --- | --- | ---: | ---: | ---: | ---: |
| ecriture_ly | scoretk (frozen) | 1020 | 753 (73.8 %) | 90.33 | 83.25 |
| ecriture_mscz_corpus | scoretk (frozen) | 21 | 17 (81.0 %) | 100.00 | 100.00 |
| ecriture_mscz_corpus | MuseScore 4.7.4 | 21 | 17 (81.0 %) | 100.00 | 100.00 |
| ecriture_mscz_musique | scoretk (frozen) | 8 | 1 (12.5 %) | 66.25 | 52.20 |
| ecriture_mscz_musique | MuseScore 4.7.4 | 8 | 3 (37.5 %) | — | — |

MuseScore is the reference for both `.mscz` lots, so its 17/21 on the corpus
is the ceiling (the four non-exact rows have a reference with **zero notes**:
`mmrest`, `repeat68`, `repeat69`, `testTempoPrecision`). On `~/Musique` it gets
3/8; the three extras are `Ad_te_Suspiramus` (MuseScore 2.06, which scoretk's
reader rejects), `Stella_maris_voix-égales` and `Puer_natus_est_nobis_-_De_Martre`.
On the five other files MuseScore's fresh headless export differs widely from
the stored `.midi` reference (e.g. `Wellerman`: 732 produced notes for 183
reference notes), so those rows are reference/tool artefacts, not scoretk
defects. The two actionable gaps are therefore the 2.06 reader (out of scope
for the writer) and the tied-note duration below.

### Non-exact files grouped by first cause, with one reduced example each

`ecriture_ly` (267 non-exact): `octave` 81
(`2206_Pere_Pro/.../02_02_Segura_L-aigle-mexicain_partition.ly`, clef/ottava
octave from the LilyPond reader), `notes_manquantes` 55
(`Fait/0a/lily-8546410f.ly`, voices/nested input), `duree_seule` 49
(`Partitions/Accordeon/Encantada/Encantada.ly`), `attaque_decalee` 46
(`2024_Noel_1re/À-Notre-Dame.ly`), `notes_supp` 35
(`Promos/22_StMichel/22_HymneStMichel.ly`), `autre` 1 (empty reference).
`ecriture_mscz_corpus`: only the four empty references above.
`ecriture_mscz_musique`: `octave`/`notes_manquantes` 5, `duree_seule` 1
(`Puer_natus_est_nobis_-_De_Martre.mscz`), plus the unreadable 2.06 file.

The 46 `attaque_decalee` files have no single systematic offset (the per-note
deltas spread from −24 to +16 quarters); they are repeat/volta and pickup
differences between the two notations, not a localized writer defect. Most
`duree_seule` hits are a **comparator artefact**: in a two-voice staff LilyPond
merges simultaneous unisons into one note while MuseScore (and scoretk) writes
both, so the oracle's `(channel, pitch)` note-on map collapses the duplicates.
Reduced example: `Fait/8a/lily-376d6605.ly`, pitch 60 at 89.5 quarters —
LilyPond has one note-on/one note-off (1 quarter), scoretk has two note-ons
(one per voice), which the oracle pairs as a 0.5-quarter note. This is faithful
to MuseScore's `EventsHolder` (`events[channel].emplace`, no unison merge) and
was intentionally left unchanged. The same mechanism accounts for the
"ratio 0.5" family (163 notes of 1→0.5 quarter, 132 of 0.5→0.25).

### Cause fixed: tie duration must carry the instrument gateTime

`calculateTieLength` (`compatmidirenderinternal.cpp:771-786`) reads
`nel[0].len()` of every tied note, and that play-event length has already been
multiplied by the instrument `gateTime` in `renderChord`
(`compatmidirender.cpp:263`, `e.setLen(e.len() * gateTime / 100)`). The port
added the raw `actualTicks()` of the tied note, so a tied note on an instrument
with a default articulation below 100 % was too long. Reduced test
(`tests/gate_time_ties.rs`): an instrument with an unnamed articulation at
95 % and two tied quarter notes gave a note-off at tick 935 instead of 911;
a single note (455) and a 100 % gate (959) are unchanged. The fix passes
`instrument`/`source_tick` to `tie_length` and applies the same gated
`playEvents` length per tied chord.

### Before / after (same frozen manifests and oracle)

| Lot | Before exact | After exact | Gain | Loss |
| --- | ---: | ---: | ---: | ---: |
| ecriture_ly | 753 (73.8 %) | 753 (73.8 %) | 0 | 0 |
| ecriture_mscz_corpus | 17 (81.0 %) | 17 (81.0 %) | 0 | 0 |
| ecriture_mscz_musique | 1 (12.5 %) | **2 (25.0 %)** | 1 (`Puer_natus`) | 0 |

Per-file comparison of N1, N2 and N2d shows exactly one gain and zero losses
across the 1049 files. The `.ref.mid` corpus test (`corpus_midi`) and the
`e2e_corpus` test remain at 21/21 and unchanged. Every conversion in this
phase used `timeout` + `prlimit --as=2000000000`.

The byte-level oracle (`scratchpad/oracle/compare.sh`) is **not a usable gate
for this change**: its reference was generated on 2026-11-18 and is stale with
respect to later commits (262 diffs, of which 248 are in unrelated
MusicXML/Mxl/LilyPond/ABC outputs). Re-running the frozen binary and the new
binary on the 14 differing `.midi` cases shows that only two actually change:
`v3_voices.mscx` and `v4_testExcludeInvisibleElements.mscx`. Both declare a
default instrument `gateTime` of 95 %, so the new note-off placement is the
intended, MuseScore-faithful result, not a regression.

## Playback transposition from a LilyPond MIDI-only score (2026-11-20)

### What is stored and where it is applied

Some LilyPond files engrave one `\score` but render their MIDI from a second,
transposed `\score` (`\midi` without `\layout`, Notation Reference §24.3;
`\transpose frompitch topitch`, §1.2.2). The pivot model still carries the
engraved score; `Score::playback_transposition`
(`scoretk-core/src/model/playback.rs`, `Option<PlaybackTransposition>` with a
chromatic semitone count) additionally records the interval. Every writer
except MIDI ignores it.

The LilyPond reader only records it when the two scores are structurally the
same music after removing the leading `\transpose` envelopes (same groups,
staves, voices, notes, durations, rests, chords and ties). A MIDI score that
differs in any other way is ignored; see `docs/agents/lilypond.md` §15 for the
seven concrete cases in the corpus.

The MIDI writer applies the interval in `render.rs::played_pitch` to the note
numbers of **Note On / Note Off messages only** (MIDI 1.0 §Channel Voice), for
main notes, grace notes and both chords of a two-chord tremolo. The `FF 59`
key-signature meta-event keeps the engraved key. Reason from the SMF
specification (§Meta-Events): `FF 59 02 sf mi` is a *meta-event* carrying the
key signature for score display/analysis; it is not a channel voice message
and never changes a note number. Transposing it would make the MIDI key
disagree with the pivot's engraved `KeySig` and with the MusicXML, LilyPond,
ABC and MuseScore outputs, with no audible effect. The same choice keeps the
`sound` pitch and the displayed key consistent for every other input format
(the field is `None` there).

### Measurement (same manifests and oracles as §"Final re-measurement")

Baseline: frozen corrected-FIFO campaign `lym-fifo-final` (before this
change). All conversions used `timeout 10 prlimit --as=2000000000`.

| Lot | Baseline exact | Current exact | N1 gains / losses | N2 gains / losses |
| --- | ---: | ---: | ---: | ---: |
| ecriture_ly | 692 (67.8 %) | 692 (67.8 %) | 6 / 0 | 6 / 0 |
| ecriture_mscz_corpus | 17 | 17 | 0 / 0 | 0 / 0 |
| ecriture_mscz_musique | 2 | 2 | 0 / 0 | 0 / 0 |
| lecture_musique | 596 | 596 | 0 / 0 | 0 / 0 |
| lecture_mutopia | 49 | 49 | 0 / 0 | 0 / 0 |

`ecriture_ly` N1 rises 91.00 → 92.27 % and N2 83.86 → 85.77 %. The six files
that gain are `Gueguen.ly` (23 → 258 matched notes), `Sonis (copie 1).ly`
(47 → 258), `Sonis.ly` (46 → 266), and the three `GaudeteChristusEstNatus*`
files (189 → 402, 189 → 402, 441 → 1074). None becomes exact: the three
Gaudete files now match all pitches and attacks but have duration-only
differences, and the Gueguen/Sonis references still have the extra notes of an
unfolded `\repeat volta` (the engraved score is not unfolded). No file loses
N1 or N2; the `octave` flag falls from 60 to 55 files. The two MIDI reading
lots and the LilyPond reading oracle (108/120/46 exact on `mus`/`ra`/`mut`)
are unchanged, because MusicXML output ignores the new field.

Tests: `crates/scoretk/tests/playback_transposition.rs` (reduced `.ly` with an
engraved score, a `\transpose c d` MIDI score and ordinary music; byte-identical
MusicXML, MIDI pitched a tone higher, and a MIDI score that differs otherwise
records nothing) plus the interval assertions added to the reader's existing
`partition_explicite_prioritaire_et_premiere` test.

## Ottava playback and the MuseScore 2.x repeat gap (2026-11-20)

### Ottava: `Staff::pitchOffset` in the MIDI writer

`crates/scoretk-midi-export/src/render.rs` now builds, once per staff, the
`pitchOffset` map of `Staff::updateOttava` (`engraving/dom/staff.cpp:1279-1301`):
each `SpannerType::Ottava` writes its `pitchShift` at its start tick and `0` at
its end tick (`BTreeMap::insert` reproduces `insert_or_assign`, last writer
wins). `played_pitch` adds the last value at a tick ≤ the note's **source** tick
(`findLessOrEqual`, `staff.cpp:1293-1301`). Shifts come from `ottavaDefault`
(`engraving/dom/ottava.h:67-74`): 8va/8vb ±12, 15ma/15mb ±24 semitones. This is
`Note::ppitch = m_pitch + ottaveCapoFret()` (`note.cpp:2664-2690`), which
`compatmidirenderinternal.cpp:860` uses for Note On/Off. The source tick, not
the unfolded `utick`, is correct: up to `RepeatList` expansion the C++ note
object still belongs to the original segment. SMF §Channel Voice defines note
numbers but no ottava (the tool decides the playback transposition), so the
behaviour follows MuseScore, not a normative text.

`Spanner::visible` is intentionally ignored: an invisible 8va still shifts
playback (`setPlaySpanner`, `ottava.cpp:304-309`, is independent of `visible`).
The model has no `playSpanner` flag and MuseScore's default is true.

Tests (`crates/scoretk-midi-export/tests/ottava.rs`): an 8va over two notes
then a note outside it, mid-measure end with the unmodified control, two staves
with 8va/8vb, a chord, a rest and a tied note, and the MusicXML
`<octave-shift>` → model → MIDI chain. The MusicXML importer subtracts the
shift from the written pitch (`pass2.rs::build_note`), so MIDI restores the
sounding pitch of the XML; all four expected pitch/attack/duration triples
come from the reduced scores rendered with MuseScore 4.7.4, without a binary
fixture.

### Measurement against a frozen master binary

Frozen baseline `/tmp/scoretk-master-ottava` built from this worktree's HEAD
(`20e6833`, before the change); same manifests and oracle for both runs,
`timeout 10 prlimit --as=2000000000` on every launch
(`scripts/oracle/midi/mesure.py`, writing lots regenerated with `lots.sh`).

| Lot | Baseline exact / N1 / N2 | Current exact / N1 / N2 |
| --- | --- | --- |
| `ecriture_ly` (1020) | 692 / 92.27 / 85.77 | 692 / 92.25 / 85.76 |
| `ecriture_mscz_corpus` (21) | 17 / 100.00 / 100.00 | 17 / 100.00 / 100.00 |
| `ecriture_mscz_musique` (8) | 2 / 69.89 / 57.25 | 2 / 69.89 / 57.25 |

The two `.mscz` lots and all 21 corpus files are byte-identical in score. On
`ecriture_ly`, two files lose a few matches and **must be explained**:
`Ackermans_NoelDesPetitsSantons.ly` (N1 2201 → 2180, N2 2011 → 1994) and
`Salazar_ALaEstrellaQueEsmalta.ly` (N1 1240 → 1239, N2 1240 → 1238). Both use
`\ottava`; in these two contexts the LilyPond reader's ottava spanner covers
one more chord than LilyPond's own MIDI playback, so a shift that was inert
before is now audible (for example a note at 165 quarters is 48 in the
reference and becomes 72 under `\ottava #2`). Correcting that anchor belongs
to the LilyPond reader and is deliberately left out of this MIDI-only change;
the aggregate loss is 0.02 N1 / 0.01 N2 percentage points. No other file
changes.

MusicXML oracle (`scripts/oracle/musicxml/mesure.py`, same 174-file manifest,
current binary, N2 per version line as in §20): 3.x/4.x N1 0.9792, N2 0.9740
(unchanged); 1.x/2.x N1 0.9739 (was 0.9700), N2 0.6754 (was 0.6742). The gain
is concentrated in `Vingt_danses_enfantines_pour_piano.mscz` (N1 0.9896 →
0.9985, N2 0.3106 → 0.3134, 26437/26477 matched pitches). The MIDI writer
does not change `core`, XSD or `exact` counts.

The byte-level oracle `scratchpad/oracle/compare.sh` is not a usable gate for
this change either: its frozen reference predates several commits and already
reports 276 DIFFs on the untouched master tree. With the ottava change it
reports 281. The five new rows are exactly the MIDI outputs of the four corpus
scores carrying ottavas (`v3/layout_elements`, `v4/copypaste_parts`,
`v4/partExclusion`, `v4/testExcludeInvisibleElements`, each as `.mscx` and as
`.ref.musicxml`). All four are now **closer** to the conformance target:
`corpus_midi` still reports `MIDI : 21/21 identiques` against the `.ref.mid`
produced by MuseScore 4.7.4, and no non-MIDI output changes.

### Repeats: the 2.x reader and the unfolding are not the cause

The piano file was reduced to the *first* passage where attacks shift: a
hand-written 2.06 score with `segno`/`fine` markers, a D.C. `<Jump>`, a
`<startRepeat>`/`<endRepeat>4` and voltas with `endings` `1, 3` and `2, 4`.
`crates/scoretk-midi-export/tests/repeat_2x.rs` checks its 65 MuseScore 4.7.4
attacks: they match exactly. The real file is also read completely — 2040
measures, 60 voltas, 8 ottavas, 144 marker/repeat elements, all `<Marker>`,
`<Jump>`, `<Volta id>` + `<endSpanner id>` and all `<endings>` values converted
by `read206` (`docs/agents/mscx-format.md` §19). `RepeatList::unwind`
(`repeat_list.rs:348-586`) was compared line by line with
`RepeatList::unwind` (`engraving/dom/repeatlist.cpp:787-1069`), including
`findMarker` (`:643-726`), `performJump` (`:737-782`) and the repeat/volta
rewind (`:886-908`): no algorithmic difference was found that would explain
the measured offsets. Neither the reader nor the unfolding needed a change.

The remaining +6/+4.5/+13/+3-quarter offsets are **section-pause tempo
differences**, not repeat passes. In the reference MIDI the pause at 1118
quarters is 40 BPM for 2 quarters (a 2/4 measure, `quarterNotesPerMeasure /
3 s`) whereas scoretk writes 60 BPM for 3 quarters, and the reference has 93
tempo events against 58 in scoretk (`PauseMap::calculate`,
`engraving/compat/midi/pausemap.cpp:47-76`, versus `pause_map.rs:74-115`).
Removing the `pause.tick == segment.tick` guard decreased N2 from 8298 to
5411 on that file, so that guard is correct and was kept. The remaining
pitch divergences are the 2.x ottavas whose extent ends mid-measure
(`docs/agents/mscx-format.md` §19.8), where the model anchor is less precise.
Fixing either would require a separate port of the tempo timeline / 2.x
spanner anchoring, not a change to `read206` or `repeat_list.rs`; none is made
here because a wrong change there would regress the 21/21 conformant corpus.

### Pause map: faithful `PauseMap::calculate` (2026-11-20, follow-up)

`pause_map.rs` now ports `PauseMap::calculate` line by line
(`pausemap.cpp:47-76`). It first builds the **unfolded** tempo timeline:
`source_tempos` collects every `<Tempo>`/tempo annotation (plus the default 2
bps at tick 0, `tempotimeline.cpp:44-46`), `source_pauses` collects the section
breaks (3 s, `Sid::sectionPause`), both are unfolded through `RepeatList`
segments, then `tempo_timeline` merge-walks them by utick like
`TempoTimeline::rebuild` (`tempotimeline.cpp:134-180`). A pause point carries
the bps of the point that precedes it, so the restore no longer uses a separate
source-tick lookup. `render.rs::tempo_events` now only converts
`PauseMap::tempo_events` (the C++ `m_tempoEvents` map) to microseconds: there
is no second, independent unfolding of tempo marks. Fermatas and caesuras have
no duration in the pivot model (the upstream playback module that fills
`PausesMap` is absent), so only section breaks produce pauses; the existing
reduced test `points_d_orgue_et_respirations_n_allongent_pas_le_rendu` guards
that choice.

Two upstream details matter for 4.7.4:

* `RepeatList::utick2tick` (`repeatlist.cpp:239-258`) bounds the search by the
  **next segment's utick**. The Rust port compared an unfolded tick with
  `segment_end`, a source tick, so every pause after the first repeat mapped
  to source tick 0 and inherited the wrong time signature. Fixed in
  `repeat_list.rs`.
* 4.7.4 takes the pause length from the **actual measure length**, not the
  declared `<TimeSig>`: the 2.x piano file has `len="2/4"` measures declaring
  `3/4`, and the reference writes 40 BPM for 2 quarters where the 5.0 source
  would write 60 BPM for 3. Per `docs/agents/upstream.md`, 4.7.4 wins.
  `quarter_notes_at` uses `Measure::len` and falls back to the time signature.

Reduced test: `pause_map::tests::un_saut_de_section_insere_toujours_une_pause`
builds a 2/4 measure with a 3/4 `<TimeSig>` and asserts the 2/3 bps pause, the
960 inserted ticks and the 2 bps restore. `corpus_midi` stays 21/21.

Measurement (frozen baseline from master `82b28dd`, same manifests and oracle,
`timeout 10 prlimit --as=2000000000`): the three MIDI writing lots are
**unchanged file by file** (no gain, no loss). MusicXML oracle, N2 per version:
3.x/4.x 0.9740 unchanged; 1.x/2.x **0.6754 → 0.7094**. The piano file alone
goes from N2 0.3134 to 0.3913 (8298 → 10361 matched attacks) and N2d 7720 →
9643, with its first three section pauses now at the reference's 672/952/1118
quarters, 40 BPM for 2 quarters each. `core`, XSD and `exact` counts are
unchanged.

### Ottava anchoring: 2.x `endSpanner` and LilyPond `\ottava` (2026-11-20)

Two anchoring gaps remained from the first ottava commit.

* **MuseScore 2.x.** In `read206/transform.rs`, a measure-level
  `<endSpanner id>` was anchored on the measure's start tick. `readMeasure206`
  (`read206.cpp:2834-2852`) uses `ctx.tick()` — the current position — so an
  `<endSpanner>` between two chords ends the ottava at the following chord,
  not at the barline. The anchor now uses `SpanKey::Chord` with the current
  chord ordinal, like the 2.x ottava start; `spanners.rs::anchor` falls back
  to the measure end when the spanner closes after the last chord. Reduced
  test `ottava_2x_milieu_de_mesure.rs`: a 2.06 score with `<Ottava id="1">`
  after two chords of bar 1 and `<endSpanner id="1"/>` after two chords of bar
  2; its eight MuseScore 4.7.4 pitches/ticks are asserted (76/77/79/81 under
  the 8va, 71/72 outside). Before the fix G4/A4 were played as 67/69.
* **LilyPond.** Notation Reference §1.3.3 is explicit: ottava brackets
  "raise or lower displayed notes by one or more octaves, leaving the
  sounding pitch unchanged". This is confirmed with LilyPond 2.26: a plain
  `c'4` and a `c'4` under `\ottava #1` both produce MIDI note 60. MuseScore's
  ottava, conversely, changes playback. The model now carries
  `Spanner::play` (the C++ `Spanner::playSpanner()`, default true); the
  LilyPond reader sets it to false for ottava spans, and
  `render.rs::pitch_offsets` skips such spans just as
  `Staff::updateOttava` (`staff.cpp:1285`) skips `!playSpanner()`. The span
  itself is unchanged (still exactly the notes between `\ottava #1` and
  `\ottava #0`), so LilyPond and MusicXML export keep the bracket. Reduced
  test `read_lilypond::ottava_de_lilypond_est_une_convention_d_ecriture`
  asserts the span ticks (480 → 1440) and `play == false`. This removes the
  two `ecriture_ly` losses of the first ottava commit without changing any
  other score.

Measurement (frozen baseline from master `aaa9cf5`, same oracle, `timeout 10
prlimit --as=2000000000`): the three MIDI writing lots show exactly two
changed files, both gains — `Ackermans_NoelDesPetitsSantons.ly` N1 2180 →
2201, N2 1994 → 2011 and `Salazar_ALaEstrellaQueEsmalta.ly` N1 1239 → 1240,
N2 1238 → 1240 (the `octave` flag disappears); aggregate `ecriture_ly` returns
to 92.27 / 85.76, the two `.mscz` lots are unchanged. MusicXML oracle: 3.x/4.x
N2 0.9740 unchanged, 1.x/2.x 0.7094 unchanged; the piano file gains 10 more
matched pitches (26437 → 26447) and 2 attacks (10361 → 10363). `core`, XSD,
`exact` and `corpus_midi` (21/21) are unchanged.

## `.mscz` losses at first divergence: model check (2026-11-20)

The writing campaign's N2 table for `.mscz` was measured on **LilyPond's MIDI
of scoretk's `.ly`**, not on scoretk's MIDI writer. Before changing the reader
or the MIDI writer, the five files named in that table were re-checked with
`scoretk S --to midi` against the *same* MuseScore 4.7.4 reference MIDI
(`/tmp/ly95-rank/refs/<md5>.mid`, `timeout 10 prlimit --as=2000000000`, Q16
grid). The MusicXML dump (`--to musicxml`) was read with `midi_notes.read_xml`
on the same grid.

| File | `<Note>` in source `.mscx` | model MusicXML | our MIDI | reference MIDI | N2 (our MIDI) |
| --- | ---: | ---: | ---: | ---: | ---: |
| `Espana_Cani.mscz` | 1605 | 1534 | 3070 | 3078 | **99.74 %** |
| `Sicut_Cervus__Palestrina.mscz` | 1209 | 1119 | 1119 | 1119 | **100.00 %** |
| `El_Choclo.mscz` | 508 | 508 | 924 | 1161 | 28.17 % |
| `JMHaydn_Viderunt_omnes.mscz` | 7398 | 7287 | 7340 | 8022 | 91.50 % |
| `Minuit chrétien - Basson.mscz` | 4871 | 4802 | 4809 | 5241 | 91.76 % |

Conclusions, in decreasing order of the earlier claim:

1. **`Espana_Cani` and `Sicut_Cervus` are not model or writer losses.** The
   earlier 49.84 % / 50.85 % came from LilyPond's rendering of our `.ly`
   (unison merging, §"tremolo/artefacts" above), not from model N2. Our MIDI
   export matches the reference to within eight notes and exactly,
   respectively. No reader or writer change is warranted; the premise that
   these two need a fix is retracted with the measurement above.
2. **The extra notes are playback-generated, and the reader is not at fault.**
   Matching the extra MIDI notes `(tick, pitch, track)` against the `.mscx`
   identifies two constructs:
   - **Trills** in `JMHaydn` (206 `<Articulation><subtype>ornamentTrill`) and
     `Minuit chrétien` (16 `<Ornament>` of the same subtype). Their 682 and
     432 extra notes have a duration of ~0.12 quarter (59 ticks) and alternate
     two diatonic degrees — e.g. JMHaydn tick 55.5/55.75/56.0: `74, 72, 74`,
     then `79, 77, 79`. MuseScore's `excursions` table
     (`compatmidirenderinternal.h:89-92`) and `renderNoteArticulation`
     (`compatmidirender.cpp:668-810`) generate exactly this. The `.mscz` reader
     already stores `OrnamentType::TrillMark` (`read/notation_names.rs:139`);
     the **MIDI writer** ignored it. Fixed below.
   - **Chord symbols** in `El_Choclo` (32 `<Harmony>`). At tick 0 the reference
     adds `62, 65, 69` (D minor) where the source has a single `38`: the
     `<Harmony><root>16</root><name>m</name>` is realised and played by
     `renderHarmony` (`compatmidirenderinternal.cpp:1021-1078`) using the
     `RealizedHarmony` voicing. The pivot carries the symbol but the MIDI
     writer did not realise it; fixed below.

### Cause fixed: chord-symbol playback

`crates/scoretk-midi-export/src/harmony.rs` (new file) ports the default-style
`RealizedHarmony` voicing (`realizedharmony.cpp:92-180`): bass at
`DEFAULT_OCTAVE - 2` (octave 3), root and close-position degrees at
`DEFAULT_OCTAVE` (octave 5), intervals from the chord name (minor/major, 7th,
dim, aug, sus…), and `getActualDuration` with the default
`UNTIL_NEXT_CHORD_SYMBOL` (`styledef.cpp:420`). `render.rs` unfolds those
symbols on the same repeat segments as the notes (`renderHarmony`,
`compatmidirenderinternal.cpp:1021-1078`). The reduced test
`tests/harmony_playback.rs` uses the notes of a two-measure `Dm` → `A7` score
exported by MuseScore 4.7.4 (bass 38/45, close voicing 62/65/69 and
69/61/64/67), without a binary fixture.

Full `.mscz` re-measurement on the same 171 reference MIDIs: **6 files gain
1522 N2 matches, zero losses**; N2 94.17 % → **94.57 %**. Gains:
`Czardas Monti` +394, `Triumphal March` +364, `ZORBA THE GREEK` +355,
`Adios Muchachos` +192, `La Partida` +158, `El Choclo` +59 (the first
passages now match exactly; the later ones keep a pre-existing shift).

### Remaining `.mscz` losses (common manifest, by lost notes)

| File | Lost | N1 / N2 | First divergence and attribution |
| --- | ---: | ---: | --- |
| `Vingt_danses_enfantines_pour_piano.mscz` | 16 114 | 99.89 / 39.14 | From tick 1507 a 1-quarter drift; documented above as section-pause tempo (40 BPM/2 quarters vs 60/3) and 2.x mid-measure ottava anchoring — not a repeat or voicing fault. |
| `Vivaldi … Dominus_fredipi.mscz` | 897 | 100 / 40.71 | All pitches present, attacks shifted; same pause/volta family. |
| `El_Choclo.mscz` | 775 | 100 / 33.25 | First 70 quarters exact after the chord fix; then a 1.5-quarter drift from a later passage (repeat/pause), not the voicing. |
| four others | 519 | — | Accordion registration and 2.x ottava ends (§19.8 of `mscx-format.md`). |

These require the tempo timeline / 2.x spanner anchoring port discussed above;
neither is a chord-symbol, trill or reader-note defect. No per-file loss was
introduced by either playback cause.

### Cause fixed: trill playback in the MIDI writer

`crates/scoretk-midi-export/src/ornaments.rs` (new file) ports the default-style
`ornamentTrill` excursion: 32nd step (60 ticks at 480/quarter), prefix `{0,1}`,
body `{0,1}` repeated to fill the chord, diatonic upper degree, with the
64 Hz/8 Hz frequency bounds of `renderNoteArticulation`. `render.rs` replaces
the sustained note by that excursion when a chord carries `TrillMark` and no
grace notes are merged (`graceNotesMerged`, `compatmidirenderinternal.cpp:1736`).
The reduced test `tests/trill_playback.rs` uses the eight attacks and three
following notes exported by MuseScore 4.7.4 from a minimal `.mscx` (both the
`<Articulation>` and `<Ornament>` encodings), not a binary fixture.

Full `.mscz` re-measurement with the same 171 reference MIDIs and both binaries
under `timeout 10 prlimit --as=2000000000`: **30 files gain 3870 N2 matches,
zero losses**; N2 93.14 % → **94.17 %**. Largest gains: `JMHaydn` +584 (91.50 →
98.78 %), the nine `Minuit chrétien` parts +252 each (91.76 → 96.57 %), four
`Missa in Es` Haydn movements +128/+77, and several `Adeste fideles` parts +62.
`Espana_Cani` and `Sicut_Cervus` remain unchanged (already 99.74/100 %).

### Remaining causes: diagnosis (2026-11-20, follow-up)

Matching the remaining extra notes `(tick, pitch, track)` against the `.mscx`
gives the following diagnoses. None is a reader or voicing fault; each needs a
separate port beyond the chord/trill work above.

- **`Vingt_danses_enfantines_pour_piano.mscz`** (16 114 lost, N1 99.89 %): all
  pitches are present; from playback tick 1507 every attack is exactly **one
  quarter early**. Our timemap puts segment 15's last measure (index 474, a
  `D.S. al Fine` with `jumpTo=segno4`, `playUntil=fine4`,
  `continueAt=segno5`) at ticks 1503–1506 and segment 16 (target measure 346)
  at 1506; MuseScore resumes at 1507 — a one-quarter gap at the jump. The
  section pauses of this file are at ticks 536 640 and 837 600, far from the
  divergence, so `PauseMap` is not the cause. `quarter_notes_at` was suspected
  (`qn=2` vs `sigqn=3` at some pauses) but a reduced 2.06 score with an
  irregular `len="2/4"` under `3/4` showed MuseScore 4.7.4 also uses the
  **measure length**, as the current code does; that hypothesis is retracted.
  The remaining work is the jump/fine gap in `RepeatList` (`repeatlist.cpp:
  737-782`, `performJump`), not the pause map.
- **`Vivaldi … Dominus_fredipi.mscz`** (897 lost, N1 100 %): the file has 18
  `ornamentTrill`, 4 `fermataAbove` and one `section` break. Our trill
  renderer produces 206 more notes than the reference and spells the upper
  degree a semitone high (reference 79, ours 80 at tick 10.5): the step must be
  resolved against the key signature (2 sharps), not only the note's TPC
  (`tpcInterval` in `realizedharmony.cpp`). Fermata time stretch
  (`Fermata::timeStretch`) has no duration in the pivot model (`PausesMap` is
  absent from the reference tree), so its playback cannot be reproduced yet.
- **`El_Choclo.mscz`** (775 lost, N1 100 %): the first 70 quarters match
  exactly after the chord fix, then the same one-quarter drift as above;
  the file contains `<playUntil>fine</playUntil>` and a `segno`, so the jump
  gap is the common cause.

### Ports 2 and 3: results (2026-11-20, continued)

- **Fermata — no port needed.** `Fermata::timeStretch`
  (`engraving/dom/fermata.cpp:138-139, 250`, `Pid::TIME_STRETCH`) is not read
  by the compatibility MIDI renderer. A reduced score with `fermataAbove` on
  a quarter, exported by MuseScore 4.7.4, gives four quarter notes at 0/1/2/3,
  exactly like our output. `tests/fermata_no_stretch.rs` pins that behaviour.
- **Pause at the D.S. jump — rejected experiment.** Making the segment bound
  inclusive (`tick <= segment.end`) fixed a reduced 2.06 case but regressed
  `Vingt_danses` by 2 373 attacks: section pauses also fall on ordinary
  segment boundaries there. The change was reverted; a correct fix must
  attach the pause to the measure carrying `LayoutBreak` and replay it at
  jump passages in `PauseMap`/`RepeatList`.

The only port in that series is the key-signature-dependent trill helper
(previous commit): 19 `.mscz` files gain 1 319 attacks, none loses,
N2 94.57 → 94.92 %.

### Pauses: port of `collectPauses` per segment (2026-11-20)

The earlier `jump_back` heuristic was not a port: it gained nothing and lost
19 notes in `Vingt_danses`. MuseScore's
`ScoreAutomationController::collectPauses`
(`scoreautomationcontroller.cpp:1100-1131`, called at `:549` for every
measure of every `RepeatSegment`) registers a section-break pause at the
`startUtick` of the measure following the break in raw order, with
`tickOffset = seg->utick - seg->tick` (`:542`); `PauseMap::calculate`
(`pausemap.cpp:47-76`) uses those uticks directly.
`pause_map.rs::pauses_by_segment` ported that mechanism, with the reduced
test `tests/section_pause_jump.rs` (2.06, notes at ticks 0, 4 and 12).
Full comparison: **no loss**, `Vingt_danses` recovers 19 notes
(N2 10 344 → 10 363), overall N2 94.91 → **94.92 %**. The one-quarter
offset at tick 1507 remained unexplained.

### Pause at the `Vingt_danses` jump: tempo events (2026-11-20)

The `set_tempo` events around tick 1507 identify the cause: the 4.7.4
reference has **20 BPM from 722 880 to 723 360** (then 120 BPM), i.e. a
one-quarter pause at the `D.S. al Fine` passage. It is the section pause of
the pickup measure (`len="1/4"`) played at the jump boundary.
`pauses_by_segment` now handles that case (`pause_quarters` in `TempoPoint`,
used by `PauseMap::calculate` instead of the source measure length), with a
restored `index > 0` guard. Full comparison: **+358 notes on
`Vingt_danses`, 0 loss**, `.mscz` N2 94.92 → **95.01 %**.

### Correction of `d39d7f5`: fractional pause measures (2026-11-20)

`d39d7f5` converted `quarter_notes_at()` to `i64` before reuse, truncating
a 5/8 measure from 2.5 to 2 quarters. `pause_quarters` is now `Option<f64>`
and keeps the fraction (`pausemap.cpp:68-71`: `quarterNotesPerMeasure` is a
`double`, only `ticksPerMeasure` is an `int`), with unit tests
`pause_en_cinq_huit_sans_troncature` and
`premiere_mesure_sans_pause_de_section`. The announced +358 notes were an
artefact of that truncation, not of the jump rule; with the correct value
the jump rule adds nothing and was removed. The tick-1507 offset remains
open: the reference's 20 BPM event corresponds to
`quarterNotesPerMeasure / pause = 1 / 3 s` (`pausemap.cpp:71`), but no
attempt to anchor that pickup in the jump `RepeatSegment` improved other
passages.

### End of the pause campaign (2026-11-20)

File-by-file comparison against **master** with the same references and
manifest: keeping the fractional tempo and MuseScore's `ticksPerMeasure`
truncation (`pausemap.cpp:70`) gives **no file gaining or losing**; the two
apparent differences (`Vivaldi … Judicabit`, `Vivaldi … Gloria Patri`) were
10 s oracle timeouts and reproduce master's values (2 469 and 1 677) under
a 20 s bound. The truncation fix is kept for fidelity to `pausemap.cpp:68-71`
without a measurable net gain.
## MIDI import oracle — upstream tests and LilyPond MIDIs (2026-11-21)

Phase 4 of `docs/agents/plan-fiabilite.md` asks for a reading measurement with
two lots: MuseScore's upstream `importexport/midi/tests/midiimport_data`
(`.mid` + expected `.mscx`) and the LilyPond MIDIs whose `.ly` source we read.

`scripts/oracle/midi/import_mesure.py` builds both manifests at run time (no
third-party data is committed), converts each MIDI with the frozen CLI under
`timeout`/`prlimit`, and compares the result with the reference on
`(pitch, onset, duration, voice)` at the Q16 grid (±1/16 quarter). The expected
`.mscx` is parsed directly: sending it through our MusicXML exporter would
apply the instrument's transposition and shift the reference by an octave
(`instrument_clef` is the instructive case). For the `.ly` lot the `.ly` itself
is the ideal-quantisation reference.

The oracle initially skipped `<chord/>` notes on the "got" side, undercounting
chords (three notes became one). That bug is fixed: a `<chord/>` note is now
counted at the previous note's onset without advancing the cursor. All numbers
below use the corrected oracle; with the earlier version the baseline read
0.3637 and the first fix 0.3648, which are not comparable.

Baseline on 98 upstream files: **N = 0.3989** (0.4000 after the SMPTE fix);
the largest losses are `human_tempo` (142 notes, score 0.035), `chord_collect`
(48, 0.0), the tuplet tests and the percussion tests. Many upstream tests set
non-default import options (`dontSimplify`, `maxVoiceCount V_1`), so their
expected `.mscx` cannot be matched by the CLI's default options; those files
are documented, not chased as importer bugs.

### Cause fixed: SMPTE ticks

`toMuseScoreTicks` (`importexport/midi/internal/midiimport/importmidi_fraction.cpp:350-354`)
returns `ReducedFraction::fromTicks(tick)` unchanged when the SMF header uses a
time division. The Rust port converted SMPTE ticks to quarters at a fixed
120 BPM, distorting every onset and duration. Reduced test `smpte_ticks`
(written first, failed before the fix): `to_muse_score_ticks(1000, 80, true)`
now gives the raw 1000 ticks. Measured gain on `up-division` (an SMPTE file):
score 0.0000 → 0.2500; on the corrected oracle N goes from 0.3989 to 0.4000.

### Cause fixed: chord collection and voice occupancy before simplification

Two faithful-port gaps combined to lose the whole `chord_collect` file (48
notes, score 0.0):

1. `quantizeAllTracks` calls `MChord::collectChords(mtrack, { 2, 1 }, { 4, 3 })`
   just before `quantizeChords` (`importmidi.cpp:274`) — a non-human tolerance
   of 4/3 of the smallest duration, "less sensitive to on time inaccuracies
   than 1/2 earlier". The port only did the earlier 1/2 pass, so six notes
   staggered by one MIDI tick stayed in three pairs. Reduced test
   `quant_collect_chords` (written first, failed before the fix) reproduces
   the six-note arpeggio.
2. `MidiTuplet::chordInterval` uses `Quantize::findMaxQuantizedOffTime`
   (`importmidi_tuplet_voice.cpp:50-66`): the voice occupancy of a chord is
   computed from its **quantised** note-offs. The port used raw off-times;
   in `chord_collect.mid` the last note ends at 44.542 quarters, which
   quantises to 44.5, so the next chord at 44.5 was considered overlapping.
   Chords were spread over two voices, and `minimizeNumberOfRests` then
   lengthened every other note from an eighth to a quarter. `assign_initial_voices_quantized`
   now recomputes voices after `quantizeChords` using
   `quantize_value(off_time, quant_for_len(...))`.

Measured on the corrected oracle: N 0.4000 → **0.4516**; `up-chord_collect`
0.0000 → 0.8958, `up-chord_legato` 0.3333 → 1.0000, `up-clef_melody`
0.6667 → 1.0000, `up-tuplet_7_staccato` 0.1429 → 0.7143,
`up-tuplet_duplet` 0.1667 → 0.3333 and `up-human_tempo` 0.0352 → 0.0493.

Five files lose score, all tuplet tests: `up-tuplet_3_5_7_tuplets` (0.2222 →
0.1667), `up-tuplet_2_voices_3_5_tuplets` (0.6364 → 0.5455), `up-voice_tuplet`
(0.5000 → 0.3333), `up-tuplet_tied_3_5_tuplets` (0.3333 → 0.0) and
`up-tuplet_tied_3_5_tuplets2` (0.3750 → 0.0). Their expected `.mscx` contains
`<Tuplet>` (Q32 quantisation in upstream tests) that our CLI defaults do not
detect; the voice renumbering changes their (already non-conformant) layout.
Their combined loss is about 9 notes against 53 gained elsewhere.

## Oracle corrections, category breakdown and second measurement pass (2026-11-21)

The numbers above were produced by an oracle that had three more biases, now
fixed in `scripts/oracle/midi/import_mesure.py` :

* a `<Tuplet>` in the expected `.mscx` scales its notes' real durations
  (`normalNotes/actualNotes`), which the parser ignored — this made every
  tuplet test look wrong even when our import was exact;
* the measure length comes from the last `<TimeSig>` seen (carried over from
  measure to measure), not from a hard-coded 4/4 — `perc_drums` starts with
  three 2/4 measures;
* the LilyPond `.ly` references are parsed by `ly_notes.py` instead of being
  fed to the XML parser.

With the corrected oracle the upstream baseline is **N = 0.2290** and the
current tree **N = 0.2588** (the earlier 0.3989/0.4516 figures are not
comparable).

`scripts/oracle/midi/categories.py` reads a measurement and assigns each file
its worst musical category; the report for the current tree is:

| Category | Files | Weighted loss (notes) |
| --- | ---: | ---: |
| voice | 15 | 225 |
| clef | 12 | 54 |
| duration | 25 | 53 |
| onset | 10 | 43 |
| tuplet | 15 | 36 |

Pitch is never the bottleneck. The biggest single file is `human_tempo`
(142 notes, voice 0.44, onset 0.76): fixing it needs MuseScore's beat tracker
(`importmidi_beat.cpp`), which is still unported. Most "voice" losses are on
upstream tests compiled with `maxVoiceCount V_1`, which our CLI defaults
cannot reproduce.

**LilyPond-MIDI lot.** Only four `.mid` files under `~/Musique` have a
same-stem `.ly` source; two are inside `~/Musique` and are the ones measured
by the oracle (`Ottavio-Catalano-Audite-coeli`, 854 source notes;
`Angels_Carol__John_Rutter`, 253). Their full 4-tuple scores are 0.007 and
0.028, but the minimal `.ly` parser is not measure/voice aware, so these
numbers are **indicative only** and must not be compared with the upstream
lot. The pitch-multiset part is more informative: 1.0000 for `Angels_Carol`
(all source pitches are present after import) and 0.2728 for the mensural
`Ottavio` file, whose deutsch note names and relative octaves the minimal
parser does not fully handle. The plan's 1 228 `.ly`/`.mid` pairs are not
present in this checkout; extending this lot requires generating those MIDIs.

### Cause fixed: tied/staccato tuplet acceptance

`isTupletAllowed` (`importmidi_tuplet_detect.cpp:34-82`) requires only one
note reaching half a tuplet cell, while `detectStaccato` (`:265-278`) rejects
tuplets whose notes are mostly staccato. Our `detect_tuplet` required **all**
notes to fit in their cell, which rejected the tied triplet of
`tuplet_tied_3_5_tuplets.mid`. Reduced test `tuplet_tied` (written first,
failed before the fix) reproduces the three onsets at 0, 1/3 and 2/3 of a
quarter with the last note held for 1.5 quarters. The port now tolerates at
most one short note, which also keeps `tuplet_7_staccato` exact.
`assign_initial_voices_quantized` is skipped when a track has tuplets: the
previous commit's voice renumbering otherwise changed those files' voices.

Measured against the pre-turn tree: N 0.2489 → 0.2558;
`up-tuplet_tied_3_5_tuplets` 0.1111 → 1.0000,
`up-tuplet_tied_3_5_tuplets2` 0.1250 → 1.0000,
`up-tuplet_7_staccato` 0.8571 → 1.0000.
One file loses score, `up-tuplet_16th_8th` (0.9091 → 0.7273): our heuristic
accepts one extra tuplet there and lengthens a note, where MuseScore's
`tupletSumError` / `regularSumError` filter rejects it. That filter is part
of the still-unported `importmidi_tuplet_filter.cpp`.

### Cause fixed: unreduced time signature for drum durations

`ReducedFraction` keeps a 4/4 signature as `(4, 4)`; our `Fraction` reduces
it to `1/1`, so `beatLength` (`importmidi_meter.cpp:135-147`) lost the number
of beats. `beat_length_raw` now takes the raw `Timesig` numerator. It is
applied to drum tracks only: applying it everywhere regressed three compound
meter / duplet files (`meter_last_quarter_rest_compound`, `tuplet_duplet`,
`tuplet_quadruplet`) because `meter_to_duration_list` still uses the reduced
signature — making both sides consistent is the next step of that port.

Measured: N 0.2558 → **0.2588** with no per-file loss; `up-perc_drums`
0.8462 → 1.0000 and `up-perc_short_notes` 0.7000 → 0.8000.

## LilyPond lot and second pass (2026-11-21, continued)

The LilyPond lot is now built correctly: LilyPond writes `.midi` (1 228 under
`~/Musique`, 1 020 with a same-stem `.ly`). `import_mesure.py` uses **our own
LilyPond reader** as reference (`.ly` → CLI → model), not a Python parser; the
`ly_notes.py` experiment is removed. Baseline with the corrected oracle:
**upstream N = 0.7132 / 96 files, LilyPond N = 0.7592 / 1 020 files**.

Three oracle bugs were fixed before attributing any gain: `<chord/>` notes must
be counted at the previous note's onset, MusicXML voices are global
`(staff-1)*4 + local voice` and must be normalised before comparison, and
`.mscx` measure lengths come from the carried `<TimeSig>`. Together they raise
N from 0.6200 to 0.7589 on the combined lot; all figures above use the
corrected oracle.

### Cause: unreduced 12/8 numerator in duration simplification

`Fraction` reduces 12/8 to 3/4, but MuseScore's `ReducedFraction` keeps
`(12, 8)`; `beatLength` (`importmidi_meter.cpp:135-147`) therefore gives a
dotted-eighth pulse, not a quarter. `minimizeNumberOfRests` now uses
`beat_length_raw` when the raw numerator is 12 (drum tracks were already
handled by the previous commit). Regression test `compound_128` covers 12/8.
Measured: LilyPond loss 31 778.2 → 31 775.2 notes (one file, 0.6792 → 0.6906),
upstream unchanged, no per-file loss. The other narrow numerators were tested
and rejected: 6/8 regresses three upstream files, 15/8 and 9/8 have no effect.

The full unreduced-numerator propagation (`division_info`, `meter_to_duration_list`,
all compound meters) was also measured and rejected: 197 gains but 120 losses,
including `simplify_8th_dont` 1.0 → 0.33 and `meter_chord_example` 0.53 → 0.07.
Porting it correctly requires carrying the raw numerator in the model's
`Measure`, which is the next step of the `importmidi_meter.cpp` port.

## Raw time signature numerator in `Measure` (2026-11-21)

MuseScore's `TimeSigMap` keeps `ReducedFraction(4, 4)` rather than the reduced
`1/1`; `Meter::isDuple`, `isCompound`, `beatLength` and
`metricDivisionsOfBar` (`importmidi_meter.cpp:35-108, 135-164`) all depend on
that numerator. `Fraction` reduced it, so 4/4 fell into the "other meter"
branch and produced duplicate bar divisions. Failing test first:
`raw_timesig_in_measure` in `scoretk-core` (method and fields absent before
the fix).

`Measure` now carries `time_sig_numerator` / `time_sig_denominator`, set by
`with_raw_timesig` in `createMeasures`; `meter.rs` gained
`metric_divisions_of_bar_raw`, `divisions_of_bar_for_tuplets_raw`,
`division_info_raw` and `meter_to_duration_list_with_sig`. They are used by
the duration simplifier, the n-olet detector and the human-performance
classifier. `beat_length_raw` remains limited to drums and 12/8: applying it
to every meter regressed the LilyPond source lot by 768 notes and the MIDI
round-trip test on `v3/testTuplets8` (2/4), because those oracles preserve the
source durations while `lengthenNote` now legitimately simplifies them; that
wider propagation is left for a dedicated fidelity decision.

Measured with the corrected oracle: upstream loss 261.0 → 258.0,
LilyPond loss 31 775.2 → 31 614.9, no per-file loss unexplained. Gains:
`up-human_tempo` 0.4366 → 0.5000, `up-meter_12-8` 0.3636 → 0.8182,
`up-tuplet_16th_8th` 0.7273 → 0.8182, `up-tuplet_off_time_other_bar2`
0.7143 → 1.0000. The six upstream losses are all upstream tests compiled
with non-default options: `meter_chord_example` and `meter_two_beats_over`
use `dontSimplify` (`midiimport_tests.cpp:302, 334`),
`tuplet_triplet_first_tied`/`2` use `dontSimplify` (`:441`),
`tuplet_3_5_7_tuplets` disables `simplifyDurations`/`changeClef` (`:368`) and
`simplify_8th_dont` uses `simplification()` with `maxVoiceCount V_1` (`:640`).
Their expected `.mscx` therefore describes a different import configuration
than the CLI defaults; the raw divisions now make that difference visible.

## BeatRoot port (2026-11-21)

BeatRoot, bundled by MuseScore under
`src/importexport/midi/thirdparty/beatroot/`, is GPL-2.0-or-later (authors
Simon Dixon, Chris Cannam, QMUL, 2011). It has been rewritten in Rust under
GPL-3.0-only, one module per C++ file:
`beatroot/{event,agent,agent_list,induction,beat_tracker}.rs` plus the
`importmidi_beat.cpp` glue in `beat_locator.rs`. The original licence and
authorship are declared in `NOTICE`; no C++ file is distributed.

Reduced tests first (`tests/beatroot.rs`): a regular 0.5 s onset grid must
produce at least six regularly spaced beats, and a single onset cannot induce
a tempo. Both pass with the port.

The glue was measured against the corrected oracle before being enabled:
calling `MidiBeat::findBeatLocations` + `adjustChordsToBeats` from
`convertMidi` changes only `human_tempo`, from 0.5000 to 0.2958 (29 notes
lost), and nothing in the LilyPond lot. The detected beat set for that file
has 17 beats spanning 1.93–23.8 quarters with a 3/4 candidate, whereas the
reference expects the first note at 4.0 quarters; the scaling of the raw beat
grid to whole quarters therefore moves it to 2.5 quarters. The remaining
`updateFirstLastBeats` step of `MidiBeat::setTimeSignature` is not yet
reproduced and is needed to anchor the grid to the bar. The glue is therefore
kept in the crate but **not wired** into `convertMidi`, so both lots stay at
upstream loss 258.0 and LilyPond loss 31 614.9. This is a measured, explained
non-gain rather than an untested module; wiring it back in requires finishing
that anchoring step first.

## The `.ly` reference is not MuseScore (2026-11-21, historical)

With MuseScore 4.7.4 available (`env -u LD_LIBRARY_PATH QT_QPA_PLATFORM=offscreen
mscore`), the worst LilyPond-lot file `Laudate_pueri…` gives: MuseScore import
of the same `.midi` 1 826 notes; our import 1 836 notes, score **0.8587**
against MuseScore; the `.ly` reference read by our LilyPond reader 1 796 notes,
and MuseScore scores only **0.4014** against it. The 1 063 "lost" notes
against `.ly` were therefore mostly MuseScore's import choices, not port
errors. The earlier N = 0.7592 against `.ly` must not be read as fidelity to
MuseScore.

## MuseScore oracle rebuilt on 4.7.4 (2026-11-21, historical)

`scripts/oracle/midi/ms-oracle.sh` converts the 1 020 `.midi` files with
MuseScore 4.7.4 in parallel with an out-of-repository cache and a `done.txt`
completion condition; `categories_ms.py` scores against that reference.
Full result: **weighted N = 0.8018** (29 049.8 lost notes out of 146 573),
by category: pitch 18 702.9 (625 files), duration 5 210.0 (295),
voice 3 030.8 (78), tuplets 1 928.0 (15), onset 178.0 (7). Pitch dominates
because MuseScore fills `Instrument::transpose` from its catalogue and our
MIDI reader did not.

### Cause fixed: instrument transposition (contrabass)

`share/instruments/instruments.xml` gives `contrabass` (GM programs 32/43)
`<transposeDiatonic>-7</transposeDiatonic>` and
`<transposeChromatic>-12</transposeChromatic>`; MuseScore 4.7.4 writes
`<transpose><octave-change>-1</…>` and written pitches an octave higher.
Reduced test `instrument_transpose` checks program 43; on the 15 worst files
`GaudeteChristus…` goes **0.6495 → 1.0000** (150 notes recovered), total loss
2 776 → 2 626, no regression.

### Instrument template selection replaces the special case

The `match program { 32 | 43 => … }` case was removed. The generated
`crates/scoretk-core/src/instruments.rs` now carries all channel programs per
template, transpositions, clef, staff group, drumset, ranges and genre, plus
`GM1_PREFERRED_IDS` (`importmidi_instrument.cpp:60-190`);
`MidiInstr::template_for_track` ports `findInstrumentsForProgram` and
`sortInstrumentTemplates`. The remaining blocker was Tpc: `write_pitch`
subtracts `transpose_chromatic` but `tpc_transposed` must be shifted by the
opposite interval. Reduced tests `instrument_transpose`, `instrument_template`.
Full result against MuseScore: **N 0.8018 → 0.8264**, pitch loss 18 702.9 →
15 359.0; 16 files gain, three ocarina variants lose (1.0000 → 0.8569/0.9299)
because simple range sorting picks a transposed template where 4.7.4 keeps
the preferred non-transposing one.

### Ocarina fix and table compaction (2026-11-21)

Reduced test `instrument_template` first: program 79 must keep the preferred
non-transposing template. Template `minPitchP`/`maxPitchP` are **written**
pitches while MIDI chords are in concert pitch; our sorting compared them
without conversion. The fix adds `+transposeChromatic` to the track range
only when the preferred template is non-transposing (the ocarina case);
applying it to all templates was measured and rejected (12 regressions).
Full result: **N 0.8264 → 0.8279**, pitch loss 15 359.0 → 15 137.0,
**3 gains and 0 loss**. Wasm: 1 924 312 raw / 783 580 gzip bytes after
compacting `programs: &[i16]` into a `program_mask: u128`.

## Upstream oracle: concert pitch vs written pitch (2026-11-21)

Three upstream losses that appeared with template selection
(`instrument_clef`, `pickup_long`, `instrument_channels`) were not import
regressions: the oracle read concert pitch from the expected `.mscx` and
written pitch from our MusicXML. Reduced test `test_concert_pitch.py`;
`import_mesure.py` now converts our pitches to concert when the reference is
`.mscx`. Result: **3 gains, 0 loss** upstream, LilyPond unchanged.

## Open causes (2026-11-21)

### Three-note simplification — stalled

Reduced test `simplify_order.rs` (ignored with reason): pitches 57@5.0→6.5,
60@6.0→6.5, 62@6.5→7.0. MuseScore writes 2.0/0.5/0.5, we write 57 as two
tied notes 1.0 + 0.5. Mechanical reduction ruled out grand staff, channel
and track count. The cause was identified as **voice allocation**, not
simplification: the 4.7.4 reference places 57 in voice 2 and 60/62 in voice
1, while ours placed 57/62 in voice 2 and 60 in voice 1, so `lengthenNote`
followed different next chords.

## Remaining causes ranked (MuseScore lots, 2026-11-21)

From the 1 020-file measurement: pitch 15 137 notes, duration 5 234,
voice 3 031, tuplets 1 928, onset 178. Remaining pitch loss is mostly
instrument-template choice. An earlier hybrid measurement listed
`Transeamus` 160 notes, `W-A-Mozart-Ave-Maria` 111, `Ad_te_Suspiramus` 199,
`Sancta_Maria_Mater_Dei_Mozart` 200 and `Maiana` 140; range adjustment for
downward transpositions was measured and rejected (no gain, −0.07 on
`Gaudete…`) and reverted. The literal template-selection port below resolved
the `lily-ced0e264`/`Transeamus` group.

## Instrument template selection: literal port of `importmidi_instrument.cpp` (2026-11-21)

The previous selection was a partial reconstruction: range sorting **adjusted**
by `transposeChromatic` when the preferred template was not transposing, a
single fallback on the lower program, `useDrumset` always false in the
generated table (the generator looked for `<Drumset>` while the format writes
`<drumset>`), no genres, no channel order, no "synth" / "snare drum" fallbacks.
The port mirrors the C++ function by function:

| C++ `importmidi_instrument.cpp` | Rust `instrument.rs` |
| --- | --- |
| `getPreferredInstrument` :195 | `preferred_template_for_program` (table) |
| `instrumentName` :207 | `instrument_name` |
| `isGrandStaff` :216 / `isSameChannel` :223 | `is_grand_staff` / `is_same_channel` |
| `is3StaffOrgan` :228 | `is_3_staff_organ` |
| `areNext2GrandStaff` :234 / `areNext3OrganStaff` :242 | `are_next_2_grand_staff` / `are_next_3_organ_staff` |
| `concatenateWithComma` :255 | `concatenate_with_comma` |
| `setGrandStaffProgram` :269 | `set_grand_staff_program` (grand staff, 3-staff organ, names) |
| `findAllPitches` :294 | `find_all_pitches` |
| `findNotEmptyDrumPitches` :305 | `find_not_empty_drum_pitches` |
| `hasNotDefinedDrumPitch` :317 | `has_not_defined_drum_pitch` |
| `findInstrument` :328 | `find_instrument` |
| `countMissingPitches` :347 | `count_missing_pitches` |
| `findClosestInstrument` :359 | `find_closest_instrument` |
| `findInstrumentsForProgram` :409 | `find_instruments_for_program` |
| `findMinMaxPitch` :498 | `find_min_max_pitch` |
| `findMaxPitchDiff` :512 | `find_max_pitch_diff` |
| `hasCommonGenre` :525 | `has_common_genre` |
| `sortInstrumentTemplates` :535 | `sort_instrument_templates` |
| `findSuitableInstruments` :569 | `template_for_track` (`msInstrIndex` = 0) |

Behaviours that change the result:

- `findInstrumentsForProgram` walks **every** catalogue template, in
  `instruments.xml` order (groups, then instruments), after the preferred
  template; `sortInstrumentTemplates` is a **stable** sort on the raw range gap
  (`minPitchP`/`maxPitchP` compared with the track's concert pitches, **without**
  any transposition adjustment), then explicit drumset, `common` genre, and
  decreasing genre count.
- Exact fallbacks: program 80-103 → `effect-synth` (or
  `percussion-synthesizer`), program ≥ 112 → `snare-drum`, otherwise
  `findClosestInstrument` (highest channel program **below** the track's, first
  satisfying channel in file order, and for drums the fewest missing pitches).
- `findNotEmptyDrumPitches` uses a `drum_pitch_mask` (bit `p` when
  `<Drum pitch="p">` has a non-empty `<name>`); `has_drumset` distinguishes an
  explicit kit from the common one.

`crates/scoretk-core/src/instruments.rs` (still generated by
`scripts/gen-instruments-table.py`, never hand-edited) now carries the group,
the channel programs **in order**, the genres, `use_drumset` (fixed),
`has_drumset` and `drum_pitch_mask`; a missing range means
`MIN_PITCH`/`MAX_PITCH` (0/127), as in `InstrumentTemplate`. Measured cost:
`nix build .#wasm` gives 1 989 235 raw bytes / 798 227 gzip, against
1 924 312 / 783 580 before (+3.4 % raw, +1.9 % gzip).

Reduced test (`instrument_template.rs::plage_de_la_piste_departage_les_gabarits`):
program 73 with the real ranges of `Transeamus` (58-70) and `lily-ced0e264`
(55-75) → `alto-flute` (-3, -5); 60-90 (inside the flute range) → preferred
template `flute`.

Measurement (same manifests, MuseScore 4.7.4 oracle, frozen
`/tmp/scoretk-pitch-base`, `timeout 10 prlimit --as=2000000000`; numbers with
the tie-merging and unique-identifier oracle described below):

| Lot | Before | After |
| --- | --- | --- |
| LilyPond (1 020 files), N against MuseScore | 0,8536 | **0.9491** |
| total loss | 19 949,1 notes | **6 941.0** |
| `pitch` dimension (lost notes) | 8 676,9 | **0,0** |
| upstream lot (96 files), N against `.mscx` | 0.7350 | 0.7350 |

161 files gain pitches, **none loses any**; `Transeamus` and `Transeamus_a3`
go from 0.742 to 1.0000, `W-A-Mozart-Ave-Maria` from 0.474 to 0.9950. The other
dimensions (onset 142.0, duration 2 450.3, voice 2 451.0) are unchanged.
The pitch multiset in concert pitch was already exact before the port (see
below): what the port recovers is the **written** pitch, since MuseScore writes
the chosen template's transposition and the port now does the same.

## Import oracle: unique identifiers for same-named scores (2026-11-21)

Fifteen LilyPond MIDIs share their file name with another (`SanctePieX`
Gregorian and `SanctePieX` from the cantiques, three `AStMichel`, twelve
duplicates under `Partitions/Cantiques`); nine are byte-identical, three are
not. `import_mesure.py`'s `ly-<name>` identifier made the conversion **and**
the MuseScore reference overwrite the same path: the second row was compared
with the first row's reference, and `SanctePieX` weighed 82 phantom notes in
the `pitch` column.

`identifiants_lilypond` (reduced test `test_ids.py`) assigns a unique,
deterministic identifier: the first occurrence in sorted order keeps
`ly-<name>` (its MuseScore reference already exists), the next ones get
`-2`, `-3`… The fifteen missing references were regenerated with MuseScore
4.7.4. No other figure changes: the upstream N and the LilyPond scores are
identical file by file.

## Import oracle: tied notes are one note (2026-11-21)

MuseScore splits a note that crosses a beat into tied fragments
(`<tie type="start"/>` … `<tie type="stop"/>`), and its expected `.mscx`
carries a `<Tie><startElement>` on every continuation. The import oracle
counted each fragment as a note, so any importer writing a single note of the
same total length was penalised: after the template port, the remaining 1 825.7
"pitch" notes were exactly these fragments. Reduced example `LaMarcheAuCamp`:
MIDI 67 from 1.5 to 2.5 quarters → MuseScore 1.5-2.0 and 2.0-2.5 tied, ours
1.5-2.5.

`notes()` (MusicXML) and `mscx_notes()` (expected `.mscx`) now merge each tie
chain into **one** note whose duration is the sum of its fragments, on **both**
sides, keyed by (part, staff, voice, pitch) for MusicXML and by `<eid>` /
`<startElement>` for `.mscx`. Reduced test `test_ties_import.py` covers a
three-fragment chain in both formats plus an untied repeated pitch.

Re-measurement (same manifests, same frozen binary):

| Metric | Fragment counting | Tie-merged |
| --- | --- | --- |
| reference notes, LilyPond lot | 146 492 | 136 236 |
| LilyPond N, after the port | 0,9251 | **0.9491** |
| `pitch` dimension, after the port | 1 825,7 | **0,0** |
| upstream N (`up-` lot) | 0,7165 (910 notes) | **0.7350** (864 notes) |

The upstream figures are not comparable with the previous ones (46 tie
fragments removed from its references); they are unchanged between the two
binaries. The categories are now real: the remaining loss is voice (2 451.0),
duration (2 450.3), tuplet (930 as a worst category) and onset (142.0).

## Durations: literal port of `importmidi_simplify.cpp` (2026-11-21)

The simplifier was already close, but three upstream details were missing. The
port now mirrors the file function by function:

| C++ `importmidi_simplify.cpp` | Rust `simplify.rs` |
| --- | --- |
| `hasComplexBeamedDurations` :40 | `has_complex_beamed_durations` |
| `lengthenNote` :56 | `lengthen_note` |
| `shortenDrumNote` :170 | `shorten_drum_note` |
| `minimizeNumberOfRests` :211 | `minimize_number_of_rests` |
| `simplifyDurations` :281 | `simplify_durations` |
| `simplifyDurationsForDrums` :318 | `simplify_durations_for_drums` |
| `simplifyDurationsNotDrums` :323 | `simplify_durations_not_drums` |

What changed:

- **`beatLength` always uses the raw measure.** `Meter::beatLength`
  (`importmidi_meter.cpp:135-147`) receives the unreduced bar fraction
  (`ReducedFraction(6, 8)`, `(2, 4)`, `(4, 4)`), which `ReducedFraction` keeps
  as is. `beat_length_raw(bar_fraction, timesig.numerator)` was previously
  limited to drum tracks and 12/8; it is now used for every meter. For 6/8 the
  beat is 3/8 whole (1.5 quarters), for 2/4 it is 1/4 whole (1 quarter).
- **`next` is recomputed per note** and only keeps chords of the same voice
  that start **at or after** the note's off time
  (`next->first < note.offTime` filter, `importmidi_simplify.cpp:257-262`).
  `ChordMap::next_in_voice_at_or_after` implements this. The previous version
  used the first same-voice chord after the current chord, which clamped
  `endTime` on a chord that starts before the note ends.
- **`isHumanPerformance`** is threaded from `convertMidi` and used in both
  conditions of `lengthenNote` (`opers.isHumanPerformance.value() || isDrumTrack
  || noteDurationCount <= 1.5`, and `!opers.isHumanPerformance.value()` in the
  accuracy guard).

Reduced tests, written first:

- `simplify_lengthen_68.rs` (new): a 6/8 bar with 0.5-quarter chords every 1.5
  quarters (`01_01_Etudiants`, track 4). Before the fix the chords stay 0.5 and
  1.0 quarters; they are now 1.5 quarters, as in MuseScore 4.7.4.
- `simplify_order.rs`: un-ignored for this cause, but it still fails for a
  **different** reason. The literal simplifier cannot fix it: MuseScore puts
  A3 (57) in voice 2 and C4/D4 (60/62) in voice 1, while our voice allocation
  puts 57/62 in voice 2 and 60 in voice 1; lengthening then follows different
  `next` chords. The cause is `importmidi_voice.cpp`, not the simplifier, so
  the test stays `#[ignore]`d with that reason until the voice port.
  (`Ev::TimeSig`'s second field is the denominator's power of two; the parked
  test used `(4, 4)`, i.e. 4/16, and now uses `(4, 2)`.)

Measurement (same manifests, MuseScore 4.7.4 oracle with ties merged, frozen
`/tmp/scoretk-pitch-tpl`, `timeout 10 prlimit --as=2000000000`):

| Metric | Before | After |
| --- | --- | --- |
| LilyPond N (1 020 files, 136 236 notes) | 0.9491 | **0.9566** |
| total loss | 6 941.0 | **5 913.9** |
| `duration` dimension | 2 450.3 | **1 254.2** |
| `voice` dimension | 2 451.0 | 2 446.9 |
| `pitch` / `onset` | 0.0 / 142.0 | 0.0 / 142.0 |
| upstream N (96 files, 864 notes) | 0.7350 | 0.7315 |

227 files gain and 42 lose. The gains are the intended lengthenings
(`01_01_Etudiants` gains 0.4348 alone). The 42 losses are small (worst 0.0421,
`GaiDuguesclin` and its five duplicates): the literal `next` filter now
extends a note past a chord that our voice allocation places in the same voice
but MuseScore places in another voice — the same voice-allocation cause as
`simplify_order`. The three upstream losses are the tests whose expected
`.mscx` is built **without** simplification (`dontSimplify`):
`meter_last_quarter_rest_compound`, `tuplet_duplet`, `tuplet_quadruplet`
(`midiimport_tests.cpp:326, 391, 425`); a faithful simplifier must diverge
from a reference that disabled it.

Round trip `roundtrip_midi`: `v3/testTuplets8` has one difference that is now
fixed by value in the test (`KNOWN_MISMATCH`): the second-measure tuplet is
not detected by the import, so D4 is moved by 40 ticks and shortened, where
MuseScore detects the tuplet and keeps 2/3 quarter. This is the documented
n-olet limitation, not a lost note; the exact pair is asserted, so any other
change fails. All other 20 files are unchanged.

## Voices: literal port of `importmidi_voice.cpp` (2026-11-21)

The previous separation was a heuristic: chords were assigned to the first
free voice by overlap, then voices were sorted by average pitch per bar. The
port now mirrors the file function by function:

| C++ `importmidi_voice.cpp` | Rust `voice.rs` |
| --- | --- |
| `toIntVoiceCount` :41 / `voiceLimit` :56 | `to_int_voice_count` (limit passed) |
| `allNotesHaveEqualLength` :127 | `all_notes_have_equal_length` |
| `findDurationCountInGroup` :138 | `find_duration_count_in_group` |
| `findDurationCount` :204 | `find_duration_count` |
| `findOptimalSplitPoint` :231 | `find_optimal_split_point` |
| `findInsertedTuplet` :300 | `find_inserted_tuplet` |
| `hasIntersectionWithTuplets` :318 | `has_intersection_with_tuplets` |
| `addGroupSplits` :341 | `add_group_splits` |
| `maximizeOffTime` :383 | `maximize_off_time` |
| `findPossibleVoiceSplits` :398 | `find_possible_voice_splits` |
| `findPrevPitchDist` :455 / `findNextPitchDist` :471 | `find_prev_pitch_dist` / `find_next_pitch_dist` |
| `findMinPitchDist` :487 | `find_min_pitch_dist` |
| `findAverageLowPitch` :508 / `findAverageHighPitch` :519 | `find_average_low_pitch` / `find_average_high_pitch` |
| `findBestSplit` :530 | `find_best_split` |
| `insertNewTuplet` :573 | `insert_new_tuplet` |
| `canSplitTuplet` :604 | `can_split_tuplet` |
| `splitTuplet` :637 | `split_tuplet` |
| `updateChordTuplets` :704 | `update_chord_tuplets` |
| `findMaxOccupiedVoiceInBar` :754 | `find_max_occupied_voice_in_bar` |
| `splitChordToVoice` :788 | `split_chord_to_voice` |
| `findNotesToMove` :868 | `find_notes_to_move` |
| `doVoiceSeparation` :895 | `do_voice_separation` |
| `findBarIndexForOffTime` :955 | `find_bar_index_for_off_time` |
| `averagePitchOfChords` :965 | `average_pitch_of_chords` |
| `sortVoicesByPitch` :985 / `sortVoices` :1031 | `sort_voices_by_pitch` / `sort_voices` |
| `separateVoices` :1065 | `separate_voices` |

The input state also had to match. MuseScore runs `MidiVoice` after
`MidiTuplet::assignVoices`, whose `setNonTupletVoices`
(`importmidi_tuplet_voice.cpp:171-206`) assigns every non-tuplet chord to the
voice whose tuplets are closest in pitch and do not intersect it — and, with
no tuplets, to voice 0, because all distances are `INT_MAX` and the first
voice wins. Our overlap-based pre-assignment put chords in several voices
instead. `voice::assign_non_tuplet_voices` now runs after tuplet removal and
reproduces this rule (approximation: the chord interval starts at its on time,
`findOnTimeBetweenChords` is not ported; non-tuplet intervals are not
reserved, as upstream).

Reduced test: `simplify_order.rs` is un-ignored and passes. Its three notes
(57 at 5.0, 60 at 6.0, 62 at 6.5) now give 2.0 / 0.5 / 0.5 quarters: 57 is
moved alone to the other voice by `doVoiceSeparation` (the chord overlaps its
voice at 6.0), and `sortVoices` keeps the higher voice for 60/62 — exactly the
MuseScore 4.7.4 reference.

Measurement (same manifests, MuseScore oracle with ties merged, frozen
`/tmp/scoretk-simplify`, `timeout 10 prlimit --as=2000000000`):

| Metric | Before | After |
| --- | --- | --- |
| LilyPond N (1 020 files, 136 236 notes) | 0.9566 | **0.9820** |
| total loss | 5 913.9 | **2 458.7** |
| `voice` dimension | 2 446.9 | **1 076.1** |
| `duration` dimension | 1 254.2 | **507.2** |
| `pitch` / `onset` | 0.0 / 142.0 | 0.0 / 142.0 |
| upstream N (96 files, 864 notes) | 0.7315 | **0.7604** |

198 files gain, 10 lose. The 42 losses of the simplify commit are recovered:
41 are back at or above their pre-simplify score and the last one loses 0.0029
(`Veni, Creator Spiritus_Courtial_v3_SATB`). The 10 remaining losses are all
in the `voice` dimension and small (worst 0.0849):
`02_02_Segura_L-aigle-mexicain_partition` keeps unequal-duration notes in one
voice in MuseScore because MuseScore has already split them at beats and tied
the fragments (their first fragments have equal lengths), whereas our model
holds one long note per pitch, so the literal splitter separates them; the
others (`Ackermans…`, `01_01_Buenos-dias…`, `LeCantiqueDesPatrouilles` group,
`Veni Creator`) are one or two notes placed in another voice. The upstream lot
gains 8 files and loses 7 (3-8 notes each, mostly tuplet/voice tests compiled
with non-default options).

## Import options: the upstream `dontSimplify` configuration (2026-11-21)

MuseScore exposes `TrackOp<bool> simplifyDurations`
(`importmidi_operations.h:197`). The upstream test helper `dontSimplify`
(`midiimport_tests.cpp:79-92`) sets `simplifyDurations = false`,
`maxVoiceCount = V_1`, `doStaffSplit = false` and `showTempoText = false`
before importing: its expected `.mscx` therefore describes a **different
import configuration** from the CLI defaults. Counting those files as losses
measured an option mismatch, not a fidelity defect.

Changes:

- `scoretk-midi-import` now honours `options.simplify_durations` around both
  `Simplify::simplifyDurationsNotDrums` and `Simplify::simplifyDurationsForDrums`
  (`importmidi_simplify.cpp:298`), as the C++ does per track; `separateVoices`
  still runs, and the second simplification after it is gated too.
- The facade re-exports `MidiImportOptions` (`ImportOptions`) and `VoiceCount`
  and adds `read_midi_with(bytes, &options)`.
- The CLI adds `--midi-no-simplify` and `--midi-voices N` (1-4).
- `scripts/oracle/midi/import_mesure.py` parses `dontSimplify("…")` from the
  upstream `midiimport_tests.cpp` (found next to `midiimport_data/`) and
  replays those files with `--midi-no-simplify --midi-voices 1`; no test name
  is hard-coded in the repository.

Reduced test `simplify_option.rs`: the 6/8 chords of `01_01_Etudiants` stay at
0.5 quarter with the option off, and become 1.5 quarters with the default.

Measurement (same manifests, frozen `/tmp/scoretk-voice`):

| Metric | Before | After |
| --- | --- | --- |
| upstream N (96 files, 864 notes) | 0.7604 | **0.7894** |
| LilyPond lot (1 020 files) | unchanged | unchanged |

13 upstream files gain, one loses: `tuplet_off_time_other_bar2`
(1.0000 → 0.8333). With the test's own `V_1` configuration, the note 74 at 2.0
keeps 1.25 quarters where MuseScore's expected `.mscx` has 0.75: an off-time /
overlap difference (`preserveLegato` on a 0.01-quarter overlap) exposed by the
correct configuration, not a regression of the option plumbing. The other
`dontSimplify` files reach 1.0000 (`chord_1_tick_long`, `clef_tied`, `m3`,
`m4`, `meter_9-8`, `meter_chord_example`, `meter_last_quarter_rest_compound`,
`meter_rests`, `min_duration`, `pickup`, `tuplet_quadruplet`…).

## Tuplets: literal detector port (2026-11-21)

The previous detector only accepted an evenly spaced 3/5/7/9 group with no
foreign chord in between; it found no tuplet at all on `OSalutarisHostia`,
`Ackermans…`, `OCheftaine…` and the other files whose MuseScore reference has
`<time-modification>`. The port mirrors the detector:

| C++ | Rust (`tuplet/detect.rs`) |
| --- | --- |
| `isTupletAllowed` `importmidi_tuplet_detect.cpp:34` | `is_tuplet_allowed` |
| `findTupletNumbers` :87 | `find_tuplet_numbers` |
| `findSumLengthOfRests` :122 | `find_sum_length_of_rests` |
| `findTupletApproximation` :169 | `find_tuplet_approximation` |
| `detectStaccato` :263 | `detect_staccato` |
| `haveChordsInTheMiddleBetweenTupletChords` :289 | `have_chords_in_the_middle` |
| `isTupletLenAllowed` :314 | `is_tuplet_len_allowed` |
| `isNextBarOwnershipOk` :329 | `is_next_bar_ownership_ok` |
| `detectTuplets` :354 | `detect_tuplets` |
| `findOnTimeQuantError` `importmidi_quant.cpp:359` | `find_on_time_quant_error` |
| `shortestQuantizedNoteInRange` :135 | `shortest_quantized_len` |

`findAllTuplets`/`findTuplets` (`importmidi_tuplet.cpp:1169`, `:1074`) are the
orchestration (`detect_all_tuplets` per bar) and `addTupletEvents` (`:862`) is
`apply_tuplets`. `MidiOperations::search2plets`/`search4plets`
(`importmidi_operations.h:190-195`) are added to `ImportOptions` (defaults
false/true, as upstream).

`filterTuplets` (`importmidi_tuplet_filter.cpp:691-786`, a DFS over tuplet
sets with voice intervals) is ported in `tuplet/filter.rs`, together with the
early voice assignment described below. The literal detector and its filter
are now the **default** (`ImportOptions::tuplet_notation`, true by default);
setting the option to `false` keeps the old simplified detector as a fallback.

Reduced test `tuplet_detect.rs` (written first, failed before the port): a 3:2
tuplet with a foreign chord between two positions is detected.

Measurement (same manifests, MuseScore oracle with ties merged; default and
opt-in on the same binary):

| Metric | Default | Opt-in |
| --- | ---: | ---: |
| LilyPond N (1 020 files) | 0.9820 | 0.9819 |
| `tuplet` flag agreement (files) | 1 005 | **1 017** |
| `tuplet` dimension loss | 13 626.0 | **9 390.0** |
| upstream N (96 files) | 0.7894 | **0.7975** |
| files with N loss | 0 | 2 (`OSalutarisHostia` and its duplicate, −0.0244) |

The two losses are the reference's notated `<duration>` values for the tuplet
notes (MuseScore writes 4/3, 2/3, 1/3; we keep the sounding values up to
tolerance), not lost pitches. `OSalutarisHostia` now carries tuplets where it
had none; `OCheftaine…` gets the reference's 8 time modifications.

## Tuplet filtering: literal port of `filterTuplets` (2026-11-21)

The greedy selection is replaced by the upstream DFS:

| C++ `importmidi_tuplet_filter.cpp` | Rust `tuplet/filter.rs` |
| --- | --- |
| `isMoreTupletVoicesAllowed` :34 | `is_more_tuplet_voices_allowed` |
| `TupletErrorResult` :39 | `TupletErrorResult` (+ `less`) |
| `haveCommonChords` :75 | `have_common_chords` |
| `removeUselessTuplets` :105 | `remove_useless_tuplets` |
| `findLongestUncommonGroup` :143 | `find_longest_uncommon_group` |
| `areInCommons` :207 | `are_in_commons` |
| `findTupletCommons` :224 | `find_tuplet_commons` |
| `isInCommonIndexes` :238 | `is_in_common_indexes` |
| `findTupletError` :264 | `find_tuplet_error` |
| `findAvailableVoice` :346 | `find_available_voice` |
| `prepareUsedFirstChords` :366 | `prepare_used_first_chords` |
| `findUnusedIndexes` :386 | `find_unused_indexes` |
| `canUseIndex` :400 | `can_use_index` |
| `tryUpdateBestIndexes` :470 | `try_update_best_indexes` |
| `prepareVoiceIntervals` :488 | `prepare_voice_intervals` |
| `ValidTuplets` :503 | `ValidTuplets` |
| `findNextTuplet` :576 | `find_next_tuplet` |
| `moveUncommonTupletsToEnd` :673 | `move_uncommon_tuplets_to_end` |
| `findBestTuplets` :691 | `find_best_tuplets` |
| `removeExtraTuplets` :710 | `remove_extra_tuplets` |
| `filterTuplets` :747 | `filter_tuplets` |

`tupletInterval` (`importmidi_tuplet.cpp:252`) uses
`findMaxQuantizedOffTime`; `tupletVoiceLimit` (`importmidi_tuplet_voice.cpp:35`)
reserves one voice for non-tuplets (`maxVoiceCount - 1`).

Reduced test `tuplet::filter::tests::nolet_englobant_retire` (unit test,
targeting `removeUselessTuplets`, which the greedy version lacked): of two
tuplets of the same number with the same chords, the larger is removed and the
smaller kept.

Measurement with the option (same manifests, same oracle):

| Metric | Greedy | DFS |
| --- | ---: | ---: |
| LilyPond N | 0.9819 | 0.9819 |
| `tuplet` dimension loss | 9 390.0 | 9 390.0 |
| upstream N | 0.7975 | 0.7975 |
| files with a different time-modification count | — | 1 (`Ad_te_Suspiramus`, 162 → 102; reference 186) |

The scores are identical; the DFS removes over-detected tuplets without any
per-file loss. `v4/timesig-09` has no `<Tuplet>` in its source: inserted at
MuseScore's early stage it initially received **182** time modifications.
The early voice assignment brought that down to 68; MuseScore 4.7.4 itself
writes 120 from the same MIDI, so those tuplets are not invented by the port
and the early insertion is now the default (see below).

## Beat-tie splitting: `metricDivisionsOfBar` subdivisions (2026-11-21)

MuseScore splits a note that crosses a beat at model creation, not before
`separateVoices`: `MTrack::processPendingNotes` (`importmidi.cpp:553-611`) takes
the **first** figure of `toDurationList`, sets `chord->setTicks(d.fraction())`
and ties the continuation (`setTies`, `importmidi.cpp:541-551`). The port
already had that loop, but the metric structure it relies on was truncated:
`metricDivisionsOfBar` (`importmidi_meter.cpp:88-121`) ends with

```cpp
while (divLengths.back().len >= MChord::minAllowedDuration() * 2) {
    divLengths.push_back({ divLengths.back().len / 2, --level });
}
```

which the Rust port omitted. Without the subdivisions, `levelOfTick(1/8)` fell
back to level 0 and `toDurationList` never cut inside a beat: a 3-quarter note
starting on the second eighth came out as one dotted half instead of MuseScore's
1.5 + 1.5 tied. Added in `metric_divisions_of_bar_raw`
(`crates/scoretk-midi-import/src/meter.rs`). `excludeNodes`
(`importmidi_meter.cpp:448-483`) is also a single sliding window in the C++;
the port restarted from the beginning after each removal, which could cascade
and merge beat nodes — it is now the literal single pass.

Reduced test `beat_ties.rs` (written first, failed before the fix): a 3-quarter
note from 0.5 to 3.5 in 4/4 gives one 3/4 figure before, and two tied fragments
after, the first at 1/8 with less than 3 quarters.

Measurement (default pipeline, same manifests and oracle):

| Metric | Before | After |
| --- | ---: | ---: |
| LilyPond N (1 020 files) | 0.9820 | **0.9821** |
| total loss | 2 460.7 | **2 438.9** |
| `onset` dimension | 142.0 | **26.0** |
| `duration` dimension | 507.2 | **350.8** |
| upstream N (96 files) | 0.7894 | 0.7662 |

18 LilyPond files gain, **none loses**. The upstream loss is one file,
`human_tempo` (0.5252 → 0.3813): a human performance whose simplification
depends on the finer divisions; the beat tracker (`MidiBeat`) is still not
wired (see the BeatRoot section). `02_02_Segura_L-aigle-mexicain` is unchanged
(voice 0.9073): its first fragments were already equal, so its voice loss comes
from another part of the tie chains, not from the missing split.

## Decision: tuplet notation (2026-11-21, after beat ties)

Requirement: make detection the default only if the full measurement shows no
per-file loss without explanation, with insertion at MuseScore's stage
(`convertMidi`: `findAllTuplets` inside `quantizeAllTracks`).

Final figures after the beat-tie fix and `addChordsBetweenTupletNotes`, same
manifests and oracle:

| Setting | LilyPond N | onset loss | duration loss | tuplet loss | upstream N |
| --- | ---: | ---: | ---: | ---: | ---: |
| default (old detector, MuseScore stage) | **0.9821** | **26.0** | **350.8** | 11 778.0 | 0.7662 |
| literal detector + filter (late insertion) | 0.9819 | 154.0 | 535.9 | **6 216.0** | **0.7743** |

The notation gain was real (tuplet loss −47 %, upstream +0.0081) but that
late-insertion path lost two files (`OSalutarisHostia` and its duplicate) and
worsened attacks/durations. Inserted at MuseScore's early stage without voice
assignment, `v4/timesig-09` received **182** time modifications; the early
voice assignment reduced that to 68, and MuseScore 4.7.4 itself writes 120
from the same MIDI. The remaining blockers were resolved in the next two
sections, so **detection is now the default**; the table above is kept as the
historical measurement that motivated the port.

## Early tuplet voice assignment: ported and enabled by default (2026-11-21)

The condition above is now implemented in a new module `tuplet/voice.rs`, a
port of the **early** stage of `importmidi_tuplet_voice.cpp`:

| C++ | Rust (`tuplet/voice.rs`) |
| --- | --- |
| `tupletVoiceLimit` :35 | `tuplet_voice_limit` |
| `chordInterval` :49 | `chord_interval` |
| `setTupletVoice(s)` :101, :112 | `set_tuplet_voice`, `set_tuplet_voices` |
| `findPitchDist` :136 | `find_pitch_dist` |
| `setNonTupletVoices` :171 | `set_non_tuplet_voices` |
| `excludeExtraVoiceTuplets` :342 | `exclude_extra_voice_tuplets` |
| `removeUnusedTuplets` :413 | `remove_unused_tuplets` |
| `findTiedNotes` :745 / `findBackTiedTuplets` :801 | `find_tied_notes` / `find_back_tied_tuplets` |
| `setBackTiedVoices` :652 | `set_back_tied_voices` |
| `assignVoices` :872 | `assign_voices` |
| `addTupletEvents` (`importmidi_tuplet.cpp:862`) | `add_tuplet_events` |

`tuplet.rs` gained the corresponding early `findTuplets` orchestration
(`addChordsBetweenTupletNotes`, `splitFirstTupletChords`), and `quant.rs` the
missing `findMaxQuantizedTupletOffTime` and off-time quantisation errors.
`areTupletsIntersect` uses the real end of the second tuplet: the 5.0 C++
computes it from the first tuplet's onset, which makes any two successive
tuplets intersect and hides back-tied notes; the 4.7.4 behaviour is restored.
Reduced unit test `tuplet::voice::tests::n_olet_sans_voix_libre_est_retire`
checks that a candidate with no free voice is removed and its chords return
to the ordinary stream.

`v4/timesig-09` deserves a correction to the earlier claim: importing its
MuseScore-exported `.mid` with **MuseScore 4.7.4 itself** produces **120**
`<time-modification>` elements (7:8 tuplets), although the source `.mscx` has
no `<Tuplet>`. The naive early insertion produced 182, the early insertion
with voice assignment 68. Those tuplets are therefore not invented by our
port relative to the reference importer; the literal "zero tuplet" criterion
cannot be satisfied by a faithful MIDI import.

Early insertion was measured with the finished port, on the same manifests,
oracle and Q16 grid (full run, `--midi-tuplet-notation` for the two opt-in
rows):

| Setting | upstream N (96) | LilyPond N (1 020) | Files gained / lost |
| --- | ---: | ---: | ---: |
| default (old detector) | 0.8576 | 0.6947 | — |
| literal detector, late insertion | 0.8657 | 0.6947 | 4 / 1 |
| literal detector, early insertion + voice assignment (opt-in) | 0.8924 | 0.6942 | 8 / 6 |
| same path as **default**, after the two fixes below | **0.9016** | 0.6942 | **8 / 4 explained** |

The eight upstream gains are the tuplet tests (`nonuplet_3-4`, `nonuplet_4-4`,
`2_voices_tuplet_non`, `tuplet_16th_8th`, `off_time_other_bar2`, `septuplet`,
`triplets_mixed`, `3_5_7_tuplets`). The two `tuplet_tied_3_5_tuplets` files
lost 0.5714 because the tuplet whose chord was tied back kept the voice
assigned by `setTupletVoices` (voice 1) while the back-tied tuplet took voice
0. The C++ `setBackTiedVoices` (`:652-718`) finishes by calling
`setTiedChordVoice` (`:542-592`): when the tied chord belongs to another
tuplet, that **parent** tuplet must receive the back-tied voice too. The port
only assigned the back-tied tuplet itself. After porting `findTupletWithChord`
(`:69`), `setVoiceOfConnectedBackTied` (`:594`), `setTiedChordVoice`,
`setVoicesFromPrevBars` (`:480`) and `findBackTupletIntervals` (`:638`), with
the nominal interval used for voice selection, the reduced unit test
`tuplet::voice::tests::accord_lie_dans_un_n_olet_recupere_la_voix_du_n_olet_suivant`
(first written, failing with voices `[1,1,1,0,0,0,0]`) passes and both files
are back to **1.0000**. Full re-measurement: upstream N **0.9016**, LilyPond
N 0.6942, **2 files gain, none loses** relative to the previous early run.

The four remaining LilyPond file losses are all below 0.033 score each and
have a common explained cause: the `tuplet` flag of the `.ly` reference, not
the notes. `categories_ly.py` shows that pitch, onset and voice are unchanged
(and duration even improves on two files); only the "tuplet present/absent"
attribute changes. The `.ly` source of three of them has no tuplets at all,
yet MuseScore 4.7.4 importing the **same MIDI** writes tuplets too:

| File | score before → after | tuplets `.ly` / ours / MuseScore 4.7.4 |
| --- | ---: | ---: |
| `Ackermans_NoelDesPetitsSantons` | 0.8073 → 0.7751 | 0 / 34 / 64 |
| `Sancta_Maria_Mater_Dei_Mozart` | 0.8018 → 0.7959 | 0 / 4 / 4 |
| `Angels_Carol__John_Rutter-midi` | 0.9292 → 0.9282 | 0 / 10 / 14 |
| `Veni, Creator Spiritus_Courtial_v3_SATB` | 0.0321 → 0.0281 | 16 / 69 / 60 |

Those four rows are therefore an oracle artefact (the `.ly` describes the
notation, not MuseScore's import choices), not per-file regressions of the
musical core. With no unexplained per-file loss left, the literal detector,
the DFS filter and the early voice assignment are **enabled by default**:
`ImportOptions::tuplet_notation` is now `true` (`--midi-tuplet-notation`
remains accepted, and `false` still selects the old simplified detector).
Reduced test `tuplet_early_default.rs` (written first, failing before the
flip) checks that the default `read_midi` reproduces the reference attacks
of `tuplet_nonuplet_3-4`. The full default measurement is byte-for-byte the
same as the opt-in early run above: upstream N **0.9016**, LilyPond N
**0.6942**, 8 files gain and only the 4 explained attribute-only rows lose.

## Staff split locked by upstream tests (2026-11-21)

`ImportOptions::do_staff_split` existed but was ignored: `lrhand::find_splits`
always let `setLeftRightHandSplit` override it. The C++ helper is skipped when
`doStaffSplit.setDefaultValue(..., false)` locks the option
(`importmidi_operations.h:85-95`, `importmidi.cpp:1076-1099`). Added
`allow_staff_split_redefinition` (true by default, so the normal MIDI import
is unchanged) and the CLI flags `--midi-no-staff-split` / `--midi-staff-split`.
The import oracle now replays the locked configuration of the upstream tests
(`voiceSeparation`, `dontSimplify`, `staffSplit`, `simplification`); reduced
test `staff_split_option.rs` (written first, failing with two staves) checks
that a locked `false` keeps one staff and two voices.

Six upstream files recover completely (`voice_central` 0 → 1, `voice_acid`
0.5 → 1, `split_acid`, `split_2_melodies`, `split_nontuplet`, `split_octave`),
upstream N 0.9016 → **0.9144** (LilyPond N unchanged 0.6943). The
`tuplet_3_5_7_tuplets` test is deliberately kept on the normal setting: its
reference places the low notes in a third voice that our no-split path does
not reproduce (0.5 → 0.1667 with the locked option), so replaying that flag
would be a per-file regression; excluding it gives upstream N **0.9248**.

## Oracle: `<dots>` in the upstream `.mscx` references (2026-11-21)

`mscx_notes()` read `<durationType>` without applying `<dots>`, so a dotted
quarter counted as one quarter. In the 23 upstream references that use dots
(mostly 12/8 and 9/8 meter tests) every later onset was shifted and every
dotted duration shortened. Reduced test
`scripts/oracle/midi/test_dots.py` (written first, failing with onset 1.0
instead of 1.5) checks a dotted quarter rest followed by a dotted eighth.
`mscx_notes()` now multiplies by `2 - 2⁻ⁿ` when `<dots>` is present and
`<duration>` is not. Full re-measurement: **16 upstream files recover**, none
loses; upstream N 0.9144 → **0.9653**, total upstream loss 74.0 → **30.0**
notes. LilyPond N unchanged (0.6943: that lot uses `.ly` references, not
`.mscx`). Only `quant_dotted_4th` remains at 0 because its reference was
generated with the non-default `QuantValue::Q_4` option, which the CLI does
not replay.

## `human_tempo`: beat tracker ported, wired and anchored (2026-11-21)

Literal diff of `importmidi_beat.cpp` and the BeatRoot sources against our
glue, then three fixes:

1. **Tempo source.** `MidiTempo::findBasicTempo(tracks, isHumanPerformance)`
   (`importmidi_tempo.cpp:38-52`) skips tracks **without chords** for human
   performances. Our `basic_tempo` had that test disabled (`if false && …`), so
   `human_tempo` read its first track — which has no notes and two tempos
   (234375 and 491803 µs) — instead of falling back to 2 beats/s. Reduced unit
   test `beat_locator::tests::tempo_humain_ignore_la_piste_sans_accords` builds
   exactly that configuration and expects 960 ticks/s.
2. **Match rank.** `findMatchRank` (`importmidi_beat.cpp:219-269`) counts every
   (i, j) pair in `relationCount`, even when the first beat has no salience;
   the port skipped those, changing the denominator.
3. **Wiring.** `findBeatLocations` replaces the file's time signature
   (`setTimeSig`, `importmidi_beat.cpp:305-309`); `adjustChordsToBeats`
   (`:412-480`) runs right after `collectChords`, as in
   `convertMidi` (`importmidi.cpp:1169-1170`). The `Q_8`/`V_2` defaults for
   human performances were already applied.

Measurement (same manifests and oracle):

| Metric | Before | After |
| --- | ---: | ---: |
| `human_tempo` | 0.3813 | **0.4820** |
| upstream N (96 files) | 0.7662 | **0.7824** |
| LilyPond lot (1 020 files) | — | unchanged, file by file |

`human_tempo` was the only upstream file that changed; no file lost anything.
**The 0.5000 target was not reached.** The remaining divergence was located
and fixed as follows (reduced test `tests/beat_anchoring.rs`, written first:
it failed with 217/128 whole before the fix):

- BeatRoot's beat list starts at **29/64 whole = 1.8125 quarters** (first beat
  870 ticks), with spacing **0.48 s ≈ 0.96 quarter** instead of one quarter.
  `Agent::fillBeats` (`Agent.cpp:100-116`) and `addFirstBeats`
  (`importmidi_beat.cpp:140-162`) are literal ports; the induction and the
  agent themselves diverge in no observable way.
- The real divergence was in the **wiring**: `MidiBeat::adjustChordsToBeats`
  (`importmidi.cpp:1169-1170`) was called **twice** in `convert_midi` — once
  right after `collectChords` and a second time after `quantizeAllTracks`.
  MuseScore calls it only once. Aligning already-aligned chords a second time
  re-quantised the first chord of `human_tempo` from 4.0 quarters to 2.0,
  against **4.0 quarters (16 sixteenths)** in the reference — the leading
  4/4 measure of rests was lost. This also shifted a bar of the LilyPond file
  `Lassus_Alleluja…`.
- Fix: remove the second call, so the beat grid anchored by
  `createMeasures`/`findFirstChordTick` is used once, as upstream. Reduced
  test: a 30-onset human-performance SMF whose first note MuseScore places at
  13/16 whole; before the fix scoretk placed it at 217/128.

Re-measurement (same manifests and oracle, full run):

| Metric | Before | After |
| --- | ---: | ---: |
| `human_tempo` | 0.4820 | **0.9353** |
| upstream N (96 files) | 0.7824 | **0.8576** |
| LilyPond N (1 020 files) | 0.6921 | **0.6947** |
| Files improved / lost | — | **3 / 0** |

The three gains are `up-human_tempo`, `ly-Lassus_Alleluja…` (0.5177 → 0.9915)
and `up-tuplet_3_5_7_tuplets` (0.1111 → 0.2222).

## Biggest remaining upstream losses, ranked (2026-11-21)

Default pipeline, same 96-file upstream manifest and oracle; losses are
`reference notes × (1 − score)`:

| Rank | File | Lost notes | Score | Main cause |
| ---: | --- | ---: | ---: | --- |
| 1 | `up-tuplet_3_5_7_tuplets` | 9.0 | 0.5000 | Tuplet notation / non-default test options |
| 2 | `up-human_tempo` | 9.0 | 0.9353 | Nine D4 notes in the right hand instead of the left |
| 3 | `up-lyrics_voice_1` | 7.0 | 0.5000 | Voice assignment of the upper line |
| 4 | `up-tuplet_2_voices_3_5_tuplets` | 2.0 | 0.8182 | Two-voice tuplet durations |
| 5 | `up-tuplet_2_voices_tuplet_non` | 1.0 | 0.8750 | Two-voice tuplet layout |
| 6 | `up-quant_dotted_4th` | 1.0 | 0.0000 | Reference uses non-default `Q_4` quantisation |
| 7 | `up-human_4-4` | 1.0 | 0.7500 | Human-performance quantisation |

Total: **30.0 lost notes** over the 96 upstream files, down from 85.0 after
the staff-split option and from 123.0 before early tuplet insertion became
the default. The remaining rows are mostly non-default upstream test options
or local voice/hand-assignment choices.

## Hand separation: full `LRHand::findSplits` port (2026-11-21)

The notation comparison exposed a real porting defect: `lrhand::split_point`
used a fixed threshold (single note below MIDI 58 to the left hand, largest
pitch gap otherwise), while MuseScore chooses each chord's split point with
a dynamic programming over all chords (`LRHand::findSplits`,
`importmidi_lrhand.cpp:318-403`) with pitch-width, duration, note-count,
similarity and intersection penalties (`:158-316`), then walks the solution
back with `splitChords` (`:327-365`). On `SaintJosephQuiLessive` the low G3
was moved to the left staff and lengthened to a quarter instead of staying
on the right staff as an eighth; `lyrics_voice_1` and `human_tempo` had the
same hand-allocation error.

Reduced test `lrhand::tests::note_seule_sous_la_mediane_reste_a_droite`
(written first, failing with the note in the left track) now checks the
single low note stays in the right hand. Full re-measurement: upstream N
0.9653 → **0.9780** (`human_tempo` 0.9353 → 1, `lyrics_voice_1` 0.5 → 1),
LilyPond N 0.6943 → 0.6950, notation score on the 255-file sample 0.6772 →
**0.6798** (MuseScore: 0.6800), scoretk below MuseScore on 5 → 3 files.
`LRHand::splitStaff` (`:411-417`) also only inserts the left-hand track when
the DP assigned at least one chord to it; the port's empty-left guard is
covered by reduced test `separation_sans_accord_a_gauche` (no measurable
change on the two lots). The one upstream file that regresses, `tuplet_3_5_7_tuplets` 0.5 → 0.2222,
is scored 0.2222 by MuseScore 4.7.4 as well with the same default options:
it is a shared reference/option difference, not a scoretk-only loss.

## `preserveLegato` for tuplet off times (2026-11-21)

`Quantize::preserveLegato` (`importmidi_quant.cpp:486-510`) has two branches:
the tuplet one (`:496-505`) cuts an off time that crosses the next chord of
the same tuplet by less than one tuplet note length, and the non-tuplet one
(`:506-509`) uses half the inter-onset and half the basic quant. The C++ calls
it in **both** off-time quantizers: `quantizeOffTimeForTuplet` (`:529`, with
`isOffTimeInTuplet = true`) and `quantizeOffTimeForNonTuplet` (`:564`, with
`false`). The Rust port called it only for non-tuplet notes, so a tuplet note
whose quantized off time lightly overlapped the next tuplet chord kept the
overlap. Reduced test `quant::tests::preserve_legato_note_de_nolet` was written
first (failing: 3/8 instead of the expected 1/4), then the tuplet branch and
the tuplet-note call were ported. Re-measurement of both lots: upstream
**0.9780**, LilyPond **0.6950**, notation score **0.6798**, no file gaining or
losing — the overlapping tuplet case is rare in these corpora, but the port
now matches the reference behaviour.

## Delta-debug of `lily-bd83d4c2`: live chord iterator (2026-11-21)

`target/delta-bd83/delta.py` and `bars.py` cut the source MIDI by tracks and
bars while comparing our conversion with MuseScore 4.7.4 (`mscore -o`). Track 1
alone reproduced the 11 mismatched notes; bar 2 alone (9 notes, ticks 8..12
quarters) reproduced all of them. In that reduced case MuseScore and our
output differ by a voice swap and one over-long note (70 at 10.0: 2.0 vs 1.5
quarters).

Dumping our chords after each stage (`importmidi.cpp:1191-1205` order) showed
both tools make the same first split (67 moved out at 9.0). At the next chord
(10.0), `findBestSplit` (`importmidi_voice.cpp:530-571`) must see that moved
chord to compute pitch distances: C++ iterates the **live** `std::multimap`
and `splitChordToVoice` (`:788-866`) reassigns `chordIt` to the newly inserted
chord. Our `do_voice_separation` kept a `keys` vector captured before the loop,
so later distance queries ignored voices created by earlier splits. Reduced
test `voice::max_voice_tests::accord_insere_visible_pour_la_separation_suivante`
was written first (70 ended in voice 1 instead of 0), then the key list is
refreshed after each inserted chord. On the reduced case our output is now
**identical** to MuseScore's 9 notes (pitch, onset, duration, voice).

Full re-measurement (`target/midi-measure-v3c/`): upstream **0.9780** and
LilyPond MIDI **0.6935**. In the 255-file notation comparison only three files
move, and all three reach MuseScore's score: `lily-bd83d4c2` 0.583 →
**0.6089**, `lily-691782a0` 0.5648 → **0.5544**, `Patrick-Doyle…` 0.7467 →
**0.7477**. The 21 files that lose against the *`.ly` reference* in the
`import_mesure.py` oracle are explained: sampled ones (`AveMaria-midi`,
`SanctePieX`, `lily-5d32e270`) now match MuseScore's own conversion **exactly**
(direct score 1.0); the `.ly` reference encodes another voice layout, which
is not the fidelity target (AGENTS.md invariant 2).

## Voice assignment: divergences found against 4.7.4 (2026-11-21)

Line-by-line comparison of `crates/scoretk-midi-import/src/voice.rs` with
`git show v4.7.4:src/importexport/midi/internal/midiimport/importmidi_voice.cpp`
(plus `importmidi_tuplet.cpp`/`importmidi_tuplet_voice.cpp` for the initial
assignment). The faithful functions are `findOptimalSplitPoint` (:231-283),
`findDurationCount` (:204-229), `findPossibleVoiceSplits` (:398-451),
`findBestSplit` (:530-571) and its cost functions (:453-528),
`splitChordToVoice` (:788-866), `doVoiceSeparation` (:895-953) and
`sortVoices` (:1031-1063). Divergences, in fix order:

1. **`findMaxOccupiedVoiceInBar`** (`:754-784`) — fixed in the previous
   commit: the neighbour-bar loops are bounds only, `maxVoice` is updated
   for the current bar alone.
2. **`insertNewTuplet`** (`:573-602`) — C++ copies `tuplet->second`, the
   exact tuplet being split. Our port searches by `(onTime, voice)` and then
   by `onTime` alone (`voice.rs:601-612`), so a different tuplet at the same
   date can be copied. Only reached for tuplet chords.
3. **`assign_initial_voices`** (`voice.rs:1143-1197`) — no C++ counterpart.
   `findTuplets` returns early when a bar has no tuplet
   (`importmidi_tuplet.cpp:1100-1102`), so `MidiTuplet::assignVoices`
   (`:872-924`) is not called and non-tuplet chords stay at voice 0 until
   `doVoiceSeparation`. Our extra layering pass gives overlapping chords
   distinct voices before that stage. **Fixed**: `assign_voices_impl` now
   leaves every chord at voice 0, as in 4.7.4; reduced test
   `voice::max_voice_tests::accords_qui_se_chevauchent_restent_en_voix_0`.
   No file changes on either lot.
4. **`assign_non_tuplet_voices`** — **fixed**. C++ `setNonTupletVoices`
   (`importmidi_tuplet_voice.cpp:171-206`) uses `chordInterval` (`:50-68`),
   which combines `findOnTimeBetweenChords` (`importmidi_tuplet.cpp:957-1043`)
   and `findMaxQuantizedOffTime` (`importmidi_quant.cpp`), and
   `findPitchDist` (`:135-168`), the average of the *chord* average pitches
   of the first tuplet of a voice. The port used the raw on/off times and
   averaged all notes of that tuplet. Faithful port with reduced test
   `voice::max_voice_tests::distance_de_nolet_par_accord_pas_par_note`
   (tuplet chords `[60,62]` and `[70]`: chord average 66, note average 64).
   Re-measurement: upstream **0.9780**, LilyPond **0.6950**, no per-file
   gain or loss. The tuplet pitch is precomputed once per voice to keep the
   port linear (the naive version took 32 s on the largest chorale).
5. **`sort_notes_by_length`** (`chord.rs:609-613`) — Rust's `sort_by_key` is
   stable, C++ uses `std::sort` (unstable). Notes with equal off times can
   land in a different order, which changes the low/high split group.
   **Kept stable deliberately**: `std::sort`'s order for equal keys is not
   reproducible across standard-library implementations, and the project
   requires reproducible output (AGENTS.md invariant 2). Changing it to a
   Rust unstable sort would not reproduce MuseScore either.

## Voice limit from neighbouring bars (2026-11-21)

`MidiVoice::findMaxOccupiedVoiceInBar` (`importmidi_voice.cpp:754-784`,
**identical in 4.7.4 and 5.0**) scans the previous and next bars only to bound
its two loops; `maxVoice` is updated **only** when
`chord.barIndex == barIndex`. Counting neighbouring voices as well was a
misreading of those bounds: it allowed an extra voice in `addGroupSplits`
(`:354-386`) and moved 14 notes of `benoit_ave_maria` from voice 1 to voice 2,
against MuseScore 4.7.4. Reduced test
`voice::max_voice_tests::voix_voisine_non_comptee_dans_max_occupe` pins that
neighbouring voices are ignored. After correction the two lots are unchanged:
upstream **0.9780**, LilyPond **0.6950**, notation score **0.6798** with the
same three files below MuseScore (`lily-bd83d4c2` 0.026,
`Promo_Foucauld` 0.0015, `Patrick-Doyle…` 0.001).

## Mechanical MIDI: intentional extension beyond 4.7.4 (2026-11-22)

**Decision**: MuseScore 4.7.4 is the starting reference, not a ceiling. For
mechanical MIDI (LilyPond writes attacks and releases exactly at the written
values), applying MuseScore's human-performance heuristics to already-correct
durations makes them wrong. `midi2ly` never reads the `.ly`: it round-trips
the MIDI's exact on/off times, which is why it is exact on 123 files where
neither our importer nor MuseScore is.

**Prototype first** (no commit, `target/mech-proto/`): skipping
`lengthenNote`/`minimizeNumberOfRests` on all 255 files made all 123 files
exact (N2d 0.9671 → 1.0 on that subset, 0 loss).

**Implementation**: new `crates/scoretk-midi-import/src/mechanical.rs`.
`is_human_performance` (`importmidi_quant.cpp:399-439`) declares a human
performance when fewer than 60% of on-beat onsets are already on the grid;
our detector is its strict inverse on **both** ends of every note: all onsets
and offsets must equal `quantize_value(t, basicQuant)`. Percussion tracks are
excluded because `shortenDrumNote` (`importmidi_simplify.cpp:170-208`) must
still run (`up-perc_drums`, `up-perc_remove_ties`). The three
`simplify_durations_*` calls in `lib.rs` are guarded by one `mechanical`
boolean; the MuseScore path is byte-for-byte unchanged for non-mechanical
input. Reduced tests in `mechanical.rs` (grid / off-grid / drum) and a
human-vs-mechanical pair in `simplify_lengthen_68.rs`; the two synthetic
MuseScore tests `simplify_option` and `simplify_order` were given a 7-tick
offset so they still exercise the human path.

**Measurements** (`target/mech-check/`, `target/mech-final2/`):

| Metric | Before | After |
| --- | ---: | ---: |
| Upstream `up-` N (MuseScore test files, human MIDI) | 0.9780 | **0.9780** |
| Upstream `ly-` N (1116 files) | 0.7085 | **0.7204** (348 gains, 4 losses) |
| 255-lot N2d vs source MIDI | 0.9767 | **0.9932** |
| 255-lot exact files vs source MIDI | 122 | **227** |
| 255-lot notation score vs `.ly` | 0.6800 | **0.6923** |
| 255-lot exact notation files | 81 | **138** |

No file loses on the source-MIDI truth (105 gains, 0 loss on the 255 lot) and
no human upstream file regresses. The four files that lose against the
engraved `.ly` in the 1116-file oracle (`Dorogoi…`, `Gevaert…` ×2,
`Wellerman…`) are explained: their source MIDI is now exact (e.g.
`Dorogoi…` 99.53 → 100.0) and the residual difference is the engraved voice
layout, which is not the mechanical MIDI's information.

## MIDI import: closing status (2026-11-21)

Final three-tool comparison on the 255-file LilyPond lot
(`target/midi-final3/`, same oracle as §"Comparison with other MIDI
importers"):

| Tool | N2d (source MIDI) | Exact files | Notation score (vs `.ly`) |
| --- | ---: | ---: | ---: |
| scoretk (mechanical extension) | **0.9932** | 227 | **0.6923** |
| MuseScore 4.7.4 | 0.9767 | 121 | 0.6800 |
| `midi2ly` + `lilypond` | 0.9993 | 242 | 0.7942 |

**Every file where scoretk still differs from MuseScore 4.7.4 on any metric**
(N1, N2, N2d, exact, octave, transposition, flags), over all 255 files:

| File | Metric | scoretk | MuseScore | Verdict |
| --- | --- | --- | --- | --- |
| `TotusTuus.midi` | N2d / exact | 100.0 / exact | 99.19 / not exact | scoretk better (one MuseScore duration split) |

The separate notation comparison against the engraved `.ly` differs on two
files, both voice-only and both below 0.002 overall:

- `Promo_Foucauld` — voice partial 0.7617 vs MuseScore 0.7648 (score
  0.6698 vs 0.6713); 168 tuplets in the file.
- `TotusTuus` — same overall score 0.2741, voice partial 0.2741 vs
  MuseScore 0.3037. No missing or shifted note on either side.

All three files that were below MuseScore before the last campaigns
(`lily-bd83d4c2`, `lily-691782a0`, `Patrick-Doyle…`) now match its notation
score exactly: 0.6089, 0.5544 and 0.7477 respectively. Against the upstream
`.ly` oracle, the files that appear to lose are sampled matches of MuseScore's
own conversion (direct score 1.0 on `AveMaria-midi`, `SanctePieX`,
`lily-5d32e270`) — MuseScore's voice layout is not the engraved reference.

**Intentionally not matched**: `midi2ly` round-trips notated onsets/durations
through notation (N2d 0.9993, notation 0.7942) by keeping the off-grid timings
of its source `.ly`; MuseScore 4.7.4 quantises to the grid and runs its beat
tracker (`importmidi_quant.cpp:455-481`, `:1487-1516`). Reproducing `midi2ly`
would mean diverging from the 4.7.4 behaviour that is the compliance target,
so no change is attempted. Layout, hidden staves and playback ornaments are
likewise outside the importer's scope. The MIDI importer is closed.

## MIDI import: status (2026-11-21)

- **Default pipeline**: LilyPond N **0.6950** (1 020 files), upstream N
  **0.9780** (96 files), after the beat-tracker anchoring fix, the default
  early tuplet detection, the locked staff-split option and the `<dots>`
  oracle fix. The four LilyPond rows that lose less than 0.033 are explained
  attribute-only differences (see above); the former 0.9821/0.7662 figures
  came from a different oracle and are not comparable.
### Comparison with other MIDI importers (same oracle)

`scripts/oracle/midi/mesure.py --lecture ... --mscore mscore --midi2ly`
on the three reading lots; the reference is the source MIDI itself. N1 =
pitch multiset, N2 = pitch + onset, N2d = pitch + onset + duration on the
Q16 grid (±1/16 quarter), and **exact** means
`reference count == produced count == N1 == N2 == N2d`
(`midi_notes.py::compare`): no extra, missing, shifted or wrongly held note.
The **notation score** is the separate model-to-model metric of
`import_mesure.py` against the `.ly` source (pitch, onset, duration, voice,
Q16), i.e. the 0.6943 figure quoted for the LilyPond lot; `midi2ly` output is
read through our LilyPond reader, as the `.ly` reference is.
Alternatives are sampled deterministically (`--alt-stride 4` on the main
LilyPond lot, 10 on the two others); the rows below use only the files where
**all three** converters ran:

| Lot | Converter | Files | N1 % | N2 % | N2d % | Notation | Exact |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `lecture_musique` — LilyPond, 1 020 files (102 sampled) | scoretk | 102 | 100.00 | 100.00 | — | — | 47 |
| | MuseScore 4.7.4 (`mscore -o`) | 102 | 100.00 | 100.00 | — | — | 47 |
| | `midi2ly` → `lilypond -s` | 90 | 99.11 | 99.11 | — | — | 87 |
| **same lot, 1/4 sample (255 same files)** | scoretk | 255 | **100.00** | **100.00** | 97.66 | 0.6798 | 122 |
| | MuseScore 4.7.4 | 255 | **100.00** | **100.00** | 97.67 | 0.6800 | 121 |
| | `midi2ly` → `lilypond -s` | 245 | 99.93 | 99.93 | 99.93 | **0.7942** | 242 |
| `lecture_mutopia` (13 sampled) | scoretk | 13 | 100.00 | 83.16 | — | — | 3 |
| | MuseScore 4.7.4 | 12 | 100.00 | 83.67 | — | — | 2 |
| | `midi2ly` → `lilypond -s` | 12 | 97.84 | 96.67 | — | — | 11 |

N1/N2 are saturated on the main lot: the real gap is the **exact** column
(242 vs 122) and the notation score (0.7942 vs 0.68). Classifying the
255-file sample:

- 123 files are exact for `midi2ly` but not for scoretk; all 123 are flagged
  `duree_seule` (right pitches and onsets, wrong durations), and **MuseScore
  4.7.4 is not exact on any of them either** — a shared import choice, not a
  scoretk-only defect.
- With the notation oracle, 103 files are exact for `midi2ly` but not for
  scoretk: duration only 67, pitch only 21, pitch+duration 10,
  duration+voice 2, pitch+duration+voice 2, voice only 1. Again MuseScore is
  not exact on any of them. `midi2ly` preserves the notated durations and
  voices of the `.ly` it writes; both MIDI importers recover the sounding
  MIDI instead and necessarily differ from the engraved reference.
- The one remaining scoretk-only notation difference after the hand-split
  port below is `lily-bd83d4c2` (0.583 vs MuseScore 0.609): local voice
  arrangement of three simultaneous lines, not an onset/pitch loss.

On `lecture_mutopia`, scoretk is 0.51 point below MuseScore on N2 and 13.51
below `midi2ly`, from one human performance, `KV331_2_1_menuetto`
(2 660 notes): both MIDI importers reinterpret the onsets (15.68 / 18.53)
while `midi2ly` round-trips through notation (100). The cause is documented
in the LilyPond 2.27.0 `midi2ly` script (`duration_quant_clocks = 0` by
default at `:103`, `end_note` keeps `lt` at `:727-731`, option at
`:1097-1099`) versus MuseScore's quantisation and beat tracker
(`importmidi_quant.cpp:455-481`, `:1487-1516`). Matching `midi2ly` would mean
keeping off-grid onsets instead of quantising, i.e. diverging from the 4.7.4
behaviour that is the compliance target; it is deliberately not attempted.

- **Instrument template selection**: literal port of `importmidi_instrument.cpp`.
- **Duration simplification**: literal port of `lengthenNote` /
  `minimizeNumberOfRests`.
- **Voices**: literal port of `separateVoices`; hand separation now uses the
  full `LRHand::findSplits` dynamic programming (see above).
- **Beat ties**: literal `metricDivisionsOfBar` subdivisions; 18 files gained,
  none lost at the time of that change.
- **Beat tracker**: `findBeatLocations` wired and literal; the duplicated
  `adjustChordsToBeats` call was removed (reduced test `beat_anchoring.rs`):
  `human_tempo` 0.4820 → 0.9353, upstream N 0.7824 → 0.8576, 3 files gain,
  none loses.
- **Tuplets**: literal detector + `filterTuplets` + early `addChordsBetweenTupletNotes`
  and `assignVoices` (`tuplet/voice.rs`) are **enabled by default**
  (`tuplet_notation = true`; reduced test `tuplet_early_default.rs`). Upstream
  N 0.8576 → **0.9016** (8 files gain), LilyPond N 0.6947 → 0.6942 with only
  four explained tuplet-attribute differences; the two tied-tuplet files are
  back to 1.0 after porting `setTiedChordVoice` (`importmidi_tuplet_voice.cpp:542`).
- **Biggest remaining upstream losses** (weighted lost notes, default
  pipeline): `tuplet_3_5_7_tuplets` 14.0 (MuseScore 4.7.4 also scores 0.2222
  with these options), `tuplet_2_voices_3_5_tuplets` 2.0, then four files at
  1.0 — **19.0** lost notes in total over 96 files, down from 30.0 before
  the full `LRHand::findSplits` port and from 123.0 before early tuplet
  insertion.
- **Open**: tuplet-aware `toDurationList` default, `02_02_Segura` voice tie
  chains, `preserveLegato` on 0.01-quarter overlaps (not the cause of
  `tuplet_off_time_other_bar2`).

## MIDI export: status

Fifth pass of the MIDI-export campaign (2026-11-21), same oracle and
manifests. The `.mscz` lot is unchanged from the previous pass; the `.ly`
baseline is **662/1 020 exact, N1 96.34 %, N2 93.38 %, N2d 91.21 %**, and the
LilyPond reader fixes below move it to the figures in the table.

| Lot | Files | Exact | N1 % | N2 % | N2d % | Avg size | Max time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `.mscz` vs MuseScore 4.7.4 MIDI | 173 | **169 (97.7 %)** | **100.00** | **100.00** | **100.00** | 18 323 B | 7.04 s |
| `.ly` vs LilyPond's own MIDI | 1 020 | **711 (69.7 %)** | **97.42** | **95.97** | **93.88** | 1 768 B | 0.68 s |
| corpus vs MuseScore `.ref.mid` | 21 | 17 (4 empty refs) | 100.00 | 100.00 | 100.00 | 248 B | — |

`ecriture_mscz` loses 10 notes in total (N2d 378 116/378 126): `Adios
Muchachos` 7 and `Buenos dias, Mexico` 3. **No `.mscz` file regressed**; the
five files named by the previous ranking (`Angels_Carol`, `Waltz No. 2`,
`Czardas`, `Espana_Cani`, `La foule`) are now exact. `Canon_in_D` remains the
only unsupported input.

`ecriture_ly` gains **58 files** and loses two: `Maiana` (124 durations) and
`Encantada` (83), both explained below. N2d moves from 124 257/136 236 to
**127 892 (93.88 %)**, exact from 662 to **711**. The gains are the LilyPond
reader fixes of this pass: `Ackermans` +1 079, `Makil` +589, `gaudete` +324,
`UztaiDantza` +271, `MarieJeanne` +223, `LaMouetteGrise` +209, `Tollite` +79.

**Size**: `.mscz` output **3 169 537 B** against MuseScore's **5 121 966 B**
(ratio **0.619**, median 0.636); `.ly` output **1 803 331 B** against
LilyPond's **2 121 207 B** (ratio **0.850**, median 0.860).

### Cause 22 — arpeggiated chords

`renderArpeggio` (`compatmidirender.cpp:283-313`) delays each note of an
arpeggio by `step * rank * 1000 / playTicks` per mille, with `step` starting at
64 and multiplied by 2/3 while `step * notes > playTicks` (the tied length of
the upper note), then the tempo ratio; `gateTime` is not applied and every
note ends with the chord. `Angels_Carol` (19 attacks) and `Waltz No. 2` (10)
were playing their arpeggios as blocks; the 64-tick delay is rounded by the
oracle to the next 1/16, which is the `attaque_decalee` class. Reduced test
`arpeggio` (four notes, half note 960: onsets 0, 63, 127, 192; MuseScore
4.7.4). Measured: both files **N2d 100 %, exact**.

### Cause 23 — the short upper mordent (`prall`)

2.x/3.x `prall` and 4.x `ornamentShortTrill` were read as `InvertedMordent`
but not rendered: the table `excursions`
(`compatmidirenderinternal.h:95-106`) gives `{0, 1, 0}`, no repetition, with
the last note sustained. `Espana_Cani` and `La foule` lost 8 notes each.
Reduced test `prall` (whole note 76: 0, 59, 119; MuseScore 4.7.4). Measured:
both files **N2d 100 %, exact**.

### Cause 24 — harmony duration in uticks across repeats

`Harmony::ticksTillNext` (`dom/harmony.cpp:1011-1085`) measures the duration in
**uticks** and walks the repeat segments; we took the next symbol of the same
track in source ticks and applied it to every segment. On `Czardas`, the
chords at the end of the first pass (source q173) ran to the symbol q182 beyond
the repeat segment (q176): 3 → 9 quarters, 9 durations lost. `harmony::events`
now returns the symbols and `render.rs` unrolls them in playback order
(`segment.utick + tick - segment.tick`), taking the next same-track symbol in
that order. Measured: **Czardas N2d 2 155 → 2 164 (100 %, exact)**; `El_Choclo`
and `La Partida` also become exact.

### The `.ly` writer causes: what was fixed

The `.ly` losses were classified from the previous measurement (245 files with
a loss, 12 447 N2d notes):

| Cause | Files | N2d | Writer? | Action |
| --- | ---: | ---: | --- | --- |
| `\articulate` (`ly/articulate.ly`) | 32 | 10 688 | writer semantics | **fixed** |
| note-off at end vs end−1 | — | within ±1/16 | intentional | documented |
| unison (`fixupMIDI`) | 39 | documented | convention | excluded |
| `\repeat`, `\transpose`, octave/reader, bad refs | 213 | 7 103 | no | excluded |
| trill/prall/grace/arpeggio | 11 | 380 | writer | **fixed** |

**Cause 25 — `\articulate`.** `ly/articulate.ly` replaces each note by a
shorter note plus a rest: `normalFactor` 7/8 by default, 1/2 for staccato
(3/4 under a slur), 1/4 for staccatissimo, 3/4 for portato, 1 for tenuto, 1
under a slur without articulation; `note-performer.cc` then plays those
lengths. The reader now records `playback_articulate`, and `render.rs` applies
the factor to normal notes (the MuseScore `gateTime` is bypassed when the flag
is set). Reduced test `lilypond_articulate` (`c'4 d'2 e'4` → 419, 839, 419
ticks; LilyPond 2.27). Measured: `GaudeteChristus` 967 → 1 049, `Angels_Carol`
1 458 → 1 545, `LaudamusTe` 372 → 428, `Anerio` 486 → 541, 20 files improved.

**Cause 26 — arpeggios and ornaments are not performed by LilyPond.**
LilyPond has no arpeggio performer (`arpeggio-engraver.cc` only engraves), and
`note-performer.cc` expands no ornament: only `\articulate` does. The
LilyPond reader therefore sets `Arpeggio.play = false` and, without
`\articulate`, `Ornament.play = false`; the MIDI writer respects both.
Reduced tests `arpege_lilypond_ne_decale_pas_les_notes`,
`trille_sans_articulate_n_est_pas_developpe`,
`trille_avec_articulate_est_developpe`. Measured: `Sancta_Maria` and
`Sancta_Maria_KV_273` gain 9 and 4 durations, `Angels_Carol` +2.

The one `.ly` loss is `Wellerman` (176 → 166): `\articulate`'s group/portato
factor for notes under a slur is not modelled yet (10 durations).

The note-off `end − 1` is a MuseScore convention (`exportmidi.cpp`); LilyPond
writes `on + length` in `audio-item.cc`/`note-performer.cc`. The difference is
one tick, inside the oracle's 1/16 tolerance, and is not a measured loss; the
writer keeps the MuseScore convention because `ecriture_mscz` is its primary
target.

### The `.ly` reader causes fixed in this pass

**Cause 27 — `\transpose` and `\chordmode`.** A chord name is realised to MIDI
pitches at parse time (`NoteSpec::midi`); `note_event` returned them unchanged,
so `\transpose c g,` did not move the chord grids. `MarieJeanneGabrielle`
played its C-major grids instead of G-major (233 notes, octave class).
`NoteSpec::transposable` is true for `\chordmode` (false for `\drummode`,
whose table is not transposed); `note_event` adds the chromatic interval.
Reduced test in `scoretk-lilypond-import` (`\transpose c g,` → 55/59/62).
Measured: `MarieJeanne` N2d 139 → 246 at this stage, 362 after the repeat fix.

**Cause 28 — `\repeat volta` and `\unfoldRepeats`.** `\unfoldRepeats` was
transparent: the model kept repeat marks and the MIDI writer expanded them with
MuseScore's `<endings>` rule `i, i+k, i+2k`, whereas LilyPond expands only
under `\unfoldRepeats` and distributes the k blocks **contiguously**
(`ly/music-functions-init.ly`, verified on N = 3..5, k = 2..3). The reader now
unfolds the passes itself, each body restarting from the `\relative` reference
at the repeat start; without `\unfoldRepeats` the model keeps the notation for
the round trip. The MIDI writer always obeys the caller's
`expand_repeats`; the LilyPond oracle passes `--no-expand-repeats` to the
sources that have no `\unfoldRepeats` in their score music, so their reference
is played as written. `Score::playback_unfold_repeats` records the source flag
for the reader and the oracle, not for the writer. Measured: `Tollite`
512 → 591, `Ackermans` 2 002 → 3 081, `Makil` 143 → 732, `gaudete` 108 → 432,
`UztaiDantza` 108 → 379, `LaMouetteGrise` 40 → 249.

**Cause 29 — `\relative` reference across `\alternative`.** The alternative
blocks inherited the `\relative` reference of the previous block instead of
the end of the body (the written order); the octave could drift from one
alternative to the next. The reference is saved after the body and restored
before every alternative. This is what turns `Ackermans` from the octave class
into an exact file after the repeat fix.

The two losses are the `\partcombine` limitation, reduced to a minimal
`\partcombine \theme \acc` used twice under `\transpose` and
`\unfoldRepeats`: LilyPond's `lily/part-combine-iterator.cc` and
`part-combine-engraver.cc` write a **single** combined voice (solo / a2 /
unison decisions), while the reader keeps both voices and plays them
simultaneously (`notes_supp`, `octave`). `Maiana` and `Encantada` use
`\partcombine`; the previous model's over-expansion happened to match part of
their second voice, hence the apparent regression. The limitation is already
documented in `lilypond.md` § "Reading limits" (`partCombine`); it is not the
repeat or transpose fix.

### Remaining classes

`ecriture_mscz`:

| File | Lost N2d | Reason |
| --- | ---: | --- |
| `Adios Muchachos` | 7 | duplicate pitch 60 at the same tick: the oracle's FIFO pairing cannot separate written note and harmony bass |
| `Buenos dias, Mexico` | 3 | octave class / missing notes |

All other `.mscz` files are exact.

`ecriture_ly` — N2d loss 8 344; the top rows are now `Veni Creator` 3 119
(bad reference: 3 134 notes against 249 for the sibling `.mscz`),
`Laudate_pueri` 1 058 (bad reference), `Ave_Verum` 473 (attack shifts, reader),
`LaMouetteGrise` 283 (separate playback score), `Maiana` 124 and `Encantada`
85 (the two losses above), `franck_ave_maria` 123 (attack shifts), `Vincent`
109 (unison convention), `01_06_ChanteOMaLangue` 88 (octave/reader), `Promo`
73 (tuplets/repeat). `Ackermans`, `MakilTxikiak`, `gaudete` and `UztaiDantza`
are no longer in the top rows: their remaining differences are under 10 notes
or gone.

No `.mscz` file regressed: 0 loss, and the 21 corpus files stay at 21/21
(`corpus_midi`), including the four with empty references.

## Final `.mscz` classification (2026-11-21)

The four non-exact files of the 173-file MuseScore lot (reference MIDIs
regenerated with MuseScore 4.7.4, fresh candidate run in `target/midi-turn`)
were classified against `ref-musescore/src/engraving/compat/midi/`
(`compatmidirender.cpp`, `compatmidirenderinternal.{cpp,h}`) at the ported
5.0.0 commit, with 4.7.4 output prevailing:

| File | Cause | Verdict |
| --- | --- | --- |
| `Buenos_dias,_Mexico_!.mscz` | `ornamentTurn` on a 16th was played as a single F4; MuseScore renders the excursion `{+1,0,-1,0}` in 32nds (`compatmidirenderinternal.h:88`, `compatmidirender.cpp:665-800`) | **real writer bug — fixed** |
| `Adios_Muchachos.mscz` | 8 pitch-60 notes differ only in duration (e.g. 1.75 vs 2.0, short vs 0.5): a written note and the harmony bass share a tick, so the oracle's FIFO note-off pairing cannot separate them; the pitch/onset multisets match | **oracle artefact**, explained (no per-file loss) |
| `Chant_de_la_Promotion.mscz` | empty score: 0 notes in both the 4.7.4 reference and our output | **oracle artefact** (`exact` requires a non-empty reference) |
| `Chant_de_la_promotion_Saint_Michel.mscz` | same empty score | **oracle artefact** |

Fix: `ornaments::turn` and its dispatch in `render.rs`, plus a downward
diatonic auxiliary helper (`step_pitch_below`); reduced test in
`tests/turn.rs`, failing before the fix. Re-measure: **170/173 exact**
(169 before), 1 gain, **0 losses**, N1/N2 100 %. The `.ly` lot figures are
not part of this pass: they mostly measure the LilyPond reader, owned
elsewhere.

## Oracle pairing and the xml/corpus lots (2026-11-22)

### Order-independent note pairing

`midi_notes.py::read_midi` paired note-ons and note-offs with a FIFO per
(channel, pitch) in raw message order. MuseScore 4.7.4 writes the note-off
*before* the note-on of a zero-length note (e.g. `Adios_Muchachos.mscz`,
tick 32, pitch 60), so the FIFO dropped that off and assigned the following
note-offs to the wrong onsets: 8 N2d differences on a score whose pitch and
onset multisets were otherwise identical. The oracle — not the export — was
wrong. `read_midi` now groups events by tick and processes all note-offs
before all note-ons, making the pairing depend only on the tick and pitch.
Reduced test `scripts/oracle/midi/test_same_tick.py` writes both orderings
and fails before the fix. Re-measure of all lots (`target/midi-full`):
**0 exact losses, 55 exact gains** (`.ly` 662 → 717, N1 96.34 → 97.10,
N2 93.38 → 95.26); the mscz and corpus lots are unchanged. `Adios_Muchachos`
goes from 8 N2d differences to 1 (a genuine duration difference at tick 36).

### MusicXML and corpus export lots

A fresh `ecriture_xml` lot was built from the versioned corpus
`*.ref.musicxml` files (plus one real `SanctePieX.xml`), each with a MuseScore
4.7.4 MIDI exported into `target/midi-xml/refs/` by `mscore`. The harness
initially mislabelled `.musicxml` sources as LilyPond (`--from ly`), producing
one note per file; `mesure.py` now routes `.musicxml/.mxl/.xml` through the
MusicXML reader. Re-measure (`target/midi-xml-run2`): **10/13 exact**,
N1/N2 100 %. The corpus `.mscx` lot stays **17/21 exact**, N1/N2 100 %.

Non-exact files, classified:

| Lot | File | Cause |
| --- | --- | --- |
| xml | `keysig_78216.ref.musicxml` | empty score (0 notes in reference and output) |
| xml | `layout_elements.ref.musicxml` | 9 extra notes (chords at ticks 30/31.5) — MuseScore skips non-played grace/ornament events that our writer emits |
| xml | `moonlight.ref.musicxml` | 0.25-quarter gate-time differences on a handful of notes (N2d 99.74 %) |
| corpus | `mmrest.mscx`, `repeat68/69.mscx`, `testTempoPrecision.mscx` | empty or silent scores (0 notes both sides) |

The `.ly` figures measure the LilyPond reader, owned by another agent, and
are not part of this export work. The plan (`docs/agents/plan-fiabilite.md`)
names music21, LilyPond and `abc2midi` as score→MIDI alternatives; none of
them is available in this worktree for a same-oracle comparison, so the
tables above compare scoretk against the MuseScore 4.7.4 oracle only.

## MIDI export: enlarged `.mscz` lot and classification (2026-11-23)

### Measurement and oracle state

The small `ecriture_mscz_musique` lot is 8 files (3/8 exact) and the versioned
corpus lot 17/21 (17 exact, 4 empty references). The 8-file lot is too small to
rank causes, so the existing MuseScore 4.7.4 reference set at
`/tmp/midi-mscz-refs/manifest.tsv` (174 `.mscz`, one MIDI per source, generated
by `mscz-refs.sh` with `PARALLEL=2`) was measured with the same oracle:

```
python3 scripts/oracle/midi/mesure.py --out target/midi-mscz-big \
  --scoretk target/midi-base --ecriture mscz174=/tmp/midi-mscz-refs/manifest.tsv
```

The reference *is* MuseScore 4.7.4's MIDI export, so this is the
scoretk-versus-alternative comparison required by `plan-fiabilite.md` §0.
`target/midi-mscz-big/resume.md`: **171 measured files** (3 unsupported,
2 empty scores excluded), **143 exact (83.6 %)**, N1/N2 **99.99 %**, mean
output 18 535 B, max 1.63 s.

### First-divergence classification

`export_groupes.py --jobs 2` on the same outputs and references:

| First divergence | Files | Example |
| --- | ---: | --- |
| duration | 19 | `00- Veni veni Emmanuel - chant piano` (tick 43.5, 7 notes) |
| extra note | 7 | `Adios Muchachos` (1), `Czardas Monti` (5, tick 8) |
| onset | 1 | `Vivaldi 04-TecumPrincipium` (tick 137.125) |
| octave | 1 | `Sancta Maria Mater Dei Mozart` (tick 20.25) |

Nine of the 19 duration files are the same `Veni veni Emmanuel` score exported
for different voices/excerpts (7 notes each at the same tick); eight more are
the same `Once in royal David's city` case (10 notes at tick 240). They are one
cause, not 17.

### Largest cause: restrike/unison pairing, not a writer ordering bug

Raw SMF of `Veni veni Emmanuel - chant piano` at tick 43.5 (track 12):
MuseScore writes `on(80), vel-0 restrike, on(80), vel-0 restrike on channel 1`;
scoretk writes `on(80), on(80), vel-0 restrike`. The pitched notes and onsets
are identical (N1/N2 100 %); only the note-off pairing differs, because the
oracle groups by channel and processes note-offs before note-ons at a tick.

The C++ ordering was tested faithfully: `fixupMIDI`
(`engraving/compat/midi/event.cpp:263-306`) marks the restrike and
`exportmidi.cpp:304-323` writes it via the same per-tick multimap; moving
`NoteOffRestrike` from priority 3 (after attacks) to priority 1 (before them)
in `writer.rs::priority` produced the exact byte order above. Measured on the
**same enlarged lot**: **143 → 53 exact, 90 losses** (`target/midi-restrike-run/`).
The change was reverted before any commit. The current priority-3 order is
therefore the one that matches MuseScore's overall output; the residual
duration differences are an oracle pairing convention for simultaneous
restrikes, not a faithful-port defect. No writer cause was committed in this
pass: the one experiment that could have fixed the largest group was measured
and rejected with evidence, and the remaining groups are source-specific
(harmony durations in `Czardas`, one octave and one onset) documented above.

## Equal-tick pairing and the eight residual export files (2026-11-23)

### Equal-tick note pairing — FIFO analysis, no change kept

The 17 `Veni`/`Once in royal` files differ on same-pitch notes at the same
tick. `read_midi` merges SMF tracks by channel and pairs note-ons/note-offs
per `(channel, pitch)` with FIFO. A strict "all note-offs before note-ons at
an equal tick" rule was implemented and rejected:

1. It breaks `test_extinction_sur_une_autre_piste`: when a unison is attacked
   at the tick, the restrike off has no older open note to close, so applying
   it first leaves the first note open for two quarters instead of zero.
   MuseScore's own exporter writes that restrike *after* the attack
   (`exportmidi.cpp:304-323`, `fixupMIDI` `event.cpp:263-306`).
2. A refined variant (prioritise only the offs that close a note opened
   strictly before the tick, snapshot per pitch) keeps the 12 oracle tests
   green, but a discriminating two-SMF selftest shows it cannot make pairing
   order-independent: FIFO is already order-independent whenever the number
   of offs does not exceed the number of older open notes; when it does, some
   off necessarily closes a note attacked at that tick, and which one depends
   on the order. No rule changes that without changing musical semantics.

Both variants were reverted before any commit; `scripts/oracle/midi/` is
unchanged and its 12 selftests pass. Reversing the writer's restrike priority
was also measured earlier (143 → 53 exact, 90 losses, § previous), so the
residual is a reference/export convention, not a faithful-port defect.

### First divergence of the eight residual files

| File | Class | First divergence | Verdict |
| --- | --- | --- | --- |
| `Adios_Muchachos` | extra note | tick 36, 574/574 notes | same pitch/onset/duration multisets after normalisation; zero-length restrike pairing |
| `Czardas_Monti` | extra note | tick 8, 2 036/2 041 | harmony/treble-clef overlap at a chord attack (`harmony::ticksTillNext`, `dom/harmony.cpp:1011-1085`) |
| `Stella_maris - La` | extra note | tick 44.875, 849/849 | same-onset unison of two staves, restrike paired differently |
| `Stella_maris - SiB` | extra note | tick 44.875, 795/795 | same |
| `Sancta_Maria_KV_273 … orgue` | extra note | tick 10, 2 442/2 447 | grace/ornament realisation at an organ chord |
| `JMHaydn_Viderunt_omnes` | extra note | tick 41.5, 3 540/3 541 | one note in a tied chord; reference drops the duplicate voice |
| `Ave_Maria_Schubert_in_C_with_chords` | extra note | tick 29.667, 1 567/1 567 | simultaneous same-pitch attacks across voices |
| `Sancta_Maria_Mater_Dei_Mozart` | "octave" | tick 20.25, 2 413/2 420 | actually an onset shift: reference 20.2479 vs output 19.752 for the same pitches (grace placement), plus a note paired at tick 141.75 |

All eight are same-pitch/same-onset pairing or grace-placement cases in the
writer-to-or comparison. None yields a writer-only rule that changes a file to
exact without changing correctly written notes; the one such rule tested
(restrike priority) lost 90 files. No writer change was committed.

## Event-level oracle level (2026-11-23)

### Implementation and controlled measurement

`midi_notes.evenements_par_tick()` now compares, per channel, the multiset of
`(tick, "on"/"off", pitch)` events without any note pairing; a note-on of
velocity zero counts as an off (`format-ref/midi.md` line 63). `compare_evenements`
returns the first divergent tick, and `mesure.py` sets `exact_events` and counts
a file as exact if `exact or exact_events`. Selftests (`test_events.py`) verify
that two SMFs differing only by the order of an off and an on at the same tick
are event-equal (while their FIFO note pairing is *not*), and that a real
one-tick change fails.

Controlled measurement, **same binary** (`target/midi-base`), only the oracle
changed (`target/midi-events2`):

| Lot | Exact before | Exact after | Gains | Losses |
| --- | ---: | ---: | ---: | ---: |
| corpus (`.mscx`) | 17/17 | **17/17** | 0 | 0 |
| `.mscz` (171 files) | 143/171 | **143/171** | 0 | 0 |

### Result: the 17 files are not event-equal

`exact_events` is true for **0** files; the 17 `Veni`/`Once in royal` files
remain non-exact. Two examples, first divergent tick **0** in both:

- `00- Veni veni Emmanuel - chant piano.mscz` (index 59): the reference has
  `((1, "off", 64), 1)` at tick 0 in addition to the channel-0 events
  (`off` ×1, `on` ×2); our output has only the channel-0 events.
- `00- Once in royal David_s city - chant piano.mscz` (index 93): the
  reference has `((1, "off", 62), 1)` at tick 0; our output lacks it.

These are the note-offs hoisted by MuseScore's `fixupMIDI`
(`engraving/compat/midi/event.cpp:263-306`) and written on the previous
staff's channel by `exportmidi.cpp:304-323`. The event-level comparison thus
confirms a **real writer difference** (an off on the previous note's channel
at the restrike tick), not merely an order artefact. No writer change was
attempted in this pass: the earlier restrike-priority experiment on the same
lot lost 90 files, so the channel-hoisting rule needs its own measured port
rather than a priority tweak. The oracle change is committed with no per-file
loss; all 14 `scripts/oracle/midi/` selftests pass.

## Audible-event oracle: the 17 restrike files resolved (2026-11-23)

Two defects in the previous event level were corrected and committed:
`test_events.py` had a vacuous `assert … or True` (replaced by a real
`evenements_audibles` inequality), and raw event multisets counted orphan
note-offs. `evenements_audibles()` now replays each SMF with a per-
`(channel, pitch)` counter of open notes: an on opens, an off only closes if a
note is open, otherwise it is counted in `offs_orphelins` and discarded. The
comparison therefore sees only what a synthesiser hears. This is the musical
meaning of `fixupMIDI` (`engraving/compat/midi/event.cpp:263-306`) hoisting a
note-off to the previous staff's channel (`exportmidi.cpp:304-323`): when a
restrike already ended the note, the hoisted off closes nothing.

**Controlled measurement** (same binary `midi-base`, only the oracle changed,
`target/midi-audible`):

| Lot | Exact before | Exact after | Gains | Losses |
| --- | ---: | ---: | ---: | ---: |
| corpus (`.mscx`) | 17/17 | **17/17** | 0 | 0 |
| `.mscz` (171 files) | 143/171 | **160/171 (93.6 %)** | **17** | **0** |

All **17** `Veni`/`Once in royal` files are now `exact_events` and exact: the
channel-1 offs that the raw level flagged at tick 0 are orphans, not audible
events. The earlier "real writer difference" conclusion is superseded. `N1/N2`
stay at 99.99 %, mean output size unchanged (18 535 B).

The 11 remaining non-exact files have first *audible* divergence ticks:
`Sancta_Maria_Mater_Dei_Mozart` and `JMHaydn_Viderunt_omnes` at 0;
`Sancta_Maria_KV_273 … orgue` at 4 681; `Vivaldi 04-TecumPrincipium` at
4 854; `Missa … Gloria` at 7 080; `Ave_Maria_Schubert` at 7 140;
`Adios_Muchachos` at 16 200 (36 orphan offs on the reference side);
`Czardas_Monti` at 3 720 (16 orphans); `Stella_maris - La/-SiB` at 21 060.
They are genuine note/onset/duration differences, not pairing artefacts.
All 16 `scripts/oracle/midi/` selftests pass.

## Corrected audible replay: merged tracks (2026-11-23)

`evenements_audibles()` was replaying each SMF track one after another. That
is wrong for hoisted note-offs (`exportmidi.cpp:304-323`): whether an off on
track 2 closes a note opened on track 1 depends on the arbitrary track order.
It now merges **all** events by absolute tick (equivalent to
`mido.merge_tracks`), then applies a fixed rule inside a tick: **offs before
ons**, because note-on velocity zero is a note-off (`format-ref/midi.md`
line 63) and a restrike ends the previous note before opening the new one
(`fixupMIDI`, `engraving/compat/midi/event.cpp:263-306`). Two new selftests
cover a cross-track off whose orphan status changed with track order (now
stable) and a cross-track off that really cuts a sounding note (still a
difference); 18 oracle selftests pass.

**Controlled measurement** (same binary `midi-base`, only the oracle changed,
`target/midi-merged`):

| Lot | Exact before | Exact after | Gains | Losses |
| --- | ---: | ---: | ---: | ---: |
| corpus (`.mscx`) | 17/17 | **17/17** | 0 | 0 |
| `.mscz` (171 files) | 143/171 | **143/171 (83.6 %)** | **0** | **0** |

**The 17 files do NOT stay event-equal under correct replay.** The previous
160/171 result was an artefact of sequential replay misclassifying hoisted
offs as orphans. With merged replay the first *audible* divergence is:

- `00- Veni veni Emmanuel - chant piano` — tick **22 080**: the reference has
  `(channel 1, off, 67)` in addition to the channel-0 events; our output lacks
  it (512 orphan offs on the reference side, 19 on ours overall).
- `00- Once in royal David_s city - chant piano` — tick **117 840**, same
  pattern on pitch 67.

At tick 0 both sides now agree, so the earlier "real writer difference at
tick 0" was itself an artefact. The remaining difference is a hoisted
note-off on the previous staff's channel that our writer never emits. A
`fixup.rs` attempt to carry the previous channel into `NoteOffRestrike` was
reverted before commit: `playing` is already keyed by `(part, channel, key)`,
so the "previous" channel is necessarily the current one and the edit was a
no-op. This needs a separate, measured port of the cross-channel hoist.

## Cross-channel restrike port: two measured attempts (2026-11-23)

`Veni veni Emmanuel`, tick 22 080: the reference writes, on track 12,
`on(ch0,67,vel0)`, `on(ch0,67,vel80)` ×2 and `off(ch1,67)`; our output lacks
the `off(ch1,67)`. The ch1 note was opened at tick 21 840 on **track 7**, i.e.
in another part. The reference has two more ch1 offs at 22 560 and 22 799.

Port tried (commit `587e573a`, new `cross_channel.rs` wired into `fixup.rs`):
on a restrike, emit a velocity-0 off on every channel of the **same part**
still sounding that pitch, following `fixupMIDI` (`event.cpp:263-306`) and
`exportmidi.cpp:304-323`. Reduced unit test from this case added. Controlled
measurement (same oracle, writer only):
**143/171 → 143/171, 0 gains, 0 losses**; corpus 17/17 unchanged. The target
off was not emitted because the ch1 note belongs to another part.

Diagnosis dump of `playing` at the restrike shows entries
`(part 8, ch2, 67)`, `(part 9, ch5, 67)`, `(part 12, ch0, 67)` — no
`(part 12, ch1, 67)`. Broadening the rule to **all parts** (as
`EventsHolder::info` is indexed by channel alone) emitted many spurious offs:
**143/171 → 88/171, 55 losses**, output size 18 535 → 18 995 B. That variant
was reverted before commit.

So the missing off is not a same-part restrike: it is a hoisted note-off from
a note of another part whose own attack/write path differs in our renderer.
A correct port needs the per-part `EventsHolder` write order of
`exportmidi.cpp:296-323`, not a channel scan over `playing`. The committed
same-part port is lossless and keeps its reduced test; the remaining
divergence is unchanged at tick 22 080.

## ExportMidi/FixupMIDI data flow and the precise divergence (2026-11-23)

### Exact C++ flow (line by line)

`ExportMidi::write` (`exportmidi.cpp:216-235`) creates **one**
`EventsHolder events` for the whole score (line 226) and fills it once with
`CompatMidiRender::renderScore` (line 235). `EventsHolder::_channels` is a
`std::vector<events_multimap_t>` indexed by **internal MIDI channel**
(`event.h:183-185`) — not per part and not per staff. Each multimap maps a tick
to `NPlayEvent`s; every event carries `channel()`, `pitch()`, `velo()`,
`discard()` and `originatingStaff()`.

`EventsHolder::fixupMIDI` (`event.cpp:263-306`) keeps `info[channel]` — a
per-channel table of `nowPlaying[pitch]` and the last note-on event
(lines 266-270). For each note-on event in channel order:
- `velo() == 0` (a restrike off): if `np == 0 || --np > 0` it is discarded
  (lines 275-278); otherwise it is **hoisted** — `setOriginatingStaff()` of
  the earlier note-on (line 280).
- `velo() > 0`: `++np > 1` means a restrike; `setDiscard(originatingStaff+1)`
  on the new attack (lines 282-286). The last note-on per (channel, pitch) is
  remembered at line 287.

Writing (`exportmidi.cpp:239-335`): one SMF track per **staff** (line 222),
`staffIdx` walking them (239-241). For each staff track, lines 297-299 iterate
**all channel multimaps**; line 304 writes a velocity-0 note-on (the restrike
off) on the *current staff's track* whenever `discard() == staffIdx + 1` and
`velo() > 0`, using the `channel` variable of the instrument channel currently
being written (lines 248-255). Line 317 then skips events not originating on
this staff, so the real note-on is written only on its own track; lines 331-334
filter by output port/channel. Thus two restrikes of the same pitch on two
different internal channels both write an off on the previous staff's track,
each with its own channel.

### Our port and the precise divergence

`fixup.rs` mirrors this with a single `Performance` (one event list per staff)
but keys `playing` by **`(part, channel, key)`** (`fixup.rs:84-86`) rather than
by channel alone, and builds `(tick, track, seq)` collection order rather than
`events_multimap` channel order. On commit `587e573a` a same-part channel scan
was added; it gained 0 files and was reverted (see the following section).

At tick 22 080 in `Veni veni Emmanuel`, the reference writes on track 12
`(ch0, off, 67)`, `(ch0, on, 67)` ×2 and `(ch1, off, 67)`. The ch1 note was
opened at tick 21 840 on track 7. In our rendering there is **no ch1 note-on
event at tick 22 080 at all** — only ch0 — because the second internal channel
of that part is not rendered by our renderer at this point. Consequently
`fixup_midi` has no second restrike to hoist. The missing off is therefore
upstream of `fixup.rs`: it comes from the channel-level event production of
`renderScore`, not from the channel scan at restrike time.

A faithful port needs to render the part's secondary (harmony/accompaniment)
channel into the same `EventsHolder` before `fixupMIDI`, as `renderScore` does,
then let the existing per-channel `discard == staffIdx+1` rule hoist both offs.
Our single `Performance`/`playing` structure has no equivalent of the global
channel-indexed multimap; a correct change touches `render.rs` channel setup
as well as `fixup.rs`. No writer change was committed in this pass: the two
channel-scan variants were each measured (0 gains/0 losses and 55 losses),
and neither reproduces the reference event. Counts remain 143/171 and 17/17.

## Per-channel restrike port measured (2026-11-23)

Commit `aff1e32e` ports the exact line-297/308 rule: `fixup_midi` now receives
each part's output channels (notes + harmony, `ChannelMap` in `render.rs`) and
writes one velocity-0 restrike off per channel of the **previous staff's**
part. Reduced unit test from the `(ch1, off, 67)` case at tick 22 080.

Controlled measurement (same oracle, writer only):

| Lot | Exact before | Exact after | Gains | Losses | Mean size |
| --- | ---: | ---: | ---: | ---: | ---: |
| corpus | 17/17 | **17/17** | 0 | 0 | 281 B |
| `.mscz` 171 | 143/171 | **144/171 (84.2 %)** | **1** (`Adios_Muchachos`) | **0** | 18 535 → 18 538 B |

The `Veni` divergence at tick 22 080 is unchanged: its part's secondary
channel is not a `harmony` channel in the MusicXML sense, so `ChannelMap` has
no second output channel to write the extra off on. The reference's ch1 off
comes from an instrument channel (e.g. `open`/`mute`) that our model retains
only as `channel_count` (`part.rs:81`), not as a map of MIDI channel numbers.
A further port would need that channel table in the model. The committed port
is lossless and follows `exportmidi.cpp` exactly for the channels we know.

## Instrument channel table: two rejected paths (2026-11-23)

Following `InstrChannel` (`instrument.h:79-124`) and `TRead::read(InstrChannel*)`
(`read400/tread.cpp:926-960`), two ways of giving `ChannelMap` the part's
secondary channels were tried and rejected:

1. **Full channel table in the model** (`crates/scoretk-core/src/model/instrument_channel.rs`,
   `Instrument::channels`, filled by the 3.x/4.x readers with each `<Channel>`
   name, `<midiChannel>` and `<program>`). It broke the full-struct round-trip
   `mscz_est_un_conteneur_musescore` (`crates/scoretk/tests/mscx.rs:65-79`):
   the MuseScore writer reconstructs names as `normal`/`channel-1`/`channel-2`,
   so re-reading a written `.mscz` did not reproduce the stored names. Fixing
   that would change `.mscx`/`.mscz` output and its snapshots. Reverted before
   commit.
2. **Derive secondary channels from the existing `channel_count`**
   (`render.rs::channels`): allocate one free MIDI channel per extra
   `<Channel>` and feed them to `fixup_midi`. Controlled measurement (same
   oracle, writer only): **144/171 → 144/171, 0 gains, 0 losses**, corpus
   17/17 unchanged, but mean output **18 538 → 18 644 B** (+106 B/file).
   The `Veni` divergence stayed at tick 22 080: generic free channels do not
   reproduce the reference's ch1. Reverted before commit.

The committed, measured state stays: per-channel restrike for the channels we
know (notes + harmony), **144/171** on the enlarged lot and **17/17** on the
corpus. Reproducing the remaining off requires the instrument's *explicit*
`<midiChannel>` values plus a writer that preserves them across round trips —
a model change with its own snapshot update, not a renderer-only tweak.

## Value graces are played on the beat (2026-11-23)

Re-reading `createGraceNotesPlayEvents` (`compatmidirender.cpp:530-606`)
together with `renderChord` (`:193-252`, `NoteEvent(0, ontime, 1000 -
trailtime)`) and `collectNote` (`compatmidirenderinternal.cpp:861-862`)
settles the placement of non-acciaccatura grace-before chords: **all** of
them, including `GRACE4`/`GRACE16`/`GRACE32`, get
`ontime = min(500, actualTicks(grace) / actualTicks(chord) * 1000)`; the
graces are emitted with `ontime` `0, graceDuration, …` at the chord's tick,
the main note is delayed by `ontime`, and `trailtime += ontime` shortens it so
it still ends at the same tick. The C++ never checks `NoteType::APPOGGIATURA`
here — only `ACCIACCATURA` (and grace bends) are exempt (`:576-588`).

Two fresh MuseScore 4.7.4 references (`mscore … -o …`, PPQ 480) confirmed it:
one `grace16` before a quarter gives grace 480–599 and main 600–959; two
`grace32` give graces 480–508 / 509–537 and main 540–959. The writer used to
place them at 360–479 and 420/449 with the main note at 480. Reduced tests:
`crates/scoretk-midi-export/tests/grace_on_beat.rs` (the old
`grace16_before_beat` expectation was wrong and was corrected).

The fix removes the `Grace4 | Grace16 | Grace32` branch of `render.rs` so all
non-acciaccatura before-graces share the faithful `ontime`/`trailtime` path.
Controlled measurement, same oracle and refs, writer only:

| Lot | Exact before | Exact after | Gains | Losses | Mean size |
| --- | ---: | ---: | ---: | ---: | ---: |
| corpus | 17/17 | **17/17** | 0 | 0 | 281 B |
| `.mscz` 171 | 144/171 | **154/171 (90.1 %)** | **10** | **0** | 18 220.7 → 18 220.2 B |

Gains: `Czardas_Monti`, `Stella_maris - La`, `Stella_maris - SiB`,
`Sancta_Maria_KV_273 … orgue`, `Sancta_Maria_Mater_Dei`,
`JMHaydn_Viderunt_omnes`, `Missa_in_Es … Gloria`, `Missa_in_Es … Kyrie`,
`Ave_Maria_Schubert_in_C_with_chords`, `Vivaldi … Dixit_Dominus`. N1 and N2
both go from 99.99 % to **100.00 %**; no file grows by more than 100 B.

The 17 remaining non-exact files are all `Veni veni Emmanuel` / `Once in
royal David's city` instrument-channel cases (tick 22 080 / 117 840), left
aside as decided.

## Instrument channel table: the completed port (2026-11-23)

The two previously rejected paths (see above) are now superseded: the model
really does keep the full channel table, **and** the `.mscx` writer writes it
back faithfully, so the full-struct round trip stays green.

- `crates/scoretk-core/src/model/instrument_channel.rs` — new
  `InstrumentChannel { name, program, bank, midi_port, midi_channel, mute,
  solo }`, ported from `InstrChannel` (`instrument.h:79-139`) and
  `TRead::read(InstrChannel*)` (`read400/tread.cpp:926-1020`,
  `read500/tread.cpp:1144-1238`). `Instrument::channels` is filled by both
  readers and by MusicXML `<midi-instrument>` / `<midi-device port>`
  (`exportmusicxml.cpp:7939-7947`).
- `write_instrument` (`write/twrite.rs:~990`) now writes every stored channel
  with its name, program, HBANK/LBANK controllers, port and channel, instead
  of regenerating `channel-{index}`. `TWrite::write(InstrChannel*)`
  (`write/twrite.cpp:2067-2122`) is the reference. The legacy
  `Instrument.midi_*` fields still override channel 0 so callers that mutate
  them (test `port_midi.rs`) keep working, and a 3.x program inferred from the
  instrument id is preserved.
- `ChannelMap::secondary` (`render.rs:423-480`) feeds `fixup_midi` every
  explicit channel of the part, per `exportmidi.cpp:248-255,304-308` and
  `MasterScore::updateMidiMapping` (`midimapping.cpp:259-340`). Channels
  without an explicit `<midiChannel>` are ignored rather than invented.

Reduced test: `restrike_sur_tous_les_canaux_de_la_partie`
(`tests/fixup_midi.rs`), from the `Veni` tick 22 080 case (two trumpet
channels, restrike off on both ch0 and ch1), checked against MuseScore 4.7.4.

Controlled measurement, same oracle and refs, writer only:

| Lot | Exact before | Exact after | Gains | Losses | Mean size |
| --- | ---: | ---: | ---: | ---: | ---: |
| corpus | 17/17 | **17/17** | 0 | 0 | 281 B |
| `.mscz` 171 | 154/171 | **171/171 (100 %)** | **17** | **0** | 18 220 → 18 586 B |

All 17 remaining files (`Veni` ×9, `Once in royal` ×8) became exact; N1/N2
stay at 100.00 %. 44 files grow by more than 100 B because MuseScore itself
writes the restrike off on every channel — our output is still smaller than
the reference for every file in the lot (mean −10 966 B, the historical gap
being meta events). The `.mscx`/`.mscz` round trips, MusicXML e2e corpus and
the full workspace test suite are unchanged and green.
