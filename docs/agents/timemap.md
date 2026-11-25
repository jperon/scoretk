# Time map (`timemap`)

The time map is a **by-product of MIDI rendering**: it associates each
performance tick of the produced file with its time in microseconds and
lists the events useful to a score reader (measures played,
note onsets, tempo changes, section pauses). It is defined
in `crates/scoretk-midi-export/src/timemap.rs` and filled by `render.rs` at the same
places as the MIDI events: its ticks are therefore exactly those of the SMF,
pauses included. Byte-for-byte invariance is checked by
`crates/scoretk-midi-export/tests/timemap_alignment.rs`.

## 1. API

```rust
use scoretk_midi_export::{write_midi_with_timemap, write_midi_with_timemap_and, MidiOptions};

let (midi, timemap) = write_midi_with_timemap(&score)?;      // repeats unfolded
let (midi, timemap) = write_midi_with_timemap_and(&score, MidiOptions { expand_repeats: false })?;
```

The `scoretk` facade exposes the full version:

```rust
use scoretk::{write_playback, Options};

let playback = write_playback(&score, &Options::default())?;
// playback.musicxml : Vec<u8> (id + xml:id forced)
// playback.midi     : Vec<u8> (identical to ordinary MIDI)
// playback.timemap  : scoretk::Timemap
```

`write_playback` forces `stable_ids` for the MusicXML: the `xmlId`s of the map
exist as-is in the document. Ordinary MusicXML export stays without
identifiers (`Options::default()`), to remain identical to the
MuseScore 4.7.4 references.

## 2. Rust structure

```rust
pub struct Timemap {
    pub division: u32,          // 480 ticks per quarter note
    pub ticks: i64,             // total duration, pauses included
    pub tempo: Vec<TempoChange>,
    pub measures: Vec<MeasureSync>,
    pub notes: Vec<NoteSync>,
    pub pauses: Vec<PauseSync>,
}

pub struct TempoChange { pub tick, pub time_us, pub micros_per_quarter }
pub struct MeasureSync {
    pub tick, pub end_tick, pub time_us, pub end_time_us,
    pub measure, pub number, pub segment, pub pass, pub pause_before,
}
pub struct NoteSync {
    pub tick, pub time_us, pub duration_ticks,
    pub measure, pub staff, pub voice, pub chord, pub note, pub pitch,
    pub pass, pub grace, pub grace_index, pub grace_before, pub tremolo,
}
pub struct PauseSync { pub tick, pub end_tick, pub time_us, pub end_time_us, pub seconds }
```

All ticks are performance ticks (after repeat unfolding and
offset by pauses). `MeasureSync::end_tick` is exclusive; a tick at the end of a
measure belongs to the next measure (or to a pause).

The bounds of each measure occurrence are **frozen at render time**, at the point
where the measure is traversed (`render.rs`, just before `render_measure`), and not
recomputed in `Builder::finish`: the map therefore cannot diverge from the MIDI.
`finish` only checks in debug that recomputation from `RepeatList` and
`PauseMap` gives the same value.

`Timemap::notes` is sorted by increasing `tick`, then, for equal ticks, by
`measure`, `staff`, `voice`, `chord`, `note`, `pitch`. The sort is done when
building the map (`Builder::finish`), not on the JavaScript side: the Rust
map and `timeline.notes` therefore have the same deterministic order, and `tick` is
non-decreasing there.

## 3. MusicXML identifiers

The formulas come from `scoretk_core::model::stable_ids`, shared with the export:
no duplication, hence exact equality between the map and the MusicXML.

| Element | Function | Form |
| --- | --- | --- |
| Measure | `stable_measure_id(part, measure)` | `p{part}-m{measure}` |
| Note / rest | `stable_note_id(measure, staff, voice, chord, note)` | `m{measure}-s{staff}-v{voice}-c{chord}-n{note}` |
| Grace note | `stable_grace_note_id(measure, staff, voice, chord, grace_index, note, before)` | `m{measure}-s{staff}-v{voice}-c{chord}-g{grace_index}{b\|a}n{note}` |

- `part`, `measure`, `chord`, `note`, `grace_index` are model indices
  (zero-based), not displayed numbers or passage ranks.
- `chord` is the rank of the `ChordRest` in its voice, **rests included**, in
  rendering order; `note` is zero for a rest.
- `staff` is the staff index (`StaffId::raw()`), `voice` the voice 0 to 3.
- When `stable_ids` is active, the export writes **`id` and `xml:id`** with the
  same value on every `<measure>` and every `<note>`; that is the value
  returned by `MeasureSync::xml_id` and `NoteSync::xml_id`. The measure
  identifier `p{part}-m{measure}` is **unique throughout the document**: what the `partwise` MusicXML
  repeats is the *presence* of the same `<measure>`
  in each `<part>`, and the `p{part}` prefix disambiguates. On the
  JavaScript side, `MeasureSync` exposes only `xmlId` of part 0.

## 4. Repeats: `segment` and `passage`

A model measure can be played several times. The map lists each
**occurrence** in performance order:

- `measure`: index of the measure in `Score::measures` (identical from one
  pass to another);
- `number`: model measure number (zero-based);
- `segment`: index of the repeat segment (`RepeatList::segments`);
- `passage` (`pass` in Rust): pass number of the segment, starting at 1;
- `tick` / `end_tick`: position of **this** occurrence in the performance.

With `expand_repeats: false` (`RepeatList::flatten`), each measure
appears only once. A D.S./D.C. jump can replay a measure in
segments with the same `playback_count`: `passage` can therefore be identical while
`segment` or `tick` differ; the reliable association remains `xmlId` +
`tick`.

## 5. Times, tempos and pauses

Time is authoritative in **integer arithmetic**: each tempo segment
contributes `delta_ticks × micros_per_quarter / DIVISION`, accumulated in `i64`.
There is neither floating point nor clock, hence no drift with the
length of the piece. `time_us_at(tick)` extends the last tempo in force beyond the
last change.

A section pause inserts a measure's worth of ticks and two tempo changes
(start and end of pause, 3 s by default). Between the two, `measure_at(tick)`
returns `None`: the tick falls in the pause. `MeasureSync::pause_before`
flags the measure following a pause; the end of the previous measure remains
fixed at the start of the pause.

## 6. JavaScript structure

`convertWithTimeline` returns `{ musicxml, midi, timeline }`. The Rust
type remains `Timemap`; the JS fields are in `camelCase`:

| Rust (`Timemap`) | JS (`Timeline`) |
| --- | --- |
| `division`, `ticks` | `division`, `ticks` |
| `tempo[].time_us`, `micros_per_quarter` | `tempo[].timeUs`, `microsPerQuarter` |
| `measures[].measure/segment/pass` | `measures[].measure/segment/passage` |
| `measures[].measure` | `measures[].ordinal` (0-based index, only anchoring key) |
| `measures[].xml_id(0)` | `measures[].xmlId` |
| `measures[].end_tick`, `time_us`, `end_time_us` | `measures[].endTick`, `timeUs`, `endTimeUs`, `timeMs` |
| `notes[].duration_ticks`, `pass`, `grace`, `tremolo`, `xml_id()` | `notes[].durationTicks`, `passage`, `grace`, `tremolo`, `xmlId` |
| `notes[].source_offset`, `source_end` (optionsnels) | `notes[].sourceOffset`, `sourceEnd` (`number \| null`) |
| `pauses[].end_tick`, `time_us`, `end_time_us`, `seconds` | `pauses[].endTick`, `timeUs`, `endTimeUs`, `seconds` |

`sourceOffset` / `sourceEnd` are **additive** (AGENTS.md invariant 5): they are
`null` for notes without a textual source (MuseScore, MIDI, …). For GABC they
are UTF-16 code units from the start of the **whole file** (JavaScript string
indices, not UTF-8 bytes): `sourceOffset` points at the *sung pitch letter*,
not at a preceding alteration nor at the `(`, and `sourceEnd` just after the
note token. They are also encoded in the stable MusicXML identifier, e.g.
`m0-s0-v0-c0-n0-o18-e19`, produced by the same
`stable_note_id_with_source` function on the export and timemap sides; without
a source the identifier is unchanged. Example `name:t;\n%%\n(c4) é(fgh)b(gixi)
(::)\n` (é = 1 UTF-16 unit, 2 UTF-8 bytes) gives offsets 18, 19, 20, 24 and 27
(body offsets 7, 8, 9, 13, 16 plus the 11-unit header).

`timeMs` is `timeUs / 1000`. `TimelineMeasure::ordinal` is exactly
`measure` (0-based index in `Score::measures`, identical to the position of the
`<measure>` in each `<part>` of the MusicXML): it is the only anchoring key to
use. The Rust fields absent from the JS surface are `MeasureSync::number`,
`MeasureSync::pause_before`, `NoteSync::grace_index` and
`NoteSync::grace_before`: they serve identifier computation and rendering.
`number` is not exposed (0-based on the model side, 1-based and reset to 1 per section
on the MusicXML side), precisely to avoid confusion with `ordinal`.

## 7. Limits

- **No geometry**: neither x-coordinate, nor system, nor page. Placing a cursor on
  screen requires a layout engine, outside this map.
- **`Note::velocity` not applied**: MIDI velocities come from dynamics,
  hairpins and articulations. `NoteSync` therefore carries no velocity
  and the velocity entered on the note (MIDI import) does not appear.
- **Fermatas**: MuseScore derives no lengthening from them in `engraving` (the
  fermata is absent from `compat/midi/`); their duration is therefore in neither
  the ticks nor the times, which is faithful.
- **Caesuras**: the pause seconds are computed by the *playback* module
  (`PausesMap`, `tempotimeline.h:33`), absent from the port; no pause is
  inserted, as for fermatas.
- **Tremolos**: several `NoteSync`s can share the same `xmlId` (the
  repeated attacks of the same model event), as the notes of a
  chord share a measure.
- **Notes without an attack**: rests and tied continuation notes
  (`<tie type="stop">` / `<tied type="stop">`) have an `id` in the stable MusicXML
  but no `NoteSync`: MIDI rendering emits no attack for
  them, a tied chain having only one initial attack. The test
  `timemap_alignment::note_ids_match_stable_musicxml` compares the multiset
  of notes in the map with that of the `<note>`s in the MusicXML **after explicit subtraction**
  of these two categories.
- **Pauses**: only section pauses are represented; a caesura or
  a fermata shifts nothing (see above).

## 8. Client side: associating a time with a measure

1. search with `measure_at(tick)` (or `timeline.measures` sorted by `tick`)
   for the last measure with `tick <= time < endTick`;
2. if the tick falls in a pause, no measure corresponds: use
   `pauses` to display the pause;
3. to find a graphical element, use `xmlId`: it exists identically
   in the MusicXML (`id` and `xml:id`); to anchor a measure,
   use `ordinal` (0-based index);
4. in case of multiple passes, do not index by `xmlId` alone: keep the
   pair (`xmlId`, `tick`) or the occurrence index in `measures`.

Minimal example:

```js
const at = timeline.measures.find((m) => tick >= m.tick && tick < m.endTick);
const el = at && document.querySelector(`[xml\\:id="${at.xmlId}"], [id="${at.xmlId}"]`);
```
