# GABC — reader/writer campaign (phase 0: references and oracle)

No scoretk GABC reader or writer exists yet. This page records the
**reference source**, the **measurement lot**, the **oracle** and its sanity
baseline. Gregorio is the only reference of truth. The architectural
decision (optional L6 Gregorian layer) is in
[model-modularisation.md](model-modularisation.md) §7.1 and awaits the user's
decision. Syntax and mapping are in [format-ref/gabc.md](format-ref/gabc.md)
and [format-mapping/gabc.md](format-mapping/gabc.md).

## 1. References and licences

All files are local, read-only and unversioned under
`/home/jacques/programmes/musique/specs/`, catalogued in `specs/README.md`
§ GABC / Gregorian chant. The only executable run is Gregorio's built C
parser; no downloaded script is imported or sourced.

| Source | Version | Licence | Use |
| --- | --- | --- | --- |
| Gregorio `doc/Gabc.tex`, `src/gabc/*.y/.l`, `src/dump/dump.c`, `src/struct.c`, `src/gregorio-utils.c` | `b63f074` ("6.2.0-develop") | GPL-3.0-or-later (`gregorio/COPYING.md`) | **syntax, parser, grammar and reference truth** |
| GregoBase SQL dump | `8851677e`, 2024-01-05 | **no site-wide licence statement** | corpus source, only `copyrighted = 0` rows, private reference |
| Gregorio `doc/`, `examples/` | `b63f074` | GPL-3.0-or-later | nine parsing exercises |

GregoBase has no licence grant; `build_lot.py` uses the per-entry
`gregobase_chants.copyrighted` flag as the only filter and keeps the lot
outside the repository. The extracted corpus is private reference data and
must not be redistributed.

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
Every divergence is classified by the failing check and written to
`mesure.jsonl`; nothing is silently dropped.

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
- **Our reader vs Gregorio: 132/197 core-exact.** The 65 residual files are
  classified in `mesure.jsonl` by `premier_divergence`: pitch 18, text 24,
  bars 23 (signs-only differences are N2). These are defects in **our**
  parser and are the work list for the future reader: bracketed `[…]` layout
  tags inside note code, clef changes embedded in a bar group, and markup
  normalisation are the main causes (see `canonical.py` and
  `docs/agents/format-ref/gabc.md` §9 pitfalls 5 and 6).

No converter is measured yet: this baseline only establishes that the
reference is deterministic and records what our source reader still gets
wrong.

## 6. How to run

See `scripts/oracle/gabc/README.md` § Usage: build the lot, build Gregorio,
run `--selftest`, run the full measurement; report gains and losses **per
file** as for the other nine directions (`docs/agents/plan-fiabilite.md`).
