# Guitar Pro (`.gp3`–`.gp7`, `.gpx`) — condensed reference for coding agents

Target: what MuseScore **4.7.4** imports. **No public specification exists**
for GP3–GP5 (binary) or GP6/GP7 (`GPIF` XML). `specs/README.md` § Guitar Pro
records the absence; the reference is the importer:

- `ref-musescore/.474/src/importexport/guitarpro/internal/gtp/*.h` — the
  in-memory model of the format, shared by all versions;
- `importgtp.cpp` — version sniffing, GP3–GP5 binary reader, PowerTab
  (`ptab`) dispatch, `convertGTP`;
- `importgtp-gp3/4/5.cpp` — per-version binary fields;
- `gtp/gp67dombuilder.cpp` — GP6/GP7 `GPIF` XML → the same model;
- `gtp/gpconverter.cpp` — the model → MuseScore score conversion;
- `tests/data/` — ≈250 fixture inputs (`.gp3/.gp4/.gp5/.gp/.gpx`) with 4.7.4
  `*-ref.mscx` references.

Upstream GPL-3.0-only; `ref-musescore/` is read-only and unversioned.
Companion: [Guitar Pro ↔ pivot model](../format-mapping/guitarpro.md).
Planned reader only: `crates/scoretk-guitarpro-{common,import}`.

## 1. Identity and versions

Dispatch is by **content**, not suffix (`importGTP`, `importgtp.cpp:2798-2870`):

| First bytes | Format | Reader class |
| --- | --- | --- |
| `ptab` | PowerTab (not Guitar Pro) | `PowerTab`, `importptb.cpp` |
| `PK\x03\x04` | ZIP container, GP7 | `GuitarPro7` (`importgtp-gp7.cpp:45-59`) |
| `BCFZ` | GP6 custom compressed container (`.gpx`) | `GuitarPro6` (`importgtp-gp6.cpp:400-465`) |
| `\0FIC…` / `xxFIC…` | GP3–GP5 binary; the string after the first byte starts with `FICHIER GUITAR PRO ` or `FICHIER GUITARE PRO ` | `GuitarPro3/4/5` |

- The old binary header carries the version as text, `X.xx`; versions `1`–`5`
  map to `GuitarPro1..5` (`importgtp.cpp:2826-2856`). Version 1/2 files are
  read through dedicated subclasses of the 3/4 readers.
- GP7 `.gp` is a ZIP with `Content/score.gpif` (the score XML) and
  `Content/PartConfiguration` (a binary block giving number of instruments and
  their sound banks, `GuitarPro7::read`, `importgtp-gp7.cpp:45-59`).
- GP6 `.gpx` is a **BCFS** container: a bit-packed LZ stream (or an
  uncompressed sector image) holding named files, of which the `.gpif` XML is
  parsed (`GuitarPro6::readGPX`, `importgtp-gp6.cpp:400-465`). It is neither
  ZIP nor zlib; do not try `unzip`.
- The importer accepts a file whose extension is wrong. A port must sniff the
  same signatures; suffix detection alone is not sufficient.

## 2. The shared in-memory model

All versions build the same classes, which are the best available description
of the format's semantics:

```
GPScore            title, subtitle, artist, album, poet, composer, multiVoice
GPMasterTracks     tempo automations (bar, position, value, linear), track count
  GPMasterBar      timeSig, keySig(+useFlats), repeat, alternateEndings,
                   tripletFeel, fermatas, section, directions, barlineType,
                   freeTime, bars[]
    GPBar          clef, simileMark, voices[]
      GPVoice      id, pos (track index), beats[]
        GPBeat     notes[], rhythm (GPRhythm), dynamic, legato, ottava,
                   letRing, palmMute, trill, tappingHand, slapped, popped,
                   harmonicMark*, arpeggio(+stretch), brush, graceNotes,
                   freeText, time, vibratoWTremBar, vibratoLeftHand, fadding,
                   hairpin, rasgueado, pickStroke, tremolo, wah, golpe,
                   barre(fret,string), dive, pickScrape, deadSlapped,
                   stemOrientation, beamMode, lyrics, diagramIdx
          GPNote   midiPitch(midi,string,fret,octave,tone,element,variation),
                   accidental, harmonic, letRing, palmMute, tie, bend,
                   ghostNote, muted(dead), rightHandTapping, leftHandTapped,
                   accents, slides, fingerings, showStringNumber, vibrato,
                   trill, ornament, hammerOn, pickScrape
GPRhythm           rhythm (64th…whole), dotCount, tuplet(num, denom)
GPTrack            name, shortName, instrument, RSE(volume,pan), programm,
                   midiChannel, isGuitar, staffProperty[](tuning, capoFret,
                   fretCount, useFlats, ignoreFlats), staffCount, transpose,
                   diagrams[], lyrics, lineCount
```

(`gpscore.h`, `gpmastertracks.h`, `gpmasterbar.h`, `gpbar.h`, `gpvoice.h`,
`gpbeat.h:20-313`, `gpnote.h:20-195`, `gprhythm.h:20-36`, `gptrack.h:20-177`.)

- A **beat** is the rhythmic unit (chord + rhythm), a **note** is one string
  of that beat; a beat with no notes is a rest (`GPBeat::isRest`,
  `gpbeat.h`). A **voice** is one GP track's voice inside a bar; a bar may
  carry several voices. A **master bar** is the global bar (meter, key,
  repeat, endings) that points at the per-track bars.
- This model is *not* sequential: `GPBeat::time` may place a beat at an
  absolute position in the bar (used for grace-before-beat and incomplete
  measures). The converter sorts and fills gaps (`convertBeat`,
  `gpconverter.cpp:545-648`, `fixEmptyMeasures`, `:363-424`).

## 3. GP3–GP5 binary layout

There is no published spec; the field order is exactly the order of the read
functions. Read them in this order:

1. `GuitarPro::readInfo` (`importgtp-gp5.cpp:95-116`): title, subtitle,
   artist, album, lyricist, composer, copyright, tab author, instructions;
   then the lyrics lines (GP5 keeps five), RSE/volume data, tempo, key.
2. `readTracks` (`:540-671`): per track — flags, name, string count,
   **tuning per string**, port/channel/instrument/volume/pan, capo, colour,
   then chord diagrams (GP4+) and the 5-line lyrics block.
3. `readMeasures` (`:675-...`): per bar — flags (new time signature, new key,
   repeat, alternate ending, new tempo, double bar), the values, then
   per-voice beats.
4. `readBeat` (`:193-448`): duration byte (low bits = note value, dots and
   tuplet in the high bits), beat effects (`readBeatEffects`, `:120-190`),
   then `readNote` per string.
5. `readNote` / `readNoteEffects` (`:1347-1470`, `:1071-...`): string, fret,
   velocity, tie, then two effect bit masks.

Key constants (`importgtp.h:176-203`):

| Mask | GP5 meaning |
| --- | --- |
| `EFFECT_BEND 0x1`, `EFFECT_HAMMER 0x2`, `EFFECT_SLIDE_OLD 0x4`, `EFFECT_SLIDE 0x8`, `EFFECT_LET_RING 0x8`, `EFFECT_GRACE 0x10`, `EFFECT_TRILL 0x20`, `EFFECT_VIBRATO 0x40` | note effects (`modMask1`) |
| `EFFECT_STACCATO 0x1`, `EFFECT_PALM_MUTE 0x2`, `EFFECT_TREMOLO 0x4`, `EFFECT_ARTIFICIAL_HARMONIC 0x10`, `EFFECT_GHOST 0x01` | same byte, different meanings by version — **never reuse one mask table across versions** |
| `NOTE_GHOST 0x04`, `NOTE_DEAD 0x20`, `NOTE_SLUR 0x08`, `NOTE_MARCATO 0x02`, `NOTE_SFORZATO 0x40`, `NOTE_FINGERING 0x80`, `NOTE_DYNAMIC 0x10`, `NOTE_FRET 0x20` | note flags |

- Bend data is a sub-record read by `readBendDataFromFile`
  (`importgtp.cpp:609-627`): `icon`, `amplitude`, `numPoints`, then per point
  `time` (0–60), `pitch` (100 = one semitone), `vibrato`. These are already
  MuseScore `PitchValue` units (`engraving/types/pitchvalue.h:36-49`).
- Grace notes are an effect block on the note, not separate notes
  (`EFFECT_GRACE`, `importgtp-gp5.cpp:1090-1130`); GP5 supports one grace note
  per note ("advertised" as an eighth).
- Multi-voice bars: the voice count and per-voice beat counts are explicit in
  the binary; `GPBar`/`GPVoice` keep them.

## 4. GP6/GP7 `GPIF` XML

- GP7: `Content/score.gpif` inside a ZIP; GP6: the same XML inside the BCFS
  container. Encoding UTF-8; root `<GPIF>` with `<GPIFVersion>`.
- Structure, from `gp67dombuilder.cpp`:

```
GPIF
├── Score
│   ├── Title, Subtitle, Artist, Album, Words, Music, Copyright, Tab
│   ├── MasterTrack
│   │   └── Automations/Automation (Linear, Type, Bar, Position, Value, Text)
│   ├── Tracks/Track (@id)
│   │   ├── Name, ShortName, GeneralMidi/Program, RSE/…
│   │   ├── Properties/Property (name="Tuning"…, "CapoFret", "FretCount")
│   │   ├── Instrument/Tuning (@string)…
│   │   ├── Diagrams/Diagram (Frets, FretCount, BaseFret, Strings)
│   │   └── Lyrics/Lines/Line, Staves/Staff/Properties…
│   ├── MasterBars/MasterBar (@id)
│   │   ├── Time, Key, Repeat, AlternateEndings, Bars (indices),
│   │   ├── TripletFeel, Fermatas, Section, Directions, DoubleBar, FreeTime
│   ├── Bars/Bar (@id) → Clef, Voices (indices), SimileMark, …
│   ├── Voices/Voice (@id) → Beats (indices)
│   ├── Beats/Beat (@id)
│   │   ├── Rhythm (ref), Notes (indices), Dynamic, Legato (@origin,@destination),
│   │   ├── Properties/Property (name="Brush"…), XProperties/Property (@id)
│   └── Notes/Note (@id)
│       ├── Accidental, Tie (@origin,@destination), LetRing, …
│       └── Properties/Property name="Midi|String|Fret|Tone|Octave|Bended|
│                              Harmonic|PalmMuted|Muted|Slide|HopoOrigin|Tapped|…"
└── Rhythms/Rhythm (@id) → NoteValue, PrimaryTuplet/@num,@den, AugmentationDots
```

- Indices are **space-separated strings** of ids (`"0 1 4"`), and objects are
  resolved after the whole document is read (`buildGPDomModel` and
  `createGP*`, `gp67dombuilder.cpp:17-68`, `:74-270`). A streaming reader
  cannot resolve them.
- `Property` entries are `name`-keyed with a typed child: booleans use
  `<Enable/>`, numbers/strings use text content (`readNoteProperties`,
  `:910-1013`; `readBeatProperties`, `:1110-1217`).
- Ties and legato are **origin/destination** pairs (`Tie @origin @destination`,
  `Legato @origin @destination`), turned into a start/middle/end state
  (`createGPNote`, `:686-...`).
- `XProperties` are numeric-id properties; the only one used is `688062467`
  = trill speed (`readNoteXProperties`, `:895-908`). Unknown ids must be
  skipped, not treated as errors.
- `MasterBar/Directions` carry repeats, jumps and markers; `Section` carries
  rehearsal names; `AlternateEndings` numbers the voltas
  (`readEnding`, `:1303-1315`; `readRepeatsJumps`).

## 5. Pitch, string, tuning, capo

- A note always has both a **MIDI pitch** and, for fretted instruments, a
  `(string, fret)` pair. `string` is **0-based from the highest-pitched
  string** in GP (`String` property, `GPNote::string`); MuseScore reverses it:
  `musescoreString = stringCount - 1 - gpString`
  (`setPitch`, `gpconverter.cpp:2070-2123`). Getting this wrong mirrors every
  tablature.
- Tunings are MIDI note numbers, low string first in `GPTrack::staffProperty`
  `tunning` (standard `{40,45,50,55,59,64}` = E2 A2 D3 G3 B3 E4,
  `gpconverter.cpp:1140-1160`).
- `capoFret` is stored on the part (`setCapoFret`), and `transpose` is
  subtracted from every tuning string before building `StringData`
  (`gpconverter.cpp:1142-1160`).
- `fretCount` (usually 24) and `useFlats`/`ignoreFlats` choose the preset
  spelling table (`flatPresets`).
- Percussion tracks have no strings: `midiChannel == PERC_CHANNEL` selects a
  drum staff and the element/variation pair is resolved through
  `calculateDrumPitch` (`gpconverter.cpp:2152-2170`) and the drumset tables.

## 6. Rhythm

- `GPRhythm` = note value (64th…whole) + dot count + optional tuplet
  `(num, denom)` (`gprhythm.h:20-36`). There is no breve/longa; the longest
  value is a whole note.
- The binary encodes the same in a duration byte (note value in the low bits,
  dots/tuplet in flags) — read it through `readBeat`/`GPRhythm`, not by hand.
- `GPBeat::time` (GP6/7) is an absolute position in the bar for beats that are
  not sequential (grace-before-beat, incomplete measures); `-1` means "append".
- `freeTime` master bars have no meter constraint (`GPMasterBar::freeTime`).
- A `BeamMode` per beat (`AUTO`, `JOINED`, `BROKEN`, `BROKEN2`,
  `BROKEN2_JOINED`) plus a per-beat user stem orientation.

## 7. Note-level effects

| GP concept | GP5 effect | GP6/7 property | MuseScore element |
| --- | --- | --- | --- |
| bend / pre-bend / release | `EFFECT_BEND` + bend points | `Property name="Bended"` | `GuitarBend` (MuseScore 4.7) or `TremoloBar` |
| harmonic (natural, artificial, pinch, tap, semi, feedback) | `EFFECT_ARTIFICIAL_HARMONIC`, harmonic block | `Harmonic`, `HarmonicFret`, `HarmonicType` | harmonic note-head + `HarmonicMark` |
| let ring | `EFFECT_LET_RING` | `LetRing` | `LetRing` line |
| palm mute | `EFFECT_PALM_MUTE` | `PalmMuted` | `PalmMute` line |
| dead note | `NOTE_DEAD` | `Muted` | dead note head |
| ghost note | `NOTE_GHOST` | (accidental/shape) | ghost note head |
| slide in/out, legato, shift | `EFFECT_SLIDE` / `EFFECT_SLIDE_OLD` bitfields | `Slide` (64/128 = pick scrape) | `Glissando`/`Slide` spanner |
| hammer-on / pull-off | `EFFECT_HAMMER` | `HopoOrigin` + destination | `HammerOnPullOff` |
| vibrato | `EFFECT_VIBRATO` | (beat) `VibratoWTremBar`, note vibrato | `Vibrato` |
| trill | `EFFECT_TRILL` | `XProperties` trill speed | `Trill` spanner |
| mordent / turn | — | `Ornament` | `Ornament` |
| tapping (right/left hand) | — | `Tapped`, `LeftHandTapped` | text + playing technique |
| pick scrape | `Slide` 64/128 | `PickScrape` | `PickScrape` |
| accents / marcato / sforzato / staccato | `NOTE_MARCATO`, `NOTE_SFORZATO`, `EFFECT_STACCATO` | `Accentuation` bits | `Articulation` |
| fingering (left/right) | `NOTE_FINGERING` | `LeftFingering`, `RightFingering` | `Fingering` |
| string number | — | `ShowStringNumber` | circled `Fingering` |
| tie | note flag | `Tie @origin @destination` | `Note::tie_forward/back` |

## 8. Beat-level effects

Dynamic (`PPP`…`FFF`), hairpin (crescendo/decrescendo; GP7 also volume swell),
fade in/out, arpeggio (up/down + stretch), brush (up/down), tremolo
(numerator/denominator), pick stroke (up/down), wah (open/closed), golpe
(finger/thumb), rasgueado (18 patterns), slap/pop, barre (fret+string),
ottava (`8va/8vb/15ma/15mb`), grace notes (on-beat/before-beat), free text,
timer, chord diagram reference, lyrics per beat. (`gpbeat.h:20-313`;
`gpconverter.cpp:588-648` calls one `add*` per effect.)

## 9. Tracks, parts, instruments

- A GP **track** becomes a MuseScore `Part` with `staffCount` staves; two
  staves get a brace and a barline span (`setUpTrack`,
  `gpconverter.cpp:1065-1177`).
- `RSE` volume/pan are floats 0–1 scaled to 0–127; `programm` is the MIDI
  program; `midiChannel` selects percussion when it is the percussion channel.
- `instrument` is a GP sound identifier (`"a-bass4"`, `"2Mrcs"`…) mapped
  through `GuitarPro6::instrumentMapping` (`importgtp-gp6.cpp:78-...`); it is
  a *name*, not a GM program.
- Chord diagrams (`Diagram`: `stringCount`, `fretCount`, `baseFret`,
  `frets{string→fret}`) are MuseScore `FretDiagram` objects — **not modelled**
  by the pivot.

## Traps that silently break data

1. Dispatch is by content, not extension: `ptab`, `PK`, `BCFZ`, `FIC…`.
   (`.gp` is a ZIP, `.gpx` is BCFS, not zlib.) (§1)
2. GP string numbering is reversed relative to MuseScore's `StringData`
   (`strings - 1 - gpString`); a wrong sign mirrors the whole tablature. (§5)
3. Tuning is stored as MIDI numbers with the **lowest string first** in the
   GP model, but `String` is 0-based **from the highest**. Both conventions
   coexist in the same file. (§5)
4. `transpose` is subtracted from tunings before `StringData`; capo is stored
   separately. Applying transpose twice detunes every note. (§5)
5. Effect bit masks have **different meanings per version** for the same bit
   (`EFFECT_PALM_MUTE` and `EFFECT_HAMMER` are both `0x2`). (§3)
6. Bend points are `(time 0–60, pitch 100 = semitone)` in GP3–5, but GP6/7 XML
   stores **percentages** 0–100 for time that the converter scales
   (`gpTimeToMuTime`); applying one convention to the other doubles or halves
   the curve. (§3, §7)
7. GP6/7 object ids are indices resolved after the whole document; a streaming
   reader loses them. (§4)
8. `Vibrato`, `trill`, `ornament` exist both at note and beat level; duplicate
   them and MuseScore drops one. (§7, §8)
9. Grace notes are fabricated by the reader (a real note plus an adjustment of
   the previous note); they are not in the GP note list. (§3)
10. A beat with no notes is a rest, not an empty chord; `GPBeat::isRest`
    checks `notes.empty()`. (§2)
11. `FreeTime` bars have no meter: assuming the previous time signature
    creates wrong measure lengths. (§6)
12. PowerTab (`.ptb`, signature `ptab`) is not Guitar Pro; it is read by
    `importptb.cpp` into the same model. Do not feed it to the GP reader. (§1)

## Minimal valid example

GPIF (GP7) is the only container that can be shown as text; this is the shape
the builder expects, not a complete file:

```xml
<?xml version="1.0" encoding="utf-8"?>
<GPIF>
  <GPIFVersion>7.0</GPIFVersion>
  <Score>
    <Title>Tiny</Title>
    <MasterTrack>
      <Automations><Automation><Type>Tempo</Type><Bar>0</Bar>
        <Position>0</Position><Value>120</Value></Automation></Automations>
    </MasterTrack>
    <Tracks><Track id="0">
      <Name>Guitar</Name>
      <GeneralMidi><Program>25</Program></GeneralMidi>
      <Properties>
        <Property name="Tuning"><String><Pitch>64</Pitch></String></Property>
        <Property name="CapoFret"><Number>0</Number></Property>
      </Properties>
    </Track></Tracks>
    <MasterBars><MasterBar id="0">
      <Time>4/4</Time><Key><AccidentalCount>0</AccidentalCount></Key>
      <Bars>0</Bars>
    </MasterBar></MasterBars>
    <Bars><Bar id="0"><Clef>G2</Clef><Voices>0</Voices></Bar></Bars>
    <Voices><Voice id="0"><Beats>0 1</Beats></Voice></Voices>
    <Beats>
      <Beat id="0"><Rhythm ref="0"/><Notes>0</Notes><Dynamic>MF</Dynamic></Beat>
      <Beat id="1"><Rhythm ref="0"/><Notes>1</Notes><Dynamic>MF</Dynamic></Beat>
    </Beats>
    <Notes>
      <Note id="0"><Properties>
        <Property name="Midi"><Number>64</Number></Property>
        <Property name="String"><Number>0</Number></Property>
        <Property name="Fret"><Number>0</Number></Property>
      </Properties></Note>
      <Note id="1"><Tie origin="true" destination="false"/><Properties>
        <Property name="Midi"><Number>64</Number></Property>
        <Property name="String"><Number>0</Number></Property>
        <Property name="Fret"><Number>0</Number></Property>
      </Properties></Note>
    </Notes>
    <Rhythms><Rhythm id="0"><NoteValue>Quarter</NoteValue></Rhythm></Rhythms>
  </Score>
</GPIF>
```
Two quarter notes, E4 on the high string, tied. The real files wrap this in
the ZIP entry `Content/score.gpif` and add dozens of optional elements; the
builder ignores unknown ones.
