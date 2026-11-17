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

The reference MIDI files (`midi/<version>/*.ref.mid`) are produced from the
same sources by `scripts/gen-midi-references.sh`, with the same MuseScore
version and the same headless mode; only a small subset of the corpus is
exported (short scores covering repeats, voltas, jumps, dynamics, hairpins,
tuplets, ties, grace notes, articulations and tremolos).

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

## Corpus enrichment by family

Added on 2026-11-16 for the texts, repeats and dynamics iterations. Files
are copied unmodified from the same upstream commit.

### Texts (8 files)

| File | Version | Upstream path | Element covered |
| --- | --- | --- | --- |
| `v3/barlinedelete.mscx` | 3.01 | `src/engraving/tests/barline_data/barlinedelete.mscx` | staff text, system text, tempo text, rehearsal mark, title/composer metadata |
| `v3/testTboxWords1.mscx` | 3.01 | `src/importexport/musicxml/tests/data/testTboxWords1.mscx` | title, composer and lyricist text boxes, staff text |
| `v3/Beam-CrossM1.mscx` | 3.01 | `src/engraving/tests/beam_data/Beam-CrossM1.mscx` | title and subtitle text boxes (credits) |
| `v3/change.mscx` | 3.01 | `src/engraving/tests/instrumentchange_data/change.mscx` | instrument change indication, title |
| `v3/undoResequenceMeasure.mscx` | 3.01 | `src/engraving/tests/tools_data/undoResequenceMeasure.mscx` | rehearsal marks (numeric, alphabetic, custom) |
| `v4/expression-1.mscx` | 4.00 | `src/engraving/tests/expression_data/expression-1.mscx` | expression text |
| `v4/partExclusion.mscx` | 4.20 | `src/engraving/tests/parts_data/partExclusion.mscx` | staff text and system text |
| `v4/testTempoPrecision.mscx` | 4.00 | `src/importexport/musicxml/tests/data/testTempoPrecision.mscx` | tempo with metronome mark, followed by tempo text |

### Repeats (8 files)

| File | Version | Upstream path | Element covered |
| --- | --- | --- | --- |
| `v3/repeat06.mscx` | 3.01 | `src/engraving/tests/repeat_data/repeat06.mscx` | simple volta, three endings, repeat count 2 |
| `v3/repeat07.mscx` | 3.01 | `src/engraving/tests/repeat_data/repeat07.mscx` | Fine, D.C. al Fine |
| `v3/repeat11.mscx` | 3.01 | `src/engraving/tests/repeat_data/repeat11.mscx` | Segno, To Coda, Coda, D.S. al Coda, volta |
| `v3/repeat20.mscx` | 3.01 | `src/engraving/tests/repeat_data/repeat20.mscx` | D.S. al Coda then D.S. al Fine, two segnos, Fine |
| `v3/repeat39.mscx` | 3.01 | `src/engraving/tests/repeat_data/repeat39.mscx` | alternating voltas, repeat count 3 |
| `v4/repeat66.mscx` | 4.00 | `src/engraving/tests/repeat_data/repeat66.mscx` | repeat count 3, final-repeat reset across sections, D.C. |
| `v4/repeat68.mscx` | 4.00 | `src/engraving/tests/repeat_data/repeat68.mscx` | volta/ending at end of score |
| `v4/repeat69.mscx` | 4.50 | `src/engraving/tests/repeat_data/repeat69.mscx` | D.S. to the same Segno twice |

### Dynamics and hairpins (8 files)

| File | Version | Upstream path | Element covered |
| --- | --- | --- | --- |
| `v3/selectionfilter1.mscx` | 3.01 | `src/engraving/tests/selectionfilter_data/selectionfilter1.mscx` | dynamic `f`, crescendo hairpin |
| `v3/testWedge4.mscx` | 3.01 | `src/importexport/musicxml/tests/data/testWedge4.mscx` | three hairpins (crescendo/diminuendo) |
| `v3/timesig-05.mscx` | 3.01 | `src/engraving/tests/timesig_data/timesig-05.mscx` | dynamic `p`, crescendo and diminuendo |
| `v3/measure-4.mscx` | 3.01 | `src/engraving/tests/measure_data/measure-4.mscx` | diminuendo hairpin |
| `v4/hairpin_simple.mscx` | 4.10 | `src/engraving/tests/midi/midirenderer_data/hairpin_simple.mscx` | `p` to `f` crescendo |
| `v4/hairpin_dynamic_jump.mscx` | 4.10 | `src/engraving/tests/midi/midirenderer_data/hairpin_dynamic_jump.mscx` | `f`/`mf`/`ff` inside and around hairpins |
| `v4/hairpins_and_repeats.mscx` | 4.40 | `src/engraving/tests/playback/playbackcontext_data/dynamics/hairpins_and_repeats.mscx` | `p`/`f`/`fff`/`ppp` with three hairpins |
| `v4/dynamics_niente.mscx` | 4.40 | `src/engraving/tests/playback/playbackcontext_data/dynamics/dynamics_niente.mscx` | `f` to niente, crescendo/diminuendo |

## Files that fail to export

Export test run on 2026-11-16 with MuseScore 4.7.4 : les 56 fichiers
ci-dessus (dont les 24 ajouts) ont tous été exportés correctement
(`scripts/gen-references.sh`, code de sortie 0). Aucun fichier n'a dû être
retiré.

## MIDI references

Les fichiers `midi/{v3,v4}/<nom>.ref.mid` sont produits par
`scripts/gen-midi-references.sh` (MuseScore **4.7.4**, headless) à partir des
`.mscx` ci-dessus. Les fichiers `midi/import/{v3,v4}/<nom>.ref.musicxml` sont
produits par `scripts/gen-midi-import-references.sh` : MuseScore importe chaque
`.ref.mid` avec ses options par défaut et exporte le résultat en MusicXML. Ce
sont des sorties du programme MuseScore, couvertes par la même licence
(GPL-3.0, Copyright (C) MuseScore Limited and others).
