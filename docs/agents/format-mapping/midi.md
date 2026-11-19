# MIDI (SMF 1.0, MIDI 1.0, General MIDI 1) ↔ scoretk model

References (outside the repository): `specs/README.md`, section MIDI. The
normative SMF 1.0 and MIDI 1.0 Detailed Specification PDFs are hosted by the
MIDI Association behind a login, so only their landing pages were copied; the
working summaries are the openly licensed Wikipedia articles on the Standard
MIDI File layout and General MIDI 1 program/percussion tables. Section names
below follow the SMF structure (header/track chunks, channel voice messages,
meta-events, running status) and the General MIDI 1 sound set.
Reader: `crates/scoretk-midi-import`; writer: `crates/scoretk-midi-export`.

| Format property (spec section) | Model type / field (file:line) | Reader | Writer | Notes |
| --- | --- | --- | --- | --- |
| `MThd` header, format 0/1, `division` (SMF header chunk) | `Score::division` (`crates/scoretk-core/src/model/score.rs:122`), `Score::measures` (`:128`) | supported (`import/src/midifile.rs`) | supported (`export/src/writer.rs:22`) | Input accepts the tracks `midly` decodes and merges channels; output is SMF format 1, 480 ticks per quarter note. |
| `MTrk` track chunks, end-of-track `FF 2F` (SMF track chunk) | `Part` (`.../model/part.rs:200-221`), `Staff` (`:308-333`), `Track` (`.../model/ids.rs:123`) | supported (`import/src/lib.rs:90`) | supported (`export/src/render.rs:121`) | One output track per staff, with the part name. |
| Note On / Note Off, note number, velocity (MIDI 1.0 channel voice) | `Note::pitch`, `Note::velocity` (`.../model/note.rs:264,316`), `Chord` (`.../model/chord.rs:216-254`) | supported (`import/src/chord.rs:490`) | supported (`export/src/render.rs:672`) | Reader stores raw velocity in `Note::velocity`; writer derives attack velocity from dynamics/hairpins, then applies `user_velocity`. |
| Program Change; General MIDI 1 program names (GM level 1 sound set) | `Instrument::midi_program`, `track_name`, `long_name` (`.../model/part.rs:57-88`), `gm_names` (`import/src/instrument/gm_names.rs:162`) | partial (`import/src/instrument.rs:38`) | supported (`export/src/render.rs`) | Reader uses the GM name table, not MuseScore's instrument catalogue: "Grand Piano" instead of "Piano", no short names. |
| Control Change 7 (volume) and 10 (pan) (MIDI 1.0 continuous controllers) | `Instrument::volume`, `pan` (`.../model/part.rs:82-84`) | supported (`import/src/midifile.rs`) | supported (`export/src/render.rs`) | Defaults 100 and 64; initial controllers are emitted per part. |
| Channel 10 / GM percussion key map (GM level 1 drum map) | `Staff::default_clef` (`.../model/part.rs:332`), `Instrument` (`part.rs:57-88`) | partial (`import/src/drum.rs:45-142`) | partial | Only kit pitches 35/36/44 are known and drum-track splitting is not exercised; no drumset table in the model. |
| Meta `FF 51` tempo (SMF meta-events) | `Tempo::beats_per_second`, `TempoKind` (`.../model/annotation.rs:322-364`) | supported (`import/src/tempo.rs`) | supported (`export/src/render.rs`) | Default 2 beats/s; section breaks add the MuseScore 3 s pause. |
| Meta `FF 58` time signature (SMF meta-events) | `TimeSig` (`.../model/segment.rs:166-207`), `Measure::nominal_len` (`.../model/measure.rs:61`) | supported (`import/src/timesigmap.rs`) | supported (`export/src/render.rs`) | Time changes become `TimeSig` segments and nominal bar lengths. |
| Meta `FF 59` key signature (SMF meta-events) | `KeySig`, `Key`, `KeyMode` (`.../model/segment.rs:43-129`) | supported (`import/src/key.rs:86-223`) | supported (`export/src/render.rs`) | Key changes written as meta-events after each repeat expansion. |
| Meta `FF 03` track name, `FF 02` copyright, title/composer (SMF meta-events) | `Instrument::track_name` (`.../model/part.rs:67`), `ScoreMetadata` (`score.rs:50-87`) | partial (`import/src/lib.rs:497-531`) | partial (`export/src/render.rs`) | Track name, copyright, title, subtitle, composer, translator and poet are read; generic `FF 01` text is not. Writer emits the part name and essential metas only. |
| Meta `FF 05` lyrics (SMF meta-events) | `Lyrics`, `LyricsSyllabic` (`.../model/lyrics.rs:36-63`) | supported (`import/src/lyrics.rs`, `lib.rs:1039`) | missing | `docs/agents/midi.md` §"Écarts" lists lyric meta-events as not written by MuseScore's exporter either. |
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
   the MIDI writer does not emit their meta-events.
5. **Instrument identity**: the model has program/channel/volume/pan but no
   link back to the MuseScore instrument catalogue; imported names differ from
   catalogue names and short names are not produced.
6. **Detection of nested/overlapping tuplets** and a full human-performance
   beat/tempo estimate: the reader only sees simple metric tuplets and
   explicit tempo events.
