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
