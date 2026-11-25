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

**What is lost on a round trip** (and therefore measured honestly): header
comments, bracketed layout/tuning/TeX attributes stripped by
`strip_attributes` (`:689-700,908-931,1093-1118`), custos and line breaks
`z`/`Z` (`:872-906`), NABC cuts `|`, zero-width spaces `!`, within-group
ordering of bars versus notes, and exact line breaks/indentation. These are
not fields of the payload, so a byte comparison would have hidden their loss
by copying them; see §9 for the canonical results.

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
| Separation bars and their exact token | `<barline>/<bar-style>` for the modern view (`,` → `tick`, `;` → `short`, `:` → `light-light`, `::` → `light-heavy`); a `<breath-mark/>` on the last sung note; `gabc:syllable` `b=` fields for the exact tokens `;1`…`;8`, `::`, `:?`, … | XSD 4.0 `barline`, `breath-mark`; `Gabc.tex:801-845` (minima = quarter bar, minor = half bar, maior = full bar, finalis), `src/dump/dump.c:805-821` |
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
