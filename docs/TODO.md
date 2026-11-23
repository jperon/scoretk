# TODO

Only outstanding work. Finished items are in the code and in the per-format
reports under `docs/agents/`.

## Reliability campaign

Exact files against an independent oracle, figures of 2026-11-23:

| Direction | scoretk | Reference alternative | Target |
| --- | ---: | ---: | --- |
| LilyPond import | 1 661 / 1 724 (96.3 %) | — | ≥ 98 % (95 % is a floor) |
| ABC import | 4 721 / 4 756 | abc2xml 4 655 | beat the alternative |
| MIDI import | 1 026 / 1 029 | — | 100 % (3 non-reader causes, documented) |
| MIDI export | 171 / 171 | — | 100 % |
| LilyPond export | 1 441 / 1 447 | musicxml2ly 1 282 | 100 % |
| ABC export | 250 / 261 | xml2abc 101 | 100 % |
| MusicXML export | 157 / 173 | — | 100 % |

Export target is 100 %, except for a file whose reference is wrong or whose
notation depends on Scheme/esoteric features; each exception is documented
individually. Method (oracle, graduated measurement, no tolerated per-file
loss): `docs/agents/plan-fiabilite.md`.

## New formats

In this order. Each one gets the same architecture —
`scoretk-<fmt>-common` / `scoretk-<fmt>-import` / `scoretk-<fmt>-export`,
symmetric `<fmt>-import` / `<fmt>-export` facade features, and wasm — with the
specification read first and an oracle plus a controlled measurement from day
one.

1. **GABC** — import and export. Mapping to modern notation based on gabctk
   (`jperon/gabctk`, the author's own project); Gregorio's grammar as the
   syntax reference.
2. **MEI** — import and export. Port MuseScore 4.7.4's MEI module.
3. **Capella** — import only. Port MuseScore 4.7.4's Capella import.
4. **Guitar Pro** — import only. Port MuseScore 4.7.4's Guitar Pro import.
5. **Humdrum** (`**kern`) — last. Candidate sources: humlib (Craig Sapp, BSD-2)
   for parsing and Verovio's `iohumdrum.cpp` (LGPL-3) for the notation
   mapping, possibly through the MEI work; a better GPL-3-compatible source
   may be chosen after a licence check. Verify licences when porting.

**Prerequisite for MEI, Capella and Guitar Pro:** `ref-musescore/` currently
holds only `src/importexport/midi` and `src/importexport/musicxml`.
Sparse-fetch the missing modules at the 4.7.4 tag, following
`docs/agents/upstream.md`.

## Standing constraints

- The library stays portable: no `fs`/`env`/`net`/`process`, no threads, no
  clock and no C dependency in the model core; wasm (cantus), Android and iOS
  must keep building.
- Do not port `read114` (MuseScore 1.x) without a real need: one 1.14 file in
  the corpus, which gets a clear "unsupported version" error.
