# ABC: reading and writing in scoretk

Crates `crates/scoretk-abc-common`, `crates/scoretk-abc-import` and
`crates/scoretk-abc-export`, the facade's `abc-import` and `abc-export` features,
enabled by the CLI.
This document describes the covered ABC subset, the mapping to the
pivot model, the measured results and the known limits. The project core
remains in `AGENTS.md`; the facade is described in `docs/agents/scoretk.md`.

## 1. Origin and licence

MuseScore Studio has **no** native ABC import or export: the reference
copy `ref-musescore/` contains only `src/importexport/midi` and
`src/importexport/musicxml`. There is therefore no MuseScore C++ code to
port for the format itself. The crates follow:

- the **ABC 2.1 standard**, <https://abcnotation.com/wiki/abc:standard:v2.1>;
- the behaviour of **`xml2abc.py`** (MusicXML → ABC) and **`abc2xml.py`**
  (ABC → MusicXML) by Willem Vree:
  - `xml2abc.py` version **177**, `Copyright (C) 2012-2025: W.G. Vree`;
  - `abc2xml.py` version **269**, `Copyright (C) 2012-2026: Willem G. Vree`;
  - downloaded on 2026-11-17 from
    <https://wim.vree.org/svgParse/xml2abc.html> and
    <https://wim.vree.org/svgParse/abc2xml.html> (`xml2abc.py-177.zip`,
    `abc2xml.py-269.zip`);
  - header: "Lesser GNU General Public License as published by the Free
    Software Foundation", **without a version number**. LGPL 2.1 § 3
    permits conversion to GPL; the project is distributed under
    **GPL-3.0-only** (see `NOTICE`).

No line of these scripts is copied: the implementation is a Rust
rewrite. The scripts serve as a behaviour reference, as
`musicxml2ly` does for `scoretk-lilypond-export`.

## 2. API and files

```rust
pub fn write_abc(score: &Score) -> Result<String, Error>;
pub fn read_abc(src: &str) -> Result<Score, Error>;
pub fn read_abc_tune(src: &str, tune: usize) -> Result<Score, Error>;
pub fn read_abc_with(src: &str, options: &ReadOptions) -> Result<ReadOutput>;
```

The code is split across **three crates**: a shared base and one crate per
direction. None does I/O; all compile for `wasm32-unknown-unknown`.

| Crate | Contents |
| --- | --- |
| `crates/scoretk-abc-common` | `error.rs` (errors, warnings, `Result`), `pitch.rs` (pitches, key signatures, clefs, modes, `AccidentalState`), `duration.rs` (`L:` multipliers), `decorations.rs` (dynamics, articulations, ornaments), `text.rs` (annotations and `w:` lyrics) |
| `crates/scoretk-abc-import` | `src/lib.rs` (reading API), `src/reader/mod.rs` (lines, header, `X:` tunes), `src/reader/music.rs` (music line), `src/reader/lyrics.rs` (lyrics), `src/reader/build.rs` (model building) |
| `crates/scoretk-abc-export` | `src/lib.rs` (`write_abc`), `src/writer.rs` (header and orchestration), `src/voice.rs` (writing a voice) |

Sharing is **verified module by module**: every module of
`scoretk-abc-common` is used by both directions. `pitch.rs` mixes helpers
specific to one direction (`tpc_of`, `parse_key`, `parse_clef` for reading;
`note_spelling`, `abc_note`, `key_field`, `clef_name` for writing), but
the whole module remains common because `AccidentalState` and `key_alter` are
shared and the step and accidental tables come from the same source.

| Element | Location |
| --- | --- |
| Reading API | `crates/scoretk-abc-import/src/lib.rs` |
| Writing API | `crates/scoretk-abc-export/src/lib.rs` |
| Errors and warnings | `crates/scoretk-abc-common/src/error.rs` |
| Pitches, key signatures, clefs, modes | `crates/scoretk-abc-common/src/pitch.rs` |
| Durations, multipliers, note values | `crates/scoretk-abc-common/src/duration.rs` |
| Decorations, dynamics, articulations | `crates/scoretk-abc-common/src/decorations.rs` |
| Annotation and lyrics escaping | `crates/scoretk-abc-common/src/text.rs` |
| Writing header and orchestration | `crates/scoretk-abc-export/src/writer.rs` |
| Writing a voice | `crates/scoretk-abc-export/src/voice.rs` |
| Line preparation, header, `X:` tunes | `crates/scoretk-abc-import/src/reader/mod.rs` |
| Music-line parsing | `crates/scoretk-abc-import/src/reader/music.rs` |
| `w:` lyrics | `crates/scoretk-abc-import/src/reader/lyrics.rs` |
| Model building | `crates/scoretk-abc-import/src/reader/build.rs` |
| Reading only (hand-written tunes) | `crates/scoretk-abc-import/tests/lecture.rs` |
| Tune round trip | `crates/scoretk/tests/abc_roundtrip.rs` |
| Corpus `.abc` snapshots | `crates/scoretk/tests/abc_corpus.rs` |
| Round trip and musical core | `crates/scoretk/tests/abc_aller_retour.rs` |
| External tools (ignored by default) | `crates/scoretk/tests/abc_outils_externes.rs` |

## 3. Writing

The header is `%abc-2.1`, `X:1`, `T:`, `C:`, `M:`, `L:1/8`, `Q:` (first
numeric tempo) and `K:` with the clef (`K:C clef=bass`). Multiple voices
are written one per `V:`.

- **One ABC voice per (staff, model voice) with content.** An empty staff
  keeps voice 0. `%%score` follows `abc2xml`'s grammar:
  `(1 2)` groups voices on one staff, `{…}` is a brace group, `[…]` a bracket
  group, a lone number an isolated staff.
- **Durations relative to `L:1/8`**: integer multiplier (`2`, `4`, `3`) or
  fractional (`3/2`, `2/3`); every model duration is representable.
- **Pitches as written pitch** (`pitch - transpose`), `tpc_transposed` if
  valid, plus the ottava offset like `pitch2xml`. An explicit accidental
  is written only if it differs from the key signature or measure state.
- **Barlines**: `|`, `||`, `|]`, `|:`/`:|`/`::`, voltas `[1`, `[1,2`.
- **N-tuplets**: `(p`, `(p:q`, `(p:q:r`; `r` is explicit as soon as a n-tuplet
  contains nested n-tuplets or a note count different from `p`.
- **Chords** `[CEG]`, ties `-`, slurs `( )`, grace notes `{…}`: one
  group per chord, `/` before each acciaccatura note (`{/g}`,
  `{g /a}`, the space preventing the slash from being read as a duration
  fraction); consecutive model grace groups are merged.
- **Decorations**: dynamics `!p!`, articulations `!staccato!`, `!accent!`,
  `!tenuto!`, `!fermata!`, `!trill!`, `!mordent!`, `!arpeggio!`, hairpins
  `!<(!`/`!<)!` and `!>(!`/`!>)!`, fingerings `!1!`.
- **Annotations**: `"^text"` above, `"_text"` below, chord
  symbols `"C"`; system annotations (tempo, rehearsal mark) stay in
  the staff's voice 0.
- **Lyrics**: `w:` after each music line, one line per verse;
  caesuras `-`, extensions `_`, missing syllables `*`, literal hyphen `\-`.

## 4. Reading

The reader reads the **first tune** (`X:`) of a file; `read_abc_tune`
chooses another index. It accepts the writing subset, plus:

- `K:` modes (major, minor, dorian, phrygian, lydian, mixolydian,
  aeolian, locrian): modes other than major/minor are reduced to their
  key signature with a warning (the model does not carry the modal tonic);
- `K:none` (empty key) and `K:HP` (reduced to D major);
- `M:C`, `M:C|`;
- broken rhythm `<`/`>` (up to two levels), shorthand ornaments
  `.`, `~`, `H`, `M`, `P`, `T`, `u`, `v`;
- repeats `|:`, `:|`, `::`, voltas `[1`/`[2`;
- inline fields `[K:…]`, `[M:…]`, `[L:…]`, `[V:…]` and changes of
  key, metre and clef within a measure;
- `%%score`/`%%staves` (first directive only), other `%%` ignored with a
  `Warning`;
- several `X:` tunes.

Structural errors (unknown pitch, zero duration, missing `K:`, unclosed chord)
return `Error` with the line number. Constructions outside the
subset (`&`, `P:`, `U:`, `I:`) produce a warning and are
ignored, so as not to lose a whole tune.

## 5. Mapping to the model

| ABC | Model |
| --- | --- |
| `V:n` + `%%score` | `Part` and `Staff`; one ABC voice per `Track(staff, voice)` |
| `K:` | `KeySig` (`concert_key`, `transposed_key`, `mode`) |
| `M:` | `TimeSig` (`nominal_len`, `TimeSig` segments) |
| `clef=` | `ClefType` and `Staff::default_clef` |
| pitch | `Note::pitch`, `Note::tpc`/`tpc_transposed`, `Accidental` |
| duration + `(p:q:r` | `Duration`, `ChordRest::ticks`, `Tuplet` |
| `-` | `Note::tie_forward`/`tie_back` |
| `( )` | `SpannerType::Slur` |
| `!<(!`/`!<)!` | `SpannerType::Hairpin` |
| `!p!` | `Annotation::Dynamic` |
| `"C"` | `Annotation::Harmony` |
| `"^text"`/`"_text"` | `Annotation::StaffText` |
| `w:` | `Lyrics` per verse |
| `[1`/`[2` | `SpannerType::Volta` with `VoltaInfo::endings` |

## 6. Measured results (2026-11-17)

- **Snapshots**: 56/56 corpus v3/v4 files written to `.abc` in
  `tests/corpus/abc/` and read back without error.
- **Round trip** `.mscx` → ABC → model → MusicXML compared to direct export
  at the **musical core** level (measures, key, metre, clef, pitch,
  note type, dots, voice, staff, ties, n-tuplets, lyrics, repeats):
  **46/56 identical**, listed in `EXPECTED_ROUNDTRIP` of
  `crates/scoretk/tests/abc_aller_retour.rs`. The 10 remaining discrepancies:
  - `clef-1`, `copypaste_parts`, `layout_elements`: clef changed within a
    measure or two clefs at the same tick, which direct export reduces;
  - `moonlight`: long piece with multiple keys/clefs/voices; the
    first discrepancies are filler rests;
  - `testMultiInstrumentPart3`: instrument change and local key;
  - `testWedge4`, `voices`: implicit rests (gaps) that direct export
    renders as `<forward>` and ABC writing as `z`;
  - `linked-ties`: tie on a grace note, with no standard ABC equivalent;
  - `timesig-09`: two-note tremolo (`time-modification` 2:1) not rendered;
  - `voiceswitching`: nested n-tuplets combined with multiple voices;
    pitches/positions stay correct, event order differs.
- **External tools**: `scripts/check-abc-external.sh` compiles
  `crates/scoretk/tests/abc_outils_externes.rs` (features `abc-import` and
  `abc-export`) and runs it
  under `nix shell nixpkgs#abcm2ps nixpkgs#abcmidi`. 46 snapshots and 2 tunes are accepted by `abcm2ps` and
  `abc2midi`; the 10 files in `IGNORED` are rejected by these tools
  (nested n-tuplets, old clefs, repeats without notes), not by the standard.
- **Facade and CLI**: `crates/scoretk/tests/abc.rs` and
  `crates/scoretk-cli/tests/abc.rs` check reading, writing, stdin/stdout
  and `--list-formats`. `abc_roundtrip`, `abc_corpus`, `abc_aller_retour` and
  `abc_outils_externes` live in `crates/scoretk/tests/`: they exercise
  **both halves** and the corpus, not an isolated direction.
- **Comparison to `xml2abc.py`** (2026-11-17, `dynamics`): same pitches,
  durations, voices and dynamics; the differences are layout and script-specific
  choices (`L:1/4` instead of `L:1/8`, `I:linebreak`, `nm`/`snm`),
  with no effect on the musical core.

## 7. Known limits

- **Nested n-tuplets** are an extension outside ABC 2.1; `abcm2ps` and
  `abc2midi` reject them. Writing uses explicit `(p:q:r` and the
  reader accepts them.
- **No standard transposition**: ABC has no `transpose=` field in
  the standard. Writing uses written pitch and the reader keeps it
  as concert pitch; a round trip through ABC therefore does not restore
  `<transpose>`.
- **Local key signatures, metres and clefs**: ABC has only one key and one
  metre per tune. Writing takes those of the part's first staff; MuseScore's
  local directions are lost.
- **Layout**: no system breaks, margins, stems, beams,
  noteheads, unlisted articulations, two-note tremolos, pedals,
  ottavas (the pitch shift is applied, the line is not written).
- **Repeats**: play count (`repeat_count` > 2) has no
  ABC syntax; multi-measure voltas are approximated by the first
  barline.
- **Micro-intervals** (`AccidentalType::Other`, quarter tones) not written.
- **`M:none`** is reduced to 4/4; **`K:` modal** loses the modal tonic.
- The reader ignores `&` (superposition), `P:` (parts), `U:` and `I:` with
  a warning.

## 8. Tests and regeneration

```sh
nix develop -c cargo test -p scoretk-abc-common
nix develop -c cargo test -p scoretk-abc-import
nix develop -c cargo test -p scoretk-abc-export
nix develop -c cargo test -p scoretk-abc-common
nix develop -c cargo test -p scoretk-abc-import
nix develop -c cargo test -p scoretk-abc-export
nix develop -c cargo test -p scoretk --features abc-import,abc-export --test abc_aller_retour -- --nocapture
UPDATE_ABC_SNAPSHOTS=1 nix develop -c cargo test -p scoretk --features abc-import,abc-export --test abc_corpus
scripts/check-abc-external.sh
```

`EXPECTED_ROUNDTRIP` is **non-regressive**: a listed file that ceases
to be identical fails `abc_aller_retour`. Snapshots are versioned
in `tests/corpus/abc/` and covered by the `tests/corpus` filter of
`flake.nix`.

## 9. Upgrading `xml2abc.py` and `abc2xml.py`

These scripts are **not** versioned in the repository: download them again
outside the repository (e.g. `/tmp/abcchk/`) to consult them.

1. **Identify the ported version.** Current headers:
   - `xml2abc.py`: `VERSION = 177`, `Copyright (C) 2012-2025: W.G. Vree`;
   - `abc2xml.py`: `VERSION = 269`, `Copyright (C) 2012-2026: Willem G.
     Vree`;
   - downloaded on 2026-11-17, URLs in § 1.
2. **Get a new version.** Download `xml2abc.py-NNN.zip` and
   `abc2xml.py-NNN.zip` from <https://wim.vree.org/svgParse/xml2abc.html>
   and <https://wim.vree.org/svgParse/abc2xml.html>, then `unzip` into a
   temporary directory. Check the licence header: LGPL 2.1 or 3,
   even "only", remains convertible to GPL-3.0 (LGPL 2.1 § 3, LGPL 3 § 3);
   only an explicitly incompatible licence (proprietary, CC-BY-NC…)
   would require sticking to the ABC 2.1 standard alone (see `NOTICE`).
3. **Spot the relevant changes.** Compare the files
   (`diff -u old/xml2abc.py new/xml2abc.py`) and watch:
   - `VERSION`, the `note_ornamentation_map` tables, decorations
     (`!staccato!`, `!trill!`, dynamics);
   - `%%score`/`%%staves` and the `( )`, `{ }`, `[ ]` grammar;
   - `K:`, `M:`, `L:`, `Q:`, `w:` fields, modes and clefs;
   - n-tuplets (`(p:q:r`), broken rhythm and repeats.
   The corresponding Rust files are `decorations.rs`, `pitch.rs`,
   `duration.rs`, `writer.rs`/`voice.rs` (writing) and `reader/music.rs`,
   `reader/build.rs` (reading).
4. **Compare outputs.** For a corpus sample (`xml2abc`
   expects MusicXML, `abc2xml` expects ABC):
   ```sh
   nix develop -c cargo run -q -p scoretk-cli -- tests/corpus/v4/dynamics.mscx -o /tmp/dynamics.musicxml
   python3 /tmp/abcchk/xml2abc_177/xml2abc.py /tmp/dynamics.musicxml -o /tmp/abc-ref
   diff -u /tmp/abc-ref/dynamics.abc tests/corpus/abc/v4_dynamics.abc
   python3 /tmp/abcchk/abc2xml_269/abc2xml.py tests/corpus/abc/v4_dynamics.abc -o /tmp/xml-ref
   ```
   Compare differences that change the **musical core**, not
   layout (`L:`, spacing, `I:linebreak`, `nm`/`snm` are script-specific).
5. **Regenerate and revalidate.**
   ```sh
   UPDATE_ABC_SNAPSHOTS=1 nix develop -c cargo test -p scoretk --features abc-import,abc-export --test abc_corpus
   nix develop -c cargo test -p scoretk-abc-common -p scoretk-abc-import -p scoretk-abc-export
   nix develop -c cargo test -p scoretk --features abc-import,abc-export --test abc_aller_retour -- --nocapture
   scripts/check-abc-external.sh
   nix develop -c cargo test --workspace
   nix flake check
   ```
   Review the snapshot diff and update `EXPECTED_ROUNDTRIP` (an
   entry is removed only with a documented discrepancy) and § 6 of this document.
6. **When to do it.** Only if the ABC 2.1 standard evolves, if a
   behaviour disagreement blocks a real file, or during an annual
   reference review. Do not follow every `xml2abc.py` release:
   the project's compliance target remains MuseScore 4.7.4 and the pivot model.

## 10. Upgrading MuseScore

The crate ports no MuseScore C++ file: it depends on the **pivot
model** (`crates/scoretk-core/src/model`) and its readers. A MuseScore
upgrade therefore follows `docs/agents/upstream.md` (commit, version, corpus
regeneration, e2e validation). On that occasion, check that `ref-musescore/`
still has no native ABC export; if one appeared, it would be a
new behaviour source to declare in `NOTICE`.

## 11. Upgrading the ABC 2.1 standard

The crate follows the **ABC 2.1 standard** (stable version published by the
ABC community, <https://abcnotation.com/wiki/abc:standard:v2.1>), not a
reference implementation. There is therefore no upstream code to compare,
but a specification to reread and revalidate.

1. **Followed version.** ABC 2.1, no sub-version: this is the version cited
   in the headers of `crates/scoretk-abc-common/src/` files and in
   `NOTICE`. The `%abc-2.1` prefix written by `writer.rs` follows from it.
2. **Where to get the specification.** The page in § 1 (abcnotation.com wiki),
   or its offline archive. Version changes are announced on the
   ABC forum/mailing list of abcnotation.com; keep a dated copy outside
   the repository (e.g. `/tmp/abcstd/`) for the diff.
3. **Spot the relevant changes.** Compare the new version to
   2.1 and watch the sections matching the Rust files:
   - pitches, key signatures, modes, clefs: `pitch.rs`;
   - `L:`, `M:`, durations, n-tuplets, broken rhythm: `duration.rs`;
   - decorations, dynamics, articulations: `decorations.rs`;
   - header fields, `w:` lines, `%%` directives: `writer.rs`,
     `reader/mod.rs`;
   - music syntax (ties, slurs, chords, grace notes, barlines,
     repeats): `reader/music.rs` and `voice.rs`.
   A new feature is implemented only if it changes the **musical core**
   or unblocks a real file; layout remains out of scope.
4. **Revalidate.**
   ```sh
   nix develop -c cargo test -p scoretk-abc-common -p scoretk-abc-import -p scoretk-abc-export
   nix develop -c cargo test -p scoretk --features abc-import,abc-export --test abc_aller_retour -- --nocapture
   UPDATE_ABC_SNAPSHOTS=1 nix develop -c cargo test -p scoretk --features abc-import,abc-export --test abc_corpus
   scripts/check-abc-external.sh
   nix develop -c cargo test --workspace
   nix flake check
   ```
   Review the snapshot diff: a standard extension must not
   change the output of already-conformant files.
5. **When to do it.** When version 2.2 (or later) becomes the stable
   standard, or when `abcm2ps`/`abc2midi` accept a syntax the reader
   rejects on a real file. Do not follow drafts: the reference
   remains 2.1 until an explicit decision is made (updating
   `NOTICE`, the `%abc-2.1` prefix and this document).

## 12. External verification tools

`abcm2ps` and `abc2midi` (nixpkgs packages `abcm2ps` and `abcmidi`) serve as
acceptance checks, not as a behaviour source: no line of
their code is followed. Their versions are **not pinned** (the script
`scripts/check-abc-external.sh` takes them from the current nixpkgs), since they
determine neither the output nor the snapshots. If a file in `IGNORED`
becomes accepted, remove it from the list; if a conformant file becomes
rejected, first check that the output is still readable by the internal
reader before concluding a regression.
## 13. Measurement against `abc2midi` (phases 0-1, 2026-11-19)

This section is in English by convention for the reliability measurement
reports. No scoretk code was changed for this baseline.

### 13.1 Oracle, alternative, scripts

- **Oracle**: `abc2midi` 5.03 (nixpkgs `abcmidi`) → MIDI. It is called with
  `-NGRA` (no grace notes) and `-NGUI` (no guitar-chord strumming from chord
  symbols such as `"C"`, which are harmony annotations, not written notes).
  Repeat barlines are replaced by plain bars and `P:` field lines are removed
  on a copy, so **written** notes are compared, not notes replayed by repeats.
  A non-zero exit code is accepted when a non-empty MIDI file was written.
- **Alternative to beat**: `abc2xml.py` 269 (Willem Vree, outside the
  repository), measured with the same oracle.
- **Scripts**: `scripts/oracle/abc/{mesure.sh,mesure.py,cmp.py,one.sh,aggreg.py}`
  (GPL-3.0-only headers, no third-party data or script copied into the repo).
  `cmp.py` computes **N1** (multiset of pitches) and **N2** (pitch + attack
  quantized to 1/24 of a quarter note) with the Python standard library only.

### 13.2 Lots and result

Three local lots, 1 402 files / 4 756 `X:` tunes: `abcjs-tests` (271 files,
MIT), `music21-corpus` (1 116 files; `essenFolksong` excluded), and
`abc-2.1-standard` (15 fragments, NC: local measurement only, never copied
into the repository). One tune is one measurement; files without `X:` are
counted but excluded from the note comparison. Raw outputs are in
`/tmp/abc-mesure/final2/` (not versioned).

| Lot | Tunes | Exact (tk / alt) | Ref notes | N1 correct tk | N1 correct alt | N2 tk | N2 alt | Failures tk / alt |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| abcjs | 272 | 105 / 120 | 2 448 | 1 595 (65.2 %) | 2 250 (91.9 %) | 46.7 % | 71.3 % | 143 / 124 |
| music21 | 4 465 | 2 810 / 3 601 | 478 557 | 374 404 (78.2 %) | 466 816 (97.5 %) | 37.7 % | 80.0 % | 805 / 0 |
| ABC 2.1 (NC) | 19 | 1 / 8 | 1 007 | 258 (25.6 %) | 910 (90.4 %) | 17.6 % | 56.2 % | 6 / 6 |
| **Total** | **4 756** | **2 916 / 3 729** | **482 012** | **376 257 (78.1 %)** | **469 976 (97.5 %)** | **37.7 %** | **79.9 %** | **954 / 130** |

Files whose *every* tune is exact: **877 / 1 402 (62.6 %)** for scoretk,
**1 095 / 1 402 (78.1 %)** for the alternative. Empty outputs: 23 (tk) vs
20 (alt). The alternative is ahead by **19.4 points on N1** and **42.2 on N2**;
it never failed when the oracle succeeded (its 130 failures are the 109 files
without `X:` and the 21 tunes rejected by `abc2midi`). The baseline to beat for
the ABC reading campaign is therefore N1 78.1 % / N2 37.7 %, target 97.5 % /
79.9 %.

### 13.3 Ten largest groups of gaps (scoretk, unless noted)

Counts are tune measurements; example paths are under
`/home/jacques/programmes/musique/corpus/`.

1. **Syntax rejected by the reader (824)** — mostly unexpected `` ` `` (336),
   inline ending `1` (195), `k` (92), quarter-tone `/` (58), invalid pitch
   after `|` (33). Example: `abcjs-tests/extracted-abc/flattener.test__abcRepeat.abc`
   X:1 (inline `1`).
2. **Missing notes (645)** — written notes absent from the MusicXML, partly
   from ornaments on chords. Example: `abcjs-tests/extracted-abc/misc.test__abcChordClass.abc` X:1.
3. **Mixed missing and extra notes (148)** — many come from per-voice
   `transpose=` ignored by the reader. Example: `abcjs-tests/extracted-abc/flattener.test__abcTranspose.abc` X:1.
4. **No `X:` field (109 files)** — ABC fragments, not comparable. Example:
   `abc-2.1-standard/fragments/example-010.abc`.
5. **Pitch offset: MIDI transpose, octave clefs, local key (48 + 4 + 2)** —
   `%%MIDI transpose -2` is applied by `abc2midi`, not by scoretk. Example:
   `abcjs-tests/extracted-abc/flattener.test__abcMidiOptions.abc` X:1.
6. **Oracle cannot convert the tune (21)** — `abc2midi` limitation, not a
   scoretk error; affects both candidates. Example:
   `abcjs-tests/extracted-abc/parsing.test__abc3.abc` X:360 (bare `[V:]` lines).
7. **Oracle has no notes (14)** — empty reference, affects both candidates.
   Example: `abc-2.1-standard/fragments/example-007.abc` X:2.
8. **Truncated output (11)** — e.g. drum tracks counted by `abc2midi` and
   absent from MusicXML. Example:
   `abcjs-tests/extracted-abc/flattener.test__abcMetronome.abc` X:1.
9. **Empty scoretk output (9)** — a global `%%stretchlast 1` makes the reader
   emit measures with no notes (reduced locally to a 6-line file). Example:
   `abcjs-tests/extracted-abc/wrap.test__abcShortMeasures.abc` X:1.
10. **Percussion / unpitched notes (3)** — drum maps have no MusicXML pitch.
    Example: `abcjs-tests/extracted-abc/flattener.test__abcSnare.abc` X:1.

Rarer residual groups: extra notes (2), rhythm/attack only (0 for scoretk,
1 for the alternative), ornament pitches (0 / 1).

### 13.4 Reproducing

```sh
BIN=./target/release/scoretk
ALT=/tmp/abcchk/abc2xml_269/abc2xml.py          # outside the repository
SORTIES=/tmp/abc-mesure/nouveau

scripts/oracle/abc/mesure.sh liste.txt "$SORTIES" "$BIN" "$ALT"
python3 scripts/oracle/abc/aggreg.py "$SORTIES/mesures.tsv"
# one file only:
scripts/oracle/abc/one.sh "$SORTIES/un" "$BIN" air.abc "$ALT"
```

`liste.txt` holds one `lot<TAB>/absolute/path/file.abc` line per file. Every
scoretk launch is bounded by `timeout 10 prlimit --as=2000000000`. Re-run the
complete campaign before merging any reader fix, and require no per-file
regression on N1 and N2 (see `docs/agents/plan-fiabilite.md`).

## 14. Reader fixes for rejected tunes (2026-11-19)

Each fix is measured with `scripts/oracle/abc/one.sh` on a reduced real-world
tune (one `X:` tune, kept outside the repository). The baseline is section 13:
scoretk N1 78.1 % / N2 37.7 %. Fixes are committed one by one, in decreasing
order of frequency. A construction we do not understand must produce a warning
and a local skip, never the loss of the whole tune.

### 14.1 Back quotes between notes (336 rejected tunes)

- Reduced file: X:1 extracted from `music21-corpus/music21/corpus/airdsAirs/book1.abc`
  into `/tmp/abc-reduits/backtick.abc` (back quotes between `B2` and `G2`,
  and between `FA` and `FD`).
- Behaviour: ABC 2.1 § 4.7 states that back quotes between beamed notes are
  typographic spacers that programs must ignore; `abc2xml.py` also skips them
  as recoverable characters.
- Fix: `reader/music.rs` skips U+0060 in the main parse loop and before every
  note, including notes inside chords and grace notes.
- Measurement: N1/N2 **0 % → 100 %** (69/69 notes), no regression.

### 14.2 Inline first/second endings (195 rejected tunes)

- Reduced file: `/tmp/abc-reduits/volta1.abc`, X:1 extracted from
  `abcjs-tests/extracted-abc/flattener.test__abcRepeat.abc`
  (`...|1"G"g4 fedc:|...`).
- Behaviour: ABC 2.1 § 4.9 allows `|1` and `:|2` as short forms of `|[1` and
  `:[2` (a space is required only in front of the explicit bracket).
  `abc2xml.py` accepts digits after a barline as volta numbers.
- Fix: `reader/music.rs` reads a volta number list after any barline and records
  it on the measure that follows; the bracketed and inline forms now share the
  same `finish_bar` path.
- Measurement: N1/N2 **0 % → 100 %** (13/13 notes), no regression. The unit
  test compares the inline form with the bracketed form note by note.

### 14.3 Redefinable symbols such as `k` (92 rejected tunes)

- Reduced file: `/tmp/abc-reduits/k-seul.abc`, X:1 extracted from
  `music21-corpus/music21/corpus/ryansMammoth/BirdOnTheWingJig.abc`
  (`vkd2k^d2 ...`, `uk(d2.D) ...`).
- Behaviour: ABC 2.1 § 4.16 reserves H-W, h-w and `~` for symbols defined by
  `U:` (`U: k = !trill!`); `abc2xml.py` parses any of these letters as a
  decoration. An undefined symbol is only a warning, not a fatal parse error.
- Fix: `reader/mod.rs` stores `U:` definitions; `reader/music.rs` maps defined
  symbols to known decorations, keeps the standard default meanings
  (`H` fermata, `M` mordent, `P` inverted mordent, `T` trill, `u`/`v` bowings,
  `~` turn) and warns + skips undefined ones, including inside chords and
  grace notes.
- Measurement: N1/N2 **0 % → 100 %** (35/35 notes), no regression. Two real
  files from the same corpus now parse end to end under `timeout 10`.

### 14.4 Quarter-tone slash (58 rejected tunes)

- Reduced file: `/tmp/abc-reduits/quarter.abc`, X:1 extracted from
  `abcjs-tests/extracted-abc/flattener.test__abcQuarterTones.abc`
  (`^/A^/B^/c`, `_/A_/B`, and the `K: C ^/f _/B _A ^D` header).
- Behaviour: `abc2xml.py` extends the standard with microtonal accidentals
  (`^/`, `_/`, `^3/2`); with its default `edo=12` it rounds them to whole
  semitones when writing MusicXML pitches.
- Fix: `reader/music.rs` consumes the optional numerator, slashes and
  denominator after an accidental, keeps the semitone already read and emits a
  warning. The note is no longer lost; this also applies inside chords and
  grace notes.
- Measurement: N1 **0 % → 84.4 %** (76/90 notes), N2 **0 % → 71.1 %**; the
  alternative reaches only 80.0 % / 71.1 % on the same reduced tune. The
  residual `TRANSPOSITION` classification comes from explicit accidentals in
  `K:` being ignored by the shared key parser (both candidates), not from a
  rejection. No per-file regression.

### 14.5 Invalid pitch after a barline: invisible bars (33 rejected tunes)

- Reduced file: `/tmp/abc-reduits/invisible.abc`, X:1 extracted from
  `music21-corpus/music21/corpus/ryansMammoth/AbithaMugginsFavoriteReel.abc`
  (the `...[|]|]` ending).
- Behaviour: ABC 2.1 § 4.8 defines `[|]` (and `[]`) as an invisible bar line and
  allows liberal barline sequences such as `[|`. `abc2xml.py` recognises them
  as barlines; previously `[` started a chord and `|` was read as a note
  pitch, which rejected the whole tune.
- Fix: `reader/music.rs` recognises `[|]`, `[]` and `[|` in `parse_bracket` and
  closes the measure through the shared `finish_bar` path; ordinary chords
  such as `[CEG]` are unchanged.
- Measurement: N1/N2 **0 % → 98.4 %** (60/61 notes), same as the alternative.
  The one missing note is a grace-note difference affecting both candidates
  (group 2 in section 13.3), not a rejection. No per-file regression.

### Overall campaign after the five fixes

Re-run of the exact three-lot campaign of section 13.4, in the foreground, with
the release CLI built from this worktree (`mesure.sh` list + `aggreg.py`):

| Candidate | Exact tunes | N1 | N2 | Failures | Files fully exact |
| --- | ---: | ---: | ---: | ---: | ---: |
| scoretk (baseline, § 13.2) | 2 916 | 78.1 % | 37.7 % | 954 | 877 / 1 402 |
| scoretk (after fixes) | 3 395 | **92.2 %** | **44.6 %** | **284** | **1 078 / 1 402** |
| alternative (unchanged) | 3 729 | 97.5 % | 79.9 % | 130 | 1 095 / 1 402 |

Per lot, scoretk N1 is 88.8 % (abcjs), 92.4 % (music21) and 25.6 % (ABC 2.1
fragments, unchanged: those files have no `X:`). Rejections `ECHEC_TK` fell from
824 to **154**. The remaining dominant gap is `NOTES_MANQUANTES` (767), i.e.
group 2 of section 13.3, which is outside the scope of this syntax campaign; it
affects the alternative too (781). No per-file regression was introduced: each
change only accepts a previously rejected construct or replaces a rejection by
a local, warned skip.

## 15. Transposition fixes, `NOTES_MANQUANTES` diagnosis, full campaign (2026-11-19)

### 15.1 Cause 3 — `transpose=` in `K:` / `V:` (fixed)

- Reference behaviour: `abc2xml.py` 269 reads `(?:^t=| t=|transpose=)(-?[\d]+)`
  in `K:` and `V:` fields and emits `<transpose><chromatic>N</chromatic></transpose>`
  while keeping the written `<pitch>` unchanged. `abc2midi` sounds written + N.
- Fix: `reader/mod.rs` parses `transpose=`/`t=` from `K:` parameters and `V:`
  fields (invalid values warn and are skipped); `reader/music.rs` stores the
  **sounding** pitch (written + N) in `Note::pitch`, keeping the written spelling
  in `tpc`; `reader/build.rs` sets `Part::instrument.transpose = Transpose::new(0, N)`
  so the MusicXML exporter writes the written `<pitch>` plus `<transpose>`, like
  the alternative.
- Reduced real file: `/tmp/abc-reduits/transpose.abc`, X:1 extracted from
  `abcjs-tests/extracted-abc/flattener.test__abcTranspose.abc` (three voices,
  `V:1 transpose=-2`, `V:3 transpose=4`, chords, rests, ties and barlines).
- Measurement: scoretk N1/N2 **58.3 % → 100 %** (12/12 notes), alternative 100 %.
- Reduced tests: `crates/scoretk-abc-import/tests/transposition.rs`
  (`transposition_k_et_v`, with a no-transpose control case).

### 15.2 Cause 5 — `%%MIDI transpose` (fixed)

- Reference behaviour: `abc2xml.py` routes `%%MIDI transpose N` through the same
  attribute path: one `<transpose>` element with `<chromatic>N</chromatic>`,
  written `<pitch>` unchanged.
- Fix: `reader/mod.rs::handle_directive` parses `%%MIDI transpose N`, stores it
  as the tune default and applies it to already-created voices; other `%%MIDI`
  directives keep their existing warning.
- Reduced real file: `/tmp/abc-reduits/midi-transpose.abc`, X:1 of
  `abcjs-tests/extracted-abc/flattener.test__abcMidiOptions.abc`
  (`%%MIDI transpose -2`, key signature A, chord).
- Measurement: scoretk N1/N2 **50 % → 100 %** (4/4 notes), alternative 100 %.
- Reduced test: `transposition_midi_directive`, covering an invalid value and
  `transpose=0` (no `<transpose>` emitted).

### 15.3 Diagnosis of `NOTES_MANQUANTES` (767 tunes, diagnosis only)

Grouping uses all 767 scoretk `NOTES_MANQUANTES` tunes of the post-syntax-fix
baseline (`final2-full.tsv`); "alt" is `abc2xml.py` on the same tune.

| Sub-cause | Tunes | Reduced real example | alt also loses? |
| --- | ---: | --- | --- |
| Trill / mordent / turn / roll / slide realization | 678 | `flattener.test__abcDecoration.abc` X:1 → `/tmp/abc-reduits/ornament.abc` | **Yes** (alt `NOTES_MANQUANTES`) |
| Tie written between two different pitches (`A-B`) | 78 | `0001-0050.abc` X:1 → `/tmp/abc-reduits/tie.abc` | No (alt OK) |
| Unclosed `!` decoration swallowing the rest of a line | 3 | `book1.abc` X:0143 → `/tmp/abc-reduits/unclosed.abc` | No (alt OK) |
| Grace/tuplet/bowing combinations (mixed) | 5 | `0351-0400.abc` X:399, `0550-0625.abc` X:612, `AnnieHughesJig.abc` X:1 | Yes (alt `NOTES_MANQUANTES`) |
| Ornaments with alt correct | 2 | `0001-0050.abc` X:7 and X:14 | No (alt OK) |
| Other | 1 | `MaidMarianHornpipe.abc` X:1 | No (alt OK) |

- **Ornaments (678, plus 5 mixed).** `abc2midi` is called with `-NGRA`, which
  disables grace notes but **not** ornament realization: a single `Tc` is played
  as C-D-C-D-C. On the reduced `/tmp/abc-reduits/ornament.abc` the oracle has
  20 notes and both scoretk and the alternative write only 6 (N1 30 % for both).
  The MusicXML `<trill-mark>`/`<mordent>` symbol is the faithful written form;
  the extra notes are a performance realization. For these 678 + 5 tunes the
  **oracle reference is what over-counts**, not the reader: matching it would
  mean emitting notes that are not written in the ABC. No fix attempted.
- **Different-pitch ties (78).** `cmp.py` skips MusicXML notes carrying
  `<tie type="stop">`, while `abc2midi` counts both note-ons. On the reduced
  `/tmp/abc-reduits/tie.abc` (`G3-A G2-E2`) scoretk gets 7/8 notes and the
  alternative 8/8: the alternative writes the two notes without a stop tie.
  This is a genuine scoretk/abc2xml behavioural difference, but it is an oracle
  comparator artifact rather than a lost written note. No fix attempted.
- **Unclosed `!` (3).** An orphan `!` makes `reader/music.rs::parse_decoration`
  consume the rest of the line as a decoration name, dropping the notes after
  it; `abc2xml.py` recovers. Reduced example: 11/18 notes for scoretk versus
  18/18 for the alternative. Real reader bug, deferred.
- **Residual (8).** Five mixed grace/tuplet/bowing tunes are also missed by the
  alternative (oracle likely over-counting); three are genuine scoretk-only
  gaps in the `0001-0050.abc` and `MaidMarianHornpipe.abc` collections.

### 15.4 Full campaign after the two transposition fixes

Exact re-run of the three-lot campaign of section 13.4 (1 402 files, 4 756
`X:` tunes), in the foreground in three slices, with the release CLI of this
worktree under `timeout 10 prlimit --as=2000000000`:

| Candidate | Exact tunes | N1 | N2 | Failures | Files fully exact |
| --- | ---: | ---: | ---: | ---: | ---: |
| scoretk (after the five syntax fixes, § 14) | 3 395 | 92.24 % | 44.60 % | 284 | 1 078 / 1 402 |
| scoretk (after transposition fixes) | 3 400 | **92.44 %** | **44.64 %** | **284** | unchanged |
| alternative (unchanged) | 3 729 | 97.5 % | 79.9 % | 130 | 1 095 / 1 402 |

`TRANSPOSITION` fell from 59 to 55 tunes and `OCTAVE` from 5 to 2; the
`NOTES_MANQUANTES` count (767) is unchanged, consistent with section 15.3.
Sixteen tunes improved, mostly the Josquin files of `music21` with
`%%MIDI transpose -12`/`-24` (e.g. `fortunaDunGranTempo.abc` X:2 went from
`TRANSPOSITION` to `OK`) and the two reduced abcjs files.

One per-tune N2 decrease: `abc-2.1-standard/fragments/example-006.abc` X:1
(N1 23 → 25, N2 12 → 2). This NC fragment combines `%%score (T1 T2) (B1 B2)`,
`clef=treble-8`, `middle=d` and `transpose=-24`; its voice names (`T1`, `B1`)
are not ABC voice numbers, so all four voices collapse into one part and the
last `-24` is applied globally. The extra bass transposition raises N1 but
moves previously matching attacks below the reference range. The tune stays in
the same `OCTAVE` class (the alternative also fails it, standard-lot N1 25.8 %),
and no other tune regressed on N1 or N2.

## 16. Phase 1 — faithful port of `abc2xml.py`: oracle cleanup, three reader fixes and N2 diagnosis (2026-11-19)

This section is in English by convention for the measurement and port reports.
No upstream Python script is committed; the copy used is
`/tmp/abcchk/abc2xml_269/abc2xml.py` (version 269). Section 15 left two
"diagnosis only" causes (different-pitch ties, unclosed `!`) and an oracle that
over-counted ornaments. Phase 1 fixes those three items and diagnoses what
remains of the N2 gap. The decision is to **port `abc2xml.py` faithfully**
(function names and structure, upstream references in comments) while still
building scoretk's generic model, not MusicXML directly.

### 16.1 Oracle and reader fixes

- **Oracle — ornaments** (`scripts/oracle/abc/mesure.py`, `sans_ornements()`):
  `abc2midi` realizes `Tc` as C-D-C-D-C (and likewise mordents, turns, rolls),
  but both candidates write only the written note plus an ornament symbol. The
  reference therefore over-counted 678 tunes. `abc2midi` has no switch for this
  (`-NGRA` only covers grace notes), so the ornaments of `abc2xml.py`'s
  `ornMap`/`uSyms` are stripped **from the oracle copy only**; the candidates
  still receive the unmodified tune. On the reduced `ornament.abc` the reference
  goes from 20 to the 6 written notes (both candidates N1/N2 100 %). Documented
  in `scripts/oracle/abc/README.md`.
- **Oracle — ties** (`scripts/oracle/abc/cmp.py`): a `<tie type="stop">` note has
  no MIDI note-on and is not counted, **but its duration must advance the
  timeline**. The old `continue` skipped the duration too, shifting all later
  attacks and accidentally compensating scoretk's `<forward>` padding (19
  apparent N2 drops once the reader tie fix landed). This now measures both
  candidates the same way.
- **Reader — different-pitch ties** (`reader/music.rs::drop_foreign_ties`): port
  of `abc2xml.py::doNotations`, lines 1445–1460 (`if pt == ptup: continue`, then
  `info('tie between different pitches …')`, delete the start tie and turn it
  into a slur). A pending tie whose pitch is absent from the current event is
  now dropped instead of crossing the intervening note. Reduced `tie.abc`:
  scoretk 7/8 → 8/8 notes, N2 0.25 → 1.00, matching the alternative.
- **Reader — unclosed `!`** (`reader/music.rs::parse_decoration`): port of
  `abc2xml.py`'s `long_decoration` (line 374, name contains no space or other
  delimiter and must be closed) and `linebreak` (line 432, `~decorations + '!'`).
  An orphan delimiter is now a one-character line break instead of swallowing
  the rest of the line. Reduced `unclosed.abc`: scoretk 11/18 → 18/18 notes,
  N2 0.61 → 1.00, matching the alternative.

### 16.2 Phase 1 campaign (foreground, three slices, 1 402 files / 4 756 tunes)

Both columns below use the **corrected oracle**; "before" is the pre-fix binary
and "after" the binary with the two reader fixes.

| Candidate | Campaign | Exact tunes | N1 | N2 | Failures |
| --- | --- | ---: | ---: | ---: | ---: |
| scoretk | § 15.4, old oracle | 3 400 | 92.44 % | 44.64 % | 284 |
| scoretk | corrected oracle, before | 3 964 | 94.53 % | 50.12 % | 284 |
| scoretk | corrected oracle, **after** | **4 069** | **94.68 %** | **50.20 %** | 284 |
| `abc2xml.py` | corrected oracle, unchanged | 4 406 | 99.70 % | 92.85 % | 130 |

Per-tune comparison of the same campaign, before vs after: **N1 115 gains / 0
losses**, **N2 75 gains / 2 losses**. The two N2 losses are `book6.abc` X:1091
and X:1110: both are `K:A Minor` tunes with several orphan `!`s where the
alternative itself is far from the oracle (N2 0.63 / 0.46); the fix recovers the
notes after the orphan delimiters with correct pitches (N1 +15 / +28 points)
but their attacks are displaced by the already-known modal-key offset. These
tunes stay in the `TRANSPOSITION` / `ARMURE` classes; there is **no unexplained
per-tune regression**.

### 16.3 N2 gap diagnosis — 2 515 tunes where the alternative is right and scoretk is wrong

Selection: both candidates N1-exact and `statut == ok`, `n2(alt) ≥ 0.8`,
`n2(tk) < 0.9`, `n2(alt) - n2(tk) ≥ 0.2`. Counts are mutually exclusive, assigned
in the order of the table. Examples are real corpus tunes kept outside the
repository; the "minimal" example is the smallest by reference note count.

| Cause | Tunes | Minimal real example (N2 tk / alt) | `abc2xml.py` function(s) to port |
| --- | ---: | --- | --- |
| Short measures padded to nominal length: `reader/mod.rs::finish_measure` uses `actual.max(nominal)`, so the exporter emits `<forward>` and shifts every later attack. `abc2xml.py` writes a measure at its written length. | **2 488** | `tablature.test__bracketPlacement.abc` X:5 (2 notes, 0.50 / 1.00); representative `0811-0899.abc` X:884 (94 notes, 0.49 / 1.00, the short `AGG G2` before a repeat shifts attacks by +12 ticks) | `doMaat` (line 674), `abc_grammar` measure/bar rules (lines 428–436) |
| Tuplets and broken rhythm (`<`/`>`) attack placement | 11 | `timing.test__abcWarp.abc` X:1 (9 notes, 0.33 / 1.00) | `convertBroken` (line 594), `tuplet_start` (line 395), `mkTmod` (line 728) |
| Parallel voices not merged into one measure (scoretk has no `<backup>` where the alternative has one) | 5 | `flattener.test__abcNoChordVoice.abc` X:1 (4 notes, 0.25 / 1.00) | `mergeMeasure` (line 876), `mergeParts` (line 935), `mergePartMeasure` (line 951) |
| Multi-measure rests `Znn`: scoretk emits one padded measure instead of `nn` written rests | 3 | `misc.test__abcMeasureNumbers.abc` X:1 (`Z24`, 1 note, 0.00 / 1.00) | `restActn` (line 554), `note_length` (line 473) |
| Other (overlays `&`, annotation/timing corners) | 8 | `tablature.test__overlay.abc` X:1 (5 notes, 0.20 / 1.00) | `abc_grammar` `voice_overlay` (line 425), `element` (line 433) |

A further **297** tunes of the phase 1 selection have a N1 gap too; they are the
survivors of the § 15.3 table after the three phase 1 fixes removed the ornament
(678), different-pitch tie (78) and unclosed-`!` (3) causes.

### 16.4 Port plan — order, function map, replacement strategy

The reader is replaced **piece by piece**, one `abc2xml.py` function family at a
time in decreasing expected N2 gain. Each step keeps the current Rust API, adds
a focused failing test first, is measured with `scripts/oracle/abc/one.sh` on a
reduced real tune, then with the full three-lot campaign, and is committed
separately. Comments in the ported code cite the Python function and line. The
existing reader is never deleted wholesale: a new module is added per family,
`reader/mod.rs` keeps the orchestration and delegates to it, and the old path is
removed only once the new one matches on its whole reduced set.

| Order | `abc2xml.py` function (lines) | Rust destination | Expected N2 gain |
| ---: | --- | --- | ---: |
| 1 | `doMaat` (674), measure/bar grammar (428–436) | `reader/mod.rs::Voice::finish_measure` (526): use the written `actual` length for non-pickup measures instead of `actual.max(nominal)`; keep the pickup case; audit the exporter's `move_to_tick` so the model, not a forced nominal length, drives `<forward>` | 2 488 tunes (≈ +40 pts) |
| 2 | `restActn` (554), `note_length` (473) | `reader/music.rs::parse_chord_or_note` `Z`/`X` path (268–300): expand `Znn` into `nn` full-measure rests | 3 tunes |
| 3 | `convertBroken` (594), `tuplet_start` (395), `mkTmod` (728) | `reader/music.rs::apply_rhythm` (572) and the tuplet block (615–660): compute written durations exactly as the Python actions do | 11 tunes |
| 4 | `mergeMeasure` (876), `mergeParts` (935), `mergePartMeasure` (951) | `reader/build.rs::groups`/`create_parts` (322–410): keep parallel voices in the same measure and check the exporter writes `<backup>`; part/staff grouping follows `splitHeaderVoices` (779) | 5 tunes (+13 with cause 1) |
| 5 | `abc_grammar` `voice_overlay`/`element` (425, 433) | `reader/music.rs` overlay branch (105): warn + local skip instead of collapsing overlays | 8 tunes |
| done | `doNotations` tie cleanup (1445–1460) | `reader/music.rs::drop_foreign_ties` | phase 1 |
| done | `long_decoration` (374), `linebreak` (432) | `reader/music.rs::parse_decoration` | phase 1 |

Only the **written** representation is ported: ornaments stay symbols
(`<trill-mark>`, `<mordent>`, …), not realized note runs — the corrected oracle
verifies that choice. Performance-only tables (`abc2midi` event handling) are
out of scope. The conformity target remains MuseScore 4.7.4 for scoretk's
exporters; `abc2xml.py` is the reference for the *reading* decisions only.

### 16.5 Step 1 — written measure length (`doMaat`)

**Implementation.** New module `reader/measure.rs` ports `abc2xml.py::doMaat`
(line 674) and the `measure`/`bar_right` grammar (lines 428-436): a measure
keeps its written length `actual` instead of `actual.max(nominal)`; the pickup
case is unchanged (a short first measure was already kept at `actual`) and the
empty-measure fallback still uses `nominal` so the model stays valid.
`reader/mod.rs::finish_measure` now only delegates to `measure::do_maat`.
The model drives the exporter: `write_measure` calls
`move_to_tick(measure.end_tick())`, so with `Measure::len = actual` a short
measure produces no `<forward>` padding — exactly what `abc2xml.py` writes
(it emits no padding element for an implicit/irregular measure). The only
other use of `nominal_len` in the exporter is the pickup beam calculation in
`beams.rs`, which does not touch `<forward>`.

**Reduced real tune.** `0811-0899.abc` X:884 (`/tmp/abc-reduits/short-measure-884.abc`,
94 notes, the short `AGG G2` before a repeat): scoretk N1 100 %, N2
**0.4894 → 1.00**, the alternative stays 1.00. The tablature X:5 reduction is
covered by `crates/scoretk-abc-import/tests/measure_length.rs` (model lengths,
pitches, durations, pickup, EOF and empty-measure cases) and
`crates/scoretk/tests/abc_short_measures.rs` (no `<forward>`, full-measure
control, both failing before the fix).

**Full campaign** (foreground, three slices, same corrected oracle and release
binary as § 16.2, 1 402 files / 4 756 tunes):

| Candidate | Exact tunes | N1 | N2 | Failures |
| --- | ---: | ---: | ---: | ---: |
| scoretk (after phase 1, § 16.2) | 4 069 | 94.68 % | 50.20 % | 284 |
| scoretk (after step 1) | 4 069 | **94.68 %** | **63.00 %** | **284** |
| `abc2xml.py` (unchanged) | 4 406 | 99.70 % | 92.85 % | 130 |

Per-lot N2 for scoretk goes 56.26 → 57.96 % (abcjs), 50.24 → 63.11 %
(music21) and 17.43 → 23.25 % (ABC 2.1 fragments). Per-tune N1 is **unchanged:
0 gains / 0 losses**. Per-tune N2: **2 091 gains / 1 102 losses** (net
+60 348 correctly placed attacks). In the § 16.3 target population of 2 455
tunes the mean N2 goes 0.419 → 0.646 (1 681 gains, 664 losses, 110 unchanged).

The +40 point estimate of § 16.4 was optimistic: the old `<forward>` padding
was also compensating for scoretk's own *step 3* timing shortfall. For broken
rhythm `<`/`>`, slash durations `x/`, tuplets and grace notes, scoretk writes
the attack positions early (the local `<backup>`/duration is wrong, see the
`convertBroken`/`mkTmod` row of § 16.4), so a measure ends early; the old
padding shifted every later measure forward by exactly that shortfall and
accidentally re-aligned them with the oracle. Removing it exposes that
already-known, separately diagnosed error. **1 086 of the 1 102 N2 losses are
in tunes containing broken rhythm, slash durations, tuplets or grace notes;**
the remaining 16 contain backticks, `[G2G,2]` chords, `u`/`v` bowings, voice
switches or multi-part annotations — also known reader limitations, not a new
regression. No plain tune with only notes, rests and barlines regressed. Step 3
(`convertBroken`, `tuplet_start`, `mkTmod`) must land to turn those masked
losses into gains; the conformity target remains MuseScore 4.7.4.

The MuseScore corpus is unaffected: `cargo test -p scoretk --test e2e_corpus`
passes 5/5.


### 16.6 Step 3 — written durations: broken rhythm, slashes, tuplets, grace notes (2026-11-19)

**Implementation.** New module `reader/rhythm.rs` ports the pure duration
calculations of `abc2xml.py` 269: `simplify` (line 590), `doBroken` /
`convertBroken` (lines 594/614), `note_length` (lines 362/473),
`tuplet_start` (lines 395/484/2012), `mkTmod` (line 728 — the actual/normal
pair carried by the model's tuplet ratio) and `doGrace` (line 680 with the
fixed 1/8 or 1/16 grace unit of `mkNote`, lines 1314-1315). `reader/mod.rs`
only declares the module; `reader/music.rs` keeps the character consumption
and delegates the arithmetic to it, each call site citing the Python
function and line. Three reader corrections came out of the port:

1. **Broken rhythm before building the event.** The old reader applied `<`/`>`
   *after* pushing the note: it dotted the previous figure but left
   `voice.tick` at the unmodified advance, so the measure ended short and
   every later attack was early. `apply_rhythm` now consumes the marker
   *before* the event is pushed (port of the `convertBroken` scan, which
   multiplies the written duration of the note preceding the marker and
   stores the other factor for the next one). The attack positions are then
   correct in the model, without a compensating `<backup>`.
2. **All slash characters.** `read_length_text` consumed only the first `/`:
   `e//` was read as `e/`. It now reads `[digits][slashes][digits]` as the
   `note_length` grammar does, so `//` is a quarter divisor and `3//2` is
   3/4 of a unit. `parse_length` delegates to `rhythm::note_length`.
3. **Tuplet defaults and chord lengths.** `(2`, `(4` and `(8` are now
   duoles on 3 (`tuplet_start`, line 2014: `into = 3 if n in [2,4,8] else 2`),
   instead of always being read on 2. Chords also honour the per-note
   lengths inside `[...]` — `convertChord` (line 651) multiplies each note's
   duration by the chord's; `[G2G,2]` is a quarter, not an eighth. This was
   one of the "16 other" losses listed in § 16.5 and was cheap to fix; the
   remaining ones are covered by the per-tune audit below.

**Reduced real tunes.** `timing.test__abcWarp.abc` X:1 (`CD E>F | (3GAB Ac`,
9 notes): scoretk N2 **0.333 → 1.000**, alternative 1.000. A reduction of
`music21-corpus/.../book1.abc` X:0008 (`FA A(G/F/)|…|d>ef>g|aA A2`, 17
notes): both candidates N1/N2 1.000. `book1.abc` X:0142's `G2 [G2G,2]` and
the `(d/e//f//)` phrase are covered by
`crates/scoretk-abc-import/tests/rhythm.rs` (precise pitches, written
durations, absolute attack ticks, measure lengths, a control measure without
rhythm features and the unclosed-chord boundary); all four tests fail before
the port.

**Full campaign** (foreground, three slices, same corrected oracle and
release binary as § 16.2, 1 402 files / 4 756 tunes):

| Candidate | Exact tunes | N1 | N2 | Failures |
| --- | ---: | ---: | ---: | ---: |
| scoretk (after step 1, § 16.5) | 4 069 | 94.68 % | 63.00 % | 284 |
| scoretk (after step 3) | **4 129** | **96.22 %** | **89.61 %** | **216** |
| `abc2xml.py` (unchanged) | 4 406 | 99.70 % | 92.85 % | 130 |

Per-tune comparison of the same campaign, step 3 versus **master** (before
step 1, § 16.5): **N1 68 gains / 0 losses**, **N2 3 382 gains / 25 losses**
(net +185 753 correctly placed attacks). In the § 16.3 target population the
step-1 losses are essentially recovered: of the 1 102 N2 losses exposed by
step 1, 1 073 are now gains again (8 are back to master's value).

**The 25 remaining per-tune N2 losses versus master, explained individually.**
Every one of them has (a) at least one *short written measure* (the written
note durations do not fill `M:`), and (b) **exactly the same N2 as
`abc2xml.py` on the same tune**. Master's `<forward>` padding of those short
measures happened to re-align their attacks with the oracle; the faithful
written-length reading used here — and by the alternative — does not pad.
This is the short-measure cause of § 16.3/§ 16.5, not a step-3 regression and
not a placement error of the ported functions. List (file, X:, N2 after step 3
= N2 of the alternative, drop versus master):

| File | X: | N2 (tk = alt) | Drop vs master |
| --- | ---: | ---: | ---: |
| `0550-0625.abc` | 587 | 0.438 | 0.137 |
| `0051-0100.abc` | 71 | 0.154 | 0.123 |
| `1476-1555.abc` | 1517 | 0.315 | 0.120 |
| `0550-0625.abc` | 600 | 0.126 | 0.102 |
| `book4.abc` | 0766 | 0.219 | 0.102 |
| `0351-0400.abc` | 385 | 0.500 | 0.091 |
| `0051-0100.abc` | 76 | 0.500 | 0.087 |
| `1476-1555.abc` | 1531 | 0.492 | 0.063 |
| `0550-0625.abc` | 578 | 0.354 | 0.062 |
| `0101-0200.abc` | 175 | 0.395 | 0.061 |
| `0051-0100.abc` | 54 | 0.288 | 0.061 |
| `1476-1555.abc` | 1495 | 0.508 | 0.048 |
| `1556-1576.abc` | 1563 | 0.007 | 0.034 |
| `0051-0100.abc` | 59 | 0.446 | 0.033 |
| `0051-0100.abc` | 74 | 0.475 | 0.025 |
| `americanfifeopus.abc` | 130 | 0.640 | 0.020 |
| `0401-0486.abc` | 462 | 0.508 | 0.017 |
| `1476-1555.abc` | 1513 | 0.516 | 0.016 |
| `americanfifeopus.abc` | 16 | 0.391 | 0.014 |
| `americanfifeopus.abc` | 116 | 0.391 | 0.014 |
| `0550-0625.abc` | 552 | 0.338 | 0.014 |
| `0201-0300.abc` | 286 | 0.178 | 0.010 |
| `1577-1624.abc` | 1608 | 0.276 | 0.009 |
| `1577-1624.abc` | 1584 | 0.344 | 0.008 |
| `0626-0700.abc` | 670 | 0.403 | 0.007 |

No plain tune with only notes, rests and barlines regressed. The MuseScore
corpus is unaffected: `cargo test -p scoretk --test e2e_corpus` passes 5/5.

### 16.7 Remaining port — mode names, rejected tunes, Znn, named voices, overlays (2026-11-19)

This subsection is in English by convention for the measurement reports. The
porting rules are unchanged: the upstream Python script is **not** committed,
every ported decision cites its Python function and line in a comment, and each
step adds its own failing test before the implementation. The baseline is the
end of §16.6 (step 3): N1 **96.22 %**, N2 **89.61 %**, 216 failures, 4 129 exact
tunes. The alternative (`abc2xml.py` 269) reads N1 **99.70 %**, N2 **92.85 %**,
130 failures, 4 406 exact tunes.

#### Diagnosis of the remaining gap (before this campaign)

| Gap | Tunes | Cause / `abc2xml.py` function to port |
| --- | ---: | --- |
| N1 (alt right, tk wrong) | 261 | `K:E Minor`, `K:A Dorian`, … : `parse_key` read only the first whitespace token, so `E Minor` became E **major** (4 sharps instead of 1). Port of `doHeaderField` (lines 1808-1820, regex `([A-G][#b]?)\s*([a-zA-Z]*)`). |
| Rejected tunes (tk fails, alt OK) | 85 | 27 ties separated from the note by a space or broken rhythm (`tie`, line 380), 10 `y` spacers (`basenote`/`noteActn`, lines 367/549), 9 orphan accidentals/line breaks (`errors`, line 431), 8 unknown `K:` forms (`doHeaderField`, lines 1808-1851), 5 stray symbols in chords/graces (`chord_note`, line 385), plus bare tempo lines, additive meters and inline endings. |
| N2 (alt right, tk wrong) | 20 | Multi-measure rests `Z24` written as one padded rest instead of 24 rests (`fixSlurs.f`, lines 767-775). |
| N2 | 17 | Parallel named voices collapsed into one part (`%%score (S A) (T B)`), so no `<backup>` was written (`splitHeaderVoices`, line 779; `voiceNamesAndMaps`, line 2253; `mergeMeasure`/`mergeParts`/`mergePartMeasure`, lines 876/935/951). |
| N2 | 7 | Overlays `&` written sequentially instead of simultaneously (`voice_overlay`, line 425). |

#### Steps landed (one commit each, failing test first, full campaign after each)

1. **Separated mode names.** `parse_key` now joins a recognised mode word to the tonic even when a space separates them, and treats an unknown three-letter mode as major (Python line 1817). New test `tests/key_modes.rs`. Full campaign: N1 96.22 → **97.62 %**, N2 89.61 → **90.96 %**, **261 N1 gains / 0 losses**, 216 failures unchanged.
2. **Rejected-tune causes.** The parse loop now recovers misplaced characters as warned local skips (Python `errors`, line 431), accepts `.-` and separated ties, `y` spacers, digits after invisible bars, decorations/rests inside chords, orphan accidentals, unknown `K:` values, additive meters (`2+3/8`, line 1800) and pre-`K:` bare lines. New test `tests/tolerance.rs`. Full campaign: N1 97.62 → **99.46 %**, N2 90.96 → **92.64 %**, **80 N1 gains / 0 losses**, failures 216 → **135**.
3. **`restActn` / `Znn`** (`fixSlurs.f`, lines 767-775): `Z24` expands to 24 full-measure rests. New test `tests/multi_rests.rs`. Four tunes improved, 12 attacks, no loss.
4. **`mergeMeasure` / `mergeParts` / `mergePartMeasure`**: named `V:` voices receive unique IDs and `%%score` resolves names as well as numbers; overlays and named voices share the correct staff. New test `tests/named_voices.rs`; `tests/measure_length.rs` updated to the now-distinct named voices. About 20 N2 gains, no unexplained loss.
5. **`voice_overlay` (`&`)**: each overlay opens a voice on the same staff at the first note's tick, with the exporter emitting `<backup>`; overlay voices are reused and their empty measures have zero length so a pickup is not stretched. New test `tests/overlays.rs`. Four tunes improved (one pickup-overlay regression found and fixed before the final campaign).
6. **Last five rejections**: stray symbols inside chords/graces and microtonal numerators after an accidental are skipped locally. New test `tests/chord_noise.rs`.

#### Final three-lot campaign (foreground, 1 402 files / 4 756 tunes)

| Candidate | Exact tunes | N1 | N2 | Failures | Files fully exact |
| --- | ---: | ---: | ---: | ---: | ---: |
| scoretk (§ 16.6 baseline) | 4 129 | 96.22 % | 89.61 % | 216 | 1 132 / 1 402 |
| scoretk (after this campaign) | **4 446** | **99.58 %** | **92.81 %** | **131** | **1 175 / 1 402** |
| `abc2xml.py` (unchanged) | 4 406 | 99.70 % | 92.85 % | 130 | 1 147 / 1 402 |

Per-tune comparison against the § 16.6 master: **N1 345 gains / 1 loss**, **N2 371 gains / 0 losses** (net +15 095 correctly placed attacks). The single N1 loss is the NC fragment `abc-2.1-standard/fragments/example-006.abc` X:1 (OCTAVE, 25 → 23 notes): its four named voices are now distinct, so the per-voice `transpose=-24` no longer applies to the two treble voices as an accidental global transposition. This is the faithful per-voice behaviour (the alternative also fails the tune, standard-lot N1 25.8 %), not a regression of a ported function. No plain tune with only notes, rests and barlines regressed.

#### Remaining gap

scoretk is now within **0.12 point of N1 and 0.04 of N2** of `abc2xml.py`, with one more rejection (131 vs 130; 109 files without `X:`, 21 oracle failures shared by both, and one remaining `"` inside a quoted chord annotation in `MyLadysGoonHasGairsOntStrathspey.abc` X:1). The residual N1 gap is 9 empty outputs from `%%stretchlast 1`, 12 partial mismatches, 4 transposition/clef cases and 2 octave cases, all already classified in § 13.3/§ 15.3 and outside this syntax port. The MuseScore corpus is unaffected: `cargo test -p scoretk --test e2e_corpus` passes 5/5.

#### 16.7.1 Last rejection — text volta `["Coda"` (2026-11-20)

This subsection is in English by convention for the measurement reports. The
**frozen baseline** is the release binary of § 16.7 (reader unchanged since
commit `6969f0a`); its campaign data is `rejectfix-full/mesures.tsv`, which
reproduces the § 16.7 table exactly. No upstream Python script is committed.

| Gap | Tunes | Cause / `abc2xml.py` function to port |
| --- | ---: | --- |
| Rejection where the alternative succeeds | 1 | `MyLadysGoonHasGairsOntStrathspey.abc` X:1: `||["Coda" e3g…`. `abc2xml.py`'s `volta_text` (line 417, `Suppress('[') + pas(r'"[^"]+"')`) and `bar_left` (line 424) read `["Coda"` as a *text-labelled ending*. scoretk read `[` as a chord, dropped the opening quote and then rejected the remaining `"`/`Coda` characters as invalid pitches (`LigneInvalide`, `hauteur « " » invalide`). |

ABC 2.1 § 4.9 defines numeric endings (`[1`, `[2`) and is **silent** on
text-labelled endings; the construct is an `abc2xml.py` extension, so the
Python grammar is the behaviour source here. The model has no field for a
volta text, so the fix follows the usual "warn + local skip" rule: consume the
whole `["…"` token, keep the notes that follow, and emit a `Warning`.

- **Fix** (`reader/music.rs::parse_bracket`): a `[` followed by `"` consumes
  the quoted text through its closing quote (at least two characters per
  loop turn) and returns; `[|`, `[]`, `[1` and `[K:…]` are checked first and
  are unchanged.
- **Reduced tune** (the extracted X:1 as-is): scoretk N1 **0 → 1.00**, N2
  **0 → 0.9556**, exactly the alternative's N1/N2 on the same tune (135/135
  notes, 129 attacks placed).
- **Test** (`crates/scoretk-abc-import/tests/chord_noise.rs`):
  `volta_avec_texte_entre_crochets` combines ordinary music (barlines, a
  `[CEG]` chord, slurs, an inline `"4"` annotation, a tie), the text volta
  followed by notes, and a text volta at end of file; it asserts precise
  pitches, written durations and the measure count. It fails on the frozen
  reader with `hauteur « " » invalide`.
- **CLI smoke test**: three real `ryansMammoth/*.abc` files, each under
  `timeout 10 prlimit --as=2000000000`, all exit 0 with non-empty MusicXML.

Full campaign (foreground, three slices, 1 402 files / 4 756 tunes, same
corrected oracle and release binary as § 16.7, every launch under
`timeout 10 prlimit --as=2000000000`):

| Candidate | Exact tunes | N1 | N2 | Failures |
| --- | ---: | ---: | ---: | ---: |
| scoretk (frozen baseline, § 16.7) | 4 446 | 99.58 % | 92.81 % | 131 |
| scoretk (after the text-volta fix) | **4 447** | **99.61 %** | **92.84 %** | 139 |
| `abc2xml.py` (unchanged) | 4 406 | 99.70 % | 92.85 % | 130 |

Per-tune comparison against the frozen baseline: **N1 1 gain / 0 loss**,
**N2 1 gain / 0 loss** — the single gain is `MyLadysGoonHasGairsOntStrathspey.abc`
X:1, now identical to the alternative. The failure count rises from 131 to 139
without any note or attack loss: eight tunes that already produced **empty**
output (N1 = N2 = 0, classified `VIDE` in the baseline) are now classified
`ECHEC_TK`. All eight contain a `%%` directive before `X:1`; the pre-existing
check added by commit `1ac3b83` (`EnTeteIncomplet("K:")` when `seen_key` is
false) turns their previously silent empty output into a rejection. The
text-volta fix never applies to them (their ABC has no `["` sequence), so this
is not a regression of this fix; it is a separate line-preparation limitation
of `prepare()` (see the continuation subsection below). No plain tune with
notes regressed.

#### 16.7.2 Fields after a continued line, and ranked remaining N2 gaps (2026-11-20)

This subsection is in English by convention for the measurement reports. The
**frozen baseline** is the release binary of the previous subsection (after
the text-volta fix); its campaign data is `textvolta-full/mesures.tsv`.

**Cause — bare `L:` field after a `\` continuation (281 lost attacks).**
ABC 2.1 § 2.2.6 (local standard, line 1529) states that "any information
fields and stylesheet directives are processed at the point where the physical
line-break occurs", and warns (line 349) that abc files cannot generally be
parsed by pre-processing continuations into single lines. `abc2xml.py` does
exactly that: `r2.match(x2)` recognises a field independently of `mcont` and
`escField(fln)` emits it as an inline field (lines 803-818). `prepare()` in
`reader/mod.rs` instead appended the following field line to the continued
music line, so `b4 (b/2a/2f3)…| L:1/16` lost its `L:` and all later notes were
read at the old unit.

- **Fix** (`reader/mod.rs::prepare`): if the line after a `\`-continued music
  line is a field (`X:` style) or a `%%` directive, the music line is emitted
  first and the field follows as its own line; ordinary music continuations
  are still joined.
- **Reduced real tune** (`0626-0635.abc` X:631): scoretk N2 **0.093 → 0.987**,
  N1 0.987 unchanged, exactly the alternative's values (151 reference notes).
- **Test** (`crates/scoretk-abc-import/tests/continuations.rs`): `L:1/16` and
  `L:1/8` after `\`, precise pitches/durations/measures, an ordinary music
  continuation as control, and a field at end of file; fails on the frozen
  reader.
- **Full campaign** (same three slices, corrected oracle, all launches
  bounded): scoretk N2 **92.84 → 92.86 %**, N1 99.61 % unchanged, 1 N2 gain /
  0 losses, exact tunes and failures unchanged. scoretk is now **0.01 point
  ahead** of `abc2xml.py` (92.85 %) on N2.

| Candidate | Exact tunes | N1 | N2 | Failures |
| --- | ---: | ---: | ---: | ---: |
| scoretk (frozen baseline, § 16.7.1) | 4 447 | 99.61 % | 92.84 % | 139 |
| scoretk (after the continuation fix) | 4 447 | 99.61 % | **92.86 %** | 139 |
| `abc2xml.py` (unchanged) | 4 406 | 99.70 % | 92.85 % | 130 |

**Ranked remaining N2 gaps** (tunes where `abc2xml.py` places strictly more
attacks, grouped by the ABC construct at the first diverging attack; "lost"
is `sum(alt.com2 − tk.com2)` in attacks). These are all far below the
0.3-point-of-N2 threshold of the reliability plan; the two largest causes
above it were fixed in this campaign and in § 16.7.1.

| First cause (construct at first diverging attack) | Tunes | Lost attacks | N2 points | Minimal real example (N2 tk / alt) | Reference to port (not attempted, below threshold) |
| --- | ---: | ---: | ---: | --- | --- |
| Mid-measure multi-measure rest `Z2` (`G4 Z2 \|\|`): the first rest is shortened by the current tick instead of being a full measure | 1 | 146 | 0.031 | `0626-0700.abc` X:637 (0.319 / 0.940) | ABC 2.1 § 4.5; `abc2xml.py` `restActn` (line 554), `fixSlurs.f` (lines 767-775) |
| Broken rhythm `>` and (nested) tuplets `(3(3` attack placement | 2 | 176 | 0.037 | `1625-1700.abc` X:1649 (0.387 / 1.000), `1176-1275.abc` X:1185 (0.372 / 1.000) | ABC 2.1 § 4.4, § 4.13; `convertBroken` (line 594), `tuplet_start` (line 395) |
| Chord durations with per-note multipliers `[F2A2]` | 2 | 89 | 0.019 | `americanfifeopus.abc` X:119 (0.121 / 0.500) | ABC 2.1 § 4.17; `convertChord` (line 651) |
| Inline key change per voice `[K:F]` | 1 | 27 | 0.006 | `parsing.test__abcInlineKeyPerVoice.abc` X:1 (0.438 / 1.000) | ABC 2.1 § 7.3; `doHeaderField` (lines 1808-1851) |
| Octave clefs (`clef=treble-8`, `clef=bass`) | 3 | 17 | 0.004 | `flattener.test__abcOctaveClefs.abc` X:1 (0.000 / 0.818) | ABC 2.1 § 4.6; `doAttr` (lines 1329-1331) |
| Other 1-3 attack cases (long tunes, annotations, layout corners) | 27 | 51 | 0.011 | `book3.abc` X:0556 (0.822 / 0.850) | mixed, no single construct |
| **Total** | **36** | **506** | **0.107** | | |

No fix is attempted for these causes: the largest is worth 0.031 point of
N2, well under the 0.3-point threshold, and scoretk now exceeds the
alternative on N2 overall. No per-tune loss was introduced by the three fixes
of §§ 16.7.1-16.7.2; every measured change is a gain or a pitch-preserving
status reclassification already explained above.

## 17. Writing export: baseline against `xml2abc.py` (2026-11-19)

This section is in English by convention for the reliability measurement
reports. The upstream script is **not** committed: the copy used is
`/tmp/abcchk/xml2abc_177/xml2abc.py` (version 177). The three fixes below are
faithful ports of its behaviour; each cites the Python function and line in a
Rust comment, and each landed with its own failing test first.

### 17.1 Oracle

`scripts/oracle/abc/ecriture.py` (wrapper `ecriture.sh`, lot builder
`lots-ecriture.sh`) measures the **writing** direction against the same
`abc2midi` 5.03 oracle as the reading campaign of § 13. For each source `S`:

- scoretk writes `.abc` directly from `S`;
- `xml2abc.py` writes `.abc` from the versioned `*.ref.musicxml` (corpus),
  MuseScore's MusicXML export (`.mscz`), the MusicXML source itself (`.mxl`/
  `.xml`), or scoretk's MusicXML as a fallback. The `xml_source` column records
  the origin in every row.
- Both `.abc` are played by `abc2midi -silent -NGRA -NGUI` after `mesure.py`'s
  `sans_reprises`/`sans_ornements` cleanup of a **copy**: written notes, played
  once, not repeats or realized ornaments. The resulting MIDI is compared to
  the reference MIDI of `S` with `midi_notes.py` N1/N2/N2d (Q16 grid):
  MuseScore 4.7.4 for `.mscx`/`.mscz`, scoretk's MIDI for MusicXML.
- `abc2midi` error/warning lines and the `.abc` byte count are recorded. Every
  binary runs under `timeout 10 prlimit --as=2000000000`.

Two oracle corrections were needed before comparing campaigns: the `.mxl`
extractor now skips `META-INF/container.xml` and selects the member containing
`<score-partwise>` (otherwise `xml2abc.py` failed on the three 18 598-note
files), and the reference cache of the `mscz03` chunk was completed so both
campaigns use byte-identical reference MIDI/MusicXML. `cmp.py` and `mesure.py`
are reused, not duplicated.

### 17.2 Lots and baseline

Three lots from `scripts/oracle/lilypond/lots-ecriture.sh`: 56 corpus `.mscx`,
174 `.mscz` of `~/Musique` (split into four chunks), 32 genuine MusicXML.
Frozen baseline binary: `/tmp/abc-ecriture/bin/scoretk-base`. "ABC OK" counts
non-empty `.abc` files; "Exact" counts files with N1 = N2 = N2d = 100 %.
Percentages are weighted by reference note counts.

| Lot | Tool | Files | ABC OK | Exact | N1 % | N2 % | N2d % | Errors | Warnings | Mean .abc B |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 20 | 77.84 | 72.11 | 68.40 | 20 | 79 | 298 |
| corpus | xml2abc | 56 | 56 | 23 | 78.96 | 68.23 | 64.79 | 39 | 165 | 338 |
| mscz00 | scoretk | 44 | 32 | 6 | 80.08 | 67.87 | 61.84 | 0 | 2 152 | 3 966 |
| mscz00 | xml2abc | 44 | 44 | 10 | 81.49 | 69.30 | 66.24 | 49 | 1 318 | 4 582 |
| mscz01 | scoretk | 44 | 44 | 1 | 88.79 | 87.57 | 80.13 | 13 855 | 10 072 | 13 565 |
| mscz01 | xml2abc | 44 | 44 | 4 | 92.62 | 92.43 | 85.92 | 13 389 | 859 | 14 938 |
| mscz02 | scoretk | 44 | 44 | 0 | 73.36 | 55.89 | 51.17 | 49 895 | 12 480 | 15 293 |
| mscz02 | xml2abc | 44 | 44 | 0 | 76.25 | 60.86 | 56.29 | 47 825 | 3 979 | 18 562 |
| mscz03 | scoretk | 42 | 32 | 0 | 84.18 | 65.87 | 63.14 | 3 696 | 9 242 | 11 788 |
| mscz03 | xml2abc | 42 | 42 | 4 | 85.65 | 56.18 | 53.84 | 6 079 | 4 052 | 15 114 |
| xml | scoretk | 32 | 32 | 4 | 90.47 | 25.28 | 22.25 | 24 | 8 312 | 13 410 |
| xml | xml2abc | 32 | 32 | 7 | 93.07 | 27.26 | 24.29 | 664 | 6 340 | 13 516 |

Per-file with both candidates valid (N2d): scoretk better **21**, equal 87,
worse **132**. The large error/warning counts of `mscz01`/`mscz02` come from
`abc2midi` rejecting nested tuplets and old clefs (the same files are listed
in `IGNORED` in § 6), not from a silent loss.

### 17.3 Causes and fixes (one commit each)

Files where `xml2abc.py` was better were grouped by first cause:

1. **Instrument transposition missing (fix 1).** `xml2abc.py::doAttr`
   (lines 1278, 1319-1332) writes `transpose=<chromatic + 12*octave-change>`
   in `V:`. scoretk only wrote the written pitch, so `abc2midi` played e.g.
   the steel guitar an octave high and the B♭ clarinet two semitones high
   (N1 0 % on `hairpin_{simple,dynamic_*}.mscx`). The writer now emits the
   field. Reduced test: `crates/scoretk/tests/abc_ecriture_transposition.rs`,
   with a non-transposing voice as control and the null-transposition case.
   Six corpus snapshots gained `transpose=`.
2. **Octave clefs (fix 2).** `xml2abc.py::doAttr` lines 1329-1331
   (`clefOct[n] = -clef-octave-change`) and `mkNote` line 1154
   (`oct += clefOct[...]`) shift the written octave because `abc2midi` ignores
   the `-8`/`+8` suffix of an ABC clef. scoretk wrote the unshifted note, so
   an octave-treble tenor sounded 12 semitones low (N1 58.8 % vs 100 % on
   `Omni_die_dic_Mariae.mscz`). Reduced test:
   `crates/scoretk/tests/abc_ecriture_clef_octave.rs` (both signs, a control
   voice, chords, rests, ties, two measures).
3. **Ottava wrongly emitted as transposition (fix 3).** Fix 1 initially used
   `transpose_at`, which also subtracts the ottava compensation. On
   `copypaste_parts.mscx` this double-counted an octave (N2d −9.09 points).
   `xml2abc.py::doAttr` reads only `<transpose>`, never `<octave-shift>`: the
   writer now uses the instrument transpose alone. The added test
   `ottava_ne_devient_pas_une_transposition` fails before the fix; the
   snapshot of that corpus file is back to its original content.

### 17.4 Final campaign (frozen `scoretk-final3`)

| Lot | Tool | Files | ABC OK | Exact | N1 % | N2 % | N2d % | Errors | Warnings | Mean .abc B |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 23 | 79.01 | 73.66 | 69.95 | 20 | 79 | 299 |
| corpus | xml2abc | 56 | 56 | 23 | 78.96 | 68.23 | 64.79 | 39 | 165 | 338 |
| mscz00 | scoretk | 44 | 32 | 10 | 86.15 | 74.53 | 68.48 | 0 | 2 152 | 3 944 |
| mscz00 | xml2abc | 44 | 44 | 10 | 81.49 | 69.30 | 66.24 | 49 | 1 318 | 4 582 |
| mscz01 | scoretk | 44 | 44 | 4 | 92.42 | 91.61 | 84.08 | 13 855 | 10 072 | 13 538 |
| mscz01 | xml2abc | 44 | 44 | 4 | 92.62 | 92.43 | 85.92 | 13 389 | 859 | 14 938 |
| mscz02 | scoretk | 44 | 44 | 0 | 75.46 | 58.38 | 53.64 | 49 895 | 12 480 | 15 295 |
| mscz02 | xml2abc | 44 | 44 | 0 | 76.25 | 60.86 | 56.29 | 47 825 | 3 979 | 18 562 |
| mscz03 | scoretk | 42 | 32 | 0 | 91.84 | 73.13 | 70.21 | 3 696 | 9 242 | 11 706 |
| mscz03 | xml2abc | 42 | 42 | 4 | 85.65 | 56.18 | 53.84 | 6 079 | 4 052 | 15 114 |
| xml | scoretk | 32 | 32 | 7 | 92.94 | 27.82 | 24.68 | 24 | 8 312 | 13 375 |
| xml | xml2abc | 32 | 32 | 7 | 93.07 | 27.26 | 24.29 | 664 | 6 340 | 13 516 |

Per-file with both candidates valid (N2d): scoretk better **29**, equal 140,
worse **71** (baseline 21 / 87 / 132). Against the frozen baseline, per-file
N2d is **90 gains / 0 losses / 150 unchanged**; no file lost a pitch or an
attack. scoretk now matches or beats `xml2abc.py` on the corpus and on the
XML lot (where the weighted average is dominated by three 18 598-note files
that both tools handle badly), and has closed most of the gap on `mscz00`
and `mscz03`. Its `.abc` output stays 4-23 % smaller than the alternative's.

### 17.5 Remaining losses, by first cause

- **Reader limitation (22 files).** The scoretk `.mscx`/`.mscz` reader
  rejects MuseScore 1.x/2.x (`version MuseScore non prise en charge : 2.06`):
  all the Vivaldi/Pitoni/Palestrina chorales and most accordion files. No ABC
  is written at all. This is a reader issue, outside this writing campaign,
  as in § 13 of `docs/agents/lilypond.md`.
- **Short written measures and attack shifts (~20 files).** On e.g.
  `Buenos_dias,_Mexico_!.mscz` (N1 98.8 % both), `Omni_die_dic_Mariae.mscz`
  and `00- Adeste fideles - choeur.mscz`, pitches are right but a handful of
  attacks are late: `xml2abc.py` writes `$` line breaks and explicit short
  rests where the writer emits `Z` or fill rests, and `abc2midi` pads the two
  forms differently. Porting `doMaat` (line 674) and the measure/bar grammar
  (lines 428-436) of `xml2abc.py` is the next step; no note is lost.
- **Nested tuplets (1 corpus file).** `testTuplets8.mscx`: `(3:2:3` with an
  inner `(5:4` here versus `(3:2:7`/`(5:4:5` in `xml2abc.py`; both are outside
  ABC 2.1 and `abc2midi` accepts neither the same way. The existing reader
  port of `tuplet_start`/`mkTmod` does not cover the writer.
- **Notes missing in both candidates (rest of the losses, mostly `mscz02`).**
  Both N1 values are below 100 % with the same `notes_manquantes` flag; the
  MuseScore reference MIDI realizes ornaments, glissandi and multi-measure
  voltas that neither written ABC contains. This is the oracle artefact
  already diagnosed in § 15.3/§ 16.1, not a scoretk-only regression.

No file was dropped from any lot, no reference was regenerated with another
MuseScore version, and the only snapshots changed are the five legitimate
`transpose=` additions of fix 1.

### 17.6 Second pass: `abc2midi` warnings and remaining N2/N2d (2026-11-19)

This subsection is in English by convention for the measurement reports. The
binary was frozen **before** any edit of this pass (`scoretk-final3`, the
binary of § 17.4) and the final campaign below uses the new frozen
`scoretk-final5`; all launches are bounded by
`timeout 10 prlimit --as=2000000000`. The frozen binaries, manifests and raw
outputs stay outside the repository.

#### 17.6.1 Warning grouping and fixes

`abc2midi` output was captured for the three files of each lot with the most
messages and grouped after replacing digits by `N`. On that sample of 18
files the messages split as follows (left: baseline binary):

| Message text | Count | Cause in our output | Action |
| --- | ---: | --- | --- |
| `Error … Channel limit exceeded` | 11 486 | 17+ `V:` voices in orchestral scores; `abc2midi` has 16 channels. `xml2abc.py` writes the same 17 voices and the same errors (83 on one file). | Not a warning; shared with the reference, documented. |
| `Warning … Line of music without lyrics` | 5 537 | `w:` written only on measures that have lyrics, whereas `xml2abc.py::writePart` (lines 331-355) writes one `w:` per verse after **every** music line of a sung voice. | Fixed (see below). |
| `Warning … instruction !staccato! ignored` | 3 585 | Our `!staccato!`; `note_ornamentation_map` line 47 writes `.` | Fixed (see below). |
| `Warning … instruction !N! ignored` | 3 145 | Fingerings `!1!`; `xml2abc.py` line 1054 writes the same tokens. | Shared with the reference, documented. |
| `Warning … Track N Bar N has N time units …` | 851 | Irregular written measures; `xml2abc.py::doMaat` writes `$` line breaks and explicit short rests. | Partially fixed by the short-rest fix; remainder is a layout choice. |
| `Warning … Different length notes in tuple` | 676 | Explicit `(p:q:r` tuplets of the reader port; `abc2midi` does not support them. | Out of writing scope (ABC 2.1 extension). |
| `Warning … instruction !marcato!`, `!sf!`, `!fz!` | 426 | `note_ornamentation_map` line 46 writes `!^!`; `dynamics_map` (lines 59-71) has no `sf`/`fz`. | Fixed (see below). |
| `Warning … whitespace in field declaration` / `Potentially ambiguous line` | 216 | `:|` at the beginning of a continuation line on a grouped score. | Layout only, left unchanged. |

Three fixes landed, each with a failing test first:

- **Short written measures** (`xml2abc.py::doMaat`, line 674; measure/bar
  grammar, lines 428-436). `Z` is a *full-measure* rest for `abc2midi`: its
  length comes from `M:`, so a short measure written `Z` shifted every later
  attack. Short and pickup measures now write `z` + multiplier; a genuine
  full measure still writes `Z`. A full-measure rest takes its written length
  from `Measure::len` (its `ChordRest` carries no `ticks`). Reduced real tune
  `Omni_die_dic_Mariae__Carlo_Rossini.mscz` (both copies): N2d
  **61.34 % → 100 %**, now exact like the alternative. New test
  `crates/scoretk/tests/abc_ecriture_silences_courts.rs` (two staves, chord,
  short rest, full-measure control, end of file).
- **Decoration tokens** (`xml2abc.py::note_ornamentation_map`, lines 32-57;
  `dynamics_map`, lines 59-71). Staccato is `.` (not `!staccato!`),
  staccatissimo/spiccato `!wedge!`, accent `!>!`, strong accent `!^!`;
  `sf`/`fz` are annotations, not `!…!`. `abc2midi` rejects a short token
  followed by `!…!` (`.!mp!d` is a "malformed note"), so short tokens are
  written last, as in `xml2abc.py`'s `doDirection` → `addChord` order. On
  `00- Il est né le divin enfant – Rutter – Conducteur.mscz`, the 278
  `!staccato! ignored` warnings disappeared with no new error. New test
  `crates/scoretk/tests/abc_ecriture_decorations.rs`; six snapshots changed
  by token rename only.
- **`w:` on every sung line** (`xml2abc.py::writePart`, lines 331-355).
  Verses are collected over the whole voice; each music line containing
  notes gets one `w:` per verse, with `*` where a syllable is missing.
  On the same file the `Line of music without lyrics` warnings fell
  **282 → 82** (the 82 remaining lines contain no notes at all, which
  `abc2midi` cannot attach lyrics to even with a `w:`). New test
  `crates/scoretk/tests/abc_ecriture_paroles.rs`; four snapshots gained
  placeholder lines.

Warning totals of the final campaign (baseline → new): corpus 79 → 51,
mscz00 2 152 → 1 157, mscz01 **10 072 → 4 618**, mscz02 **12 480 → 4 492**,
mscz03 9 242 → 7 578, xml **8 312 → 5 969** (overall 42 337 → 23 865,
−43.6 %). The remaining difference is mostly rest-only lines and the shared
`Channel limit exceeded`/fingering messages.

#### 17.6.2 Final campaign (frozen `scoretk-final5`)

Measured in the foreground, in six chunks, with the same reference caches on
both sides. The alternative's numbers differ slightly from § 17.4 because its
reference cache was completed after that table; within one campaign both
candidates use byte-identical references.

| Lot | Tool | Files | ABC OK | Exact | N1 % | N2 % | N2d % | Errors | Warnings |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 23 | 79.01 | 73.66 | 69.87 | 20 | 51 |
| corpus | xml2abc | 56 | 56 | 23 | 78.96 | 68.23 | 64.79 | 39 | 165 |
| mscz00 | scoretk | 44 | 32 | 10 | 86.15 | 75.08 | 69.10 | 1 | 1 157 |
| mscz00 | xml2abc | 44 | 44 | 10 | 81.49 | 69.30 | 66.24 | 49 | 1 318 |
| mscz01 | scoretk | 44 | 44 | 4 | 92.42 | 91.84 | 85.38 | 13 855 | 4 618 |
| mscz01 | xml2abc | 44 | 44 | 4 | 92.62 | 92.43 | 85.92 | 13 389 | 859 |
| mscz02 | scoretk | 44 | 43 | 0 | 76.27 | 59.54 | 54.72 | 46 172 | 4 492 |
| mscz02 | xml2abc | 44 | 42 | 0 | 78.00 | 62.98 | 58.36 | 40 379 | 3 429 |
| mscz03 | scoretk | 42 | 32 | 2 | 91.84 | 73.27 | 70.38 | 3 699 | 7 578 |
| mscz03 | xml2abc | 42 | 39 | 3 | 92.30 | 72.68 | 70.31 | 5 903 | 2 425 |
| xml | scoretk | 32 | 32 | 8 | 92.94 | 30.85 | 27.53 | 48 | 5 969 |
| xml | xml2abc | 32 | 32 | 7 | 93.07 | 27.26 | 24.29 | 664 | 6 340 |

Per-file with both candidates valid (N2d): scoretk better **30**, equal **154**,
worse **57** (baseline 29 / 140 / 71). Against the frozen baseline binary,
per-file N2d is **33 gains / 1 loss / 242 unchanged and 0 N1 change**; no file
lost a pitch or an attack. The single N2d loss is
`tests/corpus/v3/timesig-05.mscx` (9.09 % → 3.03 %, N1 and N2 unchanged): the
faithful `.` staccato token makes `abc2midi` realize short durations, whereas
the previously ignored `!staccato!` accidentally matched two more reference
durations on a file that already misses 9 of 33 notes (N2 12.12 %) and where
`xml2abc.py` itself only reaches 9.09 %. No per-file regression is unexplained.

#### 17.6.3 Remaining gaps, by first cause

Files where `xml2abc.py` still wins, grouped by the first flag of `midi_notes.py`
(the `octave` flag is set whenever a missing pitch exists an octave away in
the output, so it also catches semitone and channel-limit differences):

| Cause | mscz01 | mscz02 | mscz03 | xml | Diagnosis |
| --- | ---: | ---: | ---: | ---: | --- |
| `octave` | 12 | 23 | 6 | 8 | Mostly the oracle artefact of § 17.5: the reference MIDI realizes ornaments/voltanas that neither written file contains; on `Stella_maris_instruments – SiB.mscz` the two candidates now have the **same** pitch multiset (1 363/1 363), the residual N2d gap is attack placement under `abc2midi`'s channel limit. |
| `attaque_decalee` | 0 | 1 | 1 | 3 | Short written measures and `$` line breaks of `xml2abc.py::doMaat`; partially fixed, the rest needs multi-measure per-line grouping (a layout choice). |
| `notes_manquantes` | 0 | 0 | 0 | 0 | Out of writing scope; shared oracle artefact. |

`mscz03` files where the alternative is exact: `Cantate_Domino__G.O._Pitoni.mscz`
(out of scope, MuseScore 2.x), `Sancta_Maria_Lux_tempestiva…mscz` (N2d
99.72 %, two notes an octave off) and both copies of
`Omni_die_dic_Mariae__Carlo_Rossini.mscz` (**now exact**, fixed above).

**Out of scope — unreadable sources (22 files).** scoretk's reader rejects
MuseScore 1.x/2.x (`version MuseScore non prise en charge : 2.06`), so no ABC
is written: 12 files of `mscz00` (accordion collection,
`Autumn_Leaves_Frank_Marocco`, `Canon_in_D_Pachelbel_-_arranged_for_accordion`,
`Czardas_Monti_-_advanced_version_for_accordion`, `Espana_Cani`,
`Jig_-_Tripping_up_the_Stairs`, `La_foule`, `La_Partida__Venezualan_Waltz__`,
`Spanish_Ladies`, `Triumphal_March`, `ZORBA_THE_GREEK`, `Ad_te_Suspiramus`,
`Hallelujah_-_Haendel`) and 10 of `mscz03` (`AnthemChristmas_Lullaby`,
`Sicut_Cervus__Palestrina`, `Cantate_Domino__G.O._Pitoni`,
`Ave_Verum____Camille_SAINT-SANS`, five Vivaldi `Dixit Dominus` movements and
`Vingt_danses_enfantines_pour_piano`). One further `mscz02` file
(`03- Minuit chretien - Vcl - Cb - Timb.mscz`) has no reference MIDI because
MuseScore hit the 10 s timeout; it is not a scoretk reader failure. None of
these is counted as a cause of the writing gap.

No corpus reference was regenerated with another MuseScore version; the
snapshot changes of this pass are ten decoration-token renames and 13 added
`w:` placeholder lines.

### 17.7 Third pass: the ABC 2.1 clauses behind the messages (2026-11-20)

This subsection is in English by convention for the measurement reports. The
binary was frozen **before** any edit of this pass as `scoretk-final6`; the
campaign below uses `scoretk-final7`. Both campaigns reused the same reference
MIDI/MusicXML caches and every binary ran under
`timeout 10 prlimit --as=2000000000`. All message counts below are from the
single final run (digits replaced by `N` when grouping); the first baseline
run was repeated and its raw journals contain each message twice, so only the
per-file oracle summaries are used for the before/after totals.

#### 17.7.1 Grouping every remaining `abc2midi` message

| Message (normalised) | Final count | Clause of ABC 2.1 / `xml2abc.py` code | Action |
| --- | ---: | --- | --- |
| `Channel limit exceeded`, `All N MIDI channels used up` | ~68 000 | §7 defines one `V:` per voice and leaves staff grouping to `%%score` (§11.1); the 16-channel limit is an `abc2midi` implementation limit. `xml2abc.py` writes `V:` per **staff** from `s.clefs` (lines 431-435): 6 voices on `00- Adeste fideles - choeur.mscz` where scoretk writes 19. | Documented; merging independent voices would lose data in a conforming reader (§7). |
| `Line of music without lyrics` | 14 874 | §5.1: "syllables are not aligned on grace notes, rests or spacers"; a `w:` after a rest-only line aligns nothing. `writePart` (lines 331-355) writes one `w:` per line to please `abc2midi`. | Documented as a tool heuristic; see §17.7.2. |
| `Track N Bar N has N time units …`, `Track N is N.N quarter notes long …` | 3 653 | §3.1.6 `M:`, §4.3 note lengths, §4.8 bar lines. `doMaat` (line 674) and the measure grammar (lines 428-436) write `$` breaks and short rests. | Layout choice; `xml2abc.py` emits **more** of these than we do; no note is lost. |
| `Different length notes in tuple`, `nested tuples` | 2 200 | §4.13 explicitly allows `(p:q:r` with unequal note values and nested groups ("notes of different lengths within a tuplet"). `xml2abc.py` writes the same general form (lines 602-606). | `abc2midi` limitation, documented, not bent. |
| `instruction !N! ignored` (fingerings), `!>!`, `!tenuto!`, `!^!` | 4 963 | §4.14 defines `!0!`–`!5!`, `!>!, !tenuto!`; playback of most symbols is optional ("Player programs may choose to ignore most of the symbols"). `note_ornamentation_map` (lines 32-57) and `dynamics_map` (lines 59-71) write the same tokens. | Shared with the reference, documented. |
| `Slur within slur` | 448 | §4.11: "Slurs may be nested". `matchSlur` (lines 1020-1036) writes nested slurs too. | `abc2midi` limitation, documented. |
| `whitespace in field declaration`, `Potentially ambiguous line` | 238 | §4.8 makes `:|` the end-repeat symbol; the warning is `abc2midi`'s line heuristic. `addBar` (lines 263-278) writes the same `:|`. | Layout only, unchanged. |
| `Unrecognized character: ,` / `N` | 34 | §4.10 allows `[1,3,5-7` endings ("any list of numbers and ranges as long as it contains no spaces"); `addBar` line 276 writes them. | `abc2midi` only handles `[1`/`[2`; documented. |
| `instruction !<u>N</u>! ignored` | 6 (12 in baseline) | §4.14: an undefined symbol is ignored; the `<u>` markup comes from a MusicXML fingering. `xml2abc.py` line 1054 writes the raw fingering text. | Shared artefact, documented. |
| `clef varbaritone not recognized` | 2 | §4.6 lists the clef names and says unknown names are ignored/default to treble. `xml2abc.py`'s `teken` table (line 1275) uses `bass`/`bass3` for F clefs on lines 4/3. | Left unchanged: our reader accepts `varbaritone` (`pitch.rs::parse_clef`); emitting `bass3` would change the clef sign and break the round-trip. Documented. |
| `found [N or like before |:`, `Assuming repeat` | 4 | §4.9: `[1` without an explicit `|:` is legal ("in many abc files the `\|:` may not be present"); `addBar` lines 267-277 writes the same voltas. | `abc2midi` assumption, documented. |
| `Malformed note : expecting a-g or A-G` | **0** (20 in baseline) | §4.20: "<grace notes>, <chord symbols>, <annotations>/<decorations>, …". A staccato dot before `{…}` is not a note. `mkNote` line 1217 opens the grace group before `doDirection` (lines 1040-1054) appends decorations; `doMaat` line 656 joins them in that order. | **Fixed** below. |
| `instruction !invertedmordent! ignored` | **0** (1 in baseline) | §4.14 defines `!uppermordent!` (abbrev. `P`) but not `!invertedmordent!`, and says unknown symbols are ignored. `note_ornamentation_map` line 35 maps `inverted-mordent` to `P`. | **Fixed** below. |

Three fixes landed, each with a failing test first:

1. **Grace notes before decorations** (`voice.rs::render_chord`/`render_item`,
   §4.20). `render_chord` now returns the `{…}` group separately and the voice
   writer emits it before the decoration tokens. On the three
   `Vingt_danses_enfantines_pour_piano*.mxl` files the malformed-note errors
   disappear. Test `crates/scoretk/tests/abc_ecriture_ordre_agrements.rs`
   (acciaccatura + staccato, tie, chord, rest, second staff without grace).
2. **Terminal `w:` lines made only of `*`** (§5.1). A `w:` after the last real
   syllable of a verse no longer aligns anything; it is dropped. Intermediate
   star lines are kept because §5.1 starts the next verse's alignment "at the
   first note after the notes aligned to the previous `w:` field". Four corpus
   snapshots lose 12 such lines. Test
   `crates/scoretk/tests/abc_ecriture_paroles_finales.rs`.
3. **Inverted mordent** (`decorations.rs::ornament_token`, §4.14). The token is
   now `!uppermordent!` and the reader accepts that long name as well; the
   ordinary mordent stays `!mordent!`. Test
   `crates/scoretk/tests/abc_ecriture_mordent_inverse.rs`.

#### 17.7.2 The `w: *` check (§5.1)

On the frozen baseline, 8 312 `w:` lines contained only `*`; 7 317 were
followed by a real syllable **of the same verse** and are therefore required
by §5.1, while 995 were terminal and carried no alignment information. Only
the latter are now omitted (12 lines in four corpus snapshots). A rest-only line never gets a `w:` either:
§5.1 does not align syllables on rests. Removing those lines raises the
`Line of music without lyrics` count by ~2 600: `abc2midi` expects a `w:`
after **every** music line of a sung voice, including lines with no alignable
note. That expectation is not in §5.1, so it is documented rather than paid
for with bytes.

#### 17.7.3 Remaining N2d gaps on mscz01/mscz02, by cause

| Cause | mscz01 | mscz02 | Diagnosis |
| --- | ---: | ---: | --- |
| `octave, notes_manquantes` with equal or nearly equal N1 | 10 | 26 | Oracle artefact of §17.5: the reference MIDI realizes ornaments and multi-measure voltas that neither written file contains. All 11 `mscz01` files where the alternative has a better N1 use the **same** MuseScore MusicXML (`xml_source = mscore`); the residual is attack placement and channel limit, not missing pitches. On `Stella_maris_instruments – SiB.mscz` both candidates now have the same pitch multiset (1 363/1 363). |
| `attaque_decalee` | 0 | 1 | `00- Adeste fideles - choeur.mscz`: N1 100 % for both, N2 82.45 % vs 100 %. At tick 16 the reference has four simultaneous notes; our 19 `V:` exceed `abc2midi`'s 16 channels and two attacks are dropped, whereas `xml2abc.py` writes 6 `V:` (§7 permits one per voice, so this is a tool limit). |
| shared low N1 (~56 %) | 0 | 7 | The seven `Minuit chretien` parts: both candidates miss the same reference notes; outside the writing scope (§15.3). |

#### 17.7.4 Final campaign (frozen `scoretk-final7`)

Same six chunks, same caches, foreground, all launches bounded.

| Lot | Tool | Files | ABC OK | Exact | N1 % | N2 % | N2d % | Errors | Warnings | Mean .abc B |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 23 | 79.01 | 73.06 | 69.31 | 20 | 77 | 297 |
| corpus | xml2abc | 56 | 56 | 23 | 78.96 | 68.23 | 64.79 | 39 | 165 | 338 |
| mscz00 | scoretk | 44 | 32 | 10 | 86.15 | 75.07 | 69.10 | 0 | 1 160 | 3 689 |
| mscz00 | xml2abc | 44 | 44 | 10 | 81.49 | 69.30 | 66.24 | 49 | 1 318 | 4 582 |
| mscz01 | scoretk | 44 | 44 | 4 | 92.42 | 91.84 | 85.38 | 13 855 | 5 384 | 13 626 |
| mscz01 | xml2abc | 44 | 44 | 4 | 92.62 | 92.43 | 85.92 | 13 389 | 859 | 14 938 |
| mscz02 | scoretk | 44 | 44 | 0 | 75.46 | 58.61 | 53.83 | 49 895 | 6 211 | 16 056 |
| mscz02 | xml2abc | 44 | 44 | 0 | 76.25 | 60.86 | 56.29 | 47 825 | 3 979 | 18 562 |
| mscz03 | scoretk | 42 | 32 | 2 | 91.84 | 73.27 | 70.38 | 3 696 | 7 745 | 11 841 |
| mscz03 | xml2abc | 42 | 40 | 4 | 92.22 | 74.15 | 71.84 | 5 903 | 2 432 | 13 088 |
| xml | scoretk | 32 | 32 | 8 | 92.94 | 30.93 | 27.57 | 24 | 6 039 | 12 488 |
| xml | xml2abc | 32 | 32 | 7 | 93.07 | 27.26 | 24.29 | 664 | 6 340 | 13 516 |

Against the frozen baseline, per file: **7 metric gains, 5 losses**. The gains
are the three `Vingt_danses_enfantines_pour_piano*.mxl` files (malformed notes)
and a small duration gain on `Tarentella_Napoletana_Accordion_Solo.mscz`; no
file gained or lost a pitch. The five
losses all come from the §4.20 fix making `abc2midi` finally *realize* a
fermata that the baseline's misplaced token caused it to ignore:
`layout_elements.mscx` (N2 63.64 → 43.94), `copypaste_parts.mscx`
(15.91 → 13.64) and `Il_Postino_accordion_solo.mscz` (34.05 → 33.83, N2d
unchanged). A minimal ABC confirms it: `!fermata!{/=G}E2` gives E a 1-quarter
duration, while the standard-conformant `{/=G}!fermata!E2` gives it 2 quarters
and shifts the next attack. The MuseScore reference and `xml2abc.py` do not
realize that fermata in these measures; this is the same oracle artefact as
§17.5, not a regression of the writer. Oracle totals: errors
67 518 → 67 490 (malformed notes and the unknown mordent gone), warnings
24 018 → 26 616 (the documented `w:` effect); written `.abc` stays 2-19 %
smaller than `xml2abc.py`'s. No corpus reference was regenerated and no file
was dropped from a lot.

### 17.8 Fourth pass: verified warning histogram and two output fixes (2026-11-20)

This subsection is in English by convention for the measurement reports. The
frozen baseline is `scoretk-final6`, the binary built from 5ef2686 (the commit
before `abc-export3`, 4d78e80); the final campaign uses a fresh release build
of this worktree. Both sides reuse the same six-lot manifests, the same
reference MIDI/MusicXML caches and `timeout 10 prlimit --as=2000000000` for
every launch. The pass-3 baseline journals had been accumulated by two runs,
so both sides were rerun cleanly into `/tmp/abc-ecriture/pass4-{base,courant,final}`
before any conclusion was drawn.

#### 17.8.1 Verified cause of the 24 018 → 26 616 increase

The clean baseline reproduces **24 018** warnings for scoretk. On the 240
files readable by both the baseline and current master, master has exactly
**+2 598** warnings (corpus +26, mscz00 +3, mscz01 +766, mscz02 +1 566,
mscz03 +167, xml +70): this is the delta of § 17.7.4, and every one of those
messages is `Line of music without lyrics`. The only relevant change on those
files was the removal of terminal `w:` lines by § 17.7 fix 2.

That removal was legal but not required by the standard. § 5.1 says that `*` is
"equivalent to a blank syllable" and that "the remaining notes have no lyric
(blank syllables)"; it also allows a `w:` field to associate notes that
appeared previously. `xml2abc.py::writePart` (lines 331-355) writes one `w:`
after every music line of a sung voice. The `abc2midi` warning is a tool
heuristic, not a § 5.1 requirement, but the blank-syllable line is conforming
and suppresses it. Terminal lines are therefore written again (commit
fa4c8de). The cost is 12 added `w:` lines in four corpus snapshots, with no
change to pitches, durations or attacks; on the common files master had 26 616
warnings and the final binary has 24 026.

#### 17.8.2 Histogram by message and by file

Digits were replaced by `N`. Counts are scoretk messages of the final run
(with the pre-fix current-master count in parentheses where it differs):

| Normalised message | Final (pre-fix) | Clause / decision |
| --- | ---: | --- |
| `Line of music without lyrics` | 13 709 (16 305) | § 5.1 blank syllables; **fixed** (§ 17.8.1). |
| `Track N Bar N has N time units …` | 10 339 (10 339) | § 3.1.6, § 4.3, § 4.8. On a reduced real file the warning appears only after a `!fermata!` note whose realization by `abc2midi` lengthens the bar; `xml2abc.py` emits more of these than we do. Tool/fermata artefact, documented (§ 17.5, § 17.7.4). |
| `Track N is N.N quarter notes long …` | 888 (888) | Same cause, long-score variant; documented. |
| `instruction !N! ignored` (fingerings), `!>!`, `!^!`, `!tenuto!`, `!D.S.!`, … | 8 574 (7 466) plus 2 680 / 822 / 592 / ~100 | § 4.14 defines these symbols and states "Player programs may choose to ignore most of the symbols"; `xml2abc.py` writes the same tokens. Tool limitation, documented. The 1 108-message rise is the `<font` class below becoming valid `!N!` tokens. |
| `Different length notes in tuple` | 2 224 (2 224) | § 4.13 explicitly allows `(p:q:r` with unequal note values; `abc2midi` does not support it. Tool limitation, documented. |
| `Slur within slur` | 1 319 (1 319) | § 4.11: "Slurs may be nested". Tool limitation, documented. |
| `instruction !<font! ignored`, `!<u><font! ignored` | 0 (1 103) | § 4.14: a symbol name may not contain spaces or `[ ] : |`. MuseScore 2.x stores the fingering `1` as `font size='9'/font … 1`. **Fixed** (a8f3148): only the final fingering symbol is written; `abc2midi` still ignores fingerings, so these messages move to the documented `!N!` class, but the ABC is now valid. |
| `whitespace in field declaration`, `Potentially ambiguous line` | 157 each (157) | § 4.8 makes `:|` an end repeat; `abc2midi`'s line heuristic. Layout only, unchanged. |
| `Unrecognized character: ,`, `clef varbaritone`, `Assuming repeat`, … | < 50 | § 4.10 ending lists, § 4.6 clef names, § 4.9 optional `|:`; documented in § 17.7.1. |

By file, the largest counts are on long lyric-heavy scores
(`Missa_in_Es_Hob._XXII4_II._Gloria_-_Haydn.mscz`: 1 545 `Line of music without
lyrics`; `Magnificat__Prt.mscz`: 598 bar-length messages) and on the three
`Vingt_danses_enfantines_pour_piano*.mxl` (973 fingering messages each);
`Vingt_danses_enfantines_pour_piano.mscz` carried the 973 `!<font!` messages
that a8f3148 removes. No file was excluded from the count.

The `Channel limit exceeded` lines (67 390 for scoretk in the pre-fix run) are
**errors**, not warnings: § 7 permits one `V:` per voice and `abc2midi` has 16
channels; `xml2abc.py` produces the same errors (67 032). They remain
documented as a tool limit, as in § 17.7.1.

#### 17.8.3 Final table per lot (frozen baseline vs final binary)

| Lot | Binary | Files | ABC OK | Exact | N1 % | N2 % | N2d % | Errors | Warnings | Mean .abc B |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | baseline | 56 | 56 | 23 | 79.01 | 73.66 | 69.87 | 20 | 51 | 298 |
| corpus | final | 56 | 56 | 23 | 79.01 | 73.06 | 69.31 | 20 | 53 | 298 |
| mscz00 | baseline | 44 | 32 | 10 | 86.16 | 75.08 | 69.10 | 1 | 1 157 | 3 708 |
| mscz00 | final | 44 | 43 | 10 | 80.91 | 69.02 | 64.38 | 0 | 1 342 | 4 090 |
| mscz01 | baseline | 44 | 44 | 4 | 92.42 | 91.85 | 85.39 | 13 855 | 4 618 | 13 725 |
| mscz01 | final | 44 | 44 | 4 | 92.42 | 91.85 | 85.39 | 13 855 | 4 618 | 13 725 |
| mscz02 | baseline | 44 | 44 | 0 | 75.46 | 58.61 | 53.83 | 49 895 | 4 645 | 16 376 |
| mscz02 | final | 44 | 44 | 0 | 75.46 | 58.61 | 53.83 | 49 895 | 4 645 | 16 376 |
| mscz03 | baseline | 42 | 32 | 2 | 91.84 | 73.27 | 70.39 | 3 699 | 7 578 | 11 913 |
| mscz03 | final | 42 | 42 | 3 | 84.33 | 54.78 | 52.13 | 7 648 | 9 805 | 13 660 |
| xml | baseline | 32 | 32 | 10 | 92.99 | 31.06 | 27.16 | 48 | 5 969 | 12 547 |
| xml | final | 32 | 32 | 10 | 92.99 | 31.14 | 27.21 | 24 | 5 975 | 12 547 |

Total warnings go from 24 018 to 26 438. This is **not** a writer regression:
the frozen baseline predates the MuseScore 2.x reader, so 21 files that it
rejected are now written (11 in `mscz00`, 10 in `mscz03`), with a naturally
lower N1 on pieces such as the Vivaldi/Palestrina chorales; they carry the
extra 2 412 warnings and the lower lot averages. Restricted to the 240 files
readable by both binaries, warnings are 24 018 → 24 026 and total `.abc` bytes
2 242 647 → 2 242 602. The two writer fixes therefore remove the entire
master increase on common files at essentially unchanged size.

#### 17.8.4 Per-file audit

Against the frozen baseline, no file lost a pitch: N1 is unchanged on every
commonly readable file, and 24 files gained notes (the newly readable 2.x
scores) or attacks (the three `Vingt_danses_enfantines_pour_piano*.mxl`). There
are exactly three per-file N2 losses, all already present in § 17.7.4 and
unchanged by this pass: `layout_elements.mscx` (29/42 instead of 42/42),
`copypaste_parts.mscx` (6/11 instead of 7/11) and
`Il_Postino_accordion_solo.mscz` (158/451 instead of 159/451). Each is the
§ 4.20 grace-before-decoration fix making `abc2midi` finally realize a fermata
that the old, malformed token order caused it to ignore; the MuseScore
reference and `xml2abc.py` do not realize it, so this is the oracle artefact of
§ 17.5, not a placement error of the writer.

No corpus reference was regenerated, no file was dropped from a lot, and the
only snapshot changes are the 12 blank-syllable `w:` lines of § 17.8.1.

## 17. Import ABC — campagne 2026-11-21

Oracle ABC rétabli sur les 1 402 fichiers / 4 756 airs (`liste.txt`,
`abc2midi` + `abc2xml.py` 269) : scoretk **4 447 airs exacts**, N1 99,6 %,
alternative 4 406 exacts. Les 125 `NOTES_MANQUANTES` sont **tous** également
manquants chez l'alternative (0 cas spécifique à scoretk) : sur-décompte de
l'oracle sur les ornements, pas un défaut du lecteur. Les 109 `SANS_X` sont
exclus par construction.

### 17.1 Préambule avant le premier `X:` (commit 4679d5e)

`split_tunes` traitait les champs globaux (`M:`, `O:`, `R:`…) précédant le
premier `X:` comme un air séparé, sans `K:`, ce qui rejetait l'air entier.
`abc2xml.py::splitHeaderVoices` (ligne 789) ouvre le premier air sur `X:` et y
inclut le préambule. Deux tests réduits écrits d'abord, échouant avant le
correctif. 8 airs précédemment rejetés (`English.abc`, `Reels.abc`,
`Strspys.abc`, `wrap.test__abcShortMeasures`) sont désormais lus.

### 17.2 Octave des clefs `K:` (commit en cours)

`abc2xml.py::doClef` (lignes 1736-1744) décale toutes les hauteurs d'un ou
deux octaves pour `treble+8`, `bass-8`, `+15`… (`s.gtrans`). Le lecteur ne
prenait que la clef graphique. Test réduit `[K: treble+8]C4|` écrit d'abord,
échouant avant le correctif (60 au lieu de 72). Sur les 4 airs concernés :
**1 air devient exact**, les autres passent de N1 0,09/0,43/0,56 à
0,45/0,71/0,56, aucun cas nouveau en échec.

### 17.3 Campagne complète après les deux causes (2026-11-21)

Comparaison air par air avec la campagne de référence du même jour :
**12 gains N1, 0 perte, 4 743 inchangés**. Airs exacts 4 447 → **4 456**,
N1 global 99,6 → **99,8 %** ; le lot `std` passe de 2 à **9 airs exacts** sur
19 (94,9 % de notes justes, désormais au-dessus de l'alternative à 91,2 %).
scoretk dépasse l'alternative en airs exacts (4 456 contre 4 406) sur ce lot.
Les causes restantes sont dominées par `NOTES_MANQUANTES` (125) et `SANS_X`
(109), toutes deux identiques chez l'alternative ou exclues par construction :
elles ne sont pas imputables au lecteur. La catégorie `OCTAVE` passe de 2 à 3
après la correction partielle des clefs : les 2 airs restants combinent
plusieurs changements de clef inline et demandent un port plus complet de
`doClef` (notamment `clef-octave-change` et `gtrans` par voix).

## 18. N2 (attaques) et N3 (fonctions musicales) — 2026-11-21

### 18.1 N2

N2 est déjà mesuré par `cmp.py` : `abc2midi` fournit les temps en ticks SMF,
l'attaque est convertie en quarts (`tick/tpq`) puis quantifiée au 1/24 de
quart ; le MusicXML cumule `<duration>/<divisions>` en quarts avec
`<backup>`/`<forward>`. Aucune conversion en secondes (tempo/gate d'`abc2midi`)
n'intervient. Sur la campagne de référence, **825 airs N1-exacts ont N2 < 0,99**
(music21 803, abcjs 21, std 1) ; mais seulement 3 airs ont une alternative
correcte (N2 ≥ 0,9) et scoretk mauvais (N2 < 0,8). Le déficit N2 est donc
presque entièrement partagé avec `abc2xml.py` : il vient de l'oracle/tempo,
pas du lecteur. Les 3 écarts spécifiques (X:637, X:1185, X:1649 de music21)
restent à diagnostiquer.

### 18.2 N3 — comparateur et déficits

Nouveau comparateur `scripts/oracle/abc/n3.py` : il extrait de chaque paire
`score.musicxml` / `alt/air.xml` les compteurs de lyrics, liaisons (ties),
slurs, triolets, notes de grâce, ornements, harmonies, reprises, voltas,
armures, indications de mesure, clefs et voix, puis écrit `n3.tsv`.

Déficits par catégorie (airs où scoretk a moins que l'alternative) : slurs
**3 055**, ornements **3 075**, reprises **1 250**, armures 39, indications de
mesure 32, triolets 26, clefs 21 (mais 4 583 de plus : l'alternative n'écrit
presque jamais de `<clef>`), voix 18, voltas 15, paroles 10, liaisons 3.

### 18.3 Cause 1 — slurs (commit a5b14f1)

`add_spanner` (`reader/build.rs`) créait les slurs ABC sans
`SpannerNotation::Slur`. L'export MusicXML n'écrit les `<slur>` que lorsque le
spanner porte cette notation (`notations.rs::slurs`) : **0 fichier avec un
`<slur>`** sur toute la campagne, contre 3 055 chez l'alternative. Port de
`abc2xml.py::doSlurs` (lignes 1566-1580). Test réduit modèle écrit d'abord,
échouant avant le correctif.

### 18.4 Cause 2 — `:|` puis `|:` (commit en cours)

Dans `finish_bar`, une mesure vide ouverte par `|:` écrasait `end_bar` de la
mesure précédente avec `Normal`, effaçant le `:|` qui la fermait. La norme ABC
2.1 §4.9 définit `:|` et `|:` comme deux barres distinctes ; `abc2xml.py`
écrit les deux `<repeat>` (lignes 1566-1603). Le port ne conserve plus que
l'écrasement quand `end` n'est pas `Normal`. Test réduit
`C D:|` / `|: E F|` écrit d'abord, échouant avant le correctif.

### 18.5 Campagne finale N1/N2/N3

Après les deux causes, la campagne complète (1 402 fichiers / 4 756 airs)
donne : **N1 0 gain / 0 perte**, **N2 0 gain / 0 perte**, **N3 : 2 976 airs
gagnent des slurs, 1 077 gagnent des `<repeat>`, 0 perte dans toutes les
catégories**. Les nombres d'airs exacts et les N1/N2 globaux sont inchangés
(4 456, 99,8 %) : ces deux causes ne modifient pas les hauteurs ni les
attaques, seulement les fonctions musicales mesurées par N3. Le comparateur
N3 ne compte pas le conteneur générique `<notations>` comme un ornement
(sinon l'ajout des slurs gonflait faussement la catégorie `decorations`).

Les déficits N3 restants les plus gros sont les ornements (661 airs),
`clefs` (4 582 airs où scoretk écrit une clef et pas l'alternative — choix
différent, pas un manque) et les triolets (26). Les 3 écarts N2 spécifiques
restent à diagnostiquer séparément.

## 19. N3 — ornements (décorations) 2026-11-21

Le lecteur ABC transforme `!fermata!`, `H`, `!trill!`, `.`, `~`… en
articulations/ornements sur la note (`push_event`), mais l'export MusicXML
n'écrivait le `<fermata>` que depuis `Annotation::Fermata` : les points d'orgue
ABC étaient silencieusement perdus (`notations.rs::articulations` sautait
`is_ornament`). Port de `abc2xml.py::doDecos` (lignes 1491-1492) : une
articulation `Fermata*` s'écrit désormais `<fermata type="upright"/>` (ou
`inverted` si l'ancre est basse). Test réduit (articulation Fermata sur un
accord) écrit d'abord, échouant avant le correctif. Sur
`decorations.test__abcFermata` : 12 `<fermata>` produits, comme l'alternative.


## 20. N3 — liaisons entre hauteurs différentes converties en slurs (2026-11-21)

`abc2xml.py::doNotations` (lignes 1445-1460, ABC 2.1 §4.14) signale
« tie between different pitches … converted to slur » : une liaison `-` ouverte
dont la hauteur n'est pas dans la note suivante n'est pas abandonnée mais
transformée en slur d'expression. Le lecteur ne faisait que supprimer la
liaison en attente (`drop_foreign_ties` avertissait « abandonnée ») : les 151
airs N3 en défaut de slurs perdaient ces liaisons.

Le correctif conserve la position d'origine de chaque liaison
(`pending_ties: BTreeMap<u8, (NoteId, Position)>`), retire le `tie_forward`
(pour que le comparateur N1 compte toujours la note, § 15.3) et crée un
`Spanner` `Slur` entre le tick d'origine et le tick de la note courante.
Test réduit `C-D|` écrit d'abord et échouant avant le correctif
(`tests/foreign_ties.rs`). Sur `0001-0050.abc` X:16 (`A4-B2`…) : 24 slurs
start / 24 stop produits, comme l'alternative, contre 0 avant.

## 21. N3 — fin de reprise `:|` suivie d'une barre double `||` (2026-11-21)

Une mesure vide ouverte par la barre double `||` qui suit `:|` écrasait
`end_bar_line` de la mesure précédente (`finish_bar`, branche mesure vide) :
le `:|` devenait `Double`, perdant le `<repeat direction="backward"/>`.
`abc2xml.py::mergePartMeasure` conserve la répétition, comme le montre
`0701-0720.abc` X:708 (7 `:|` suivis de `||` → 7 repeats dans l'alternative,
1 seul dans le port). Le correctif empêche une barre `Double` d'écraser une fin
de reprise déjà posée (ABC 2.1 §4.9). Test réduit
`C D:| / ||E F:| / ||G A:|` écrit d'abord, échouant avant le correctif (1 au
lieu de 3) ; sur X:708, 7/7 repeats produits comme l'alternative.

## 22. Mesure N3 final après les trois correctifs (2026-11-21)

Campagne complète rejouée (`mesure.sh`, 1 402 fichiers, 4 756 `X:` tunes,
binaire de ce worktree) puis `n3.py`. Comparaison air par air avec la référence
`/tmp/abc-n3-final` (avant ces trois correctifs).

Catégories en défaut (scoretk < alternatif), avant → après :

| Catégorie | Avant | Après | Gain |
| --- | ---: | ---: | ---: |
| decorations | 661 | **437** | 224 |
| repeats | 175 | **35** | 140 |
| slurs | 151 | **27** | 124 |

Table N3 complète après : decorations 437 (235 en excès), keys 39, repeats 35,
times 32, slurs 27 (19 en excès), tuplets 26, clefs 21 (4 583 en excès),
voices 18, voltas 15, lyrics 10, ties 3, harmony 2, graces 2.

Aucune perte air par air : sur 4 755 tunes communs, N1 et N2 sont strictement
inchangés (0 gain, 0 perte) et aucune catégorie N3 ne voit son écart se creuser
(0 perte). Les 3 causes corrigées ne touchent que l'écriture des ornements,
des slurs de liaisons invalides et des fins de reprise.
