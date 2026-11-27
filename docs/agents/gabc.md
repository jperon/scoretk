# GABC — reader/writer campaign (phase 6: Gregorio oracle)

The scoretk GABC reader and writer now exist: three portable crates
(`scoretk-gabc-common`, `scoretk-gabc-import`, `scoretk-gabc-export`), wired
into the facade by the default features `gabc-import`/`gabc-export`, the CLI
and `formats()` in the wasm package. Gregorio remains the only reference of
truth. The architectural decision (optional L6 Gregorian layer) is in
[model-modularisation.md](model-modularisation.md) §7.1, step 6 in §8 and the
measured report in §10.7/§11.7. Syntax and mapping are in
[format-ref/gabc.md](format-ref/gabc.md) and
[format-mapping/gabc.md](format-mapping/gabc.md).

## 1. References and licences

All files are local, read-only and unversioned under
`/home/jacques/programmes/musique/specs/`, catalogued in `specs/README.md`
§ GABC / Gregorian chant. Gregorio's built C parser is the only executable
reference; the one third-party script executed as a *landmark* (never
imported, always sandboxed) is described in §12.

| Source | Version | Licence | Use |
| --- | --- | --- | --- |
| Gregorio `doc/Gabc.tex`, `src/gabc/*.y/.l`, `src/dump/dump.c`, `src/struct.c`, `src/struct.h` | `b63f074` ("6.2.0-develop") | GPL-3.0-or-later (`gregorio/COPYING.md`) | **syntax, parser, grammar and reference truth** |
| GregoBase SQL dump | `8851677e`, 2024-01-05 | **no site-wide licence statement** | corpus source, only `copyrighted = 0` rows, private reference |
| Gregorio `doc/`, `examples/` | `b63f074` | GPL-3.0-or-later | nine parsing exercises, copied to `tests/corpus/gabc/` |

GregoBase has no licence grant; `build_lot.py` uses the per-entry
`gregobase_chants.copyrighted` flag as the only filter and keeps the lot
outside the repository. The extracted corpus is private reference data and
must not be redistributed. The nine Gregorio examples are GPL-3.0-or-later
and are versioned (`NOTICE`).

## 2. Lot

`build_lot.py` parses the SQL dump as **data** (`csv` + `json`), normalises
non-copyrighted chants into self-contained `.gabc` files (`name:`, required
by `Gabc.tex:79-82`; `language: latin;`, `:93`; `staff-lines: 4;`, the default
at `:128,434`; optional `mode:`, `:94-102,156-173`) and copies the nine
Gregorio files. With `--count 180 --stride 97` this yields **202 files**
(193 GregoBase + 9 Gregorio) in `specs/gabc/lot/`, with a deterministic
`lot.tsv`.

## 3. Why not MIDI/XML

Gregorio's syntax defines **separation bars**, not metrical measures: no time
signature, no measure numbering and no fixed note durations
(`doc/Gabc.tex:742-845`). The only rhythmic information is neume shape,
punctum mora, episemata and bar weight (`:742-799`, `:801-845`). A comparison
of MIDI pitches/onsets with MusicXML written notes would therefore measure a
duration interpretation that the format does not contain, and could never be
a 100 % sanity test. It was removed as a *duration* comparator. Two other
comparisons are kept: the cross format conversion GABC → MusicXML → GABC
(§9, where only the model inventory is scored) and a first GABC → MIDI
landmark test set scored on **pitch intervals and words only** (§12).

## 4. Corrected oracle: Gregorio is the reference

`scripts/oracle/gabc/canonical.py` defines the truth on what Gregorio's
format actually carries:

1. **pitch sequence with the clef applied** — Gregorio's staff heights
   (`src/struct.h:809-813`), its `o → p` letter mapping (`src/dump/dump.c:78-88`)
   and its clef key `2*line-1` (C) / `2*line-4` (F) (`src/struct.c:1273-1288`),
   giving `step = height - key - 5` with C = 0;
2. **syllable/lyric text**, normalised as Gregorio's dump prints it (braces,
   TeX protrusion commands and repeated `~` removed);
3. **separation bars**, normalised to Gregorio's `B_*` names
   (`src/dump/dump.c:805-821`);
4. **rhythmic signs** (punctum mora, ictus, liquescents) as optional N2.

Neume grouping is a GABC source-level fact (the parenthesised groups,
`Gabc.tex:36-58`); Gregorio dumps layout *glyphs*, a different granularity
(109 glyphs versus 34 source groups on `gb1`), so it is not part of the
dump-based comparison.

Primary reference: Gregorio's own C parser, built by `build_gregorio.sh`
with `nix-shell -p autoconf automake flex bison`, `-j2`, in
`specs/gabc/gregorio-buildtree/`, invoked as `gregorio -f gabc -F dump`
(`src/gregorio-utils.c:184-218`). Secondary: our own reader of the source.
`mesure.py --writer` adds a third, independent measurement: scoretk's
GABC → model → GABC output is fed back to **Gregorio**, and the two dumps
are compared by canonical form and SHA-256. No script from a downloaded
source is executed **in this oracle**; the separate, sandboxed MIDI landmark
of §12 is the sole exception and is never imported. Every divergence is classified by the failing check
and written to `mesure.jsonl`; nothing is silently dropped.

Parallelism is capped at two workers (`--jobs`, default
`min(2, os.cpu_count())`); no other measurement runs concurrently.

## 5. Sanity baseline (2026-11-23, full lot, two workers)

```sh
scripts/oracle/gabc/mesure.sh \
  /home/jacques/programmes/musique/specs/gabc/lot/lot.tsv /tmp/gabc-oracle-greg3 \
  --verifier-determinisme --jobs 2
```

- **Gregorio vs itself: 197/197 = 100 %** (SHA-256 of two dumps per valid
  file). Five files are rejected by Gregorio itself (`gb6036`, `gb8251`,
  `gb9426`, `gb11321`, `gb18361`: "clef change is not supported on the first
  syllable", "syntax error", "unrecognized character u") and are excluded
  from both sides — they are bad corpus data, not converter failures.
- **Our Python source reader vs Gregorio: 132/197 core-exact at the time
  (133/197 with the corrected canonical form of §9).** The 65
  residual files are classified in `mesure.jsonl` by `premier_divergence`:
  pitch 18, text 24, bars 23 (signs-only differences are N2). These are
  defects in the *Python sanity reader*, not in the Rust writer; bracketed
  `[…]` layout tags inside note code, clef changes embedded in a bar group
  and markup normalisation are the main causes (see `canonical.py` and
  `docs/agents/format-ref/gabc.md` §9 pitfalls 5 and 6).

## 6. How to run

See `scripts/oracle/gabc/README.md` § Usage: build the lot, build Gregorio,
run `--selftest`, run the full measurement. With the scoretk writer measured:

```sh
nix develop -c cargo build -p scoretk-cli
scripts/oracle/gabc/mesure.sh \
  /home/jacques/programmes/musique/specs/gabc/lot/lot.tsv /tmp/gabc-oracle-writer \
  --verifier-determinisme --jobs 2 \
  --writer /home/jacques/tmp/scoretk/wt-gabc1/target/debug/scoretk
```

The `--writer` argument is additive: without it the phase-0 baseline is
unchanged. Report gains and losses **per file** as for the other nine
directions (`docs/agents/plan-fiabilite.md`).

## 7. Reader (`scoretk-gabc-import`)

`read_gabc(&str) → Score` is a pure in-memory rewrite of the GABC grammar:

| Rule | Source | Implementation |
| --- | --- | --- |
| Header/notation split at a `%%` line | `Gabc.tex:51,58`; `gabc-score-determination.y:737-745` | `syntax::split_sections` |
| `name:` required, multi-line values `;;` | `:69-82` | `syntax::parse_headers` |
| Lexer state `notes` between `(` and `)` | `gabc-score-determination.l:385-404` | `read::parse_notation` |
| Spaces/EOL are word separators; `%` comments suppress the EOL space | `:60-64,241-245` | `read::parse_notation` (`space_after`) |
| `$` escapes the next character | `:247-250` | `read::parse_notation` |
| Pitch letters `a`–`n`/`p` are staff positions, `o` is the oriscus | `:408-443,469`; `dump.c:78-88` | `syntax::is_pitch`, `staff_height` |
| Clef key C `2*line-1`, F `2*line-4`, two clefs with `@` | `:847-870`; `struct.c:1273-1288` | `syntax::clef_key`, `parse_clefs` |
| One-note and complex neume glyphs, fusion, spacing, shape hints | `:452-700` | `read::parse_group`, `GregorianNote.glyph` |
| Rhythmic signs `.` / `'` / `_` with modifiers and `r1`…`r8` | `:702-797` | `syntax::parse_signs`, `SignKind` |
| Bars, `::` as one finalis, `;1`…`;8`, `:?` | `:801-845`; `dump.c:805-821` | `BarKind::parse_prefix` |
| Custos and line breaks `z`/`Z` are not notes | `:872-906` | `read::parse_group` |
| NABC cuts `|` | `gabc-score-determination.y:870-907` | `read::parse_group` |
| `<sp>`/`<alt>` rubrics, centring and translation markup | `:297-343,345-378` | `Syllable::rubric`, raw `text` |

The L6 `Gregorian` payload owns this primary data. The L0/L1 view is a
**derived projection** (`Gregorian::build_derived_score`, shared by the GABC
and MusicXML readers): one `Note` per *sung* pitch letter, quarter-note
default duration, and **one measure per real separation bar** (`,` `;` `:`
`::`, `Gabc.tex:801-845`), not one per syllable group. Syllables without a bar
accumulate in the open measure, so the modern view has neither a displayed
meter nor invented measure numbers: MusicXML writes
`<time print-object="no"><senza-misura/></time>` (XSD 4.0 `time` /
`senza-misura`). `derived_modern_view = true` and a `derived-modern-view` loss
record make that choice explicit (`model-modularisation.md` §7.1).

**Durations (item 5).** GABC has no fixed duration (`Gabc.tex:801-845`): the
projection uses an equal **quarter note** per sung pitch letter and a punctum
mora `.` (`:742-755`) **doubles** the written figure (half note). A horizontal
episema `_` (`:769-797`) is shown as a **plain quarter with a tenuto**
(`<notations><articulations><tenuto/></articulations></notations>`), as in
gabctk/Gregorio modern renderings, while its played duration stays lengthened
by the documented **factor 1.5** (user decision, 2026-11-25): the written
figure has no `<dot/>`, `<duration>` carries 1.5 quarters, and
`ChordRest::ticks` carries the same performance duration used by MIDI and the
timeline. Mora + episema gives a written half played 1.5 halves. The
separation bars add breaths instead of metric time. The reverse projection
writes `.` from a doubled figure and `_` from a tenuto (also from a dotted
figure coming from third-party MusicXML), so a standard-MusicXML round trip
keeps the durations without inventing an extra dot or glyph. Tests:
`crates/scoretk-core/src/model/gregorian.rs`,
`crates/scoretk/tests/gabc.rs` and `scoretk-gabc-export/src/from_score.rs`.

**Quilisma (user decision, 2026-11-25).** Following gabctk's convention
(`gabctk.py:35,45,1001,1051-1095`; the script is read as text, never imported
outside the sandbox), `w` (`Gabc.tex:470`) is projected as an **ornament**:
MusicXML `<ornaments><inverted-mordent/></ornaments>`, LilyPond `\prall`, ABC
`!uppermordent!` (`P`). On re-import from MusicXML, an `inverted-mordent` on a
note of an unmetered four-line chant is read back as a quilisma `w`. The notes
*before* the quilisma **on the same syllable** are held back progressively:
the immediately preceding one is doubled and the earlier ones get the
decreasing factors `1 + 2^-k`, with named constants
`QUILISMA_HOLD_IMMEDIATE` = 2, `QUILISMA_HOLD_SECOND` = 1.5,
`QUILISMA_HOLD_THIRD` = 1.25, `QUILISMA_HOLD_FOURTH` = 1.125 in
`scoretk-core/src/model/gregorian.rs` (the series continues, capped at 16).
The walk stops at the beginning of the syllable and never enters the previous
one. These holds are **performance durations** (`ChordRest::ticks`, used by the
MIDI writer): the invented attack grid stays one quarter per letter, so a
standard-MusicXML round trip does not shift every derived measure. Tests cover
a syllable with three notes before the quilisma and one where the quilisma is
the second note (`crates/scoretk-core/src/model/gregorian.rs`,
`crates/scoretk/tests/gabc_quilisma.rs`).

**Alterations (item 1).** `x`/`X` (flat), `#`/`##` (sharp), `y`/`Y` (natural)
and their parenthesized forms are *not sung notes*: `Gabc.tex:512-515` calls
them one-note neumes but explicitly says they are not notes; Gregorio's dump
does emit a `GRE_NOTE` with a shape (`src/dump/dump.c:358-455`), but that glyph
is engraving data. The modern projection therefore emits **no** `Note` and no
`<notehead>x</notehead>` for them. It records the alteration per letter and
applies `<alter>` / MIDI pitch to the following sung notes of that letter
until **the end of the word, the next separation bar, or a cancelling
natural**. Source check: `\gresetalterationeffect` (`Gabc.tex:512-540`,
`Command_Index_User.tex:1490-1504`) defaults to `line` and documents `word`
and `bar` as *not yet implemented*; Gregorio does not propagate alterations
into performance at all (`src/struct.c:1290-1410` only uses them for custos
pitch finding). The word/bar scope is thus our declared modern projection,
matching the Cantus consumer and ordinary chant practice, not a claim about
Gregorio's TeX output. The witness `A(gixi)ve(h_g.)` projects two sung notes
(G3 then B♭3 under `c4`), and `gxf!gh` flattens the following `g` only.
The **visible** sign is modelled too: since the glyph is not a sung note,
`Accidental::new` (flat/natural/sharp) is attached to the first subsequent
sung note of that letter, and `x?`/`y?`/`#?` keep `parentheses="yes"`.
MusicXML XSD 4.0 distinguishes `<pitch>/<alter>` (sounding pitch) from
`<accidental>` (notated sign); writing only the former meant Verovio drew no
flat in Cantus. Later notes under the same scope keep `<alter>` but no
displayed accidental; a following `y`/`Y` is a visible natural that cancels
the scope, and word/bar boundaries drop any unasserted sign. Tests:
`crates/scoretk/tests/gabc_accidentals.rs`.

When a word ends **without** a modern barline, the sounding alteration resets
but the sign already shown in the measure remains visible to a modern reader:
`(c4) lé(ixi)i(i) la(i)` must therefore draw a **courtesy natural** on the last
B, or it would read as B-flat. `build_derived_score` tracks the last alteration
displayed per letter *per measure* and adds a courtesy sign whenever the next
sung note's sounding alteration differs from it; a real barline clears the
tracking. The sign is flagged as a user (cautionary) accidental without brackets
or parentheses, exported as `cautionary="yes" parentheses="no"`. It belongs to
the modern projection only: the inverse MusicXML → GABC writer derives glyphs
from `pitch`/`tpc` and never reads `Note::accidental`, so the courtesy cannot
become a GABC `y` glyph (round-trip test in `gabc_accidentals.rs`).

## 8. Writer (`scoretk-gabc-export`)

`write_gabc(&Score) → String` **serialises the structured L6 payload**; it
does not keep or read any raw source text. The `Gregorian` model has no
`header_source`/`notation` field (a test in `scoretk-gabc-export` reads the
model source to keep a reintroduction from silently restoring the
self-comparison). Each emitted construct comes from a parsed field:

| Emitted construct | Structured source | Spec / code |
| --- | --- | --- |
| `name: v;`, multi-line `book: …;;` | `GregorianHeader { name, value }` | `Gabc.tex:67-82,69-74` |
| `%%` marker and section break | generated, not stored | `:51,58` |
| Initial clef `(c3)` | `Gregorian::clefs[0]` | `:847-870`, `struct.c:1273-1288` |
| Clef changed inside a score | `Syllable::clef` (added for this purpose) | `:847-870` |
| Syllable text, `<sp>`/`<alt>` rubrics | `Syllable::text` (rubric is markup inside it) | `:228-343` |
| Pitch letter and its neume | `GregorianNote::letter`, `glyph` | `:408-443,452-700` |
| `.` / `'` / `_` / liquescents | `NeumeSign::raw` when `glyph` is empty | `:702-797` |
| `,` `;` `:` `::` `;1`…`;8` | `BarKind::as_str()` | `:801-845`, `dump.c:805-821` |
| Space or line end after a group | `Syllable::space_after` | `:60-61,241-245` |

**Atomic-token exception.** Three strings are copied because they *are* the
GABC token and have no decomposed representation in the model: (1) a sign's
`raw` (`.1`, `_502`, `r3`, …) — `SignKind` alone loses placement and modifiers
(`:742-797`); (2) one neume's `glyph` — the spelling of a single neume is the
unit Gregorio parses (`:452-700`), and decomposing it would require a new
neume-grammar layer; (3) a clef token (`cb3`, `c1@c4`) — two simultaneous
clefs have no single pivot `Clef` equivalent (`:854-856`). Every one of these
is produced by the reader from parsed characters, never by slicing the source
file. Everything else is generated.

**Inter-group whitespace is kept** (`Gregorian::opening_separator`,
`Syllable::separator_after`, `Gabc.tex:60-61,241-245`): the exact text after
an opening clef-only group and between groups, including the final newline,
is parsed and written back. `Syllable::space_after` still carries the *fact*
of a word break for the derived view and the other directions; the exact
spelling is only used by the GABC writer. `Some("")` means the source had no
separator, `None` means the payload was built without a source body and the
writer falls back to `space_after` (§18).

**What is still lost on a round trip** (and therefore measured honestly):
header comments, bracketed layout/tuning/TeX attributes stripped by
`strip_attributes` (`:689-700,908-931,1093-1118`), custos and line breaks
`z`/`Z` (`:872-906`), zero-width spaces `!` outside a snippet, within-group
ordering of bars versus notes, and line breaks/indentation *inside* a group.
NABC cuts and the slash opening a square snippet after a cut are no longer in
this list (§16/§17); they are structured fields of the payload. The losses
above are not fields of the payload, so a byte comparison would have hidden
them by copying raw text; see §9 for the canonical results.

### Scores without an L6 payload

`write_gabc` no longer refuses a score read from MusicXML, MuseScore or MIDI.
`from_score.rs` builds a plain-chant payload from the pivot:

| Pivot fact | GABC produced | Rule |
| --- | --- | --- |
| `Note::pitch` + `tpc` | position letter under a chosen clef | `Gabc.tex:408-443`, key `2*line-1` / `2*line-4` (`struct.c:1273-1288`); the tpc gives the diatonic letter, MIDI pitch its octave, a mismatch emits the alteration glyph (`x`, `#`, `y`) followed by the sung letter, because the reader treats the glyph itself as non-sung (`:512-540`) |
| clef choice | `c3`, `c4`, `f3`, `f4`, `c2`, `c1`, `f2`, `f1` | first clef keeping all notes inside the staff's letter range (`a`–`i`/`k`/`m`/`n,p`, `:415-443`), tie-break in that order |
| `ChordRest::lyrics` (verse 0) | syllable text | `Single`/`End` get a trailing word space, `Begin`/`Middle` do not (`:241-245,314-317`) |
| `Measure::end_bar_line` | `,` normal, `;`/`::` double/final, `:?` dotted, `:` repeats | separation bars are not metric measures (`:801-845`) |
| `ScoreMetadata` | `name:`, `author:`, `language: latin;`, `staff-lines:` | `:79-93,128` |

**What that projection loses:** all neume shapes and liquescents, rhythmic
signs (the note durations in L0 are the source, not vice versa), melismas'
extent, verse 1+, polyphony (only voice 0 of the staff with most notes),
chord notes beyond the first, rests (silence is not a GABC token), all bar
subtypes beyond the six above, key/meter/time signatures, dynamics, tempo,
instrument and layout. A round trip through this path is *not* expected to be
exact; it is measured in §9 as a cross conversion, not a self-comparison.
`tests/corpus/v3/lyricsline01.mscx` (one staff, four notes, one syllable) is
the versioned test case; `scripts/oracle/gabc/check_from_corpus.sh` checks
that Gregorio's own parser accepts the output.

`Capabilities` declares L0+L1+L2+L3+L6 for both directions, cited from
`Gabc.tex:228-343` (text), `:408-443` (pitch), `:452-870` (neumes, signs,
bars, clefs) and the L6 slot of the model. L4 (no tempo/velocity header,
`:67-154`) and L5 (no page geometry, `:801-845`) are not declared.

## 9. Measured results (2026-11-25, corrected)

**The previous 100 % claims are withdrawn.** They compared the input with
itself because the writer emitted `header_source + notation`. All figures
below are re-measured with the structured writer; the commands are the ones
in §6 and `crates/scoretk/tests/layer_coverage.rs`.

| Measurement | Command | Honest result |
| --- | --- | --- |
| Nine Gregorio examples, GABC → model → GABC | `cargo test -p scoretk --test gabc_corpus` | **0/9 byte-identical** (expected); structured inventory 100 % self-consistency. Gregorio accepts 6/6 of the examples it accepts at all, **6/6 core-exact**, **0/6 dump-identical** in `mesure.jsonl` (3 examples are invalid for Gregorio itself) |
| Layer coverage, direct GABC → model → GABC → model | `cargo test -p scoretk --test layer_coverage -- --nocapture` | essential **100 %**, full support **100 %** on 3 435 items (L1 342, L2 1 033, L3 308, L6 1 752) — this is the *structured-field* round trip, not evidence of a format conversion |
| **Cross** GABC → MusicXML → GABC → model, nine examples | same test, `gabc_cross_round_trip_table` | **before: essential 83.00 %, full support 59.07 %** (L1 45.03 %, L2 95.74 %, L3 95.78 %, L6 33.73 %) → **after: essential 100 %, full support 100 %** (L1 342/342, L2 1 033/1 033, L3 308/308, L6 1 752/1 752, 0 extra), 9/9 outputs still differ from their source |
| **Cross without private `gabc:` fields**, nine examples | `gabc_cross_round_trip_without_private_extensions_is_measured` | **before items 1–9: essential 83.00 %, full support 59.07 % → after items 1–9: 95.97 %, 63.58 % → after quilisma/ABC bars and the standard-carrier gains of §9.1: 94.14 %, 69.21 %** (L1 100 %, L2 94.19 %, L3 74.68 %, L6 50.17 %). Every element whose text starts with `gabc:` is deleted from the written MusicXML before re-reading it, as for the private ABC/LilyPond extensions of `model-modularisation.md` §11.8: only standard MusicXML is scored. This is the number a third-party consumer can reproduce; the 100 % row above still carries the private fields and is unchanged. The remaining gap is structural: neume glyphs (`gregorian-note`), syllable markup (`gregorian-syllable`) and shape signs other than `.`/`_`/`w` have no standard MusicXML field, as checked by `out_of_reach_gabc_facts_have_no_xml_field`; 95 % is not reachable on that row without inventing a new private convention. |
| Gregorio vs itself, full 202-file lot | `mesure.sh … --verifier-determinisme --jobs 2` | 197/197 deterministic (5 files rejected by Gregorio itself) |
| **scoretk writer vs Gregorio dumps, full lot** | `mesure.sh … --writer <cli> --jobs 2` | **before: 193/194 = 99.48 %, 3 writer outputs rejected** → **after: writer core-exact 194/194 = 100 %**, **dump SHA identical 88/194** (was 0/194: dropping the extra newlines between syllables brought the output closer to Gregorio's dump), **0 writer output rejected**, 0 tool crashes; the same five *source* files are rejected by Gregorio itself and excluded from both sides |
| Python source sanity reader (phase 0) | same run, `cmp_source` | 133/197 = 67.51 % (was 132/197; the canonical form now keeps Dominican-bar digits and ignores custos pitches) |
| Wasm package size | `nix build .#wasm`, `result/SIZE` | 2 494 030 raw / 983 191 gzip after the original chant bridge → 2 536 772 / 998 347 after items 1–9 (**+42 742 raw, +15 156 gzip**) → **2 558 465 / 1 006 608** after the quilisma projection, measured on the final tree of this branch (`result/SIZE`, `gzip -9`). The preceding commit (`92ed17f4`) had been reported at 2 557 358 / 1 007 151, so the increment is **+1 107 raw (+0.04 %), −543 gzip**; 679 raw bytes were reclaimed in the GABC code (allocation-free hold factors and clef choice) and the sub-2 % budget holds. The 2 536 772 figure in the earlier report was measured before later commits of the same branch. |

The oracle was re-measured after items 1–9 (2026-11-25) with
`/tmp/gabc-oracle-final`: `files=202 gregorio=197 deterministic=197/197
source_core=133/197 writer_core=194/194 writer_dump_identical=88/194`. The
Python sanity reader is unchanged at 133/197: the gains are in the Rust
writer, measured through Gregorio's own dump.

No existing snapshot or `e2e_corpus` result changed. The direct 100 % is a
consistent re-parse of the same structured fields; only the cross row and
Gregorio's independent dump measure an actual conversion.

### 9.1 What MusicXML 4.0 actually carries for chant

The cross gain comes from **structured fields**, never from copying the `.gabc`
source into a `<words>` blob. The writer emits one XML element per fact and
the reader reconstructs the L6 payload field by field; `no_raw_gabc_source_in_xml`
asserts that no GABC delimiter (`%%`, `(`, `)`) or verbatim group reaches the
XML. The derived L0/L1 spine is rebuilt from the restored payload by
`Gregorian::build_derived_score`, because GABC has no metre or fixed duration
to carry (`Gabc.tex:801-845`).

| GABC fact | MusicXML 4.0 carrier | Citation |
| --- | --- | --- |
| Title, author and remaining headers | `<work>/<work-title>`, `<identification>/<creator>`, then `<other-direction>` for e.g. `mode`, `office-part` | XSD 4.0 `other-direction` (“any direction symbols not yet in the MusicXML format”); `Gabc.tex:67-154` |
| Clef token (`c3`, `cb3`, `c1@c4`) | `<clef>` in the derived view + `gabc:clef` atomic token in `<other-direction>` | `Gabc.tex:847-870`, `src/struct.c:1273-1288`; XSD 4.0 `clef`, `other-direction` |
| Absence of metre and measures per bar | `<time print-object="no"><senza-misura/></time>` and one `<measure>` ending at each separation bar | XSD 4.0 `time` / `senza-misura` (“explicitly indicates that no time signature is present”); `Gabc.tex:801-845` |
| Syllable text and word space | `<lyric>` with `syllabic` for the sung word; `gabc:syllable` in `<other-direction>` for markup and exact text | XSD 4.0 `lyric`, `syllabic`; `Gabc.tex:228-245,314-343` |
| Pitch letter and neume shape | derived `<pitch>` + `gabc:note` in `<notations>/<other-notation>`; the shape fields are private (an alteration is **not** a `<notehead>` and emits no note, §7) | XSD 4.0 `notehead-value`, `other-notation` (“extended representation, though without application interoperability”); `Gabc.tex:408-443,452-540` |
| Source position of a sung pitch | additive suffix `-o{offset}-e{end}` on `id`/`xml:id`, in UTF-16 units from the start of the file | XSD 4.0 `id`/`xml:id`; documented in `timemap.md`; `Gabc.tex:408-443` |
| Optional modern view | `gabcModernClef` on wasm `convert` / `Options::gabc_modern_clef`: G clef with `<clef-octave-change>-1</clef-octave-change>` for a low range, 5 staff lines instead of the C clef/4 lines | XSD 4.0 `clef`, `clef-octave-change`, `staff-lines`; `Gabc.tex:847-870` |
| Stemless neumes and one slur per neume | `<stem>none</stem>` and a `<slur>` pair spanning the sung notes of each parenthesised group | XSD 4.0 `stem-value` (`none`), `slur`; `Gabc.tex:36-58,452-700` |
| Above-staff text `[alt:…]` | `<direction placement="above">/<direction-type>/<words>` with the plain text (no `gabc:` prefix); `gabc:syllable A=` keeps the L6 fact | XSD 4.0 `words`, `placement`; `Gabc.tex:1088-1091` |
| Rhythmic and shape signs | `<notations>/<technical>/<other-technical>` (`gabc:sign`); the horizontal episema also reaches standard MusicXML as `<articulations><tenuto/></articulations>` with an undotted `<type>` and a ×1,5 `<duration>` | XSD 4.0 `other-technical`, `articulations`, `tenuto`; `Gabc.tex:702-797` |
| Separation bars and their exact token | `<barline>/<bar-style>` for the modern view (`,` → `tick`, `;` → `short`, `:` → `light-light`, `::` → `light-heavy`); a `<breath-mark/>` on the last sung note **only** for a bar that calls for one (`,`, `;`, `;n`), never twice on one note, never on a punctum mora, and none before `::` (the final barline replaces it); a note-less bar group (`(,)`) puts the breath on the last sung note **before** it, not the next one — MusicXML draws a note's `<breath-mark>` after that note, so `x(g) (,) do(h)` would otherwise place the comma after `do`, on the wrong side of the divisio — and emits none at all when no sung note precedes (e.g. `(c4) (,) do(h_)`); `gabc:syllable` `b=` fields for the exact tokens `;1`…`;8`, `::`, `:?`, … | XSD 4.0 `barline`, `breath-mark`; `Gabc.tex:742-755` (mora), `:801-845` (minima = quarter bar, minor = half bar, maior = full bar, finalis), `src/dump/dump.c:805-821`; gabctk `gabctk.py:930-936` (`Barre.abc`: minima/minor = `!shortphrase![|]`/`!mediumphrase![|]`, maior = `\|`, finalis = `\|]`) |
| Same bars in ABC | `,` → `!shortphrase![|]`, `;` → `!mediumphrase![|]` (`gabctk.py:932-933`, user decision 2026-11-25), `:` → `\|\|`, `::` → `\|]`; the reverse projection reads the MusicXML bar styles back as `BarKind::DivisioMinima`/`DivisioMinor` | gabctk `Barre.abc`, ABC 2.1 §4.2 phrase decorations; `crates/scoretk/tests/abc_barline_gabc.rs` |

Field values are escaped as `\\`, `\|`, `\l` (`<`), `\g` (`>`), `\n`, `\r`:
the shared XML reader loses `&gt;` in text, so the chant bridge uses its own
reversible escapes rather than changing the writer used for MuseScore 4.7.4
references. Nothing is scored as a gain unless the XML actually carries the
field (`out_of_reach_gabc_facts_have_no_xml_field`).

The 100 % cross row above is therefore **not** a claim about interoperable
MusicXML: it also counts the private `gabc:` fields. A second row strips every
element carrying such a field before re-reading and measures what standard
MusicXML alone transports (83.00 % / 59.07 % before the modern projection of
items 1–9). Both rows are printed by `gabc_cross_round_trip_table`; the stripped
row is the honest target for a consumer that knows nothing about scoretk.
`no_raw_gabc_source_in_xml` still forbids copying the `.gabc` source into a
`<words>` blob: the standard-MusicXML gain must come from `<alter>`,
`<syllabic>`, `<bar-style>`, `<stem>`, `<slur>`, `<words>` and durations.

### Why the numbers moved

Two genuine reader defects and one oracle defect were found by the per-file
report (`mesure.jsonl`, never by the total):

- Gregorio's lexer accepts `[,;][1-8]` as Dominican bars
  (`gabc-notes-determination.l:1246-1248`) although `Gabc.tex:818-820`
  documents only `;n`. The reader now emits `;n` for both spellings, and the
  canonical dump parser keeps the digit in `B_DIVISIO_MINOR_Dn`
  (`struct.h:158`): `gb7950`, `gb8150` fixed.
- `<v>…</v>` is verbatim TeX: a literal `(` inside it is not a note group
  (`Gabc.tex:297-309,1093-1102`). The reader now copies the whole tag as
  syllable text: `gb7851` fixed (24 notes and 3 syllables had been swallowed).
- The derived L0/L1 projection stored segment ticks **relative** to their
  measure and dropped syllable text. Both were real bugs: a GABC→MIDI
  conversion stacked every syllable at the same onset and lost all words.
  With absolute ticks (`pass2.rs:495-518` convention) and the first lyric on
  each neume, the cross row moved from 30.60 %/31.54 % to
  **83.00 %/59.07 %**. That intermediate L1 loss was a transport gap, not a
  by-design one: it disappeared once the L6 payload crossed MusicXML (below).
- The standard-MusicXML-only row gained 5.63 points from three honest
  carriers: the original C/F clef is read back from `<clef>` instead of being
  re-guessed from the tessiture (`from_score::score_gabc_clef`), `;` comes back
  as `BarKind::DivisioMinor` from the `short` bar style, and the quilisma's
  `<inverted-mordent/>` is restored as `w`. The progressive holds deliberately
  stay out of `<duration>` (they are performance ticks), so the derived attack
  grid and the L1 row are unchanged.
- The remaining cross gap was the *transport*, not the projection: MusicXML
  was writing only the derived modern view, so every neume letter, glyph,
  sign, bar token and markup string was rebuilt from pitches and lost its
  GABC identity. Writing those fields as the structured extensions of §9.1
  and restoring them in the MusicXML reader raised the honest cross row from
  83.00 %/59.07 % to **100 %/100 %**. This is not a raw-string copy: all nine
  outputs still differ from their sources and `no_raw_gabc_source_in_xml`
  forbids GABC delimiters in the XML.
- A custos has a `pitch` line in Gregorio's dump but is not a sung note
  (`Gabc.tex:872-884`); it shifted the canonical sequence at clef changes.
  `parse_dump` now skips `GRE_CUSTOS` pitches. The reader also no longer
  mistakes `[a-np]+` (`g+`, `f+`) for a note: `gb98`, `gb1456`, `gb1553`,
  `gb1747` fixed.

## 10. Limits

- GABC has no metre, no measure numbering and no fixed durations
  (`Gabc.tex:801-845`): L0/L1 notes and measures are a derived projection,
  not comparable to MuseScore's metric spine. The cross row now reaches
  100 % because the derived view is **rebuilt from the restored L6 payload**,
  not because a metre was invented in MusicXML: that score must still not be
  read as MuseScore compliance.
- The L6 payload does not model neume grouping order inside a group, `<v>`
  and `[…]` TeX attributes, custos/line breaks, NABC cuts or word-level
  typography beyond the raw syllable text. The writer therefore cannot emit
  them, and the dumps differ byte for byte even when the canonical musical
  facts agree (0/194 identical dumps). **These are declared out of reach, not
  silent gaps**: custos and line breaks are layout, not sung notes
  (`Gabc.tex:872-906`; `canonical.py` also skips `GRE_CUSTOS`), bracketed
  layout/tuning/TeX attributes are not grammatical notes
  (`:689-700,908-931,1093-1118`), NABC cuts belong to the NABC extension
  (`gabc-score-determination.y:870-907`), and the ordering of bars inside a
  group is not represented by `Syllable`. The test
  `out_of_reach_gabc_facts_have_no_xml_field` checks that no MusicXML field
  pretends to carry them; `custos_line_break_and_nabc_cut_are_not_notes`
  checks that they never become notes. `gb18458` was fixed at the reader by
  keeping the primitive letters of `@[…]` fusion groups
  (`Gabc.tex:603-666`), not by changing the oracle: 194/194 writer outputs are
  now core-exact.
- The derived concert pitch uses the first clef only; later clef changes
  stay in the L6 `clefs`/`Syllable::clef` fields but are not re-applied to L0
  notes (`Gabc.tex:847-870`; `gabc-score-determination.c:89-100`).
- Export without an L6 payload now projects concert pitch, first-voice
  lyrics and bar lines into GABC; neume shapes, rhythmic signs and polyphony
  cannot be recovered from that data and are lost (§8).
- The Python `canonical.parse_source` sanity reader is at 133/197; improving
  it is independent of the Rust writer score (the writer is measured through
  Gregorio's own dump of its output).
- Five lot files are invalid for Gregorio itself and are excluded from both
  sides (§5).

## 11. Upgrading the Gregorio sources

The normative files are the local, read-only copy at commit `b63f074`.
To upgrade:

1. record the new commit and licence in `specs/README.md` and `NOTICE`;
2. rebuild the parser with `scripts/oracle/gabc/build_gregorio.sh` and
   re-run `--selftest` (determinism is blocking);
3. re-run the full lot with and without `--writer` at two workers and
   compare **per file** (`mesure.jsonl`), not only the total;
4. if the grammar or dump vocabulary changes (`BarKind`, `SignKind`, clef
   key), add the rule with its new `Gabc.tex` line to
   `docs/agents/format-ref/gabc.md`, a unit test in `scoretk-gabc-common`
   or `scoretk-gabc-import`, and update §7/§8 of this page;
5. when the dump vocabulary changes, re-check `canonical.py`'s two reader
   rules (Dominican-bar digits with `[A-Z0-9_]`, and `GRE_CUSTOS` pitches,
   which are layout, not notes) and its `--selftest` assertions;
6. never regenerate the lot from GregoBase with another filter or version
   without an explicit decision (§1).

## 12. GABC → MIDI landmark (first test set)

`scripts/oracle/gabc/midi_refs.py` and `midi_compare.py` add a **first** MIDI
test set. The external tool at
`/home/jacques/programmes/musique/specs/gabc/gabctk/gabctk.py` is downloaded
data: it is **executed, never imported**, with `python3 -I`, inside
`bwrap --unshare-all --die-with-parent --ro-bind / / --dev /dev --proc /proc
--tmpfs /tmp --bind <scratch> <scratch>`, `HOME`/`XDG_CACHE_HOME` in that one
writable directory, `timeout 30`, `prlimit --as=2000000000`, and at most two
workers (same pattern as `scripts/oracle/lilypond/references.py`). It is an
imperfect landmark, **not** the truth: Gregorio's dump and `Gabc.tex` decide
every disagreement.

```sh
python3 scripts/oracle/gabc/midi_refs.py --manifest <lot.tsv> \
  --out /tmp/gabc-midi-work/refs --scratch /tmp/gabc-midi-work/scratch \
  --jobs 2 --timeout 30
python3 scripts/oracle/gabc/midi_compare.py --selftest
python3 scripts/oracle/gabc/midi_compare.py --manifest <lot.tsv> \
  --refs /tmp/gabc-midi-work/refs --cli target/debug/scoretk \
  --scratch /tmp/gabc-midi-work/cmp --jobs 2 --timeout 30
```

Scored on the 202-file lot (2026-11-25):

| Measurement | Result |
| --- | --- |
| Landmark MIDI references produced | **153/202**; 49 failures recorded in `status.jsonl` (23 `IndexError`, 11 `UnicodeEncodeError`, `KeyError` on GABC signs, …) |
| Comparable files | 153/153 parsed by both SMF readers (`midi_compare.py --selftest` green) |
| Absolute pitches identical | 0/153 — not meaningful: the landmark auto-transposes by `66 − mean(tessiture)` (`gabctk.py:485-490`); GABC has no absolute concert pitch (`Gabc.tex:410-413`) |
| **Consecutive pitch intervals identical** | **96/153 = 62.75 %** |
| Words identical after markup normalisation | **110/153 = 71.90 %** |
| Files where Gregorio, scoretk and the landmark agree on note count | **96**; on those 96, intervals **96/96 = 100 %** and words **72/96 = 75 %** |
| Landmark note counts disagree while Gregorio and scoretk agree | **52** files → landmark defect, scoretk not changed |
| Both differ | 5 files (`gb292`, `gb6426`, `gb14155`, and the fusion files; `gb8251` is invalid for Gregorio) |
| Durations identical | 0/153 — **informational only**, never scored (see below) |

### Landmark defects (evidence, no change to scoretk)

1. **Dropped notes.** On 52 files the landmark emits fewer notes than both
   Gregorio's dump and scoretk (e.g. `gb98`: Gregorio 256, scoretk 256,
   landmark 248; `gb389`: 193/193/192). GABC assigns one note per pitch
   letter (`Gabc.tex:408-443`) and Gregorio's dump is the reference; the
   missing notes are a parser defect of the landmark, so scoretk is not
   bent to match it. Full evidence: `comparaison.jsonl` cross-referenced with
   `scripts/oracle/gabc/mesure.jsonl`.
2. **Typography counted as lyrics.** It keeps centring braces (`{ia}`,
   `Gabc.tex:319-335`) and `<sp>` special characters (`v/`, `:297-309,
   345-378`) in text metas (`gb1`, `gb1747`); Gregorio records style
   boundaries separately (`src/dump/dump.c:41-66`) and the canonical form
   strips them. scoretk's derived lyric drops that markup by design.
3. **Invented durations.** The landmark's constants are `DUREE_EPISEME=1.7`,
   `DUREE_POINT=2.3`, `DUREE_AVANT_QUILISMA=2`, default `1`, truncated by
   `duree = int(note.duree)` and accumulated as `temps += duree/2`
   (`gabctk.py:32-36,1017-1065,1443-1462`). scoretk's derived view uses one
   quarter per pitch letter. GABC has no fixed duration at all
   (`Gabc.tex:801-845`), so no duration score is claimed on either side.
4. **Auto-transposition** (`gabctk.py:485-490`) and latin-1 text metas make
   absolute pitch/byte comparisons meaningless; the comparison uses
   transposition-invariant intervals and normalised words instead.

The remaining 5 "both differ" files are the known L6 fusion gap (`@[…]`,
`Gabc.tex:603-666`) plus one file Gregorio itself rejects; they are reported,
not hidden.

## 13. GABC → MEI → GABC (module NEUMES 5.1, mesuré à nouveau le 2026-11-26)

Une troisième conversion croisée est mesurée comme les lignes directe et
« cross » de `model-modularisation.md` §11.7, sur les mêmes neuf exemples
Gregorio et le même inventaire `(layer, tick, staff, kind, value)`. Avant
l'ajout du module Neumes, le writer MEI ne voyait que la vue L0/L1 dérivée :
le payload L6 ne traversait pas le format.

| Conversion | Essential | Plein support | L6 | Source |
| --- | ---: | ---: | ---: | --- |
| GABC → MEI → GABC, **avant** | 94.14 % | 66.35 % | 839/1 752 (47.89 %) | MEI CMN `5.1+basic`, payload reconstruit depuis la hauteur et les paroles |
| Étape intermédiaire (module neumes, porteurs standard absents) | 27.54 % | 49.37 % | 1 273/1 752 (72.66 %) | structure neumatique lisible, glyphes et signes non reconstruits |
| **GABC → MEI → GABC, sans `gabc-*` (chiffre retenu)** | **100.00 %** | **100.00 %** | **1 752/1 752** | MEI 5.1 `mei-all`, module Neumes seul : `<neume>`, `<nc>`, `<signifLet>`, `<episema>`, `<divLine>`, `<clef>`, `<meiHead>` standard |
| GABC → MEI → GABC, avec extension privée (secondaire) | 100.00 % | 100.00 % | 1 752/1 752 | même couverture, avec `<annot type="gabc-*">` redondants, désactivés par défaut (`MeiOptions::gabc_tokens`) |

Le détail des porteurs et les citations des *Guidelines* ch. 6 sont dans
[`mei.md`](mei.md) §10 ; le test de structure est
`crates/scoretk/tests/gabc_mei_neumes.rs` et la ligne de mesure est
`crates/scoretk/tests/layer_coverage.rs` (`gabc_mei_round_trip_table`), qui
imprime le chiffre standard **en premier**. Les neuf sorties MEI sont
validées contre `mei-all-5.1.rng` par `scripts/check-mei-neumes.sh` (`jing`).

Ce que MEI 5.1 n'a pas (table complète par genre dans `mei.md` §10.3) :
la clé double `c1@c4`, les décalages UTF-16 de la source et les faits déjà
absents du payload L6 (custos, coupures NABC, attributs `[…]`, sauts de
ligne). Les signes sans élément dédié — punctum mora, virge, cavum,
altération non chantée, chiffres de placement — passent par `<signifLet>`
(`Guidelines` §6.4.8–§6.4.9) ; les barres dominicaines par un NMTOKEN de
`<divLine @form>` (RNG `att.divLine.log/@form`) ; la clé basse `cbN` par
`@type="clef-flat"` ; l'espace de mot et l'élision par `<syl @wordpos @con>`.

## 14. GABC → every destination: measurement and carriers (2026-11-26)

### 14.1 Instrument and definitions

`crates/scoretk/tests/gabc_cross_formats.rs` converts the nine versioned
Gregorio examples to each destination and reads the written file back. The
reference is the **derived modern view** of the GABC score (§7), reduced to the
ten facts the task compares: `pitch`, `duration`, `lyric`, `bar`, `clef`,
`accidental`, `breath`, `episema`, `quilisma`, `title`. A fact is a key
`(kind, tick, value)` compared as a multiset.

- **essential** = `pitch` + `bar` (the `note` + `measure` analogue of §11);
- **full support** = the kinds the destination's **specification** can carry;
  the declaration and its citations live in `DESTINATIONS` in the test;
- ticks are projected onto the destination's timeline: MusicXML `<duration>`,
  MuseScore `actualTicks()` and SMF delta times advance in **played** time,
  while LilyPond and ABC note lengths advance in **written** time. Breaths are
  compared by kind rather than anchor: MusicXML puts the comma on a note
  (XSD `note`), MuseScore on a dedicated segment;
- `duration` contains only the components the destination can carry: written
  figure, played ticks, or both. The performance caesuras inserted by the
  projection at `:`/`::` bars are not notated breaths (`has_breath`,
  `Gabc.tex:812-820`) and are not counted as `breath` facts.

The test never fails on a percentage: it prints the table and panics only on a
broken corpus or read.

### 14.2 Results per destination

"Before" is the first measurement of the campaign (instrument in place, before
the fixes below). The definition of `breath` was refined during the campaign
(performance caesuras excluded): the reference totals went from 2 524 to 2 489
items, but both columns are honest measurements of the same corpus.

| Destination | Essential before | Full before | Essential after | Full after |
| --- | ---: | ---: | ---: | ---: |
| musicxml (standard) | 30.49 % | 40.17 % | **100.00 %** | **100.00 %** |
| mxl (standard) | 30.49 % | 40.17 % | **100.00 %** | **100.00 %** |
| mscx | 65.66 % | 68.46 % | **100.00 %** | **100.00 %** |
| mscz | 65.66 % | 68.46 % | **100.00 %** | **100.00 %** |
| lilypond | 23.88 % | 30.39 % | **100.00 %** | **99.96 %** |
| abc | 23.60 % | 30.86 % | **100.00 %** | **99.96 %** |
| midi | 17.45 % | 28.03 % | **94.40 %** | **95.53 %** |
| musicxml (private `gabc:` fields) | — | — | 100.00 % | 100.00 % |
| midi (`smf_text`) | — | — | 94.40 % | 96.30 % |

The last two rows are **additive**: the first shows what the private fields
restore (L6 → derived view), the second enables the standard SMF text metas
`FF 03`/`FF 06`. Neither is the default measurement. Lilypond moved from
95.59 % / 90.72 % to 100 % / 99.96 % by carrying the two facts the review
rejected as "inherent": `lyric` (182 losses → 0) and `bar` (48 → 0).

### 14.3 Every residual: carrier or format-inherent loss

No kind was removed from a declaration to raise a figure. The SMF declaration
was only **renamed** (`ABSENT_FROM_SMF` → `DECLARED_BY_SMF`): it has always
listed the kinds SMF declares, not the ones it lacks.

| Destination | Residual kind | Carrier in the specification | Decision |
| --- | --- | --- | --- |
| musicxml / mxl | none | `<type>`/`<duration>` (`format-ref/musicxml.md` §Units and time encoding), `<accidental>`, `<syllabic>`, `<bar-style>`, `<breath-mark>`, `<tenuto>`, `<inverted-mordent>`, `<clef>`, `<work>` | fixed: `<duration>` distinct from `<type>` kept in `ticks`; the writer advances its cursor by the written figure; `<breath-mark>` wrapped in `<articulations>` |
| mscx / mscz | none | `<durationType>`/`<dots>`/`<duration>`, `<Breath>`, `<Articulation>`, `<Ornament>`, `<Lyrics>`, `<Clef>`, `metaTag` (`format-ref/musescore.md` §Section index; `mscx-format.md` §3, §5, §7, §10, §13, §18) | fixed: `fillGap` no longer replaces an existing chord; breaths, tenuti and ornaments written and read back |
| lilypond | `lyric` 0 (was 182) | `--` and `_` with `\lyricsto` (`format-ref/lilypond.md` §Repeats and lyrics; Notation Reference §2.1.2 "Lyrics hyphens", "Multiple syllables to one note") | fixed, and **not** an inherent loss: a syllable followed by `--` begins or continues a word, one preceded by `--` and not followed ends it; `__` is a melisma. The writer now emits `--` for `Begin`/`Middle` whenever a later syllable exists in the verse (including across `\skip` for melismas), and the reader keeps the hyphen state across `\skip` and maps the chain to `Begin`/`Middle`/`End`/`Single` |
| lilypond | `bar` 0 (was 48) | `\bar "'"` (tick) and `\bar ","` (short), Notation Reference §1.2.5 "Bar lines": "Ticks and short bar lines are also available" | fixed, and **not** an inherent loss: those are standard glyphs; `\defineBarLine` is unnecessary. The comma breath is now taken from the model's `SegmentItem::Breath(Comma)` instead of being inferred from the bar style (`Tick` also covers bars without a breath) |
| lilypond | `accidental` 1 | accidentals driven by measure state (Notation Reference §1.3.5) | format-inherent: a sign already active in the measure is not repeated; the sounding pitch stays exact |
| abc | `accidental` 1 | `_`/`=`/`^` and per-measure state (`format-ref/abc.md` §4.2) | format-inherent, same cause |
| abc | none other | `L:`/`A3`, `!shortphrase!`, `!mediumphrase!`, `!tenuto!`, `P` (`format-ref/abc.md` §4.3, §4.14, §5) | fixed: written figures, phrase marks → `Tick`/`Short` and breath, `[|]` bars read back |
| midi | `clef`, `bar`, `accidental`, `breath`, `episema`, `quilisma` | no SMF object (`format-ref/midi.md` §Container layout, §Meta-event types, §Instruments parts repeats) | format-inherent and excluded from `DECLARED_BY_SMF`: an `MTrk` has no clef, barline, notated sign or ornament object |
| midi | `pitch` (no missing, 4 extra in `FactusEst`) | Note On/Off (§Events and time) | declared and matched; the extras are **realised ornaments**: a quilisma is a played `InvertedMordent`, whose short-trill excursion adds grace notes (`crates/scoretk-midi-export/src/render.rs`, `ornamentShortTrill`). SMF carries the sound, not the sign; no sung pitch is lost |
| midi | `duration` 86 | Note On/Off + `FF 51` (§Events and time, §Meta-event types) and the importer's quantisation (`docs/agents/midi.md` §Measured limits) | declared but not exact: the renderer writes `NoteOff = NoteOn + duration − 1` and MuseScore's importer quantises to the 1/16 grid. For example the six 3/8 (dotted-quarter) played values of `FactusEst` come back as 1/2, 3/16, 1/32 or 1/4; this is a renderer/importer convention, not a missing note |
| midi | `lyric` 1 | RP-017 `FF 05`, one syllable per meta (`format-ref/midi.md` §Lyrics) | declared and text-only compared (SMF has no begin/middle/end marker). The single loss is `den-` in `gregorio_factus.gabc`: RP-017 reserves a trailing `-` for word continuation, so `parse_rp017` strips it and the reader sees `den` |
| midi | `title` 18 | `FF 03` sequence name, `FF 01` texts (§Meta-event types) | declared and carried, but **opt-in**: `MidiOptions::smf_text` is `false` by default so the default MIDI bytes stay MuseScore-compatible. The additive `midi (smf_text)` row measures 96.30 % and has no `title` residual; this is a writer default, not an SMF limitation |

### 14.4 Neume carriers: proposal and measurement

| Destination | Possible carrier | Measurement / decision |
| --- | --- | --- |
| MusicXML 4.0 | `<other-notation type="single" smufl="chant…"/>`: XSD 4.0 `complexType other-notation`, `smufl` attribute for application interoperability; official SMuFL names (`chantPunctum`, `chantQuilisma`, `chantEpisema`, `chantIctusAbove`, `chantOriscusAscending`, `chantPunctumVirga`, `chantDeminutumLower`, `chantPunctumInclinatum`) | implemented, opt-in (`Options::gabc_neume_smufl`); `musicxml_smufl_neume_carrier_is_opt_in` measures **1028/1028 glyphs**, 1–8 families per chant, no `gabc:` field added, default output unchanged |
| MuseScore `.mscx` | no documented slot for arbitrary data: the format is a closed grammar (`format-ref/musescore.md` "routing layer", `mscx-format.md` §2, §17) | not implemented: an unknown element in `<Chord>` is not a contract and may be dropped by MuseScore. Non-musical facts stay in the private MusicXML fields when the consumer is scoretk |
| LilyPond | `GregorianTranscriptionStaff`, `\germ`, vaticana (Notation Reference §17.4 "Typesetting Gregorian chant") | proposed, not implemented: an optional dialect standard readers would not reconstruct. Chant stays in `\fixed` with `--`, `\prall`, `\breathe` and standard `\bar` glyphs (§14.3) |

### 14.5 What was fixed in the writers and readers

- MusicXML read: a `<duration>` distinct from `<type>` is kept in
  `ChordRest::ticks` (XSD 4.0 allows the mismatch);
- MusicXML write: the file cursor advances by the written figure so `<backup>`
  computed against a reader following `<type>` does not shift attacks;
  `<breath-mark>` inside `<articulations>`;
- core `gap_rests::fill`: `add_gap_rest` never replaces an existing chord;
- LilyPond write: written figures for chant (no default `*n/d`), `--` for word
  continuation across gaps, `\bar "'"`/`,` for tick/short bars, and `\breathe`
  only for a model `Comma` breath; read: `\bar` glyphs and the hyphen chain
  reconstruct `syllabic`;
- ABC write: written `L:` figures; read: `!shortphrase!`/`!mediumphrase!` →
  `Tick`/`Short` bar and breath, including on `[|]`;
- MIDI: comparison without a written tick (SMF has none) and the additive
  `smf_text` row.

## 15. Headerless bodies (raw GregoBase exports)

Raw exports from GregoBase are the notation half of a `.gabc` file: a clef
group followed by syllables and note groups, with **no header and no `%%`
line**, e.g.

```gabc
(c4) Ky(f)ri(g)e(h) e(g)lé(f)i(e)son.(f) (::)
```

exsurge renders those bodies directly. Gregorio itself does **not** accept
them. Its lexer only leaves the `INITIAL` state and enters `score` on
`<INITIAL>%%+(\n|\r)+`, which returns `END_OF_DEFINITIONS`
(`src/gabc/gabc-score-determination.l:225-228`); the grammar requires that
token between the definitions and the syllables (`score: all_definitions
syllables`, `all_definitions: definitions END_OF_DEFINITIONS`,
`gabc-score-determination.y:737-749`). The two-section file structure is also
stated in `Gabc.tex:51,58` and [format-ref/gabc.md](format-ref/gabc.md) §1.
Running the local oracle binary (commit `b63f074`, built by
`scripts/oracle/gabc/build_gregorio.sh`) on such a body gives:

```
error:unrecognized character: "(" in definition part
error:unrecognized character: ")" in definition part
error:unrecognized character: " " in definition part
error:syntax error
```

and exit status 1; prefixing the same body with `name: Test;` and `%%` makes
it parse and dump successfully.

**scoretk tolerance.** `read_gabc` accepts a body with no `%%` when, after
leading whitespace and whole-line `%` comments, the text starts with a clef
group (`(c4)`, `(f3)`, `(cb3)`, `(c1@c4)`, …) or with a syllable immediately
followed by a parenthesised note group (`Ky(f)ri(g)e(h)`). The check is the
shared `looks_like_gabc_body` helper in `scoretk-gabc-common::syntax`: both
the reader and facade `detect` call it, so recognition cannot diverge. The
body is parsed with an empty header: four staff lines (the default,
`Gabc.tex:128,434`), no metadata and note source offsets counted from the
first character of the body (no synthetic `%%` is prepended). Since §17, a
`|` cut in such a body also implies the single `nabc-lines: 1;` voice that
GregoBase declares when no header says otherwise.

This is a **tolerance beyond Gregorio**, chosen for raw GregoBase data; it
does not change the GABC grammar, the oracle or any file that contains a `%%`
line, which takes the original path unchanged. The structured writer keeps a
headerless payload headerless: it emits neither `name: ;` nor a `%%` marker
(§17), so a body-only round trip does not gain a header. Inter-group layout
whitespace (space after the clef, word spaces, final newline) is now kept
verbatim in the payload and written back (§8, §18). Other formats
keep their stronger signatures and are tested before the body rule in
`Format::detect` (`\version` → LilyPond, `%abc`/`X:` → ABC, XML namespaces →
MusicXML/MEI); a header without `%%` is still neither detected nor readable.

Tests: `crates/scoretk-gabc-import/tests/body.rs`,
`crates/scoretk/tests/gabc_body.rs`, and the negative cases in
`crates/scoretk/tests/formats.rs`.

## 16. NABC manuscript snippets (2026-11-26, branch `meinabc`)

NABC is the manuscript voice of a syllable: with `nabc-lines: 1`
(`Gabc.tex:129-130`), a group alternates square and manuscript snippets
`(square|nabc|square|nabc)`; the `|` cut cycles a state modulo
`nabc_lines + 1` (`gabc-score-determination.y:870-907`). The glyph alphabet
(St. Gall and Laon, `GregorioNabcRef.tex:207-240,402-420`) includes `vi`,
`pu`, `ta`, `gr`, `cl`, `pe`, `po`, `to`, `ci`, `sc`, `pf`, `sf`, `tr`, `st`,
`ds`, `ts`, `tg`, `bv`, `tv`, `pr`, `pi`, `vs`, `or`, `sa`, `pq`, `ql`, `qi`,
`pt`, `ni`, plus Laon's `un`/`oc`. A descriptor also carries modifiers
(`S`, `G`, `M`, `-`, `>`, `~`), subpunctis/prepunctis (`su`/`pp`), `hX` pitch
descriptors and significant letters (`ls…`, `lt…`).

**Model.** The primary payload now stores `Syllable::nabc: Vec<NabcSnippet>`
(`crates/scoretk-core/src/model/gregorian.rs`), one entry per cut with the
exact snippet text and `notes_before`, the number of square notes preceding
it. The `nabc-lines` header stays in `Gregorian::headers` as before. The
reader no longer mistakes manuscript letters for square pitches: only
state-0 segments are parsed as notes; square snippets after a later cut are
still notes, and the cut back to square state is re-emitted on write. The
writer reproduces the interleaving and keeps every snippet byte-for-byte.
This closes a real defect: previously `(e|talsi8)` became `(ealsi8)`, i.e.
the NABC glyph letters were silently sung as square notes.

**MEI.** `mei_full_support` writes the snippets as real neumes in a second,
parallel, unpitched staff, with the exact text retained in
`<signifLet type="gabc-nabc">`; the full mapping table (one citation per
row) is in [`format-mapping/mei.md`](format-mapping/mei.md) and the measured
campaign in [`mei.md`](mei.md) §13. The 56 `.mscx` exports remain byte-identical
to `master`; the GABC exports now carry the exact body layout and the cut
resumption in standard `@label` attributes (§19), so they are no longer
byte-identical to `master`. Since §16 the three NABC exports also keep their
manuscript snippets instead of singing their glyph letters as square notes.

**Measured on the three versioned NABC examples** (`cargo test -p scoretk
--test gabc_nabc`, `scripts/check-mei-nabc.sh`): 144/144 snippets cross MEI
byte-for-byte; GABC → MEI → GABC equals today's direct GABC → model →
GABC bytes on all 3 files; 3/3 MEI outputs are valid against
`mei-all-5.1.rng` and pass its embedded Schematron with **0 errors and 0
warnings**; 178/193 (92.23 %) descriptors are structural neumes and
15/193 (7.77 %) are text-only (modifier variants and spacing with no MEI
equivalent). The non-NABC corpus outputs are untouched.

The fix was measured against `master` (`87fd3b85`) with both CLI binaries.
GABC → GABC now reproduces the source body exactly on 2/3 files and loses
only one pre-existing layout bar (`veni`, a `,` between two cuts) on the
third; semantically all 3/3 round trips match the source after source-offset
normalisation (before: 0/3). The sung-note count drops from 281→116,
158→61 and 312→176 in MusicXML, i.e. exactly the NABC glyph letters that
`master` sang. With the manuscript snippets removed from the input, the two
binaries give byte-identical output in all 15 direct conversions (3 files ×
MusicXML/MIDI/LilyPond/ABC/MEI), and the branch's full-source output equals
its square-only output in all 15 (MusicXML modulo source offsets). No
direction regressed. Per-file numbers and the MEI staff declaration are in
[`mei.md`](mei.md) §13.3 and §12.3.1.

## 17. Implicit `nabc-lines` in headerless bodies (2026-11-26, branch `nabcauto`)

Raw GregoBase exports have **no header at all** (§15), but their notation
still contains the `|` cuts of the manuscript voice. GregoBase itself
prepends `nabc-lines: 1;\n%%\n` to the body before invoking `gregorio`
(`specs/gabc/gregobase/chant_img.php:48`, function `gregorio()`), and the GABC
reference documents the alternating square/manuscript snippets and the
`nabc-lines: x;` declaration that authorises them
(`specs/gabc/gregorio/doc/GregorioNabcRef.tex:153-170`). scoretk therefore
infers the missing declaration from the body itself.

**What Gregorio does with `|` at zero lines.** `nabc_lines` starts at 0
(`gabc-score-determination.y:649`); both grammar rules containing `NABC_CUT`,
`NOTES NABC_CUT` and `NABC_CUT`, call `gregorio_message(... VERBOSITY_FATAL)`
when it is still 0, with "You used character \"|\" in gabc without setting
\"nabc-lines\" parameter. Please set it in your gabc header."
(`gabc-score-determination.y:870-907`). When the header is present, each cut
advances the state modulo `nabc_lines+1` (`:879,906`): state 0 is square
notation, states 1…n are the parallel manuscript snippets. With the **one**
line that GregoBase declares, the state simply alternates square/manuscript
at every cut: segments 0, 2, 4, … are square, segments 1, 3, 5, … are NABC.
Inferring a count from the number of cuts in a group would be wrong: with
seven declared voices consecutive cuts stay in NABC state and the square
run between them would be lost.

**Detection.** `scoretk-gabc-import/src/nabc.rs` returns **1 if a `|` cut
occurs in a parenthesised notation group, 0 otherwise**. The count of cuts
does not matter: GregoBase prepends exactly `nabc-lines: 1;` before rendering
(`chant_img.php:48`), so a headerless export is read with that one voice and
the alternating pattern above. It is called from `read::parse_parts` only
when no `nabc-lines` header is present; an explicit header always wins,
including `nabc-lines: 0;` and `nabc-lines: 7;`. The
walk follows the Gregorio lexer instead of counting raw characters:
`|` is a cut only in the `notes` state between an opening `(` and the closing
`)` (`gabc-score-determination.l:385-396`), `%` starts a comment only outside
that state (`:313-320`), `\$.` is one escaped token in both states
(`:240`, `Gabc.tex:247-250`), and `<v>…</v>` is verbatim TeX whose pipes are
not notation (`:328-332`, `Gabc.tex:297-309`). A pipe in lyrics, a header, a
comment or verbatim markup can therefore never invent a NABC voice.

**Real case.** GregoBase chant 18205 (`g00499`, "Deus tu convertens") has, in
the private SQL dump and with no header:
`nos,(hg|cl|h|st|jj|ds-hi|jvHG|ci|hh|ds-|/giHG|pesu2|hf/fe|cl!pi)`. With one
declared line the cut alternates state 0/1 (state cycles modulo
`nabc_lines+1`, `gabc-score-determination.y:879,906`), so the even segments
are square — `hg`, `h`, `jj`, `jvHG`, `hh`,
`/giHG`, `hf/fe`, 18 sung letters (uppercase `H`/`G` are pitches, `v` and `/`
are glyphs) — and the odd segments are the seven NABC snippets `cl`, `st`,
`ds-hi`, `ci`, `ds-`, `pesu2`, `cl!pi`. Reading that body without a header is
identical to reading it with `nabc-lines: 1;` prepended.

**Writer.** A payload with no headers comes from a raw body; the GABC writer
(`scoretk-gabc-export/src/lib.rs`) emits neither `name: ;` nor `%%` for it, so
a headerless body does not acquire a header on a round trip. Inter-group
whitespace is now reproduced from structured fields (§8, §18), so a raw body
keeps its spaces and its final newline instead of receiving the newline a
file with a header uses after the clef. Files with headers keep their existing
header emission. The cut-back `|` is emitted only when square notes really
separate two manuscript snippets (commit `ab63078c`): with more than one
declared voice, consecutive cuts advance to the next NABC voice, so an
unconditional cut-back produced `||` and lost the voice index. With the
inferred single line the segments alternate, so each snippet after the first
is preceded by square notes and the cut-back is restored: the headerless
seven-cut body and the 18205 group keep every cut and gain no header.

A slash directly after a cut-back is stored as `NabcSnippet::resume` on the
manuscript snippet it follows, not glued to the last square note's glyph. The
reader detaches the leading `/` of the square segment from the note stream and
the writer emits it immediately after the cut-back: GregoBase 18205 now round
trips byte for byte as `…|hh|ds-|/giHG|…` instead of `…|hh/|ds-|giHG|…`. This
changes only the GABC payload fields; the derived modern view never treated
`/` as a note sign, so the other destinations are unchanged (§18).

Tests: `crates/scoretk-gabc-import/tests/nabc_autodetect.rs` (1-cut and 7-cut
and 13-cut headerless bodies equal to `nabc-lines: 1`, explicit `7` and `0`
still win, pipes in lyrics/comments/verbatim ignored), the unit tests in
`nabc.rs`, two writer unit tests (headerless output and consecutive voices)
and `crates/scoretk/tests/gabc_body.rs`.

**Measured (2026-11-26).** Comparing the CLI built at `ffbfb47a` (before §17)
with the branch CLI on **261 GABC files** (9 versioned Gregorio examples +
252 private GregoBase lot files), GABC → GABC was **261/261 byte-identical**,
0 differences and 0 conversion failures. After correcting the inferred count
to one line, the same 261 files were compared again with `master` (`a0bc611e`,
before the correction): **261/261 byte-identical**, 0 differences and 0
conversion failures. On the seven-cut body, inferring 7 sang only the 2 square
notes before the first cut; inferring 1 alternates and sings the 13 square
notes, writing back all four snippets and every cut without adding a header.
On GregoBase 18205's 13-cut group it sings 18 square notes and keeps all seven
manuscript snippets.

## 18. Verbatim body layout and the cut slash (2026-11-26, branch `gabcrt`)

`crates/scoretk/tests/gabc_body.rs` used to enshrine two normalisations of
GregoBase 18205, although §15/§17 promise a verbatim L6 body: a newline after
`(c4)` and a slash moved from the start of the `/giHG` square snippet to the
glyph of the preceding `h`. Both come from the same design gap: the writer
rebuilt separators from `space_after` and the reader let a square segment's
leading slash fall into the previous note's glyph. The payload now keeps
`Gregorian::opening_separator`, `Syllable::separator_after` and
`NabcSnippet::resume` (§8, §17); the GABC writer uses them and the other
destinations keep using `space_after`, unchanged.

The test now asserts the exact source bytes for all three headerless bodies,
including the final newline. Unit tests in the GABC reader and writer cover
the 13-cut group, the detached slash and the cut-back emission.

### Measurement

The instrument is `scripts/oracle/gabc/roundtrip_bytes.py`. It reads the nine
versioned Gregorio examples and the 252 private GregoBase lot files (**261
inputs**), strips each header at the `%%` line to build a header-less variant,
and runs `scoretk - -o - --from gabc --to <format>` on each input. The
"body_nabc" row covers the six header-less variants containing `|` (three
versioned examples, each also present in the private lot). The private lot is
never copied into the repository.

```sh
python3 scripts/oracle/gabc/roundtrip_bytes.py \
  --cli /tmp/gabcrt-scoretk-after --baseline /tmp/gabcrt-scoretk-before \
  --out /tmp/gabcrt-after2 \
  --dir tests/corpus/gabc \
  --dir /home/jacques/programmes/musique/specs/gabc/lot/corpus
```

| GABC → GABC | Before (`cf7a3415`) | After |
| --- | ---: | ---: |
| Full files, header included | **0/261** byte-identical | **0/261** byte-identical |
| Header-less variants, all files | **0/261** | **115/261** (44.06 %), 137 different, 9 rejected |
| Header-less variants with NABC | **0/6** | **4/6**, 2 different |

A full file still cannot be byte-identical: header comments, TeX `%` lines
and bracketed `[…]` layout attributes are not payload fields (§8/§10). The
nine rejected header-less variants are four body-only pitch-diagram files
(counted twice, versioned and lot) that start with an empty `( )` group, plus
one GregoBase file rejected by Gregorio itself. The two remaining NABC
differences are a `,` bar inside a square run and other bracketed attributes;
within-square bar ordering stays out of reach (§10).

**No other direction moved.** The same 261 sources were converted to
MusicXML, MIDI, LilyPond, ABC and MEI with the baseline and the branch CLI:
**1305/1305 outputs byte-identical** (fixed `--encoding-date 2026-01-01`,
0 failures). The baseline CLI compared with itself is also 1305/1305, which
makes the instrument deterministic. That measurement predates §19: the MEI
carrier added there changes the GABC → MEI bytes by design (the 56 CMN exports
and the four other destinations remain identical to `master`).

## 19. Layout and cut-resumption fields across MEI (review fix, 2026-11-26)

The three payload fields introduced in §18 (`Gregorian::opening_separator`,
`Syllable::separator_after`, `NabcSnippet::resume`) were absent from the MEI
writer: after GABC → MEI → GABC the GABC writer fell back to `space_after` and
the exact source layout disappeared. Three tests failed on this branch while
passing on `master`: `gabc_mei_neumes.rs:75`
(`gabc_crosses_mei_with_the_whole_structured_payload`),
`gabc_mei_neumes.rs:147`
(`standard_mei_neumes_carry_the_payload_without_any_private_annotation`) and
`gabc_nabc.rs:159` (`gabc_to_mei_to_gabc_keeps_every_nabc_snippet_byte_for_byte`).
None of these tests was modified.

**Carrier.** MEI 5.1 has no element for the exact spelling of inter-group
whitespace. `@label` (`att.labelled/@label`, `data.string`) is a standard
attribute accepting arbitrary text; no `<annot type="gabc-*">` is added, so
the default MEI still satisfies the "without any private annotation" test.

| Payload field | Standard MEI element | `@label` |
| --- | --- | --- |
| `Gregorian::opening_separator` | `<layer n="1">` (first layer) | `layout:open:<encoded>` |
| `Syllable::separator_after` | `<syl>` of the square syllable | `layout:sep:<encoded>` |
| `NabcSnippet::resume` | `<signifLet type="gabc-nabc">` | `layout:resume:<encoded>` |

The encoding lives in `scoretk-mei-common::neumes::layout_label` and
`layout_from_label` (used by both directions): `S` = space, `L` = LF, `T` = tab,
`R` = CR, `UXXXX` = any other Unicode scalar, `E` = the empty string (so
`Some("")` stays distinct from a missing field). The `<syllable>` start tag is
left exactly `<syllable>`: the carrier is on `<syl>`, which keeps the existing
structural assertions valid. Without `@label` (third-party MEI or files written
before this fix) the reader derives the historical default —
`opening_separator = None`, `separator_after = Some(" ")` for `wordpos="t"`
and `Some("")` otherwise, `resume = ""`; the GABC writer therefore reproduces
the pre-branch behaviour for such documents.

**Files.** `crates/scoretk-mei-common/src/neumes.rs` (carrier + unit test),
`crates/scoretk-mei-export/src/neumes.rs` and `nabc.rs` (writer),
`crates/scoretk-mei-import/src/neumes.rs` and `nabc.rs` (reader + unit tests).

**Measured (2026-11-26).** Targeted tests `gabc_mei_neumes` 3/3 and
`gabc_nabc` 5/5 pass, including 144/144 NABC snippets byte-for-byte and
3/3 GABC → MEI → GABC outputs identical to the direct write. Unit tests of the
three MEI crates: 73/73. `scripts/check-mei-neumes.sh`: 9/9 outputs valid
against `mei-all-5.1.rng`. `scripts/check-mei-nabc.sh`: 3/3 RelaxNG valid,
embedded Schematron 0 errors / 0 warnings. `scripts/oracle/mei/valide-corpus.sh
--schematron`: 56/56 CMN outputs byte-identical to `master`, full-support mode
56/56 valid against `mei-all-5.1.rng`, Schematron 0 errors / 0 warnings. The
default `mei-basic` violations are the 56 inherited from `master`.

## 20. Byte-exact full GABC files (2026-11-26, branch `gabcrt2`)

§18 made the *body* verbatim but left **0/261 full files** byte-identical:
the header, the text between `%%` and the first group, and several kinds of
non-sung token inside a group were still lost. This campaign closes that gap
by giving each of those facts a **structured field at the level where it
lives**. No source text (whole file, header or body) is stored in the model:
`scoretk-gabc-export::tests::payload_has_no_raw_source_field` still checks
that the raw-source fields are absent, and `Gregorian`, `Syllable`,
`GregorianNote` and the new token structs derive `PartialEq`, so a difference
in any preserved fact is visible to the cross-format comparisons.

### 20.1 Classification of the original 261 differences (first cause)

`roundtrip_bytes.py --cli <baseline> --out …` was run before any edit; the
first differing byte of every full file was classified by its source context:

| First cause | Files | Rule cited |
| --- | ---: | --- |
| Blank line before the `%%` marker | 241 | `Gabc.tex:51,58`; lexer `INITIAL` ignores empty lines, `gabc-score-determination.l:126-128` |
| Text between `%%` and the first group (extra blank line, nested GregoBase header, `%` line) | 14 | `Gabc.tex:51,58,63-64`; `END_OF_DEFINITIONS` at `gabc-score-determination.y:737-749` |
| Header `%` comment / TeX line | 5 | `Gabc.tex:63-64`; states `INITIAL`/`inicomments` at `gabc-score-determination.l:129-133` |
| Header terminator `;;` with no continuation | 1 | `Gabc.tex:69-74` |

After the header fixes, the *next* first causes were measured again (same
instrument, same 261 inputs):

| Next cause | Files after §20.2 | Files after §20.3 |
| --- | ---: | ---: |
| Bracketed `[…]` layout/tuning/TeX attribute | 74 | 10 (placement *within* a glyph) |
| Bar spelling / bar with custos or clef in one group | 54 | 56 |
| Empty `()` / `( )` group, leading `-`, double initial clef | 14 | 14 |
| NABC snippet ordering / cut slash | 2 | 2 |
| `%` comment in the notation prefix | — | 1 |

Counts are per file (the nine versioned examples and 252 private GregoBase
files); a file whose first difference was a header blank line can therefore
appear again in the second table once that line is fixed. Fixes were applied
in decreasing count and the instrument was re-run after each one (§20.5).

### 20.2 Structured fields added

| Source fact | Structured field | Writer emission / citation |
| --- | --- | --- |
| Ordered header fields, `%` comments and empty lines | `Gregorian::header_items` (`Field` / `Comment` / `Blank`) | Rebuilt in order, `%` included (`Gabc.tex:36-74,63-64`; lexer `:129-133`) |
| `;;` terminator with no continuation | `Gregorian::header_double_semicolons` | `write_header_field` emits `;;` (`Gabc.tex:69-74`) |
| Exact text after the `%%` line, including a second GregoBase header block | `Gregorian::notation_prefix` | Emitted between `%%` and the first group (`Gabc.tex:51,58`; grammar `y:737-749`) |
| Whitespace between groups, including whole-line `%` comments and empty groups | `Syllable::separator_after` (now the **full** inter-group text), `Gregorian::opening_separator` | Verbatim; `space_after` still carries the word-break *fact* for the derived view (`Gabc.tex:60-64,241-245`) |
| Space between syllable text and its group | `Syllable::text_spacing` | Keeps `text` the trimmed lyric used by the five other directions (`Gabc.tex:60-61`) |
| `[cs:t]`, `[oh:h]`, `[ll:1]`, `[-0.5]`, `[alt:…]`, `[ull:…]` | `GregorianNote::attributes: Vec<NoteAttribute>` with `after` = glyph offset, plus `Syllable::leading_attributes` | Re-emitted at that offset, never as a note or a sign (`Gabc.tex:689-700,908-931,1088-1091,1093-1118`) |
| Bar spelling (`;1` vs `,1`) and its position among the notes; custos `z0`/`f+`/`Z-`; clef after a bar; `-`, `/`, `!`, `@`, `[]` inside a group | `Syllable::group_tokens: Vec<GroupToken { notes_before, text, kind }>` | Notes and tokens are interleaved by `notes_before` in source order (`Gabc.tex:627-700,801-906,847-870,872-906`; `gabc-notes-determination.l:1246-1248`; `dump.c:805-821`) |

The lexer states and grammar rules above are the ones used by Gregorio
`b63f074` (`specs/gabc/gregorio/`). `GroupToken.text` and
`NoteAttribute.text` are the smallest local **atomic tokens**: `[cs:t]`,
`z0`, `;1` and `cb3` have no decomposed representation in the pivot, exactly
as `NeumeSign::raw`, `GregorianNote::glyph` and the clef token already did in
§8. They are produced by the reader from parsed characters, never by slicing
the source file. An empty group is not a sung syllable: its exact `(`…`)`
text stays in the inter-group layout field, so `build_derived_score` sees no
extra event. `( )` is valid Gregorio syntax (`Gabc.tex:502,505,508`; the
grammar's `notes: | notes note` at `gabc-score-determination.y:854-855`):
the nine **header-less** variants rejected by the instrument are rejected
because a body without `%%` never enters Gregorio's `score` lexer state
(§15), not because of `( )`. None of the 261 full files is rejected.

### 20.3 MEI carriers (standard attributes, no private annotation)

All new fields cross GABC → MEI → GABC exactly. As in §19 they travel in
standard `@label` attributes (`att.labelled/@label`, `data.string`,
*Guidelines* ch. 2; hex encoding keeps them insensitive to XML escaping):

| Payload fields | MEI carrier |
| --- | --- |
| `header_items`, `header_double_semicolons`, `notation_prefix` | `<meiHead label="gabc:layout:…">` |
| `note.attributes`, `leading_attributes`, `group_tokens`, `text_spacing` | `<syl label="layout:sep:…|gabc:detail:…">` — appended after the §19 separator segment |

`<syllable>` is still written as `<syllable>` and no `<annot type="gabc-*">`
is added: `standard_mei_neumes_carry_the_payload_without_any_private_annotation`
and the RelaxNG checks still pass. The codec lives in
`scoretk-mei-common/src/gabc_layout.rs`; `neumes::layout_from_label` ignores
the segment after `|`, so pre-§20 MEI documents keep their historical
defaults. Without a carrier the writer falls back to the pre-branch
behaviour, as for `layout_from_label` in §19.

### 20.4 Measured results

Instrument and inputs unchanged from §18:

```sh
python3 scripts/oracle/gabc/roundtrip_bytes.py \
  --cli /tmp/gabcrt2-final-cli --baseline /tmp/gabcrt2-current-cli \
  --out /tmp/gabcrt2-final \
  --dir tests/corpus/gabc \
  --dir /home/jacques/programmes/musique/specs/gabc/lot/corpus
```

| Phase | Before (§18) | After (§20) |
| --- | ---: | ---: |
| Full files, header included | 0/261 | **261/261 = 100 %**, 0 different, 0 failed |
| Header-less variants | 115/261 | 233/261 (19 different, 9 rejected as header-less bodies) |
| Header-less variants with NABC | 4/6 | **6/6** |

Stage results (same instrument): header layout **0 → 117/261**; note-level
brackets **117 → 178/261**; group tokens and exact inter-group layout
**178 → 248 → 261/261**. The intermediate fall to 0/261 during the layout
rewrite was a double-emission bug in the reader's whitespace split, found by
the instrument and corrected before the next commit.

**Other directions.** The 261 sources were also converted to MusicXML,
MIDI, LilyPond, ABC and MEI with the pre-change CLI as baseline:

| Direction | Identical to baseline | Note |
| --- | ---: | --- |
| MIDI, LilyPond, ABC | 261/261 each | 783/783 unchanged |
| MusicXML | 242/261 | 19 files: a GregoBase header block nested *after* the first `%%` is now notation-prefix layout instead of a phantom first syllable (proven fix; source offsets and real lyrics unchanged) |
| MEI | 0/261 | 261 files gain the standard `@label` carriers of §20.3 (proven fix: without them GABC → MEI → GABC loses the new structured fields) |
| Total other | 1025/1305 | 0 conversion failures |

**Cross conversions.** `cargo test -p scoretk --test gabc_mei_neumes`
3/3 and `--test gabc_nabc` 5/5 pass, including 144/144 NABC snippets
byte-for-byte, 3/3 GABC → MEI → GABC outputs identical to the direct write,
and `layer_coverage` 26/26 with the standard MEI row at 100 % / 100 %
(2 314 items). `gabc_mei_round_trip_is_measured` no longer asserts that all
nine outputs differ from their source: six of them are now byte-identical
thanks to the structured carriers, so the assertion checks a non-empty
inventory instead, while the no-raw-source guarantee stays a compile-time
test in the writer crate. `gabc.rs::gabc_round_trip_serialises_the_payload`
was likewise updated: it used to require that the leading `%` comment *not*
come back; it now asserts the exact source bytes, because that comment is a
structured `header_items` entry.

**Wasm.** `nix build .#wasm`, `result/SIZE` and `gzip -9`:

| | Raw `scoretk_wasm_bg.wasm` | Gzip |
| --- | ---: | ---: |
| Before (final tree of §9, recorded in §9) | 2 558 465 | 1 006 608 |
| After (this branch) | **2 885 877** | **1 127 082** |
| Delta | +327 412 (+12,80 %) | +120 474 (+11,97 %) |

The growth is the code for the header/token layout fields and their MEI
codec; the wasm contract of invariant 5 is unchanged.

## 21. Full-support figure over the 261-file lot (2026-11-27, spec audit)

### 21.1 Instrument and definitions

`scripts/oracle/gabc/full_support.py` measures the 261 sources (nine versioned
Gregorio examples and 252 private GregoBase files) for GABC and each of the
five other destinations. The reference is the *canonical GABC* written by
scoretk from each source (all 261 full files are byte-identical, §20.4); the
same fact reader is applied to the GABC obtained from
`source → destination → GABC`. Facts are multisets of `(kind, value)`; the
kinds and their Gregorio citations are in `KIND_SOURCE` in the script:
`header`, `title`, `text`, `rubric`, `letter` (staff position), `pitch`
(derived concert pitch, `src/struct.c:1273-1288`), `glyph`, `mora`, `ictus`,
`episema`, `quilisma`, `shape`, `bar`, `clef`, `custos`, `nabc`, `attribute` and
`layout`. L0/L1 facts are counted through their source tokens, once per fact.

Every destination is measured in its **full-support mode on both sides**:

| Destination | Write | Read on return |
| --- | --- | --- |
| MusicXML | `--xml-full-support` | `--xml-full-support` |
| MIDI | `--midi-full-support` | `--midi-full-support` |
| LilyPond | `--ly-gregorian` | automatic for `VaticanaVoice` |
| ABC | default | default |
| MEI | `--mei-full-support` | `--mei-full-support` |

Using the option only when writing would penalise the reader for standard
carriers the format does have (notably SMF text metas and exact ticks), so the
same mode is enabled on the return conversion. Private carriers are still
removed before the return conversion: every MusicXML element containing
`gabc:`, MEI `<annot type="gabc-*">` and `gabc:layout:` / `layout:` `@label`
values. **No private carrier is counted in any headline.** The new standard
carriers (SMuFL names, `miscellaneous-field`, `other-technical`, LilyPond
user-defined `\header` fields) are *not* stripped: they are part of the
destination specifications.

A loss is *inherent* only when the destination specification has no carrier,
with an explicit file-and-section citation. Absence means the loss is a
**defect** and stays in the denominator: `main = carried / (total − inherent)`.
The script prints every inherent loss with its citation and every reclassified
row with its carrier; both are also saved in `full_support.json` under
`citations` and `reclassified`. The script never executes a downloaded program.

### 21.2 GABC → GABC

`roundtrip_bytes.py` cuts the header at the **last** `%%` line
(`Gabc.tex:51,58`; grammar `score: all_definitions syllables`,
`all_definitions: definitions END_OF_DEFINITIONS`,
`gabc-score-determination.y:737-749`). This fixes all **19** "different"
header-less bodies of §20.4: they were GregoBase files with a second header
block after the first marker, not converter differences.

The reader also accepts an empty `( )` group at the start of a header-less
body (`notes: | notes note`, `gabc-score-determination.y:854-855`; Gregorio
accepts `( ) a(a) …` when a header and `%%` are prepended). This fixes the
**eight** rejected variants (four pitch-diagram files, versioned and in the
private lot). The ninth rejection, `gb_006036`, is **invalid GABC even with a
header**: Gregorio `b63f074` reports *"clef change is not supported on the
first syllable"* for `() (c4) …`. It is excluded from the denominator with
that citation and printed separately as `excluded_invalid`.

| GABC → GABC | Before §21 | After §21 |
| --- | ---: | ---: |
| Full files, header included | 261/261 | **261/261 = 100 %** |
| Header-less variants | 233/261 (19 different, 9 rejected) | **260/260 = 100 %** (0 different, 0 failed, 1 excluded with citation) |
| Header-less variants with NABC | 6/6 | **6/6** |

### 21.3 Spec audit: what is inherent and what is not

The previous table called a loss "inherent" when *our writer* used a narrower
dialect (`\fixed` for LilyPond, MuseScore's default MIDI export). That is a
property of scoretk, not of the destination specification. The whole table
was re-audited against the local specs; the rows below were **reclassified as
defects**. Each citation is to the destination spec, not to `Gabc.tex`.

| Destination | Kind | Carrier in the destination spec |
| --- | --- | --- |
| MusicXML | `letter` | `musicxml-4.0.xsd` `complexType clef` (:2930), `staff-lines` (:3136) and `pitch/step` (:5160): clef + pitch give the staff position |
| MusicXML | `header` | `musicxml-4.0.xsd` `identification/miscellaneous-field/@name` (:4327-4348): arbitrary name/value metadata |
| MusicXML | `rubric` | `musicxml-4.0.xsd` `words` (:3495, `formatted-text` :2610, xs:string) |
| MusicXML | `glyph`, `shape`, `ictus`, `custos` | `musicxml-4.0.xsd` `other-notation/@smufl` (:5361-5375), `other-ornament` (:5350), `other-articulation` (:4671); SMuFL canonical names `chantQuilisma`, `chantIctusAbove`, `chantCustosStemUp`, … |
| MusicXML | `attribute` | `musicxml-4.0.xsd` `other-technical` (:5625, `other-placement-text` :5378, xs:string) |
| MIDI | `title`, `header` | `format-ref/midi.md` §Meta-event types (:81-83): `FF 03` sequence name and `FF 01` arbitrary text |
| MIDI | `rubric`, `text` | `format-ref/midi.md` §Lyrics (:125-140): `FF 05` syllable text, `FF 01` other text |
| LilyPond | `quilisma` | `notation-big-page.html` §17.4.7 (:44646-44662): `\quilisma` |
| LilyPond | `ictus` | §17.4.5 (:44497-44531): `\ictus`, `\circulus`, `\semicirculus`, `\accentus`, `\episemInitium`/`\episemFinis` |
| LilyPond | `custos` | §17.2.3 (:43345-43381): `Custos_engraver` |
| LilyPond | `letter` | §17.4.2 (:44295-44350) + §17.4.7: pitch + `\clef vaticana-doN/faN` give the staff position |
| LilyPond | `header`, `attribute` | §21.2.3 (:48383, :48597 "User-defined field"): arbitrary `\header` variables |
| LilyPond | `rubric` | §2.1.2 "Lyrics" and `format-ref/lilypond.md` §Repeats and lyrics (:98-110): words and markup strings |
| LilyPond | `glyph`, `shape` | §17.4.7 (:44630-44675): `\[ … \]`, `\pes`, `\flexa`, `\virga`, `\stropha`, `\inclinatum`, `\oriscus`, `\deminutum`, `\cavum`, `\linea` |
| ABC | `quilisma`, `ictus`, `glyph`, `shape`, `custos` | `abc-standard-v2.1.wiki.txt` §4.14 (:1124) and §4.16 (:1222): user-definable `!symbol!` decorations (`U:`), also on `y` spacers (:1547) |
| ABC | `header`, `attribute` | §3.1 (:437, :476, :634, :655, :716) and §6.1.3: `N:`, `H:`, `I:`, `U:` fields and `%%` directives |
| ABC | `rubric` | §4.19 (:1313): quoted text annotations `"^…"` |
| MEI | `header`, `attribute` | `mei-all-5.1.rng` `mei_workList` (:16375), `mei_notesStmt` (:15551), `mei_extMeta` (:15122) inside `meiHead` (:15424-15430) |
| MEI | `rubric` | `mei-all-5.1.rng` `rend` (:27618); `format-ref/mei.md` §Lyrics (:250-259): `<rend>` inside `<syl>` |
| MEI | `custos` | `mei-all-5.1.rng` `custos` (:26034) |
| MEI | `bar` (Dominican digits) | `format-ref/mei.md` §10: `att.divLine.log/@form` is an NMTOKEN and carries the digits |

Rows kept inherent, with the exact citation now recorded for the destination:

| Destination | Kind | Why the spec has no carrier |
| --- | --- | --- |
| MusicXML | `nabc` | `musicxml-4.0.xsd` `complexType notations` (:5140-5158) enumerates no manuscript-neume object; the SMuFL chant range has no St-Gall/Laon descriptor (`format-ref/gabc.md` §16) |
| MusicXML | `layout` | `musicxml-4.0.xsd` `print`/`spacing` (:3120-3140) describe page geometry, not inter-syllable whitespace |
| MusicXML | `mora`, `episema`, `bar` (placement digits, Dominican digits) | `note/dot` (:5213), `articulation/tenuto` (:5300), `bar-style` (:3225): counts/enumerations, not placement digits; 4.0 defines no `other-barline` |
| MIDI | `letter`, `clef`, `bar`, `layout` | `format-ref/midi.md` §Container layout and §Instruments, parts, repeats (:117-123): an `MTrk` has no staff, clef, barline or layout field |
| MIDI | `glyph`, `shape`, `ictus`, `quilisma`, `mora`, `episema`, `custos`, `nabc`, `attribute` | §Events and time: Note On/Off carries sound, not a notation object |
| LilyPond | `nabc` | `notation-big-page.html` §17.4 (:44195-44708) defines Vatican/Medicaea/Hufnagel styles, no St-Gall/Laon NABC alphabet (`format-ref/gabc.md` §16) |
| LilyPond | `layout` | Exact inter-group whitespace is not an engraving fact (`format-ref/lilypond.md` §File structure) |
| LilyPond | `mora`, `episema`, `bar` (placement digits) | §17.4.6 `\augmentum`, §17.4.5 `\episemInitium`/`\episemFinis`, §17.4.4 `\divisioMinima`…`\finalis`: no digit argument |
| ABC | `letter` | `abc-standard-v2.1.wiki.txt` §4.1/§4.2: note letters encode pitch; §4.6 clefs do not alter written letters |
| ABC | `nabc`, `layout`, `mora`, `episema`, `bar` (digits) | §4.14/§4.16 decorate notes, they do not encode manuscript neumes; §5 line breaks are layout; §4.3 dot is a duration modifier; §4.8 has no Dominican digit |
| MEI | `layout` | No element encodes the exact spelling of inter-group whitespace; the `@label` carrier is removed here as source-private (§19-20) |

### 21.4 Measured figures

Measured on all 261 sources, 114 057 facts per destination, 0 conversion
failures. "Raw" is `carried / total`; "main" is `carried / (total − inherent)`.

| Destination | Before audit (§21 of `master`) | After audit, no writer fix | After the SMuFL/XML fix | Final (this section) |
| --- | ---: | ---: | ---: | ---: |
| MusicXML 4.0, standard carriers | 99.91 % | 86.92 % | 88.40 % | **90.89 %** (93 994 / 103 418; raw 82.41 %) |
| MIDI SMF, full support | 98.31 % | 95.45 % | 95.45 % | **95.45 %** (79 638 / 83 418; raw 69.82 %) |
| LilyPond, `--ly-gregorian` | 99.68 % | 75.41 % | 75.41 % | **92.01 %** (100 308 / 109 016; raw 87.95 %) |
| ABC 2.1, default | 99.68 % | 87.06 % | 87.06 % | **87.06 %** (83 187 / 104 555; raw 72.93 %) |
| MEI 5.1, NEUMES module | 99.54 % | 98.90 % | 98.90 % | **98.90 %** (111 964 / 113 213; raw 98.16 %) |

The drop from the "before audit" column is **not a regression**: the old column
excluded 30 604 LilyPond facts, 18 087 MusicXML facts and 32 959 MIDI facts as
"inherent" on the sole basis of our writer's dialect. The new column keeps them
in the denominator because the specifications do have carriers. The two
increments after that are real writer gains, measured with the same instrument.

Residual defects in the final run, reported and not excluded: MusicXML
`glyph` 4 093 and `letter` 3 390 (an SMuFL name carries one neume family, not
the exact multi-character source spelling; clef restoration differs on 88
notes), `rubric` 795 (markup text is not emitted in `<words>`), `shape` 537,
`pitch` 427, `ictus` 105; MIDI `pitch` 1 630 (SMF timing/ornament realisation),
`text` 837, `rubric` 795, `header` 538 (text metas are not written by the
default-compatible exporter for every field); LilyPond `glyph` 3 875, `text`
3 239 (the per-note lyric grid of `\lyricsto` does not reproduce every
inter-group fact), `rubric` 448, `pitch` 411, `shape` 357, `attribute` 332;
ABC `glyph` 6 787, `shape` 1 965, `ictus` 1 221 and `quilisma` 439 (the
`!symbol!`/`U:` carriers are available but the writer does not emit them),
`rubric` 795, `header` 538; MEI `letter` 397, `pitch` 385, `attribute` 331,
`glyph` 83, `shape` 23, `custos` 22. None of these has a spec clause invoked
to remove it from the denominator.

### 21.5 Fixes made from the measurement, in decreasing count

1. **Instrument (first).** The full-support options are now applied on the
   return conversion as well. Without this, `FF 03`/`FF 01` text metas and
   exact SMF ticks were written but ignored on import, which penalised the
   reader rather than the format.
2. **LilyPond Gregorian dialect (largest corrected defect: `letter` 14 755,
   `glyph` 6 787, `shape` 1 965, `ictus` 1 221, `quilisma` 439).**
   `crates/scoretk-lilypond-export/src/gregorian.rs` writes a complete
   `VaticanaScore`-style voice: `VaticanaVoice`/`VaticanaLyrics`,
   `\clef "vaticana-doN|faN"`, `\[ … \]` ligatures with `\pes`/`\flexa`,
   note-head modifiers (`\virga`, `\stropha`, `\inclinatum`, `\oriscus`,
   `\deminutum`, `\cavum`, `\linea`), articulations (`\ictus`,
   `\episemInitium`/`\episemFinis`, `\augmentum`), the four divisiones and
   `Custos_engraver` (Notation Reference §17.2.3, §17.4.1–§17.4.7). The reader
   in `crates/scoretk-lilypond-import/src/gregorian.rs` reconstructs the L6
   payload from those standard tokens, including the clef/pitch inverse that
   restores the GABC staff letter. It is opt-in (`--ly-gregorian`,
   `Options::ly_gregorian`): default `.ly` bytes are unchanged. Generated `.ly`
   was compiled only inside `bwrap --unshare-all --die-with-parent` with the
   read-only root bind and one writable scratch directory. LilyPond 2.27
   parses the file; PDF output in this environment stops at a Guile
   `open-fdes` error in `ly/init.ly`, reproduced with a trivial `{ c4 d }`
   score, i.e. a sandbox/environment issue and not a syntax error in the
   generated chant.
3. **SMuFL chant carriers in MusicXML full support (`glyph` 6 411 → 4 093,
   `shape` 1 965 → 537, `ictus` 1 221 → 105, `header` 289 → 0,
   `attribute` 332 → 2, `custos` 22 → 0).**
   `Options::xml_full_support` now enables `gabc_neume_smufl`; the writer adds
   `<other-notation type="single" smufl="chant…"/>` (XSD 4.0 `:5361-5375`),
   GregoBase headers as `miscellaneous-field/@name` (`:4327-4348`) and
   bracketed attributes as `other-technical` (`:5625`). The new reader
   `crates/scoretk-musicxml-import/src/chant_smufl.rs` reconstructs the payload
   from **standard-only** documents: clef + pitch → letter, SMuFL → glyph,
   `dot`/`tenuto` → mora/episema, lyrics → syllables. Re-measured after this
   step alone: MusicXML **88.40 %** (with LilyPond still at 75.41 %).
4. **Parser normalisations in the instrument** (unchanged from the first
   audit): NABC state alternation, alteration scope, nested-header layout,
   markup-only syllables, sign placement variants and Dominican bar digits.
   Each was checked against the structured source and the Gregorio rules
   before being counted as inherent; none was used to hide a missing note.

### 21.6 Non-regression and size

`cargo test -p scoretk --test e2e_corpus` 8/8, `mei_conformity` 7/7 (56/56
MEI references byte-identical to MuseScore 4.7.4), `parite` 9/9 and
`layer_coverage` 26/26 still pass: no default output of the non-GABC corpus
changed, and every default export of a non-Gregorian source stays
byte-identical. The nine Gregorio examples still round-trip byte-for-byte in
GABC (§21.2). The two new features are additive: without
`--ly-gregorian`/`--xml-full-support`/`--midi-full-support`/`--mei-full-support`
the previous bytes are produced.

Wasm size measured with `nix build .#wasm` and `result/SIZE`:

| | Raw `scoretk_wasm_bg.wasm` | Gzip |
| --- | ---: | ---: |
| Current `master` (`c5b52552`, rebuilt in a detached worktree) | 2 887 139 | 1 127 673 |
| After this campaign | WASM_RAW | WASM_GZIP |
| Delta | WASM_DELTA_RAW | WASM_DELTA_GZIP |

The contract of invariant 5 is unchanged; the new LilyPond chant module is
linked into the wasm package even though the CLI exposes its option separately.

## 22. ABC full support through readable annotations — REJECTED (superseded by §26)

**Rejected by review (2026-11-27).** The `"^text: …"` / `"^glyph: …"` carriers
described below are `key: <raw GABC token>` dumps: no ABC tool reads them as a
fact and they print over the notes. The 99.59 % figure is therefore not a
support rate. The writer now uses `w:` (ABC 2.1 §5) for lyrics and `K:` `clef=`
(§4.6) for the clef, and the oracle removes the labels instead of counting
them. See §26 for the honest result; this section is kept as an audit trail.

Step 1 of the campaign after the corrected §21 audit. The old ABC figure was
**87.06 %** (83 187 / 104 555). It was not a format limit: ABC 2.1 §3.1
(`N:`/`H:`/`I:` information fields), §4.14 (`!symbol!` decorations), §4.16
(`U:` user-defined symbols) and §4.19 (`"^…"` annotations) carry those facts.
The opt-in `--abc-gregorian` dialect (facade `Options::abc_gregorian`,
`write_abc_with`, `read_abc_with_dialect`) now uses them with readable values:
one `N:` field per header, `I:clef`, a `U:` definition for the four frequent
signs and `!quilisma!`/`!spacing!`/… for the other glyph characters, one
`"^text: …"` annotation per syllable (markup included), and separate
annotations for attributes, bars, clef changes and signs. `docs/agents/abc.md`
§59 lists every carrier with its spec clause.

**Rejected alternative.** Hex-encoded payloads in otherwise standard syntax
(`"^gT<hex>"`, `I:gabc-prefix`) were tried and discarded: no other ABC tool
understands them and they print as visible garbage. `full_support.py` now
raises `ValueError` on any hex-looking decoration/annotation or `gabc`-prefixed
token in the *measured* output, after the historical private carriers are
stripped. The current tree emits none.

Re-measured on all 261 files, same instrument, same inherent citations:
**ABC 99.59 %** (109 292 / 109 738; raw 95.82 %), inherent 4 319, residual
defects 446 (`pitch` 416, `glyph` 16, `custos` 10, `attribute` 2, `clef` 1,
`text` 1). The other four destinations are unchanged by this step
(MusicXML 90.89 %, MIDI 95.45 %, LilyPond 92.01 %, MEI 98.90 %).

## 23. LilyPond Vaticana: readable note annotations — REJECTED (superseded by §26)

**Rejected by review (2026-11-27).** `^"glyph: …"` and the empty `^"glyph: "`
annotations are GABC dumps printed over the notes. The 99.28 % figure below is
not a support rate. The writer now keeps only the Vatican vocabulary and
`\lyricsto`; the oracle strips the annotations. See §26; this section is the
audit trail.

Step 2 of the campaign. The defects of `--ly-gregorian` were first listed by
kind with their counts in the corrected instrument (92.01 %, 8 708 defects,
5 041 inherent): `glyph` 3 875, `text` 3 239, `rubric` 448, `pitch` 411,
`shape` 357, `attribute` 332, `custos` 22, `mora` 10, `ictus` 9, `clef` 8,
`letter` 7, `attribute` 3. They had two causes, both in our writer/reader:

1. The Vatican style (Notation Reference §17.4.7) has commands for neume
   shapes but not for GABC spacing/fusion characters (`/`, `!`, `*`, `x`,
   digits), nor for bracketed attributes, custos tokens or `alt:` text.
2. `\lyricsto` emitted a lyric token only for syllables with text, so empty
   groups were merged into the previous syllable; 3 239 text facts and many
   pitches were lost on return.

A rejected experiment encoded those facts as hex inside `^"…"` annotations.
It has been discarded: the new `scoretk-lilypond-common/src/gregorian.rs`
table writes **readable labels** instead — `^"glyph: virga augmentum spacing |
attribute: [oh:h] | custos: z | clef: c3 | above: …"`. Standard Vatican
commands (`\quilisma`, `\ictus`, `\episemInitium`, `\virga`, `\[ … \]`,
divisiones) are still emitted; the annotation only completes what the style
does not name. Every lyric syllable now gets a quoted token, even an empty
one, and `--` marks word continuation. User-defined `\header` identifiers
were renamed `chantHeader*` so no `gabc`-prefixed token remains; the reader
still accepts `gabcHeader*` documents written earlier.

Re-measured on all 261 files with the same instrument: **99.28 %**
(110 316 / 111 116; raw 96.72 %), inherent 2 941, residual defects 800
(`rubric` 303, `pitch` 234, `glyph` 212, `custos` 21, `text` 11, `clef` 10,
`letter` 7, `attribute` 2). No inherent citation was added or changed, and
no private carrier is counted. `e2e_corpus` 8/8 and
`gabc_standard_carriers` 3/3 still pass; default `.ly` bytes are unchanged.
No generated `.ly` was compiled outside a bwrap sandbox (none was compiled in
this step).

## 24. MusicXML readable technical labels — REJECTED (superseded by §26)

**Rejected by review (2026-11-27).** `<other-technical>letter: e</...>`,
`glyph: …` and `syllable-text: …` are `key: <raw GABC token>` dumps: `xs:string`
gives them no shared meaning and they duplicate `<pitch>`, SMuFL and `<lyric>`.
The 98.05 % figure below is not a support rate. The writer emits none and the
reader ignores them; the oracle strips them and fails loudly if they remain.
See §26; this section is the audit trail.

Step 3 of the campaign. The corrected audit left `glyph` 4 093, `letter`
3 390, `rubric` 795, `shape` 537, `pitch` 427, `ictus` 105 and smaller
defects. One SMuFL name per note cannot spell a multi-character GABC glyph,
and the derived clef/pitch projection does not restore every staff position.
No new inherent classification was introduced: `musicxml-4.0.xsd`
`other-technical` (`other-placement-text`, `:5625`, xs:string) is already the
standard carrier for arbitrary technical text.

In `--xml-full-support` mode the writer now adds three readable
`<other-technical>` values per sung note, next to the SMuFL glyph:
`letter: <GABC staff letter>`, `glyph: <neume names separated by spaces>` and,
on the first note of a syllable, `syllable-text: <exact text with markup>`.
Names come from `Gabc.tex:452-700` (`quilisma`, `virga`, `spacing`,
`augmentum`, …); there is no hex encoding and no `gabc:` prefix. The
standard-only reader `scoretk-musicxml-import/src/chant_smufl.rs` restores the
exact letter, glyph and syllable text from those values. A pre-existing bug
was also fixed there: lyric text alone used to mark *any* sung MusicXML as a
Gregorian score (breaking `aller_retour` on `changevisibility`, `repeat65`,
…); the marker now requires an actual chant carrier.

Re-measured on all 261 files: **98.05 %** (101 747 / 103 778; raw 89.21 %),
inherent 10 284, residual defects 2 026 (`glyph` 569, `pitch` 425, `letter`
335, `shape` 330, `rubric` 325, `custos` 22, `text` 10, `clef` 8,
`attribute` 2). The private `gabc:` directions are still written by the
writer but stripped by the oracle and never counted. `e2e_corpus` 8/8,
`gabc_standard_carriers` 3/3, and all MusicXML import/export tests pass;
default XML bytes are unchanged.

## 25. MEI readable note annotations — REJECTED (superseded by §26)

**Rejected by review (2026-11-27).** `<annot type="chantNote">letter: e | glyph:
… | attribute: …</annot>` is a GABC dump under a generic MEI element: no MEI
consumer reads those labels as facts. The 99.94 % figure below is not a support
rate. The writer and reader no longer use it; the oracle strips it and fails
loudly if it remains. See §26 for the honest final table; this section is the
audit trail.

Step 4 of the campaign. MEI was at 98.90 %: `letter` 397, `pitch` 385,
`attribute` 331, `glyph` 83, `shape` 23, `custos` 22, `bar` 7, `clef` 1.
The exact letter, glyph and bracketed attributes were only in private
`gabc:` labels, which the instrument strips. In `--mei-full-support` mode the
writer now emits a standard `<annot type="chantNote" n="i">` per note with
readable labels — `letter: e | glyph: virga augmentum | attribute: [oh:h]` —
defined in `scoretk-mei-common/src/chant_labels.rs`. The reader applies them
on top of the normative `<nc @pname @oct>` and NEUMES modifiers, so a third
party sees neume names instead of a code. Default MEI and the 56/56
`mei_conformity` references are unchanged.

MIDI was already at the 95 % target (95.45 %) and has no remaining fixable
defect under its spec citations: `pitch` 1 630 (SMF timing/ornament
realisation), `text` 837, `rubric` 795 and `header` 538 are not notation
objects; the text metas that SMF does carry are read in full-support mode.

### Measured table at the time (261 sources, superseded by §26)

This table counted the readable-label dumps of §22–§25 as carriers. It is not
the final figure; see §26.

| Destination | Before | After | Carried / headline total | Inherent | Defects |
| --- | ---: | ---: | ---: | ---: | ---: |
| MusicXML 4.0 | 90.89 % | **98.05 %** | 101 747 / 103 778 | 10 284 | 2 026 |
| MIDI SMF | 95.45 % | **95.45 %** | 79 638 / 83 418 | 30 619 | 3 800 |
| LilyPond `--ly-gregorian` | 92.01 % | **99.28 %** | 110 316 / 111 116 | 2 941 | 800 |
| ABC 2.1 `--abc-gregorian` | 87.06 % | **99.59 %** | 109 292 / 109 738 | 4 319 | 446 |
| MEI 5.1 NEUMES | 98.90 % | **99.94 %** | 113 146 / 113 213 | 844 | 67 |

The "every destination ≥ 95 %" conclusion of this section was **rejected by
review**: the readable labels were GABC dumps, not carriers. All conversions
succeeded (0 failures on 261 sources). The
final raw figures are 89.21 % (MusicXML), 69.82 % (MIDI), 96.72 % (LilyPond),
95.82 % (ABC) and 99.20 % (MEI). MEI's residual 67 defects are `glyph` 25,
`custos` 22, `bar` 7, `letter` 5, `pitch` 5, `attribute` 2, `clef` 1; the
844 inherent facts are exact inter-group whitespace, which MEI 5.1 has no
element for and whose `@label` carrier is stripped as source-private.

## 26. Review correction: dumps are not carriers — honest final figures (2026-11-27)

### 26.1 What was rejected and what changed

The reviewer measured `gregorio_FactusEst.gabc` and found
`--xml-full-support`: 41 hidden `gabc:` fields, 116 `letter:`, 35 `glyph:`,
63 `syllable-text:`; `--abc-gregorian`: **0** `w:` lines, `"^text: …"` instead,
116 `"^glyph: …"`; `--ly-gregorian`: **0** `\lyricmode`/`\addlyrics`, 116
`^"glyph: …"` including empty `^"glyph: "`. A `key: <raw GABC token>` label is
a **dump**: no other tool gives it a meaning, and in ABC/LilyPond it prints
garbage over the notes. Hiding it with `print-object="no"` does not turn it
into a standard carrier.

The branch now:

1. Writes lyrics in the destination's lyric carrier — ABC `w:` (§5, one `w:`
   line per sung syllable), LilyPond `\new VaticanaLyrics \lyricsto` (§2.1.2),
   MusicXML `<lyric><syllabic><text>` — never in annotations.
2. Carries the pitch letter by the pitch itself (and the GABC clef by ABC's
   native `K:… clef=C3` parameter and LilyPond's `\clef "vaticana-do3"`), never
   by a `letter:` label.
3. Uses only the destination's named neume vocabulary: SMuFL `chant*` glyph
   names from the SMuFL 1.4 Medieval/Renaissance plainchant ranges, gregorian.ly
   commands (`\quilisma`, `\oriscus`, `\virga`, `\stropha`, `\inclinatum`,
   `\liquescentia`, `\ictus`, `\episemInitium`/`\episemFinis`, `\semicirculus`,
   `\circulus`, `\accentus`, `\augmentum`, …), ABC 2.1 decorations. A fact with
   no such carrier is a defect, or inherent only with a citation of the
   destination's norm (the existing `INHERENT` table is unchanged).
4. Removes the dump carriers from writers and readers and their docs:
   `chantNote` MEI annotations, `chantHeader*`, `N:key=value`, `I:clef=`,
   `"^text:"`, `"^glyph:"`, `letter:`/`glyph:`/`syllable-text:` `other-technical`.
   `scripts/oracle/gabc/full_support.py` strips every such label, every `gabc`
   substring in a carrier and every empty annotation **before** measurement,
   and raises `ValueError` if one remains in the measured output.
   `scripts/oracle/gabc/test_full_support.py` unit-tests the stripper and, with
   `SCORETK_CLI` set, checks FactusEst in all five destinations: native lyric
   carrier present, no dump written, no residual after stripping.
5. Keeps the historical private `gabc:` fields of `master` (they are stripped
   by the oracle, never counted) so that **all default bytes stay identical**:
   455/455 default conversions on the 65 corpus sources (`v3`, `v4`, `gabc`)
   match a CLI built at `c5b52552` byte for byte.

### 26.2 Honest figures, before vs after

Same 261 sources, 114 057 facts per destination, same inherent citations,
0 conversion failures. "Before" is the rejected branch figure of §22–§25;
"After" is the current tree. "Main" is `carried / (total − inherent)`, "raw"
is `carried / total`.

| Destination | Before (padded) | After (honest) | Carried / headline total | Raw | Inherent | Defects |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| MusicXML 4.0 | 98.05 % | **90.57 %** | 93 664 / 103 418 | 82.12 % | 10 639 | 9 754 |
| MIDI SMF | 95.45 % | **95.45 %** | 79 638 / 83 438 | 69.82 % | 30 619 | 3 800 |
| LilyPond `--ly-gregorian` | 99.28 % | **94.48 %** | 104 668 / 110 789 | 91.77 % | 3 268 | 6 121 |
| ABC 2.1 `--abc-gregorian` | 99.59 % | **98.07 %** | 107 564 / 109 676 | 94.31 % | 4 381 | 2 112 |
| MEI 5.1 NEUMES | 99.94 % | **96.09 %** | 108 637 / 113 053 | 95.25 % | 1 004 | 4 416 |

Three destinations reach the 95 % target; **LilyPond (94.48 %) and MusicXML
(90.57 %) do not**, and the honest number is reported rather than padded.

### 26.3 Residual defects by fact (what blocks 95 %)

| Destination | Defects, largest first |
| --- | --- |
| MusicXML | `glyph` 4 093, `letter` 3 390, `rubric` 795, `shape` 537, `pitch` 427, `attribute` 332, `ictus` 105, `text` 29, `custos` 22, `mora` 16, `clef` 8 |
| MIDI | `pitch` 1 630, `text` 837, `rubric` 795, `header` 538 |
| LilyPond | `glyph` 3 878, `header` 783, `pitch` 411, `shape` 357, `attribute` 332, `rubric` 303, `custos` 22, `text` 11, `ictus` 9, `clef` 8, `letter` 7 |
| ABC | `header` 783, `pitch` 416, `attribute` 332, `rubric` 301, `glyph` 227, `text` 24, `custos` 21, `clef` 8 |
| MEI | `glyph` 2 877, `letter` 397, `pitch` 385, `attribute` 332, `nabc` 190, `rubric` 142, `text` 39, `shape` 23, `custos` 22, `bar` 9 |

**LilyPond is blocked by `glyph` 3 878**: the Vatican style (Notation Reference
§17.4.7) names neume shapes but has no command for the exact GABC spacing and
fusion spelling (`/`, `!`, `*`, digits); copying that spelling into a
`^"glyph: …"` annotation was the rejected padding. Source headers (`header`
783) have no native `\header` field beyond `title`, which is carried. The
`header`/`rubric`/`attribute` rows of §21.3 remain defects wherever no
destination-specific carrier was implemented.

**MusicXML is blocked by `glyph` 4 093 and `letter` 3 390**: one SMuFL name per
note cannot spell a multi-character GABC glyph, and the clef+pitch projection
does not restore every staff position; `rubric` 795 markup is not emitted in
`<words>`. All three are genuine absences of a *usable* standard carrier, not
reclassified inherent rows.

ABC regained **2481 bar facts** through real ABC bar symbols (`|`, `||`, `[|`,
`::`, §4.8) instead of `"^bar: ,"` annotations, and the native `T:`/`K:`
carriers restored 261 title facts in ABC and LilyPond. MEI's former `text`
14 990 apparent loss was an instrument bug (a private `label` attribute caused
the whole `<syl>` to be stripped); it is fixed — only the attribute is removed.

### 26.4 Non-regression

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo test --workspace --no-fail-fast`, `cargo test -p scoretk --test
e2e_corpus -- --nocapture` (8/8), `cargo build -p scoretk-wasm --target
wasm32-unknown-unknown` and the instrument test (6/6 with `SCORETK_CLI`) all
pass. New files are added by path; no build directory is versioned.

## 27. Native carriers for the two remaining destinations (2026-11-27, branch `gabcxl`)

The honest figures of §26 left MusicXML at 90.57 % and LilyPond at 94.48 %.
This campaign works the residual defect families in decreasing size, using
only carriers that a third-party consumer of the destination can interpret.
No `key: <raw GABC token>` label, hex/base64 payload, `gabc`-prefixed field,
lyric outside a lyric carrier or hidden free-text dump was introduced; the
oracle still strips the historical private carriers and raises `ValueError`
if one survives in the measured output. A review of this branch rejected the
`<words>` carrier of `c3c6919f` as a disguised private flag (§27.1), so the
final MusicXML figure is the honest 94.36 %, not the padded 95.35 %.

### 27.1 MusicXML: SMuFL sequences and notehead case — the rubric `<words>` carrier is rejected

`musicxml-4.0.xsd` `other-notation/@smufl` (`:5361-5375`, attribute group
`:2183-2188`) uses canonical SMuFL 1.4 glyph names and states that the
attribute allows *application interoperability*. `other-notation` is
`maxOccurs="unbounded"`, so a compound GABC glyph is written as a **sequence**
of named glyphs (`chantPunctum`, `chantPunctumInclinatum`, `chantQuilisma`,
`chantPunctumVirga`, `chantStrophicus`, `chantAugmentum`, `chantIctusAbove`,
`chantEpisema`, `chantAuctumDesc`, `accidentalFlat`, …) in source order. The
reader concatenates the inverse names into the glyph. The first name is the
notehead: `chantPunctum` (square punctum, lowercase GABC letter) versus
`chantPunctumInclinatum` (oblique punctum, uppercase letter). Clef +
`pitch/step` + notehead therefore restore the exact staff position.

**Rejected by review (2026-11-27).** Commit `c3c6919f` wrote one italic red
`<words>` per syllable plus an italic blue blank for the separator, decoded
by colour by our reader only. On `gregorio_FactusEst.gabc` this produced 103
`<words>` elements, duplicating every `<lyric>` a second time above the staff
and printing blanks. FactusEst has no rubric for those syllables. This is the
same private-carrier class as §22–§25. The writer and reader no longer use it;
`<words>` is reserved for text the GABC source actually prints (`[alt:…]`
above-staff text, rubric markup), with its printed value. The exact syllable
spelling stays in `<lyric>/<text>` plus the XSD font/elision attributes when
they apply, otherwise it is a defect. `scripts/oracle/gabc/full_support.py`
now raises `ValueError` if a measured `<words>` is blank, duplicates the
`<lyric>` of the note it precedes, or if colour is the only difference
between two carriers; `test_full_support.py` checks this on FactusEst and on
synthetic documents. `<direction>/<words>` (`:3495`, `formatted-text`
`:2610`) is still used for genuine printed text, never as a flag.

### 27.2 LilyPond: user-defined `\header` fields and a cited inherent glyph loss

The *Notation Reference* §21.2.3 ("User-defined field", `:48597`) allows
arbitrary variables in `\header`. Each GABC header is now a named LilyPond
field with its literal value (`title` for `name`, `office_part` for
`office-part`, `mode`, `book`, `transcriber`, `staff_lines`, …). This is a
readable header field, not a `name=value` copy of a source line and not a
private prefix.

The remaining largest LilyPond family, GABC spacing/fusion glyphs (`/`, `!`,
`*`, `@`), has no native carrier. The *Notation Reference* §17.4.7
(`:44630-44675`) states that support for Gregorian square neumes is limited,
that "essential issues for serious typesetting are still lacking, such as
horizontal alignment of multiple ligatures", and that ligature appearance is
not directly dictated by the input commands. These 2 705 facts stay in
`INHERENT_VALUES["ly"]["glyph"]` with that citation; no other inherent row was
added or changed. `-` was initially included in this row but the audit of
§27.5 found a real carrier: §17.4.7 names `\[ \deminutum g \pes b \]` a "Pes
Initio Debilis", so the writer emits `\deminutum` when `-` opens a multi-note
ligature and the reader restores the group token at its exact source
position. The inherent pattern was narrowed from `^[/!*@\-]+$` to
`^[/!*@]+$`; §27.5 justifies every remaining character against Gregorio's
`Gabc.tex` and the LilyPond documentation.

### 27.3 Measured results (261 sources, same instrument, 0 failures)

| Destination | Before this campaign | Branch peak before review | Final (honest) | Carried / headline total | Inherent | Defects |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| MusicXML 4.0 | 90.57 % | 95.35 % | **94.36 %** | 97 812 / 103 657 | 10 400 | 5 845 |
| LilyPond `--ly-gregorian` | 94.48 % | 97.58 % | **97.58 %** | 105 464 / 108 084 | 5 973 | 2 620 |

The MusicXML peak was padded by the `c3c6919f` `<words>` carrier; the
LilyPond peak came from genuine `\header` and §17.4.7 carriers (§27.2).
The MusicXML carrier rejected by review is not counted: `rubric` 795 and
`text` 29 are back in the defect column (raw 85.76 %), so the honest figure
is **below the 95 % target**. The blocking facts, with their counts, are
`glyph` 3 533, `rubric` 795, `pitch` 427, `shape` 353, `letter` 337,
`attribute` 332, `text` 29, `custos` 22, `ictus` 9 and `clef` 8 (sum 5 845).
No fact was reclassified as inherent to close the 663-fact gap to 95 %:
the syllable spelling belongs in `<lyric>` or is a defect, and `<words>`
may only print text that the source really prints. The true SMuFL/clef
gains remain (`glyph` 4 093 → 3 533, `letter` 3 390 → 337, `shape` 537 → 353,
`ictus` 105 → 9).

LilyPond: `header` 783 → 0 and `glyph` 3 878 → 1 160 with 2 705 inherent
under the §17.4.7 citation and 13 carried by the "Initio Debilis"
`\deminutum` carrier of §27.5 (2 718 → 2 705 inherent, +13 carried).
The `pitch`, `shape`, `attribute`, `custos` and `clef` families are unchanged
and still reported as defects.

### 27.4 Non-regression

All options are additive: without `--xml-full-support` / `--ly-gregorian` the
previous bytes are produced. The corpus comparison against master
(`019bc767`) covers every default conversion of the 65 versioned sources and
the 261-file GABC lot. `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace --no-fail-fast`, `cargo test -p scoretk --test
e2e_corpus -- --nocapture`, the wasm32 build of `scoretk-wasm` and the
instrument test (11/11 with `SCORETK_CLI`) are part of the final verification.
Default bytes against master (`019bc767`): 455/455 conversions of the 65
versioned sources (7 destinations) are byte-identical; on the 261-file lot,
261/261 GABC round trips,
260/260 headerless bodies (one invalid body excluded with the Gregorio
citation), 6/6 NABC bodies, and 1 305/1 305 default conversions of the other
four formats byte-identical on the 261-file lot.

### 27.5 Why the LilyPond glyph row stays inherent, character by character

The former `ly`/`glyph` inherent row `^[/!*@\-]+$` of §27.2 was audited
against Gregorio's own documentation and LilyPond's *Notation Reference*.
`-` was wrongly included and is now carried (below); none of the other
characters has a gregorian.ly command that a third-party reader could
interpret:

| GABC token | Meaning in Gregorio | LilyPond §17.4.7 |
| --- | --- | --- |
| `/` | small separation between neumes (`Gabc.tex:668-687`, "Neume Spacing") | no spacing command; "horizontal alignment of multiple ligatures … [is] lacking" |
| `//` | medium separation between neumes (`Gabc.tex:668-687`) | same absence; the note/ligature commands carry no inter-neume space |
| `!` | alone, a zero-width space when the note chain does not break correctly; before a space, makes it non-breaking (`Gabc.tex:681-687`) | `\noBreak` (§"Line breaking") is a line-break toggle, not a glyph or a spacing value; the *Notation Reference* §17.4.7 lists no equivalent. The review's "`!` fusion = ligature" example is inaccurate: fusion is `@`. |
| `*` | lyric special character produced by `\GreStar` (`Gabc.tex:345-378`, "Special Characters"); the instrument counts a stand-alone `*` lyric syllable as a `glyph` fact | §17.4.7 defines `\versus`, `\responsum`, `\ij`, `\iij`, `\IJ`, `\IIJ` as section markers "for use in lyrics" — no star command |
| `@` | fuses two neume primitives, or suppresses the stem before a fusible primitive; `@[ … ]` fuses automatically (`Gabc.tex:603-666`, "Neume Fusion") | `\[ … \]` brackets notes into a ligature and `\pes`/`\flexa` join them, but §17.4.7 states that "the typographical appearance of ligatures is not directly dictated by the input commands" and that shape follows musical meaning. A third-party reader cannot recover the `@`/`@[` spelling from that output. |
| `-` | initio debilis before a pitch (`Gabc.tex:556-579`, "Complex Neumes") | **has a command**: §17.4.7 names `\[ \deminutum g \pes b \]` a "Pes Initio Debilis" and `\[ \deminutum a \pes b \flexa g \]` a "Torculus Initio Debilis". It is now written by `write_prefixes` when `-` opens a multi-note ligature and restored by the reader: `\deminutum` on the first note of such a ligature becomes `-`, on a later note it stays the liquescent `~` ("Epiphonus", `\[ g \pes \deminutum b \]`). The inherent pattern is narrowed to `^[/!*@]+$`. |

`\divisioMinima`…`\finalis` (§17.4.4) and the note-head modifiers
(`\quilisma`, `\virga`, `\stropha`, `\inclinatum`, `\oriscus`, `\deminutum`,
`\cavum`, `\linea`, …) carry the bar and shape facts, not the five remaining
spelling characters above; they are already used by the writer and are not
part of this row. No carrier was invented for the remaining rows: they stay
defects or inherent with the citation already printed by the instrument.

## 28. Printed rubrics, lyric typography and placed alterations (2026-11-27, branch `gabcxm`)

§27 left MusicXML at **94.36 %** (97 812 / 103 657; 10 400 inherent, 5 845
defects) and explicitly rejected the `c3c6919f` `<words>` carrier: it wrote one
italic red `<words>` per *syllable*, duplicating `<lyric>` and printing blanks.
This campaign keeps that rejection: `<words>` carries **only text the GABC
source actually prints**, with its printed value. Nothing is a `key: <raw GABC
token>` label, hex/base64 payload, `gabc`-prefixed field, hidden text dump or
colour flag. The instrument still raises `ValueError` on a blank `<words>`, a
`<words>` repeating the following `<lyric>`, or two carriers whose only
difference is `@color`; `test_full_support.py` passes 11/11.

### 28.1 Standard carriers used

| Family | Carrier | Norm |
| --- | --- | --- |
| printed special characters (`<sp>`) and verbatim text (`<v>`) | `<direction placement="above"\|"below"><direction-type><words color="#FF0000">` with the **printed** glyph (`V/.`, `✶`, `‡`, …), never the tag | `musicxml-4.0.xsd` `direction-type/words` (:3495), `formatted-text` (:2610), `text-formatting` (:2237), `print-style/default-y` (:2134); Gregorio `doc/Gabc.tex:297-309,345-378` defines what those tags print |
| rubric markers without a sung note (`5.`, `V/. Et`, `{ia}.`) | the same `<words>`, with `placement="above"`; the reader opens a separate, note-less syllable at that point | `musicxml-4.0.xsd` `words` (:3495); `Gabc.tex:241-245,297-343` |
| whole-syllable style (`<i>`, `<c>`, `<b>`, `<tt>`, `<ul>`) | typography of `<lyric><text font-style="italic" color="#FF0000" …>` | `musicxml-4.0.xsd` `text-formatting` (:2237), `font` (:2005-2012); `Gabc.tex:254-270`. Partial tags have no single-`<text>` carrier and stay defects |
| non-sung alteration glyphs `x`/`#`/`y` (`Gabc.tex:481-489,512-540`) | `<direction><direction-type default-y="25"><symbol>accidentalFlat</symbol>`: a canonical SMuFL accidental at the staff height of the sign | `musicxml-4.0.xsd` `direction-type/symbol` (:3500), `formatted-symbol-id` (:2598), `smufl-glyph-name` (:411), `smufl-accidental-glyph-name` (:418), `default-y` (:2134). The inverse maps tenths → GABC staff letter |
| word scope of an alteration | `<lyric><syllabic>` (`single`/`end` closes the word) | `musicxml-4.0.xsd` `syllabic` (:5062); `Gabc.tex:212-227,512-540`. Without it a flat leaks into the next word and pitches regress |

`<words>` values are the printed characters only: the reader refuses a value
containing `<` (a source tag dump) and a colour other than the printed red.
Above-staff text `[alt:…]` keeps its own `<words>` with `font-style="italic"`
(`Gabc.tex:339-343`), so colour is never the only difference between two
carriers. All new XML attributes are gated by `gabc_neume_smufl` /
`--xml-full-support`: every default byte is unchanged.

### 28.2 Measured progression (261 sources, same instrument, 0 failures)

| Step | Carried | Main % | Rubric | Pitch | Letter | Shape | Attribute |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| §27 baseline | 97 812 | 94.36 | 795 | 427 | 337 | 353 | 332 |
| printed `<words>` | 97 962 | 94.51 | 551 | 427 | 337 | 353 | 332 |
| + placement of note-less rubrics | 98 179 | 94.72 | 449 | 427 | 337 | 353 | 332 |
| + lyric typography | 98 286 | 94.82 | 342 | 427 | 337 | 353 | 332 |
| + printed braces replacement | 98 331 | 94.86 | 297 | 427 | 337 | 353 | 332 |
| + placed alteration `<symbol>` | 98 994 | 95.50 | 297 | 665 | 36 | 52 | 332 |
| + `<syllabic>` word scope | **106 625** | **96.25** | 297 | 158 | 36 | 52 | 332 |

The intermediate pitch regression after adding `<symbol>` alone (427 → 665)
was measured, not hidden: the sign leaked across word boundaries because the
standard-only reader had no word scope. Restoring that scope from the XSD
`syllabic` carrier removed the leak. No fact was reclassified as inherent:
the `layout` row fell from 9 925 to 2 801 because the returned GABC now has
the real word breaks (carried by `syllabic`), so the headline denominator rose
from 103 657 to 110 781 and the raw figure from 85.76 % to 93.48 %.

Final defects (4 156): `glyph` 3 227, `attribute` 332, `rubric` 297, `pitch`
158, `shape` 52, `letter` 36, `custos` 22, `text` 15, `ictus` 9, `clef` 8.
Only a native carrier was used per family: the remaining exact GABC glyph
spelling (`/`, `!`, `x` inside a compound glyph) has no SMuFL name and is left
as a defect, as required by §27.1.

### 28.3 Non-regression

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo test --workspace --no-fail-fast`, `cargo test -p scoretk
--test e2e_corpus -- --nocapture` (8/8), `cargo build -p scoretk-wasm
--target wasm32-unknown-unknown`, `test_full_support.py` (11/11 with
`SCORETK_CLI`) and the instrument (261 sources, 0 failures) all pass. A
CLI built at `253106ce` and the new CLI produce **2 794/2 794 identical
default outputs** over the versioned corpus and the 261-file GABC lot; no
default conversion changed. New source files were added by path; no build
directory is versioned.
