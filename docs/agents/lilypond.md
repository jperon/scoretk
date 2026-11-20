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
