# GABC oracle — Gregorio only

Versioned drivers for the GABC measurement lot. Gregorio is the **only**
reference of truth. Downloaded sources are untrusted data: the built C
parser is the only executable run, in its own build directory; no downloaded
script is imported or sourced. The corpus and all outputs stay outside the
repository. See `docs/agents/gabc.md`.

## Truth

`canonical.py` defines it on the invariants Gregorio's syntax actually
carries: pitch with the **clef applied** (`src/struct.h:809-813`,
`src/dump/dump.c:78-88`, `src/struct.c:1273-1288`), syllable text, separation
bars (`doc/Gabc.tex:801-845`), and rhythmic signs as optional N2
(`:742-799`). GABC has no fixed durations (`:742-845`), so MIDI/XML onsets
are never compared.

Two measurements, both against `gregorio -f gabc -F dump`:

* Gregorio against itself (SHA-256 of two dumps per valid file) — 100 %;
* our own source reader against Gregorio — the residual divergences are
  ours to fix, classified by failing check in `mesure.jsonl`.

## Build the reference parser

```sh
scripts/oracle/gabc/build_gregorio.sh
# /home/jacques/programmes/musique/specs/gabc/gregorio-buildtree/src/gregorio-6_2_0
```

`nix-shell -p autoconf automake flex bison`, `-j2`, own directory. Override
the binary by passing it as the 3rd argument to `mesure.sh`.

## Usage

```sh
python3 scripts/oracle/gabc/build_lot.py \
  --sql /home/jacques/programmes/musique/specs/gabc/gregobase/gregobase_online.sql \
  --gregorio /home/jacques/programmes/musique/specs/gabc/gregorio \
  --out /home/jacques/programmes/musique/specs/gabc/lot --count 180 --stride 97

scripts/oracle/gabc/mesure.sh - /tmp/gabc-selftest --selftest
scripts/oracle/gabc/mesure.sh \
  /home/jacques/programmes/musique/specs/gabc/lot/lot.tsv /tmp/gabc-oracle \
  --verifier-determinisme --jobs 2
```

At most two workers (`--jobs`, default `min(2, os.cpu_count())`); never run
two measurements at once. No scoretk reader/writer is involved yet.

## Outputs

`<out>/travail/<id>/ref.dump` and `again.dump` (Gregorio), `<out>/mesure.jsonl`
(`cmp_source`, `divergences`, `gregorio_deterministe`), `<out>/resume.md`.
