# MusicXML 4.0 — condensed reference for coding agents

Target: **MusicXML 4.0**, W3C Music Notation Community Group Final Report,
2021-06-01. Local sources: `specs/README.md` § MusicXML;
`specs/musicxml/musicxml-4.0.xsd` (normative XSD, cited `xsd:<complexType|group>`),
`specs/musicxml/reference-elements.html` (`ref:<element>`),
`specs/musicxml/tutorial-*.html` (`tutorial:<chapter>`),
`specs/musicxml/container-4.0.xsd`. Do **not** use the 4.1 draft files as the
4.0 reference. Licence: W3C FSA, © 2004–2021 Contributors to the MusicXML
Specification; this is an original summary, not copied spec text.
Companion: [MusicXML ↔ pivot model](../format-mapping/musicxml.md).
Reader `crates/scoretk-musicxml-import`, writer `crates/scoretk-musicxml-export`.

## Document shape

- Two equivalent orderings: `score-partwise` (`<part>` → `<measure>`) and
  `score-timewise` (`<measure>` → `<part>`). scoretk reads/writes **partwise**.
  (`xsd:score-partwise`; tutorial:structure-of-musicxml-files → Top-Level Elements)
- Header: `<work>`, `<movement-title>`, `<identification><creator type="composer|lyricist|arranger">`, `<defaults><scaling>`, `<credit>`.
  (`xsd:work`, `identification`, `credit`, `defaults`)
- `<part-list>` → one or more `<score-part id="P1"><part-name>…`; order in
  `part-list` is top-to-bottom score order. `<part-group type="start|stop">`
  supplies braces/brackets; a multi-staff part needs no part-group.
  (`xsd:part-list`, `score-part`, `part-group`)
- Musical data: `<part id="P1">` → `<measure number="1">`; measures are
  sequential, no absolute tick on a measure. (`xsd:score-partwise`)
- `.mxl` = ZIP: `META-INF/container.xml` lists `<rootfiles><rootfile full-path="…" media-type="application/vnd.recordare.musicxml+xml"/>`;
  optional `mimetype` (`application/vnd.recordare.musicxml`) is the first,
  uncompressed entry. Use the first rootfile that is the score (not a style
  part). (`container-4.0.xsd`; tutorial:compressed-mxl-files)

## Units and time encoding

- `<divisions>N</divisions>` = N divisions per **quarter note**, declared in
  `<attributes>`; `<duration>` is an integer number of divisions. If absent,
  divisions default to 1 (so integer durations are whole quarters).
  (`xsd:attributes`, group `duration`; tutorial:midi-compatible-part → Divisions)
- Notated figure `<type>` (`1024th` … `whole`, `breve`, `long`) + `<dot>` is
  **display**; `<duration>` is intended sounding time. They may disagree; never
  derive one from the other. (`group:full-note`, `group:duration`)
- Choose divisions large enough that every tuplet duration is an integer;
  truncation silently shifts all later events in the measure.
  (`xsd:positive-divisions`; tutorial:notation-basics → Tuplets)
- Measure position is implicit: each sounding note advances it by its
  `<duration>`; `<backup><duration>` rewinds and `<forward>` skips. Both take a
  positive duration, must not cross a measure boundary or a mid-measure
  divisions change. (`xsd:backup`, `forward`)
- `<measure-style>` children: `<multiple-rest>`, `<measure-repeat type="start|stop">`.
  (`xsd:measure-style`, `measure-repeat`, `multiple-rest`)
- Default tempo: `<direction><sound tempo="120"/>`; tempo is **quarter notes
  per minute**, not SMPTE. (`xsd:sound`; tutorial:midi-compatible-part → Sound Suggestions)

## Pitch, accidentals, key

- `<pitch><step>C</step><alter>1</alter><octave>4</octave></pitch>`;
  `<alter>` = semitones (decimal, may be 0.5), absent = 0; `<octave>` is the
  scientific octave (60 = C4). (`xsd:pitch`; tutorial:notation-basics → Accidentals)
- `<accidental>` (`sharp`, `flat`, `natural`, `double-sharp`, …) is **notation
  only**; it never changes sounding pitch. The key signature (`<fifths>`) also
  does not alter `<pitch>`/`<alter>`. (`xsd:accidental`, `key`)
- `<key><fifths>2</fifths><mode>major</mode></key>`; `fifths` −7..+7 (cycle of
  fifths), `mode` major/minor/church mode; `<cancel>` cancels a previous key.
  `number="<staff>"` targets one staff. (`xsd:key`, group `traditional-key`)
- `<transpose>` inside `<attributes>` gives written ↔ concert offset;
  `<note>` pitch may be concert or written depending on the score's choice —
  keep the two consistent across the whole document. (`xsd:transpose`, `attributes`)
- Percussion uses `<unpitched><display-step>/<display-octave>` instead of
  `<pitch>`; never pitch-map a percussion note by ear. (`xsd:unpitched`; tutorial:percussion)

## Clef, time, staves, parts

- `<clef><sign>G</sign><line>2</line><clef-octave-change>-1</clef-octave-change></clef>`;
  `number` selects a staff, default 1; `G8vb` is sign G + octave-change −1.
  (`xsd:clef`; tutorial:notation-basics → Clef)
- `<time><beats>3</beats><beat-type>4</beat-type></time>`; multiple
  `beats`/`beat-type` pairs form composite meters; `symbol="common|cut"` is a
  display override; `number` selects a staff. (`xsd:time`, group `time-signature`)
- `<staves>N</staves>` in `<attributes>` declares the staff count for the
  part; each `<note>` selects its staff with `<staff>1|2|…</staff>` (default 1).
  `<staff-details><staff-lines>` for non-5-line staves. (`xsd:attributes`, `staff-details`)
- `<voice>` is a **label** (string/int) grouping notes that share a timeline;
  it does not itself move the cursor. Multiple voices in one part require
  `<backup>` between them. (`xsd:note`, `backup`; tutorial:notation-basics → Multi-Part Music)
- `<midi-instrument>` under `<score-instrument>`: `midi-channel` 1–16,
  `midi-program` 1–128, `volume`, `pan`. (`xsd:midi-instrument`, `score-instrument`)

## Notes, rests, chords, beams

- `<note>` child order is an XSD **sequence**: `grace?`, (`pitch|unpitched|rest`),
  `duration?`, `tie*`, `voice?`, `type?`, `dot*`, `accidental?`,
  `time-modification?`, `stem?`, `notehead?`, `staff?`, `beam*`,
  `notations?`, `lyric*`. Appending children out of order breaks validation.
  (`xsd:note`, group `full-note`)
- `<rest/>` (optionally `measure="yes"`) or `<unpitched>`; in a chord, only the
  first note owns the position — every later note carries `<chord/>` and its
  `<duration>` must **not** advance the cursor. (`xsd:rest`, group `full-note`)
- `<beam number="1">begin|continue|end|forward hook|backward hook</beam>`;
  number 1 = eighth beam, 2 = sixteenth, … (`xsd:beam`)
- `<notations>` holds `articulations`, `ornaments`, `fermata`, `technical`,
  `slur`, `tuplet`, `tied`, `arpeggiate`; several `<notations>` elements are
  allowed. (`xsd:notations`)

## Ties, slurs, tuplets, grace

- **Two parallel encodings**: `<tie type="start|stop"/>` = sounding tie;
  `<notations><tied type="start|stop"/></notations>` = notated tie. Emit both
  halves (or both encodings) or playback/display differs. `time-only` limits a
  tie to certain repeat passes. (`xsd:tie`, `tied`)
- Slur: `<notations><slur type="start|stop|continue" number="1"/></notations>`;
  `number` distinguishes nested slurs; add `line-type`/placement as needed.
  (`xsd:slur`; tutorial:notation-basics → Notations)
- Tuplet sound = `<time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification>`;
  display = `<notations><tuplet type="start|stop" number="1" bracket="yes"><tuplet-number>3</tuplet-number><tuplet-type>eighth</tuplet-type></tuplet></notations>`.
  Nested tuplets need matching `number` on both. (`xsd:time-modification`, `tuplet`)
- Grace note: `<grace slash="yes" steal-time-previous="…"/>` before the pitch;
  grace notes have **no** `<duration>`. `<cue>` is a cue-sized note *with* a
  duration and without ties. (`xsd:grace`, `note`)

## Repeats, jumps, directions, lyrics

- `<barline location="right"><bar-style>light-heavy</bar-style><repeat direction="forward|backward" times="2"/></barline>`;
  `direction="forward"` starts, `"backward"` ends a repeat. `location` must
  match the barline's real position. (`xsd:barline`, `repeat`)
- Volta: `<ending number="1" type="start|stop|discontinue">` — `start` on the
  left barline of the first ending measure, `stop/discontinue` on the right
  barline of the last. (`xsd:ending`)
- Jumps: `<direction><direction-type><segno/></direction-type><sound dalsegno="…"/></direction>`
  and the analogous `<coda>/<tocoda>`, `<fine>`. (`xsd:segno`, `coda`, `sound`)
- Dynamics/metronome/text live in `<direction><direction-type>…</direction-type><sound …/></direction>`;
  `<wedge type="crescendo|diminuendo">` is a spanner with `start`/`stop` on
  two directions. (`xsd:wedge`, `metronome`, `dynamics`; tutorial:notation-basics → Musical Directions)
- `<lyric number="1" name="verse"><syllabic>single|begin|middle|end</syllabic><text>He</text><extend type="start|stop|continue"/></lyric>`;
  `<elision/>` joins syllables; `number` gives simultaneous verses. An
  unclosed `<extend>` leaks the melisma to the end of the part. (`xsd:lyric`, `extend`)

## Traps that silently break data

1. `<alter>`/`<fifths>` vs `<accidental>`: only the first two are sounding.
2. Rounding `<duration>` to divisions: silently shifts every later note.
3. `<chord/>` notes must not advance the cursor; otherwise all following
   notes are late. (`xsd:full-note`)
4. New voice without `<backup>` overwrites the previous voice's timeline;
   `<voice>` alone does nothing. (`xsd:backup`)
5. Emitting only `<tie>` (sound) or only `<tied>` (display) loses half the
   information; both halves of a tie are separate elements.
6. `<type>`/`<dot>` disagreeing with `<duration>` is legal; trusting the
   figure alone corrupts timing.
7. `<time-modification>` (sound) without `<tuplet>` (display), or vice versa.
8. Element order inside `<note>`/`<attributes>` is fixed by the XSD sequence.
9. `.mxl`: taking the wrong rootfile or treating `mimetype` as compressed.
10. `<ending>`/`repeat` attached with the wrong `location` attribute.

## Tiny valid example

```xml
<?xml version="1.0" encoding="utf-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>8</divisions>
        <key><fifths>1</fifths><mode>major</mode></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note>
        <pitch><step>G</step><octave>4</octave></pitch><duration>8</duration>
        <type>quarter</type>
        <notations><slur type="start" number="1"/></notations>
        <lyric number="1"><syllabic>begin</syllabic><text>He</text></lyric>
      </note>
      <note>
        <chord/><pitch><step>B</step><octave>4</octave></pitch>
        <duration>8</duration><type>quarter</type>
      </note>
      <note>
        <pitch><step>A</step><octave>4</octave></pitch><duration>8</duration>
        <type>quarter</type><tie type="start"/>
        <notations><tied type="start"/><slur type="stop" number="1"/></notations>
        <lyric number="1"><syllabic>end</syllabic><text>llo</text></lyric>
      </note>
      <note>
        <pitch><step>A</step><octave>4</octave></pitch><duration>8</duration>
        <type>quarter</type><tie type="stop"/>
        <notations><tied type="stop"/></notations>
      </note>
    </measure>
  </part>
</score-partwise>
```
