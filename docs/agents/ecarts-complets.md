# Full discrepancies on the corpus (full parity)

This document is produced by the `ecarts_complets_document` test in `crates/scoretk/tests/e2e_corpus.rs`. It compares each `.mscx` to its `X.ref.musicxml` reference after removing **pure layout** only: attributes `default-x`, `default-y`, `relative-x`, `relative-y`, `width`, `<defaults>`, `<print>` blocks (and their `<system-layout>`/`<staff-layout>`) and `<encoding-date>`. Everything else is compared: `<stem>`, `<beam>`, `<barline>`, `<direction>`, `<notations>`, `<credit>`, `<part-list>`, `<identification>`, etc.

## Table by file

### Corpus v3

| File | Differing lines |
| --- | ---: |
| Beam-CrossM1 | **identical** |
| barline01 | **identical** |
| barlinedelete | **identical** |
| change | **identical** |
| clef-1 | **identical** |
| copypaste_tuplet_01 | **identical** |
| grace | **identical** |
| keysig_78216 | **identical** |
| layout_elements | **identical** |
| lyricsline01 | **identical** |
| measure-4 | **identical** |
| moonlight | 18 |
| repeat01 | **identical** |
| repeat06 | **identical** |
| repeat07 | **identical** |
| repeat11 | **identical** |
| repeat20 | **identical** |
| repeat39 | **identical** |
| selectionfilter1 | **identical** |
| testBreaksPage | **identical** |
| testChordSymbols | **identical** |
| testHairpinDynamics | **identical** |
| testHarmony6 | **identical** |
| testMultiInstrumentPart3 | **identical** |
| testTboxWords1 | **identical** |
| testTuplets8 | **identical** |
| testWedge4 | **identical** |
| timesig-05 | **identical** |
| undoResequenceMeasure | **identical** |
| voices | 6 |

### Corpus v4

| File | Differing lines |
| --- | ---: |
| beamStemDir | **identical** |
| changevisibility | **identical** |
| copypaste_parts | **identical** |
| copypastesymbollist-lyrics | **identical** |
| dynamics | **identical** |
| dynamics_niente | **identical** |
| expression-1 | **identical** |
| hairpin_dynamic_inside | **identical** |
| hairpin_dynamic_jump | **identical** |
| hairpin_simple | **identical** |
| hairpins_and_repeats | **identical** |
| linked-ties | **identical** |
| mmrest | **identical** |
| part-textlines | **identical** |
| partExclusion | **identical** |
| repeat65 | **identical** |
| repeat66 | **identical** |
| repeat68 | **identical** |
| repeat69 | **identical** |
| tenuto_accent | **identical** |
| testExcludeInvisibleElements | **identical** |
| testLocalTimesig1 | **identical** |
| testNoteheads2 | **identical** |
| testTempoPrecision | **identical** |
| timesig-09 | **identical** |
| voiceswitching | **identical** |

## Discrepancies grouped by tag

Frequency = number of modified canonical lines across both corpora; files = number of affected files. Sorted by decreasing frequency.

| Tag | Lines | Files | Examples |
| --- | ---: | ---: | --- |
| `stem` | 18 | 1 | `<stem>down</stem>` `<stem>up</stem>` |
| `barline` | 4 | 1 | `<barline location="right">` `</barline>` |
| `bar-style` | 2 | 1 | `<bar-style>light-light</bar-style>` |

## Discrepancies that depend on computed layout

Some discrepancies do not come from a datum of the `.mscx` but from an object that MuseScore builds during layout, absent from the model. They are irreducible without porting the layout engine:

- **stem direction of a cross beam** (`v3/moonlight`, 9 notes): `BeamLayout::layout1` sets the beam direction from `computeUpForMovedCross` and `isBelowCrossBeam`, hence from beam grouping and note geometry, not from the file's `<Beam><StemDirection>`;
- **courtesy barlines of a system cut by page width** (`v3/voices`, 2 `light-light` barlines): `MeasureLayout::createEndBarLines` places the double barline only if the measure is the last in the system (`isLastMeasureInSystem`) and the next one changes key signature. When the system break appears in the file (`<LayoutBreak>`), it is reproduced (see `export/systems.rs` and `read/measure_repeat.rs`); an automatic cut depends on the computed width of measures, hence on the whole horizontal layout.

## Diffs

```diff

=== v3/moonlight: 18 differing lines ===
--- reference
+++ sortie
@@ -3465,5 +3465,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>down</stem>
+        <stem>up</stem>
         <staff>2</staff>
         <beam number="1">begin</beam>
@@ -3485,5 +3485,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>down</stem>
+        <stem>up</stem>
         <staff>2</staff>
         <beam number="1">continue</beam>
@@ -3501,5 +3501,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>down</stem>
+        <stem>up</stem>
         <staff>2</staff>
         <beam number="1">end</beam>
@@ -6896,5 +6896,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>up</stem>
+        <stem>down</stem>
         <staff>2</staff>
         <beam number="1">begin</beam>
@@ -6916,5 +6916,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>up</stem>
+        <stem>down</stem>
         <staff>2</staff>
         <beam number="1">continue</beam>
@@ -9198,5 +9198,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>up</stem>
+        <stem>down</stem>
         <staff>2</staff>
         <beam number="1">begin</beam>
@@ -10084,5 +10084,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>up</stem>
+        <stem>down</stem>
         <staff>2</staff>
         <beam number="1">begin</beam>
@@ -10105,5 +10105,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>up</stem>
+        <stem>down</stem>
         <staff>2</staff>
         <beam number="1">continue</beam>
@@ -10122,5 +10122,5 @@
           <normal-notes>2</normal-notes>
… (diff tronqué)

=== v3/voices: 6 differing lines ===
--- reference
+++ sortie
@@ -554,7 +554,4 @@
         <staff>2</staff>
       </note>
-      <barline location="right">
-        <bar-style>light-light</bar-style>
-      </barline>
     </measure>
     <measure number="6">
@@ -937,7 +934,4 @@
         </notations>
       </note>
-      <barline location="right">
-        <bar-style>light-light</bar-style>
-      </barline>
     </measure>
     <measure number="6">
```
