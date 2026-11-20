# MuseScore export oracle — `.mscz` reopened by MuseScore 4.7.4

Independent measurement of scoretk's MuseScore writer. The reference is
**MuseScore 4.7.4**: a score written by scoretk as `.mscz` is reopened by
MuseScore, which exports MusicXML and MIDI. Those outputs are compared with
scoretk's **direct** MusicXML and MIDI from the same source. No third-party
script or corpus file is copied into the repository: manifests and raw outputs
live outside it. Every binary is launched under `timeout` and
`prlimit --as=2000000000`.

## What is measured

For each source `S` of a lot:

1. `scoretk S --to mscz -o tk.mscz`;
2. `mscore -o ms.musicxml tk.mscz` and `mscore -o ms.mid tk.mscz`
   (`QT_QPA_PLATFORM=offscreen`, without the project shell's `LD_LIBRARY_PATH`);
3. `scoretk S --to musicxml -o tk.musicxml` and
   `scoretk S --to midi -o tk.mid`;
4. the alternative to beat: MuseScore's own import of `S` (`mscore -o
   alt.musicxml S`, i.e. its own `.mscz` for an mscz source, MusicXML/LilyPond/
   ABC import otherwise).

Three measurements follow:

- **Opened** — both `mscore` conversions of `tk.mscz` succeeded, with
  non-empty outputs; their stderr is kept (`journal`) and lines matching
  error/warning words are counted (`averts`).
- **Core fidelity** — the canonical-line comparison of `ms.musicxml` (reference)
  against `tk.musicxml`. The normalisation is *shared*, not duplicated: it is
  the `core_norm` of `crates/scoretk/tests/common/musicxml_core.rs`, driven by
  the already existing ignored helper `crates/scoretk/tests/oracle_core.rs`.
  A file is core-identical when 0 core lines changed. `core.tsv` also records
  the changed tags and the first differing line, used for grouping gaps by
  musical family (`scripts/oracle/musicxml/groupes.py` reads the same columns).
- **N1/N2** — pitch multiset and pitch+quantised attack of `ms.mid` (reference)
  against `tk.mid`, using `scripts/oracle/midi/midi_notes.py` at the Q16 grid
  (±1/16 quarter). `alt_core` compares the alternative's MusicXML to scoretk's
  on the same files.

## Lots

`lots.sh` reuses the exact manifests of the LilyPond/ABC writing oracles
(`scripts/oracle/lilypond/lots-ecriture.sh`): 56 corpus `.mscx`, 174 `.mscz`,
the genuine MusicXML of `~/Musique`, a deterministic 1/5 sample of `.ly`; it
adds the same 1/5 sample of `.abc`. Path lists only, no file is versioned.

## Usage

```sh
nix develop -c cargo build -p scoretk-cli
cp target/debug/scoretk /tmp/scoretk-mscz-base   # freeze the binary
scripts/oracle/musescore/lots.sh /tmp/oracle-mscz/lots
scripts/oracle/musescore/mesure.sh --out /tmp/oracle-mscz/base \
  --scoretk /tmp/scoretk-mscz-base --mscore mscore \
  --lot corpus=/tmp/oracle-mscz/lots/manifestes/corpus.txt \
  --lot mscz=/tmp/oracle-mscz/lots/manifestes/mscz.txt \
  --lot xml=/tmp/oracle-mscz/lots/manifestes/xml.txt \
  --lot ly=/tmp/oracle-mscz/lots/manifestes/ly.txt \
  --lot abc=/tmp/oracle-mscz/lots/manifestes/abc.txt
# Rebuild only the summary (or resume missing files):
scripts/oracle/musescore/mesure.sh --out /tmp/oracle-mscz/base --resume \
  --scoretk /tmp/scoretk-mscz-base --mscore mscore \
  --lot corpus=/tmp/oracle-mscz/lots/manifestes/corpus.txt
```

`mesure.sh` wraps `mesure.py` with `python3Packages.mido`. Outputs in `$OUT`:
`<lot>.jsonl` (one row per file), `paires.tsv`/`alt_paires.tsv` (rebuilt from
the rows), `core.tsv`/`alt_core.tsv`, `travail/<lot>/<i>/` (all converted
files) and `resume.md` (the summary table). With `--resume`, already measured
files are skipped and the core comparison is recomputed from the rows.
