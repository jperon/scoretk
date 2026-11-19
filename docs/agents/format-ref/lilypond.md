# LilyPond 2.26.0 — condensed reference for coding agents

Target: **LilyPond 2.26.0** (see `specs/README.md` § LilyPond). Local sources:
`specs/lilypond/notation-big-page.html` (notation reference; section numbers
below, e.g. `§2.1.1`), `specs/lilypond/learning-*.html` (learning manual,
chapter numbers `§4.1.x` etc.). Licence: GNU FDL 1.1-or-later for the
documentation; this is an original summary, not copied doc text.
Companion: [LilyPond ↔ pivot model](../format-mapping/lilypond.md).
Reader `crates/scoretk-lilypond-import`, writer `crates/scoretk-lilypond-export`.

## File structure and expressions

- A file is Scheme-flavoured text: `\version "2.26.0"`, `\header { title = "…" }`,
  assignments `name = { … }`, then `\score { << … >> \layout {} }` with
  `\new Staff`, `\new Voice`, etc. (Learning §4.1.1–4.1.5, §4.2.1–4.2.2,
  §4.4.4; Notation §6.1.1)
- `{ … }` = sequential music; `<< … >>` = simultaneous music. `\\` inside
  sequential music starts the next voice; `\oneVoice` collapses them.
  (Learning §4.1.3, §4.2.2; Notation §5.2.1, §5.2.6)
- `\relative c' { … }` chooses each note's octave nearest the previous note
  (interval < a fifth, accidentals ignored); `\fixed c' { … }` keeps written
  octaves literal. `\relative` and `\transpose` are distinct from `\key`.
  (Notation §1.1.2, §1.2.2, §1.3.4; Learning §3.4.3)
- Variables expand at use site: `mel = { c'4 d' }` then `\mel`; a variable
  can hold music, a pitch, or a markup. (Learning §4.4.4)

## Durations, ticks, bars

- Duration numbers are reciprocals: `1` whole, `2` half, `4` quarter, `8`
  eighth, `16`, `32`, `64`, `128`, …, `4.` = dotted quarter, `4..` double
  dotted; `\breve`, `\longa`, `\maxima` for longer. `4*2` scales a written
  duration; a bare duration after a note reuses the **last explicit pitch**.
  (Notation §2.1.1, §2.1.3)
- Rests: `r4` (pitch-less rest), `R1` (full-bar rest, auto-expands to multiple
  bars), `s4` (spacer, invisible but consumes time). A full-bar rest does not
  need `\time`-dependent rewriting. (Notation §2.2.1–§2.2.3)
- `\time 3/4`; `\partial 4` = anacrusis (shifts bar numbering); `\time` changes
  do **not** rescale existing durations. Complex fractions need Scheme syntax.
  (Notation §2.3.1, §2.3.3, §2.3.5)
- Bars `|`, `||`, `|.`; `\bar "|:|"`. Rehearsal marks `\mark \default`.
  Automatic bar lines are inserted at measure boundaries; a stray `|` in the
  middle creates an extra bar check error. (Notation §2.5.1–§2.5.5)

## Pitch, key, clef, accidentals

- Note names `c d e f g a b` (no English/German names by default). `'` raises
  an octave, `,` lowers one; `c'` is middle C. Sharps/flats are suffixes:
  `cis` C♯, `des` D♭, `cisis`, `deses`, `as` = A♭. (Notation §1.1.1, §1.1.3)
- `\key g \major`, `\key e \minor`, `\key d \dorian` (also `\ionian`,
  `\phrygian`, `\lydian`, `\mixolydian`, `\aeolian`, `\locrian`). A key
  signature changes **display**, not the pitch of `c`/`fis` in the input.
  (Notation §1.3.2)
- `\clef treble|alto|tenor|bass|"G_8"|percussion|tab`; default treble.
  `\clef "G_8"` = treble with an octave-change 8 below. (Notation §1.3.1)
- `\transpose c d` rewrites written pitches; `\set Staff.instrumentName = …`
  and `\transposition` handle transposing instruments. (Notation §1.3.4,
  §10.1; Learning §3.4.1)
- Automatic accidentals are context-driven, so an input `fis` with `f` already
  in the key may print a natural; exporting must decide whether to emit the
  accidental explicitly. (Notation §1.3.5)

## Parts, staves, voices, chords

- `\new Staff { … }` instantiates a staff; `S1 = \new Staff …`, then
  `\score { << \S1 \S2 >> \layout {} }`. Staff groups use `\new StaffGroup`
  or `\new GrandStaff`; `\with { instrumentName = "…" }` adds labels.
  (Notation §6.1.1–§6.1.3; Learning §4.4.2)
- Polyphony on one staff: `<< { \voiceOne … } \\ { \voiceTwo … } >>`;
  `\voiceThree`/`\voiceFour` for more; `\change Staff = "up"` moves notes
  between staves. (Notation §5.2.1–§5.2.6)
- Chords are `<c e g>4` (angle brackets, duration after the closing `>`);
  `<>` is a chord-shaped silence. Notes inside a chord share one tick.
  (Notation §5.1.1)

## Ties, slurs, tuplets, grace

- Tie `~` between two notes of the same pitch extends duration;
  `\repeatTie`, `\laissezVibrer` for one-sided ties. A tie created after a
  `\relative` re-read must still join equal *sounding* pitches.
  (Notation §2.1.4)
- Slur `( … )`, nestable; `\=1(` … `)` labels matching starts/stops,
  phrasing slur `\( … \)`. In polyphonic music a slur must be closed in the
  same voice it started. (Notation §3.2.1–§3.2.2)
- `\tuplet 3/2 { c'8 c' c' }` plays 3 written eighths in the time of 2; the
  fraction is printed. `\tuplet 3/2 8 { … }` groups several tuplets;
  `\times 2/3 { … }` scales durations without printing a tuplet number;
  `\tupletUp`, `\noTupletBracket`, nested `\tuplet` are allowed.
  (Notation §2.1.2–§2.1.3)
- `\grace { … }` takes no logical time; `\acciaccatura`, `\appoggiatura`,
  `\slashedGrace`, `\afterGrace` are variants. The written duration of a
  grace note is not added to the measure. (Notation §2.6.1)
- Articulations/ornaments `-.`, `->`, `-^`, `\!`, `\fermata`, `\trill`,
  dynamics `p`/`mf`/`f`, hairpins `\< … \!`; text `^"above"`, `_"below"`.
  (Notation §3.1.1–§3.1.3, §3.2.3, §3.3.1–§3.3.3, §8.1)

## Repeats and lyrics

- `\repeat volta 2 { … }`; alternatives `\repeat volta N { … \alternative {
  \volta 1 { … } \volta 2 { … } } }`. `\repeat unfold N { … }` writes the
  music out N times; `\repeat percent N`, `\repeat tremolo` for short repeats.
  `\segno`, `\coda`, `\fine` provide explicit jump marks.
  (Notation §4.1.1–§4.1.8, §4.2.1–§4.2.2)
- Lyrics enter `\lyricmode { Heigh4 ho2 }` (digits are syllable durations),
  or align automatically with `\addlyrics { … }` / `\lyricsto "name" { … }`.
  `--` = hyphenated syllable, `_` = melisma extender, `\skip N` = skip N
  notes, `~` joins words under one syllable. One `\lyricsto` per verse/stanza.
  (Notation §9.1.2–§9.1.8, §9.2.1–§9.2.6, §9.3)

## Traps that silently break data

1. `\relative` recomputes every subsequent octave: editing one note changes
   the rest. Prefer `\fixed` when emitting from a model. (§1.1.2)
2. A bare duration (`4`, `8`) after a note repeats the previous **pitch**,
   not a rest; a missing pitch is not an error. (§2.1.4, §2.1.1)
3. Input pitch ignores `\key`: `c` is C natural even in G major. Deriving
   input `alter` from the key signature corrupts spelling. (§1.3.2)
4. `\tuplet` multiplies the music's time; using sounding values instead of
   written values inside the braces double-scales the notes. (§2.1.2)
5. `\time` does not rescale durations; `R1` follows the *current* meter.
   (§2.3.1, §2.2.3)
6. `\partial` shifts bar numbering and bar-check positions. (§2.3.3)
7. A slur opened in one voice and closed in another fails silently or is
   dropped; `\\` creates a new voice context. (§3.2.1, §5.2.1)
8. Grace notes take zero logical time — do not advance a tick counter for
   them; lyrics still skip them. (§2.6.1, §9.1.2)
9. `\repeat volta` is a structural wrapper, not an event: an importer must
   either expand it with a repeat list or keep the volta boundaries.
   (§4.1.2–§4.1.3)
10. `<c e g>4` is one chord at one tick, not three sequential notes.
    (§5.1.1)

## Tiny valid example

```lilypond
\version "2.26.0"
\header { title = "Tiny" }
mel = \relative c' {
  \key g \major \time 4/4 \clef treble
  \repeat volta 2 {
    \grace { d'16 } g'2 <b, d g>2 |
    \tuplet 3/2 { c'8 d' e' } a'2 ~ a'2 |
  }
  \alternative { \volta 1 { b'2 g'2 } \volta 2 { c''2 g'1 } }
}
bas = \relative c {
  \key g \major \time 4/4 \clef bass
  g2 d'2 | g,2 d'2 | g,2 d'2 | g,1 |
}
\score {
  <<
    \new Staff << \mel \addlyrics { Heigh -- ho my dear } >>
    \new Staff \bas
  >>
  \layout {}
}
```
The `\alternative` block supplies two endings for the `\repeat volta 2`;
`Heigh -- ho` demonstrates a hyphenated syllable (`--`) aligned to the
melody voice.
