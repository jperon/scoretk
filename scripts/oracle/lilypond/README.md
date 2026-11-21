# LilyPond oracle: measuring `.ly` to MusicXML reading

The independent oracle is the MIDI produced by **LilyPond** from the same
`.ly` source. Neither this oracle nor the alternatives define scoretk's
behaviour: they are measurement instruments only. No corpus file and no
third-party script is copied into the repository. Every binary is launched
under `timeout` and `prlimit --as=2000000000`. The scoretk budget is 45 s:
the 17 616-note `Vingt_danses_enfantines_pour_piano` copies need about 23 s,
and a shorter budget made the xml aggregate fluctuate with which copy timed
out.

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

### Grace notes: two N2 rules (`N2` and `N2-grace`)

Notation Reference §2.6.1 states that grace notes "take up no additional
logical time in a measure"; `lily/grace-iterator.cc` puts them in the *grace
part* of a musical moment and `lily/midi-walker.cc` notes that their MIDI can
even start at negative times. §2.6.1 also states that each grace note in MIDI
is played at 1/4 of its written duration. MusicXML `<grace/>` instead has zero
duration and is
read by `ly_notes.read_xml()` at the current cursor, i.e. at the **following
main note's attack**. Quantizing the MIDI grace onsets directly to the Q16
grid therefore compares two different times.

The driver now records both rules:

- **N2** (unchanged): every note, including graces, is compared on
  `(pitch, quantized attack)`. This is the non-regression figure.
- **N2-grace**: each candidate `<grace/>` note is paired with the *latest*
  unmatched MIDI note of the **same pitch** in the window
  `[main attack − 1 quarter, main attack + grid/2]`; paired graces are compared
  on their main note's logical attack and counted separately (`graces_ref`,
  `graces_test`, `graces_match`). Pairing is deterministic (one MIDI note per
  candidate grace, latest first) and never reassigns a main note. A missing or
  stale grace remains unmatched and still lowers the score.

`ly_notes.py selftest` and `mesure.py --selftest` cover the grace marker, the
pairing, the missing-grace case and the unaffected case. No reader change is
involved: this is a comparison rule, not a tuning knob.

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

## Writing oracle: scoretk against musicxml2ly

`ecriture.py` measures the **other direction**: `.ly` written from a source
`S`, then compiled to MIDI by LilyPond 2.26. It reuses `midi_notes.py`
(MIDI ↔ MIDI N1/N2/N2d, Q16 grid) and `ly_notes.py` (repeat detection). For
each `S` it writes:

- scoretk's `.ly` directly from `S` (`scoretk S --to ly`);
- musicxml2ly's `.ly` from the **reference MusicXML** of `S`: the versioned
  `*.ref.musicxml` for the corpus, MuseScore's own export for `.mscz`, the
  source itself for `.xml`, and scoretk's MusicXML for `.ly` round trips.
  Each row records `xml_source` so the report always says which input was used.

A copy of each `.ly` receives an injected `\midi { }` and an `\unfoldRepeats`
wrapper around the score music: without the latter neither converter unfolds
volta repeats in MIDI, while the reference MIDI (MuseScore for `.mscx`/`.mscz`/
`.xml`, LilyPond for `.ly`) does. Both candidates are therefore measured under
the same conditions. The reference MIDI and MusicXML are cached in
`<out>/refs/<hash>`, so a re-run only recompiles the candidates.

Columns of interest per row: `compile_ok`, `erreurs`, `avertissements`,
`taille_ly`, `ref`/`test`, `n1`/`n2`/`n2d`, `exact`, `drapeaux`. A failed
compilation is recorded as such but its MIDI is still measured when LilyPond
produced one; the two facts are complementary.

```sh
scripts/oracle/lilypond/lots-ecriture.sh /tmp/lots-ecriture
nix develop -c cargo build -p scoretk-cli
scripts/oracle/lilypond/ecriture.sh --out /tmp/ecriture-base \
  --scoretk target/debug/scoretk \
  --lot corpus=/tmp/lots-ecriture/manifestes/corpus.txt \
  --lot mscz=/tmp/lots-ecriture/manifestes/mscz.txt \
  --lot xml=/tmp/lots-ecriture/manifestes/xml.txt \
  --lot ly=/tmp/lots-ecriture/manifestes/ly.txt
scripts/oracle/lilypond/ecriture.sh --out /tmp/ecriture-base --resume
python3 scripts/oracle/lilypond/ecriture.py --selftest
```

`lots-ecriture.sh` builds four manifests: the 56 corpus `.mscx`, the 174
`.mscz` of `~/Musique`, the genuine MusicXML files of `~/Musique`, and a 1/5
deterministic sample of the `.ly` files (`SAMPLE=5`, overridable).
The `.mxl` sources of the XML lot are decompressed before being handed to
`musicxml2ly`: passing the raw ZIP bytes under a `.musicxml` name makes its
XML parser fail before conversion. Failures inside `musicxml2ly` itself (for
example its `UnboundLocalError: part_list` on some scores) are recorded as
`echec` rows, not silently discarded.

`ecriture.sh` provides `mido`, LilyPond 2.26 and `musicxml2ly`; it also works
around a transient nixpkgs MuseScore/alsa-lib glibc mismatch through the
overridable `MSCORE_LIBS` variable. Freeze the scoretk binary under a stable
path for a campaign (`--scoretk /tmp/scoretk-base`) so a rebuild cannot mix two
versions in one report.

## Unsupported inputs (exit code 3)

`scoretk` exits with **3** when the input is deliberately unsupported: a
MuseScore version outside the ported subset, or a format without a converter in
the requested direction (see `scoretk --help` and `docs/agents/scoretk.md`).
Such a file is *not* a conversion failure. Both drivers apply the same rule
through `scripts/oracle/common.py`, never copied per script:

- `mesure.py` (reading): the input is a `.ly`, for which the reader has no
  "unsupported version" variant today, so nothing is expected to be excluded
  on the `.ly` lots; the guard is still in place and stops the file before any
  alternative row is written if `scoretk` ever returns 3.
- `ecriture.py` (writing): on the `.mscz` lot, a score whose MuseScore version
  is not ported is recorded with an `exclu` marker, excluded from every
  numerator and denominator **for both candidates** (only what scoretk refuses
  leaves the batch, so the comparison stays fair), and reported once as
  `Unsupported (excluded): N` in `resume.md` plus one warning line on stderr.
  Example: the MuseScore 1.14
  `Canon_in_D_Pachelbel_-_arranged_for_accordion.mscz`.
