# GABC — reader/writer campaign (phase 0: references and oracle)

No scoretk GABC reader or writer exists yet. This page records the
**reference sources**, the **measurement lot**, the **corrected oracle** and
its sanity baseline. The architectural decision (optional L6 Gregorian
layer) is in [model-modularisation.md](model-modularisation.md) §7.1 and
awaits the user's decision; nothing here assumes it has been taken. Syntax
and mapping are in [format-ref/gabc.md](format-ref/gabc.md) and
[format-mapping/gabc.md](format-mapping/gabc.md).

## 1. References and licences

All files are local, read-only and unversioned under
`/home/jacques/programmes/musique/specs/`, catalogued in `specs/README.md`
§ GABC / Gregorian chant. Only the two recorded executables are run; no
downloaded script is imported or sourced.

| Source | Version | Licence | Use |
| --- | --- | --- | --- |
| Gregorio `doc/Gabc.tex`, `src/gabc/*.y/.l`, `src/dump/dump.c`, `src/struct.c` | `b63f074` ("6.2.0-develop") | GPL-3.0-or-later (`gregorio/COPYING.md`) | **syntax, parser and reference truth** |
| gabctk `gabctk.py`, bundled `abc2xml.py`/`midiutil` | tag `v240107`, `40b58b62` | **GPL-2.0-or-later** (`gabctk/LICENSE`) | candidate mapping, characterised against Gregorio |
| GregoBase SQL dump | `8851677e`, 2024-01-05 | **no site-wide licence statement** | corpus source, only `copyrighted = 0` rows, private reference |
| Gregorio `doc/`, `examples/` | `b63f074` | GPL-3.0-or-later | nine parsing exercises |

GregoBase has no licence grant; `build_lot.py` uses the per-entry
`gregobase_chants.copyrighted` flag as the only filter and keeps the lot
outside the repository. gabctk is GPL-2.0-**or-later**, so the "or later"
clause permits GPL-3.0-only reuse **with attribution** if any logic is ever
ported; record it in `NOTICE` and `docs/licences-tierces.md` then.

## 2. Lot

`build_lot.py` parses the SQL dump as **data** (`csv` + `json`), normalises
non-copyrighted chants into self-contained `.gabc` files (`name:`,
`language: latin;`, `staff-lines: 4;`, optional `mode:` — the four-line staff
is required by gabctk's fixed table at `gabctk.py:1214-1261`), and copies the
nine Gregorio files. With `--count 180 --stride 97` this yields **202 files**
(193 GregoBase + 9 Gregorio) in `specs/gabc/lot/`, with a deterministic
`lot.tsv`.

## 3. Why the first oracle was wrong

The earlier `mesure.py` compared gabctk's **MIDI** (`-o`) with its
**MusicXML** (`-x`) and reported `n1 = ref` on 130/148 and **0 fully exact,
N2 ≈ 40 %**. That is not a defect of either exporter:

- the MusicXML is produced through **ABC → `abc2xml.py`**
  (`gabctk.py:1390-1410`), with `M: none` and `L: 1/8` (`:38-48`);
- the MIDI truncates durations with `int(note.duree)` and writes a tempo per
  note (`gabctk.py:1443-1454`);
- **GABC has no fixed durations** at all (`docs/agents/format-ref/gabc.md` §7;
  rhythmic signs at `Gabc.tex:742-797`).

Comparing the two therefore measures a *duration interpretation* that GABC
does not contain. It cannot be a 100 % sanity test and was removed as a truth
comparator.

## 4. Corrected oracle: Gregorio is the reference

`scripts/oracle/gabc/canonical.py` defines the truth on what GABC carries:

1. **pitch sequence with the clef applied** — Gregorio's staff heights
   (`src/struct.h:809-813`), its `o → p` letter mapping (`src/dump/dump.c:78-88`)
   and its clef key `2*line-1` (C) / `2*line-4` (F) (`src/struct.c:1273-1288`),
   giving `step = height - key - 5` with C = 0;
2. **syllable/lyric text**, normalised the way both tools print it (braces,
   TeX protrusion commands and repeated `~` removed);
3. **neume grouping** — GABC's parenthesised groups, compared between our
   source reader and gabctk's `-m` (Gregorio dumps layout *glyphs*, which are
   a different granularity: 109 glyphs versus 34 GABC groups on `gb1`);
4. **separation bars**, normalised to Gregorio's `B_*` names
   (`src/dump/dump.c:805-821`);
5. **rhythmic signs** (punctum mora, ictus, liquescents) as optional N2.

Primary reference: Gregorio's own C parser, built by
`build_gregorio.sh` with `nix-shell -p autoconf automake flex bison`, `-j2`,
in `specs/gabc/gregorio-buildtree/`, and invoked as
`gregorio -f gabc -F dump` (`src/gregorio-utils.c:184-218`). Secondary
reference: our own reader of the source (diagnostic only). Candidate:
gabctk's `-m` re-emission plus `-e` lyrics. Where they disagree, Gregorio
prevails. Each divergence is classified by the failing check and written to
`mesure.jsonl`; nothing is silently dropped.

Parallelism is capped at two workers (`--jobs`, default
`min(2, os.cpu_count())`); no other measurement runs concurrently.

## 5. Sanity baseline (2026-11-23, full lot, two workers)

```sh
scripts/oracle/gabc/mesure.sh \
  /home/jacques/programmes/musique/specs/gabc/lot/lot.tsv /tmp/gabc-oracle-greg3 \
  /home/jacques/programmes/musique/specs/gabc/gabctk/gabctk.py - \
  --verifier-determinisme --jobs 2
```

- **Gregorio vs itself: 197/197 = 100 %** (SHA-256 of two dumps per valid
  file). Five files are rejected by Gregorio itself (`gb6036`, `gb8251`,
  `gb9426`, `gb11321`, `gb18361`: "clef change is not supported on the first
  syllable", "syntax error", "unrecognized character u") and are excluded
  from both sides — they are bad corpus data, not converter failures.
- Our source reader vs Gregorio (diagnostic): **132/197** core-exact; the
  residual differences are layout tags `[…]` inside note code, clef changes
  embedded in a bar group, and markup normalisation — classified, not hidden.
- **gabctk vs Gregorio: 122/163 core-exact** (36 fully exact including signs).
  Disagreements: signs only 86, bars 23, pitch 10, text 8. Neume grouping
  (source vs gabctk) agrees on 150/202.

### Gabctk defects found (for upstream fixing)

Where gabctk is the candidate, these are defects **in gabctk**, not in the
format or in Gregorio:

1. **Dropped note after a clef change inside a bar group** — `gb98`: at note
   index 124 Gregorio has step `-4` (letter `d` under the new `c3` clef) that
   gabctk's `-m` omits; the rest of the sequence is shifted by one.
2. **Out-of-range ABC pitch table** — `gabctk.py:1186-1194` indexes a fixed
   tuple by `hauteur - 58`; GregoBase chants with very low/high tessiture wrap
   to the wrong pitch (also the source of the earlier 36-semitone anomaly on
   `gb195`).
3. **Extra separation bar** — `gb4276`: gabctk emits one more
   `DIVISIO_MINOR` than Gregorio (8 vs 7).
4. **Lyric marker reordering** — `gb1`: `-e` text has `.ij.*~` where Gregorio
   (and the source) have `.*ij.~`; the Euouae `*` marker is moved.
5. **Latin-1 encoder and unsupported bars** — `abc2xml.py` raises
   `UnicodeEncodeError` on `U+01FD`/`U+2020`, and `gabctk.py:928-936` has no
   mapping for the virgula `` ` `` (`KeyError`). These affect the legacy
   MusicXML/MIDI path, no longer the canonical oracle.

None of these blocks writing a scoretk reader: the reader must follow
Gregorio (the reference implementation of the format), and the oracle now
measures against it.

## 6. How to run

See `scripts/oracle/gabc/README.md` § Usage: build the lot, build Gregorio,
run `--selftest`, run the full measurement; report gains and losses **per
file** as for the other nine directions (`docs/agents/plan-fiabilite.md`).
Import/export hooks in `mesure.py` are intentionally absent until there is a
reader/writer to measure.
