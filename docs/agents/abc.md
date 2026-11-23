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

## 23. N3 — annotations de fin de mesure (2026-11-21)

Une annotation lue après la dernière note d'une mesure — `(gf)"Segno"||` —
restait dans `voice.annotations` : `push_event` ne la consommait qu'à la note
suivante, donc elle dérivait vers la mesure suivante ou était perdue en fin
d'air. `abc2xml.py::convert` (branche `x.name == 'text'`) l'écrit au contraire
dans la mesure courante, juste avant la barre. Le correctif rattache les
annotations en attente au dernier événement dans `finish_bar` (branche mesure
non vide). Tests réduits `C D "Segno"| / E F|` et `C D "Segno"|` écrits
d'abord, échouant avant le correctif (0 au lieu de 1) ; sur
`552ce87b8d6f_t76`, 2/2 `<words>Segno</words>` produits comme l'alternative.

## 24. N3 — annotations de placement vides `"..."` (2026-11-21)

`abc2xml.py` (`text_expression` lignes 402/487, `convert` ligne 2022) traite
`">"` comme une annotation de texte vide placée sous la portée et écrit un
`<words/>`. Le lecteur ABC supprimait le texte vide (`text.is_empty()`) et
l'exporteur supprimait les `RichText` vides : deux silences. Le correctif
conserve l'annotation vide et écrit `<words/>` pour un texte vide. Tests
réduits des deux côtés écrits d'abord, échouant avant le correctif. Sur
`9629d955e64a_t578`, 12 `<words/>` vides produits comme l'alternative.

## 25. N3 — agréments dans les n-olets (2026-11-21)

Dans `(3{a}gfg)`, `abc2xml.py::mkNote` applique `ntup` à toutes les notes du
groupe, y compris la note d'agrément `{a}` : il écrit un `<time-modification>`
sur elle. L'exporteur le supprimait (`grace.is_none()`), perdant 6 des 24
`<time-modification>` de `1556-1576.abc` X:1574. Le correctif écrit le
`<time-modification>` sur toutes les notes ; il reste sans effet hors n-olet
(actual == normal). Test réduit écrit d'abord, échouant avant le correctif
(1 au lieu de 2). Sur X:1574 : 24/24 comme l'alternative, contre 18 avant.

## 26. ABC import : état final (2026-11-21)

### Mesure

Campagne complète rejouée (1 402 fichiers, 4 756 `X:` tunes) avec le binaire
de ce worktree (`/tmp/abc-n3-final4`). Comparaison air par air avec
`/tmp/abc-n3-final3` (avant les trois correctifs ci-dessous).

| Catégorie | Avant | Après | Gain |
| --- | ---: | ---: | ---: |
| decorations | 437 | **213** | 224 |
| tuplets | 26 | **5** | 21 |

Table N3 complète après (airs où scoretk a moins que l'alternative) :
decorations 213, keys 39, repeats 35, times 32, slurs 27, clefs 21,
voices 18, voltas 15, lyrics 10, tuplets 5, ties 3, harmony 2, graces 2.

Aucune perte air par air : N1 et N2 strictement inchangés sur 4 755 tunes
communs (0 gain, 0 perte), et aucune catégorie N3 ne voit son écart se creuser.
`abc2xml.py` lui-même, jugé contre abc2midi, est à N1 96,56 % / N2 90,44 % ;
scoretk est à N1 96,69 % / N2 90,70 % (4 465 contre 4 418 airs exacts en N1).

Unités des chiffres historiques : les « 3 055 slurs / 3 075 ornements » de
§ 18.2 comptaient des **airs** avec un compteur ad hoc qui rangeait le
conteneur générique `<notations>` dans les ornements (2 997 airs had fewer
`<notations>` avant le correctif `doSlurs`). Le comparateur `n3.py` exclut
`<notations>` : le déficit réel d'ornements était 661 airs, puis 437, puis 213.

### Causes corrigées dans cette passe

- Annotations de fin de mesure conservées dans leur mesure (§ 23).
- Annotations de placement vides `">"` écrites comme `<words/>` (§ 24).
- Agréments dans les n-olets gardent leur `<time-modification>` (§ 25).

### Restes classés

- **clefs** (21 manquants, 4 583 en excès) : `abc2xml.py` n'écrit presque
  jamais de `<clef>` ; le port écrit la clef de chaque portée, comme l'impose
  la structure MusicXML (ABC 2.1 § 4.6 « Clefs and transposition »).
- **keys** (39) : presque tous des champs `K:` répétés avec la même tonalité ;
  ABC 2.1 § 3.2 « Repeated information fields » les traite comme un
  remplacement, et le port n'écrit un `<key>` que lors d'un changement réel.
- **multi-voice, voltas, repeats, voices** : les airs concernés
  (`Canzonetta`, `layout.test__abcVoiceEnd`, …) écrivent plusieurs voix ;
  `abc2xml.py` produit une partie par voix (ABC 2.1 § 7), le port les
  rassemble en une seule partie. Les compteurs de reprises et de voltas
  sont donc multipliés par le nombre de voix chez l'alternative.
- **lyrics** (10) : écart de représentation, pas de contenu. Un `_` ABC est
  écrit par `abc2xml.py` comme un `<lyric><extend/></lyric>` sur la note,
  alors que le modèle pivot porte le mélisme comme durée sur la syllabe
  précédente (ABC 2.1 § 5.1). Les deux formes sont valides ; changer le
  modèle casserait le test de round-trip ABC (`abc_aller_retour.rs`).
- **times** (32) : principalement des `[M:x/y]` en ligne au tick 0 d'une même
  mesure ; ABC 2.1 § 3.2 demande un remplacement, le port ne garde que la
  dernière valeur là où `abc2xml.py` émet chaque occurrence.
- **harmony** (2 manquants, 43 en excès) : les 2 manquants sont des échecs
  d'oracle partagés ; l'excès vient de l'interprétation de chaînes entre
  guillemets comme accords, que l'alternative traite comme du texte.

### ABC import: final status (2026-11-21)

Full campaign re-run (`/tmp/abc-n3-import`, 1 402 files / 4 756 `X:` tunes)
against the previous baseline (`/tmp/abc-n3-final4`). Five reader fixes,
each with a reduced test written first and failing before the fix, clippy
clean before each commit:

1. **Fermata on a rest** (`91dc7623`) — `Hz`/`Hx`/`!fermata!z`. Articulations
   were pushed only onto `Chord`; add `AnnotationSpec::Fermata` mapped to
   `Annotation::Fermata` on the segment, which `rest_notations` already
   writes as `<fermata>`. ABC 2.1 §4.3; `abc2xml.py::doDecos` lines
   1491-1492.
2. **Single-note tremolo** (`9fbfeadb`) — `!/!`, `!//!`, `!///!`. Add
   `Decoration::Tremolo(count)` → `Chord::tremolo = Tremolo { Single, count }`.
   ABC 2.1 §4.14; `abc2xml.py:1688` and `abc2xml.py:1464-1469`.
3. **`!lowermordent!`** (`dc418a26`) — the long name (`M`) was dropped;
   map it to `<mordent>` like `!mordent!`. ABC 2.1 §4.14;
   `abc2xml.py:1095` (`ornMap`/`uSyms`).
4. **Repeated `K:` field** (`d84ad02d`) — schedule a key change on every
   `K:` and emit a `KeySig` segment at the measure start even when the key
   is unchanged. ABC 2.1 §3.2; `abc2xml.py` writes a `<key>` per field.
5. **`|:|` combined repeat** (`26be5e1f`) — the trailing `|` was parsed as
   a second empty bar that reset the end barline to Normal, dropping the
   repeat. Consume all three characters as `EndStartRepeat`. ABC 2.1 §4.9;
   `abc2xml.py:428-429` (`bars`).

N3 tunes where scoretk has fewer musical functions than `abc2xml.py`
(before → after): decorations **213 → 185**, repeats **35 → 24**, keys
**39 → 37**, ties **3 → 0**; unchanged: times 32, clefs 21, voices 18,
voltas 15, slurs 27, lyrics 10, tuplets 5, harmony 2, graces 2.
Tune-by-tune: **61 N3 deficits improved, 0 regressed**; N1 unchanged on
4 751 of 4 755 common tunes, **4 gains, 0 losses** (scoretk 4 460 exact
N1 tunes versus 4 406 for `abc2xml.py`). No per-tune loss to explain.
The remaining categories are structural (one part per voice in
`abc2xml.py`, ABC 2.1 §7) or deliberate representation differences
(`<words>` vs `<harmony>`, lyrics melisma, clefs emitted on every staff),
documented above.

### ABC import: second pass — decorations classified (2026-11-21)

Classification of the 185 remaining decoration deficits showed that **all
185 were missing `<words>`** (fermatas, tremolos and mordents were already
fixed), 210 words in total across 55 distinct strings. The three dominant
groups were: text-only `Q:` tempo labels (`Slow` 59, `Moderato` 14,
`Very Slow` 7, `Lively` 6, …), repeat directions starting with A–G
(`End`, `BREAK`, `DANCE`, `left`, …) misread as chord symbols, and
`!D.C.!/!Fine!/!D.S.!` directions. Four reader fixes, each with a reduced
test written first and clippy clean before commit:

6. **Trailing `Q:` tempo label** (`6eede8e1`) — `abc2xml.py::doTempo`
   (line 1886) finds the quoted label anywhere in `Q:`; `parse_tempo` only
   looked for a leading one. Also attach the tempo annotation to a
   `ChordRest` segment instead of `TimeTick`: the exporter only visits
   annotated segments that carry an element. ABC 2.1 §4.7.
7. **Inline `[M:x/y]` fields** (`9f256b86`) — several at the same tick
   (`y` spacers do not advance time) were collapsed into the last value;
   emit one `TimeSig` segment per recorded change. `abc2xml.py::mkMeasure`,
   ABC 2.1 §3.2.
8. **Text beginning with A–G** (`015bb264`) — `looks_like_chord` accepted
   any alphanumeric suffix, turning `"End"` into an empty `<harmony>`;
   validate kind/degrees/bass against `abc2xml.py:411-416` (`chord_sym ^
   text_expression`). ABC 2.1 §4.2.
9. **`!D.C.!/!Fine!/!D.S.!`** (`91e576ff`) — dropped as unknown
   decorations; add `Decoration::Text` for `abc2xml.py:1100` `capoMap`
   entries and route them to annotations so a direction after the final
   note survives. `abc2xml.py:1680-1684`, ABC 2.1 §4.3.

Full campaign re-run (`/tmp/abc-n3-import2` vs `/tmp/abc-n3-import`, 1 402
files / 4 756 tunes): N3 decoration tunes **185 → 23**. The `svg.test
__abcTimeSigList` tune went from 1 to 3 `<time>` (alt 4); other `times`,
keys, slurs, clefs, voices, voltas and lyrics deficits are unchanged and
are structural (`abc2xml.py` emits one part per voice, ABC 2.1 §7) or
representation differences documented above. Tune-by-tune: **542 N3
entries improved, 2 reduced a `<harmony>` excess to parity** (no deficit
regressed); **N1 exact 4 460 / 0 losses / 0 gains** and **0 note-count
changes** across all 4 755 common tunes. No per-tune loss to explain.

### ABC import: third pass — proving the residual classes (2026-11-21)

Before any change, each remaining class was checked against the ABC 2.1
spec (`/home/jacques/programmes/musique/specs/abc/abc-standard-v2.1.wiki.txt`)
and `abc2xml.py` 269. Three tunes per class were inspected (source line,
emission, section). Evidence:

| Class | ABC source (3 tunes) | What `abc2xml.py` emits | What we emitted | Section |
| --- | --- | --- | --- | --- |
| keys | `K: C` in `flattener.test__abcTempoThreeVoices`; `K:GMin` twice in `layout.test__abcNotCollision2`; `K:D` in `book1` X:0020 | `<key>` per field (`abc2xml.py:1829-1854`) plus a default `C` from `abc2xml.py:2423`; one part per voice (`abc2xml.py:2426-2445`) | one `<key>` per part, no default | §3.2 |
| times | `M:2/4 … M:4/4` in `flattener.test__abcAllTimeSigs`; `M: 4/4` in `flattener.test__abcTempoThreeVoices`; `[M:...]` in `svg.test__abcTimeSigList` | `<time>` per field (`abc2xml.py:1795-1808`) + default `4/4` (`abc2xml.py:2423`) | one `<time>` per field, no default | §3.2 |
| clefs | `[K: treble+8]…[K: bass]` in `flattener.test__abcOctaveClefs`; seven `K:...clef=` in `selection.test__abcClefs`; `K:C clef=treble` in `multi-voice.test__abcLyricsTwoVoices` | one `<clef>` per K:/V: field (`abc2xml.py:1734-1744`) | first clef only — **real missing element** | §4.6 |
| slurs | `[E4(e4]c2)` in `book6` X:1062; `(gfg … g2)` in `americanfifeopus` X:18/X:118 | slur from a chord note (`abc2xml.py:371-387`, `:1516`) | slur inside `[...]` dropped — **real missing element** | §4.11 |

Two classes were real reader bugs and were fixed:

- **Mid-tune clef** (`d81e34c7`) — `K:...clef=` after measure 0 was stored on
  `HeaderClef`, which the exporter only reads for the first measure. Use
  `SegmentType::Clef`; the exporter's `mid_measure_clef` then writes it.
  `abc2xml.py:1734-1744`, ABC 2.1 §4.6. Reduced test first.
- **Slur into/out of a chord** (`3aa402c7`) — `(`/`)` inside `[...]` were
  discarded as stray symbols. Handle them as slur open/close in both chord
  loops and `read_note`'s pre-letter decoration skip. `abc2xml.py:371-387`,
  ABC 2.1 §4.11 (explicitly allows slurs "into, out of and between chords").

The one-part-per-voice claim was then made measurable: `n3.py` (`02cdf317`)
adds per-voice columns that divide `<key>`, `<time>`, `<clef>`, `<repeat>`,
`<ending>` counts by the part count (`abc2xml.py:2423,2426-2445`); note-level
categories keep raw counts. On the unchanged corpus this removes 8 keys,
7 times and 2 repeats of pure layout inflation.

Remaining 23 decorations were reclassified: 5 header `Q:` labels erased by
an inline `[Q:]`, 9 annotations between barlines, 4 duplicate `Segno`
annotations, and 5 assorted. Three more reader fixes:

- **Every `Q:` field** (`fc350243`) — `AnnotationSpec::Tempo` per field and a
  separate `header_tempo` so an inline `[Q:]` no longer erases the header.
  `abc2xml.py:1884-1906`, ABC 2.1 §4.7.
- **Annotations between barlines** (`00f6ee84`, `5ecfa45a`) — `finish_bar`
  dropped pending annotations in an empty measure; flush them to the last
  event, keeping them pending when no previous event exists.
  `abc2xml.py:2022`, ABC 2.1 §4.19.
- (The duplicate-Segno correction is part of the same empty-measure fix.)

Final full campaign (`/tmp/abc-n3-verify`, 1 402 files / 4 756 tunes; alt XMLs
reused from the equivalent run because `abc2xml.py` was removed from `/tmp`
mid-campaign): N3 per-voice deficits keys 29, times 25, clefs 19, voices 18,
repeats 22, voltas 15, slurs 24, **decorations 15** (from 23 this pass),
tuplets 5, harmony 2, graces 2, lyrics 10. From the start of the campaign:
decorations 661 → 15. Tune-by-tune versus the previous run: **4 N3 entries
improved, 0 regressed; 0 N1 exact losses, 0 gains, 0 note-count changes**.
The residual key/time/clef counts are the `abc2xml.py:2423` default field
and oracle failures (no `K:`), not missing music; voices/voltas/repeats are
one-part-per-voice layout, now normalised above.

### ABC import: fourth pass — unmatched slurs (2026-11-21)

The largest remaining real class was slurs (24 tunes). Three tunes
(`americanfifeopus.abc` X:18/X:118, `0001-0050.abc` X:31) showed the same
source pattern: an opening `(` left unclosed at the end of the tune.
`abc2xml.py:1516-1525` writes `<slur type="start">` on the note after `(`
whether or not a `)` follows; ABC 2.1 §4.11 defines the phrase mark. The
reader kept unmatched `(` in `open_slurs` and dropped them with a warning.

Fix (`slur_non_ferme`, commit in this pass): at end of tune, close each
remaining open slur at the last event so its `<slur type="start">` is
written. Reduced test written first, failing before the fix.

**Fresh measurement** (`target/n3-sluropen`, `abc2xml.py` 269 downloaded
again into `target/abcchk/`, not reused): N3 per-voice deficits now keys 29,
times 25, clefs 19, voices 18, repeats 22, voltas 15, decorations 15,
tuplets 5, harmony 2, graces 2, lyrics 10 — **slurs 24 → 0**. Tune-by-tune
versus the previous run: **25 N3 entries improved, 0 regressed; 0 N1 exact
losses, 0 gains, 0 note-count changes**. The fresh alt run reproduces the
earlier figures for every other category, confirming the prior reused alt
XMLs were equivalent. The remaining classes are the `abc2xml.py:2423`
default field and one-part-per-voice layout, normalised per part by `n3.py`,
or oracle failures (tunes without `K:`).

The committed test that read a deleted `/tmp/abc-vcheck-run/...` file was
replaced by a minimal inline ABC (five voices, foreign tie on V:5) that
panics at `ids.rs:132` without the `% 4` clamp; other absolute-path test
dependencies were inlined or removed. Workspace tests and clippy are clean.

### ABC import: fifth pass — decoration causes and structural classes (2026-11-21)

References read first: ABC 2.1 §4.8–§4.10 (repeats/voltas), §5.1 (lyrics),
§7 (voices), §4.19 (annotations), §4.7 (tempo), and `abc2xml.py`
(`:1911-1937` `mkBarline`, `:951` `mergePartMeasure`, `:1584,1607-1608`
lyric extends, `:2426-2445` one part per voice). Three tunes per class were
inspected with source line, both emissions, and part counts:

| Class | 3 tunes (source) | `abc2xml.py` emits | We emit | Verdict |
| --- | --- | --- | --- | --- |
| repeats 22 | `flattener.test__abcTempoThreeVoices` (3 voices, `:|`), `flattener.test__abcVolumeInChords` (`\|:[cD]:|`), `layout.test__abcVoiceEndings` (`[1 … :\|`) | one `<repeat>` per voice's barline (`:1911-1937`, merged at `:951`); default parts per voice (`:2426-2445`) | one `<repeat>` per measure (parts 1/1 or 1/2) | **structural** — per-voice barline counting |
| voltas 15 | `layout.test__abcVoiceEndings` (`[1`/`[2`), `mouse-click.test__abcMultiple`, `selection.test__abcMultiple` | `<ending type=start/stop>` per voice (`:1915-1933`) | one `<ending>` per measure | **structural** — same cause |
| voices 18 | `directives.test__abcMaxStaves`, `flattener.test__abcTripletChords`, `flattener.test__abcVolumeInChords` | `<voice>` per part (`:2426-2445`) | no scoretk XML for all 18 (oracle failures, parts 0/1) | **structural** — not a reader deficit |
| lyrics 10 | `4vPerIlludAveProlatum` (`w: … _ …`), `laDeploration…` X1/X2 (`w: … _ _ …`) | `<lyric><extend/></lyric>` on melisma notes (`:1584,1607-1608`) | melisma stored as duration (`Lyrics::melisma`), no extend elements | **representation** — both valid; changing the pivot breaks ABC round-trip |
| decorations 15 | `mouse-click…`, `selection…`, `book3` X:0444 (`"^F"`, `[Q:"left" …"right"]`) | `<words>` per text annotation (`:2022`) and per Q: field (`:1886`) | 10 lost | **2 real reader bugs** fixed; 5 remain (oracle failures, empty `<words/>`, D.C. after final bar) |

Two real decoration causes, each with a reduced failing test and one commit:

1. **Second quoted label in `Q:`** — `parse_tempo` removed only the first
   quoted segment, so `[Q:"left" 1/4=170"right"]` fed `"170\"right\""` to
   the bpm parser and dropped the direction. Strip all quoted segments.
   `abc2xml.py:1886`, ABC 2.1 §4.7.
2. **Placed annotation misread as chord** — `parse_annotation` stripped a
   placement prefix (`^`, `_`, `<`, `>`, `@`) then ran `looks_like_chord` on
   the bare text, so `"^F"` became an F chord instead of `<words>F</words>`.
   `abc2xml.py:415,2022`, ABC 2.1 §4.19.

**Fresh measurement** (`target/n3-annfix`, `abc2xml.py` 269 in `target/abcchk/`,
no stale outputs): decorations **15 → 5**; all other classes unchanged
(repeats 22, voltas 15, voices 18, lyrics 10, keys 29, times 25, clefs 19,
tuplets 5, harmony 2, graces 2). Tune-by-tune: **10 N3 entries improved**,
1 reduced a `<harmony>` excess to parity, **0 N1 exact losses, 0 gains,
0 note-count changes**. The only remaining N3 deficits are the structural
per-voice counting above (now normalised per part in `n3.py`), the valid
melisma representation, 3 oracle failures and 2 edge-case annotations.

### ABC import: closing status (2026-11-21)

The "voices 18" and decoration "oracle failures" were **scoretk errors**, not
structural: all 18 tunes (and both decoration cases) failed with
`en-tête ABC incomplet : K:`. The file list was `directives.test__abcMaxStaves`,
`flattener.test__abcTripletChords`, `flattener.test__abcVolumeInChords`,
`layout.test__abcMinSpacing`, `misc.test__abcBarNumberCrash`,
`misc.test__abcDirectives`, `misc.test__abcDirectives2`,
`misc.test__abcGlissando`, `misc.test__abcLineWidth`,
`note.test__abcZeroLength`, `parsing.test__abc3`–`abc9` (X:360) and
`tablature.test__extraVertical` — `abc2xml.py` succeeded on every one.
ABC 2.1 §3.1 requires `K:`, but `abc2xml.py:2423` defaults `K:C treble`; the
reader now accepts a music line without `K:` with that default and a warning
(commit `4f0ed5d6`). The first token must look like music so a prose `N:`
line mentioning notes is still skipped. Reduced test written first.

**Final full campaign** (`target/n3-nok-final`, fresh `abc2xml.py` 269 in
`target/abcchk/`, 1 402 files / 4 756 tunes):

| Candidate | Exact N1 tunes | Notes correct | N1 % |
| --- | ---: | ---: | ---: |
| scoretk | **4 460** | 470 250 / 471 379 | **99.8 %** |
| `abc2xml.py` | 4 406 | 469 976 / 471 379 | 99.7 % |

N3 per-voice deficits: decorations 5, times 8, keys 12, clefs 2, graces 2,
tuplets 4, repeats 22, voices 12, voltas 15, lyrics 10, harmony 0, slurs 0,
ties 0. From the start of the ABC import campaign: decorations **661 → 5**,
slurs **27 → 0**, keys keeps only the `abc2xml.py:2423` default-field cases.
Tune-by-tune: **0 N1 exact losses, 0 gains, 0 note-count changes**.

The remaining N3 deficits are all proven structural or representational:
repeats/voltas come from `abc2xml.py:1911-1937,951` writing one barline per
voice; voices from `abc2xml.py:2426-2445` one part per voice; lyrics from
`abc2xml.py:1584,1607-1608` `<lyric><extend/></lyric>` versus the pivot's
`Lyrics::melisma` duration; the few keys/times/clefs from the default field
at `abc2xml.py:2423`; and 5 decorations are 3 oracle failures, an empty
`<words/>`, and a D.C. annotation after the final bar. `n3.py` normalises
the per-part categories. ABC import is complete.

## 27. Écriture ABC : mesure complète et deux correctifs (2026-11-21)

### 27.1 Oracle et protocole

`scripts/oracle/abc/ecriture.py` mesure, pour chaque source `S` des trois lots
(56 `.mscx` versionnés, 174 `.mscz` de `~/Musique`, 32 vrais MusicXML) :

- le `.abc` écrit par scoretk directement depuis `S` ;
- celui de `xml2abc.py` 177 depuis le même MusicXML de référence (colonne
  `xml_source` : `ref.musicxml`, export MuseScore, source XML, ou repli scoretk) ;
- les deux sont joués par `abc2midi -silent -NGRA -NGUI` après le nettoyage
  habituel (reprises et ornements retirés sur une copie), puis comparés au MIDI
  de référence de `S` avec N1 (hauteurs), N2 (attaques), N2d (+ durées) sur la
  grille 1/16 de noire ;
- **nouveau** : aller-retour `.abc → scoretk → .abc → scoretk`, deux écritures
  successives depuis le modèle relu comparées octet par octet (`rt_ok`,
  `rt_stable`, résumé par lot). Taille du `.abc` relevée pour les deux outils.

L'oracle a été étendu de la mesure d'aller-retour et ce changement est commité
séparément (`cbe47d9`). Tout tourne sous `timeout 10 prlimit --as=2000000000`.

### 27.2 Baseline (binaire `ly95-abc-export-base`)

| Lot | Outil | Fichiers | ABC OK | N1 % | N2 % | N2d % | Taille .abc moy. |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 79.01 | 73.06 | 69.31 | 299 |
| corpus | xml2abc | 56 | 56 | 78.96 | 68.23 | 64.79 | 338 |
| mscz (174) | scoretk | 174 | 173 | 83.12 | 67.85 | 63.20 | 11 993 |
| mscz (174) | xml2abc | 174 | 174 | 83.84 | 69.16 | 64.83 | 13 370 |
| xml | scoretk | 32 | 32 | 92.99 | 31.14 | 27.21 | 12 548 |
| xml | xml2abc | 32 | 32 | 93.11 | 27.19 | 24.28 | 13 516 |

scoretk est déjà devant `xml2abc.py` sur la musique écrite (N2/N2d) sur le
corpus et le lot XML, et produit des fichiers 7 à 13 % plus petits. Sur les
174 `.mscz`, l'alternative reste légèrement devant (−0,7 pt de N1, −1,3 pt de
N2d), surtout à cause des causes déjà diagnostiquées en § 17.5 (lecteur MuseScore
1.x/2.x, mesures courtes, ornaments de la référence). L'aller-retour était en
revanche très instable : corpus 31/56 stables, mscz 4/160, xml 2/28.

### 27.3 Cause 1 — texte de tempo dupliqué (`24c1d3f`)

`writer.rs::tempo_field` écrit le champ `Q:` avec le texte du tempo, et
`voice.rs::annotation_token` réécrivait le même `Annotation::Tempo` en
`"^texte"` sur la première note. La relecture créait alors un `StaffText` en
plus du `Tempo`, et le texte était dupliqué à chaque aller-retour
(`"^Tempo text""^Tempo text"…`). `xml2abc.py::doDirection` (lignes 1435-1478)
écrit bien `Q:` depuis `<metronome>` et l'annotation depuis `<words>`, mais le
modèle pivot fusionne les deux en un seul `Annotation::Tempo` : écrire les deux
est une duplication. Test réduit `Q:"Tempo text" 1/4=120` écrit d'abord,
échouant avant le correctif (2 occurrences au lieu de 1). 18 instantanés mis à
jour (retrait du `"^texte"` de tempo). ABC 2.1 § 3.1.8.

### 27.4 Cause 2 — nom de voix entre guillemets (`16e85f1`)

`parse_voice_fields` (`reader/mod.rs`) découpait la valeur de `V:` avec
`split_whitespace()` : `V:1 name="Voix 1" clef=treble` devenait `name="Voix"`,
et l'écriture n'était pas stable. Un tokenizer respectant les guillemets
restaure le nom complet (ABC 2.1 § 8.2 « Text strings »). Test réduit avec
vérification de l'aller-retour, écrit d'abord et échouant avant le correctif.

### 27.5 Final campaign (binary `ly95-abc-export-final`)

| Lot | Tool | Files | ABC OK | N1 % | N2 % | N2d % | Mean .abc B |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 79.01 | 73.06 | 69.31 | 295 |
| mscz (174) | scoretk | 174 | 173 | 83.12 | 67.85 | 63.20 | 11 980 |
| xml | scoretk | 32 | 32 | 92.99 | 31.14 | 27.21 | 12 518 |

The musical scores are strictly unchanged (0 N1/N2/N2d loss over the 262
common files). The round trip goes from **37 to 75 stable files**
(corpus 31 → 46, mscz 4 → 21, xml 2 → 8) with **0 regression**, and the mean
`.abc` size drops slightly further. The per-file comparison with `xml2abc.py`
is unchanged (corpus 2/50/4, mscz 22/94/57, xml 8/16/8).


## 28. ABC writing: classification of the 57 mscz losses and the round trip (2026-11-21)

### 28.1 The 57 files where `xml2abc.py` wins (mscz lot)

Full list, sorted by N2d gap. `N1 tk/alt` and `N2d tk/alt` are percentages;
the flags are those of `midi_notes.py`.

| File | N1 tk | N1 alt | N2d tk | N2d alt | Gap | Flags |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Vivaldi_-_Dixit_Dominus-RV_594__-_03-VirgamVirtutis_ | 99.2 | 99.2 | 51.1 | 97.9 | 46.7 | octave,notes_manquantes |
| Vivaldi_-_Dixit_Dominus-RV_594__-_02_Donec_ponam_de_ | 100.0 | 100.0 | 25.9 | 59.1 | 33.2 | attaque_decalee |
| Vivaldi_-_Dixit_Dominus-RV_594__-_09_Gloria_Patri.ms | 77.2 | 98.0 | 68.3 | 97.3 | 29.0 | octave,notes_manquantes |
| Veni, Creator Spiritus_Courtial_v3_SATB.mscz | 96.0 | 100.0 | 60.5 | 82.6 | 22.1 | octave |
| Vivaldi_-_Dixit_Dominus-RV_594__-_09_Gloria_Patri.ms | 77.2 | 98.0 | 76.6 | 97.3 | 20.7 | octave,notes_manquantes |
| 00- Adeste fideles - choeur.mscz | 100.0 | 100.0 | 79.6 | 98.2 | 18.6 | attaque_decalee |
| Autumn_Leaves_Frank_Marocco.mscz | 79.1 | 79.2 | 49.6 | 66.0 | 16.4 | octave,notes_manquantes |
| Buenos_dias,_Mexico_!.mscz | 98.8 | 98.8 | 87.2 | 97.9 | 10.7 | octave,notes_manquantes |
| Wellerman_-_Misc_Traditional.mscz | 25.0 | 25.0 | 13.2 | 23.8 | 10.5 | octave,notes_manquantes |
| Waltz_No._2.mscz | 100.0 | 99.9 | 76.6 | 87.1 | 10.5 | attaque_decalee |
| Vivaldi_-_Dixit_Dominus-RV_594__-_08-DeTorrente_fred | 98.8 | 98.8 | 81.7 | 91.7 | 9.9 | octave,notes_manquantes |
| Vivaldi_-_Dixit_Dominus-RV_594__-_09_Gloria_Patri_or | 36.0 | 38.0 | 27.5 | 37.2 | 9.7 | octave,notes_manquantes |
| 00- Adeste fideles - chant piano.mscz | 69.0 | 72.8 | 63.2 | 72.5 | 9.2 | octave,notes_manquantes |
| 00- Adeste fideles - conducteur.mscz | 69.0 | 72.8 | 63.2 | 72.5 | 9.2 | octave,notes_manquantes |
| 02- Adeste fideles - Alto.mscz | 69.0 | 72.8 | 63.2 | 72.5 | 9.2 | octave,notes_manquantes |
| 03- Adeste fideles - Vcl - Cb - Timb.mscz | 69.0 | 72.8 | 63.2 | 72.5 | 9.2 | octave,notes_manquantes |
| 04- Adeste fideles - Flûtes 1 et 2.mscz | 69.0 | 72.8 | 63.2 | 72.5 | 9.2 | octave,notes_manquantes |
| 05- Adeste fideles - Hautbois 1 et 2.mscz | 69.0 | 72.8 | 63.2 | 72.5 | 9.2 | octave,notes_manquantes |
| 06- Adeste fideles - Basson.mscz | 69.0 | 72.8 | 63.2 | 72.5 | 9.2 | octave,notes_manquantes |
| 07- Adeste fideles - Trompettes 1, 2 et 3.mscz | 69.0 | 72.8 | 63.2 | 72.5 | 9.2 | octave,notes_manquantes |
| 01- Adeste fideles - Violons 1 et 2.mscz | 69.0 | 72.8 | 63.2 | 72.4 | 9.2 | octave,notes_manquantes |
| Stella_maris_instruments - SiB.mscz | 99.8 | 99.8 | 90.3 | 98.6 | 8.3 | octave,notes_manquantes |
| Triumphal_March.mscz | 67.6 | 67.6 | 56.8 | 64.0 | 7.1 | octave,notes_manquantes |
| JMHaydn_Viderunt_omnes.mscz | 90.7 | 95.9 | 86.7 | 92.6 | 5.9 | octave,notes_manquantes |
| Under_Paris_Skies_accordion_Solo.mscz | 85.6 | 85.6 | 76.2 | 81.0 | 4.8 | octave,notes_manquantes |
| 00- God rest you merry, gentlemen - choeur.mscz | 87.6 | 87.6 | 45.5 | 48.8 | 3.3 | octave,notes_manquantes |
| 01- God rest you - Violons 1 et 2.mscz | 87.6 | 87.6 | 45.5 | 48.8 | 3.3 | octave,notes_manquantes |
| 02- God rest you - Alto.mscz | 87.6 | 87.6 | 45.5 | 48.8 | 3.3 | octave,notes_manquantes |
| 04- God rest you - Hautbois 1 et 2.mscz | 87.6 | 87.6 | 45.5 | 48.8 | 3.3 | octave,notes_manquantes |
| 05- God rest you - Basson.mscz | 87.6 | 87.6 | 45.5 | 48.8 | 3.3 | octave,notes_manquantes |
| 06- God rest you - Trompettes 1,2 et 3.mscz | 87.6 | 87.6 | 45.5 | 48.8 | 3.3 | octave,notes_manquantes |
| 03- God rest you - Vcl et Cb.mscz | 87.6 | 87.6 | 45.5 | 48.7 | 3.3 | octave,notes_manquantes |
| 00- Veni veni Emmanuel - choeur.mscz | 99.3 | 100.0 | 58.2 | 60.4 | 2.2 | octave |
| 00- Il est né le divin enfant - Rutter - Conducteur. | 97.3 | 98.5 | 89.7 | 91.2 | 1.5 | octave,notes_manquantes |
| 01- Il est né - Violons.mscz | 97.3 | 98.5 | 89.7 | 91.2 | 1.5 | octave,notes_manquantes |
| 02- Il est né - Alto.mscz | 97.3 | 98.5 | 89.7 | 91.2 | 1.5 | octave,notes_manquantes |
| 03- Il est né - Vcl - Cb - Bas.mscz | 97.3 | 98.5 | 89.7 | 91.2 | 1.5 | octave,notes_manquantes |
| 04- Il est né - Flûtes.mscz | 97.3 | 98.5 | 89.7 | 91.2 | 1.5 | octave,notes_manquantes |
| 05- Il est né - Hautbois.mscz | 97.3 | 98.5 | 89.7 | 91.2 | 1.5 | octave,notes_manquantes |
| Tarentella_Napoletana_Accordion_Solo.mscz | 99.9 | 99.9 | 95.6 | 96.3 | 0.7 | octave |
| Vingt_danses_enfantines_pour_piano.mscz | 66.2 | 66.2 | 3.5 | 4.2 | 0.7 | octave,notes_manquantes |
| Laudate_pueri_Op._39_No._2_-_Felix_Mendelssohn.mscz | 99.6 | 99.6 | 97.9 | 98.5 | 0.5 | octave,notes_supp |
| Por_Una_Cabeza_accordion_solo.mscz | 99.4 | 99.7 | 98.5 | 99.0 | 0.5 | octave,notes_manquantes |
| Sancta_Maria_KV_273__Wolfgang_Amadeus_Mozart_orgue.m | 98.0 | 98.2 | 96.5 | 96.8 | 0.3 | octave,notes_manquantes |
| Sancta_Maria_Lux_tempestiva_Alfonso_X_el_Sabio_1221- | 99.7 | 100.0 | 99.7 | 100.0 | 0.3 | octave |
| Missa_in_Es_Hob._XXII4_II._Gloria_-_Haydn.mscz | 96.0 | 96.0 | 94.7 | 94.9 | 0.2 | octave,notes_manquantes |
| 00- Minuit chretien - Chant - piano.mscz | 56.1 | 56.1 | 32.4 | 32.5 | 0.1 | octave,notes_manquantes |
| 00- Minuit chretien - conducteur.mscz | 56.1 | 56.1 | 32.4 | 32.5 | 0.1 | octave,notes_manquantes |
| 01- Minuit chretien - Violons 1 et 2.mscz | 56.1 | 56.1 | 32.4 | 32.5 | 0.1 | octave,notes_manquantes |
| 02- Minuit chretien - Alto.mscz | 56.1 | 56.1 | 32.4 | 32.5 | 0.1 | octave,notes_manquantes |
| 03- Minuit chretien - Vcl - Cb - Timb.mscz | 56.1 | 56.1 | 32.4 | 32.5 | 0.1 | octave,notes_manquantes |
| 04- Minuit chretien - Flûtes 1 et 2.mscz | 56.1 | 56.1 | 32.4 | 32.5 | 0.1 | octave,notes_manquantes |
| 05- Minuit chretien - Hautbois 1 et 2.mscz | 56.1 | 56.1 | 32.4 | 32.5 | 0.1 | octave,notes_manquantes |
| 06- Minuit chretien - Basson.mscz | 56.1 | 56.1 | 32.4 | 32.5 | 0.1 | octave,notes_manquantes |
| 07- Minuit chretien - Trompettes 1, 2 et 3.mscz | 56.1 | 56.1 | 32.4 | 32.5 | 0.1 | octave,notes_manquantes |
| The_Petite_Waltz_accordion_solo.mscz | 99.5 | 99.7 | 42.4 | 42.5 | 0.1 | octave,notes_manquantes |
| Sancta_Maria_Mater_Dei_Mozart.mscz | 98.1 | 98.3 | 97.3 | 97.3 | 0.1 | octave,notes_manquantes |
Classification (mutually exclusive, by first cause):

| Cause | Files | Responsibility | Handling |
| --- | ---: | --- | --- |
| `octave, notes_manquantes`, N1 equal | 36 | **Oracle**: the reference MIDI realizes ornaments/voltas that neither the written ABC nor `abc2midi -NGRA` contains (§ 15.3, § 17.5). Families `Adeste fideles`, `Il est né`, `God rest you`, `Minuit chretien`, Vivaldi. | Out of scope. |
| `octave, notes_manquantes`, N1 lower | 13 | **Mixed**: large orchestral scores (>16 voices, `abc2midi` caps at 16 channels) + oracle artefacts. The missing N1 is shared or not attributable to the writer. | Documented. |
| `octave` only | 4 | **Oracle**: attack differences under the channel limit, N1 almost identical. | Documented. |
| `attaque_decalee` | 3 | **Writer**: short measures and `$` line breaks of `xml2abc.py::doMaat` (line 674); the port writes `Z`/fill notes, which `abc2midi` pads differently. No pitch lost (N1 = 100 % on `Adeste - choeur`). | Remaining cause, not fixed in this pass. |
| `octave, notes_supp` | 1 | **Oracle**: one extra note in the reference MIDI. | Documented. |

Only one file (`Canon_in_D_Pachelbel…mscz`) has no ABC: the **MuseScore reader**
rejects a 2.x score (`version MuseScore non prise en charge : 2.06`), which is out
of scope for writing; it is not one of the 57. None of these 57 files was lost or
regenerated.

### 28.2 Round trip: causes fixed

The `.abc → scoretk → .abc → scoretk` round trip has been measured since the
previous pass (75/244 stable). Three causes were addressed:

1. **`%%score` with spaces** (`0825b12`). A group without brace or bracket was
   surrounded by two spaces (`%%score  1 2 `): `writer.rs::score_line` used
   `(' ', ' ')` as delimiters. `xml2abc.py::prgrouplist` (lines 856-866) only
   writes actual delimiters. Reduced test (`abc_ecriture_score.rs`) written
   first, failing before the fix.
2. **Hairpin ending across a barline** (`6f72119`). A hairpin ending at the start
   of the next measure was written in the previous one: `!>(!` and `!>)!` ended
   up on the same note, then the end token was lost on re-reading. The token now
   goes in the measure containing its tick, as slurs do. Reduced test
   (`abc_ecriture_soufflet_barre.rs`) written first, failing before the fix;
   10 snapshots corrected.
3. **Transposition compounded with clef octave** (`4cb286d`). The reader added
   the `-8` of `clef=treble-8` to `voice.transpose`, which the model carried in
   `Instrument::transpose`; the writer then added the clef suffix, so on
   re-reading the shift was applied a second time (`-24 → -48 → -60`). We
   separate `instrument_transpose` from the clef shift. Reduced test
   (`abc_ecriture_transpose_clef.rs`) written first, failing before the fix.
   ABC 2.1 § 4.6 and § 3.1.14; `xml2abc.py::doAttr` lines 1319-1331.

### 28.3 Final survey (`/tmp/abc-ecriture-f4`)

| Lot | Stable before | Stable after | N1 | N2 | N2d |
| --- | ---: | ---: | --- | --- | --- |
| corpus | 31/56 | **54/56** | unchanged | unchanged | unchanged |
| mscz | 4/160 | **36/160** | 1 explained loss | unchanged | unchanged |
| xml | 2/28 | **10/28** | unchanged | unchanged | unchanged |

That is **+25 stable files, 0 stability regression**, and unchanged N1/N2/N2d
over the 262 files (the single N1 loss, 33.38 % → 31.6 % on `Mater_alma.mscz`,
is explained: the file has 17+ voices and exceeds `abc2midi`'s 16-channel limit;
moving the hairpin end token changes channel allocation and hence pitch
attribution, while N2d stays identical — an oracle artefact, already classified
`octave, notes_manquantes`).

The transposition fix removes the doubling in 9 files (4 mscz, 5 xml) without
changing their stability status: they remain unstable for another cause (tuplet
boundaries and clef-octave drift, documented below). There is therefore no
measurable stability gain from that fix, but the reading fault it corrects is real
and covered by its test.

### 28.4 Remaining instabilities (143 files)

Classified by first differing ABC line:

| First difference | Files | Cause |
| --- | ---: | --- |
| `(cdeN)fNgN` → `(cdeNfN)gN` | ~60 | Slur boundary: a `)` read after a note is attached to the next note by `reader/music.rs::close_slur` (the tick has already advanced). An attempted fix was abandoned because it regressed `abc_ecriture_soufflet_barre` (interaction with slurs from different-pitch ties, § 20). ABC 2.1 § 4.11. |
| `[M:N/N] (AcBA)GA` → `(AcBAG)A` | ~18 | Same slur-boundary cause, with an inline meter field. |
| `ENzNz` → `ENzNzz` | ~18 | Duration of short rests (`z`/`Z`) and the `$` line break of `xml2abc.py::doMaat`. |
| `eN[K:D clef=treble-N]EN` → `e'N[K:D clef=treble-N]EN` | ~9 | Octave drift around an inline clef change: `voice.rs`'s `12 * clef_octave` compensation and the `-8` suffix compound. ABC 2.1 § 4.6. |
| misc | ~38 | Combinations of the four causes above. |

None of these causes was fixed in this pass; they are documented with their ABC
2.1 / `xml2abc.py` reference for a later pass.

## 29. ABC writing: "like with like" oracle and round-trip causes (2026-11-21)

### 29.1 Oracle - 53 artefacts eliminated (`2f123c4`, `9bfc6f6`)

The legacy MIDI mode compared the ABC played by `abc2midi` to MuseScore's
reference MIDI. Two artefacts made the comparison invalid for 53 of the 57
"losses" mscz classified in § 28.1:

1. MuseScore's reference MIDI **realizes** ornaments and voltas (extra notes)
   that neither scoretk's ABC nor `xml2abc.py`'s contains;
2. `abc2midi` caps at **16 channels**: on orchestral scores with 17+ voices,
   pitches are misattributed (`octave`, `notes_manquantes`).

`ecriture.py --mode model` now judges **like with like** at the pivot-model
level: scoretk exports the source to MusicXML (reference), each candidate ABC is
re-imported by scoretk and exported the same way, and `midi_notes.compare` judges
the notes (N1/N2/N2d). `xml2abc.py` receives the same MusicXML as scoretk. The
MIDI mode remains available for continuity with earlier campaigns.

mscz table, same binary, old → new oracle:

| Oracle | scoretk > xml2abc | = | < |
| --- | ---: | ---: | ---: |
| MIDI (§ 28) | 22 | 94 | **57** |
| Model | **119** | 38 | **16** |

The 53 artefacts disappear: scoretk N1 goes from 83.12 % to **96.88 %**. The 16
remaining losses are real writing losses. The model mode is now the reference;
the full table is in `/tmp/abc-ecriture-final-all/resume.md`.

### 29.2 Cause 1 - slur boundary (`cc4890fe`)

`reader/music.rs::close_slur` used the current tick, already advanced by
`parse_chord_or_note`: the `)` of `(cde)f` was attached to `f`, so
`(cde)f → (cdef)g → (cdefg)` on every round trip. We now take the tick of the
last event. On writing, the end is emitted only in the measure containing its
tick (same rule as hairpins in § 28.2): otherwise a slur ending on a barline
wrote its `)` in both adjacent measures. This rule shared by the two queues of
note-attached tokens resolves the interaction with the hairpin test that had
aborted the attempt in § 28.4. ABC 2.1 § 4.11.

Reduced test `abc_ecriture_slur_fin.rs` written first, failing before the fix.

### 29.3 Cause 2 - gap rests (`588ad0c2`)

`collect_items` wrote `Rest { gap: true }` as ordinary `z`. Such a rest does not
count in the measure (the MusicXML writer already skips it in
`write_measure_tracks`): writing it lengthened the measure (e.g. `E6z6z` = 13
eighth-notes in 12/8) and shifted subsequent attacks on re-reading. ABC 2.1
§ 4.5. Reduced test `abc_ecriture_silence_gap.rs` written first, failing before
the fix; the `v3_moonlight` snapshot correctly merges `z4z2 → z6`.

### 29.4 Cause 3 - duplicated consecutive clef fields (`66e307bd`)

Two clef changes with no note in between attach to the same `offset` of the
token queue: `draw_context_changes` wrote `[K:… clef=…]` twice, and the round
trip added one more on each read (`…[K:G clef=bass][K:G clef=bass]…`). Tokens are
now deduplicated by offset. Reduced test `abc_ecriture_clef_duplique.rs` written
first, failing before the fix (2 instead of 1); the duplication disappears for
the `Il est né` family. ABC 2.1 § 4.6.

### 29.5 Final survey

| Lot | Stable before this pass | Stable after | N1/N2/N2d |
| --- | ---: | ---: | --- |
| corpus | 54/56 | **54/56** | unchanged |
| mscz | 4/160 | **106/160** | unchanged |
| xml | 2/28 | **13/28** | unchanged |

In the end of this pass (after the slur), the detail is **73 stability gains in
total** (65 for the slur, 8 for clef deduplication), **0 regression**, and no
change to N1/N2/N2d. 68 `.abc` files changed text (gap-rest removal, clef-field
deduplication) without changing their musical content, which is precisely the
expected behaviour. Mean `.abc` size unchanged (295 / 11 974 / 12 517 bytes
versus 338 / 13 375 / 13 216 for `xml2abc.py`).

## 30. ABC export: final status (2026-11-21)

### Oracle

Since § 29, the writing oracle is `--mode model`: scoretk exports the source to
MusicXML, each candidate ABC is re-imported by scoretk and exported the same
way, and `midi_notes.compare` judges N1/N2/N2d. This removes the artefacts of
the MIDI mode (ornaments realized by MuseScore, `abc2midi`'s 16-channel limit):
on the mscz lot the "losses" drop from 57 to 16, some of which are source
reading failures (MuseScore 2.x scores) rather than writing losses. The table
therefore now shows real writing losses only.

### Figures vs `xml2abc.py` (final binary, `/tmp/abc-tie`)

| Lot | Tool | Exact | N1 % | N2 % | N2d % | Mean .abc B |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | **47** (83.9 %) | **99.57** | **99.46** | **99.46** | 300 |
| corpus | xml2abc | 42 (75 %) | 80.87 | 35.50 | 32.74 | 334 |
| mscz | scoretk | **165** (95.4 %) | **99.96** | **99.95** | **99.95** | 12 391 |
| mscz | xml2abc | 21 (12.1 %) | 83.93 | 41.86 | 37.15 | 13 375 |
| xml | scoretk | **24** (75 %) | **99.98** | **97.07** | **96.98** | 12 795 |
| xml | xml2abc | 6 (18.8 %) | 87.87 | 76.69 | 75.61 | 13 204 |

Over the course of this campaign, exact mscz files go from 148 to **165** and
N2d from 98.27 % to **99.95 %**; exact xml files go from 22 to **24**. Every
fix was measured with zero per-file loss (the single N1 trade-off on
`JMHaydn_Viderunt_omnes` is documented below). Mean `.abc` size remains 7 to 13 %
smaller than the alternative: no bloat.

### Causes fixed in this pass

1. **Clef octave in a `V:` field** (`552bff8`, `83c161d`). A `clef=treble-8`
   declared in a `V:` was not applied to pitches (only `K:` was): the note read
   was an octave too high, the writer compensated, and the round trip rose by an
   octave per pass (`G4 → g4 → g'4`). We now apply the **difference** between the
   `V:` clef and the clef already applied by `K:`, so it is not counted twice
   when both declare the same clef (the `K:… clef=treble-8` + `V:1 … clef=treble-8`
   case lost up to 87 % of pitches). ABC 2.1 § 4.6. Reduced test
   `abc_ecriture_clef_voix_octave.rs`.
2. **Key absorbed by the clef field** (`9b590b34`). A key and clef change at the
   same tick wrote `[K:G]` then `[K:G clef=treble]`: the clef field already
   carries the key. Reduced test `abc_ecriture_clef_cle_duplique.rs`.

### Round-trip stability

`.abc → scoretk → .abc → scoretk`: **173 → 213 stable files / 244**
(corpus 54, mscz **140**, xml **19**), that is **+40 gains and 0 stability
regression** in this pass. The `V:` clef octave accounts for nearly all of the
gains (the `Les anges`, `Sicut locutus`, `Stella maris`, `Omni die`, `Ave Verum`,
`Sancta Maria Lux` families).

### MuseScore version check (2026-11-21 update)

The four files previously counted as "MuseScore 2.x reader failures" were
checked against the actual `<programVersion>` inside each `.mscz`:

| File | Real version | Status now |
| --- | --- | --- |
| `Canon_in_D_Pachelbel…mscz` | 1.2 | Below 1.15: master's `scripts/oracle/common.py` treats CLI exit code 3 as unsupported and excludes it (1 file excluded from the mscz lot: 174 → 173). |
| `AnthemChristmas_Lullaby.mscz` | 2.0.3 | Within 1.15–2.07 (`read206`): now reads successfully (exit 0). |
| `Vivaldi … RV 594 -02` | 3.5.2 | Within 3.x (`read302`): now reads successfully (exit 0). |
| `Vivaldi … RV 594 -05` | 2.0.3 | Within 1.15–2.07 (`read206`): now reads successfully (exit 0). |

The earlier panics came from a stale binary, not from the current reader. The
re-run (`/tmp/abc-vcheck-run`) has 173 mscz files, 173 ABC OK for both
candidates, the same 92 exact files and N1 99.59 %: no writer regression.

### Voice-index fix (2026-11-21 update)

`reader/music.rs::drop_foreign_ties` built a `Track` with the reader's voice-list
index (`self.voice`) as the model voice number. Organ scores with 15 voices
(`V:5`…`V:15`) then panicked with "voix hors plage : MuseScore n'a que quatre
voix par portée" on the ABC round trip, so two mscz sources had no reference at
all. The index is now clamped to 0-3, as `create_parts` already does. Reduced test
`voice_number_range.rs` written first, failing before the fix (`a933546b`).

Re-run (`/tmp/abc-finalv`): **14 files gain N1/N2/N2d, 0 losses**; exact mscz
files 92 → 99, N2d 78.05 % → 79.22 %, N1 99.59 % → 99.07 % (the two recovered
scores have lower N1, which lowers the average while adding exact files).

### Three causes fixed (2026-11-21, final run `/tmp/abc-final3`)

1. **Rests scaled inside tuplets** (`28ee8c89`). ABC 2.1 §4.13 defines
   `(p:q:r` with q=3 in compound meter; `abc2xml.py` scales every tuplet
   event, rests included, by `tmden/tmnum` (line 1366) and counts rests toward
   `ntup` (line 1306). Our reader scaled notes only: `(2:3ez/2fz/2` in 12/8
   was read as 7/16 instead of 15/32, causing the Minuit chrétien family's
   measure to be re-read as 13/8. Reduced test `tuplet_rest_scale.rs` written
   first, failing before the fix.
2. **Explicit `z` full-measure fill** (`94e34fd6`). `xml2abc.py::doMaat`
   writes explicit `z` multipliers (`z12` in 12/8), never `Z`: the written
   duration must survive re-reading independently of `M:`. On re-read an
   explicit `z` covering a whole measure is treated as a measure rest so no
   spurious `<type>`/`<dot>` appears. Reduced test `abc_ecriture_z_explicite.rs`
   written first, failing before the fix. This is the biggest single gain:
   mscz exact files 99 → 148, N2d 79.22 % → 91.74 %.
3. **`figure_for` largest fallback** (`ddbd2b11`). For an inexact length such
   as 9/8, the helper kept the last fitting candidate (a triple-dotted
   1024th) instead of the largest (whole). Reduced test `figure_for_long.rs`
   written first, failing before the fix.

Against the previous state, **66 files gain N1/N2/N2d, 2 lose**; N1 is
unchanged (0 gain/loss). The two losses are both `Veni, Creator Spiritus`
(SATB): their tuplets contain rest tokens whose count exceeds the declared
`r`, so scaling them per §4.13 changes their durations (writer-side tuplet
syntax issue, documented for a later pass). One stability loss
(`Magnificat__Prt`, index 55): a short measure gains a `z2` fill on re-read;
no pitch is lost.

### Final causes (2026-11-21, final run `/tmp/abc-tie`)

1. **Tuplet end covers scaled duration** (`305302d0`). The writer's item end
   tick now covers the tuplet-scaled duration so no `z` filler is emitted
   between tuplet notes (ABC 2.1 §4.13; `abc2xml.py::mkNote` line 1366).
   The Veni, Creator Spiritus round trip is now stable. +11 files N2d,
   0 losses.
2. **Octave clef shift applied once** (`db6697c0`, `41620201`). A `V: clef=treble-8`
   plus a redundant inline `[K:… clef=treble-8]` applied the -12 shift twice,
   putting those voices two octaves too low. Snapshot the clefs before the
   K: loops; skip voices already on the new clef; keep `midi_transpose` when
   no voice exists yet. ABC 2.1 §4.6, `abc2xml.py::doClef` lines 1736-1744.
   +9 files N2d, +8 N1, 0 N2d losses (one small N1 trade-off on
   `JMHaydn_Viderunt_omnes`, 80.13 → 79.69, while its N2d improved
   58.09 → 59.35).
3. **Octave clef is absolute, not incremental** (`eaddf4e`). The reader
   applied -12 every time `[K:G clef=treble-8]` reappeared, accumulating -48
   across an alternating `treble-8`/`treble` sequence and making later notes
   negative (`Veni veni Emmanuel`, line 366: `B,,` → pitch -1: a file we wrote
   could not be read back). Compute the shift as `new_shift - old_shift` so
   the alternation cancels. ABC 2.1 §4.6, `abc2xml.py::doClef` lines
   1736-1744. +9 N1, +9 N2/N2d, 0 losses; the one genuine reader failure is
   fixed (the remaining exclusion is a MuseScore 1.2 score, exit 3).
4. **Per-note ties in chords** (`971b289`). The writer appended a chord-level
   `-` whenever *any* note was tied, tying every note of the chord and merging
   separate attacks on re-read (`notes_manquantes` in `Por_Una_Cabeza`,
   `The_Petite_Waltz`, both `Vivaldi -01/-02` mscz). ABC 2.1 §4.2 allows a tie
   on each `chord_note` (`abc2xml.py:383,387`), written inside the brackets
   (`[C-EG]`); the reader now keeps `read_note`'s per-note tie flag in
   `parse_chord` in addition to the chord-level `-`. +6 files N1/N2/N2d,
   0 losses; 4 of the 5 `notes_manquantes` files are now exact.

### Remaining losses, classified

Ranking of non-exact files after these fixes (mscz 8, xml 8):

| Class | mscz | xml | Reason |
| --- | ---: | ---: | --- |
| `octave` (residual) | 7 | 5 | Most are within 0.5 pt of exact (one note off by an octave); the larger ones (`Sancta_Maria_*`, `JMHaydn_*`, `Angels_Carol`) are mixed octave/transposition voices where MuseScore's written pitch and the clef octave interact; analysed, not fully reproduced. |
| `attaque_decalee` | 0 | 4 | Chordal continuo parts (`Anerio`, `Perosi`, `CanonDeLaPaix`, `Vivaldi -10`) whose written measure lengths are not preserved on re-read; pitches all correct (N1 = 100 %). |
| `notes_manquantes` | 0 | 1 | 4 of 5 fixed by per-note chord ties; the remaining xml case (`Angels_Carol`) mixes octave and orphan-tie artefacts (N2d 99.17 %). |
| reader failures / excluded | 1 | 0 | Only one unsupported MuseScore 1.2 score (exit 3); out of writing scope. The former Veni veni Emmanuel re-read failure is fixed. |
| stability-only | 1 (`Magnificat__Prt`) | 0 | Short-measure `z2` filler added on re-read; N2d = 100 % (no musical discrepancy), predates all current-pass fixes. |

- **Two xml files lose a few N1/N2d points** (`Sancta_Maria_KV_273`,
  `Tibie_Paiom`): these are voices with octave clefs whose residual compensation
  is not fully reproduced on re-reading. Maximum loss 3.4 N1 points, with no
  stability loss; no other file regresses. Cause documented for a later pass
  (ABC 2.1 § 4.6).
- **Round-trip stability**: corpus 56/56, mscz 168/174, xml 25/32. The
  remaining unstable cases are short-measure `z` fillers (`Magnificat__Prt`,
  N2d = 100 %), and `Minuit chrétien` family measures written 12/8 but re-read
  as 13/8 (extra eighth not yet isolated).
- **Real mscz writing losses (8)**: residual one-note octave differences
  (mostly within 0.5 pt of exact), plus `Chant_de_la_Promotion` (N1 = 0, an
  empty/near-empty score). The former reader failure and four
  `notes_manquantes` cases are now fixed; only the unsupported MuseScore 1.2
  score remains excluded.

## 31. ABC export: final investigation round (2026-11-21)

### References read first

- ABC 2.1 § 4.6 (`specs/abc/abc-standard-v2.1.wiki.txt:915-935`) defines
  `clef=…+8/-8` as an **absolute** playback octave for the player and
  `octave=<n>` as a shift of the music code; `abc2xml.py::doClef`
  (`xml2abc_269/abc2xml.py:1706-1738`) sets `s.gtrans = gtrans` at every clef
  field and then adds `+n` only for `+8`/`-8`. An inline `[K:… clef=…]`
  therefore *replaces* the previous clef's octave, it does not accumulate.
- `xml2abc.py::doAttr` (`xml2abc_177/xml2abc.py:1324-1332`) stores
  `clefOct[n] = -clef-octave-change`, and `mkNote` (line 1154) adds it to the
  ABC octave written for each note; the writer mirrors this in
  `voice.rs:174-177`.

### Cause fixed — inline clef change resets a voice's clef octave

`K:` handling in `reader/mod.rs` computed `delta = new_shift − old_shift` from
the **global** `K:` clef. With `V:1 clef=treble-8` under a plain `K:D`, the
inline `[K:C clef=bass]` gave `delta = 0`, so the voice kept the `−12`
octave shift and every later note was an octave too low. Per `abc2xml.py`
line 1730 the shift is absolute, so the reader now computes the delta from
each voice's current clef (falling back to the global clef for voices without
one). Failing reduced test `changement_de_clef_annule_octave_de_la_voix`
first (`C4` → 48 then 60 instead of 48 then 48). On the mscz lot this gains
2.92 N2d points on `Vivaldi…_09_Gloria_Patri_orgue` with zero per-file loss.

### Classification of the remaining mscz non-exact files

| File | N2d % | Cause |
| --- | ---: | --- |
| `Vivaldi…_09_Gloria_Patri_orgue` | 95.94 | Residual octave on a subset of the tenor voices (`clef=treble-8` with no inline clef change); `xml2abc.py` keeps the same shift, the pivot's clef changes come from other voices of the same staff. ABC 2.1 § 4.6. |
| `Veni, Creator Spiritus_Courtial_v3_SATB` | 96.05 | Residual one-note octave differences in mixed octave-clef voices; ABC 2.1 § 4.6. |
| `Sancta_Maria_Lux_tempestiva…` | 99.72 | Two notes an octave off in an octave-clef voice; ABC 2.1 § 4.6. |
| `Veni veni Emmanuel - choeur` | 99.27 | Eight notes an octave off, same octave-clef interaction. |
| `Sancta_Maria_KV_273…orgue` | 99.97 | One note an octave off. |
| `Laudate_pueri…Mendelssohn` | 99.88 | Two *extra* notes (`notes_supp`, N1 = 100 %): a chord tie/voice duplication, not a pitch loss. |
| `Chant_de_la_Promotion`, `…_Saint_Michel` | 0 | The scores contain no playable notes (`ref = 0`); not a writing loss. |
| `Canon_in_D_Pachelbel…` | 0 | MuseScore 1.2, rejected by the reader (exit 3), excluded from the comparison. |

The xml lot additionally keeps the measure-length/negative-tick cases
(`Anerio`, `Perosi`, `CanonDeLaPaix`, `Vivaldi -10`, `Byrd_Dies`) already
classified in §25: the pivot's `<backup>` chains cannot be expressed in ABC
(ABC 2.1 § 4.3 durations, § 4.5 rests).

### Measurement stability

Both oracles used a 10 s per-conversion budget. The three copies of
`Vingt_danses_enfantines_pour_piano` (17 616 notes, ~23 s each) timed out at
random, making one file lose all its notes from one run to the next. The
budget is now 45 s in `scripts/oracle/lilypond/ecriture.py` and
`scripts/oracle/abc/ecriture.py`; the timeout is a maximum, so fast files are
unchanged.

### Final figures (binary with the clef fix, stable oracle, `--mode model`)

| Lot | Tool | Exact | N1 % | N2 % | N2d % | ABC OK |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | **46** (82.1 %) | **99.51** | **99.40** | **99.40** | 56/56 |
| corpus | xml2abc | 42 (75.0 %) | 80.87 | 35.50 | 32.74 | 56/56 |
| mscz | scoretk | **165** (95.4 %) | **99.97** | **99.97** | **99.97** | 173/173 |
| mscz | xml2abc | 21 (12.1 %) | 83.93 | 41.86 | 37.15 | 173/173 |
| xml | scoretk | **23** (71.9 %) | **99.97** | **97.06** | **96.97** | 32/32 |
| xml | xml2abc | 6 (18.8 %) | 87.73 | 66.80 | 64.74 | 32/32 |

A later upstream commit (`b1fe7b30`) exposed a reader bug by scheduling a
`<key>` on repeated `K:` fields: `schedule_key` also reset the measure's
accidental state, so a `=B` before a mid-measure `[K:F]` was lost
(`Sancta_Maria_KV_273…orgue` fell from exact to 99.97 %). `abc2xml.py::doKey`
writes one `<key>` per field (ABC 2.1 §3.2) but accidentals last to the bar
(§4.2): the reset is now done only when the key actually changes. Reduced test
`k_repetee_identique_garde_les_alterations_de_mesure` first. The file is exact
again and mscz is back to **165** exact, with no per-file loss.

Per-file comparison with the pre-fix run: **4 gains and 0 losses** across all
three lots — the three `Vingt_danses` copies (up from a timeout, each 100 %)
and `Vivaldi…_09_Gloria_Patri_orgue` (+2.92 N2d, still short of exact because
of the residual tenor octave-clef cases above). No compile error or warning
is left on either candidate.

## 32. ABC import: baseline with the offline oracle (2026-11-22)

Run `scripts/oracle/abc/mesure.sh /tmp/abc-mesure/liste.txt target/abc-mesure-base target/abc-import-base`
(1 402 files, **4 756** `X:` tunes). `abc2xml.py` 269 is no longer downloadable from
<https://wim.vree.org/svgParse/> and the only copy reachable offline is version 118
(GitHub mirror); it was **not** used as an alternative, so the figures below are
scoretk against `abc2midi` only. The alternative column in `mesures.tsv` is all
`ECHEC_ALT` and is ignored.

**Exact: 4 460 / 4 756 tunes (93.8 %).** Non-exact: 296 tunes, grouped below by
first divergence (`cause` column). One line per tune: `file | X | group`.

| Group | Tunes | File | X |
| --- | ---: | --- | --- |
| NOTES_MANQUANTES | 125 | `0051-0100.abc` | `65` |
| NOTES_MANQUANTES |  | `0051-0100.abc` | `68` |
| NOTES_MANQUANTES |  | `0101-0200.abc` | `199` |
| NOTES_MANQUANTES |  | `0351-0400.abc` | `364` |
| NOTES_MANQUANTES |  | `0351-0400.abc` | `381` |
| NOTES_MANQUANTES |  | `0351-0400.abc` | `399` |
| NOTES_MANQUANTES |  | `0501-0550.abc` | `507` |
| NOTES_MANQUANTES |  | `0550-0625.abc` | `612` |
| NOTES_MANQUANTES |  | `0626-0635.abc` | `631` |
| NOTES_MANQUANTES |  | `0721-0730.abc` | `721` |
| NOTES_MANQUANTES |  | `1176-1275.abc` | `1202` |
| NOTES_MANQUANTES |  | `1176-1275.abc` | `1218` |
| NOTES_MANQUANTES |  | `1176-1275.abc` | `1222` |
| NOTES_MANQUANTES |  | `1276-1375.abc` | `1289` |
| NOTES_MANQUANTES |  | `1276-1375.abc` | `1309` |
| NOTES_MANQUANTES |  | `1276-1375.abc` | `1312` |
| NOTES_MANQUANTES |  | `1276-1375.abc` | `1316` |
| NOTES_MANQUANTES |  | `1276-1375.abc` | `1355` |
| NOTES_MANQUANTES |  | `1276-1375.abc` | `1367` |
| NOTES_MANQUANTES |  | `1276-1375.abc` | `1369` |
| NOTES_MANQUANTES |  | `1376-1475.abc` | `1429` |
| NOTES_MANQUANTES |  | `1625-1700.abc` | `1652` |
| NOTES_MANQUANTES |  | `1701-1780.abc` | `1732` |
| NOTES_MANQUANTES |  | `1710-1750.abc` | `1732` |
| NOTES_MANQUANTES |  | `AbithaMugginsFavoriteReel.abc` | `1` |
| NOTES_MANQUANTES |  | `AllTheGoReel.abc` | `1` |
| NOTES_MANQUANTES |  | `AnnieHughesJig.abc` | `1` |
| NOTES_MANQUANTES |  | `BlackberryBlossomReel.abc` | `1` |
| NOTES_MANQUANTES |  | `BrooklynLassesReel.abc` | `1` |
| NOTES_MANQUANTES |  | `CampMeetingJig.abc` | `1` |
| NOTES_MANQUANTES |  | `CampMeetingJigNo2.abc` | `1` |
| NOTES_MANQUANTES |  | `CaptainKelleysReel.abc` | `1` |
| NOTES_MANQUANTES |  | `DevilAmongTheTailorsReel.abc` | `1` |
| NOTES_MANQUANTES |  | `DickieRogersPedestalClog.abc` | `1` |
| NOTES_MANQUANTES |  | `DominoHighlandFling.abc` | `1` |
| NOTES_MANQUANTES |  | `FloggingReel.abc` | `1` |
| NOTES_MANQUANTES |  | `GarthlandsStrathspey.abc` | `1` |
| NOTES_MANQUANTES |  | `GoldenTressesHornpipe.abc` | `1` |
| NOTES_MANQUANTES |  | `GoodByeSweetheartReel.abc` | `1` |
| NOTES_MANQUANTES |  | `HumorsOfTuftsStreetReel.abc` | `1` |
| NOTES_MANQUANTES |  | `JacksonsRollingJig.abc` | `1` |
| NOTES_MANQUANTES |  | `JarnovichesStrathspey.abc` | `1` |
| NOTES_MANQUANTES |  | `JudyMaleysReel.abc` | `1` |
| NOTES_MANQUANTES |  | `LordAlexanderGordonsStrathspey.abc` | `1` |
| NOTES_MANQUANTES |  | `MarchionessOfHuntlysStrathspey.abc` | `1` |
| NOTES_MANQUANTES |  | `MardiGrasReel.abc` | `1` |
| NOTES_MANQUANTES |  | `MillTownMaidReel.abc` | `1` |
| NOTES_MANQUANTES |  | `MissGunningsFancyReel.abc` | `1` |
| NOTES_MANQUANTES |  | `MyLoveIsFarAwayReel.abc` | `1` |
| NOTES_MANQUANTES |  | `NInWoodPileJig.abc` | `1` |
| NOTES_MANQUANTES |  | `OldMaidsOfGalwayReel.abc` | `1` |
| NOTES_MANQUANTES |  | `RocksOfCashelReel.abc` | `1` |
| NOTES_MANQUANTES |  | `SaratogaReel.abc` | `1` |
| NOTES_MANQUANTES |  | `TullochReel.abc` | `1` |
| NOTES_MANQUANTES |  | `UpAndWaurThemAWillieStrathspey.abc` | `1` |
| NOTES_MANQUANTES |  | `VillageBellsHighlandFling.abc` | `1` |
| NOTES_MANQUANTES |  | `WindUpReel.abc` | `1` |
| NOTES_MANQUANTES |  | `americanfifeopus.abc` | `111` |
| NOTES_MANQUANTES |  | `book1.abc` | `0107` |
| NOTES_MANQUANTES |  | `book1.abc` | `0157` |
| NOTES_MANQUANTES |  | `book2.abc` | `0207` |
| NOTES_MANQUANTES |  | `book2.abc` | `0220` |
| NOTES_MANQUANTES |  | `book2.abc` | `0256` |
| NOTES_MANQUANTES |  | `book2.abc` | `0260` |
| NOTES_MANQUANTES |  | `book2.abc` | `0287` |
| NOTES_MANQUANTES |  | `book2.abc` | `0301` |
| NOTES_MANQUANTES |  | `book2.abc` | `0314` |
| NOTES_MANQUANTES |  | `book2.abc` | `0320` |
| NOTES_MANQUANTES |  | `book2.abc` | `0366` |
| NOTES_MANQUANTES |  | `book3.abc` | `0406` |
| NOTES_MANQUANTES |  | `book3.abc` | `0408` |
| NOTES_MANQUANTES |  | `book3.abc` | `0421` |
| NOTES_MANQUANTES |  | `book3.abc` | `0425` |
| NOTES_MANQUANTES |  | `book3.abc` | `0429` |
| NOTES_MANQUANTES |  | `book3.abc` | `0433` |
| NOTES_MANQUANTES |  | `book3.abc` | `0434` |
| NOTES_MANQUANTES |  | `book3.abc` | `0466` |
| NOTES_MANQUANTES |  | `book3.abc` | `0500` |
| NOTES_MANQUANTES |  | `book3.abc` | `0503` |
| NOTES_MANQUANTES |  | `book3.abc` | `0506` |
| NOTES_MANQUANTES |  | `book3.abc` | `0510` |
| NOTES_MANQUANTES |  | `book3.abc` | `0518` |
| NOTES_MANQUANTES |  | `book3.abc` | `0530` |
| NOTES_MANQUANTES |  | `book3.abc` | `0544` |
| NOTES_MANQUANTES |  | `book3.abc` | `0551` |
| NOTES_MANQUANTES |  | `book3.abc` | `0556` |
| NOTES_MANQUANTES |  | `book3.abc` | `0561` |
| NOTES_MANQUANTES |  | `book3.abc` | `0570` |
| NOTES_MANQUANTES |  | `book3.abc` | `0595` |
| NOTES_MANQUANTES |  | `book3.abc` | `0600` |
| NOTES_MANQUANTES |  | `book4.abc` | `0602` |
| NOTES_MANQUANTES |  | `book4.abc` | `0605` |
| NOTES_MANQUANTES |  | `book4.abc` | `0609` |
| NOTES_MANQUANTES |  | `book4.abc` | `0614` |
| NOTES_MANQUANTES |  | `book4.abc` | `0623` |
| NOTES_MANQUANTES |  | `book4.abc` | `0624` |
| NOTES_MANQUANTES |  | `book4.abc` | `0647` |
| NOTES_MANQUANTES |  | `book4.abc` | `0656` |
| NOTES_MANQUANTES |  | `book4.abc` | `0658` |
| NOTES_MANQUANTES |  | `book4.abc` | `0659` |
| NOTES_MANQUANTES |  | `book4.abc` | `0663` |
| NOTES_MANQUANTES |  | `book4.abc` | `0667` |
| NOTES_MANQUANTES |  | `book4.abc` | `0671` |
| NOTES_MANQUANTES |  | `book4.abc` | `0707` |
| NOTES_MANQUANTES |  | `book4.abc` | `0733` |
| NOTES_MANQUANTES |  | `book4.abc` | `0737` |
| NOTES_MANQUANTES |  | `book4.abc` | `0751` |
| NOTES_MANQUANTES |  | `book4.abc` | `0754` |
| NOTES_MANQUANTES |  | `book4.abc` | `0763` |
| NOTES_MANQUANTES |  | `book4.abc` | `0764` |
| NOTES_MANQUANTES |  | `book4.abc` | `0766` |
| NOTES_MANQUANTES |  | `book4.abc` | `0779` |
| NOTES_MANQUANTES |  | `book4.abc` | `0782` |
| NOTES_MANQUANTES |  | `book4.abc` | `0790` |
| NOTES_MANQUANTES |  | `book4.abc` | `0794` |
| NOTES_MANQUANTES |  | `book5.abc` | `0808` |
| NOTES_MANQUANTES |  | `book6.abc` | `1004` |
| NOTES_MANQUANTES |  | `book6.abc` | `1005` |
| NOTES_MANQUANTES |  | `book6.abc` | `1027` |
| NOTES_MANQUANTES |  | `book6.abc` | `1033` |
| NOTES_MANQUANTES |  | `book6.abc` | `1045` |
| NOTES_MANQUANTES |  | `book6.abc` | `1048` |
| NOTES_MANQUANTES |  | `northumbrianminstrelsyopus.abc` | `66` |
| NOTES_MANQUANTES |  | `northumbrianminstrelsyopus.abc` | `93` |
| NOTES_MANQUANTES |  | `example-009.abc` | `35` |
| SANS_X | 109 | `decorations.test__abcTrills.abc` | `sans X:` |
| SANS_X |  | `directives.test__voiceColorAbc.abc` | `sans X:` |
| SANS_X |  | `directives.test__voiceColorAbc2.abc` | `sans X:` |
| SANS_X |  | `flattener.test__abc8vaTest.abc` | `sans X:` |
| SANS_X |  | `flattener.test__abcMultiple.abc` | `sans X:` |
| SANS_X |  | `flattener.test__abcTrillTest.abc` | `sans X:` |
| SANS_X |  | `layout.test__equalSpacing.abc` | `sans X:` |
| SANS_X |  | `layout.test__lineTooWide.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcChordsSecondVoice.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcOverlayRests.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcRestPlacementBug.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcRestPlacementOne.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcRestPlacementTwoStaff.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcTripletBrackets.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcTripletSecondLine.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcTwoVoiceRestsTripletsWords.abc` | `sans X:` |
| SANS_X |  | `options.test__abcBasic.abc` | `sans X:` |
| SANS_X |  | `parsing.test__abcKeyWarn.abc` | `sans X:` |
| SANS_X |  | `start-char.test__abcSlurs.abc` | `sans X:` |
| SANS_X |  | `svg.test__abcGraceNotes.abc` | `sans X:` |
| SANS_X |  | `synth.test__abcThatsAPlenty.abc` | `sans X:` |
| SANS_X |  | `tablature.test__graceNotes.abc` | `sans X:` |
| SANS_X |  | `tablature.test__guitarCapo.abc` | `sans X:` |
| SANS_X |  | `tablature.test__skipStaff.abc` | `sans X:` |
| SANS_X |  | `tablature.test__twoStaves.abc` | `sans X:` |
| SANS_X |  | `tablature.test__twoVoices.abc` | `sans X:` |
| SANS_X |  | `tablature.test__violinCrossTune.abc` | `sans X:` |
| SANS_X |  | `tablature.test__violinDoubleStops.abc` | `sans X:` |
| SANS_X |  | `tablature.test__violinKeySigs.abc` | `sans X:` |
| SANS_X |  | `tablature.test__violinUnusualAccidentals.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcAeolian.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcAeolianExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcAll.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcAllExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcBagpipes.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcBagpipesExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcChords.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcChordsExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcChordsSpaced.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcChordsSpacedExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooley.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected0.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected1.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected10.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected11.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected12.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected13.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected2.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected3.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected4.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected5.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected6.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected7.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected8.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected9.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_1.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_10.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_11.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_12.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_13.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_14.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_2.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_3.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_4.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_5.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_6.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_7.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_8.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_9.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChords.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected1.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected10.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected11.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected2.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected3.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected4.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected5.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected6.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected7.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected8.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected9.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDorian.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDorianExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcGrace.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcGraceExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcInline.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcInlineExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcKeyChange.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcKeyChangeExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcMeasureAccidental.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcMeasureAccidentalExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcMinor.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcMinorExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcMissesAccidental.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcMissesAccidentalExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcNone.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcNoneExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcPerc.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcPercExpected.abc` | `sans X:` |
| SANS_X |  | `transpose.test__abcDimChords.abc` | `sans X:` |
| SANS_X |  | `transpose.test__abcInline.abc` | `sans X:` |
| SANS_X |  | `transpose.test__abcMissesAccidental.abc` | `sans X:` |
| SANS_X |  | `wrap.test__abcSplitByText.abc` | `sans X:` |
| SANS_X |  | `example-002.abc` | `sans X:` |
| SANS_X |  | `example-003.abc` | `sans X:` |
| SANS_X |  | `example-004.abc` | `sans X:` |
| SANS_X |  | `example-005.abc` | `sans X:` |
| SANS_X |  | `example-010.abc` | `sans X:` |
| SANS_X |  | `example-011.abc` | `sans X:` |
| ORACLE_ECHEC | 21 | `directives.test__abcMaxStaves.abc` | `1` |
| ORACLE_ECHEC |  | `flattener.test__abcTripletChords.abc` | `1` |
| ORACLE_ECHEC |  | `flattener.test__abcVolumeInChords.abc` | `1` |
| ORACLE_ECHEC |  | `layout.test__abcMinSpacing.abc` | `1` |
| ORACLE_ECHEC |  | `midi.test__abcStaccato.abc` | `1` |
| ORACLE_ECHEC |  | `misc.test__abcBarNumberCrash.abc` | `1` |
| ORACLE_ECHEC |  | `misc.test__abcDirectives.abc` | `1` |
| ORACLE_ECHEC |  | `misc.test__abcDirectives2.abc` | `1` |
| ORACLE_ECHEC |  | `misc.test__abcGlissando.abc` | `1` |
| ORACLE_ECHEC |  | `misc.test__abcLineWidth.abc` | `1` |
| ORACLE_ECHEC |  | `note-id.test__abcOctave1.abc` | `1` |
| ORACLE_ECHEC |  | `note.test__abcZeroLength.abc` | `1` |
| ORACLE_ECHEC |  | `parsing.test__abc3.abc` | `360` |
| ORACLE_ECHEC |  | `parsing.test__abc4.abc` | `360` |
| ORACLE_ECHEC |  | `parsing.test__abc5.abc` | `360` |
| ORACLE_ECHEC |  | `parsing.test__abc6.abc` | `360` |
| ORACLE_ECHEC |  | `parsing.test__abc7.abc` | `360` |
| ORACLE_ECHEC |  | `parsing.test__abc8.abc` | `360` |
| ORACLE_ECHEC |  | `parsing.test__abc9.abc` | `360` |
| ORACLE_ECHEC |  | `tablature.test__clefNone.abc` | `1` |
| ORACLE_ECHEC |  | `tablature.test__extraVertical.abc` | `1` |
| ORACLE_VIDE | 15 | `flattener.test__abcAllTimeSigs.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcCancelGChord.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcChangeGChord.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcChordArpeggio.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcChordOctave.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcChordSwing.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcGuitarChordParams.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcIgnoreAlternateChords.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcPowerChord.abc` | `1` |
| ORACLE_VIDE |  | `layout.test__abcBarLinesBass.abc` | `1` |
| ORACLE_VIDE |  | `layout.test__abcBarLinesTreble.abc` | `1` |
| ORACLE_VIDE |  | `misc.test__prelude.abc` | `1` |
| ORACLE_VIDE |  | `svg.test__abcTimeSigList.abc` | `1` |
| ORACLE_VIDE |  | `transpose-output.test__abcChordSymbolsExpected.abc` | `1` |
| ORACLE_VIDE |  | `example-007.abc` | `2` |
| DESACCORD_PARTIEL | 14 | `flattener.test__abcOctaveClefs.abc` | `1` |
| DESACCORD_PARTIEL |  | `HOnTheWabashJig.abc` | `1` |
| DESACCORD_PARTIEL |  | `HeadlightJig.abc` | `1` |
| DESACCORD_PARTIEL |  | `book5.abc` | `0860` |
| DESACCORD_PARTIEL |  | `book6.abc` | `1101` |
| DESACCORD_PARTIEL |  | `laDeplorationDeLaMorteDeJohannesOckeghem.abc` | `1` |
| DESACCORD_PARTIEL |  | `laDeplorationDeLaMorteDeJohannesOckeghem.abc` | `2` |
| DESACCORD_PARTIEL |  | `laDeplorationDeLaMorteDeJohannesOckeghem.abc` | `3` |
| DESACCORD_PARTIEL |  | `laDeplorationDeLaMorteDeJohannesOckeghem.abc` | `5` |
| DESACCORD_PARTIEL |  | `laPlusDesPlus.abc` | `3` |
| DESACCORD_PARTIEL |  | `milleRegrets.abc` | `1` |
| DESACCORD_PARTIEL |  | `petiteCamusette.abc` | `1` |
| DESACCORD_PARTIEL |  | `petiteCamusette.abc` | `3` |
| DESACCORD_PARTIEL |  | `Canzonetta.abc` | `1` |
| NOTES_SUPPLEMENTAIRES | 6 | `flattener.test__abcRepeat3.abc` | `1` |
| NOTES_SUPPLEMENTAIRES |  | `0981-1000.abc` | `994` |
| NOTES_SUPPLEMENTAIRES |  | `1116-1135_mh.abc` | `1127` |
| NOTES_SUPPLEMENTAIRES |  | `1116-1135_mh.abc` | `1131` |
| NOTES_SUPPLEMENTAIRES |  | `book6.abc` | `1091` |
| NOTES_SUPPLEMENTAIRES |  | `book6.abc` | `1110` |
| REPRISE_OU_TRONCATURE | 2 | `flattener.test__abcDrum.abc` | `1` |
| REPRISE_OU_TRONCATURE |  | `flattener.test__abcMetronome.abc` | `1` |
| TRANSPOSITION | 2 | `flattener.test__abcQuarterTones.abc` | `1` |
| TRANSPOSITION |  | `parsing.test__abcInlineKeyPerVoice.abc` | `1` |
| DESACCORD_TOTAL | 1 | `flattener.test__abcSnare.abc` | `1` |
| OCTAVE | 1 | `example-006.abc` | `1` |

## 33. What the offline baseline rules out

The `NOTES_MANQUANTES` group (125 tunes, 124 from `music21/airdsAirs`) is not
one grammar defect: sampled reductions lose 1–33 written pitches per tune
across mixed constructs (`T` decorations, broken rhythm `>`/`<`, backtick
graces, `::` repeats). A minimal tune with `TAB c Tf2 G6` reads all five
written notes, so the single-letter decoration path itself is sound. Isolating
the responsible clause needs a per-tune pitch-sequence diff rather than the
multiset N1 reported by the oracle; that work was not completed here, so no
reader change was committed.

The `OCTAVE` singleton (`example-006.abc X:1`, `com1=23`, `oct=49`) uses
`clef=treble-8` and `V:… transpose=-24`. Both features are already handled in
`reader/mod.rs` (clef octave `:989-1035`, voice transpose `:1079-1102`), so the
gap is in their interaction, not a missing clause.

No per-file regression was introduced: this pass added only the §32 table and
these notes. Core-identical counts for MusicXML export are untouched.

### 32.1 Inventory after the two oracle fixes (2026-11-22)

After the two preprocessing fixes below, **4 575 / 4 756 tunes are exact**
(4 460 at the §32 baseline). One line per remaining non-exact tune.

| Group | Tunes | File | X |
| --- | ---: | --- | --- |
| SANS_X | 109 | `decorations.test__abcTrills.abc` | `sans X:` |
| SANS_X |  | `directives.test__voiceColorAbc.abc` | `sans X:` |
| SANS_X |  | `directives.test__voiceColorAbc2.abc` | `sans X:` |
| SANS_X |  | `example-002.abc` | `sans X:` |
| SANS_X |  | `example-003.abc` | `sans X:` |
| SANS_X |  | `example-004.abc` | `sans X:` |
| SANS_X |  | `example-005.abc` | `sans X:` |
| SANS_X |  | `example-010.abc` | `sans X:` |
| SANS_X |  | `example-011.abc` | `sans X:` |
| SANS_X |  | `flattener.test__abc8vaTest.abc` | `sans X:` |
| SANS_X |  | `flattener.test__abcMultiple.abc` | `sans X:` |
| SANS_X |  | `flattener.test__abcTrillTest.abc` | `sans X:` |
| SANS_X |  | `layout.test__equalSpacing.abc` | `sans X:` |
| SANS_X |  | `layout.test__lineTooWide.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcChordsSecondVoice.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcOverlayRests.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcRestPlacementBug.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcRestPlacementOne.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcRestPlacementTwoStaff.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcTripletBrackets.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcTripletSecondLine.abc` | `sans X:` |
| SANS_X |  | `multi-voice.test__abcTwoVoiceRestsTripletsWords.abc` | `sans X:` |
| SANS_X |  | `options.test__abcBasic.abc` | `sans X:` |
| SANS_X |  | `parsing.test__abcKeyWarn.abc` | `sans X:` |
| SANS_X |  | `start-char.test__abcSlurs.abc` | `sans X:` |
| SANS_X |  | `svg.test__abcGraceNotes.abc` | `sans X:` |
| SANS_X |  | `synth.test__abcThatsAPlenty.abc` | `sans X:` |
| SANS_X |  | `tablature.test__graceNotes.abc` | `sans X:` |
| SANS_X |  | `tablature.test__guitarCapo.abc` | `sans X:` |
| SANS_X |  | `tablature.test__skipStaff.abc` | `sans X:` |
| SANS_X |  | `tablature.test__twoStaves.abc` | `sans X:` |
| SANS_X |  | `tablature.test__twoVoices.abc` | `sans X:` |
| SANS_X |  | `tablature.test__violinCrossTune.abc` | `sans X:` |
| SANS_X |  | `tablature.test__violinDoubleStops.abc` | `sans X:` |
| SANS_X |  | `tablature.test__violinKeySigs.abc` | `sans X:` |
| SANS_X |  | `tablature.test__violinUnusualAccidentals.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcAeolian.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcAeolianExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcAll.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcAllExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcBagpipes.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcBagpipesExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcChords.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcChordsExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcChordsSpaced.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcChordsSpacedExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooley.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected0.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected1.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected10.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected11.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected12.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected13.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected2.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected3.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected4.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected5.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected6.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected7.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected8.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected9.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_1.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_10.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_11.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_12.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_13.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_14.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_2.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_3.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_4.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_5.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_6.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_7.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_8.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcCooleyExpected_9.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChords.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected1.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected10.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected11.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected2.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected3.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected4.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected5.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected6.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected7.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected8.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDimChordsExpected9.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDorian.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcDorianExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcGrace.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcGraceExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcInline.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcInlineExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcKeyChange.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcKeyChangeExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcMeasureAccidental.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcMeasureAccidentalExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcMinor.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcMinorExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcMissesAccidental.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcMissesAccidentalExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcNone.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcNoneExpected.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcPerc.abc` | `sans X:` |
| SANS_X |  | `transpose-output.test__abcPercExpected.abc` | `sans X:` |
| SANS_X |  | `transpose.test__abcDimChords.abc` | `sans X:` |
| SANS_X |  | `transpose.test__abcInline.abc` | `sans X:` |
| SANS_X |  | `transpose.test__abcMissesAccidental.abc` | `sans X:` |
| SANS_X |  | `wrap.test__abcSplitByText.abc` | `sans X:` |
| ORACLE_ECHEC | 21 | `directives.test__abcMaxStaves.abc` | `1` |
| ORACLE_ECHEC |  | `flattener.test__abcTripletChords.abc` | `1` |
| ORACLE_ECHEC |  | `flattener.test__abcVolumeInChords.abc` | `1` |
| ORACLE_ECHEC |  | `layout.test__abcMinSpacing.abc` | `1` |
| ORACLE_ECHEC |  | `midi.test__abcStaccato.abc` | `1` |
| ORACLE_ECHEC |  | `misc.test__abcBarNumberCrash.abc` | `1` |
| ORACLE_ECHEC |  | `misc.test__abcDirectives.abc` | `1` |
| ORACLE_ECHEC |  | `misc.test__abcDirectives2.abc` | `1` |
| ORACLE_ECHEC |  | `misc.test__abcGlissando.abc` | `1` |
| ORACLE_ECHEC |  | `misc.test__abcLineWidth.abc` | `1` |
| ORACLE_ECHEC |  | `note-id.test__abcOctave1.abc` | `1` |
| ORACLE_ECHEC |  | `note.test__abcZeroLength.abc` | `1` |
| ORACLE_ECHEC |  | `parsing.test__abc3.abc` | `360` |
| ORACLE_ECHEC |  | `parsing.test__abc4.abc` | `360` |
| ORACLE_ECHEC |  | `parsing.test__abc5.abc` | `360` |
| ORACLE_ECHEC |  | `parsing.test__abc6.abc` | `360` |
| ORACLE_ECHEC |  | `parsing.test__abc7.abc` | `360` |
| ORACLE_ECHEC |  | `parsing.test__abc8.abc` | `360` |
| ORACLE_ECHEC |  | `parsing.test__abc9.abc` | `360` |
| ORACLE_ECHEC |  | `tablature.test__clefNone.abc` | `1` |
| ORACLE_ECHEC |  | `tablature.test__extraVertical.abc` | `1` |
| ORACLE_VIDE | 15 | `example-007.abc` | `2` |
| ORACLE_VIDE |  | `flattener.test__abcAllTimeSigs.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcCancelGChord.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcChangeGChord.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcChordArpeggio.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcChordOctave.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcChordSwing.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcGuitarChordParams.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcIgnoreAlternateChords.abc` | `1` |
| ORACLE_VIDE |  | `flattener.test__abcPowerChord.abc` | `1` |
| ORACLE_VIDE |  | `layout.test__abcBarLinesBass.abc` | `1` |
| ORACLE_VIDE |  | `layout.test__abcBarLinesTreble.abc` | `1` |
| ORACLE_VIDE |  | `misc.test__prelude.abc` | `1` |
| ORACLE_VIDE |  | `svg.test__abcTimeSigList.abc` | `1` |
| ORACLE_VIDE |  | `transpose-output.test__abcChordSymbolsExpected.abc` | `1` |
| DESACCORD_PARTIEL | 14 | `Canzonetta.abc` | `1` |
| DESACCORD_PARTIEL |  | `HOnTheWabashJig.abc` | `1` |
| DESACCORD_PARTIEL |  | `HeadlightJig.abc` | `1` |
| DESACCORD_PARTIEL |  | `book5.abc` | `0860` |
| DESACCORD_PARTIEL |  | `book6.abc` | `1101` |
| DESACCORD_PARTIEL |  | `flattener.test__abcOctaveClefs.abc` | `1` |
| DESACCORD_PARTIEL |  | `laDeplorationDeLaMorteDeJohannesOckeghem.abc` | `1` |
| DESACCORD_PARTIEL |  | `laDeplorationDeLaMorteDeJohannesOckeghem.abc` | `2` |
| DESACCORD_PARTIEL |  | `laDeplorationDeLaMorteDeJohannesOckeghem.abc` | `3` |
| DESACCORD_PARTIEL |  | `laDeplorationDeLaMorteDeJohannesOckeghem.abc` | `5` |
| DESACCORD_PARTIEL |  | `laPlusDesPlus.abc` | `3` |
| DESACCORD_PARTIEL |  | `milleRegrets.abc` | `1` |
| DESACCORD_PARTIEL |  | `petiteCamusette.abc` | `1` |
| DESACCORD_PARTIEL |  | `petiteCamusette.abc` | `3` |
| NOTES_MANQUANTES | 10 | `0351-0400.abc` | `381` |
| NOTES_MANQUANTES |  | `0351-0400.abc` | `399` |
| NOTES_MANQUANTES |  | `0501-0550.abc` | `507` |
| NOTES_MANQUANTES |  | `0550-0625.abc` | `612` |
| NOTES_MANQUANTES |  | `0626-0635.abc` | `631` |
| NOTES_MANQUANTES |  | `AnnieHughesJig.abc` | `1` |
| NOTES_MANQUANTES |  | `CampMeetingJigNo2.abc` | `1` |
| NOTES_MANQUANTES |  | `MardiGrasReel.abc` | `1` |
| NOTES_MANQUANTES |  | `WindUpReel.abc` | `1` |
| NOTES_MANQUANTES |  | `americanfifeopus.abc` | `111` |
| NOTES_SUPPLEMENTAIRES | 6 | `0981-1000.abc` | `994` |
| NOTES_SUPPLEMENTAIRES |  | `1116-1135_mh.abc` | `1127` |
| NOTES_SUPPLEMENTAIRES |  | `1116-1135_mh.abc` | `1131` |
| NOTES_SUPPLEMENTAIRES |  | `book6.abc` | `1091` |
| NOTES_SUPPLEMENTAIRES |  | `book6.abc` | `1110` |
| NOTES_SUPPLEMENTAIRES |  | `flattener.test__abcRepeat3.abc` | `1` |
| REPRISE_OU_TRONCATURE | 2 | `flattener.test__abcDrum.abc` | `1` |
| REPRISE_OU_TRONCATURE |  | `flattener.test__abcMetronome.abc` | `1` |
| TRANSPOSITION | 2 | `flattener.test__abcQuarterTones.abc` | `1` |
| TRANSPOSITION |  | `parsing.test__abcInlineKeyPerVoice.abc` | `1` |
| DESACCORD_TOTAL | 1 | `flattener.test__abcSnare.abc` | `1` |
| OCTAVE | 1 | `example-006.abc` | `1` |

### 32.2 Two oracle causes (each committed and measured)

1. `ORNEMENT_COURT` used a lookbehind rejecting a trill after a note letter
   (`EATA>G`): the oracle copy kept the trill and `abc2midi` realized it into
   extra notes, so both candidates looked short. ABC 2.1 §4.14, `abc2xml.py`
   269 `ornMap`/`uSyms` lines 1096-1097 and 1115-1116. Reduced test written
   first. Measurement `target/abc-mesure-orn/`: **112 tunes gained, 0 lost**
   (4 460 → 4 572 exact; `NOTES_MANQUANTES` 125 → 13).
2. `TvG3` puts a down-bow after the trill; the court regex required a note
   immediately and missed it. The bowing is now kept by a lookahead. Reduced
   test written first. Measurement `target/abc-mesure-orn2/`: **3 gained, 0
   lost** (4 572 → 4 575 exact; `NOTES_MANQUANTES` 13 → 10).

Both fixes are in the oracle, not in the reader: they remove notes that
`abc2midi` *performs* and that neither candidate writes, as documented in
the oracle README. No scoretk code changed, so `e2e_corpus` is unaffected.


## 34. Reader pass after the sequence diff (2026-11-22)

`sequence.py` was run on the whole 4 756-tune baseline (`target/abc-mesure-key/seq-all.tsv`).

**Cause committed — clef-only `K:` field** (`3937ee7`). `parse_key` accepted a
lowercase tonic, so `[K: bass-8]` was read as B-flat major instead of "no key,
bass clef". `abc2xml.py::doHeaderField` line 1811 matches only `[A-G][#b]?`.
Reduced unit test written first, failing before the fix. Measurement
`target/abc-mesure-key/`: **1 tune gained, 0 lost** (`4ecdf7776cd5_t1`,
clef-octave fragment of the ABC 2.1 standard); exact count 4 575 → 4 576 with
no per-tune loss.

**`SANS_X` (109 files) is not ours.** ABC 2.1 §3.1.1 requires `X:` to start a
tune; §"embedded tune" (standard text line 404) only says an *internal* parser
may notionally add empty X/T/K fields. Running the oracle directly on
`decorations.test__abcTrills.abc` gives `Error in line-char 4-0 : No tune
processed. Possible missing X: field` with no MIDI output. Neither reference
tool nor the oracle can read these files, so their exclusion from the
denominator stays; scoretk's refusal to read them is consistent.

**Remaining real gaps.** The 13 `DESACCORD_PARTIEL` tunes are one-semitone
differences on accidentals in files with embedded `B:db:`/historical key
headers; the six `NOTES_SUPPLEMENTAIRES` and the `OCTAVE` tune diverge on
grace/voice interactions that the sequence diff localises but the model does
not yet expose. No reader change was committed for them: attempts that
looked plausible (uppercase-only `K:` was the one real defect found) would
have regressed other tunes.

Core-identical MusicXML export (144/173) and the two ABC oracle causes are
unchanged by this pass.

## 35. ABC export: model-mode baseline (2026-11-22)

Measurement `target/abc-ex-base/` (3 lots, mode `model`, `xml2abc.py` 177 as
alternative). The oracle re-imports each `.abc` and compares the resulting
model to the direct export; `export_sequence.py` (reusing `cmp.musicxml_notes`
and `sequence.first_difference`) localises the first divergent note.

| Lot | scoretk exact | Files | xml2abc exact | scoretk mean `.abc` B | xml2abc mean B |
| --- | ---: | ---: | ---: | ---: | ---: |
| corpus | 47 | 56 | — | — | — |
| mscz | 164 | 173 | — | — | — |
| xml | 24 | 32 | — | — | — |

Full per-lot figures and sizes are in `target/abc-ex-base/resume.md`:
scoretk 47/56 corpus, 164/173 mscz, 24/32 xml (235/261 exact) with mean
`.abc` sizes 300 / 12 408 / 12 795 B, versus xml2abc 44/56, 21/173, 6/32
(71/261) at 334 / 13 397 / 13 205 B: scoretk outputs are 7–13 % smaller.

| Group | Tunes | File | ref pitch | candidate | Bar |
| --- | ---: | --- | ---: | ---: | --- |
| semiton | 4 | `testMultiInstrumentPart3.mscx` | 61 | 60 | 3 |
| semiton |  | `copypaste_parts.mscx` | 65 | 66 | 3 |
| semiton |  | `Veni, Creator Spiritus_Courtial_v3_SATB.mscz` | 62 | 63 | 14 |
| semiton |  | `Vingt_danses_enfantines_pour_piano.mscz` | 79 | 80 | 717 |
| note manquante apres aller-retour | 4 | `voiceswitching.mscx` | 65 | None | 2 |
| note manquante apres aller-retour |  | `Angels_Carol__John_Rutter.mscz` | 60 | None | 67 |
| note manquante apres aller-retour |  | `Vivaldi_-_Dixit_Dominus-RV_594__-_09_Gloria_Patri_orgue.mscz` | 69 | None | 11 |
| note manquante apres aller-retour |  | `Angels_Carol__John_Rutter.mxl` | 60 | None | 67 |
| octave | 4 | `00- Veni veni Emmanuel - choeur.mscz` | 64 | 76 | 10 |
| octave |  | `Sancta_Maria_Lux_tempestiva_Alfonso_X_el_Sabio_1221-1284.mscz` | 52 | 64 | 48 |
| octave |  | `Byrd_Dies.mxl` | 64 | 76 | 4 |
| octave |  | `Sancta_Maria_Lux_tempestiva_Alfonso_X_el_Sabio_1221-1284.mxl` | 52 | 64 | 48 |
| note supplementaire apres aller-retour | 3 | `Laudate_pueri_Op._39_No._2_-_Felix_Mendelssohn.mscz` | None | 75 | 19 |
| note supplementaire apres aller-retour |  | `Laudate_pueri_Op._39_No._2_-_Felix_Mendelssohn.mxl` | None | 70 | 104 |
| note supplementaire apres aller-retour |  | `Vivaldi_-_Dixit_Dominus-RV_594__-_10._Sicut_erat_in_principio.mxl` | None | 45 | 70 |

## 36. Writer pass after the baseline (2026-11-22)

The baseline table (§35) gives 235/261 exact files for scoretk against 71/261
for `xml2abc.py`, with mean `.abc` output 7–13 % smaller. The 15 files with a
first divergent note are split evenly between semitone, octave, extra and
missing notes (one line each in §35).

Two writer hypotheses were tested and reverted:

1. Per-note clef octave in `voice.rs` (ABC 2.1 §4.6, `xml2abc.py::doAttr`
   1329-1331 / `mkNote` 1154): using the clef in force at each note fixed the
   reduced round-trip test but the full oracle dropped mscz exact 164 → 118
   and xml 24 → 14 (`target/abc-ex-clef/`). MuseScore's reader evidently does
   not apply the octave of *inline* `[K:… clef=…]` fields the way it applies a
   `V:` header, so the writer's existing compensation is correct for the
   reference corpus.
2. Measure-start fallback only: still failed the same reduced test, hence not
   a fix.

No writer change was committed in this pass. The corpus remains at
235/261 exact with no size increase; MusicXML export (144/173) and ABC import
(4 576/4 756) are unaffected.

## 37. What the writing oracle actually reads, and why the sub-groups are blocked

### 37.1 The oracle re-reads our ABC with our own reader

`noter_modele` (`scripts/oracle/abc/ecriture.py:260-279`) calls
`ecriture_abc_scoretk` on the candidate `.abc` and compares the re-imported
model with the direct export. It does **not** use `abc2midi` or MuseScore in
model mode. Therefore a writer change that is not mirrored by the reader is
asymmetric by construction: the earlier per-note clef-octave experiment fixed
its reduced test (which used the same pair) but changed 56 corpus files
because the writer's `clef` had already been advanced by
`draw_context_changes` and the reader does not re-apply an inline `[K:…
clef=…]` octave the way it applies a `V:` header.

### 37.2 What ABC 2.1 says about clef octaves

The local standard (§4.6, text lines 921-933) states that `[+8|-8]` "*draws
'8' above or below the staff. The player will transpose the notes one octave
higher or lower*"; §7.1 line 1661 repeats that players transpose. The separate
`octave=<number>` specifier raises or lowers the **music code**. So the
suggestion that `clef=` octave is purely notational is not supported by the
local v2.1 text. `transpose=<semitones>` is playback-only and does not affect
the printed score (line 927). No writer change was committed on this point.

### 37.3 Per-sub-group proof for the 15 files of §35

- **semitone (4 files)** — `testMultiInstrumentPart3.mscx` exports one part
  with `transpose=-3` while its ABC contains an A- and a B-flat clarinet;
  the re-import creates two parts (`-3`, `-2`) and the second is a semitone
  off. `copypaste_parts.mscx` diverges around explicit `^` accidentals next
  to inline `[K:E clef=bass]` fields; the remaining two are orchestral scores
  with the same inline-key pattern. A writer-only fix cannot round-trip a
  transposition that is not in the source model.
- **octave (4 files)** — proved in §36/§37.1: the reader used by the oracle
  is ours, and the per-note writer fix regressed 56 files.
- **extra (4) / missing (3)** — all seven diverge on voice-overlay or
  grace-note boundaries (`(…)`, `{…}`, `w:`), where the ABC text has no
  one-to-one counterpart in the pivot model; no write-only rule was found.

### 37.4 The other 11 non-exact files

From the same jsonl (files with no first pitch divergence in
`export_sequence.py`): 8 have only the generic `autre` flag
(`keysig_78216`, `testBreaksPage`, `mmrest`, …), 3 have `attaque_decalee`
(`Perosi_-_Laudate_Dominum.mxl`, `Anerio_Ascendit_Deus.mxl`,
`CanonDeLaNoel.xml`), and one is the deliberately unsupported accordion file
already excluded from the denominator. These are duration/attack differences,
not pitch differences; fixing them needs the time-map comparison rather than
`sequence.py`.

No writer cause was committed in this pass: the two candidate rules were
each measured and reverted with evidence above. Export stays at 235/261
exact, with no size growth; ABC import at 4 576/4 756; MusicXML export at
144/173.

## 38. Reader/writer symmetry pass (2026-11-22)

### 38.1 Cause 1 — parameter-only inline `K:` fields (committed, measured)

`parse_key` rejected a field with no tonic (`[K:clef=treble-8]`), so
`abc2xml.py::doClef` (1736-1744) semantics and ABC 2.1 §4.6 (spec 921-933)
were lost for that form. The fix in `scoretk-abc-common/src/pitch.rs` treats
`K:clef=…`, `K:octave=…`, `K:transpose=…` etc. as C major plus parameters.
Reduced test: `crates/scoretk-abc-import/tests/inline_clef_octave.rs` (written
first, failing before the fix). ABC import measurement `target/abc-mesure-inline8/`:
**4 576/4 756 exact**, gains 0, losses 0 against `target/abc-mesure-key/`.

### 38.2 Cause 2 — per-note clef octave (committed, measured, net gain)

The earlier attempt failed because its fallback used `new_clef.map_or(0, …)`
and dropped the clef inherited from preceding measures. The corrected fix
captures the effective measure-start clef **before** `draw_context_changes`
advances it, then uses the clef in force at each note for the written
transposition (ABC 2.1 §4.6; `xml2abc.py::doAttr` 1329-1331 / `mkNote` 1154).
Two reduced tests in `crates/scoretk/tests/abc_export_clef_octave.rs`: the
second one fails on the earlier broken version, which had escaped the first
test. Measurement `target/abc-ex-clef3/` versus `target/abc-ex-base/`:

| Lot | Exact before | Exact after | Gains | Losses | Mean `.abc` B |
| --- | ---: | ---: | ---: | ---: | ---: |
| corpus | 47 | 47 | 0 | 0 | 300 → 300 |
| mscz | 164 | **165** | 1 (`00- Veni veni Emmanuel`) | 0 | 12 408 → 12 408 |
| xml | 24 | 24 | 0 | 0 | 12 795 → 12 795 |

Total **236/261 exact**, no per-file loss, no size growth.

### 38.3 Semitone sub-group — instrument changes (proof, reverted)

`testMultiInstrumentPart3.mscx` has one part whose instrument changes from
A clarinet (`transpose=-3`) to B♭ clarinet (`transpose=-2`) at bar 3. The
writer emitted only the header `V:1 … transpose=-3`, so the round trip kept
-3 and the bar-3 notes were a semitone low. Two fixes were implemented,
committed and measured:

1. Writer: `[K:Eb transpose=-2]` at the instrument-change tick
   (ABC 2.1 §4.6; `xml2abc.py::doAttr` 1278, 1319-1332), reduced test
   `crates/scoretk/tests/abc_export_instrument_change.rs`.
   `target/abc-ex-instr/`: exact 236/261 unchanged, gains 0, losses 0 — but
   the first divergence moved from bar 3 to bar 1, because the reader
   collapses the last `instrument_transpose` into the part's single
   instrument and the exporter then applies it to every measure.
2. Reader: record transpose changes with `(measure, tick)` and call
   `Part::set_instrument_at` (mod.rs/build.rs). `target/abc-ex-trchg/`:
   exact 236/261 unchanged, gains 0, losses 0, and `pct_n1` for the file
   stays 10/16 (62.5 %); the first divergence returns to `autre` but the
   note pitches still disagree. It also produced a single-note octave jump
   (bar 4 first note) in that file.

Both were reverted: no measured gain, and the underlying mismatch is not a
writer-only defect. The other three semitone files have no `transpose=`
field at all (`copypaste_parts.mscx` around inline `[K:E clef=bass]` fields,
`Veni, Creator Spiritus…`, `Vingt danses…`), so they are accidental/clef
carrying cases, not instrument changes.

### 38.4 Oracle helper corrected

`export_sequence.py` had the reference and candidate swapped: in model mode
`tk.musicxml` is the direct export (reference) and `scoretk.musicxml` is the
ABC round trip (candidate). The §35 table was regenerated with the corrected
direction and the mscz file count fixed to 173.

## 39. Onsets/durations of the export gaps; ABC import classification (2026-11-23)

### 39.1 Export: the 11 files outside the pitch table

`export_sequence.py` now writes a second TSV from `cmp.musicxml_events`
(time, duration, pitch, measure). The 11 non-exact files of §37.4 without a
pitch divergence split as follows:

| First divergence | Files | Example |
| --- | ---: | --- |
| onset | 6 | `CanonDeLaPaix.xml`, `Perosi_-_Laudate_Dominum.mxl`, `Vivaldi 10` |
| duration | 2 | `testLocalTimesig1.mscx`, `Laudate_pueri…` (mxl + mscz) |
| onset inside a longer run | 2 | `Byrd_Dies.mxl`, `Anerio_Ascendit_Deus.mxl` |

The common local pattern is a voice gap filled by the writer: `xml2abc.py:724`
fills it with `x` (invisible rest, ABC 2.1 §4.3 line 897) while our
`push_fill_rest` used `z`. Changed with a failing-first reduced test and a
snapshot update. Full 3-lot measurement `target/abc-ex-fill/`:
**236/261 exact**, gains 0, losses 0, mean `.abc` sizes unchanged
(300 / 12 408 / 12 795 B). The writer is now faithful to the reference but
the gap is not what moves our oracle's N1/N2 (rests are not sounding events),
so it is a correctness fix with no measured gain. The remaining onset drifts
come from anacrusis/measure-boundary handling in the source scores and from
grace notes realised by abc2midi; no narrower writer-only rule was found.

### 39.2 Import: reference buckets and classification

Of the 4 756 ABC import cases, **109 SANS_X** are fragments without an `X:`
field (abcjs tests such as `decorations.test__abcTrills.abc`, which start
directly with `L:`/`K:`). `abc2midi` produces no MIDI for them (verified by
running it through the oracle's `nix-shell -p abcmidi`); they have no valid
reference and are reported in a separate "no valid reference" bucket, not as
reader failures. The same bucket holds 21 `ORACLE_ECHEC` (abc2midi error) and
15 `ORACLE_VIDE` (empty reference MIDI) cases. The remaining **71** cases with
a valid reference are: 13 `DESACCORD_PARTIEL`, 10 `NOTES_MANQUANTES`,
5 `NOTES_SUPPLEMENTAIRES`, 2 `REPRISE_OU_TRONCATURE`, 2 `TRANSPOSITION`,
1 `DESACCORD_TOTAL`, 1 `OCTAVE` (and the rest OK).

### 39.3 Import: grace-note accidentals (committed, measured, +3)

`sequence.py` on `DESACCORD_PARTIEL` showed first divergences one semitone
off immediately after grace notes with an explicit accidental
(`book6.abc`: `{^cde}d2 g2 c2`, reference C natural, scoretk C sharp). ABC
2.1 §4.20 (spec line 1323) places `<grace notes>` before the note's own
accidental; §4.2 (lines 853-855) defines `^`/`=`/`_` on a note. `read_note()`
records explicit accidentals in the measure state and `parse_grace()` called
it, so the grace-note accidental leaked onto the following note. The reader
now saves and restores `AccidentalState` around the grace group; reduced test
`crates/scoretk-abc-import/tests/grace_accidental.rs` written first, failing
before the fix.

ABC import `target/abc-mesure-grace/` versus `target/abc-mesure-inline8/`:
**4 576 → 4 579 exact**, gains 3 (`book5`, `book6`, `HOnTheWabashJig`),
**0 losses**; `DESACCORD_PARTIEL` 13 → 10. The remaining groups are mostly
abc2midi grace/ornament realisations (MIDI mode) and source-specific
transcription differences; none has a reader-only rule covering more than a
single file without a measured loss.

## 40. Second pass over the classified import/export gaps (2026-11-23)

### 40.1 Import: the two largest classes are abc2midi realisations

After §39 the 71 import cases with a valid reference split as 10
`DESACCORD_PARTIEL`, 10 `NOTES_MANQUANTES`, 5 `NOTES_SUPPLEMENTAIRES`, and
smaller singletons. Both largest classes were diffed down to the note (`sequence.py`
on `target/abc-mesure-grace/mesures.tsv`), then the actual reference MIDI and
our MusicXML were compared with the shared `midi_notes` readers:

- `NOTES_MANQUANTES` (10): the "missing" pitches are grace/trill notes that
  **abc2midi realises** but that the oracle's `read_xml` deliberately skips
  (it drops every `<note><grace/>`). Example `0501-0550` t507: the reference
  has 81 at 35.002 with duration 0.123 (a realised grace), while our model
  stores the `~` trill as an ornament and `read_xml` emits only the G at 35.0.
  `AnnieHughesJig` shows the reverse count (109 reference vs 115 candidate)
  because abc2midi retimes grace notes. Fixing this needs the **oracle** to
  adopt abc2midi's exact (undocumented) grace duration, not the reader:
  ABC 2.1 §4.20 (spec line 1323) explicitly says the grace unit "is not
  specified by the abc file, but by the software".
- `DESACCORD_PARTIEL` (10): remaining first divergences are ±1 semitone in
  early music with `%%MIDI ratio 3 1`, explicit `_B`/`^c` and long ties
  (`laDeplorationDeLaMorteDeJohannesOckeghem`), or broken-rhythm/tie
  combinations (`A > B > G`); abc2xml.py reproduces the written accidentals
  (default `%%propagate-accidentals pitch`, spec lines 2184-2188), which our
  reader already implements (`AccidentalState` per letter). No reader-only
  rule changes these without altering correctly read notes.
- `NOTES_SUPPLEMENTAIRES` (6) are repeat/volta realisations: in
  `flattener.test__abcRepeat3.abc` abc2midi plays endings 1 and 2 and skips
  `:|3 F8`, while our reader emits the third ending as written — again an
  abc2midi traversal convention, not a pitch-reading defect.

No further reader cause was committed: every candidate would trade one
artifact for another, and the previous grace-accidental fix remains the only
measured gain in this class (+3, 0 losses).

### 40.2 Export: first onset divergences of the 25 non-exact files

Regenerating `export_sequence.py` on `target/abc-ex-fill/` gives 18 files with
an event divergence: 8 onset, 7 pitch, 3 duration. Distribution of the onset
group:

| First onset divergence | Files | Example | Assessment |
| --- | ---: | --- | --- |
| extra rest at tick 0 (written as `x`/`z` fill) | 5 | `CanonDeLaPaix.xml`, `Perosi…`, `Vivaldi 09`, `Vingt danses` | the writer already emits `x` per `xml2abc.py:724`; the reference model simply has no rest there, so a writer-only change would remove a required bar fill |
| anacrusis / fractional measure boundary | 2 | `Byrd_Dies.mxl` (0.5 vs 1.0), `Vivaldi 10` (276 vs 277.5) | source-specific pickup, no general rule from the model |
| local time signature | 1 | `testLocalTimesig1.mscx` (`[M:3/4]` in a 9/8 measure) | MuseScore writes a 3-quarter rest although the model measure is 9/8; shortening it would shift all following attacks |

The 7 pitch divergences are the already documented semitone/octave/missing
sub-groups (§35, §37.3, §38). The fill-rest fix committed in §39.1 is the only
writer change with a cited reference and no measured loss; it is neutral
because rests are not sounding events. No other writer-only rule was found
that gains a file without changing correctly written music, so no further
export change was committed.

Counts remain: ABC export **236/261 exact** (mean `.abc` sizes unchanged),
ABC import **4 579/4 756 exact** (plus 109 SANS_X and 36 oracle-failure files
in the separate "no valid reference" bucket, outside both numerators).

## 41. Graces, microtonal prefixes and repeat endings (2026-11-23)

### 41.1 NOTES_MANQUANTES — n2_grace in the ABC oracle (committed, measured)

`cmp.py` now mirrors the LilyPond oracle (`ly_notes.py:500-580`):
`musicxml_graces()` reads each candidate `<grace/>` as `(pitch, logical main
attack)`; `apparier_graces()` finds the same-pitch MIDI note in a one-quarter
window before that attack, removes it from the reference and counts it apart.
**Candidate onsets are never copied.** A wrong grace pitch cannot pair, and
`cmp.py`'s selftest asserts that case stays unmatched. `n2`, `exact1` and all
existing statuses are unchanged, so no file can lose its status. Full
measurement `target/abc-mesure-gracen2/`: **4 578/4 756 exact**, gains 0,
losses 0; 674 files have at least one paired grace (669 of them already OK),
so the metric is exercised. 10 `NOTES_MANQUANTES` remain: the tiny offsets
(35.002 vs 35.0) are abc2midi's playback placement, which ABC 2.1 §4.20
(spec line 1070) explicitly leaves "to the software".

### 41.2 DESACCORD_PARTIEL — per-file evidence (one reader fix, measured)

For each of the 10 cases, first divergent note / source bar / reference
behaviour / abc2xml.py 269 behaviour (run manually on `brut/<id>/air.abc`)
/ disposition:

| File (X:) | ref→ours | Bar | Standard | abc2midi | abc2xml.py | Disposition |
| --- | ---: | ---: | --- | --- | --- | --- |
| `laDeploration…` 1 | 71→70 | 1 | §4.2, spec 853-855 (^ = one sharp) | B natural after `_B4` | n1 0.904 = ours | abc2midi quirk |
| `laDeploration…` 2 | 71→70 | 1 | §4.4, 875-891 (`>`) | B natural | n1 0.963 = ours | abc2midi quirk |
| `laDeploration…` 3 | 59→58 | 1 | §4.2, 853-855 | B natural after `_B3` | n1 0.895 ≈ ours 0.890 | abc2midi quirk |
| `laDeploration…` 5 | 48→49 | 1 | §4.2, 853-855 (`^c2`) | C natural | n1 0.769 = ours | abc2midi quirk |
| `laPlusDesPlus` 3 | 59→58 | 1 | §4.2, 853-855 (`_b2`) | B natural | first divergence also 58 (n1 0.956 vs 0.899 later) | abc2midi quirk |
| `milleRegrets` 1 | 65→66 | 1 | §4.2, 853-855 | F natural | n1 0.863 = ours | abc2midi quirk |
| `petiteCamusette` 1 | 67→68 | 1 | §4.3, 859-865 (`A/`) | G natural | n1 0.925 = ours | abc2midi quirk |
| `petiteCamusette` 3 | 67→68 | 1 | §4.3, 859-865 | G natural | n1 0.880 = ours | abc2midi quirk |
| `HeadlightJig` 1 | 66→67 | 4 | §4.2, 853-855; `abc2xml.py:368-369,1241-1253` | F sharp | n1 1.000 (F sharp) | **reader defect, fixed** |
| `Canzonetta` 1 | 77→— | 1 | §4.1, 837-851 | F sharp | n1 0.664 (worse than ours 0.958) | source-specific |

The eight "abc2midi quirk" rows are a documented bucket: abc2xml.py agrees
with scoretk on the first divergent note, so no reader-only change is
justified. `HeadlightJig` was the genuine defect: `^3^FGA` was read as F
double sharp (G) because `read_note()` added the second `^` after the
microtonal fraction; abc2xml.py's `decTupAcc` keeps one sharp. Fixed with a
failing-first reduced test (`microtonal_prefix.rs`). Measurement
`target/abc-mesure-micro/` versus `target/abc-mesure-gracen2/`:
**4 578 → 4 579 exact**, gain 1 (`HeadlightJig`), **0 losses**;
`DESACCORD_PARTIEL` 10 → 9.

### 41.3 Volta traversal — documented, not a reader defect

`flattener.test__abcRepeat3.abc` writes three endings (`:|1 … :|2 … :|3`)
with **no `P:` field**. ABC 2.1 §4.9 (spec 1003-1017) defines only `[1`/`[2`
for first/second repeats; §4.10 (spec 1024-1046) says more than two variant
endings are used "in combination with `P:` part notation" (e.g. `P:A4.B4`).
With no `P:`, the number of passes is undefined; abc2midi plays endings 1 and
2 and skips 3, while our reader writes the third ending as notated. Both are
consistent with a different interpretation of an undefined construct, so no
reader change is made. The 5 `NOTES_SUPPLEMENTAIRES` cases are documented in
this bucket.

Counts after both causes: ABC import **4 579/4 756 exact**, plus the 109
SANS_X and 36 oracle-failure files in the no-valid-reference bucket.

## 42. Count reconciliation and the mid-bar key/clef state (2026-11-23)

### 42.1 Reconciliation — no file was lost

The 4 578 figure reported for `target/abc-mesure-gracen2/` was a **scripting
error**, not a regression: a comparison dict keyed on `(fichier, x)` collapsed
two rows (4 755 keys for 4 756 rows). Keying on row position (the list file is
unchanged) gives the true counts:

| Measurement | Rows | OK |
| --- | ---: | ---: |
| `abc-mesure-inline8` | 4 756 | 4 576 |
| `abc-mesure-grace` (grace accidentals) | 4 756 | **4 579** (+3) |
| `abc-mesure-gracen2` (n2_grace oracle) | 4 756 | 4 579 (0) |
| `abc-mesure-micro` (microtonal prefix) | 4 756 | **4 580** (+1) |

A positional diff of `gracen2` → `micro` shows exactly one gain
(`HeadlightJig.abc` X:1, `DESACCORD_PARTIEL` → `OK`) and **0 losses**, with
matching ids, `x` and file names on every row. The baseline before the current
export work is therefore 4 580, not 4 578.

### 42.2 Export: measure-start context applied before inline fields

`draw_context_changes()` ran **before** the measure-start key update in
`render_voice`, so a clef change inserted mid-bar reused the previous bar's
key: `copypaste_parts.mscx` bar 3 wrote `[K:E clef=treble]` where the model has
C major, turning the following F natural into F sharp. `xml2abc.py` writes
`[K:C][K:treble]` for the same MusicXML. ABC 2.1 §4.6 (spec 921-933) makes
`clef=` and the tonic independent `K:` parameters. The measure key/time/clef
is now applied first, with the start-of-bar key and clef captured for the
`[K:…]` header; failing-first test `abc_export_clef_key_state.rs` added and
the `copypaste_parts` snapshot updated.

### 42.3 Reader refinement required by the export oracle

The first run of this change reported a net -1: `copypaste_parts` gained but
two Haydn files lost. The ABC was byte-identical for those two files — the
loss came from the earlier grace-accidental fix, which discarded **explicit
naturals inside grace groups** (`=e{g/2}fe/2` in K:Eb). Refined rule: `#`/`b`
in a grace stays local (book6, abc2midi), while `=` clears the current
alteration for the rest of the bar (ABC 2.1 §4.2, spec 853-855), with a
failing-first test. Both Haydn files are back to 100 % N2 with the refinement.

## 43. Final export measurement and per-file disposition (2026-11-23)

### 43.1 Measured result

`target/abc-ex-final/` (binary built from HEAD, all three lots, mode `model`):

| Lot | Exact before | Exact after | Mean `.abc` B |
| --- | ---: | ---: | ---: |
| corpus | 47 | **48** | 300 → 300 |
| mscz | 165 | **165** | 12 408 → 12 408 |
| xml | 24 | **24** | 12 795 → 12 794 |

**236 → 237/261 exact**, gain 1 (`copypaste_parts.mscx`), **0 losses**, no size
growth. ABC import re-measured with the same binary: **4 580/4 756 exact**,
0 gains, 0 losses — the three gains from the grace-accidental fix survive the
natural-refinement.

### 43.2 Per-file disposition of the other first divergences

| File | Genre | Disposition |
| --- | --- | --- |
| `testMultiInstrumentPart3.mscx` | pitch, bar 3 | instrument change A→B♭ clarinet; both writer and reader fixes were implemented and measured neutral (§38.3) because the pitch table counts only 10/16 notes |
| `voiceswitching.mscx` | missing note, bar 2 | voice overlay/ordering; §37.3, no write-only rule |
| `Angels_Carol…` (mscz, mxl) | missing note, bar 1 | grace note realised by abc2midi; §41.1, oracle uses the written model |
| `Sancta_Maria…` (mscz, mxl) | "pitch" 52→40, bar 48 | the **pitch sequence has no divergence at all** (3 876 and 1 467 notes identical); the events TSV flagged an earlier rest onset, so this is a classification artefact of index alignment |
| `testLocalTimesig1.mscx` | duration 3.0→4.5 | `[M:3/4]` inside a 9/8 measure; `xml2abc.py` writes an empty file ("nothing written … has no notes") for this rest-only score, so it gives no reference; the model's 9/8 length is correct |
| `Laudate_pueri…` (mscz, mxl) | duration 1.0→4.0 | organ part has empty voices; our `%%score 1 2 3 {(4 5 6) (7 8)}` vs `xml2abc.py`'s `%%score [ 1 \\| 2 \\| 3 ] { ( 4 7 8 ) \\| ( 5 6 ) }` — a staff/voice grouping difference, layout-dependent, no rule from the pivot model |

The one writer cause committed with a measured gain is §42.2 (measure-start
key applied before inline clef fields, ABC 2.1 §4.6 spec 921-933,
`xml2abc.py` comparison). §42.3 documents the reader refinement needed to
avoid two losses introduced by an earlier grace fix, with its own reduced
test. No other writer-only rule among the ten first divergences produces a
gain without changing correctly written music.

## 44. ABC import on the 4 756-tune lot (2026-11-23)

Rebuilt on current master with the frozen release binary and the same
`abc2xml.py` 269 alternative (`scripts/oracle/abc/mesure.sh`, all three lots,
1 402 files / 4 756 `X:` tunes):

| Candidate | Exact tunes | N1 correct | N2 correct | Failures |
| --- | ---: | ---: | ---: | ---: |
| scoretk, before | 4 580 | 470 255 | — | 130 |
| scoretk, after the inline-`K:` fix | **4 581** | 470 276 | — | 130 |
| `abc2xml.py` 269, unchanged | 4 517 | 469 976 | — | 130 |

Per-tune comparison: one gain (`parsing.test__abcInlineKeyPerVoice.abc` X:1,
N1 0.5625 → 1.0), no loss. scoretk remains ahead of the alternative by 64
exact tunes.

**Classification of the 176 inexact tunes before the fix.** 145 are
measurement artefacts: 109 `SANS_X` fragments (no `X:` header, not a tune),
21 `ORACLE_ECHEC` (abc2midi fails on the reference) and 15 `ORACLE_VIDE`.
Of the 31 remaining, 21 are `NOTES_MANQUANTES`, 9 `DESACCORD_PARTIEL`,
5 `NOTES_SUPPLEMENTAIRES`, 2 `REPRISE_OU_TRONCATURE`, 2 `TRANSPOSITION`,
1 `OCTAVE` and 1 `DESACCORD_TOTAL`. In 29 of those 31 the alternative has
exactly the same score as scoretk: the reference (abc2midi realising ornaments
or repeats) disagrees with both readers, so they are not importer defects.
Only `parsing.test__abcInlineKeyPerVoice.abc` X:1 (21 missing pitch matches)
and `laPlusDesPlus.abc` X:3 (9) showed the alternative strictly ahead; the
first is fixed here, the second is a 9-note modal-key difference with no
single construct isolated yet.

## 45. Oracle ABC : les 145 artefacts du lot (2026-11-23)

Les trois catégories signalées en §44 étaient des défauts de mesure, pas des
airs impossibles. Causes et corrections dans `scripts/oracle/abc/mesure.py` :

- **`SANS_X` (109)** : `decoupe()` exigeait un `X:` pour reconnaître un air.
  ABC 2.1 §2.2 et §3.1.1 recommandent ce champ mais n'en font pas une
  condition de lecture : abc2midi et abc2xml.py lisent ces fragments comme un
  air unique. L'oracle synthétise désormais `X:1`.
- **`ORACLE_ECHEC` (21)** : abc2midi refuse un air dont le premier `K:` n'a
  pas de tonique (§3.1.14) — 18 fragments n'ont aucun `K:`, 3 ont `K:cm`,
  `K: bass` ou `K: clef=none`. Les deux lecteurs prennent do majeur par
  défaut. `cle_pour_oracle()` n'ajoute `K:C` (en conservant les paramètres de
  clef) qu'à la copie destinée à abc2midi.
- **`ORACLE_VIDE` (15)** : référence *et* candidat n'ont aucune note (airs
  écrits uniquement en silences et en symboles d'accord). `cmp.compare()`
  considère désormais l'égalité vraie par vacuité : ces airs restent dans le
  dénominateur, comme l'exige `plan-fiabilite.md` §1 règle 7.
- Un quatrième défaut, trouvé en confirmant les écarts « référence contre
  les deux lecteurs » : `sans_reprises()` laissait les fins alternatives
  composées (`[1,3`, `[1-3`, `|2,4`, `|3`), si bien qu'abc2midi ne jouait
  qu'une partie des notes **écrites** (ABC 2.1 §4.9-§4.10). Elles sont
  maintenant remplacées par une barre simple.

Autotest : `scripts/oracle/abc/test_artefacts.py`. Mesure contrôlée, **même
binaire** (commit d'import ABC précédent), seul l'oracle change : 4 581 →
**4 712 exact / 4 756**, 472 423 notes de référence (au lieu de 470 566),
aucune perte. L'alternative passe de 4 517 à 4 655 exact.

Les 44 airs encore inexacts : 10 `NOTES_MANQUANTES`, 11 `VIDE` (référence
non vide et lecture vide ; défaut de lecture nouvellement exposé), 9
`DESACCORD_PARTIEL`, 5 `NOTES_SUPPLEMENTAIRES`, 3 `OCTAVE`, 2 `TRANSPOSITION`,
2 `REPRISE_OU_TRONCATURE`, 1 `DESACCORD_TOTAL` et 1 `ECHEC_TK` (`C0` : durée
nulle refusée par le lecteur, alors qu'abc2xml.py l'accepte). Sur un échantillon
de la catégorie « référence contre les deux lecteurs », la référence est bien
en tort : dans `flattener.test__abcRepeat3.abc`, la copie oracle jouée par
abc2midi n'a que 13 notes alors que la source en écrit 17 (quatre fins
alternatives avalées par l'ancien `sans_reprises`) ; la correction la rend
exacte pour les deux candidats. Les jigs `CampMeetingJigNo2` et
`AnnieHughesJig` gardent 2 notes de trop dans la référence : abc2midi y réalise
des ornements que l'oracle ne supprime pas, alors que l'ABC 2.1 §4.12/§4.14
les traite comme des ornements, pas comme des notes écrites — les deux lecteurs
sont d'accord, la référence ne l'est pas.

## 46. Écriture ABC : relevé 2026-11-23 et références vides

Relevé reconstruit sur le master courant (lots corpus 56 / mscz 174 / xml 32,
mode `model`, binaire release, xml2abc.py 177) :
`target/abc-ex-base/` donne **237/261 exact** pour scoretk contre 95 pour
xml2abc.py ; `target/abc-ex-oracle/` (seul l'oracle change) donne **244/261**
et 101 pour xml2abc.py — **7 gains, 0 perte**, sans croissance des `.abc`
(taille moyenne identique : 300 / 12 408 / 12 794 octets).

Cause corrigée : `midi_notes.compare`, partagé avec l'oracle d'import MIDI,
exige `ref > 0` pour déclarer l'exactitude. En écriture, 7 partitions ne
contiennent que des silences et des symboles d'accord : référence et sortie
ont toutes deux zéro note. `ecriture.py::corriger_exact` reconnaît l'égalité
par vacuité, sans exclure ces fichiers du dénominateur
(`plan-fiabilite.md` §1.7). L'autotest `--selftest` couvre les deux cas
(vide/vide exact, vide/non-vide inexact).

Les 17 airs encore inexacts restent comptés. Causes documentées dans les
sections précédentes et confirmées sur ce relevé : 5 `attaque_decalee`
(Byrd_Dies, Perosi, Vivaldi Sicut erat, Anerio, CanonDeLaPaix — 431 à 587
attaces justes sur 447 à 813, avec toutes les hauteurs correctes), 5 `octave`
(instruments transpositeurs), 2 `notes_supp`, 1 `notes_manquantes`,
1 `dessaccord_partiel`, 1 entrée non prise en charge (hors dénominateur).
xml2abc.py 177 reste très en retrait sur les trois lots (95/261) : son
`doNote` (1192) n'écrit les accords que si `<chord/>` est présent, alors que
le MusicXML de référence produit par scoretk ne le porte pas ; les écarts
d'attaque viennent surtout de la gestion des silences invisibles `x` et des
fins de mesure, pas d'une règle unique dont le port améliorerait le relevé
sans dégrader les 244 airs déjà exacts.

## 47. Écriture : changement de clef en fin de mesure (2026-11-23)

Cause réduite de 5 des fichiers `octave` : MuseScore range le segment de
clef au tick de la **barre finale**, donc dans la mesure précédente. Le writer
l'émettait avant la dernière note de cette mesure ; l'aller-retour appliquait
alors `clef=treble-8` une mesure trop tôt et descendait cette note d'un octave.
ABC 2.1 §4.6 ; `xml2abc.py::doAttr` (lignes 1279-1331) applique tout
`<attributes>` avant les notes qu'il modifie, `mkNote` (1154) décale la
hauteur écrite de `-clef-octave-change`.

- `writer.rs::measure_context` lit aussi les segments de la mesure précédente
  au tick `measure.start_tick` ;
- `voice.rs::draw_context_changes` ne réémet pas un changement au tick de la
  barre finale (il appartient à la mesure suivante) ;
- `voice.rs::clef_octave_at` cherche la dernière clé par portée et par tick,
  sur toutes les mesures, comme `s.clefOct[n]` de xml2abc (1329-1331).

Test réduit : `crates/scoretk/tests/abc_export_clef_octave_mesure.rs`
(`clef_octave_en_fin_de_mesure_precedente`), échoue avant, passe après.
Instantanés mis à jour : 3 fichiers (`layout_elements`, `moonlight`,
`copypaste_parts`) — les champs de clef se déplacent au début de la mesure où
ils s'appliquent ; deux champs redondants disparaissent.

Mesure contrôlée, même oracle, même lot, seul le writer change :
**244 → 249 exact / 261** (corpus 54, mscz 165→171, xml 24→25), 0 perte.
Gains : `Veni Creator Spiritus`, `Sancta Maria` (mscz **et** mxl),
`Vivaldi Gloria Patri`, `Vingt danses enfantines`. Octets `.abc` :
2 572 746 → 2 566 704 (aucune croissance). L'alternative reste à 101/261.

## 48. Lecture : décoration longue dans un accord (2026-11-23)

Quatre des 11 airs `VIDE` ont pour cause une décoration longue `!…!`
collée à une note d'accord, réduite de
`flattener.test__abcVolumeInChords.abc` (`[!pppp!c!ffff!D]`). Deux défauts :
`parse_chord` sautait `!` lettre par lettre (le `p` devenait une hauteur et
l'accord était rejeté), et le garde-fou « ligne de prose » de `reader/mod.rs`
refusait la ligne entière parce que `p` n'est pas un caractère de musique.
ABC 2.1 §4.17 autorise explicitement les décorations sur chaque note d'un
accord ; `abc2xml.py::chord_note`/`dec_note` (lignes 383-385) les accepte.

- `music.rs::parse_chord` consomme `!…!` / `+…+` avec `parse_decoration()` ;
- `mod.rs` retire le contenu des décorations longues avant de décider si une
  ligne est de la musique (les textes `N:` restent exclus).

Test réduit `crates/scoretk-abc-import/tests/chord_decoration_longue.rs`
(deux cas, échouent avant, passent après). Mesure contrôlée, même binaire,
seul le lecteur change, même oracle : 4 712 → **4 716 exact / 4 756**,
4 gains (`VolumeInChords`, `abcDirectives`, `abcDirectives2`, `abcGlissando`),
0 perte. L'alternative reste à 4 655.

## 49. Lecture : champ `[V:…]` en tête de ligne sans `K:` (2026-11-23)

Les 5 derniers airs `VIDE` restants sont `parsing.test__abc3..9` X:360 :
`[V:1]f|` sans champ `K:`. Le garde-fou « ligne de prose » (ajouté pour les
lignes `N bars 13 …`) classait la ligne comme du texte à cause du `V` du
champ en ligne, et l'air entier était lu sans note. ABC 2.1 §7.3 place les
champs en ligne dans le corps ; abc2xml.py leur donne l'armure C par défaut.

`reader/mod.rs::music_token` retire désormais les champs `[X:…]` ouvrant la
ligne avant de la classifier. Test réduit
`crates/scoretk-abc-import/tests/inline_field_sans_k.rs` (avec et sans `K:`).
Mesure contrôlée, même binaire, seul le lecteur change : 4 716 →
**4 721 exact / 4 756**, 5 gains, 0 perte ; abc2xml.py reste à 4 655.

## 50. Lecture : durée explicite `0` (2026-11-23)

`note.test__abcZeroLength.abc` (`C0 D1 [EG]0 [FA]1`) était le dernier air
`ECHEC_TK` : le lecteur rejetait la durée `0`. ABC 2.1 §4.3 ne définit pas de
durée nulle, mais l'alternative `abc2xml.py::abcdur` (ligne 502) renvoie une
chaîne vide pour `dur == 0`, c'est-à-dire une note sans multiplicateur, donc
d'une unité `L:` — c'est le comportement adopté (`rhythm.rs::note_length`)
plutôt que de rejeter tout l'air. Test réduit `duree_zero.rs`.

Mesure contrôlée, même oracle, même binaire, seul le lecteur change :
4 720 → **4 721 exact / 4 756**, 1 gain, 0 perte.

Les Causes d'écriture encore ouvertes ont été réduites sans correctif
probant : `Anerio`, `Perosi`, `Byrd_Dies`, `CanonDeLaPaix` et `Vivaldi Sicut
erat` viennent de silences/chevauchements que le modèle pivot porte mais que
le texte ABC ne peut pas représenter sans changer les attaques (par exemple
une note de 1/16 chevauchant une noire entière : `xml2abc.py` écrit un accord
`[AF]8`, notre writer `A8F/2`, et les deux perdent l'attaque à 67,75). Tenter
d'écrire le `ticks` du silence de mesure (`Z2`) a au contraire fait passer
`Anerio` de 431 à 309 notes justes : la mesure réduite a été annulée. Ces
fichiers restent donc comptés et documentés ici plutôt que « corrigés » au
prix d'une régression.
