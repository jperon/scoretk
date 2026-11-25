# MusicXML 4.0 ↔ scoretk model

Reference documents (outside the repository): `specs/README.md`, section
MusicXML; normative target **MusicXML 4.0**, W3C Music Notation
Community Group final report of 2021-06-01. Reader:
`crates/scoretk-musicxml-import`; writer: `crates/scoretk-musicxml-export`.
Statuses are based on the code, not on the format's theoretical capabilities.

Legend: **supported** = the property is read/written and exercised by the
round-trip tests; **partial** = only a subset of values or a lossy mapping;
**missing** = the reader/writer does not handle it; **not representable in the
model** = the pivot `Score` has no field for it.

| Format property (spec section) | Model type / field (file:line) | Reader | Writer | Notes |
| --- | --- | --- | --- | --- |
| `<score-partwise>`, `<work>`, `<movement-title>`, `<identification>` (`<musicxml-reference>`) | `ScoreMetadata` (`crates/scoretk-core/src/model/score.rs:50-87`), `Score::metadata` (`:118`) | supported (`src/pass2.rs:187`) | supported (`src/exportmusicxml.rs`) | `<creator type="composer\|lyricist\|arranger">` and `<rights>` are mapped individually. |
| `<defaults>`, `<scaling>`, `<page-layout>`, `<page-margins>` | `StyleSettings::page_*`, `TENTHS` (`.../model/style.rs:44-101`) | supported (`src/defaults.rs`) | supported (`src/layout.rs:41`) | Page size, spatium, odd/even margins and credit font sizes are read back (`defaults_style.rs`); `<print>` and per-system layout are still ignored on import (`docs/agents/musicxml-import.md` §5). |
| `<credit>`, `<credit-words>` | `Credit`, `CreditStyle`, `CreditJustify`, `CreditValign` (`.../model/text.rs:445-610`); `Score::credits` (`score.rs:130`) | supported (`src/credits.rs:33`) | supported (`src/exportmusicxml.rs`) | `default-x`/`default-y` positions are not modelled (`text.rs:583-585`); multiple `<credit-words>` are kept in `Credit::extra_words`. |
| `<part-list>`, `<score-part>`, `<part-group>` | `Part` (`.../model/part.rs:200-221`), `Staff` (`:308-333`), `StaffBracket` (`:295-302`) | supported (`src/pass1.rs`) | supported (`src/exportmusicxml.rs:802`) | Braces/brackets/barline spans come from `<part-group>`. |
| `<score-instrument>`, `<midi-instrument>` (`midi-channel`, `midi-program`, `volume`, `pan`) | `Instrument` (`.../model/part.rs:57-88`) | supported (`src/pass1.rs`) | supported (`src/exportmusicxml.rs:2251`) | Channel/program/volume/pan are preserved on the instrument. |
| `<measure>`, `<attributes>/<divisions>` | `Measure` (`.../model/measure.rs:52-101`), `Score::division` (`score.rs:122`) | supported (`src/pass1.rs:834`, `src/pass2.rs:284`) | supported (`src/exportmusicxml.rs:832`) | Real vs nominal measure length (`len` ≠ `nominal_len`) models pickups and irregular bars. |
| `<attributes>/<key>` (`fifths`, `mode`, `print-object`) | `KeySig`, `Key`, `KeyMode` (`.../model/segment.rs:43-129`) | supported (`src/pass2.rs:844`) | supported (`src/exportmusicxml.rs`) | Concert and transposed keys are kept separately; courtesy/visibility flags map to `print-object`. |
| `<attributes>/<time>` (`beats`, `beat-type`, `symbol`), `<time-modification>` | `TimeSig`, `TimeSigType`, `TimeSig::stretch` (`.../model/segment.rs:148-207`) | supported (`src/pass2.rs:872`) | supported (`src/exportmusicxml.rs`) | `symbol="common"/"cut"` → `FourFour`/`AllaBreve`; local stretch is per staff. |
| `<attributes>/<clef>` (`sign`, `line`, `clef-octave-change`) | `Clef`, `ClefType`, `ClefSign` (`.../model/segment.rs:213-406`) | supported (`src/pass2.rs:954`) | supported (`src/exportmusicxml.rs:2129`) | Exotic historic clefs are approximated by the nearest `ClefType` variant. |
| `<attributes>/<staves>`, `<staff-details>/<staff-lines>` | `Staff::lines`, `Staff::time_stretch` (`.../model/part.rs:315-320`) | partial (`src/pass2.rs:822,351`) | supported (`src/exportmusicxml.rs:916,2229`) | `print-object="no"` on `<staff-details>` is lost (hidden staves are layout data). |
| `<note>/<pitch>` (`step`, `alter`, `octave`), `<accidental>` | `Note::pitch`, `tpc`, `tpc_transposed`, `accidental` (`.../model/note.rs:263-293`, `:232-244`) | supported (`src/pass2.rs:1178,1207`) | supported (`src/exportmusicxml.rs:2561,2818`) | `Accidental::user`, `bracket`, `parentheses`, `visible` map to cautionary/editorial/`print-object`. |
| `<note>/<type>`, `<dot>`, `<duration>` | `Duration`, `DurationType` (`.../model/duration.rs:37-164`), `ChordRest` (`.../model/chord.rs:323-348`) | supported (`src/noteduration.rs:109`) | supported (`src/exportmusicxml.rs:2600`) | Note figure and real duration can differ (`ChordRest::ticks`). |
| `<chord>` | `Chord::notes: Vec<NoteId>` (`.../model/chord.rs:216-218`), `Score::notes` (`score.rs:134`) | supported (`src/pass2.rs:552`) | supported (`src/exportmusicxml.rs`) | Notes live in a flat arena; chords hold typed ids. |
| `<voice>`, `<backup>`, `<forward>`, `<staff>` | `Track`, `VOICES` (`.../model/ids.rs`), `Chord::staff_move` (`chord.rs:234`) | supported (`src/pass1.rs`, `src/pass2.rs`) | supported (`src/exportmusicxml.rs:1001`) | MuseScore renumbers voices on import (1,2,3,5 → 1,2,3,4); the original numbering is not restored (`docs/agents/musicxml-import.md` §4). |
| `<beam>` (`begin`/`continue`/`end`, hooks) | `ChordRest::beam_mode`, `beam_direction` (`.../model/chord.rs:337-345`), `BeamMode` (`:45-65`) | supported (`src/pass2.rs:1322`) | supported (`src/exportmusicxml.rs:1841`) | Group direction is inferred from the first `<stem>`; MuseScore `<TimeSig><Groups>` is read into `TimeSig::groups` and drives `Groups::baseBeamMode`; cross-staff beams remain approximate. |
| `<notations>/<tuplet>`, `<time-modification>` (`actual-notes`, `normal-notes`) | `Tuplet` (`.../model/tuplet.rs:53-81`), `ChordRest::tuplet` (`chord.rs:334`) | supported (`src/pass2.rs`) | supported (`src/exportmusicxml.rs:1445`) | Incomplete tuplets that MuseScore fills with rests cannot be reconstructed from `<type>` alone. |
| `<note>/<grace>`, `<cue>` | `GraceType`, `GraceChord` (`.../model/chord.rs:87-104,264-270`) | supported (`src/pass2.rs`) | supported (`src/exportmusicxml.rs`) | Before/after grace notes are both modelled. |
| `<tie>`, `<tied>` | `Note::tie_forward`, `tie_back`, `tie_up`, visibility (`.../model/note.rs:271-291`) | supported (`src/pass2.rs`) | supported (`src/exportmusicxml.rs`) | Direction and invisible tie halves are preserved. |
| `<notations>/<articulations>`, `<ornaments>`, `<fermata>`, `<arpeggiate>`, `<technical><fingering>` | `Articulation` (`.../model/chord.rs:125-210`), `Ornament`/`Fermata`/`Arpeggio`/`Fingering` (`.../model/notation.rs:100-429`) | supported (`src/notations.rs:315,397`) | supported (`src/notations.rs:597-890`) | Other `<technical>` children (bends, taps…) have no model field. |
| `<notations>/<slur>`, `<slide>`, `<glissando>`, `<wavy-line>` | `SpannerType::Slur/Glissando/Trill`, `Spanner`, `SpannerNotation` (`.../model/spanner.rs:268-322`, `notation.rs:362-404`) | supported (`src/notations.rs`) | supported (`src/notations.rs:474-597`) | Slur numbers (`Spanner::number`) disambiguate nested slurs. |
| `<direction>/<dynamics>`, `<sound dynamics="…">` | `Dynamic`, `Annotation::Dynamic` (`.../model/annotation.rs:80-115,197-225`) | supported (`src/direction.rs:687`) | supported (`src/nuances.rs:379`) | `<other-dynamics>` preserved via `Dynamic::custom`. |
| `<direction>/<wedge>` (`crescendo`, `diminuendo`, `niente`) | `SpannerType::Hairpin`, `Hairpin` (`.../model/spanner.rs:272`, `.../model/hairpin.rs:41-58`) | supported (`src/direction.rs:761`) | supported (`src/nuances.rs:187`) | Hairpin text and fonts are kept; `<veloChange>` is not. |
| `<direction>/<words>`, `<rehearsal>`, `<metronome>`, `<sound tempo>` | `StaffText`, `SystemText`, `RehearsalMark`, `Tempo` (`.../model/annotation.rs:231-381`) | supported (`src/direction.rs:151`) | supported (`src/nuances.rs:301`, `src/exportmusicxml.rs`) | Markup subset: bold/italic/underline/strike, explicit font/size; sub/sup and style resets are lossy. |
| `<direction>/<octave-shift>`, `<pedal>`, `<dashes>/<bracket>` | `SpannerType::Ottava/Pedal/TextLine` (`.../model/spanner.rs:273-281`), `PedalInfo` (`:81-94`), `TextLineInfo` (`:216-241`) | supported (`src/direction.rs:797,839,868`) | supported (`src/ottava.rs:110`, `src/pedals.rs:54`, `src/textlines.rs`) | Line hooks and fonts are mapped. |
| `<harmony>` (`root`, `kind`, `bass`, `numeral`, `function`, `degree`) | `Harmony`, `HarmonyDegree` (`.../model/annotation.rs:428-471`) | supported (`src/harmony.rs:40`) | supported (`src/harmony.rs:54`) | `<frame>` guitar diagrams have no model field; unrecognised kinds fall back to text. |
| `<figured-bass>` | `FiguredBass`, `FiguredBassItem` (`.../model/figured_bass.rs:118-165`) | supported (`src/harmony.rs:209`) | supported (`src/figured_bass.rs:115`) | Modifiers and continuation lines are mapped. |
| `<sound><instrument-change>` + `<transpose>` | `InstrumentChange` (`.../model/annotation.rs:400-421`), `Part::instrument_changes` (`.../model/part.rs:218`) | supported (`src/direction.rs:980`) | supported (`src/exportmusicxml.rs`) | The instrument in force is resolved at each note's tick. |
| `<barline>/<repeat>`, `<ending>` | `Measure::start_bar_line`, `end_bar_line`, `repeat_count` (`.../model/measure.rs:63-70`), `BarLineType` (`segment.rs:413-441`), `VoltaInfo` (`repeat.rs:265-270`) | supported (`src/barline.rs:37,136`) | supported (`src/repeats.rs:218-358`) | Multiple `<ending>` elements on one bar are only partly represented. |
| `<direction>/<segno>/<coda>/<fine>`, `<sound><dalsegno>…` | `Marker`, `Jump`, `RepeatMark`, `MarkerType`, `JumpType` (`.../model/repeat.rs:42-257`) | supported (`src/direction.rs:1022`) | supported (`src/repeats.rs:447,518`) | Jump type is inferred from the three target labels. |
| `<measure-style>/<multiple-rest>`, `<measure-repeat>` | `Measure::multi_measure_rest`, `measure_repeat`, `MeasureRepeatMark` (`.../model/measure.rs:41-46,76-87`) | supported (`src/pass2.rs:357,364`) | supported (`src/repeats.rs:175`) | Repeated bars are copied from the source measure in the model. |
| `<lyric>` (`number`, `syllabic`, `extend`, `placement`) | `Lyrics`, `LyricsSyllabic` (`.../model/lyrics.rs:36-63`) | supported (`src/pass2.rs:1292`) | supported (`src/exportmusicxml.rs:2652`) | Multiple melisma blocks are flattened; extended lyric formatting is lossy. |
| `<print>/<system-break>`, `<page-break>`, `<sound><swing>` | `Measure::line_break` (`.../model/measure.rs:95`), `SwingParameters` (`.../model/annotation.rs:272-277`) | partial | supported (`src/layout.rs`, `src/systems.rs`) | Only explicit line breaks and `<swing unit/ratio>` survive; fine layout does not. |
| `<play>`, `<listen>`, `<other-play>`, `<sound><midi-instrument>` playback details | — | missing | missing | No model field for per-note playback instructions or articulation MIDI maps beyond `Instrument::midi_articulations`. |

## Model gaps

Ranked by musical importance (what the pivot `Score` cannot represent today):

1. **Precise layout coordinates** (`default-x`/`default-y`, staff distances,
   system widths): `StyleSettings` only holds page-level dimensions, and
   `Credit` only a generic `vbox_height`; no per-element position exists.
2. **Extended note techniques** (`<technical><bend>`, taps, harmonics,
   string/fingering details): `Note::fingerings` is the only technical field.
3. **Chord diagrams** (`<harmony><frame>`) and the full MusicXML chord-kind
   vocabulary: `Harmony` stores a text/name plus optional root, bass and degrees.
4. **Rich text** (subscript/superscript, nested style resets, full SMuFL glyph
   table): `RichText` keeps a subset (`text.rs:64-77,388-408`).
5. **Playback instructions** (`<play>`, `<listen>`, `<other-play>`,
   `<veloChange>` on hairpins) and the original MusicXML voice numbering.
6. **Multiple volta endings attached to the same bar**: `VoltaInfo::endings`
   is a flat list and `Measure` has one `repeat_count`.
