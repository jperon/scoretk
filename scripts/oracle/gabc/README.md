# GABC oracle — Gregorio is the reference

Versioned drivers for the GABC measurement lot. Downloaded sources are
**untrusted data**: no downloaded script is imported; the two executables
(Gregorio's built C parser and gabctk's recorded `gabctk.py`) are run as
subprocesses, gabctk under `python3 -I` in its own venv. The corpus and all
outputs stay outside the repository. See `docs/agents/gabc.md`.

## Why the old oracle gave 0 % N2

The previous sanity run compared gabctk's **MIDI** with its **MusicXML**.
That MusicXML is not a GABC export: `gabctk.py:1390-1410` generates ABC and
calls the bundled `abc2xml.py`; the MIDI writer uses `int(note.duree)` and a
per-note tempo trick (`:1413-1471`). GABC has **no fixed durations**
(`docs/agents/format-ref/gabc.md` §7), so the two exporters disagree on
onsets by construction. A 100 % note/onset match there is neither expected
nor meaningful.

## Truth and references

`canonical.py` defines the truth on the invariants GABC actually carries:

1. pitch sequence with the **clef applied** — Gregorio's staff heights
   (`src/struct.h:809`, `src/dump/dump.c:78-88`) and clef key
   (`src/struct.c:1273-1288`);
2. syllable/lyric text;
3. neume grouping (compared between the GABC source and gabctk's `-m`);
4. separation bars;
5. rhythmic signs as optional N2.

Primary reference: **Gregorio** `b63f074`, built as described below,
invoked as `gregorio -f gabc -F dump`. Secondary: our own source reader
(diagnostic). Candidate: gabctk tag `v240107`. Gregorio prevails on
disagreement.

## Build the reference parser

```sh
scripts/oracle/gabc/build_gregorio.sh
# prints /home/jacques/programmes/musique/specs/gabc/gregorio-buildtree/src/gregorio-6_2_0
```

The build uses `nix-shell -p autoconf automake flex bison`, `-j2`, in its own
directory. `build.sh` is not used (it requires a TeX/PDF toolchain); the C
binary is built directly with configure/make. Set `GABC_GREGORIO` to override
the binary passed to `mesure.sh`.

## Usage

```sh
# Build the lot (our csv+json parser; no downloaded code executed).
python3 scripts/oracle/gabc/build_lot.py \
  --sql /home/jacques/programmes/musique/specs/gabc/gregobase/gregobase_online.sql \
  --gregorio /home/jacques/programmes/musique/specs/gabc/gregorio \
  --out /home/jacques/programmes/musique/specs/gabc/lot --count 180 --stride 97

# Selftest (no manifest: '-').
scripts/oracle/gabc/mesure.sh - /tmp/gabc-selftest \
  /home/jacques/programmes/musique/specs/gabc/gabctk/gabctk.py - --selftest

# Full sanity: Gregorio against itself + gabctk against Gregorio.
scripts/oracle/gabc/mesure.sh \
  /home/jacques/programmes/musique/specs/gabc/lot/lot.tsv /tmp/gabc-oracle \
  /home/jacques/programmes/musique/specs/gabc/gabctk/gabctk.py - \
  --verifier-determinisme --jobs 2
```

The 4th argument is the venv path (`-` → `<out>/.venv`). At most two workers
(`--jobs`, default `min(2, os.cpu_count())`); never run two measurements at
once. No scoretk reader/writer is involved yet.

## Outputs

- `<out>/travail/<id>/ref.dump`, `again.dump` — Gregorio dumps;
- `<out>/travail/<id>/cand.gabc`, `cand.txt` — gabctk re-emission;
- `<out>/mesure.jsonl` — `cmp_source` (diagnostic), `cmp_gabctk`
  (classification), `cmp_grouping` (source vs gabctk), `gregorio_deterministe`;
- `<out>/resume.md` — totals and per-file first divergence.
