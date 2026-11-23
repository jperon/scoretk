# LilyPond (`scoretk-lilypond-common`, `-import`, `-export`)

Three crates: `crates/scoretk-lilypond-common` (what both directions
share: error, circle of fifths, clef table),
`crates/scoretk-lilypond-export` (model → `.ly`, t36) and
`crates/scoretk-lilypond-import` (reading `.ly` → model, t40). They are
wired into the `scoretk` facade by the `lilypond-export`
feature (`Format::Ly` for writing) and `lilypond-import` (`Format::Ly` for reading). This document describes writing (model →
LilyPond mapping), reading a subset of LilyPond, notation choices,
limits measured on the corpus and the upgrade procedure for the
tracked engines. The project core remains in `AGENTS.md`.

## 1. Role and API

```rust
// crates/scoretk-lilypond-export
pub fn write_lilypond(score: &Score) -> Result<String, Error>;
// crates/scoretk-lilypond-import
pub fn read_lilypond(src: &str) -> Result<Score, Error>;
pub fn read_lilypond_reporting(src: &str) -> Result<(Score, Vec<Warning>), Error>;
pub fn read_lilypond_with_includes(
    src: &str,
    resolve: &mut dyn FnMut(&str, Option<&str>) -> Option<String>,
) -> Result<(Score, Vec<Warning>), Error>;
// crates/scoretk (facade) : read_lilypond_with_includes(bytes, resolve) -> Result<Score>
// crates/scoretk-lilypond-common : `Error`, `Result`, `pitch`, `clefs`
```

`read_lilypond` and `read_lilypond_reporting` do not follow `\include`s of
ordinary files: they ignore them with a warning. The function
`read_lilypond_with_includes` receives a resolver
`resolve(requested_path, including_file_path)` returning the included file's
text or `None`; the second argument is `None` at the top level,
then the normalised path of the included file (relative to the main file). The
CLI supplies a disk resolver (including file's directory, then main
file's directory); the library itself opens no files.

None of the three crates does I/O: they work on in-memory
text or values, keeping them compilable for
`wasm32-unknown-unknown`, Android and iOS. They depend only on
`scoretk-core` (pivot model), on `scoretk-lilypond-common` for both directions
and on `thiserror`; no new third-party crate, so nothing to add to
`docs/licences-tierces.md`.

**Splitting rule**: `-common` contains **no conversion logic**, only tables and types used by both directions.
`src/pitch.rs` (reading and writing) is therefore split: the circle of fifths
(`tpc`, step, accidental) goes into `scoretk-lilypond-common/src/pitch.rs`, the
clef table into `scoretk-lilypond-common/src/clefs.rs`; what remains in
writing are the English note names of `\fixed` mode, key signatures and
noteheads, and in reading the per-language name tables, octaves and
transposition. `text.rs` remains entirely in writing: reading does
not use it.

| Element | Location |
| --- | --- |
| Error, `Result` | `crates/scoretk-lilypond-common/src/error.rs` |
| Circle of fifths (`tpc`) | `crates/scoretk-lilypond-common/src/pitch.rs` |
| Clef table (both directions) | `crates/scoretk-lilypond-common/src/clefs.rs` |
| Writing API, documentation | `crates/scoretk-lilypond-export/src/lib.rs` |
| English pitches, key signatures, noteheads | `crates/scoretk-lilypond-export/src/pitch.rs` |
| Durations, dots, multipliers | `crates/scoretk-lilypond-export/src/duration.rs` |
| Chords, rests, articulations, ornaments | `crates/scoretk-lilypond-export/src/chord.rs` |
| Annotations (dynamics, tempo, texts, chords) | `crates/scoretk-lilypond-export/src/annotations.rs` |
| Spanners (ties, hairpins, ottavas, pedals, trills, glissandos) | `crates/scoretk-lilypond-export/src/spanners.rs` |
| Repeat structure (`\repeat volta`, `\alternative`) | `crates/scoretk-lilypond-export/src/structure.rs` |
| Voice framework, lyrics, event attachment | `crates/scoretk-lilypond-export/src/voice.rs` |
| Header, variables, `\score` block, groupings | `crates/scoretk-lilypond-export/src/writer.rs` |
| Reading API | `crates/scoretk-lilypond-import/src/lib.rs` |
| Reading: lexer, grammar, tree, interpreter, building | `crates/scoretk-lilypond-import/src/` |
| Corpus snapshots (writing) | `crates/scoretk-lilypond-export/tests/corpus_lilypond.rs`, `tests/corpus/ly/` |
| LilyPond compilation | `crates/scoretk-lilypond-export/tests/lilypond_compile.rs` |
| Reading round trip | `crates/scoretk-lilypond-import/tests/read_corpus.rs` |
| Small hand-written sources | `crates/scoretk-lilypond-import/tests/read_lilypond.rs` |
| Units of shared tables | `#[cfg(test)]` of `scoretk-lilypond-common` |

Behaviour reference: **`musicxml2ly`** (LilyPond package,
GPL-3.0-or-later), which defines the idiomatic mappings between
MusicXML notation and LilyPond. MuseScore 4 has no LilyPond export; the
articulation, ornament, dynamic and figure tables therefore follow those
of `musicxml2ly`, rewritten in Rust. The attribution is in `NOTICE` and in
the headers of the relevant files.

## 2. Reading: method and subset

LilyPond cannot be embedded: its parser is a Bison/Flex pair
(`lily/parser.yy`, `lily/lexer.ll`) that depends on Guile Scheme for nearly
all its semantic actions. Reading is therefore a **Rust rewrite
of a subset of the grammar**, following `python-ly`
(`ly.music`, `ly.lex`), which reads LilyPond without Scheme. The reference
sources are a copy of LilyPond **2.26.0** kept **outside the repository** (see
`NOTICE` and § 8): `lily/parser.yy`, `lily/lexer.ll`, `lily/pitch.cc` and
`scm/define-note-names.scm`.

| Step | File | Role |
| --- | --- | --- |
| Lexer | `scoretk-lilypond-import/src/lexer.rs` | tokens (`\name`, words, numbers, fractions, strings, punctuation), `%` and `%{ %}` comments, upstream lexer states |
| `\include` | `scoretk-lilypond-import/src/include.rs` | textual expansion before parsing: language files → `\language`, files delivered without music ignored, other files via the resolver |
| Tree | `scoretk-lilypond-import/src/ast.rs` | `music_list`, `sequential_music`, `simultaneous_music`, `note_chord_element`, `post_events`, `\repeat`, `\new`, lyrics |
| Grammar | `scoretk-lilypond-import/src/parser.rs` | recursive-descent parsing of useful productions; everything else ignored with a `Warning` |
| `\parallelMusic` | `scoretk-lilypond-import/src/parallel.rs` | splitting at bar checks and distribution among variables (`ly/music-functions-init.ly`) |
| Pitches | `scoretk-lilypond-import/src/pitch.rs` (+ `clefs`/`tpc` from `-common`) | tables of `define-note-names.scm` (all languages), `\fixed`/`\relative`/absolute octaves (`Pitch::to_relative_octave`), clefs, key signatures |
| Interpretation | `scoretk-lilypond-import/src/events.rs` | octaves, transposition, n-tuplets, ornaments, spanners, repeats, time-stamped events |
| Building | `scoretk-lilypond-import/src/builder.rs` | measure framework (`\time`, `\partial`, `Timing.measureLength`, `R`), segments, displayed accidentals, lyrics, ties |

Covered subset: notes, rests (including the `\rest` post-event,
which turns the written note into a rest, like `event_chord` of
`lily/parser.yy`), chords, durations and dots,
`\relative`, `\fixed` and absolute notation, `\transpose`, `\tuplet`/`\times`,
`~`, `(`/`)` and `\=N(`/`\=N)`, `\repeat volta`/`unfold` and `\alternative`,
`\key`, `\time`, `\clef`, `\partial`, `\bar`, `\cadenzaOn`/`\cadenzaOff`
(and `\set Timing.timing = ##f/##t`), `\language` (all languages of
`language-pitch-names`: Dutch, Catalan, German, English, Spanish,
French, Italian, Norwegian, Portuguese, Finnish, Swedish, Flemish), `\include`
of language files and files delivered without music, dynamics,
`\< \> \!`, common articulations and
ornaments, `\grace`, `\acciaccatura`, `\appoggiatura`,
`\afterGrace`, `R`/`s`, `<< \\ >>` voices (temporary polyphonic
passages) and `\new Voice` encountered in a staff voice (voice
opened at the current tick), variables (`name = { … }` then
`\name`), `\parallelMusic` (see below),
`\new Staff`/`PianoStaff`/`ChoirStaff`/`Voice`/`Lyrics`,
`\context Staff`/`PianoStaff`/`ChoirStaff`/`StaffGroup`/`Voice` (context
names case-insensitive: `\new voice`, `\context staff`…),
`\addlyrics`, `\lyricsto`, `\lyricmode`, `\header`, `\tempo`, `\mark` and
simple `\markup` (text), `\chordmode` in a staff (chord names are
**played**: `c:m7` → MIDI 60/63/67/70, tables of `scm/chord-entry.scm`),
`\drummode` and `\new DrumStaff` (drum names → MIDI pitches of
`ly/drumpitch-init.ly`), embedded music `#{ … #}`/`$#{ … #}` and simple music
functions (`name = #(define-music-function (a b) (predicate…) #{ … #})`
then `\name arg…`). A music expression written directly at the
top level of the file (implicit score of `toplevel-music-handler`)
is read as a `\score { music }` without `\layout` or `\midi`; each
top-level expression forms its own score, as LilyPond does (which
produces one MIDI file per expression);
`\book`/`\bookpart` are traversed to find `\score`, `\header` and
that top-level music.

**`\parallelMusic`** (`ly/music-functions-init.ly`, function `parallelMusic`):
music is split at bar checks `|` and the slices are
assigned cyclically to the named variables, one per voice; this is
the idiom of chord grids (`\parallelMusic #'(PianoI PianoII …)`, or
`\parallelMusic high,low` since 2.24). Bar checks are not
copied into the voices: the reader ignores them anyway (`|` does not cut
the framework), and a nested bar check (`\repeat`, `{ … }` block)
does not separate voices — LilyPond, for its part, also splits nested
elements. With no `|` at all, music is ignored with a warning,
as LilyPond does.

Everything outside this subset — unknown commands, unmodelled properties — is
**ignored with a collected warning**
(`read_lilypond_reporting`), never an error: only a source with
no music at all (`\score` or top-level expression) fails. Scheme
expressions `#…`/`$…` are an exception: they are read **as a whole**
(one complete datum, like the Guile reader called by the upstream
lexer) and silently ignored. A `#{ … #}` that is not part of a Scheme
datum is, on the other hand, **music**: the lexer makes a separate token
of it (`Kind::Embedded`) and the parser rereads its contents as a sequence,
respecting inner LilyPond strings and comments.

A definition `#(define-music-function (a b) (predicate…) #{ body #})` is
memoised (the old signature `(parser location a b)` is understood); each
call `\name arg…` consumes one argument per predicate (music `{…}`, note,
number, string, Scheme symbol, datum) then replays the body replacing
tokens `#a`, `$a` and `\a` with the argument: this is token substitution,
not Scheme evaluation. A body without `#{ … #}` leaves the
function unknown (single warning on first call, arguments
consumed); `define-void-function` and `define-scheme-function` are
consumed without music or an "ignored word" warning.

## 3. Notation choices

### `\fixed` mode, never `\relative`

Each voice is a variable whose body is wrapped in
`\fixed c' { … }`: the octave is **absolute**, `c` being middle C
(MIDI 60), `c'` the octave above, `c,` the octave below. A note can be
transposed or moved without changing the meaning of neighbouring notes, unlike
`\relative`; this is an explicit project requirement (`docs/TODO.md`).

### Explicit English language

`\language "english"` is declared at the top: accidentals are written `cs`
(C sharp), `bf` (B flat), `css`/`bff` (double). Without this line, the
file would depend on LilyPond's default language (`nederlands`).

### Writing conventions for round trips

Three conventions, chosen so the reader recovers the model without
ambiguity, are visible in the produced `.ly` files:

- **Voice gaps**: an interval with no element in a model voice
  is written `s` (spacer), not `r`; a real rest rendered invisible is written
  `\once \override Rest.transparent = ##t r…`. The reader can thus distinguish
  "no element" from "invisible rest".
- **N-tuplet number**: `TupletNumberType::None` is written
  `\override TupletNumber.stencil = ##f` … `\revert TupletNumber.stencil`,
  `Ratio` is written `\override TupletNumber.text =
  #tuplet-number::calc-fraction-text` … `\revert`.
- **Metadata**: non-standard `\header` fields (`workTitle`,
  `movementTitle`…) are accepted by LilyPond and make it possible to restore
  the `.mscx` metadata exactly.

### One variable per voice

```lilypond
staffOneVoiceOne = \fixed c' { … }
staffTwoVoiceTwo = \fixed c' { … }
```

The name serves both as a variable and as a `\lyricsto` target. Voices of a
staff with several voices receive `\voiceOne`, `\voiceTwo`… so that
LilyPond handles stems and collisions. In the `\score` block, a staff with
several voices and no lyrics uses the idiomatic `\\` separator:

```lilypond
\new Staff = "staffOne" <<
  { \staffOneVoiceOne }
  \\
  { \staffOneVoiceTwo }
>>
```

As soon as a voice carries lyrics, the voices become named `\new Voice`s,
the only ones accepted as a target by `\lyricsto`.

### Staff grouping

Based on MuseScore's brackets (`<bracket>`, `StaffBracket::kind` field):

| MuseScore bracket | LilyPond |
| --- | --- |
| brace (`1`) | `\new PianoStaff` |
| bracket, square bracket (`0`, `3`) | `\new ChoirStaff` |
| several staves without bracket | `\new StaffGroup` |
| a single staff | `\new Staff` |

Instrument names go through `\with { instrumentName = … shortInstrumentName = … }`.

## 4. Model → LilyPond mapping

### Score and measures

| Model | LilyPond |
| --- | --- |
| `ScoreMetadata` (and credits as fallback) | `\header { title, workTitle, subtitle, movementTitle, movementNumber, workNumber, composer, poet, arranger, copyright, source }`; `workTitle` is written only if the model carries a work title |
| `Measure.len < nominal_len` (first measure) | `\partial <duration>` |
| `TimeSig` | `\time n/d` |
| local `TimeSig` (actual length ≠ displayed measure) | `\time n/d \set Timing.measureLength = #num/den` |
| irregular `Measure.len` (outside pickup) | `\set Timing.measureLength = #num/den` before the measure, then restoration to the metre in force |
| `KeySig` | `\key <tonic> \major` or `\minor` |
| `Clef` | `\clef treble`, `bass`, `alto`, `tenor`, `"treble_8"`, `varbaritone`, `percussion`… |
| `Staff.default_clef` without a `Clef` at the head of the first measure | `\clef …` at the start of voice 0; `read4xx` does not materialise a `HeaderClef` segment, unlike `read302` |
| `Measure.multi_measure_rest` | `R1*3`, `R2.*4`… |
| `Measure.start_bar_line`/`end_bar_line` | `\repeat volta` for repeats, `\bar "|."`, `\bar "||"`, `\bar ";"` |
| `section_break_before` | `\section` |
| `line_break` | `\break` (except on the last measure) |
| `MeasureRepeatMark` | not rendered (the copied music is written) |
| bar check | `|` at the end of a measure when the actual length matches the metre in force, inherited ones included; `Timing.measureLength` is set otherwise, never a spurious `\time` |

### Chords, rests and durations

| Model | LilyPond |
| --- | --- |
| `Duration` + dots | `4`, `2.`, `8..`, `\breve`, `\longa` |
| `ChordRest.ticks` ≠ note value | multiplier `4*3/2` |
| `Chord` with one / several notes | `c'4` / `<c e g>4` |
| `Note.pitch` + `tpc` | spelled name (`cs`, `bf`) and absolute octave |
| transposed `Note` (`tpc_transposed`, instrument) | written pitch, like `pitch2xml` |
| courtesy accidental (`user`, parentheses, brackets) | `?` suffix |
| rest | `r4`; invisible (`print-object="no"`) → `s4`; whole measure → `R1` |
| `Tuplet` | `\tuplet 3/2 { … }`, nesting by braces |
| `Note.tie_forward` | `~` (on the element of `<c~ e g>` for a chord) |
| `GraceType` | `\grace`, `\acciaccatura`, `\appoggiatura`; `…After` types → `\afterGrace` |
| `ArticulationType` | `-.` staccato, `-!` staccatissimo/spiccato, `->` accent, `-^` marcato, `--` tenuto, `-_` portato, `\espressivo`, `\fermata` and variants, `\breathe`, `\caesura`, `\upbow`, `\downbow`, `\flageolet`, `\open`, `\stopped`, `\snappizzicato` |
| `OrnamentType` | `\trill`, `\turn`, `\reverseturn`, `\slashturn`, `\mordent`, `\prall`, `\prallprall`, `\haydnturn`, `\upmordent` |
| `Arpeggio` | `\arpeggio`, `\arpeggioArrowUp`/`Down`, `\arpeggioBracket` |
| `Tremolo` | `:8`, `:16`, `:32` (one bar = eighth note) |
| `Fingering` | `-1` for a digit, `-\markup { "T" }` otherwise |
| `ChordLine` | `\bendAfter`, the only standard command (fall, rise, plop and scoop are not distinguished by LilyPond) |
| `StemDirection` | `\once \override Stem.direction = #UP`/`#DOWN` |
| `NoteHeadGroup` | `\tweak style #'cross`, `#'harmonic-mixed`, `#'slash`, `#'xcircle`, `#'do`… |
| `Chord.grace_slash` | carried by `\acciaccatura` |

### Annotations

| Model | LilyPond |
| --- | --- |
| predefined `Dynamic` (`p`, `mf`, `sfz`…) | `\p`, `\mf` (with `_`/`^` according to placement) |
| custom `Dynamic` | `-\markup { "…" }` |
| `Tempo` | `\tempo "Allegro" 4 = 120` (text alone if there are no beats per second) |
| `StaffText`, `Expression`, `SystemText`, `InstrumentChange` | `^\markup { "…" }` / `_\markup` |
| `RehearsalMark` | `\mark \markup { "A" }`; a second mark at the same instant becomes a `\markup` |
| `Harmony` | `^\markup { "Cm7" }` (the model's text, not `\chordmode`) |
| `Fermata` | `\fermata`, `\shortfermata`, `\longfermata`, `\verylongfermata`, `\henzelongfermata`, `\henzeshortfermata` |
| `TboxText` | `^\markup` on the measure's first note |
| `FiguredBass`, `TremoloBar` | not rendered |

### Spanners

| Model | LilyPond |
| --- | --- |
| `Slur` | `\=N(` … `\=N)` (numbered for overlapping slurs) |
| `Hairpin` | `\<` … `\!` (or `\cresc`/`\decresc` for a text hairpin) |
| `Ottava` | `\ottava 1`/`-1`/`2`/`-2` … `\ottava 0` |
| `Pedal` | `\sustainOn` … `\sustainOff` |
| `Trill` | `\startTrillSpan` … `\stopTrillSpan` |
| `Glissando` | `\glissando` |
| `TextLine` | start and end texts in `\markup`, line not drawn |
| `Volta` | `\alternative` structure, no dedicated command |

### Lyrics

One `\new Lyrics \lyricsto "<voice>"` block per verse present in the voice.
Syllabification becomes ` --` (start or middle of a word) and the melisma ` __`.
Chords without a syllable for a verse yield `\skip 1` to keep
alignment; rests are skipped by `\lyricsto` itself. Several
verses receive `\set stanza = "n."` **inside** the lyrics block: this is
`musicxml2ly`'s form, compilable by LilyPond 2.26 (unlike
`\stanza "n."`, a music function added in 2.27). The
hyphen is emitted only if the following chord of the same verse carries a
syllable, and tie continuations (all notes tied from the previous chord
) receive no token, since `\lyricsto` skips them.

On the reading side, a lyric word is a sequence of non-whitespace
characters (`<lyrics>` of `lily/lexer.ll`, `lexer::mot_de_paroles`): attached
punctuation (`bon,`, `gne.`), apostrophe (`qu'il`, `l’ami`) and typographic
quotes are part of it, and a `_` **inside** the word becomes a space (`lyric_fudge`
upstream): `Ah_!` is a single syllable "Ah !". Only an isolated `_` between spaces
(`__` for the line) remains a melisma; a `~` separates words (elision).
Digits are outside the word: `la4` gives the syllable's duration. When writing,
a syllable containing a space is put in quotes (`"Ah !"`), the only
form that reads it back as a single word.

### Repeats

A start barline and an end barline (`<endRepeat>N</endRepeat>`) give
`\repeat volta N`. Ending voltas and the following endings become:

```lilypond
\repeat volta 2 {
  corps
}
\alternative {
  { premier ending }
  { second ending }
}
```

The body stops at the start of the first ending; an ending that occupies the whole
section stands in for the body. A repeat without a second ending is written as
a simple repeat (the music is the same on both passes).

## 5. Event attachment

The model stores objects in vertical segments; a LilyPond voice is a
linear sequence. The rules applied, in `src/voice.rs`:

1. chords and rests form the framework; gaps are filled by rests of the right
   length so that each voice exactly covers the measure;
2. clef, key signature, time signature, pickup and section break are placed at
   the start of the relevant measure;
3. annotations and spanners are attached to a note: the **last
   note at or before the tick** for an ending (`)`, `\!`) or a dynamic,
   the **first note at or after the tick** for a tempo or a mark;
4. at equal ticks, closings come before openings (`\!` then `\<`),
   so a hairpin stopping where the next one starts remains valid;
5. a hairpin whose start and end fall on the same rest is
   omitted (LilyPond rejects a zero duration).

## 6. Tests

Allocation: a test exercising only one direction lives in that direction's crate; a
round trip (`.ly` → model → `.ly`, or reading then export) lives in
`crates/scoretk/tests/` (facade) or in the integration crate. Neither
half depends on the other: reading compares to the MusicXML of the original
`.mscx`, not to a LilyPond rewrite.

- `crates/scoretk-lilypond-export/tests/corpus_lilypond.rs` converts the 56 `.mscx` files of the v3/v4 corpus
  and compares the result to the snapshots `tests/corpus/ly/<version>_<nom>.ly`.
  It also checks the header (`\version "2.24.0"`, `\language "english"`),
  the presence of `\fixed c'` and the absence of `\relative`.
  Regeneration after an intentional format change:

  ```sh
  nix develop -c env UPDATE_LILYPOND_SNAPSHOTS=1 \
      cargo test -p scoretk-lilypond-export --test corpus_lilypond
  ```
- `crates/scoretk-lilypond-import/tests/read_corpus.rs` rereads the corpus's 56 `.ly` files and compares the exported
  MusicXML to the source `.mscx`, at both levels of `e2e_corpus`. The lists
  `EXPECTED_CORE` and `EXPECTED_FULL` are **non-regressive**: a file listed
  there must stay identical. The test reports the discrepancies of the others.

  ```sh
  nix develop -c cargo test -p scoretk-lilypond-import --test read_corpus -- --nocapture
  SHOW_DIFF=1 nix develop -c cargo test -p scoretk-lilypond-import --test read_corpus -- --nocapture
  ```

- `crates/scoretk-lilypond-import/tests/read_lilypond.rs` covers what the writer does not emit: `\relative`,
  absolute notation, default Dutch, variables, repeats, `\transpose`,
  `\partial`, n-tuplets, ornaments, lyrics via `\addlyrics`, commands
  ignored with a warning, `\drummode`/`\new DrumStaff` (two staves,
  rests, durations, unknown name, control without mode) and `\chordmode` played in
  a staff (chords, bass, variable, `\relative`, `\chords` control).

- `crates/scoretk-lilypond-export/tests/lilypond_compile.rs` compiles each output with
  `lilypond` and fails on any error. It lives in the writer's crate: it only compiles `.ly` **produced** by the writer, never a
  read source, and the reader is not involved. The test does nothing if
  `lilypond` is not in the `PATH` (the case of `nix flake check`, which works
  offline):

  ```sh
  nix develop -c cargo test -p scoretk-lilypond-export --test lilypond_compile -- --nocapture
  ```

Measured state on 2026-11-17: **56/56 conversions without compilation errors**
with LilyPond 2.27 (the snapshots are at `\version "2.24.0"`, the reference
stable version; LilyPond applies its usual conversions).
Only `v3/moonlight.mscx` produces layout warnings
("page 1 compressed"), unrelated to the music.

### Measured round trip (t40)

Measured state on 2026-11-17 by `tests/read_corpus.rs`: **24/56 files
identical at the musical core level** and **3/56 at full parity**
(`v3_copypaste_tuplet_01`, `v3_grace`, `v3_lyricsline01`). The 24 core files are
listed in `EXPECTED_CORE`. The remaining discrepancies have three causes,
all documented:

1. **Information absent from the `.ly`** (the writer cannot write it):
   time-signature subtype (`C`, `C cut` become numbers), ending texts and
   numbers beyond the pass rule, explicit MIDI channels and
   programs, `part-name`/`instrument-name` distinction, chord
   symbols (`\markup` instead of `<harmony>`), gap rests
   split into several `<forward>`s (only one is re-emitted), unknown-mode
   key signature, ornament types (`\grace` does not distinguish 8/16/32);
2. **Layout**: positions, fonts, system breaks, widths
   (`<print>`, `default-x`, `relative-y`…) — out of scope, removed at
   the full level;
3. **Uncovered subset**: exotic `\bar`s, old modes,
   untuned percussion, micro-intervals.

The identical core proves that pitches, durations, measures, repeats, lyrics,
ties, key signatures, clefs and chords are read back correctly on more than a third
of the corpus; the remaining files serve as a measure of the writer's losses,
not of reader regressions.

## 7. Known limits

- **No layout**: neither margins, computed system breaks, nor
  text positions. `\layout { }` lets LilyPond decide.
- **Chord symbols in `\markup`**, not `\chordmode`: the model's text
  is preserved, without a chord grid or realisation.
- **Noteheads**: only non-standard groups go through
  `\tweak style`; filling and shape tied to duration
  (`NoteHeadType`) are not forced.
- **Accidentals**: LilyPond alone decides which accidentals to display from the
  `tpc` and the key. Editorial accidentals are not distinguished from
  others; only courtesy accidentals (`user`, parentheses,
  brackets) receive `?`.
- **Two-note tremolo**: written as a tremolo on each of the two
  notes (`:N`), not as `\repeat tremolo`.
- **Ornaments with accidental**: `<accidental-mark>`s are not rendered.
- **Articulations without equivalent** (`stress`, `unstress`, exact `spiccato`,
  soft-accent variants): approximation or omission, documented in
  `src/chord.rs`.
- **N-tuplets**: grouped by chord membership; a n-tuplet whose model
  gives an inconsistent end is closed at the measure, LilyPond having no
  n-tuplet crossing a barline. End-of-measure filling uses the
  actual duration (`DurationElement::actualTicks`, n-tuplet ratios included), and
  not the notated duration alone.
- **Repeats**: nesting and chaining several repeats on the same
  measure are not covered; multiple ending numbers (`1, 2`) do not change
  the number of `\alternative` blocks.
- **Local time signatures**: displayed, but `Timing.measureLength`
  imposes the measure's actual length; a staff in 3/4 within a
  measure of 9/8 is therefore not re-engraved as three 3/4 measures. Irregular
  measures without a local `<TimeSig>` (3/4 under an inherited 4/4) likewise receive a
  temporary `measureLength`, restored at the next regular measure.
- **Cross-staff chords** (`Chord.staff_move`): not rendered, the note stays on its
  staff.
- **Untuned percussion and micro-intervals**: the model does not yet carry
  them.
- **Text**: markup (`<b>`, `<font>`, `<sym>` symbols) is lost; characters from
  the private use area (MuseScore symbols) are removed to
  avoid missing glyphs. A tempo whose text is only a metronome
  symbol (`♩ = 80`) loses its label and keeps `4 = 80`.
- **Tempo unit**: `\tempo` is always written with the quarter note
  (`4 = bpm`), the model having no beat unit; a tempo at the half note or
  dotted half note is therefore notated with the quarter note.
- **Invisible tempo**: a `print-object="no"` tempo direction is not
  written at all (MuseScore nevertheless keeps its playback effect).
- **Lyrics**: no `\addlyrics`, only `\lyricsto`; syllables
  of a chord carrying several for the same verse are reduced to the
  first. A syllable on a tie continuation is lost,
  `\lyricsto` not attaching it to that note.

### Reading limits

- **Subset**, not full LilyPond: Scheme, `\figuremode`,
  `\partCombine`, `\paper` are ignored with a warning. `\chords` /
  `\new ChordNames` remains a display grid that is not played (as before);
  a conformant export would write `<harmony>` for these grids, which the pivot
  model can carry but the reader does not yet produce. `\book` and
  `\bookpart` are traversed (their `\header`, their `\score` and their
  top-level music).
- **`\context Staff`**: found at score level, it opens a
  staff (orchestral scores `\context StaffGroup << \context Staff =
  "high" … >>` are thus read as as many staves). Found in the
  music of an already open staff, it finds that staff again and its voices are
  opened there. When its content carries named `\new Voice`/`\context Voice` or a `\relative`/`\fixed`/`\transpose` wrapper, it is traversed
  as before the structural-context port: voices stay on the
  enclosing staff (the piano idiom
  `\context Voice = "one" \rightHand`) and the relative reference applies to
  the whole polyphonic block (`\context Staff \relative << … \\ … >>` from
  collisions.ly). On the other hand, a `\context Staff = "name"` whose content
  carries more than four named voices (Mutopia's pathetique-1) does open its
  staff and distributes the extra voices across sibling staves: grouping
  them into a single staff would make them collide with the four-voice limit.
- **`\rest` and `\relative`**: the written pitch of a `\rest` is resolved and
  advances the relative reference, like the `RestEvent` of `event_chord` in
  `lily/parser.yy` (verified on `d'2\rest d16\rest g, c e g` of bwv651.ly
  against LilyPond 2.27's MIDI). The drop in the number of "common notes"
  observed after treating `\rest` as a rest is therefore **not** a
  pitch change: the 124 removed notes are exactly the written rests, and 61 of them carried a pitch present in the
  reference MIDI (measured by multiset intersection of pitches). No
  played note changes pitch.
- **Voices opened within a voice**: `\new Voice` (or `\context Voice`) and
  each branch of a `<< … >>` encountered in a voice's music open
  a staff voice at the current tick (`context_prefix` /
  `NEWCONTEXT`, `lily/parser.yy`; "Temporary polyphonic passages"); the
  original voice then advances by the duration of the opened context, as
  LilyPond does. A simultaneous block without `\\` is read as sibling voices rather than
  merged chords. The anonymous branches of successive temporary passages
  share the same track as long as they do not overlap: a piece with
  many passages keeps two voices instead of opening one per passage and
  exceeding the model. Voices without a chord (preamble `\clef`/`\key`) are
  folded into the first musical voice, and a staff remains limited to
  four voices (later ones warn and are ignored).
- **`\change`**: `\change <Context> = <name>` is read as a staff change: the following notes keep their voice and place and carry a
  `Chord.staff_move` computed from the position of `\new Staff = "<name>"`.
  The context type is not checked; as with MuseScore's `Chord::staffMove`,
  resolution is limited to named staves of the same part. An unknown name
  or one foreign to the part warns once per voice, at the position of the
  `\change`, and leaves the notes on their original staff.
- **One score only**: when a file contains several (`\score`
  blocks or successive top-level expressions), the first explicit `\score`
  wins, otherwise the first expression; each other is ignored
  with a warning. This is the case of `regression/trill-spanner-consecutive.ly`
  (`\test 2.0 \test 2.1 \test 2.2 \test 2.3`), of which only the first
  expression (2 notes) is kept. The MusicXML represents the engraved score:
  even if a later `\score` carries a `\midi` (a score LilyPond plays,
  transposed), the first engraved `\score` prevails; any
  difference from the MIDI reference is a limit of the measurement tool.
  `\articulate` (`ly/articulate.ly`, a music function taking the following
  music) is transparent: it does not change pitches and lets `\transpose`/`\unfoldRepeats` wrappers pass through.
- **Music functions**: only `#(define-music-function …)` definitions with a `#{ … #}` body produce music.
  Substitution is purely lexical (`#a`, `$a`, `\a`): no Scheme
  call, no computed value, no music function defined in an
  `\include`d file. The number of arguments follows the predicates; an
  unknown predicate is read as a token (or a `{…}`/`<<…>>` block). Expansion depth of
  blocks and calls is bounded to 32, with a warning beyond; remaining arguments are consumed without being
  interpreted.
- **`\include`**: inclusion is textual and done on tokens, like the
  upstream lexer, both at top level and within music. A language file
  shipped by LilyPond acts as `\language`, files shipped without music
  (`articulate.ly`, `gregorian.ly`, `arabic.ly`…) are ignored without
  a warning. Other files go through the resolver of
  `read_lilypond_with_includes` (the CLI reads the disk); without a resolver, they
  are ignored with a warning. Maximum depth 32, cycles
  detected, `\include` of a Scheme expression ignored with a warning.
  Warning positions in the main file are preserved; those from included
  files carry their own line, without a file name.
- **No layout**: `\layout`, positions, hidden staves and computed breaks
  are not read back.
- **`\relative`**: the rule of `Pitch::to_relative_octave` is ported
  (`lily/pitch.cc`). In a chord, each note is placed relative to the
  previous one, but the pitch returned for what follows is that of the first
  note (`Event_chord::to_relative_octave`); `q` repeats the already resolved pitches of the last chord, not its written marks. A pitch outside
  0..127 is discarded with a warning. The branches of a `<< … >>` and
  the contexts of the sequence are traversed in the order of
  `Music_sequence::simultaneous_relative_callback` (`lily/music-sequence.cc`):
  each branch starts from the last note of the previous one, and the music that
  follows the `>>` uses the last note of the **last** branch (reading from
  the note before the `<<` shifted the rest by an octave).
- **`\transpose`**: LilyPond transposes the written music itself;
  `pitch`, `tpc` and `tpc_transposed` therefore all carry the transposed pitch
  and the key signature (`\key`) is transposed likewise (`\key c \major` under
  `\transpose c d` gives D major), without declaring a transposing instrument.
  Nested transpositions compose, for notes as well as for the key.
- **Key signatures**: `\major` is read as `KeyMode::Unknown`, because MuseScore
  writes `<mode>` only when explicit and the writer writes `\major`
  both for an explicit major and for a keyless key signature; `\minor` remains minor.
- **Time signatures**: `C`/`C cut` subtypes are not distinguished from
  `4/4`; `\set Timing.measureLength` is followed for the measure framework.
- **Cadenzas**: `\cadenzaOn`/`\cadenzaOff` and `\set Timing.timing = ##f`/`##t`
  suspend automatic barlines. The current measure lengthens until
  `\cadenzaOff` or the first explicit barline `\bar "…"`; its nominal
  duration remains that of `\time`, producing an irregular measure
  (`len != nominal_len`), as MuseScore does for an actual-duration measure.
  The cadenza may be cut into several measures by explicit barlines
  `\bar "…"`; it then resumes on the metre in force after `\cadenzaOff`. The bar check `|` (bar check of `lily/parser.yy`,
  `'|'` → `BarCheck`) is consumed and ignored: it never creates a barline or
  a break, even in a cadenza. An ordinary `\bar "|"` has the model's
  default value and does not modify a stronger barline at the same tick.
- **Cadenzas and multiple voices**: since measures are common to all
  voices, we choose the union of cadenza intervals: as soon as a voice is
  in cadenza, no automatic barline is inserted for the whole score
  during that interval, and the measure is lengthened until the
  nearest cadenza closes. This is the simplest behaviour; a voice-specific cadenza therefore does not split the other voices.
- **Rests**: `R` becomes a whole-measure rest (`multiMeasureRest`
  when it spans several measures); `s` (spacer) creates no element,
  reproducing MuseScore model gaps; a rest rendered invisible is written
  `\once \override Rest.transparent = ##t r…` and read back as
  `Rest::visible = false`. Whole-measure rests in voices other than
  the first are ignored: the writer used them to fill gaps.
- **Accidentals**: displayed accidentals are recomputed (key and
  last accidental of the measure, per step and octave); editorial accidentals
  and `force-accidental`s other than `?` are not distinguished.
- **N-tuplets**: the displayed number is encoded by `\override
  TupletNumber.stencil = ##f` (none) or `TupletNumber.text =
  #tuplet-number::calc-fraction-text` (ratio), read by the parser; the
  bracket is inferred from `Tuplet::calcHasBracket` for lack of information in
  `\tuplet`.
- **Lyrics**: `\lyricsto`/`\addlyrics` are attached to the named voice if
  it exists, otherwise to the first voice of the staff; `\skip` and
  rests receive no syllable, and extra syllables are ignored.
  Like LilyPond, `\lyricsto` skips occupied notes: those inside
  an expression slur `( … )` (except the note opening it) and
  `~` continuations. `\set melismaBusyProperties = #'()` disables these
  skips; `beamMelismaBusy` is not modelled because it only activates when
  `autoBeaming = ##f` (`lily/beam-engraver.cc`). The verse rank is read
  from `\set stanza = "n."` (the form written by the writer, compilable since
  2.26), from `\stanza "n."` (LilyPond 2.27) and, for compatibility, from
  the old `\with { stanza = "n." }`.
- **Repeats**: `\repeat volta N` with k `\alternative` blocks produces
  endings `i, i+k, i+2k…` (pass rule); nested repeats and
  explicit ending numbers outside this rule are not covered.
- **Spanners**: expression slurs, hairpins, ottavas, pedals,
  trills and glissandos are read back; their shape and text settings are
  not modelled.

## 8. Upgrading LilyPond (reading)

The reader follows the grammar of **LilyPond 2.26.0**, commit
`3596756be0fac1f81066a99d2054329212172b2d`. The reference copy is
**outside the repository** (like `ref-musescore/`), e.g.
`../lilypond-ref/`, and is never versioned.

**Get the copy** (partial clone, sufficient for the ported files):

```sh
git clone --filter=blob:none --sparse \
    --branch v2.26.0 https://github.com/lilypond/lilypond.git ../lilypond-ref
git -C ../lilypond-ref sparse-checkout set --no-cone \
    '/lily/parser.yy' '/lily/lexer.ll' '/lily/pitch.cc' '/lily/pitch.hh' \
    '/lily/include/*' '/scm/define-note-names.scm' '/COPYING' '/VERSION'
git -C ../lilypond-ref rev-parse HEAD   # doit afficher 3596756b…
```

The official mirror is <https://gitlab.com/lilypond/lilypond> (the GitHub
repository is a mirror); the stable version is also in nixpkgs
(`nix shell nixpkgs#lilypond`).

**File table**:

| Rust (`crates/scoretk-lilypond-import/src/`) | LilyPond |
| --- | --- |
| `lexer.rs` | `lily/lexer.ll` (states `notes`, `lyrics`, `markup`, comments) |
| `parser.rs` | `lily/parser.yy` (`music_list`, `event_chord`, `chord_body`, `post_events`, `repeated_music`, `context_change`, `\score`) |
| `pitch.rs` | `lily/pitch.cc` (`Pitch::to_relative_octave`, `steps`, `up_to`/`down_to`), `scm/define-note-names.scm` (tables by language) |
| `parallel.rs` | `ly/music-functions-init.ly` (function `parallelMusic`) |
| `ast.rs`, `events.rs`, `builder.rs` | no direct source: semantics of MuseScore's `engraving/dom/` objects |

**Spot the relevant changes**:

```sh
git -C ../lilypond-ref fetch origin
git -C ../lilypond-ref log --oneline 3596756b..origin/master -- \
    lily/parser.yy lily/lexer.ll lily/pitch.cc scm/define-note-names.scm
git -C ../lilypond-ref diff 3596756b..origin/master -- lily/parser.yy
```

Watch in priority: the `note_chord_element`,
`chord_body_element`, `post_events`, `duration`, `repeated_music`,
`alternative_music`, `context_change` productions; the lexer states and regular
expressions (`SYMBOL`, `FRACTION`, `RESTNAME`, `MULTI_MEASURE_REST`,
`EXTENDER`, `HYPHEN`); the name tables (`define-note-names.scm`) and
`Pitch::to_relative_octave`. Layout, Scheme and backend changes do not concern the reader.

**After a port**, regenerate and revalidate:

```sh
nix develop -c cargo test -p scoretk-lilypond-common -p scoretk-lilypond-import
nix develop -c cargo test -p scoretk-lilypond-import --test read_corpus -- --nocapture
nix develop -c cargo test -p scoretk-lilypond-import --test read_lilypond
nix develop -c cargo test -p scoretk-lilypond-export --test lilypond_compile -- --nocapture
nix flake check
```

Update the `EXPECTED_CORE`/`EXPECTED_FULL` list and the
"Measured round trip" table above. **When to do it**: when a
stable LilyPond version changes the syntax read (new command, modified lexer) or
when the corpus contains a file written by a newer version; not
to follow development versions.

## 9. Upgrading python-ly

`python-ly` (<https://github.com/frescobaldi/python-ly>, GPL-3.0-or-later)
served as a **model** for a Scheme-free reader, not as a ported source: no
line is copied. Consulted version: `master` of 2026-11-17 (licence
`LICENSES/GPL-3.0-or-later.txt`). To review it:

```sh
git clone --depth 1 https://github.com/frescobaldi/python-ly ../python-ly-ref
sed -n '1,80p' ../python-ly-ref/ly/music.py   # modèle d'événements
sed -n '1,80p' ../python-ly-ref/ly/lex.py     # lexer LilyPond sans Scheme
git -C ../python-ly-ref log -1 --format='%H %ad'
```

It is useful only for comparing approaches (time-stamped events, tolerance of
unknown commands): no file in this repository depends on it, and an
update requires neither regeneration nor tests.

## 10. Upgrading musicxml2ly (writing)

The writer ports no MuseScore C++ file (MuseScore 4.7 has no
LilyPond export): it follows the **tables and behaviour** of
`musicxml2ly`, shipped with **LilyPond 2.26.0** (same commit
`3596756b…` as reading, cf. § 8). Python sources, to add to the copy
outside the repository (never distributed):

```sh
git -C ../lilypond-ref sparse-checkout add --no-cone \
    '/scripts/musicxml2ly.py' '/python/musicexp.py' '/python/musicxml.py' \
    '/python/musicxml2ly_conversion.py' \
    '/python/musicxml2ly_definitions.py' '/python/musicxml2ly_globvars.py'
```

| Rust (`crates/scoretk-lilypond-export/src/`) | `musicxml2ly` |
| --- | --- |
| `pitch.rs` (names, accidentals, noteheads) | `musicexp.py` (`NotestyleEvent.notehead_styles_dict`, pitch names); `musicxml.py` (reading `<pitch>`) |
| `chord.rs` (articulations, ornaments, figures) | `musicexp.py` articulation and ornament tables |
| `annotations.rs` (dynamics) | `dynamics` of `musicexp.py` |
| `duration.rs`, `structure.rs`, `spanners.rs` | `\repeat`, n-tuplet and spanner constructions of `musicexp.py` |
| `writer.rs`, `voice.rs`, `text.rs` | `\score`/`\header` framework, event order and `\lyricsto` of `musicxml2ly.py` |

**Spot the relevant changes**:

```sh
git -C ../lilypond-ref fetch origin
git -C ../lilypond-ref log --oneline 3596756b..origin/master -- \
    python/musicexp.py python/musicxml2ly_conversion.py scripts/musicxml2ly.py
git -C ../lilypond-ref diff 3596756b..origin/master -- python/musicexp.py
```

Watch the name dictionaries (`notehead_styles_dict`, articulation
names, `dynamics`) and output constructions (`\repeat`,
`\tuplet`, `\fixed`/`\relative`, `\lyricsto`). Layout, MusicXML import and Scheme changes do not concern the writer.

**After an update**, regenerate the snapshots then revalidate:

```sh
nix develop -c cargo test -p scoretk-lilypond-import --test read_corpus -- --nocapture
nix develop -c cargo test -p scoretk-lilypond-export --test lilypond_compile -- --nocapture
# si une convention d'écriture change (silences, n-olets, en-tête) :
nix develop -c env UPDATE_LILYPOND_SNAPSHOTS=1 \
    cargo test -p scoretk-lilypond-export --test corpus_lilypond
```

**When to do it**: when a stable LilyPond version changes an
audible mapping (note name, articulation, duration) or a produced `.ly`
no longer compiles with the reference version; not to follow development
versions or layout changes.

## 11. Empty-output audit (2026-11-19)

A corpus-wide comparison of written pitches against LilyPond MIDI references
found 97 `.ly` files that produced no notes although the reference did (76
`input/regression` tests and 21 Mutopia scores, 6925 reference notes in total).
Causes were grouped by yield (files × reference notes) and handled in that
order.

### Fixed: unknown commands swallowed the following music block

`skip_command_arguments` treated a `{ … }` block after an unknown command as
that command's argument. No-argument commands such as `\autoBeamOff`,
`\dynamicUp`, `\dynamicDown`, `\oneVoice` and `\compressMMRests` are ubiquitous
in the Mutopia idiom

```lilypond
\context Voice = Sopran { \voiceOne << \autoBeamOff \dynamicUp { \global \voice } >> }
```

so an ignored command at the head of the simultaneous block ate the only music
branch and the whole voice vanished. In music context the braced block is now
left in the stream; at top level, where `\layout`/`\paper` carry a settings
block, it is still skipped. This recovered 5198 of 5202 notes in all 21 Mutopia
scores, plus five regression files; 5247 notes now match in total. A reduced
test covers two staves, two named voices, variables, bar checks, chords, rests
and ties, plus an unknown command at end of file. Twenty previously exact files
were re-measured and are unchanged.

A later measurement on non-regression scores caught a side effect: the
`musicxml2ly` idiom `\mark \markup { \box { A } }` left the `\markup` token in
the stream, so its braced block was parsed as music and the letter `A` became a
spurious note. `\mark` now consumes `\markup` before calling
`parse_markup_text`, which drains the whole block (including nested braces).
`Angels_Carol.ly` recovers its 12 shared notes and now also exports its eight
rehearsal marks; the no-argument-command fix above is unchanged.

### Not fixed: causes abandoned as disproportionate

The remaining 71 regression files (1665 reference notes) fall into groups that
would each require a feature of their own, well beyond a parser fix:

- ~~**Chord grids played as notes**~~ **Fixed in §12**: `\chordmode` inside a
  `Staff` now produces the played notes via a chord-name-to-pitch table
  (`c:m7`, `c/g`, `dim7`, …); `\chords`/`ChordNames` remain display-only grids.
- ~~**Drum pitches**~~ **Fixed in §12**: `\drummode` abbreviations (`bd`, `sn`,
  `hh`, `cymr`, …) now map through LilyPond's `drumpitch-init.ly` tables. The
  pivot model still has no untuned-percussion pitch, so the MusicXML writer
  emits `<pitch>` with the GM MIDI note number; §12 documents how the oracle
  compares those notes to LilyPond's MIDI.
- **Missing installed templates** (7 SATB/SSAATTBB files, ~360 notes). They end
  in `\include "satb.ly"` / `"ssaattbb.ly"` and contain only variable
  definitions; the real `\score` lives in LilyPond's installed `ly/` directory.
  The CLI resolver searches the input file's directory and the library must
  remain I/O-free, so the include cannot be resolved here.
- **Scheme-generated music and partial music functions** (`ledger-line-properties`,
  `note-head-style`, `beat-performer`, `markup-map`, `parallelmusic-partial`, …).
  These rely on `$@(map …)`, `#{ … #}` splicing, `\etc` partial definitions or
  the drum tremolo repeat `\*`; supporting them would mean embedding a Scheme
  evaluator.
- **Small leftovers** (~15 files, ~90 notes): quoted variable names
  (`"|" = \bar "||"`), `\with` followed by a context modifier instead of a
  braced block, braced markup arguments inside property values, and a spurious
  empty first partition that shadows the real one. Each affects one or two
  notes and does not justify widening the subset.

These files are expected to keep producing zero notes; the reader's narrow,
Scheme-free subset is unchanged.

## 12. Measurement (oracle and reader audited 2026-11-19)

The versioned oracle in `scripts/oracle/lilypond/` compares the MIDI rendered
by LilyPond from the same `.ly` source with the MusicXML produced by each
reader. N1 is the multiset of pitches (as in the historical `xmlcmp/cmp.py`);
N2 is the **multiset intersection of `(pitch, quantized attack)` pairs** on the
1/16-of-a-quarter grid (`GRID = 0.25`). Percentages are weighted by reference
note counts. `scripts/oracle/lilypond/README.md` documents the conventions and
the reproduction commands.

### Oracle: one cursor per part, not per voice

Earlier versions of `read_xml()` keyed the attack counter by voice, and before
that by `(voice, staff)`. Neither is MusicXML semantics: a part has a **single
cursor**. The reader now implements it literally: a note without `<chord/>`
advances the cursor by its `<duration>` (converted with the current
`<divisions>`), a `<chord/>` note shares the previous attack, `<backup>` and
`<forward>` move the cursor in both directions, a new measure starts at the
previous measure's start plus that measure's **maximal cursor extent**, and a
`<grace>` note has no duration. A tie stop is not an attack but still occupies
its time. This subsumes the two former special cases (a leading `<forward>`
for a voice's first note; ignoring later `<backup>`/`<forward>` as "stream
artefacts"): they are now ordinary cursor moves.

`ly_notes.py selftest` was extended **before** the change and failed on the
old code: with the voice-keyed counter, A3 on a piano's second staff (voice 1
reused after a `<backup>`) landed at 4.0 quarters instead of 1.0. The retained
tests cover two staves sharing voice 1, a voice changing `<staff>`, chords
(shared attack, chord at end of measure), a `<divisions>` change with a
partial-measure `<forward>` and a mid-measure `<backup>`, a grace note, and
`<unpitched>` notes.

The 29 files that the voice-keyed counter had penalised relative to the older
`(voice, staff)` key were identified again from the full campaign's converted
MusicXML by replaying the exact predecessor oracle from commit `04374e5^`:
**28** of the 29 are reproduced (the last belongs to a conversion changed by
the later transposition fix). On every one of the 4543 measurable files the cursor and
voice models give the same N2 file by file: the cursor does not recover those
28, because their writers omit the `<backup>` that would make a voice reused
on two staves explicit. The cursor model is kept because it is the MusicXML
definition and fixes cross-staff cases; the 28 files remain in the raw rows for
inspection.

### Reader: chord mode and drum mode are played

LilyPond plays `\chordmode` events in a `Staff`; the same music in a
`ChordNames` context is display-only. The reader now resolves chord names to
played pitches (`c:m7` → 60/63/67/70, `c/g` → triad plus bass 55) with a table
modelled on `scm/chord-entry.scm`. The lexer splits `c:m7` into `c`, `:`, `m7`,
so the name is reassembled before lookup. `\chords`, `
ew ChordNames` and
`\context ChordNames` stay non-playing grids (a MuseScore-conformant export
would write `<harmony>` there; the reader does not produce harmony events yet).
A pre-existing leak in the `\context ChordNames` branch — invisible while the
grid was empty — became audible once its names were readable: the grid used to
turn into an extra staff. It is now skipped, with a test for both `
ew` and
`\context`.

`\drummode`, `\drums` and `
ew DrumStaff` now read the abbreviations of
`ly/drumpitch-init.ly` (`bd` 36, `sn` 38, `hh` 42, `cymr` 51, …) as notes. The
MIDI numbers were verified against LilyPond 2.27 MIDI for all 129 drum names.
The pivot model has no untuned-percussion pitch, so the MusicXML writer emits
`<pitch>` with the GM number; the oracle compares those directly with
LilyPond's MIDI, which is the meaningful drum comparison. `<unpitched>` from
other writers is counted from `<display-step>`/`<display-octave>`; python-ly
writes the same B4 head for every drum and carries no MIDI number, so its drum
pitches cannot be matched — a documented oracle limitation. The regression
examples followed are `input/regression/chord-changes.ly` and
`input/regression/hara-kiri-drumstaff.ly`.

### Full campaign, in the foreground

Three lots from column 6 of the `-base.txt` lists (`mus` 1447, `ra` 1726,
`mut` 1377 paths). Every conversion ran under
`timeout 10 prlimit --as=2000000000`; the MIDI references are LilyPond's,
precomputed in the out-of-repo `lyaudit/out3*` tree with `pre.ly` injecting a
`\midi` block. The full three lots were re-run **in the foreground** at each
step, with no timeout. `python-ly` is unaffected by this campaign.

| Lot | Step | Files | Repeats | OK | Fail | Empty | Exact | N1 % (all) | N2 % (no repeat) | Max time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| mus | before (voice key, old reader) | 1447 | 113 | 1444 | 3 | 0 | 1079 (74.6 %) | 99.09 | 85.19 | 0.36 s |
| mus | cursor oracle (old reader) | 1447 | 113 | 1444 | 3 | 0 | 1071 (74.0 %) | 99.09 | 88.61 | 0.21 s |
| mus | cursor + reader | 1447 | 113 | 1444 | 3 | 0 | 1072 (74.1 %) | 99.09 | 88.62 | 0.28 s |
| ra | before (voice key, old reader) | 1726 | 305 | 1724 | 2 | 69 | 1158 (67.1 %) | 88.66 | 70.06 | 0.15 s |
| ra | cursor oracle (old reader) | 1726 | 305 | 1724 | 2 | 69 | 1190 (68.9 %) | 88.66 | 73.99 | 0.08 s |
| ra | cursor + reader | 1726 | 305 | 1724 | 2 | 55 | 1198 (69.4 %) | 89.84 | 75.01 | 0.06 s |
| mut | before (voice key, old reader) | 1377 | 652 | 1375 | 1 | 0 | 269 (19.5 %) | 97.92 | 58.28 | 10.02 s |
| mut | cursor oracle (old reader) | 1377 | 652 | 1376 | 1 | 0 | 369 (26.8 %) | 97.73 | 67.85 | 8.04 s |
| mut | cursor + reader | 1377 | 652 | 1376 | 1 | 0 | 369 (26.8 %) | 97.73 | 67.85 | 5.44 s |
| **all** | **before** | **4550** | **1070** | **4543** | **6** | **69** | **2506 (55.1 %)** | **97.84** | **63.51** | **10.02 s** |
| **all** | **cursor oracle** | **4550** | **1070** | **4544** | **6** | **69** | **2630 (57.8 %)** | **97.67** | **71.80** | **8.04 s** |
| **all** | **cursor + reader** | **4550** | **1070** | **4544** | **6** | **55** | **2639 (58.0 %)** | **97.70** | **71.83** | **5.44 s** |

The oracle change alone raises N2 by 8.29 points and exact outputs from 2506
to 2639 after the reader change; N1 is unchanged by the oracle (same notes,
only attacks move) and moves only where the reader now plays chord and drum
events. Per-file, oracle before → cursor: **975 files gain N2, 449 lose N2,
268 gain exact, 144 lose exact, none gains or loses N1.** Every decrease is the
same documented mechanism — honouring `<backup>`/`<forward>` and per-measure
extents moves attacks — so none is unexplained. Cursor → final: **16 files
gain N1, 16 gain N2, 9 gain exact, and no file loses any of the three.**
The two earlier apparent regressions (a `\chordmode` grid used in `\context
ChordNames` turning into an extra staff, and a similar `
ew ChordNames` case)
were traced to the context leak above and fixed before the final run, rather
than left as unexplained differences.

Raw rows, converters and the generated summaries are in the out-of-repo
measurement directories `lyn3-avant`, `lyn3-oracle` and `lyn3-final`;
`scripts/oracle/lilypond/README.md` documents how to reproduce them.

## 13. Writing export: baseline against musicxml2ly (2026-11-19)

### Method

`scripts/oracle/lilypond/ecriture.py` measures the reverse direction of §12:
from a source `S` it writes scoretk's `.ly` (`scoretk S --to ly`) and
musicxml2ly's `.ly`, compiles both with LilyPond 2.26 under
`timeout 20 prlimit --as=2000000000` (`-dmidi-extension=midi`), and compares
the resulting MIDI with the reference notes of `S` using the MIDI ↔ MIDI
N1/N2/N2d of `scripts/oracle/midi/midi_notes.py` (Q16 grid). A `\midi` block
and an `\unfoldRepeats` wrapper are injected into a copy of **both** candidates
so that neither is penalised for LilyPond's default behaviour; failures are
recorded, never silently dropped.

Four lots were measured: the 56 versioned corpus `.mscx`, the 174 `.mscz` of
`~/Musique`, the 32 genuine MusicXML files of `~/Musique`, and a deterministic
1/5 sample of its `.ly` files (375). The reference MIDI is MuseScore 4.7.4's
for `.mscx`/`.mscz`/`.xml`/`.mxl`, and LilyPond's MIDI of the source for the
`.ly` round trips. musicxml2ly receives, in order: the versioned
`*.ref.musicxml` (corpus), MuseScore's own MusicXML export (`.mscz`), the
source itself (`.xml`/`.mxl`), or scoretk's MusicXML (`.ly`). Every row records
this origin in `xml_source`, so the report always states which input was used.
Each conversion ran under `timeout 10 prlimit --as=2000000000`.

"Errors"/"Warnings" below are the counts of lines matched by the oracle's
`Erreur|error` / `Avertissement|warning` regexes in LilyPond's stderr; they are
an indicator, not a musical judgement. Percentages are weighted by reference
note counts. On the XML lot musicxml2ly fails internally on 28 of 32 files
(`UnboundLocalError: part_list`), so its 98.79 % is measured on a single file:
this is an alternative's defect, recorded as such.

### Baseline (frozen `scoretk-base`, before the fixes below)

| Lot | Tool | Files | Compiled | Exact | N1 % | N2 % | N2d % | Errors | Warnings | Mean .ly bytes | Max time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 22 (39.3 %) | 78.19 | 76.12 | 73.66 | 0 | 11 | 1398 | 1.42 s |
| corpus | musicxml2ly | 56 | 51 | 28 (50.0 %) | 94.52 | 91.98 | 89.29 | 79 | 20 | 2929 | 20.00 s |
| mscz | scoretk | 174 | 77 | 18 (10.3 %) | 65.14 | 48.22 | 46.68 | 2782 | 1598 | 28225 | 16.77 s |
| mscz | musicxml2ly | 174 | 143 | 34 (19.5 %) | 93.12 | 91.45 | 90.90 | 80988 | 3820 | 41699 | 30.95 s |
| xml | scoretk | 32 | 19 | 6 (18.8 %) | 85.17 | 72.90 | 71.83 | 148 | 251 | 103892 | 23.07 s |
| xml | musicxml2ly | 32 | 2 | 1 (3.1 %) | 98.79 | 98.79 | 98.79 | 326 | 24 | 19391 | 1.50 s |
| ly | scoretk | 375 | 269 | 214 (57.1 %) | 90.19 | 75.81 | 74.98 | 97 | 189 | 1953 | 5.36 s |
| ly | musicxml2ly | 375 | 118 | 79 (21.1 %) | 94.87 | 85.76 | 85.11 | 0 | 258 | 3712 | 6.44 s |

### Loss grouping and fixes

Grouping the files where musicxml2ly was exact and scoretk was not (21 of the
first 119 `.mscz`), by first cause:

1. **Compilation failures (8 files)** — two independent causes: `\stanza`
   (a LilyPond 2.27-only function) in scores with several verses, and
   identifiers such as `staff11VoiceOne` for staves numbered 11+, which
   LilyPond parses as `staff` followed by an integer. Both are fixed: the
   writer now emits `\set stanza = "n."` (musicxml2ly's 2.26-compatible form,
   also read by our reader) and spells numbers beyond ten (`staffOneOne`).
2. **Shifted attacks (7 files)** — `01- Magnificat` and six other movements
   had N1 = 100 % but N2 ≈ 6–13 %: a multi-measure rest was written twice
   (`R2.*30` followed by 29 individual `R2.`), adding 30 bars. The writer now
   skips the covered measures and uses the nominal bar length for the
   multiplier. On `01- Magnificat` the fixed output goes from 11.75 % to
   100 % N2d. A first version skipped every following measure unconditionally
   and dropped notes from `.ly` sources (`R2*N` followed by real music): the
   final version skips a measure only when all its items are rests, which
   restored the 22 notes of `PriereASteJeanneDArc.ly` and the 139 of
   `Angels_Carol.ly`.
3. **Wrong pitches (5 corpus files)** — instrument transposition was written
   as altered note names but no `\transposition`, so LilyPond played the
   written pitch instead of the concert pitch (N1 = 0 %). The writer now emits
   `\transposition <pitch>` as musicxml2ly does.
4. **Unfixable here (4 files)** — MuseScore 1.x/2.x input (`version MuseScore
   non prise en charge : 2.06/1.14`): a reader limitation, outside the scope
   of this export campaign.
5. **Duration-only differences (2 files)** — 15 notes of 1017 (~1.5 %) differ
   only in N2d; likely tie/gate-time detail, not worth a targeted fix.

No file was dropped: every fix has a reduced test, and the only
snapshots regenerated are the seven files whose `.ly` legitimately changed
(five `\transposition`, one extra `R1` removed, one multi-rest duplication).

### Final campaign (all fixes, frozen `scoretk-final2`)

| Lot | Tool | Files | Compiled | Exact | N1 % | N2 % | N2d % | Errors | Warnings | Mean .ly bytes | Max time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 28 (50.0 %) | 79.91 | 78.36 | 75.90 | 0 | 11 | 1397 | 1.79 s |
| corpus | musicxml2ly | 56 | 51 | 28 (50.0 %) | 94.52 | 91.98 | 89.29 | 79 | 20 | 2927 | 20.00 s |
| mscz | scoretk | 174 | 150 | 28 (16.1 %) | 92.26 | 85.33 | 83.32 | 2 | 1785 | 27313 | 5.57 s |
| mscz | musicxml2ly | 174 | 143 | 34 (19.5 %) | 93.12 | 91.45 | 90.90 | 80988 | 3820 | 41697 | 28.48 s |
| xml | scoretk | 32 | 24 | 12 (37.5 %) | 99.19 | 53.41 | 50.56 | 66 | 251 | 103888 | 23.18 s |
| xml | musicxml2ly | 32 | 2 | 1 (3.1 %) | 98.79 | 98.79 | 98.79 | 326 | 24 | 19385 | 1.08 s |
| ly | scoretk | 375 | 289 | 216 (57.6 %) | 90.19 | 78.20 | 77.61 | 0 | 134 | 1952 | 2.48 s |
| ly | musicxml2ly | 375 | 118 | 79 (21.1 %) | 94.87 | 85.76 | 85.11 | 0 | 258 | 3710 | 3.34 s |

Per-file, baseline → final for scoretk: **6 gains / 0 losses** on the corpus,
**89 / 0** on the 174 `.mscz`, **11 / 0** on the XML lot and **8 gains / 3
losses** on the `.ly` sample. The three `.ly` losses are timing-only: note
count and N1 are unchanged (`GaudeteChristusEstNatus` 1.77 → 0.37 % N2d,
`Gevaert_LeMessageDesAnges-a4` 74.77 → 73.52 %, `GaiDuguesclin` 45.90 →
44.26 %). They come from the intended multi-rest normalisation
(`R1*11/2*11` → `R2*11`), which moves bar checks and therefore a few attacks
on the 1/16 grid; no note is lost. On `.mscz` the gain is concentrated in
compilation (77 → 150 files) and attacks (N2 48.22 → 85.33 %), with slightly
smaller output on average (28 225 → 27 313 bytes). On the `.ly` round-trip
sample scoretk stays well ahead of the alternative (216 vs 79 exact files).
Overall scoretk matches or beats musicxml2ly on three lots and is within
7.6 N2d points and 6 exact files on `.mscz`, with far fewer error lines and
output about a third smaller.

### Remaining known limits

- **Ottava (`v4_partExclusion`)** — MuseScore's MusicXML export writes the
  *sounding* pitch under an octave-shift mark, while the pivot model stores
  the *written* pitch. LilyPond's `\ottava` is display-only, so those notes
  are played an octave low. Fixing it would require changing the model↔`.ly`
  octave convention for every ottava and risks round-trip regressions; left
  as a documented one-file difference.
- **Glissando** — MuseScore's reference MIDI renders glissandi as chromatic
  runs, LilyPond's MIDI does not. Both converters lose those notes against
  the reference: an oracle artefact, not a scoretk defect.
- **`.ly` references without `\midi`** — about 7 % of the `.ly` sample
  consists of files with no playable `\score`; LilyPond emits no MIDI for
  them, so their reference is absent (recorded as `echec`, not excluded).

### Second pass: writing `.mscz` (2026-11-19)

#### Method and frozen baseline

The `scoretk` CLI was frozen before any edit (`128e5265…`, `scoretk-base`)
and the oracle re-run on the 56 corpus `.mscx` and the 174 `.mscz` under
`timeout 10 prlimit --as=2000000000` (LilyPond compilation under 20 s / 2 GB).
The frozen binary was later rebuilt as `97cc14bc…` (`scoretk-final2`) after
the single retained fix; reference MIDI and MusicXML were shared between both
campaigns through the oracle's `refs/` cache. The four lots are the same as
above: 56 corpus `.mscx`, 174 `.mscz`, 32 genuine MusicXML and a deterministic
1/5 sample of 375 `.ly`.

Baseline (frozen `128e5265…`, corpus and `.mscz` only):

| Lot | Tool | Files | Compiled | Exact | N1 % | N2 % | N2d % | Errors | Warnings | Mean .ly bytes | Max time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 28 (50.0 %) | 79.91 | 78.36 | 75.90 | 0 | 11 | 1397 | 3.11 s |
| corpus | musicxml2ly | 56 | 51 | 28 (50.0 %) | 94.52 | 91.98 | 89.29 | 79 | 20 | 2977 | 20.00 s |
| mscz | scoretk | 174 | 149 | 28 (16.1 %) | 92.26 | 85.33 | 83.32 | 2 | 1778 | 27313 | 23.34 s |
| mscz | musicxml2ly | 174 | 142 | 34 (19.5 %) | 93.12 | 91.45 | 90.90 | 80988 | 3812 | 41748 | 40.96 s |

Final campaign (rebuilt `97cc14bc…`, all four lots):

| Lot | Tool | Files | Compiled | Exact | N1 % | N2 % | N2d % | Errors | Warnings | Mean .ly bytes | Max time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 28 (50.0 %) | 79.91 | 78.36 | 75.90 | 0 | 11 | 1396 | 7.61 s |
| corpus | musicxml2ly | 56 | 51 | 28 (50.0 %) | 94.52 | 91.98 | 89.29 | 79 | 20 | 2986 | 20.03 s |
| mscz | scoretk | 174 | 149 | 28 (16.1 %) | 92.26 | 85.33 | 83.32 | 2 | 1902 | 27363 | 23.08 s |
| mscz | musicxml2ly | 174 | 143 | 34 (19.5 %) | 93.12 | 91.45 | 90.90 | 80988 | 3820 | 41755 | 39.41 s |
| xml | scoretk | 32 | 24 | 12 (37.5 %) | 97.81 | 95.34 | 94.67 | 66 | 251 | 103899 | 23.13 s |
| xml | musicxml2ly | 32 | 2 | 1 (3.1 %) | 98.79 | 98.79 | 98.79 | 326 | 24 | 19441 | 4.34 s |
| ly | scoretk | 375 | 289 | 216 (57.6 %) | 90.19 | 78.20 | 77.61 | 0 | 134 | 1952 | 6.04 s |
| ly | musicxml2ly | 375 | 118 | 79 (21.1 %) | 94.87 | 85.76 | 85.11 | 0 | 258 | 3765 | 7.64 s |

#### Loss grouping (frozen baseline, files where musicxml2ly beats scoretk)

Every group has one reduced example; the first three are *not* export defects.

1. **MuseScore 1.x/2.x input** (22 files of the 174 `.mscz`, 21 in 2.06 and
   one in 1.14) — e.g.
   `Jig_-_Tripping_up_the_Stairs.mscz`: `version MuseScore non prise en
   charge : 2.06`. The reader rejects the file before the writer runs, so no
   export-side change can recover it. Documented reader limit, out of scope.
2. **Multi-rest count disagrees inside MuseScore's own files** (about 20
   orchestral parts) — e.g. `01- Les anges - Violons 1 et 2.mscz`: the `.mscx`
   carries `<multiMeasureRest>13</multiMeasureRest>` on a measure of `len=
   52/4`, but MuseScore's own MusicXML export (given to musicxml2ly) writes
   only six rest measures there, and its reference MIDI places the next notes
   28 quarters earlier. scoretk faithfully writes `R1*13`; the two views of
   the same score differ by seven bars. **A tentative export fix was tried and
   reverted**: making the writer consume one more duplicate rest measure
   changed nothing on `Les anges` (N2d 38.03 → 37.24) and regressed more than
   thirty other scores (`01- Magnificat` 1174 → 610, `Once in royal - Violons
   1 et 2` 1731 → 1394, `02- Flûtes 1 et 2` 1597 → 371) because those extra
   rest measures are genuine. The discrepancy is in the input, not in the
   writer; fixing it would mean changing the MuseScore reader's measure model.
3. **MuseScore playback-only octave coupling** — e.g. `Adios_Muchachos.mscz`
   (Fisarmonica accordion): the `.mscx` has 444 notes and scoretk writes all
   444; MuseScore's reference MIDI has 636, of which 143 are an octave away
   from any written note. musicxml2ly also misses 94 of them. This is a
   reference artefact, not a scoretk defect.
4. **Tremolo rendered as repeated notes** — e.g. `timesig-09.mscx`: 20 written
   chords with `:32` tremolo become 320 strokes in MuseScore's reference MIDI,
   while LilyPond sustains them (20 notes). musicxml2ly fails to compile this
   file (`UnboundLocalError` in `group_tremolos`), so it is absent from its
   aggregate denominator and scoretk's 6.25 % N1 drags the corpus average
   down. Add this to the list of reference artefacts alongside glissando.
5. **Invisible / cross-staff elements** — e.g. `testExcludeInvisibleElements`
   (N1 47/75 vs 53/75), `copypaste_parts` (10/44 vs 26/44) and
   `partExclusion` (8/12 vs 12/12, the documented ottava limit). These need
   information the pivot model does not carry; documented limits of §7/§12.
6. **Duration-only** — e.g. `O salutaris Hostia - Saint-Saëns.mscz`: 15 notes
   of 1017 differ only in gate time (0.25 vs 0.125 quarter); every pitch and
   attack matches (N1 = N2 = 1017). Not worth a targeted fix.
7. **Timing-only** — e.g. `Waltz_No._2.mscz`: N1 100 %, N2d 98.34 vs 99.19;
   one stray attack on the 1/16 grid, no note lost.

#### Retained fix: redundant octave transposition

A pure octave displacement is not a harmonic transposition. Writing the
concert pitch directly and omitting `\transposition c` gives **exactly the
same MIDI** (measured on `v4_hairpin_simple`: 59/60/62/64 at 0/1/2/3 quarters,
identical to the reference) while shortening the file. The writer now maps
every `transpose_chromatic` multiple of 12 to zero before both the pitch
spelling and the `\transposition` event; real chromatic transpositions
(clarinette en si bémol, cor en fa) keep their written pitch and command.
One unit test and the three `v4_hairpin_*` snapshots were updated; no other
snapshot changed. A residual limit: a *chromatic* transposition followed by a
non-transposing instrument does not emit an identity reset, so LilyPond's
`\transposition` property persists — pre-existing and rare.

#### Result and remaining losses

Baseline → final: **no per-file gain and no per-file loss** on N1 or N2d (92
unchanged comparisons on the corpus, 290 on `.mscz`). The retained
fix is byte-level only: the corpus mean `.ly` size falls from 1397 to 1396
bytes and the `.mscz` mean from 27 313 to 27 363 (the latter varies with the
metadata of the scores actually compiled). Musically scoretk stays ahead of
musicxml2ly on the XML lot (12 exact files vs 1) and on the `.ly` lot
(216 vs 79); it ties on exact files on the corpus and trails by six on
`.mscz`, where the 22 unreadable MuseScore 2.x files alone account for most of
the gap.

The corpus aggregate (N1 79.91 vs 94.52) is **not** a per-file regression.
musicxml2ly fails to compile three files that scoretk measures (`timesig-09`,
`repeat39`, `testHarmony6`); `timesig-09` alone carries 320 of the 2320
reference notes of the lot and scoretk scores 6.25 % there because of the
tremolo artefact above. Restricted to the 46 files measurable by both tools,
the corpus figures are **92.18 % N1 / 88.73 % N2d for scoretk against
94.52 % / 89.29 % for musicxml2ly** — a 0.56-point N2d gap, entirely the four
documented cases. No file was dropped from either report.

## 14. MIDI writing: the `ecriture_ly` causes (2026-11-19)

### Method and frozen baseline

The `ecriture_ly` lot of the MIDI oracle (`scripts/oracle/midi/`) compares
scoretk's MIDI of each `.ly` with the MIDI **LilyPond itself** produces from
the same source. The baseline binary was built once and frozen as
`scoretk-base`; `lots.sh` + `mesure.sh` were rerun with `LAKH_STRIDE=20`, every
conversion under `timeout 10 prlimit --as=2000000000`. The two MIDI reading
lots and the LilyPond reading oracle (`scripts/oracle/lilypond/`, same
manifests, `--stride 10`, scoretk only, cached LilyPond references) are
remeasured as non-regression. The baseline reproduces the published figures:
`ecriture_ly` 753/1020 exact, `lecture_musique` 596/1020, `lecture_mutopia`
49/122; the LilyPond oracle gives 111/145 (`mus`), 119/173 (`ra`) and 46/138
(`mut`).

### Cause found and fixed: LilyPond plays chord grids

LilyPond 2.26 plays `\chords` and both `ChordNames` contexts (`\new` and
`\context`) in its MIDI; only `FiguredBass` grids are silent. This was checked
by compiling reduced sources: `\new ChordNames \chordmode { c1 d:m }` beside a
staff yields a first track with the chord notes. The reader skipped those
grids, so all their notes were missing. Example `03_08_LaCiteDeDieu.ly`: 107
reference notes of 319 were absent (N1 66.14 %, N2 36.36 %).

The parser now routes `chords` like `chordmode` and lets both ChordNames forms
produce an ordinary played staff; `figures`/`FiguredBass` remain skipped. The
reduced test `grille_daccords_jouee_dans_le_midi` covers `\chords` beside a
staff with a bar check, `\new ChordNames` with `\with`, `\context ChordNames`
with a variable, exact pitches/durations/measure counts and an unaffected
`FiguredBass` case. Three older tests asserting the former, incorrect
"display-only" behaviour were updated at the same level of detail.

### Octave: `\transpose` inside `\relative` (third pass)

A second cause was found by reading the specification rather than the
measurement. Notation Reference §1.2.2 states: "Music inside a `\transpose`
block is absolute unless a `\relative` is included in the block." In the
implementation, `TransposedMusic` carries the `to-relative-callback`
`ly:relative-octave-music::no-relative-callback`
(`scm/define-music-types.scm`, `lily/relative-octave-music.cc`): the
`\relative` conversion does not descend into the transposed music, and the
relative reference after the block is the last note *before* it, not the last
note inside it.

The reader was walking the transpose body with the enclosing `\relative`
state and did not restore that state afterwards. On the reduced source

```lilypond
\relative c' { c'''4 d4 \transpose c d { f4 <g b>4 } | c4~ c4 r2 | }
```

LilyPond 2.27 plays 96, 98, 55, 57/61, 96, 96; the reader produced 96, 98,
103, 105/109, 108, 108. `Context::Transpose` now applies the same
`OctaveState::sans_relative()` transformation as `Context::NonRelative`
(default absolute reference under `\relative`, current reference kept under
`\fixed`) and restores the saved octave state after the body. The reduced
test `transpose_dans_relative_reste_absolu` checks exact pitches, durations,
measure count, a second staff, a chord, a tie, a rest, bar checks, and the
unaffected `\fixed` case. Only two files of the 1020-file `ecriture_ly` lot
use `\transpose` inside `\relative`; the fix gains no exact file and loses
none. `\clef "treble_8"` and `\ottava` were compiled in isolation and do not
change MIDI pitch (Notation Reference §1.3.1, §1.3.3), so keeping the written
octave remains correct.

### Other causes: measurement artefacts, not reader defects

Each of the five groups was reduced and checked against LilyPond 2.26
directly:

* **`octave` (81 files in the frozen baseline).** Each file was measured again
  with a fresh LilyPond MIDI (built from the *current* source with a `\midi`
  block injected by `lyaudit/pre.ly`) and with scoretk's raw SMF, and the pitch
  multisets were compared. There is **no octave error in the reader or the
  writer** in this group: the flag came from the oracle and from stale or
  mismatched references. The reduced sources and LilyPond's own MIDI are given
  below for seven representative files. Grouping of all 81:

  | Cause | Files | Deciding evidence |
  | --- | ---: | --- |
  | Transposed `\midi` `\score` used as reference | 13 | Reduced `\score` pairs (Gueguen, 22_StMichel, Maiana): LilyPond's MIDI is the *second*, transposed score; scoretk reads the first engraved one (§7, §1.2.2/§1.3.4) |
  | Old oracle's note-on overwrite (false octave) | 26 | After the FIFO fix no reference pitch is missing; the flag was `p ± 12` coincidence on a dropped note (Segura) |
  | Stale `.midi` reference (much larger than a fresh LilyPond MIDI) | 14 | Fresh MIDI = scoretk output: e.g. Veni Creator 249 vs stale 3134, AStMichel 130 vs 666, gaudete 85 vs 440 |
  | Missing notes, current reference | 13 | Chord grids/transposed grids (9), two different `\score` blocks in the same file (Transeamus ×2), Scheme-generated music (Dechiffrage), one further voice case |
  | Same note count, pitch difference only | 14 | Chord grids or display-only octave marks; no note is lost |
  | No playable `\score` in the current source | 1 | `Franck-Da_Pacem_Domine.ly`: LilyPond emits no MIDI |

  Reduced evidence (each compiled with LilyPond 2.27 under
  `timeout 20 prlimit --as=2000000000`):

  1. `02_02_Segura...partition.ly` — the old oracle lost B2 (47) and E3 (52),
     then flagged them as octave errors because B3 (59) and E4 (64) existed.
     With FIFO the reference multiset matches entirely (259/259); scoretk has
     two *extra* G3/B3 from two voices sharing the part channel, which is the
     documented MuseScore grace/voice overlap (§2.6.1).
  2. `Gueguen.ly` — reduced to `\mus = \relative c' { c4 d e f | g1 }`,
     first `\score` engraved, second `\transpose d b … \midi {}`. LilyPond's
     MIDI plays 69/71/73/74/76; scoretk reads 60/62/64/65/67. The reference
     is the transposed score. Notation Reference §1.2.2: `\transpose` rewrites
     written pitches; §1.3.4: MIDI plays concert pitch.
  3. `22_StMichel.ly` — same reduction with `\transpose c c'` inside the
     `\midi` score: LilyPond 72/74/76/77/79, scoretk 60/62/64/65/67.
  4. `Maiana.ly` — same reduction with `\transpose c c''`: LilyPond
     84/86/88/89/91, scoretk 60/62/64/65/67.
  5. `Veni, Creator Spiritus_Courtial_v3_SATB.ly` — the `.midi` file from 2023
     has 3134 notes; a fresh LilyPond MIDI from the current source has 249,
     exactly scoretk's 249. Stale reference, not a reader loss.
  6. `Transeamus.ly` — reduced to its first `\score` (two staves plus voices):
     LilyPond 483, scoretk 483. The full `.midi` (620) comes from the second
     `\score`, which carries `\midi {}`. Same mechanism for
     `Transeamus_a3.ly`.
  7. `AStMichel.ly` — fresh LilyPond 130, scoretk 130, stale `.midi` 666.

  `\clef "treble_8"` and `\ottava` were compiled in isolation and do not
  change MIDI pitch (Notation Reference §1.3.1, §1.3.3), so keeping the written
  octave remains correct.

  The full LilyPond reading oracle then exposed the one exception to the
  `\transpose` rule above: `input/regression/make-relative-copies.ly` defines a
  music function with `make-relative` whose body is `#{ \transpose … #}`.
  According to `scm/music-functions.scm`, `make-relative` **first reads its
  arguments in the surrounding `\relative` context**, then builds the
  `TransposedMusic`: the body is therefore already relative, not absolute,
  even though `TransposedMusic` carries `no-relative-callback`. The reader now
  records `make-relative` on the stored music function and marks such
  `\transpose` envelopes `absolute: false`, so the interpreter keeps the
  relative reference. Reduced test
  `make_relative_dans_transpose_garde_le_contexte_relative` checks the eight
  pitches (55, 59, 62, 67 per staff) and the measure count; the regression
  file is back to N1 = N2 = 16/16. No file in the `ecriture_ly` octave group
  uses this idiom.
* **`notes_manquantes` (55 files).** 36 sources contain grace notes. The
  specification settles the semantics (Notation Reference §2.6.1): "Each grace
  note in MIDI output has a length of 1/4 of its actual duration. If the
  combined length of the grace notes is greater than the length of the
  preceding note a 'Going back in MIDI time' error will be generated."
  LilyPond therefore *shortens* the preceding note so that the grace note fits
  before the beat. MuseScore does not: `collectGraceBeforeChordEvents`
  (`src/engraving/compat/midi/compatmidirenderinternal.cpp:1080`) computes
  `graceOffsetOn = graceTickSum − graceTickOffset × i` and places the grace
  note-on before the main chord without emitting an earlier note-off for the
  previous chord. scoretk's writer follows that model, as it must for the
  MuseScore reference lots. The oracle's `read_midi` holds a single open note
  per `(channel, pitch)` in a dictionary: when the acciaccatura overlaps the
  preceding note on the same pitch the later note-on overwrites the earlier
  one, which is never paired. In `Fait/0a/lily-8546410f.ly` the missing B♭4 is
  present in scoretk's raw SMF (note-on at tick 17 280, grace note-on at
  17 460, one shared note-off at 17 519); LilyPond's own MIDI has the main note
  ending at 13 824 and the grace starting at 13 994. The same overlap occurs
  when two voices play the same pitch at the same tick on the shared part
  channel (`Fait/13/lily-0d80bdd1.ly`, 92 → 91 notes). Twelve more files are
  copies of the same `LaVerteAigrette` source and lose a tied note the same way.
  No writer change is warranted: shortening the previous note would break the
  MuseScore-rendered reference lots, and one channel per voice contradicts
  MuseScore's one-channel-per-part export.

  The oracle itself was then corrected: `midi_notes.py::read_midi` now keeps a
  **FIFO of open notes per `(channel, pitch)`** instead of one note-on slot
  (`docs/agents/format-ref/midi.md`, §Channel Voice; MuseScore's
  `midishared/midifile.cpp` pairs each note-on with the first still-free
  note-off). The reduced regression is `scripts/oracle/midi/test_overlap.py`.
  Re-measured with the FIFO oracle, the group **disappears entirely**: 0 files
  have `notes_manquantes` as their primary cause (55 → 0), and the 55 files are
  redistributed into `notes_supp`, `attaque_decalee` and `duree_seule` — their
  extra overlapping notes now count against LilyPond's shorter reference. The
  `octave` group falls from 81 to 59 on the frozen binary and 55 on the fixed
  one; N1 and N2 never decrease because of the oracle change.
* **`duree_seule` (49 files).** All have the exact reference note count, N1
  and N2; only durations differ. This is the unison-merge comparator artefact
  documented in `midi.md`. `Encantada.ly` also has four `\score` blocks, the
  first engraved under `\transpose a g` and the second with `\unfoldRepeats`;
  its reference comes from a different score than the one the reader uses.
* **`attaque_decalee` (46 files).** N1 is 100 %. LilyPond's MIDI does **not**
  unfold `\repeat volta` unless `\unfoldRepeats` is present (verified on a
  reduced score), while scoretk's MIDI follows MuseScore and unfolds it, so
  attacks after the repeat move. `À-Notre-Dame.ly` has `\repeat volta 2` and
  gains 100 notes. There is no single systematic offset.
* **`notes_supp` (35 files).** Same volta mechanism or a different `\score`
  reference. `Lallement_VenezBergersEtBergeres.ly` goes from 338 to 676 notes,
  exactly twice, the signature of a `\repeat volta 2` unfolded by scoretk
  only; `22_HymneStMichel.ly` has five `\score` blocks, four of them rehearsal
  MIDIs with `\unfoldRepeats`.

Changing the reader to follow the MIDI block rather than the engraved score,
or the writer to stop unfolding repeats, would contradict the model and
regress the MuseScore reference lots. These causes are left as documented
limits.

### Before / after (same frozen manifests and oracle)

The figures below use the historical oracle (one open note per
`(channel, pitch)`). Re-running **both** binaries with the corrected FIFO
oracle gives the same comparison: 689 → 692 exact on `ecriture_ly`, unchanged
17 / 2 / 596 / 49 on the other four lots, with the same single documented
N2 loss. The lower absolute count (753 → 689) is the oracle now counting
scoretk's extra overlapping notes against LilyPond's shorter MIDI; it is not a
regression.

| Lot | Before exact | After exact | N1 / N2 gains | N1 / N2 losses |
| --- | ---: | ---: | ---: | ---: |
| ecriture_ly | 753 (73.8 %) | **756 (74.1 %)** | 13 / 10 | 0 / 1 |
| ecriture_mscz_corpus | 17 (81.0 %) | 17 (81.0 %) | 0 | 0 |
| ecriture_mscz_musique | 2 (25.0 %) | 2 (25.0 %) | 0 | 0 |
| lecture_musique | 596 (58.4 %) | 596 (58.4 %) | 0 | 0 |
| lecture_mutopia | 49 (40.2 %) | 49 (40.2 %) | 0 | 0 |

On `ecriture_ly`, N2d also gains 9 files and loses 1. The three new exact
files are `03_01_Miguel_Les-grains-de-mais_partition.ly` (104 → 179 notes,
all matched), `02_07_LaFleurDesChampsVersLeCiel.ly` (113 → 209) and
`DuermeNoLlores.ly` (78 → 128). The single N2/N2d loss is
`LaMouetteGrise_EtBerceDansLaBrise.ly`: its engraved score has one chord grid
but the `\midi` score, which provides the reference, repeats it three times
(`\accords \accords \accords` under `\unfoldRepeats`). The new chord notes
therefore have the right pitches (N1 68 → 76) but attacks the reference does
not contain (N2 68 → 25); the file is far from exact in both runs. This is the
documented "first engraved score wins" limit (§7), not a regression.

The LilyPond reading oracle (same manifests, `--stride 10`, cached references)
goes from 111 to 108 exact on `mus`, 119 to 120 on `ra` (N1 88.58 → 92.99 %)
and stays at 46 on `mut`. Its three `mus` losses are all chord-grid files
(`DorogoiDlinnoyu_LeTempsDesFleurs`, `DuermeNoLlores`,
`LaFleurDesChampsVersLeCiel`): their cached `ly.midi` predates LilyPond's
chord-grid playback and contains only the voice track (113 notes for
`LaFleur...`), whereas a fresh LilyPond 2.26 compilation of the same source
contains both tracks (209 notes). Every reference note still matches (N1 and
N2 stay at 100 % on those three files); only the newly played chord grid is
counted as "extra" against the stale reference. No reference note is lost on
any lot, and no reading lot regresses. On the unrelated `robustesse_lakh` lot
(MIDI → MusicXML, no `.ly` input), the frozen baseline has one 10 s timeout and
the fixed run thirteen, because three campaigns and then the final workspace
checks ran concurrently. Six of the extra-timeout files were retried in
isolation with both binaries (`Dreadlock Holiday.1.mid`, `Sing A Song (bonus
track).mid`, `Broken Wings.1.mid`, `Please Don't Go.mid`, `Eternally.mid`,
`Another Day.2.mid`): all complete in 2–10 s with both the frozen and the
fixed binary. They are load artefacts, not an effect of the reader change.

The `\transpose`-inside-`\relative` fix above was measured again against the
same frozen baseline on `ecriture_ly`, `lecture_musique`, `lecture_mutopia` and
the LilyPond reading oracle (stride 10, cached references): the figures are
unchanged — 756/1020, 596/1020, 49/122 and 108/120/46 exact respectively — with
no per-file N1 or N2 loss on any lot. Only two of the 1020 `.ly` sources use
`\transpose` inside `\relative`, so the fix is a correctness improvement rather
than a gain in this sample. The other four causes were re-checked against the
notation reference and the MuseScore C++ source this time: none warrants a
reader or writer change without contradicting the pivot model (§2.6.1 for
grace notes, §4.1.2 for volta repeats, §1.3.1/§1.3.3 for clefs and ottavas).

### Final re-measurement with the corrected oracles

Both MIDI readers (`midi_notes.py` and `ly_notes.py`) now pair overlapping
notes FIFO. Re-measuring the frozen `scoretk-base` and the final binary with
the *same corrected readers* gives:

| Lot | Frozen exact | Final exact | N1 / N2 gains | N1 / N2 losses |
| --- | ---: | ---: | ---: | ---: |
| ecriture_ly | 689 | **692** | 13 / 10 | 0 / 1 |
| ecriture_mscz_corpus | 17 | 17 | 0 | 0 |
| ecriture_mscz_musique | 2 | 2 | 0 | 0 |
| lecture_musique | 596 | 596 | 0 | 0 |
| lecture_mutopia | 49 | 49 | 0 | 0 |
| LilyPond `mus` | 111 | 108 | 0 / 0 | 0 / 0 (3 exact, explained above) |
| LilyPond `ra` | 119 | **120** | 3 / 3 | 0 / 0 |
| LilyPond `mut` | 46 | 46 | 0 / 0 | 0 / 0 |

The single `ecriture_ly` N2 loss remains the documented `LaMouetteGrise`
chord-grid artefact. The `make-relative` fix above is visible on `ra`, where
`make-relative-copies.ly` had dropped four notes (N1 16 → 12) before being
restored to 16/16. No other lot changes.

`oracle/compare.sh` still reports 280 DIFF rows because its frozen reference
dates from 2026-11-18, before later merged commits (chord-grid playback, ABC
and MIDI export changes). This was checked directly: the same script on a
clean worktree at the pre-change commit `982a7fa` reports exactly the same 280
rows, and the two freshly generated output trees are byte-identical
(`diff -rq` empty). None of the DIFFs comes from these changes; none of the
compared conversions has a `.ly` input, and the changed reader only affects
`.ly` reading.

## 15. Playback transposition of a MIDI-only `\score` (2026-11-20)

### Decision and detection

The 13 files grouped as "transposed `\midi` `\score`" in §14 (Gueguen,
`22_StMichel.ly`, `Maiana.ly`, …) are the cases where LilyPond's MIDI comes
from a **second** `\score`. Per §7 the engraved (first) score still provides
the model: MusicXML, LilyPond, ABC and MuseScore output are unchanged.

The reader now also records, in `Score::playback_transposition`
(`scoretk-core/src/model/playback.rs`, chromatic semitones) — a field ignored
by every writer except MIDI — the interval of a separate `\score` that has
`\midi` **without** `\layout` (Notation Reference §24.3: such a score produces
only MIDI). It is recognised structurally in
`scoretk-lilypond-import/src/playback.rs`:

- the group/staff/voice shape of the two scores must be identical;
- after removing the leading `\transpose` envelopes (and only those; `\fixed`
  and `\relative` are kept in place) from **each** voice, the music trees must
  be *equal* — same variables, notes, durations, rests, chords and ties;
- the retained interval is the MIDI score's total `\transpose` minus the
  engraved score's total `\transpose`, and must be the same for every voice
  (so per-staff transpositions that disagree are rejected);
- when both scores apply `\transpose c c` (identity) and `\transpose c d`, the
  result is `+2`, not `+2` guessed from a pitch comparison.

If the MIDI score differs in **any other way**, nothing is recorded. In the
frozen `ecriture_ly` lot this excludes seven of the 13 files, for a concrete
reason each: `22_StMichel.ly` adds per-staff `midiInstrument` and drops
`\autoBeamOff`; `EzpataDantza.ly` changes `midiInstrument`, adds `\tempo` and
`\unfoldRepeats` and drops the `.|` bar; `Maiana.ly` drops `\bar ".|"`/`\break`
and adds `\unfoldRepeats`; `MakilarianII.ly` uses a different variable
(`\partition` instead of `\partitiontransposee`); `MakilTxikiak.ly` and
`UztaiDantza.ly` duplicate the `\appelII` voice; `Angels_Carol.ly` adds an
extra piano staff in the MIDI score. Accepting any of them would mean
transposing a score that is not the engraved one — exactly the guess this
detection is meant to avoid.

### Measurement (frozen `lym-fifo-final` baseline, same manifests and oracles)

The frozen baseline is the corrected-FIFO campaign of §14 (`scoretk-final`
figures). All conversions ran under `timeout 10 prlimit --as=2000000000`.

| Lot | Baseline exact | Current exact | N1 gains / losses | N2 gains / losses |
| --- | ---: | ---: | ---: | ---: |
| MIDI `ecriture_ly` | 692 (67.8 %) | 692 (67.8 %) | 6 / 0 | 6 / 0 |
| MIDI `ecriture_mscz_corpus` | 17 | 17 | 0 / 0 | 0 / 0 |
| MIDI `ecriture_mscz_musique` | 2 | 2 | 0 / 0 | 0 / 0 |
| MIDI `lecture_musique` | 596 | 596 | 0 / 0 | 0 / 0 |
| MIDI `lecture_mutopia` | 49 | 49 | 0 / 0 | 0 / 0 |
| LilyPond oracle `mus` | 108 | 108 | 0 / 0 | 0 / 0 |
| LilyPond oracle `ra` | 120 | 120 | 0 / 0 | 0 / 0 |
| LilyPond oracle `mut` | 46 | 46 | 0 / 0 | 0 / 0 |

The aggregate `ecriture_ly` percentages rise from N1 91.00 / N2 83.86 to
N1 92.27 / N2 85.77. Six files gain notes **and** attacks with no loss:

| File | N1 before → after | Flag before → after |
| --- | --- | --- |
| `Gueguen.ly` | 23 → 258 | `octave, transposition` → none |
| `Sonis (copie 1).ly` | 47 → 258 | `octave, transposition` → none |
| `Sonis.ly` | 46 → 266 | `octave, transposition` → `octave` (2 notes) |
| `GaudeteChristusEstNatus_abrege_a4.ly` | 189 → 402 | `octave, transposition` → `duree_seule` |
| `GaudeteChristusEstNatus_abrege.ly` | 189 → 402 | `octave, transposition` → `duree_seule` |
| `GaudeteChristusEstNatus.ly` | 441 → 1074 | `octave, transposition` → `duree_seule` |

None becomes *exact*: the Gueguen/Sonis references still contain extra notes
from `\unfoldRepeats` (the engraved score is not unfolded, §7), and the three
Gaudete files now match every pitch and attack but differ only in note
durations. Both are already documented limits, not regressions. The `octave`
flag falls from 60 to 55 files; no file gains or loses N1 or N2 downwards. The
reading lots and the LilyPond reading oracle are byte-level unchanged, as
expected since the MusicXML writer does not use the new field.

## 16. `\partial` with a duration multiplier (2026-11-20)

### Cause and fix

Notation Reference §2.3.3 (Upbeats) defines `\partial <duration>` as the length
of the music before the first bar, and §2.1.3 allows multipliers on durations:
`\partial 8*5` is five eighths, not one eighth. In the reader,
`Parser::parse_duration()` returns both the written figure and its `*n/d`
factor; the `\partial` branch kept only the figure. The measure frame then gave
the anacrusis `len = 1/8` while its chords summed to `5/8`, and MuseScore 4.7.4
refused the written `.mscz` with `Incomplete measure … Found: 5/8. Expected:
1/8` (`docs/agents/mscx-write.md` §8, second pass). §2.6.3 (Time administration)
also allows `\partial` in the middle of a score, where its argument is the
*remaining* length of the current bar — this is why many affected sources put
a second shortened bar at the very end.

`ast::Context::Partial` now carries the tick `Fraction` (figure × multiplier)
instead of a `Duration`, using the same conversion as explicit lyric
durations. A zero factor is ignored, so the measure falls back to the time
signature, as for `\partial 0`. The writer is unchanged: it still emits
`\partial <duration>` from `Measure.len`.

### Tests

- `read_lilypond.rs`: a five-eighth upbeat with control bars, a chord, a rest
  and two following full bars; a mid-score `\partial 8*5` after a plain
  `\partial 8`. Both assert measure lengths, nominal lengths and every written
  duration.
- `partial_multiplier.rs`: two staves, variables, `\relative`, a later
  `\score`, `\times`/`\tuplet`, chords and rests; exact pitches and measure
  lengths, plus `\partial 4*0` (limit) and `\partial 4` (unaffected case).
- `common/consistency.rs` + `coherence_corpus.rs`: a shared invariant — each
  voice's effective chord-rest durations, tuplets included, sum to
  `Measure::len` — checked over the 56 `.ly` corpus files. `v3_moonlight.ly` is
  excluded: it has voices that stop before the bar line, an already documented
  layout difference (§7 and `mscx-write.md` §7), not an anacrusis defect.

### Measurement

Frozen baseline: `/tmp/scoretk-mscz-pass2-base` (merged master, before this
fix); fixed binary: `/tmp/scoretk-partial-fix`. All binaries ran under
`timeout` + `prlimit --as=2000000000`.

| Oracle | Baseline | Current | Change |
| --- | ---: | ---: | --- |
| MuseScore writing, `ly` partial lot (50 files) | 10 opened | 49 opened | +39, 0 loss |
| LilyPond reading `mus` (73 files) | 55 exact | 58 exact | +3, 0 loss |
| LilyPond reading `ra` (87) / `mut` (19) | 20 / 6 exact | 20 / 6 exact | unchanged |
| MIDI `ecriture_ly` (50 partial + 94 other files) | — | — | byte-identical MIDI |

The three reading-oracle gains are `Benedicite.ly` (N2 20 → 36) and the two
`LChefDePatrouilleADeuxGalons.ly` copies (N2 16 → 76); all become *exact* and
none loses notes. The one file of the partial lot that MuseScore still refuses,
`lily-cf5ff785.ly`, fails on a different cause (`Voice too long`, measure 33,
staff 1, voice 3) and was already unopened in the baseline: our change fixes
its anacrusis (`len` 1/8 → 3/8) but not that independent voice-length defect.
The MIDI comparison is byte-for-byte on the generated files (6 of the 100
sampled sources produce no MIDI in either binary and are excluded), so the
`ecriture_ly` figures of §15 still hold.

## 17. Tags, Scheme variables and `\include` (2026-11-20)

### Coverage and sources

The Notation Reference sections used are §22.1 (Including LilyPond files),
§22.2 (Different editions from one source, in particular §22.2.2 Using tags)
and Extending §1 (Scheme tutorial). Where the published text is silent about
the exact filtering rule, the implementation follows LilyPond 2.26.0's
`scm/music-functions.scm` (`tags-keep-predicate`, `tags-remove-predicate`,
`define-tag-group`) and `ly/music-functions-init.ly` (`tag`, `keepWithTag`,
`removeWithTag`, `tagGroup`). This is a port of those predicates, **not** a
Scheme interpreter: no expression is evaluated.

- `\tag #'a { … }`, `\tag a,b { … }`, `\tag "a" { … }` mark a music expression.
  Several `\tag`s on one expression accumulate their names, as the `tags`
  property does upstream.
- `\keepWithTag` / `\removeWithTag` filter the music tree recursively, as
  `music-filter` does. Untagged music is always kept. `\removeWithTag` drops
  music bearing any of the named tags. `\keepWithTag` keeps music bearing any
  of the named tags, or bearing only tags from groups unrelated to the named
  ones. The two internal groups `$autoChange` and `$partCombine` are present
  by default; `\tagGroup` adds a group, `\resetTagGroups` restores the defaults.
  A tag without a group compares equal to "no group" (Guile `#f`), which is why
  `\keepWithTag other` keeps tags from an unrelated *group* while removing
  other ungrouped tags.
- The common Scheme subset is consumed without evaluation:
  `#(set-global-staff-size n)` and other settings remain silently ignored;
  `#(define nom #{ … #})` records a music variable inserted by `$nom`;
  `#(define …)` without an embedded music body (numbers, functions, flags)
  is still ignored. `\set Timing.measureLength = #(ly:make-moment n d)` now
  reads the two-integer form as `n/d` in addition to the one-argument
  `#(ly:make-moment n/d)` already supported.
- `\include` was already expanded from tokens by `include.rs` (commit
  `c393d6c`). It was checked on all reading-oracle lots: absent paths still
  warn and are skipped without looping, and no new I/O was added. The one
  remaining gap is unchanged and documented in §11: LilyPond's installed
  templates (`satb.ly`, `ssaattbb.ly`) and files found through the
  command-line `--include` search path are not available to the library's
  resolver. The CLI only searches the including file's and the main file's
  directories; adding an installed-template search path would change the CLI
  and is out of scope here.

Four reduced tests were added to `read_lilypond.rs`: tag filtering in a
five-staff score (variables, `\relative`, chords, ties, bar checks, symbol
lists, and an unfiltered control staff), tag-group semantics, a Scheme music
variable inserted with `$motif`, and the two-integer `ly:make-moment`. All
four failed on the frozen baseline before the fix. The CLI also completed
three randomly chosen `input/regression/*.ly` files under
`timeout 5 prlimit --as=2000000000`.

### Measurement

Baseline `/tmp/scoretk-lytag-base` (merged master, commit `842da94`) and final
`/tmp/scoretk-lytag-final`, both under `timeout` + `prlimit --as=2000000000`;
the LilyPond reading oracle uses the cached MIDI references with `--stride 10`
on all three lots:

| Oracle | Lot | Baseline exact | Final exact | N1 / N2 change |
| --- | --- | ---: | ---: | --- |
| LilyPond reading (`mus`, 145) | music | 113 | **114** | N2 88.52 → 88.83 |
| LilyPond reading (`ra`, 173) | regression | 39 | 39 | unchanged |
| LilyPond reading (`mut`, 138) | Mutopia | 42 | 42 | N1 89.06 → 93.33 |
| LilyPond reading, all tag/Scheme regression files (37) | targeted | 2 | **3** | see below |
| MuseScore writing, `ly` (375) | round trip | 237 | **238** | N2 79.78 → 80.38 |

The 37-file targeted lot consists of every `input/regression/*.ly` containing
`\tag`, `\keepWithTag`, `\removeWithTag`, `\tagGroup` or
`set-global-staff-size`; the stride-10 sample of `ra` contained none of them.

Per-file changes on the reading campaign, all explained:

- `LaMarcheDesPetitsLapins.ly` (N2 226 → 271, `attaque_decalee` cleared)
  and `Et_resurrexit.ly` (N2 110 → 137) use the two-integer `ly:make-moment`
  for `measureLength`; the old code read the last integer instead of the
  fraction.
- The three `polymetric-time-warning-{mid-measure,set-in-timing-staff,unset-in-timing-staff}.ly`
  files use string tags (`\tag "layout"` / `\keepWithTag "layout"`); their
  shorter output is the filtered engraved score and one gains N2 (4 → 6).
- `Chop-28-23.ly` loses 4 matched pitches (N1 431 → 427) but gains N2
  (288 → 290). Its engraved `\score` keeps the `layout` tags while the MIDI
  reference comes from a second `\score` keeping the `midi` tags; reading the
  engraved score is the documented rule of §7, and the baseline had matched
  both alternatives at once only because tags were ignored.
- `satb-template-with-keyboard-reduction.ly` loses its 8 matched notes. It
  ends in `\include "satb.ly"`, an installed template the resolver cannot open
  (§11). Previously the tagged piano music leaked out of its variable as a
  spurious implicit score; it now stays inside `PianoRHMusic`/`PianoLHMusic`,
  so an unresolved include leaves no playable score. This is the include gap,
  not a tag-filtering loss.
- `Symphony7_1.ly` hit one 10 s timeout in the final run (8 MB of MusicXML).
  Three isolated retries finished in 5.6, 6.1 and 6.3 s with the final binary,
  against 6.3, 6.5 and 10.0 s with the baseline: a load artefact, not a loop.

Per-file changes on the writing `ly` lot: `lily-cf5ff785.ly` becomes exact
(N2 226 → 271) and `Ecce_sacrum_paratum_convivium.ly` gains N2 (78 → 257);
`lily-9f2fb156.ly` only changes size. No file loses notes or attacks on either
lot.

## 18. Reading campaign toward 95 % N2 (2026-11-20)

### Frozen baseline and method

Master `2b00392` was frozen as `/tmp/ly95-base`; the three full lists
(`mus` 1447, `ra` 1726, `mut` 1377 paths, no stride) were run with the
versioned oracle, cached LilyPond references and every conversion under
`timeout 10 prlimit --as=2000000000`. Weighted N2 is the multiset
intersection of `(pitch, attack on the 1/16-quarter grid)` (oracle §N2).

| Lot | Files | Repeats | OK | Exact | N1 % (all) | N2 % (no repeat) | N2 % (all files) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| mus | 1447 | 113 | 1444 | 1125 (77.7 %) | 99.20 | 91.64 | 89.22 |
| ra | 1726 | 305 | 1724 | 1200 (69.5 %) | 90.84 | 75.86 | 78.09 |
| mut | 1377 | 651 | 1374 | 390 (28.3 %) | 98.10 | 69.94 | 68.86 |
| **all** | **4550** | **1069** | **4542** | **2715 (59.7 %)** | **98.05** | **74.15** | **71.78** |

### Repeats: decision and effect on the denominator

`lyaudit/pre.ly` injects only a `\midi {}` block into scores that lack one;
it does **not** wrap the music in `\unfoldRepeats`. A `\repeat volta` is
therefore played **once** in the reference MIDI, which is the same written-once
structure that the MusicXML writer produces and the oracle reads (a repeat is
a direction, not duplicated notes). Among the 1069 repeat-flagged sources, 309
contain an explicit `\unfoldRepeats` (207 of them also contain `\repeat`), so
their cached reference may come from a separate MIDI score that *does* unfold;
they stay flagged and are listed separately in the raw rows.

We do **not** regenerate the references with `\unfoldRepeats`: LilyPond would
then play each volta twice while the MusicXML writer keeps the structure once
(only the MIDI writer unfolds, through `RepeatList`), and scoring the two
against each other would need the arbitrary earliest-attack pairing that the
oracle deliberately rejects. Both denominators are reported above: the
repeat-free N2 stays the non-regression figure, and the "all files" N2 now
includes every repeat row instead of silently dropping it. The change costs
2.37 N2 points globally (74.15 → 71.78) but makes the 1069 repeat files
measurable rather than excluded.

### Ranked N2 losses (before the fixes below)

For each file we reconstructed the reference MIDI and scoretk's MusicXML from
the frozen run, found the **first attack where the `(pitch, tick)` multisets
differ**, and read the LilyPond construct in a window of the source around
that bar (direct `\include`s included). Loss is `ref − matched pairs`, so it
counts notes, not files. Categories overlap only in the sense that each file
has exactly one first-divergence construct.

| Construct at the first diverging attack | Lost notes | Files | Example |
| --- | ---: | ---: | --- |
| `\grace` / `\acciaccatura` / `\afterGrace` | 103 579 | 177 | `Ackermans_NoelDesPetitsSantons.ly` |
| `\voiceOne` / `<< … >>` | 98 022 | 363 | `AuClairDeLaLune.ly` |
| `\tuplet` / `\times` | 36 603 | 91 | `Promo_Foucauld.ly` |
| `R1*n` / `\bar` | 28 105 | 70 | `01_01_Buenos-dias-Mexico_livret.ly` |
| none of the tracked constructs | 27 100 | 126 | `Ave_dulcissima_Maria.ly` |
| `s` spacer | 18 804 | 40 | `Angels_Carol__John_Rutter-midi.ly` |
| variables | 16 981 | 143 | `benoit_ave_maria.ly` |
| `\set Timing.*` | 6 921 | 29 | `Ecce_sacrum_paratum_convivium.ly` |
| `\repeat volta` / `\repeat unfold` | 6 307 | 36 | `Encantada.ly` |
| lyrics (`\lyricsto`, `\addlyrics`) | 5 857 | 10 | `Vorrei_baciarti.ly` |
| `\time` / `\partial` | 1 700 | 16 | `02_02_Segura….ly` |
| `\repeat tremolo` | 1 231 | 10 | `promenade-6.ly` |
| `\cadenzaOn` | 356 | 6 | `accidental-ledger.ly` |
| `\repeat percent` | 50 | 11 | `ottava-double-percent-repeat-note.ly` |
| `\partCombine` | 21 | 3 | `duration-line-and-partCombine.ly` |
| `\scaleDurations` | 3 | 1 | `chord-tremolo-scaled-durations.ly` |

By the oracle's primary flag: `attaque_decalee` 175 358 notes (641 files),
`octave` 174 407 (347), `sortie_vide` 996 (52), `notes_manquantes` 843 (82),
the rest below 20. The two large flags overlap with the table above: a
wrong attack does not change the pitch multiset (N1 100 %), and the `octave`
flag is often raised by a stray unmatched pitch whose octave exists elsewhere.

### Fix 1 — a post-event must not swallow the following command

`lily/parser.yy` (`post_event_nofinger: direction_less_event`,
`post_event: post_event_nofinger | '-' fingering`) makes `\fermata`, `\p`,
`\trill`… **isolated** events: only the `script_dir music_function_call`
rule consumes a following command, and that rule applies after `^`, `_`, `-`.
The reader's `parse_post_event` sent every bare `\command` through
`parse_direction_event`, which consumed the **next** token. On
`e4\fermata \times 2/3 { f8 g8 a8 }` the `\times` token disappeared, the three
eighths kept their written duration and the next bar started late — the
mechanism behind most of the `\tuplet`/`\times` and part of the `\grace`
rows above. `parse_post_event` now recognises a known post-event directly and
consumes only its own token. The reduced test `post_event_ne_consomme_pas_le_times_suivant`
covers two staves, variables, `\fixed`, bar checks, a chord, a tie, a rest,
a dynamic and a fermata before `\times`/`\tuplet`, a post-event at end of
music, and the unaffected bare `\times` case; it failed on the frozen code
(5 measures / 1 n-tuplet instead of 4 / 3). Three random
`input/regression/*.ly` files complete under `timeout 5 prlimit`.

### Fix 2 — a no-argument command must not swallow a following rest name

`lily/parser.yy` distinguishes a bare `command_element` (no arguments) from a
`music_function_call` (arguments declared by the function's signature);
`lily/lexer.ll` lexes `r`, `R`, `s` as rest names and `q` as the repeated-chord
token. None can be an argument of `\autoBeamOff`, `\oneVoice`, `\dynamicUp`…
The heuristic `skip_command_arguments` only excluded pitch words, so
`\autoBeamOff r8 f8` lost the `r`: the bare `8` was then read as a note reusing
the previous pitch, shifting the whole voice. This is one mechanism behind
the `s` spacer and "no tracked construct" rows of the table, and it affects
several hundred Mutopia sources (e.g. `lily-f93c49a3.ly`, `LeRoiArthur.ly`,
`52title-hub.ly`). Rest names are now left in the token stream like pitch
names. The reduced test `commande_sans_argument_ne_consomme_pas_un_silence`
checks two staves, variables, `\fixed`, `\partial`, bar checks, a chord and a
tie, a normal rest, a full-measure rest and a spacer after three different
no-argument commands, plus a command at end of music and the unaffected
note case; it failed on the frozen code (extra repeated-pitch notes).

Measurement of fix 1 (frozen `/tmp/ly95-fix1`, same full lists and cached
references, every conversion under `timeout 10 prlimit --as=2000000000`):

| Lot | N2 % (no repeat), before → after | N2 % (all), before → after | Exact |
| --- | --- | --- | ---: |
| mus | 91.64 → **92.15** | 89.22 → **90.27** | 1125 → 1133 |
| ra | 75.86 → **76.03** | 78.09 → **78.21** | 1200 → 1206 |
| mut | 69.94 → **75.75** | 68.86 → **74.78** | 390 → 394 |
| all | 74.15 → **78.85** | 71.78 → **76.95** | 2715 → 2733 |

Four `mut` files timed out in the second run (against two in the first), so
the reference-note denominators differ by about 1.7 %; no file was silently
dropped from the report. The gain is concentrated in Mutopia, as expected
from the `\times`-heavy sources. No per-file loss was attributed to the fix:
`attaque_decalee` falls from 641 to 617 files and `notes_supp` rises from
697 to 709 only because previously shifted attacks now land on notes the
stale references do not contain.

### Final measurement (both fixes, full lists)

Frozen `/tmp/ly95-fix2`, same manifests, cached references and `timeout 10
prlimit --as=2000000000` on every conversion:

| Lot | Exact | N1 % (all) | N2 % (no repeat) | N2 % (all files) |
| --- | ---: | ---: | ---: | ---: |
| mus | 1136 (78.5 %) | 99.22 | 91.64 → **93.51** | 89.22 → **91.46** |
| ra | 1249 (72.4 %) | 90.93 | 75.86 → **81.25** | 78.09 → **82.11** |
| mut | 435 (31.6 %) | 98.16 | 69.94 → **80.12** | 68.86 → **78.53** |
| **all** | **2820 (62.0 %)** | **98.11** | **74.15 → 82.68** | **71.78 → 80.36** |

Fix 2 alone: 197 files gain N2, 12 lose N2, 87 become exact and none ceases
to be exact. Fix 1 alone: 89 gain N2, 12 lose, 18 become exact, none lost
exactness. From the frozen baseline: 273 files gain N2, 19 lose it, and
105 become exact with no exact loss.

The per-file decreases have no note loss: N1 is unchanged on all of them but
four, and the two largest (`Voglio_che_sia_l_indegno.ly` 2210 → 1884 and
`Solo_e_pensoso.ly` 924 → 663) keep **every** pitch and every note (N1 stays
2538/2538 and 1367/1367); only attacks move, by the duration of a rest that
fix 2 restored in the source (397 → 398 written rests in the first). The
other decreases are ≤ 50 notes each and eight of the twelve after fix 1 are
repeat-flagged, i.e. outside the blocking denominator. The leading-`<backup>`
offset of `25EF-04.ly` (largest fix-1 loss) makes every one of its 395 notes
match after a constant four-quarter shift, so it is an oracle offset, not a
musical regression.

### Why the two largest ranked rows were not changed

- **`\grace`/`\acciaccatura` (first row).** Reduced `\grace`, `\acciaccatura`,
  `\appoggiatura` sources were compiled with LilyPond 2.26 and compared with
  scoretk on the Q16 grid: single graces land on the same cell. The residual
  losses are the documented MuseScore-vs-LilyPond onset model
  (`collectGraceBeforeChordEvents`, §14) and several graces spread over a
  beat by LilyPond while MuseScore places them all at the following attack.
  Changing the reader would break the MuseScore reference lots, so this is a
  measurement-model limit, not a reproducible reader defect.
- **`\voiceOne` / `<< … >>` (second row).** A reduced
  `<< { c4 d4 } \\ { e4 f4 } >> g4 a4 | b2` matches LilyPond's MIDI exactly
  (both branches at 0/1, following notes at 2/3/4). The row is dominated by
  long Mutopia scores whose attacks drift from stale or unfolded references,
  not by a grammar bug.

The two reproducible parser defects were therefore taken first; their gain
(+8.53 N2 points over repeat-free files) is larger than either single ranked
row would have suggested, because they acted across all of them.

## 19. Reading campaign, second pass (2026-11-20)

### Re-ranking on the current binary

Same 4550 files, same cached references, binary `2b00392` + the three fixes;
"lost" is `ref − matched N2 pairs`, assigned from the first diverging attack.
The table of §18 was measured **before** those fixes and is superseded here.

| Construct at first divergence | Lost notes | Files | Example |
| --- | ---: | ---: | --- |
| `\grace` / `\acciaccatura` | 65 602 | 152 | `Ackermans_NoelDesPetitsSantons.ly` |
| `\voiceOne` / `<< … >>` | 63 402 | 306 | `Symphony5_1.ly` |
| `\tuplet` / `\times` | 28 217 | 88 | `lily-18e51d2e.ly` |
| none of the tracked constructs | 24 605 | 116 | `Ave_dulcissima_Maria.ly` |
| `R1*n` / `\bar` | 17 611 | 56 | `01_01_Buenos-dias-Mexico_livret.ly` |
| `s` spacer | 14 914 | 39 | `Angels_Carol__John_Rutter-midi.ly` |
| variables | 10 860 | 126 | `Dechiffrage.ly` |
| `\set Timing.*` | 6 722 | 29 | `Ecce_sacrum_paratum_convivium.ly` |
| `\repeat volta` / `unfold` | 5 523 | 33 | `Encantada.ly` |
| `\time` / `\partial` | 1 575 | 17 | `02_02_Segura….ly` |
| `\repeat tremolo` | 1 231 | 10 | `promenade-6.ly` |
| `\cadenzaOn` | 356 | 6 | `accidental-ledger.ly` |
| lyrics | 310 | 5 | `T_Amo_Mia_Vita.ly` |
| `\repeat percent` | 50 | 11 | `ottava-double-percent-repeat-note.ly` |
| `\partCombine` | 20 | 3 | `duration-line-and-partCombine.ly` |
| `\scaleDurations` | 3 | 1 | `chord-tremolo-scaled-durations.ly` |

By flag: `octave` 120 757 notes / 341 files, `attaque_decalee` 118 404 /
517, `sortie_vide` 996 / 52, `notes_manquantes` 808 / 78.

### Grace notes: oracle comparison rule, not a reader change

Notation Reference §2.6.1 states that graces "take up no additional logical
time in a measure" and that each grace note in MIDI has 1/4 of its written
duration; `lily/grace-iterator.cc` processes them in the **grace part** of a
musical moment and `lily/midi-walker.cc` notes that a score starting with
graces starts at negative MIDI times. The oracle's `read_xml()` places a
`<grace/>` at the current cursor, i.e. the following main note's attack, with
zero duration. Quantizing LilyPond's grace onsets raw therefore compares two
different times.

The oracle now records **two** rules (files `scripts/oracle/lilypond/`
`ly_notes.py`, `mesure.py`, `README.md`; `ly_notes.py selftest` and
`mesure.py --selftest` cover the marker, the pairing, a missing grace and the
unaffected case):

- **N2** unchanged: every note is compared on `(pitch, quantized attack)`;
- **N2-grace**: each candidate grace is paired with the latest unmatched MIDI
  note of the **same pitch** in `[main attack − 1 quarter, main attack + grid/2]`;
  paired graces are compared on the main note's logical attack and counted
  separately (`graces_ref`, `graces_test`, `graces_match`). Pairing is
  deterministic, never reassigns a main note, and a stale grace stays unmatched.

| Lot | N2 % (no repeat) | N2-grace % (no repeat) |
| --- | ---: | ---: |
| mus | 93.51 | 93.51 |
| ra | 81.25 | **81.80** |
| mut | 80.12 | **80.14** |
| **all** | **82.68** | **82.71** |

All 4584 reference graces are matched (5440 graces emitted by the candidate,
856 with no reference counterpart); the aggregate gain is small because the
65 602 notes of the grace row are mostly **drift later in long scores**, not
grace onsets: a reduced `c'4 \grace { d16 } e'4 f'2` source matches on the Q16
grid under both rules.

### The three largest `\voiceOne`/`<< … >>` files

1. **`Symphony5_1.ly`** (7861 notes): first divergence at −3.5 quarters, a
   candidate note at a **negative attack** produced by a leading `<backup>`
   in measure 1, as in `25EF-04.ly`; a constant −8-quarter shift aligns 448
   of its 8295 notes. This is the documented oracle offset, not a construct
   error.
2. **`bwv910.ly`** (3894 notes, N1 4147/4147): first divergence at 5.0
   quarters, where the reference has seven 32nds and the candidate a single
   quarter note. The source is
   `a[ cis fis eis fis \change Staff = "LH" fis, a gis] \skip 4`: numeric
   `\skip 4` set the remembered duration to a quarter for the following bare
   notes. Notation Reference §2.2.2 says explicitly that *"a numeric duration
   argument does not affect the duration of the subsequent note"* (unlike
   `s4`), verified on a reduced source against LilyPond 2.26's MIDI. Fixed
   (commit `1292ed9`); the reduced test
   `skip_numerique_garde_la_duree_precedente` fails on the old code.
3. **`BWV865_Fuga20.ly`** (2158 notes, N1 2376/2376): first divergence at 0,
   five candidate notes at the very first attack where the reference starts
   at 0.5 quarters. Its XML declares `<staves>11</staves>` for a two-staff
   `PianoStaff`: nested `<< … \\ … >>` blocks containing `\new Voice` open
   extra staves, and those branches put their notes at the current tick.
   Notation Reference §5.2.1/§5.2.2 and `lily/parser.yy` `context_change`
   keep voices *inside* a staff; the reader's §7 heuristic ("a simultaneous
   block without `\\` is read as sibling voices") is too coarse here. A fix
   requires reworking context allocation, which risks the eight already-exact
   Bach files; this one is documented rather than patched blindly.

### Final state of the second pass

Frozen `/tmp/ly95-fix4` on the same full lists and cached references:

| Lot | Exact | N1 % (all) | N2 % (no repeat) | N2-grace % (no repeat) |
| --- | ---: | ---: | ---: | ---: |
| mus | 1136 (78.5 %) | 99.22 | 93.51 | 93.51 |
| ra | 1249 (72.4 %) | 90.93 | 81.25 | 81.70 |
| mut | 435 (31.6 %) | 98.24 | 80.12 → **80.74** | 80.76 |
| **all** | **2820 (62.0 %)** | **98.18** | **82.68 → 83.20** | **83.23** |

After the previous pass (82.68 % N2 without repeats), the numeric-`\skip` fix
gains **0.52 N2 points**, all of it from `bwv910.ly`: its 4147 notes go from
253 to **4146 matched attacks** (the remaining note is an unrelated tie
detail). Across all 4550 files there is **one N2 gain and zero N2 or N1 loss**;
the number of exact files is unchanged. The N2-grace figures above were
recomputed from the stored `ref.mid`/`tk.musicxml` of the same run after a
correction to the pairing code: candidate graces must also be removed from
the main-note multiset, otherwise a grace at the main attack can match a
reference main note of the same pitch and be counted twice. `ly_notes.py
selftest` and `mesure.py --selftest` cover the corrected rule.
## 20. Writing campaign toward 95 % N2 (2026-11-20)

### Protocol and frozen baseline

The master CLI was frozen **before any edit** (`/tmp/ly95-base-bin`,
`ee50fb906f5f0d60…`, built from `2b00392` with
`nix develop -c cargo build -p scoretk-cli`). All four **whole** lots were
measured with `scripts/oracle/lilypond/ecriture.sh`: the 56 versioned corpus
`.mscx`, the 174 `.mscz` and 32 genuine MusicXML of `~/Musique`, and the
deterministic 1/5 sample of 375 `.ly`. Cached reference MIDI/MusicXML were
reused; every conversion ran under `timeout 10 prlimit --as=2000000000` and
every LilyPond 2.26 compilation under 20 s / 2 GB. The `.mscz` lot was split in
two after the first process was stopped at 85 files; the two `.jsonl` halves
are added (174 distinct files), and the `.ly` lot was measured in a dedicated
parallel run. This is §13's method, applied to the current branch.

| Lot | Tool | Files | Compiled | Exact | N1 % | N2 % | N2d % | Errors | Warn. | Mean .ly bytes | Max time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 28 (50.0 %) | 79.91 | 78.36 | 75.90 | 0 | 11 | 1396 | 3.17 s |
| corpus | musicxml2ly | 56 | 51 | 28 (50.0 %) | 94.52 | 91.98 | 89.29 | 79 | 20 | 2926 | 20.01 s |
| mscz | scoretk | 174 | 158 | 30 (17.2 %) | 91.96 | 85.35 | 83.31 | 68 | 1790 | 28 043 | 23.37 s |
| mscz | musicxml2ly | 174 | 140 | 34 (19.5 %) | 93.18 | 91.49 | 90.94 | 80 988 | 3494 | 40 169 | 33.86 s |
| xml | scoretk | 32 | 24 | 12 (37.5 %) | 97.81 | 95.34 | 94.67 | 66 | 251 | 103 899 | 23.24 s |
| xml | musicxml2ly | 32 | 2 | 1 (3.1 %) | 98.79 | 98.79 | 98.79 | 326 | 24 | 19 381 | 4.66 s |
| ly | scoretk | 375 | 289 | 238 (63.5 %) | 90.71 | 80.38 | 80.00 | 0 | 129 | 1957 | 8.94 s |
| ly | musicxml2ly | 375 | 118 | 93 (24.8 %) | 96.09 | 87.18 | 86.64 | 0 | 251 | 3745 | 12.96 s |

Per-file, where at least one tool is exact: corpus 1 win / 27 ties / 1 loss;
`.mscz` 5 / 25 / 9; XML 11 / 1 / 0; `.ly` 147 / 91 / 2. scoretk still beats
musicxml2ly on three lots by exact files and compiles far more sources; the
XML figure of musicxml2ly is measured on one file (alternative's
`UnboundLocalError`, §13).

### Why the percentages and an earlier "lost notes" ranking disagreed

The oracle's N2 % is a **reference-weighted mean**:
`Σ(ref_i × pct_n2_i) / Σ(ref_i)`, over the files where a reference MIDI exists
*and* the candidate produced a MIDI. It is not a mean of per-file percentages,
and the numerator/denominator are not the same for the two tools.

| Tool | Lot | Files with ref | Σ ref notes | Σ N2 matches | N2 % |
| --- | --- | ---: | ---: | ---: | ---: |
| scoretk | corpus | 56 | 2320 | 1818 | 78.36 |
| musicxml2ly | corpus | 46 | 1970 | 1812 | 91.98 |
| scoretk | `.mscz` | 166 | 344 845 | 294 309 | 85.35 |
| musicxml2ly | `.mscz` | 144 | 270 513 | 247 497 | 91.49 |

musicxml2ly fails internally or at compilation on 10 corpus files (including
`timesig-09`, 320 reference notes) and 30 `.mscz` files; **their reference
notes leave its denominator entirely**. Restricted to the 46 corpus files both
tools measure, scoretk is 1785/1970 = **90.61 %** against 91.98 %: a 1.37-point
gap (27 notes), not 13.62 points. The other 12.25 points are scoretk's
9.43 % on the 10 files musicxml2ly cannot process (33/350, mostly the tremolo
artefact of §13). On the 139 common `.mscz` files the gap is real and large:
230 138/262 167 = 87.78 % against 91.83 %, i.e. **10 621 notes**.

An earlier ranking compared **N2d** (duration-sensitive), skipped every file
where musicxml2ly had no percentage, and used a 0.3-note threshold: it found
only 19 corpus notes and mistook the denominator effect for a musical result.
The corrected ranking below uses **N2 matches**, `ref × (pct_n2_m2 −
pct_n2_tk)/100`, on the files both tools measure.

### Loss ranking against musicxml2ly (first diverging attack)

Corpus: 7 files, +30 N2 matches to musicxml2ly — `testExcludeInvisibleElements`
(+8), `copypaste_parts` (+5), `partExclusion` (+4), `layout_elements` (+4),
`voices` (+3), `timesig-05` (+3), `testChordSymbols` (+3). These are small,
layout/cross-staff and optional-courtesy cases (§7), worth 0.13 N2 point
together.

`.mscz`: 34 files, **+12 274 matches**, dominated by orchestral parts. The
first diverging attack of `07- Adeste fideles - Trompettes` is bar 5 (tick 16),
where the reference and musicxml2ly have pitches 55/59/62 and scoretk has none;
of `02- Les anges - Alto` it is tick 68, where three voices have pitches 69/76
in the reference and musicxml2ly but only 61 in scoretk; of `01- Trompettes`
it is tick 256, where pitch 41 is missing. In each case scoretk's notes are
**shifted later by 28 quarters (7 bars)**, not absent. The construct is the
multi-measure rest: `musicxml2ly` writes the number of rest measures actually
present in the MusicXML, while the writer trusted `Measure::multi_measure_rest`
even when it overstates them.

This is **not** a reader defect: `scoretk file.mscz --to musicxml` was compared
to MuseScore's own MusicXML export for these four files — identical note count
and 100 % N2 against the reference for `Les anges`/`01- Trompettes` (both 2022
and 1813 notes). The model given to the writer is right; the writer inserted
seven phantom bars. The `.ly` control case `silence_multi_mesures_ne_mange_pas_
les_notes_suivantes` already required not swallowing real music; the new
multiplier test below adds the converse case (field larger than the music).

| Cause (first divergence) | Lost N2 matches | Files | Decision |
| --- | ---: | ---: | --- |
| Multi-measure-rest multiplier | 12 274 | 34 | **Fixed below**: derive the multiplier from the measures present and the measure length, capped by the field. |
| Invisible/cross-staff/courtesy accidents | 30 | 7 | Model carries no visibility/cross-staff data (§7); below 0.3 point. |
| MuseScore 1.x/2.x input | 0 (excluded from both) | 22 | Reader limit of the other agent, not this crate. |
| Tremolo/glissando/playback coupling | — | — | Reference artefacts of §13; no writer-side equivalent. |

### Cause fixed: `\transposition` identity reset

Notation Reference §1.3.4 (Instrument transpositions) states that
« The `\transposition` may be changed during a piece » and prescribes
`\transposition c` for a concert-pitch instrument; LilyPond's
`scm/define-context-properties.scm:475-478` defines `instrumentTransposition`
as the pitch sounding for written middle C and says it is « used to transpose
the MIDI output ». It is therefore a **persistent context property**: after a
B-flat clarinet changes to a non-transposing instrument, omitting the command
keeps the MIDI transposed a tone high. `musicxml2ly` writes the same command
(`python/musicexp.py:4507-4512`, `Transposition.ly_expression`). The writer now
emits `\transposition c` on the measure where `written_transposition` returns
to zero after a non-zero value. A reduced two-staff test (chords, a rest, a
tie across the bar line, bar checks, and an untouched non-transposing control
staff) failed before the fix and passes after it.

### Cause fixed: numeric lyric syllables

Notation Reference §9.1.2 (Entering lyrics): « A word or syllable of lyrics
begins with an alphabetic character » (plus `_ ? ! : '`, control characters
and characters above 127). `lily/lexer.ll:568-612` puts the `FRACTION`,
`STRICTREAL` and `UNSIGNED` rules before the lyric-word rule, whose **two**
character classes both exclude digits: a digit anywhere (`7`, `a7`) is lexed
as a duration and LilyPond answers « this is not a duration » (verified with
a minimal `.ly` compiled by LilyPond 2.26). The writer now quotes a syllable
that cannot be a bare lyric word — a space, a digit, or an invalid first
character — using the same quoting it already applied to spaces. On
`La_foule.mscz` this changes `\skip 1 7` into `\skip 1 "7"`; LilyPond 2.26 now
compiles the output with **0 errors instead of 25** (`compile_ok` false →
true). Its N2d stays 52.62 %: LilyPond already emitted the MIDI before the
fatal error, so the gain is compilation fidelity, not notes. The control
cases (`m`, `l'ami!`, `ta.`) remain bare words and the corpus snapshots are
unchanged.

### Cause fixed: multi-measure-rest multiplier

Notation Reference §2.2.3 (Full measure rests): « The duration of a
multi-measure rest must always be equal to the length of one or several
measures. » `musicxml2ly` builds the multiplier from the duration actually read
(`python/musicexp.py:310-311`, `Duration.ly_expression`:
`if self.repeat != 1: dur_str += f'*{self.repeat}'`), not from an annotation. The
writer used `Measure::multi_measure_rest` verbatim. On the four orchestral files
analysed above the field says 13 but only six measures of silence precede the
music, so `R1*13` shifted every following attack by 28 quarters in four staves
while the model, MuseScore's own MusicXML export and the reference MIDI all
place the notes at tick 68. The writer now computes the multiplier as
`max(consecutive all-rest model measures, measure.len/nominal_len)`, capped by
the field; the two earlier tests (field smaller than or equal to the music)
still pass, and a new reduced test (field 5, three rest measures, then a note)
failed before and passes after, emitting `R1*3`.

Full re-measurement of the four-lot oracle with the final binary
(`/tmp/ly95-final`, same cached references, all launches under
`timeout 10 prlimit --as=2000000000`):

| Lot | Baseline N2 % | Final N2 % | Change on comparable files |
| --- | ---: | ---: | --- |
| corpus | 78.36 | 78.36 | 0 gain, 0 loss (56 files) |
| `.mscz` | 85.35 | 82.23 | **+1.99 points** (85.35 → 87.34) on the 166 files measurable in both runs |

The raw `.mscz` aggregate fell only because five large scores had no cached
reference MIDI in the baseline run and gained one in the final run
(`Vingt_danses_enfantines_pour_piano.mscz` alone has 26 477 reference notes).
On the 166 common files the fix gains **+6878 N2 matches over 12 files, with
zero losses**: the `Minuit chrétien` parts (+1105, +1089, +1089, +1004, +872,
+549), `00- Il est né … choeur` (+340) and four `Adeste fideles` parts
(+220, +195, +168, +151). The first two fixes are byte-inert: the intermediate
binary (`6398909639c5933b…`) was compared to the frozen one on all 637 lot
sources: **637/637 identical `.ly`** (one transient 10 s timeout on a large
Vivaldi score was rerun alone in 9.4 s with identical bytes). No corpus
snapshot changed and no file loses N1, N2 or N2d.

The first version of that fix was inert: its helper scanned the item list from
index 0 instead of the current item, so every score kept the marker's value.
Instrumenting one conversion showed `mi=11 count=13 presents=55 etendue=1`,
obviously an out-of-range scan. After passing `item_index` to the helper, the
**real** measure grid is used. The earlier conclusion that `Les anges` was a
reader defect was therefore wrong and is retracted here: with the corrected
helper those files match musicxml2ly exactly. The reduced test
`grille_de_mesures_preval_sur_la_len_du_multi_silence` (a 13-bar `len` plus five
distinct rest measures, then a note) failed before and passes after.

### Final re-measurement (all three lots, common files)

| Lot | Common files | scoretk N2 | musicxml2ly N2 | Gap |
| --- | ---: | ---: | ---: | ---: |
| corpus | 46 | 90.61 % | 91.98 % | −1.37 pt (27 notes) |
| `.mscz` | 143 | **92.15 %** | 91.33 % | **+0.82 pt in our favour** |
| `.ly` | 118 | **90.72 %** | 87.41 % | +3.31 pt in our favour |

Per-file gains from the corrected helper (`.mscz`, against the previous run):
`Les anges - Alto` and `- Violons` +1162 each, `Adeste - Trompettes` +1230,
`Minuit chrétien` parts +964/+964/+840/+719/+468, `Adeste - Flûtes` +955,
`- Basson` +937, etc. — 30 files gain 14 292 matches. One apparent −2588 on
`Vivaldi … Juravit Dominus` was a 10 s conversion timeout under load; an
isolated retry in 6.9 s produced **byte-identical** `.ly` to the previous run.
No corpus file changed and no file loses N1, N2 or N2d.

### Cause fixed: stretched chord and its covered measures

Notation Reference §2.1.3 (Scaling durations) allows `4*N`: a chord whose
model `ticks` spans several measures is written once with that multiplier.
`musicxml2ly` does the same (`python/musicexp.py:310-311`). The writer also
re-emitted the following empty model measures as `s1`, adding phantom bars and
shifting the music (first divergence of `HavaNagila.ly`: `<e g b>1*4` followed
by three `s1` instead of the single chord). It now skips measures covered by
the previous element's `end_tick`, exactly as it already did for multi-rests.
The reduced test `accord_etire_ne_reecrit_pas_les_mesures_couvertes` failed
before and passes after; the targeted compile gives `HavaNagila` 156 → 203 N2
matches (96.21 %, same as musicxml2ly), with no change on the other sampled
`.ly` files and no corpus snapshot change.

### Remaining causes, ranked and deliberately not fixed

| Cause | Notes (lot) | Verdict |
| --- | ---: | --- |
| Ottava playback | 19 (corpus, 4 files) | **Not fixable with `\ottava`.** Notation Reference §1.3.3 states it leaves the sounding pitch unchanged; a minimal `.ly` compiled with LilyPond 2.26 confirms `f4` under `\ottava 1` still plays MIDI 65. Reproducing MuseScore's shift would require writing sounding pitches or composing `\transposition`, breaking the `.ly` ↔ model octave convention (§13). A placement fix was tried and retracted: it changed four snapshots for zero N2 gain. |
| `03_05_Enfants`, `Lassus_In_pace`, two snippets | 103 (`.ly`) | 0.64 point of that lot; model dumps show the first diverging attack is already wrong in the MusicXML export for `Lassus` (pitch 64 absent at tick 30), i.e. a reader-side model, and the other case is a single wrong written pitch under a multi-score `\transpose`. Both below the actionable threshold for this crate. |
| `La_foule`, accordion octave coupling | 821 (`.mscz`) | 0.30 point on that lot; MuseScore reference artefacts (playback coupling, §13) plus one wrong written pitch under invisible elements. |

All three lots now beat musicxml2ly on exact files; on the common files
scoretk wins `.mscz` (92.15 % vs 91.33 %) and `.ly` (90.72 % vs 87.41 %) and
trails only on the corpus (90.61 % vs 91.98 %, 27 notes). Every remaining
cause is either below 0.3 point or requires the reader, which this campaign
does not touch.

### Re-ranking against the reference MIDI (all four lots, 2026-11-20)

The target is now **95 % N2 against the reference MIDI**, not musicxml2ly.
Full re-measurement with the merged binary `9b5b6ed5524f3fc0…` (all four
whole lists, cached references, `timeout 10 prlimit --as=2000000000`,
LilyPond compilation 20 s / 2 GB) gives:

| Lot | Files | Reference notes | N2 matches | N2 % | Files < 95 % | Lost notes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 2320 | 1818 | 78.36 | 16 | 482 |
| `.mscz` | 174 | 351 649 | 322 213 | 91.63 | 73 | 28 319 |
| xml | 32 | 17 612 | 16 837 | 95.60 | 6 | 630 |
| `.ly` | 375 | 37 441 | 30 592 | 81.71 | 35 | 6803 |
| **all** | **637** | **409 022** | **371 460** | **90.82** | **130** | **36 234** |

By first diverging attack: `octave` 33 761 notes / 100 files,
`attaque_decalee` 1874 / 20, `notes_manquantes` 599 / 10.

**Every top-ranked loss was checked with a MusicXML dump of the model before
attributing it.** For the biggest files the model itself already disagrees
with the reference, so they belong to the reading agent, not to this writer.
The `N2(model)` column below is the multiset match of `scoretk S --to
musicxml` against the same reference MIDI, using the oracle's Q16 grid.

| File | Lost | N2(model) | N2(written) | Attribution |
| --- | ---: | ---: | ---: | --- |
| `JMHaydn_Viderunt_omnes.mscz` | 876 | 90.70 % | 88.99 % | Reader 1.7 pt; rest = unison artefact |
| `Espana_Cani.mscz` | 1544 | 49.84 % | 49.84 % | Reader |
| `El_Choclo.mscz` | 867 | 23.51 % | 25.32 % | Reader |
| `Sicut_Cervus.mscz` | 550 | 50.85 % | 50.85 % | Reader |
| `Minuit chrétien` (9 parts) | 538 each | 91.62 % | 89.73 % | Reader 1.9 pt; rest = unison artefact |
| `Angels_Carol.ly` | 1568 | 1.24 % | 2.43 % | Reader |
| `GaudeteChristusEstNatus.ly` | 1070 | 0.37 % | 0.37 % | Reader |
| `AStMichel.ly` | 657 | 1.35 % | 1.35 % | Reader (model 130 notes vs ref 666) |
| `22_StMichel.ly` | 352 | 9.97 % | 9.97 % | Reader |
| `03_05_Enfants.ly` | 167 | 67.29 % | 55.23 % | Writer (45) + reader (122) |
| `À-Notre-Dame.ly` | 108 | 100 % | 53.04 % | Repeat-unfold artefact (see below) |

### Reader causes for the reading agent (do not touch import here)

1. **MIDI-only `\score` with `\transpose`** — `GaudeteChristusEstNatus.ly`
   (`\score { \transpose d e … }`, model first chord MIDI 45/48/52/57 against
   reference 47/50/54/59, 2 semitones low), `22_StMichel.ly`
   (`\unfoldRepeats \articulate {\transpose c c' …}`, model an octave low),
   `À-Notre-Dame.ly` (`\transpose d e`). Reference: Notation Reference §1.2.2
   (Transpose) and §24.3; LilyPond `lily/parser.yy` `transpose` production.
   File to inspect: `crates/scoretk-lilypond-import/src/playback.rs`
   (detection of a second, MIDI-only score) and `import/src/events.rs`
   (application of the interval to notes and key signatures).
2. **`\clef "treble_8"` / `\clef "G_8"`** — `22_StMichel.ly`, `AStMichel.ly`:
   model pitches one octave below the reference. Reference: Notation
   Reference §1.3.1 (clefs), §1.3.3 (octave shift); `lily/parser.yy` clef
   production and `scm/parser-clef.scm`. Inspect `import/src/events.rs`
   (clef handling) and `import/src/pitch.rs` (absolute octave).
3. **`\chordmode` grids and multi-score selection** — `AStMichel.ly` model has
   130 notes against 662 in the reference: voices from a second score are
   lost. Inspect `import/src/parser.rs` and `import/src/builder.rs`
   (score selection with `toplevel-music-handler`).

### Reference artefacts proven with LilyPond 2.26

- **Unison chords**: `<g g g>4` compiled to MIDI yields a single pitch 67,
  whereas MuseScore's reference plays all three. This explains the ~99 notes
  per `Minuit chrétien` part and ~130 in `JMHaydn`. Not a writer defect.
- **`\ottava` playback**: `\ottava 1` leaves MIDI unchanged (verified: `f4`
  still plays 65), Notation Reference §1.3.3 « leaving the sounding pitch
  unchanged ». MuseScore's reference does shift; unfixable in the writer
  without changing the model↔`.ly` octave convention.
- **Repeat unfolding of `.ly` references**: the writing oracle injects
  `\unfoldRepeats` into both candidates but the reference MIDI of a `.ly`
  source is LilyPond's plain `\midi`. Minimal compile: `\repeat volta 2 {
  c' d' e' f' } g'` gives 5 notes plain, 9 unfolded. This is why
  `À-Notre-Dame.ly` has 330 written notes against 230 reference notes even
  though the model matches the reference 100 %. Oracle/method artefact, not
  a writer loss.
- **Tremolo / glissando / accordion playback coupling** — §13: MuseScore's
  reference renders tremolos as repeated strokes and couples octaves;
  LilyPond sustains them. No writer-side equivalent.

No writer cause worth more than 0.3 N2 point remains after these attributions;
the 95 % target requires the reader-side fixes above plus the artefact
denominator corrections.

## 21. Reading campaign, third pass (2026-11-20)

### Current ranking (binary after the first two passes)

Same 4550 files and cached references; lost notes = `ref − matched N2 pairs`
at the first diverging attack:

| Construct at first divergence | Lost notes | Files |
| --- | ---: | ---: |
| `\voiceOne` / `<< … >>` | 64 759 | 306 |
| `\grace` / `\acciaccatura` | 59 812 | 151 |
| `\tuplet` / `\times` | 28 217 | 88 |
| none of the tracked constructs | 24 605 | 116 |
| `R1*n` / `\bar` | 17 611 | 56 |
| `s` spacer | 14 914 | 39 |
| variables | 10 860 | 126 |
| `\set Timing.*` | 6 722 | 29 |
| `\repeat volta` / `unfold` | 5 523 | 33 |

### Why the grace row was a detection site, not the cause

The three files with the most lost notes in that row were reconstructed note
by note from the stored reference MIDI and candidate XML:

- **`Symphony5_2.ly`** (6126 notes): the first seven notes match exactly, then
  everything is shifted by **+3.5 quarters**. The candidate has four empty
  measures before the first note (seven voices of `\partial 8 r8`, then `R4.`
  in 3/8), while LilyPond's MIDI starts at 0.5. This is the global-measure
  framework of §19 (one voice's full-measure rests delay every voice), not a
  grace onset.
- **`ballade-4.ly`** (5600) and **`Cellosonata_No3_1.ly`** (5037): the first
  divergence is at attack 0 with, respectively, 45 and 7 declared staves for
  a two-staff score and a large chord of candidate notes at tick 0. The
  construct "grace" was simply present in the first bar of the source window.

Reduced sources settle the grace semantics themselves: `c'4 \grace { d16 } e'4
f'2`, the same inside `\relative`, and `\acciaccatura`/`\appoggiatura`
variants were compiled with LilyPond 2.26 and compared on the Q16 grid. Durations,
octaves and the following note's attack all match; only the N2-grace onset rule
of §19 differs, and it moves the aggregate by 0.03 points. Notation Reference
§2.6.1, `lily/grace-iterator.cc` and `lily/midi-walker.cc` are therefore
satisfied by the reader: no reader change is warranted for this row.

### The context-creation bug and why the direct fix was rejected

`lily/parser.yy` `context_prefix` (l. 1686–1690) maps `\context` to
`context_find_or_create` and `\new` to `context_create`; both create a **Voice
context inside the current Staff** (Notation Reference §5.2.1), never a sibling
Staff. `lily/context.cc::find_create_context` reuses a context with the same id.
The reader's `collect_staves_in` instead turns every `Music::New` of kind
`Voice` into a new `StaffBlock`: a two-staff `PianoStaff` containing one
`\new Voice` already produced three staves, and `BWV865_Fuga20` produced
eleven. A count over the stored outputs finds **230 files** whose declared
`<staves>` exceeds the number of MIDI tracks (e.g. `ballade.ly` 113 vs 2,
`ballade-4.ly` 45 vs 2, `BWV865_Fuga20.ly` 11 vs 2).

A first fix merged these `\new Voice` groups into the current anonymous staff,
capping it at the model's four voices. It reduced the stave counts (11 → 7,
45 → 14, 7 → 2) but **lost notes**: `BWV865_Fuga20` N1 fell from 2376 to 1876
and `ballade-4` from 4788 to 4618, because voices that LilyPond keeps on one
staff exceed the model's four-voice limit and are dropped. Marking anonymous
`\new Voice` voices as mergeable temporary voices recovered part of the loss
but not all of it. The change was reverted rather than shipped as a regression:
a correct fix needs a voice-allocation redesign (start ticks and overlap
handling per voice), outside a localised parser patch.

### `\repeat tremolo` (fixed)

Notation Reference §4.2.2: "The duration of the tremolo equals the duration of
the braced expression multiplied by the number of repeats". LilyPond 2.26's
MIDI was checked for n = 2, 4, 6, 8 on `\repeat tremolo n { c'16 d'16 }`: it
plays **two sustained notes**, each `n ×` the written duration, with no extra
attacks. The reader read the body once with unstretched durations, so every
later note arrived early (up to 1231 lost notes in ten sources, led by
`promenade-6.ly`). `\repeat tremolo n` now scales the body's durations by `n`
at parse time; the reduced test `repeat_tremolo_etire_les_durees` fails on the
old code (case n = 1 and a bar check included). On `promenade-6.ly` N2 goes
from 65 to **319 of 1207** notes.

### Full measurement of the third pass

Frozen `/tmp/ly95lect-fix9`, same manifests and cached references, every
conversion under `timeout 10 prlimit --as=2000000000`:

| Lot | Exact | N1 % | N2 % (no repeat) | N2-grace % (no repeat) |
| --- | ---: | ---: | ---: | ---: |
| mus | 1136 (78.5 %) | 99.22 | 93.51 | 93.51 |
| ra | 1266 (73.3 %) | 90.93 | 81.25 | 81.70 |
| mut | 435 (31.6 %) | 97.83 | 78.91 | 78.93 |

Per file against the previous run: **25 N2 gains, 0 N1 or N2 loss**, and the
number of exact files goes from 2820 to **2837**. The official repeat-free N2
stays at 83.20 % because the "\repeat tremolo" sources are all repeat-flagged
by the oracle; over **all** files N2 goes from 80.528 to **80.606 %**. The
apparent drop of the Mutopia aggregate (80.74 → 78.91 %) is a denominator
effect: the previous run had four 10-second timeouts and this one has none, so
40 000 more reference notes are now counted. Restricted to the files measured
in both runs, N2 is identical before and after the fix except for the tremolo
sources.

### Context creation, completed (fourth pass)

The direct_merge attempt described above was replaced by a three-part fix that
keeps every note:

1. `\new Voice` / `\context Voice` inside a Staff stay **voices of that Staff**
   (`lily/parser.yy` 1686–1690 `context_prefix`: `context_create` /
   `context_find_or_create`, `lily/context.cc::find_create_context`, NR §5.2.1).
   They are no longer emitted as sibling staves.
2. Consecutive Voice groups that belong to one `<< … >>` (or one `{ … }`) are
   interpreted by **one** interpreter, so their sequential positions are kept;
   `assign_voice_slots` gives the first four voices the four model slots and
   **reuses a slot as soon as the previous voice's last event has passed**
   (LilyPond contexts die at the end of their music).
3. Only a voice that is still active when all four slots are occupied is moved
   to a sibling staff, with a warning — instead of being dropped at the old
   `index >= VOICES` guard.

Two reduced tests: `new_voice_reuse_un_emplacement_de_voix` (four successive
`\new Voice`, one staff, one occupied track, all eight notes kept, two `\new
Staff` still two staves) and the existing
`transposition_du_global_atteint_les_voix_de_la_portee`, updated because the two
guitar voices now share one part, with the `\global` transposition still
applied. On the 230 files that previously declared more staves than the
reference has MIDI tracks, **140 now declare fewer, none more, and 68 exactly
match the reference** (e.g. `pathetique-1` 14 → 5 staves, N1 4111 → 4154 and
N2 2890 → 2946; `Cellosonata_No3_1` 7 → 3, N1 5111 → 5210).

#### Final state of the context fix

Narrowing the grouping to a Staff whose content is only Voice contexts and
has more than two of them keeps the common two-staff idiom untouched (each
`\context Staff` keeps its own part) while fixing the many-voices scores. The
safety cap on sibling staves was raised to 1024 so no voice is ever dropped.

| Lot | Exact | N1 % | N2 % (no repeat) | N2-grace % (no repeat) |
| --- | ---: | ---: | ---: | ---: |
| mus | 1136 (78.5 %) | 99.22 | 93.51 → **93.44** | 93.44 |
| ra | 1269 (73.5 %) | 91.28 | 81.25 → **81.66** | 82.12 |
| mut | 439 (31.9 %) | 98.12 | 80.12 → **79.15** | 79.17 |

Against the previous run: **51 N1 gains / 1 loss, 53 N2 gains / 2 losses**,
exact files 2837 → **2844**. On the 230 files that declared too many staves,
143 now declare fewer, none more, and 79 match the MIDI track count.
`pathetique-1` gains 43 played notes and 56 matched attacks; `ballade` goes
from 113 staves to 41 with 17 more matched pitches; `BWV865_Fuga20` from 11 to
7 without losing a note.

The three per-file decreases are explained and are not note losses:
`milan-pavan2.ly` **gains** 61 pitches (N1 143 → 204) while its new attacks
differ from the stale reference (N2 143 → 72); `AveMaria-midi.ly` keeps all
155 pitches but its two `\context Staff=trackB` (same id, so LilyPond reuses
one staff) now share a part, which changes the oracle's per-part cursor
attacks (N2 139 → 39); `LVB_Sonate_10no1_3.ly` loses one pitch out of 1975
(a tie/grace edge) while N2 stays unchanged.

### The "+3.5 quarters" shift: construct located, fix deferred

The reduced source

```lilypond
\new PianoStaff <<
  \new Staff { \time 3/8 \partial 8 r8 c'8 d'8 e'8 | f'4. }
  \new Staff { \time 3/8 R4. R4. R4. R4. c4. }
>>
```

reproduces the shift seen in `Symphony5_2` and the Rutter carols: LilyPond's
MIDI plays 60/62/64 at 0.5/1.0/1.5 and the last note of the first staff at
2.0, while scoretk writes it at 3.0. The culprit is the first `R4.` of the
second staff: it is written with the **pickup measure length** (0.5 quarter)
instead of the nominal 3/8 (1.5 quarters). A full-measure rest is stored as
`Duration::measure()`, whose written duration comes from `Measure::len` — and
`Measure::len` is the anacrusis length in measure 1. Notation Reference §2.2.3
requires `R` to span the current time signature, and §2.3.3 keeps the meter
unchanged in an upbeat. Fixing it means distinguishing `Measure::len` from
`Measure::nominal_len` for full-measure rests in the writer and in the measure
framework; this is a cross-cutting change and is deferred rather than patched
in the reader.

## 22. Reading campaign, sixth pass (2026-11-20)

### Reconciliation of the `mut` figure

The reported "80.12 → 79.15" compared runs with **different denominators**.
Exact aggregation of the `mut` rows (repeat-free files only):

| Run | Ref notes included | Matched | N2 |
| --- | ---: | ---: | ---: |
| before the tremolo fix (`ly95-fix4-run`) | 552 944 | 446 448 | 80.74 % |
| after tremolo, before context (`ly95lect-trem-run`) | 593 055 | 467 994 | 78.91 % |
| after context (`ly95lect-ctx3-run`) | 593 055 | 469 424 | **79.15 %** |

The 40 111 extra reference notes come from four sources that timed out in the
first run (10 s) and no longer did in the later ones: their state changes from
`echec` to `ok`, adding low-scoring notes to the denominator. With the same
denominator, the context fix **raises** `mut` N2 by 0.24 points: of the 1377
files, 48 change (`ΣΔref = 0`, `ΣΔmatched = +2635`), one loses 71 matched notes
and one loses none. The apparent drop was therefore entirely a denominator
effect, not a regression.

### `R4.` in an anacrusis: reference, two faithful attempts, blocker

`lily/parser.yy` `event_chord` uses
`MULTI_MEASURE_REST optional_notemode_duration` and passes that duration to
`multi_measure_rest`; `ly/music-functions-init.ly:1697` defines `\partial` as a
`PartialSet` in the Timing context — it changes `measurePosition`, **not** the
last duration. Notation Reference §2.2.3 requires `R` to cover the current
time signature and §2.3.3 keeps the meter unchanged in an upbeat. A reduced
test `silence_de_mesure_sous_levee_garde_la_duree_ecrite` was written first and
failed (`ticks = None`).

Two implementations were tried and **reverted**:

1. Store `event.actual` as the full-measure rest's model `ticks` when its
   duration is explicit. The rest then has the right written duration (3/8)
   but the shared measure 1 keeps the pickup length, so the XML writer extends
   measure 1 and every later note shifts by one quarter.
2. Also extend the global measure 1 to the explicit rest's duration. The
   up-staff pickup boundary and the down-staff full-bar boundary then collide
   in `plan_measures_polymetric`, producing a 3/4 measure 1 and leaving the
   down-staff note half a quarter late.

A faithful fix requires **per-staff measure grids**: LilyPond's MIDI timeline
is continuous, while the pivot model has one shared measure framework, so a
0.5-quarter pickup on one staff and a 1.5-quarter `R4.` on another cannot both
be represented. This is a model-level change, outside this reader campaign;
the reduced source and both failed attempts are recorded here rather than
leaving a regression in the reader.

### Next rows: first divergences and governing references

| Row | Three biggest files (lost notes) | First divergence and construct |
| --- | --- | --- |
| `\tuplet`/`\times` | `troldhaugen` (3401), `LVB_Sonate_57_3` (3145), `biga` (2022) | 8.0 quarters with a duplicated F♯4; 1.5 quarters with two low notes added by a `\change Staff` voice; 0 with a one-octave error on the first chord. NR §2.1.2–2.1.3, `lily/parser.yy` `tuplet` |
| `R1*n`/`\bar` | `Oyseaux-Endfassung` (2586), `ricercare6` (2149), `Cellosonata_No3_2` (2039) | 52, 156 and 121 quarters: a constant −28/−32/+3-quarter shift of the rest of the piece, i.e. the shared-measure issue above. NR §2.2.3 |
| none | `Ave_dulcissima_Maria` (163), `baba` (145), `Chop-28-17` (145) | 150/18/16 quarters, best global shift −2/−8/−27: late drift, no local construct at the divergence. NR §2.1.1 |
| `s` spacer | `rach-prelude23-05` (2380), `Angels_Carol__John_Rutter-midi` (1714), `a-breeze-from-alabama` (1572) | 48/8.5/0 quarters, best shift −4/−3/+6: the spacer consumes time correctly but the surrounding voices drift. NR §2.2.2 |
| variables | `20title-hub`, `21title-hub` (1669 each), `chopin_fantaisie-impromptu` (1066) | attack 0 with 6 extra notes and a +25-quarter shift; 0 with +25; 0 with −8. NR §4.4.4, `lily/parser.yy` `identifier` |

No local, reference-backed reader fix was found for these rows in this pass:
they are all instances of the shared measure grid or of voice allocation
already documented above, and changing them locally would regress the files
fixed in §21. They are recorded with their exact divergences rather than
patched blindly.

### Cross-bar rests: the single-grid fix (not a model limit)

Correction to the previous section: LilyPond also has one grid.
`ly/engraver-init.ly:743-756` establishes `Timing` as an **alias of `Score`**,
and `ly/music-functions-init.ly` `partial` only issues a `PartialSet` in that
context; a staff whose `R4.` overruns the pickup does not get its own bar.
The reader's model ticks were already correct — the bug was that a
`ChordRest` spanning several measures was stored as one `<rest measure="yes"/>`,
which the writer truncates to the first measure's length. `place_chord` now
splits any full-measure rest that crosses a barline into consecutive rests,
each with its explicit duration (and a matching notated figure), while
in-measure rests keep the normal full-measure representation.

The reduced two-staff test `elements_a_travers_les_barres_sont_decoupes`
failed before the fix and now matches LilyPond 2.26's MIDI exactly: notes at
0.5/1/1.5/2 quarters on the upper staff and 6 quarters on the lower one.
Its assertions cover absolute ticks and the four explicit rest pieces.

Full lots, **constant denominator** (same file set and 593 055 reference
notes for the repeat-free `mut` aggregation as in the previous run):

| Lot | N2 before → after | Exact |
| --- | --- | ---: |
| mus | 93.44 → **93.67** | 1136 |
| ra | 81.66 → 81.66 | 1269 |
| mut | 79.15 → **80.26** | 435 → 442 |
| all matched notes | 615 076 → **621 967** (`+6891`) | 2844 → 2847 |

Per file: **31 N2 gains, 15 losses, 0 N1 change**. The losses are all small
(≤21 notes) and occur where the writer's `<backup>` layout changes by one
tick for surrounding notes; no note is lost and the reduced source follows
the reference exactly. The rows that were previously attributed to the
"shared grid" improve as expected: `R1*n` 17 338 → 16 730 lost notes (56 → 55
files) and "none" 24 988 → 23 890. `\tuplet`, `s` and variables are unchanged
and keep the first divergences recorded in the table above.

### Exact piece figures and `\tuplet` span (seventh pass)

The cross-bar rest split now uses the project's own duration decomposition:
a local port of `engraving/durationtype.cpp::toDurationList` (the same function
as `scoretk-midi-import::meter::to_duration_list`, l. 412), with up to four
dots. Each bar piece becomes one or more rests whose written duration sums to
it exactly: `3/8` is a dotted quarter, `5/16` a quarter plus a sixteenth,
`7/16` a twice-dotted quarter. The reduced test
`silences_decoupes_en_figures_exactes` checks a `3/8` and a `5/16` piece and
fails on the previous approximated figures.

`ly/music-functions-init.ly:2470` defines `\tuplet` with an **optional**
`tuplet-span` duration (`\tuplet 3/2 8 { … }`); it is used for engraving
grouping but the music is always compressed by the ratio. The reader did not
consume that token, so it was read as a bare duration note and the tuplet was
lost. It is now consumed without changing the default duration. The reduced
test `tuplet_avec_duree_de_regroupement` asserts exact pitches, the 3/2 ratio,
six tuplet notes, the measure count and the unaffected no-span case; on
`Lassus_In_pace` N2 goes 43 → 79 and on `finger-glide-spanner` 4 → 22.

Full lots, same manifest and cached references as the previous run:

| Lot | N2 % (no repeat) | Exact |
| --- | --- | ---: |
| mus | 93.67 → **93.69** | 1136 |
| ra | 81.66 → **82.31** | 1279 |
| mut | 80.26 → **80.46** | 443 |
| all exact | | 2847 → **2858** |

Per file: **23 N2 gains / 1 loss** and **1 N1 loss**. `baba.ly` loses 17
matched pitches but gains **847** matched attacks (N2 1056 → 1903) because a
tuplet span was previously misread. `liszt-consolation-no3.ly` keeps all its
pitches but loses 51 attacks: the newly preserved note at tick 0 is absent
from the reference, an interaction with the rest inside that tuplet which is
outside this fix. No denominator changed.

### Factorisation de `toDurationList` (neuvième passe)

La même fonction existait en trois exemplaires : `gap_rests.rs:237` (sans
points), `scoretk-midi-import/src/meter.rs:412` (avec `useDots`/`maxDots`) et la
copie locale du lecteur LilyPond. Un port public unique vit maintenant dans
`scoretk-core/src/duration_list.rs`, avec l'en-tête GPL, la notice de
modification (« Portage de `engraving/dom/durationtype.cpp::toDurationList` …
modifié (réécrit en Rust) en 2026 ») et des tests unitaires (3/8, 5/16, 7/16
avec et sans points, Long/Breve, somme exacte). La signature suit le C++ :
`(length, useDots, maxDots, printRestRemains)` ; le drapeau de journalisation
est conservé mais sans effet dans un noyau sans I/O. L'ordre des essais est
celui du constructeur `TDuration(l, true, maxDots)` : types du plus long au
plus court, puis points décroissants. `gap_rests` passe `useDots = false`,
`maxDots = 0` ; `Meter` passe 1 point ; le lecteur LilyPond passe 4.
Vérification : 134 suites de tests `--workspace`, et les deux arbres de sorties
générés par `oracle/compare.sh` avant et après sont **octet pour octet
identiques** (489 écarts préexistants inchangés de part et d'autre).

### Re-classement après le correctif de `\tuplet`

La rangée `\tuplet`/`\times` passe de 28 217 à 25 518 notes perdues (88 → 74
fichiers). Les rangées restantes, avec leurs trois plus gros fichiers et la
première divergence (`best` = décalage global qui aligne le plus de notes) :

| Rangée | Notes / fichiers | Trois plus gros fichiers (div, décalage) | Référence |
| --- | ---: | --- | --- |
| `\voiceOne`/`<< >>` | 77 806 / 288 | `violin_concerto_1` (4, +1), `Symphony5_1` (−3,5, −21), `ballade` (0, +9) | NR §5.2.1, `parser.yy` `context_prefix` |
| graces | 73 573 / 151 | `Symphony7_1` (0, −3), `Symphony5_2` (0,5, −26), `Violinsonata_No9_1` (26, +4) | NR §2.6.1, `grace-iterator.cc` |
| « aucun » | 24 001 / 118 | `Beatus_vir_primo` (28, −16), `Chop-28-17` (16, −27), `mendelssohn-scherzo` (57, −18) | NR §2.1.1 |
| `R1*n`/`\bar` | 16 694 / 55 | `Oyseaux-Endfassung` (52, −28), `ricercare6` (156, −32), `Cellosonata_No3_2` (121, +3) | NR §2.2.3 |
| `s` | 13 928 / 38 | `rach-prelude23-05` (48, +1), `Angels_Carol` (8,5, −3), `a-breeze` (0, +6) | NR §2.2.2 |
| variables | 10 018 / 116 | `20title-hub`/`21title-hub` (0, +25), `chopin_fantaisie-impromptu` (0, −8) | NR §4.4.4 |

Toutes ces divergences sont des décalagesglobaux de la fin d'une pièce, liés
aux constructions déjà examinées (contextes, levées et grille de mesure), et
non un jeton mal consommé comme l'était la durée optionnelle de `\tuplet`.

### Two whole-score divergences proven to be reference artefacts

**`Symphony5_1` (divergence −3,5, best shift −21).** Bisection of the score
(the variables are defined in included files) first reduced it to one staff
with `<< \context Voice = "text" \struktur \context Voice = fl \flauti >>`:
the divergence stays. With only `\flauti` it stays; emptying `\struktur` down
to `\time 2/4 \key c \minor` it stays. The first note of the flute part is at
35,5 quarters in LilyPond's MIDI and at 38,5 in scoretk's model, but the
writer's `<backup>` chain makes the oracle see it at negative ticks — this is
the known per-part-cursor artefact.

The actual cause is in the source itself: LilyPond 2.26 reports
`échec du contrôle de mesure à 1/4` at `flauti.ly:22:13` (twice), because
`\compressFullBarRests` with `r2^\fermata R2*2 r2^\fermata` leaves the bar
position at 1/4. A minimal reduced source

```lilypond
\compressFullBarRests \time 2/4 \key c \minor
\repeat volta 2 { r2 r2^\fermata R2*2 r2^\fermata R2*12 r8 <d''' g'''>8 }
```

reproduces the warning and shows LilyPond's MIDI at 35,5 quarters against
scoretk's 34,5: **LilyPond inserts a quarter of silence to repair the bar**.
Notation Reference §2.5.4 says a bar check only reports warnings about
entered durations and that an incorrect duration can garble the score; the
reader deliberately ignores bar checks (`|` is consumed, never a barline),
so its timeline follows the written durations and the oracle's +1 is a
reference artefact on malformed input, not a reader defect.

**`Symphony5_2` (divergence 0,5, best shift −26).** The same reduction isolates
`flauti.ly:13:2` with two `échec du contrôle de mesure à 1/4` warnings;
`\partial 8 r8 R4. …` in 3/8 with a temporary `<< … \\ … >>` passage leaves
the bar position at 1/4 on the inner voice. LilyPond repairs it in MIDI and
scoretk does not, so again the reference is garbled by an input error.
`Oyseaux-Endfassung`, `ricercare6` and `ballade` were compiled too and show
**no** bar-check warnings, so their drifts are not this artefact and remain
open with the divergences recorded above.

No reader change was made for either file: fixing them would mean reproducing
LilyPond's error-repair rule, which would corrupt well-formed scores.

### §20 reader causes: MIDI-only `\score` selection

Notation Reference §24.3: a `\score` with music and `\midi` but **no `\layout`
produces only a MIDI file**. Compiling `GaudeteChristusEstNatus.ly` with
LilyPond 2.26 gives exactly one MIDI (`ly95lect-gaudete.midi`), from the second
score (concert pitch), while the first score is engraved under `\transpose d c`.
The reader used the first score, so its model was two semitones low; the same
mistake explained the "octave low" of `22_StMichel.ly` and the 130-vs-666 note
mismatch of `AStMichel.ly`.

`Parser::document` now selects a `\midi`-without-`\layout` score whenever one
exists, even when its music differs from the engraved score, and clears
`playback_transposition` because the model already carries concert pitches.
Two reduced tests were written first and failed on the old code:
`score_midi_seul_choisi_pour_le_modele` (two scores, same music under
`\transpose`) and `score_midi_seul_choisi_meme_si_musique_differente`.
The existing `transpose_traversant_articulate_et_unfold_repeats` control cases
were updated: the MIDI score now supplies the model.

Verified against LilyPond's **own** MIDI (plain compilation, no `pre.ly`):
`GaudeteChristusEstNatus` 1074/1074, `22_StMichel` 391/391, `AStMichel`
666/666 matched pitches and attacks; `Angels_Carol` 1607/1607 against its
cached reference. The `\clef "treble_8"`/`"G_8"` cause of §20 is therefore the
same root cause: a control test `clef_treble_8_ne_change_pas_la_hauteur`
confirms the clef itself never shifts pitch (NR §1.1.3/§1.3.3).

Because the reading oracle's cached MIDI came from `pre.ly` (which injects
`\midi` into the engraved score), the cached MIDI of the 191 files whose
model changed was removed from a hard-linked copy of the refs tree. The driver
then regenerated each of them with **plain** LilyPond (no injected `\midi`)
into its work directory; both binaries were measured against that same tree,
so the comparison keeps a constant denominator and a constant reference.

#### Measurement of the MIDI-score selection

Both binaries were measured with the **same** reference tree and the same
manifests (constant denominator); 191 sources whose model changed had their
cached `ly.midi` removed so the driver regenerated them with plain LilyPond
2.26. The regenerated reference is now the one LilyPond actually writes (the
`\midi`-only score), not the engraved score that `pre.ly` used to inject MIDI
into.

| Lot | N2 before → after (same refs) | Exact |
| --- | --- | ---: |
| mus | 88.85 → **94.10** | 1093 → 1115 |
| ra | 82.31 → 82.31 | 1279 |
| mut | 78.84 → **80.77** | 434 → 444 |

Per file: **133 N1 gains / 21 losses, 139 N2 gains / 26 losses**, exact files
2806 → 2838. The losses are concentrated in repeat-flagged files whose
reference still came from an older `pre.ly` compilation: `GaudeteChristusEstNatus`
loses 1070 matched notes because the cached MIDI there was the engraved,
transposed score, whereas a fresh plain LilyPond compile gives 1074/1074
matching pitches and attacks with the fixed binary. Several other losses
(`rach-prelude23-05`, `Prelude-et-fugue-en-la-majeur`) are the same stale-ref
effect; they are not reader regressions. Restricted to the files whose
reference was actually regenerated and to non-repeat files, the change is a
clear gain. The `\clef "treble_8"` cause of §20 vanishes with the selection
fix: the pitch error came from choosing the transposed engraved score, and a
control test confirms the clef itself never changes MIDI pitch.

### Réconciliation des pertes sur références fraîches

Les 27 fichiers qui had recorded a loss were recompiled with LilyPond 2.26
(plain compilation, no `pre.ly`) and their cached MIDI updated in the oracle
tree. Both binaries were then re-measured on those 27 files with the same
fresh references:

| Fichier | N1 avant → après | N2 avant → après | Verdict |
| --- | ---: | ---: | --- |
| `GaudeteChristusEstNatus.ly` | 441 → **1074** | 4 → **1074** | gain, référence obsolète confirmée |
| `Gueguen.ly` | +235 | +256 | gain |
| `Sonis.ly` | +220 | +240 | gain |
| `Prière-pour-nos-anciens.ly` (+6 copies) | 149 → 149 | 134 → **149** | **réel bug de sélection** |
| `22_HymneStMichel.ly` | +0 | +0 | corrigé par l'affinement |
| `Chop-28-16.ly` | −252 | −444 | bug lecteur préexistant, voir ci-dessous |

L'affinement a consisté à choisir le **premier score portant `\midi`**, même
s'il porte aussi `\layout` : `Prière-pour-nos-anciens` a cinq `\score` dont le
premier est `\transpose sol sib` avec `\midi`, et LilyPond écrit ce MIDI en
premier. Un test réduit
`score_grave_avec_midi_preval_sur_les_scores_midi_suivants` échouait avant
l'affinement. Les 11 fichiers qui avaient perdu 99/134 notes sont revenus à
l'état de référence. Le correctif `first_midi` a réduit le nombre de fichiers
dont le modèle change de 197 à 184.

Les pertes qui subsistent sur références fraîches sont un **bug préexistant**
du lecteur, isolé par une source réduite : LilyPond applique `\relative` à
l'arbre complet **avant** de filtrer avec `\keepWithTag` (`music-filter` de
`scm/music-functions.scm` filtre le résultat de la fonction musicale), tandis
que le lecteur filtre d'abord, puis résout les octaves. Sur

```lilypond
\keepWithTag #'midi \relative {
  \tag #'layout { <ef' a gf'>4 }
  \tag #'midi { <ef a gf'>4 }
}
```

LilyPond 2.26 joue 63/69/78 (la branche `layout` a fixé la référence relative
avant d'être retirée) ; le lecteur donne 51/57/66. Le test
`keepwithtag_apres_relative_pour_la_reference_octave` est marqué `#[ignore]`
avec cette preuve : corriger ce comportement demande de différer le filtrage
après la résolution relative, un changement d'architecture hors de cette passe.

### Fresh reference cache and baseline

The 27 losing files were recompiled with LilyPond 2.26 and their cached MIDI
updated. Their re-measurement on identical fresh references **confirmed** the
stale-ref attribution for 11 of them (`Prière-pour-nos-anciens` and six copies,
`22_HymneStMichel`, `01_01_Etudiants`, `Prelude-et-fugue-en-la-majeur`,
`mozart_abendruhe`…), and confirmed genuine gains for six others
(`GaudeteChristusEstNatus` 4 → 1074, `Gueguen` +256, `Sonis` +240,
`Partition` +139, both `Gaudete…abrege` +92). The refinement choosing the
**first score with `\midi`** even when it also has `\layout` fixed the 11
files: a reduced test
`score_grave_avec_midi_preval_sur_les_scores_midi_suivants` failed before it.

The whole cache was then regenerated: 2808 sources recompiled with plain
LilyPond, 1742 sources produce no MIDI in a plain compile and keep their
previous cached reference (both binaries see the same files). Both binaries
were re-measured on all three lists with that cache:

| Lot | Old binary N2 (fresh refs) | Current N2 (fresh refs) |
| --- | ---: | ---: |
| mus | 88.53 % | **93.95 %** |
| ra | 82.37 % | 82.37 % |
| mut | 80.16 % | **82.24 %** |

Fresh-reference baseline: **141 N1 gains / 6 losses, 147 N2 gains / 10
losses**, exact files 2769 → 2817. All remaining losses are the pre-existing
`\relative`/`\keepWithTag` ordering bug isolated above (Chop-28-16 −444 N2,
baba −461, rach-prelude23-05 −792, In_dulci_jubilo −235,…), with a failing
reduced test kept `#[ignore]` pending the interpreter refactor.

### Deux correctifs de lecture sur le lot Mutopia (2026-11-20)

#### `\relative` avant `\keepWithTag`

`music-filter` (`scm/music-functions.scm`) applique `\keepWithTag` /
`\removeWithTag` au **résultat** de la fonction musicale : dans
`\keepWithTag #'midi \relative { … }`, la conversion relative a déjà parcouru
les branches `\tag #'layout` avant qu'elles ne soient retirées
(`lily/relative-octave-music.cc`). Le lecteur filtrait d'abord, puis résolvait
les octaves, donc la référence d'octave de la branche écartée manquait.

Le filtrage est maintenant différé à l'interprétation (`Music::Filtered`,
`events.rs`) ; l'enveloppe survit à l'extraction des groupes
(`score_groups_in`), et une note filtrée met à jour la référence d'octave sans
émettre d'événement ni avancer le curseur. Le test réduit
`keepwithtag_apres_relative_pour_la_reference_octave` échouait avant le
correctif (51/57/66 au lieu de 63/69/78).

Mesure ciblée sur 11 fichiers Mutopia perdants, références fraîches : aucun
recul ; `Chop-28-16` +252 N1 / +444 N2, `baba` +88 / +461, `Chop-28-18`
+33 / +34, `Chop-28-23` +29 / +3.

#### Passages temporaires enchâssés dans `\context Staff`

`lily/parser.yy` (`context_change`, « temporary polyphonic passages ») joue
les branches `<< … \\ \new Voice … >>` dans les emplacements de la portée
courante : elles se succèdent dans le temps. L'heuristique du lecteur
comptait les `\new Voice` à n'importe quelle profondeur et ouvrait une portée
sœur dès qu'il y en avait plus de deux. Dans `ballade-4`, les 28 `\new Voice`
enchâssés sous `\relative` produisaient 14 portées et rejouaient chaque
passage au tick 0.

Seuls les `\new Voice` portés **directement** par les éléments du `<< … >>`
déclenchent désormais la répartition en portées ; les voix enchâssées dans un
enveloppeur restent une seule musique de portée, que l'interpréteur lit avec
ses passages temporaires. Le test réduit
`passages_temporaires_dans_un_contexte_staff_restent_sur_une_portee`
échouait avant le correctif (4 portées au lieu d'une).

Mesure ciblée sur dix fichiers (références et dénominateur constants) :
`ballade-4` +1171 N1 / **+5437 N2** (450 → 5887 sur 6050, 97,31 %) ; aucun
changement sur `pathetique-1`, `Violinsonata_No7_1`, `Symphony5_2`,
`Concerto_No3`, `BWV865_Fuga20`, `ricercare6` et
`mendelssohn-scherzo-eminor`.

#### Mesure complète après les deux correctifs

Une passe complète sur les trois listes, avec le même cache de références
frîches et les mêmes manifestes que la baseline, donne :

| Lot | N2 baseline | N2 après | Exact |
| --- | ---: | ---: | ---: |
| mus | 93,95 % | 93,95 % | 1119 |
| ra | 82,37 % | 82,37 % | 1279 |
| mut | 82,24 % | 82,10 % | 420 → 421 |

Le pourcentage `mut` baisse **uniquement pour une raison de dénominateur** :
trois gros fichiers ont changé de statut entre les deux passes à la borne de
10 s du pilote (`Symphony7_1` et `violin_concerto_1` réussissent après,
`Concerto_No3` échoue après). Mesurés à part sans borne (`timeout 90`), ces
trois fichiers donnent des résultats identiques avec l'ancien et le nouveau
binaire (par exemple `Concerto_No3` 7675/12338 N2 des deux côtés) : ce ne sont
pas des régressions. Avec un dénominateur constant, les gains réels sont
`ballade-4` +5437 N2, `Chop-28-16` +444, `baba` +461, `5.Inquietude` +636,
et deux pertes mineures sur des fichiers déjà désynchronisés portant des
`\tag` : `bridge` −7 et `two-jews` −6 N2, sans perte N1. Aucun autre fichier
des trois lots n'a changé.

### Troisième passe : pertes `bridge`/`two-jews`, profil, voix séquentielles

#### `bridge` −7 et `two-jews` −6 : un `s` filtré réservait encore du temps

Les six notes de `two-jews` (F3/F#3 aux attaques 99, 100, 101, 103, 104, 105)
étaient décalées de +1/2 noire, et sept notes de `bridge` de +2 noires, sans
perte N1. Les deux fichiers viennent du correctif `\relative`/`\keepWithTag` :
dans `events.rs`, le retour anticipé de `ChordContent::Skip` (`s`) avait lieu
**avant** le test `tags_pass_filters()`. Un `s` étiqueté retiré par le filtre
avançait donc le curseur et changeait la durée par défaut, alors que LilyPond
filtre l'arbre avant l'interprétation (`music-filter`, `scm/music-functions.scm`).

Source réduite compilée par LilyPond 2.26 :
`\keepWithTag #'midi { c'4, \tag #'layout { s2 }, \tag #'midi { s2 }, d'4 }`
donne les attaques 0 et 3 ; le lecteur donnait 0 et 5. Le test
`skip_etiquete_filtre_ne_consomme_pas_de_temps` échouait avant le correctif.
Après correction, les deux fichiers retrouvent exactement leur score initial
(`bridge` 172/172, `two-jews` 827/827 N2) et la source réduite 46/46.

#### Profil des conversions lentes : `link_ties` quadratique

`Symphony7_1`, `violin_concerto_1` et `Concerto_No3` basculent autour de la
borne de 10 s du pilote. Un chronométrage par phase (pas de `perf` disponible :
`perf_event_paranoid = 2`) montre que le temps est partagé entre la lecture et
l'écriture MusicXML, et que dans le lecteur `link_ties` domine : 365 ms sur
`Symphony7_1` (455 mesures, 20 556 notes), contre 103 ms pour
`place_measures` et 75 ms pour `apply_accidentals`. Pour chaque accord,
`link_ties` parcourait toutes les mesures puis tous les segments pour trouver
 celui du tick courant, soit O(événements × mesures × segments).

Un index `BTreeMap<tick, (mesure, segment)>` construit une fois ramène la
recherche à O(log n). Le test `liaisons_indexees_sur_beaucoup_de_mesures`
vérifie 61 liaisons sur 61 mesures. La sortie MusicXML est **octet pour octet
identique** avant/après sur les 1377 fichiers du lot `mut` (deux binaires,
même date d'encodage, zéro différence, zéro fichier sauté). Le temps total de
`Symphony7_1` passe de 6,0/7,0/6,5 s à 5,7/6,2/4,1 s sur trois mesures.

#### Voix enchâssées dans un `\context Staff` séquentiel

`BWV865_Fuga20` déclarait sept portées pour un `PianoStaff` à deux portées :
le contenu de `\context Staff = "up"` est une séquence (`\clef`, `\time`, puis
trois `<< … \new Voice … >>`) et passait par `collect_staves`, qui ouvrait une
portée par voix enchâssée ; chaque passage était alors lu depuis le tick 0.
LilyPond 2.26 (`lily/parser.yy`, `context_change`) les joue sur la même portée
aux ticks 0, 4 et 8. Le test réduit
`passages_temporaires_sequenciels_dans_un_contexte_staff` échouait avant le
correctif (5 portées au lieu de 2, notes aux ticks 0–3 au lieu de 0–11). Le
lecteur garde désormais la séquence entière comme unique musique de portée
quand aucune voix n'est portée directement par le `<< … >>`.

Mesure ciblée sur 26 fichiers, références constantes : `pathetique-1`
+24 N1 / +934 N2, `Violinsonata_No7_1` +313 N2, `bwv652` +554 N2,
`bridge` +7 et `two-jews` +6 (récupération du correctif précédent). Les
fichiers à deux `\context Voice` directs (`bach_siciliano_bmv_1031`,
`bach_air_bmv_1068`, `faure_dolly_berceuse`) conservent leur score initial,
et `ballade-4` garde son gain de +5437 N2. Seul `ballade.ly` présente un
arbitrage : −32 N1 (7 notes de moins, 32 hauteurs en désaccord d'octave sur
6958) pour **+1422 N2** (79 → 1501 attaques appariées) ; le gain musical
net est largement positif.

#### Bilan final sur références fraîches

Dernière passe complète après les trois correctifs de cette section, avec
les mêmes manifestes et le même cache que la baseline (le pilote n'a plus
aucun délai à 10 s, temps max 4,33 s sur `mut`) :

| Lot | N2 baseline | N2 final | Exact |
| --- | ---: | ---: | ---: |
| mus | 93,95 % | 93,95 % | 1119 |
| ra | 82,37 % | 82,37 % | 1279 |
| mut | 82,24 % | 82,34 % | 420 |

À dénominateur constant, 17 fichiers `mut` gagnent des notes appariées et
un seul perd : `ballade.ly` (-32 N1, mais +1422 N2, voir ci-dessus). Les
gains principaux sont `ballade-4` +5437 N2, `pathetique-1` +934,
`Concerto_No3` +3610 (réglage du délai), `bwv652` +554, `5.Inquietude` +636,
`baba` +461. Aucun changement sur `mus` ni `ra`.

### Règle de portée LilyPond à la place des seuils de voix (2026-11-20)

Les heuristiques `direct_named_voices > 2`, `named_voice_count > 3`,
`early_voices` et `has_any_voice` sont retirées. Un `\new Staff` ou un
`\context Staff` produit désormais **une seule portée** dont toute la musique
est gardée en une seule expression : les voix ouvertes par `\\`, `\new Voice`
ou `\context Voice` sont créées par l'interpréteur au moment où la musique
les rencontre, et se voient attribuer une des quatre pistes par
`assign_voice_slots` ; une voix terminée libère sa piste. Au-delà de quatre
voix réellement simultanées, le constructeur ouvre des portées sœurs (le
modèle n'en a que quatre). Sources : `lily/context.cc` (`find_create_context`),
`lily/parser.yy` (`context_change`, `NEWCONTEXT`), `ly/engraver-init.ly`
(`Staff \accepts Voice`), NR §5.2.1/§5.2.2. Un effet de bord a été corrigé au
passage : la durée par défaut posée par une branche d'un `<< … >>` ne fuit
plus dans les branches sœurs (`lily/music-sequence.cc`, `simultaneous_music`).

Le test réduit `une_portee_par_contexte_voix_ouvertes_a_leur_tick` échouait
avant (quatre voix de deux notes au tick 0 au lieu de deux voix de quatre
notes aux ticks 0–3). Les paroles portées par la musique d'une portée sont
conservées, et le nom de la dernière voix sert uniquement à la cible d'un
`\addlyrics`. Deux assertions d'artefacts d'implémentation ont été mises à
jour : une portée LilyPond donne une partie MusicXML (test de transposition)
et la mesure du cas `\global` + voix nommée suit le tick courant.

Mesure complète, références et manifestes constants :

| Lot | N2 avant | N2 après | Exact |
| --- | ---: | ---: | ---: |
| mus | 93,95 % | 93,95 % | 1119 |
| ra | 82,37 % | 82,34 % | 1279 |
| mut | 82,34 % | **85,26 %** | 415 |

Sur `mut`, 45 fichiers gagnent (jusqu'à +2312 N2 sur `Symphony7_1`, +2171 sur
`BWV865_Fuga20`, +2087 sur `Trio_op11_3`) et 49 perdent. Les pertes sont
analysées par cause dans le tableau suivant ; deux gros fichiers
(`violin_concerto_1`, `Concerto_No3`) ont dépassé la borne de 10 s et sont
 donc exclus de l'agrégat des deux côtés.

### Classement des pertes restantes par cause (lot `mut`)

Sur les 722 fichiers `mut` sans reprise correctement lus (références
fraîches, dernier relevé), 259 perdent encore des attaques, pour un total de
95 077 notes non appariées. Chaque fichier est classé dans la **première**
cause qui s'applique (les constructes se cumulent, les lignes ne se
chevauchent donc pas) ; les nombres entre parenthèses sont les notes perdues.

| Cause | Fichiers | Notes perdues | Exemples |
| --- | ---: | ---: | --- |
| Voix `\new|\context Voice`, `\\` (positions encore décalées) | 129 | 24 994 | `djinns` (2382), `Rumores_de_la-caleta` (1307), `BWV533` (1250) |
| Transposition / octave / `set-octavation` (Scheme ignoré) | 18 | 24 054 | `violin_concerto_1` (12200), `Concerto_No3` (3597), `Violin_Concerto_3` (1611) |
| Autre / non classé | 41 | 14 274 | `Violinsonata_No7_1` (4199), `Cellosonata_No3_2` (2039) |
| Référence réparée par LilyPond (erreurs de contrôle de mesure) | 5 | 13 567 | `Symphony5_2` (6079), `Oyseaux-Endfassung` (2586), `Beatus_vir_primo` (2579) |
| `\partial` / anacrouse | 42 | 7 622 | `miserere-mei-deus` (1357), `SongWW_opus53no5` (1213) |
| `\set measureLength` | 9 | 4 080 | `chopin_prelude_op45` (1222), `Laudate_Dominum` (635) |
| Silences multi-mesures `R1*n` | 8 | 3 582 | `contrapunctusXIV` (1341), `contrapunctusXIX` (858) |
| `\tag` | 2 | 2 578 | deux `score.ly` (1562, 1016) |
| Ornements `\grace` | 4 | 182 | `Czerny_Op_821_No_013` (121) |
| `\times` / n-olets | 1 | 144 | `Czerny_Op_821_No_014` |

#### Correctif de la plus grosse cause mesurable : multiplicateurs enchaînés

Avant ce correctif, la cause « silences multi-mesures » totalisait 16 569
notes perdues sur 60 fichiers. Un échantillon réduit a montré que le vrai
défaut n'était pas `R1*n` mais les **multiplicateurs enchaînés** :

```lilypond
\time 12/8 R1*12/8*4 c'4 d'4 e'4
```

LilyPond 2.26 place `c'4` au tick 24 (6 mesures de 12/8) ; le lecteur ne
lisait que le premier `*12/8` et le plaçait au tick 6. `parse_optional_duration_spec`
ne consommait qu'un `*n/d`. Il multiplie désormais tous les multiplicateurs
consécutifs, comme `lily/parser.yy` (`duration` → `multiplier` appliqués
successivement dans `event_chord`). Le test
`multiplicateurs_de_duree_enchaines` échouait avant (ticks 3/2, 7/4, 2, 9/4
au lieu de 6, 25/4, 13/2).

Mesure complète sur les trois lots, références constantes : 11 fichiers
gagnent (jusqu'à +1758 N2 sur un `score.ly`, +1348 sur `bwv-1006a_1`,
+1168 sur `bwv-1006a_1g`, +1099 sur `contrapunctusXII`) et trois perdent,
tous marginalement : `contrapunctusXIV` −20 et `bwv656-a4`/`bwv656-let`
−2 chacun, où le déplacement du silence révèle un décalage de bar-check
préexistant. Le pourcentage agrégé `mut` passe de 85,26 % à 84,23 % uniquement
parce que deux gros fichiers précédemment exclus pour délai
(`violin_concerto_1`, `Concerto_No3`) entrent dans le dénominateur.

### Cause 1 du tableau : `\transposition` par portée

`\transposition` est une propriété de Staff, pas de MusicXML Part. Quand un
`StaffGroup` réunit plusieurs portées, la partie n'a qu'un `<transpose>` et le
constructeur ne décalait aucune portée : les hauteurs concert restaient les
hauteurs écrites. LilyPond 2.26 joue pourtant la portée transpositrice à sa
hauteur concert (`lily/audio-item.cc`, `instrumentTransposition` porté par
`lily/note-performer.cc`) : sur

```lilypond
\new StaffGroup <<
  \new Staff { \fixed c' { c4 d4 e4 f4 } }
  \new Staff { \transposition a \fixed c' { c4 d4 e4 f4 } }
>>
```

le MIDI donne do4/ré4/mi4/fa4 puis la2/si2/do3/ré3. Le test
`transposition_sur_une_seule_portee_dun_groupe` échouait avant (8 hauteurs
écrites). Le constructeur décale désormais la concert de la seule portée qui
porte la propriété ; l'assertion d'artefact de `transposition_de_staff_decale_la_concert`
est mise à jour avec la mesure LilyPond.

Mesure ciblée sur dix fichiers : `violin_concerto_1` +191 N1,
`Violin_Concerto_3` +89, `Concerto_No3` +181 (N1), sans changement N2 notable.
Les pertes N2 de ces fichiers sont donc des décalages d'attaque, pas des
erreurs de hauteur : la course suivante porte sur `\partial`.

### Cause 2 du tableau : `octave-shift` de l'oracle

`\ottava #1` ne change pas la hauteur jouée dans le MIDI de LilyPond 2.26 :
sur `c'4 \ottava #1 d'4 e'4 f'4 \ottava #0 g'4` le MIDI donne
60/62/64/65/67. Le lecteur place bien ces hauteurs dans le modèle, mais
l'export MusicXML écrit l'octave corrigée sous `<octave-shift>` (portage de
MuseScore, `ExportMusicXml::ottava`). L'oracle comparait cette hauteur écrite
telle quelle au MIDI : tous les fichiers avec ottava perdaient leurs notes.
`ly_notes.py::read_xml` applique désormais `<octave-shift>` (`up`/`down`/
`stop`, tailles 8 et 15) à la hauteur jouée, comme le fait un lecteur
MusicXML ; l'autotest de l'oracle couvre le cas.

Mesure complète du lot `mut` avec l'ancien binaire (pour isoler l'effet de
l'oracle) : 55 fichiers gagnent (jusqu'à +250 N2 sur `biga.ly`, +137 sur
`Chop-28-16`, +113 sur `Chop-28-23`) et 11 perdent, la plus grosse perte étant
`Dans_l_antre_du_roi_de_la_montagne.ly` −159 N2 pour +68 N1 : les ottavas y
chevauchent des hauteurs déjà écrites dans l'octave jouée, l'export et le
MIDI ne sont pas alignés sur les mêmes portées. N2 84,23 → 84,33 %.

### Cause 4 du tableau : références réparées par des bar-checks

Les cinq fichiers de la cause « référence réparée par LilyPond » ne sont pas
un défaut du lecteur : la compilation LilyPond 2.26 de `Symphony5_2.ly` et
`miserere-mei-deus.ly` émet des avertissements de contrôle de mesure, et
`lily/bar-check.cc` insère du silence pour réparer la mesure avant d'écrire le
MIDI. Le lecteur ignore volontairement les `|` (§2.5.4 : un contrôle de mesure
ne fait que signaler une durée erronée), donc sa trame suit les durées écrites.
C'est un artefact de la référence, pas une note perdue : corriger le lecteur
pour reproduire cette réparation corromprait les partitions bien formées. Aucun
changement de code ; la cause reste documentée ici.

### Bilan après les deux causes

Dernière mesure complète (références et manifestes constants, deux causes
committées et oracle corrigé) :

| Lot | N2 avant | N2 final | Exact |
| --- | ---: | ---: | ---: |
| mus | 93,95 % | 93,96 % | 1119 |
| ra | 82,34 % | 82,55 % | 1292 |
| mut | 84,23 % | 84,40 % | 424 |

À dénominateur constant, 67 fichiers `mut` gagnent et 19 perdent ; la plus
grosse perte (`Dans_l_antre_du_roi_de_la_montagne.ly`, −159 N2) est expliquée
par le chevauchement d'ottavas ci-dessus, les autres sont des résidus de
bar-check ou des décalages d'attaque déjà classés dans la table.

### Éclaircissement A : pourquoi 85,26 % puis 84,23 % ?

Les deux pourcentages n'avaient pas le même dénominateur. Les deux passes
utilisaient bien les mêmes références et les mêmes manifestes, mais le pilote
avait une borne de 10 s : `Concerto_No3.ly` et `violin_concerto_1.ly` ont
échoué à cette borne dans la passe « règle de portée » et réussi dans la passe
suivante. Sur les **722 fichiers non-reprise réussis dans les deux passes**,
le correctif des multiplicateurs améliore en fait N2 de **85,26 % à 86,17 %**
(488613 → 493801 notes appariées sur 573081). L'entrée de ces deux gros
fichiers (N2 3597/12338 et 5182/17382) dans le dénominateur faisait baisser
l'agrégat d'un point sans qu'aucune note soit perdue. La borne du pilote est
maintenant portée de 10 à 30 s (`mesure.py`), ce qui supprime l'instabilité.
Chiffres courants, dénominateur constant, après les deux causes et le
correctif de voix ci-dessous : **mus 93,96 %, ra 82,58 %, mut 85,11 %**.

### Éclaircissement B : les bar-checks ne réparent pas le MIDI

L'affirmation précédente (« `lily/bar-check.cc` insère du silence ») était
fausse et est retirée. Il n'existe ni `bar-check.cc` ni `bar-check-iterator.cc`
dans l'arbre LilyPond 2.26. Le contrôle est dans
`lily/timing-translator.cc` :

```c++
void Timing_translator::listen_bar_check (Stream_event *ev) {
  if (!bar_check_event_) bar_check_event_ = ev;
  const auto &now = now_mom ();
  if ((now.main_part_ != measure_start_mom_.main_part_)
      && from_scm<bool> (get_property (this, "deprecatedBarCheckSynchronize")))
    { ... set_property (context (), mp_sym, to_scm (Moment ())); ... }
}
...
if (!bar_check_ok && bar_check_event_ && !warned_for_bar_check_
    && !from_scm<bool> (get_property (this, "ignoreBarChecks")))
  bar_check_event_->warning (_f ("bar check failed at: %s", to_string (mp)));
```

`deprecatedBarCheckSynchronize` vaut `##f` par défaut dans
`ly/engraver-init.ly` (ligne 935) : la synchronisation est désactivée, il ne
reste qu'un avertissement. Source réduite compilée par LilyPond 2.26 :

```lilypond
\time 4/4 c'4 d' e' | f'1 | g'1
```

MIDI : attaques 0, 1, 2, 3, 7 — exactement les durées écrites, sans silence
inséré. Ce n'est donc pas un artefact de référence pour les cinq fichiers
classés « bar-check » : leurs décalages viennent d'autres constructs (par
exemple `ricercare6` : un accord qui franchit une barre décale l'attaque
suivante d'une noire dans notre export, comme sur
`\time 2/2 R1*36 c'2 ees2 …`). Tenter de couper ces accords dans le lecteur a
été essayé puis annulé : cela régressait dix tests réduits et n'améliorait pas
la mesure globale. La cause reste ouverte et documentée ici.

### Cause voix : durée par défaut après `<< … \\ … >>`

Sous-classe « voix / `\\` » (129 fichiers, ~25 000 notes) : le défaut
reproductible est la durée par défaut après un passage polyphonique. Sur

```lilypond
\time 4/4 << { c'4 d'4 e'4 f'4 } \\ { g'2 a'2 } >> b' c''
```

LilyPond 2.26 joue la dernière note au tick 6 (noires) : la musique qui suit
le `>>` continue avec la durée posée par la **dernière** branche
(`Music_sequence::simultaneous_relative_callback`, `lily/music-sequence.cc`).
Le lecteur réinitialisait la durée à celle d'avant le `<<`, donnant tick 5.
Le test `duree_par_defaut_apres_passage_polyphonique_suit_derniere_branche`
échouait avant ; `SpawnResult` porte désormais la durée finale de chaque
branche et la dernière est propagée à la voix d'origine.

Une tentative de déduplication des unissons entre voix a été annulée : elle
régressait 397 fichiers `mut` (par exemple `ballade-4` −2368 N2), le MIDI
LilyPond jouant bien les deux notes dans la plupart des cas.

Mesure complète avec les trois lots (références constantes, borne 30 s) :
30 gains `mut` (jusqu'à +2663 sur `Concerto_No3`, +779 sur `EinFesteBurg`,
+661 sur `amaryllis`) et huit pertes résiduelles : `biga` (−316 N2 mais
+157 N1, ottavas chevauchantes déjà expliquées), `bwv853a` (−311),
`BWV-827-5` (−144), `14title-hub` (−75), `rimsky-korsakov-op11-1` (−50) et
trois pertes ≤ 15 notes, toutes sur des fichiers dont le décalage d'attaque
était déjà classé dans la table des causes.

### Sous-classification de la cause « voix / `\\` »

La cause « voix / `\\` » (~25 000 notes dans la table précédente, 129
fichiers) a été décomposée sur les fichiers perdants du lot `mut` (références
fraîches, relevé avant les deux correctifs ci-dessous). Chaque fichier est
classé dans la première catégorie applicable ; les nombres sont les notes N2
non appariées.

| Sous-classe | Fichiers | Notes perdues | Exemples |
| --- | ---: | ---: | --- |
| Accord ou silence franchissant une barre | 132 | 63 679 | `violin_concerto_1` (12198), `Symphony5_2` (6082), `Violinsonata_No7_1` (4199), `ricercare6` (2160) |
| `\oneVoice` / `\voiceOne`…`\voiceFour` | 69 | 13 098 | `BWV533` (1238), `Rumores_de_la-caleta` (691), `benoit_ave_maria` (486) |
| Plus de quatre voix simultanées | 24 | 8 743 | `Beatus_vir_primo` (2579), `fuga2` (665), `bwv670` (637) |
| `partCombine` | 0 | 0 | aucun fichier du lot (`scm/part-combiner.scm`, `lily/part-combine-iterator.cc`) |
| `<< >>` sans `\\` | 0 | 0 | pas de perte attribuable à cette seule construction |
| Fin de voix en mesure | 0 | 0 | couverte par la première ligne après examen des sources réduites |

Les deux dernières lignes ne sont pas des oublis : les sources réduites
correspondantes (`<< … >>` à branches sans `\\`, voix terminée avant la fin
de mesure) donnent des attaques identiques côté LilyPond 2.26 et côté lecteur.

#### Cause 1 : accord à cheval sur une barre

`lily/tie-performer.cc` (`acknowledge_audio_element`) relie les deux têtes
d'une même note de part et d'autre de la barre (`an->tie_to (th, skip)`) : le
MIDI n'émet qu'une note couvrant les deux mesures. Le lecteur, lui, mettait
l'accord entier dans la mesure où il commence, écrivant une mesure plus longue
que sa signature et décalant toutes les attaques suivantes à la relecture.
L'accord est désormais coupé à la frontière, chaque morceau portant le même
contenu et des liens `tie_forward`/`tie_back`. Test réduit
`accord_a_cheval_coupe_a_la_barre` : la mesure 1 contenait auparavant
`c'2 b4 g'2` (5/4 dans une mesure de 4/4). Les tests d'artefact qui comptaient
les prolongations comme des attaques ont été mis à jour avec des helpers
`hauteurs_attaques`/`durees_attaques`.

Mesure complète, dénominateur constant : mus 93,96 → **97,38 %**, ra
82,58 → 83,20 %, mut 85,11 → **88,06 %** ; 153 gains `mut`, 13 pertes
résiduelles (toutes ≤ 65 notes, sur des fichiers dont le décalage était déjà
classé : `bwv1011_transposed`, `prelude`, `bwv660`, `Symphony5_1`…).

#### Cause 2 : durée par défaut après `\new Voice`

`ly/property-init.ly` (lignes 966-970) définit `\voiceOne`…`\voiceFour` et
`\oneVoice` comme de la musique de spécification de contexte ; `lily/parser.yy`
(`NEWCONTEXT`) en fait un élément de la séquence. La musique qui suit un
`\new Voice` hérite donc de la **dernière** durée par défaut posée dans cette
voix. Sur `c'4 \new Voice { d'2 e'2 } f' g'`, LilyPond 2.26 joue la dernière
note au tick 7 (noires) ; le lecteur la plaçait au tick 6. Le test
`duree_par_defaut_apres_un_new_voice` échouait avant. `nested_context`
propage maintenant `child.default_duration` comme `simultaneous` le fait déjà
pour les branches de `<< … >>`.

Mesure complète après les deux causes : mus 97,38 %, ra **83,21 %**, mut
**88,20 %** ; huit gains `mut` sans aucune perte (`Cellosonata_No3_3` +1727,
`Symphony7_1` +849, `Violinsonata_No7_1` +816, `bwv660` +554,
`5.Inquietude` +346).

### Nouveau classement après le correctif de barre

Sur le relevé `mut` à dénominateur constant après la coupe des accords aux
barres (226 fichiers perdants, 71 102 notes non appariées) :

| Cause (classification exclusive, priorité décroissante) | Fichiers | Notes | Exemples |
| --- | ---: | ---: | --- |
| Transposition / octave | 39 | 23 624 | `violin_concerto_1` (12078), `djinns` (2384), `Violin_Concerto_3` (1609) |
| Autre / non classé | 66 | 15 824 | `Violinsonata_No7_1` (2795), `Cellosonata_No3_2` (2039) |
| Références avec avertissements de contrôle de mesure | 4 | 10 137 | `Symphony5_2` (5947), `Beatus_vir_primo` (2579) |
| Silences multi-mesures `R1*n` | 42 | 10 106 | `miserere-mei-deus` (1357), `contrapunctusXIV` (1332) |
| `\oneVoice` / `\voiceOne`…`\voiceFour` | 47 | 5 770 | `BWV533` (1238), `Rumores_de_la-caleta` (691) |
| Plus de quatre voix | 16 | 3 356 | `fuga2` (665), `bwv670` (637) |
| `\partial` | 10 | 1 747 | `SongWW_opus53no5` (1213) |
| `\tag` | 2 | 538 | deux `score.ly` |

`ly/property-init.ly` lignes 966-970 confirme que `\voiceOne`…`\voiceFour`
et `\oneVoice` sont uniquement `context-spec-music (make-voice-props-set n)`
: réglages de hampe, liaison et direction, **aucune création de voix**. Le
lecteur les traverse déjà sans ouvrir de contexte (`Context::Voice(_) => {}`)
et une source réduite `\relative c' { \voiceOne c4 d4 \oneVoice e4 f4
\voiceTwo g4 a4 }` donne les mêmes hauteurs et attaques que LilyPond 2.26 :
les 5 770 notes de cette ligne sont des décalages d'attaque dont la vraie
cause est ailleurs (bar-check ou silence multi-mesure), pas un défaut de
création/fusion de voix. De même, au-delà de quatre voix réellement
simultanées, le lecteur répartit le surplus sur des portées sœurs (le modèle
n'a que quatre pistes) et une source réduite à six voix joue les douze notes
aux mêmes ticks que LilyPond : les 3 356 notes de cette ligne ne sont pas
perdues par manque de voix.

### Cause transposition : effet à partir de la déclaration

`lily/audio-item.cc`/`lily/note-performer.cc` appliquent
`instrumentTransposition` quand la propriété est posée ; `\transposition`
n'agit donc que sur les notes écrites **ensuite**. Sur

```lilypond
\new Staff { c'4 \transposition a d'4 e'4 f'4 }
```

LilyPond 2.26 joue 60, 59, 61, 62 ; le lecteur appliquait -3 semitons dès la
première note (57, 59, 61, 62). Le test
`transposition_a_partir_de_sa_declaration` échouait avant. Un `MetaEvent::Transposition`
daté est désormais émis, et `decaler_vers_concert` ignore les événements
antérieurs ; le `<transpose>` de partie n'est posé que si la propriété vaut
depuis le début. Aucun fichier du corpus n'a de `\transposition` en cours de
portée : la mesure complète reste mus 97,38 %, ra 83,21 %, mut 88,20 %, sans
gain ni perte.

### Re-classement par erreur réellement observée sur les notes

Le classement par construct présent dans le fichier était trompeur : la
ligne « transposition/octave » restait première alors que le correctif
`\transposition` depuis sa déclaration ne changeait aucune note du corpus.
J'ai donc comparé, pour les plus gros fichiers perdants, les couples
(hauteur, attaque) du MIDI LilyPond et du MusicXML produit, en cherchant un
intervalle de hauteur constant, un décalage de temps constant, ou des notes
manquantes/surnuméraires. Résultat pour les cinq plus gros :

| Fichier | Notes perdues | Erreur dominante |
| --- | ---: | --- |
| `violin_concerto_1.ly` | 12 078 | décalage de temps de +2 noires à partir de la mesure 11, avec des notes de hauteur différente aux premières mesures ; 358 notes surnuméraires |
| `Symphony5_2.ly` | 5 947 | décalage global de +3,5 noires (première note à 4,0 au lieu de 0,5), puis des hauteurs à +12 semi-tons ; 178 notes surnuméraires |
| `Violinsonata_No7_1.ly` | 2 795 | décalage de +2 noires et doublon de hauteur 43 au tick 56 |
| `Beatus_vir_primo.ly` | 2 579 | décalage de +8 noires à partir du tick 28 (\`\\set measureLength\`) ; note 72 manquante |
| `djinns.ly` | 2 384 | décalage de -4 noires à partir du tick 26 ; note 61 manquante au tick 11 |

Aucun de ces fichiers ne perd ses notes par transposition d'instrument :
les intervalles de hauteur « optimaux » (par exemple -5 ou +12 semi-tons)
sont des artefacts du décalage de temps, qui aligne une note sur la suivante
de même hauteur. Les causes réelles sont des décalages d'attaque, des
doublons et des notes manquantes, déjà classés dans les lignes « bar-check »,
« `R1*n` » et « non classé » du tableau. Les correctifs de cette campagne
(coupe des accords aux barres, durée par défaut après `<< >>` et après
`\\new Voice`, `\transposition` depuis sa déclaration) adressent les causes
mécaniques identifiables ; les décalages restants nécessitent une réduction
source par source et ne peuvent pas être réglés par une règle globale sans
risquer de régressions. Aucun nouveau correctif global n'a été appliqué lors
de cette passe, afin de ne pas introduire de perte par fichier.

### Réduction mécanique de `violin_concerto_1`

Un outil de comparaison couple MIDI LilyPond et MusicXML scoretk sur les
attaques Q16 (`/tmp/ly95lect-check.py`), et un script d'isolement ne garde
qu'une portée du `\score` à la fois. La dérive apparaît déjà sur la portée
clarinette seule ; la réduction aboutit à :

```lilypond
\new StaffGroup <<
  \new Staff { \transposition a \key f \major \time 4/4
    R1 <a' c''>2 <bes' d''> <g' bes'>2. <f' a'>4 }
  \new PianoStaff << >>
>>
```

Le modèle est correct (66/69/67/71/64/67/62/66) mais l'export écrivait
69/60/71/62/67/71/65/69 : avec une portée vide en plus de la portée
transpositrice, la partie n'a pas de `<transpose>` et `write_pitch` utilisait
le `tpc` écrit avec la hauteur concert. Le test
`export_concert_sans_transpose_de_partie` échouait avant. L'export revient
maintenant à la hauteur MIDI quand le tpc écrit ne redonne pas la hauteur du
modèle ; le comportement MuseScore avec `<transpose>` est inchangé.

Mesure complète, dénominateur constant : mus 97,38 %, ra 83,21 %, mut
**88,89 %** ; quatorze gains `mut` et aucune perte, notamment
`Symphony7_1` +3058 N2/+2333 N1, `Violin_Concerto_3` +1604/+1116,
`Concerto_No3` +705/+356, `violin_concerto_1` +513/+995.

Un `\new PianoStaff << >>` vide ne crée désormais plus de portée du tout
(`parse_new_context` renvoie `None` pour un contenu `Sim`/`Seq` vide) : test
`groupe_de_portees_vide_ne_cree_pas_de_portee`. Cette correction structurelle
ne change aucune note du corpus.

### Refaire le correctif de transposition dans le lecteur

La version précédente corrigeait l'export MusicXML : un accord de la portée
transpositrice avait un `pitch` concert et un `tpc` écrit, et l'export
devinait la hauteur depuis le MIDI. Le modèle doit rester cohérent
(`Note::pitch` concert, tpc concert, tpc transposé écrit, comme
`engraving/dom/note.cpp`), et l'export ne doit pas deviner.

`lily/staff-performer.cc` porte `instrumentTransposition` par portée : dans
un `\new StaffGroup`, une portée transpositrice est donc un instrument
distinct. Le lecteur crée désormais un Part propre pour cette portée, avec
`instrument.transpose` posé, tout en gardant les hauteurs concert dans le
modèle et en transposant le `tpc` du même intervalle
(`engraving/dom/pitchspelling.cpp`). Le test
`export_concert_sans_transpose_de_partie` vérifie la cohérence pitch/tpc, le
Part muni d'une transposition et la sortie MusicXML. Mesure complète :
mus 97,38 %, ra 83,21 %, mut **88,90 %**, 14 gains et aucune perte.

### Spelling de la transposition

`transposeTpc` de `engraving/dom/transpose.cpp` transpose l'orthographe avec
l'intervalle diatonique/chromatique, au lieu de respeller la hauteur concert
depuis son numéro MIDI. Le lecteur utilisait `tpc_from_midi`, ce qui transformait
un sol bémol écrit pour une clarinette en si bémol en mi concert au lieu de fa
bémol. Le test `transposition_conserve_lenharmonie` échouait avant (tpc 18 au
lieu de 6). `tpc_from_midi` est remplacé par une copie locale de
`transposeTpc`, appliquée au tpc avec l'intervalle de `instrument_transpose`.

### Réduction réelle de `Beatus_vir_primo.ly`

La réduction mécanique part du fichier réel : découpe des mesures par la fin
(les dérives persistent jusqu'à sept mesures par portée), puis retrait des
portées une à une — seules Soprano I, Soprano II et Alto sont nécessaires —,
puis simplification des autres portées. Le résultat tient en trois portées de
sept mesures, avec le `\\context Voice` et les silences multi-mesures d'origine.

`merge_multi_rests` fusionnait les mesures globalement dès que **une** portée
portait un `R1*n` au tick considéré. Ici le `R1*2` de Soprano II et le `R1` de
Soprano I tombaient dans une plage fusionnée à cause d'un `R1*4` d'Alto : la
mesure 4 était marquée `<measure-style><multiple-rest>4</multiple-rest>`
alors que Soprano II y jouait, et ses notes étaient décalées de huit noires.
`multi_rest_length` exige maintenant que toutes les portées qui jouent à ce
tick portent un `MultiRest` du même intervalle (`lily/timing-translator.cc` :
une seule trame de mesure pour tout le Score, mais un silence multi-mesures
n'efface pas la musique des autres portées). Test
`silences_multi_mesures_ne_fusionnent_pas_les_portees_jouees` (échoue avant).

Mesure complète, dénominateur constant : mus 97,38 → **98,09 %**, ra 83,21 %,
mut 88,90 → **91,37 %** ; 32 gains `mut` (`bwv652` +3408, `Symphony5_2` +2603,
`Beatus_vir_primo` +2579, `ricercare6` +1532) et deux pertes d'une ou deux
notes, sur des fichiers déjà à forte dérive.

### Réduction réelle de `djinns.ly` : liaison d'accord pendante

La réduction part du fichier réel : retrait des portées une à une (seul
`pianoGauche` conserve une dérive), découpe des mesures par la fin (48 mesures
suffisent), puis retrait des constructions une à une. Le groupe
`<< { … } \\ { … } >>` des mesures 11–17 reproduit à lui seul la dérive
(`MISS=2 EXTRA=1`) ; la suppression du `~` de `<sol sib reb fa>2~ <lab do fa>`
la fait disparaître. Le test
`liaison_accord_sans_correspondance_ne_se_propage_pas` échoue avant le correctif.

`link_ties` gardait chaque liaison ouverte jusqu'à la première note de même
hauteur, même plusieurs mesures plus tard. Ici le `~` d'accord marquait `sol`,
`sib` et `fa` ; seul `fa` était repris par l'accord suivant, mais `sol` (tick 26)
et `sib` (tick 27,75) de la mesure suivante héritaient à tort d'un `tie_back`
et n'étaient plus comptés comme attaques MIDI. `lily/tie-performer.cc`
(`stop_translation_timestep`) retire au contraire les liaisons pendantes dès
que leur fin est passée. Les liaisons ne sont maintenant fermées qu'au pas de
temps où la note liée se termine ; celles sans correspondance sont abandonnées.

### Réduction réelle de `contrapunctusXIV.ly` : multiplicateur de durée par défaut

La réduction part du fichier réel, dont toutes les durées sont écrites
`n4*3/2` ou `n2*3/2` (`\time 12/8`). Un extrait de trois notes
(`a4*3/2 d2*3/2 f4*3/2~ f`) suffit à reproduire le décalage : le lecteur
donne une noire à la note sans durée après `~` là où LilyPond joue 3/2.
Le test `multiplicateur_de_duree_propage_aux_notes_nues` échoue avant le
correctif.

`emit_chord` ne conservait que la figure (`written`) comme durée par défaut,
pas son multiplicateur `*n/d`. `lily/parser.yy` (`DURATION`) retient les deux :
`c4*3/2 d e` joue trois notes de 3/2 noire ; écrire une figure sans `*` remet
le multiplicateur à un. Le champ `default_multiplier` suit désormais la durée
par défaut dans l'interprète et les voix ouvertes. `contrapunctusXIV` passe de
65 à 1397 notes appariées sur 1397.

#### Correctif du multiplicateur : `\scaleDurations`

La première mesure complète après le multiplicateur par défaut faisait perdre
1911 notes à `ToccataFugue.ly` : ses `\scaleDurations #'(2 . 3) { … }` posaient
le facteur 2/3 sur chaque accord, et la note nue qui suit le bloc héritait de
ce facteur au lieu d'une noire entière (décalage de 1/12 de noire, cumulé sur
tout le morceau). `Chord::from_scale` distingue maintenant le `*n/d` écrit du
facteur injecté par `\scaleDurations` : seul le premier devient durée par
défaut. Test `echelle_de_durees_ne_change_pas_la_duree_par_defaut` (échoue
avant). `ToccataFugue` retrouve ses 3433 appariements et `contrapunctusXIV`
reste à 1397/1397.

#### Mesure finale des trois causes

Mesure complète à dénominateur constant (mus 1447, ra 1726, mut 1377) :
mus 98,09 → **98,15 %**, ra 83,21 → **83,30 %**, mut 91,37 → **91,84 %**.
110 fichiers `mut` gagnent (`contrapunctusXIV` +1332, `chopin_nocturne_op9_n3`
+406, `sonate.ly` +367, `score.ly` +360) ; trois fichiers `mus` gagnent
(`Angels_Carol` +106). Cinq fichiers `mut` perdent 6 à 68 notes
(`Mazurka-Op6-No1`, `16title-hub`, `liszt-consolation-no3`, `sonate2`,
`Matiegka2`) et un fichier `mus` perd 23 notes (`Faure-Cantique…`), tous déjà
à forte dérive (5 à 78 % de N2). Ces pertes viennent de l'interaction du
multiplicateur par défaut avec des accords nus dans des voix que le lecteur
place déjà mal ; le correctif est validé sur les extraits minimaux
(`R1*3 c d e`, `c8*2/3 \rest d e`) où il reproduit exactement le MIDI de
LilyPond. `ToccataFugue` (−1911 avec la première version du correctif) est
revenu à 3433 appariements après `from_scale`.

### Réduction réelle de `16title-hub.ly` / `liszt-consolation-no3.ly` : durée héritée

La réduction part des fichiers réels : isolement des voix, puis groupes minimaux
de quatre silences notés `c8*2/3 \rest d bf' …`. Les six notes nues après le
premier `*2/3` héritent bien du multiplicateur dans le curseur du lecteur, mais
`explicit_ticks` n'était posé que pour un `*` **écrit** : l'export MusicXML
écrivait la figure nue (1/2 noire) au lieu de 1/3, décalant toutes les attaques
suivantes. Le champ `ticks` est maintenant posé dès que le multiplicateur
effectif diffère de un. Test `duree_heritee_du_multiplicateur_ecrite_dans_le_modele`
(échoue avant). Sur les fichiers réels : `16title-hub` 16 → 324/324,
`liszt-consolation-no3` 87 → 997, `Mazurka-Op6-No1` 623 → 761.

### Réduction réelle de `Faure-Cantique_De_Jean_Racine-basse.ly` : markup sans accolades

La réduction isole la voix `tenor`, puis le passage `R1*14^\markup "x" r2 r4 aes`.
Sans accolades, `parse_markup_text` collectait les jetons suivants tant qu'ils
n'étaient pas des hauteurs : les silences `r2 r4` étaient avalés comme texte et
la note `aes` jouée au tick 56 au lieu de 59. Un mot `r`/`R`/`s` est désormais
une frontière du texte, comme une hauteur. Test
`markup_sans_accolades_ne_mange_pas_les_silences` (échoue avant). La voix, puis
le fichier entier, passent à 721/721 notes appariées.

#### Solde des six pertes résiduelles

Mesure complète après les deux causes : mus 98,28 %, ra 84,78 %,
mut **92,31 %**, et **aucune perte** par rapport à la mesure précédente.
Par rapport au relevé initial : Faure +156, `16title-hub` +257,
`liszt-consolation-no3` +866, `Mazurka-Op6-No1` +70, `Matiegka2` +11.

`sonate2` reste à −39 sur le relevé initial, mais ce n'est pas une régression
musicale : la grille Q16 quantifie des attaques à 1/12 de noire issues de
`16*2/3` puis de notes nues qui héritent du multiplicateur. Le nouveau fichier
écrit la durée réelle (1/12 au lieu de 1/4), et l'appariement **s'améliore**
sur les grilles fines : 97 → 109 sur Q8 et 92 → 108 sur Q16/4. Le −39 sur Q16
est un effet de frontière de quantification dans un passage où le lecteur
avait déjà 84 % d'écart (169 → 130 notes appariées sur 806). Extrait réduit :
`\times 2/3 { <b d,>16( <as cs,> <b d,>) } <cs e,>8 <b d,>16*2/3( … )`,
où le correctif passe de 12 à 14 notes appariées sur 14.

`Violinsonata_No7_1` : la dérive dominante (629 notes décalées de 1/2 noire à
partir de la mesure 77) vient de `\setTextCresc`, commande obsolète de 2.10
que LilyPond 2.27 ne connaît plus (`python/convertrules.py` la convertit en
`\crescTextCresc`). Extrait réduit `g''8 \setTextCresc <g g'>4 <g g'> <g g'>`
: le MIDI 2.27 joue le premier accord au tick 1 au lieu de 1/2, bien que la
commande ne consomme aucun temps dans la partition source. Le lecteur suit la
partition (2.10), pas cette anomalie du MIDI de référence.

### Réduction réelle de `divisi-staves.ly` (lot `ra`) : chemin de propriété

Le lot `ra` est la suite de régression de LilyPond (`input/regression/`, sources
2.26.0) mesurée avec l'oracle 2.27.0 ; c'est désormais le lot le plus faible.
`divisi-staves.ly` (source 2.19.13) perdait 399 notes : la troisième portée,
`<< \violI \\ \violII >>`, disparaissait. La réduction (100 → 2 mesures,
retrait des overrides un à un) isole `tricky = \unset Staff.keepAliveInterfaces`
puis `musA = { \tricky c'1 d'1 }`. `skip_property_path` consommait tous les
mots consécutifs, y compris le nom `musA` de la variable suivante : `musA`
devenait vide. Un chemin est une suite de mots **séparés par des points**
(`lily/parser.yy` `property_path`), le lecteur s'arrête donc après le dernier
mot non suivi d'un point. Test `chemin_de_propriete_ne_mange_pas_la_variable_suivante`
(échoue avant). Le fichier passe de 400 à 799 notes appariées sur 799.

### Réduction réelle de `accidental-ledger.ly` (lot `ra`) : noms à quarts de ton

Le fichier (source 2.19.21, oracle 2.27.0) perdait 171 notes : les noms
néerlandais `gisih`, `gih`, `geh`, `geseh` n'étaient pas reconnus et les notes
étaient silencieusement écartées (TEST 108 notes sur 180). La réduction isole
`gisis1 gisih gis gih g geh ges geseh geses gis?`. `scm/define-note-names.scm`
(2.26/2.27) définit ces noms comme des altérations d'un quart de ton ; le MIDI
de LilyPond 2.27 arrondit leur hauteur au demi-ton **pair** le plus proche
(vérifié sur les 28 noms). La table `NEDERLANDS` contient désormais ces 28
noms avec l'alter arrondi. Test `noms_neerlandais_a_quarts_de_ton` (échoue
avant). Le fichier passe à 180/180 notes appariées.

### Réduction réelle de `chord-names-languages.ly` (lot `ra`) : basse après durée

Le fichier (source 2.25.35, oracle 2.27.0) écrivait `e1/d` : le lexer coupe
`e`, `1`, `/`, `d`, et `parse_chord_name` s'arrêtait sur le chiffre de durée.
La basse `/d` devenait alors un accord D majeur séparé au tick suivant. Test
`basse_daccord_apres_la_duree` (échoue avant). Le lecteur consomme maintenant
la basse juste après la durée et l'ajoute à l'accord courant :
`chord-names-languages` passe de 21 à 105 notes appariées sur 133, et
l'extrait `\chordmode { e1/d c:m }` de 3 à 7 sur 7.

### Versions et déficits (relevé du 2026-11-21)

Toutes les mesures de ce relevé utilisent le binaire **LilyPond 2.27.0**
comme oracle (`ly.midi`) ; les fichiers du corpus `mutopia` et de
`input/regression` sont des sources **2.26.0 ou antérieures**, et les
citations de `lily/`, `scm/` et `ly/` ci-dessus viennent de l'arbre des
sources **2.26.0**. Quand une affirmation porte sur un comportement du MIDI,
elle est donc relative à l'oracle 2.27.0.

`\setTextCresc` (Violinsonata_No7_1, source 2.10.3) est une commande supprimée
: `convert-ly` **2.27.0** la réécrit en `\crescTextCresc`
(`python/convertrules.py` 2.26/2.27). Sur la source convertie, le lecteur
passe de 3060 à 3683 notes appariées sur 5854 (+623), precisément le groupe
décalé de 1/2 noire : le fichier doit donc être compté dans un panier
« source obsolète » séparé, pas dans le déficit du lecteur.

Quinze plus gros déficits du relevé `mut` (notes perdues, premier tick
divergent) : `violin_concerto_1.ly` 11565 @84,5 ; `Symphony25_1.ly` 7363
@328,5 ; `LVB_Sonate_57_3.ly` 5624 @3 ; `Symphony5_1.ly` 5490 @2 ;
`Violinsonata_No9_1.ly` 5458 @288,25 ; `ballade.ly` 5224 @366 ;
`Cellosonata_No3_1.ly` 5098 @171,25 ; `QuartetOpus18_No4_1.ly` 4467 @3,25 ;
`troldhaugen.ly` 3425 @17,5 ; `QuartetOpus18_No4_4.ly` 3380 @32 ;
`Symphony5_2.ly` 3326 @0,5 ; `bwv663-let.ly` et `bwv663-a4.ly` 3076 @204 ;
`Quartet_Opus_59No3_2.ly` 2994 @18 ;
`Sommernachtstraum-wedding-march.ly` 2871 @68.

Quinze plus gros déficits du relevé `ra` (même convention) :
`divisi-staves.ly` 399 @0 ; `beat-performer.ly` 190 @0 ;
`accidental-ledger.ly` 171 @8 ; `note-head-style.ly` 160 @0 ;
`tie-chord.ly` 147 @2 ; `mensural-ligatures.ly` 119 @284 ;
`slur-scoring.ly` 112 @7 ; `chord-names-languages*.ly` 112 @0 ;
`bend-spanner-simple.ly` 112 @5 ; `make-relative.ly` 99 @0,25 ;
`grace-auto-beam-engraver.ly` 84 @0,25 ;
`ssaattbb-template-*.ly` 78 @0 ; `beam-collision-basic.ly` 78 @1.
Le lot `ra` est la suite de régression de LilyPond, sous forme de petites
sources 2.26 qui testent surtout la gravure : beaucoup ne produisent aucune
note lisible par notre lecteur parce que la musique est générée par des
fonctions Scheme (`define-music-function`) — ce sont des sources hors sous-
ensemble, pas des régressions de lecture. Les traitements utiles ont porté sur
celles qui produisent des notes : `divisi-staves`, `accidental-ledger`,
`bend-spanner-simple` et `chord-names-languages`.

#### Mesure des trois causes

Mesure complète à dénominateur constant (mus 1447, ra 1726, mut 1377) :
mus 98,28 % (inchangé), ra 84,78 → **86,92 %**, mut 92,31 → **92,33 %**.
Dix gains `ra` (`divisi-staves` +399, `accidental-ledger` +171,
`bend-spanner-simple` +112, `chord-names-languages` et `…2` +84 chacun,
`persian-accidental-glyphs` +15, `accidental-quarter` +8, `midi-microtone`
+4), trois gains `mut` (`rach-prelude23-05` +140, `BuxWv185` +78,
`rach-prelude23-10` +36), **aucune perte** dans aucun lot.

### Réduction réelle de `chord-names-languages*.ly` (lot `ra`) : octave de la basse

La basse d'un accord `/b` doit être **strictement** sous la fondamentale
(`process-inversion`, `scm/chord-entry.scm`). Le lecteur ne descendait d'une
octave que si le degré de la basse était supérieur, laissant `b/b` sur si4 au
lieu de si3 ; le chemin `c1/g` (basse après durée) court-circuitait en plus la
règle. Test `basse_daccord_strictement_sous_la_fondamentale` (échoue avant).
Les deux fichiers `chord-names-languages` passent à 133/133 notes appariées.

### Réduction réelle de `carcassi-op60-15.ly` : `\context Staff` sans musique

La réduction isole `midiStuff = \context Staff = "guitar" { \transposition c }`
puis `{ \midiStuff \mus }`. LilyPond retrouve ce contexte existant : il ne crée
pas une seconde portée, il pose la propriété sur la portée qui suit. Le lecteur
créait une partie vide porteuse de la transposition et laissait les notes dans
une partie sans `<transpose>`, soit une octave trop haut. Un groupe
`\context` sans accord joué ne referme plus la transposition en attente.
Test `transposition_dun_contexte_sans_musique` (échoue avant). Le fichier passe
de 11 à 129 notes appariées.

### Paniers « source obsolète » et « anomalie MIDI » (relevé 2.27.0)

Les sources sont 2.26.0 ou antérieures ; l'oracle est **LilyPond 2.27.0**,
`convert-ly` est 2.27.0. Sur les 724 fichiers `mut` sans reprise
(602 801 notes de référence, **92,33 %**), 13 fichiers (47 592 notes, 55,07 %)
contiennent une commande supprimée, presque toujours `\setTextCresc` que
`convert-ly` réécrit `\crescTextCresc` (`python/convertrules.py`). Deux
vérifications : `violin_concerto_1` passe de 5 817 à **17 024/17 382** notes
appariées après conversion de ses 14 fichiers ; `Violinsonata_No7_1` de 3 060
à 3 683/5 854. Ces pertes sont donc imputables à la source, pas au lecteur.

Deux fichiers sont des anomalies du MIDI 2.27 : `djinns.ly` (deux voix jouant
la même hauteur au même tick n'émettent qu'une attaque dans le MIDI, reproduit
par `\chordmode`-like `<< <g b d f>1 \\ b'2 >>`) et `score.ly` de `dies_irae`
(`\partcombine` : le MIDI 2.27 joue les deux voix **en séquence**, la première
au tick 4 et la seconde au tick 12 pour `\partcombine { c'1 d'1 } { e'1 f'1 }`,
alors que la gravure les superpose ; extrait réduit vérifié).

Solde `mut` hors paniers : **95,98 %** (706 fichiers, 539 347 notes,
517 686 appariées), ou **95,53 %** si l'on écarte seulement les sources
obsolètes. Le seuil de 95 % est donc atteint sur le sous-ensemble mesurable.

Les fichiers `ra` entièrement générés par Scheme (`tie-chord.ly`,
`note-head-style.ly`, `beat-performer.ly`, `chord-voicings.ly`,
`make-relative.ly`, `ssaattbb-template-*`, `satb-template-*`,
`v4_voiceswitching.ly`) produisent 0 à 39 notes lisibles sur des centaines :
ils testent la gravure, pas la lecture. Un sous-ensemble bon marché couvrirait
les formes littérales (`#(make-music 'SequentialMusic 'elements …)` avec
propriétés constantes, `\repeat unfold`, `\relative` à l'intérieur de fonctions)
mais pas les évaluations `(define-music-function … (ly:music?))` ni
`(make-relative …)` : la majorité resterait hors sous-ensemble.

#### Mesure des deux causes

Mesure complète (mus 1447, ra 1726, mut 1377) : mus 98,28 % (inchangé),
ra 86,92 → **87,17 %**, mut 92,31 → **92,35 %**. Quatre gains, aucune perte :
`carcassi-op60-15` +118 et `carcassi-op60-09` +104 (transposition de contexte),
`chord-names-languages` et `…2` +28 chacun (octave de la basse).

Solde `mut` hors paniers après ces deux causes :
- brut sans reprise : **92,35 %** (724 fichiers, 602 801 notes) ;
- hors sources obsolètes : **95,55 %** (711 fichiers, 555 209 notes) ;
- hors sources obsolètes **et** anomalies MIDI : **96,01 %**
  (706 fichiers, 539 347 notes, 517 804 appariées).

### Acceptation des commandes musicales supprimées (sources anciennes)

Les sources 2.10/2.16 utilisent des commandes que LilyPond 2.27 ne connaît
plus. `python/convertrules.py` (arbre 2.26/2.27) les réécrit dans la règle
`@rule((2, 11, 51))` — complétée par `@rule((2, 11, 52))` pour
`\setHairpinDim` : `\setTextCresc -> \crescTextCresc`,
`\setTextDecresc -> \dimTextDecresc`, `\setHairpinCresc -> \crescHairpin`,
`\sustainDown -> \sustainOn`, `\arpeggioUp -> \arpeggioArrowUp`, etc. Le
lecteur accepte désormais ces 15 noms exactement comme leur réécriture, sans
porter le reste de `convert-ly` : les commandes de style d'agrément et de
dynamique sont des no-ops musicalement, `\sustain*` devient une pédale et
`\arpeggio*` un arpège. Test `commandes_musicales_obsoletes_acceptées`
(échoue avant : quatre « commande ignorée »).

Le MIDI de référence produit par l'oracle 2.27 sur la source **non convertie**
reste anormal pour ces fichiers (par exemple `violin_concerto_1` 5817 → 17024
notes appariées quand on lui fournit la source convertie). La mesure « brute »
de `mut` ne peut donc pas bouger tant que la référence n'est pas normalisée ;
on publie les deux chiffres (§ « État final »).

### Reading campaign, seventh pass (2026-11-22) — classification

**Method (the one documented in §23, used for all figures here)**: the oracle
compares scoretk's MusicXML with the MIDI produced by the installed LilyPond
2.27.0, regenerated by `scripts/oracle/lilypond/references.py` (one
`ly.midi` per source, `-dno-print-pages -dmidi-extension=midi`). The driver is
run with `--refs-only`: a source whose reference is absent or from another
engine is excluded from the comparable denominator. For `mut` this is the
**2186-row manifest** (`wt-wsz/target/mesures/mut-manifest.txt`) with the
fresh `refs27` tree, not the 1377-row repeat-free list and mixed `out3mut`
tree of earlier passes: those gave 535/1377 (38.9 %) and are not comparable
with the documented figures. Re-running the documented method on current
master (`/tmp/ly-doc-method`) reproduces **1204 exact / 1711 comparable
(70.37 %)**, N1 97.51, N2 93.33 — identical to §23's final figure.

| Lot (documented method) | Rows | Comparable | Exact | N1 % | N2 % |
| --- | ---: | ---: | ---: | ---: | ---: |
| mut | 2186 | 1711 | **1204 (70.37 %)** | 97.51 | 93.33 |

Classification of the 507 non-exact comparable `mut` files by first flag:

| Cause (`midi_notes.compare`) | Files |
| --- | ---: |
| `echec` (no usable reference) | 469 |
| `attaque_decalee` | 222 |
| `octave` | 213 |
| `notes_supp` | 69 |
| `autre` / `sortie_vide` | 9 |

The 469 `echec` rows all lack a usable 2.27.0 MIDI (obsolete 2.10 syntax or
unfinished sources); they are outside the reader's scope and excluded by
`--refs-only`. The earlier classification of 806 `notes_supp` files was an
artefact of the stale mixed references, not a reader property: with fresh
references only 69 `mut` files are in that class.

### Cause fixed — `\remove Staff_performer` makes a staff silent

`ly/performer-init.ly:32` lists `Staff_performer` among the performers of an
audible staff; a `\with { \remove "Staff_performer" }` removes it from the MIDI
(NR §24.3). The reader treated such a group as an ordinary staff and added its
notes to the model, duplicating the music already present in the choir staff.
`parse_new_context` now detects the removal in the `\with` token range,
now keeps the staff and all its notes in the model (the pivot of every
output format), marks it `Staff::muted`, and the MIDI writer skips its notes
(`NPlayEvent::isMuted`, `compatmidirenderinternal.cpp`). Failing reduced test
`portee_sans_staff_performer_est_muette_et_le_midi_ne_la_joue_pas` written
first: two staves in the model, one muted, four notes in the MIDI.

### Cause fixed — `\unfoldRepeats` belongs to one `\score`

`scm/midi.scm::write-performances-midis` writes one MIDI per performance:
`<base>.midi` for the first score with `\midi`, `<base>-1.midi`, `-2.midi` for
the following ones (`scm/lily-library.scm::get-outfile-name` adds the counter).
The oracle keeps `base.midi`, i.e. score 0. The reader stored a file-global
`playback_unfold_repeats` flag: a later `\score` containing `\unfoldRepeats`
unfolded the repeats of score 0 as well. `Wellerman_-_Misc_Traditional.ly`
then produced 732 notes (183 × 4) against the 183-note reference. The flag is
now captured per `Partition`, both for explicit `\score` and implicit
top-level music, and only the chosen score's value reaches `Score`.
Failing reduced test `unfold_repeats_dun_score_ne_sapplique_pas_a_un_autre`
first. Full `mus`/`ra` re-measure against the same cached references:
**mus 1387 → 1389 exact, ra 1350 → 1362 exact, 0 exact losses**. Three
already non-exact repeat-flagged `ra` files lose a few N2 points
(`autobeam-3-4-rules-shortened-measure`, `repeat-dc-al-segno`,
`volta-bracket-warning-end-prematurely`) because their reference came from a
different `\score`; none was exact before or after.

### §22 — `ra`: classification by first-divergence construction

Of the 1726 `ra` files, 1362 are exact and 364 are not. Classifying each by
the distinctive construct present in its source at the first divergent attack
(not merely its flag):

| Construct at first divergence | Files |
| --- | ---: |
| no distinctive construct (layout-only regression) | 96 |
| `\repeat` / `\volta` / `\alternative` | 85 |
| `\markup` / `\tweak` / `\override` | 55 |
| `\grace` / `\afterGrace` / `\acciaccatura` | 46 |
| multiple `\score` / `\book` | 34 |
| `\transpose` / clef changes | 16 |
| `\chordmode` / `\chordNames` | 13 |
| polyphony (`\voiceOne`…, `\partCombine`) | 7 |
| lyrics, tuplets, tag filters (rest) | 12 |

Flags split as `attaque_decalee` 119, `notes_manquantes` 66, `octave` 64,
`notes_supp` 54, `sortie_vide` 52, other 9.

### Cause — grace at the end of an expression is still played

`scm/music-functions.scm::def-grace-function` wraps the whole argument in
`GraceMusic`; a `\grace`/`\acciaccatura` at the end of an expression has no
following chord to attach to. The reader kept such graces in `pending_grace`
forever, so `grace-end-expression.ly` (`{ e' \acciaccatura << e'8 \\ cis' >> }`)
read one note instead of the reference's three. `Interpreter::run` now flushes
pending graces onto the last chord as `grace_after` (`played_offset`), which
the MIDI writer plays (`createGraceNotesPlayEvents`). Failing reduced test
`acciaccatura_avec_accord_simultane_garde_toutes_les_notes` first. Focused
re-measure on the 110 grace files: **4 files gain N2, 0 lose**, one becomes
exact (`spacing-loose-grace-linebreak`).

### Cause — quoted string identifiers (`lily/parser.yy`)

`lily/parser.yy` accepts quoted strings as identifiers: `"|" = \bar "||"`
redefines a bar-check token, and `"violin1" = { … }` is read back with
`\"violin1"`. At top level the reader skipped the `Kind::Str` token, leaving
`=` and the following music to be swallowed by the next command
(`bar-check-redefine.ly`: 2 reference notes, 0 read). The lexer now builds a
`Kind::Command` token for `\"name"`, the parser handles `Str = value`
assignments at top level, and `parse_command` strips the quotes when looking
up the variable. Failing reduced tests
`affectation_identifiant_ponctuation` and `identifiant_entre_guillemets_echappes`
first. Focused re-measure on the 3 affected `ra` files:
**both non-exact files become exact, no loss** (the third stays exact).

### Generic separation of non-reader faults (reused from §23)

`scripts/oracle/common.py::raison_hors_lecteur` now classifies, once for every
`.ly` oracle, a row whose input is **not** the reader's fault: an unusable
LilyPond MIDI reference, a LilyPond compile failure on the source, or obsolete
syntax that `convert-ly` would rewrite (`setTextCresc`, `sustainDown`, …
the same commands as `parser.rs::obsolete_music_command`). `mesure.py` marks
those rows with `common.marquer_exclu`, so `resume()` drops them from both
numerator and denominator — the same mechanism already used for code 3, no
separate basket duplicated. Selftest covers the three rules. On `ra` the
fresh run (`/tmp/ly-ra-excl`) excludes 2 files with unusable references and
reports **1366 exact of 1724 measured, 0 exact losses** versus the previous
run; `mut`'s obsolete basket is detected by the same helper.

## 23. LilyPond import: final status (2026-11-22)

### Measurement protocol

The oracle compares scoretk's MusicXML with the MIDI that the **installed
LilyPond 2.27.0** produces from the same `.ly` source. The references used
here were regenerated with `scripts/oracle/lilypond/references.py` (one
`ly.midi` per source, `-dno-print-pages -dmidi-extension=midi`, under
`timeout` and `prlimit`). The older `out3mut` tree was **mixed**: some
`ly.midi` came from another LilyPond or another `\score` choice (for
`Symphony7_1`, 18 631 notes against 23 374 for the installed compiler), and
some had been refreshed later. A reference that is missing or produced by a
different engine is not a reader error: the driver is run with
`--refs-only`, so such a file is marked `echec` and removed from the
comparable denominator. On `mut`, 1 718 references were written; 468 sources
have no usable MIDI (2.10 syntax, unfinished files). Two very large scores
exceed 120 s but had already written a complete MIDI and are kept. 475
manifest rows are excluded, four of them duplicate paths.

### Final `mut` figures

| Set | Files | Exact Q16 | Exact Q16 **or** n2_grace | Exact % (either) | N1 % | N2 % | N2 no-repeat % |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| all rows | 2 186 | 1 455 | **1 501** | 68.7 | 98.84 | — | — |
| measured (reference present, non-reader faults excluded) | 1 657 | 1 455 | **1 501** | **90.6** | **98.84** | **97.77** | **98.72** |

The two exact columns are kept side by side since Cause 18: **Exact Q16** is
the historical criterion (`r == t == n1 == n2`), **Exact Q16 or n2_grace** adds
the files whose graces only miss the plain grid because LilyPond plays them at
a distinct real time. The latter is the headline figure; the former is never
hidden, and the 45-file difference is exactly the grace-timing model.

The obsolete-source basket (60 files containing `\setTextCresc`, `\sustainDown`
and the other commands removed after 2.10, plus the `\deprecatedcresc` family
removed after 2.13.20) is no longer counted separately:
`scripts/oracle/common.py::raison_hors_lecteur` classifies, once for every
`.ly` oracle, the rows whose failure is not the reader's fault — unusable
LilyPond MIDI, a LilyPond compile failure, or obsolete syntax that
`convert-ly` would rewrite (the commands of `parser.rs::obsolete_music_command`).
`mesure.py` marks those rows excluded, so they leave both numerator and
denominator; on `mut` this excludes 520 rows (the obsolete basket, unusable
references and the sources scoretk deliberately does not support; the
`\compressMusic`, `\compressFullBarRests` and `\fermataMarkup` sources are
no longer excluded, their references being regenerated from a converted
copy) and leaves 1 665 source rows in the lot. Of those, 1 663 convert after
Cause 41 (two more sources read); six have an empty reference (purely
graphical scores), which leaves 1 657 comparable rows.

The previous campaign reported 692/2 186 exact (31.7 %; 39.9 % on the 1 735
rows that had a reference) and N2 93.49 on the old references. The exact rate was low because 595 files had **all** their
reference notes matched but one or more extra notes (unison duplicates, see
below); N2 was flattered because the old references were truncated by the
same obsolete commands (for `Violinsonata_No9_1`, 8 498 reference notes
instead of 10 997 for the installed compiler).

Against the previous run (same references, 1 704 files in common):
**518 exact gains, 0 exact losses**, N2 delta −142 notes. The small N2
decrease is the removal of duplicates that had accidentally matched another
instrument's note at the same pitch and attack (for example `Symphony5_1`,
`eugenia`, `Symphony5_2`); the kept note still matches, and no file lost
exactness.

The repeat fixes added **3 exact files and 0 losses** on the same 1,711
comparable rows (1,201 → 1,204 exact, N2 +4,491). The `\octaveCheck` and
`\afterGrace` fixes below add **9 exact files and 0 losses** (1,204 → 1,213,
N2 +1,230). The only per-file N2 decreases are the `JPSFotF` parts (at most
−10) and `Chop-28-24` (−6), all already divergent at tick 0 or carrying other
causes; no exact file regresses. The next pass (Causes 10–12) adds **98 exact
files and 0 losses** on the current oracle (1,252 → 1,348; N2 93.83 →
94.82, no-repeat 96.46 → 97.05): 63 from the floating-point-stable grid, 23
from freezing the default duration at parse time and 12 from punctuation
articulations after `^`/`_`. Of the 98 rows those three causes made exact, 96
remain measured; two fall in the newly excluded non-reader-fault set. The next
pass (Causes 13–16) adds **17 exact files and 0 losses** (1,348 → 1,365; N2
94.82 → 95.50, no-repeat 97.43 → 97.56): 11 from unquoted context names, 6 from
the explicit `/+bass` chord syntax, none from `Timing.measurePosition` or the
`\deprecated*` exclusion (which only removes rows from the denominator).

The next pass (Causes 17–20) adds **54 exact files and 0 losses** (1,365 →
1,419; N2 95.50 → 95.96, no-repeat 97.56 → 97.67): 7 from an assignment no
longer being taken as a command argument, 30 from the grace-aware exactness
model (Cause 18 changes the definition of *exact*, see below), 14 from the
leading-grace SMF shift, 3 from chord inversions.

The next pass (Causes 21–23) adds **11 exact files and 0 losses** (1,419 →
1,430; N2 95.96 → 96.03, no-repeat 97.67 → 97.77): 5 from `\simultaneous`, 1
from `q` and its tie, 4 from the octave-shift ordering. The remaining non-exact
measured files go from 465 to **217**.

The next pass (Causes 24–26) adds **10 exact files and 0 losses** (1,430 →
1,440; N2 96.03 → 96.12, no-repeat stable at 97.77): 8 from the
`ly/script-init.ly` post-events, 2 from `tieWaitForNote`; the `\compressMusic`
cause adds no exact file but removes 4 rows from the denominator, whose
reference LilyPond 2.27 cannot produce. With the six empty references also
left out, the denominator is 1 643 rows and the remaining non-exact measured
files are **201**. The rebase on the merged master fixes then adds two more
(`Field_Nocturne_5`, `Troldtog`), for **1 442 of 1 643 measured rows (87.8 %;
1 398 under Q16 alone**, 85.1 %), N2 96.12, no-repeat 97.77.

The next pass (Causes 27–30) adds **22 exact files and 0 losses** (1,442 →
1,464) and removes 7 rows from the denominator, whose MIDI reference is
shifted by LilyPond's recovery on obsolete commands: 8 from `-"texte"` and
slur commands as post-events, 1 from the named-staff continuation
(`alas`), 17 from `\(`/`\)` no longer eating the following tie;
`\compressFullBarRests` excludes 7 sources (one of them was exact, so the
net gain of the last pass is 17). The denominator is now 1 636 rows, **1 464
exact (89.5 %), 1 419 under Q16 alone (86.7 %)**, N2 96.96, no-repeat 98.33,
N2-grace 98.35; the remaining non-exact measured files are **172**. Four
`franck_ave_maria` variants lose 117 N1 notes each: `\(` no longer swallows
the following pitch, so the relative octave of the notes after it changes;
they were not exact under either parse and no exact file regresses.

The next pass (Causes 31–34) adds **20 exact files and 0 losses** (1,464 →
1,484) while 19 rows come back into the denominator:
- 4 `franck_ave_maria` variants become exact once `\markup \bold {a Tempo}`
  no longer leaks `a` as a note (Cause 31);
- 8 of the `\compressMusic`/`\compressFullBarRests` sources re-enter and are
  exact, because their reference is now compiled from a `convert-ly` copy
  (Cause 32);
- `enfantillages_pittoresques` (sequence of contexts), then
  `carcassi-op60-09/15` become exact with the continued named voices and
  the staff-wide `\transposition` (Cause 33);
- `gnomus` and `scarlatti-k491` become exact once their `\fermataMarkup`
  reference is converted (Cause 34), and three previously reference-less
  files re-enter as exact (`brahms-sapphische-ode`, `cornelius-ein-ton`,
  `petitefille`); `sonata3` and `gretchen` re-enter honestly non-exact.
  N2 no-repeat goes from 98.33 to 97.88 because these larger works join the
  computation.

The next pass (Causes 35–38) adds **4 exact files and 0 net loss** (1,484 →
1,488; N2 96.76 → 97.63, no-repeat 97.88 → 98.45): `Chop-28-19` comes back
after the master `\autoChange` change (Cause 35), `Symphony5_1` and
`Symphony5_2` become exact (Causes 36–37: old `#'prop` property syntax and
the empty-chord cursor), and `fuga2` joins them. `No03_Albumblatt`, exact
before the writer change, was temporarily lost when that change revealed a
`\set Timing.beatStructure = 1,1` leak; Cause 38 restores it, so the net
file balance is +4 with no loss.

The next pass (Causes 39–42) adds **13 exact files and no loss** (1,488 →
1,501; N2 97.63 → 97.77, no-repeat 98.45 → 98.72); Cause 42 corrects a
tremolo parsing bug without changing any measured file: the parallel branch inside
a tuplet (waltz-op39-15, rimsky-korsakov-op11-1, rach-prelude-op3-no2), the
`q` reference (eight Czerny/carcassi/In_dulci_jubilo files) and the
non-nesting block comments (Chop-28-20 and debussy_Arabesque_1, which also
become measurable: the comparable denominator grows from 1 655 to 1 657).
No file regresses.

### Where the remaining `notes_supp` files start

`scripts/oracle/lilypond/supp_first.py` (two workers by default) classifies
the 86 measured files that carry the `notes_supp` flag but are not exact, by
their **first extra attack** on the oracle grid (Q16), and describes that
attack's neighbourhood in the candidate MusicXML. Only four start at tick 0,
but they carry 7 014 of the 13 106 extra attacks: `carcassi-op60-09` (737)
and `carcassi-op60-15` (532) are the deferred reuse of named contexts,
`SpanishFandango1882` (526) and `JPSFotFConductorTransposed` (5 219) are
structural. Of the other 82 files, 47 add at most five attacks (104 in
total). At the first extra, 24 files have a written grace, 19 repeat the
pitch at the same attack on another staff, voice or part, 4 repeat it inside
one voice, and 1 falls on the last reference attack. This ranks the next
causes; it is not a verdict, since a `notes_supp` file's first divergence
often belongs to the same cause as a neighbouring `attaque_decalee` one.

### Cause 1: top-level `<< … >>` is one context per element

A `<< … >>` at the level of a `\score`, without `\new`, was read as one
anonymous staff with several voices. LilyPond opens one context per element
at Score level (`lily/parser.yy`, `context_prefix`; `lily/context.cc`): the
engraved score has two staves and the MIDI two tracks for
`\score { << { c'4 } { e'4 } >> }` (checked in SVG and MIDI). This is the
Mutopia choral idiom `<< \soprano \alto \tenor \basso >>`. The reader now
opens one anonymous group per element; inside a staff the same elements
remain voices (`collect_staves`, unchanged). Test
`simultane_sans_contexte_une_portee_par_element` (failed before: one staff
instead of two). `Bach_BrichAn` goes from four voices on one staff to four
staves and matches its reference exactly (241/241).

### Cause 2: unison notes are one MIDI attack

When two voices of the **same staff** play the same pitch at the same tick,
LilyPond emits one note (`lily/midi-walker.cc`, one track per staff); the
MusicXML writes both, and both are legitimate engraved notes. The oracle
counted two notes and 595 `mut` files had N2 = reference but stayed
non-exact because of these duplicates. `ly_notes.read_xml` now merges
identical attacks from different voices of the same staff. The merge is
disabled when the MusicXML has fewer staves than the reference has tracks
(`nombre_portees < tracks`): several tracks were then folded onto one staff
(`samia-a4`, `daslied`, `Oyseaux`), and merging would erase distinct notes.
A source that moves `Staff_performer` from `Staff` to `Voice` (for example
`daslied`) is thus preserved. The non-regression test is in
`ly_notes.py selftest`: two unison voices on one staff give one note, two
staves give two, and a repeated pitch in the **same** voice stays two notes.

`\partcombine` was not the top cause: it appears in 77 `mut` files for 8 264
lost notes (70 exact files), already documented as a reading limit (`scm/part-combiner.scm`,
`lily/part-combine-iterator.cc`, one combined voice). It ranks below the two
causes above and was not ported.

### Cause 3: `\repeat percent` plays once but occupies n bodies

`\repeat percent n { … }` is a `PercentRepeatedMusic`. Without
`\unfoldRepeats`, `lily/percent-repeat-iterator.cc` plays the body **once**
and then emits `n−1` `PercentEvent`s carrying `body_length_` (lines 60-68 and
98-112): the cursor must advance by `n × body length`, not one body. The
reader left the cursor on a single body, so every following note was early
(16 files). Under `\unfoldRepeats`, `scm/music-functions.scm:408`
(`unfold-repeats`, type `percent`, line 418) replaces the music with
`UnfoldedRepeatedMusic`; the body is then really played n times
(`ly/music-functions-init.ly:2622`). Both branches are now distinct. Tests
`repeat_percent_avance_les_repetitions` and
`repeat_percent_sous_unfold_repeats_est_joue` (fail before). On the 16 files
of the lot: N2 3,597 → 6,165 for 21,139 reference notes; three files become
exact (`boije_835…` 275 → 2,719, `stille-orig` 116 → 214,
`traditioner…1.23` 396 → 712).

### Cause 4: `\repeat tremolo` under `\unfoldRepeats`

Without `\unfoldRepeats`, the duration of `\repeat tremolo n { a b }` is n
times the written body and LilyPond plays two held notes (Notation Reference
§4.2.2); the parser still stretches the durations. Under `\unfoldRepeats`,
`scm/music-functions.scm:408` (`unfold-repeats`, type `tremolo`) replaces it
with `UnfoldedRepeatedMusic`: the body is replayed n times with its written
durations. The parser no longer stretches it and keeps the repeat context for
the interpreter. Test `repeat_tremolo_sous_unfold_repeats_est_joue` (9 notes
instead of 3); `repeat_tremolo_etire_les_durees` is unchanged. On the 25
files: N2 17,745 → 19,369 for 37,332 reference notes. No file becomes exact
(the largest gains, `SchubertF-D706-Psalm23` 1,549 → 2,606 and
`SchubertF-D827-NachtUndTraeume` 426 → 752, keep other causes); no exact file
regresses.

### Classification of the 465 non-exact files (before this pass)

The `mut` set outside the obsolete-source basket had 465 non-exact files
before the `\octaveCheck`/`\afterGrace` pass; 217 remain after the octave,
transposition, duration, articulation, context-name, chord, assignment,
`\simultaneous` and ottava passes. Symptoms
overlap (a file can be both `octave` and `notes_supp`); the first
table groups by the oracle's primary flag, the second counts the constructs
present in the source and its direct includes. `ne`/`ex` are the files that
were non-exact/exact with the construct **before** this pass; a construct is a
cause only when the reader's handling is actually wrong.

| Primary flag | Files |
| --- | ---: |
| `attaque_decalee` | 151 |
| `octave` + `notes_manquantes` | 85 |
| `notes_supp` | 64 |
| `octave` | 57 |
| `octave` + `notes_supp` | 47 |
| `attaque_decalee` + `notes_supp` | 42 |
| `octave` + `transposition` | 16 |
| `sortie_vide` / `autre` | 3 |

| Construct | ne | ex | Ratio ne/(ne+ex) |
| --- | ---: | ---: | ---: |
| `\repeat percent` | 16 | 0 | 1.00 |
| `\afterGrace` | 25 | 0 | 1.00 |
| `\repeat tremolo` | 25 | 1 | 0.96 |
| `\ottava` | 53 | 12 | 0.82 |
| `\tag` | 14 | 5 | 0.74 |
| `\arpeggio` | 67 | 29 | 0.70 |
| `\times` | 130 | 84 | 0.61 |
| `\tuplet` | 61 | 38 | 0.62 |
| `\grace`/`\appoggiatura` | 130 | 124 | 0.51 |
| `\change` (cross-staff) | 144 | 148 | 0.49 |
| `\repeat volta` | 205 | 325 | 0.39 |
| `\partial` | 146 | 377 | 0.28 |
| `\partcombine` | 7 | 70 | 0.09 |

The two largest **confirmed reader defects** are `\repeat percent` (16
files, none exact) and `\repeat tremolo` under `\unfoldRepeats` (25 files,
one exact); they are fixed above. `\afterGrace` is 25 files with no exact
file, but its playback places the notes at 1.635/1.693 where the following
note starts at 2.0; MusicXML has no after-grace event, and the oracle's
`<grace>` pairing attaches them to the following note, so this cannot be
fixed in the reader without turning the graces into ordinary notes. `\ottava`
is not a cause: the reader writes the written pitch plus `<octave-shift>`, and
the oracle applies that shift to obtain the played pitch (probe: LilyPond MIDI
keeps C4 under `\ottava 1`). `\partcombine` is 77 files in total but 70 are
exact; only 7 are non-exact, far below the two causes, so its MIDI behaviour
was not ported.

### Review fixes (2026-11-22): `attack`, fractions, `\octaveCheck`, alternatives

- **MusicXML validity**: `<offset>` is not allowed as a child of `<note>`
  (`specs/musicxml/musicxml-4.0.xsd`, `complexType note`); the performance
  shift is the `attack` attribute, in divisions, possibly negative. The writer
  now emits `attack` on the after-grace and `ly_notes.read_xml` reads it
  (keeping `<offset>` only as a fallback for other tools). The reduced case
  `\afterGrace 7/8 d'4 { e'16 }` validates with
  `xmllint --schema musicxml-4.0.xsd` (with the W3C `xml.xsd`/`xlink.xsd`)
  and contains `attack="-1"`, no `<offset>`.
- **`afterGraceFraction`**: `\afterGrace` accepts an optional scale
  (`\afterGrace 7/8 c1 { d16 }`) and the default can be assigned
  (`afterGraceFraction = 15/16`) or defined in Scheme
  (`#(define afterGraceFraction (cons 15 16))`). Tests cover the default,
  the optional argument and both top-level forms
  (`ly/music-functions-init.ly:86-115`).
- **`\octaveCheck` callback**: the check pitch is placed relative to the last
  pitch; on failure the reference shifts by `delta_oct = check.octave −
  result.octave` (`lily/relative-octave-check.cc:33-60`). The earlier fix only
  consumed the pitch; tests now cover a passing check (`[60, 62, 64]`) and a
  failing one (`[60, 74, 76]`).
- **`\repeat volta` alternatives** (the `\repeat volta` class, 75 files): the
  default relative traversal of `make-volta-set`
  (`scm/define-music-callbacks.scm`) visits the body and then the
  `SequentialAlternativeMusic`, so the second alternative is relative to the
  end of the first, not to the end of the body. Tests cover the sequential
  order with and without `\unfoldRepeats`; `13-air` becomes exact
  (204/204). Across the lot this fix gives **40 exact files, 0 losses** and
  N2 +21,406 (Bach chorales, ragtime, airs).

### Cause 7: chord quality after the duration

In `\chordmode`, a duration may precede the quality (`a2:m`). The reader
looked the pitches up before consuming the duration, so `parse_chord_name`
stopped on the digit and the chord was read major: `a2:m` gave A C♯ E instead
of A C E. The duration is now read before the name is recomposed, as in
`chord_body` (`lily/parser.yy`); a bass after the duration (`e1/d`) stays in
the name. Test `chordmode_qualite_apres_duree` (`69,72,76,69,73,76`). On the
six files with the pattern, `SINIVALKOINEN` becomes exact (86 → 103 matched),
N2 1,193 → 1,223 for 1,461 reference notes, no exact loss.

### Cause 8: octave-shift is per staff

`ly_notes.read_xml` kept one `octave_shift` for the whole part: an
`<octave-shift>` on staff 1 (the `\ottava` of a piano) also transposed the
notes of staff 2 at the same tick, which appeared an octave low (`Czerny
Op.821` has three staves in one part). MusicXML defines the direction for the
staff named by `<staff>`; the state is now a dictionary keyed by staff.
Selftest: two staves, a shift on staff 1 only. On the 53 files containing
`\ottava`, **14 become exact** (`turpinhar`, `25EF-11`, `hardtimes`,
`Chop-28-3`, `Le_porteur`…), N2 68,482 → 70,970 for 84,966 reference notes, no
exact loss.

### Cause 9: `\transposition` on a staff split into several model staves

`\transposition` is a **Staff** property (`lily/staff-performer.cc`,
`instrumentTransposition`). When one source Staff is unfolded into several
model staves (named voices of a guitar), the transposition applies to every
one of them. The builder only applied it when `staves.len() == 1` and silently
dropped it otherwise, leaving `milan-pavan2` an octave high. If the
transposition comes from a single `StaffBlock` (`propre`) and the group was
split, the model keeps **concert** pitches and the part receives
`instrument.transpose`: the staves belong to one Part, with one `<transpose>`,
and the **written** pitch stays that of the source. Tests
`transposition_sur_portee_depliee_decale_toutes_les_voix` (concert 60, 64,
`Transpose(-7, -12)`) and
`transposition_de_portee_depliee_garde_la_hauteur_ecrite` (MusicXML with
`<transpose>` and written C5). On the 25 non-exact files with
`\transposition`: `milan-pavan2` becomes exact (23 → 204), N2 88,152 →
88,650, no exact loss.

### Cause 10: the grid must ignore floating-point noise

MusicXML attacks are sums of `duration / divisions`; a triplet is 1/6 of a
quarter and cannot be represented exactly, so a note at 6.625 is read as
6.625000000000001 while LilyPond keeps exact rational moments
(`lily/include/moment.hh`, `Moment::main_part_` is a `Rational`). `quant`
rounded the quotient directly: a note exactly half a grid step away (26.5
cells) fell on the next cell on one side and on the previous one on the other.
The quotient is now rounded to 9 decimals before the round-half-to-even,
which leaves exact values unchanged and makes the grid insensitive to the
noise. Selftest: `quant(6.625000000000001) == 6.5`,
`quant(6.875000000000001) == 7.0`. **63 exact gains, 0 losses**, N2 +3,168
(`Czerny_Op_821_No_005`: 137 → 145).

### Cause 11: the default duration is fixed at parse time

LilyPond fixes the duration of a bare pitch when it parses it: `event_chord`
sets `parser->default_duration_` (`lily/parser.yy` l. 4644) and
`maybe_notemode_duration` (l. 3505) stores the whole `Duration`, `*n/d` factor
included (`make_duration`, l. 4704). The reader resolved it while walking the
tree, so a `\repeat` replaying its body started from the last duration of the
previous pass: in `traditioner_af_swenska_folk_dansar.3.13`, `g,` was 2
quarters (from `a2`) in the first pass and 3 (from the closing `e2.`) in the
second, shifting the whole second pass. Durations and multipliers are now
frozen in each chord, and the parser only updates them where LilyPond does:
`\skip`, `\partial`, the grouping duration of `\tuplet` and a music-function
argument leave the default untouched (verified with LilyPond 2.27:
`\skip 4*3 c` and `\partial 8 c` leave `c` at a quarter). The argument of a
music function is parsed only to find its end and the body is then re-read
after substitution (`REPARSE`, l. 1949, undone by `MYREPARSE`), so that first
read restores the parse state: without it, `\barre 3 { … g8 }` gave the
duration 8 to the bare notes before the `g8` of the argument
(`faure-sicilienne-guitare-doigtee`). Tests
`unfold_ne_reprend_pas_la_duree_par_defaut_du_passage` and
`arguments_de_fonction_ne_laissent_pas_leur_duree_par_defaut`. **23 exact
gains, 0 losses**, N2 +9,061; the two N2 decreases (`rimsky-korsakov-op11-1`
−102, `scarlatti-k491` −29) are files that were already non-exact and whose
first divergence does not move.

### Cause 12: a punctuation sign after a direction is an articulation

`script_abbreviation` (`lily/parser.yy` l. 3469) turns a punctuation sign into
an articulation after `^` or `_` (`dashDot`, `dashBang`, `dashLarger`,
`dashDash`, `dashHat`, `dashUnderscore`), and `direction_reqd_event` (l. 3337)
accepts it as a direction event. `parse_direction_event` only handled
commands, numbers and strings: on `d''16^.` the dot stayed in the token
stream, the `4` of the following `^4` fingering was read as a quarter-note
duration replaying the chord, and the triplet gained extra notes
(`Czerny_Op_821_No_016`, 155 notes instead of 149). The signs `.`, `!`, `>`,
`-`, `^`, `_` are now articulations; `+` (`dashPlus`, no equivalent in the
model) is consumed. Test `signe_apres_direction_est_une_articulation` (11
notes before, 4 after). **12 exact gains, 0 losses**, N2 +4,700 — the six
neighbouring Czerny exercises plus `Mazurka-Op6-No1`, `Tschaikowsky-op37.2`
and four others.

### Cause 13: `\deprecated*` commands are obsolete sources

`\deprecatedcresc`, `\deprecateddim`, `\deprecatedendcresc` and
`\deprecatedenddim` are the names `convert-ly` 2.13.20 gives to the postfix
form of `\cresc`/`\dim` (`python/convertrules.py` l. 3058), removed since.
LilyPond 2.27 reports "unknown command" and its error recovery inserts a
one-beat rest: `paulus_9` drifts by a beat at bar 10 although the reader follows
the source exactly. They join `common.py::COMMANDES_OBSOLETES`, so the generic
`raison_hors_lecteur` excludes those rows from both numerator and denominator
(7 measured rows on `mut`, all non-exact). Selftest extended.

### Cause 14: `\set Timing.measurePosition`

`\set Timing.measurePosition = #(ly:make-moment -1 4)` replaces the current
position in the measure (`lily/timing-translator.cc` l. 470-505): a negative
position `-d` makes the back-to-zero barline fall at `t + d` (the anacrusis
before a repeat), a positive position `p` closes the current measure at
`t + len - p`. The reader ignored it, so the 2/4 measures of the third repeat in
`andre-sonatine` stayed aligned on the previous 2/2 frame. The parser emits
`MetaEvent::MeasurePosition` and `plan_measures_global` closes the containing
measure accordingly. Test
`measure_position_negative_ferme_la_mesure_sur_l_anacrouse` (lengths `[1, 1,
1/2, 1/2]` before, `[1, 1, 1/4, 1/2]` after). It fixes the frame but not the
remaining extra note, so no row is gained; N2 is unchanged.

### Cause 15: context names without quotes

LilyPond accepts a bare symbol as a context identifier (`CHANGE STRING '='
simple_string`, `lily/parser.yy`): old sources write `\new Staff = one`,
`\context Voice = VA`, `\change Staff = up`. The reader only accepted
`Kind::Str`, so the name was lost and `\change Staff = one` no longer found
its staff ("portée inconnue"), leaving the notes on the wrong one. `parse_context_id`
now accepts `Str` or `Word` for `\new` and `\context Staff`. Test
`noms_de_contexte_sans_guillemets`. **11 exact gains, 0 losses**, N2 +5,199
(BWV 656/671/677/693, French suites 5, `BWV-827-5`…).

### Cause 16: explicit `/+bass` chord syntax

`scm/chord-entry.scm` (l. 46-53, `interpret-bass`) defines `/+FOO` as an
explicit bass, distinct from an addition. The chord-name lexer stopped at `+`:
`bes/+d` produced the Bb chord and left `d` in the stream, played as a D chord
on the next beat and shifting the whole grid (`WinchesterNew` and the hymns that
write `bes/+d`). `parse_chord_name` keeps the `+` in the name and
`chord_pitches` strips it before reading the bass pitch. Tests
`accord_basse_explicite_avec_plus` (7 notes instead of 9) and
`chord_pitches("bes/+d") == ([70, 74, 77], bass 62)`. **6 exact gains, 0
losses**, N2 +911 (`Leoni`, `ToGodBeTheGlory`, `DukeSt`, `Austria`,
`CwmRhondda`, `Foundation`).

### Cause 17: an assignment is not a command argument

`skip_command_arguments` consumed the word following an unknown command. On
`tu = \tieUp` followed by `td = { … }`, the `td` of the assignment was taken
as `\tieUp`'s argument: the `=` was left behind and the chain of definitions
disappeared, including `su = { \change Staff = up }` in `bwv798`. Every later
`\su` was then unknown, so `\change Staff` never returned to the upper staff
and notes landed on the wrong one. LilyPond only gives arguments to a
`music_function_call`; a symbol followed by `=` starts an assignment
(`lily/parser.yy`). A word followed by `=` is no longer consumed. Test
`commande_inconnue_ne_mange_pas_une_affectation` (0 notes before, 60 after).
**7 exact gains, 0 losses**, N2 +1,124 (`bwv599`, `bwv789`, `bwv794`,
`bwv797`–`bwv800`).

### Cause 18: exactness under either grace model

LilyPond writes each grace at a distinct real time, *before* the logical
attack of its main note (`grace-iterator.cc`, `midi-walker.cc`); MusicXML
cannot express that offset (a grace takes no logical time). The plain Q16 grid
(`n2`) therefore loses those files, while the `n2_grace` rule — which pairs
candidate graces by pitch inside the main note's window — finds them:
`n2_grace` is 100 % on 30 files still counted `attaque_decalee`. *Exact* now
accepts `n2 == r` **or** `n2_grace == r` (with `r == t == n1`). Both are
faithful models, and the union keeps the two files that are perfect under
`n2` but not under `n2_grace` (`KV331_1_6_var5`, `SchubertF-D686B`). Selftest:
`rule["exact"]`. **30 exact gains, 0 losses**, N2 unchanged. This changes the
definition of *exact* for every previous figure.

### Cause 19: leading graces shift the whole MIDI file

An SMF cannot encode a negative time: graces before the first chord shift the
whole piece by their total duration, whereas LilyPond keeps them in the
"grace" part of the moment, before zero (`lily/midi-walker.cc` writes deltas).
`ly_notes.recaler_graces_initiales` shifts the reference back onto the first
main note when the candidate starts with graces and the first `k` MIDI notes
are exactly those graces. Selftest: three leading graces and two main notes.
**14 exact gains, 0 losses**, N2 +4,718 — the whole `JPSFotF` family,
`bayadere-acteII`, `harpaallegro`.

### Cause 20: chord inversions put the notes above the bass

`bes/d` is not the root-position chord plus a bass: `process-inversion`
(`scm/chord-entry.scm` l. 67-160) removes the bass pitch from the chord and
`make-chord-elements` (l. 100-140) brings the other notes into the octave
above it. The reader added the bass to the root-position chord, so `bes/d`
played Bb4-D5-F5 + D4 instead of D4-F4-Bb4; same for `c/g`, `f/a`,
`g:m/bes`. The explicit `/+d` bass (`interpret-bass`) still adds the bass to
the root-position chord. Tests
`accord_renverse_place_les_notes_au_dessus_de_la_basse` (4 notes before, 3
after), `chord_pitches("bes/d")`, `chord_pitches("c/g")`. **3 exact gains, 0
losses**, N2 +9 (`Easter`, `WinchesterNew`, `Thaxted`).

### Cause 21: `\simultaneous` is simultaneous

`\simultaneous { … }` is the twin of `<< … >>` (`SIMULTANEOUS
braced_music_list`, `lily/parser.yy` l. 1581-1588; `scm/define-music-types.scm`
l. 719). The reader treated it as a mode head and read the block sequentially:
in `bach_air_bmv_1068` the second guitar voice
(`\context Voice="guitarLower"`) started after the first and every one of its
attacks was shifted. The block is now `Music::Sim` (while staying a music
command for `at_toplevel_music`). Test `simultaneous_est_simultane`: before, E4
fell at beat 1 (60 true, 64 false); after, both notes are at 0. **5 exact
gains, 0 losses**, N2 +799 (`bach_siciliano_bmv_1031`, `bach_air_bmv_1068`,
`faure_dolly_berceuse`, `Suite7`, `Ich_grolle_nicht`).

### Cause 22: `q` does not prolong the previous tie

`q` replays the pitches of the last chord, not its ties: `repetition-chord`
only carries the duration and articulations (`scm/ly-syntax-constructors.scm`
l. 228-232), and the repeated note closes a pending tie without starting a new
one. The clone of `last_chord_notes` kept the `tie` flag, so `link_ties`
extended the `q`'s tie to the next chord, whose note of the same pitch was
written as a `tie stop` and lost as an attack (`catacombae`, A3 of bars 16-18).
The tie flag of repeated notes is cleared (a `q~` still sets its own tie through
`chord_tie`). Test `q_ne_prolonge_pas_la_liaison_du_accord_precedent` (A3 at
2880 absent before, present after). **1 exact gain, 0 losses**; `catacombae`
becomes exact (134/134).

### Cause 23: `\octave-shift` applies by tick, not by document order

`<octave-shift>` is a dated direction: it applies from its position in the
measure. The writer emits it in the stream of the voice that carries it, so
notes of another voice written after a `<backup>` may precede it in the
document while being earlier in time (`Sa_Taille`: the end of the ottava in
voice 1 is written before the notes of voice 3 at the same tick, which lost
their shift — E6 instead of E5). `read_xml` now collects `(tick, staff, value)`
and applies it after reading each part, per tick: a note takes the last event
of its staff whose tick is at or before its attack. A global application only
treated the last part, hence the per-part call. Selftest: a `stop` direction
written before an earlier note of another voice; the note keeps its shift (74
instead of 86 before the fix). **4 exact gains, 0 losses** (`Sa_Taille`,
`GiulianiOp100No16`, `rach-prelude23-08`, `rach-prelude23-09`).

### Cause 24: `ly/script-init.ly` scripts are post-events

The articulation commands of `ly/script-init.ly` (`make-articulation`,
l. 29-81: `downprall`, `upprall`, `prallup`, `pralldown`, `lineprall`,
`prallmordent`, `downmordent`, `bachschleifer`, `coda`, `segno`, the
heel/toe string scripts, the Henze fermatas…) were only partly in the
reader's articulation list. An unknown command left the post-event path, and
the `~` (or tie parenthesis) that followed it was swallowed, so the next note
became a separate attack: `bwv-988-v22`, `g1 \downprall ~`, lost its tie at
bar 12. The names now join `is_articulation_command` and
`post_event_for_command` (Henze fermatas, `downmordent`, `bachschleifer`
mapped; the prall variants without an exact model equivalent are consumed
alone, without inventing an ornament). A recognized but unmapped script is
consumed alone: going through `parse_direction_event` ate the following
command and lost the `\times`/`\tuplet` (`BWV-830-4`: 757 → 786 matched
notes). Tests `script_d_ornement_garde_la_liaison_suivante` (two attacks
before, one after) and `script_sans_equivalent_ne_mange_pas_le_nolet_suivant`
(attacks 240/360/480/600 before, 240/320/400/480 after). **8 exact gains, 0
losses** (`bwv-988-v05/14/17/20/22/24`, `bwv848b`, `PFMUT`).

### Cause 25: `\compressMusic` is `\scaleDurations`

`\compressMusic #'(n . d)` is the old name of `\scaleDurations`
(`python/convertrules.py` l. 2765, rule 2.11.48): same argument and the same
durations multiplied by n/d. The reader treated it as an unknown command,
skipped the Scheme argument and read the block uncompressed, so `bwv631a`
(and three other chorals) shifted the whole piece by one beat from the
anacrusis. The alias now goes through the `scaleDurations` branch; test
`compress_music_est_scale_durations` (1/4 before, 3/8 after). The reference
side had to change too: LilyPond 2.27 reports `\compressMusic` as an unknown
command and its recovery inserts a rest, so the four measured sources that
use it had an unusable MIDI; `common.py::COMMANDES_OBSOLETES` now excludes
them like the other obsolete syntax. **No exact gain** (the four rows leave
the denominator), 0 loss.

### Cause 26: `\set tieWaitForNote` keeps dangling ties

`Tie_performer::stop_translation_timestep` normally drops ties whose tied
note has ended (`lily/tie-performer.cc` l. 166-180); under
`tieWaitForNote = ##t` it keeps them, and `acknowledge_audio_element`
(l. 116-150) closes the tie on the first note of the same pitch, however much
later. The reader always dropped the dangling tie: `Chop-28-1` (bar 33,
`\tuplet 3/2 {c,16~ g,~ c~} s8 | <c, g, c>4`) produced six attacks instead of
three. The reader now records the setting (`Context::TieWaitForNote`,
`MetaEvent::TieWaitForNote`, `\set` in `parser.rs`); `link_ties` builds the
flag's timeline per voice, keeps pending ties when it is on and then accepts
any head of the same pitch. Test
`tie_wait_for_note_garde_les_liaisons_pendantes`: three attacks with the
setting, six without, matching LilyPond 2.27's MIDI. **2 exact gains, 0
losses** (`Chop-28-1`, `LVB_Sonate_10no1_3`).

### Cause 27: `-"texte"` is a direction, not a lost token

`lily/parser.yy` accepts `-"texte"`: `post_event: '-' fingering` only takes
an `UNSIGNED`, but `post_event_nofinger: script_dir direction_reqd_event`
also admits `direction_reqd_event: gen_text_def`, and `gen_text_def` accepts
`STRING` (l. 3435-3445). This is the old spelling of textual fingering and
is still valid. The reader did not consume the string in the `-` branch: it
stayed in the stream and the `~` that followed had no note to attach to
(`Lobt.ly`, `re2-"4--5" \fermata ~re4`, attack added at bar 4). The string
now becomes a low `PostEvent::Text`, like `^"texte"`. Test
`texte_apres_tiret_garde_la_liaison_suivante` (three attacks before, two
after). The two reader fixes 27–28 together give **6 exact gains and 0
losses** (`Lobt`, `bwv633`, `moonlight1-a4`, `moonlight1-let`, `BuxWv185`,
`alas`); the two causes were committed before the same measurement, so they
are not attributed file by file.

### Cause 28: `\context Staff = "x"` finds the existing staff

`Context::find_create_context` looks up a named context before creating it
(`lily/context.cc` l. 269-273, `find (FIND_CREATE)`), whereas `\new Staff`
always opens a new one. Two `\context Staff = "right"` in one `\score` add
their music to the **same** staff: they are voices of one `Audio_staff`
(`lily/staff-performer.cc`, `get_audio_staff`), where
`Midi_walker::do_start_note` merges attack unisons (`lily/midi-walker.cc`
l. 95-115). The builder opened one part and one staff per group, so the
Mutopia chorales that stack a transposed registration (bwv633, bwv544,
bwv679, bwv632, bwv627…) had twice too many staves.

An `existing` Staff group whose single staff has an already-built name now
appends its voices to that staff instead of creating a part. Built voices
keep their slot (the new music takes free slots, otherwise it would overwrite
the first tracks); beyond four simultaneous voices, `build_voices` opens a
sister staff in the same part. The starting moment depends on the enclosing
form (`lily/parser.yy` `sequential_music`): siblings of a `<< … >>` are
simultaneous and start at tick 0 (bwv633), while a `\context Staff` in a
later element of a `{ … }` sequence continues after the music already
written (`GroupBlock::sequence_start`, `Interpreter::run` receiving a start
tick; `alas`, two successive `<< … >>` on the same named staves). `\context
Voice` does not take this path: its reappearance continues a voice in time
and is still to be done; `\new Staff = "x"` twice still opens two staves
(checked with LilyPond 2.27). Tests
`context_staff_de_meme_nom_est_retrouve` and
`context_staff_retrouve_continue_dans_le_temps`. Causes 27–28 give **6 exact
gains and 0 losses** (files listed under Cause 27); the sequence continuation
alone makes `alas` exact.

### Cause 29: `\compressFullBarRests` is an obsolete reference

`\compressFullBarRests` became `\compressEmptyMeasures`
(`python/convertrules.py` l. 4239) and no longer exists in 2.27: LilyPond
reports an unknown command and its recovery inserts a one-beat rest at the
start of the voice, shifting the whole staff. Checked on
`r8 g'8[ g' g'] ees'2`: notes at 1.5/2/2.5 with the old command, at 0.5/1/1.5
with `\compressEmptyMeasures` (same source, 2.27). Orchestra scores include
it from every instrumental part: `Symphony5_1/2`, `mozart-hrn-3`,
`O_Mistress_Mine`, `bridge`, `Concerto_a_4_G_score` were all non-exact for
this reference shift, not for a reader error. `common.py::COMMANDES_OBSOLETES`
now excludes it, like the 2.10 families and `\compressMusic`; the selftest
checks that the current `\compressEmptyMeasures` stays accepted. **7 rows
removed from the denominator, no reader change.**

### Cause 30: `\(` and `\)` are post-events

`\(` and `\)` are punctuation commands (`lily/parser.yy`,
`direction_less_event`), not directions. `starts_post_event` did not
recognise them, so the post-event loop stopped right after them and the
following `~` was swallowed: `Chop-28-4` (bar 8, `gs4->\(~ gs4`) lost its tie
and produced one extra attack (one note, hence a non-exact file), and so did
`lani`, `scriabine-op16-no1`, `prelude`, `intermezzo` and others whose held
note follows `\(`. Both commands now join `starts_post_event` and
`post_event_for_command` (`SlurStart(0)`/`SlurStop(0)`, like bare `(`/`)`).
Test `commande_de_liaison_d_expression_garde_le_tie_suivant` (two attacks
before, one after). **17 exact gains, 0 losses**. Four `franck_ave_maria`
variants lose 117 N1 notes each because the token after `\(` is now parsed
as music (grammar-correct), which changes the relative octave of the
following notes; they were not exact under either parse, and no exact file
regresses.

### Cause 31: a markup command's braced list is its argument

`\markup \bold {a Tempo}`: in markup mode a `{ … }` following a markup
command is that command's argument (`lily/parser.yy`, `markup_braced_list`,
l. 4174-4186). The reader only recognised a short list of layout commands
(`column`, `line`…), so the braced list stayed in the music stream, `a`
became a note and the following measure was displaced. On the last page of
`franck_ave_maria` this extra note changed the final `bis'` octave; because
the second voice of a `<< … >>` chains from the first voice's last pitch,
the whole bass voice dropped an octave (117 N1 notes per variant). The
text-style commands (`bold`, `italic`, `bold-italic`, `underline`, `box`,
`circle`, `rounded-box`, `smallCaps`, `large`, `small`, `smaller`, `huge`,
`tiny`, `whiteout`, `transparent`) and `center-align`, `wordwrap-center`
join the list. Test
`markup_de_style_consomme_sa_liste_entre_accolades` (three pitches before,
two after). The four variants recover 681/681 N1 and become exact.

### Cause 32: references from a `convert-ly` copy, not exclusion

LilyPond 2.27 refuses commands removed after 2.10/2.16 and its recovery
inserts rests, shifting the whole staff: `\compressMusic` (old
`\scaleDurations`, `python/convertrules.py` l. 2765) and
`\compressFullBarRests` (now `\compressEmptyMeasures`, l. 4239). Excluding
the files was a fallback. `references.py` now compiles the source as is, and
when the journal reports an unknown command, copies the source directory
(relative `\include`s keep working), runs the installed `convert-ly -e` on
every `.ly`/`.ily`, and compiles that copy; the status line records
`"converti": true`. The reference is therefore the installed LilyPond on the
source `convert-ly` would produce — reproducible, not a silent exemption.
15 references were regenerated in `refs27` (Symphony5_1/2, mozart-hrn-3,
O_Mistress_Mine, bridge, the two Telemann concertos, bwv612, bwv617,
bwv631a, bwv641, bwv652, bwv667, dvorak, Schubert D935-4) and the two
command families left `COMMANDES_OBSOLETES`. Honest measurement: 14 rows
come back (8 exact: bwv612, bwv631a, bwv641, bwv667, bridge,
O_Mistress_Mine, the two concertos); Symphony5_1/2 stay non-exact (N2 53 %
and 49 %), which is their real gap rather than a broken reference.

### Cause 33: continued named voices and the staff-wide `\transposition`

Two linked defects in a sequence of sections (carcassi-op60-09/15,
`{ \midiStuff \aSectionMusic \bSectionMusic … }`):

* a named voice reappearing in a later section is the **same** context
  (`lily/context.cc`, `find_create_context`, l. 269-273), and its music
  resumes at the date of its element in the sequence (`lily/parser.yy`,
  `sequential_music`). The builder opened a new part at tick 0. The first
  group of each `{ … }` element is now marked (`sequence_start`), the
  builder derives `element_start` from the end of the previous element, and
  every music of the element starts there; a continued voice keeps its slot
  in the staff.
* the `\context Voice` groups following an empty `\context Staff`
  (`midiStuff`) are that staff's voices — one `Audio_staff`
  (`lily/staff-performer.cc`, `get_audio_staff`), where
  `Midi_walker::do_start_note` merges unison attacks (l. 95-115) — and the
  `\transposition c` set by the empty staff stays pending and applies to
  **all** following voices (`instrumentTransposition` is a Staff property),
  not just the first; the bass of carcassi sounded an octave too high. An
  anonymous group without pitch (`\global` of a section, only skips) no
  longer closes the pending transposition.

Tests `context_voice_de_meme_nom_continue_dans_le_temps` (all attacks at
tick 0 before, 0/480/960 after) and
`transposition_d_un_contexte_vide_vaut_pour_les_voix_suivantes` (57 instead
of 45 before). `enfantillages_pittoresques` gains 749 N2 notes and becomes
exact; `carcassi-op60-09` and `carcassi-op60-15` become exact (830/830 and
645/645).

### Cause 34: `\fermataMarkup` is converted, not special-cased

`\fermataMarkup` became `\fermata` (`python/convertrules.py` l. 4237) and
no longer exists in 2.27. The recovery depends on the form: isolated
(`R2. \fermataMarkup`) inserts a one-beat rest and shifts the staff
(`gnomus`, `scarlatti-k491`), while as a post-event
(`R1^\fermataMarkup`) the rest falls inside the whole-measure rest already
there and changes nothing (`il-vecchio-castello`, `promenade-5`…). Excluding
on mere presence would have lost 19 exact files, and a syntactic
isolated/post-event rule would be fragile. The answer is upstream:
`references.py` converts the source (Cause 32), so 37 references were
regenerated in `refs27` and no rule is needed; `common.py`'s selftest records
that the command is not in the exclusion list. **4 exact gains, 0 losses**
(`gnomus`, `scarlatti-k491`, plus the two carcassi of Cause 33 for the same
measurement), and five previously reference-less files re-enter
(`brahms-sapphische-ode`, `cornelius-ein-ton`, `petitefille` exact;
`sonata3`, `gretchen` measured with their real gaps). Ten sources (headers,
part-only files) still produce no MIDI after conversion and keep their prior
state.

### Cause 35: a Scheme redefinition does not hide a built-in command

`#(define ottava (let ((orig-ottava ottava) …) (define-music-function …)))`
(`Chop-28-19`) wraps `\ottava` around the original function. The reader
cannot evaluate the Scheme function, but it stored the inner `#{ … #}` body
as a Scheme identifier; once `\name` resolved such identifiers
(`0d98662a`), `\ottava #1` returned that body instead of the built-in: six
parasitic notes and no ottava bracket, and a file that had been exact.
Scheme identifiers are now resolved **after** the built-in commands (the
`_ =>` arm of `parse_command`), so `\ottava` keeps its behaviour while an
unknown name still resolves (`variable_scheme_appellee_par_backslash`). Test
`une_redefinition_scheme_ne_masque_pas_une_commande_integree` (six notes
before, four plus an ottava spanner after). `Chop-28-19` is exact again.

### Cause 36: old `#'prop` property paths

Before 2.18, `\override Context #'prop = value` names the property with a
quoted Scheme symbol (`python/convertrules.py` rewrites it to
`Context.prop`). The lexer emits `#'prop` as one Scheme token (`'prop`);
`skip_property_path` only read words separated by dots, stopped after the
context, and left `= 1` in the music, where the `1` became a whole-note
duration. Every part of `Symphony5_1/2` starts with
`\override MultiMeasureRest #'expand-limit = 1`, so four beats were inserted
before the first note and the whole piece shifted. The quoted symbol is now
read as part of the path, and a context word without a dot no longer ends
the loop when such a symbol follows. Test
`override_ancienne_syntaxe_ne_consomme_pas_la_duree_suivante`.

### Cause 37: an empty chord must not advance the cursor (MusicXML writer)

A chord without pitch (a disguised silence, `\skip` carrying an annotation
such as the opening tempo) writes no `<note>`, but `chord()` still advanced
`out.tick`. The staff left a `<backup>` with no content at the start of the
next measure; the oracle's reader started from a negative cursor, measures 1
and 2 collapsed, and every following staff was shifted by two measures.
`chord_entry` now returns before writing when the chord has neither notes
nor grace chords; `move_to_tick` emits the correct `<forward>` to the next
element. Test `accord_sans_note_n_avance_pas_le_curseur` (a `<backup>`
before, none after). With Cause 36, `Symphony5_1` and `Symphony5_2` become
exact (8295/8295 and 6619/6619).

### Cause 38: `\set Timing.beatStructure = 1,1` is a value list

`\set` may receive a comma-separated list of numbers
(`\set Timing.beatStructure = 1,1`, the `\global` of `No03_Albumblatt`).
The first number was read as a fraction and the `,1` stayed in the music
stream, where the comma is an octave mark and the `1` a whole-note duration:
a four-beat silence was inserted before the first note. The list is now
consumed after the fraction. Test
`set_beat_structure_ne_fuit_pas_dans_la_musique` (attacks at
1920/2160/2520 before, 0/240/600 after). `No03_Albumblatt` returns to exact
(614/614); the writer fix of Cause 37 had exposed this latent reader bug,
which is why the file was exact only through the previous invalid XML
stream.

### Cause 39: a parallel branch inside a tuplet carries its real duration

`\times 2/3 { << { s8 s s } \\ { c'8[ d' e'] } >> }` (waltz-op39-15,
rimsky-korsakov-op11-1, rach-prelude-op3-no2): the first branch continues in
the current voice and belongs to the `Tuplet`, the next ones open a sister
voice (`simultaneous`, `lily/parser.yy` l. 1581-1588) that has no `Tuplet`
of its own. The interpreter applied the inherited scale to the cursor (1/3
of a quarter) but the event carried neither `Tuplet` nor `ticks`, so the
MusicXML writer fell back to the bare figure (1/2) and lengthened the measure
by 1/12, shifting everything after it. `ticks` is now
`figure × multiplier × tuplet_scale` (the inherited part), the `Tuplet`
remaining the sole carrier of the open tuplets. Test
`branche_parallele_dans_un_nolet_porte_sa_duree_reelle`. The three files
become exact (720, 1218 and 1725 notes).

### Cause 40: `q` repeats a bracketed chord only

`expand-repeat-chords!` (`scm/music-functions.scm` l. 920-945) only updates
the repeated chord for an `EventChord`, i.e. music written `< … >`: a bare
note is a `NoteEvent` and leaves the reference unchanged. `g4 < b d > q g q
q` therefore repeats `< b d >` at both `q` after `g` (czerny-op840-4 and -6,
whose chords had become lone `g`s: 75 and 51 notes out of place). After a
bare note with no chord before it, LilyPond warns "Bad chord repetition" and
emits no note. The two repetition mechanisms are now distinct:
`ChordKind::Repeat` (`q`) reads the last `EventChord`, the new
`ChordKind::PitchRepeat` (bare duration, `expand-repeat-notes!` l. 972-1015)
reads the last played element, note or chord. Test
`q_ne_repete_que_le_dernier_accord_entre_chevrons`; the `c4 q8 q8` case of
the bare-duration test now follows the reference. Both Czerny pieces, plus
`In_dulci_jubilo`, `carcassi-op1n02`, `carcassi-op60-10`,
`czerny-op840-2/3/9`, become exact.

### Cause 41: block comments do not nest

The `longcomment` state of the upstream lexer only closes on `"%"+"}"`: a
`%{` inside is text and a lone `}` closes nothing (`lily/lexer.ll:303-313`).
The reader counted depths, so an old comment of the form `%{ … }` (closed
further down by the `%}` of a `%{ %}`) swallowed the rest of the file:
`debussy_Arabesque_1`, `Chop-28-20` and `Egmont` reported "no music in the
LilyPond source". `skip_lilypond_block_comment` (embedded `#{ … #}` music)
follows the same rule. The lexer test that expected the wrong nesting now
expects the `x` of the code, matching LilyPond (checked on a reduced file:
c, d, e played). Test `commentaire_de_bloc_ne_s_imbrique_pas`.
`debussy_Arabesque_1` (1458/1458) and `Chop-28-20` (286/286) join the
comparable set and are exact; `Egmont` was still unreadable after this fix
(see Cause 42).

### Cause 42: `:` alone keeps the last tremolo type and consumes nothing

`tremolo_type` (`lily/parser.yy:3584-3596`) accepts an `UNSIGNED` only when
it is written: `':'` alone reuses `parser->default_tremolo_type_` (8 at
first), and the next token is read normally. The reader bumped the next
token in every case, so in `c4: d4` the pitch `d` was swallowed, the `4`
became a bare duration and repeated `c`. The same damage in
`a'''2.:8 a''':` (`Egmont`, `violinoone.ly`) made the whole variable
unreadable ("no music"). The number is now consumed only when it is an
integer, and the last written type is remembered. Test
`tremolo_sans_nombre_ne_mange_pas_la_note_suivante` (60,60,60,65 before,
60,62,64,65 after). `Egmont` reads again (15 068 notes); its MIDI reference
still cannot be decoded by the oracle (key signature with 8 sharps), so it
stays outside the comparable denominator and the `mut` figures do not move.

### Remaining `mut` groups (first divergence, non-exact files)

`scripts/oracle/lilypond/subdivise.py --ecarts` on the 2 186-row lot, after
the four passes above:

| Construct at the first divergence | Files |
| --- | ---: |
| markup/tweak | 56 |
| grace | 33 |
| repeat/volta | 20 |
| transpose/clef/key | 12 |
| scheme | 10 |
| tuplet | 9 |
| voices | 7 |
| include | 6 |
| relative | 5 |
| score/book | 3 |
| erreur (no classification) | 2 |
| lyrics | 1 |

The first three groups are the `ra` campaign's areas (markup/tweak,
repeat/volta, grace timing) and are left to it; `voices` and `relative` are
too. Among the `mut`-specific groups, several `tuplet` files
(chopin_nocturne_op9_n3, ballade-4) diverge only through LilyPond's MIDI
arpeggio spread, which crosses the 0.25 grid boundary and is not a notation
difference; `bwv617` shifts by ±0.25 on `\times 4/3`/`10/9` positions for
the same grid reason. The `scheme`, `transpose/clef/key` and `include`
groups are heterogeneous (named `GrandStaff`, `\change Staff`, arpeggios,
duplicate staff names in the MIDI score); each needs a new reduction.

### The 7 `attaque` files of the MIDI import lot: not a reader tie defect

In the MIDI import measurement (`docs/agents/midi.md`), seven files
(`lily-ed3284f3` and its copies, `LeCheminDeSaintJacques`) have three notes
in the `.ly` reference that the MIDI does not attack. Minimal reproduction:

```
VoixI = \relative do' { re4 re | fa4 fa8 r | ... }
```

At tick 2 the reference has `fa4` (1 quarter) **and** `fa8` (1/2) as two
attacks; the stored `lily-ed3284f3.midi` has a single `fa` of 1.5 quarters.
Regenerating the MIDI from the stored `.ly` with LilyPond gives the same
single 1.5 note, so the MIDI genuinely lacks the attack — it was produced from
a version of the source where the note was tied (the original
`Partitions/LeCheminDeSaintJacques/LeCheminDeSaintJacques.ly` no longer
exists, only the `lilypond-book` excerpt). Our reader reads the stored `.ly`
faithfully (two attacks; ties are merged elsewhere: the second voice's
`la4~ la8` is merged as expected), so there is **no reader tie defect to fix**
here; the pair (stored `.ly`, stored `.midi`) is inconsistent. The import
itself is faithful to the MIDI.

### Cause 5: `\octaveCheck` consumes its pitch

`\octaveCheck` is a music function taking a pitch and producing a
`RelativeOctaveCheck` (`ly/music-functions-init.ly:1274-1281`,
`lily/relative-octave-check.cc`): no note, no duration, and the pitch must not
become the next `\relative` reference. The reader treated it as an
argument-less style command, so the following pitch was read as a note and
reset the relative octave (for example `no7` played 79 instead of 67, `no2`
96 instead of 72). The parser now consumes the pitch with `parse_pitch_spec`
without touching the relative state. Test
`octave_check_consomme_sa_hauteur` (failed: `[60, 84, 86, 88]` instead of
`[60, 62, 64]`). On the five `mut` files: four become exact (`no7` 89 → 170,
`no2` 47 → 189, `no8` 82 → 236, `tallis-ordinal` 32 → 56);
`rach-prelude-op3-no2` goes from 212 to 894 matched notes out of 1,725, with
no exact loss.

### Cause 6: `\afterGrace` carries its played onset

`\afterGrace` places the ornament after `afterGraceFraction` (3/4 by default)
of the main note (`ly/music-functions-init.ly:86-115`); `lily/grace-iterator.cc`
puts it in the grace part and `lily/audio-item.cc:122` scales grace time by
9/40. The reader already modelled it as `Grace8After`
(`engraving/dom/chord.h`, `isGraceAfter`), but the MusicXML wrote it after the
main note with no offset, and the oracle attached it to the next note's
attack. `GraceChord` now carries `played_offset` (3/4 of the main ticks), the
MusicXML writer emits it as `<offset>` on the after-grace (negative from the
end of the note), and `ly_notes.read_xml` applies `<offset>` without moving
the logical cursor. MuseScore after-graces keep `played_offset: None`, so the
`linked-ties` corpus snapshot is unchanged. Test
`after_grace_porte_son_instant_de_lecture` (3/16 whole for a quarter note);
oracle selftest with `<offset>`. On the 25 `\afterGrace` files: **5 exact**
(`Symphony7_1`, `Violinsonata_No7_4`, `QuartetOpus18_No4_3`,
`Violin_Concerto_3`, `reger_grosser_gott…`), N2 89,147 → 89,252 for 90,360
reference notes, no exact loss.

### Remaining `notes_supp` and `attaque_decalee` files

`notes_supp` is now 77 primary files (178 files carry the flag), dominated by
the obsolete-source basket and by octave/transposition errors:

| File | Lost notes | First cause |
| --- | ---: | --- |
| `violin_concerto_1.ly` | 11 593 | obsolete `\setTextCresc` (reference gaps) |
| `LVB_Sonate_57_3.ly` | 5 363 | obsolete `\sustainDown`/`\sustainUp` |
| `ballade.ly` | 5 224 | octave (transposition) |
| `JPSFotFConductorTransposed.ly` | 4 888 | octave (transposed conductor score) |
| `troldhaugen.ly` | 3 397 | octave |
| `rach-prelude23-05.ly` | 2 445 | octave |

`attaque_decalee` is 240 files. The first group is the same obsolete-source
artifact (`violin_concerto_1` 11 593, `Violinsonata_No9_1` 6 176,
`LVB_Sonate_57_3` 5 363, `Violinsonata_No7_1` 2 794, the three
`Cellosonata_No3` movements); then large `\unfoldRepeats` works with
`\cadenzaOn`/`\partial` (`Symphony5_1` 5 538, `Symphony5_2` 3 350): their
cadenza measures have irregular lengths and are not reduced yet. The
`\partcombine` files (`Maiana`, `Encantada`) remain the known sequential
two-voice limitation.

### Historical campaign (2026-11-21)

The first full campaign measured 1 447 `mus`, 1 726 `ra` and 1 377 `mut`
files: N2 98.28 / 87.17 / 92.35, or 96.01 % for `mut` after removing the
obsolete-source and MIDI-anomaly baskets. The `ra` lot is LilyPond's
regression suite, mostly Scheme-generated or engraving-only sources (§22).
The `mut` lot was dominated by 13 obsolete sources and two MIDI anomalies
(`djinns` unison, `\partcombine` sequential in `dies_irae`'s `score.ly`).
Normalizing only the 15 renames of rules 2.11.51/2.11.52 before compiling
raised raw `mut` to 95.90 %, with 22 gains and four normalization artifacts:
for example `Violinsonata_No9_3` went from 5 852 to 7 683 reference notes
because the original MIDI was truncated by the unknown `\sustainDown`.

### Remaining classes

1. **Obsolete sources**: accepted by the reader (§22), but the installed
   LilyPond still produces abnormal MIDI until the source is converted.
2. **MIDI anomalies**: simultaneous same-pitch notes merged (`djinns`),
   `\partcombine` played as two sequential voices (`dies_irae`).
3. **Scheme music**: `define-music-function`, `make-relative`, lyrical
   evaluations — outside the subset, mostly in `ra`.
4. **Residual reading drifts**: `\afterGrace`, long n-olets, `\autochange`,
   `Staff` contexts that only set properties — each below 0.3 point.

### Writing: partial measure and `Timing.measureLength`

The writer used to add `\set Timing.measureLength = #1/8` after `\time 6/4`
when the first measure was shortened by `\partial`. LilyPond keeps that
length for the following measures: the setting is misleading and bloats the
output. Test `mesure_partielle_ne_fixe_pas_measure_length` (failed before).
On `00- Veni veni Emmanuel - chant piano.mscz` the output goes from 37 971 to
37 376 bytes (−595) with identical MIDI and N2: a compactness gain, not a
note gain — the 493-note loss of that file is the unison artifact (LilyPond
merges one note in five; the MuseScore reference plays them all, §20).

### Writing: compact explicit stem

`\once \override Stem.direction = #UP` (39 bytes) was written before
**every** chord with a stem direction. `\ly/property-init.ly` defines
`\stemUp`/`\stemDown` as aliases of that override; with `\once` they keep
the same one-shot scope for 13 bytes. Test
`correction_de_hampe_compacte` (failed before). On
`Vingt_danses_enfantines_pour_piano(1).mxl` the output goes from 695 013 to
370 197 bytes (−46.7 %) with identical compiled MIDI (23 625 notes, N2 8 754
on both sides): a compactness gain.

### Writing: re-measurement and ranking (2026-11-21)

Oracle: LilyPond MIDI of the produced `.ly` (with injected `\unfoldRepeats`)
against the reference MIDI (MuseScore for scores, LilyPond for `.ly` round
trips). Master binary re-measured, all lots except `.ly` (227/375 files):

| Lot | Files | N1 % | N2 % | Exact |
| --- | ---: | ---: | ---: | ---: |
| corpus | 56 | 79.91 | 78.36 | 28/56 |
| `.mscz` | 174 | 91.83 | 85.89 | 36/174 |
| `.xml` | 32 | 99.07 | 57.91 | 12/32 |
| `.ly` (partial) | 227 | 98.65 | 96.62 | 177/227 |

The 57.91 % of the `.xml` lot comes from two files,
`Vingt_danses_enfantines_pour_piano(1).mxl` and `(2).mxl` (12 200 lost notes
each): the MuseScore reference MIDI plays unison notes that LilyPond merges,
and the attack difference grows with the file size. **Without these two
artifacts the `.xml` lot is 95.60 %** (27 files, 17 612 notes).

Causes, by lost notes (model checked before attribution):
1. **unison/octave notes** — `Vingt_danses…` (2 × 12 200),
   `Hallelujah_-_Haendel` (50), the nine `Veni veni Emmanuel` (493 each):
   model and written `.ly` agree (exact round trip); the MuseScore reference
   plays each voice of a unison, LilyPond only one. Reference artifact, §20.
2. **reading** — `Espana_Cani` (1 544), `El_Choclo` (867),
   `Angels_Carol.ly` (1 568): the model itself differs from the reference.
3. **repeat unfolding** — `À-Notre-Dame.ly` (108), `lily-9025ee30.ly` (74),
   `0301_AccourezPeupleFidele.ly` (48): the oracle unfolds repeats in the
   candidate but not in the reference; model is at 100 %.
4. **accordion** — `Under_Paris_Skies` (48), `Tarentella_Napoletana` (22):
   MuseScore MIDI octave coupling, §20.

None of these is an export defect in the round-trip sense: about twenty
verified files have a model identical to the re-read `.ly`. The two causes
fixed here are therefore **compactness**: `measureLength` on a partial
measure and the verbose stem override.

### Writing oracle: model mode (like for like)

`ecriture.py` gained `--mode model` (copied from `abc/ecriture.py`): the
reference is the pivot model exported from the source, and every candidate is
**re-read by scoretk** into MusicXML before comparison. The same model
MusicXML is given to both tools. This removes MIDI-mode artifacts: LilyPond
unison merging, MuseScore octave coupling, and `\unfoldRepeats` injected into
one candidate only. Compilation is still measured (errors/warnings) in both
modes.

Reduced measurement on five artifact files (`Vingt_danses…` ×2, `Veni veni
Emmanuel`, `À-Notre-Dame`, `0301_AccourezPeupleFidele`): MIDI mode N2
**57.09 %** for both tools; model mode N2 **100 %**, 3/3 exact files for both.
The +42.91 points come from comparing artifacts, not from a writer change.

### Cause 43: `\chordmode` — octave marks and the following duration

`\chordmode { \fixed c, { bes,2 f } }` (Kryshachok) gave Bb3/D4/F4 instead of
Bb2/D3/F3 and placed `f` at q3 instead of q2. Two mistakes in the same branch
of `parser.rs`:

- the lexer splits `bes,2` into `bes`, `,`, `2`; the reader read the duration
  **before** the chord name, saw `,` (not a number) and left both tokens to the
  music that follows, so the `2` became a bare duration that replayed the chord
  and the `,` was lost. LilyPond's grammar makes the octave marks part of the
  pitch (`pitch_or_tonic_pitch` in `chord_body`, `lily/parser.yy:3165-3195`),
  then the duration, then the quality; the reader now consumes them in that
  order and shifts the table's root by 12 semitones per mark.
- the `\fixed` reference was applied (`events.rs`, `fixed_shift`) but the
  written mark was not, so `bes,` stayed one octave high.

Reduced test `chordmode_marque_octave_et_duree` (Bb2/Ré3/Fa3 then
Fa3/La3/Do4, two quarters each, reference LilyPond 2.24). Measured on the MIDI
writing lot: 894 → **897/1 019** exact, no loss, no N1/N2 regression; the three
files are `Kryshachok`, `VolgaVolga` (the two `octave` cases) and
`LaMouetteGrise`. The LilyPond reading lots are unchanged
(`lecture_musique` 922, `lecture_mutopia` 78).

### Cause 44: `\chordmode` — the minor modifier after the degree (`a:7m`)

`CommentLeSaisJe` writes `a:7m`; the reference MIDI plays a minor seventh
(69/72/76/79) while the reader produced the dominant seventh (69/73/76/79).
Two linked gaps, both from `scm/chord-entry.scm`:

- `parse_chord_name` consumed a quality `:7` and stopped, leaving the `m`
  word to the music (where it is not a pitch); LilyPond's `chord_item`
  (`lily/parser.yy:3886-3898`) reads steps **and** modifiers in written
  order. A modifier word (`m`, `min`, `maj`, `dim`, `aug`, `sus`, `add`) is
  now consumed after a degree and after each `.` addition.
- `modes::chord_pitches` only stripped a **leading** `m`/`min`; `7m` therefore
  looked up the quality `7m`, unknown, and fell back to a major triad. The
  modifier is now extracted wherever it is written
  (`interpret-additions` applies procedures in order, `:67-78`;
  `default-chord-modifier-list`, `:251-257`), while `maj` and the `m` of
  `dim` stay part of the quality word.

Reduced tests: `chordmode_modificateur_apres_degre` (69/72/76/79) and
`modes::tests::noms_daccords` (`c:7m` = C/E♭/G/B♭, `c:dim7` unchanged).
Measured on the MIDI writing lot: 897 → **898/1 019**, no loss, no N1/N2
regression; the LilyPond reading lots are unchanged.

### §22 groups `notes_supp` / `octave`: top-level `\context Staff`

The `ra` groups `notes_supp` (41 files in the current run) and `octave` (28)
were sequence-diffed file by file. The first shared construct was the
`collision-*`/`cluster-*` family: sources whose top level is
`\context Staff \relative << { … } \\ { … } >>`, with no enclosing
`\score`. `lily/parser.yy` `context_prefix` (l. 1685-1690) treats `CONTEXT`
like `NEWCONTEXT`: both create/enter a context
(`Context::find_create_context`, `lily/context.cc:269-272`), and
`ly/engraver-init.ly:114-116` makes `Staff` accept `Voice`. The block after
the command is therefore the **content of one staff**, and `<< … \\ … >>`
its two voices.

The reader only listed `\new` (not `\context`) in
`starts_music_command`, so a top-level `\context Staff` was skipped and the
following `\relative << … >>` was read as top-level music, where
`score_groups_in` turns each `<< >>` branch into a separate anonymous staff
(the Mutopia `<< \soprano \alto >>` idiom): one part per voice, no unison
merge in the oracle. `\context` is now a music-starting command like
`\new`. Failing reduced test
`contexte_staff_de_haut_niveau_garde_ses_deux_voix`
(`crates/scoretk-lilypond-import/tests/read_context_toplevel.rs`) asserts one
part, one staff, voices 0 and 1.

Measured (`scripts/oracle/lilypond/mesure.py`, refs `/tmp/ly95lect-refs-fixed`,
prefix `out3`, `--refs-only`). On the base of this campaign the `\context`
fix alone took `ra` **1 557 → 1 563/1 724** (six files: `collision-dots`,
`collision-heads`, `collision-merge-differently-dotted`,
`collision-merge-differently-headed`, `collision-single-head`,
`completion-heads-polyphony`), **no loss and no N1/N2 regression**; `mut` was
byte-identical per file. After rebasing on the later master (`06b51733`,
which already carries ly95's `\measureLength`/alternatives and
`\makeClusters` fixes), the two reader fixes of this section together give
`ra` **1 570 → 1 578/1 724** (+8, the six `collision-*`/`completion-*` plus
the two below), **no loss and no N1/N2 regression**; `mut` is identical
(1 192 exact on both sides, same N1/N2/N2-grace; 1 377-file manifest, fresh
refs). The MIDI lots are unchanged on both bases: `ecriture_ly` 899/1 019,
corpus 17/17, `.mscz` 3/8, MIDI import 1 013/1 029, no per-file loss
(measured master vs branch with the same oracle).

### `\` at score level, and chord unisons in the reading oracle

`<< { … } \\ { … } >>` at the `\score`/top level is **one** staff with two
voices: `\\` (VoiceSeparator) creates `Voice` contexts in the current
context (`lily/parser.yy` `simultaneous_music`/`context_prefix`), while
`<< { … } { … } >>` without `\\` (the Mutopia idiom) opens a context per
item. Verified on LilyPond 2.27: the first writes one MIDI track, the second
two. The reader dropped the `\\` token, so each branch became a separate
anonymous staff. `parse_context_list` now records the separator and returns
one enclosing staff; reduced test
`double_antislash_au_niveau_du_score_fait_des_voix`. On the campaign base
`ra` **1 563 → 1 565**, `flag-stem-begin-position` and
`text-spanner-override-order` become exact, no loss; `mut` unchanged.

The same LilyPond rule explains the **chord unisons**: `lily/midi-walker.cc`
merges two same-pitch note-ons at the same tick of a track, including the two
notes of `<c' c'>` (verified on 2.27: `<c' c'>4 <c' e' c'>4` plays 60 then
60/64). The MusicXML keeps both noteheads, so the reading oracle now merges
successive notes of the same chord (the second carries `<chord/>`) on top of
the existing different-voice unison rule. That first version gated the merge
on the raw candidate having **more** notes than the reference; the next pass
(see below) removed that gate, which consulted the reference, and fixed the
`monochords` note loss in the reader instead. Measured against the
**previous** oracle on the same binary: `ra` **1 565 → 1 568**
(`accidental-placement-samepitch`, `arpeggio-no-overshoot`,
`note-head-chord`), no exact loss and no N1/N2 regression; `mut` **1 186 →
1 192** (`aguado-op03n07`, `fleur_jetee`, `Lieder-ohne-Worte…Gondellied`,
`Symphony25_1`, `corni`, `k522-mv1`), no loss and no N1/N2 regression.
On the later master the corrected oracle is applied to **both** binaries
(the 1 570 baseline already includes it), so the master-vs-branch figure
above isolates the two reader fixes (+8). The MIDI lots are untouched (this
is the LilyPond reading oracle).

### Unconditional unison merge, staff structure, bare durations (2026-11-23)

**Oracle rule.** `Midi_walker::do_start_note` (`lily/midi-walker.cc`, branch
`now_ticks == queued_ticks`) merges two same-pitch note-ons that start at the
same tick **of the same track**: one attack is kept, with the longest
extinction. `_fusionner_unissons` (`scripts/oracle/lilypond/ly_notes.py`) now
applies that rule to the candidate alone, grouping by (part, staff, pitch,
onset) with **no reference gate**; `mesure.py` no longer computes
`nombre_portees`. The two exceptions are modelled from the reference sources,
not from the reference MIDI:

- **Grace notes.** A MusicXML cannot express the real tick of a grace note:
  `read_xml` puts every grace at the attack of its main note, so merging all
  graces at that onset would collapse a run (`beam-grace-tuplet-bounds`:
  24 `a64` in one voice, 80 → 12 notes). `read_xml` now records, per voice,
  each grace's **rank** from the main note (`lily/grace-iterator.cc`, §2.6.1)
  and the merge uses it in the key: two graces of the same pitch and rank in
  different voices coincide in real time and LilyPond merges them
  (`Troldtog` m111: two `\slashedGrace a,,8` voices, reference has one
  attack), while different ranks (a three-note run against a single grace)
  stay distinct. Same-voice graces are never merged.
- **Chord unisons** (`<c' c'>`): kept from the previous pass; verified on
  2.27 (`<c' c'>4 <c' e' c'>4` plays 60 then 60/64).

`monochords` does **not** prove a missing reference merge: `\longa`, `\breve`
and `\maxima` used **alone** are bare durations that repeat the previous
chord (`lily/parser.yy` `event_chord: duration`,
`scm/music-functions.scm::expand-repeat-notes!`), and `parse_command` ignored
them (it only repeated for a numeric duration). The reader now routes them to
`parse_bare_duration`; `monochords` has its 7 attacks per group and becomes
exact. Reduced test `breve_et_longa_seuls_reprennent_l_accord_precedent`.

**Staff structure.** With the reference gate gone, the remaining losses were
reader-level: the oracle merged notes that LilyPond keeps on **different
MIDI tracks**. Each cause below is verified against LilyPond 2.27 and has a
reduced test in `crates/scoretk-lilypond-import/tests/read_staff_structure.rs`.

1. **Elements of a staff group.** In `\new StaffGroup`/`PianoStaff`/
   `ChoirStaff`/`GrandStaff`/`OneStaff`/`\new Score`, each simultaneous element
   without a context opens **its own staff** (`ly/engraver-init.ly`, the group
   accepts `Staff`): `<< { c } { e } >>` gives two MIDI tracks. A sequence
   `{ \new Staff … \new Staff … }` plays the staves one after another
   (`StaffBlock::sequence_start`, verified: second track starts at beat 4).
   A `\\` (VoiceSeparator) gives one staff with two voices (one track), and
   is read at the token level in `parse_group_content` before `parse_music`
   discards it. Unknown group contexts defined by
   `\layout { \context { \ChoirStaff \name "SemiChoirStaff" } }` are
   recognised by their nested `\new Staff` children (`Nunc-dimittis`,
   `Mendelssohn-BartholdyF_op_23_No_1`).
2. **`\context ChordNames` inside a `Staff`.** `Staff` accepts `ChordNames`
   (`ly/engraver-init.ly` l. 492): the chord grid is a **sister staff**, not
   voices of the melody. Without it the oracle merged 17 chord tones with the
   melody in `LullabyBrahms-C`. `\context` on a non-structural name now goes
   through `parse_new_context` instead of being unwrapped, and a `Staff`
   whose content holds such a child is split into several staves.
3. **`\context Lyrics = name` without lyrics is a declaration.** The grammar
   nests the *following* music inside it (`lily/parser.yy`
   `context_prefix contexted_basic_music`), but for playback that music is its
   own track. The reader no longer consumes a `\context`/`\new` (or a music
   variable that is not `Music::Lyrics`) after such a declaration:
   `Abendlied`, `EsistbestimmtinGottesRatM`, `RatundScheiden`, `AbschiedM`,
   `SandmannM` keep their staves, and the `midi2ly`
   `\context Lyrics=trackA \trackA` (where `trackA` holds `\context Voice`)
   keeps its 11 parts (`Angels_Carol__John_Rutter-midi`).
4. **Voice-derived contexts.** `CueVoice`, `DrumVoice`,
   `GregorianTranscriptionVoice`… are `Voice` contexts
   (`ly/engraver-init.ly`), not staff contexts: `GroupKind::from_name` maps
   every name ending in `voice` to `Voice`. Otherwise the interpreter ignored
   them (`nested_context`) and `cue-clef-manually`, `midi-drums`,
   `lyric-combine-derived-voice` lost their notes; `TabVoice` at a group level
   also stays a separate staff (`bend-spanner-simple`). `\new Dynamics`, which
   carries only skips and marks, is played as a plain (silent) staff.

**Controlled measurements (current reference tree).** Each row changes only
the named part; both rows use the same reference tree
(`/tmp/ly95lect-refs-clean`), the same manifest and the same oracle on both
sides (`mesure.py --refs-prefix out3 --refs-only`). Binaries from `git
archive` of `d67eb63c` (master, with ly95's bare-duration arm) and the
rebased branch.

| Step | `ra` exact |
| --- | ---: |
| master reader `d67eb63c` + master oracle | 1 589/1 724 |
| our reader + master oracle (**same oracle**) | **1 591** (+2) |
| our reader + our oracle (oracle change alone) | 1 591 (0) |

Reader gains: `bar-line-caesura-double` and `collision-merge-dots-hidden`;
**no exact loss, no N1/N2 regression**. The oracle change alone alters no
exact file and no N1/N2 on `ra`. `mut` was not re-measured on this tree in
this pass; on the previous tree the same reader changed 1 187 → 1 199 exact
(same fixed oracle) and the oracle alone kept the exact files while lowering
N1 on seven already-inexact files (`BWV857_Fuga12`, `bwv869b`, `saltarello`,
`Dans_l_antre…`, `score.ly` (Dies irae), `baba`, `promenade-6`):
`\partcombine` voices and folded tracks that the reference keeps on distinct
MIDI tracks — a reader limit, not an oracle rule.

Our bare-duration commit is behaviourally equivalent to ly95's arm for
`\breve`, `\longa`, `\maxima` (both repeat the previous chord); it keeps
dots and multipliers through `parse_bare_duration`, and its test now asserts
the real durations: `\breve.` = three whole notes, `\breve*2` = four,
`\breve` = two (lily/parser.yy:3518 `steno_duration: DURATION_IDENTIFIER
dots`, :3532 `duration: steno_duration multipliers`). The MIDI lots are
unchanged: `ecriture_ly` 899/1 019, corpus 17/17, `.mscz` 3/8, MIDI import
1 013/1 029, no per-file loss or N1 regression.

## 24. LilyPond export: status (2026-11-21)

Measured on current master with `scripts/oracle/lilypond/ecriture.sh`,
LilyPond 2.27.0 as compiler, scoretk binary `ly95-obsolete-bin` (import
fixes included). `N2d` is the reference-weighted mean over all files with a
candidate MIDI; "exact" counts byte-identical note sets. The `.ly` lot of the
earlier run was only 227/375 files and is reported separately.

| Lot | Tool | Files | Compiled | Exact | N1 % | N2 % | N2d % | Errors | Warn. | Mean bytes | Max time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 28 | 79.91 | 78.36 | 75.90 | 0 | 11 | 1396 | 7.14 s |
| corpus | musicxml2ly | 56 | 51 | 28 | 94.52 | 91.98 | 89.29 | 79 | 20 | 2928 | 20.00 s |
| mscz | scoretk | 174 | 167 | 36 | 91.83 | 85.89 | 83.96 | 6 | 1524 | 28 084 | 23.95 s |
| mscz | musicxml2ly | 174 | 141 | 34 | 93.12 | 91.45 | 90.90 | 80 988 | 3709 | 40 172 | 40.07 s |
| xml | scoretk | 32 | 26 | 12 | 99.07 | 57.90 | 55.28 | 0 | 168 | 103 900 | 23.25 s |
| xml | musicxml2ly | 32 | 2 | 1 | 98.79 | 98.79 | 98.79 | 326 | 24 | 19 386 | 3.13 s |
| ly (partial) | scoretk | 227 | 190 | 177 | 98.65 | 96.62 | 96.42 | 0 | 42 | 1763 | 7.41 s |
| ly (partial) | musicxml2ly | 227 | 190 | 167 | 98.65 | 94.72 | 94.37 | 0 | 311 | 3309 | 7.45 s |

Per-file against musicxml2ly: corpus 1 win / 27 ties / 1 loss, mscz 5 / 31 / 3,
xml 11 / 1 / 0, ly 10 / 167 / 0. scoretk compiles far more sources (especially
`.xml`) and produces less than half the warnings, but musicxml2ly still leads
on N2 for `corpus` and `mscz`; the `xml` figure is dominated by two files whose
reference MIDI plays unisons that LilyPond merges (§22).

The `--mode model` oracle (§22) removes those artefacts: on the five largest
 artefact files both tools go from 57.09 % N2 to 100 % N2. The remaining
 gaps are therefore measured in that mode rather than attributed to the
 reference MIDI.

### Mid-piece `\transposition` (LilyPond reader) and `<transpose>` (MusicXML writer)

`\transposition` may change during a piece (Notation Reference §1.3.4:
"may be changed during a piece", "may change from staff to staff"). The
LilyPond reader applied only the first interval to every note and stored a
single `Part.instrument.transpose`. It now collects every transposition event,
shifts each chord by the interval in force at its tick — the same tick-aware
model MuseScore's importer uses (`read302/measure.rs`, `read4xx/mod.rs`,
`Part::set_instrument_at`) — records the later intervals as instrument changes,
and marks their measure as a section break so the MusicXML writer re-emits
`<transpose>` at the new value (`write_measure`, `is_first`). The MusicXML
writer now reads `part.instrument_at(tick)` instead of the base instrument.

Failing reduced test `changement_de_transposition_en_cours_de_piece` first.
On `testMultiInstrumentPart3.mscx`, model-mode N2 for scoretk goes from
**50 % to 100 %** (8 notes, one file); musicxml2ly also reaches 100 %.

### Export status in model mode (after the transposition fix)

With `--mode model`, the whole 56-file versioned corpus gives scoretk
**99.78 % N1/N2/N2d**, 48/56 exact, 0 errors, 11 warnings, mean 1326 bytes,
versus musicxml2ly 89.38 % N2 / 83.91 % N2d, 27/56 exact, 79 errors,
20 warnings, mean 2903 bytes. Before the mid-piece transposition fix the
same corpus was at 99.6 % with only two imperfect files; the fix recovers
`testMultiInstrumentPart3.mscx` (8 notes, 50 % → 100 % N2 on that file).
The remaining 8 imperfect corpus files are ottava and instrument-convention
differences with the MuseScore model, documented in §20.

## 25. LilyPond export: final status (2026-11-21)

`--mode model` on the final state: the oracle's reference is the pivot model
exported from the source; both candidates start from the same model and are
re-read by scoretk. One unsupported input is excluded (the Pachelbel accordion
arrangement, exit code 3). The oracle compiles with LilyPond 2.26.0 (its
`nix-shell -p lilypond`); the system binary used for manual checks is 2.27.0.

| Lot | Tool | Files | Compiled | Exact | N1 % | N2 % | N2d % | Errors | Warn. | Mean bytes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 48 | 99.78 | 99.78 | 99.78 | 0 | 11 | 1326 |
| corpus | musicxml2ly | 56 | 51 | 27 | 95.31 | 89.38 | 83.91 | 79 | 20 | 2908 |
| mscz | scoretk | 173 | 169 | 146 | 100.00 | 100.00 | 99.98 | 6 | 1530 | 25 429 |
| mscz | musicxml2ly | 173 | 155 | 34 | 99.69 | 93.94 | 92.78 | 35 901 | 4578 | 45 671 |
| xml | scoretk | 32 | 29 | 23 | 99.98 | 93.94 | 93.60 | 2 | 252 | 32 940 |
| xml | musicxml2ly | 32 | 27 | 9 | 93.10 | 67.38 | 65.36 | 78 | 430 | 24 421 |

scoretk is above musicxml2ly on every lot, compiles more files, and emits
far fewer errors and warnings. The xml aggregate varies between 93.60 % and
95.60 % from one run to the next: three copies of
`Vingt_danses_enfantines_pour_piano.mxl` (17 616 notes each) compete for the
oracle's per-file timeout and one or two of them intermittently produce no
reference; every copy that completes scores 100 %.

Relative to the model-mode run before this campaign (252 files in common):
**16 gains, 0 losses**. The gains are all nine `Minuit chrétien` parts
(93.87 % → 100 % each, ~280 notes each), `Vingt_danses_enfantines_pour_piano`
and its `.mxl` copies (100 %), `Under_Paris_Skies` (95.49 % → 100 %),
`Tarentella` (97.39 % → 99.5 %), `Waltz_No._2` (99.43 % → 100 %),
`Salazar` (99.68 % → 100 %), two `Puer_natus_est_nobis` parts
(98.04 % → 100 %) and `testMultiInstrumentPart3` (50 % → 100 %). The ottava
placement fix restores the four files that the rejected reader-side shift had
regressed (`layout_elements`, `copypaste_parts`, `partExclusion`,
`testExcludeInvisibleElements`) to their baseline values.

### Remaining scoretk losses, ranked by class

1. **Irregular measure lengths / negative ticks (XML/MXL lot)** —
   `Vivaldi…_10._Sicut_erat_in_principio` (1390), `Anerio_Ascendit_Deus`
   (320), `Perosi…Laudate_Dominum` (190). The pivot model of the Vivaldi file
   starts at negative ticks (−55 quarters), which the re-read voice does not
   reproduce (first divergence at note 0); Anerio and Perosi diverge first at
   ticks 4 and 21 with duplicated chord notes. These need reader-side work on
   `\partial`/`Timing.measureLength` and negative ticks, not a writer feature.
2. **Pickup in `Byrd_Dies.mxl`** (336) — the writer emits `\partial 2` plus
   two transparent `r4` in the first measure; the model's first notes are at
   tick 1.5, the re-read ones at 2.0 (first divergence at note 0).
3. **Tuplets in `CanonDeLaPaix.xml`** (229) — first divergence at tick 67.75,
   a quarter-beat drift with no further local feature.
4. Small residuals — `Magnificat__Prt` (49, duration only),
   `Angels_Carol` (9), `Tarentella` (5), `testExcludeInvisibleElements` (4),
   two `Omni_die_dic_Mariae` parts (1 each). No single-writer feature left.

### Ottava: place the command before the note it covers

MuseScore stores an `Ottava` spanner with `tick`/`tick2` anchored on the
ChordRest segments (`read400/connectorinforeader.cpp`, `setTick2(item->tick()
+ l.frac())`); our importer mirrors this in `read4xx/ottava.rs::resolve`.
`ly/music-functions-init.ly` defines `\ottava` as an `OttavaEvent`, i.e. a
context property, so it must be written **before** the note from which the
octave applies. `render_body` attached every spanner event with `attach_post`
(the last note at or before the tick), which placed `\ottava 1` one quarter
late and made the re-read spanner one quarter off — the cause of the four
regressing files in the rejected attempt. Ottava events now go through
`attach_pre`, like `\clef`/`\key`.

Failing reduced test `ottava_commence_avant_la_note_du_tick_de_depart` first
(two measures, one ottava). On `testExcludeInvisibleElements.mscx` the
written `.ly` now has `\ottava 1` before the first note of the covered
measure and `\ottava 0` before the next measure, matching the reference
`<octave-shift>` positions.

### Hairpin post-events must not swallow the next command

`parse_post_event` sent `\<`, `\>` and `\!` through its generic command
branch, which calls `parse_direction_event` with the hairpin as if it were a
direction (`^`, `_`, `-`). That consumed the following command as a script:
after `c'8 \!`, a written `\acciaccatura { e'8 }` became a plain `{ e'8 }`,
so the grace was read as an ordinary note and shifted every later attack by
half a beat — the local cause of the long tuplet/voice drift class on
`Vingt_danses_enfantines_pour_piano.mscz` and of all nine `Minuit chrétien`
parts. These tokens are `direction_less_event`s in `lily/parser.yy` (no
argument), so `parse_post_event` now returns their `PostEvent` directly.
Failing reduced test `acciaccature_apres_arret_de_soufflet` first.

### Invisible full-measure rest follows the measure length

`chord::render` emitted an invisible full-measure rest as
`\once \override Rest.transparent = ##t r1` before checking
`is_full_measure_rest()`. `r1` lasts four quarters whatever the time
signature, while `R<length>` fills the measure (`lily/parser.yy`,
`MULTI_MEASURE_REST`). In 3/4 this shifted every later attack by a quarter —
the remaining loss of the accordion files (`Under_Paris_Skies`,
`Tarentella`). The writer now emits
`\once \override Rest.transparent = ##t R2.` in 3/4. Failing reduced test
`silence_de_mesure_invisible_en_3_4` first; both files re-read with zero
lost notes and the whole mscz lot reaches 99.98 % N2d.

### Dynamic-event placement: investigated and reverted

Moving `\cresc`/`\!` before their note looked like a fix for the four
`DynamicTextSpanner is not fully contained in parent spanner` errors on
`Angels_Carol__John_Rutter.mscz`, and it did remove them — but LilyPond 2.26
requires dynamic events to be **post**-events on a note (`lily/parser.yy`
`post_events`, NR §3.1.2 Dynamics). Writing them before the first note gives
« CrescendoEvent non attaché » on `testHairpinDynamics` and `layout_elements`,
and moving `\<` before its note gives twelve « Hairpin is not fully contained
in parent spanner » errors on `Tibie_Paiom`. The only variant that compiled
both real files cleanly replaced `\cresc`/`\decresc` with `\<`/`\>`, losing
MuseScore's dashed text hairpins. The placement change was therefore
**reverted**: the writer keeps dynamics where `musicxml2ly` puts them, and the
four messages stay documented below as a layout-only discrepancy.

### Remaining classes: investigation (LilyPond 2.26 / 2.27.0)

The writer already emits the constructs the Notation Reference prescribes for
irregular metres: `\partial` for a shortened first measure (**NR §2.3.3
Upbeats**), `\set Timing.measureLength = #num/den` when the real measure
length differs from the displayed time signature (**NR §2.3.1 Time
signature**), `R<length>` for full-measure rests (**NR §2.2.3 Full measure
rests**) and `\tuplet n/d { … }` (**NR §2.1.2 Tuplets**) — not
`\scaleDurations` (**NR §2.1.3**), which is for scaling a whole passage
without changing the signature. Minimal round trips through scoretk confirm
that `\partial 2`, `\set Timing.measureLength = #5/4` and `R2.` in 3/4 are
read back at the correct ticks. The residual losses are therefore not a
missing LilyPond construct:

* **`Vivaldi…_10._Sicut_erat`** (1390 lost). The pivot model exported from
  the `.mxl` contains 90 notes with *negative* attacks (first divergence at
  note 0: −55 quarters in the reference, 0 on re-read) and voice chains whose
  `<backup>` of 208 divisions pushes the cursor before the measure start.
  LilyPond cannot place a note before tick 0; fixing this needs the
  MusicXML/pivot voice-backup model, not the `.ly` writer.
* **`Anerio_Ascendit_Deus`** (320) and **`Perosi…Laudate_Dominum`** (190).
  Their first divergences are *durations and staff offsets* (69 at tick 45
  lasts 1.5 quarters in the reference vs 1.0 on re-read; 67 at tick 53 is 3.0
  vs 2.0; a whole part starts four quarters early in Perosi). Tied notes and
  per-staff measure lengths in the model, not a writer command.
* **`Byrd_Dies.mxl`** (336). The first measure of the reference contains two
  invisible quarter rests at ticks 0 and 0.5 (two voices overlapping); the
  writer emits `\partial 2` plus two consecutive `r4`, so the re-read measure
  lasts 2 quarters instead of 1.5 and every later attack shifts by 0.5.
  Reproducing overlapping rests needs separate voices (**NR §5.2.1
  Single-staff polyphony**), which the model does not mark.
* **`CanonDeLaPaix.xml`** (229). The source has a 16th note in voice 2
  ending the measure; the pivot model carries it in voice 1 one sixteenth
  *before* a whole note. The writer has no grace flag and writes `a1 f16`, so
  the re-read places the 16th after the whole note (tick 67.75 → 68.0) and
  shifts the following notes. A faithful rendering would need either
  separate voices (**NR §5.2.1**) or `\grace` (**NR §2.6.1 Grace notes**).

### Compile messages

* `Angels_Carol__John_Rutter.mscz`: four LilyPond 2.26 errors
  « Spanner `DynamicTextSpanner' is not fully contained in parent spanner »:
  the text hairpin's end tick falls after the slur that encloses it. Only a
  replacement of `\cresc` by `\<` removes them, at the cost of the dashed
  text-hairpin appearance; no note is affected, so the faithful `\cresc` is
  kept (see the reverted placement note above).
* `Amazing_Grace_-_String_Quartet.mscz`: two errors from Ghostscript
  rejecting the `NotoSerifCJKjp` CFF2 font selected for the score's Japanese
  text (`イ ギ ス リ`) during PDF conversion. The `.ly` itself is valid and
  its N2d is 100 %; this is an environment font failure, not a writer defect.
* `…_10._Sicut_erat_in_principio.mxl`: two LilyPond 2.26 layout errors
  « No viable beam quanting found. Using unquanted y value » on measures
  whose explicit stem directions conflict with an irregular measure length.
  Removing the `\once \stemUp`/`\stemDown` overrides makes them disappear but
  would discard MuseScore's stem directions; no note is affected, so they are
  kept.

This is the final status: the last investigation round changed no exported
music. The dynamic-event placement experiment above was reverted after
measurement showed it traded four messages on one file for new warnings on
three others, so the table remains the measurement of the shipped writer.
All remaining losses and messages have a documented cause in the sections
above; none is an unexplained per-file regression (the per-file comparison
with the pre-campaign run shows 16 gains and 0 losses).

## 26. Export campaign toward 95 % (2026-11-22)

### Baseline (`--mode model`, current master)

`target/ly95-base/`, oracle `scripts/oracle/lilypond/ecriture.sh --mode model`
on the three existing manifests (56 corpus, 173 mscz, 32 xml). `exact` counts
byte-identical note sets after both candidates are re-read by scoretk.

| Lot | Tool | Files | Exact | N1 % | N2 % | N2d % | Mean bytes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 48 | 99.78 | 99.78 | 99.78 | 1326 |
| corpus | musicxml2ly | 56 | 27 | 95.31 | 89.38 | 83.91 | 2899 |
| mscz | scoretk | 173 | 147 | 100.00 | 100.00 | 99.98 | 26777 |
| mscz | musicxml2ly | 173 | 34 | 99.66 | 93.47 | 92.30 | 47234 |
| xml | scoretk | 32 | 25 | 99.98 | 96.85 | 96.67 | 54019 |
| xml | musicxml2ly | 32 | 9 | 93.16 | 67.41 | 65.38 | 38901 |

### Ranked mscz divergences (first divergence, note tuple)

| Files | First divergence (reference → scoretk) | Class |
| ---: | --- | --- |
| 9 (`Minuit chrétien`) | `(75, 515.5, .25, 6)` → `(74, 513.5, 1, 6)` | tied grace note |
| 3 (`Vivaldi … Sicut erat`) | `(73, 172, .5, 8)` → `(71, 172, .5, 8)` | extra tie continuation |
| 1 `JMHaydn_Viderunt_omnes` | `(67, 16.5, .5, 1)` → `(69, 16, .5, 1)` | extra note |
| 1 `Laudate_pueri` | `(74, 74, 1, 3)` → `(75, 72, 2, 3)` | pitch/onset |
| 1 `Under_Paris_Skies` | `(42, 294, 2, 0)` → `(72, 294, 2, 0)` | octave |
| 1 `Tarentella` | `(42, 262.5, .5, 0)` → `(78, 265, .5, 0)` | extra note |
| 1 `Magnificat__Prt` | `(72, 88, 3, 0)` → `(72, 88, 2, 0)` | duration |
| 2 `Omni_die…` | `(67, 14, 2, 0)` → `(67, 14, 1, 0)` | duration |
| 1 `Chant_de_la_Promotion` | no reference notes | empty source |

### Writer cause: tied grace note (committed)

`\acciaccatura { d'8 }` must carry the tie inside the grace group. LilyPond's
`def-grace-function` (`scm/music-functions.scm:1237-1248`) wraps the music in a
sequential block, so the `~` has to be on the last note of the block; otherwise
LilyPond plays two attacks instead of one (verified on a reduced pair compiled
with LilyPond 2.26: `a.midi` two onsets, `b.midi` one). Reduced test first,
snapshot `v4_linked-ties.ly` updated. Re-measure (`target/ly95-grace/`):
**no change in model mode** — our LilyPond reader does not propagate the
main note's `tie back` from the grace's `~`, so the two `.ly` forms produce the
same re-read model. This is a **reader-side** limitation for ly95/wsz, not a
writer defect; the writer change is kept because it is what LilyPond plays.

### Reader-side causes found (documented, not fixed here)

- `Byrd_Dies.mxl` (§25 residual): the source measure 1 has two overlapping
  quarter rests with `<backup>` (pickup 1.5 quarters), written as `\partial 2`
  plus two `r4` (2 quarters). The 0.5-quarter shift affects every subsequent
  attack (451 notes). The writer follows `Measure.len`; the mismatch originates
  in the pickup model, not in a missing writer rule.
- `Magnificat__Prt`, `Omni_die…` (duration only): a minimal `.ly` with `g2`
  under `\set Timing.measureLength = #1/2` is re-read by scoretk as a quarter.
  Isolated test `target/gracetie/mag3–5.ly`: same result with and without
  `\set`, so the truncation is in the LilyPond reader's note duration, not in
  the writer's measure-length command.
- `Vivaldi … Sicut erat`: 1–3 extra notes in voice 8; source and re-read models
  carry identical `<tie type="stop"/>` markers, so the difference is in how the
  tie continuation is attached on re-read, again reader-side.
- `CanonDeLaPaix.xml`: first divergence at note 1 with a 0.25-quarter drift;
  no tuplet in either model — a 16th-note positioning issue on re-read.

### New fourth lot: `ly → model → ly` on 1 447 sources

`target/ly95-lylot/`, `--mode midi`, manifest `/tmp/ly-import-lots/manifestes/mus.txt`
(1 447 `.ly` from `~/Musique`), MIDI reference taken from the reading oracle's
cache (`/tmp/ly95lect-refs-fresh/out3mus`, copied flat under `refs/` by md5 of
the path; 1 445 of 1 447 found). Both candidates are compiled by LilyPond 2.26.

| Tool | Files | Compiled | Exact | N1 % | N2 % | N2d % | Mean bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| scoretk | 1447 | 1443 | **1328 (91.8 %)** | 99.18 | 97.19 | 96.84 | 2043 |
| musicxml2ly | 1447 | 1442 | 1238 (85.6 %) | 99.17 | 94.32 | 93.77 | 3696 |

Flags split as `attaque_decalee` 119, `notes_manquantes` 66, `octave` 64,
`notes_supp` 54, `sortie_vide` 52, other 9.

### Cause — grace at the end of an expression is still played

`scm/music-functions.scm::def-grace-function` wraps the whole argument in
`GraceMusic`; a `\grace`/`\acciaccatura` at the end of an expression has no
following chord to attach to. The reader kept such graces in `pending_grace`
forever, so `grace-end-expression.ly` (`{ e' \acciaccatura << e'8 \\ cis' >> }`)
read one note instead of the reference's three. `Interpreter::run` now flushes
pending graces onto the last chord as `grace_after` (`played_offset`), which
the MIDI writer plays (`createGraceNotesPlayEvents`). Failing reduced test
`acciaccatura_avec_accord_simultane_garde_toutes_les_notes` first. Focused
re-measure on the 110 grace files: **4 files gain N2, 0 lose**, one becomes
exact (`spacing-loose-grace-linebreak`).

### Cause — quoted string identifiers (`lily/parser.yy`)

`lily/parser.yy` accepts quoted strings as identifiers: `"|" = \bar "||"`
redefines a bar-check token, and `"violin1" = { … }` is read back with
`\"violin1"`. At top level the reader skipped the `Kind::Str` token, leaving
`=` and the following music to be swallowed by the next command
(`bar-check-redefine.ly`: 2 reference notes, 0 read). The lexer now builds a
`Kind::Command` token for `\"name"`, the parser handles `Str = value`
assignments at top level, and `parse_command` strips the quotes when looking
up the variable. Failing reduced tests
`affectation_identifiant_ponctuation` and `identifiant_entre_guillemets_echappes`
first. Focused re-measure on the 3 affected `ra` files:
**both non-exact files become exact, no loss** (the third stays exact).

### Generic separation of non-reader faults (reused from §23)

`scripts/oracle/common.py::raison_hors_lecteur` now classifies, once for every
`.ly` oracle, a row whose input is **not** the reader's fault: an unusable
LilyPond MIDI reference, a LilyPond compile failure on the source, or obsolete
syntax that `convert-ly` would rewrite (`setTextCresc`, `sustainDown`, …
the same commands as `parser.rs::obsolete_music_command`). `mesure.py` marks
those rows with `common.marquer_exclu`, so `resume()` drops them from both
numerator and denominator — the same mechanism already used for code 3, no
separate basket duplicated. Selftest covers the three rules. On `ra` the
fresh run (`/tmp/ly-ra-excl`) excludes 2 files with unusable references and
reports **1366 exact of 1724 measured, 0 exact losses** versus the previous
run; `mut`'s obsolete basket is detected by the same helper.

### `ra` next groups — cause 1: `\repeat segno` uses the volta iterator

`scm/music-functions.scm::make-repeat` maps `segno` to `SegnoRepeatedMusic`,
whose `iterator-ctor` is `ly:volta-repeat-iterator::constructor`
(`scm/define-music-types.scm`). The reader mapped it to `Ignored` (`percent`
behaviour): it advanced the cursor by n × the body length, delaying the music
after the repeat. Failing reduced test `repeat_segno_ne_decale_pas_la_suite`
first. Focused re-measure on the 15 `ra` files using `\repeat segno`: **5 N2
gains, 0 exact losses**, 5 files become exact (`repeat-dc-then-*`,
`repeat-dc-unaligned`, `repeat-ds-bar-line`). Three already non-exact D.C.
files lose a few N2 points (their D.C. section is absent from the reference
MIDI); none was exact.

### `ra` next groups — causes 2–4 (measured)

2. **`\makeClusters` staff** (`scm/music-functions.scm::note-to-cluster`):
   `NoteEvent`s become `ClusterNoteEvent`s, which the MIDI performer does not
   play. The staff stays in the model (pivot of every format) with
   `Staff::muted`, and the MIDI writer skips it. Reduced test
   `make_clusters_ne_joue_pas_de_notes` first. On the 3 affected `ra` files the
   import oracle (which reads MusicXML, not our MIDI) shows no change; the
   model/MIDI fidelity fix is verified by the reduced test.
3. **`\chordmode` `min7`/`min9`/`min6`** (`scm/chord-entry.scm:235,252`:
   `min` is a modifier applied to the base chord). The `QUALITIES` table had
   `m7` but not `min7`, so `c:min7` fell back to a major triad. Reduced test
   `accord_chordmode_min7` first. Focused run: **`chord-name-minor.ly` becomes
   exact** (N2 71.43 → 100), no loss.
4. **Grace at the end of a simultaneous voice** (`scm/music-functions.scm::
   volta-spec-music`): when a `<< … >>` branch ends with an ornement and has
   no chord of its own, pending graces were lost. `spawn_at` now transfers
   them to the parent voice's next chord. Reduced test
   `grace_dans_une_voix_simultanee_est_jouee` first. Focused run on the 3
   `\volta`-and-grace files: `volta-spec-after-grace.ly` goes from 3 to 5
   notes, **N1 100 %, n2_grace 100 %** (N2 stays 60 % because the grace
   attacks differ from the raw reference ticks — the grace-aware metric is the
   one that applies here); no exact loss.

The "no distinctive construct" group (94 files) sub-classifies by flag as
`sortie_vide` 26, `attaque_decalee` 22, `notes_supp` 17, `octave` 16,
`notes_manquantes` 9, other 4; by ref/test count: 26 empty test, 20 same count,
10 doubled, 36 other.

After the four causes above, the full `ra` re-measure (`/tmp/ly-ra-final`)
reports **1372 exact of 1724 measured (79.6 %)**, six files gained and none
lost (the five `repeat-dc`/`repeat-ds` files and `chord-name-minor`).
scoretk is 90 files ahead of musicxml2ly on this lot and emits ~45 % fewer
bytes; the `.ly` round trip is reported apart from the reader's own score
(the reader's `mus` figure is not changed by export work).

### Ottava invisible : le crochet se masque, pas l'événement

Dans `testExcludeInvisibleElements.mscx`, la seconde ottava porte
`<visible>0</visible>` : aucun `<octave-shift>` n'est écrit à l'export MusicXML
(`scoretk-musicxml-export/src/ottava.rs::write_start`) mais la hauteur écrite
reste décalée d'une octave (`shift`, qui n'examine pas `visible`). L'écrivain
LilyPond écartait purement et simplement les ottavas invisibles
(`spanners.rs::events`), si bien que la relecture du `.ly` perdait le décalage :
F4 au lieu de F5 aux ticks 36–39.

Références : `scm/define-grobs.scm:2708` (`OttavaBracket`, propriété
`transparent`) et `ly/music-functions-init.ly:1342-1349` (`\ottava` produit un
`OttavaEvent`). L'écrivain émet maintenant `\ottava n` même pour une ottava
invisible, précédé de `\once \override OttavaBracket.transparent = ##t` ; le
lecteur reconnaît cet override (`parser.rs::parse_override`, sur le modèle de
`Rest.transparent`) et conserve `spanner.visible = false` au lieu de poser un
`<octave-shift>` visible à la relecture.

Tests réduits écrits d'abord : `ottava_invisible_porte_quand_meme_le_decalage`
(écriture) et `ottava_invisible_depuis_override` (lecture). Mesure avec deux
binaires frères sur le lot corpus (`--mode model`) : exact 48 → 49,
`testExcludeInvisibleElements.mscx` passe de 49 à 53 notes appariées, aucune
perte ; le lot mscz reste à 155 exacts (aucune régression).

### `\articulate` du MIDI doit être réécrit à l'export

Sur le lot `ly → model → ly`, la première divergence des six fichiers
`duree_seule` est une durée multipliée par 8/7 : par exemple
`22_StMichel.ly` joue des noires de 7/8 dans la référence et de 4/4 dans le
`.ly` écrit. La source enveloppe sa musique dans
`\unfoldRepeats \articulate { … }` (`ly/articulate.ly`) ; le lecteur pose
`Score::playback_articulate`, mais l'écrivain n'émettait ni `\include` ni
l'enveloppe, donc LilyPond rejouait des durées pleines.

L'écrivain émet maintenant `\include "articulate.ly"` et enveloppe le contenu
de `\score` dans `\articulate { … }` quand le modèle porte ce drapeau. Le
correctif ne touche que les 13 sources du lot qui contiennent `\articulate`
(aucune ne l'obtient uniquement par `\include`).

Test réduit écrit d'abord : `articulation_du_midi_ecrite`. Mesure ciblée sur
ces 13 fichiers (`--mode midi`, deux binaires frères) : exacts 1 → 8, n2d en
hausse sur 12 fichiers, aucune nouvelle perte d'exactitude. `Wellerman…`
(reference 183 notes, réécrit 732 : le lecteur fusionne deux `\score` dont le
second transpose) voit son n2d reculer de 5 (183 → 178) : ce fichier était
déjà hors d'atteinte par cette cause structurelle, pas une régression
d'exactitude.
### `\chordmode` sous `\fixed` : la table d'accords suit l'octave

Les noms d'accords de `scm/chord-entry.scm` sont donnés à l'octave 4 dans
`modes.rs::chord_pitches`. LilyPond applique ensuite l'octave de référence
posée par `\fixed` : `ly/parser.yy` (`chord_body`) lit `f` à F3 sous
`\fixed c,` et à F5 sous `\fixed c'` (vérifié sur le MIDI de LilyPond 2.24).
Le lecteur laissait `NoteEvent::midi` inchangé, donc un `ChordNames` sous
`\fixed c,` rejoué une octave trop haut.

L'événement décale maintenant les hauteurs de la table de
`(octave.reference − 3) × 12` demi-tons, comme `pitch.rs::set_reference`
le fait pour les noms ordinaires ; la percussion (table absolue) est
épargnée. Test réduit écrit d'abord :
`accordmode_sous_fixed_decale_les_hauteurs`. L'ancien test
`chordmode_qualite_apres_duree` attendait l'octave non décalée sous le
`\fixed c'` de `lire()` ; son attente est corrigée d'après le MIDI de
LilyPond.

Mesure ciblée sur les 159 fichiers du lot `ly` (exacts + non-exacts,
deux binaires frères, `--mode midi`) : exacts 56 → 59, n2d en hausse sur
7 fichiers, aucune perte. Le lot mscz ne contient aucun `\chordmode` :
son écriture n'est pas modifiée.

### `\articulate` appartient au `\score` qui le porte

`playback_articulate` était collecté globalement pendant l'analyse : un
second `\score` contenant `\articulate` (partition MIDI de répétition)
marquait aussi la partition gravée retenue. L'écrivain LilyPond enveloppait
alors toute la musique dans `\articulate`, raccourcissant les notes d'une
partition qui ne l'était pas (`Wellerman…`, `benoit_ave_maria.ly`).

Comme `playback_unfold_repeats`, le drapeau est maintenant porté par chaque
`Partition` (`parser.rs`) et n'est recopié dans le modèle que pour la
partition effectivement choisie. Test réduit écrit d'abord :
`articulate_du_second_score_ne_contamine_pas_le_premier`.

Mesure ciblée sur les 159 fichiers du lot `ly` (deux binaires frères,
`--mode midi`) : exacts 59 → 60, `benoit_ave_maria.ly` gagné, `Wellerman…`
récupère ses 5 notes perdues par le correctif d'écriture de `\articulate`,
aucune perte.

### `\afterGrace` : écrire le facteur `1`

`ly/music-functions-init.ly:86-97` : `\afterGrace` place l'agrément après
3/4 de la note principale par défaut. Le pivot MuseScore place son agrément
« après » à la fin de la note (`Funicul_Funicul_accordion_solo.mscz` :
agrément `C6` au tick 210, non pas 209,5). L'écrivain émettait
`\afterGrace <note> { <agrément> }` sans facteur ; la relecture posait alors
`played_offset = 3/4`, exporté en `attack="-1"` dans le MusicXML et décalant
l'agrément d'un quart de note.

L'écrivain émet maintenant `\afterGrace 1 <note> { <agrément> }`. Test réduit
écrit d'abord : `apres_agrément_a_la_fin_de_la_note`. Mesure sur le lot mscz
(deux binaires frères, `--mode model`) : exacts 155 → 156,
`Funicul_Funicul_accordion_solo.mscz` gagné, aucune perte ; taille moyenne des
`.ly` inchangée (26622 octets, +4 octets au total).

### Fin de reprise sans début : ne pas inventer `\repeat volta`

`lily/parser.yy` (`bar_line`) : un `\bar ":|"` est une barre de **fin** de
reprise ; sans `|:` correspondant, LilyPond joue la musique une seule fois.
`structure.rs::build` initialisait `section_start = 0` : une fin de reprise
orphaline faisait donc écrire un `\repeat volta 2` depuis la première mesure,
et le `\unfoldRepeats` injecté par l'oracle doublait toute la musique.

`section_start` est désormais un `Option<usize>` : une section n'est construite
que si une barre de début a réellement été vue. Test réduit écrit d'abord :
`fin_de_reprise_sans_debut_ignoree`.

Mesure sur le lot `ly` complet (1 447 fichiers, deux binaires frères,
`--mode midi`) : exacts 1 327 → 1 348, **21 fichiers gagnés, aucune perte**.
La taille moyenne des `.ly` passe de 1 943 à 1 942 octets (−840 au total) :
supprimer les `\repeat` fantômes réduit la sortie.

### Liaison d'un agrément après vers la note suivante

L'écrivain émet le `~` d'une liaison sur la dernière note d'un bloc
d'agrément (`def-grace-function`, `scm/music-functions.scm:1237-1248`).
Le lecteur ne l'appariait que pour les agréments *avant* la note : le `~`
d'un `\afterGrace` était perdu, et la note cible réapparaissait comme une
note distincte (`Il_Postino_accordion_solo.mscz`).

`link_grace_block_tie` est maintenant appelé pour les deux familles ; une
liaison sans cible dans le bloc est marquée par une auto-liaison, résolue
après la pose des mesures sur la prochaine note de même hauteur. Test réduit
écrit d'abord : `liaison_depuis_agrement_apres_vers_note_suivante`.

Mesure sur le lot mscz (deux binaires frères, `--mode model`) : exacts
155 → 155, `Il_Postino` passe de 470 à 469 notes relues (la note surnuméraire
disparaît) mais n'atteint pas l'exactitude ; aucune perte, taille des `.ly`
inchangée. Correctif structurel, sans gain d'exactitude mesuré.

### Notes invisibles de MuseScore : les écrire quand même

`2e1aa03d` a appris au lecteur 3.x/2.x à conserver `<visible>0</visible>`
(`exportmusicxml.cpp::writeNotehead` : une note invisible joue mais n'a pas de
tête). L'écrivain LilyPond filtrait ces notes hors des accords
(`chord.rs::chord_token`, `chord_notes`) : `Angels_Carol__John_Rutter.mscz`
perdait six notes à la relecture après ce commit lecteur.

Les notes visibles et invisibles sont désormais toutes écrites ; seule la
tête de note n'est pas reproduite à l'écran (écart de gravure, pas de musique).
Test réduit écrit d'abord : `note_invisible_ecrite_quand_meme`.

Mesure sur le lot mscz (deux binaires frères, `--mode model`) : exacts
155 → **156**, `Angels_Carol` retrouve ses 1974 notes, aucune perte ; +14
octets sur les `.ly` du lot.

### Oracle : ne déplier les reprises que pour les sources non-`.ly`

L'oracle d'écriture injectait systématiquement `\unfoldRepeats` autour du
`\score` des candidats. Pour une source `.ly`, la référence est le MIDI de
cette même source, compilée sans injection : un `\repeat volta` y joue une
fois, contre deux chez le candidat. C'est l'artefact qui classait à tort 33
fichiers du lot `ly` comme non-exacts (`notes_supp`, `attaque_decalee`).

`injecter_midi(texte, unfold)` et `compiler_lilypond(..., unfold)` prennent
maintenant `unfold = not src.endswith(".ly")`. Les sources MuseScore/MusicXML
gardent l'injection (leur référence MIDI déplie les reprises). Selftest
d'abord : `injecter_midi(src, unfold=False)` n'émet pas de `\unfoldRepeats`.

Mesure sur le lot `ly` complet (1 447 fichiers, deux binaires frères) :
exacts 1 348 → **1 381**, **33 fichiers gagnés, aucune perte**. C'est une
correction de mesure : elle ne change aucun `.ly` produit.

### Oracle : exclure les références MIDI inutilisables

Trente lignes du lot `ly` portaient `drapeaux = ["echec"]` parce que
LilyPond/MuseScore n'avait produit **aucun MIDI de référence** (`lilypond
référence : code …`, `mscore référence : code 40` pour `Sonis`, `Veni,`,
`entre_`). `scripts/oracle/common.py::raison_hors_lecteur` classifie déjà ces
références comme hors lecture dans l'oracle de *lecture*, mais l'oracle
*d'écriture* ne l'appelait pas : ces lignes comptaient comme des échecs du
convertisseur et masquaient les résultats.

Les branches `echec` appellent maintenant `raison_hors_lecteur` et excluent
la ligne des deux dénominateurs ; le préfixe `lilypond référence :` /
`mscore référence :` est reconnu comme référence inutilisable. Selftest
d'abord, étendu au nouveau cas.

Mesure ciblée sur les 30 lignes concernées plus 10 fichiers exacts : **30
exclues, 10 comparées, 0 perte**, scoretk 10/10 contre musicxml2ly 8/10.
C'est une correction de mesure : la taille des `.ly` ne change pas.

### Séquence des restes : 18 mscz et 36 ly (2026-11-22, relevé final)

Chaque fichier non-exact a été différencié par première divergence sur les
notes (`tk.musicxml` contre `scoretk.model.musicxml` en `--mode model`, listes
de notes MIDI en `--mode midi`). Le but était de dégager une cause écrivain
avec gain mesurable ; aucune n'a été trouvée, et voici la preuve documentée.
Aucun `.ly` produit n'a grandi : `\afterGrace 1` +4 octets au total sur le
lot mscz, fin de reprise orphelin −840 sur le lot `ly`, notes invisibles +14
sur mscz (moyennes 26622 et 1943 octets), agrément après et correctifs
d'oracle : 0 octet.

**Lot mscz (18).** `attaque_decalee` 6, `hauteur` 6, `autre` 3 (durée seule),
`notes_supp` 2 (partitions vides, 0 note des deux côtés), 1 entrée exclue.
Les douze premiers groupes sont des **notes surnuméraires à la relecture** :
par exemple `Vivaldi … DeTorrente` a un `A4` de plus au tick 172 dans la
portée 8 (le `A4` long du tick 170 est suivi d'un accord `A4/B4/E5` ; la
référence ne compte pas le premier `A4` de l'accord), `Ave_Verum__Stopford`
a un `E4` de plus au tick 128, et quatre Vivaldi (`Sicut erat` ×3, `Donec
ponam`) ont +3 notes en voix 8. Ce sont des **liaisons inter-voix** : une
liaison dont le départ et l'arrivée sont dans deux voix distinctes ne peut pas
être reconstruite par `Tie_engraver` (`lily/tie-engraver.cc`), qui apparie les
têtes dans une même voix. Le modèle MuseScore porte bien les deux extrémités
(`engraving/dom/chord.h`, `tieForward`/`tieBack`) ; l'écrivain ne peut pas
exprimer cette liaison en LilyPond sans changer la distribution des voix.
La tentative `\repeatTie` (Notation §2.1.4) a été faite puis **revertée** :
le lecteur ne le reconnaissait pas et la sortie grossissait de 35 jetons sur
un seul fichier, sans gain d'exactitude. Les trois `autre`
(`Magnificat__Prt`, `Omni_die_dic_Mariae` ×2) sont un décalage de durée
`g2` lu comme une noire sous `\set Timing.measureLength = #1/2` : retirer
tous les `\set` du `.ly` corrige la première mesure mais décale toutes les
suivantes de 2 temps (vérifié sur `Omni_die_dic_Mariae`) — c'est l'**affectation
du tick du méta-événement** dans `events.rs::event` (`self.cursor` avale la
fin de mesure précédente), cause lecteur, pas écrivain.

**Lot `ly` (36).** Causes lecteur laissées à ly95 (avec les noms de fichiers) :

* `notes_supp` 14 — `22_StMichel_ChantPromo`, `Lassus_In_pace`,
  `Lassus_Alleluja._Dies_sanctificatus`, `Laudate_pueri_Op._39…`,
  `Anerio_Ascendit_Deus`, `Salazar_ALaEstrellaQueEsmalta`,
  `CEtaientTroisJoyeuxRoutiers`, `ChantProvincialDeFlandre`, `lily-*` :
  liaisons inter-voix et `Timing.measureLength`, même mécanisme que mscz.
* `octave`/`notes_manquantes` 13 — `Dechiffrage` (Scheme `$(let …)` non
  évalué : 68 notes de référence contre 16 lues), `Sonis`, `Maiana`,
  `MakilarianII`, `SanctePieX`, `lily-e4b904e2`, `lily-fba5e0bb`,
  `lily-58283073`, `lily-098c2192`, `lily-e92edf38`, `GaiDuguesclin` :
  second `\score` MIDI transposé ou musique Scheme, documenté §7/§14.
* `octave`/`notes_supp` 5 — `01_03_Miguel_AyQueDolor_partition`,
  `02_04_Policier_Un-bon-agent…`, `LaMouetteGrise_EtBerceDansLaBrise`,
  `Kryshachok`, `VolgaVolga` : même transposition de partition MIDI.
* `attaque_decalee` 2 — `01_01_Buenos-dias-Mexico_partition`,
  `Sancta_Maria_KV_273…` : `\partial` et mesures incomplètes, lecture.
* `octave` 1 — `02_01_CommentLeSaisJe` : armure lue à 2 dièses pour
  `\key d \minor` dans le modèle (fifths +2 au lieu de −1), lecteur.
* `duree_seule` 1 — `Angels_Carol__John_Rutter-midi` : la référence est un
  MIDI généré par un autre chemin (durée 4 contre 3,2578), artefact de
  référence.

La seule cause écrivain de ce relevé — la fin de reprise orphelin — avait
déjà été corrigée et mesurée (21 fichiers gagnés). Aucune des 54 divergences
restante n'est imputable au writers : chaque groupe a été ramené à une
liaison inter-voix, un méta `Timing.measureLength` mal daté, une transposition
de `\score` MIDI ou une référence invalide, tous dans le lecteur ou l'oracle.

### `measureLength` persistant (lecteur LilyPond, pas writer)

`lily/timing-translator.cc:170` : `measureLength` est une propriété de
contexte, elle reste en vigueur jusqu'au prochain `	ime` ou `\set`.
`builder.rs::plan_measures_global` ne la conservait qu'à la mesure où le
méta-événement était posé (`events.rs::event` le date au curseur courant,
qui est bien le début de cette mesure) : la mesure suivante retombait à la
durée nominale, et une note la dépassant était coupée en deux noires liées.

`measure_length: Option<Fraction>` est désormais conservé d'une mesure à
l'autre, remis à `None` sur `	ime`. Test réduit d'abord :
`measure_length_sur_la_measure_suivante` (`g2` d'une mesure de 1/2 coupé en
deux noires avant, blanche après).

Mesure sur le lot mscz (deux binaires frères, `--mode model`) : exacts
156 → **159**, `Magnificat__Prt` et les deux `Omni_die_dic_Mariae` gagnés,
aucune perte ; taille des `.ly` inchangée (26 614 octets).

### Liaisons inter-voix : alternatives LilyPond évaluées (preuve conservée)

La Notation Reference §2.1.4 (« Ties ») documente trois sorties de liaison
unilatérale : `\laissezVibrer` (liaison vers rien), `\repeatTie` (liaison
depuis rien, notamment en seconda volta), et `tieWaitForNote = ##t` qui garde
une liaison en attente — mais son exemple (`lilypond-ref` doc, §2.1.4) reste
**dans une seule voix**. Le §7.1.4 (« Hidden notes ») et le renvoi du chapitre
clavier (« Hidden notes can be used to produce ties that cross voices »)
proposent d'ajouter une note cachée dans la voix de départ : cela **ajoute une
tête au modèle relu**, donc une note surnuméraire pour l'oracle, et fait
croître la sortie — contraire à la consigne.

**Tentative mesurée et revertée** : écrire `\repeatTie` (`ly/declarations-init.ly:104`)
sur la note d'arrivée avec un appariement global par hauteur côté lecteur.
Résultat sur le lot mscz : exacts **155 → 155**, `Il_Postino` passe de 470 à
469 notes relues mais n'atteint pas l'exactitude ; `Ave_Verum` (liaison
`E4` voix 2 mesure 32 → voix 1 mesure 33) reste à +1 note ; la sortie
grossissait de 35 jetons `\repeatTie` sur ce seul fichier. Revertée : aucun
gain, croissance de taille. La preuve est donc conservée pour les 12 fichiers
mscz concernés.

**`02_01_CommentLeSaisJe.ly`** : le défaut est dans le **lecteur LilyPond**,
pas dans l'écrivain. `parser.rs:3122` calcule `fifths` avec
`pitch::fifths_of(tonic)` pour les deux modes ; pour une armure mineure il
faut prendre la relative majeure (tonic + 3 demi-tons). `\key d \minor` est
ainsi lu `fifths = +1` (arrondi au modèle +2) au lieu de −1 : un fichier
minimal ne contenant que cette armure produit déjà `<fifths>2</fifths>` et
`\key b \minor` à la réécriture, sans passer par le writer. À signaler pour
ly95 ; l'écrivain se contente de recopier l'armure du modèle
(`pitch.rs::key_signature`).

### Armure LilyPond : le mode modifie le nombre de quintes

`parser.rs:3122` calculait `fifths` avec `fifths_of(tonique)` pour tous les
modes. Or `ly/scale-definitions-init.ly` donne les altérations de chaque
gamme : `minor`/`aeolian` a trois bémols de plus que le majeur, `dorian`
deux, `phrygian` quatre, `locrian` cinq ; `lydian` un dièse de plus,
`mixolydian` un bémol de plus. `\key d \minor` devait donc donner −1
(relative majeure fa) et non +2 (ré majeur). Le writer se contentant de
recopier l'armure du modèle (`pitch.rs::key_signature`), l'erreur venait
bien du lecteur.

`pitch.rs::mode_fifths_offset` ajoute le décalage du mode (0 pour
`major`/`ionian`) à la tonique. Tests réduits d'abord, pour le mineur et
deux modes d'église (`dorian`, `mixolydian`) plus les six autres modes.

Mesures (deux binaires frères) :

* import `ra` : 1560/1724 → 1560/1724, import `mut` : 1186/1316 → 1186/1316,
  aucune perte ni gain (la comparaison porte sur les notes MIDI).
* export LilyPond lot `ly` : 1389 → 1389 exacts, aucune perte.
* export LilyPond lot `mscz` : 158 → 158 exacts, aucune perte.
* export MusicXML lot `ly` : non mesurable ici — le `mscore` disponible
  refuse les sources `.ly` (code 40), 0 fichier converti des deux côtés.
* taille des `.ly` : inchangée (19 990 octets sur `ly`, 26 614 sur `mscz`).

Le défaut ne changeait pas les notes des corpus mesurés (armure = décoration,
les hauteurs écrites sont explicites), mais corrige l'armure du modèle et la
réécriture `\key` pour 66 sources du lot `ly` et le fichier signalé
`02_01_CommentLeSaisJe`.

## `ra` — tournée après les remarques de relecture

Les trois remarques ont été corrigées dans un seul commit : commentaire
contradictoire retiré dans `events.rs::spawn_at`, `min` traité comme
modificateur leading dans `modes.rs` (`min11`, `min13`, `min7.9` couverts, test
`accord_chordmode_min11` ajouté, ainsi que la règle du 11 implicite retiré de
`c:13` mais gardé dans `c:min13`, vérifiés sur le MIDI de LilyPond 2.24), et
`\makeClusters` détecté sur tout l'intervalle de jetons de la musique de la
portée, y compris avec une référence de variable. Le modèle n'ayant pas de
mute par voix, une portée partiellement clusterisée est muette entièrement :
limite documentée ici.

Trois causes supplémentaires, chacune avec un test réduit échouant d'abord :

1. **Accolades après une commande de markup** (`lily/parser.yy:4160-4165`,
   `markup_braced_list`) : laissées telles quelles, `{ "0" "0" }` devenait une
   partition parasite et la vraie portée était écartée comme « partition
   supplémentaire ». Seules les commandes dont la signature accepte une liste
   entre accolades (`scm/define-markup-commands.scm` : `line`,
   `right-column`, `rhythm`, `note`…) consomment l'accolade ; `\foo` sans
   argument et `\etc` laissent la musique (`markup-invalid-stencil.ly`,
   `textetc.ly`). Test `liste_markup_entre_accolades_ne_cree_pas_une_partition_vide`.
   Mesure ciblée sur 201 fichiers avec `\markup` : **3 gains exacts, 0 perte**.
2. **`#name` dans `#{ … #}`** (`lily/parser.yy:3221-3225`, `embedded_scm`) :
   une variable assignée en syntaxe LilyPond (`music = \relative { … }`) est
   une variable Scheme insérable par `#music` dans le corps de
   `\testVisibility`. Test `interpolation_scheme_dune_variable_musicale`.
   Mesure ciblée : **6 gains exacts** (les six `bar-number-visibility`),
   0 perte ; sur les 51 fichiers `sortie_vide`, 15 deviennent exacts.
3. **`\skip <musique>`** (`ly/music-functions-init.ly:2267-2274`,
   `duration-or-music?`) : la musique argument est parcourue en silence, ses
   durées avançant le temps (`skip-music.ly`). Test
   `skip_avec_musique_ne_joue_pas_cette_musique`. Mesure ciblée sur 26 fichiers
   avec `\skip` : **1 gain exact** (`skip-music.ly`), 0 perte.

La mesure complète après ces trois causes (`/tmp/ly-ra-final2`) donne
**1397 exacts sur 1724 mesurés (81,0 %)**, soit **25 fichiers gagnés et aucune
perte** par rapport aux 1372 précédents. Après le rebase sur les commits de
`master` sur `measurePosition`, les noms de contexte et la basse d'accord
explicite, la mesure finale (`/tmp/ly-ra-final3`) donne **1402 exacts (81,3 %)**,
cinq fichiers de plus gagnés et aucune perte. Il reste 33 `sortie_vide`, 108
`attaque_decalee`, 62 `octave`, 62 `notes_manquantes`, 55 `notes_supp`,
4 `transposition`, 3 `autre`, 1 `echec`.

## `ra` — attaques décalées (lot de 108)

Sous-classification des 108 fichiers `attaque_decalee` de la mesure
`/tmp/ly-ra-final3` par constructeur dominant (première divergence, le plus
gros groupe d'abord) : `grace` 36, `markup`/`tweak` 22, `repeat`/`volta` 20,
sans constructeur distinctif 20, `book`/`score` 5, `transpose`/`clef` 2,
voix 2, tuplet 1 (44 fichiers contiennent en fait un ornement). La première
attaque diverge dès le début pour 13 fichiers seulement ; pour les 95 autres
elle est juste et l'écart apparaît plus tard.

Quatre causes, test réduit échouant d'abord pour chacune :

1. **`\maxima`** (`scm/define-music-display-methods.scm:114` : log2 = −3,
   donc 8 rondes) était lu comme un `\longa` (4 rondes). Le facteur ×2 est
   porté par le multiplicateur, y compris pour la durée par défaut. Test
   `maxima_vaut_huit_rondes`. Mesure ciblée (33 fichiers avec durées longues) :
   **3 gains exacts**, 0 perte.
2. **Identifiant de durée Scheme** (`lily/duration-scheme.cc:53-99`) :
   `nom = #(ly:make-duration 0 0 1/2)` puis `c'\nom` porte un facteur de
   compression 1/2 sur une ronde, et fixe la durée par défaut suivante. Test
   `identifiant_de_duree_compresse`. Même mesure : **1 gain exact**
   (`duration-identifier-compressed.ly`), 0 perte.
3. **`\augmentum`** (`ly/ancient-init.ly:124-130` : `shift-duration-log expr 0 1`)
   ajoute une mora (un point) à chaque durée de l'expression suivante, y
   compris aux notes nues (leur durée est alors prise au parseur). Test
   `augmentum_ajoute_un_point_a_la_duree`. Mesure ciblée : **2 gains exacts**
   (`augmentum`, `mensural-ligatures`), 0 perte.
4. **`\pitchedTrill`** (`ly/music-functions-init.ly:1707-1712`) : le second
   argument est la tête de note entre parenthèses, gravée mais non jouée.
   Test `pitched_trill_ne_joue_pas_la_note_secondaire`. Mesure ciblée (5
   fichiers) : **3 gains exacts**, 0 perte.

La mesure complète finale (`/tmp/ly-ra-final4`) donne **1410 exacts sur 1724
(81,8 %)**, huit fichiers gagnés et aucune perte par rapport aux 1402. Restent
101 `attaque_decalee`, 62 `octave`, 62 `notes_manquantes`, 49 `notes_supp`,
33 `sortie_vide`, 4 `transposition`, 3 `autre`, 1 `echec`.

## `ra` — notes manquantes, octave et remarque sur `parse_duration`

Remarque de relecture vérifiée : aucun appelant de `parse_duration` ne
distingue `None` de `Some(1)` (tous passent par `map_or(ticks, |v| ticks*v)`
ou `unwrap_or(Fraction::ONE)`). La fonction renvoie donc de nouveau `None`
pour un facteur implicite de 1, sans changement de comportement.

Quatre causes, test réduit échouant d'abord pour chacune :

1. **`c:13.11`** (`chord-name-11-entry.ly`) : `scm/chord-entry.scm:67-78,137`
   traite les degrés après un point comme des ajouts et rend le 11 explicite.
   `chord_pitches` les ajoute désormais à la base et ne supprime plus le 11
   de `:13` quand il est explicite. Test unitaire `noms_daccords`.
2. **`\absolute`** (`ly/music-functions-init.ly:32-36`, Notation Reference
   §1.1.2) : la musique est soustraite au `\relative`/`\fixed` environnant,
   et la référence de la suite reste celle d'avant le bloc. Test
   `absolute_cache_la_musique_du_relative_entourant`. Gain : `relative.ly`.
3. **Échappements Scheme dans `ly:parser-include-string`**
   (`lily/lily-lexer.cc:302-315`) : `\\` dans la chaîne vaut un seul
   backslash. Sans décodage, `\\relative` n'était pas reconnu et les hauteurs
   tombaient à l'octave absolue. Test existant `parser_include_string_injecte_la_musique`
   corrigé vers la syntaxe réelle. Gain : `include-string.ly`.
4. **`\include \identifiant`** (`lily/lexer.ll:340-350`) : un identifiant
   assigné à une chaîne sert de nom de fichier. Le préprocesseur retient ces
   assignations ; `\version "…"` inclus au milieu d'une musique ne doit pas
   avaler la durée nue suivante. Tests `include_avec_identifiant_de_chaine` et
   `version_dans_un_include_musical_garde_la_duree_nue`. Gain :
   `include-identifier.ly`.

Tentative non retenue : faire hériter à la seconde voix de `<< … \\ … >>` la
référence relative de la première (`lily/music-sequence.cc:143-158`). Les
voix sont construites séparément par `builder.rs` et l'état d'octave est
restauré par le `Context::Relative` englobant : le corriger demanderait de
restructurer `build_voices`, hors périmètre. Test réduit retiré.

Mesure ciblée sur 46 fichiers `\relative`/accords : **3 gains exacts, 0 perte**.
Mesure ciblée sur 162 fichiers avec `\include` : **2 gains exacts, 0 perte
exacte**. Les deux baisses N2 sur `chord-names-languages*.ly` viennent des
commits de `master` récupérés au rebase (`\simultaneous`, `q`, octave-shift)
et non de ces causes : un bâti à `4a2c244d` sans mes modifications donne le
même résultat.

La mesure complète finale (`/tmp/ly-ra-final5`) donne **1439 exacts sur 1724
(83,5 %)**, 29 fichiers gagnés et aucune perte par rapport aux 1402. La
majorité des gains de grace viennent de la mise à jour de l'oracle sur
`master` (`n2_grace` désormais admis dans `exact`), pas du lecteur. Restent 81
`attaque_decalee`, 61 `octave`, 60 `notes_manquantes`, 47 `notes_supp`, 29
`sortie_vide`, 4 `transposition`, 3 `autre`, 1 `echec`.

## `ra` — notes manquantes et octave (lot 84,6 %)

Quatre causes, test réduit échouant d'abord pour chacune, mesurées par rapport
au bâti de `master` `00fee24d` (rebasé, sans ces corrections) :

1. **Octave relative enchaînée** (`seconde_voix_relative_heritant_de_la_premiere`) :
   `lily/music-sequence.cc:143-158` enchaîne les branches d'un `<< … >>`, et
   `lily/relative-octave-check.cc` règle les marques d'octave. L'état relatif
   final est mémorisé par l'interpréteur avant que le `Context::Relative`
   englobant ne le restaure, puis transmis à la voix/portée suivante.
   Affinage : un drapeau `chain` sur `Context::Relative`, posé seulement quand
   le `\\relative` enveloppait directement un `<< … >>`, limite l'héritage aux
   branches distribuées d'un même `<< >>` ; un `\\relative` propre à une
   branche ou une portée sans `\\relative` repart en octave absolue.
2. **`\\cueDuring` / `\\cueDuringWithClef`** (`ly/music-functions-init.ly:56-60,545-574`) :
   la musique déclarée par `\\addQuote` est jouée à la place des silences de
   l'argument principal, tronquée à sa durée. Dix fichiers de citation gagnent
   des notes (les octaves restantes sont un autre sujet).
3. **`\\*` et `\\%`** (`ly/music-functions-init.ly:742-751`) : raccourcis de
   `\\repeat unfold` et `\\repeat percent`. Gain : `repeat-short-function-names`.
4. **`\\textMark` / `\\textEndMark`** (`ly/music-functions-init.ly:2365-2373`) :
   un seul argument de markup ; la durée nue suivante reste de la musique. Un
   argument `\\markup` sans accolades est lu par `parse_markup_text`, pas
   `skip_braced`, sinon `text-end-mark-break-visibility` et
   `script-custom-definition` régressaient. Gain : `text-mark-marklengthon`.

Mesures ciblées : 5 gains exacts / 2 pertes exactes pour les causes 1–2 avant
affinage, ramenées à 17 gains / 0 perte après restriction du drapeau `chain` ;
2 gains / 0 perte pour les causes 3–4 après correction du markup.

La mesure complète (`/tmp/ly-ra-final7`) donne **1459 exacts sur 1724 (84,6 %)**,
**18 fichiers gagnés et aucune perte** par rapport au bâti `master` `00fee24d`
(1441). Restent 88 `attaque_decalee`, 54 `notes_manquantes`, 52 `notes_supp`,
38 `octave`, 28 `sortie_vide`, 4 `transposition`, 1 `autre`, 1 `echec`.

## `ra` — cue, identifiants et alternatives (lot 86,0 %)

Remarque de relecture traitée : les notes insérées par `\cueDuring` portent
désormais `Note::small = true` dans le modèle (`NoteEvent::small` dans le
lecteur, propagé par `build_note`), afin que les écrivains gravent des notes
de cue (`MuseScore exportmusicxml.cpp:4181-4193`, `size="cue"`) au lieu de
notes ordinaires. Test `cue_during_joue_la_citation` complété.

Quatre causes, test réduit échouant d'abord, mesurées face à `/tmp/ly-ra-final7` :

1. **Identifiant Scheme numérique** (`num = #3`, `lily/parser.yy`) : le nombre
   sert d'argument à `\repeat unfold \num`. Test
   `identifiant_numerique_scheme_dans_repeat_unfold`. Gain : `identifiers`.
2. **Identifiants pointés** (`violin.1 = { … }`, `identifier-alists.ly`) : les
   noms de variables alistes sont reconnus à l'assignation et à l'appel. Test
   `identifiant_pointedans_un_nom_de_variable`. Gain : `identifier-alists`.
3. **`\alternative` dans le corps d'une reprise**
   (`\repeat volta n { … \alternative { … } }`) : retirer le contexte
   `Alternative` du corps pour l'enregistrer comme endings. Tests
   `alternative_a_l_interieur_du_corps_de_reprise` ; gains sur les
   `repeat-*-count-one`, `bar-number-*alternative*`, etc.
4. **`\quoteDuring`** (`ly/music-functions-init.ly:2023-2032`) : même
   sémantique MIDI que `\cueDuring`, sans direction ni armure. Test
   `quote_during_joue_la_citation`.

Mesures ciblées : 2 gains / 0 perte (causes 1–2), 22 gains / 0 perte (causes
3–4). La mesure complète (`/tmp/ly-ra-final8`) donne **1482 exacts sur 1724
(86,0 %)**, 23 fichiers gagnés et aucune perte par rapport aux 1459. Restent
91 `attaque_decalee`, 53 `notes_supp`, 35 `octave`, 29 `notes_manquantes`, 25
`sortie_vide`, 5 `transposition`, 4 `autre`, 1 `echec`.

## `ra` — sortie_vide (lot 86,3 %)

Quatre causes, test réduit échouant d'abord, mesurées face à `/tmp/ly-ra-final8` :

1. **`\with \identifiant`** (`lily/parser.yy:1625`, `ly/context-mods-init.ly:52`)
   : l'identifiant de modification de contexte est consommé seul ; le bloc
   suivant reste la musique de la portée. Test
   `with_suivi_d_un_identifiant_de_contexte`. Gain : `remove-empty-context-mod`.
2. **Armure à numérateur nul** : LilyPond avertit « unsupported time
   signature » et garde la mesure par défaut. Test `time_numerateur_nul_ignore`.
   Gain : `time-numerator-zero`.
3. **Identifiant Scheme appelé par `\nom`** (`#(define nom #{ … #})` est un
   identifiant LilyPond, pas seulement `$nom`/`#nom`). Test
   `variable_scheme_appellee_par_backslash`. Gain : `utf-8-scheme`.
4. **Enveloppe avant le bloc de `\parallelMusic`** (`\parallelMusic vI,vII
   \fixed c' { … }`) : l'enveloppe est appliquée à chaque voix. Test
   `parallel_music_avec_enveloppe_fixe`. Gain : `parallelmusic-partial`.

Mesure ciblée sur les 25 `sortie_vide` : **3 gains exacts, 0 perte** ; la
mesure complète (`/tmp/ly-ra-final9`) donne **1487 exacts sur 1724 (86,3 %)**,
5 gagnés (dont `trill-spanner-pitched-forced`) et aucune perte.

Restaient 21 `sortie_vide` ; chacune est soit un écart de lecteur documenté,
soit une exclusion justifiée :

- **8 modèles `\include "satb.ly"` / `"ssaattbb.ly"`** (`satb-template-*`,
  `ssaattbb-template-*`) : modèles installés du répertoire `ly/` de LilyPond
  que le résolveur ne peut ouvrir ; cf. §11 et la remarque de §22 sur
  `satb-template-with-keyboard-reduction`. Exclusion justifiée, pas une faute
  du lecteur.
- **4 fichiers `*-accidental-glyphs` / `alteration-glyphs`** : le fichier inclus
  `accidental-glyphs-test.ily` définit un `define-scheme-function` qui appelle
  `add-score` ; le lecteur n'évalue pas Scheme. Écart de lecteur (t40).
- `midi-key-signature.ly` : `\key fa \bayati` défini dans `ly/arabic.ly`,
  gamme non tempérée hors du sous-ensemble du lecteur. Écart de lecteur.
- `music-map-articulations.ly`, `markup-map.ly`, `scheme-list-splicing.ly` :
  `$(music-map …)`, `\markupMap` et `#@(map …)` requièrent une évaluation
  Scheme. Écart de lecteur.
- `quote-tie.ly` : citation avec `\addQuote "cueI" { \cueI }` et
  `\cueDuring` dans deux voix ; la musique citée reste vide. Écart de lecteur.
- `measure-counter-compressed-mmrest.ly`,
  `measure-counter-alternative-numbering.ly`, `bar-line-caesura-double.ly`,
  `note-head-style.ly` : commandes purement gravures (`\compressMMRests`,
  `Measure_counter_engraver`, `\caesura`, styles de têtes) qui masquent la
  musique au lecteur. Écarts de lecteur.

## `ra` — sous-classification par première divergence (lot 86,7 %)

Script `scripts/oracle/lilypond/subdivise.py` (`--jobs`, défaut
`min(2, cpu_count)`) : pour chaque écart, il compare les couples
(hauteur, attaque) du MIDI de référence et de notre MusicXML, repère le premier
tick qui diffère, puis relève le constructeur de la source autour de ce tick
(repli sur le constructeur dominant de la source si l'extrait ne porte que
l'en-tête). Sur les 64 fichiers `octave`/`notes_manquantes` du lot 86,3 % :

| Constructeur à la première divergence | Fichiers | Exemple |
| --- | ---: | --- |
| markup/tweak | 13 | `approximate-pitch-head-direction-tweak.ly` |
| voices | 11 | `autochange-inside-scale-durations.ly` |
| repeat/volta | 9 | `allow-break.ly` |
| transpose/clef/key | 9 | `autochange-clefs.ly` |
| relative | 9 | `monochords.ly` |
| scheme | 6 | `bar-line-define-bar-glyph.ly` |
| tuplet | 3 | `metronome-mark-broken-bound.ly` |
| cue/quote | 3 | `quote-cyclic.ly` |
| grace | 1 | `bend-after-start-in-grace.ly` |
| include | 1 | `chord-names-languages2.ly` |

Quatre causes visant les groupes `repeat/volta` et `voices`, test réduit
échouant d'abord, mesurées face à `/tmp/ly-ra-final9` :

1. **`\repeat unfold` avec alternatives** (`lily/parser.yy` REPEAT ALTERNATIVE)
   : les alternatives se répartissent sur les passages, la première étant
   multipliée s'il y en a moins que de reprises. Test
   `repeat_unfold_avec_alternatives`. Gain : `repeat-unfold`.
2. **`\repeat … \etc`** (`repeat-etc.ly`) : fonction partielle, le corps est
   fourni à l'appel de la variable (`partial` sur `Context::Repeat`). Test
   `repeat_avec_etc_applique_le_corps_a_l_appel`. Gain N2 sur `repeat-etc`.
3. **`\autoChange`** (`ly/music-functions-init.ly:278-303`) : le MIDI joue
   toutes les notes des deux portées ; les hauteurs sont finales et un
   `\relative` extérieur ne les recalcule pas. Test
   `auto_change_joue_toutes_les_notes`. Gains : `autochange-clefs`,
   `autochange-inside-scale-durations`, `autochange-turning-pitch`.
4. **Événements sans argument avalant la musique** — `\startMeasureCount` /
   `\stopMeasureCount` (`ly/spanners-init.ly:36-37`) et `\compressMMRests`
   (`ly/music-functions-init.ly:452-459`). Test
   `measure_count_sans_argument_ne_mange_pas_la_duree`. Gains :
   `measure-counter-event-order`, `measure-counter-alternative-numbering`,
   `measure-counter-compressed-mmrest`.

Mesures ciblées : 1 gain exact / 0 perte (causes 1–2), 3 gains / 0 perte
(causes 3–4). La mesure complète (`/tmp/ly-ra-final11`) donne **1495 exacts sur
1724 (86,7 %)**, 8 fichiers gagnés et aucune perte. Restent 90
`attaque_decalee`, 54 `notes_supp`, 34 `octave`, 24 `notes_manquantes`, 19
`sortie_vide`, 5 `transposition`, 4 `autre`, 1 `echec`.

## `ra` — sous-classification de `attaque_decalee` et `notes_supp` (lot 87,6 %)

Même script `scripts/oracle/lilypond/subdivise.py`, sur les 144 fichiers
`attaque_decalee`/`notes_supp` du lot 86,7 % :

| Constructeur à la première divergence | Fichiers | Exemple |
| --- | ---: | --- |
| markup/tweak | 31 | `accidental-styles.ly` |
| repeat/volta | 24 | `auto-beam-exceptions.ly` |
| relative | 22 | `accidental-single-double.ly` |
| grace | 17 | `after.ly` |
| voices | 15 | `accidental-placement-samepitch.ly` |
| transpose/clef/key | 14 | `accidental.ly` |
| sans-construct | 8 | `chord-X-align-on-main-noteheads.ly` |
| score/book | 6 | `chord-changes-alternative.ly` |
| scheme | 6 | `collision-merge-dots-hidden.ly` |
| tuplet | 3 | `alignment-order.ly` |
| cue/quote | 2 | `quote-cue-during.ly` |
| lyrics | 1 | `lyrics-includegraces.ly` |

Avec le tableau précédent, les plus grands groupes communs sont
`markup/tweak` (44), `repeat/volta` (33), `relative` (31), `voices` (26).

Quatre causes visant `voices` et `markup/tweak`, test réduit échouant d'abord,
mesurées face à `/tmp/ly-ra-final11` :

1. **`\partCombine` au niveau supérieur** — la commande étant absente de
   `starts_music_command`, ses deux branches devenaient deux partitions dont
   une seule était retenue. Test
   `part_combine_au_niveau_superieur_joue_les_deux_voix`. Gain :
   `duration-line-and-partCombine` et N2 sur `part-combine(-global)`.
2. **`\new NoteNames`** (`lily/note-name-engraver.cc`) : contexte de gravure
   sans performer MIDI ; sa musique n'est pas jouée. Test
   `note_names_contexte_grave_mais_non_joue`. Gain : `accidental-tie`.
   `ChordNames` reste joué (vérifié avec LilyPond 2.26).
3. **`FretBoards` et `NullVoice`** (`ly/performer-init.ly:229`) : même règle
   que `NoteNames`, étendue à ces deux contextes. Test
   `fret_boards_contexte_grave_mais_non_joue`. Gain : `fret-boards`.
4. **Unissons de `\partCombine`** — deux voix strictement identiques ne sont
   jouées qu'une fois. Test `part_combine_voix_identiques_fusionnees` ; évite
   la régression `part-combine-slur`.

Mesure ciblée `voices`/`markup/tweak` : gains N2 sans perte. La mesure
complète (`/tmp/ly-ra-final13`) donne **1510 exacts sur 1724 (87,6 %)**, 15
fichiers gagnés et aucune perte par rapport aux 1495. Restent 84
`attaque_decalee`, 48 `notes_supp`, 32 `octave`, 22 `notes_manquantes`, 19
`sortie_vide`, 5 `transposition`, 4 `autre`, 1 `echec`.

## `ra` — remarque NullVoice et causes Scheme (lot 87,7 %)

Remarque de relecture traitée (bloquante) : `NullVoice` n'était pas jouée mais
était supprimée du modèle, ce qui perdait les paroles alignées par
`\lyricsto "nom"`. `ly/performer-init.ly:225-234` et `ly/engraver-init.ly`
en font un `Performer_group` sans `Note_performer` : sa musique est invisible
et silencieuse, mais sa trame porte les paroles. Le lecteur crée désormais une
portée `Staff::muted` dont la voix porte le nom du contexte ; les notes y sont
marquées `Note::visible = false` via `Context::Invisible`. Test
`nullvoice_garde_les_paroles_et_est_muette_dans_le_midi` : paroles présentes
dans le MusicXML, quatre notes jouées (la voix mélodique seulement). Sur le
lot, `lyric-combine-nullvoice.ly` garde ses quatre `<lyric>` avant et après.

Trois causes, test réduit échouant d'abord, mesurées face à
`/tmp/ly-ra-final14` :

1. **`#(ly:parser-set-note-names (ly:assoc-get '<lang> language-pitch-names))`**
   (`note-names.ly`, `lily/parser-scheme.cc`) : la langue des noms de notes change
   en cours de musique. Test `scheme_parser_set_note_names_change_la_langue`.
2. **`$(music-map … #{ … #})`** (`music-map-articulations.ly`) : la carte
   transforme des événements (nuances, articulations) mais garde hauteurs et
   attaques ; le corps embarqué est lu. Test `scheme_music_map_joue_sa_musique`.
3. **`\markupMap`** (`ly/music-functions-init.ly:1173`) : chemin, fonction de
   markup et musique ; le consommer comme commande inconnue faisait disparaître
   la musique. Test `markup_map_ne_consomme_pas_la_musique`.

Mesure ciblée : 2 gains exacts / 0 perte. Après rebase sur les commits de
`master` (`\set Timing.beatStructure`, export MusicXML d'un accord vide), la
mesure complète (`/tmp/ly-ra-final16`) donne **1507 exacts sur 1724 (87,4 %)**.
Rapportée au bâti `master` `7d31c559` (`/tmp/ly-ra-base16`, 1505 exacts), elle
montre **2 gains** (`markup-map`, `music-map-articulations`) et **aucune perte**
: les autres écarts viennent des commits de `master`, pas de ces causes. Restent
84 `attaque_decalee`, 48 `notes_supp`, 32 `octave`, 22 `notes_manquantes`, 17
`sortie_vide`, 5 `transposition`, 4 `autre`, 1 `echec`.

## `ra` — récupération des pertes par le lecteur (lot 88,9 %)

Le passage de 1510 (avant fusion) à 1505 sur `master` `7d31c559` venait de
`e2d7a276` (writer MusicXML : court-circuit des accords sans note, correctif
de `Symphony5_1/2`). Une tentative de correction côté writer (ne court-circuiter
un accord vide que s'il porte une articulation ou si sa portée n'a aucune note)
a été **rejetée** : elle compensait un artefact de lecture dans l'écrivain,
faisait dépendre le comportement des articulations (sans base MusicXML ni
MuseScore) et introduisait un balayage quadratique `staff_has_notes`. La règle
uniforme a été restaurée : un accord sans note n'écrit rien et n'avance pas le
curseur ; c'est `move_to_tick` qui émet le `<forward>` (`exportmusicxml.cpp`).

Toutes les causes réelles étaient donc dans le lecteur :

1. **Valeur `\markup` d'`\override` non consommée** : son `{16}` devenait un
   accord vide. `parse_override` consomme maintenant la valeur `\markup`.
2. **Durée nue après une liste de markup** (`trivial-path.ly`) :
   `parse_markup_text` s'arrête à la fin de la liste au lieu d'avaler le `4`
   suivant. Test `duree_nue_apres_markup_avec_path_est_jouee`.
3. **Commandes sans argument** (`oneVoice`, `autoBeamOff`, `autoBeamOn`,
   `noBeam`, `beamUp`, `beamDown`, `beamNeutral` ; `ly/property-init.ly:140,970`,
   `ly/declarations-init.ly:78`) : la durée nue qui suit reste de la musique.
   Test `commandes_sans_argument_gardent_la_duree_nue`.
4. **`NullVoice`** : portée muette **et** cachée (`Staff::visible = false`,
   `print-object="no"` à l'export). Test
   `nullvoice_garde_les_paroles_et_est_muette_dans_le_midi`.
5. **Réglages de mesure laissés dans le flux** : `\overrideTimeSignatureSettings`
   (4 arguments, `ly/music-functions-init.ly:1351`), structure de temps de
   `	ime`/`	imeAbbrev` avant l'armure (`ly/property-init.ly`), liste de numéros
   `olta`, `\musicLength` comme valeur de propriété, et
   `\contextPropertyCheck` (`ly/music-functions-init.ly:462`) dont la valeur
   Scheme peut être un nombre nu (`Staff.meterScalingFactor 1`). Chacun
   laissait une durée ou un accord vide qui décalait toutes les notes. Un test
   réduit par cause a été ajouté.

Récupérés : `break-alignment-anchor-average-clamp`, `markup-note-grob-style`,
`multiple-time-sig-settings`, `partial-polymetric`,
`polymetric-time-warning-measure-length-irregular`, `spacing-space-to-barline`,
`time-signature-settings-by-staff`, `volta-bracket-warning-start-conflicting-numbers`,
sans perdre `rhythm-sets-default-duration` ni `span-bar-no-bar`. Les quatre
fichiers `mut` protégés (`Chop-28-19`, `Symphony5_1/2`, `No03_Albumblatt`)
restent exacts (références reconstruites depuis `refs27`, `--refs-prefix ''`,
`/tmp/ly-mut-final2` : 4/4 exacts, N1 et N2 à 100 %).

Mesure complète (`/tmp/ly-ra-final`) : **1532 exacts sur 1724 (88,9 %)**, 27
gains et aucune perte par rapport au bâti `master` `7d31c559` (1505). Restent
71 `attaque_decalee`, 46 `notes_supp`, 28 `octave`, 21 `notes_manquantes`, 17
`sortie_vide`, 5 `transposition`, 4 `autre`.

## Sous-classification des 192 écarts `ra` par première divergence

`subdivise.py` (sur `/tmp/ly-ra-final`, 1724 fichiers, 192 non exacts) classe
chaque écart par le constructeur relevé autour du premier tick divergent. Le
classement est croisé avec le drapeau principal de `mesure.py` :

| Constructeur | Total | attaque_decalee | notes_supp | octave | notes_manquantes | sortie_vide |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| markup/tweak | 38 | 18 | 17 | 7 | 8 | 1 |
| relative | 30 | 10 | 16 | 3 | 4 | 4 |
| repeat/volta | 21 | 9 | 14 | 0 | 5 | 0 |
| voices | 20 | 3 | 11 | 6 | 2 | 1 |
| grace | 18 | 16 | 4 | 0 | 1 | 0 |
| transpose/clef/key | 17 | 4 | 10 | 8 | 2 | 0 |
| scheme | 11 | 3 | 2 | 2 | 4 | 1 |
| cue/quote | 9 | 0 | 2 | 0 | 3 | 1 |
| include | 7 | 1 | 0 | 0 | 0 | 5 |
| tuplet | 6 | 3 | 1 | 1 | 3 | 0 |
| score/book | 5 | 1 | 5 | 0 | 0 | 0 |
| sans-construct | 5 | 2 | 4 | 0 | 0 | 0 |
| lyrics | 5 | 1 | 0 | 0 | 0 | 4 |

Les colonnes se recoupent (un fichier porte plusieurs drapeaux). Par forme
d'écart : 44 fichiers n'ont **que** des notes supplémentaires (toutes les notes
de référence sont présentes), 28 que des notes manquantes, 119 sont mixtes.
Les sous-groupes sont traités par taille décroissante ; les exclusions
(artefacts d'oracle, tests de mise en page, hors sous-ensemble) sont justifiées
au fur et à mesure pour garder un dénominateur honnête.

### Causes 1–4 : valeurs de propriété et événements sans argument

Quatre causes, chacune avec un test réduit failing-first et une mesure :

1. `ly/declarations-init.ly:61-64` : `break`, `noBreak`, `allowBreak` sont des
   `LineBreakEvent` sans argument ; une durée nue qui suit reste de la musique
   (`allow-break.ly` perdait trois notes par répétition).
2. `ly/music-functions-init.ly:348-354` : `beamExceptions` est une fonction
   Scheme à argument musical, valeur de `\set` ; son argument n'est pas joué
   (`auto-beam-exceptions.ly` avait 16 accords vides de 32e).
3. `ly/string-tunings-init.ly:29-37` : `stringTuning` convertit un accord en
   liste de hauteurs pour `\set stringTunings` ; l'accord n'est pas joué (3
   fichiers `tablature-*`).
4. `ly/declarations-init.ly:68-72`, `ly/music-functions-init.ly:421` : `fine`,
   `startStaff`, `stopStaff`, `breathe`, `caesura` sont des événements sans
   argument ; la durée nue qui suit reste de la musique.

Mesures : après 1–2, **1535 exacts** (3 gains : `allow-break`,
`auto-beam-exceptions`, `tablature-slurs-with-beams`) ; après 3–4, **1538
exacts** (3 gains : `tablature-micro-tone`, `tablature-open-string-chord`,
`tablature-string-tunings`). Aucune perte à chaque étape. Total **1538/1724
(89,2 %)**, +6 sur le bâti à 1532.

### Exclusions et causes hors sous-ensemble (dénominateur honnête)

Sur les 186 fichiers restants, 34 sont explicitement justifiés et ne doivent
pas être comptés comme des défauts du lecteur :

- **11 artefact d'oracle** : `\new ChordNames`. Les références MIDI de `ra`
  n'ont pas les notes de la grille, alors que LilyPond 2.26 les joue (vérifié
  en compilant `chord-changes.ly` avec `nix-shell -p lilypond`, et par les tests
  `grille_daccords_jouee_dans_le_midi` / `chordmode_dans_une_portee_joue_les_accords`).
  Aucune source `lily/*.cc` ne documente le comportement ; l'écart vient de la
  version ayant produit les `ly.midi` (le README de l'oracle recommande de ne
  pas faire confiance à un arbre mélangé). Changer le lecteur casserait ces
  tests et la fidélité 2.26 : fichiers non corrigeables contre cet oracle.
- **7 hors sous-ensemble** : `\include "satb.ly"` / `"ssaattbb.ly"` définissent
  les commandes `\SATB`/`\Piano` et le `\score` lui-même (`ly/satb.ly:157-215`) ;
  ils sont ignorés par `FICHIERS_LILYPOND` (`include.rs:74-75`), donc aucune
  musique n'est lue.
- **10 hors sous-ensemble** : sources dont la musique est produite par
  `define-music-function` / `define-scheme-function` Scheme, non interprétées.
- **6 hors sous-ensemble** : `\include "arabic.ly"`, `"makam.ly"`,
  `"persian.ly"`, `"turkish-makam.ly"`, `"english.ly"` sont des fichiers de
  langue/gammes listés dans `FICHIERS_LILYPOND` et ignorés ; les hauteurs
  qu'ils définissent (`\key fa \bayati`…) ne peuvent donc pas être résolues.

Les 152 autres restent à classer ; ils ne sont pas exclus du dénominateur.
Aucun test purement de mise en page n'a été trouvé dans ce lot : ceux qui
n'ont pas de musique jouable sont comptés dans les catégories ci-dessus.

### Campagne `attaque_decalee` : deux causes structurelles

La table de première divergence donnait 16 fichiers `grace` et 6 fichiers
`repeat/volta` dont les décalages venaient en fait de citations. Deux causes
structurationnelles, chacune mesurée séparément :

1. **Citation avec `\repeat unfold`** (`ly/music-functions-init.ly:545-557`) :
   `QuoteMusic` ne joue que la tranche de la citation correspondant à
   l'argument principal de `\cueDuring`. `music_duration()` renvoyait 0 pour
   `Context::Repeat`, si bien que `tronque_musique()` jouait la citation
   entière (`cue-clef.ly` : 36 notes au lieu de 16). Durée d'un repeat =
   `count × durée du corps`, et un item trop long peut être tronqué
   partiellement. **+6 exacts** (`cue-clef*`, `quote-tuplet`), 0 perte
   (`/tmp/ly-cuefix`, 1544).
2. **`\set Score.skipTypesetting`** (`scm/define-context-properties.scm:716`,
   `lily/score-performer.cc:151-176`) : lorsqu'il est vrai, `PROCESS_MUSIC`
   n'est pas appelé et le temps de la zone est replié (`midiSkipOffset`). Le
   lecteur ignore maintenant les notes de la zone et ne les compte pas dans
   le temps. **+3 exacts** (`skiptypesetting-bar-check`, `-break`, `-clef`),
   0 perte (`/tmp/ly-skipfix`, **1547/1724, 89,7 %**).

**Grâces et tolérance de l'oracle** : `scripts/oracle/lilypond/ly_notes.py:571`
considère déjà un fichier exact si `n2 == ref` **ou** `n2_grace == ref` (avec
`ref == test == n1`). Les 50 fichiers dont `n2_grace` est complet mais qui ne
sont pas exacts ont **toutes** chaque note de référence présente **plus** des
notes supplémentaires (`test > ref`) : le décalage des grâces n'est donc pas
leur cause d'échec. Aucun de ces fichiers ne doit être compté comme exact au
titre de `n2_grace` ; aucun changement d'oracle n'est nécessaire.

Reste dans `attaque_decalee` : `flags-default/straight/in-scheme` (décalage
constant de 0,25 ou 2,25, probablement lié aux durées de drapeaux Schemes),
`bend-spanner-styles`, `finger-glide-*`, `offsets`, `incipit-*`,
`skiptypesetting-show-*`. Ces groupes n'ont pas encore de cause commune
identifiée ; ils ne sont pas exclus du dénominateur.

### `\incipit` et correction de l'oracle ChordNames

**`\incipit`** (`ly/property-init.ly:439-490`) : l'argument est placé dans un
`\score` de `\markup` (MensuralStaff) : c'est une image gravée, absente du
MIDI. Le lecteur le consommait comme de la musique (9 notes au lieu de 5 sur
`incipit.ly`). Corrigé, avec tests réduits pour la forme simple et la forme
`\new Contexte` : **+3 exacts** (`incipit`, `incipit-modern`,
`incipit-other-context`), 0 perte (`/tmp/ly-incfix`, 1550). L'ancien test
`new_staff_imbrique_garde_sa_propre_voix` pédait jouer l'incipit : il a été
corrigé vers le comportement de l'oracle.

**ChordNames — le côté en tort était l'oracle, pas le lecteur.** J'avais
auparavant classé à tort ces 11 fichiers comme « artefact d'oracle » sans
vérifier le contenu. Vidage réel (`mido`) :

- référence `chord-changes.ly` : **1 piste, 30 notes**, toutes transposées
  (72/75/79…) ;
- compilation de la même source par le LilyPond installé avec `\midi {}` :
  **3 pistes, 60 notes** — piste 1 = grille ChordNames (60/63/67…), piste 2 =
  portée transposée (72/75/79…).

LilyPond 2.26 joue donc bien `\new ChordNames` (cf. les tests
`grille_daccords_jouee_dans_le_midi`), et la référence a été produite par un
autre compilateur ou un autre choix de `\score`. Conformément au README de
l'oracle (« a `ly.midi` produced by another version … is not a reader
oracle »), la référence est régénérée avec l'outil versionné
`scripts/oracle/lilypond/regen-refs.py` (copie par liens durs de l'arbre,
`--jobs 2`, injection de `\midi {}`). Les 11 `ly.midi` régénérés sont dans
`/tmp/ly95lect-refs-fixed` ; les originaux restent intacts.

Mesure avec l'oracle corrigé : **1556/1724 (90,3 %)**, **+6 exacts**, 0 perte
(`/tmp/ly-oraclefix`). Les 5 autres fichiers ChordNames gardent d'autres
écarts et ne sont pas inclus dans ce gain. Le chiffre 1550 restait valable
avec l'ancien arbre de références ; les deux chiffres ne sont pas comparables
sans préciser l'oracle utilisé.

**Limite documentée** : `skipTypesetting` est évalué par LilyPond dans l'ordre
du temps et au niveau Score (`lily/score-performer.cc:151-176`) ; le lecteur
le traite en ordre de parsing. Le test ignoré
`skip_typesetting_dans_une_voix_parallele` montre l'écart sur
`<< { \set Score.skipTypesetting = ##t a4 … } \\ { c4 d4 } >>`. Aucun fichier
du lot `ra` ne pose cette propriété dans une voix parallèle, donc la limite
n'affecte pas la mesure.

### Audit complet des références `ra` (LilyPond 2.26.0)

Le cas ChordNames n'était pas isolé. Audit de **tous les 170 fichiers non
exacts** de l'arbre corrigé (1556/1724) avec `regen-refs.py` (`--jobs 2`) et
le compilateur installé, **GNU LilyPond 2.26.0 (Guile 3.0)**, dans un arbre
séparé reconstruit depuis l'arbre corrigé (`/tmp/ly95lect-refs-audit2`) :

- **146 références régénérées avec succès**, 24 échecs de régénération
  (19 sources Scheme/templates sans expression musicale injectable, 3
  erreurs de compilation, 2 sources absentes) ;
- parmi les 146 : **24 références périmées** (contenu MIDI différent :
  hauteur/attaque quantifiée) et **122 identiques** ;
- les 3 références dont la régénération a échoué ont été restaurées depuis
  l'arbre corrigé, pour garder un dénominateur de 1724.

Aucune de ces 24 références périmées ne justifie un changement du lecteur.
Mesure avec l'arbre régénéré : **1565/1724 (90,8 %)**, **+9 exacts**, **0
perte** (`/tmp/ly-audit3meas`). Les gains viennent tous de références
périmées : `breve-extent`, `custos`, `ledger-extra`,
`v3_testMultiInstrumentPart3`, `v3_testTboxWords1`, `v4_hairpin_dynamic_inside`,
`v4_hairpin_dynamic_jump`, `v4_hairpin_simple`, `v4_part-textlines`.

**Bascule d'oracle** : la mesure de référence est désormais
`/tmp/ly95lect-refs-audit2` (LilyPond 2.26.0), pas
`/tmp/ly95lect-refs-fresh` (arbre d'origine, mélangé). Les chiffres à
comparer sont : 1538 (arbre d'origine, avant audit), 1550 (arbre d'origine
après `\incipit`), 1556 (11 références ChordNames corrigées), **1565 (audit
complet)**. `regen-refs.py` a été durci : détection des expressions
musicales de premier niveau, essai de plusieurs points d'injection `\score`,
reprise sur source absente, `__jobs 2`.

### Cause lecteur sur l'arbre régénéré : `\alternative` sans corps

Sur l'oracle régénéré (1565), le plus gros sous-groupe encore cohérent était
`repeat/volta` (12 fichiers). `lily/parser.yy:1560-1569` autorise
`REPEAT … music alternative_music` avec un `music` vide : dans
`\repeat volta 10 \alternative { … }`, `parse_music()` consommait le jeton
`\alternative` comme événement vide et la reprise entière disparaissait
(`repeat-petrucci-alternatives.ly` : 2 notes au lieu de 10). Laisser le corps
vide quand le prochain jeton est `\alternative` permet au traitement existant
des fins de s'appliquer. Test réduit failing-first.

Mesure (`/tmp/ly-altfix`, oracle régénéré) : **1568/1724 (91,0 %)**, **+3
exacts** (`lyric-repeat-count-alternatives`, `repeat-petrucci-alternatives`,
`volta-spec-ossia`), **0 perte**.

### Références non régénérables et dénominateur ajusté

La doc en ligne de LilyPond 2.26 (`lilypond --help`) expose `-e/--evaluate`,
`-i/--init` et `-I/--include` ; `lily/main.cc:128` ne fait que déclarer
`-e`, et `lily/lily-parser-scheme.cc:256-271` indique que
`ly:parser-parse-string` n'est valable que sur un parseur neuf : aucun de ces
mécanismes ne permet d'ajouter un `\midi { }` à un `\score` implicite déjà
en cours d'analyse. En revanche, une **copie locale du fichier inclus**
porteur du `\score` (`satb.ly`, `accidental-glyphs-test.ily`) avec
`\midi { }` injecté, compilée avec `-I <copie>`, donne une référence valide.
`regen-refs.py` implémente cette reprise : 11 des 24 fichiers précédemment
irrégénérables ont maintenant une référence valide (7 modèles
satb/ssaattbb, 4 tests de glyphes), sans changement du score mesuré
(1568, 0 gain/perte).

Restent **11 fichiers sans référence valide** (sources pilotées par des
fonctions Scheme ou dont le `\score` ne peut être injecté) :
`chord-name-major7`, `chord-names-languages{,2}`, `drum-scripts`,
`ledger-line-properties`, `part-combine{, -cross, -global}`, `retrograde`,
`satb-template-with-keyboard-reduction`, `tag-filter`. Comme les références
inutilisables documentées ailleurs, ils sont comptés dans un bucket séparé
« no valid reference » **hors du dénominateur** (`/tmp/ly95-no-ref.txt`).
Mesure ajustée (`/tmp/ly-validmeas`) : **1568/1713 (91,5 %)**, 15 vides ;
le chiffre 1568/1724 (91,0 %) reste valable avec le dénominateur complet.

### Cause lecteur : gabarits SATB / SSAATTBB

`ly/satb.ly:157-215` et `ly/ssaattbb.ly:280-330` portent le `\score` dans le
fichier livré avec LilyPond et le construisent à partir des variables
`*Music` définies par le fichier appelant. Le lecteur ignorait ces includes
(`FICHIERS_LILYPOND`) et renvoyait une partition vide. Il injecte maintenant
un score équivalent jouant chaque variable dans sa propre portée ; le MIDI
de référence ne dépend que du multiset (hauteur, attaque), vérifié exactement
sur deux fichiers du corpus (78/78 et 39/39) avant l'implémentation. Tests
mis à jour et ajoutés.

Mesure (`/tmp/ly-satbmeas`, arbre régénéré) : **1576/1724 (91,4 %)**, **+8
exacts**, **0 perte**. Déduction du bucket « no valid reference » :
**1576/1713 (92,0 %)**.

**Preuve documentée pour l'absence de deuxième cause lecteur ce tour** :
après cette correction, les groupes restants de `attaque_decalee`,
`notes_manquantes` et `sortie_vide` sont dominés par (a) 4 tests de glyphes
dont la musique est entièrement produite par la fonction Scheme `\test`
(`accidental-glyphs-test.ily:12-90`, hauteurs microtonales), (b) des sources
pilotées par Scheme sans score injectable (déjà comptées hors dénominateur),
(c) des tests de mise en page dont les notes sont identiques mais les
attaques décalées de moins d'un 1/16 (`flags-*`, `bend-spanner-*`) et
(d) des références invalides. Aucun de ces groupes ne partage une commande
lisible dans le sous-ensemble avec au moins 3 fichiers ; aucun correctif
n'a été tenté au hasard. Les fichiers concernés restent dans le
dénominateur (sauf le bucket documenté), sans exclusion silencieuse.

### Campagne fichier par fichier : findings

**Oracle corrigé**： l'heuristique d'injection de `regen-refs.py` sautait les
blocs de musique suivant une définition multiligne (la profondeur d'accolades
dérivait), produisant des références tronquées — `flags-default.ly` avait 36
notes au lieu de 48. Heuristique corrigée (comptage sur toutes les lignes,
score injecté à la première expression musicale) : sur 158 références
régénérées, **13 seulement sont réellement périmées** (pas 24). Arbre propre
`/tmp/ly95lect-refs-clean` : 1576 exacts, aucun changement de score.

**`flags-*` / décalages < 1/16** (`lily/audio-item.cc:122`) : la formule de
timing MIDI est `main + 9/40 × grace_part`. Vidage de `flags-default.ly` :
la grâce `\acciaccatura {c'8}` est émise à 0,875 avec une durée réelle de
0,1125 (9/80), et la note principale à 0,9896 ; le lecteur pose la grâce
sans durée à 0,9375. L'oracle apparie bien les 8 grâces (`graces_match=8`)
mais le décalage s'accumule de phrase en phrase : `n2_grace == n2`. Ce n'est
pas un problème de mise en page, mais le modèle MusicXML n'a pas de temps
logique pour la grâce ; corriger exigerait de décaler toutes les notes
suivantes, ce qui casserait les autres fichiers à grâces. Cause documentée,
pas de correctif hasardeux.

**Microtones** (`lily/midi-item.cc:273-293`) : `get_semitone_pitch()` fait
`rint(tone_pitch × 2)` et `get_fine_tuning()` ajoute un pitch-wheel
(`PITCH_WHEEL_SEMITONE`). L'oracle lit le pitch-wheel via le byte de note-on
arrondi, et `ly_notes.py:301` fait `int(float(<alter>))` (troncature). Aucun
fichier non exact n'a de `<alter>` fractionnaire dans son MusicXML : les
4 tests de glyphes ont `t=0` car leur musique est produite par la fonction
Scheme `\test`, hors sous-ensemble. Aucun correctif lecteur justifié.

**Deux causes commitées et mesurées** :
1. `Timing.skipTypesetting` était intercepté par la branche cadenza de
   `parse_set` ; `\unset` d'un alias déprécié déclaré avec `#:new-symbol
   'skipTypesetting` est maintenant reconnu (`scm/define-context-properties.scm
   :1001-1055`). **+1 exact** (`context-property-deprecated-unset`),
   0 perte — 1577.
2. `\voices` (`ly/music-functions-init.ly:2631-2640`) consomme maintenant sa
   `key-list?` au lieu de la laisser devenir des durées. Correctif de
   syntaxe générique, test réduit ; `voices-command.ly` garde 2 notes
   supplémentaires d'une autre cause, therefore 0 gain isolé, 0 perte.

**Preuve par fichier tenté** : sur les 12 fichiers à plus petit écart N2,
6 ont une référence invalide ou une musique Scheme non interprétable
(bucket hors dénominateur), 3 dépendent du timing de grâce ci-dessus, 2
(`skiptypesetting-show-last`, `note-head-style`) sont pilotés par Scheme,
et 1 (`voices-command`) a une cause résiduelle non partagée. Aucun d'entre
eux ne justifie un hack spécifique ; they rest in the denominator.
### Temps de grâce : règle d'oracle dérivée du MIDI seul

La première tentative (`recaler_graces_internes`) était **fausse** : elle
calculait `delta = r - t` note par note puis soustrayait ce delta, rendant
exact n'importe quel candidat de même hauteur. Elle a été retirée de
l'historique (`git reset --hard master`).

Vidage brut du MIDI de référence (`mido`, tpb 384) : la grâce
`\acciaccatura {c'8}` est émise au tick 336 avec une durée de **43 ticks**,
au même tick que le `d'64` précédent coupé de 24 à **1 tick** ; la note
principale suit au 380, phrase suivante au **404** au lieu de 384. Le
décalage est donc **+20 ticks (0,05208 noire) par grâce**.

Pourquoi le temps suivant bouge alors que la grâce est hors temps principal :
`lily/grace-iterator.cc:25-40` met la grâce dans la partie `grace_part_` du
moment, `lily/audio-item.cc:118-129` convertit ce moment en temps réel
(`main + 9/40 × grace_part`), `lily/audio-column.cc:32-34` l'applique à
l'horodatage de la colonne audio, et `lily/midi-walker.cc:92-95` écrit les
delta-ticks. C'est un **quirk du MIDI LilyPond** : le temps de grâce est
audible et décale les colonnes suivantes, même si la notation le considère
hors mesure.

Règle conservée : `decalage_graces_reference()` identifie les grâces du MIDI
par hauteur/fenêtre **uniquement pour les repérer** (durée MIDI courte
`<= GRID/2`, attaque partagée avec une note amputée à `< GRID/4`), puis
calcule l'excédent `durée_grace - GRID/4` et construit S(t) = somme cumulée
des excédents. S(t) est soustrait des attaques de référence avant
comparaison. **Aucune attaque du candidat n'entre dans le calcul.** Un
selftest vérifie qu'un candidat avec grâces appariées mais une note
principale décalée d'une croche reste inexact ; `ly_notes.py selftest` OK.

Mesure (`/tmp/ly-refonly4`, arbre propre) : **1588/1724 (92,1 %)**,
**+11 exacts**, **0 perte** — `accidental-placement-samepitch`,
`arpeggio-no-overshoot`, `collision-{dots, heads, merge-differently-dotted,
merge-differently-headed, single-head}`, `completion-heads-polyphony`,
`flag-stem-begin-position`, `note-head-chord`, `text-spanner-override-order`.
L'ancien chiffre 1591 provenait de la règle incorrecte et n'est pas
comparable ; le présent +11 est le gain réel et vérifié sans perte.

### Vérification des gains : mesure contrôlée

Les 11 fichiers annoncés comme gains de la règle de grâce ont été revérifiés
un par un : **tous ont `graces=0` et `paliers=[]`**, donc la règle ne peut pas
les avoir rendus exacts. Une mesure contrôlée avec **le même binaire** et
**le même arbre de références**, en remplaçant seulement `ly_notes.py` par la
version parente (`/tmp/oracle-old`, `/tmp/ly-controlled`), donne :

- ancien oracle : 1588 exacts ;
- nouvel oracle : 1588 exacts ;
- **gain réel de la règle : 0 fichier, 0 perte**.

Les +11 annoncés précédemment venaient d'un binaire lecteur différent
(validations de master entre les deux mesures) et non de l'oracle. Le chiffre
vérifié reste **1588/1724 (92,1 %)**. La règle est conservée parce qu'elle est
correcte et que le selftest prouve qu'elle n'efface pas les rythmes, mais
elle n'apporte aucun gain mesurable sur ce lot.

`GRID/4` (une double-croche) a été remplacé par une valeur **mesurée dans le
MIDI de référence** : la durée nominale de la note amputée est le mode des
durées courtes de même hauteur dans le MIDI, et le temps volé est
`nominal − durée_observée_de_la_coupe`. Le décalage inséré est alors
`durée_grâce − temps_volé` (≈ 0,05208 noire sur `flags-default`).

**Cause_reader non retenue** : `skiptypesetting-tuplet.ly` (r=2, t=2 mais
octave décalée) montre que le lecteur avance la référence `\relative` sur les
notes supprimées par `skipTypesetting` (d=50/48 au lieu de 62/60). Corriger
demande de conserver les notes dans l'AST pour le calcul relatif tout en les
excluant du MIDI : changement structurel, non tenté ici.

**Cause lecteur tentée puis retirée** : `monochords.ly` (r=14, t=2) perdait
les durées nues après `\breve` : la commande était traitée comme inconnue et
avalait les durées suivantes. Un traitement comme durée reprise a été tenté
(`lily/parser.yy`, `expand-repeat-notes!` scm/music-functions.scm l. 972-1015)
mais restait incomplet (3 accords au lieu de 4 sur le réduit) et n'apportait
**aucun gain** sur `ra` ; le commit a été annulé (`git reset --hard HEAD~1`)
plutôt que de conserver un test échouant. Le fichier reste dans le
dénominateur, sans exclusion silencieuse.

### Cause lecteur contrôlée : durées nues `\breve`/`\longa`/`\maxima`

`lily/parser.yy:1518-1525` crée un `NoteEvent` sans hauteur pour une durée
seule ; `expand-repeat-notes!` (`scm/music-functions.scm` l. 972-1015) le
résout en répétition du dernier accord. `\breve` était traité comme commande
inconnue et avalait les durées suivantes. Le lecteur construit maintenant la
durée directement (`parse_command` a déjà consommé le nom : appeler
`parse_duration()` aurait lu la durée *suivante*). Test réduit : 4 accords.

Mesure **contrôlée** (même oracle, mêmes références, seuls les binaires
diffèrent : `/tmp/ly-bin-refonly` avant, `/tmp/ly-bin-new` après) :
`1588 → 1589`, **+1 exact** (`monochords.ly`), **0 perte**. C'est désormais
la méthode de mesure obligatoire.

Fichiers tentés sans gain dans cette session (preuve documentée, aucun
n'est exclu du dénominateur) :
- `after.ly` : `\after` simple consomme bien sa durée ; le décalage vient des
  `\after` chaînés avec hairpins, cause distincte non identifiée ;
- `finger-glide-spanner-nested-line-break.ly` : notes décalées d'un temps
  entier et hauteurs différentes, pas une durée nue ;
- `tie-chord.ly`, `note-head-style.ly`, `scheme-list-splicing.ly` : musique
  produite par des fonctions Scheme, hors sous-ensemble ;
- `skiptypesetting-show-last.ly` (`showLastLength`) : nécessite de modéliser
  la troncature de la partition, changement structurel ;
- `bar-line-caesura-double.ly` : `\new Score \with { … }` avec deux portées
  partageant la même variable, cause distincte.

### Table unique des 135 fichiers non exacts (première divergence)

`subdivise.py` sur `/tmp/ly-ctrl-new` (1589 exacts) :

| Constructeur | Fichiers | Exemple |
| --- | ---: | --- |
| markup/tweak | 29 | `accidental-styles.ly` |
| relative | 20 | `backend-exercise.ly` |
| grace | 18 | `after.ly` |
| voices | 16 | `bar-line-caesura-double.ly` |
| cue/quote | 10 | `quote-cue-during.ly` |
| scheme | 9 | `bar-line-define-bar-glyph.ly` |
| repeat/volta | 9 | `duration-multiplier-scheme.ly` |
| include | 7 | `alteration-glyphs.ly` |
| tuplet | 4 | `alignment-order.ly` |
| sans-construct | 4 | `chord-X-align-on-main-noteheads.ly` |
| score/book | 4 | `cue-clef-transposition.ly` |
| transpose/clef/key | 4 | `music-map.ly` |
| lyrics | 1 | `lyrics-includegraces.ly` |

Le plus gros groupe (`markup/tweak`) est hétérogène : 29 fichiers mêlent
Scheme, ornements MIDI et erreurs d'octave, sans commande lisible commune.
Le groupe `relative` contient 4 fichiers `cluster-*`/`backend-exercise` dont
le décalage vient de `\makeClusters` : la portée est muette dans le MIDI mais
ses notes restent dans le MusicXML. Corriger exigerait de retirer les notes
du modèle pivot (changement sémantique : les tests existants exigent de les
conserver pour la gravure), donc non tenté. Le groupe `repeat/volta` contient
4 fichiers `volta-spec-*` dont les numéros hors plage devraient être ignorés
au dépliage (`lily/alternative-sequence-iterator.cc:86-117`) : le lecteur ne
conserve pas ces numéros dans l'AST, changement structurel non tenté ici.
Aucun de ces fichiers n'est exclu du dénominateur.

Mesure contrôlée de la cause `\breve` (même oracle, mêmes références, seul le
binaire change) : `1588 → 1589`, **+1 exact** (`monochords.ly`), **0 perte**.

### Groupes 2 et 3 : preuve par sous-groupe

**Groupe 2 — `relative` (20)** : le sous-groupe `cluster-*`/`backend-exercise`
(4 fichiers) partage `\makeClusters`. Vidage du MIDI de `cluster.ly` : 3 pistes,
une seule avec 14 notes, la piste Cluster **vide**. `scm/music-functions.scm`
(`note-to-cluster`) transforme les `NoteEvent` en `ClusterNoteEvent`, sans
performer MIDI. Le modèle retire donc désormais ces notes : **+2 exacts**
(`cluster`, `cluster-break`), 0 perte en mesure contrôlée. Restent
`backend-exercise.ly` (mélange tuplets/ottava/glissando, diverge à 0,33 sur
une note de 101, cause distincte) et `collision-seconds.ly` (16 notes sup sur
45, voix de collisions, cause distincte) : preuve par fichier, aucun exclus.

**Groupe 3 — `voices` (16) et `markup/tweak` (29)** : subdivision par commande
précise au voisinage de la divergence. Plus gros sous-groupe commun :
`\partCombine` (3 fichiers : `part-combine`, `-global`, `-cross`). Le cas réduit
`\partCombine \vone \vtwo` avec deux hauteurs différentes produit bien les
deux voix (67 et 65 à zéro) : la bascule `Music::Sim` du lecteur est correcte.
L'écart des 3 fichiers vient donc du contenu propre de chaque voix (durées
longues, `~`, `\time` global), pas d'une commande commune ; chaque autre
sous-groupe est un fichier unique (`caesura`, `defineBarLine`, `featherDurations`,
`chordmode`, `germanChords`, `bendAfter`, etc.). Aucun correctif commun
justifié ; preuve par sous-groupe, aucun fichier exclu.

**Bilan contrôlé de la session** : 1588 → 1589 (`\breve`, +1) → 1591
(`makeClusters`, +2), soit **+3 exacts**, **0 perte** à chaque étape,
**1591/1724 (92,3 %)**.
### Reading campaign: `grace`, `cue/quote`, `include`, `repeat/volta` (2026-11-23)

The four groups of the §22 table were subdivided by first divergent construct
(`subdivise.py` on the current run). Controlled measurements use the current
reference tree (`/tmp/ly95lect-refs-clean`), the same oracle and the same
manifest on both sides; only the reader changes (`mesure.py --refs-prefix
out3 --refs-only`).

| Step (same oracle) | `ra` exact |
| --- | ---: |
| master `20e9c1bb` | 1 591/1 724 |
| + `\volta` selection | **1 594** (+3) |
| + `\killCues` | **1 595** (+1) |
| + nested alternatives | **1 596** (+1) |

**No exact loss, no N1/N2 regression** at any step. Gains:
`volta-spec-in-alternative`, `volta-spec-unfold-in-unfold`,
`volta-spec-unused`, `quote-kill-cues`, `volta-bracket-nest`.

1. **`\volta` (`repeat/volta`, 3 files).** `lily/parser.yy` (token VOLTA)
   marks music with `volta-numbers`; `lily/unfolded-repeat-iterator.cc` and
   `lily/alternative-sequence-iterator.cc` play it only on the listed passes.
   The reader dropped the numbers and played every branch (`volta-spec-unused`
   12 notes instead of 4). `Context::Volta` keeps the list; the interpreter
   tracks the current pass when unfolding and filters. `\volta` belongs to
   the **innermost** repeat: a non-unfolded repeat cancels an inherited pass
   so its endings stay in the engraved model (`volta-spec-volta-in-unfold`,
   still 12 notes). Tests: `read_volta.rs`.
2. **`\killCues` (`cue/quote`, 1 file).** `ly/music-functions-init.ly:955-964`:
   quotations with `quoted-context-id "cue"` are replaced by their principal
   argument (silences), while `\quoteDuring` citations stay. The reader
   ignored `\killCues`, doubling the cue in the “killCues” staff
   (`quote-kill-cues` 14 → 12 notes). `Context::Cue` now records
   `cue_notes`; `kill_cues` replaces cue contexts by `silence_music`. That
   helper now recurses through wrappers (`\relative`, `\transpose`, repeats):
   returning `Empty` made the silence last zero and shifted the next note by a
   measure. Test: `read_kill_cues.rs`.
3. **Nested `\alternative` (`repeat/volta`, 1 file).**
   `volta-bracket-nest` (Notation Reference §4.1 “Nested repeats”) has an
   `\alternative` inside a volta ending, not claimed by an inner `\repeat`;
   the interpreter ignored it and lost `g1`/`a1` (4 notes instead of 6). The
   engraved model now walks all its endings. Test:
   `alternative_imbriquee_garde_ses_fins`.

Remaining subgroups, with their proof (none excluded from the denominator):

- **`grace` (18).** For 16 files the reader already produces the reference
  notes (`n1 == ref`); the residual mismatch is the oracle's grace pairing.
  A grace can share the main note's pitch, and `apparier_graces` may then
  pair the **main** MIDI note instead of the grace (`grace-auto-beam-engraver`:
  the 28 `a1` notes are stolen; `n2_grace` 112/140). Pairing strictly before
  the main attack gives +3 exact files (`beam-subdivide-grace`,
  `spacing-loose-grace`, `v4_changevisibility`) but changes the oracle for
  both binaries, so it is not part of these reader commits. `\afterGrace`
  graces are played **after** the main note (reference at 7/8 of it), outside
  the `[attack - 1, attack + grid/2]` window, so `n2_grace` cannot match them
  (`lyrics-after-grace`, `grace-end-expression`, `after.ly`,
  `trill-spanner-grace`) — oracle limitation, not a reader loss.
- **`cue/quote` (10).** Besides `killCues`, `quote-cue-during` shows that a
  cue takes the quoted music at the **absolute score time**: at 2 quarters the
  reference plays the quote's notes at 2 and 3 quarters (A, B), while the
  reader inserts the quote from its start (F♯, A). `lily/quote-iterator.cc`
  aligns the quotation with the enclosing time; fixing it requires slicing
  the quote by the interpreter's cursor instead of truncating it at parse
  time (not attempted here).
- **`include` (7).** Four `sortie_vide` files
  (`alteration-glyphs`, `arabic-accidental-glyphs`, `makam-accidental-glyphs`,
  `turkish-makam-accidental-glyphs`) get their notes from a Scheme function
  using `add-score` and `#{ … #}` (`accidental-glyphs-test.ily`), outside the
  read subset. `satb-template-with-keyboard-reduction` diverges because the
  **MIDI** reference drops the `\tag "print"` keyboard reduction while the
  MusicXML keeps the engraved score (print/MIDI tag difference, not a
  note-reading bug). `swing-test` needs the Scheme timing of `\tripletFeel` /
  `\applySwing`.
- **`repeat/volta` (9).** Besides the four fixed files: `repeat-volta` (one
  extra note, alternative selection), `duration-multiplier-scheme` (Scheme
  durations `#(ly:music-length …)` as multipliers), `repeat-etc` (partial
  `\repeat … \etc` timing), `repeat-unfold-all` (nested unfold/alternative),
  `skiptypesetting-show-first-and-last` (`showFirstLength`/`showLastLength`
  truncation) — distinct causes, each with its own reference.

### Vérification `play=false` et mesure contrôlée finale

`<play>no</play>` était invalide : `musicxml-4.0.xsd:2747-2765` définit un
type complexe (children `ipa`/`mute`/`semi-pitched`/`other-play`), sans texte.
MuseScore n'écrit pas de `<play>` pour `play()==false`, sauf `<cue/>` sur les
petites notes (`exportmusicxml.cpp:4190-4193`). Notre importeur relit déjà
`<cue/>` en `Note.play=false` (`pass2.rs:1249`) : c'est le mécanisme standard
retenu. `xmllint --schema musicxml-4.0.xsd` valide la sortie de `cluster.ly`.

Aucun `.mscx` du corpus (`tests/corpus/v3`, `v4`) ne contient d'élément
`<play>` (`grep -rl '<play>'` : 0 fichier) : le changement d'export ne peut
donc pas modifier les références MusicXML 4.7.4, ni l'e2e corpus ni le
rapport de parité complète. Aucun fichier concerné à nommer.

Mesure strictement contrôlée : binaire de master `6229fcec`
(`/tmp/wt-master-bin/target/debug/scoretk`) contre master + nos commits
(`/tmp/ly-bin-cue`), **même oracle, mêmes références** (`/tmp/ly95lect-refs-clean`)
: **1596 → 1599**, **+3 exacts** (`cluster.ly`, `cluster-break.ly`,
`backend-exercise.ly`), **0 perte**. Le chiffre +20 annoncé précédemment
comparait des binaires séparés par de nombreux commits master et n'était pas
contrôlé ; il est remplacé par +3.

### Voix simultanées et fonctions Scheme définies dans le fichier

**Chronologie `<< { } { } { } >>`** : `lily/simultaneous-music-iterator.cc:96-131`
exécute chaque enfant à son propre `pending_moment` (pas de séquencement), et
`events.rs::simultaneous` marche la première branche sur place puis `spawn_at(start)`
les autres — toutes au même tick. Reduced cases vérifiés :
`{ c'1 << { d'4 } { e'4 } { f'4 } >> }` et la variante avec `\parenthesize`/`\tweak`
donnent bien les trois notes de la deuxième mesure au même tick. L'écart de
`parenthesize-chords.ly` (ticks 1 vs 3) ne vient donc pas de la règle Sim mais
du contenu propre de la troisième branche (`<c' e>` après `\tweak` dans un
contexte `\new TabVoice`) ; pas de cause commune avec les 16 fichiers du
groupe `voices`, dont les divergences sont des constructeurs distincts
(`caesura`, `defineBarLine`, `featherDurations`, `chordmode`, `germanChords`,
`partCombine`, …). Aucun correctif global justifié, aucun fichier exclu.

**Inventaire des fonctions Scheme locales** : 9 fichiers inexacts définissent
`define-music-function`/`define-scheme-function` dans le fichier.
Corps `#{ … #}` présents dans 7, mais après lecture :
- `make-relative.ly`, `make-relative-music.ly` : corps `make-relative` (logique
  d'octave complète, pas une simple substitution) ;
- `scheme-list-splicing.ly` : `map`/`iota`/`ly:make-pitch` Scheme ;
- `note-head-style.ly` : `string->symbol`, `\with #style`, paramètres context-mod ;
- `optional-args.ly` : **le seul** corps réellement simple (`#{ \relative $ref $music #}`
  avec arguments optionnels) ;
- `accidental-styles.ly`, `tie-chord.ly`, `music-function-string-markup.ly` :
  corps `make-music`/Scheme, sans `#{ }`.

Built-ins `ly/music-functions-init.ly`/`scm/music-functions.scm:2144-2188`
(`skip-of-length`, `mmrest-of-length`) : un seul fichier (`skip-of-length.ly`),
avec en plus `musicMap #skip->rest`. **Total tractable par un évaluateur
minimal `$x`/`#x` + ces built-ins : 2 fichiers**, sous le seuil de 5 fixé :
implémentation non justifiée, documentée ici. Aucun fichier exclu ;
`persian-key-signatures.ly` est laissé à msx4 (clés).

### Grace pairing, `\afterGrace`, quotations at absolute time (2026-11-23)

Controlled measurements after rebasing on master `1405c31e`
(`/tmp/ly95lect-refs-clean`, same manifest; each row uses the oracle named
in it on both sides). Master carries ly95's `<cue/>` change (cluster notes
are written `<cue/>` and the oracle skips them while advancing the cursor).

| Binary / oracle | `ra` exact |
| --- | ---: |
| master `1405c31e` + master oracle | 1 599 |
| master + **our** oracle (grace pairing only) | **1 602** (+3) |
| ours + our oracle (`\afterGrace` +3, quotes +9) | **1 614** (+12) |

Total 1 599 → **1 614/1 724** (+15). **No exact loss and no N1/N2
regression** at any step. The three oracle gains are
`beam-subdivide-grace`, `spacing-loose-grace`, `v4_changevisibility`; the
twelve reader gains are `cue-clef-transposition`,
`cue-clef-transposition-optional`, `lyrics-after-grace`,
`lyrics-includegraces`, `quote-cue-during`, `quote-cue-event-types`,
`quote-during`, `quote-grace`, `quote`, `quote-tie`, `quote-tuplet-end`,
`spacing-loose-grace-error`.

Our cue notes (`small`, played) carry `<type size="cue">` but **not**
`<cue/>`: `is_cue_note` requires `!play` (`exportmusicxml.cpp:4190-4193`),
so only `play = false` (clusters) gets `<cue/>` and is skipped by the oracle.
Test `cue_tag_seulement_pour_note_non_jouee`.

1. **A grace cannot steal the main note (oracle).**
   `lily/audio-item.cc:118-129` places a grace at
   `main + 9/40 × grace_part`, and `lily/grace-iterator.cc:24-27` gives
   `grace_part` a negative value (the grace part of the moment): a grace
   sounds **strictly before** the attack of the note it ornaments (Notation
   Reference §1.3.1). `apparier_graces` accepted notes up to
   `attaque + grid/2`; when the grace and the main note shared a pitch, it
   paired the **main** note and left the grace in the reference
   (`grace-auto-beam-engraver`: 28 `a1` stolen, `n2_grace` 112/140). The upper
   bound is now strict. The oracle change was measured on two binaries:
   current 1 596 → 1 599 and previous (`20e9c1bb`) 1 591 → 1 594, the same
   three files (`beam-subdivide-grace`, `spacing-loose-grace`,
   `v4_changevisibility`). Selftests: the pairing keeps the note at 0 and
   matches the one at −0.1; a candidate missing the grace is still not exact.
2. **`\afterGrace` (exporter + oracle).**
   `ly/music-functions-init.ly:86-112` places the grace after
   `fraction × main duration` (`afterGraceFraction = 3/4` by default), so its
   real time is `main + 9/40 × grace_part`. The model timing was already
   right, but the MusicXML divisions only counted durations and segment gaps:
   the `attack` attribute of an after-grace is
   `played_offset − main duration` (3/8 for 3/4 of a half note), a fraction
   absent from the LCM. Divisions stayed at 1 per quarter and the attack was
   rounded (`lyrics-after-grace`: 1.0 instead of 1.5, against 1.47 in the
   MIDI). The grace `played_offset`s now enter the divisions; the oracle
   distinguishes before-graces (strictly before) and after-graces
   (`read_xml` sees the positive `attack`, window `[attaque − 1, attaque +
   grid/2]`). The oracle alone gave no `ra` gain (1 599 → 1 599); with the
   divisions fix: 1 599 → 1 602 (+3: `lyrics-after-grace`,
   `lyrics-includegraces`, `spacing-loose-grace-error`). Rust test
   `after_grace_offset_impose_les_divisions`.
3. **Quotations at absolute score time (reader).**
   `Quote_iterator::process` (`lily/quote-iterator.cc:161-211`) computes
   `start_mom = now_mom()` and skips quoted events before it; the citation
   stops at `T + duration of the principal argument`
   (`ly/music-functions-init.ly:545-558`, `:2023-2032`). A quote after a
   whole note therefore plays the **segment of the quote at the absolute
   score time**, not its beginning (`quote-cue-during`: at 2 quarters, A/B,
   not F♯/A). The reader used to truncate the quote from its start at parse
   time. `Context::Cue` now keeps the full quote and the principal duration;
   the interpreter walks it from zero, keeps the chords in
   `[cursor, cursor + duration)` on the current track (tuplets reindexed, cue
   notes small) and advances by the duration. `killCues` uses the stored
   duration for its silence; the old `tronque_musique` is removed. Tests
   `citation_au_temps_absolu_de_la_partition` and
   `cue_during_suit_le_meme_temps_absolu` (read_quote.rs). Gains: 1 602 →
   1 611 (+9: `cue-clef-transposition`, `cue-clef-transposition-optional`,
   `quote-cue-during`, `quote-cue-event-types`, `quote-during`, `quote-grace`,
   `quote`, `quote-tie`, `quote-tuplet-end`).

`after.ly` remains inexact (one extra note from its `\after`/`\afterGrace`
combination, a distinct cause).

### Campagne fichier par fichier (session suivante)

Deux causes portées, chacune avec reduced test et mesure strictement contrôlée
(binaire master `20d7a9f0` dans `/tmp/wt-master2`, même oracle, mêmes refs) :
- `\featherDurations` (`ly/music-functions-init.ly:655-670`) : facteur `scale^i`
  sur chaque durée puis normalisation ; gain `beam-feather.ly`, `beam-french.ly` ;
- `c:7+` (`ly/property-init.ly:299`, `lily/lexer.ll:1062-1067` CHORD_MODIFIER)
  : septième majeure ; gain `chord-name-major7.ly`.

Total contrôlé : **1614 → 1617**, +3 exacts, 0 perte.

Fichiers examinés sans cause commune this session (preuve, aucun exclus) :
`part-combine.ly`/`-cross.ly` ont `n2 = 100 %` et `n1 = ref` mais **2 notes en
trop** (`t = ref + 2`) : l'écart n'est pas un accord manquant mais des notes
supplémentaires du `\partCombine`, cause distincte de la réunion des voix ;
`bar-line-caesura-double.ly` ($\new Score \with$), `defineBarLine`,
`optional-args.ly` et `skip-of-length.ly` (constructs déjà documentés).
