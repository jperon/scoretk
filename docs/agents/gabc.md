# GABC — reader/writer campaign (phase 0: references and oracle)

No scoretk GABC reader or writer exists yet. This page records the
**reference sources**, the **measurement lot** and the **oracle** prepared
before any pivot-model change. The architectural decision (optional L6
Gregorian layer) is recorded in
[model-modularisation.md](model-modularisation.md) §7.1 and awaits the user's
decision; nothing here assumes it has been taken. Syntax and mapping are in
[format-ref/gabc.md](format-ref/gabc.md) and
[format-mapping/gabc.md](format-mapping/gabc.md).

## 1. References and licences

All files are local, read-only and unversioned under
`/home/jacques/programmes/musique/specs/`, catalogued in `specs/README.md`
§ GABC / Gregorian chant. No script from them was executed except the
recorded `gabctk.py` CLI, run under `python3 -I` in a dedicated venv.

| Source | Version | Licence | Use |
| --- | --- | --- | --- |
| Gregorio `doc/Gabc.tex`, `src/gabc/*.y/.l` | `b63f074` | GPL-3.0-or-later | normative syntax and grammar |
| gabctk `gabctk.py`, bundled `abc2xml.py`/`midiutil` | tag `v240107`, `40b58b62` | **GPL-2.0-or-later** | modern-notation mapping and oracle reference |
| GregoBase SQL dump | `8851677e`, 2024-01-05 | **no site-wide licence statement** | corpus source, only `copyrighted = 0` rows, private reference |
| Gregorio `doc/`, `examples/` | `b63f074` | GPL-3.0-or-later | nine parsing exercises |

The GregoBase repository's README and SQL contain no licence grant; per-entry
`gregobase_chants.copyrighted` is the only usable filter (`scripts/oracle/gabc/build_lot.py`
keeps `copyrighted = 0`). The extracted corpus must stay outside the
repository and must not be redistributed. gabctk is GPL-**2.0-or-later**, so
the “or later” clause permits GPL-3.0-only reuse **with attribution** if any
of its logic is ever ported; note this in `NOTICE` and
`docs/licences-tierces.md` at that time.

## 2. Lot

`build_lot.py` parses the SQL dump as **data** (`csv` + `json`, no downloaded
code), keeps non-copyrighted chants, normalises them into self-contained
`.gabc` files (`name:`, `language: latin;`, `staff-lines: 4;`, optional
`mode:` — the four-line staff is required by gabctk's fixed table at
`gabctk.py:1214-1261`), and copies the nine Gregorio files. With
`--count 180 --stride 97` this yields **202 files** (193 GregoBase + 9
Gregorio) in `specs/gabc/lot/`, with `lot.tsv` (`id<TAB>path`). The manifest
is regenerable and deterministic.

## 3. Oracle design

`scripts/oracle/gabc/mesure.py` (companion `mesure.sh`, `README.md`):

1. For each source, run gabctk `python3 -I gabctk.py -i src -o ref.mid -x
   ref.xml -c ref.abc`. `-I` ignores ambient `PYTHONPATH`/user site; gabctk
   appends its own directory at `gabctk.py:22`, so its bundled modules still
   resolve. Failures are recorded, not fatal.
2. Extract notes with the existing shared extractor
   `scripts/oracle/midi/midi_notes.py:60` (`read_midi`) and `:247`
   (`read_xml`), and compare with `:658` (`compare`) to obtain N1 (pitch
   multiset), N2 (pitch + onset) and N2d (plus duration).
3. `cmp_ref` is **gabctk MusicXML vs gabctk MIDI** — a characterisation of
   the reference, not a scoretk result. `n1 = ref` is expected; `n2 < ref`
   is reference noise from gabctk's two independent exporters
   (`gabctk.py:1390-1410` for MusicXML via ABC→`abc2xml`, `:1413-1471` for
   MIDI with `int(duree)` truncation).
4. Determinism is checked on the first three files by running gabctk twice
   and comparing SHA-256 of both outputs.
5. Import direction (later): `--scoretk <cli>` converts the same `.gabc` to
   MusicXML/MIDI and `cmp_import_*` compares each to the matching reference.
6. Export direction (later): `--roundtrip --refs <dir>` compares a GABC file
   **exported by scoretk** against references produced from the original.
   The writer must preserve the L6 neume payload; the derived projection is
   not a round-trip target (see §4).

Parallelism is capped: `--jobs` defaults to `min(2, os.cpu_count())` and the
pool is clamped to two workers. No other measurement runs concurrently.

## 4. Measured sanity baseline (2026-11-23)

Command (full lot, two workers, determinism check):

```sh
scripts/oracle/gabc/mesure.sh \
  /home/jacques/programmes/musique/specs/gabc/lot/lot.tsv /tmp/gabc-oracle-full \
  /home/jacques/programmes/musique/specs/gabc/gabctk/gabctk.py - \
  --verifier-determinisme --jobs 2
```

Result: **202 files, 148 references produced, 54 gabctk failures**;
`n1 = ref` on **130/148**; **0 fully exact** (N2 mean 40.2 %); determinism
**3/3** on the checked files. The earlier 12-file sample gave 12/12
references, 9/12 N1-exact, 3/3 deterministic — same pattern.

Failure causes (gabctk limits, not scoretk): `IndexError` 23 (fixed ABC pitch
table `gabctk.py:1186-1194` indexed by `hauteur - 58`), `UnicodeEncodeError`
14 (`abc2xml.py` latin-1 output), `KeyError` 12 (bar `` ` `` missing from
`gabctk.py:928-936`), 5 others. N1 mismatches are also the out-of-range ABC
pitch table: on `gb195`, two notes are pitch 60 in MIDI and 96 in MusicXML,
i.e. a 36-semitone transposition wrapped to the wrong tuple index. Those
rows remain in `mesure.jsonl` with their flags; none is silently discarded.

**Consequence:** the oracle can serve as a musical-N1/N2 comparator, but
scoretk must be measured against the **MIDI** reference for pitch and onset
and against the MusicXML reference for written notes, with `cmp_ref` quoted
alongside every figure so reference noise is never read as a regression.

## 5. How to run

See `scripts/oracle/gabc/README.md` § Usage. In short: build the lot, run
`--selftest`, run the full measurement, then (once a reader exists) add
`--scoretk <binary>`; for the writer, use `--roundtrip --refs`. Always report
gains and losses **per file**, never only totals, as for the other nine
directions (`docs/agents/plan-fiabilite.md`).
