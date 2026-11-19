# MIDI / Standard MIDI File — condensed reference for coding agents

English reference for the scoretk MIDI readers/writers. All statements below
are checked against the local normative texts listed in `specs/README.md` §
MIDI; short quotations only, everything else is paraphrase. Companion:
[MIDI ↔ pivot model](../format-mapping/midi.md). Reader
`crates/scoretk-midi-import`, writer `crates/scoretk-midi-export`; internal
porting notes `docs/agents/midi.md`.

## Source key (cited inline as `[…]`)

- `[SMF]` — Standard MIDI Files 1.0 (= Recommended Practice RP-001), reproduced
  in the AMEI book, chapter 4, §"Sequences, Tracks, Chunks" / "Header Chunk" /
  "Format 0, 1, 2" / "Track Chunk" / "Meta-Events" (pp. 4-1…4-11, English
  equivalents in the Complete MIDI 1.0 Detailed Specification).
- `[MIDI]` — MIDI 1.0 Detailed Specification, doc. v4.2.1 (96-1-4, 1996), §Data
  Format / Data Types / Running Status / Channel Voice Messages and Tables I–II.
  This document is **protocol only**; it contains no SMF specification.
- `[GM]` — General MIDI System Level 1, RP-003 / MMA0007, §Performance
  Requirements and Sound Set (Tables 1–3), §Additional Messages.
- `[RP-017]` — SMF Lyric Meta Event Definition (1997).
- `[RP-019A]` — SMF Device Name and Program Name Meta Event (1998/99).
- `[RP-026]` — SMF Language and Display Extensions (1999).
- `[RP-032]` — SMF Meta Event for XMF Patch Type Prefix (2000/01).

## Container layout

- An SMF is a stream of bytes made of chunks: `MThd` + one or more `MTrk`.
  Each chunk is a 4-byte ASCII type + a 32-bit **big-endian** byte length +
  payload; the length excludes those 8 bytes. A reader must skip unknown chunk
  types rather than fail. `[SMF §Chunks]`
- `MThd` length is **6** (ignore extra bytes but honour the declared length):
  three 16-bit words, `format`, `ntrks`, `division`. `[SMF §Header Chunk]`
  - `format` 0 = exactly one multi-channel track; 1 = one or more **simultaneous**
    tracks (or MIDI outputs) of a single sequence; 2 = one or more **sequentially
    independent** single-track patterns. `ntrks` is always 1 for format 0.
  - `division`: if bit 15 = 0, bits 0–14 are ticks per quarter note (metrical).
    If bit 15 = 1, time is SMPTE-based: bits 8–14 hold the frames/second as a
    negative two's-complement value −24, −25, −29 (30 drop-frame) or −30, and
    bits 0–7 hold ticks per frame (typically 4, 8, 10, 80 or 100; 25 fps × 40
    gives millisecond resolution). Example: `E250` = 30 fps, bit resolution.
  - Defaults when absent: **4/4 and 120 BPM**; tempo (and time signature)
    meta-events belong in the first `MTrk` of a format 1 file. `[SMF §Format 0,1,2]`
- `MTrk` payload = `<delta-time><event>` repeated (at least one event).
  There are no absolute timestamps and no bar/measure/staff/voice fields.
  `[SMF §Track Chunk]`

## Events and time

- Delta time is a **variable-length quantity (VLQ)**: 7 data bits per byte,
  MSB set on all but the last byte, big-endian; 480 = `83 60`. It is always
  present, even when zero, and is in the units given by `division`. The maximum
  representable value is `0x0FFFFFFF` (268 435 455; `FF FF FF 7F`).
  `[SMF §Conventions; §Track Chunk]`
- Running status: for channel voice/mode messages only, a repeated status byte
  may be omitted; it **persists across delta times**, but each `MTrk` must
  start with an explicit status. System-exclusive **and meta events cancel any
  running status in effect**; real-time bytes do not affect it. `[SMF §Track
  Chunk; MIDI §Running Status]`
- Channel status nibbles: `0x8` note off, `0x9` note on, `0xA` polyphonic key
  pressure, `0xB` control change, `0xC` program change (one data byte), `0xD`
  channel pressure, `0xE` pitch bend; low nibble = channel 0–15. Data bytes are
  0x00–0x7F; note-on velocity 0 is a legal note-off. `[MIDI §Channel Voice;
  Table II]`
- Note number 60 (`3C`) is middle C on melodic channels; 69 = A4 = 440 Hz.
  `[MIDI §Note Number; GM §Performance Requirements]`
- SysEx events: `F0 <len> <data…>` (length excludes `F0` and the length itself;
  the last data byte must be `F7`) and `F7 <len> <data…>` (escape/continuation
  of a multi-packet SysEx). No transmitable MIDI event may occur between
  packets of one multi-packet SysEx. `[SMF §Track Chunk]`
- Meta events: `FF <type> <varlen length> <data>`. Unknown types must be
  skipped, their declared length must always be honoured (even if longer than
  expected), and no private data may be appended to a known meta event.
  `[SMF §Meta-Events]`

### Meta-event types (type, fixed/variable length, meaning)

| Bytes | Meaning | Source |
| --- | --- | --- |
| `FF 00 02 ssss` | Sequence number; at time 0 before any nonzero delta / transmitable event | `[SMF]` |
| `FF 01 len text` | Text event (types `01`–`0F` reserved for text) | `[SMF]` |
| `FF 02 len text` | Copyright notice; first event at time 0 of track 1 | `[SMF]` |
| `FF 03 len text` | Sequence name (format 0 / first track) or track name | `[SMF]` |
| `FF 04 len text` | Instrument name | `[SMF]` |
| `FF 05 len text` | Lyric: "each syllable will be a separate lyric event" | `[SMF; RP-017]` |
| `FF 06 len text` | Marker (rehearsal mark / section name) | `[SMF]` |
| `FF 07 len text` | Cue point (film/stage description) | `[SMF]` |
| `FF 08 len text`, `FF 09 len text` | Program name, device name (>16 channels / reorchestration) | `[RP-019A]` |
| `FF 20 01 cc` | MIDI channel prefix (channel 0–15 for following meta/SysEx) | `[SMF]` |
| `FF 2F 00` | End of track — **mandatory**, last event of every track | `[SMF]` |
| `FF 51 03 tttttt` | Tempo: microseconds per MIDI quarter note (24 MIDI clocks); 500000 = 120 BPM | `[SMF]` |
| `FF 54 05 hr mn se fr ff` | SMPTE offset (`hr` encodes the SMPTE format; `ff` in 1/100 frame) | `[SMF]` |
| `FF 58 04 nn dd cc bb` | Time signature: `dd` = exponent of 2 (2 = quarter, 3 = eighth); `cc` = MIDI clocks per metronome click (24 per quarter); `bb` = 32nds per MIDI quarter (normally 8). 6/8 = `06 03 18 08` | `[SMF]` |
| `FF 59 02 sf mi` | Key signature: `sf` −7 (7 flats)…0 (C)…+7 (7 sharps); `mi` 0 = major, 1 = minor | `[SMF]` |
| `FF 60 01 pp` | XMF patch type prefix (`01` GM1 default, `02` GM2, `03` DLS); only valid as the first event of a track | `[RP-032]` |
| `FF 7F len data` | Sequencer-specific (starts with manufacturer ID) | `[SMF]` |

Concurrent meta-events at the same tick may appear in any order; sequence
number and sequence/track name must be at time 0, and end-of-track last.
`[SMF §Meta-Events]`

## Instruments, parts, repeats

- GM1: **128 melodic programs, numbered 1–128 in the published tables but
  0–127 on the wire** (`C0 00` = program 1, Acoustic Grand Piano). All
  channels except 10 are melodic. `[GM §Sound Set, Table 2]`
- Percussion: key-based percussion is **always on channel 10** (wire channel
  9); 47 preset drum sounds at keys **35–81**; key 60 there is Hi Bongo, not
  middle C. `[GM §Requirements, Table 3]`
- GM1 controllers: 1 modulation, 7 volume, 10 pan, 11 expression, 64 sustain,
  121 reset all controllers, 123 all notes off. Registered parameters 0 pitch
  bend sensitivity, 1 fine tuning, 2 coarse tuning. Defaults: volume 100, bend
  0, pitch bend range ±2 semitones. (Controllers 66/67 sostenuto/soft often
  listed elsewhere are **not** in the RP-003 GM1 table.) `[GM §Requirements]`
- GM mode on/off SysEx: `F0 7E <dev> 09 01 F7` / `F0 7E <dev> 09 02 F7`.
  `[GM §Additional Messages]`
- Format 1 tracks are parallel event streams; SMF has no part/staff/voice
  fields, so the writer must assign them to tracks/channels. Format 2 tracks
  are independent patterns, not simultaneous parts. `[SMF §Format 0,1,2]`
- SMF has no repeats, voltas, tuplets, ties or slurs: repeats must be expanded,
  ties become one longer note, tuplets become tick arithmetic, grace notes
  become short notes at shifted ticks. Lyrics survive only as meta text.
  `[docs/agents/midi.md]`

## Lyrics: RP-017 and RP-026

- RP-001 defines `FF 05` only loosely; **RP-017** fixes the interchange rules:
  one lyric meta-event per syllable, beginning at the event time; a trailing
  ASCII space (0x20) terminates a word (no space ⇒ next event continues the
  word); punctuation goes at the end of a syllable before that space; a
  carriage return (0x0D) alone ends a line and a line feed (0x0A) alone ends a
  paragraph; an **empty** lyric event is a melisma (hold the previous
  syllable). A first lyric event at tick 0 is recommended as an "on" flag.
- **RP-026** renames them *Lyric/Display Meta Events* and reserves five ASCII
  characters: `\` command codes (`\r`, `\n`, `\t`, and `\\`, `\{`, `\}`, `\[`,
  `\]` escapes), `[`…`]` Ruby (furigana) parts, `{@LATIN}` / `{@JP}` character
  code sets (ANSI default; a `FF FE` / `FE FF` BOM implies Unicode), and
  `{#TITLE=}` / `{#COMPOSER=}` / `{#LYRICS=}` / `{#ARTIST=}` / `{#}` song
  information tags. It does **not** supplant the other text metas (`FF 01`,
  `02`, `03`, `04`, `06`, `07`).

## Traps that silently break data

1. VLQ delta times and running status are the classic reader bugs: treating
   delta as absolute, or keeping running status after a meta/SysEx event,
   shifts/corrupts every later note. `[SMF]`
2. Note-on velocity 0 is note-off; ignoring it leaves hanging notes. `[MIDI]`
3. Missing `FF 2F 00` lets a reader run into the next chunk; meta lengths and
   chunk lengths must be trusted, not guessed. `[SMF]`
4. Confusing `division` (a resolution, possibly SMPTE) with tempo (a meta
   event); SMPTE files need a frame-rate-aware time map. `[SMF]`
5. Program numbers are 0-based on the wire but 1-based in GM tables; channel 10
   key numbers are drum IDs. `[GM]`
6. Rounding ticks between different PPQs accumulates drift; use a rational
   time map. Format 0 merges all channels into one track; format 1 track order
   is not a semantic part order; format 2 tracks are not simultaneous.
7. RP-026 markup is *data*, not plain text: a reader that does not decode it
   stores `\r`, `{@JP}`, `[…]` literally in lyrics; Shift-JIS text read as UTF-8
   becomes mojibake. `[RP-017; RP-026]`

## Tiny valid example

```text
4D 54 68 64  00 00 00 06  00 01  00 01  01 E0   MThd: format 1, 1 track, 480 tpqn
4D 54 72 6B  00 00 00 1F                         MTrk: 31 bytes follow
00 FF 51 03 07 A1 20                             delta 0: tempo 500000 us/qn = 120 bpm
00 FF 58 04 04 02 18 08                          delta 0: 4/4, 24 clocks/click, 8 32nds/qn
00 C0 00                                         delta 0: GM program 1 (Acoustic Grand Piano)
00 90 3C 40                                      delta 0: note on C4 (60), vel 64
83 60 80 3C 40                                   delta 480 (VLQ 83 60): note off C4
00 FF 2F 00                                      delta 0: end of track
```
`83 60` = 480 ticks = one quarter note at `division = 480`; the `MTrk` length
`0x1F` = 31 bytes, counted from the first `00 FF 51` through `00 FF 2F 00`.
