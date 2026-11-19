# LilyPond oracle: measuring `.ly` to MusicXML reading

The independent oracle is the MIDI produced by **LilyPond** from the same
`.ly` source. Neither this oracle nor the alternatives define scoretk's
behaviour: they are measurement instruments only. No corpus file and no
third-party script is copied into the repository. Every binary is launched
under `timeout` and `prlimit --as=2000000000`.

## Levels

A note is a tuple `(pitch, attack, duration, track)`; attacks and durations are
in **quarter notes**. Pitches are MIDI note numbers (concert pitch). Tie stops
are ignored on both sides (a MIDI note-off closes an attack, it does not open
one); **grace notes are counted on both sides**: LilyPond plays them in its
MIDI and the readers write them as `<grace>` elements. In MusicXML a grace note
is placed at the current attack (the following note's) and borrows its time, so
it does not advance the position counter. MusicXML `<chromatic>` /
`<octave-change>` transposition is applied.

- **N1** — multiset of pitches, as in the historical `xmlcmp/cmp.py`. An octave
  error (±12/±24 semitones) and a constant transposition are also reported to
  classify the discrepancy.
- **N2** — multiset of `(pitch, quantized attack)`: attacks are rounded to the
  grid **1/16 of a quarter note** (a 32nd note, `GRID = 0.25`) and the two
  multisets are intersected. This is the Q16 grid of `scripts/oracle/midi`.
  There is no "pair the earliest attacks of each pitch" rule: with duplicated
  notes such a rule matches arbitrary events, depends on list order and moves
  every attack after an unfolded block. A file is *exact* when the note count,
  N1 and N2 all match the reference.

Percentages are weighted by the number of reference notes, so a short and a
long score do not carry the same weight.

### Repeats: N2 is restricted to repeat-free scores

LilyPond's MIDI unfolds `\repeat unfold` always, and `\repeat volta` when
`\unfoldRepeats` is present; a MusicXML reader may write the structure once
instead. The two timelines are then not comparable beat for beat. Rather than
pair attacks heuristically, the driver detects `\repeat`, `\unfoldRepeats`,
`\volta` and `\alternative` in the source **and its direct `\include`s**, and
records a `repeat` flag on every row. N1 is still reported for all files;
**N2 is aggregated only over repeat-free files**. The raw rows keep N2 for
repeat files (recorded, never silently dropped) so the effect can be inspected.

### MusicXML time is one cursor per part

`read_xml()` no longer keeps **any** per-voice or per-`(voice, staff)` counter.
MusicXML defines time per **part** with a single cursor, expressed in quarter
notes:

- a `<note>` without `<chord/>` is placed at the cursor and advances it by its
  `<duration>` (converted with the current `<divisions>`, which may change in
  `<attributes>` mid-measure); a `<chord/>` note shares the previous attack;
- `<backup>` moves the cursor backwards, `<forward>` forwards — both are real
  timeline moves, not writer artefacts to ignore;
- a new `<measure>` starts at the previous measure's start plus that measure's
  **maximal cursor extent** (the furthest position reached, not the final one,
  which a trailing `<backup>` usually brings back to the measure start);
- a `<grace>` note is counted but has no duration, so it does not move the
  cursor; `<tie type="stop"/>` is not an attack but still occupies its time.

The old counter keyed by voice was itself a correction of an even older
`(voice, staff)` key: a cross-staff voice (`\change Staff`) keeps one `<voice>`
and changes `<staff>`, so the pair key restarted its timeline. The cursor model
keeps cross-staff notes monotonic too, and additionally handles a piano part
that reuses voice 1 on both staves (a `<backup>` separates them). On the 4543
measurable files of the full last campaign the cursor and voice models give the
same N2 file by file, but the aggregate changes because the cursor also honours
`<backup>`/`<forward>` and per-measure extents; see `docs/agents/lilypond.md`.

Minimal reproductions used to validate these rules (each has an assertion in
`ly_notes.py selftest`): a piano part with voice 1 reused on both staves and a
`<backup>` between them; a voice changing `<staff>`; chords and a chord at end
of measure; `<backup>`/`<forward>` with a `<divisions>` change; a partial-measure
`<forward>` whose first note is in the next measure (`knjze12monferA.ly`); a
leading `<forward>` plus a grace (`input/regression/grace-nest5.ly`); and a
`\repeat unfold` / `\repeat volta` score (repeat flag).

### Percussion (`<unpitched>`)

LilyPond plays `\drummode` / `\new DrumStaff` notes on General MIDI drum
channels. The pivot model has no untuned-percussion pitch, so scoretk exports
these notes as ordinary `<pitch>` elements carrying the **GM MIDI note number**
from `ly/drumpitch-init.ly` (`bd` 36, `sn` 38, `hh` 42, `cymr` 51, …). The
oracle therefore compares them to LilyPond's MIDI exactly as pitched notes;
that is the meaningful drum comparison.

Other writers may write `<unpitched>` instead. `read_xml()` then uses
`<display-step>`/`<display-octave>` as the comparison pitch (B4 = 71), so the
note is at least counted and its attack checked. python-ly emits the same B4
head for every drum voice and carries no MIDI number, so its drum pitches
cannot be matched to LilyPond's MIDI — a documented oracle limitation, not a
reader defect. No corpus file or third-party script is copied into the
repository.

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

# Unit-level checks (no corpus, no process):
python3 scripts/oracle/lilypond/ly_notes.py selftest
python3 scripts/oracle/lilypond/mesure.py --selftest
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
