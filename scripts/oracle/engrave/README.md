# Geometric engraving oracle (P1 skeleton)

Compares the positions of noteheads, stems and bar lines produced by
`scoretk` with those exported by **MuseScore Studio 4.7.4** (the same
authoritative oracle as the other directions, `docs/agents/plan-fiabilite.md`).
Nothing from the reference is executed: `mscore` only reads corpus `.mscx`
files and writes SVGs into an unversioned output directory.

## Method

1. `lots.sh` selects the single-staff corpus files (`tests/corpus/v3`, `v4`)
   and writes their absolute paths to `<out>/lots/corpus.txt`.
2. `refs.sh` exports one SVG per file with `mscore -o`, at most **two in
   parallel** (`xargs -P 2`), under `timeout` with a neutralised
   `LD_LIBRARY_PATH` and `QT_QPA_PLATFORM=offscreen` — as the other oracles
   do. MuseScore appends `-1` to the output name for page 1.
3. `ours.sh` engraves the same files with `scoretk in.mscx -o out.svg
   --page 0`, also two in parallel, under `timeout 10 prlimit --as=2000000000`.
4. `compare.py` converts both SVGs to staff spaces using each document's own
   staff lines (reference: polylines; ours: rectangles). Each element is
   localised to its own five-line staff system before comparison: absolute y
   would compare notes from different systems and report system distances as
   pitch errors. It extracts notehead bounding-box centres, stem extents and
   bar-line centres, matches elements of the same kind, and reports:
   - `within_0_1_sp`: share of all expected elements within 0.1 staff space in
     **both** axes; unmatched elements and failures count in the denominator;
   - `vertical_pitch`: share of matched noteheads whose staff line agrees
     within 0.1 sp (pitch, ledger and stem-length agreement are reported
     separately from horizontal spacing, which is expected to differ until
     P2);
   - `stem_length`: share of matched stems whose length agrees within 0.1 sp;
   - per-file rows in `raw.jsonl` and a summary in `resume.md`;
   - measure widths from consecutive bar lines and the per-system measure
     counts (system breaks). Bar lines are grouped by **absolute** y: their
     local y repeats on every system of the page, so grouping by local y
     merged all systems and produced negative measure widths. Double/final
     bar lines (two MuseScore strokes 0.6-1.1 sp apart, two scoretk
     rectangles 0.37 sp apart) count as one boundary; a plain start bar
     line is not drawn by MuseScore and is not counted either.
   `worst.py <out> [count]` dumps the worst element-level differences (20 by
   default), unmatched elements included.

## Usage

```sh
# The CLI must be built with the engrave feature (enabled by default in the CLI):
nix develop -c cargo build -p scoretk-cli
OUT=/tmp/oracle-engrave
scripts/oracle/engrave/mesure.sh "$OUT" /home/jacques/tmp/scoretk/target-eng1/debug/scoretk
```

`mesure.sh --resume "$OUT"` only reruns `compare.py` on already produced
SVGs. `MSCORE` can be set to an explicit `mscore` binary, as in
`scripts/gen-references.sh`.
