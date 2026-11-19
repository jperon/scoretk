# LilyPond oracle: measuring `.ly` to MusicXML reading

The independent oracle is the MIDI produced by **LilyPond** from the same
`.ly` source. Neither this oracle nor the alternatives define scoretk's
behaviour: they are measurement instruments only. No corpus file and no
third-party script is copied into the repository. Every binary is launched
under `timeout` and `prlimit --as=2000000000`.

## Levels

A note is a tuple `(pitch, attack, duration, track)`; attacks and durations are
in **quarter notes**. Pitches are MIDI note numbers (concert pitch). Grace
notes and tie stops are ignored on both sides; MusicXML `<chromatic>` /
`<octave-change>` transposition is applied.

- **N1** — multiset of pitches, as in the historical `xmlcmp/cmp.py`. An octave
  error (±12/±24 semitones) and a constant transposition are also reported to
  classify the discrepancy.
- **N2** — multiset of `(pitch, quantized attack)`. This is the same level as
  `scripts/oracle/midi`: grid **1/16 of a quarter note** (a 32nd note, `GRID =
  0.25`), matching tolerance **±1/16 of a quarter note** (`TOL = 0.25`), i.e.
  MuseScore's default Q16 MIDI-import grid. A file is *exact* when the note
  count, N1 and N2 all match the reference.

Percentages are weighted by the number of reference notes, so a short and a
long score do not carry the same weight.

### Repeats: making the attack comparison fair

LilyPond's MIDI backend always unfolds `\repeat unfold`, and unfolds `\repeat
volta` whenever `\unfoldRepeats` is present in the source. The MusicXML side,
whether written by scoretk or by python-ly, writes each note **once, as
written**. Comparing raw attack times would therefore penalize every note of
an unfolded pass after the first.

N2 is consequently computed **only on the subset selected by N1**: for each
pitch, the two attack lists are sorted and the `min(count_ref, count_test)`
earliest attacks are paired in order. The surplus copies contributed by an
unfolded repeat arrive after the first pass and are discarded by the N1
intersection before attacks are compared. This is neutral for scores without
repeats (all notes are paired) and keeps the comparison "as written" on both
sides. It does not need a second LilyPond run or a source rewrite.

Known limit: material *following* an unfolded block is shifted in the MIDI by
the length of the extra passes, so its attack can differ even though N1 is
correct. Such cases stay flagged `attaque_decalee`; they are a small minority
and are reported as such rather than silently filtered.

## Alternatives measured with the same oracle

1. **python-ly** (nixpkgs `python3Packages.python-ly`, 0.9.10): `ly musicxml
   -o out.musicxml file.ly`. Its MusicXML is read by the same `read_xml()` and
   compared to the same reference MIDI.
2. **music21** (10.5.2, installed in an out-of-repo virtualenv): `converter.parse`
   is attempted and its exception recorded. This version of music21 has **no
   LilyPond reader**: `.ly` is not a recognized extension
   (`ConverterFileException: cannot find format from file extensions`), and an
   explicit `format='lilypond'` is rejected as well
   (`ConverterFileException: File is not in a correct format`). It does not
   call LilyPond; it cannot produce MusicXML from a `.ly` source, so no N1/N2
   figure can be reported for it. The failed attempts are kept in the raw
   `.jsonl` as evidence.

## Usage

```sh
# One absolute path per line; lots.sh extracts column 6 of the base lists.
BASEDIR=/path/to/base/lists scripts/oracle/lilypond/lots.sh /tmp/oracle-out

nix develop -c cargo build -p scoretk-cli
scripts/oracle/lilypond/mesure.sh \
  --out /tmp/oracle-out --refs /path/to/lyaudit \
  --scoretk target/debug/scoretk \
  --python-ly ly --music21 /tmp/m21venv/bin/python \
  --lot mus=/tmp/oracle-out/manifestes/mus.txt \
  --lot ra=/tmp/oracle-out/manifestes/ra.txt \
  --lot mut=/tmp/oracle-out/manifestes/mut.txt \
  --stride 5

# Rebuild only the summary from raw measurements:
scripts/oracle/lilypond/mesure.sh --out /tmp/oracle-out --resume

# Unit-level check of the comparator (no corpus, no process):
python3 scripts/oracle/lilypond/ly_notes.py selftest
```

`--stride N` keeps one file in N (`NR % N == 1` equivalent) for a deterministic
sample; the value used must be stated in `docs/agents/lilypond.md`.
`--refs` points at the precomputed MIDI references laid out as
`<prefix><lot>/<md5_basename>/ly.midi` (`--refs-prefix`, default `out3`, so
`out3mus/`, `out3ra/`, `out3mut/`); the hash includes the trailing newline of
`echo "$path"`, as in the historical oracle. When a reference is absent the
driver regenerates it with LilyPond under a 20 s / 2 GB budget.

Outputs in `--out`: one `.jsonl` per lot (one row per file and per tool), the
converted files under `travail/`, and `resume.md` (tables and the ten most
frequent scoretk discrepancy causes). The report in `docs/agents/lilypond.md`,
section "Measurement", is a formatted version of `resume.md`.
