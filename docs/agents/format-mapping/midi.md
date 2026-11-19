# MIDI (SMF 1.0, MIDI 1.0, General MIDI 1) ↔ scoretk model

References (outside the repository, never committed): `specs/README.md` § MIDI
and the local official texts: SMF 1.0 (= RP-001, chapter 4 of the AMEI Japanese
book), MIDI 1.0 Detailed Specification v4.2.1, GM1 (RP-003), RP-017/019A/026/032.
Section names below follow those specifications (header/track chunks, channel
voice messages, meta-events, running status, GM sound set); the concise syntax
reference with citations is `docs/agents/format-ref/midi.md` (EN).
Reader: `crates/scoretk-midi-import`; writer: `crates/scoretk-midi-export`.
`file:line` references below are from this worktree and were checked against
the code, not inferred from a report.

| Format property (spec section) | Model type / field (file:line) | Reader | Writer | Notes |
| --- | --- | --- | --- | --- |
| `MThd` header, format 0/1/2, `division` (SMF §Header Chunk / §Format 0,1,2) | `Score::division` (`crates/scoretk-core/src/model/score.rs:122`), `Score::measures` (`:128`) | partial (`import/src/midifile.rs:263-369`) | supported (`export/src/writer.rs:35-40`) | Input accepts the tracks `midly` decodes and merges channels; format 2 and SMPTE division are mishandled (see below). Output is SMF format 1, 480 ticks per quarter note. |
| `MTrk` track chunks, end-of-track `FF 2F` (SMF track chunk) | `Part` (`.../model/part.rs:200-221`), `Staff` (`:308-333`), `Track` (`.../model/ids.rs:123`) | supported (`import/src/lib.rs:90`) | supported (`export/src/render.rs:121`) | One output track per staff, with the part name. |
| Note On / Note Off, note number, velocity (MIDI 1.0 channel voice) | `Note::pitch`, `Note::velocity` (`.../model/note.rs:264,316`), `Chord` (`.../model/chord.rs:216-254`) | supported (`import/src/chord.rs:490`) | supported (`export/src/render.rs:672`) | Reader stores raw velocity in `Note::velocity`; writer derives attack velocity from dynamics/hairpins, then applies `user_velocity`. |
| Program Change; General MIDI 1 programs (GM §Sound Set, Table 2; MIDI §Program Change) | `Instrument::midi_program`, `track_name`, `long_name` (`.../model/part.rs:57-88`), `gm_names` (`import/src/instrument/gm_names.rs:162`) | partial (`import/src/instrument.rs:38`) | supported (`export/src/render.rs:315`) | GM numbers are 1–128 in the spec tables but 0–127 on the wire (`C0 00` = program 1); the reader maps the wire byte through its name table to "Grand Piano" rather than MuseScore's "Piano", with no short names. |
| Control Change 7 (volume) and 10 (pan) (MIDI 1.0 continuous controllers) | `Instrument::volume`, `pan` (`.../model/part.rs:82-84`) | supported (`import/src/midifile.rs`) | supported (`export/src/render.rs`) | Defaults 100 and 64; initial controllers are emitted per part. |
| Channel 10 / GM percussion key map (GM level 1 drum map) | `Staff::default_clef` (`.../model/part.rs:332`), `Instrument` (`part.rs:57-88`) | partial (`import/src/drum.rs:45-142`) | partial | Only kit pitches 35/36/44 are known and drum-track splitting is not exercised; no drumset table in the model. |
| Meta `FF 51` tempo (SMF meta-events) | `Tempo::beats_per_second`, `TempoKind` (`.../model/annotation.rs:322-364`) | supported (`import/src/tempo.rs`) | supported (`export/src/render.rs`) | Default 2 beats/s; section breaks add the MuseScore 3 s pause. |
| Meta `FF 58` time signature (SMF meta-events) | `TimeSig` (`.../model/segment.rs:166-207`), `Measure::nominal_len` (`.../model/measure.rs:61`) | supported (`import/src/timesigmap.rs`) | supported (`export/src/render.rs`) | Time changes become `TimeSig` segments and nominal bar lengths. |
| Meta `FF 59` key signature (SMF meta-events) | `KeySig`, `Key`, `KeyMode` (`.../model/segment.rs:43-129`) | supported (`import/src/key.rs:86-223`) | supported (`export/src/render.rs`) | Key changes written as meta-events after each repeat expansion. |
| Meta `FF 03` track name, `FF 02` copyright, title/composer (SMF §Meta-Events) | `Instrument::track_name` (`.../model/part.rs:67`), `ScoreMetadata` (`score.rs:50-87`) | partial (`import/src/lib.rs:496-535`) | partial (`export/src/render.rs:158`) | Track name, copyright, title, subtitle, composer, translator and poet are read; generic `FF 01` text is read as lyrics by `lib.rs:1052-1063`, not as generic text. Writer emits the part name and essential metas only. |
| Meta `FF 05` lyrics (SMF §Meta-Events; RP-017, RP-026) | `Lyrics`, `LyricsSyllabic` (`.../model/lyrics.rs:36-63`) | partial (`import/src/lyrics.rs:59-96`, `lib.rs:1038-1064`) | missing | Text is attached to notes, but RP-017 melismas/syllabic state and RP-026 commands/character sets are not decoded (see below). `docs/agents/midi.md` §"Écarts" lists lyric meta-events as not written by MuseScore's exporter either. |
| Meta `FF 06`/`FF 07` marker and cue point (SMF meta-events) | `Marker`, `Jump`, `RepeatMark` (`.../model/repeat.rs:42-257`) | missing | missing | Marker bytes are decoded by `midifile.rs:52,324` but are not mapped to the model by `lib.rs:481-539`. Repeats are expanded on output via `RepeatList` (`export/src/repeat_list.rs`), never written as markers. |
| Running status and variable-length quantities (SMF track chunk) | — (serialization detail) | supported (`import/src/midifile.rs`, via `midly`) | supported (`export/src/writer.rs`, via `midly`) | Not a musical property; no model field is needed. |
| Tuplet ratios inferred from grid (MIDI import heuristics) | `Tuplet` (`.../model/tuplet.rs:53-81`), `ChordRest::tuplet` (`chord.rs:334`) | partial (`import/src/tuplet.rs`) | supported (`export/src/render.rs`) | Only simple 3/5/7/9 tuplets on non-binary grids; nested/overlapping/duolets are not detected. |
| Quantization, voice separation, left/right-hand split (MIDI import pipeline) | `Track`, `Segment::elements` (`.../model/segment.rs:708-805`), `Chord::staff_move` (`chord.rs:234`) | partial (`import/src/quant.rs`, `import/src/voice.rs`, `import/src/lrhand.rs:56`) | supported | Human performances are quantized to eighth notes; dynamic-programming hand split is not ported. |
| Dynamics and hairpins (not present in SMF; only velocities) | `Dynamic` (`.../model/annotation.rs:197-225`), `SpannerType::Hairpin`, `Hairpin` (`.../model/hairpin.rs:41-58`) | missing (no such metadata in SMF) | supported (`export/src/velocity.rs:80-320`) | `VelocityMap` renders nuances, hairpins and articulation multipliers into attacks. |
| Repeats, voltas, D.C./D.S. jumps (not present in SMF) | `Measure::repeat_count` (`measure.rs:70`), `RepeatMark`, `VoltaInfo` (`repeat.rs:109-270`) | missing (not encoded in SMF) | supported (`export/src/repeat_list.rs:348`) | `RepeatList::unwind` produces the played measures; `docs/agents/midi.md` reports 21/21 reference files identical. |
| Grace notes and two-note tremolo (MIDI import/expansion) | `GraceType` (`.../model/chord.rs:87-104`), `Tremolo` (`.../model/notation.rs:187-208`) | missing (`docs/agents/midi.md` §"Limites mesurées") | supported (`export/src/render.rs`) | Grace notes are rendered before/after the main chord; unmeasured tremolo is not rendered. |
| Fermatas, breaths, pedal, instrument changes, rehearsal marks (not representable as standard MIDI events) | `Fermata` (`notation.rs:73-80`), `Breath` (`segment.rs:514-520`), `PedalInfo` (`spanner.rs:81-94`), `InstrumentChange` (`annotation.rs:400-421`), `RehearsalMark` (`:370-391`) | missing | missing | MuseScore's exporter has no playback rendering for these; fermatas and breaths deliberately do not lengthen playback. |
| Pitch bend, aftertouch, polyphonic pressure, system exclusive, CC modulation (MIDI 1.0 detailed spec) | — | missing | missing | No pivot field: nearest related types are `TremoloBar` (`.../model/tremolo_bar.rs:77-84`, guitar bend) and `Instrument::volume`/`pan`, neither of which models MIDI controllers. |
| Swing, chord names, per-note playback (`%%MIDI`-style playback data) | `SwingParameters` (`annotation.rs:272-277`), `Harmony` (`:428-471`) | missing (`docs/agents/midi.md`: `importmidi_swing`, `importmidi_chordname` not ported) | missing | No swing or chord-name events are produced. |

## Spec features the reader/writer mishandles (file:line)

Checked against the official texts in `specs/midi/`; the syntax/section key is
`docs/agents/format-ref/midi.md`. These are *SMF-level* defects or omissions,
distinct from the model gaps listed below.

| Spec feature (source) | Actual behaviour in this worktree | file:line |
| --- | --- | --- |
| SMPTE `division` (SMF §Header Chunk: negative frames/s in bits 8–14, ticks/frame in bits 0–7) | Frames × ticks/frame are collapsed into one integer "ticks per second" (`Fps29` becomes the float 29.97 for 30 drop-frame); `to_muse_score_ticks` then treats each such tick as 1/480 of a quarter note, bypassing the tempo map. SMPTE files get wrong note times and durations. | `crates/scoretk-midi-import/src/midifile.rs:266-277`, `:504-507`; used at `lib.rs:492`, `lib.rs:1060`, `tempo.rs:83` |
| Format 2 "sequentially independent single-track patterns" (SMF §Format 0,1,2) | `format` is parsed and stored but never consulted: patterns are merged into one score as if they were simultaneous parallel tracks. | `crates/scoretk-midi-import/src/midifile.rs:255`, `:368`; no branch in `lib.rs:156-172` |
| Running status cancelled by SysEx and meta events, persistent across delta times (SMF §Track Chunk; MIDI §Running Status) | Correct in practice because parsing/writing is delegated to `midly` (`Smf::parse`, `smf.write`); no project code implements it and no local test covers the meta/SysEx cancellation boundary. Listed here as a coverage gap, not a known misparse. | `crates/scoretk-midi-import/src/midifile.rs:264`; `crates/scoretk-midi-export/src/writer.rs:79-82` |
| RP-017 lyric conventions (word space, CR/LF alone, empty event = melisma, first event at tick 0) | Empty lyric meta text is skipped, so melismas are lost; no event carries syllable continuation/punctuation state, so every syllable is stored as `LyricsSyllabic::Single`. | `crates/scoretk-midi-import/src/lib.rs:1052-1063`; `crates/scoretk-midi-import/src/lyrics.rs:92-94` |
| RP-026 Lyric/Display extensions (`\r`/`\n` commands, `{@JP}` Shift-JIS, `[…]` Ruby, `{#…}` tags) | No decoding: `{@JP}`, `[…]`, `{#TITLE=}` are kept literally in lyric text; only `/` and `\` are stripped; `String::from_utf8_lossy` turns Shift-JIS bytes into mojibake. | `crates/scoretk-midi-import/src/lib.rs:1052-1063`; `crates/scoretk-midi-import/src/lyrics.rs:43-52`; `crates/scoretk-midi-import/src/midifile.rs:166-171` |
| Meta `FF 59` minor flag `mi` (SMF §Meta-Events: 0 = major, 1 = minor) | Reader keeps `mi` in `data[1]` but only uses `data[0]`; writer always emits `false` (major). Minor keys are imported and re-exported as major. | `crates/scoretk-midi-import/src/midifile.rs:345-347`; `crates/scoretk-midi-import/src/lib.rs:503-519`; `crates/scoretk-midi-export/src/writer.rs:109-111` |
| Meta events `FF 00` sequence number, `FF 20` channel prefix, `FF 54` SMPTE offset, `FF 7F` sequencer-specific; RP-019A `FF 08`/`FF 09`, RP-032 `FF 60`; SysEx `F0`/`F7` | Silently dropped by the catch-all match arms; no pivot field exists for them. The writer emits none of them either. | `crates/scoretk-midi-import/src/midifile.rs:348`, `:352` |
| Channel voice messages other than note on/off, program and CC (MIDI §Channel Voice: aftertouch, channel pressure, pitch bend) | Dropped by the catch-all arm even though `midly` decodes them. | `crates/scoretk-midi-import/src/midifile.rs:309` |

## Model gaps

Ranked by musical importance:

1. **MIDI controller and performance data**: pitch bend, aftertouch,
   polyphonic pressure, modulation and arbitrary CC/automation have no model
   field; `TremoloBar` is a guitar notation object, not a MIDI bend stream.
2. **Percussion**: there is no drumset/key-map structure, so GM channel 10
   tracks cannot be represented faithfully (only pitches 35/36/44 are known).
3. **Playback of expressive marks**: fermata/breath pauses (`PauseMap` covers
   section breaks only), `singleNoteDynamics` CC2/CC11, and `<veloChange>` on
   hairpins are not stored in the pivot.
4. **Lyrics and marker meta-events on output**, plus swing and chord names:
   the model has `Lyrics`, `Marker`/`Jump`, `SwingParameters` and `Harmony`, but
   the MIDI writer does not emit their meta-events. On input, RP-017/RP-026
   lyric conventions are not interpreted (see the spec-gaps table above).
5. **Instrument identity**: the model has program/channel/volume/pan but no
   link back to the MuseScore instrument catalogue; imported names differ from
   catalogue names and short names are not produced.
6. **Detection of nested/overlapping tuplets** and a full human-performance
   beat/tempo estimate: the reader only sees simple metric tuplets and
   explicit tempo events.
