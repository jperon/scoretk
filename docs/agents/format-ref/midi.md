# MIDI / Standard MIDI File — condensed reference for coding agents

Targets: **SMF 1.0** container, **MIDI 1.0** channel messages, **General MIDI 1**
instrument map. Local sources: `specs/README.md` § MIDI;
`specs/midi/wikipedia-standard-midi-file.html` (`WP-SMF` section names),
`specs/midi/wikipedia-general-midi.html` (`WP-GM` section names). Licence:
CC BY-SA 4.0 with attribution/share-alike; this is an original summary, not
copied text. The normative MMA PDFs are login-gated and were **not** downloaded:
section references point to the openly licensed summaries and to internal docs.
Companion: [MIDI ↔ pivot model](../format-mapping/midi.md).
Reader `crates/scoretk-midi-import`, writer `crates/scoretk-midi-export`;
internal detail `docs/agents/midi.md`.

## Container layout

- A file is a concatenation of chunks: `MThd` + `MTrk`…; each chunk =
  4-byte ASCII type + 4-byte big-endian length + payload. (WP-SMF →
  Computer files / Standard MIDI File)
- `MThd` payload = 6 bytes: `format` (0 = one merged track, 1 = parallel
  tracks, typically one per part, 2 = independent songs), `ntrks`, `division`.
  (WP-SMF → Computer files / Standard MIDI File)
- `MTrk` payload = a stream of `<delta-time><event>`; no absolute timestamps,
  no bar/measure/staff/voice fields exist in SMF. (WP-SMF → Computer files)
- `division` is either ticks per quarter note (0x0001–0x7FFF) or, if the high
  byte is negative, SMPTE: `-24`/`-25`/`-29`/`-30` frames/s in the high byte
  and ticks/frame in the low byte. All tick math depends on this field.
  (WP-SMF → Computer files / Technical specifications)

## Event stream and time

- Delta time is a **variable-length quantity**: 7 data bits per byte, most
  significant bit set on every byte except the last, big-endian; 480 = `83 60`.
  (WP-SMF → Messages; SMF 1.0 varlen encoding)
- Running status: a channel message whose status byte equals the previous
  channel status may omit it. A parser must remember the last status across
  delta times but reset it at meta/sysex events. (WP-SMF → Messages)
- Status byte high nibble: `0x8` note off, `0x9` note on, `0xA` aftertouch,
  `0xB` control change, `0xC` program change (1 data byte), `0xD` channel
  pressure, `0xE` pitch bend; low nibble = channel 0–15 (channel **9** = GM
  channel 10, percussion). (WP-SMF → Messages, Instrument control; WP-GM →
  Percussion)
- Note on `9n kk vv`: key 0–127 (60 = C4/middle C, 69 = A4 = 440 Hz), velocity
  1–127; velocity 0 is a legal note-off (a very common writer optimisation).
  Note off `8n kk vv` (release velocity usually 0). (WP-SMF → Messages)
- Meta events `FF <type> <varlen length> <data>`: `FF 00` sequence number,
  `FF 01`–`FF 0F` text (incl. `FF 03` track name, `FF 05` lyrics, `FF 06`
  marker, `FF 07` cue), `FF 2F 00` end of track (mandatory), `FF 51 03`
  microseconds per quarter note (500000 = 120 BPM), `FF 58 04` time signature
  `nn dd cc bb` (dd = 2^denominator, cc = clocks/click, bb = 32nds/quarter),
  `FF 59 02 sf mi` key. System exclusive `F0 … F7`. (WP-SMF → Messages,
  Technical specifications)
- Tempo lives in a **meta event** (usually track 0), not in the header; an
  absent tempo means 500000 µs/qn. (WP-SMF → Computer files / Messages)

## Instruments, parts, repeats

- GM1: 128 melodic programs, program numbers 1–128 in user-facing notation
  but 0–127 on the wire (`0xC0 0x00` = piano); channel 10 is reserved for 47
  percussion sounds at keys 35–81 (key 60 = high bongo, not middle C).
  (WP-GM → General MIDI 1 requirements, Program change events, Percussion)
- GM controllers: 1 modulation, 7 volume, 10 pan, 11 expression, 64 sustain,
  66 sostenuto, 67 soft, 121 reset, 123 all notes off; pitch bend and fine/
  coarse tuning use RPN via CC 100/101 + 6/38. (WP-GM → Controller events, RPN)
- Format 1 tracks are independent event streams played in parallel; score
  parts/staves/voices must be assigned to tracks or channels by the writer —
  SMF itself has no such fields. (WP-SMF → Computer files / Standard MIDI File)
- SMF has **no repeats, volta, tuplets, ties or slurs**: repeats must be
  expanded into events before writing; a tie becomes one longer note (or
  re-articulation at the same pitch), tuplets become tick arithmetic, grace
  notes become short notes at shifted ticks, slurs are invisible except via
  CC/text. Lyrics survive only as `FF 05` text. (`docs/agents/midi.md`)

## Traps that silently break data

1. Varlen delta times and running status are the two classic reader bugs;
   treating delta as absolute or forgetting to reset running status after a
   meta event shifts/corrupts all following notes. (WP-SMF → Messages)
2. A note-on without a matching note-off (or a velocity-0 note-on not treated
   as note-off) leaves hanging notes. (WP-SMF → Messages)
3. Missing `FF 2F 00` end-of-track: readers may read past the track boundary
   into the next chunk. (WP-SMF → Messages)
4. Confusing ticks-per-quarter with BPM; `division` is a resolution, tempo is
   a meta event. (WP-SMF → Computer files)
5. Program numbers are 0-based on the wire; GM channel 10 (wire 9) is drums,
   so key numbers there are instrument IDs, not pitches. (WP-GM → Percussion)
6. Rounding ticks when converting to/from a model with a different PPQ
   silently accumulates drift; use a rational time map.
7. Assuming one track per staff: format 0 merges everything, and format 1
   track order is not a semantic part order.
8. Instrument changes are `0xCn` events at a tick, not score-level metadata.

## Tiny valid example

```text
4D 54 68 64  00 00 00 06  00 01  00 01  01 E0   MThd: format 1, 1 track, 480 tpqn
4D 54 72 6B  00 00 00 1F                         MTrk: 31 bytes follow
00 FF 51 03 07 A1 20                             delta 0: tempo 500000 us/qn = 120 bpm
00 FF 58 04 04 02 18 08                          delta 0: 4/4, 24 clocks/click, 8 32nds/qn
00 C0 00                                         delta 0: program 1 (Acoustic Grand Piano)
00 90 3C 40                                      delta 0: note on C4 (60), vel 64
83 60 80 3C 40                                   delta 480 (varlen 83 60): note off C4
00 FF 2F 00                                      delta 0: end of track
```
`83 60` = 480 ticks = one quarter note at `division = 480`; the `MTrk` length
`0x1F` = 31 bytes, counted from the first `00 FF 51` through `00 FF 2F 00`.
