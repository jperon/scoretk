# MEI 5.1 — condensed reference for coding agents

Target: **MEI 5.1** (`meiversion="5.1+basic"` in files written by MuseScore
4.7.4). Local sources: `specs/README.md` § MEI, `specs/mei/guidelines-index.html`
(chapter list), `specs/mei/introduction.html`, `specs/mei/cmn.html` (Common
Music Notation, the only notation chapter fetched), and the normative schema
`specs/mei/mei-all-5.1.rng` (ECL-2.0, GPL-3 compatible; the guidelines HTML
has **no licence statement** and is private, so quote nothing from it).
Implementation to port: `ref-musescore/.474/src/importexport/mei/`.
Companion: [MEI ↔ pivot model](../format-mapping/mei.md).
Reader/writer planned: `crates/scoretk-mei-{common,import,export}`.

Schema citations below are `<define>` lines of `mei-all-5.1.rng`; guideline
citations are "§x.y" with the line in `cmn.html`.

## 1. Identity, versions, files

- Root element `<mei xmlns="http://www.music-encoding.org/ns/mei"
  meiversion="5.1+basic">`; extension `.mei`, plain UTF-8 XML, no container.
- Two conformance profiles exist: `mei-basic` (small subset, what MuseScore
  reads and writes) and `mei-all` (everything, including mensural, neumes,
  tablature, facsimile). A reader for `mei-all` accepts `mei-basic`, not the
  reverse.
- `mei-all-5.1.rng` is a RelaxNG schema. MuseScore uses the vendored **libmei**
  C++ classes (generated from the schema) to read elements; it does **not**
  validate against the RNG at runtime, and its importer is a **best-effort
  parser**: unknown elements are skipped, not rejected
  (`meiimporter.cpp:1654-1699`, `:2358-2430`). Do the same and report the
  skip.
- Chapters of MEI 5.1: introduction, shared, metadata, CMN, lyrics/performance
  directions, tablature, mensural, neumes, facsimiles, analysis/harmony,
  scholarly editing, text encoding, linking data, integration
  (`guidelines-index.html:1-300`). Only introduction, CMN and the schema are
  local; for tablature, neumes and mensural there is **no local guideline
  text**, only the schema and MuseScore's (absent) support.

## 2. Document skeleton

```
mei
├── meiHead                     (§3)
└── music
    └── body
        └── mdiv
            └── score
                ├── scoreDef     initial parameters (§4)
                └── section*
                    ├── scoreDef intermediate parameter change
                    ├── ending    first/second ending (§5)
                    ├── measure*
                    │   ├── staff* (@n)
                    │   │   └── layer* (@n) → events (§6–§10)
                    │   └── <control events>   (§10)
                    ├── pb / sb   page / system break (§13)
                    └── ...
```

- `score` (`mei-all-5.1.rng:27789`) holds **one** `scoreDef` followed by
  `section`s. Multiple `score`s mean a composite (movements); MuseScore reads
  `./music//score`, i.e. the first one
  (`meiimporter.cpp:1011-1047`).
- A file with `<parts>` instead of `<score>` (parts-only or "CMN with
  `<mdiv>` inside `<parts>`) is legal MEI but **not read** by MuseScore
  (`meiimporter.cpp:1013` selects `score` only).
- Element order is significant for control events: `<measure>` children are
  `<staff>`… then the control events that refer to them; a control event may
  also sit inside a `<layer>`.

## 3. The header: `meiHead`

- `meiHead/fileDesc/titleStmt/title` — first title = main title; further
  `title[@type]` gives `subordinate` → subtitle, any other type → a meta tag
  `<type>Title`. `respStmt/persName[@role]` carries `composer`, `arranger`,
  `lyricist`, `translator`; dedicated `titleStmt/composer|lyricist|arranger`
  are also read. `pubStmt/availability` → copyright; `pubStmt/date[@isodate]`
  is the encoding date. (`meiimporter.cpp:933-1010`; example
  `ref-musescore/.474/src/importexport/mei/tests/data/metadata-01.mei`.)
- MuseScore stores the **whole** `meiHead` verbatim in a meta tag `meiHead`
  and re-emits it on export (`meiimporter.cpp:940-948`,
  `meiexporter.cpp:198-211`). This is the model's only lossless path for
  header data it does not understand.
- `workList`, `encodingDesc`, `sourceDesc`, `revisionDesc` are **not**
  interpreted; they survive only through that verbatim blob. A port without
  the blob must report them as dropped (L6).

## 4. Score definition: `scoreDef`, `staffGrp`, `staffDef`

- `scoreDef` (`mei-all-5.1.rng:27816`) may carry global `@keysig`,
  `@meter.count`, `@meter.unit`, `@meter.sym`, `@ppq`, `@tempo`.
  `staffDef` (`:28050`) carries per-staff `@n`, `@lines`, `@keysig`,
  `@meter.count`/`@meter.unit`, `@trans.diat`/`@trans.semi`, and children
  `<label>`, `<labelAbbr>`, `<clef shape="G" line="2">`, `<instrDef>`.
  `staffGrp` (`:28215`) with `@symbol="bracket"`/`"brace"`, `@bar.thru="true"`
  groups staves into brackets. (§4.2.2 `cmn.html:219-745`.)
- Precedence: a `scoreDef` value overrides the `staffDef` value for the same
  parameter; the importer keeps ordered maps and lets the scoreDef win
  (`meiimporter.cpp:1049-1101` comment "Since the map are ordered, these will
  have priority").
- `@keysig` is a string: `0`, `1s`…`7s`, `1f`…`7f`, plus `x`/`ff` mixtures
  for non-standard signatures (`meiconverter.cpp:2066-2090`). `@meter.sym`
  values `common`/`cut` are `C`/`C|`.
- `staffDef@trans.diat`/`@trans.semi` are the **written→sounding** interval
  (e.g. Horn in F: `-4`/`-7`). MuseScore reads them for the staff's instrument
  transposition (`meiconverter.cpp:3064-3084`,
  `meiimporter.cpp:1052-1101`).
- A `scoreDef` inside a `section` changes parameters from that point
  (`readSectionElements`, `meiimporter.cpp:1346-1393`); it may be empty with
  only one `staffDef`.
- `<instrDef @midi.instrnum>` → MIDI program (`meiimporter.cpp:2849-2866`).

## 5. Measures, sections, endings, repeats

- `measure` (`mei-all-5.1.rng:10640`) is the only rhythmic container; a
  measure-less stream is **not** representable (every degree of the pivot
  expects measures).
- `@n` is a *label*, not necessarily an integer: `meiimporter.cpp:2211-2253`
  only parses it when it starts with a digit and derives `number_offset` from
  the difference to the running count. A missing `@n` marks the measure
  irregular (`@metcon="false"` is the other way to say it).
- `@left`/`@right` are barline renditions (`single`, `dbl`, `end`, `rptstart`,
  `rptend`, `rptboth`, `dotted`, `dashed`, `invis`) → start/end barline and
  repeat flags (`meiconverter.cpp:574-625`, `:2211-2253`).
- A measured repeat's count is a **MuseScore-specific** `@type` value
  `mscore-repeat-N` (`MEASURE_REPEAT_TYPE "mscore-repeat-"`,
  `meiconverter.h:80`; read by `getTypeValuesWithPrefix`), not standard MEI.
  A port must keep it as an extension and report it as such.
- `section` (`:27857`) is a structural wrapper; MuseScore ignores it except
  for layout. `ending` (`:26411`) with `@n="1,2"` wraps the measures of a
  volta (`meiimporter.cpp:1394-1433`); the volta ends at the next measure
  outside the ending.
- `<mRpt>` inside a layer is a one-measure repeat symbol
  (`meiimporter.cpp:2010-2039`); it does not expand.

## 6. Staves, layers, events

- `staff` (`:28007`) is bound by `@n`, **not** by document order; the importer
  builds an `MEI staff@n → pivot staff index` map and falls back to order only
  for files without `@n` (`buildStaffLayerMap`,
  `meiimporter.cpp:3309-3347`).
- `layer` (`:26877`) is "an independent stream of events on a staff"; `@n`
  1-based. In MuseScore's model a layer is a **voice** (`VOICE` 1–4), and
  `@n` maps to `voice - 1` (`getVoiceIndex`, `meiimporter.cpp:224-241`).
  More than four layers cannot be represented by MuseScore; report them.
- Events are read in document order and their **ticks are implicit**: each
  event is placed at the current cursor and advances it by its duration
  (`meiimporter.cpp:1654-1699`, `:242-379`).
- `@tstamp` (1-based in **beats of the current meter**) and `@dur` can place
  an event absolutely. MuseScore converts with
  `tstampFromFraction`/`tstampToFraction` (`meiconverter.cpp:3828-3878`),
  which treats `@tstamp` as a floating-point beat index and approximates the
  fraction by continued fractions (precision 0.01). For `<dir>`, `<dynam>`,
  `<hairpin>`… this is the only position; a `@tstamp="0"` is clamped to the
  first event.
- `<space>` (`:27923`) is an invisible rhythmic placeholder: it advances time
  but emits no symbol (`meiimporter.cpp:2160-2178`).

## 7. Pitches, durations, accidentals

- `<note>` (`:27202`) is leaf of a layer; `@pname` ∈ `c d e f g a b` and
  `@oct` are the written pitch. **Middle C is `@pname="c" @oct="3"`**
  (`pitch = step + alter + (oct + 1) * 12`, `meiconverter.cpp:2765-2819`).
- `@dur` (`:2916-2959`) is a **power-of-two denominator**: `1` whole, `2`
  half, `4` quarter, `8` eighth…, plus `breve`, `long`, `maxima`, `0` (=
  "no duration", used by grace notes). This is *not* MusicXML `<duration>`
  divisions and *not* a reciprocal.
- `@dots` is an integer count (`0`–`3` legal, more is unusual).
- `@accid` is the **written** accidental (`s`, `f`, `ss`, `ff`, `x`, `xs`,
  `n`, `nf`…); `@accid.ges` is the **gestural** (sounding) accidental used
  when the written one is absent (`meiconverter.cpp:160-232`). `@enclose`
  = `brack`/`paren` for cautionary accidentals. `@func` distinguishes
  editorial/cautionary roles in MEI; MuseScore only keeps bracket/parenthesis.
- `@oct.ges` / `@pitch` carry the sounding pitch when it differs from the
  written one (octave-transposing instruments, scordatura). MuseScore exports
  `@oct.ges` when `ppitch` differs (`meiconverter.cpp:2820-2874`) but does
  **not** import it.

## 8. Chords, rests, spaces

- `<chord>` (`:25872`) = two or more notes **of the same duration** on one
  layer; `@dur`/`@dots` live on the `<chord>`, individual `<note>`s usually
  carry only `@pname`/`@oct`/`@accid`. A `<chord>` with one `<note>` is legal.
  (`meiimporter.cpp:1835-1878`.)
- A bare `<note>` **is** a one-note chord; the importer creates the chord
  itself (`meiimporter.cpp:2040-2138`).
- `<rest>` (`:27713`) takes `@dur`/`@dots`; `@visible="false"` gives an
  invisible rest. `<mRest>` (`:10722`) is a whole-measure rest of the
  measure's actual length, `<mRpt>` a repeat glyph. `<space>` is not a rest
  and must not be exported as one.

## 9. Beams, tremolos, tuplets

- A `<beam>` container (`:10235`) groups consecutive events. MuseScore also
  understands a non-MEI-Basic short form: `@type="mscore-beam-begin"`,
  `"mscore-beam-mid"`, `"mscore-beam-none"` on the event, for partial beams
  (`beamFromMEI`, `meiconverter.cpp:626-676`).
- `@breaksec` on the event breaks the secondary beam level; value `1` =
  `BEGIN16`, `2` = `BEGIN32` (`breaksecFromMEI`, `:662-691`). Any other value
  is a warning.
- `<tuplet>` (`:11105`) is a container with `@num`/`@numbase` = ratio
  `num/numbase` (e.g. `3`/`2`), `@num.visible`, `@num.format="ratio"`,
  `@bracket.visible`, `@bracket.place`/`@num.place`
  (`tupletFromMEI`, `:3595-3670`). **Nested tuplets are rejected** by the 4.7.4
  importer (`meiimporter.cpp:2200-2214`) — port that limitation or report it.
- `<bTrem>` (`:10389`) = single-note tremolo, `@unitdur` is the sounding
  subdivision; `<fTrem>` (`:10429`) = two-chord tremolo, its two `<chord>`s
  are written with **double** the sounding value and the importer halves them
  (`meiimporter.cpp:242-379` comment and `readChord`).

## 10. Control events

All of these may sit after the `<staff>`s of a `<measure>` (or inside a
`<layer>`), and identify their anchors by `@startid`/`@endid` (an `xml:id`
reference) or by `@tstamp`/`@tstamp2`/`@dur`. The importer dispatches on the
element name in `readControlEvents` (`meiimporter.cpp:2358-2430`) and
`addSpanner`/`addAnnotation` (`:451-603`).

| Element | Meaning | MuseScore mapping |
| --- | --- | --- |
| `<tie>` | tie start→end by `@startid`/`@endid` | `Note::tie_forward` (`meiconverter.cpp:3526-3544`, `meiimporter.cpp:3111-3146`) |
| `<slur>` | slur, `@curvedir` | `SpannerType::Slur` (`:2997-3052`) |
| `<phrase>` | phrase mark | parsed as a slur (`meiimporter.cpp:2405`) |
| `<dynam>` | dynamic, text + `@label` | `Dynamic` (`:1275-1405`) |
| `<dir>` | direction/expression; `@extender` + `<rend>`/`<lb/>` text | `StaffText`/`SystemText`/`TextLine` by `@type` (`mscore-staff-text`, `mscore-system-text`, `mscore-hairpin`; `:1133-1274`) |
| `<tempo>` | tempo text, `@midi.bpm`, `@mm` | `Tempo` (`:3327-3390`) |
| `<reh>` | rehearsal mark | `RehearsalMark` (`meiimporter.cpp:2998-3028`) |
| `<hairpin>` | crescendo/diminuendo, `@form="cres"`/`"dim"` | `Hairpin` (`:1730-1799`) |
| `<pedal>` | sustain pedal span | `SpannerType::Pedal` (`:2693-2764`) |
| `<octave>` | 8va/8vb, `@dis`/`@dis.place` | `Ottava` (`:2462-2567`) |
| `<trill>`, `<mordent>`, `<turn>`, `<ornam>` | ornaments | `Ornament`/`Trill` (`:2335-2692`, `:3545-3730`) |
| `<artic>` | articulation, `@artic` (`stacc`, `acc`, `ten`, `marc`, `spicc`…) | `Articulation` (`:306-573`) |
| `<arpeg>` | arpeggio, `@arpeg` up/down | `Arpeggio` (`:254-305`) |
| `<fermata>` | fermata, `@form` | `Fermata` (`:1517-1613`) |
| `<breath>`, `<caesura>` | breath / caesura | `Breath` (`:692-809`) |
| `<gliss>` | glissando, `@lform` | `Glissando` (`:1654-1699`) |
| `<fing>` | fingering, `@place` | `Fingering` (`:1614-1653`) |
| `<harm>` | chord symbol, `<rend>` text | `Harmony` (`:1800-1863`) |
| `<fb>` + `<f>` | figured bass | `FiguredBass` (`:1471-1516`) |
| `<harpPedal>` | harp pedal diagram | not in the pivot (L6) |
| `<lv>` | laissez vibrer | parsed as a `LaissezVib` symbol (`:1962-1981`) |
| `<repeatMark>` | segno/coda/DC/DS/Fine | `Marker`/`Jump` (`:1982-2065`, `:2117-2210`) |
| `<annot>` | editorial annotation | ignored |

- A control event with neither `@startid` nor `@tstamp` is anchored by
  `findStart()` scanning forward for the first event at or after the current
  position (`meiimporter.cpp:620-741`); this is why *order* matters.
- `<dir>` and `<dynam>` text may contain `<lb/>`, `<rend>`, SMuFL glyphs in
  `<symbol>`; the pivot's `RichText` (text.rs:115) can hold the plain text and
  line breaks only.

## 11. Lyrics

- `<verse n="1">` is a child of the **note or chord it starts on**
  (`:16474`); inside it, `<syl>` (`:28260`) is one syllable with `@con`
  (syllabic: `d` disconnect, `u` begin, `m` middle, `t` end), optional
  `<rend>` styling and optional `<elision>` between syllables.
- A syllable is extended over following notes either by a following
  `<verse>`-less note plus `@wordpos="i"`/`"m"`/`"t"`, or by the exporter's
  convention of empty `<syl>`s; MuseScore's importer remembers the last
  lyric and extends it until the next syllable
  (`hasLyricsToExtend`/`extendLyrics`, `meiimporter.cpp:828-890`).
- Verses are numbered globally (`@n`), not per system.

## 12. Written vs gestural/perform time

- `@dur.ges` (sounding duration), `@tstamp.ges`, `@tstamp.real`
  (`cmn.html:1140`, `:3925`) and `@ppq` (pulses per quarter, schema) describe
  how the music *sounds*, independently of how it is written. The pivot has
  only one time axis (`ChordRest::duration` + `ticks`, chord.rs:341-363);
  this is the known model gap (see the mapping document).
- `@oct.ges`, `@accid.ges` and `@pitch` are the same distinction at the note
  level.

## 13. Layout, linking, facsimile (mostly L5/L6)

- `<pb>`/`<sb>` are page/system breaks; MuseScore imports only those with a
  user `@type` (`meiimporter.cpp:1502-1543`) and exports them when the
  "layout" option is on (`mei/internal/meiconfiguration.cpp:28-45,
  59-66`).
- `<pgHead>` is the first-page header block (`:27407`), written into a
  MuseScore `VBox` (`meiexporter.cpp:386-459`).
- `xml:id` is optional; MuseScore generates ids on export (`meiUseMuseScoreIds`
  option, default false). `@copyof`, `@sameas`, `@facs`, `<facsimile>`,
  `<expansion>`, `<timeline>` are **not** read.
- `@color` is read/written on most objects (`meiconverter.cpp:1048-1111`).

## Traps that silently break data

1. `@dur` is a denominator (`4` = quarter), not a reciprocal count and not
   MusicXML divisions; a factor-of-4 error shifts every rhythm. (§7)
2. Middle C is `oct="3"`, while MusicXML octave 4 and Capella `C5` both mean
   the same pitch — three different conventions in one pipeline. (§7)
3. `@tstamp` is **1-based** and measured in meter beats, not ticks; `0` is
   clamped. (§6)
4. Staff/layer membership comes from `@n`, not document order; assuming order
   misplaces voices when a staff is omitted. (§6)
5. A `<note>` is a chord of one; only `<chord>` carries `@dur` for several
   notes. Putting `@dur` on each note of a chord is legal MEI but changes the
   reading if the durations differ. (§8)
6. `@accid` is written, `@accid.ges` is sounding; ignoring `.ges` loses
   accidentals of transposing or scordatura parts, trusting it over `@accid`
   loses cautionary accidentals. (§7)
7. Control events are anchored by id or tstamp; reordering measures without
   rewriting ids silently reattaches slurs. (§10)
8. `@type`-encoded beams (`mscore-beam-begin`/`-mid`/`-none`) coexist with
   `<beam>` containers; read both or beamed groups split. (§9)
9. Nested tuplets are rejected by the 4.7.4 importer; a port that accepts
   them will diverge from the oracle. (§9)
10. The `<meiHead>` is opaque to MuseScore and round-trips verbatim; a port
    that parses only known metadata fields loses `workList`, sources and
    revision history. (§3)
11. `mei-basic` files written by MuseScore put control events *after* the
    staves; a streaming reader that stops at `</staff>` loses every slur.
    (§2, §10)

## Minimal valid example

```xml
<?xml version="1.0" encoding="UTF-8"?>
<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">
  <meiHead>
    <fileDesc>
      <titleStmt><title type="main">Tiny</title>
        <respStmt><persName role="composer">Anon</persName></respStmt>
      </titleStmt>
      <pubStmt><date isodate="2026-01-01"/></pubStmt>
    </fileDesc>
  </meiHead>
  <music><body><mdiv><score>
    <scoreDef>
      <staffGrp>
        <staffDef n="1" lines="5" meter.count="4" meter.unit="4" keysig="1s">
          <label>Oboe</label><labelAbbr>Ob.</labelAbbr>
          <clef shape="G" line="2"/>
        </staffDef>
      </staffGrp>
    </scoreDef>
    <section>
      <measure n="1">
        <staff n="1"><layer n="1">
          <beam>
            <note xml:id="n1" dur="8" pname="g" oct="4"/>
            <note xml:id="n2" dur="8" pname="a" oct="4"/>
          </beam>
          <chord xml:id="c1" dur="2">
            <note pname="c" oct="5"/><note pname="e" oct="5"/><note pname="g" oct="5"/>
          </chord>
          <rest dur="4"/>
        </layer></staff>
        <slur startid="#n1" endid="#n2"/>
        <dynam staff="1" startid="#c1" place="below">p</dynam>
      </measure>
    </section>
  </score></mdiv></body></music>
</mei>
```
Bar 1 = 4/4: two eighths (`@dur="8"`, 1/4 total) + a half-note chord
(`@dur="2"`, 1/2) + a quarter rest (`@dur="4"`, 1/4) = 1 whole note. Only
control events (`<slur>`, `<dynam>`) follow the `<staff>`s of a measure;
everything rhythmic is inside a `<layer>`.
