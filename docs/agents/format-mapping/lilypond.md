# LilyPond 2.26.0 ↔ scoretk model

References (outside the repository): `specs/README.md`, section LilyPond — the
Notation Reference 2.26.0 (complete single page and chapter index) and the
Learning Manual chapters on input syntax. The local `lilypond-ref/` checkout
contains only the C++/Scheme sources and **no `Documentation/` tree**, so the
published 2.26.0 documentation was fetched from lilypond.org; the source of the
reader port is `lily/parser.yy` and `lily/lexer.ll` of that checkout.
Section names below follow the Notation Reference and the Learning Manual
chapters "How to write input files", "Input characters", "Working on input
files", "How LilyPond input files work". Reader: `crates/scoretk-lilypond-import`;
writer: `crates/scoretk-lilypond-export`.

| Format property (spec section) | Model type / field (file:line) | Reader | Writer | Notes |
| --- | --- | --- | --- | --- |
| `\header { title, composer, poet, … }` (Learning: input files) | `ScoreMetadata` (`crates/scoretk-core/src/model/score.rs:50-87`), `Credit` (`.../model/text.rs:586-610`) | supported (`import/src/builder.rs:684-710`) | supported (`export/src/structure.rs:89`) | Writer emits `\header` plus credits as fallback. |
| `\version`, `\language`, `\include` (Learning: input files) | — | partial (`import/src/include.rs:91`) | missing | Includes with a resolver are expanded; `\include` of Scheme or missing files warns. No version/include is emitted by the writer. |
| Variables `name = { … }`, `\name` (Learning: how input files work) | no dedicated model field (music is flattened into `Segment`s) | supported (`import/src/parser.rs:56,363`) | supported (`export/src/writer.rs:139`) | Writer creates one variable per voice in `\fixed` mode; original variable names are not preserved. |
| Notes and pitch names `c'`, `bf`, `cis` (Notation: pitches) | `Note::pitch`, `tpc`, `tpc_transposed` (`.../model/note.rs:263-270`) | supported (`import/src/events.rs:1169`) | supported (`export/src/pitch.rs:73`) | Written pitch uses `tpc_transposed`; reader works in the configured note-name language. |
| Durations, dots, `4*3/2` multipliers (Notation: durations) | `Duration`, `DurationType` (`.../model/duration.rs:37-164`), `ChordRest::ticks` (`.../model/chord.rs:323-348`) | supported (`import/src/lexer.rs:180`) | supported (`export/src/duration.rs:56,108`) | `ChordRest::ticks` carries stretched durations. |
| Rests `r`, full-bar `R1*3`, spacer `s` (Notation: rests) | `Rest::full_measure`, `Rest::gap` (`.../model/chord.rs:297-315`), `Measure::multi_measure_rest` (`.../model/measure.rs:76`) | supported | supported (`export/src/duration.rs:117`) | Invisible rests map to spacer rests on output. |
| `\partial`, irregular bar lengths (Notation: time signatures) | `Measure::len`, `nominal_len`, `is_irregular` (`.../model/measure.rs:59-61,135-137`) | supported | supported (`export/src/structure.rs`) | Local `Timing.measureLength` is used for non-acknowledged irregular bars. |
| `\key`, `\time`, `\clef` (Notation: key/time/clef) | `KeySig` (`.../model/segment.rs:118-129`), `TimeSig` (`:166-179`), `Clef` (`:377-382`) | supported (`import/src/events.rs:1221`) | supported (`export/src/pitch.rs:135,149,166`) | Only known clefs are emitted; historic clefs are approximated. |
| Bar checks `\|` and `\bar "…"` (Notation: bar lines) | `Measure::start_bar_line`, `end_bar_line` (`.../model/measure.rs:63-65`), `BarLineType` (`segment.rs:413-441`) | supported (`import/src/events.rs:1368`) | supported (`export/src/voice.rs`) | `\|` is emitted only when the real length matches the current metre. |
| `\repeat volta N`, `\alternative` (Notation: repeats) | `Measure::repeat_count` (`.../model/measure.rs:70`), `SpannerType::Volta`, `VoltaInfo` (`.../model/repeat.rs:265-270`) | supported (`import/src/events.rs:835`) | supported (`export/src/structure.rs:89`) | Nested repeats and multiple ending numbers are not fully covered (`docs/agents/lilypond.md` §7). |
| `\tuplet 3/2 { … }` (Notation: tuplets) | `Tuplet` (`.../model/tuplet.rs:53-81`), `ChordRest::tuplet` (`chord.rs:334`) | supported (`import/src/events.rs:773`) | supported (`export/src/chord.rs:424`) | A tuplet whose modelled end is inconsistent is closed at the bar line. |
| `\grace`, `\acciaccatura`, `\appoggiatura`, `\afterGrace` (Notation: grace notes) | `GraceType`, `GraceChord` (`.../model/chord.rs:87-104,264-270`) | supported (`import/src/events.rs:825`) | supported (`export/src/chord.rs:50`) | `\afterGrace` without a chord warns on input. |
| Ties `~`, slurs `()` (Notation: ties/slurs) | `Note::tie_forward`/`tie_back` (`.../model/note.rs:271-276`), `SpannerType::Slur` (`.../model/spanner.rs:270`) | supported | supported (`export/src/spanners.rs:41`) | Overlapping slurs are numbered `\=N(`. |
| Articulations, ornaments (`-.`, `->`, `\trill`, `\mordent`, `\fermata`) (Notation: expressive marks) | `Articulation` (`.../model/chord.rs:125-210`), `Ornament`, `Fermata` (`.../model/notation.rs:48-179`) | supported (`import/src/events.rs:1397,1405`) | partial (`export/src/chord.rs:299,345`) | `stress`, `unstress`, exact soft accents and ornament accidentals are approximated or dropped. |
| Tremolo `:8` / `\repeat tremolo` (Notation: tremolo) | `Tremolo`, `TremoloKind` (`.../model/notation.rs:187-208`) | partial | partial (`export/src/chord.rs:50`) | Two-note tremolo is written as `:N` on both chords, not `\repeat tremolo`. |
| `\arpeggio`, `\arpeggioArrowUp/Down` (Notation: arpeggios) | `Arpeggio`, `ArpeggioType` (`.../model/notation.rs:216-236`) | supported | supported (`export/src/chord.rs:50`) | |
| Dynamics `\p`, `\mf`, `-\markup` (Notation: dynamics) | `Dynamic` (`.../model/annotation.rs:197-225`) | supported (`import/src/events.rs:1226`) | supported (`export/src/annotations.rs:46`) | Custom dynamics become markup; placement uses `^`/`_`. |
| `\tempo "Allegro" 4 = 120` (Notation: tempo) | `Tempo`, `TempoKind` (`.../model/annotation.rs:322-364`) | supported (`import/src/events.rs`) | partial (`export/src/annotations.rs:46`) | Writer always emits the quarter-note unit; a metronome-only label or invisible tempo loses information. |
| `\new Lyrics \lyricsto` words, `--`, `__` (Notation: lyrics) | `Lyrics`, `LyricsSyllabic` (`.../model/lyrics.rs:36-63`) | supported (`import/src/lexer.rs:477`) | supported (`export/src/voice.rs`) | Only the first syllable per verse of a chord is written `\skip`-aligned. |
| `\mark`, `^\markup`, `_\markup` (Notation: text marks) | `RehearsalMark`, `StaffText`, `SystemText`, `Harmony` (`.../model/annotation.rs:231-449`) | supported | supported (`export/src/text.rs:45`) | `RichText` markup (`<b>`, `<font>`, `<sym>`) is stripped to plain text; chord symbols are `\markup`, not `\chordmode`. |
| `\chordmode`, `\figuremode`, `\drummode` (Notation: specialized modes) | `Harmony`, `FiguredBass` (`.../model/annotation.rs:428-449`, `.../model/figured_bass.rs:118-165`) | missing (ignored with a warning) | partial / missing | `Harmony` is written as markup; figured bass is not emitted at all. |
| `\<`/`\!`, `\ottava`, `\sustainOn`, `\startTrillSpan`, `\glissando` (Notation: spanners) | `SpannerType::Hairpin/Ottava/Pedal/Trill/Glissando` (`.../model/spanner.rs:268-287`) | supported | supported (`export/src/spanners.rs:41`) | Text lines lose their drawn rule; only their begin/end text is kept. |
| `\paper`, `\layout`, Scheme expressions (Learning: input files / Extending) | `StyleSettings` only page-level fields (`.../model/style.rs:48-101`) | partial | missing | The pivot has no per-element layout; output leaves an empty `\layout {}`. |
| `q` (repeated chord), `\parallelMusic` (Notation/Learning) | `Chord`, `Track` (`.../model/chord.rs:216`, `.../model/ids.rs:123`) | partial (`import/src/events.rs:969`, `parser.rs:1703`) | missing | Unsupported voices/chords are warned about and skipped, not materialized. |

## Model gaps

Ranked by musical importance:

1. **Figured bass and specialized modes** (`\figuremode`, `\chordmode`,
   `\drummode`): the model has `FiguredBass` and `Harmony`, but the LilyPond
   reader ignores the modes and the writer emits chord names as plain markup
   with no chord grid or realization.
2. **Complete articulation/ornament fidelity**: `stress`, `unstress`,
   spiccato, soft-accent variants and `<accidental-mark>` on ornaments have no
   exact LilyPond spelling in the current writer.
3. **Note-head fill and shape tied to duration**: `NoteHeadType` is not forced
   on output; only non-default `NoteHeadGroup` values are tweaked.
4. **Per-element layout, `\paper`/`\layout`, text positions and computed system
   breaks**: `StyleSettings` and `Credit` hold only global page values.
5. **Tempo unit and invisible tempo**: `Tempo` has `beats_per_second` but no
   beat unit, so `\tempo` is always written with `4 = bpm`; invisible tempos
   are lost.
6. **Crossed-staff chords** (`Chord::staff_move`), MIDI drum-set mapping and
   microtonal pitches are not expressible in the current LilyPond output.
7. **Lyrics on tied-note continuations and multiple syllables per verse**:
   `\lyricsto` skips tied notes, so those syllables cannot be represented.
