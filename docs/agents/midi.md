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
- **Fermata and breath pauses**: none is applied, which is
  faithful. `engraving/compat/midi/` mentions no fermata (only
  `CTRL_BREATH` appears, `compatmidirender.cpp:55`), and
  `Breath::BREATH_LIST` (`breath.cpp:33-44`) is used only for `isCaesura()`
  (`breath.cpp:64-74`): caesura seconds live in the `PausesMap`
  (`tempotimeline.h:33`) filled by the *playback* module, absent from
  `ref-musescore/`. `PauseMap` therefore handles only section jumps (default
  pause of 3 s). Kept by
  `pause_map::tests::points_d_orgue_et_respirations_n_allongent_pas_le_rendu`,
  to be reconfirmed on 4.7.4 if the *playback* module becomes available.
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
- **Voices**: allocation by overlap then sorting by average pitch; the
  splitting of chords and n-tuplets between `MidiVoice` voices is not ported.
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

Baseline on 98 upstream files: **N = 0.3637**; the largest losses are
`human_tempo` (142 notes, score 0.014), `chord_collect` (48, 0.0), the
tuplet tests and the percussion tests. Many upstream tests set non-default
import options (`dontSimplify`, `maxVoiceCount V_1`), so their expected `.mscx`
cannot be matched by the CLI's default options; those files are documented,
not chased as importer bugs.

### Cause fixed: SMPTE ticks

`toMuseScoreTicks` (`importexport/midi/internal/midiimport/importmidi_fraction.cpp:350-354`)
returns `ReducedFraction::fromTicks(tick)` unchanged when the SMF header uses a
time division. The Rust port converted SMPTE ticks to quarters at a fixed
120 BPM, distorting every onset and duration. Reduced test `smpte_ticks`
(written first, failed before the fix): `to_muse_score_ticks(1000, 80, true)`
now gives the raw 1000 ticks. Measured gain on `up-division` (an SMPTE file):
score 0.0000 → 0.2500; on the full upstream lot N goes from 0.3637 to 0.3648.
