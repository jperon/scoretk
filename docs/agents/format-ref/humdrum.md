# Humdrum `**kern` — condensed reference for coding agents

Target: the **`**kern` representation** of Humdrum, as described by the
reference manual and as converted by Verovio. Local sources:

- `specs/humdrum/kern-reference.html` — "The Humdrum `**kern`
  Representation" (the normative syntax text; `§x.y` below refers to its
  sections, with the line in the file);
- `specs/humdrum/humlib/` — humlib (BSD-2-Clause, commit `b9d67cfe`), the
  parser and analysis library;
- `specs/humdrum/verovio/src/iohumdrum.cpp` — Verovio's `**kern` → MEI
  converter (LGPL-3.0-or-later; reimplement, do not copy), the only complete
  notation mapping available locally;
- `specs/README.md` § Humdrum records the licences: the kern reference page
  itself has **no stated licence** (private, do not quote it at length).

There is **no "Humdrum format" container**: a file is plain text, one record
per line, tab-separated tokens. `**kern` is one representation among many
(`**mens`, `**harm`, `**dyn`, `**text`, …) and a file may mix them.

Companion: [Humdrum ↔ pivot model](../format-mapping/humdrum.md).
Planned crates `scoretk-humdrum-{common,import,export}` (import first).

## 1. Records, spines and tokens

- A file is a sequence of **records** (lines). Records are separated by a
  newline; **tokens within a record are separated by tabs**, never spaces
  (§2.1, `kern-reference.html:2579-2657`). Spaces *inside* a token are
  multiple stops (§10).
- A column is a **spine**. Spines start with an exclusive interpretation
  (`**kern`, `**harm`, …) and end with `*-`. Spines can be added, split,
  joined and exchanged during the file (§2.2, §4.1); the number of columns
  can therefore change at any record, and a "column index" is not a stable
  identifier. The identity is the spine, and only spine-path records change
  it.
- A **null token** `.` means "this spine has no new event in this record"
  (§2.1): for a note spine it is a sustain, *not* a rest.
- Records are one of: exclusive interpretation (`**…`), tandem
  interpretation (`*…`, §2.3), comment (`!`, `!!`), reference record
  (`!!!…`), barline (`=…`), data (`**kern` tokens and the like). A record's
  kind is determined from its first character.

```
**kern  **kern
*M4/4   *M4/4
=1      =1
4c      4e
.
8d      8f
*-      *-
```

## 2. Interpretations

- **Exclusive** interpretations start a spine and define its data type:
  `**kern`, `**bar`, `**harm`, `**dyn`/`**dynam`, `**text`, `**silbe`,
  `**fb`, `**recip`, `**mens`, … (§2.2, `:2658-2708`). A spine must have
  exactly one exclusive interpretation, immediately at its start.
- **Tandem** interpretations start with a single `*` and add information:
  `*clefG2`, `*k[f#c#]` (key signature), `*c#:` (key), `*M6/8` (meter),
  `*MM96.3` (tempo), `*tb32` (time base), `*I"name` (instrument name),
  `*ICstr` (instrument class), `*IGripn` (group), `*ITr` (transposing),
  `*staff1` (staff number), `*met(c)` (mensuration), `*^`/`*v`/`*+`/`*-`/`*x`
  (spine manipulators). (§2.3, `:2709-2802`; §3.11, `:4108-4133`.)
- A **null interpretation** is a bare `*`: a place-holder, no meaning.
- Tandem interpretations are **per spine** and take effect from that record
  onward, including mid-file. Different spines can carry different meters,
  keys and clefs (§2.3); a reader must not assume one global meter.
- `*staffN` assigns the staff number; it is how multi-staff systems are
  declared, and Verovio uses it to decide staff order and to detect a
  reversed spine order (`checkIfReversedSpineOrder`, `iohumdrum.cpp:1019-1066`;
  `getStaffNumForSpine`, `:1067-1086`). Without it, the printed order is
  ambiguous.

## 3. Comments, global comments, reference records

- `!` at the start of a record (or a spine token `!` inside a record) is a
  **local comment**; `!!` is a **global comment** that ignores spines
  (§2.4, `:2803-2939`).
- `!!!KEY: value` is a **reference record** (bibliographic metadata), the
  Humdrum equivalent of a header: `!!!OTL:` title, `!!!COM:` composer,
  `!!!YEC:` copyright/year, `!!!AGN:` etc. The kern reference only names
  them (§2.4, `:2927`); the field vocabulary is Humdrum-wide. Verovio maps
  them into MEI's header (`createFileDesc`,
  `iohumdrum.cpp:2867-3052`; `createWorkList`, `:4140-4538`); the pivot's
  `ScoreMetadata` is the destination.
- Reference records may be repeated and may have language selectors
  (`!!!OTL@fr:`); Verovio's `getReferenceItems`/`getAllReferenceItems`
  (`:5156-5292`) show how to collect them.

## 4. Pitch and accidentals

- Middle C is lower-case `c`; each extra letter raises one octave (`cc` =
  C5, `ccc` = C6); upper case lowers: `C` = C3, `CC` = C2 (§3.2.1,
  `:2978-3051`). Case changes only at B/C.
- Accidentals follow the pitch: `#`, `##`, `-`, `--`, `n`; sharps/flats and
  naturals are mutually exclusive (`cc#n` is illegal) (§3.2.1).
- Pitches are **absolute and context-independent**: the accidental must be
  written on *every* note, even when the key signature or an earlier note in
  the bar would supply it. There is no "accidental state" in `**kern`; the
  key signature is informational. This is the opposite of MusicXML/MEI.
- `**kern` is nominally equal-tempered; a tandem interpretation can declare
  another tuning, but the pitch letters remain nominal.

## 5. Duration, dots, tuplets, rests

- Durations are **reciprocals**: `1` whole, `2` half, `4` quarter, `8`
  eighth, `16` sixteenth, `32`, `64`, `128`; `0` is a breve (§3.2.7,
  `:3169-3219`). `3` is *not* "three of something": it is a **third-note**
  (the quarter-note triplet value), `6` an eighth-note triplet, `12` a
  sixteenth-note triplet.
- The general rule: `value = number_of_tuplets × total_duration`;
  7 notes in the time of a whole note are `7`, 23 notes in a dotted quarter
  are `92..` (§3.2.8, `:3220-3272`).
- Dots are appended to the number: `8.` dotted eighth, `2..` doubly-dotted
  half.
- `r` is a rest: `4r`, `2.r`. A full-bar rest in 3/4 is `2.r`, not `1r`; a
  whole-rest *glyph* over a longer actual value is written `2.rr` (double
  `r`) (§3.3, `:3354-3384`). A **null token** `.` is not a rest.
- A grace note is `q` instead of a duration (`G#q`), with no duration; a
  groupetto keeps its notated duration and adds `Q` (`16cQ`); an
  appoggiatura is encoded as performed, with `P` on the grace and `p` on the
  following shortened note (§3.2.9, `:3273-3321`).
- A time base `*tbN` redefines what the duration integers mean (a `*tb32`
  makes `8` a 32nd); it applies from that record in that spine (§3.11).

## 6. Ties, slurs, phrases, beams, stems, articulations

- Tie: `[` starts, `_` continues, `]` ends, on each note of a chord
  individually (§3.2.2, `:3052-3092`).
- Slur: `(` … `)`; phrase: `{` … `}`. On a multiple stop, slurs and phrases
  are written once for the whole stop, while ties are per note (§3.12).
- Beams: `L` starts a beam, `J` ends it; repeated letters are beam levels
  (`LL`/`JJ`); `K`/`k` are partial beams right/left (§3.2.10,
  `:3322-3353`).
- Stems: `/` up, `\` down (§3.2.6, `:3152-3168`); bowing: `u` up-bow, `v`
  down-bow (§3.2.5, `:3142-3151`).
- Ornaments: `m`/`M` mordent (semitone/whole tone), `w`/`W` inverted mordent,
  `t`/`T` trill, `S` turn, `$` Wagnerian turn, `O` generic ornament, `R`
  ornament ending with a turn (§3.2.3, `:3093-3125`).
- Articulations: `'` staccato, `` ` `` staccatissimo/martellato, `~` tenuto,
  `^` accent, `;` pause, `,` breath, `z` sforzando, `s` spiccato, `"` pizzicato,
  `I` generic, `o` harmonic, `U` con sordino (§3.2.4, `:3126-3141`).
- Glissando: `H` starts, `h` ends (§3.7 table, `:3601-3708`).

## 7. Barlines and measures

- A barline is a record whose tokens start with `=`: `=` simple, `==` double,
  `=12` numbered barline, `=-` invisible, `=12-` numbered invisible
  (§3.4, `:3385-3501`).
- There is **no measure element**: a measure is the data between two barline
  records. A pickup is therefore "everything before the first barline"; an
  incomplete bar has fewer durations than the meter.
- Repeat signs are encoded as barline signifiers (`=:|:`, `=:||:`, `==:|!`, …); the
  exact vocabulary is in §3.4 and differs from MusicXML's
  `<repeat direction="…"/>`.
- A barline may carry a label after `=` (rehearsal numbers, section names,
  `=1a`); it is a label, not a duration.

## 8. Multiple stops vs spine paths

- Simultaneous notes of **equal duration** in one voice are written as a
  space-separated token group inside one spine: `4c 4e` (§3.12,
  `:4134-4220`). This is one chord, not two spines.
- Notes of **different duration** in one voice require a spine split:
  `*^` spawns a new spine that inherits the interpretations; `*v` joins two
  adjacent spines with the same exclusive interpretation; `*+` adds a spine
  to the right; `*-` terminates; `*x` exchanges two spines (§4.1,
  `:4237-4395`).
- Because of splits and joins, **a chord may span several spines** at one
  record and then merge back; the reader must track spine identity through
  the manipulators. Verovio's layer analysis (`storeStaffLayerTokensForMeasure`,
  `iohumdrum.cpp:10132-10292`; `convertStaffLayer`, `:12836-12887`) is the
  reference algorithm.

## 9. Companion spines

A real Humdrum score usually has more than `**kern`:

| Spine | Content | Verovio |
| --- | --- | --- |
| `**bar` | bar numbers | — (uses `=` records) |
| `**harm`, `**mxhm`, `**rhrm` | chord symbols / roman numerals | `analyzeHarmInterpretations`, `iohumdrum.cpp:1211-1321` |
| `**dyn`, `**dynam` | dynamics | `getAssociatedDynamSpine`, `:7995-8021` |
| `**text`, `**silbe` | lyrics | `prepareVerses`, `:5308-5354` |
| `**fb`, `**fba`, `**Bnum` | figured bass | `addFiguredBassForMeasure`, `:10473-10652` |
| `**recip` | rhythm-only spine | `prepareTimeSigDur`, `:5355-5425` |
| `**mens` | mensural notation | `checkForMens`, `:1837-1860` |

A spine whose exclusive interpretation the pivot cannot map must be reported,
not silently dropped (the same rule as ABC's overlay).

## Traps that silently break data

1. **Tabs vs spaces.** Tokens are tab-separated; spaces inside a token are a
   multiple stop. Splitting on whitespace turns every chord into extra spines
   or loses the bar structure. (§1, §8)
2. **Null token vs rest.** `.` means "sustain", `r` means "rest"; turning `.`
   into a rest inserts phantom rests everywhere. (§1, §5)
3. **Duration is a reciprocal, not a value.** `4` is a quarter, `0` a breve,
   `3` a quarter triplet; treating the number as a count multiplies every
   rhythm by itself. (§5)
4. **Accidentals are explicit on every note.** There is no key-implied
   accidental state; an importer that applies the key signature adds
   accidentals. (§4)
5. **Pitch case.** `c` = C4, `C` = C3, `cc` = C5; a case-insensitive pitch
   parser is off by an octave. (§4)
6. **Spine identity changes.** `*^`, `*v`, `*+`, `*-`, `*x` renumber columns;
   a column-indexed model breaks at the first split. (§8)
7. **Spine order vs staff order.** `*staffN` decides; without it the order is
   ambiguous and Verovio may reverse it. (§2)
8. **No meter requirement.** `*M` is optional and per spine; assuming 4/4
   changes measure lengths and the number of bars. (§2, §7)
9. **`*tb` time base.** A time-base change redefines the duration integers
   mid-file; ignoring it scales everything after it. (§5)
10. **Grace notes have no duration** (`q`), while groupetti (`Q`) keep theirs
    and appoggiaturas are already performed (`P`/`p`). (§5)
11. **Ties are per note, slurs are per stop.** Applying a slur to each note
    of a chord or a tie to the whole chord is wrong. (§6, §8)
12. **Barlines are records, not attributes.** A barline record is a row like
    any other; the measure boundaries are "between records", so a barline
    that coincides with a note in another spine must not consume time.
    (§7)
13. **Repeat vocabulary.** `=:|:`, `=:||:`, `==:|!` etc. are signifier strings,
    not a MusicXML-like enum; parse the documented set and report the rest.
    (§7)

## Minimal valid example

```humdrum
!! Tiny two-voice example
**kern  **kern
*M4/4   *M4/4
*k[f#]  *k[f#]
=1      =1
4g      4b
4a      4cc
4b      4dd
4cc     4ee
=2      =2
2dd     2ff
2cc     2ee
==      ==
*-      *-
```
Two 4/4 bars: bar 1 has four quarters, bar 2 two halves. The key signature is
informational (each note already carries its accidental state); the final
`==` is a double bar; `*-` terminates both spines. A chord would be
`4g 4b` **inside one token**, not two tabs.
