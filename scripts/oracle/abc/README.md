# ABC oracle: measuring ABC to MusicXML reading

The independent oracle is **`abc2midi`** (nixpkgs package `abcmidi`), which
turns ABC into MIDI. The alternative measured against the **same oracle** is
Willem Vree's `abc2xml.py` (version 269), which produces MusicXML.

Neither tool defines scoretk's behaviour: they are measurement instruments
only. No corpus file and no third-party script is copied into the repository.

## Measurement method

1. Each ABC file is split into `X:` tunes by `decoupe()` in `mesure.py`: the
   global header (lines before the first `X:`) plus one block per tune. One
   tune is one measurement, so tunes from the same file are not confused.
   Files without an `X:` field are counted separately (`SANS_X`) and excluded
   from the note comparison.
2. The oracle receives a copy of the tune where repeat barlines (`|:`, `:|`,
   `::`, `[1`, `[2`) are replaced by plain bars and the `P:` field line is
   removed: `abc2midi` replays repeats and parts, whereas MusicXML writes each
   note **once**. We therefore measure the notes as written, not as played.
   `abc2midi` is called with `-NGRA` (no grace notes; grace notes are also
   ignored on the MusicXML side by `cmp.py`) and `-NGUI` (no guitar-chord
   strumming from chord symbols such as `"C"`: those are harmony annotations,
   not written notes).
   *A non-zero exit code is accepted when a non-empty MIDI file was written*:
   `abc2midi` uses it for recoverable warnings (unknown chord, etc.). The MIDI
   file is what counts, not the exit code.
3. scoretk is called as a release binary under
   `timeout 10 prlimit --as=2000000000`, on the extracted tune as-is.
4. `abc2xml.py` receives the same tune (path given as an argument, outside the
   repository).
5. `cmp.py` compares the MIDI with each MusicXML file:
   - **N1**: multiset of MIDI pitches (as in the LilyPond oracle's `cmp.py`),
     with MusicXML `<chromatic>`/`<octave-change>` transposition applied;
     grace notes, tie stops and unpitched notes are ignored on both sides;
   - **N2**: same, with the attack quantized to 1/24 of a quarter note;
   - a file is "exact" when N1 is exact (same number of notes, all correct).
   Counters per lot: files, tunes, exacts, notes, correct notes, percentage,
   failures, empty outputs.

`cmp.py` parses SMF and MusicXML with the Python standard library: no external
dependency (`mido` is not needed).

## Usage

```sh
# One "lot<TAB>path" line per ABC file
BIN=./target/release/scoretk
ALT=/tmp/abcchk/abc2xml_269/abc2xml.py          # outside the repository
OUT=/tmp/abc-mesure/plein

scripts/oracle/abc/mesure.sh liste.txt "$OUT" "$BIN" "$ALT"
python3 scripts/oracle/abc/aggreg.py "$OUT/mesures.tsv"

# A single file (same interface as the LilyPond oracle's one.sh)
scripts/oracle/abc/one.sh "$OUT/un" "$BIN" air.abc "$ALT"
```

Outputs in `$OUT`:

- `mesures.tsv` — one row per (tune, candidate);
- `erreurs.log` — failed candidates;
- `brut/<md5>_t<X>/` — `air.abc`, `air_plain.abc`, `ref.mid`,
  `score.musicxml`, `alt/air.xml`, `journal.log` (commands and messages).

The lots are described in `docs/agents/abc.md`, section "Measurement". Path
lists and raw outputs stay outside the repository.

## Adding a lot

Create the list with absolute paths, for example:

```sh
find /home/jacques/programmes/musique/corpus/abcjs-tests -name '*.abc' -print0 |
  sort -z | while IFS= read -r -d '' f; do printf 'abcjs\t%s\n' "$f"; done >> liste.txt
```

The NC corpora (`abc-2.1-standard`, `essenFolksong`) are for local measurement
only: never copy their files or their outputs into the repository.
