# Capella — condensed reference for coding agents

Target: what MuseScore **4.7.4** imports, since **no public specification of
either Capella format exists** (`specs/README.md` § Capella; confirmed by
searching the local specs — nothing). The reference is therefore the porting
source itself:

- `ref-musescore/.474/src/importexport/capella/internal/capella.h` — the
  importer's complete in-memory model of the format (this document's primary
  source);
- `capella.cpp` — `.cap` binary reader and the common conversion to the
  score;
- `capxml.cpp` — `.capx` (ZIP + XML, "CapXML") reader;
- `tests/data/` — 27 `.cap`/`.capx` inputs and 4.7.4 `*.mscx` references.

Upstream licence GPL-3.0-only; `ref-musescore/` is read-only and unversioned.
Companion: [Capella ↔ pivot model](../format-mapping/capella.md).
Planned reader only: `crates/scoretk-capella-{common,import}`.

Two containers share one model: the legacy binary `.cap` and the XML `.capx`.
MuseScore dispatches on the suffix (`capellareader.cpp:36-46`); both end in
`convertCapella(score, &cf, capxMode)` (`capxml.cpp:1412-1440`,
`capella.cpp:1182`), and `capxMode` changes only pitch decoding.

## 1. `.capx` container

- A ZIP archive with a single entry `score.xml`
  (`importCapXml`, `capxml.cpp:1414-1439`; `muse::ZipReader`, entry name
  hard-coded). No other entry is read.
- `score.xml` declares `ISO-8859-1` in CapXML 1.0 files and `utf-8` in
  CapXML 2.0 (capella 7) files — see `tests/data/test1.capx` vs
  `testTuplet2.capx`. Decode according to the XML declaration, never assume
  UTF-8.
- Root `<score xmlns="http://www.capella.de/CapXML/1.0">` or `…/2.0`. The
  namespace is **not** used for dispatch; element names are matched unqualified
  (`capxml.cpp:842-860`).

```
score
├── info/encodingSoftware        (2.0)
├── layout
│   ├── pages/@paperSize,@left,@top,@right,@bottom
│   ├── distances/staffLines/@small,@normal   → spatium
│   ├── instrumentNames/font
│   ├── staves/staffLayout/@description
│   │   ├── notation/@defaultClef, barlines/@mode,@from,@to
│   │   ├── distances
│   │   ├── instrument/name,abbrev
│   │   └── sound/@instr,@volume
│   ├── spacing, beamFlattening
│   └── brackets/bracket
├── barCount
└── systems
    └── system/@beamGrouping,@tempo,@justified,@leftIndent
        ├── staves/staff/@layout,@defaultTime
        │   └── voices/voice/@stemDir
        │       ├── lyricsSettings
        │       └── noteObjects
        │           ├── clefSign/@clef
        │           ├── keySign/@fifths
        │           ├── timeSign/@time
        │           ├── barline/@type
        │           ├── chord   (§3)
        │           └── rest    (§3)
        └── (draw objects only inside chords/rests/barlines)
```

- `staff/@layout` is a **reference by `staffLayout/@description`**, resolved
  with `findStaffIndex` (`capxml.cpp:997-1022`); the description may be empty
  (CapXML 1.0) or `"Unnamed1"`/`"unbenannt"` (2.0). A missing layout is an
  assertion in MuseScore, not a silent default.
- `staff/@defaultTime` is a default only (`"4/4"`, `"C"`, `"allaBreve"`,
  `"longAllaBreve"`, `"infinite"`); the real meter changes are
  `<timeSign time="…">` objects.
- `system/@tempo` is BPM; `@justified` = justified systems; `@leftIndent`
  = first-system indent; `@beamGrouping` is a default beam grouping.
- `layout/staves/staffLayout/sound/@instr` is a **0-based MIDI program**
  (`instr="24"` in `testSlurTie.capx`); `@volume` is 0–127.

## 2. `.cap` container (binary)

- 8-byte signature `cap3-v:` followed by one version character; anything else
  is `BAD_SIG` (`Capella::read`, `capella.cpp:2740-2760`). Then three length-
  prefixed strings (author, keywords, comment), a layout block, then records.
- All integers are little-endian `readInt`/`readLong`/`readDWord`; strings are
  length-prefixed bytes with an unspecified code page (MuseScore reads them
  as Latin-1/UTF-8 and keeps the bytes).
- Record dispatch uses **ASCII signatures**, all in `capella.cpp`:
  - `'S'` → system: bar-count deltas, beam mode, tempo, colour, justified and
    page-break bits, instruction-notation bits, then staves
    (`readSystem`, `:2653-2709`);
  - `'V'` → staff: default meter, layout index, distances, colour, then voices
    (`readStaff`, `:2615-2652`);
  - `'C'` → voice: lyric geometry, stem direction, then a count of note
    objects (`readVoice`, `:2531-2614`);
  - note objects are selected by a type byte
    (`CapellaNoteObjectType`, `capella.h:50-53`): `REST`, `CHORD`, `CLEF`,
    `KEY`, `METER`, `EXPL_BARLINE`, `PAGE_BKGR`.
- `readExtra()` is a forward-compatibility escape: a count byte followed by
  that many bytes, used in system/staff/voice and note objects (`:2389-2402`).
  A reader that ignores it desynchronises the stream.
- `Capella::readLayout` (`:2336-2388`) holds the page geometry, the staff
  layouts (in the same shape as CapXML's), the bracket list and the fonts.
- The format is versioned only by the signature; `.cap` files from capella
  1.x–3.x all use `cap3-v:`.

## 3. Notes, rests and durational objects

Both containers build a `BasicDurationalObj` (`capella.h:574-607`):

| Field | `.cap` encoding | `.capx` encoding |
| --- | --- | --- |
| duration | one byte: low nibble = `TIMESTEP` (`D1`…`D256`, `D_BREVE`) | `<duration base="1/4">` (or `"2/1"` for a breve) |
| dots | bits 0–1 of the flags byte (`0`–`3`) | `@dots` on `<duration>` |
| noDuration | bit 2 → duration 0 (grace/placeholder) | `@noDuration="true"` |
| invisible | bit 5 | `<display invisible="true"/>` |
| small | bit 4 | `@bSmall` via `<display>` |
| colour | separate RGB when bit 6 set | `@color` on the object |
| tuplet | optional byte: low nibble = denominator, bit 4 "tripartite", bit 5 "prolonging" | `<tuplet count="3" tripartite="…" prolong="…"/>` |
| draw objects | optional array | `<drawObjects>` |

- `TIMESTEP` → `Fraction` is `D1=1, D2=1/2, … D256=1/256, D_BREVE=2`
  (`BasicDurationalObj::ticks`, `capella.cpp:2685-2720`); dots add halves.
- `noDuration` (`ticks == 0`) on a **visible** chord is MuseScore's grace-note
  test (`readCapVoice`, `capella.cpp:659-671`). An invisible zero-length
  object is a placeholder and is dropped (`:565-568`, `:652-655`).
- `RestObj` adds `fullMeasures` (a multi-measure rest count) and
  `vertShift`/vertical centring; in CapXML `base` is then an **integer
  string** (`base="4"`), not a fraction (`BasicDurationalObj::readCapx`,
  `capxml.cpp:139-187`). This is the one place where `base` is not a note
  value.
- A chord (`ChordObj`, `capella.h:631-662`) has: `beamMode` (`AUTO`/`FORCE`/
  `SPLIT`, `capella.h:55-57`), `notationStave` (±1 cross-staff), stem
  direction and length, tremolo bar count, articulation code, ties, beam
  shift/slope, `QList<Verse>`, `QList<CNote>`.
- `CNote` (`capella.h:618-629`): `pitch`, `alteration`, `explAlteration`
  (1 force / 2 suppress a printed accidental), `headType`, `headGroup`
  (cross note-head when `headType == 6`), `silent`.
- **Pitch units differ by container.** `.cap` stores a diatonic note number
  relative to clef and key; `.capx` stores the MIDI note number, with **C5 =
  middle C = 60** (`pitchStr2Char`, `capxml.cpp:429-457`). MuseScore branches
  on `capxMode` in the conversion (`capella.cpp:760-790`).
- Articulations are a byte/code, not a name. CapXML 2.0 names them
  (`staccato`, `tenuto`, `staccato tenuto`, `staccatissimo`, `normalAccent`,
  `strongAccent`, `weakBeat`, `strongBeat`, `readCapxArticulation`,
  `capxml.cpp:360-386`); MuseScore converts only `staccato`/`tenuto`/
  `staccatissimo`/`normalAccent`/`strongAccent` and logs the rest
  (`capella.cpp:816-829`).
- Ties are **per head** in CapXML (`<tie begin="true"/>` inside `<head>`) but
  MuseScore's reader sets one `rightTie` flag on the whole chord
  (`readCapxNotes`, `capxml.cpp:487-548`; conversion `capella.cpp:803-810`).
  The binary format has `leftTie`/`rightTie` per chord.
- Lyrics: `<lyric><verse i="0" hyphen="true">text</verse></lyric>`, one verse
  per number; `hyphen` maps to `LyricsSyllabic::BEGIN`. The binary adds
  `leftAlign`, `extender`, `verseNumber` (`Verse`, `capella.h:609-616`).

## 4. Clefs, keys, meters, barlines

- Clef is `(Form, ClefLine, Oct)` — `G/C/F/PERCUSSION` × lines 1–5 ×
  `OCT_ALTA/NULL/BASSA` (`capella.h:85-100`, `CapClef::clefType`). CapXML
  names a few: `"treble"`, `"bass"`, `"G2-"` (G clef 8vb); anything else
  defaults to treble (`capxml.cpp:233-260`). The binary packs form/line/oct
  into one byte (`:2280-2290`).
- Key = `@fifths` (−7…+7) / `signature` (same), nothing else (`CapKey`,
  `capella.h:128-139`).
- Meter = numerator + `log2Denom` + `allaBreve`; `"C"` → 4/4, `"allaBreve"` →
  2/2 with the sign, `"longAllaBreve"` → 4/2, `"infinite"` unsupported
  (`qstring2timesig`, `capxml.cpp:274-329`). The binary stores `log2Denom+1`
  in the low 7 bits and `allaBreve` in bit 7 (`readStaff`,
  `capella.cpp:2624-2632`).
- Barline type enum (`CapExplicitBarline::read`, `:2498-2520`):
  `single/double/end/repEnd/repBegin/repEndBegin/dashed`, plus `@barMode`
  (0 auto, 1 lines only, 2 through). The conversion sets the measure's
  start/end repeat flags and shortens a measure whose actual length differs
  from its nominal length (`capella.cpp:933-999`).
- A barline is an **object in the voice stream**, not a measure attribute:
  measures are delimited implicitly by the accumulated durations between
  barlines. This is the single most important structural difference from
  MusicXML.

## 5. Tuplets, beams, voltas, grace notes

- **Tuplets are not containers.** Every chord/rest of the tuplet carries the
  same denominator (`count` in CapXML, low nibble in `.cap`), and a
  `<bracket>` draw object with `noteRange="n"` on the first note marks how
  many *further* notes belong to it (`ChordObj::readCapxNotes`,
  `capxml.cpp:530-547`). MuseScore reconstructs start/end by counting
  (`readCapxVoice`, `:865-950`). Capella's `tripartite` flag means the ratio
  is `n/3` rather than `n/2^k` (`TupletFractionCap`, `capella.cpp:382-419`).
  Mixed-duration tuplets are legal and are why the count is on notes, not on
  a container.
- Beams are a chord attribute: `AUTO` (engine decides), `FORCE` (beam
  this group), `SPLIT` (break here). CapXML 2.0 writes `<beam group="force">`;
  CapXML 1.0 and `.cap` store it in flags (`ChordObj::read`,
  `capella.cpp:1933-1990`).
- Voltas are draw objects attached to a chord, with `@firstNumber`/
  `@lastNumber` and bent/broken hooks (`VoltaObj::readCapx`,
  `capxml.cpp:660-685`); the span is the object's `x1`/`x2` range, mapped to
  the measures it covers.
- Grace notes are chords with `noDuration` (visible, zero ticks), placed
  before the following chord (`capella.cpp:685-715`); there is no explicit
  grace-type flag, so acciaccatura/appoggiatura is inferred from
  `postGrace`/stem/beam data and MuseScore's `SetCapGraceDuration`
  (`:119-155`).
- Multi-measure rests are a `RestObj` with `fullMeasures > 0`
  (CapXML: `base="3"`), expanded by the conversion (`capella.cpp:583-620`).

## 6. Draw objects (the "notations" layer)

Draw objects hang off a chord, a rest or a barline; `<basic>` **follows** the
typed element and carries the note span (`noteRange`)/scope
(`readCapxDrawObjectArray`, `capxml.cpp:729-816`).

| Capella type | CapXML element | MuseScore behaviour | C++ |
| --- | --- | --- | --- |
| `SLUR` | `<slur>` | span from `noteRange`; CapXML slur geometry is **discarded** (`SlurObj::readCapx` is empty) | `capxml.cpp:650-656`; `capella.cpp:1004-1050` |
| `WEDGE` | `<wedge decrescendo span>` | crescendo/diminuendo hairpin | `:705-717`; `capella.cpp:1101-1130` |
| `VOLTA` | `<volta firstNumber lastNumber>` | first/second ending | `:660-685` |
| `TRILL` | `<trill tr="true">` | trill line | `:687-703` |
| `TEXT`/`SIMPLE_TEXT` | `<text x y align><font><content>` | staff text; **`capella3` font glyphs are dynamics/fermatas, not text** (§7) | `:595-624`; `capella.cpp:156-345` |
| `TRANSPOSABLE` | `<transposable base><drawObj><group>` | chord symbol (`Harmony`) — variants for enharmonic spellings | `:626-648`; `capella.cpp:336-358` |
| `BRACKET` | `<bracket orientation number>` | **consumed** by the tuplet reconstruction | `:718-727` |
| `GUITAR` | `<guitar>` | skipped (guitar frames are not modelled) | `:781-783` |
| `LINE`, `RECT_ELLIPSE`, `POLYGON`, `WAVY_LINE`, `METAFILE`, `NOTELINES`, `GROUP` | same names | skipped, only logged | `:751-812` |

- Draw objects also carry page coordinates (`x`, `y` in spatium units) which
  MuseScore keeps as user offsets for text (`capella.cpp:300-330`) and throws
  away for lines.

## 7. The `capella3` glyph trap

Dynamics, fermatas, arpeggios and caesuras are **not elements**: they are
one-character `<text>` objects in the font `capella3`. MuseScore maps the
character code (`processBasicDrawObj`, `capella.cpp:156-300`):

| char | meaning | char | meaning |
| --- | --- | --- | --- |
| `p q r s f g h` | `p pp ppp sf f ff fff` | `i j z { \|` | `mp mf sfz fz fp` |
| `k` / `u` | fermata below / above | `181` | caesura |
| `172`/`173` | arpeggio short/long | `187`/`188` | arpeggio up/down |
| `d e n o y $` | D.C., D.S., segno, coda… (logged, **not** imported) | `a b v 186` | pedal, 8va, 15ma (logged, not imported) |

A reader that treats these as text produces "p" staff texts instead of
dynamics; the mapping to `Dynamic`/`Fermata`/`Arpeggio`/`Breath` must be
reproduced from this table.

## 8. Layout carried by the format (L5)

- Page: size, four margins, `barCount` position/font, `instrNotation`
  (none/short/long), justified systems, left indent, page breaks.
- Staff: line distances, bracket span, barline span (`@barlineMode`),
  per-staff colour, small staff, extra distance.
- Per object: text offsets and fonts, slur Bézier points (binary only),
  beam shift/slope, stem length, horizontal shift.
- Instrument: MIDI program, volume, **transposition in semitones** (`transp`),
  name/abbrev/intermediate names, percussion channel and sound maps
  (`soundMapIn`/`soundMapOut`, 128-entry tables).

MuseScore keeps part of this in its style and drops the rest; the pivot has
no coordinates at all (see the mapping document).

## Traps that silently break data

1. `.capx` is a ZIP with the single entry `score.xml`; do not scan entries.
   Encoding is ISO-8859-1 in CapXML 1.0, UTF-8 in 2.0. (§1)
2. `staff/@layout` refers to `staffLayout/@description` by string; the
   description is often empty. Index by position only as a fallback. (§1)
3. Pitch is MIDI-numbered with **C5 = middle C** in CapXML, diatonic relative
   to clef+key in `.cap`; mixing the two shifts everything by an octave and
   by the key. (§3)
4. `<duration base>` is a fraction for notes and an **integer count** for
   multi-measure rests. (§3)
5. Tuplets are inferred from per-note counts + a `<bracket noteRange>`;
   there is no tuplet container and no explicit end. (§5)
6. Barlines are voice objects; a measure's length is the accumulated
   duration, and an irregular measure is *implicitly* created when the sum
   differs from the meter. (§4)
7. Dynamics/fermatas/arpeggios are `capella3` font glyphs, not text. (§7)
8. `.capx` slurs have no geometry (`SlurObj::readCapx` does nothing); only the
   `noteRange` span is usable. (§6)
9. Ties are per head in CapXML but MuseScore applies the flag to the whole
   chord; a faithful port must decide and document which it follows. (§3)
10. `readExtra()` in the binary format is a length-prefixed skip; ignoring it
    desynchronises the whole stream. (§2)
11. `noDuration` + visible = grace note; `noDuration` + invisible =
    placeholder to drop. (§3)
12. Articulation codes 7/8 (`weakBeat`/`strongBeat`) are accepted by the
    parser but **not converted** (`capella.cpp:816-829`): a "missing"
    articulation can be a faithful copy of the reference.

## Minimal valid examples

CapXML 2.0, one staff, one 4/4 bar, a quarter C5, a quarter D5 tied to a
quarter D5, a quarter E5:

```xml
<?xml version="1.0" encoding="utf-8"?>
<score xmlns="http://www.capella.de/CapXML/2.0">
  <layout><staves><staffLayout description="Piano">
    <notation defaultClef="treble"/><sound instr="0" volume="80"/>
  </staffLayout></staves></layout>
  <systems><system>
    <staves><staff layout="Piano" defaultTime="4/4"><voices><voice stemDir="up">
      <noteObjects>
        <clefSign clef="treble"/><timeSign time="4/4"/>
        <chord><duration base="1/4"/><heads><head pitch="C5"/></heads></chord>
        <chord><duration base="1/4"/><heads>
          <head pitch="D5"><tie begin="true"/></head></heads></chord>
        <chord><duration base="1/4"/><heads>
          <head pitch="D5"><tie end="true"/></head></heads></chord>
        <chord><duration base="1/4"/><heads><head pitch="E5"/></heads></chord>
      </noteObjects>
    </voice></voices></staff></staves>
  </system></systems>
</score>
```
`.cap` equivalent: signature `cap3-v:` + one version byte, layout block, then
`S…`/`V…`/`C…` records whose note-object sequence is
`CLEF, METER, CHORD(1/4), CHORD(1/4, rightTie), CHORD(1/4, leftTie),
CHORD(1/4)`.
