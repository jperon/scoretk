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

## Writing oracle: ABC export against `xml2abc.py`

`ecriture.py` measures the reverse direction of the method above: from a source
`S`, scoretk writes `.abc` (`scoretk S -o tk.abc`) and `xml2abc.py` writes `.abc`
from the MusicXML of reference of `S` (the versioned `*.ref.musicxml` for the
corpus, MuseScore's own export for `.mscz`, the source itself for the XML lot,
scoretk's MusicXML as a fallback). The `xml_source` column of every row records
which MusicXML was used.

Both `.abc` files are played by `abc2midi` with the same flags as the reading
oracle (`-silent -NGRA -NGUI`), after the same cleanup is applied **to a copy**
(`mesure.sans_reprises` and `mesure.sans_ornements`): written notes are counted
once, without repeat replay or ornament realization. The resulting MIDI is
compared to the reference MIDI of `S` with `midi_notes.py` N1/N2/N2d (Q16 grid):
MuseScore 4.7.4's MIDI for `.mscx`/`.mscz`, scoretk's MIDI for MusicXML.
`abc2midi` error/warning lines and the `.abc` byte count are also recorded.
The helpers `mesure.py` and `cmp.py` of the reading oracle and `midi_notes.py`
of the MIDI oracle are reused, not duplicated. Every binary is launched under
`timeout 10 prlimit --as=2000000000`.

The three lots are generated by `lots-ecriture.sh`, which delegates to
`scripts/oracle/lilypond/lots-ecriture.sh`: the 56 corpus `.mscx`, the 174
`.mscz` and the 32 genuine MusicXML of `~/Musique`.

```sh
# Lot manifests
scripts/oracle/abc/lots-ecriture.sh /tmp/abc-ecriture/lots

# Baseline / final measurement (binary frozen outside the repository)
scripts/oracle/abc/ecriture.sh --out /tmp/abc-ecriture/base \
    --scoretk /tmp/abc-ecriture/bin/scoretk-base \
    --xml2abc /tmp/abcchk/xml2abc_177/xml2abc.py \
    --lot corpus=/tmp/abc-ecriture/lots/manifestes/corpus.txt \
    --lot mscz=/tmp/abc-ecriture/lots/manifestes/mscz.txt \
    --lot xml=/tmp/abc-ecriture/lots/manifestes/xml.txt

# Summary only (after all chunks have written their .jsonl into --out)
scripts/oracle/abc/ecriture.sh --out /tmp/abc-ecriture/base --resume

# Single-file smoke test / selftest (no corpus needed)
scripts/oracle/abc/ecriture.sh --selftest
```

Outputs in `$OUT`: `corpus.jsonl`, `mscz.jsonl`, `xml.jsonl` (one row per file
and candidate), `resume.md`, `refs/` (cached reference MIDI/MusicXML), and
`travail/<lot>/<i>/` (`tk.abc`, `alt/*.abc`, the cleaned `*.plain.abc`, both
`*.mid`, and `journal.log`). A non-zero `abc2midi` exit is accepted when a
non-empty MIDI was written, exactly as in the reading oracle.

## Model mode (no MIDI artefacts)

`--mode model` judges **like with like** at the pivot-model level instead of
going through `abc2midi` and the MuseScore reference MIDI:

* scoretk exports the source to MusicXML; `midi_notes.read_xml` turns it into
  the reference note list (pitch, onset, duration per voice/staff);
* each candidate's `.abc` is re-imported by scoretk, exported to MusicXML and
  read the same way; `midi_notes.compare` yields N1/N2/N2d;
* `xml2abc.py` receives the **same** model-derived MusicXML as scoretk.

This removes two artefacts of the legacy MIDI mode: MuseScore's reference MIDI
realises ornaments/voltas that neither written ABC contains, and `abc2midi`
caps voices at 16 channels (orchestral scores then misattribute pitches).
`--mode midi` stays available for continuity with the earlier campaigns.
