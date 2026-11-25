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
§ GABC / Gregorian chant. The only executable run is Gregorio's built C
parser; no downloaded script is imported or sourced.

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
a 100 % sanity test. It was removed as a comparator.

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
source is executed. Every divergence is classified by the failing check
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
- **Our Python source reader vs Gregorio: 132/197 core-exact.** The 65
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
**derived projection** (`derived::build_projection`): one `Note` per pitch
letter, quarter-note default duration and one nominal 4/4 measure per
syllable. `derived_modern_view = true` and a `derived-modern-view` loss
record make that choice explicit (`model-modularisation.md` §7.1).

## 8. Writer (`scoretk-gabc-export`)

`write_gabc(&Score) → String` emits the exact `header_source` + `notation`
stored by the reader. Because the reader keeps the bytes from before the
`%%` marker to the end of the file — comments, spaces and all — the round
trip is byte-identical. This is deliberate: spaces are music
(`Gabc.tex:60-61,668-687`) and the plan requires the L6 payload to be
written back verbatim when present. A score with no Gregorian payload is
refused (`Error::MissingGregorian`) instead of inventing chant from a
metrical L0/L1 score: durations and metre in that projection are not source
data (`:801-845`).

`Capabilities` declares L0+L1+L2+L3+L6 for both directions, cited from
`Gabc.tex:228-343` (text), `:408-443` (pitch), `:452-870` (neumes, signs,
bars, clefs) and the L6 slot of the model. L4 (no tempo/velocity header,
`:67-154`) and L5 (no page geometry, `:801-845`) are not declared.

## 9. Measured results (2026-11-25)

| Measurement | Command | Result |
| --- | --- | --- |
| Nine Gregorio examples, GABC → model → GABC | `cargo test -p scoretk --test gabc_corpus` | **9/9 byte-identical**, 100 % inventory match |
| Layer coverage on the nine examples | `cargo test -p scoretk --test layer_coverage -- --nocapture` | **essential 100 %, full support 100 %** on 3 160 items (L1 342, L2 1 034, L3 31, L6 1 753), declared L0+L1+L2+L3+L6 |
| Gregorio vs itself, full lot | `mesure.sh … --verifier-determinisme --jobs 2` | 197/197 deterministic (5 files rejected by Gregorio) |
| scoretk writer vs Gregorio dumps, full lot | `mesure.sh … --writer <cli> --jobs 2` | **writer core-exact 197/197 = 100 %, dump SHA identical 197/197**, 0 writer errors |
| Python source sanity reader (phase 0) | same run, `cmp_source` | 132/197 = 67.0 % (unchanged baseline, see §5) |
| Wasm package size | `nix build .#wasm`, `result/SIZE` | 2 394 775 raw / 939 561 gzip vs 2 379 470 / 931 754 at `a4053795`: **+15 305 raw (+0.64 %), +7 807 gzip (+0.84 %)**, total under the ≈ 2.5 MB cantus cap; below the +2 % budget |

No existing snapshot or `e2e_corpus` result changed: the new format is
additive (new crates, new format variant, L6 payload only on scores read
from GABC).

## 10. Limits

- GABC has no metre, no measure numbering and no fixed durations
  (`Gabc.tex:801-845`): L0/L1 notes and measures are a derived projection,
  not comparable to MuseScore's metric spine. GABC is therefore measured on
  its own corpus in `layer_coverage.rs`, not on `tests/corpus/{v3,v4}`.
- The derived concert pitch uses the first clef only; later clef changes
  stay in the L6 `clefs` list but are not re-applied to L0 notes
  (`Gabc.tex:847-870`; `gabc-score-determination.c:89-100`). This does not
  affect the oracle, which reads the verbatim GABC, nor the round trip.
- Export without an L6 payload is an error by design; MusicXML/MIDI → GABC
  is not supported because the durations and bars would be invented.
- The Python `canonical.parse_source` sanity reader is still at 132/197;
  improving it is independent of the Rust writer score (the writer is
  measured through Gregorio's own dump of its output).
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
5. never regenerate the lot from GregoBase with another filter or version
   without an explicit decision (§1).
