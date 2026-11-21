# MusicXML export oracle

Independent measurement of scoretk's MusicXML writer on real scores. The
reference is **MuseScore 4.7.4**; the normative target is the MusicXML 4.0 XSD in
`/home/jacques/programmes/musique/specs/musicxml/`. No corpus file and no
third-party script is copied into the repository: manifests and raw outputs live
outside it. Every binary is launched under `timeout` and `prlimit
--as=2000000000`.

## What is measured

For each `.mscz`, four conversions are produced: scoretk → MusicXML, MuseScore →
MusicXML, scoretk → MIDI, MuseScore → MIDI. Three measurements follow:

- **Validity** — `xmllint --schema` with the MusicXML 4.0 XSD; its `xml.xsd` and
  `xlink.xsd` imports are resolved through a locally generated XML catalog.
  Errors are counted by message, for scoretk *and* for the reference.
- **Core fidelity** — the canonical-line comparison already used by
  `crates/scoretk/tests/e2e_corpus.rs`. The normalisation is shared, not copied:
  it lives in `crates/scoretk/tests/common/musicxml_core.rs` and is driven by the
  ignored helper test `crates/scoretk/tests/oracle_core.rs`.
- **N1/N2** — multiset of pitches and of (pitch, quantised attack) on the MIDI
  each tool plays, using `scripts/oracle/midi/midi_notes.py` at the Q16 grid.

Core differences are grouped by musical family (notes, durations, measures,
keys, times, clefs, ties, lyrics, directions). music21 is used when present; it
is not installed in the current environment, so only MuseScore is measured.

## Usage

```sh
scripts/oracle/musicxml/lots.sh /tmp/oracle-xml
nix develop -c cargo build -p scoretk-cli
scripts/oracle/musicxml/mesure.sh --out /tmp/oracle-xml \
  --manifest /tmp/oracle-xml/manifestes/mscz.txt \
  --scoretk target/debug/scoretk --mscore mscore
scripts/oracle/musicxml/mesure.sh --out /tmp/oracle-xml --resume
```

Freeze the binary under a stable path (`/tmp/scoretk-base`) before a campaign so
a rebuild cannot mix two versions in one report. Raw rows are in
`<out>/mesure.jsonl`, the human summary in `<out>/resume.md`. The baseline and
final tables are frozen in `docs/agents/ecarts-noyau.md` § 20.

Standalone XSD check of arbitrary files:

```sh
scripts/oracle/musicxml/validite.py --xsd-dir /tmp/xsd \
  /tmp/oracle-xml/travail/0/tk.musicxml
```

`validite.py` needs `xmllint` on `PATH` (`nix-shell -p libxml2`); `mesure.sh`
provides both `xmllint` and `mido`.

## Unsupported inputs

`scoretk` exits with **3** when the input is deliberately unsupported: a
MuseScore version outside the ported subset, or a format without a converter in
the requested direction (see `scoretk --help` and `docs/agents/scoretk.md`).
Such a file is *not* a conversion failure. `mesure.py` records it with an
`exclu` marker in `mesure.jsonl`, drops it from every numerator and denominator
(it is not converted by MuseScore either, so the batch stays identical for both
tools), prints one `AVERTISSEMENT : … non prise(s) en charge (exclues)` line and
writes `Unsupported (excluded): N` into `resume.md`. The rule is shared through
`scripts/oracle/common.py`, never copied per script. Example: the MuseScore
1.14 `Canon_in_D_Pachelbel_-_arranged_for_accordion.mscz` in an `.mscz`
manifest is excluded, and the score of every other file is unchanged.
EOF
echo "README écrit"