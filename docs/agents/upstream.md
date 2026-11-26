# MuseScore Studio upstream: reference and tracking

This document describes the version of MuseScore Studio whose code is ported, how
to obtain a local copy, the correspondence between C++ files
and Rust files, and the procedure to follow to absorb a new
upstream version.

## 1. Current reference

| Element | Value |
| --- | --- |
| Project | **MuseScore Studio**, <https://github.com/musescore/MuseScore> |
| Ported commit | `2b9e82df847111d673c2051a93deeaaeea3f5a29` (2026-11-16) |
| `version.cmake` | version **5.0.0** (`MUSE_APP_UNSTABLE ON`, `MUSE_APP_IS_PRERELEASE ON`) |
| Local copy | `ref-musescore/`, **not versioned** (`.gitignore`), read-only |
| Upstream licence | GNU GPL version 3 (`LICENSE.txt`), Copyright (C) 1999-2026 MuseScore Limited and others |

Checks made on 2026-11-17: `NOTICE`, `tests/corpus/SOURCES.md` and the
`version.cmake` of the upstream commit agree. The `ref-musescore/` copy does not
exist in a fresh checkout: recreate it with the procedure below.

Two references coexist, and they must not be confused:

- **ported source**: the 5.0.0 upstream snapshot above, for code
  structure and names (`read*.cpp`, `exportmusicxml.cpp`, `engraving/dom/…`);
- **behaviour reference**: **MuseScore 4.7.4** (nixpkgs package), which
  produced the `*.ref.musicxml` of the corpus. `DEFAULT_SOFTWARE` announces this
  version, and the default encoding date is frozen for reproducibility.

In case of divergence between the 5.0.0 upstream code and the 4.7.4 output, **the
4.7.4 reference wins** until an explicit decision has been made to
change the reference version (see § 4).

### Derived data

- **Corpus references** — `tests/corpus/{v3,v4}/*.ref.musicxml`, produced
  by `scripts/gen-references.sh` with MuseScore **4.7.4** from nixpkgs, run
  in headless mode (`QT_QPA_PLATFORM=offscreen`, `LD_LIBRARY_PATH` neutralised).
  The `.mscx` sources come from the same upstream commit (see
  `tests/corpus/SOURCES.md`), mainly from
  `src/importexport/musicxml/tests/data/` and `src/engraving/tests/`.
- **Instruments table** — `crates/scoretk-core/src/instruments.rs`, a
  **generated file** produced by `scripts/gen-instruments-table.py` from the
  5.0 snapshot's `share/instruments/instruments.xml` (identifiers, names,
  MusicXML ids, MIDI programs) and the **4.7.4** catalogue for staff counts
  and concert clefs (4 bits per staff). `NOTICE` records both sources; do not
  edit the table by hand, regenerate it. The 4.7.4 clefs matter for MIDI
  import: 5.0 changed 13 of them.
- **Name tables** — `read302/tables.rs` and `read4xx/names.rs` follow
  `TConv::fromXml` (`engraving/rw/types/typesconv.cpp`, a file **generated at
  build time** by MuseScore, therefore absent from the reference copy) and the
  values of `engraving/types/types.h`.

## 2. Obtaining `ref-musescore/`

Full clone at the exact commit (simpler, several hundred MB):

```sh
git clone https://github.com/musescore/MuseScore ref-musescore
git -C ref-musescore checkout 2b9e82df847111d673c2051a93deeaaeea3f5a29
git -C ref-musescore rev-parse HEAD   # doit afficher le commit ci-dessus
```

Partial clone, sufficient for the sources and the corpus (method used to
build `tests/corpus/`):

```sh
git clone --depth 1 --filter=blob:none --sparse \
    https://github.com/musescore/MuseScore ref-musescore
git -C ref-musescore sparse-checkout set \
    src/importexport/musicxml/tests/data \
    src/engraving/tests \
    src/engraving/rw src/engraving/dom src/engraving/types \
    src/engraving/style src/engraving/rendering src/engraving/infrastructure \
    src/importexport/musicxml/internal share/instruments
git -C ref-musescore fetch --depth 1 origin 2b9e82df847111d673c2051a93deeaaeea3f5a29
git -C ref-musescore checkout 2b9e82df847111d673c2051a93deeaaeea3f5a29
```

`ref-musescore/` is ignored by Git: never add it to the repository, never
copy a C++ file into the distributed sources (the port is a
rewrite, see `NOTICE`).

### 2.1 4.7.4 modules for MEI, Capella and Guitar Pro (2026-11-23)

The 5.0 snapshot of §2 does not contain those import/export modules. They are
kept in a **second, independent sparse checkout** *inside* the already-ignored
`ref-musescore/` tree, so the 5.0 checkout is untouched and nothing is
versioned:

```sh
git clone --depth 1 --filter=blob:none --sparse --branch v4.7.4 \
    https://github.com/musescore/MuseScore ref-musescore/.474
git -C ref-musescore/.474 sparse-checkout set \
    src/importexport/mei src/importexport/capella src/importexport/guitarpro \
    src/importexport/musicxml \
    src/engraving/dom src/engraving/types src/engraving/style \
    src/engraving/infrastructure src/engraving/rw src/engraving/editing \
    src/engraving/rendering src/engraving/data/styles \
    src/engraving/tests/utils
git -C ref-musescore/.474 rev-parse HEAD   # 7688c005ad963ba09ee715aae53dbc7e8c9d5ef8
git -C ref-musescore/.474 describe --tags --exact-match HEAD   # v4.7.4
```

The tag hash is the same one already used for the 4.7.4 beam/tuplet audit
(§ A). The engraving sets are the transitive `#include` closure of the three
modules: without `src/engraving/editing` and `src/engraving/tests/utils`, four
headers (`bracketItem.h`, `transpose.h`, `scorecomp.h`, `scorerw.h`) are
missing. `src/importexport/musicxml`, `src/engraving/rendering` and
`src/engraving/data/styles` were added for the MusicXML export campaign
(`measurelayout.cpp`, `exportmusicxml.cpp`, the legacy style defaults).
Verified locally: `find src/importexport/{mei,capella,guitarpro}` finds 884
files, and no `engraving/…` include is unresolved. `ref-musescore/.474`
is read-only reference data (unversioned, like its parent); do not compile, run
or copy code from it into the distributed sources.

Useful entry points: MEI mapping
`src/importexport/mei/internal/meiconverter.cpp:626-667` (beam modes) and
`:3828-3853` (`@tstamp` ↔ `Fraction`); Capella staff layouts
`src/importexport/capella/internal/capxml.cpp:936-990`; Guitar Pro tablature
and bends `src/importexport/guitarpro/internal/gtp/gpconverter.cpp:1152-1167`
(`StringData`), `:1280-1304` (`note->string()/fret()`, `GuitarBend`).

## 3. C++ → Rust correspondence

Upstream paths are relative to `ref-musescore/src/`. The Rust column is relative to the
repository root. This table is drawn from the
« Portage de … » headers of the Rust files: **update both the header and the table**
with every new port.

### Root, container and routing

| Rust | Upstream C++ |
| --- | --- |
| `crates/scoretk-musicxml-export/src/lib.rs` | `importexport/musicxml/internal/export/` |
| `crates/scoretk-musescore-import/src/container.rs` | `engraving/infrastructure/mscreader.cpp` |
| `crates/scoretk-musescore-import/src/read/mod.rs` | `engraving/rw/rwregister.cpp` |
| `crates/scoretk-musescore-import/src/read/instruments.rs` | `engraving/dom/instrument.cpp` (`updateInstrumentId`, `recognizeMidiProgram`) and `share/instruments/instruments.xml` |

### Model (`crates/scoretk-core/src/model/`)

| Rust | Upstream C++ |
| --- | --- |
| `crates/scoretk-core/src/model/mod.rs` | `engraving/dom/` |
| `crates/scoretk-core/src/model/annotation.rs` | `engraving/dom/{segment,dynamic,stafftext,systemtext,tempotext,rehearsalmark,instrument,harmony}.h` |
| `crates/scoretk-core/src/model/chord.rs` | `engraving/dom/{chordrest,chord,rest,articulation}.h` |
| `crates/scoretk-core/src/model/duration.rs` | `engraving/dom/durationtype.h`, `engraving/types/types.h` |
| `crates/scoretk-core/src/model/fraction.rs` | `engraving/types/fraction.h` |
| `crates/scoretk-core/src/model/hairpin.rs` | `engraving/dom/hairpin.h` |
| `crates/scoretk-core/src/model/ids.rs` | `engraving/dom/{segment,score}.h` |
| `crates/scoretk-core/src/model/lyrics.rs` | `engraving/dom/lyrics.h`, `engraving/types/types.h` |
| `crates/scoretk-core/src/model/measure.rs` | `engraving/dom/measure.h` |
| `crates/scoretk-core/src/model/notation.rs` | `engraving/dom/{fermata,ornament,tremolosinglechord,tremolotwochord,arpeggio,glissando,chordline,fingering}.h` |
| `crates/scoretk-core/src/model/note.rs` | `engraving/dom/{note,tie,pitchspelling}.h` |
| `crates/scoretk-core/src/model/part.rs` | `engraving/dom/{part,staff,instrument}.h` |
| `crates/scoretk-core/src/model/repeat.rs` | `engraving/dom/{marker,jump,volta}.h` |
| `crates/scoretk-core/src/model/score.rs` | `engraving/dom/score.h` |
| `crates/scoretk-core/src/model/segment.rs` | `engraving/dom/{segment,keysig,timesig,clef,barline}.h`, `engraving/dom/groups.cpp` (`TimeSig::groups`) |
| `crates/scoretk-core/src/model/spanner.rs` | `engraving/dom/spanner.h` |
| `crates/scoretk-core/src/model/style.rs` | `engraving/style/style.h`, `importexport/musicxml/internal/export/exportmusicxml.cpp` |
| `crates/scoretk-core/src/model/text.rs` | `engraving/dom/textbase.cpp`, `importexport/musicxml/internal/shared/musicxmlfonthandler.cpp`, `…/exportmusicxml.cpp` |
| `crates/scoretk-core/src/model/tuplet.rs` | `engraving/dom/tuplet.h` |

### 2.x reader (`crates/scoretk-musescore-import/src/read/read206/`)

The format has no published specification: the syntax follows `read206.cpp`, and
the behaviour reference remains MuseScore 4.7.4 (§1). `Read206` delegates
element reading to the common `read400` reader; the port therefore rewrites the
overall conventions (voices `<track>`/`<tick>`, `<Tie id>`/`<endSpanner id>`,
`<Tuplet id>`) before handing the tree to `read302`.

| Rust | Upstream C++ |
| --- | --- |
| `crates/scoretk-musescore-import/src/read206/mod.rs` | `engraving/rw/read206/read206.cpp` (`Read206::readScoreFile`, `readScoreTag`, `Score::connectTies`) |
| `crates/scoretk-musescore-import/src/read206/transform.rs` | `read206.cpp` (`readStaffContent206`, `readMeasure206`, `readChord`, `readNote206`, `readTuplet206`, `readTextStyle206`) and `rw/rwregister.cpp` |
| `crates/scoretk-musescore-import/src/read206/spanners.rs` | `read206.cpp` (`readVolta206`, `readSlur206`, `readHairpin206`, `readOttava`, `readMeasure206` pairing) and `dom/spanner.h` |
| `crates/scoretk-musescore-import/src/read206/xml.rs` | `engraving/rw/xmlreader.cpp` (reading attributes and mixed content) |

`src/engraving/rw/read114/` was consulted only to understand the
version thresholds of `rwregister.cpp`; it is not ported (versions `<= 114` are not supported).

### 3.x readers (`crates/scoretk-musescore-import/src/read/read302/`)

| Rust | Upstream C++ |
| --- | --- |
| `crates/scoretk-musescore-import/src/read302/mod.rs` | `engraving/rw/read302/read302.cpp`, `engraving/rw/read400/` |
| `crates/scoretk-musescore-import/src/read302/annotations.rs` | `engraving/rw/read400/tread.cpp` |
| `crates/scoretk-musescore-import/src/read302/chord.rs` | `engraving/rw/read400/tread.cpp` |
| `crates/scoretk-musescore-import/src/read302/credits.rs` | `engraving/rw/read302/read302.cpp`, `read400/tread.cpp` |
| `crates/scoretk-musescore-import/src/read302/measure.rs` | `engraving/rw/read400/{staffrw,measurerw}.cpp` |
| `crates/scoretk-musescore-import/src/read302/notations.rs` | `engraving/rw/read400/tread.cpp` |
| `crates/scoretk-musescore-import/src/read302/nuances.rs` | `engraving/rw/read400/tread.cpp` |
| `crates/scoretk-musescore-import/src/read302/part.rs` | `engraving/rw/read400/tread.cpp`, `read302/read302.cpp` |
| `crates/scoretk-musescore-import/src/read302/repeats.rs` | `engraving/rw/read400/tread.cpp` |
| `crates/scoretk-musescore-import/src/read302/spanner.rs` | `engraving/rw/read400/connectorinforeader.cpp` |
| `crates/scoretk-musescore-import/src/read302/tables.rs` | `engraving/types/types.h`, `engraving/rw/` (name tables) |
| `crates/scoretk-musescore-import/src/read302/textlines.rs` | `engraving/rw/read400/tread.cpp` |

### 4.x/5.x readers (`crates/scoretk-musescore-import/src/read/read4xx/`)

| Rust | Upstream C++ |
| --- | --- |
| `crates/scoretk-musescore-import/src/read4xx/mod.rs` | `engraving/rw/read400/read400.cpp`, `read410/read410.cpp`, `read460/read460.cpp`, `read500/read500.cpp` |
| `crates/scoretk-musescore-import/src/read4xx/names.rs` | `engraving/types/types.h`, `engraving/rw/` (name tables) |
| `crates/scoretk-musescore-import/src/read4xx/notations.rs` | `engraving/rw/read400/tread.cpp` |
| `crates/scoretk-musescore-import/src/read4xx/nuances.rs` | `engraving/rw/read410/tread.cpp` |
| `crates/scoretk-musescore-import/src/read4xx/ottava.rs` | `engraving/rw/read400/connectorinforeader.cpp` |
| `crates/scoretk-musescore-import/src/read4xx/pedals.rs` | `engraving/rw/read400/{connectorinforeader,tread}.cpp` |
| `crates/scoretk-musescore-import/src/read4xx/repeats.rs` | `engraving/rw/read400/{tread,connectorinforeader}.cpp` |
| `crates/scoretk-musescore-import/src/read4xx/style.rs` | `engraving/rw/compat/readstyle.cpp` |
| `crates/scoretk-musescore-import/src/read4xx/textlines.rs` | `engraving/rw/read400/{connectorinforeader,tread}.cpp` |

### Export (`crates/scoretk-musicxml-export/src/`)

| Rust | Upstream C++ |
| --- | --- |
| `crates/scoretk-musicxml-export/src/lib.rs` | `importexport/musicxml/internal/export/` |
| `crates/scoretk-musicxml-export/src/exportmusicxml.rs` | `importexport/musicxml/internal/export/exportmusicxml.cpp`, `importexport/musicxml/internal/shared/` |
| `crates/scoretk-musicxml-export/src/beams.rs` | `engraving/rendering/score/beamlayout.cpp`, `engraving/dom/groups.cpp`, `…/exportmusicxml.cpp` |
| `crates/scoretk-musicxml-export/src/harmony.rs` | `…/exportmusicxml.cpp`, `engraving/dom/chordlist.cpp` |
| `crates/scoretk-musicxml-export/src/layout.rs` | `…/exportmusicxml.cpp` (`defaults`, `writePageFormat`, `textAsCreditWords`) |
| `crates/scoretk-musicxml-export/src/notations.rs` | `…/exportmusicxml.cpp` (`chordAttributes`, `fermata`, `symIdToArtic`) |
| `crates/scoretk-musicxml-export/src/nuances.rs` | `…/exportmusicxml.cpp` (`dynamic`, `hairpin`) |
| `crates/scoretk-musicxml-export/src/ottava.rs` | `…/exportmusicxml.cpp` (`ottava`) |
| `crates/scoretk-musicxml-export/src/pedals.rs` | `…/exportmusicxml.cpp`, `importexport/musicxml/internal/shared/` |
| `crates/scoretk-musicxml-export/src/pitch.rs` | `…/exportmusicxml.cpp` (`tpc2stepName`), `engraving/dom/pitchspelling.cpp` |
| `crates/scoretk-musicxml-export/src/repeats.rs` | `…/exportmusicxml.cpp` (`barlineLeft/Middle/Right`, markers, jumps) |
| `crates/scoretk-musicxml-export/src/stems.rs` | `engraving/rendering/score/chordlayout.cpp` |
| `crates/scoretk-musicxml-export/src/text.rs` | `…/exportmusicxml.cpp`, `…/shared/musicxmlfonthandler.cpp`, `engraving/dom/tempotext.cpp` |
| `crates/scoretk-musicxml-export/src/textlines.rs` | `…/exportmusicxml.cpp` (`textLine`, `writeHairpinText`) |
| `crates/scoretk-musicxml-export/src/writer.rs` | `engraving/rw/xmlwriter.cpp` |

### MusicXML reader (`crates/scoretk-musicxml-import/`)

This crate is outside `crates/scoretk-core` but ports the same upstream code; the
Rust paths there are relative to `crates/scoretk-musicxml-import/src/`.

| Rust | Upstream C++ |
| --- | --- |
| `pass1.rs` | `importexport/musicxml/internal/import/importmusicxmlpass1.cpp`, `musicxmlpart.cpp`, `musicxmlvoicedesc.cpp` |
| `pass2.rs` | `importexport/musicxml/internal/import/importmusicxmlpass2.cpp`, `importmusicxmlnotepitch.cpp`, `musicxmltupletstate.cpp` |
| `noteduration.rs` | `importexport/musicxml/internal/import/importmusicxmlnoteduration.cpp` |
| `container.rs` | `importexport/musicxml/internal/import/importmusicxml.cpp` (reading `.mxl`) |
| `xml.rs` | replaces `muse::XmlStreamReader` with an in-memory tree (original file) |

The discrepancies of this import are tracked in `docs/agents/musicxml-import.md`.

### MuseScore writing (`crates/scoretk-musescore-export/src/`)

This crate is outside `crates/scoretk-core` but ports the same upstream code; the
Rust paths there are relative to `crates/scoretk-musescore-export/src/`.

| Rust | Upstream C++ |
| --- | --- |
| `write/writer.rs` | `engraving/rw/write/writer.cpp` (`Writer::writeScore`) |
| `write/staffwrite.rs` | `engraving/rw/write/staffwrite.cpp`, `engraving/dom/` (parts, staves, frames, credits) |
| `write/measurewrite.rs` | `engraving/rw/write/measurewrite.cpp` (`MeasureWrite::writeMeasure`) |
| `write/twrite.rs` | `engraving/rw/write/twrite.cpp` (`TWrite::writeItem`, `writeProperties`) |
| `write/connectorinfowriter.rs` | `engraving/rw/write/connectorinfowriter.cpp` |
| `write/context.rs` | `engraving/rw/write/writecontext.h` |
| `xmlwriter.rs` | `engraving/rw/xmlwriter.cpp` |
| `names.rs` | `engraving/types/types.h`, `engraving/rw/` (`TConv::toXml` tables) |
| `text.rs` | `engraving/dom/textbase.cpp` (markup writing) |
| `container.rs` | `engraving/infrastructure/mscwriter.cpp` |

### MEI (`crates/scoretk-mei-{common,import,export}/src/`)

| Rust | Upstream C++ (4.7.4, `src/importexport/mei/internal/`) |
| --- | --- |
| `scoretk-mei-common/src/convert.rs` | `meiconverter.cpp`, `meiconverter.h` |
| `scoretk-mei-common/src/ids.rs` | `meiconverter.h:64-89` (`mscore-*` `@type` values) |
| `scoretk-mei-import/src/{lib,reader,header,controls}.rs` | `meiimporter.cpp`, `meireader.cpp` |
| `scoretk-mei-export/src/{lib,header,writer}.rs` | `meiexporter.cpp`, `meiwriter.cpp`, `meiconfiguration.cpp` |

### MIDI (`crates/scoretk-midi-common/src/`, `crates/scoretk-midi-import/src/`, `crates/scoretk-midi-export/src/`)

Writing (t35):

| Rust | Upstream C++ |
| --- | --- |
| `scoretk-midi-export/src/lib.rs` | `importexport/midi/internal/midiexport/exportmidi.cpp`, `engraving/compat/midi/compatmidirender.cpp` |
| `scoretk-midi-export/src/repeat_list.rs` | `engraving/dom/repeatlist.cpp` |
| `scoretk-midi-export/src/render.rs` | `engraving/compat/midi/{compatmidirender,compatmidirenderinternal}.cpp` |
| `scoretk-midi-export/src/velocity.rs` | `engraving/compat/midi/velocitymap.cpp` |
| `scoretk-midi-export/src/pause_map.rs` | `engraving/compat/midi/pausemap.cpp` |
| `scoretk-midi-export/src/writer.rs` | `MidiFile` writing (SMF format, `no_std`) |

Reading (t42, `import/`):

| Rust | Upstream C++ |
| --- | --- |
| `scoretk-midi-import/src/lib.rs` | `importexport/midi/internal/midiimport/importmidi.cpp` |
| `scoretk-midi-import/src/midifile.rs` | `importexport/midi/internal/midishared/{midifile,midievent}.cpp` |
| `scoretk-midi-import/src/timesigmap.rs` | `engraving/dom/sig.cpp` |
| `scoretk-midi-import/src/chord.rs` | `importexport/midi/internal/midiimport/importmidi_chord.cpp` |
| `scoretk-midi-import/src/quant.rs` | `importexport/midi/internal/midiimport/importmidi_quant.cpp` |
| `scoretk-midi-import/src/meter.rs` | `importexport/midi/internal/midiimport/importmidi_meter.cpp`, `engraving/dom/durationtype.cpp` |
| `scoretk-midi-import/src/simplify.rs` | `importexport/midi/internal/midiimport/importmidi_simplify.cpp` |
| `scoretk-midi-import/src/tuplet.rs` | `importexport/midi/internal/midiimport/importmidi_tuplet*.cpp` |
| `scoretk-midi-import/src/voice.rs` | `importexport/midi/internal/midiimport/importmidi_voice.cpp` |
| `scoretk-midi-import/src/key.rs` | `importexport/midi/internal/midiimport/importmidi_key.cpp` |
| `scoretk-midi-import/src/clef.rs` | `importexport/midi/internal/midiimport/importmidi_clef.cpp` |
| `scoretk-midi-import/src/lrhand.rs` | `importexport/midi/internal/midiimport/importmidi_lrhand.cpp` |
| `scoretk-midi-import/src/drum.rs` | `importexport/midi/internal/midiimport/importmidi_drum.cpp` |
| `scoretk-midi-import/src/lyrics.rs` | `importexport/midi/internal/midiimport/importmidi_lyrics.cpp` |
| `scoretk-midi-import/src/tempo.rs` | `importexport/midi/internal/midiimport/importmidi_tempo.cpp` |
| `scoretk-midi-import/src/instrument.rs` | `importexport/midi/internal/midiimport/importmidi_instrument.cpp`, `midishared/generalmidi.cpp` |
| `scoretk-midi-import/src/operations.rs` | `importexport/midi/internal/midiimport/importmidi_operations.h` |

### LilyPond writing (`crates/scoretk-lilypond-export/src/`)

This crate has **no** dedicated MuseScore source: MuseScore 4 has no
LilyPond export. It ports the semantics of the model objects and follows, for
notation mappings, the behaviour of **`musicxml2ly`** from the
LilyPond package (GPL-3.0-or-later), cited in `NOTICE` and in the headers.
**Reading** (t40) additionally ports a subset of `lily/parser.yy`,
`lily/lexer.ll`, `lily/pitch.cc` and `scm/define-note-names.scm` from LilyPond
2.26.0 (copy outside the repository); the full mappings and the
upgrade procedures of these engines (LilyPond, `musicxml2ly`, `python-ly`) are in
`docs/agents/lilypond.md` § 8 to § 10.

| Rust | Reference |
| --- | --- |
| `chord.rs` | `engraving/dom/{chordrest,chord,rest,articulation}.h` ; articulation and ornament table of `musicxml2ly` |
| `structure.rs` | `engraving/dom/{volta,measure}.h`, `ExportMusicXml::barlineRight` |
| `pitch.rs` | `engraving/dom/pitchspelling.cpp` (`tpc2stepName`, `tpc2alter`), `engraving/dom/clef.h` |
| `voice.rs` | `engraving/dom/{segment,measure,chordrest}.h` (voice traversal) |
| `scoretk-midi-export/src/writer.rs` | `engraving/dom/{part,staff,score}.h` (groupings and header) |

## 4. Tracking a new upstream version

Prepare one batch per family (container, 3.x reader, 4.x reader, model,
export) and one commit per batch. Never mix the port of an upstream diff
with an independent parity fix.

1. **Fetch and identify.**

   ```sh
   git -C ref-musescore fetch origin
   git -C ref-musescore log -1 --format='%H %ad %s' origin/master
   git -C ref-musescore show origin/master:version.cmake | grep MUSE_APP_VERSION
   ```

   Note the old commit (`2b9e82d…` or the last tracked one) and the new one.

2. **List changes in the ported paths only.** The exact paths are
   those of § 3; for a first overview, restrict to the ported directories:

   ```sh
   git -C ref-musescore log --oneline <ancien>..<nouveau> -- \
     src/importexport/musicxml/internal/export \
     src/importexport/musicxml/internal/shared \
     src/importexport/musicxml/tests/data \
     src/engraving/rw src/engraving/dom src/engraving/types \
     src/engraving/style src/engraving/rendering/score \
     src/engraving/infrastructure/mscreader.cpp
   git -C ref-musescore diff --stat <ancien>..<nouveau> -- <chemins>
   git -C ref-musescore diff <ancien>..<nouveau> -- <fichier>
   ```

   `src/engraving/dom/` is vast: then filter by file from the table
   (§ 3) to avoid the noise (GUI, playback, unported layout). The name tables
   (`typesconv.cpp`) are **generated at build time** by MuseScore and absent from the
   copy: in that case check `engraving/types/types.h` and the values of `TConv::fromXml`.

3. **Sort.** Discard what is out of scope (UI, playback,
   unported engraving). For each retained change, note the C++ file,
   the observable behaviour and the Rust file to modify.

4. **Regenerate the derived data** with the target version:

   ```sh
   # Instruments table: 5.0 snapshot for names/programs, 4.7.4 XML for
   # staff counts and clefs, 5.0 C++ for GM1_SOUND_DATA.
   curl -o /tmp/instruments-4.7.4.xml \
     https://raw.githubusercontent.com/musescore/MuseScore/v4.7.4/share/instruments/instruments.xml
   curl -o /tmp/importmidi_instrument.cpp \
     https://raw.githubusercontent.com/musescore/MuseScore/2b9e82df847111d673c2051a93deeaaeea3f5a29/src/importexport/midi/internal/midiimport/importmidi_instrument.cpp
   python3 scripts/gen-instruments-table.py \
     ref-musescore/share/instruments/instruments.xml \
     /tmp/instruments-4.7.4.xml /tmp/importmidi_instrument.cpp \
     crates/scoretk-core/src/instruments.rs

   # Corpus references (MuseScore 4.7.4 by default, headless)
   scripts/gen-references.sh            # tout le corpus
   scripts/gen-references.sh tests/corpus/v4/foo.mscx   # un fichier
   ```

   Change the version of MuseScore producing the references only after an
   explicit decision: it changes the compliance target, and requires
   rerunning the whole corpus and updating `NOTICE`, `tests/corpus/SOURCES.md`,
   `DEFAULT_SOFTWARE` (`export/exportmusicxml.rs`) and the discrepancy documents.

5. **Port the diffs file by file**, in the order reader → model →
   export, respecting the conventions of `docs/agents/developpement.md`:
   new code in new files of the family, updated « Portage de » header,
   comments citing the C++ names (`TRead::read(Chord*)`,
   `ExportMusicXml::chord`, …). Do not rewrite from memory: reread the C++ and
   the upstream tests.

6. **Verify.** End-to-end compliance must not regress:

   ```sh
   nix develop -c cargo test --workspace
   nix develop -c cargo test -p scoretk --test e2e_corpus -- --nocapture
   nix flake check
   ```

   The e2e test has two levels (musical core, then full parity); read the
   table and `docs/agents/ecarts-{noyau,complets}.md`. A file that becomes
   compliant is added to `EXPECTED_CONFORMANT` **and** documented; a new
   discrepancy is justified or fixed.

7. **Update traceability**: `NOTICE` (new commit, date, version,
   any new sources or data), this document (reference,
   C++ → Rust table), the headers of the touched Rust files, and
   `tests/corpus/SOURCES.md` if the corpus changes.

**Acceptance criterion for a tracking run**: `cargo test --workspace` and
`nix flake check` pass, and e2e compliance is not degraded compared
to the previous state. Any degradation must be fixed in the batch, or
documented and explicitly validated before merging.

## A. 4.7.4 beam/tuplet audit (2026-11-22)

To test whether invariant 2 (4.7.4 behaviour prevails) could explain the
remaining `tuplet bracket` divergences, tag `v4.7.4`
(`7688c005ad963ba09ee715aae53dbc7e8c9d5ef8`) was fetched **read-only and
outside the repository** (sparse clone under the session's scratch directory).
A persistent 4.7.4 sparse checkout now lives at `ref-musescore/.474` (§2.1);
it is inside the ignored reference tree and therefore still unversioned.

| File | 4.7.4 | 5.0 snapshot | Result |
| --- | --- | --- | --- |
| `src/engraving/dom/groups.cpp` | `baseBeamMode` `:107-161`, tuplet-change rule `:143-151` | identical | no diff |
| `src/engraving/dom/tuplet.cpp` | `calcHasBracket` `:206-280` | identical | no diff |
| `src/engraving/rendering/score/beamlayout.cpp` | `createBeams` `:624+` | `createBeams` `:653+` | only Jianpu rests and a `createBeamIfNeed` refactor |

**Conclusion:** the 4.7.4 *rules* for beams and tuplets are the same as the
ported 5.0 ones. The observed difference is input data, not code: the scores
carry staff `<Groups>` (read at 4.7.4
`src/engraving/rw/read400/tread.cpp:4084`, `:4035-4038` in the 5.0 snapshot)
and explicit `<BeamMode>` values, neither of which is in the pivot model, and
`calcHasBracket` additionally uses `calcBeamBreaks`, `headSide` and `isCross`
from the layout. An attempt to port `<Groups>` and approximate the
beam-boundary check lost lines in 12 files (69 → 64 core-identical,
`target/xml-beam474/`) and was reverted: the simplified check omitted those
layout predicates. It is **not** a 4.7.4-vs-5.0 behavioural divergence. The
class was closed in `docs/agents/musicxml-export.md` §51: the exact
`calcHasBracket`/`calcBeamBreaks` predicates are portable once `TimeSig::groups`,
`TDuration` dots and nested tuplets are modelled; 169 → 171/173 core-exact.
