# GABC oracle

Versioned drivers for the GABC measurement lot. Downloaded references are
**untrusted data**: no script from them is imported or executed except the
recorded `gabctk.py` entry point, which runs in its own venv under
`python3 -I`. The lot and its references are never committed.

## Sources (all outside the repository)

| Path | Version | Licence |
| --- | --- | --- |
| `/home/jacques/programmes/musique/specs/gabc/gabctk/gabctk.py` | tag `v240107`, `40b58b62` | GPL-2.0-or-later |
| `/home/jacques/programmes/musique/specs/gabc/gregorio/` | `b63f074` | GPL-3.0-or-later |
| `/home/jacques/programmes/musique/specs/gabc/gregobase/gregobase_online.sql` | `8851677e` (dump of 2024-01-05) | no site-wide statement; only rows with `copyrighted = 0` are used, private reference |
| `/home/jacques/programmes/musique/specs/gabc/lot/` | generated | private reference lot, do not redistribute |

See `specs/README.md` § GABC / Gregorian chant and `docs/agents/gabc.md`.

## Usage

```sh
# 1. Build the lot (our parser of the SQL dump; no downloaded code runs).
python3 scripts/oracle/gabc/build_lot.py \
  --sql /home/jacques/programmes/musique/specs/gabc/gregobase/gregobase_online.sql \
  --gregorio /home/jacques/programmes/musique/specs/gabc/gregorio \
  --out /home/jacques/programmes/musique/specs/gabc/lot --count 180 --stride 97

# 2. Selftest (no manifest needed: '-' stands for no list).
scripts/oracle/gabc/mesure.sh - /tmp/gabc-selftest \
  /home/jacques/programmes/musique/specs/gabc/gabctk/gabctk.py - --selftest

# 3. Sanity measurement: gabctk MusicXML vs gabctk MIDI on the full lot.
scripts/oracle/gabc/mesure.sh \
  /home/jacques/programmes/musique/specs/gabc/lot/lot.tsv /tmp/gabc-oracle \
  /home/jacques/programmes/musique/specs/gabc/gabctk/gabctk.py - \
  --verifier-determinisme --jobs 2
```

The 4th argument is the venv path; `-` selects `<out>/.venv`. Extra
arguments are passed to `mesure.py`. At most two workers are used
(`--jobs`, default `min(2, os.cpu_count())`), on a shared machine.

## Outputs

- `<out>/travail/<id>/ref.mid`, `ref.xml`, `ref.abc` — raw gabctk outputs;
- `<out>/mesure.jsonl` — one row per input, with `cmp_ref` (the reference
  self-comparison via `scripts/oracle/midi/midi_notes.py`) and, when a
  `--scoretk` CLI is given, `cmp_import_xml`/`cmp_import_mid`;
- `<out>/resume.md` — per-file table and totals.

`--roundtrip --refs <dir>` switches to the export direction: the manifest
then lists scoretk-*exported* `.gabc` files, and `<refs>/<id>/` holds the
`ref.mid`/`ref.xml` produced from the original sources. That mode is ready
but not exercised until a GABC writer exists.
