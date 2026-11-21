# TODO

## Roadmap: the other scoretk formats

**scoretk** is a **bidirectional** conversion toolkit between
five score formats; **scoretk-core** is its first converter
(MuseScore → MusicXML).

| Format | Reading | Writing |
|---|---|---|
| MuseScore (`.mscz` / `.mscx`, 3.x and 4.x) | done | done (`scoretk-musescore-export`) |
| MusicXML (`.musicxml` / `.mxl`) | done (core, `.mxl` in t43, see `docs/agents/musicxml-import.md`) | done |
| LilyPond (`.ly`) | done (subset, t40) | done (`scoretk-lilypond-export`, t36) |
| ABC (`.abc`) | done (`scoretk-abc-import`, t41) | done (`scoretk-abc-export`, t41) |
| MIDI (`.mid`) | done (`scoretk-midi-import`, t42) | done (`scoretk-midi-export`, t35) |

All combinations must work: mscz ↔ musicxml, musicxml ↔ ly,
musicxml ↔ abc, musicxml ↔ midi, and therefore also all pairs between mscz,
ly, abc and midi.

### State of phase 1 (one crate per converter)

| Crate | Role | State |
| --- | --- | --- |
| `scoretk` + `scoretk-cli` | facade (`Format`, detection, `read`/`write`) and `scoretk` CLI | done: reading `.mscz`/`.mscx`/MusicXML/`.mxl`/`.ly`/ABC/MIDI, writing MusicXML/`.mxl`/`.mscx`/`.mscz`/`.ly`/`.mid`/`.abc`; the ten symmetric features (`<format>-import` / `<format>-export` for MuseScore, MusicXML, MIDI, LilyPond and ABC) are enabled by the CLI |
| `scoretk-musicxml-import` | MusicXML reader into the pivot model | done (t34, t37, t43): musical core 26/30 v3 and 26/26 v4, full parity 23/30 and 21/26, `.mxl` read back identically over the whole corpus (discrepancies in `docs/agents/musicxml-import.md`) |
| `scoretk-midi-common` / `scoretk-midi-import` / `scoretk-midi-export` | shared error, MIDI reader and writer from/to the pivot model | done (t35 writing, t42 reading): round trip 21/21 apart from quantisation; import 4/21 identical to the MuseScore core, discrepancies in instrument names and complex n-tuplets (`docs/agents/midi.md`) |
| `scoretk-lilypond-{common,export,import}` | shared tables, LilyPond writer and reader from/to the pivot model | done (t36, t40): 56/56 files compile, core round trip 24/56 (t40) |
| `scoretk-musescore-export` | MuseScore writer (`.mscx` 4.x, `.mscz`) | done (t38): round trip identical over the whole corpus |
| `scoretk-abc-common` / `scoretk-abc-import` / `scoretk-abc-export` | shared ABC tables, ABC reader and writer (`.abc`) | done (t41): 56/56 snapshots, core round trip 46/56, discrepancies in `docs/agents/abc.md` |

MIDI is a special case: it is a performance format, not a notation format. MIDI
writing requires unfolding repeats and jumps (like MuseScore's `RepeatList`),
applying dynamics, articulations and tempo. MIDI
reading requires quantisation (durations, voices, measures, inferred key signature), hence an
approximate conversion: it is done (t42) by porting
MuseScore's import, with its default options; the limits (nested n-tuplets,
instrument catalogue, octave clefs) are in `docs/agents/midi.md`.

Leads:

- **Pivot model.** The internal model (`crates/scoretk-core/src/model`) becomes the
  common pivot. Each format provides a reader into the model and a writer
  from the model, which avoids N×N converters. It will probably need to be
  made less specific to MuseScore. Converting via MusicXML is also possible
  when simpler, but at the cost of cumulative losses.
- **Organisation.** Plan for one crate per format, or features, and a
  `scoretk` crate that groups them together. The CLI infers formats from extensions
  (`scoretk input.ly -o output.mscz`).
- **Existing references.**
  - MusicXML → mscz: MuseScore's MusicXML import
    (`src/importexport/musicxml/internal/import`).
  - MusicXML → ly: `musicxml2ly` (LilyPond).
  - ly → MusicXML: `python-ly`.
  - MusicXML ↔ abc: `xml2abc` and `abc2xml` (W. Vree).
  - MIDI: MuseScore's MIDI export (`src/importexport/midi`, `engraving/compat/midi`), its MIDI import (quantisation), and the Rust crate `midly` (pure Rust, `no_std`) for the file format.
  - Check the licences of each one before any porting.
- **LilyPond.** Writer and reader are done (`scoretk-lilypond-export` and `scoretk-lilypond-import`, features
  `lilypond` and `lilypond-import` of the facade); see
  `docs/agents/lilypond.md`.
  - Writer: voices are written in **`\fixed`** mode, never `\relative`,
    because that mode resists copy-paste errors and similar better.
  - Reader (t40): LilyPond's parser depends on Guile Scheme; we port to
    Rust the grammar of `parser.yy` and `lexer.ll` for a subset
    (`\relative`, `\fixed`, `\transpose`, `\tuplet`, `\repeat`, `\key`,
    `\time`, `\clef`, `\partial`, `\language`, lyrics, simple `\markup`).
    Arbitrary Scheme and unknown commands are ignored with a
    collected warning, following python-ly (`ly.music`).
    Corpus round trip: 24/56 files identical at the musical core,
    3/56 at full parity; the remaining discrepancies come mainly
    from information the writer cannot represent (measure
    subtypes, endings, MIDI channels, chord symbols). Details and
    upgrade procedure: `docs/agents/lilypond.md`.
- **Unchanged constraints.** The library stays portable: wasm for the PWA
  `../cantus`, Android, no fs or threads, and no C dependency.
- **Schedule.** To be undertaken once mscz → MusicXML parity is reached.
  The first deliverable remains the scoretk facade in wasm for cantus.
- **Do not port `read114` (MuseScore 1.x reader) without a real need.** One 1.14 file in a 1 401-file corpus; such files get a clear "unsupported version" error.
