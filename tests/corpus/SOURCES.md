# Test corpus provenance

The `.mscx` files in this directory are copied unmodified from the
[MuseScore](https://github.com/musescore/MuseScore) source tree.

- Upstream: https://github.com/musescore/MuseScore
- Commit: `2b9e82df847111d673c2051a93deeaaeea3f5a29` (2026-11-16)
- Fetched with: `git clone --depth 1 --filter=blob:none --sparse`, then
  `git sparse-checkout set src/importexport/musicxml/tests/data src/engraving/tests`
- License: GNU General Public License version 3 (see `LICENSE.txt` upstream).
  MuseScore Studio, Copyright (C) 1999-2026 MuseScore Limited and others.

The reference MusicXML files (`*.ref.musicxml`) are produced from these
sources by `scripts/gen-references.sh` using MuseScore:

- MuseScore 4.7.4 (`mscore`, nixpkgs), run headless
  (`QT_QPA_PLATFORM=offscreen`).

## Format 3.x (`v3/`, `museScore version="3.01"`)

| File | Upstream path |
| --- | --- |
| `moonlight.mscx` | `src/engraving/tests/all_elements_data/moonlight.mscx` |
| `layout_elements.mscx` | `src/engraving/tests/all_elements_data/layout_elements.mscx` |
| `grace.mscx` | `src/engraving/tests/note_data/grace.mscx` |
| `keysig_78216.mscx` | `src/engraving/tests/keysig_data/keysig_78216.mscx` |
| `clef-1.mscx` | `src/engraving/tests/clef_data/clef-1.mscx` |
| `voices.mscx` | `src/engraving/tests/parts_data/voices.mscx` |
| `repeat01.mscx` | `src/engraving/tests/repeat_data/repeat01.mscx` |
| `copypaste_tuplet_01.mscx` | `src/engraving/tests/copypaste_data/copypaste_tuplet_01.mscx` |
| `barline01.mscx` | `src/engraving/tests/barline_data/barline01.mscx` |
| `lyricsline01.mscx` | `src/engraving/tests/spanners_data/lyricsline01.mscx` |
| `testChordSymbols.mscx` | `src/importexport/musicxml/tests/data/testChordSymbols.mscx` |
| `testTuplets8.mscx` | `src/importexport/musicxml/tests/data/testTuplets8.mscx` |
| `testMultiInstrumentPart3.mscx` | `src/importexport/musicxml/tests/data/testMultiInstrumentPart3.mscx` |
| `testHarmony6.mscx` | `src/importexport/musicxml/tests/data/testHarmony6.mscx` |
| `testHairpinDynamics.mscx` | `src/importexport/musicxml/tests/data/testHairpinDynamics.mscx` |
| `testBreaksPage.mscx` | `src/importexport/musicxml/tests/data/testBreaksPage.mscx` |

## Format 4.x (`v4/`)

| File | Version | Upstream path |
| --- | --- | --- |
| `part-textlines.mscx` | 4.10 | `src/engraving/tests/parts_data/part-textlines.mscx` |
| `linked-ties.mscx` | 4.40 | `src/engraving/tests/parts_data/linked-ties.mscx` |
| `copypaste_parts.mscx` | 4.40 | `src/engraving/tests/copypaste_data/copypaste_parts.mscx` |
| `hairpin_dynamic_inside.mscx` | 4.10 | `src/engraving/tests/midi/midirenderer_data/hairpin_dynamic_inside.mscx` |
| `dynamics.mscx` | 4.20 | `src/engraving/tests/playback/playbackmodel_data/dynamics/dynamics.mscx` |
| `tenuto_accent.mscx` | 4.00 | `src/engraving/tests/playback/playbackeventsrenderer_data/single_note_tenuto_accent/tenuto_accent.mscx` |
| `repeat65.mscx` | 4.00 | `src/engraving/tests/repeat_data/repeat65.mscx` |
| `beamStemDir.mscx` | 4.10 | `src/engraving/tests/beam_data/beamStemDir.mscx` |
| `mmrest.mscx` | 4.00 | `src/engraving/tests/measure_data/mmrest.mscx` |
| `timesig-09.mscx` | 4.20 | `src/engraving/tests/timesig_data/timesig-09.mscx` |
| `testLocalTimesig1.mscx` | 4.20 | `src/importexport/musicxml/tests/data/testLocalTimesig1.mscx` |
| `testNoteheads2.mscx` | 4.20 | `src/importexport/musicxml/tests/data/testNoteheads2.mscx` |
| `testExcludeInvisibleElements.mscx` | 4.00 | `src/importexport/musicxml/tests/data/testExcludeInvisibleElements.mscx` |
| `voiceswitching.mscx` | 4.50 | `src/engraving/tests/voiceswitching_data/voiceswitching.mscx` |
| `changevisibility.mscx` | 4.00 | `src/engraving/tests/changevisibility_data/changevisibility.mscx` |
| `copypastesymbollist-lyrics.mscx` | 4.50 | `src/engraving/tests/copypastesymbollist_data/copypastesymbollist-lyrics.mscx` |

## Files that fail to export

Export test run on 2026-11-16 with MuseScore 4.7.4 : les 32 fichiers
ci-dessus ont tous été exportés correctement (`scripts/gen-references.sh`,
code de sortie 0). Aucun fichier n'a dû être retiré.
