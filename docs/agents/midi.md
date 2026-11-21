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
scripts/gen-midi-references.sh            # toutes les références du sous-ensemble
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
scripts/gen-midi-import-references.sh            # toutes les références
scripts/gen-midi-import-references.sh tests/corpus/midi/v4/dynamics.ref.mid
nix develop -c cargo test -p scoretk-midi-import --test import_corpus -- --nocapture
nix develop -c cargo test -p scoretk-midi-export --test roundtrip_midi -- --nocapture
```

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
scripts/gen-midi-import-references.sh                # avec MuseScore 4.7.4
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

### Ports 2 et 3 : résultats (2026-11-20, suite)

- **Fermata — aucun port nécessaire.** `Fermata::timeStretch`
  (`engraving/dom/fermata.cpp:138-139, 250`, `Pid::TIME_STRETCH`) n'est pas
  consulté par le rendu MIDI de compatibilité. Une partition réduite avec un
  `fermataAbove` sur une noire, exportée par MuseScore 4.7.4, donne quatre
  notes d'une noire à 0/1/2/3 — exactement notre sortie. Le test
  `tests/fermata_no_stretch.rs` fixe ce comportement pour éviter un port
  erroné du `timeStretch`.
- **Pause au saut D.S. — expérience refusée.** Un `.mscx` 2.06 réduit avec un
  `LayoutBreak section` dans la mesure du saut reproduit l'écart : MuseScore
  insère une pause de 4 noires après **chaque** passage, notre `unfold` ne
  l'insère que devant la mesure suivante de la partition. Rendre la borne de
  segment inclusive (`tick <= segment.end`) fait passer ce test réduit mais
  **régresse `Vingt_danses` de 2373 attaques** (mesure complète à références
  égales) : la pause y tombe aussi sur des frontières de segments ordinaires.
  Le changement a été annulé. Une correction correcte doit rattacher la pause
  à la mesure qui porte le `LayoutBreak` et la rejouer aux passages du saut,
  dans `PauseMap`/`RepeatList`, sans toucher aux frontières ordinaires.

Le seul port de cette série est donc l'auxiliaire de trille selon l'armure
(commit précédent) : 19 fichiers `.mscz` gagnent 1319 attaques, 0 perte,
N2 94,57 → 94,92 %.

### Pauses : port de `collectPauses` par segment (2026-11-20)

L'heuristique `jump_back` du commit précédent n'était pas un port : elle ne
gagnait rien et perdait 19 notes sur `Vingt_danses`. Elle est remplacée par le
mécanisme réel de MuseScore.

`ScoreAutomationController::collectPauses`
(`src/engraving/automation/internal/scoreautomationcontroller.cpp:1100-1131`,
appelé à `:549` pour chaque mesure de chaque `RepeatSegment`) inscrit la pause
d'une rupture de section au `startUtick = measure.tick() + tickOffset` de la
mesure qui suit la rupture dans l'ordre brut (`measure->prev()`, `:1105-1111`),
avec `tickOffset = seg->utick - seg->tick` (`:542`). `PauseMap::calculate`
(`compat/midi/pausemap.cpp:47-76`) utilise ces uticks directement.

`pause_map.rs::pauses_by_segment` porte ce mécanisme : il parcourt
`repeat_list.segments`, puis les mesures de chaque segment, et inscrit
`sectionPause` (3 s, `Sid::sectionPause`, `styledef.cpp:643`) à l'utick de
chaque mesure marquée `section_break_before`. L'`unfold` des tempos reste
inchangé. Le test réduit `tests/section_pause_jump.rs` (2.06, rupture de
section, MIDI MuseScore 4.7.4 : notes aux ticks 0, 4 et 12) passe.

Mesure complète à références égales : **aucune perte**, `Vingt_danses`
récupère les 19 notes perdues par l'heuristique (N2 10 344 → 10 363),
N2 global 94,91 → **94,92 %**. Le décalage d'un quart au tick 1507 n'est
toujours pas corrigé : la pause concernée n'appartient à aucun passage où le
modèle porte `section_break_before`, ce qui reste à identifier dans le
lecteur 2.x (`read206`).

### Pause au saut de `Vingt_danses` : évènements de tempo (2026-11-20)

Les évènements `MetaMessage set_tempo` des deux MIDIs autour du tick 1507
lèvent l'ambiguïté : la référence 4.7.4 porte **20 BPM de 722 880 à 723 360**
(120 BPM ensuite), soit une pause d'un quart de noire exactement au passage
du saut `D.S. al Fine` ; notre MIDI n'avait aucun évènement de tempo là. La
cause n'était donc ni un tempo écrit ni une mesure manquante, mais la pause
de la rupture de section portée par la mesure du saut et jouée au début du
segment suivant dans l'ordre d'exécution (`collectPauses`,
`scoreautomationcontroller.cpp:1100-1131` ; `RepeatSegment::utick`). Le
nombre de noires inséré est celui de la mesure qui ouvre la nouvelle section
(anacrouse d'un quart dans `Vingt_danses`).

`pauses_by_segment` porte désormais ce cas : quand le segment précédent se
termine sur une mesure avec `<Jump>` et que la mesure suivante de la partition
porte la rupture, la pause est inscrite à l'utick du segment courant avec ce
nombre de noires (`pause_quarters` dans `TempoPoint`, utilisé par
`PauseMap::calculate` à la place de la longueur de la mesure au tick source).
Le garde `index > 0` de `collectPauses` (`measure->prev()` nul pour la
première mesure) est restauré, avec un test unitaire.

Mesure complète à références égales : **+358 notes sur `Vingt_danses`,
0 perte** ; N2 `.mscz` 94,92 → **95,01 %**. Les autres lots ne changent pas.

### Correction de `d39d7f5` : fraction des mesures de pause (2026-11-20)

`d39d7f5` convertissait `quarter_notes_at()` en `i64` avant de le réutiliser :
une mesure de 5/8 (2,5 noires) devenait 2 noires, aussi bien pour le tempo que
pour les ticks insérés. `pause_quarters` est désormais `Option<f64>` et la
valeur fractionnaire est conservée (`PauseMap::calculate`, `pausemap.cpp:68-71`
: `quarterNotesPerMeasure` est un `double`, seul `ticksPerMeasure` est un
`int`). Test unitaire `pause_en_cinq_huit_sans_troncature` : 1200 ticks et
2,5/3 bps. Le garde `index > 0` de `collectPauses` est également restauré
(`measure->prev()` nul pour la première mesure), testé par
`premiere_mesure_sans_pause_de_section`.

Le gain de +358 notes annoncé par `d39d7f5` était un artefact de cette
troncature sur les mesures irrégulières de `Vingt_danses`, pas l'effet de la
règle de saut (dont le motif `RepeatMark::Jump` n'a pas de contrepartie dans
`collectPauses`, qui ne raisonne que sur `measure->prev()` en ordre brut).
Avec la valeur correcte, cette règle n'apporte rien et a été retirée ; le
décalage au tick 1507 reste ouvert. La référence porte bien un évènement
`20 BPM` de 722 880 à 723 360, mais les essais d'ancrage au segment suivant
 ont tous régressé d'autres passages — le mécanisme exact reste à identifier.

### Bilan de la campagne des pauses (2026-11-20)

Comparaison fichier par fichier contre **master** (références et manifeste
identiques) : avec la valeur fractionnaire conservée pour le tempo et la
**troncature** de `ticksPerMeasure` (`pausemap.cpp:70`), **aucun fichier ne
gagne ni ne perd** ; les deux écarts
apparents (`Vivaldi … Judicabit`, `Vivaldi … Gloria Patri`) sont des dépassements
de la borne de 10 s de l'oracle — relancés sous 20 s, ils donnent exactement
les valeurs de master (2469 et 1677).

Le décalage d'un quart au tick 1507 reste ouvert, avec la preuve suivante :
le MIDI 4.7.4 porte un changement de tempo à 20 BPM de 722 880 à 723 360, soit
`quarterNotesPerMeasure / pause = 1 / 3 s` (`pausemap.cpp:71`) : la pause est
bien celle de la mesure anacrustique `len="1/4"` qui suit le saut, mais elle est
jouée à la frontière du saut. Dans notre modèle, cette mesure n'est atteinte
qu'après `fine4`, et toutes les tentatives pour l'ancre au saut (marqueur
`Jump`,mesure d'irregularité, bornes inclusives) ont régressé d'autres passages
de la même partition — le mécanisme exact qui place cette anacrouse dans la
`RepeatSegment` de 4.7.4 n'est pas dans le portage. Un gain net sur master
n'est donc pas démontrable sans ce mécanisme ; le correctif de troncature est
conservé pour sa conformité à `pausemap.cpp:68-71`, sans gain de mesure.

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

## Ancrage BeatRoot et mesure (2026-11-21, suite)

`BeatData` porte désormais `firstChordTick`, `lastChordTick` et les nombres de
temps ajoutés ; `set_time_sig` reproduit
`MidiBeat::setTimeSignature` + `updateFirstLastBeats`
(`importmidi_beat.cpp:472-511`) : la carte de mesure est remplacée par la
signature retenue, puis les temps ajoutés avec l'ancienne signature sont
retirés et recalculés. Le bug d'index de `Agent::fillBeats` (l'insertion
d'un temps interpolé faisait sauter l'événement suivant) est corrigé.

Le câblage complet a été mesuré dans `convertMidi`, exactement à l'endroit
où MuseScore l'appelle (`setIfHumanPerformance` puis `adjustChordsToBeats`
avant la seconde collecte) : **aucun gain, `human_tempo` perd 29 notes
(0,5000 → 0,2958)**, lot LilyPond inchangé. Diagnostic chiffré : le fichier
donne 71 attaques, BeatRoot produit 11 temps espacés de ~0,32 s, et
`addFirstBeats` étend la grille à 17 temps de 1,93 à 23,8 croches. La
première attaque (1,30 s) tombe alors à 2,5 croches après mise à l'échelle,
alors que la référence l'attend à 3,875 croches. L'hypothèse de tempo
devrait être proche de 0,234 s (480 ticks) et ne l'est pas : il reste un
écart entre notre induction et celle de MuseScore sur ce fichier, et le
branchement fidèle ne peut donc pas être fusionné sans régression. La glue
est conservée mais **inactive** ; c'est un résultat mesuré, pas un port
non testé.

## Lot LilyPond : la référence `.ly` n'est pas MuseScore

MuseScore 4.7.4 est disponible et fonctionne avec
`env -u LD_LIBRARY_PATH QT_QPA_PLATFORM=offscreen mscore`. Première
vérification « like with like » sur le pire fichier du lot
(`Laudate_pueri…`) :

- import MuseScore 4.7.4 du même `.midi` → 1 826 notes ;
- notre import → 1 836 notes ; score **0,8587** contre MuseScore ;
- la référence `.ly` (notre lecteur LilyPond) → 1 796 notes ; score de
  MuseScore contre elle : **0,4014**.

Donc les 1 063 notes « perdues » de ce fichier contre le `.ly` sont
majoritairement des choix d'import de MuseScore (transposition, répartition
en voix/portées, mesure d'anacrouse), pas des erreurs de notre port. Le
score N = 0,7592 du lot LilyPond contre le `.ly` ne doit pas être lu comme
une fidélité à MuseScore ; chaque cause doit être confirmée contre
l'import 4.7.4 du même MIDI. Ventilation du lot (référence `.ly`), pour
information : voix 17 967 notes, attaque 6 645, durée 6 041, n-olets 827,
hauteur 605 ; pires fichiers : `Laudate_pueri…`, `Sancta_Maria_Mater_Dei_Mozart`,
`Vincent-Lecornier-Jeanne-d-Arc`, `Lassus_Alleluja…`, `benoit_ave_maria`.

## Oracle LilyPond reconstruit sur MuseScore 4.7.4 (2026-11-21, suite)

`scripts/oracle/midi/ms-oracle.sh` convertit les 1 020 `.midi` du lot avec
MuseScore 4.7.4 (`env -u LD_LIBRARY_PATH QT_QPA_PLATFORM=offscreen mscore`),
en parallèle (`xargs -P nproc`), avec cache hors dépôt et reprise sur les
fichiers déjà produits. Il écrit `$OUT/done.txt` en condition de fin.
`categories_ms.py` calcule ensuite le score et la ventilation des pertes
contre cette référence.

Premiers résultats sur les 15 fichiers déjà convertis à ce stade : score
moyen de notre import contre MuseScore **0,8089**, avec trois fichiers exacts
à 1,0000 (`Vincent-Lecornier-Jeanne-d-Arc`, `Lassus_Alleluja…`,
`Ave_Verum…`). La référence `.ly` n'atteint que 0,4014 contre MuseScore sur
`Laudate_pueri…`, ce qui confirme que l'ancien N = 0,7592 mesurait surtout un
écart de choix d'import, pas une fidélité.

Cause identifiée sur `GaudeteChristusEstNatus_abrege` (notre score 0,6495,
pitch seul 0,7033, attaques/durées/voix exactes) : à partir de la mesure où
apparaît le programme 43 (contrabasse), MuseScore écrit les hauteurs une
octave plus haut que les notes MIDI et pose
`<transpose><octave-change>-1</octave-change>`. L'instrument
`contrabass` de `share/instruments/instruments.xml` porte en effet
`<transposeChromatic>-12</transposeChromatic>`. Notre import MIDI ne renseigne
pas `Instrument::transpose` :
`crates/scoretk-midi-import/src/instrument.rs` n'extrait que le nom, le
programme et le grand staff. C'est une cause réelle, localisée dans le C++
(`importmidi_instrument.cpp` : sélection du gabarit d'instrument), mais sa
correction exige la table des transpositions d'instruments, absente du crate
pour l'instant ; elle n'est donc pas encore commise.

Autres causes testées et rejetées ce tour (mesures enregistrées) : tri des
voix par `findBarIndexForOffTime` (upstream 258 → 261, LilyPond 31 615 →
31 660), attribution globale des voix + `sortVoices` fidèle (aucun gain sur
le cas réduit `Maiana`), somme des hauteurs brutes (aucun gain),
`useDots=false` (très régressif), `is_human=false` (aucun effet lot
LilyPond).

### Cause fixed: transposition des instruments (contrabasse)

`share/instruments/instruments.xml` donne à l'instrument `contrabass`
`<transposeDiatonic>-7</transposeDiatonic>` et
`<transposeChromatic>-12</transposeChromatic>` pour les programmes GM 32 et
43. MuseScore 4.7.4 pose donc `<transpose><octave-change>-1</…>` et écrit les
hauteurs une octave plus haut ; notre import MIDI ne renseignait pas
`Instrument::transpose`. Failing test first : `instrument_transpose` vérifie
que le programme 43 porte une transposition chromatique de -12.

`instrument::transpose_for_program` est appliqué dans `createInstruments`.
Sur les 15 pires fichiers du lot comparés à MuseScore 4.7.4, `GaudeteChristus…
` passe de **0,6495 à 1,0000** (150 notes retrouvées) et aucun autre fichier
ne régresse ; perte totale 2 776 → 2 626 notes. La mesure complète sur les
1 020 fichiers est en cours avec le nouvel oracle MuseScore.

## Oracle MuseScore complet (1 020 fichiers)

`ms-oracle.sh` a converti les 1 020 paires en parallèle (990 conversions,
30 reprises depuis le cache) ; `categories_ms.py` donne :

**N pondéré par les notes contre MuseScore 4.7.4 = 0,8018**
(29 049,8 notes perdues sur 146 573).

| Catégorie | Fichiers | Pertes (notes) |
| --- | ---: | ---: |
| pitch | 625 | 18 702,9 |
| duration | 295 | 5 210,0 |
| voice | 78 | 3 030,8 |
| n-olets | 15 | 1 928,0 |
| attaque | 7 | 178,0 |

La catégorie « pitch » domine : elle correspond presque entièrement aux
instruments transpositeurs dont MuseScore remplit `Instrument::transpose` et
que notre import MIDI ne renseigne pas encore. La correction de la
contrebasse (programmes 32/43) a fait passer `GaudeteChristusEstNatus…` de
0,6495 à 1,0000 sur les 15 fichiers de départ. Une tentative sur le programme
73 (transposition -5 d'après la référence MuseScore) n'a produit **aucun
changement** mesuré : la transposition n'est pas appliquée par notre
exporteur pour ce cas ; elle est révertie. Les 15 pires fichiers et la
ventilation complète sont dans `/tmp/oracle-midi-import/ms-cat/categories_ms.md`.

## Choix du gabarit d'instrument (remplace le cas spécial)

Le `match program { 32 | 43 => … }` introduit pour la contrebasse est supprimé.
`scripts/gen-instruments-table.py` génère désormais, dans
`crates/scoretk-core/src/instruments.rs` (donnée partagée, sans logique de
format, conformément à la règle des crates `-common`) :

- tous les programmes MIDI de chaque gabarit (`&Channel/<program>`), et non
  plus seulement le premier ;
- `transposeDiatonic` / `transposeChromatic` ;
- `clef`, `staffGroup`, jeu de percussion, plages `pPitchRange` et genre
  `common` (utiles au tri) ;
- `GM1_PREFERRED_IDS`, les 128 identifiants préférés de `GM1_SOUND_DATA`
  (`importmidi_instrument.cpp:60-190`).

`scoretk-musescore-import/src/read/instruments.rs` ne garde que la logique de
reconnaissance et consulte la table partagée. `MidiInstr::template_for_track`
porte `findInstrumentsForProgram` (gabarit préféré, gabarits du même
programme, repli sur le programme inférieur), et `createInstruments` applique
le `transpose` du gabarit à l'instrument de la partie.

Le point qui empêchait la transposition d'agir sur les hauteurs était le Tpc :
`write_pitch` soustrait `transpose_chromatic` à la hauteur, mais le
`tpc_transposed` devait être transposé par l'intervalle **opposé** ; une
transposition d'octave pure ne touche pas le Tpc. Tests réduits :
`instrument_transpose`, `instrument_template` (cor 60 → -7, contrebasse 43 →
octave, ocarina 79 → aucun). Sur les six fichiers réduits du lot LilyPond qui
avaient un cor (score 0,1060 contre MuseScore), le passage aux gabarits
donne **0,9908** ; `GaudeteChristusEstNatus…` reste à 1,0000.

`MidiInstr::sortInstrumentTemplates` est également porté : les gabarits
candidates sont triés par écart entre la plage `pPitchRange` du gabarit et
les hauteurs réelles de la piste (tri stable, le gabarit préféré restant
premier à écart égal). Sans ce tri, 14 fichiers du lot LilyPond régressaient
(gabarit `piccolo` à l'octave choisi à la place d'un gabarit non transpositeur)
: `Marc-Pinget-Tyrolienne` 0,0057 → 1,0000,
`Caifa_BeataVirgoMaria` 0,7417 → 0,9963, etc. Un fichier régresse en
contrepartie, `GaudeteChristusEstNatus…` (1,0000 → 0,9299) : le tri choisit
un gabarit d'ocarina transposé là où MuseScore garde l'ocarina préféré.
L'écart résiduel est documenté, pas masqué.

### Mesure finale du port des gabarits

Oracle complet contre MuseScore 4.7.4 (1 020 fichiers), avant → après :

- **N : 0,8018 → 0,8264** ;
- perte totale : 29 049,8 → 25 440,8 notes ;
- catégorie **pitch** : 18 702,9 → **15 359,0** notes (gain 3 343,9) ;
- durée 5 210 → 5 234, voix 3 030,8 → 3 030,8 (bruit de classement).

16 fichiers gagnent, dont six passent de 0,0/0,1060 à 1,0/0,9908 (cors),
et `Marc-Pinget-Tyrolienne` de 0,0057 à 1,0000. Trois fichiers perdent :
`GaudeteChristusEstNatus`, `…_abrege` et `…_abrege_a4` (1,0000 → 0,8569 /
0,9299 / 0,9299). Cause unique expliquée : le tri par plage de hauteur
choisit pour l'ocarina (programme 79) un gabarit transposé d'une quinte,
alors que MuseScore 4.7.4 conserve le gabarit `ocarina` préféré non
transpositeur. Le gain net reste largement positif ; l'écart ocarina est un
point connu du tri à affiner (`minPitchP`/`maxPitchP` ajustés par la
transposition dans le C++, pas encore reproduits).

## Correction ocarina et compacité de la table (2026-11-21, suite)

Test réduit d'abord : `instrument_template` exige que le programme 79
(ocarina) garde le gabarit préféré non transpositeur. Le test échouait avec
le tri par plage simple (0,106 → 0,930 sur `Gaudete…`).

`minPitchP`/`maxPitchP` des gabarits sont des hauteurs **écrites**, alors que
les accords MIDI sont en hauteur concert. `sortInstrumentTemplates` compare
les deux ; notre port comparait sans conversion. Le correctif applique
`+ transposeChromatic` à la plage de la piste **uniquement** quand le gabarit
préféré n'est pas transpositeur — précisément le cas ocarina, mais pas le cor
ni la contrebasse, dont le gabarit préféré est transpositeur et doit rester
choisi. Un test intermédiaire avec la conversion appliquée à tous les
gabarits a été mesuré et rejeté : il cassait 12 fichiers (piccolo notamment,
`Marc-Pinget-Tyrolienne` 1,0000 → 0,0114).

Mesure complète contre MuseScore 4.7.4 : **N 0,8264 → 0,8279**, pitch
15 359,0 → 15 137,0, **3 gains (`GaudeteChristusEstNatus` et ses deux
variantes, 0,8569/0,9299 → 1,0000), 0 perte**.

Taille wasm : `nix build .#wasm` donne 1 938 223 octets bruts / 779 376 gzip
avec la table détaillée. La table a été compactée : `programs: &[i16]` devient
un `program_mask: u128` avec `template_has_program`/`template_first_program`,
et les champs inutilisés `clef` et `common_genre` sont retirés du générateur.
Nouvelle mesure : **1 924 312 octets / 783 580 gzip** (le gzip augmente
légèrement, le brut baisse de 13,9 Ko). Le fichier généré passe de 8 580 à
8 870 lignes mais de 300 Ko à 261 Ko de source ; la donnée reste générée,
jamais éditée à la main.

## Oracle amont : hauteur concert vs hauteur écrite

Les trois pertes upstream apparues avec le port des gabarits
(`instrument_clef`, `pickup_long`, `instrument_channels`) n'étaient pas des
régressions de l'import : l'oracle lisait la hauteur **concert** du `.mscx`
attendu et la hauteur **écrite** de notre MusicXML (transposé par le gabarit).
Aucun nom de piste n'existe dans ces trois MIDI, donc une recherche par nom
ne peut pas les corriger. Failing test réduit : `test_concert_pitch.py`
vérifie qu'un MusicXML avec `<transpose chromatic="-7">` donne A4=69 écrit et
D4=62 concert. `import_mesure.py` convertit désormais nos hauteurs vers le
repère concert quand la référence est un `.mscx`. Mesure : **3 gains
(0,0/0,909 → 1,0), 0 perte** sur le lot amont ; le lot LilyPond est
inchangé (ses références MuseScore sont déjà en hauteur écrite).

## Causes ouvertes (2026-11-21)

### Simplification de trois notes — stationnée

Test réduit `simplify_order.rs` (ignoré, raison en attribut) : une piste,
trois notes 57@5,0→6,5, 60@6,0→6,5, 62@6,5→7,0. MuseScore 4.7.4 donne
2,0 / 0,5 / 0,5 ; nous donnons 57 en deux notes liées 1,0 + 0,5, 60 et 62
à 0,5 (le test attendu porte ces trois valeurs). Éliminé par la réduction
mécanique : le grand-staff (deux pistes, même canal), le canal (deux pistes,
canaux différents), et le nombre de pistes (deux notes sur une seule piste
donnent le même résultat des deux côtés). Le dump temporaire `MIDI_DUMP`
(retiré) montre que la première divergence apparaît dans
`simplify_durations_not_drums`, avec des voix déjà conformes à MuseScore
(57/62 en voix 2, 60 en voix 1). Piste du split métrique vérifiée et
éliminée : `split_unequal_chords` (`chord.rs:598-640`) ne regroupe que les
notes d'un même accord ayant des fins différentes ; ici chaque accord n'a
qu'une note. `lengthenNote` (`importmidi_simplify.cpp:64-166`) expliquerait
l'allongement de 60 mais pas pourquoi MuseScore allonge 57 à la place.
**Cause identifiée (2026-11-21) : la répartition en voix, pas la
simplification.** La référence MuseScore 4.7.4 du cas réduit (regénérée par
`mscore`) place A3 (57) en voix 2 et C4/D4 (60/62) en voix 1 ; notre
répartition place 57/62 en voix 2 et 60 en voix 1. `lengthenNote` suit alors
des accords « suivants » différents et allonge 60 au lieu de 57. Le port
littéral de `importmidi_simplify.cpp` ne corrige donc pas ce cas (voir
« Durations: literal port » et le prochain chantier `importmidi_voice.cpp`).

## Classement des causes restantes (lots MuseScore)

Du relevé complet (1 020 fichiers) : pitch 15 137 notes, durée 5 234,
voix 3 031, n-olets 1 928, attaque 178. Le poste « pitch » restant est
majoritairement le choix de gabarit (transpositions encore mal classées
pour quelques programmes), déjà partiellement traité ; la prochaine cause
vise la plus grande classe hors simplification ci-dessus.

### Classement complémentaire (mesure hybrid, 1 020 fichiers)

Hors cause stationnée, les plus grosses pertes par classe sont :
pitch — `Transeamus` 160 notes (0,742), `W-A-Mozart-Ave-Maria` 111 (0,474),
six fichiers à 96 (`lily-ced0e264`…, 0,372) ; durée — `Ad_te_Suspiramus`
199 (0,825), `LaMouetteGrise…` 168 (0,758) ; voix —
`Sancta_Maria_Mater_Dei_Mozart` 200 (0,938), `Maiana` 140 (0,779).
Pour le groupe `lily-ced0e264`/`Transeamus`, la référence MuseScore porte
`<transpose chromatic="-5">`而我们 aucun : le programme MIDI 73 devrait
choisir un gabarit `alto-flute` (-5, plage 55-91) plutôt que `flute` (0,
plage 59-98). Tentatives mesurées et rejetées : l'ajustement de plage
appliqué aux transpositions vers le bas (aucun gain sur ces fichiers, et
-0,07 sur `Gaudete…`), donc réverti. Ce classement et ces échecs sont
consignés ici pour la prochaine passe ; corrigés par le port littéral de la
sélection (§ « Sélection du gabarit » ci-dessous).

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

## Tuplets: literal detector port, opt-in (2026-11-21)

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

**Opt-in.** `filterTuplets` (`importmidi_tuplet_filter.cpp:691-786`, a DFS over
tuplet sets with voice intervals) is **not** ported; the port keeps a greedy
selection of non-overlapping tuplets by error (`tuplet/filter.rs`, documented
there). With that approximation, inserting tuplets before quantisation retimed
the 320 notes of `v4/timesig-09` and changed the model boundaries of
`dense_bars`. The detector is therefore behind `ImportOptions::tuplet_notation`
(`--midi-tuplet-notation`), default **off**: the default pipeline is unchanged
(measured byte-identical file by file), and the option enables
`<time-modification>`.

Reduced test `tuplet_detect.rs` (written first, failed before the port): a 3:2
tuplet with a foreign chord between two positions is detected; it runs with
`tuplet_notation = true`.

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
MuseScore's early stage it still receives **182** invented time modifications,
so the option stays opt-in and the insertion stays after the duration stages.

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

## Decision: tuplet notation stays opt-in (2026-11-21, after beat ties)

Requirement: make detection the default only if the full measurement shows no
per-file loss without explanation, with insertion at MuseScore's stage
(`convertMidi`: `findAllTuplets` inside `quantizeAllTracks`).

Final figures after the beat-tie fix and `addChordsBetweenTupletNotes`, same
manifests and oracle:

| Setting | LilyPond N | onset loss | duration loss | tuplet loss | upstream N |
| --- | ---: | ---: | ---: | ---: | ---: |
| default (old detector, MuseScore stage) | **0.9821** | **26.0** | **350.8** | 11 778.0 | 0.7662 |
| literal detector + filter (late insertion) | 0.9819 | 154.0 | 535.9 | **6 216.0** | **0.7743** |

The notation gain is real (tuplet loss −47 %, upstream +0.0081) but the literal
path loses two files (`OSalutarisHostia` and its duplicate, −0.0244: MuseScore
notates 4/3, 2/3, 1/3 where we keep the sounding values) and worsens
attacks/durations (it passes an empty tuplet list to `toDurationList`).
Inserted at MuseScore's early stage, `v4/timesig-09` — whose source has **no**
`<Tuplet>` — receives **182** invented time modifications with the DFS filter.
That is a per-file divergence without an explanation, so **detection stays
opt-in** (`ImportOptions::tuplet_notation` / `--midi-tuplet-notation`). To flip
it: `assignVoices`'s tuplet part must reject those, and `OSalutarisHostia`
must keep MuseScore's notated durations (or the oracle must compare sounding
durations).

## `human_tempo`: beat tracking is used upstream, our glue is regressive

MuseScore does use the beat tracker on import when the performance is detected
as human: `Quantize::setIfHumanPerformance` calls `MidiBeat::findBeatLocations`
(`importmidi_quant.cpp:399-480`) and `convertMidi` always calls
`MidiBeat::adjustChordsToBeats` (`importmidi.cpp`, after `collectChords`),
which is a no-op for non-human files. Our port detects the same human
performances (`quant::is_human_performance`) and contains the BeatRoot glue,
but it is **not wired**: the measured wiring changed `human_tempo` from 0.5000
to 0.2958 before the beat-tie fix, and the induction/anchoring
(`updateFirstLastBeats`) does not reproduce MuseScore's grid. After the
beat-tie fix the file scores 0.3813 (default), so the remaining cause is the
tracker itself; wiring it without a correct anchoring would regress the row
again. Documented as an open cause, not enabled.

## Tuplet notation: MuseScore's notated durations (2026-11-21)

With the literal detector and `filterTuplets`, the remaining difference on
`OSalutarisHostia` was the model duration of tuplet notes: MixeScore writes
notated durations (`toDurationList` is called with the tuplet ratios,
`processPendingNotes`, `importmidi.cpp:553-611`), while the opt-in path passed
an empty tuplet list and kept the sounding values. `duration_list` now always
uses `find_tuplets_in_bar_for_duration`; with the option, `OSalutarisHostia`
has **4 time modifications, exactly the reference's 4**, and its notes match
(one duration at 1.5 against 1.25, within the oracle tolerance).

Measurement with the option (same manifests, same oracle):

| Metric | Default | `tuplet_notation` |
| --- | ---: | ---: |
| LilyPond N | 0.9821 | 0.9821 (identical file by file) |
| files with N gain / loss | — | **0 / 0** |
| `tuplet` dimension loss | 11 778.0 | **6 216.0** |
| `onset` / `duration` dimensions | 26.0 / 350.8 | 154.0 / 656.9 |
| upstream N | 0.7662 | **0.7743** |

The option is now **lossless per file** on both lots and gains the tuplet
notation and the upstream row. It is **still not the default** because:

- `v4/timesig-09` (source without `<Tuplet>`) receives 182 invented time
  modifications if the tuplets are inserted at MuseScore's early stage;
- the tuplet part of `assignVoices` (`setTupletVoices`,
  `importmidi_tuplet_voice.cpp:112-134`) was implemented and measured on the
  late path: moving already-sorted voices after `separateVoices` loses notes on
  **221 files** (worst −0.28, `UztaiDantza`), so it is not applied; it belongs
  to the early stage, together with `removeUnusedTuplets` /
  `excludeExtraVoiceTuplets` and the back-tied tuplets
  (`findBackTiedTuplets`, `:802-870`), which need the tie chains of the early
  model.

So the decision stays opt-in, now with a measured lossless option and a
precise condition to flip it: port the early tuplet voice assignment and the
back-tied tuplets, then the early insertion must leave `timesig-09` without
tuplets.

## MIDI import: status (2026-11-21)
retk-midi-import`, `scoretk-midi-export`)

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
scripts/gen-midi-references.sh            # toutes les références du sous-ensemble
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
scripts/gen-midi-import-references.sh            # toutes les références
scripts/gen-midi-import-references.sh tests/corpus/midi/v4/dynamics.ref.mid
nix develop -c cargo test -p scoretk-midi-import --test import_corpus -- --nocapture
nix develop -c cargo test -p scoretk-midi-export --test roundtrip_midi -- --nocapture
```

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
scripts/gen-midi-import-references.sh                # avec MuseScore 4.7.4
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

### Ports 2 et 3 : résultats (2026-11-20, suite)

- **Fermata — aucun port nécessaire.** `Fermata::timeStretch`
  (`engraving/dom/fermata.cpp:138-139, 250`, `Pid::TIME_STRETCH`) n'est pas
  consulté par le rendu MIDI de compatibilité. Une partition réduite avec un
  `fermataAbove` sur une noire, exportée par MuseScore 4.7.4, donne quatre
  notes d'une noire à 0/1/2/3 — exactement notre sortie. Le test
  `tests/fermata_no_stretch.rs` fixe ce comportement pour éviter un port
  erroné du `timeStretch`.
- **Pause au saut D.S. — expérience refusée.** Un `.mscx` 2.06 réduit avec un
  `LayoutBreak section` dans la mesure du saut reproduit l'écart : MuseScore
  insère une pause de 4 noires après **chaque** passage, notre `unfold` ne
  l'insère que devant la mesure suivante de la partition. Rendre la borne de
  segment inclusive (`tick <= segment.end`) fait passer ce test réduit mais
  **régresse `Vingt_danses` de 2373 attaques** (mesure complète à références
  égales) : la pause y tombe aussi sur des frontières de segments ordinaires.
  Le changement a été annulé. Une correction correcte doit rattacher la pause
  à la mesure qui porte le `LayoutBreak` et la rejouer aux passages du saut,
  dans `PauseMap`/`RepeatList`, sans toucher aux frontières ordinaires.

Le seul port de cette série est donc l'auxiliaire de trille selon l'armure
(commit précédent) : 19 fichiers `.mscz` gagnent 1319 attaques, 0 perte,
N2 94,57 → 94,92 %.

### Pauses : port de `collectPauses` par segment (2026-11-20)

L'heuristique `jump_back` du commit précédent n'était pas un port : elle ne
gagnait rien et perdait 19 notes sur `Vingt_danses`. Elle est remplacée par le
mécanisme réel de MuseScore.

`ScoreAutomationController::collectPauses`
(`src/engraving/automation/internal/scoreautomationcontroller.cpp:1100-1131`,
appelé à `:549` pour chaque mesure de chaque `RepeatSegment`) inscrit la pause
d'une rupture de section au `startUtick = measure.tick() + tickOffset` de la
mesure qui suit la rupture dans l'ordre brut (`measure->prev()`, `:1105-1111`),
avec `tickOffset = seg->utick - seg->tick` (`:542`). `PauseMap::calculate`
(`compat/midi/pausemap.cpp:47-76`) utilise ces uticks directement.

`pause_map.rs::pauses_by_segment` porte ce mécanisme : il parcourt
`repeat_list.segments`, puis les mesures de chaque segment, et inscrit
`sectionPause` (3 s, `Sid::sectionPause`, `styledef.cpp:643`) à l'utick de
chaque mesure marquée `section_break_before`. L'`unfold` des tempos reste
inchangé. Le test réduit `tests/section_pause_jump.rs` (2.06, rupture de
section, MIDI MuseScore 4.7.4 : notes aux ticks 0, 4 et 12) passe.

Mesure complète à références égales : **aucune perte**, `Vingt_danses`
récupère les 19 notes perdues par l'heuristique (N2 10 344 → 10 363),
N2 global 94,91 → **94,92 %**. Le décalage d'un quart au tick 1507 n'est
toujours pas corrigé : la pause concernée n'appartient à aucun passage où le
modèle porte `section_break_before`, ce qui reste à identifier dans le
lecteur 2.x (`read206`).

### Pause au saut de `Vingt_danses` : évènements de tempo (2026-11-20)

Les évènements `MetaMessage set_tempo` des deux MIDIs autour du tick 1507
lèvent l'ambiguïté : la référence 4.7.4 porte **20 BPM de 722 880 à 723 360**
(120 BPM ensuite), soit une pause d'un quart de noire exactement au passage
du saut `D.S. al Fine` ; notre MIDI n'avait aucun évènement de tempo là. La
cause n'était donc ni un tempo écrit ni une mesure manquante, mais la pause
de la rupture de section portée par la mesure du saut et jouée au début du
segment suivant dans l'ordre d'exécution (`collectPauses`,
`scoreautomationcontroller.cpp:1100-1131` ; `RepeatSegment::utick`). Le
nombre de noires inséré est celui de la mesure qui ouvre la nouvelle section
(anacrouse d'un quart dans `Vingt_danses`).

`pauses_by_segment` porte désormais ce cas : quand le segment précédent se
termine sur une mesure avec `<Jump>` et que la mesure suivante de la partition
porte la rupture, la pause est inscrite à l'utick du segment courant avec ce
nombre de noires (`pause_quarters` dans `TempoPoint`, utilisé par
`PauseMap::calculate` à la place de la longueur de la mesure au tick source).
Le garde `index > 0` de `collectPauses` (`measure->prev()` nul pour la
première mesure) est restauré, avec un test unitaire.

Mesure complète à références égales : **+358 notes sur `Vingt_danses`,
0 perte** ; N2 `.mscz` 94,92 → **95,01 %**. Les autres lots ne changent pas.

### Correction de `d39d7f5` : fraction des mesures de pause (2026-11-20)

`d39d7f5` convertissait `quarter_notes_at()` en `i64` avant de le réutiliser :
une mesure de 5/8 (2,5 noires) devenait 2 noires, aussi bien pour le tempo que
pour les ticks insérés. `pause_quarters` est désormais `Option<f64>` et la
valeur fractionnaire est conservée (`PauseMap::calculate`, `pausemap.cpp:68-71`
: `quarterNotesPerMeasure` est un `double`, seul `ticksPerMeasure` est un
`int`). Test unitaire `pause_en_cinq_huit_sans_troncature` : 1200 ticks et
2,5/3 bps. Le garde `index > 0` de `collectPauses` est également restauré
(`measure->prev()` nul pour la première mesure), testé par
`premiere_mesure_sans_pause_de_section`.

Le gain de +358 notes annoncé par `d39d7f5` était un artefact de cette
troncature sur les mesures irrégulières de `Vingt_danses`, pas l'effet de la
règle de saut (dont le motif `RepeatMark::Jump` n'a pas de contrepartie dans
`collectPauses`, qui ne raisonne que sur `measure->prev()` en ordre brut).
Avec la valeur correcte, cette règle n'apporte rien et a été retirée ; le
décalage au tick 1507 reste ouvert. La référence porte bien un évènement
`20 BPM` de 722 880 à 723 360, mais les essais d'ancrage au segment suivant
 ont tous régressé d'autres passages — le mécanisme exact reste à identifier.

### Bilan de la campagne des pauses (2026-11-20)

Comparaison fichier par fichier contre **master** (références et manifeste
identiques) : avec la valeur fractionnaire conservée pour le tempo et la
**troncature** de `ticksPerMeasure` (`pausemap.cpp:70`), **aucun fichier ne
gagne ni ne perd** ; les deux écarts
apparents (`Vivaldi … Judicabit`, `Vivaldi … Gloria Patri`) sont des dépassements
de la borne de 10 s de l'oracle — relancés sous 20 s, ils donnent exactement
les valeurs de master (2469 et 1677).

Le décalage d'un quart au tick 1507 reste ouvert, avec la preuve suivante :
le MIDI 4.7.4 porte un changement de tempo à 20 BPM de 722 880 à 723 360, soit
`quarterNotesPerMeasure / pause = 1 / 3 s` (`pausemap.cpp:71`) : la pause est
bien celle de la mesure anacrustique `len="1/4"` qui suit le saut, mais elle est
jouée à la frontière du saut. Dans notre modèle, cette mesure n'est atteinte
qu'après `fine4`, et toutes les tentatives pour l'ancre au saut (marqueur
`Jump`,mesure d'irregularité, bornes inclusives) ont régressé d'autres passages
de la même partition — le mécanisme exact qui place cette anacrouse dans la
`RepeatSegment` de 4.7.4 n'est pas dans le portage. Un gain net sur master
n'est donc pas démontrable sans ce mécanisme ; le correctif de troncature est
conservé pour sa conformité à `pausemap.cpp:68-71`, sans gain de mesure.

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

## Ancrage BeatRoot et mesure (2026-11-21, suite)

`BeatData` porte désormais `firstChordTick`, `lastChordTick` et les nombres de
temps ajoutés ; `set_time_sig` reproduit
`MidiBeat::setTimeSignature` + `updateFirstLastBeats`
(`importmidi_beat.cpp:472-511`) : la carte de mesure est remplacée par la
signature retenue, puis les temps ajoutés avec l'ancienne signature sont
retirés et recalculés. Le bug d'index de `Agent::fillBeats` (l'insertion
d'un temps interpolé faisait sauter l'événement suivant) est corrigé.

Le câblage complet a été mesuré dans `convertMidi`, exactement à l'endroit
où MuseScore l'appelle (`setIfHumanPerformance` puis `adjustChordsToBeats`
avant la seconde collecte) : **aucun gain, `human_tempo` perd 29 notes
(0,5000 → 0,2958)**, lot LilyPond inchangé. Diagnostic chiffré : le fichier
donne 71 attaques, BeatRoot produit 11 temps espacés de ~0,32 s, et
`addFirstBeats` étend la grille à 17 temps de 1,93 à 23,8 croches. La
première attaque (1,30 s) tombe alors à 2,5 croches après mise à l'échelle,
alors que la référence l'attend à 3,875 croches. L'hypothèse de tempo
devrait être proche de 0,234 s (480 ticks) et ne l'est pas : il reste un
écart entre notre induction et celle de MuseScore sur ce fichier, et le
branchement fidèle ne peut donc pas être fusionné sans régression. La glue
est conservée mais **inactive** ; c'est un résultat mesuré, pas un port
non testé.

## Lot LilyPond : la référence `.ly` n'est pas MuseScore

MuseScore 4.7.4 est disponible et fonctionne avec
`env -u LD_LIBRARY_PATH QT_QPA_PLATFORM=offscreen mscore`. Première
vérification « like with like » sur le pire fichier du lot
(`Laudate_pueri…`) :

- import MuseScore 4.7.4 du même `.midi` → 1 826 notes ;
- notre import → 1 836 notes ; score **0,8587** contre MuseScore ;
- la référence `.ly` (notre lecteur LilyPond) → 1 796 notes ; score de
  MuseScore contre elle : **0,4014**.

Donc les 1 063 notes « perdues » de ce fichier contre le `.ly` sont
majoritairement des choix d'import de MuseScore (transposition, répartition
en voix/portées, mesure d'anacrouse), pas des erreurs de notre port. Le
score N = 0,7592 du lot LilyPond contre le `.ly` ne doit pas être lu comme
une fidélité à MuseScore ; chaque cause doit être confirmée contre
l'import 4.7.4 du même MIDI. Ventilation du lot (référence `.ly`), pour
information : voix 17 967 notes, attaque 6 645, durée 6 041, n-olets 827,
hauteur 605 ; pires fichiers : `Laudate_pueri…`, `Sancta_Maria_Mater_Dei_Mozart`,
`Vincent-Lecornier-Jeanne-d-Arc`, `Lassus_Alleluja…`, `benoit_ave_maria`.

## Oracle LilyPond reconstruit sur MuseScore 4.7.4 (2026-11-21, suite)

`scripts/oracle/midi/ms-oracle.sh` convertit les 1 020 `.midi` du lot avec
MuseScore 4.7.4 (`env -u LD_LIBRARY_PATH QT_QPA_PLATFORM=offscreen mscore`),
en parallèle (`xargs -P nproc`), avec cache hors dépôt et reprise sur les
fichiers déjà produits. Il écrit `$OUT/done.txt` en condition de fin.
`categories_ms.py` calcule ensuite le score et la ventilation des pertes
contre cette référence.

Premiers résultats sur les 15 fichiers déjà convertis à ce stade : score
moyen de notre import contre MuseScore **0,8089**, avec trois fichiers exacts
à 1,0000 (`Vincent-Lecornier-Jeanne-d-Arc`, `Lassus_Alleluja…`,
`Ave_Verum…`). La référence `.ly` n'atteint que 0,4014 contre MuseScore sur
`Laudate_pueri…`, ce qui confirme que l'ancien N = 0,7592 mesurait surtout un
écart de choix d'import, pas une fidélité.

Cause identifiée sur `GaudeteChristusEstNatus_abrege` (notre score 0,6495,
pitch seul 0,7033, attaques/durées/voix exactes) : à partir de la mesure où
apparaît le programme 43 (contrabasse), MuseScore écrit les hauteurs une
octave plus haut que les notes MIDI et pose
`<transpose><octave-change>-1</octave-change>`. L'instrument
`contrabass` de `share/instruments/instruments.xml` porte en effet
`<transposeChromatic>-12</transposeChromatic>`. Notre import MIDI ne renseigne
pas `Instrument::transpose` :
`crates/scoretk-midi-import/src/instrument.rs` n'extrait que le nom, le
programme et le grand staff. C'est une cause réelle, localisée dans le C++
(`importmidi_instrument.cpp` : sélection du gabarit d'instrument), mais sa
correction exige la table des transpositions d'instruments, absente du crate
pour l'instant ; elle n'est donc pas encore commise.

Autres causes testées et rejetées ce tour (mesures enregistrées) : tri des
voix par `findBarIndexForOffTime` (upstream 258 → 261, LilyPond 31 615 →
31 660), attribution globale des voix + `sortVoices` fidèle (aucun gain sur
le cas réduit `Maiana`), somme des hauteurs brutes (aucun gain),
`useDots=false` (très régressif), `is_human=false` (aucun effet lot
LilyPond).

### Cause fixed: transposition des instruments (contrabasse)

`share/instruments/instruments.xml` donne à l'instrument `contrabass`
`<transposeDiatonic>-7</transposeDiatonic>` et
`<transposeChromatic>-12</transposeChromatic>` pour les programmes GM 32 et
43. MuseScore 4.7.4 pose donc `<transpose><octave-change>-1</…>` et écrit les
hauteurs une octave plus haut ; notre import MIDI ne renseignait pas
`Instrument::transpose`. Failing test first : `instrument_transpose` vérifie
que le programme 43 porte une transposition chromatique de -12.

`instrument::transpose_for_program` est appliqué dans `createInstruments`.
Sur les 15 pires fichiers du lot comparés à MuseScore 4.7.4, `GaudeteChristus…
` passe de **0,6495 à 1,0000** (150 notes retrouvées) et aucun autre fichier
ne régresse ; perte totale 2 776 → 2 626 notes. La mesure complète sur les
1 020 fichiers est en cours avec le nouvel oracle MuseScore.

## Oracle MuseScore complet (1 020 fichiers)

`ms-oracle.sh` a converti les 1 020 paires en parallèle (990 conversions,
30 reprises depuis le cache) ; `categories_ms.py` donne :

**N pondéré par les notes contre MuseScore 4.7.4 = 0,8018**
(29 049,8 notes perdues sur 146 573).

| Catégorie | Fichiers | Pertes (notes) |
| --- | ---: | ---: |
| pitch | 625 | 18 702,9 |
| duration | 295 | 5 210,0 |
| voice | 78 | 3 030,8 |
| n-olets | 15 | 1 928,0 |
| attaque | 7 | 178,0 |

La catégorie « pitch » domine : elle correspond presque entièrement aux
instruments transpositeurs dont MuseScore remplit `Instrument::transpose` et
que notre import MIDI ne renseigne pas encore. La correction de la
contrebasse (programmes 32/43) a fait passer `GaudeteChristusEstNatus…` de
0,6495 à 1,0000 sur les 15 fichiers de départ. Une tentative sur le programme
73 (transposition -5 d'après la référence MuseScore) n'a produit **aucun
changement** mesuré : la transposition n'est pas appliquée par notre
exporteur pour ce cas ; elle est révertie. Les 15 pires fichiers et la
ventilation complète sont dans `/tmp/oracle-midi-import/ms-cat/categories_ms.md`.

## Choix du gabarit d'instrument (remplace le cas spécial)

Le `match program { 32 | 43 => … }` introduit pour la contrebasse est supprimé.
`scripts/gen-instruments-table.py` génère désormais, dans
`crates/scoretk-core/src/instruments.rs` (donnée partagée, sans logique de
format, conformément à la règle des crates `-common`) :

- tous les programmes MIDI de chaque gabarit (`&Channel/<program>`), et non
  plus seulement le premier ;
- `transposeDiatonic` / `transposeChromatic` ;
- `clef`, `staffGroup`, jeu de percussion, plages `pPitchRange` et genre
  `common` (utiles au tri) ;
- `GM1_PREFERRED_IDS`, les 128 identifiants préférés de `GM1_SOUND_DATA`
  (`importmidi_instrument.cpp:60-190`).

`scoretk-musescore-import/src/read/instruments.rs` ne garde que la logique de
reconnaissance et consulte la table partagée. `MidiInstr::template_for_track`
porte `findInstrumentsForProgram` (gabarit préféré, gabarits du même
programme, repli sur le programme inférieur), et `createInstruments` applique
le `transpose` du gabarit à l'instrument de la partie.

Le point qui empêchait la transposition d'agir sur les hauteurs était le Tpc :
`write_pitch` soustrait `transpose_chromatic` à la hauteur, mais le
`tpc_transposed` devait être transposé par l'intervalle **opposé** ; une
transposition d'octave pure ne touche pas le Tpc. Tests réduits :
`instrument_transpose`, `instrument_template` (cor 60 → -7, contrebasse 43 →
octave, ocarina 79 → aucun). Sur les six fichiers réduits du lot LilyPond qui
avaient un cor (score 0,1060 contre MuseScore), le passage aux gabarits
donne **0,9908** ; `GaudeteChristusEstNatus…` reste à 1,0000.

`MidiInstr::sortInstrumentTemplates` est également porté : les gabarits
candidates sont triés par écart entre la plage `pPitchRange` du gabarit et
les hauteurs réelles de la piste (tri stable, le gabarit préféré restant
premier à écart égal). Sans ce tri, 14 fichiers du lot LilyPond régressaient
(gabarit `piccolo` à l'octave choisi à la place d'un gabarit non transpositeur)
: `Marc-Pinget-Tyrolienne` 0,0057 → 1,0000,
`Caifa_BeataVirgoMaria` 0,7417 → 0,9963, etc. Un fichier régresse en
contrepartie, `GaudeteChristusEstNatus…` (1,0000 → 0,9299) : le tri choisit
un gabarit d'ocarina transposé là où MuseScore garde l'ocarina préféré.
L'écart résiduel est documenté, pas masqué.

### Mesure finale du port des gabarits

Oracle complet contre MuseScore 4.7.4 (1 020 fichiers), avant → après :

- **N : 0,8018 → 0,8264** ;
- perte totale : 29 049,8 → 25 440,8 notes ;
- catégorie **pitch** : 18 702,9 → **15 359,0** notes (gain 3 343,9) ;
- durée 5 210 → 5 234, voix 3 030,8 → 3 030,8 (bruit de classement).

16 fichiers gagnent, dont six passent de 0,0/0,1060 à 1,0/0,9908 (cors),
et `Marc-Pinget-Tyrolienne` de 0,0057 à 1,0000. Trois fichiers perdent :
`GaudeteChristusEstNatus`, `…_abrege` et `…_abrege_a4` (1,0000 → 0,8569 /
0,9299 / 0,9299). Cause unique expliquée : le tri par plage de hauteur
choisit pour l'ocarina (programme 79) un gabarit transposé d'une quinte,
alors que MuseScore 4.7.4 conserve le gabarit `ocarina` préféré non
transpositeur. Le gain net reste largement positif ; l'écart ocarina est un
point connu du tri à affiner (`minPitchP`/`maxPitchP` ajustés par la
transposition dans le C++, pas encore reproduits).

## Correction ocarina et compacité de la table (2026-11-21, suite)

Test réduit d'abord : `instrument_template` exige que le programme 79
(ocarina) garde le gabarit préféré non transpositeur. Le test échouait avec
le tri par plage simple (0,106 → 0,930 sur `Gaudete…`).

`minPitchP`/`maxPitchP` des gabarits sont des hauteurs **écrites**, alors que
les accords MIDI sont en hauteur concert. `sortInstrumentTemplates` compare
les deux ; notre port comparait sans conversion. Le correctif applique
`+ transposeChromatic` à la plage de la piste **uniquement** quand le gabarit
préféré n'est pas transpositeur — précisément le cas ocarina, mais pas le cor
ni la contrebasse, dont le gabarit préféré est transpositeur et doit rester
choisi. Un test intermédiaire avec la conversion appliquée à tous les
gabarits a été mesuré et rejeté : il cassait 12 fichiers (piccolo notamment,
`Marc-Pinget-Tyrolienne` 1,0000 → 0,0114).

Mesure complète contre MuseScore 4.7.4 : **N 0,8264 → 0,8279**, pitch
15 359,0 → 15 137,0, **3 gains (`GaudeteChristusEstNatus` et ses deux
variantes, 0,8569/0,9299 → 1,0000), 0 perte**.

Taille wasm : `nix build .#wasm` donne 1 938 223 octets bruts / 779 376 gzip
avec la table détaillée. La table a été compactée : `programs: &[i16]` devient
un `program_mask: u128` avec `template_has_program`/`template_first_program`,
et les champs inutilisés `clef` et `common_genre` sont retirés du générateur.
Nouvelle mesure : **1 924 312 octets / 783 580 gzip** (le gzip augmente
légèrement, le brut baisse de 13,9 Ko). Le fichier généré passe de 8 580 à
8 870 lignes mais de 300 Ko à 261 Ko de source ; la donnée reste générée,
jamais éditée à la main.

## Oracle amont : hauteur concert vs hauteur écrite

Les trois pertes upstream apparues avec le port des gabarits
(`instrument_clef`, `pickup_long`, `instrument_channels`) n'étaient pas des
régressions de l'import : l'oracle lisait la hauteur **concert** du `.mscx`
attendu et la hauteur **écrite** de notre MusicXML (transposé par le gabarit).
Aucun nom de piste n'existe dans ces trois MIDI, donc une recherche par nom
ne peut pas les corriger. Failing test réduit : `test_concert_pitch.py`
vérifie qu'un MusicXML avec `<transpose chromatic="-7">` donne A4=69 écrit et
D4=62 concert. `import_mesure.py` convertit désormais nos hauteurs vers le
repère concert quand la référence est un `.mscx`. Mesure : **3 gains
(0,0/0,909 → 1,0), 0 perte** sur le lot amont ; le lot LilyPond est
inchangé (ses références MuseScore sont déjà en hauteur écrite).

## Causes ouvertes (2026-11-21)

### Simplification de trois notes — stationnée

Test réduit `simplify_order.rs` (ignoré, raison en attribut) : une piste,
trois notes 57@5,0→6,5, 60@6,0→6,5, 62@6,5→7,0. MuseScore 4.7.4 donne
2,0 / 0,5 / 0,5 ; nous donnons 57 en deux notes liées 1,0 + 0,5, 60 et 62
à 0,5 (le test attendu porte ces trois valeurs). Éliminé par la réduction
mécanique : le grand-staff (deux pistes, même canal), le canal (deux pistes,
canaux différents), et le nombre de pistes (deux notes sur une seule piste
donnent le même résultat des deux côtés). Le dump temporaire `MIDI_DUMP`
(retiré) montre que la première divergence apparaît dans
`simplify_durations_not_drums`, avec des voix déjà conformes à MuseScore
(57/62 en voix 2, 60 en voix 1). Piste du split métrique vérifiée et
éliminée : `split_unequal_chords` (`chord.rs:598-640`) ne regroupe que les
notes d'un même accord ayant des fins différentes ; ici chaque accord n'a
qu'une note. `lengthenNote` (`importmidi_simplify.cpp:64-166`) expliquerait
l'allongement de 60 mais pas pourquoi MuseScore allonge 57 à la place.
**Cause identifiée (2026-11-21) : la répartition en voix, pas la
simplification.** La référence MuseScore 4.7.4 du cas réduit (regénérée par
`mscore`) place A3 (57) en voix 2 et C4/D4 (60/62) en voix 1 ; notre
répartition place 57/62 en voix 2 et 60 en voix 1. `lengthenNote` suit alors
des accords « suivants » différents et allonge 60 au lieu de 57. Le port
littéral de `importmidi_simplify.cpp` ne corrige donc pas ce cas (voir
« Durations: literal port » et le prochain chantier `importmidi_voice.cpp`).

## Classement des causes restantes (lots MuseScore)

Du relevé complet (1 020 fichiers) : pitch 15 137 notes, durée 5 234,
voix 3 031, n-olets 1 928, attaque 178. Le poste « pitch » restant est
majoritairement le choix de gabarit (transpositions encore mal classées
pour quelques programmes), déjà partiellement traité ; la prochaine cause
vise la plus grande classe hors simplification ci-dessus.

### Classement complémentaire (mesure hybrid, 1 020 fichiers)

Hors cause stationnée, les plus grosses pertes par classe sont :
pitch — `Transeamus` 160 notes (0,742), `W-A-Mozart-Ave-Maria` 111 (0,474),
six fichiers à 96 (`lily-ced0e264`…, 0,372) ; durée — `Ad_te_Suspiramus`
199 (0,825), `LaMouetteGrise…` 168 (0,758) ; voix —
`Sancta_Maria_Mater_Dei_Mozart` 200 (0,938), `Maiana` 140 (0,779).
Pour le groupe `lily-ced0e264`/`Transeamus`, la référence MuseScore porte
`<transpose chromatic="-5">`而我们 aucun : le programme MIDI 73 devrait
choisir un gabarit `alto-flute` (-5, plage 55-91) plutôt que `flute` (0,
plage 59-98). Tentatives mesurées et rejetées : l'ajustement de plage
appliqué aux transpositions vers le bas (aucun gain sur ces fichiers, et
-0,07 sur `Gaudete…`), donc réverti. Ce classement et ces échecs sont
consignés ici pour la prochaine passe ; corrigés par le port littéral de la
sélection (§ « Sélection du gabarit » ci-dessous).

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

## Tuplets: literal detector port, opt-in (2026-11-21)

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

**Opt-in.** `filterTuplets` (`importmidi_tuplet_filter.cpp:691-786`, a DFS over
tuplet sets with voice intervals) is **not** ported; the port keeps a greedy
selection of non-overlapping tuplets by error (`tuplet/filter.rs`, documented
there). With that approximation, inserting tuplets before quantisation retimed
the 320 notes of `v4/timesig-09` and changed the model boundaries of
`dense_bars`. The detector is therefore behind `ImportOptions::tuplet_notation`
(`--midi-tuplet-notation`), default **off**: the default pipeline is unchanged
(measured byte-identical file by file), and the option enables
`<time-modification>`.

Reduced test `tuplet_detect.rs` (written first, failed before the port): a 3:2
tuplet with a foreign chord between two positions is detected; it runs with
`tuplet_notation = true`.

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
MuseScore's early stage it still receives **182** invented time modifications,
so the option stays opt-in and the insertion stays after the duration stages.

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

## Decision: tuplet notation stays opt-in (2026-11-21, after beat ties)

Requirement: make detection the default only if the full measurement shows no
per-file loss without explanation, with insertion at MuseScore's stage
(`convertMidi`: `findAllTuplets` inside `quantizeAllTracks`).

Final figures after the beat-tie fix and `addChordsBetweenTupletNotes`, same
manifests and oracle:

| Setting | LilyPond N | onset loss | duration loss | tuplet loss | upstream N |
| --- | ---: | ---: | ---: | ---: | ---: |
| default (old detector, MuseScore stage) | **0.9821** | **26.0** | **350.8** | 11 778.0 | 0.7662 |
| literal detector + filter (late insertion) | 0.9819 | 154.0 | 535.9 | **6 216.0** | **0.7743** |

The notation gain is real (tuplet loss −47 %, upstream +0.0081) but the literal
path loses two files (`OSalutarisHostia` and its duplicate, −0.0244: MuseScore
notates 4/3, 2/3, 1/3 where we keep the sounding values) and worsens
attacks/durations (it passes an empty tuplet list to `toDurationList`).
Inserted at MuseScore's early stage, `v4/timesig-09` — whose source has **no**
`<Tuplet>` — receives **182** invented time modifications with the DFS filter.
That is a per-file divergence without an explanation, so **detection stays
opt-in** (`ImportOptions::tuplet_notation` / `--midi-tuplet-notation`). To flip
it: `assignVoices`'s tuplet part must reject those, and `OSalutarisHostia`
must keep MuseScore's notated durations (or the oracle must compare sounding
durations).

## `human_tempo`: beat tracking is used upstream, our glue is regressive

MuseScore does use the beat tracker on import when the performance is detected
as human: `Quantize::setIfHumanPerformance` calls `MidiBeat::findBeatLocations`
(`importmidi_quant.cpp:399-480`) and `convertMidi` always calls
`MidiBeat::adjustChordsToBeats` (`importmidi.cpp`, after `collectChords`),
which is a no-op for non-human files. Our port detects the same human
performances (`quant::is_human_performance`) and contains the BeatRoot glue,
but it is **not wired**: the measured wiring changed `human_tempo` from 0.5000
to 0.2958 before the beat-tie fix, and the induction/anchoring
(`updateFirstLastBeats`) does not reproduce MuseScore's grid. After the
beat-tie fix the file scores 0.3813 (default), so the remaining cause is the
tracker itself; wiring it without a correct anchoring would regress the row
again. Documented as an open cause, not enabled.

## MIDI import: status (2026-11-21)

- **Default pipeline**: LilyPond N **0.9821** (1 020 files), upstream N
  **0.7662** (96 files). Losses: voice 1 076.1, duration 350.8, onset 26.0,
  pitch 0.0, tuplet flag 11 778.0.
- **Instrument template selection**: literal port (previous turn).
- **Duration simplification**: literal port of `lengthenNote` /
  `minimizeNumberOfRests` (previous turn).
- **Voices**: literal port of `separateVoices` (previous turn).
- **Beat ties**: literal `metricDivisionsOfBar` subdivisions (this turn):
  18 files gain, none loses; upstream `human_tempo` −0.1439.
- **Tuplets**: literal detector + `filterTuplets` + `addChordsBetweenTupletNotes`,
  opt-in; `tuplet_off_time_other_bar2` reaches 1.0 under the option.
- **Open**: `assignVoices` tuplet part, tuplet-aware `toDurationList` default,
  `human_tempo` beat tracker, `02_02_Segura` voice tie chains, `preserveLegato`
  on 0.01-quarter overlaps (not the cause of `tuplet_off_time_other_bar2`).

## MIDI export: status

Campaign of 2026-11-21 on branch `midi-export`, measured at HEAD before the
first change (**baseline**) and after the four causes below (**current**).
The oracle is the existing writer oracle, unchanged:
`scripts/oracle/midi/mesure.py` (MIDI ↔ MIDI; pitch, onset, duration and
track; Q16 grid, ±1/16-quarter tolerance) through the same
`timeout 10 prlimit --as=2000000000` bound. Unsupported inputs are recorded as
failures, not as silent zeros; the only remaining failure is the MuseScore
2.06 `.mscz` file (`Canon_in_D`, code 1) that the reader does not accept.

Lots and references (manifests outside the repository):

* **`ecriture_mscz` — 174 `.mscz` from `~/Musique` vs MuseScore 4.7.4.**
  The references are produced by `scripts/oracle/midi/mscz-refs.sh <out>`
  (new, reproducible: parallel headless export, out-of-repository cache keyed
  by a hash of the source path, `done.txt` written at the end) and consumed as
  a two-column manifest.
* **`ecriture_ly` — 1 020 `.ly` vs LilyPond's own MIDI** (`lots.sh`
  pairs each `.midi` with its `.ly`).
* **`ecriture_mscz_corpus` — the 21 versioned corpus files**
  (`tests/corpus/midi/*.ref.mid`, MuseScore 4.7.4).

```sh
nix develop -c cargo build -p scoretk-cli
scripts/oracle/midi/mscz-refs.sh /tmp/midi-mscz-refs
scripts/oracle/midi/lots.sh /tmp/midi-ws-final
nix-shell -p python3Packages.mido --run "python3 scripts/oracle/midi/mesure.py \
  --out /tmp/midi-ws-final --scoretk target/debug/scoretk \
  --ecriture ecriture_mscz=/tmp/midi-mscz-refs/manifest.tsv \
  --ecriture ecriture_ly=/tmp/midi-ws-final/manifestes/ecriture_ly.txt"
```

### Results

| Lot | Files | Exact baseline → current | N1 % | N2 % | N2d % | Avg size |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `ecriture_mscz` | 174 (173 read) | 41 (23.6 %) → **42 (24.1 %)** | 99.71 → **99.89** | 94.97 → **95.15** | 89.40 → **89.62** | 18 201 → 18 217 B |
| `ecriture_ly` | 1 020 | 706 (69.2 %) → 700 (68.6 %) | 96.32 → 96.32 | 93.14 → 93.14 | 92.13 → 92.04 | 1 806 → 1 805 B |
| `ecriture_mscz_corpus` | 21 | 17 → 17 (four empty references) | 100.00 | 100.00 | 100.00 | 248 B |

The `.mscz` lot gains **698 N2 matches and 819 N2d matches**, with 13 files
gaining attacks and **no file losing N1, N2 or N2d**. The `.ly` exact count
drops by 6 and N2d by 115 matches, all for one explained reason (the
acciaccatura divergence, below); N1 and N2 are unchanged on all 1 020 files.
The 21 corpus files are unchanged at 21/21 identical (`corpus_midi`).

Output size is measured against the reference of each lot. scoretk's MIDI is
smaller than both alternatives:

| Lot | scoretk total | Reference total | Ratio (total / median file) | Avg scoretk / avg reference |
| --- | ---: | ---: | ---: | ---: |
| `ecriture_mscz` (173) | 3 169 709 B | 5 122 359 B (MuseScore) | **0.619 / 0.636** | 18 322 / 29 609 B |
| `ecriture_ly` (1 020) | 1 841 594 B | 2 121 359 B (LilyPond) | **0.868 / 0.862** | 1 805 / 2 079 B |

The four fixes add no empty or padded events: the only size movement is the
trill events that MuseScore also writes (Under_Paris +142 notes, Hallelujah
+117, the nine Minuit chrétien files +43 each), while the 2.x anchoring fix
removes the 122 notes that the combined reader/writer change had produced.
Maximum conversion time is 6.64 s with the debug CLI (Vingt_danses); an
intermediate run on a loaded machine put four files over the 10 s bound, none
of which reproduces in isolation.

### Cause 1 — acciaccatura shortens the preceding chord

`CompatMidiRender::adjustTrailtime` (`engraving/compat/midi/compatmidirender.cpp:1035-1078`,
called from `createPlayEvents` at `:169`) adds
`min(first acciaccatura ticks, current ticks / 2)` to the current chord's
`trailtime` when the next chord carries an acciaccatura; `renderChord`
(`:263`) then applies `1000 - trailtime` to the note event. The writer had no
such look-ahead, so the note before an acciaccatura kept its full length.

Fix: `render.rs::acciaccatura_trail`, with the next chord of the track found
as `Navigation::nextChordRest` (skip grace, `skipGrace = true`) plus the
`nextMeasure->first()` fallback (`:126-136`). `tie_length` applies the same
adjustment to the tail of a tied chain. Reduced test
`tests/acciaccatura_trail.rs` (quarter, acciaccatura, quarters) expects
MuseScore 4.7.4's `72` at 0–239 instead of 0–479. Measured: **22 `.mscz`
files improve N2d (+81 matches), one becomes exact, no loss.**

On `ecriture_ly`, 37 files lose N2d matches (6 stop being exact, N2d −115):
LilyPond does not shorten the note before an acciaccatura, while MuseScore
4.7.4 does (verified on the reduced score exported by 4.7.4). The reference of
that lot is LilyPond's MIDI, so this is an expected divergence between the two
playback models, not a regression; the target remains MuseScore 4.7.4.

### Cause 2 — a trill fills the whole tied chain

`renderNoteArticulation` (`compatmidirender.cpp:660-810`) fills
`totalTiedNoteTicks(note)` (`:1196-1204`), so a trill on a tied note crosses
every measure of the tie. The port bounded the excursion to one chord
(≈24 events instead of 96 on four tied dotted halves).

Fix: `ornaments.rs::trill` now takes the tied span for the number of events
and the chord's own duration for the per-event conversion, exactly as
`makeEvent` (`ontime / chord->actualTicks()`) and `collectNote`
(`ticks * e.ontime() / 1000`) do; the step is reduced to `span / 4` when four
events do not fit, then refused below the minimum frequency, as upstream.
`render.rs::tied_span_ticks` walks `tie_forward`. Reduced tests:
`ornaments::tests::trille_sur_une_chaine_de_liaisons` (96 events at 0, 59,
119 … 5699) and the note length is now `off = on + len - 1` (58 ticks instead
of 59). Measured: **Under_Paris 89.26 → 100 % N2 (+142), Missa in Es
Benedictus +45, Gloria +6, Credo +1; no loss.**

### Cause 3 — trill spanners were not rendered

A `<Spanner type="Trill">` is looked up by
`CompatMidiRender::findFirstTrill` (`compatmidirender.cpp:1010-1030`) and
replaces the sustained note by the excursion
(`createPlayEvents`, `:480-482`; `createTrill`, `:913-927`). The model already
stores these spans (2.x/3.x/4.x readers); the MIDI writer ignored them.

Fix: `render.rs::trill_spans` (per staff; `play` honoured; the chord carrying
the end anchor is excluded, matching `findOverlapping(tick + 1, end-1)`;
`TRILL_LINE` and `PRALLPRALL_LINE`, the two types mapped to `ornamentTrill`,
are rendered). Reduced test `tests/trill_spanner.rs` (two half notes, trill
from the first to the second: 16 attacks then the second half note at 960).
Measured: **Hallelujah N2 4 204 → 4 321 (+117) and the nine Minuit chrétien
files +43 each.**

### Cause 4 — 2.x trill start is anchored at the current tick

`readMeasure206` (`src/engraving/rw/read206/read206.cpp:2818-2837` in v4.7.4)
sets a measure-level `<Trill id>` at `ctx.tick()`, i.e. after the chords
already read. `read206/transform.rs` anchored it on the measure start, so the
trill covered the preceding chords. It now uses the same `SpanKey::Chord`
anchoring as the ottava (current chord ordinal), and the test
`trille_2x_ancre_au_tick_courant` in `read206_corpus.rs` asserts start +1/2
and end +3/4 in a 4/4 measure. This is what removes the spurious trill notes
on Hallelujah: **4 443 → 4 321 produced notes (exactly the reference) and N2d
4 255 → 4 277**; no N2 change (the extra chords were already matched by the
harmonics rule, but they were not in the reference).

### Remaining causes (measured, not fixed)

The 28 files that still lose N2 concentrate almost all the remaining 18 333
lost attacks:

| File | Lost N2 | Cause already identified |
| --- | ---: | --- |
| `Vingt_danses_enfantines_pour_piano.mscz` | 16 114 | 2.x D.S. jump: one-quarter gap at tick 1507 (section-pause tempo) — see the pause-map sections above |
| `Vivaldi … 06-Dominus_fredipi.mscz` | 897 | trill auxiliary spelling + unmodelled fermata time stretch |
| `El_Choclo.mscz` | 775 | same D.S./fine jump gap |
| `Dieu_seul_suffit_-_Du_Jonchay.mscz` | 102 | chord-symbol voicing differs from 4.7.4's `RealizedHarmony` default style |
| `Magnificat__Prt.mscz` | 61 | attack offsets after a repeat/pause boundary |
| `Triumphal_March`, `Autumn_Leaves`, `Czardas`, … | ≤48 each | octave-class pitch choices (accordion registration / 2.x ottava ends) |

The `.ly` lot's dominant flags remain `notes_supp` (198 files), `duree_seule`
(113), `attaque_decalee` (81) and `octave` (51); most are LilyPond-vs-MuseScore
playback differences already documented in the earlier sections, and the
`notes_supp` class includes chord symbols that LilyPond does not realise.

### Follow-up campaign (2026-11-21): section and breath pauses, muted ornaments

Rebased on master (`7f68a884`), same oracle, same two lots
(`ecriture_mscz` = 173 readable `.mscz` against the MuseScore 4.7.4
references produced by `mscz-refs.sh`, `ecriture_ly` = 1 020 `.ly` against
LilyPond) and the same 10 s/2 GB bounds. The baseline of this campaign is the
table above (N2 95.15 %), not the older survey.

| Lot | Exact | N1 % | N2 % | N2d % | Max time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `ecriture_mscz` | 42 → **43 (24.9 %)** | 99.89 → 99.89 | 95.15 → **98.84** | 89.62 → **93.42** | 6.04 s |
| `ecriture_ly` | 700 → 700 | 96.32 | 93.14 | 92.04 | 0.45 s |
| `ecriture_mscz_corpus` | 17 → 17 | 100.00 | 100.00 | 100.00 | — |

Four files gain attacks, **none loses** one; `Magnificat__Prt` becomes exact.
Output size stays at **0.618** of MuseScore's (median per file 0.636). The
`.ly` lot is byte-for-byte unchanged in its measurements: none of these
constructs occurs there in a way that departs from LilyPond.

#### Cause 5 — the section pause belongs at the jump, not at the next raw measure

`PauseMap::calculate` (`engraving/compat/midi/pausemap.cpp:47-90`) receives a
tempomap pause list built by `Score::rebuildTempoAndTimeSigMaps`
(`engraving/dom/score.cpp:522-530`): a pause is stored at the **raw tick of
the measure that follows the break**, and each `RepeatSegment` maps it with
`s->tick + (s->utick - s->tick)`, scanning `[tick, endTick]` with the end
included (`ee = lower_bound(endTick + 1)`); a pause at the first tick of a
segment is skipped because the previous segment already emitted it.

The port placed the pause at the opening of the segment that *contains* the
marked measure. For `Vingt_danses` the D.S. measure (474) carries the break,
so the marked measure is 475; the jump from 474 lands on segno4 (measure 346),
and the segment containing 475 only starts much later. The reference inserts
the pause at the end of the jump-source segment instead (tick 722880, 480
ticks at 20 BPM, the 1/4 pickup's length), and the port was 240 measures late.

Port: `pause_map.rs::raw_pauses` (source ticks) + `unfold_pauses` (per
segment, end included, skip at the segment's first tick); the pause tick is
carried in `TempoPoint::pause_source` so `quarter_notes_at` reads the raw
measure. Reduced test `pause_du_saut_au_moment_du_saut` (six 4/4 measures, a
jump whose measure carries a section break: one 1920-tick pause at 80 BPM at
7680, as exported by MuseScore 4.7.4 for the reduced `jump.mscx`).

**Measured: `Vingt_danses` N2 10 363 → 22 558 (39.14 → 85.20 %), N2d 9 682 →
22 212; no loss.** The remaining 3 919 lost attacks in that file start after
quarter 2 193 and are trill/ornament contexts (32nd excursions in the
reference, a sustained note in ours), not pauses.

#### Cause 6 — the per-break and per-breath `<pause>` durations

`LayoutBreak::pause()` (`engraving/dom/layoutbreak.cpp:138-140`, property
`Pid::PAUSE`, default `Sid::sectionPause` = 3 s) is stored per break, and
`Breath::pause()` (`score.cpp:531-545`) per breath. Both were ignored: every
section break inserted a 3 s pause and every breath nothing.

* `Measure::section_pause: Option<f64>` (seconds; `None` = style default) and
  `Breath::pause_ms: Option<u32>` (milliseconds) are now read from `<pause>`
  by the 2.x/3.x (`read302`) and 4.x (`read4xx`) readers.
* `raw_pauses` skips a zero pause and merges breath pauses at their segment
  tick (longest breath wins, as `score.cpp` does); `PauseMap` inserts
  `quarterNotesPerMeasure` ticks at `quarterNotesPerMeasure / pause`.

Reduced tests: `saut_de_section_2x_garde_sa_pause` and
`respiration_2x_garde_sa_pause` (readers);
`pause_de_section_nulle_n_insere_rien` and
`respiration_avec_pause_insere_une_mesure_de_ticks` (`PauseMap`, 720 ticks at
180 BPM for a 0.5 s caesura before a 3/8 measure).

**Measured: `Dominus_fredipi` N2 616 → 1 513 (+897; the file writes
`<pause>0</pause>` on its only section break), `El_Choclo` 386 → 1 161
(+775; 0.5 s caesura before the 3/8 pickup), `Magnificat__Prt` 694 → 755 and
becomes exact. No loss.**

#### Cause 7 — a muted ornament still played

`Articulation::playArticulation()` is read by `renderArticulation`
(`compatmidirender.cpp:483-490`): an articulation with `<play>0</play>` stays
engraved but is silent. The port created an `Ornament` from the articulation
name and dropped the flag, so it rendered the trill.

`Articulation` and `Ornament` now carry `play`; the 3.x/4.x readers parse
`<play>` and `ornament_from_articulation` forwards it; the MIDI writer
requires `ornament.play` before replacing a note by the trill excursion.
Reduced test `trill_play` (a muted trilled half note then a normal half note:
two notes, the reference of the same reduced `.mscx` exported by 4.7.4).

**Measured: `Dominus_fredipi` produced 1 548 notes instead of 1 513; it now
matches the reference exactly and N2d goes from 1 503 to 1 508 (99.67 %).**

### Final status (2026-11-21, end of the MIDI-export campaign)

Same oracle, same manifests, same 10 s/2 GB bounds. The campaign started after
the previous merge at 43/173 exact, N1 99.89 %, N2 98.84 %, N2d 93.42 % on
`ecriture_mscz`, and 700/1 020 exact, N1 96.32 %, N2 93.14 %, N2d 92.04 % on
`ecriture_ly`.

| Lot | Files | Exact | N1 % | N2 % | N2d % | Avg size | Max time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `.mscz` vs MuseScore 4.7.4 MIDI | 173 read | **141 (81.5 %)** | **99.93** | **99.91** | **99.09** | 18 328 B | 6.86 s |
| `.ly` vs LilyPond's own MIDI | 1 020 | 661 (64.8 %) | 96.34 | 93.38 | 90.86 | 1 806 B | 0.95 s |
| corpus vs MuseScore `.ref.mid` | 21 | 17 (4 empty refs) | 100.00 | 100.00 | 100.00 | 248 B | — |

The `.mscz` lot goes from 43 to **141 exact files**, with **98 files gaining
and no file losing** an attack, a duration or an exact flag. N2 moves from
377 706 to 377 782 matched attacks (99.89 → 99.91 %) and N2d from 357 329 to
374 675 (94.50 → 99.09 %). Two files (`Vivaldi … 05 Juravit`, `… 07
Judicabit`) exceeded the 10 s bound under machine load during the batch run
but complete in isolation (118 s bound) at 100 % N1/N2/N2d; the batch
aggregate above includes them. `Canon_in_D` (MuseScore 1.14) remains the only
unsupported input.

**Size**: `.mscz` output **3 113 879 B** against MuseScore's **5 121 966 B**
(ratio **0.608**, median file 0.636); `.ly` output **1 841 696 B** against
LilyPond's **2 121 207 B** (ratio **0.868**, median 0.863). No padding.

#### Cause 11 — same-pitch overlaps on a channel: `fixupMIDI`

The task that opened this pass named trill durations (`Minuit chrétien` ×9,
639 N2d). Reducing the file note by note shows the **trills are already
identical** to the reference (same 32nd excursions, same lengths); the N2d
class was the organ's two staves playing the same pitch on one channel.
`EventsHolder::fixupMIDI` (`engraving/compat/midi/event.cpp:263-306`, called
`compatmidirenderinternal.cpp:1570`, applied at `exportmidi.cpp:304-323`)
counts playing notes by (internal channel, pitch): a new attack of a pitch
already sounding gets `discard = previous staff + 1`, the exporter writes an
immediate note-off at that tick on the previous track (restrike), and the
previous note's own note-off is dropped or hoisted to the track of the last
attack. `fixup.rs` ports this, with `EventKind::NoteOffRestrike` written after
the attacks of the same tick (the multimap insertion order) and a reduced
test `fixup_midi` (two staff organ, unison 55: 0-length note on the first
track, 960-tick note on the second).

The first version indexed by **output channel**; MuseScore indexes by
**internal channel of the part**, and two parts may share an output channel.
That over-merged the nine “Ô Notre Dame” extracts (71 durations each); the
writer now passes the part of each staff, so only unisons of two staves of the
same part (organ, piano) merge. Measured: `Minuit chrétien` N2d 4 602 → 5 227
(99.73 %), the nine extracts exact again, and **98 `.mscz` files become
exact** with no loss.

On `ecriture_ly`, 39 files lose their exact flag (N1/N2 unchanged, N2 **+0.24**
points, N2d −1.18): LilyPond keeps voices on separate tracks and does not
restrike a repeated pitch, while MuseScore merges unisons of one part. This is
the same MuseScore-vs-LilyPond playback convention documented for other
constructs; the writer follows MuseScore, the reference of the `.mscz` lot.

#### Cause 12 — unquoted `\language` (LilyPond reader)

`Lassus_Alleluja._Dies_sanctificatus.ly` writes `\language deutsch` without
quotes. LilyPond defines `\language` as a music function taking a string
(`ly/music-functions-init.ly:982-993`) but its parser accepts the bare word;
the reader only accepted `Kind::Str`, silently kept Dutch, so `h` was unknown,
`b` was B natural and every pitch of the motet was wrong. Both `\language`
handlers (document level and inside music) now accept `Kind::Word` too.
Reduced test `language_sans_guillemets` (`h'4 b'4` → 71, 70). Measured:
**N1 674 → 707 (100 %), N2 384 → 707 (100 %), N2d 341 → 669.**

#### Cause 13 — chromatic glissandos

`Autumn_Leaves` (38 lost attacks) and `Czardas` (36) are `Glissando` runs, not
accordion registers: the reference plays every chromatic step in the last
third of the chord. `renderGlissando` (`compatmidirender.cpp:646-658`) calls
`renderNoteArticulation` with `Glissando::pitchSteps`
(`engraving/dom/glissando.cpp:162-248`, default style `CHROMATIC`),
`requestedTicksPerNote = DIVISION`, `fastestFreq = 16`, `slowestFreq = 0` and
`glissandoPart = 0.33` (`:795-870`); the linear `EaseInOut::timeList`
(`dom/easeInOut.cpp:140-154`) places the run and the last note holds to the
end of the chord. New `glissando.rs`; `render.rs` replaces the starting note
(the highest note of the chord, the only one the model can identify without
the note link) by the run. Reduced test `glissando` (quarter 64 → 81:
65…80 at 1278…1426, 81 at 1920, values from MuseScore 4.7.4). Measured:
**Autumn_Leaves N1/N2 894 → 932 (100 %), Czardas +30 attacks, N2d +31.**
`Triumphal_March` has no glissando; its remaining 48 attacks are chord-symbol
voicing.

#### The three other `.ly` files named by the ranking

These are **not** reader defects; each has a separate playback `\score`:

* `LaMouetteGrise`: the first (engraved) score is one staff; the `\midi`
  score lays the same music in **three parallel voices** (`{ \voiceI \voiceII
  … }` in three braces) and repeats it. Our output follows the engraved score
  (444 notes); the paired `.midi` has 532.
* `gaudete`: the last score is `\unfoldRepeats` with `\repeat volta 4`
  around every voice and a different staff set; our pivot is the first
  (PianoStaff) score. 110 produced notes against 440.
* `UztaiDantza`: the `\midi` score is `\transpose c f` plus extra
  `\unfoldRepeats` material, the same class as the documented
  `playback_transposition` limit (354 against 392 notes).

`Veni, Creator Spiritus` (2 977) and `Laudate_pueri` (971) remain the two
**bad LilyPond references** documented in the previous status: our `.ly`
matches MuseScore's export of the sibling `.mscz` at 100 % and 99.94 % N2
respectively, while the paired `.midi` files are stale (3 134 notes vs 249;
organ tracks delayed by the whole vocal duration).

#### Remaining classes and their reason

`ecriture_mscz` — N2 loss 344 attacks in total:

| File / class | Lost N2 | Reason |
| --- | ---: | --- |
| `Dieu_seul_suffit_-_Du_Jonchay` | 102 | chord-symbol realisation (`<Harmony>` voicing differs from 4.7.4's `RealizedHarmony` default style) |
| `Triumphal_March` | 48 | same chord-symbol voicing; the file has 95 `<Harmony>` and no glissando |
| nine `Minuit chrétien` extracts | 14 each | residual trill auxiliary spelling |
| `Angels_Carol` | 19 | attack offsets around a repeat boundary |
| all others | ≤ 8 each | individual cases |

N2d loss 3 451 in total, concentrated in accordion files with `<Harmony>` or
register-dependent articulations: `Czardas` 684, `Marusia` 413, `ZORBA THE
GREEK` 349, `La Partida` 318, `El_Choclo` 251, `Vingt_danses` 249,
`Triumphal` 237, `Adios Muchachos` 186. Their attacks and pitches match
(N2 = 100 % except `Czardas` 99.72); only durations differ, so the next
writer cause is the chord-symbol/articulation duration, not the note grid.

`ecriture_ly` — N2 loss 9 025 in total; the top rows are the reference and
convention cases above (`Veni Creator` 2 977, `Ackermans` 1 071 — 3-pass volta
convention, `Laudate` 971, `MakilTxikiak` 565, `LaMouetteGrise` 434,
`gaudete` 330, `UztaiDantza` 267). The first real remaining reader cases are
`Ave_Verum…Saint-Saëns` (473, attack shifts), `MarieJeanneGabrielle` (228,
octave class) and `franck_ave_maria_D-a4` (116, attack shifts and extra
notes).

No mscz file regressed: 98 exact gains, 0 exact loss, 0 N1/N2/N2d loss; the 21
corpus files stay at 21/21 (`corpus_midi`), including the four with empty
references.
