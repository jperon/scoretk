# Reliability plan for the conversions (ten directions, five formats)

Written on 2026-11-19. Goal: for each format and each direction, a **reference
batch** and an **oracle independent of scoretk**, then correction campaigns measured like the LilyPond reading campaign (612 → 1 121 exact
files on `~/Musique` in two days, thanks to automatic file-by-file
measurement and no tolerated regression).

## 0. Objective (set by the user, 2026-11-19)

- **For every conversion, do at least as well as the alternatives, preferably better.** Each survey therefore also measures the reference alternative on the same batch and the same oracle:

  | Conversion | Alternatives to beat |
  |---|---|
  | MuseScore → MusicXML/MIDI | MuseScore 4.7.4 (reference: aim for parity); music21 |
  | MusicXML → MuseScore | MuseScore 4.7.4 import |
  | MusicXML → LilyPond | `musicxml2ly` (LilyPond); python-ly |
  | LilyPond → MusicXML | python-ly (`ly musicxml`); music21 |
  | ABC → MusicXML / MusicXML → ABC | `abc2xml.py` / `xml2abc.py`; music21; MuseScore's ABC import |
  | ABC → LilyPond | `abc2ly` |
  | MIDI → score | MuseScore's MIDI import; `midi2ly`; music21 |
  | score → MIDI | MuseScore; LilyPond; music21; `abc2midi` |

- **Scope**: not necessarily the whole musical core, but everything reasonably achievable
  **while keeping maintainable code and output whose size does not explode** (criterion for rejecting a fix:
  disproportionate complexity or bloated output).
- Third-party corpora downloaded outside the repository: `/home/jacques/programmes/musique/corpus/`
  (`README.md`: sources and licences).

## 1. Common principles

1. **One external oracle per direction.** The reference is produced by the tool that
   is authoritative for the format: MuseScore 4.7.4, LilyPond, `abc2midi`/`abc2xml`.
   Never by scoretk itself (a round trip alone hides symmetric errors).
2. **Graduated measurement, from the most robust to the finest**:
    - N1 pitches (multiset per voice/staff);
    - N2 onsets and durations (tick, normalised to quarter notes);
    - N3 structure (measures, key signatures, time signatures, clefs, repeats);
    - N4 text (lyrics, titles, directions);
    - N5 complete musical core (`core_norm`), already in place for MusicXML.
    Each campaign first targets N1–N2 over the whole batch, then moves up.
3. **Two tracked indicators**: correct notes / reference notes, and
   entirely correct files (at the measured level). Plus: failures, timeouts,
   empties.
4. **Anti-regression guard**: each merge compares against the last survey;
   no per-file drop tolerated without a written explanation.
5. **Safety**: every launch of scoretk under
   `timeout 10 prlimit --as=2000000000`.
6. **Unversioned batches** (personal files, large third-party corpora): lists
   of paths + derived references outside the repository. A **versioned sub-batch**
   (files with compatible licences, declared in `NOTICE`) feeds the blocking
   `cargo test` tests.
7. **Deliberately unsupported inputs never count as failures.** `scoretk`
   exits with code 3 (MuseScore version outside the ported subset, format
   without a converter: `docs/agents/scoretk.md` § 4); the measurement scripts
   exclude such a file from numerator *and* denominator, for every tool of the
   batch, and report it once as `Unsupported (excluded)`. Shared helper:
   `scripts/oracle/common.py`. A reader bug or a parse error keeps code 1 and
   stays a failure.

## 2. Infrastructure (phase 0, prerequisite)

- Promote the current oracle (`one.sh` + `cmp.py`, MusicXML vs MIDI pitches
  LilyPond) into a versioned tool `scripts/oracle/`: input = (file,
  reference, direction), output = N1/N2 measurement line; aggregation and
  comparison of surveys (`baisses`).
- Common comparators **MIDI ↔ MIDI** (notes: pitch, onset, duration, track)
  and **MusicXML ↔ MusicXML** (reuse `core_norm`).
- A baseline survey per direction, archived (`docs/agents/releves/`, summary
  only), to track progress.

## 3. Per format

### MuseScore (`.mscz`/`.mscx`)

| Direction | Reference batch | Oracle | State / target |
|---|---|---|---|
| Reading | 56 corpus files (versioned); **174 `.mscz` from `~/Musique`**; ~150 `.mscx` from the upstream tests (`ref-musescore/src/importexport/musicxml/tests/data`) | MuseScore 4.7.4 MusicXML export (`gen-references.sh`), compared to the core | 54/56 at the core; extend to the 174 real files |
| Writing | everything scoretk can read (mscz, xml, ly, abc) | reopening by MuseScore 4.7.4 **then MusicXML export**, compared to scoretk's direct MusicXML (fidelity, not just opening) | 56/56 opened; measure fidelity |

### MusicXML

| Direction | Batch | Oracle | State / target |
|---|---|---|---|
| Writing | the same as MuseScore reading | MuseScore 4.7.4 reference + **MusicXML 4.0 XSD validation** | near-complete core; add XSD and `~/Musique` |
| Reading | **MusicXML test suite** (207 files, `lilypond-ref/input/regression/musicxml`, designed to cover the standard); **451 `.xml` from MuseScore's upstream tests**, with their expected `.mscx`; 80 `.xml` + 27 `.mxl` from `~/Musique` | import then MusicXML export by MuseScore 4.7.4, compared to the core; for upstream tests, the expected `.mscx` | 26/30, 26/26 round trip; measure on these ~750 files |

### MIDI

| Direction | Batch | Oracle | State / target |
|---|---|---|---|
| Writing | 174 `.mscz`; corpus; **`.ly`/`.midi` pairs from `~/Musique`** (1 228 LilyPond MIDIs) | MuseScore 4.7.4 MIDI (for mscz); LilyPond MIDI (for ly: tests the ly → model → MIDI chain); N2 notes | 169/173 exact, N1 100.00 %, N2 100.00 %, N2d 100.00 % (mscz); 711/1 020 exact, N1 97.42 %, N2 95.97 %, N2d 93.88 % (ly); 17/21 corpus — see `midi.md` § "MIDI export: status" |
| Reading | **MuseScore upstream tests** (`importexport/midi/tests`, 118 `.mid` + expected `.mscx`); 1 228 MIDIs from `~/Musique` (whose `.ly` source we have) | MuseScore's expected `.mscx`; and for LilyPond MIDIs: source pitches/onsets (N1–N2 after quantisation) | 4/21; measure on ~1 350 files |

### LilyPond (`.ly`)

| Direction | Batch | Oracle | State / target |
|---|---|---|---|
| Reading | **1 447 `.ly` from `~/Musique`**; **1 693 files from `lilypond-ref/input/regression`** | LilyPond MIDI (N1 in place; add N2 onsets/durations and N4 lyrics) | 98.8% of notes, 1 121 exact files; `regression`: 84% |
| Writing | 174 `.mscz`, corpus 56, `.xml`; round trip of the 1 447 `.ly` | **error-free LilyPond compilation**, then LilyPond MIDI compared to scoretk MIDI (N2); and to the source MIDI | 56/56 compile, core 24/56 |

### ABC

| Direction | Batch | Oracle | State / target |
|---|---|---|---|
| Reading | to be built: **`abc2ly` tests** (`lilypond-ref/input/regression/abc2ly`, 8); examples from standard 2.1; a **public collection** (The Session, abcnotation.com, Nottingham Music Database — licence to check before any download, user agreement required) | `abc2midi` (abcMIDI, nixpkgs) → MIDI, N1–N2; `abc2xml.py` for N3 | no real measurement today |
| Writing | all the other batches (mscz, xml, ly) | `abc2midi` on scoretk's output vs scoretk's / reference's MIDI; `abcm2ps` without error | 46/56 round trip |

## 4. Phases and proposed order

Order guided by usage (cantus mainly reads mscz/ly/xml) and by the expected
yield of the batches already available:

1. **Phase 0 — tooling** (§ 2). Quick, unblocks everything else.
2. **Phase 1 — baseline surveys** of the ten directions, without fixes: a full
   quantified table, replacing today's disparate figures.
3. **Phase 2 — MusicXML reading** on the test suite and upstream tests
   (cantus's pivot format, ~750 oracle files already ready).
4. **Phase 3 — LilyPond**: finish reading (N2, lyrics, `\chordmode`,
   `\partCombine`, `\tag`, 91 empty files in the `regression` corpus), then
   writing measured by compilation + MIDI.
5. **Phase 4 — MIDI**: reading on the 1 228 LilyPond MIDIs (known source:
   ideal quantisation oracle) and the upstream tests; writing on ly/midi
   pairs.
6. **Phase 5 — MuseScore**: reading the 174 real files, writing fidelity by
   reopening.
7. **Phase 6 — ABC**: build the batch (with agreement), then both directions.

Each phase follows the LilyPond campaign loop: survey → grouping
of discrepancies by cause → one task per cause (reduced file + test) → full
measurement → merge if no drop → wasm/cantus rebuild.

## 5. Points to decide

- Downloading public corpora: allowed (compatible licences
  only).
- Versioning a sub-batch of `~/Musique` (personal files) or only
  lists of paths.
- Target: decided (§ 0, scope).
