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
4. **Ornaments are stripped from the oracle copy** (`sans_ornements()` in
   `mesure.py`). `abc2midi` *realizes* ornaments as extra notes: a single
   `Tc` sounds C-D-C-D-C, so a written 6-note tune with a trill, a mordent
   and a turn yields 20 reference notes. Neither candidate writes those
   performance notes (they write the ornament symbol), so the oracle was
   over-counting by construction — the dominant cause of the 678-note
   `NOTES_MANQUANTES` group in `docs/agents/abc.md` § 15.3.
   `abc2midi` has no option to disable ornament realization (`-NGRA` only
   disables grace notes, already ignored on both sides by `cmp.py`), so the
   preprocessing removes, on **music lines only**:
   - long decorations `!trill!`, `!turn!`, `!invertedturn!`, `!mordent!`,
     `!lowermordent!`, `!uppermordent!`, `!pralltriller!`, `!roll!`, `!T!`
     (and their old-style `+…+` spelling);
   - the single-letter abbreviations `T`, `M`, `P`, `~` when immediately
     followed by a (possibly accidental-prefixed) note letter.
   The names are those of `abc2xml.py` 269, `MusicXml.ornMap` (lines 1096–1097)
   and `MusicXml.uSyms` (lines 1115–1116): `T`/`trill`, `M`/`lowermordent`,
   `P`/`uppermordent`, `~`/`roll`, `mordent`, `pralltriller`, `turn`,
   `invertedturn`. Non-realizing decorations (dynamics, bowings, fermatas,
   articulations) are left alone; fermatas are handled by `-NFER`. `U:`
   definitions are left alone too, so the two candidates still receive the
   unmodified tune. On the reduced `ornament.abc` this brings the reference
   from 20 notes to the 6 written notes, and both candidates to N1/N2 100 %.

5. scoretk is called as a release binary under
   `timeout 10 prlimit --as=2000000000`, on the extracted tune as-is.
6. `abc2xml.py` receives the same tune (path given as an argument, outside the
   repository).
7. `cmp.py` compares the MIDI with each MusicXML file:
   - **N1**: multiset of MIDI pitches (as in the LilyPond oracle's `cmp.py`),
     with MusicXML `<chromatic>`/`<octave-change>` transposition applied;
     grace notes, tie stops and unpitched notes are ignored on both sides;
   - **N2**: same, with the attack quantized to 1/24 of a quarter note;
     A note carrying `<tie type="stop">` is not counted (no note-on in the
     MIDI), **but its duration still advances the timeline** — otherwise
     every subsequent attack is shifted and scoretk's `<forward>` padding
     is accidentally compensated (this inflated 19 tunes' N2 before phase 1).
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
