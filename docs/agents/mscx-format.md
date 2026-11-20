# MSCX format — musical core
Reading reference for `.mscx` 3.x/4.x/5.x for agents writing in
Rust the readers of `scoretk-musescore-import`. Verified C++ sources, read-only:
`ref-musescore/src/engraving/{rw/read302,read400,read410,read460,read500,dom}`.
Paths relative to `ref-musescore/src/engraving/`. Control corpus:
`src/importexport/musicxml/tests/data/*.mscx` (3.01 to 5.00).

## 1. Versions and dispatch
The format version is the `version` attribute of `<museScore>`, not
`<programVersion>`: a 3.6.2 file declares `version="3.02"`.

`RWRegister::reader` (`rw/rwregister.cpp:45-59`) also dispatches earlier
versions: `<= 114` → `Read114` (not ported), `<= 207` → `Read206`, `< 400`
→ `Read302`. See §19 for the 1.x/2.x conventions.
```xml
<museScore version="3.02">
  <programVersion>3.6.2</programVersion>
```
`MscLoader::readMasterScore` (`rw/mscloader.cpp:219-231`) computes
`mscVersion = major*100 + minor`: `3.02→302`, `4.10→410`, `4.60→460`,
`5.00→500`. `RWRegister::reader` (`rw/rwregister.cpp:39-58`) chooses:

| `mscVersion` | Reader | Period |
|---|---|---|
| `< 400` | `Read302` | 3.00–3.99 |
| `400..409` | `Read400` | 4.0.x |
| `410..459` | `Read410` | 4.1–4.5 |
| `460..499` | `Read460` | 4.6–4.9 |
| `>= 500` | `Read500` | 5.0+ |

The 3.x reader reuses `read400`: `Read302::readScoreTag`
(`rw/read302/read302.cpp:54`) calls `read400::StaffRead::readStaff` and
`read400::TRead`. Internal thresholds sometimes differ from the dispatch (4.1, 4.5,
5.0): compare the `mscVersion` integer, never the strings.

## 2. File structure
3.x skeleton:
```xml
<museScore version="3.02"><Score>
  <Division>480</Division><Style>...</Style>
  <metaTag name="composer">...</metaTag>
  <Part>...</Part>
  <Staff id="1"><Measure>...</Measure></Staff>
</Score></museScore>
```
In 4.x/5.x, the style is normally in `score_style.mss` of the `.mscz`
(`MscLoader::loadMscz`, `rw/mscloader.cpp:105-118`), but inline `<Style>`
is still accepted (`Read400::readScoreTag`, `rw/read400/read400.cpp:170-186`).
Linked parts are in `Excerpts/<nom>/<nom>.mscx`. 5.0 adds
`<eid>` on most objects.

Useful children of `<Score>`:

| Element | Role | Reader |
|---|---|---|
| `<Division>` | MIDI division (often 480), used for `<tick>`s | `Read302/400::readScoreTag` |
| `<Style>` | inline style (mainly 3.x) | `compat::ReadStyleHook::readStyleTag` |
| `<metaTag name>` | composer, workTitle, movementTitle… | `Score::setMetaTag` |
| `<Part>` | part and declaration of its staves | `TRead::read(Part*)` |
| `<Staff id>` | musical content of a staff | `StaffRead::readStaff` |
| nested `<Score>` | inline linked part | `readScoreTag` recursion |
| `<SpannerMap>` | 5.0 spanners | `TRead::readScoreSpanners` |

`<SystemObjects>`, `<Order>`, `<PageLocks>`, `<SystemLocks>`,
`<SystemDividers>` are outside the core. The `.mscz` container is handled by
`crates/scoretk-musescore-import/src/container.rs`: `META-INF/container.xml`, `<nom>.mscx`,
`score_style.mss`, `Excerpts/`.
`MscReader::readScoreFile` (`infrastructure/mscreader.cpp:139-163`) looks for the
announced name then the first `.mscx` at the root.

## 3. Time, track, staff
Durations are exact **fractions of a whole note**: `1/4` = quarter, `1/2` =
half, `1/1` = whole, `3/8` = three eighths. `<duration>`, `<len>`,
`<fractions>`, `<ticks_f>` use this form. `XmlReader::readFraction`
(`rw/xmlreader.cpp:117-134`) also accepts a bare integer (internal ticks) and
the old `z`/`n` attributes.

`<Division>` is only used for `<tick>`s: `Score::fileDivision(int)`
(`dom/score.h:565`) applies `(file_tick * 480 + Division/2) / Division`,
where 480 is `Constants::DIVISION` (identity if `<Division>480</Division>`). Duration
fractions do not depend on it.

- `ctx.tick()`: absolute tick from the start; `ctx.rtick()`: relative to the
  measure (`ReadContext::rtick`, `rw/read400/readcontext.cpp:219-223`).
- A measure knows `tick()` and `ticks()`; the tick is almost always
  implicit (section 6).

`VOICES = 4` (`dom/mscore.h:30`): `track = staffIdx*4 + voiceIdx` (0..3).
`staffIdx` follows the global order of `<Staff>` declarations in `<Part>`s;
`voiceIdx` is the position of the `<voice>` block, not an attribute.

Actual duration of a chord/rest:
```text
globalTicks = duree_nominale / ratios_des_olets_imbriques
actualTicks = globalTicks / timeStretch(portee, tick)
```
`DurationElement::globalTicks` (`dom/durationelement.cpp:87-94`) divides by
`t->ratio()` for each parent tuplet; `actualTicksAt`
(`dom/durationelement.cpp:108-117`) divides by `Staff::timeStretch`
(`dom/staff.cpp:560-564`), the `stretch` of the current metre.
`MeasureRead::readVoice` adds `actualTicks()` after every non-grace chord/rest
(`rw/read400/measurerw.cpp:289,328`); grace notes consume no tick.

## 4. `<Part>`, `<Staff>`, `<Instrument>`
```xml
<Part>
  <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
  <trackName>Flute</trackName>
  <Instrument id="Winds">
    <longName>Flute</longName><shortName>Fl.</shortName>
    <trackName>Flute</trackName><instrumentId>wind.group</instrumentId>
    <transposeChromatic>0</transposeChromatic>
    <Channel><program value="73"/></Channel>
  </Instrument>
</Part>
```
`<Part>` contains no music: it declares a part, its staves and its
instrument. `<Staff id="N">` in `<Part>` is **1-based** (`staffIdx=N-1`);
`TRead::readProperties(Part*)` (`rw/read400/tread.cpp:3358`) creates the `Staff` and
appends it (`appendStaff`). The order of declarations in all `<Part>`s
defines the global staff order.

- `<StaffType group="pitched|percussion|tab">`; `<trackName>`: track name,
  which may differ from the instrument's `trackName`.
- `<Instrument id>`: MuseScore model; `<instrumentId>`: MusicXML
  identifier (`wind.group`), preferred for export.
- `<transposeChromatic>`/`<transposeDiatonic>`: transposition, variable over
  time via instrument changes.
- `<clef staff="2">F</clef>`: default clef of the indicated 1-based staff;
  `<Channel><program value>`: MIDI program.
- A two-staff part declares two `<Staff>`s; each staff repeats its
  own sequence of `<Measure>`s (measures are not shared in XML).
- `<InstrumentChange>` carries a complete `<Instrument>` and fixes transposition
  from its tick. `Staff::transpose(tick)` and `Part::instrument(tick)`
  resolve the current instrument: compute per tick.

Differences: **3.x/4.0–4.5** have no `Part/@id`; declaration `<Staff>`s
carry `id`. **5.0** has `<Part id="1">`, `<Staff>`s without `id`
(order), and names in `<InstrumentLabel>` (`<longName>`, `<shortName>`).
**3.6.x** can have bogus `Instrument/@id`s; `Read302::fixInstrumentId`
(`rw/read302/read302.cpp:292-334`) corrects `Winds→winds`, harmonicas
`d12*→d10*`, several `mdl-*`. **5.0**: shared parts `<SharedPart>` /
`<sharedPart>` (EID), outside the scope of a complete export.

## 5. `<Measure>`
```xml
<Measure len="3/4">
  <voice>
    <KeySig><accidental>2</accidental></KeySig>
    <TimeSig><sigN>3</sigN><sigD>4</sigD></TimeSig>
    <Chord><durationType>quarter</durationType>
      <Note><pitch>69</pitch><tpc>17</tpc></Note></Chord>
  </voice>
</Measure>
```
`len` is optional. There is **no** measure number: the position in the
sequence is authoritative; `<noOffset>` only changes display
(`rw/read400/measurerw.cpp:129-130`). Each measure starts at the staff's current
tick. Its default duration is the inherited metre (`4/4` initially):
`measure->setTicks(f); measure->setTimesig(f);`
(`rw/read400/staffrw.cpp:58-61`). A `<TimeSig>` at the start replaces the duration;
a `<TimeSig>` at the end announces the next one (section 10). `len="N/D"` forces
`ticks=N/D` and marks the measure irregular (section 14). `<voice>`s
restart from the beginning of the measure.

The other children (`<startRepeat>`, `<endRepeat>`, `<Marker>`, `<BarLine>`,
`<LayoutBreak>`, `<stretch>`, `<irregular>`, `<vspacer*>`,
`<multiMeasureRest>`) do not change the tick. `measure->ticks()` advances the staff tick
(`rw/read400/staffrw.cpp:72`).

`StaffRead::readStaff` (`rw/read400/staffrw.cpp:39`) creates measures only
for staff 0 (`id="1"`); the following ones reuse them in order
(`:95-126`). A missing measure is created empty (`missing measure!`). The
first staff must be `id="1"` and all must have the same sequence.

`MeasureRead::readMeasure`: `rw/read400/measurerw.cpp:70`;
`Measure::checkMeasure`: `dom/check.cpp:307` (consistency, fills gaps,
logs overflows). **3.02/4.0**: `len` feeds
`compatTimeSigMap` (`:99-100`) and `<stretch>` is ignored before 4.0
(`:118-128`). **4.1+**: no more `compatTimeSigMap`, `<stretch>` read.
**5.0**: `<mmRestLast>` (EID of end of multi-measure rest) appears
(`rw/read500/measureread.cpp:179-184`).

## 6. `<voice>`
```xml
<Measure>
  <voice><Chord>...</Chord><Rest>...</Rest></voice>
  <voice>
    <location><fractions>1/4</fractions></location>
    <Chord>...</Chord>
  </voice>
</Measure>
```
The voice index is **positional**: first `<voice>` = voice 0, second =
voice 1… `MeasureRead::readVoice` does `ctx.setTrack(nextTrack++)`
(`rw/read400/measurerw.cpp:106-108`), with `nextTrack=staffIdx*4`. Each voice
starts at the measure tick (`ctx.setTick(measure->tick())`). The tick advances
by `actualTicks()` after each non-grace `<Chord>`/`<Rest>`; there is no
tick per chord.

Do not consume a tick: `<Clef>`, `<KeySig>`, `<TimeSig>`, `<BarLine>`,
`<Breath>`, `<Dynamic>`, `<Expression>`, `<Harmony>`, `<Tempo>`,
`<StaffText>`, `<RehearsalMark>`, `<Fermata>`, `<Symbol>`, `<Image>`,
`<Spanner>`, `<Tuplet>`/`<endTuplet>`, `<Beam>`, `<Ambitus>`. `<MeasureRepeat>`
/ `<RepeatMeasure>` consumes `measure->ticks()` (`:339-352`). An empty `<voice>`
creates nothing.

The C++ reader increments a counter per `<voice>`: preserve the order, do
not infer the index from content, do not drop an empty `<voice>` (this
shifts the following ones). The corpus has no empty `<voice>`: a one-voice
measure emits only one block.
Typical content: `<Clef>`/`<KeySig>`/`<TimeSig>`, chords/rests/grace notes,
annotations (`<Dynamic>`, `<Harmony>`, `<Tempo>`…), spanners, tuplets, barlines.

`MeasureRead::readVoice`: `rw/read400/measurerw.cpp:211`. **3.02/4.0**:
`<tick>` accepted as obsolete. **4.1+**: `<location>` may carry
`isTimeTick` and create a metre anchor
(`rw/read410/measureread.cpp:215-220`). **5.0**: no more `<Spanner>` inside a
measure; spanners are in `<SpannerMap>` (section 12).

## 7. `<Chord>`, `<Rest>`, `<Note>`
```xml
<Chord><durationType>quarter</durationType><dots>1</dots>
  <StemDirection>down</StemDirection>
  <Note><pitch>75</pitch><tpc>11</tpc></Note></Chord>
<Rest><durationType>quarter</durationType></Rest>
<Rest><durationType>measure</durationType><duration>1/1</duration></Rest>
```
`ChordRest` reads `<durationType>` (`long`, `breve`, `whole`, `half`, `quarter`,
`eighth`, `16th`, `32nd`, `64th`, `128th`, `256th`, `512th`, `1024th`,
`measure`). `TRead::readProperties(ChordRest*)` (`rw/read400/tread.cpp:2405`)
sets ticks to `actualDurationType().fraction()` except for `V_MEASURE`.
`<dots>` (0 to 4) applies the multiplier `(2^(n+1)-1)/2^n`
(`TDuration::fraction`, `dom/durationtype.cpp:357-404`). `<duration>` is a
whole-note fraction that **overrides** the computed ticks. `<small>`,
`<staffMove>`, `<BeamMode>`, `<Articulation>`, `<Ornament>`, `<Lyrics>`,
`<Spanner>` are secondary.

**Order**: `<dots>` must precede `<durationType>`, because ticks are
computed by reading the type with the dots already read. The corpus respects this
order; a Rust reader may read all children before computing, but must
report the reverse order.

`readVoice` creates every `<Rest>` as `V_MEASURE` with
`ticks = measure->timesig() / timeStretch` (`measurerw.cpp:304-309`). Then
`<durationType>` may change the type, and `<duration>` overrides the ticks. In
the corpus, `<duration>` is often `1/1` or `4/4` for a 4/4 measure, `8/4`
for 8/4: it is the measure length, sometimes unreduced. For export,
use `measure->ticks()` if `<duration>` and the measure differ.

Notes:
```xml
<Note><pitch>69</pitch><tpc>17</tpc><tpc2>17</tpc2></Note>
```
- `<pitch>`: **sounding** MIDI pitch 0–127, 60 = middle C
  (`dom/note.h:547`), clamped by `clampPitch` (`rw/read400/tread.cpp:3022`).
- `<tpc>`: spelling (step + accidental), sets `tpc1` **and** `tpc2`
  (`:3026-3029`). `<tpc2>` overrides the written spelling; `tpc1` = concert,
  `tpc2` = transposed part.
- `<Accidental>`: graphical accidental, distinct from `tpc`.
- `<track>`, `<velocity>`, `<play>`, `<tuning>`, `<fret>`, `<string>`,
  `<ghost>`, `<dead>`, `<head>`, `<line>`: playback. `<NoteDot>`: graphical dots;
  duration dots are in the ChordRest's `<dots>`.
- **4.x**: `<velocity>` on `<Note>` is `Pid::USER_VELOCITY`, a
  **signed** integer (deviation percentage in offset mode, absolute in user mode), and
  `<veloType>` is `offset` or `user` (`typesconv.cpp:2499-2500`). The
  property is `USER_VAL` by default (`dom/note.cpp:3316-3317`): the C++
  writes `<veloType>` only for `offset`. An unknown `<veloType>` tag is
  read as `OFFSET_VAL` (`rw/read410/tread.cpp:401-402`). MIDI rendering
  applies `Note::customizeVelocity` per note. In 3.x, note `<velocity>` is
  ignored (`read302`), as in the C++.

`Note::setupAfterRead` (`dom/note.cpp:1651-1712`) derives a missing `tpc` from
the key and `pitch`, derives the other spelling by transposition, and
adjusts `pitch` if `tpc1` is inconsistent modulo 12 (except percussion).

TPC table (`dom/pitchspelling.h:41-58`):

| Step | F | C | G | D | A | E | B |
|---|---|---|---|---|---|---|---|
| `tpc` | 13 | 14 | 15 | 16 | 17 | 18 | 19 |

Each sharp adds 7, each flat subtracts 7 (`F#=20`, `Bb=12`,
`C##=28`). `TPC_MIN=-8` (`Fbbb`), `TPC_MAX=40` (`B###`). Conversion
(`dom/pitchspelling.cpp`, `dom/pitchspelling.h:121-129`):
```text
alter  = (tpc - TPC_MIN) / 7 - 3          // -3..+3
step   = {F:3,C:0,G:4,D:1,A:5,E:2,B:6}[(tpc - TPC_MIN) % 7]
octave = (pitch - alter) / 12 - 1
```
`tpc2pitch` (`dom/pitchspelling.cpp:195-210`) gives the semitones of naturals
(`F=5, C=0, G=7, D=2, A=9, E=4, B=11`), plus `alter`. For a transposing
instrument, the written pitch is `pitch - instrument.transpose.chromatic`.

Readers: `TRead::read(Chord*)` (`rw/read400/tread.cpp:2304`),
`readProperties(Chord*)` (`:2323`), `readProperties(ChordRest*)` (`:2405`),
`read(Rest*)` (`:3447`), `read(Note*)` (`:3005`), `readProperties(Note*)`
(`:3020`). **3.02/4.0**: pre-1.12 compatibilities in `<durationType>`
(wholes converted to measure rests, obsolete `ticklen`) and pre-1.14 metres
(`:2405-2458`). **Before 4.1**: `<BeamMode>` translates
`begin64`/`begin32` to `begin32`/`begin16` (`:2433-2443`); **4.1+** reads
directly (`rw/read410/tread.cpp:2501`). **4.0+**: `ghost` = `ghost` (3.x:
`dead`); user `<velocity>` in 4.x, ignored in 3.x. **5.0**: no more
pre-1.12 compat (`rw/read500/tread.cpp:2576`), addition of `<Text>`,
`<LaissezVib>`, `<PartialTie>` in `<Note>`.

## 8. `<Tuplet>` and `<endTuplet/>`
```xml
<Tuplet>
  <normalNotes>2</normalNotes>
  <actualNotes>3</actualNotes>
  <baseNote>eighth</baseNote>
  <Number><text>3</text></Number>
  <Chord><durationType>eighth</durationType>...</Chord>
  <Chord>...</Chord><Chord>...</Chord>
</Tuplet>
<endTuplet/>
```
`<normalNotes>` = denominator of the ratio, `<actualNotes>` = numerator,
`<baseNote>` (+ `<baseDots>`) = base duration.
`ticks = baseLen.fraction() * ratio.denominator()`: eighth-note triplet =
`1/8*2 = 1/4`
(`TRead::read(Tuplet*)`, `rw/read400/tread.cpp:1519-1521`). Inner chords
keep their nominal duration (`1/8`) but their actual duration is divided
by the ratio (`1/12`).

`<endTuplet/>` is a **sibling**, not a closing XML tag: it closes the
current tuplet and returns to the parent. Nested tuplets: a `<Tuplet>` encountered
while another is open becomes its child
(`rw/read400/measurerw.cpp:587-595`); each `endTuplet` closes one level.
`endTuplet` without an opener: ignored with a log. Empty tuplet: dropped. Non-empty
unclosed tuplet: left with a log (`:596-646`). `<Tuplet>`s do not advance
the tick: only chords/rests do, with `actualTicks()` already divided
by the ratios.

Readers: `MeasureRead::readVoice` (`measurerw.cpp:587-612`),
`TRead::read(Tuplet*)` (`tread.cpp:1446`), `DurationElement::globalTicks`
(`dom/durationelement.cpp:87`). Mechanism stable from 3.02 to 5.0.

## 9. `<location>`, `<tick>`, `<move>`
```xml
<voice>
  <location><fractions>1/4</fractions></location>
  <Chord>...</Chord>
</voice>
```
Fields: `<staves>`, `<voices>`, `<measures>`, `<fractions>`, `<grace>`,
`<notes>` (`TRead::read(Location*)`, `rw/read400/tread.cpp:2859-2880`).
`<measures>` = offset in measures (0 current, 1 next, -1 previous);
`<fractions>` = offset as a whole-note fraction relative to the start of the target
measure; `<staves>`/`<voices>` = staff/voice offset, rare; `<grace>` =
grace-note index; `<notes>` = note index in a chord.

`ReadContext::setLocation` (`rw/read400/readcontext.cpp:274-298`) does
`Location::toAbsolute(location())` (adding the current position), then
`setTrack`, `setTick(l.frac())`, and in normal reading
`incTick(currentMeasure()->tick())` to make the fraction absolute. Thus
`<fractions>1/4</fractions>` places what follows at a quarter of the measure;
`<measures>1</measures><fractions>-1/4</fractions>` at a quarter before the end of
the next measure. The shift persists for following chords.

`<tick>1920</tick>` in `<Staff>` or `<voice>` sets an **absolute** tick in
file ticks:
`ctx.setTick(Fraction::fromTicks(ctx.fileDivision(e.readInt())))`
(`staffrw.cpp:89-90,121-122`; `measurerw.cpp:229-231`). It is not relative to
the measure; the tag is obsolete and absent from the corpus.

No `<move>` tag is read by the 3.x/4.x/5.x readers.
`XmlReader::readFraction` mentions `<move z="2" n="4"/>` and `<move>2/4</move>`
in a comment (`rw/xmlreader.cpp:111-115`): a remnant of very old
formats. A `<move>` would be ignored by `e.unknown()`. Shifting is done via
`<location>`.

**4.1+**: `<location>` may carry `isTimeTick` and create a metre
anchor (`rw/read410/measureread.cpp:215-220`), with no effect on the
musical tick. **5.0**: spanners no longer use `<location>` (section 12).

## 10. Key signature, metre, clef
### 10.1 `<KeySig>`
```xml
<KeySig><accidental>2</accidental></KeySig>
<KeySig><concertKey>2</concertKey><actualKey>2</actualKey></KeySig>
```
`<accidental>`: `Key` integer from -7 to +7 (0 = C major / A minor).
`<concertKey>`: concert key signature; `setConcertKey` also sets the written
key (`dom/key.cpp:205-210`). `<actualKey>`: written key signature, otherwise = concert. `<subtype>`: old encoding, fallback via `initFromSubtype`
(`dom/key.cpp:270`). `<custom>`, `<CustDef>`/`<KeySym>`: custom
key signatures; `<mode>`, `<showCourtesy>`: secondary. A key signature at the start of a measure is
real; at the end of a measure, it is a courtesy announcing the next one.

Reader: `TRead::read(KeySig*)` (`rw/read400/tread.cpp:1189`); placement
`rw/read400/measurerw.cpp:418-429`; `Staff::setKey` (`dom/staff.h:109`).
**3.02/4.0**: `<accidental>`. **4.1**: `<accidental>` or
`concertKey`/`actualKey` (`rw/read410/tread.cpp:1331-1345`). **4.6+**:
`concertKey`/`actualKey` only (`rw/read460/tread.cpp:1318-1321`). **4.5+**:
courtesy via `<isCourtesy>`, otherwise guessed from position.
### 10.2 `<TimeSig>`
```xml
<TimeSig>
  <sigN>3</sigN><sigD>4</sigD>
  <stretchN>1</stretchN><stretchD>1</stretchD>
</TimeSig>
```
`<sigN>`/`<sigD>`: metre. `<subtype>`: graphical type
(`dom/timesig.h:37-43`): 0 normal, 1 common time, 2 alla breve, 3 cut Bach,
4 cut triple; no effect on duration. `<stretchN>`/`<stretchD>`: local
stretch; divides the actual duration (`actualTicks = globalTicks / timeStretch`).
`<textN>`/`<textD>`, `<Groups>`: display/grouping. `<isCourtesy>`:
courtesy (4.5+). A metre at the start of a measure sets `timesig` and, if the
measure is not irregular, `ticks`. At the end of a measure, it announces the
next one (`ctx.setTimeSigForNextMeasure`,
`rw/read410/measureread.cpp:381-384`).

Reader: `TRead::read(TimeSig*)` (`rw/read400/tread.cpp:3979`); placement
`rw/read400/measurerw.cpp:395-417`. **3.02/4.0**: courtesy guessed from
`currTick > measure->tick()`. **4.1–4.4**: same guess if
`mscVersion < 450` (`rw/read410/measureread.cpp:370`). **4.5+**:
`isCourtesy()` alone (`rw/read460/measureread.cpp:364`). Old
`<den>`/`<nom1..4>` are still read, outside the 3.02+ scope.
### 10.3 `<Clef>`
```xml
<Clef>
  <concertClefType>G</concertClefType>
  <transposingClefType>G</transposingClefType>
  <isHeader>1</isHeader>
</Clef>
```
`<concertClefType>`/`<transposingClefType>`: concert/transposing clefs.
`<isHeader>`: header clef (4.1+). `<showCourtesyClef>`, `<clefToBarlinePos>`,
`<forInstrumentChange>`: secondary. No valid clef → `G` forced
(`rw/read400/tread.cpp:2551-2553`). Corpus values: `G`, `F`, `C3`, `G8vb`,
`G8vbp`, `PERC`, `TAB`; full table in `TConv::fromXml` (generated, absent
from the copy). Start of measure = header; elsewhere = mid-measure change.
Before 4.5, the header is guessed from position and the preceding key/metre
(`rw/read400/measurerw.cpp:360-385`).

Reader: `TRead::read(Clef*)` (`rw/read400/tread.cpp:2527`); placement
`rw/read400/measurerw.cpp:353-394`. **3.02/4.0**: no `<isHeader>`,
guessed. **4.1–4.4**: `<isHeader>` with fallback
(`rw/read410/measureread.cpp:350-360`). **4.5+**: authoritative `<isHeader>`,
no more `AFTER` correction (`rw/read460/measureread.cpp:352-356`).

## 11. Ties (`Tie`)
**Correcting a misconception.** In 3.x, a tie is not a standalone `<Tie>`:
it is already a `<Spanner type="Tie">` containing `<Tie>` and an
end `<next>`/`<prev>` (`testChordSymbols.mscx`, 3.01; `Read302` delegates
to `read400::TRead`, which knows no standalone `<Tie>` in `Note`). The real
difference: **3.x/4.x** = two half-spanners with `<location>`; **5.0** = a
`<Tie>` directly in the note, with `<startElement>`/`<endElement>` (EIDs).
```xml
<Note><pitch>75</pitch><tpc>11</tpc>
  <Spanner type="Tie"><Tie/>
    <next><location><fractions>1/4</fractions></location></next>
  </Spanner></Note>
<Note>
  <Spanner type="Tie">
    <prev><location><fractions>-1/4</fractions></location></prev>
  </Spanner><pitch>75</pitch><tpc>11</tpc></Note>
```
```xml
<!-- 5.0 -->
<Note>
  <eid>Z_Z</eid>
  <Tie>
    <startElement>S_S</startElement>
    <endElement>Z_Z</endElement>
    <eid>a_a</eid>
    <track>1</track>
  </Tie>
  <pitch>66</pitch><tpc>20</tpc>
</Note>
```
A tie consumes no tick and does not change duration. It links two
notes of the same pitch; export produces `<tie>` (sound) and `<tied>` (notation).
In 3.x/4.x, one `ConnectorInfo` per half-spanner, matching by
type/track/location, `tieFor` at the start and `tieBack` at the end
(`ConnectorInfoReader::readAddConnector(Note*)`,
`rw/read400/connectorinforeader.cpp:367-430`). In 5.0,
`TRead::readProperties(Spanner*)` records the EIDs, then
`ReadContext::connectNoteAnchoredSpanners` resolves them at the end of reading
(`rw/read500/tread.cpp:3880-3935`; `rw/read500/readcontext.cpp:261-292`). The
5.0 `<Tie>` is placed in the ending note in test files, but names
both ends: a single element is sufficient.

Readers: `TRead::read(Tie*)` (`rw/read400/tread.cpp:3974`,
`rw/read500/tread.cpp:4411`); `ConnectorInfoReader::read`
(`rw/read400/connectorinforeader.cpp:73`); `Score::connectTies` at the end of
reading (`rw/read302/read302.cpp:227`, `rw/read400/read400.cpp:297`).

## 12. Generic spanners (`Slur`, `HairPin`, …)
3.x/4.x mechanism:
```xml
<Spanner type="Slur">
  <Slur><SlurSegment no="0"/></Slur>
  <next><location><fractions>1/2</fractions></location></next>
</Spanner>
<!-- autre extrémité -->
<Spanner type="Slur">
  <prev><location><fractions>-1/2</fractions></location></prev>
</Spanner>
```
`type` = element name: `Slur`, `HairPin`, `Ottava`, `TextLine`, `Volta`,
`Trill`, `Pedal`, `LetRing`, `Vibrato`, `PalmMute`, `WhammyBar`, `Rasgueado`,
`HarmonicMark`, `GradualTempoChange`… The child has the same name and its
properties (`<subtype>`, `<color>`, `<SlurSegment>`…). The current
end is implicit (position of the containing element); `<next>`/`<prev>`
gives the other via a relative `<location>`. The reader matches the two
halves by type, track and absolute location, then creates a single object. An
isolated `<Spanner>` can be reconnected at the end of the score
(`ctx.reconnectBrokenConnectors()`) or dropped as an orphan.

5.0 mechanism:
```xml
<SpannerMap>
  <Slur>
    <startElement>N_N</startElement>
    <endElement>s_s</endElement>
    <eid>ZG_ZG</eid>
    <track>0</track>
  </Slur>
</SpannerMap>
```
`<SpannerMap>` is a direct child of `<Score>`. Each spanner is a direct
element, without `<Spanner type>` or `<next>`/`<prev>`. `<startElement>` /
`<endElement>` are EIDs of chords, rests or measures; `<track>` gives the
track. `TRead::readScoreSpanners` (`rw/read500/tread.cpp:617-632`) creates the object
via `tryCreateSpanner` (`:595-615`). Note-anchored spanners (Tie,
Glissando, NoteLine) remain in the note.

Readers: `TRead::readSpanner` (`rw/read400/tread.cpp:4338`);
`ConnectorInfoReader::read`/`readAddConnector`
(`rw/read400/connectorinforeader.cpp:73-500`); `TRead::readScoreSpanners`
(`rw/read500/tread.cpp:617`). **3.02/4.0**: `<Spanner>` may also appear
at `<Score>` level for `HairPin`, `Ottava`, `TextLine`, `Volta`, `Trill`,
`Slur`, `Pedal`, read by `TRead::readItem` (`rw/read400/read400.cpp:222-230`).
**5.0**: no more `<Spanner>` at voice/measure level; everything goes through
`<SpannerMap>` except note-anchored spanners.

## 13. `<Lyrics>` and grace notes
### 13.1 `<Lyrics>`
```xml
<Chord>
  <durationType>quarter</durationType>
  <Lyrics>
    <no>1</no>
    <syllabic>begin</syllabic>
    <text>beau-</text>
  </Lyrics>
  <Note><pitch>72</pitch><tpc>15</tpc></Note>
</Chord>
```
Child of the **Chord**, not of the Note. `<no>`: **0-based** line (absent = 0,
`1` = 2nd line). `<syllabic>`: `single`, `begin`, `end`, `middle`. `<text>`:
syllable (may contain `<sym>`). `<ticks_f>`: melisma, replaces `<ticks>`.
`<LyricsLine>`: continuation line (5.0). `<placement>`/`<offset>`: outside
the core.

Readers: `TRead::read(Lyrics*)` (`rw/read400/tread.cpp:2882`);
`readProperties(Lyrics*)` (`:2926`); creation from `<Chord>` (`:2488-2492`).
**3.02**: obsolete `<ticks>` still read. **4.0+**: `<ticks_f>`. **5.0**:
`<LyricsLine>` and `PartialLyricsLine`, no more `<ticks>`.
### 13.2 Grace notes
```xml
<Chord>
  <durationType>eighth</durationType>
  <appoggiatura/>
  <Note><pitch>72</pitch><tpc>15</tpc></Note>
</Chord>
<Chord><durationType>quarter</durationType>
  <Note><pitch>74</pitch><tpc>16</tpc></Note></Chord>
```

| Tag | `NoteType` |
|---|---|
| `<appoggiatura/>` | `APPOGGIATURA` |
| `<acciaccatura/>` | `ACCIACCATURA` |
| `<grace4/>` | `GRACE4` |
| `<grace16/>` | `GRACE16` |
| `<grace32/>` | `GRACE32` |
| `<grace8after/>` | `GRACE8_AFTER` |
| `<grace16after/>` | `GRACE16_AFTER` |
| `<grace32after/>` | `GRACE32_AFTER` |

A grace chord is a `<Chord>` with a `NoteType` other than `NORMAL`, placed
**before** the main chord. `MeasureRead::readVoice` does not add it to a
segment and does not advance the tick: it puts it in `graceNotes`
(`measurerw.cpp:276-285`), then attaches it to the next normal chord with
`graceIndex = 0,1,…` (`:280-284`). A grace note's `<durationType>` is indicative
(hook size), not a musical duration. `grace8after` means "after the
main note"; the reader still stores it in the following chord's list,
the order being resolved by `NoteType`. A grace note without a following normal
chord remains unattached: corruption to report.

Readers: `TRead::readProperties(Chord*)` (`tread.cpp:2349-2375`);
`MeasureRead::readVoice` (`measurerw.cpp:214,265-290`); `Chord::add`
(`dom/chord.cpp:686-694`). Same mechanism in 3.02/4.x/5.0; 5.0 adds
EIDs.

## 14. Irregular measures (`len`)
```xml
<Measure len="3/4">
  <voice>
    <Chord><durationType>half</durationType>...</Chord>
    <Chord><durationType>quarter</durationType>...</Chord>
  </voice>
</Measure>
```
`len` is an **attribute**, parsed by `Fraction::fromString`; it sets
`measure->ticks(len)`. The metre (`timesig`) remains inherited/declared. The
measure is marked `irregular`: a `<TimeSig>` read inside it does not rewrite
`ticks` (`rw/read400/measurerw.cpp:414-416`). Invalid `len` or `< 1/128`:
MSCX error (`:90-101`). `len` may be longer or shorter than the
metre (3/4 in 4/4, 8/4, 23/8…); the tick advances by `len`.

Reader: `MeasureRead::readMeasure` (`rw/read400/measurerw.cpp:89-101`).
**3.02/4.0**: adds `compatTimeSigMap` (`:99-100`). **4.1+**: these entries
disappear (`rw/read410/measureread.cpp:90-95`).

## 15. Excerpts and linked parts
There is **no** `<Excerpt>` tag in an MSCX.

Nested score (3.x/4.x):
```xml
<Score>...
  <Staff id="1">...</Staff>
  <Score><name>Flute</name>
    <Tracklist sTrack="0" dstTrack="0"/>
    <Staff id="1">...</Staff></Score>
</Score>
```
`Read302/400::readScoreTag` handles `<Score>` by recursion: creates a separate
`Score`, reads its content, then `Excerpt::linkMeasures(s, m)` links the measures
to the master (`rw/read302/read302.cpp:143-164`; `rw/read400/read400.cpp:233-257`).
`<name>` applies only to the child score; `<Tracklist>` maps tracks.

Separate files (4.x/5.x): `MscLoader::loadMscz`
(`rw/mscloader.cpp:156-207`) lists `Excerpts/*.mscx`, reads each file with
the same reader, then `Excerpt::linkMeasures(partScore, masterScore)`. The style
is in `Excerpts/<nom>/<nom>.mss`.

For export: full score → ignore nested `<Score>`s and
`Excerpts/` files (the master contains everything); parts → read the excerpt and
apply `<Tracklist>` (or the 5.0 EIDs), without merging measures. 5.0:
`<SharedPart>`/`<sharedPart>` (EIDs), outside the full scope. Pitfall: "all
descendant `<Staff>`s" also gathers excerpts and doubles the music;
stop at the direct children of the master `<Score>`.

## 16. Summary 3.02 → 5.0

| Subject | 3.02 | 4.0 | 4.1 | 4.6 | 5.0 |
|---|---|---|---|---|---|
| Reader | `Read302` | `Read400` | `Read410` | `Read460` | `Read500` |
| `Part/@id` | no | no | no | no | yes |
| `Staff` declaration | `id` | `id` | `id` | `id` | order, no `id` |
| Instrument names | `longName` | idem | idem | idem | `InstrumentLabel` |
| `KeySig` | `accidental` | `accidental` | both | `concertKey` | idem |
| Courtesy | guessed | guessed | guessed if `<450` | `isCourtesy` | `isCourtesy` |
| Header clef | guessed | guessed | `isHeader`+fallback | `isHeader` | `isHeader` |
| Tie | `Spanner` | `Spanner` | `Spanner` | `Spanner` | `<Tie>`+EID |
| Spanners | `Spanner` | `Spanner` | `Spanner` | `Spanner` | `SpannerMap` |
| `<eid>` | no | no | no | no | yes |
| Pre-1.12 durations | yes | yes | yes | yes | no |
| Old `BeamMode` | translated | translated | direct | direct | direct |
| Excerpts | inline | inline/files | files | files | +SharedPart |

## 17. Pitfalls
1. **Implicit tick.** No `<tick>` per chord: it starts at the current tick
   and advances by `actualTicks()`.
2. **`actualTicks()` ≠ `ticks()`.** Divide by tuplet ratios and by
   `timeStretch` (section 3).
3. **`<dots>` before `<durationType>`.** Ticks are computed when reading the
   type; later dots are ignored. `<duration>` overrides everything: for a
   `measure` rest, use the measure length, not the raw value.
4. **Positional voices and unshared staves.** The index is the order of the
   `<voice>`; do not infer it from content, do not drop an empty `<voice>`
   Staff 0 creates the measures, the following ones reuse them in
   order; one offset shifts everything.
5. **Absolute `<tick>`, relative `<location>`.** `<tick>` is in file ticks
   from the start; `<fractions>` is relative to the target measure. `<move>`
   does not exist in 3.02+: use `<location>`.
6. **Nested tuplets.** Each `endTuplet` closes one level; the tick advances
   per chord already divided by all ratios, do not add
   `tuplet->ticks()`. Empty tuplet dropped, non-empty unclosed tuplet left with a log:
   report, do not invent.
7. **Grace notes without duration.** `appoggiatura`/`acciaccatura`/`grace*` consume
   no tick and attach to the following chord via `graceIndex`.
   A final grace note with no following chord is corrupt.
8. **3.x/4.x tie = two halves** (`<next>` and `<prev>`). In 5.0, a
   `<Tie>` with EID: do not look for `<Spanner type="Tie">` in 5.0, nor for a
   standalone `<Tie>` in 3.x/4.x.
9. **5.0 spanners outside measures.** They are in `<SpannerMap>` at
   `<Score>` level; a traversal of voices only misses them.
10. **Excerpts.** Ignore for a full export; handle separately for
     parts. Do not gather the `<Staff>`s of nested `<Score>`s.
11. **Key signature 4.6+.** `<accidental>` is no longer read; use
     `concertKey`/`actualKey`. In 4.0/4.1, `<accidental>` is still written.
12. **Courtesy and header clef before 4.5.** Guessed from position;
     since 4.5, only `isCourtesy`/`isHeader` are authoritative (4.1 already writes
     `<isHeader>`).
13. **Sounding pitch, written tpc.** `<pitch>` is sounding; `tpc1` = concert,
     `tpc2` = written. For written pitch, subtract the transposition.
     `setupAfterRead` adjusts `pitch` if `tpc1` is inconsistent modulo 12.
14. **Instruments 3.6.** `Instrument/@id` may be bogus; prefer
     `<instrumentId>`/`<trackName>` and `Read302::fixInstrumentId`.
15. **`<Division>`, forms and version.** `<Division>` is only for `<tick>`s;
     `1/1`, `4/4`, `8/8` may be the same duration (compare after reduction);
     `version="5.00"` -> 500, do not compare `"5.0"` with `"5.00"` nor infer the
     schema from `<programVersion>`. A measure without `<voice>` keeps its duration:
     generate a measure rest on export.

## 18. Texts and frames
```xml
<StaffText><placement>below</placement><text>doux <b>et</b> <i>lié</i></text></StaffText>
<Tempo><tempo>2</tempo><followText>1</followText>
  <text><sym>metNoteQuarterUp</sym> = 120</text></Tempo>
<RehearsalMark><frameType>2</frameType><text>A</text></RehearsalMark>
<VBox><height>10</height>
  <Text><style>title</style><text>Mon titre</text></Text>
</VBox>
```
Segment annotations (none consumes a tick, section 6): `StaffText`,
`SystemText`, `Tempo`, `RehearsalMark`, `Expression`, `InstrumentChange`,
`Dynamic`, `Harmony`. Read properties: `<text>`, `<placement>` (`above`
default / `below`), `<visible>` (0 = not exported), `<frameType>` (0 no frame,
1 rectangle, 2 circle), `<style>`, `<italic>`/`<bold>`/`<underline>`/`<strike>`;
for `Tempo` `<tempo>` (beats **per second**) and `<followText>`; for
`InstrumentChange` `<Instrument>` (not read). `<style>` sets the placement and the
starting font that the file does not repeat: `<style>expression</style>`
gives italic text **below** the staff (`TextStyleType::EXPRESSION`), and the
`italic`/`bold` properties above override that starting format.

`<text>` is **markup**, not plain text: `TRead::readProperties`
uses `XmlReader::readXml` (`rw/xmlreader.cpp:212`), which re-serialises the
inner tags. Tags: `<b>`, `<i>`, `<u>`, `<s>`, `<sub>`, `<sup>`,
`<font face="…"/>` and `<font size="…"/>` (one property per tag;
`</font>` does not restore the previous format), `<sym>name</sym>` (a
SMuFL symbol rendered by a MuseScore code point), `\n` = line break. Named entities
(`&quot;`, `&amp;`) are preserved as-is in the internal text,
numeric references are resolved.

Export (`export/text.rs`, port of `MScoreTextToMusicXml::writeTextFragments`
and `ExportMusicXml::words`/`tempoText`/`systemText`/`rehearsal`/`credits`):
one `<words>` per fragment, format attributes only when they change compared to the
previous fragment (reference = staff text style: Edwin
10). `StaffText`/`Expression`/`InstrumentChange` → `<direction>` + `<words>`;
`SystemText` and `Tempo` → `system="only-top"`; `Tempo` → `<metronome>` when
`findMetronome`/`TempoText::findTempoDuration` recognises a formula
(`<beat-unit>`, `<beat-unit-dot>`, `<per-minute>`), plus `<sound tempo>`
(rounded to 2 decimals); an invisible tempo exports only that `<sound>`.
`RehearsalMark` → `<rehearsal>` with `enclosure` (absent if rectangle).
Text shifted from the current tick adds
`<offset sound="yes">` (tempo, `<words>`) or `<offset>` (mark).

`<VBox>` → `<credit>` (`ExportMusicXml::credits`): one `<credit page="1">` per
visible `<Text>`, `<credit-type>` from `<style>` (`title`, `subtitle`,
`composer`, `lyricist`, empty otherwise), `justify`/`valign` alignments of the style,
font and default size according to the style. `<work>`, `<movement-number>`,
`<movement-title>` and the `<creator>`s of `<identification>` come from
`<metaTag>`s, not from texts.

Not ported: `TBox` texts (`tboxTextAsWords`), spanner texts
(`TextLine`), instrument-change details (`writeInstrumentChange`),
symbols outside the metronome table, positions (`default-x`/`default-y`), score
style (`<Style>`/`.mss`) and `<miscellaneous>`. `<Marker>`/`<Jump>`
(D.C., D.S., Fine…) are exported as `<words>` by `export/repeats.rs`:
do not produce them a second time from annotations.
## 19. MuseScore 1.x/2.x (`read206`) — differences from 3.x

There is no published MSCX specification; this section follows
`src/engraving/rw/read206/read206.cpp` (snapshot `2b9e82d…`, 5.0 development)
and the MuseScore 4.7.4 export used as the conformance reference. `Read206`
(`rwregister.cpp:45-49`) handles `115..=207`; `<= 114` goes to `Read114`, which
is **not** ported (one `1.14` file in `~/Musique` remains unreadable). Nearly
every element is delegated to the common
`read400::TRead` code, already ported as `read302`; only the container-level
conventions differ.

### 19.1 `<Score>` layout

A 2.x `<Score>` carries `<Division>` and metadata inline, just like 3.x, but
`<metaTag>` names are supplemented by dedicated tags (`work-title`,
`movement-title`, `work-number`, `movement-number`, `source`, `copyright`).
`<siglist>` holds `<sig>` events with `tick`, `<nom>`, `<denom>`, optional
`<nom2>`/`<denom2>` (`Read206::readTimeSigMap`); it only feeds the obsolete
`<ticklen>` handling, since the real `<TimeSig>` elements live in measures.
`<Part>` declares `<Staff id="…">` children plus `<Instrument>`, where
`<transposeDiatonic>`/`<transposeChromatic>` may also appear on a declared
`<Staff>` (`readStaff`, `read206.cpp:889-892`) rather than on the instrument.
`<VBox>`/`<HBox>`/`<TBox>` frames precede the measures of the first staff.

### 19.2 No `<voice>` blocks; `<track>` and `<tick>` instead

`readMeasure206` (`read206.cpp:2645`) reads `<Chord>`, `<Rest>`, `<Clef>`,
`<KeySig>`, `<TimeSig>`, `<BarLine>`, annotations… as **direct children** of
`<Measure>`. There is no `<voice>` element. Voices are distinguished by a
`<track>` child on the element (`track = staffIdx * 4 + voiceIdx`), and their
position is given by an absolute `<tick>` in file divisions, converted with
`ctx.fileDivision()` (`read206.cpp:2688-2690`). The obsolete `<move>`
(`read206.cpp:2686`) is a fraction relative to the measure start. The Rust
reader groups elements into `<voice>` blocks by `track % 4` and repeats each
measure-level `<tick>` into every group, then hands the tree to `read302`.

### 19.3 `<Tie id>` and `<endSpanner id>`

A tie is **not** a two-half `<Spanner>` in 2.x. The starting `<Note>` contains
`<Tie id="N">` (`readTie206` registers the spanner by id, `read206.cpp:2619`),
and the ending element contains `<endSpanner id="N">` — in 2.x this is read
from `<Note>` (`readNoteProperties206`, `read206.cpp:1221`) as well as from the
measure. The reader reconnects both halves by id and sets `tie_forward` /
`tie_back` on the model notes. The same `<endSpanner id>` mechanism closes
slurs, hairpins, pedals, ottavas, trills and voltas.

### 19.4 `<Tuplet id>` references

`readTuplet206` (`read206.cpp:1615`) creates a tuplet from `<Tuplet id="N">`
with `<normalNotes>`, `<actualNotes>`, `<baseNote>` and a `<Number>` text.
Chords are attached by a **reference child** `<Tuplet>N</Tuplet>`
(`readDurationProperties206`, `read206.cpp:1691`). There is no `<endTuplet/>`
in 2.x. The Rust reader turns each reference into the 3.x sequence
`<Tuplet>…</Tuplet>`, referenced chords, `<endTuplet/>`; closing the tuplet at
the last referencing chord is essential — leaving it open would apply its
ratio to the following chords and drop notes through tick collisions.

### 19.5 Texts and annotations

`readText206` uses `TextReaderContext206` (`read206.cpp:1438`): the `<text>`
content is re-parsed in a second reader because in 2.x the `<style>` tag may
appear anywhere inside the element. Style names come from a fixed table
(`readTextStyle206`, `read206.cpp:317-361`) — `"Title"`, `"Staff"`, `"System"`,
`"Tempo"`, `"Dynamics"`… A `<Text>` with style `System`/`Tempo`/`Marker`/
`Jump`/`Volta` becomes a `SystemText` in 3.x (`read206.cpp:3023`). Tempo text
rewrites `<sym>unicode…` to `<sym>met…`, and marker text replaces the private
use symbols `U+E177/E178/E179` with `<sym>segno</sym>`, `<sym>coda</sym>`,
`<sym>codaSquare</sym>` (`convertMScoreFontMarkerSymbols206`). `<Dynamic>`
reads `<subtype>`, `<velocity>` and `<dynType>` (`read206.cpp:1586`).

### 19.6 `<Chord>` and `<Note>` details

Same element names as 3.x, but the TPC/spelling fallback of `readNote206`
(`read206.cpp:957-1012`) derives a missing `tpc` from the key, then derives the
other spelling via `Transpose::transposeTpc`. `<velocity>` on a 2.x note is
ignored (`read206.cpp:1151-1153`). Articulations use the old name table
(`articulationNames`, `read206.cpp:644-692`) and doubled articulations are
merged (`convertDoubleArticulations`). Ties attached to a note are `<Tie>`
(§19.3), not `<Spanner type="Tie">`.

### 19.7 Id-based spanners (voltas, slurs, hairpins, ottavas)

`read206.cpp` pairs a definition carrying `id` with a later `<endSpanner id>`:
`readVolta206` (`:2212`), `readSlur206` (`:2598`), `readHairpin206` (`:2318`),
`readOttava` (`:2286`), and `readMeasure206` (`:2834-2852`) resolves the closing
end. The Rust port collects both halves during `transform`, then `fix_spanners`
builds the model `Spanner`s after the 3.x read, exactly as `fix_ties` does for
ties. Voltas carry `<endings>` and `<endHook>` (`VoltaInfo`); slurs carry `<up>`;
hairpins carry the integer `<subtype>` (1/3 diminuendo, ≥2 dashed line),
`beginText` and `hairpinCircledTip`; ottavas carry `<subtype>` `8va`/`8vb`/
`15ma`/`15mb`. An ottava defined after some chords is anchored on the next
chord, because `readMeasure206` places it at the current tick, not the measure
start. Pedals, trills and old glissandi are still **not** ported.

### 19.8 The two remaining MusicXML differences — resolved or explained

* **Ottavas — fixed.** Before the spanner port, the piano file `Vingt danses
  enfantines` differed from 4.7.4 on 174 notes (one octave). With the 8
  `<Ottava id>` pairs reconnected, both exports write 16 `<octave-shift>`
  elements and the difference drops to 7 notes out of 17 830; the remainder
  comes from ottavas whose extent ends mid-measure (the 2.x `<endSpanner>` can
  fall on a note inside a measure, where our model anchor is less precise).
* **Written pitch of a transposing instrument — explained, not a read loss.**
  `Ad_te_Suspiramus` is a Piccolo part with `+12` chromatic transposition.
  MuseScore 4.7.4 exports it with `<concert-score/>`, i.e. `Sid::concertPitch`
  is true. In the C++ export, `ExportMusicXml::pitch2xml`
  (`exportmusicxml.cpp:1654`) uses `note->concertPitch() ? 0 :
  instr->transpose()`, and `EngravingItem::concertPitch()`
  (`engravingitem.cpp:2043`) reads that style; `read206.cpp:3371` fills it from
  `<displayInConcertPitch>`. scoretk exposes the same choice as
  `ExportOptions::concert_pitch` (`crates/scoretk/src/options.rs:55`, default
  `false`, no CLI flag), so the CLI writes written pitch (D4) where the 4.7.4
  reference writes sounding pitch (D5). A facade caller can set the option to
  match; the reader itself loses no pitch (all 1 139 notes are present).

After these fixes, **19 of the 21** MuseScore 2.x files match 4.7.4 exactly on
the multiset of written pitch/alter/octave/duration; the piano file differs on
only 7 of 17 830 notes (partial ottava extents) and `Ad_te_Suspiramus` keeps the
concert-pitch option difference described above.

### 19.9 Writing oracles on the `.mscz` lot (baseline `4d78e80` → current)

Frozen baseline: CLI built from `git worktree` at `1bcf854^` (= `4d78e80`, the
commit just before the first `read206` port). Same reference MIDI/MusicXML
files and same tolerance/grid for both runs. The `mscz` lot of the LilyPond and
ABC writing oracles has 174 files; to keep the campaign bounded we ran a
**targeted manifest of 31 files**: all 21 MuseScore 2.x files plus 10
deterministically sampled 3.x/4.x controls (every 9th non-2.x file). The MIDI
`ecriture_mscz_musique` lot is complete (8 files).

| Oracle / lot (31 files unless stated) | Tool | Baseline (master before port) | Current | Reading |
| --- | --- | --- | --- | --- |
| MIDI `ecriture_mscz_musique` (8) vs MuseScore MIDI | scoretk | 7/8 OK, 2 exact, N2 52.27 % | 8/8 OK, 2 exact, N2 57.25 % | the one 2.x file goes from hard failure to N2 100 %; the other 7 are byte-identical in score |
| LilyPond `mscz`, reference = MuseScore MIDI via `musicxml2ly` | scoretk | 10/31 compiled, 2 exact, N2d 86.69 % | 28/31 compiled, 3 exact, N2d 58.24 % | 21 previously unreadable files now compile (N2d 0 → 40–100 %); aggregate drops only because they now enter the average |
| LilyPond `mscz` | `musicxml2ly` | 28/31 compiled, 6 exact, N2d 84.59 % | 28/31 compiled, 6 exact, N2d 84.59 % | unchanged control |
| ABC `mscz`, reference = MuseScore MIDI via `xml2abc.py` | scoretk | 10/31 ABC OK, 2 exact, N2d 79.19 % | 31/31 ABC OK, 2 exact, N2d 44.77 % | same pattern: 21 new readable files, lower aggregate mean |
| ABC `mscz` | `xml2abc.py` | 31/31 ABC OK, 3 exact, N2d 45.61 % | 31/31 ABC OK, 3 exact, N2d 45.61 % | unchanged control |

**No per-file loss.** All 10 control files keep exactly the same N2d score in
all three oracles (e.g. `Buenos_dias` 97.93 % in LilyPond and 87.19 % in ABC,
before and after). The 21 2.x files go from `echec` (no output at all) to real
conversions; 2 of them still fail LilyPond compilation (`La_foule`,
`Vingt_danses`) and two ABC/LilyPond conversions show `octave` deviations,
tracked to the concert-pitch export option and to unported pedal/trill
spanners. The aggregate percentage decrease is therefore an expansion of the
denominator, not a regression.

### 19.10 Is porting `read114` worth it?

No. The only version ≤ 114 file in `~/Musique` is
`Partitions/Accordeon/Canon_in_D_Pachelbel_-_arranged_for_accordion.mscz`
(`version="1.14"`, 349 KB of MSCX, 154 measures, 930 chords, 2 staves).
`read114.cpp` is 3 257 lines of additional format conventions (one `<Staff>`
per voice, `<ticklen>`/`<move>` timing, its own tie and text encoding), which
would be a second full reader comparable to `read206` for a single file. It is
left unported and documented here rather than guessed from the 2.x reader.
