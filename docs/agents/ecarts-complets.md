# Écarts complets sur le corpus (parité complète)

Ce document est produit par le test `ecarts_complets_document` de `crates/mscz2xml/tests/e2e_corpus.rs`. Il compare chaque `.mscx` à sa référence `X.ref.musicxml` après retrait de la **mise en page pure** uniquement : attributs `default-x`, `default-y`, `relative-x`, `relative-y`, `width`, blocs `<defaults>`, `<print>` (et leurs `<system-layout>`/`<staff-layout>`) et `<encoding-date>`. Tout le reste est comparé : `<stem>`, `<beam>`, `<barline>`, `<direction>`, `<notations>`, `<credit>`, `<part-list>`, `<identification>`, etc.

## Tableau par fichier

### Corpus v3

| Fichier | Lignes d'écart |
| --- | ---: |
| Beam-CrossM1 | **identique** |
| barline01 | 6 |
| barlinedelete | **identique** |
| change | **identique** |
| clef-1 | **identique** |
| copypaste_tuplet_01 | **identique** |
| grace | **identique** |
| keysig_78216 | **identique** |
| layout_elements | 44 |
| lyricsline01 | **identique** |
| measure-4 | **identique** |
| moonlight | 70 |
| repeat01 | **identique** |
| repeat06 | **identique** |
| repeat07 | **identique** |
| repeat11 | **identique** |
| repeat20 | **identique** |
| repeat39 | **identique** |
| selectionfilter1 | **identique** |
| testBreaksPage | **identique** |
| testChordSymbols | **identique** |
| testHairpinDynamics | **identique** |
| testHarmony6 | **identique** |
| testMultiInstrumentPart3 | **identique** |
| testTboxWords1 | 10 |
| testTuplets8 | **identique** |
| testWedge4 | **identique** |
| timesig-05 | **identique** |
| undoResequenceMeasure | **identique** |
| voices | 6 |

### Corpus v4

| Fichier | Lignes d'écart |
| --- | ---: |
| beamStemDir | **identique** |
| changevisibility | **identique** |
| copypaste_parts | 2 |
| copypastesymbollist-lyrics | **identique** |
| dynamics | **identique** |
| dynamics_niente | **identique** |
| expression-1 | **identique** |
| hairpin_dynamic_inside | **identique** |
| hairpin_dynamic_jump | **identique** |
| hairpin_simple | **identique** |
| hairpins_and_repeats | **identique** |
| linked-ties | **identique** |
| mmrest | **identique** |
| part-textlines | **identique** |
| partExclusion | **identique** |
| repeat65 | **identique** |
| repeat66 | **identique** |
| repeat68 | **identique** |
| repeat69 | **identique** |
| tenuto_accent | **identique** |
| testExcludeInvisibleElements | 15 |
| testLocalTimesig1 | **identique** |
| testNoteheads2 | **identique** |
| testTempoPrecision | **identique** |
| timesig-09 | **identique** |
| voiceswitching | **identique** |

## Écarts groupés par balise

Fréquence = nombre de lignes canoniques modifiées sur les deux corpus ; fichiers = nombre de fichiers touchés. Tri par fréquence décroissante.

| Balise | Lignes | Fichiers | Exemples |
| --- | ---: | ---: | --- |
| `tied` | 18 | 2 | `<tied orientation="over" placement="above" type="start"/>` `<tied type="start"/>` `<tied orientation="under" placement="below" type="start"/>` |
| `duration` | 18 | 1 | `<duration>12</duration>` `<duration>24</duration>` `<duration>36</duration>` |
| `stem` | 18 | 1 | `<stem>down</stem>` `<stem>up</stem>` |
| `forward` | 16 | 1 | `</forward>` `<forward>` |
| `notations` | 14 | 2 | `<notations>` `</notations>` `<notations print-object="no">` |
| `barline` | 8 | 3 | `<barline location="right">` `</barline>` |
| `figure` | 8 | 1 | `<figure>` `</figure>` |
| `attributes` | 6 | 2 | `<attributes>` `</attributes>` |
| `direction` | 6 | 2 | `<direction placement="above" system="only-top">` `</direction>` `<direction placement="below">` |
| `direction-type` | 6 | 2 | `<direction-type>` `</direction-type>` |
| `bar-style` | 4 | 3 | `<bar-style>light-light</bar-style>` |
| `backup` | 4 | 1 | `</backup>` `<backup>` |
| `figure-number` | 4 | 1 | `<figure-number>5</figure-number>` `<figure-number>3</figure-number>` |
| `figured-bass` | 4 | 1 | `<figured-bass>` `</figured-bass>` |
| `bend` | 2 | 1 | `<bend>` `</bend>` |
| `measure-style` | 2 | 1 | `<measure-style number="1">` `</measure-style>` |
| `slide` | 2 | 1 | `<slide line-type="solid" number="1" type="start">gliss.</slide>` `<slide line-type="solid" number="1" type="stop"/>` |
| `staff-details` | 2 | 1 | `<staff-details print-object="no"/>` |
| `technical` | 2 | 1 | `<technical>` `</technical>` |
| `wavy-line` | 2 | 1 | `<wavy-line number="1" type="stop"/>` |
| `words` | 2 | 1 | `<words font-size="12" valign="top">Below 2</words>` |
| `bend-alter` | 1 | 1 | `<bend-alter>-1</bend-alter>` |
| `measure-repeat` | 1 | 1 | `<measure-repeat type="start">1</measure-repeat>` |
| `rehearsal` | 1 | 1 | `<rehearsal font-size="14" font-weight="bold" justify="center">A</rehearsal>` |
| `staff` | 1 | 1 | `<staff>1</staff>` |
| `with-bar` | 1 | 1 | `<with-bar>dip</with-bar>` |

## Diffs

```diff

=== v4/copypaste_parts : 2 lignes d'écart ===
--- reference
+++ sortie
@@ -285,5 +285,4 @@
             <trill-mark/>
             <wavy-line number="1" type="start"/>
-            <wavy-line number="1" type="stop"/>
           </ornaments>
         </notations>
@@ -305,4 +304,5 @@
           <ornaments>
             <tremolo type="start">3</tremolo>
+            <wavy-line number="1" type="stop"/>
           </ornaments>
         </notations>

=== v4/testExcludeInvisibleElements : 15 lignes d'écart ===
--- reference
+++ sortie
@@ -771,6 +771,8 @@
         <type>quarter</type>
         <stem>up</stem>
+        <notations>
+          <tied type="start"/>
+        </notations>
         <notations print-object="no">
-          <tied type="start"/>
           <technical>
             <fingering>4</fingering>
@@ -790,5 +792,5 @@
         <type>quarter</type>
         <stem>up</stem>
-        <notations print-object="no">
+        <notations>
           <tied type="stop"/>
         </notations>
@@ -816,8 +818,6 @@
         <type>quarter</type>
         <stem>up</stem>
-        <notations print-object="no">
-          <tied type="start"/>
-        </notations>
         <notations>
+          <tied type="start"/>
           <technical>
             <fingering>4</fingering>
@@ -837,5 +837,5 @@
         <type>quarter</type>
         <stem>up</stem>
-        <notations print-object="no">
+        <notations>
           <tied type="stop"/>
         </notations>
@@ -853,7 +853,4 @@
         <type>half</type>
       </note>
-      <barline location="right">
-        <bar-style>light-light</bar-style>
-      </barline>
     </measure>
     <measure number="16">

=== v3/barline01 : 6 lignes d'écart ===
--- reference
+++ sortie
@@ -225,7 +225,4 @@
     </measure>
     <measure number="5">
-      <attributes>
-        <staff-details print-object="no"/>
-      </attributes>
       <note>
         <rest measure="yes"/>
@@ -349,7 +346,4 @@
     </measure>
     <measure number="7">
-      <attributes>
-        <staff-details print-object="no"/>
-      </attributes>
       <note>
         <rest measure="yes"/>

=== v3/layout_elements : 44 lignes d'écart ===
--- reference
+++ sortie
@@ -650,7 +650,4 @@
         <staff>2</staff>
       </note>
-      <barline location="right">
-        <bar-style>light-light</bar-style>
-      </barline>
     </measure>
     <measure number="7">
@@ -745,12 +742,4 @@
         </notations>
       </note>
-      <figured-bass>
-        <figure>
-          <figure-number>5</figure-number>
-        </figure>
-        <figure>
-          <figure-number>3</figure-number>
-        </figure>
-      </figured-bass>
       <note>
         <pitch>
@@ -869,12 +858,4 @@
         <stem>down</stem>
         <staff>2</staff>
-        <notations>
-          <technical>
-            <bend>
-              <bend-alter>-1</bend-alter>
-              <with-bar>dip</with-bar>
-            </bend>
-          </technical>
-        </notations>
       </note>
       <direction placement="above" system="only-top">
@@ -887,15 +868,4 @@
     </measure>
     <measure number="8">
-      <attributes>
-        <measure-style number="1">
-          <measure-repeat type="start">1</measure-repeat>
-        </measure-style>
-      </attributes>
-      <direction placement="above" system="only-top">
-        <direction-type>
-          <rehearsal font-size="14" font-weight="bold" justify="center">A</rehearsal>
-        </direction-type>
-        <staff>1</staff>
-      </direction>
       <note>
         <pitch>
@@ -965,16 +935,5 @@
         <stem>up</stem>
         <staff>1</staff>
-        <notations>
-          <slide line-type="solid" number="1" type="start">gliss.</slide>
-        </notations>
       </note>
-      <figured-bass>
… (diff tronqué)

=== v3/moonlight : 70 lignes d'écart ===
--- reference
+++ sortie
@@ -1487,8 +1487,5 @@
       </note>
       <forward>
-        <duration>12</duration>
-      </forward>
-      <forward>
-        <duration>24</duration>
+        <duration>36</duration>
       </forward>
     </measure>
@@ -3465,5 +3462,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>down</stem>
+        <stem>up</stem>
         <staff>2</staff>
         <beam number="1">begin</beam>
@@ -3485,5 +3482,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>down</stem>
+        <stem>up</stem>
         <staff>2</staff>
         <beam number="1">continue</beam>
@@ -3501,5 +3498,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>down</stem>
+        <stem>up</stem>
         <staff>2</staff>
         <beam number="1">end</beam>
@@ -4542,5 +4539,5 @@
         <staff>2</staff>
         <notations>
-          <tied orientation="over" placement="above" type="start"/>
+          <tied type="start"/>
         </notations>
       </note>
@@ -5214,5 +5211,5 @@
         <staff>2</staff>
         <notations>
-          <tied orientation="over" placement="above" type="start"/>
+          <tied type="start"/>
         </notations>
       </note>
@@ -6896,5 +6893,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>up</stem>
+        <stem>down</stem>
         <staff>2</staff>
         <beam number="1">begin</beam>
@@ -6916,5 +6913,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>up</stem>
+        <stem>down</stem>
         <staff>2</staff>
… (diff tronqué)

=== v3/testTboxWords1 : 10 lignes d'écart ===
--- reference
+++ sortie
@@ -116,9 +116,4 @@
         </direction-type>
       </direction>
-      <direction placement="below">
-        <direction-type>
-          <words font-size="12" valign="top">Below 2</words>
-        </direction-type>
-      </direction>
       <note>
         <rest measure="yes"/>
@@ -128,4 +123,9 @@
     </measure>
     <measure number="3">
+      <direction placement="below">
+        <direction-type>
+          <words font-size="12" valign="top">Below 2</words>
+        </direction-type>
+      </direction>
       <note>
         <rest measure="yes"/>

=== v3/voices : 6 lignes d'écart ===
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
