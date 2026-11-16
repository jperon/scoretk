# Écarts complets sur le corpus (parité complète)

Ce document est produit par le test `ecarts_complets_document` de `crates/mscz2xml/tests/e2e_corpus.rs`. Il compare chaque `.mscx` à sa référence `X.ref.musicxml` après retrait de la **mise en page pure** uniquement : attributs `default-x`, `default-y`, `relative-x`, `relative-y`, `width`, blocs `<defaults>`, `<print>` (et leurs `<system-layout>`/`<staff-layout>`) et `<encoding-date>`. Tout le reste est comparé : `<stem>`, `<beam>`, `<barline>`, `<direction>`, `<notations>`, `<credit>`, `<part-list>`, `<identification>`, etc.

## Tableau par fichier

### Corpus v3

| Fichier | Lignes d'écart |
| --- | ---: |
| Beam-CrossM1 | **identique** |
| barline01 | 6 |
| barlinedelete | 2 |
| change | **identique** |
| clef-1 | **identique** |
| copypaste_tuplet_01 | **identique** |
| grace | **identique** |
| keysig_78216 | **identique** |
| layout_elements | 63 |
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
| testHairpinDynamics | 12 |
| testHarmony6 | **identique** |
| testMultiInstrumentPart3 | **identique** |
| testTboxWords1 | 10 |
| testTuplets8 | 6 |
| testWedge4 | **identique** |
| timesig-05 | **identique** |
| undoResequenceMeasure | 6 |
| voices | 8 |

### Corpus v4

| Fichier | Lignes d'écart |
| --- | ---: |
| beamStemDir | **identique** |
| changevisibility | **identique** |
| copypaste_parts | 12 |
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
| part-textlines | 20 |
| partExclusion | **identique** |
| repeat65 | **identique** |
| repeat66 | **identique** |
| repeat68 | **identique** |
| repeat69 | **identique** |
| tenuto_accent | **identique** |
| testExcludeInvisibleElements | 39 |
| testLocalTimesig1 | **identique** |
| testNoteheads2 | **identique** |
| testTempoPrecision | **identique** |
| timesig-09 | **identique** |
| voiceswitching | 12 |

## Écarts groupés par balise

Fréquence = nombre de lignes canoniques modifiées sur les deux corpus ; fichiers = nombre de fichiers touchés. Tri par fréquence décroissante.

| Balise | Lignes | Fichiers | Exemples |
| --- | ---: | ---: | --- |
| `direction` | 30 | 5 | `<direction placement="below">` `</direction>` `<direction placement="above" system="only-top">` |
| `direction-type` | 30 | 5 | `<direction-type>` `</direction-type>` |
| `tuplet` | 18 | 2 | `<tuplet bracket="yes" number="1" placement="above" type="start"/>` `<tuplet bracket="yes" number="1" type="start"/>` `<tuplet bracket="yes" number="1" placement="above" type="start">` |
| `duration` | 18 | 1 | `<duration>12</duration>` `<duration>24</duration>` `<duration>36</duration>` |
| `stem` | 18 | 1 | `<stem>down</stem>` `<stem>up</stem>` |
| `words` | 16 | 3 | `<words font-family="FreeSerif" font-size="12" font-style="italic">begin</words>` `<words>begin</words>` `<words font-family="FreeSerif" font-size="12" font-style="italic">end</words>` |
| `notations` | 16 | 2 | `<notations>` `</notations>` `<notations print-object="no">` |
| `tied` | 16 | 2 | `<tied orientation="over" placement="above" type="start"/>` `<tied type="start"/>` `<tied orientation="under" placement="below" type="start"/>` |
| `forward` | 16 | 1 | `</forward>` `<forward>` |
| `rehearsal` | 13 | 3 | `<rehearsal font-size="14" font-weight="bold" justify="center">1</rehearsal>` `<rehearsal font-size="14" font-weight="bold">1</rehearsal>` `<rehearsal font-size="14" font-weight="bold" justify="center">A</rehearsal>` |
| `barline` | 8 | 3 | `<barline location="right">` `</barline>` |
| `figure` | 8 | 1 | `<figure>` `</figure>` |
| `pedal` | 6 | 3 | `<pedal line="yes" sign="yes" type="start"/>` `<pedal line="yes" sign="no" type="stop"/>` |
| `attributes` | 6 | 2 | `<attributes>` `</attributes>` |
| `wedge` | 6 | 2 | `<wedge number="1" spread="11.5" type="crescendo"/>` `<wedge number="1" type="diminuendo"/>` `<wedge number="1" type="stop"/>` |
| `staff` | 5 | 1 | `<staff>2</staff>` `<staff>1</staff>` |
| `bar-style` | 4 | 3 | `<bar-style>light-light</bar-style>` |
| `backup` | 4 | 1 | `</backup>` `<backup>` |
| `figure-number` | 4 | 1 | `<figure-number>5</figure-number>` `<figure-number>3</figure-number>` |
| `figured-bass` | 4 | 1 | `<figured-bass>` `</figured-bass>` |
| `bend` | 2 | 1 | `<bend>` `</bend>` |
| `key` | 2 | 1 | `<key print-object="no">` `<key>` |
| `measure-style` | 2 | 1 | `<measure-style number="1">` `</measure-style>` |
| `slide` | 2 | 1 | `<slide line-type="solid" number="1" type="start">gliss.</slide>` `<slide line-type="solid" number="1" type="stop"/>` |
| `staff-details` | 2 | 1 | `<staff-details print-object="no"/>` |
| `technical` | 2 | 1 | `<technical>` `</technical>` |
| `time` | 2 | 1 | `<time print-object="no">` `<time>` |
| `wavy-line` | 2 | 1 | `<wavy-line number="1" type="stop"/>` |
| `bend-alter` | 1 | 1 | `<bend-alter>-1</bend-alter>` |
| `measure-repeat` | 1 | 1 | `<measure-repeat type="start">1</measure-repeat>` |
| `notehead` | 1 | 1 | `<notehead parentheses="yes">normal</notehead>` |
| `with-bar` | 1 | 1 | `<with-bar>dip</with-bar>` |

## Diffs

```diff

=== v4/copypaste_parts : 12 lignes d'écart ===
--- reference
+++ sortie
@@ -61,9 +61,4 @@
         <sound tempo="80"/>
       </direction>
-      <direction placement="below">
-        <direction-type>
-          <pedal line="yes" sign="yes" type="start"/>
-        </direction-type>
-      </direction>
       <direction placement="above">
         <direction-type>
@@ -147,9 +142,4 @@
         </notations>
       </note>
-      <direction placement="below">
-        <direction-type>
-          <pedal line="yes" sign="no" type="stop"/>
-        </direction-type>
-      </direction>
       <direction placement="above">
         <direction-type>
@@ -285,5 +275,4 @@
             <trill-mark/>
             <wavy-line number="1" type="start"/>
-            <wavy-line number="1" type="stop"/>
           </ornaments>
         </notations>
@@ -305,4 +294,5 @@
           <ornaments>
             <tremolo type="start">3</tremolo>
+            <wavy-line number="1" type="stop"/>
           </ornaments>
         </notations>

=== v4/part-textlines : 20 lignes d'écart ===
--- reference
+++ sortie
@@ -80,9 +80,4 @@
         </clef>
       </attributes>
-      <direction placement="below">
-        <direction-type>
-          <wedge number="1" spread="11.5" type="crescendo"/>
-        </direction-type>
-      </direction>
       <direction placement="above">
         <direction-type>
@@ -93,4 +88,9 @@
         </direction-type>
       </direction>
+      <direction placement="below">
+        <direction-type>
+          <wedge number="1" spread="11.5" type="crescendo"/>
+        </direction-type>
+      </direction>
       <note>
         <pitch>
@@ -265,9 +265,4 @@
     </measure>
     <measure number="3">
-      <direction placement="below">
-        <direction-type>
-          <wedge number="1" type="diminuendo"/>
-        </direction-type>
-      </direction>
       <direction placement="above">
         <direction-type>
@@ -278,4 +273,9 @@
         </direction-type>
       </direction>
+      <direction placement="below">
+        <direction-type>
+          <wedge number="1" type="diminuendo"/>
+        </direction-type>
+      </direction>
       <note>
         <pitch>

=== v4/testExcludeInvisibleElements : 39 lignes d'écart ===
--- reference
+++ sortie
@@ -332,4 +332,9 @@
     </measure>
     <measure number="6">
+      <direction placement="below">
+        <direction-type>
+          <wedge number="1" spread="11.5" type="crescendo"/>
+        </direction-type>
+      </direction>
       <note>
         <pitch>
@@ -362,4 +367,9 @@
         <stem>up</stem>
       </note>
+      <direction placement="below">
+        <direction-type>
+          <wedge number="1" type="stop"/>
+        </direction-type>
+      </direction>
       <note>
         <pitch>
@@ -374,9 +384,4 @@
     </measure>
     <measure number="7">
-      <direction placement="below">
-        <direction-type>
-          <pedal line="yes" sign="yes" type="start"/>
-        </direction-type>
-      </direction>
       <note>
         <pitch>
@@ -425,9 +430,4 @@
         </notations>
       </note>
-      <direction placement="below">
-        <direction-type>
-          <pedal line="yes" sign="no" type="stop"/>
-        </direction-type>
-      </direction>
     </measure>
     <measure number="8">
@@ -771,6 +771,8 @@
         <type>quarter</type>
         <stem>up</stem>
-        <notations print-object="no">
+        <notations>
           <tied type="start"/>
+        </notations>
+        <notations print-object="no">
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
… (diff tronqué)

=== v4/voiceswitching : 12 lignes d'écart ===
--- reference
+++ sortie
@@ -167,5 +167,5 @@
         <stem>up</stem>
         <notations>
-          <tuplet bracket="yes" placement="above" type="start"/>
+          <tuplet bracket="yes" type="start"/>
         </notations>
       </note>
@@ -263,5 +263,5 @@
         <notehead>x</notehead>
         <notations>
-          <tuplet bracket="yes" number="1" placement="above" type="start"/>
+          <tuplet bracket="yes" number="1" type="start"/>
         </notations>
       </note>
@@ -281,5 +281,5 @@
         <beam number="1">begin</beam>
         <notations>
-          <tuplet bracket="no" number="2" placement="above" type="start">
+          <tuplet bracket="no" number="2" type="start">
             <tuplet-actual>
               <tuplet-number>3</tuplet-number>
@@ -442,5 +442,5 @@
         <notehead>x</notehead>
         <notations>
-          <tuplet bracket="yes" placement="above" type="start"/>
+          <tuplet bracket="yes" type="start"/>
         </notations>
       </note>
@@ -517,5 +517,5 @@
         <notehead>x</notehead>
         <notations>
-          <tuplet bracket="yes" number="1" placement="above" type="start"/>
+          <tuplet bracket="yes" number="1" type="start"/>
         </notations>
       </note>
@@ -534,5 +534,5 @@
         <stem>up</stem>
         <notations>
-          <tuplet bracket="yes" number="2" placement="above" type="start">
+          <tuplet bracket="yes" number="2" type="start">
             <tuplet-actual>
               <tuplet-number>3</tuplet-number>

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

=== v3/barlinedelete : 2 lignes d'écart ===
--- reference
+++ sortie
@@ -149,5 +149,5 @@
       <direction placement="above" system="only-top">
         <direction-type>
-          <rehearsal font-size="14" font-weight="bold" justify="center">1</rehearsal>
+          <rehearsal font-size="14" font-weight="bold">1</rehearsal>
         </direction-type>
       </direction>

=== v3/layout_elements : 63 lignes d'écart ===
--- reference
+++ sortie
@@ -168,10 +168,4 @@
         <duration>24</duration>
       </backup>
-      <direction placement="below">
-        <direction-type>
-          <pedal line="yes" sign="yes" type="start"/>
-        </direction-type>
-        <staff>2</staff>
-      </direction>
       <note>
         <grace/>
@@ -213,10 +207,4 @@
         <staff>2</staff>
       </note>
-      <direction placement="below">
-        <direction-type>
-          <pedal line="yes" sign="no" type="stop"/>
-        </direction-type>
-        <staff>2</staff>
-      </direction>
     </measure>
     <measure number="2">
@@ -581,5 +569,4 @@
         <type>quarter</type>
         <stem>down</stem>
-        <notehead parentheses="yes">normal</notehead>
         <staff>1</staff>
       </note>
@@ -650,7 +637,4 @@
         <staff>2</staff>
       </note>
-      <barline location="right">
-        <bar-style>light-light</bar-style>
-      </barline>
     </measure>
     <measure number="7">
@@ -670,5 +654,5 @@
       <direction placement="above" system="only-top">
         <direction-type>
-          <rehearsal font-size="14" font-weight="bold" justify="center">A</rehearsal>
+          <rehearsal font-size="14" font-weight="bold">A</rehearsal>
         </direction-type>
         <staff>1</staff>
@@ -745,12 +729,4 @@
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
@@ -789,5 +765,4 @@
           <octave-shift number="1" size="8" type="down"/>
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

=== v3/testHairpinDynamics : 12 lignes d'écart ===
--- reference
+++ sortie
@@ -195,5 +195,5 @@
       <direction placement="below">
         <direction-type>
-          <words font-family="FreeSerif" font-size="12" font-style="italic">begin</words>
+          <words>begin</words>
         </direction-type>
         <direction-type>
@@ -246,5 +246,5 @@
         </direction-type>
         <direction-type>
-          <words font-family="FreeSerif" font-size="12" font-style="italic">end</words>
+          <words>end</words>
         </direction-type>
       </direction>
@@ -253,5 +253,5 @@
       <direction placement="below">
         <direction-type>
-          <words font-family="FreeSans" font-size="10" font-style="italic">from</words>
+          <words>from</words>
         </direction-type>
         <direction-type>
@@ -309,5 +309,5 @@
         </direction-type>
         <direction-type>
-          <words font-family="FreeSerif" font-size="12" font-style="italic">to</words>
+          <words>to</words>
         </direction-type>
         <direction-type>
@@ -321,5 +321,5 @@
       <direction placement="below">
         <direction-type>
-          <words font-family="FreeSerif" font-size="12" font-style="italic">cresc. from</words>
+          <words>cresc. from</words>
         </direction-type>
         <direction-type>
@@ -329,5 +329,5 @@
         </direction-type>
         <direction-type>
-          <words font-family="FreeSerif" font-size="12" font-style="italic">to</words>
+          <words>to</words>
         </direction-type>
         <direction-type>

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

=== v3/testTuplets8 : 6 lignes d'écart ===
--- reference
+++ sortie
@@ -78,5 +78,5 @@
         <stem>down</stem>
         <notations>
-          <tuplet bracket="yes" number="1" placement="above" type="start"/>
+          <tuplet bracket="yes" number="1" type="start"/>
         </notations>
       </note>
@@ -207,5 +207,5 @@
         <stem>down</stem>
         <notations>
-          <tuplet bracket="yes" number="1" placement="above" type="start"/>
+          <tuplet bracket="yes" number="1" type="start"/>
         </notations>
       </note>
@@ -340,5 +340,5 @@
         <beam number="2">begin</beam>
         <notations>
-          <tuplet bracket="yes" number="1" placement="above" type="start">
+          <tuplet bracket="yes" number="1" type="start">
             <tuplet-actual>
               <tuplet-number>3</tuplet-number>

=== v3/undoResequenceMeasure : 6 lignes d'écart ===
--- reference
+++ sortie
@@ -93,5 +93,5 @@
       <direction placement="above" system="only-top">
         <direction-type>
-          <rehearsal font-size="14" font-weight="bold" justify="center">5</rehearsal>
+          <rehearsal font-size="14" font-weight="bold">5</rehearsal>
         </direction-type>
       </direction>
@@ -142,5 +142,5 @@
       <direction placement="above" system="only-top">
         <direction-type>
-          <rehearsal font-size="14" font-weight="bold" justify="center">QQ</rehearsal>
+          <rehearsal font-size="14" font-weight="bold">QQ</rehearsal>
         </direction-type>
       </direction>
@@ -191,5 +191,5 @@
       <direction placement="above" system="only-top">
         <direction-type>
-          <rehearsal font-size="14" font-weight="bold" justify="center">Foo</rehearsal>
+          <rehearsal font-size="14" font-weight="bold">Foo</rehearsal>
         </direction-type>
       </direction>

=== v3/voices : 8 lignes d'écart ===
--- reference
+++ sortie
@@ -196,5 +196,5 @@
       <direction placement="below">
         <direction-type>
-          <words font-family="FreeSerif" font-size="12" font-style="italic">cresc.</words>
+          <words>cresc.</words>
         </direction-type>
         <direction-type>
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
