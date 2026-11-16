# Écarts complets sur le corpus (parité complète)

Ce document est produit par le test `ecarts_complets_document` de `crates/mscz2xml/tests/e2e_corpus.rs`. Il compare chaque `.mscx` à sa référence `X.ref.musicxml` après retrait de la **mise en page pure** uniquement : attributs `default-x`, `default-y`, `relative-x`, `relative-y`, `width`, blocs `<defaults>`, `<print>` (et leurs `<system-layout>`/`<staff-layout>`) et `<encoding-date>`. Tout le reste est comparé : `<stem>`, `<beam>`, `<barline>`, `<direction>`, `<notations>`, `<credit>`, `<part-list>`, `<identification>`, etc.

## Tableau par fichier

### Corpus v3

| Fichier | Lignes d'écart |
| --- | ---: |
| Beam-CrossM1 | 23 |
| barline01 | 26 |
| barlinedelete | 19 |
| change | 72 |
| clef-1 | 18 |
| copypaste_tuplet_01 | 14 |
| grace | 6 |
| keysig_78216 | 14 |
| layout_elements | 188 |
| lyricsline01 | 8 |
| measure-4 | 19 |
| moonlight | 2392 |
| repeat01 | 4 |
| repeat06 | 4 |
| repeat07 | 4 |
| repeat11 | 4 |
| repeat20 | 4 |
| repeat39 | 4 |
| selectionfilter1 | 11 |
| testBreaksPage | 4 |
| testChordSymbols | 13 |
| testHairpinDynamics | 44 |
| testHarmony6 | 42 |
| testMultiInstrumentPart3 | 50 |
| testTboxWords1 | 14 |
| testTuplets8 | 61 |
| testWedge4 | 20 |
| timesig-05 | 53 |
| undoResequenceMeasure | 11 |
| voices | 80 |

### Corpus v4

| Fichier | Lignes d'écart |
| --- | ---: |
| beamStemDir | 16 |
| changevisibility | 32 |
| copypaste_parts | 36 |
| copypastesymbollist-lyrics | 48 |
| dynamics | 29 |
| dynamics_niente | 13 |
| expression-1 | 5 |
| hairpin_dynamic_inside | 13 |
| hairpin_dynamic_jump | 16 |
| hairpin_simple | 10 |
| hairpins_and_repeats | 23 |
| linked-ties | 11 |
| mmrest | 4 |
| part-textlines | 26 |
| partExclusion | 17 |
| repeat65 | 4 |
| repeat66 | 17 |
| repeat68 | 5 |
| repeat69 | 5 |
| tenuto_accent | 5 |
| testExcludeInvisibleElements | 111 |
| testLocalTimesig1 | 5 |
| testNoteheads2 | 29 |
| testTempoPrecision | 4 |
| timesig-09 | 4 |
| voiceswitching | 82 |

## Écarts groupés par balise

Fréquence = nombre de lignes canoniques modifiées sur les deux corpus ; fichiers = nombre de fichiers touchés. Tri par fréquence décroissante.

| Balise | Lignes | Fichiers | Exemples |
| --- | ---: | ---: | --- |
| `stem` | 1479 | 32 | `<stem>up</stem>` `<stem>down</stem>` |
| `beam` | 948 | 9 | `<beam number="1">begin</beam>` `<beam number="1">continue</beam>` `<beam number="1">end</beam>` |
| `slur` | 260 | 7 | `<slur bezier-x="36.071709" bezier-y="-21.363336" number="1" type="start"/>` `<slur number="1" type="start"/>` `<slur bezier-x="-36.071709" bezier-y="-21.363336" number="1" type="stop"/>` |
| `staff` | 213 | 2 | `<staff>2</staff>` `<staff>1</staff>` |
| `miscellaneous-field` | 122 | 56 | `<miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>` `<miscellaneous-field name="platform">Linux</miscellaneous-field>` `<miscellaneous-field name="creationDate">2020-02-24</miscellaneous-field>` |
| `miscellaneous` | 112 | 56 | `<miscellaneous>` `</miscellaneous>` |
| `wedge` | 106 | 17 | `<wedge number="1" spread="12" type="crescendo"/>` `<wedge number="1" type="crescendo"/>` `<wedge number="1" spread="12" type="stop"/>` |
| `instrument` | 77 | 3 | `<instrument id="P1-I1"/>` `<instrument id="P1-I2"/>` |
| `accidental` | 34 | 1 | `<accidental cautionary="yes" parentheses="no">sharp</accidental>` `<accidental>sharp</accidental>` `<accidental cautionary="yes" parentheses="no">natural</accidental>` |
| `direction` | 30 | 5 | `<direction placement="below">` `</direction>` `<direction placement="above" system="only-top">` |
| `direction-type` | 30 | 5 | `<direction-type>` `</direction-type>` |
| `kind` | 28 | 5 | `<kind>major</kind>` `<kind text="C"></kind>` `<kind parentheses-degrees="yes" text="m">minor</kind>` |
| `note` | 20 | 3 | `<note print-object="no">` `<note>` |
| `duration` | 20 | 1 | `<duration>12</duration>` `<duration>36</duration>` `<duration>24</duration>` |
| `instrument-sound` | 19 | 14 | `<instrument-sound>voice.vocals</instrument-sound>` `<instrument-sound>wind.flutes.flute</instrument-sound>` `<instrument-sound>keyboard.piano</instrument-sound>` |
| `tuplet` | 18 | 2 | `<tuplet bracket="yes" number="1" placement="above" type="start"/>` `<tuplet bracket="yes" number="1" type="start"/>` `<tuplet bracket="yes" number="1" placement="above" type="start">` |
| `words` | 16 | 3 | `<words font-family="FreeSerif" font-size="12" font-style="italic">begin</words>` `<words>begin</words>` `<words font-family="FreeSerif" font-size="12" font-style="italic">end</words>` |
| `measure` | 16 | 2 | `<measure number="1">` `<measure number="2">` `<measure number="3">` |
| `notations` | 16 | 2 | `<notations>` `</notations>` `<notations print-object="no">` |
| `tied` | 16 | 2 | `<tied orientation="over" placement="above" type="start"/>` `<tied type="start"/>` `<tied orientation="under" placement="below" type="start"/>` |
| `forward` | 16 | 1 | `<forward>` `</forward>` |
| `rehearsal` | 13 | 3 | `<rehearsal font-size="14" font-weight="bold" justify="center">1</rehearsal>` `<rehearsal font-size="14" font-weight="bold">1</rehearsal>` `<rehearsal font-size="14" font-weight="bold" justify="center">A</rehearsal>` |
| `attributes` | 12 | 5 | `<attributes>` `</attributes>` |
| `root` | 12 | 3 | `<root>` `</root>` |
| `barline` | 10 | 4 | `<barline location="right">` `</barline>` |
| `midi-channel` | 9 | 4 | `<midi-channel>2</midi-channel>` `<midi-channel>3</midi-channel>` `<midi-channel>1</midi-channel>` |
| `line` | 8 | 4 | `<line>2</line>` `<line>4</line>` |
| `sign` | 8 | 4 | `<sign>G</sign>` `<sign>F</sign>` |
| `clef` | 8 | 3 | `<clef after-barline="yes">` `</clef>` `<clef after-barline="yes" number="2">` |
| `figure` | 8 | 1 | `<figure>` `</figure>` |
| `midi-instrument` | 6 | 3 | `<midi-instrument id="P1-I2">` `</midi-instrument>` |
| `pedal` | 6 | 3 | `<pedal line="yes" sign="yes" type="start"/>` `<pedal line="yes" sign="no" type="stop"/>` |
| `root-step` | 6 | 3 | `<root-step>D</root-step>` `<root-step text="">C</root-step>` `<root-step>G</root-step>` |
| `score-instrument` | 6 | 3 | `</score-instrument>` `<score-instrument id="P1-I2">` |
| `midi-program` | 4 | 4 | `<midi-program>53</midi-program>` `<midi-program>74</midi-program>` `<midi-program>1</midi-program>` |
| `bar-style` | 4 | 3 | `<bar-style>light-light</bar-style>` |
| `divisions` | 4 | 2 | `<divisions>1</divisions>` |
| `harmony` | 4 | 2 | `</harmony>` `<harmony print-frame="no">` |
| `key` | 4 | 2 | `<key number="1">` `<key>` `<key print-object="no">` |
| `figure-number` | 4 | 1 | `<figure-number>5</figure-number>` `<figure-number>3</figure-number>` |
| `figured-bass` | 4 | 1 | `<figured-bass>` `</figured-bass>` |
| `numeral` | 4 | 1 | `<numeral>` `</numeral>` |
| `instrument-name` | 3 | 3 | `<instrument-name>Flute</instrument-name>` `<instrument-name>Piano</instrument-name>` `<instrument-name>B♭ Clarinet</instrument-name>` |
| `instruments` | 3 | 3 | `<instruments>2</instruments>` |
| `midi-device` | 3 | 3 | `<midi-device id="P1-I2" port="1"></midi-device>` |
| `pan` | 3 | 3 | `<pan>0</pan>` |
| `volume` | 3 | 3 | `<volume>78.7402</volume>` |
| `part-group` | 3 | 1 | `<part-group number="1" type="start">` `</part-group>` `<part-group number="1" type="stop"/>` |
| `offset` | 2 | 2 | `<offset>9</offset>` `<offset>2</offset>` |
| `backup` | 2 | 1 | `<backup>` |
| `bass` | 2 | 1 | `<bass arrangement="horizontal">` `<bass>` |
| `bend` | 2 | 1 | `<bend>` `</bend>` |
| `degree` | 2 | 1 | `<degree>` `</degree>` |
| `function` | 2 | 1 | `<function>xyz</function>` `<function></function>` |
| `measure-style` | 2 | 1 | `<measure-style number="1">` `</measure-style>` |
| `numeral-root` | 2 | 1 | `<numeral-root text="i">1</numeral-root>` `<numeral-root>1</numeral-root>` |
| `octave` | 2 | 1 | `<octave>4</octave>` `<octave>5</octave>` |
| `slide` | 2 | 1 | `<slide line-type="solid" number="1" type="start">gliss.</slide>` `<slide line-type="solid" number="1" type="stop"/>` |
| `staff-details` | 2 | 1 | `<staff-details print-object="no"/>` |
| `technical` | 2 | 1 | `<technical>` `</technical>` |
| `time` | 2 | 1 | `<time print-object="no">` `<time>` |
| `wavy-line` | 2 | 1 | `<wavy-line number="1" type="stop"/>` |
| `bend-alter` | 1 | 1 | `<bend-alter>-1</bend-alter>` |
| `degree-alter` | 1 | 1 | `<degree-alter>0</degree-alter>` |
| `degree-type` | 1 | 1 | `<degree-type>add</degree-type>` |
| `degree-value` | 1 | 1 | `<degree-value>9</degree-value>` |
| `fermata` | 1 | 1 | `<fermata type="upright"/>` |
| `group-barline` | 1 | 1 | `<group-barline>yes</group-barline>` |
| `group-symbol` | 1 | 1 | `<group-symbol>bracket</group-symbol>` |
| `measure-repeat` | 1 | 1 | `<measure-repeat type="start">1</measure-repeat>` |
| `notehead` | 1 | 1 | `<notehead parentheses="yes">normal</notehead>` |
| `with-bar` | 1 | 1 | `<with-bar>dip</with-bar>` |

## Diffs

```diff

=== v4/beamStemDir : 16 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -62,5 +58,4 @@
         <stem>down</stem>
         <staff>1</staff>
-        <beam number="1">begin</beam>
       </note>
       <note>
@@ -74,5 +69,4 @@
         <stem>down</stem>
         <staff>1</staff>
-        <beam number="1">continue</beam>
       </note>
       <note>
@@ -86,5 +80,4 @@
         <stem>down</stem>
         <staff>1</staff>
-        <beam number="1">continue</beam>
       </note>
       <note>
@@ -98,5 +91,4 @@
         <stem>down</stem>
         <staff>1</staff>
-        <beam number="1">end</beam>
       </note>
       <note>
@@ -126,7 +118,5 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>down</stem>
         <staff>1</staff>
-        <beam number="1">begin</beam>
       </note>
       <note>
@@ -138,7 +128,5 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>down</stem>
         <staff>1</staff>
-        <beam number="1">continue</beam>
       </note>
       <note>
@@ -150,7 +138,5 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>down</stem>
         <staff>1</staff>
-        <beam number="1">continue</beam>
       </note>
       <note>
@@ -162,7 +148,5 @@
… (diff tronqué)

=== v4/changevisibility : 32 lignes d'écart ===
--- reference
+++ sortie
@@ -13,9 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2023-01-23</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-      <miscellaneous-field name="subtitle">Subtitle</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -63,5 +58,4 @@
         <dot/>
         <accidental>sharp</accidental>
-        <stem>up</stem>
         <notations>
           <articulations>
@@ -92,6 +86,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>up</stem>
-        <beam number="1">begin</beam>
       </note>
       <note>
@@ -104,6 +96,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>up</stem>
-        <beam number="1">continue</beam>
       </note>
       <note>
@@ -116,6 +106,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>up</stem>
-        <beam number="1">end</beam>
       </note>
       <note>
@@ -128,7 +116,6 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>up</stem>
         <notations>
-          <slur bezier-x="8.68934" bezier-y="-9.360074" number="1" type="start"/>
+          <slur number="1" type="start"/>
         </notations>
         <lyric number="1">
@@ -143,5 +130,5 @@
         <type>eighth</type>
         <notations>
-          <slur bezier-x="-8.68934" bezier-y="-9.360074" number="1" type="stop"/>
+          <slur number="1" type="stop"/>
         </notations>
       </note>
@@ -155,5 +142,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>up</stem>
       </note>
… (diff tronqué)

=== v4/copypaste_parts : 36 lignes d'écart ===
--- reference
+++ sortie
@@ -13,9 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2024-09-06</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-      <miscellaneous-field name="subtitle">Subtitle</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -61,9 +56,4 @@
         <sound tempo="80"/>
       </direction>
-      <direction placement="below">
-        <direction-type>
-          <pedal line="yes" sign="yes" type="start"/>
-        </direction-type>
-      </direction>
       <direction placement="above">
         <direction-type>
@@ -79,5 +69,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <notations>
           <articulations>
@@ -97,7 +86,6 @@
         <type>quarter</type>
         <accidental>sharp</accidental>
-        <stem>down</stem>
         <notations>
-          <slur bezier-x="10.387507" bezier-y="10.233908" number="1" type="start"/>
+          <slur number="1" type="start"/>
           <articulations>
             <staccato/>
@@ -113,7 +101,6 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <notations>
-          <slur bezier-x="-10.387507" bezier-y="10.233908" number="1" type="stop"/>
+          <slur number="1" type="stop"/>
           <articulations>
             <tenuto/>
@@ -129,5 +116,4 @@
         <voice>1</voice>
         <type>32nd</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -139,5 +125,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <notations>
           <fermata type="upright"/>
@@ -147,9 +132,4 @@
         </notations>
… (diff tronqué)

=== v4/copypastesymbollist-lyrics : 48 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2025-02-12</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -54,5 +50,4 @@
         <voice>1</voice>
         <type>half</type>
-        <stem>down</stem>
         <lyric number="1">
           <syllabic>single</syllabic>
@@ -68,5 +63,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <lyric number="1">
           <syllabic>begin</syllabic>
@@ -82,5 +76,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <lyric number="1">
           <syllabic>end</syllabic>
@@ -98,5 +91,4 @@
         <voice>1</voice>
         <type>half</type>
-        <stem>down</stem>
         <lyric number="1">
           <syllabic>single</syllabic>
@@ -112,5 +104,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <lyric number="1">
           <syllabic>begin</syllabic>
@@ -126,5 +117,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
     </measure>
@@ -138,6 +128,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>down</stem>
-        <beam number="1">begin</beam>
         <lyric number="1">
           <syllabic>end</syllabic>
@@ -154,6 +142,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>down</stem>
-        <beam number="1">continue</beam>
       </note>
… (diff tronqué)

=== v4/dynamics : 29 lignes d'écart ===
--- reference
+++ sortie
@@ -13,9 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2023-10-29</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-      <miscellaneous-field name="subtitle">Subtitle</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -80,5 +75,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -90,5 +84,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -100,5 +93,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -110,5 +102,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
     </measure>
@@ -116,5 +107,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="11.5" type="crescendo"/>
+          <wedge number="1" type="crescendo"/>
         </direction-type>
       </direction>
@@ -127,5 +118,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -137,5 +127,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -147,5 +136,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
… (diff tronqué)

=== v4/dynamics_niente : 13 lignes d'écart ===
--- reference
+++ sortie
@@ -13,9 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2024-09-08</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-      <miscellaneous-field name="subtitle">Ondertitel</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -92,5 +87,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="11.5" type="stop"/>
+          <wedge number="1" type="stop"/>
         </direction-type>
       </direction>
@@ -148,5 +143,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge niente="yes" number="1" spread="11.5" type="stop"/>
+          <wedge niente="yes" number="1" type="stop"/>
         </direction-type>
       </direction>
@@ -178,5 +173,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="11.5" type="crescendo"/>
+          <wedge number="1" type="crescendo"/>
         </direction-type>
       </direction>
@@ -226,5 +221,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge niente="yes" number="1" spread="11.5" type="crescendo"/>
+          <wedge niente="yes" number="1" type="crescendo"/>
         </direction-type>
       </direction>

=== v4/expression-1 : 5 lignes d'écart ===
--- reference
+++ sortie
@@ -13,9 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2023-04-07</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-      <miscellaneous-field name="subtitle">Subtitle</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>

=== v4/hairpin_dynamic_inside : 13 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2023-11-22</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -71,5 +67,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="11.5" type="crescendo"/>
+          <wedge number="1" type="crescendo"/>
         </direction-type>
       </direction>
@@ -82,5 +78,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -92,5 +87,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -102,5 +96,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <direction placement="below">
@@ -120,5 +113,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
     </measure>
@@ -132,5 +124,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <direction placement="below">
@@ -150,5 +141,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -160,5 +150,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <direction placement="below">

=== v4/hairpin_dynamic_jump : 16 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2023-11-22</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -71,5 +67,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="11.5" type="crescendo"/>
+          <wedge number="1" type="crescendo"/>
         </direction-type>
       </direction>
@@ -82,5 +78,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -92,5 +87,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -102,5 +96,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -112,5 +105,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <direction placement="below">
@@ -164,5 +156,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -174,5 +165,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -184,5 +174,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
… (diff tronqué)

=== v4/hairpin_simple : 10 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2023-11-21</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -71,5 +67,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="11.5" type="crescendo"/>
+          <wedge number="1" type="crescendo"/>
         </direction-type>
       </direction>
@@ -82,5 +78,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -92,5 +87,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -102,5 +96,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <direction placement="below">
@@ -125,5 +118,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <barline location="right">

=== v4/hairpins_and_repeats : 23 lignes d'écart ===
--- reference
+++ sortie
@@ -13,9 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2020-04-24</miscellaneous-field>
-      <miscellaneous-field name="mscVersion">4.00</miscellaneous-field>
-      <miscellaneous-field name="platform">Microsoft Windows</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -62,5 +57,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="11.5" type="crescendo"/>
+          <wedge number="1" type="crescendo"/>
         </direction-type>
       </direction>
@@ -73,5 +68,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -83,5 +77,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -93,5 +86,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -103,5 +95,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <direction placement="below">
@@ -126,5 +117,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="11.5" type="crescendo"/>
+          <wedge number="1" type="crescendo"/>
         </direction-type>
       </direction>
@@ -137,5 +128,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -147,5 +137,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
… (diff tronqué)

=== v4/linked-ties : 11 lignes d'écart ===
--- reference
+++ sortie
@@ -13,9 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2024-08-05</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-      <miscellaneous-field name="subtitle">Subtitle</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -61,5 +56,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <notations>
           <tied type="start"/>
@@ -75,5 +69,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <notations>
           <tied type="stop"/>
@@ -97,5 +90,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>up</stem>
         <notations>
           <tied type="start"/>
@@ -111,5 +103,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <notations>
           <tied type="stop"/>
@@ -131,5 +122,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <notations>
           <tied type="start"/>
@@ -145,5 +135,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>up</stem>
         <notations>
           <tied type="stop"/>

=== v4/mmrest : 4 lignes d'écart ===
--- reference
+++ sortie
@@ -13,8 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">

=== v4/part-textlines : 26 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -80,9 +76,4 @@
         </clef>
       </attributes>
-      <direction placement="below">
-        <direction-type>
-          <wedge number="1" spread="11.5" type="crescendo"/>
-        </direction-type>
-      </direction>
       <direction placement="above">
         <direction-type>
@@ -93,4 +84,9 @@
         </direction-type>
       </direction>
+      <direction placement="below">
+        <direction-type>
+          <wedge number="1" type="crescendo"/>
+        </direction-type>
+      </direction>
       <note>
         <pitch>
@@ -265,9 +261,4 @@
     </measure>
     <measure number="3">
-      <direction placement="below">
-        <direction-type>
-          <wedge number="1" type="diminuendo"/>
-        </direction-type>
-      </direction>
       <direction placement="above">
         <direction-type>
@@ -278,4 +269,9 @@
         </direction-type>
       </direction>
+      <direction placement="below">
+        <direction-type>
+          <wedge number="1" type="diminuendo"/>
+        </direction-type>
+      </direction>
       <note>
         <pitch>
@@ -300,5 +296,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="11.5" type="stop"/>
+          <wedge number="1" type="stop"/>
         </direction-type>
       </direction>

=== v4/partExclusion : 17 lignes d'écart ===
--- reference
+++ sortie
@@ -13,9 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2023-08-28</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-      <miscellaneous-field name="subtitle">Subtitle</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -65,5 +60,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -75,5 +69,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <direction placement="above" system="only-top">
@@ -90,5 +83,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -100,5 +92,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
     </measure>
@@ -112,5 +103,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -122,5 +112,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <attributes>
@@ -138,5 +127,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -148,5 +136,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
     </measure>
… (diff tronqué)

=== v4/repeat65 : 4 lignes d'écart ===
--- reference
+++ sortie
@@ -12,8 +12,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">

=== v4/repeat66 : 17 lignes d'écart ===
--- reference
+++ sortie
@@ -12,8 +12,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -158,8 +154,5 @@
       </barline>
     </measure>
-    <measure number="1">
-      <attributes>
-        <divisions>1</divisions>
-      </attributes>
+    <measure number="6">
       <note>
         <pitch>
@@ -172,5 +165,5 @@
       </note>
     </measure>
-    <measure number="2">
+    <measure number="7">
       <barline location="left">
         <bar-style>heavy-light</bar-style>
@@ -187,5 +180,5 @@
       </note>
     </measure>
-    <measure number="3">
+    <measure number="8">
       <barline location="left">
         <ending number="1, 2" type="start">1, 2</ending>
@@ -201,5 +194,5 @@
       </note>
     </measure>
-    <measure number="4">
+    <measure number="9">
       <note>
         <pitch>
@@ -223,5 +216,5 @@
       </barline>
     </measure>
-    <measure number="5">
+    <measure number="10">
       <barline location="left">
         <ending number="3" type="start">3.</ending>

=== v4/repeat68 : 5 lignes d'écart ===
--- reference
+++ sortie
@@ -12,9 +12,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="mscVersion">4.00</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">

=== v4/repeat69 : 5 lignes d'écart ===
--- reference
+++ sortie
@@ -13,9 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2025-04-11</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-      <miscellaneous-field name="subtitle">Subtitle</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>

=== v4/tenuto_accent : 5 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2021-12-02</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -60,5 +56,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <staff>1</staff>
         <notations>

=== v4/testExcludeInvisibleElements : 111 lignes d'écart ===
--- reference
+++ sortie
@@ -12,8 +12,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2021-06-06</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -62,5 +58,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notations>
           <fermata type="upright"/>
@@ -75,5 +70,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notations print-object="no">
           <fermata type="upright"/>
@@ -88,5 +82,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notations>
           <articulations>
@@ -103,5 +96,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notations print-object="no">
           <articulations>
@@ -125,5 +117,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -135,5 +126,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -145,5 +135,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -155,5 +144,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
     </measure>
@@ -176,5 +164,4 @@
… (diff tronqué)

=== v4/testLocalTimesig1 : 5 lignes d'écart ===
--- reference
+++ sortie
@@ -13,9 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2024-02-20</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-      <miscellaneous-field name="subtitle">Subtitle</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">

=== v4/testNoteheads2 : 29 lignes d'écart ===
--- reference
+++ sortie
@@ -13,7 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2023-09-30</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -61,5 +58,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -71,5 +67,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notehead>x</notehead>
       </note>
@@ -84,5 +79,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notehead>cross</notehead>
       </note>
@@ -95,5 +89,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notehead>circle-x</notehead>
       </note>
@@ -108,5 +101,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notehead smufl="noteheadVoidWithX">other</notehead>
       </note>
@@ -119,5 +111,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notehead>triangle</notehead>
       </note>
@@ -132,5 +123,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notehead>inverted triangle</notehead>
       </note>
@@ -143,5 +133,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notehead>slashed</notehead>
       </note>
@@ -156,5 +145,4 @@
         <voice>1</voice>
… (diff tronqué)

=== v4/testTempoPrecision : 4 lignes d'écart ===
--- reference
+++ sortie
@@ -13,8 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2021-01-17</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">

=== v4/timesig-09 : 4 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>

=== v4/voiceswitching : 82 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -74,5 +70,4 @@
         <voice>1</voice>
         <type>half</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -84,5 +79,4 @@
         <voice>1</voice>
         <type>half</type>
-        <stem>up</stem>
         <notehead>cross</notehead>
       </note>
@@ -98,5 +92,4 @@
         <voice>2</voice>
         <type>half</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -108,5 +101,4 @@
         <voice>2</voice>
         <type>half</type>
-        <stem>down</stem>
         <notehead>x</notehead>
       </note>
@@ -121,5 +113,4 @@
         <voice>1</voice>
         <type>half</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -131,5 +122,4 @@
         <voice>1</voice>
         <type>half</type>
-        <stem>up</stem>
         <notehead>x</notehead>
       </note>
@@ -143,5 +133,4 @@
         <voice>1</voice>
         <type>half</type>
-        <stem>up</stem>
         <notehead>cross</notehead>
       </note>
@@ -167,5 +156,5 @@
         <stem>up</stem>
         <notations>
-          <tuplet bracket="yes" placement="above" type="start"/>
+          <tuplet bracket="yes" type="start"/>
         </notations>
       </note>
… (diff tronqué)

=== v3/Beam-CrossM1 : 23 lignes d'écart ===
--- reference
+++ sortie
@@ -12,8 +12,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -30,10 +26,8 @@
       <score-instrument id="P1-I1">
         <instrument-name>Voice</instrument-name>
-        <instrument-sound>voice.vocals</instrument-sound>
       </score-instrument>
       <midi-device id="P1-I1" port="1"></midi-device>
       <midi-instrument id="P1-I1">
         <midi-channel>1</midi-channel>
-        <midi-program>53</midi-program>
         <volume>78.7402</volume>
         <pan>0</pan>
@@ -83,6 +77,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>up</stem>
-        <beam number="1">begin</beam>
       </note>
     </measure>
@@ -96,5 +88,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>up</stem>
         <beam number="1">continue</beam>
       </note>
@@ -107,6 +98,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>up</stem>
-        <beam number="1">continue</beam>
       </note>
       <note>
@@ -118,6 +107,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>up</stem>
-        <beam number="1">continue</beam>
       </note>
       <note>
@@ -129,6 +116,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>up</stem>
-        <beam number="1">end</beam>
       </note>
       <note>
@@ -140,6 +125,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>up</stem>
… (diff tronqué)

=== v3/barline01 : 26 lignes d'écart ===
--- reference
+++ sortie
@@ -9,14 +9,6 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
-    <part-group number="1" type="start">
-      <group-symbol>bracket</group-symbol>
-      <group-barline>yes</group-barline>
-    </part-group>
     <score-part id="P1">
       <part-name>Flute 1</part-name>
@@ -24,5 +16,4 @@
       <score-instrument id="P1-I1">
         <instrument-name>Flute</instrument-name>
-        <instrument-sound>wind.flutes.flute</instrument-sound>
       </score-instrument>
       <midi-device id="P1-I1" port="1"></midi-device>
@@ -39,5 +30,4 @@
       <score-instrument id="P2-I1">
         <instrument-name>Flute</instrument-name>
-        <instrument-sound>wind.flutes.flute</instrument-sound>
       </score-instrument>
       <midi-device id="P2-I1" port="1"></midi-device>
@@ -54,5 +44,4 @@
       <score-instrument id="P3-I1">
         <instrument-name>Flute</instrument-name>
-        <instrument-sound>wind.flutes.flute</instrument-sound>
       </score-instrument>
       <midi-device id="P3-I1" port="1"></midi-device>
@@ -64,5 +53,4 @@
       </midi-instrument>
     </score-part>
-    <part-group number="1" type="stop"/>
   </part-list>
   <part id="P1">
@@ -180,4 +168,8 @@
           <line>2</line>
         </clef>
+        <clef after-barline="yes">
+          <sign>G</sign>
+          <line>2</line>
+        </clef>
       </attributes>
       <note>
@@ -225,7 +217,4 @@
     </measure>
     <measure number="5">
-      <attributes>
-        <staff-details print-object="no"/>
-      </attributes>
       <note>
         <rest measure="yes"/>
@@ -282,4 +271,8 @@
           <line>2</line>
… (diff tronqué)

=== v3/barlinedelete : 19 lignes d'écart ===
--- reference
+++ sortie
@@ -13,8 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -24,5 +20,4 @@
       <score-instrument id="P1-I1">
         <instrument-name>Piano</instrument-name>
-        <instrument-sound>keyboard.piano</instrument-sound>
       </score-instrument>
       <midi-device id="P1-I1" port="1"></midi-device>
@@ -65,5 +60,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -75,5 +69,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -85,5 +78,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -95,5 +87,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <barline location="right">
@@ -110,5 +101,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -120,5 +110,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -130,5 +119,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -140,5 +128,4 @@
… (diff tronqué)

=== v3/change : 72 lignes d'écart ===
--- reference
+++ sortie
@@ -12,8 +12,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -27,9 +23,4 @@
       <score-instrument id="P1-I1">
         <instrument-name>Flute</instrument-name>
-        <instrument-sound>wind.flutes.flute</instrument-sound>
-      </score-instrument>
-      <score-instrument id="P1-I2">
-        <instrument-name>Flute</instrument-name>
-        <instrument-sound>wind.flutes.flute</instrument-sound>
       </score-instrument>
       <midi-device id="P1-I1" port="1"></midi-device>
@@ -40,11 +31,4 @@
         <pan>0</pan>
       </midi-instrument>
-      <midi-device id="P1-I2" port="1"></midi-device>
-      <midi-instrument id="P1-I2">
-        <midi-channel>2</midi-channel>
-        <midi-program>74</midi-program>
-        <volume>78.7402</volume>
-        <pan>0</pan>
-      </midi-instrument>
     </score-part>
     <score-part id="P2">
@@ -53,9 +37,8 @@
       <score-instrument id="P2-I1">
         <instrument-name>Oboe</instrument-name>
-        <instrument-sound>wind.reed.oboe</instrument-sound>
       </score-instrument>
       <midi-device id="P2-I1" port="1"></midi-device>
       <midi-instrument id="P2-I1">
-        <midi-channel>3</midi-channel>
+        <midi-channel>2</midi-channel>
         <midi-program>69</midi-program>
         <volume>78.7402</volume>
@@ -75,5 +58,4 @@
           <beat-type>4</beat-type>
         </time>
-        <instruments>2</instruments>
         <clef>
           <sign>G</sign>
@@ -87,8 +69,6 @@
         </pitch>
         <duration>2</duration>
-        <instrument id="P1-I1"/>
         <voice>1</voice>
         <type>half</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -98,8 +78,6 @@
… (diff tronqué)

=== v3/clef-1 : 18 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -19,5 +15,4 @@
       <score-instrument id="P1-I1">
         <instrument-name>Piano</instrument-name>
-        <instrument-sound>keyboard.piano</instrument-sound>
       </score-instrument>
       <midi-device id="P1-I1" port="1"></midi-device>
@@ -47,7 +42,9 @@
         </clef>
         <clef number="2">
-          <sign>F</sign>
-          <line>4</line>
+          <sign>G</sign>
+          <line>2</line>
         </clef>
+      </attributes>
+      <attributes>
         <clef after-barline="yes" number="1">
           <sign>F</sign>
@@ -63,5 +60,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <staff>1</staff>
       </note>
@@ -74,5 +70,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <staff>1</staff>
       </note>
@@ -85,5 +80,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <staff>1</staff>
       </note>
@@ -105,5 +99,4 @@
         <voice>5</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <staff>2</staff>
       </note>
@@ -116,5 +109,4 @@
         <voice>5</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <staff>2</staff>
       </note>
@@ -127,5 +119,4 @@
… (diff tronqué)

=== v3/copypaste_tuplet_01 : 14 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -55,5 +51,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -65,5 +60,4 @@
         <voice>1</voice>
         <type>eighth</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -89,5 +83,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -103,6 +96,4 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>up</stem>
-        <beam number="1">begin</beam>
         <notations>
           <tuplet bracket="no" type="start"/>
@@ -121,6 +112,4 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>up</stem>
-        <beam number="1">continue</beam>
       </note>
       <note>
@@ -136,6 +125,4 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>up</stem>
-        <beam number="1">end</beam>
         <notations>
           <tuplet type="stop"/>
@@ -150,5 +137,4 @@
         <voice>1</voice>
         <type>half</type>
-        <stem>up</stem>
       </note>
       <barline location="right">

=== v3/grace : 6 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -20,5 +16,4 @@
       <score-instrument id="P1-I1">
         <instrument-name>Piano</instrument-name>
-        <instrument-sound>keyboard.piano</instrument-sound>
       </score-instrument>
       <midi-device id="P1-I1" port="1"></midi-device>
@@ -60,5 +55,4 @@
         <voice>1</voice>
         <type>half</type>
-        <stem>down</stem>
         <staff>1</staff>
       </note>

=== v3/keysig_78216 : 14 lignes d'écart ===
--- reference
+++ sortie
@@ -13,8 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -24,5 +20,4 @@
       <score-instrument id="P1-I1">
         <instrument-name>Piano</instrument-name>
-        <instrument-sound>keyboard.piano</instrument-sound>
       </score-instrument>
       <midi-device id="P1-I1" port="1"></midi-device>
@@ -57,7 +52,6 @@
       </note>
     </measure>
-    <measure number="1">
+    <measure number="2">
       <attributes>
-        <divisions>1</divisions>
         <key>
           <fifths>1</fifths>
@@ -70,7 +64,6 @@
       </note>
     </measure>
-    <measure number="1">
+    <measure number="3">
       <attributes>
-        <divisions>1</divisions>
         <key>
           <fifths>2</fifths>
@@ -83,7 +76,6 @@
       </note>
     </measure>
-    <measure number="1">
+    <measure number="4">
       <attributes>
-        <divisions>1</divisions>
         <key>
           <fifths>3</fifths>

=== v3/layout_elements : 188 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -38,9 +34,4 @@
       <score-instrument id="P1-I1">
         <instrument-name>Piano</instrument-name>
-        <instrument-sound>keyboard.piano</instrument-sound>
-      </score-instrument>
-      <score-instrument id="P1-I2">
-        <instrument-name>Piano</instrument-name>
-        <instrument-sound>keyboard.piano</instrument-sound>
       </score-instrument>
       <midi-device id="P1-I1" port="1"></midi-device>
@@ -51,11 +42,4 @@
         <pan>0</pan>
       </midi-instrument>
-      <midi-device id="P1-I2" port="1"></midi-device>
-      <midi-instrument id="P1-I2">
-        <midi-channel>3</midi-channel>
-        <midi-program>1</midi-program>
-        <volume>78.7402</volume>
-        <pan>0</pan>
-      </midi-instrument>
     </score-part>
   </part-list>
@@ -72,5 +56,4 @@
         </time>
         <staves>2</staves>
-        <instruments>2</instruments>
         <clef number="1">
           <sign>G</sign>
@@ -97,5 +80,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="12" type="crescendo"/>
+          <wedge number="1" type="crescendo"/>
         </direction-type>
         <staff>1</staff>
@@ -108,11 +91,9 @@
         </pitch>
         <duration>6</duration>
-        <instrument id="P1-I1"/>
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <staff>1</staff>
         <notations>
-          <slur bezier-x="36.071709" bezier-y="-21.363336" number="1" type="start"/>
+          <slur number="1" type="start"/>
           <glissando line-type="wavy" number="1" type="start">gliss.</glissando>
         </notations>
@@ -125,8 +106,6 @@
… (diff tronqué)

=== v3/lyricsline01 : 8 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -55,5 +51,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <lyric number="1">
           <syllabic>single</syllabic>
@@ -70,5 +65,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -80,5 +74,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -90,5 +83,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <barline location="right">

=== v3/measure-4 : 19 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -20,5 +16,4 @@
       <score-instrument id="P1-I1">
         <instrument-name>Voice</instrument-name>
-        <instrument-sound>voice.vocals</instrument-sound>
       </score-instrument>
       <midi-device id="P1-I1" port="1"></midi-device>
@@ -55,5 +50,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <direction placement="below">
@@ -70,5 +64,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -80,5 +73,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -90,5 +82,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
     </measure>
@@ -102,5 +93,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -112,5 +102,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -122,9 +111,8 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <direction placement="below">
         <direction-type>
… (diff tronqué)

=== v3/moonlight : 2392 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -34,5 +30,4 @@
       <score-instrument id="P1-I1">
         <instrument-name>Piano</instrument-name>
-        <instrument-sound>keyboard.piano</instrument-sound>
       </score-instrument>
       <midi-device id="P1-I1" port="1"></midi-device>
@@ -62,6 +57,6 @@
         </clef>
         <clef number="2">
-          <sign>F</sign>
-          <line>4</line>
+          <sign>G</sign>
+          <line>2</line>
         </clef>
       </attributes>
@@ -99,7 +94,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>up</stem>
         <staff>1</staff>
-        <beam number="1">begin</beam>
         <notations>
           <tuplet bracket="no" type="start"/>
@@ -119,7 +112,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>up</stem>
         <staff>1</staff>
-        <beam number="1">continue</beam>
       </note>
       <note>
@@ -135,7 +126,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>up</stem>
         <staff>1</staff>
-        <beam number="1">end</beam>
         <notations>
           <tuplet type="stop"/>
@@ -155,7 +144,5 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>up</stem>
         <staff>1</staff>
-        <beam number="1">begin</beam>
         <notations>
           <tuplet bracket="no" type="start"/>
@@ -175,7 +162,5 @@
           <normal-notes>2</normal-notes>
… (diff tronqué)

=== v3/repeat01 : 4 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">

=== v3/repeat06 : 4 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">

=== v3/repeat07 : 4 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">

=== v3/repeat11 : 4 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">

=== v3/repeat20 : 4 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">

=== v3/repeat39 : 4 lignes d'écart ===
--- reference
+++ sortie
@@ -12,8 +12,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">

=== v3/selectionfilter1 : 11 lignes d'écart ===
--- reference
+++ sortie
@@ -12,8 +12,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -27,5 +23,4 @@
       <score-instrument id="P1-I1">
         <instrument-name>Violin</instrument-name>
-        <instrument-sound>strings.violin</instrument-sound>
       </score-instrument>
       <midi-device id="P1-I1" port="1"></midi-device>
@@ -70,9 +65,8 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="12" type="crescendo"/>
+          <wedge number="1" type="crescendo"/>
         </direction-type>
       </direction>
@@ -85,5 +79,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -95,5 +88,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <direction placement="below">
@@ -110,5 +102,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
     </measure>

=== v3/testBreaksPage : 4 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2020-02-24</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="2">

=== v3/testChordSymbols : 13 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2020-02-24</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -52,13 +48,8 @@
           <root-alter>-1</root-alter>
         </root>
-        <kind parentheses-degrees="yes" text="m">minor</kind>
-        <bass arrangement="horizontal">
+        <kind text="Ebm(add9)/F"></kind>
+        <bass>
           <bass-step>F</bass-step>
         </bass>
-        <degree>
-          <degree-value>9</degree-value>
-          <degree-alter>0</degree-alter>
-          <degree-type>add</degree-type>
-        </degree>
       </harmony>
       <note>

=== v3/testHairpinDynamics : 44 lignes d'écart ===
--- reference
+++ sortie
@@ -14,8 +14,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2020-03-14</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -74,5 +70,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="12" type="crescendo"/>
+          <wedge number="1" type="crescendo"/>
         </direction-type>
       </direction>
@@ -85,5 +81,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <lyric number="1">
           <syllabic>single</syllabic>
@@ -99,5 +94,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <lyric number="1">
           <syllabic>single</syllabic>
@@ -113,5 +107,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <note>
@@ -123,5 +116,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>down</stem>
       </note>
       <direction placement="below">
@@ -139,5 +131,5 @@
         </direction-type>
         <direction-type>
-          <wedge number="1" spread="12" type="crescendo"/>
+          <wedge number="1" type="crescendo"/>
         </direction-type>
       </direction>
@@ -150,5 +142,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <lyric number="1">
           <syllabic>single</syllabic>
@@ -164,5 +155,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
… (diff tronqué)

=== v3/testHarmony6 : 42 lignes d'écart ===
--- reference
+++ sortie
@@ -14,8 +14,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2020-06-24</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -71,5 +67,5 @@
           <root-step>C</root-step>
         </root>
-        <kind text="m">minor</kind>
+        <kind text="Cm"></kind>
       </harmony>
       <note>
@@ -89,8 +85,5 @@
     <measure number="2">
       <harmony print-frame="no">
-        <numeral>
-          <numeral-root text="i">1</numeral-root>
-        </numeral>
-        <kind>minor</kind>
+        <kind text="i"></kind>
       </harmony>
       <note>
@@ -110,8 +103,5 @@
     <measure number="3">
       <harmony print-frame="no">
-        <numeral>
-          <numeral-root>1</numeral-root>
-        </numeral>
-        <kind text="m">minor</kind>
+        <kind text="m"></kind>
       </harmony>
       <note>
@@ -131,8 +121,5 @@
     <measure number="4">
       <harmony print-frame="no">
-        <root>
-          <root-step text="">C</root-step>
-        </root>
-        <kind text="xyz">none</kind>
+        <kind text="xyz"></kind>
       </harmony>
       <note>
@@ -152,8 +139,5 @@
     <measure number="5">
       <harmony print-frame="no">
-        <root>
-          <root-step text="">C</root-step>
-        </root>
-        <kind text="xyz">none</kind>
+        <kind text="xyz"></kind>
       </harmony>
       <note>
@@ -173,6 +157,5 @@
     <measure number="6">
… (diff tronqué)

=== v3/testMultiInstrumentPart3 : 50 lignes d'écart ===
--- reference
+++ sortie
@@ -12,8 +12,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2019-10-04</miscellaneous-field>
-      <miscellaneous-field name="platform">Microsoft Windows</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -29,8 +25,4 @@
         <instrument-sound>wind.reed.clarinet.a</instrument-sound>
       </score-instrument>
-      <score-instrument id="P1-I2">
-        <instrument-name>B♭ Clarinet</instrument-name>
-        <instrument-sound>wind.reed.clarinet.bflat</instrument-sound>
-      </score-instrument>
       <midi-device id="P1-I1" port="1"></midi-device>
       <midi-instrument id="P1-I1">
@@ -40,11 +32,4 @@
         <pan>0</pan>
       </midi-instrument>
-      <midi-device id="P1-I2" port="1"></midi-device>
-      <midi-instrument id="P1-I2">
-        <midi-channel>2</midi-channel>
-        <midi-program>72</midi-program>
-        <volume>78.7402</volume>
-        <pan>0</pan>
-      </midi-instrument>
     </score-part>
   </part-list>
@@ -60,5 +45,4 @@
           <beat-type>4</beat-type>
         </time>
-        <instruments>2</instruments>
         <clef>
           <sign>G</sign>
@@ -82,8 +66,6 @@
         </pitch>
         <duration>1</duration>
-        <instrument id="P1-I1"/>
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -93,8 +75,6 @@
         </pitch>
         <duration>1</duration>
-        <instrument id="P1-I1"/>
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
       </note>
       <note>
@@ -104,8 +84,6 @@
         </pitch>
         <duration>1</duration>
-        <instrument id="P1-I1"/>
… (diff tronqué)

=== v3/testTboxWords1 : 14 lignes d'écart ===
--- reference
+++ sortie
@@ -13,8 +13,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2019-12-26</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -114,9 +110,4 @@
         <direction-type>
           <words font-size="12" valign="top">Above 2</words>
-        </direction-type>
-      </direction>
-      <direction placement="below">
-        <direction-type>
-          <words font-size="12" valign="top">Below 2</words>
         </direction-type>
       </direction>
@@ -128,4 +119,9 @@
     </measure>
     <measure number="3">
+      <direction placement="below">
+        <direction-type>
+          <words font-size="12" valign="top">Below 2</words>
+        </direction-type>
+      </direction>
       <note>
         <rest measure="yes"/>

=== v3/testTuplets8 : 61 lignes d'écart ===
--- reference
+++ sortie
@@ -14,8 +14,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2020-03-22</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -76,7 +72,6 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>down</stem>
         <notations>
-          <tuplet bracket="yes" number="1" placement="above" type="start"/>
+          <tuplet bracket="yes" number="1" type="start"/>
         </notations>
       </note>
@@ -93,5 +88,4 @@
           <normal-notes>2</normal-notes>
         </time-modification>
-        <stem>down</stem>
       </note>
       <note>
@@ -107,7 +101,4 @@
           <normal-notes>8</normal-notes>
         </time-modification>
-        <stem>up</stem>
-        <beam number="1">begin</beam>
-        <beam number="2">begin</beam>
         <notations>
           <tuplet bracket="no" number="2" type="start">
@@ -135,7 +126,4 @@
           <normal-notes>8</normal-notes>
         </time-modification>
-        <stem>up</stem>
-        <beam number="1">continue</beam>
-        <beam number="2">continue</beam>
       </note>
       <note>
@@ -151,7 +139,4 @@
           <normal-notes>8</normal-notes>
         </time-modification>
-        <stem>up</stem>
-        <beam number="1">continue</beam>
-        <beam number="2">continue</beam>
       </note>
       <note>
@@ -167,7 +152,4 @@
           <normal-notes>8</normal-notes>
         </time-modification>
-        <stem>up</stem>
-        <beam number="1">continue</beam>
-        <beam number="2">continue</beam>
       </note>
       <note>
@@ -183,7 +165,4 @@
           <normal-notes>8</normal-notes>
… (diff tronqué)

=== v3/testWedge4 : 20 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2019-10-20</miscellaneous-field>
-      <miscellaneous-field name="platform">Apple Macintosh</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -67,5 +63,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="12" type="crescendo"/>
+          <wedge number="1" type="crescendo"/>
         </direction-type>
       </direction>
@@ -78,5 +74,4 @@
         <voice>1</voice>
         <type>half</type>
-        <stem>up</stem>
       </note>
       <direction placement="below">
@@ -95,5 +90,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="12" type="crescendo"/>
+          <wedge number="1" type="crescendo"/>
         </direction-type>
       </direction>
@@ -106,9 +101,8 @@
         <voice>1</voice>
         <type>half</type>
-        <stem>up</stem>
       </note>
       <direction placement="below">
         <direction-type>
-          <wedge number="2" spread="12" type="stop"/>
+          <wedge number="2" type="stop"/>
         </direction-type>
       </direction>
@@ -116,5 +110,5 @@
         <duration>4</duration>
       </backup>
-      <note print-object="no">
+      <note>
         <rest/>
         <duration>1</duration>
@@ -122,5 +116,5 @@
         <type>quarter</type>
       </note>
-      <note print-object="no">
+      <note>
         <rest/>
         <duration>1</duration>
@@ -128,5 +122,5 @@
         <type>quarter</type>
       </note>
-      <note print-object="no">
… (diff tronqué)

=== v3/timesig-05 : 53 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -19,5 +15,4 @@
       <score-instrument id="P1-I1">
         <instrument-name>Flute</instrument-name>
-        <instrument-sound>wind.flutes.flute</instrument-sound>
       </score-instrument>
       <midi-device id="P1-I1" port="1"></midi-device>
@@ -48,5 +43,5 @@
       <direction placement="below">
         <direction-type>
-          <wedge number="1" spread="12" type="crescendo"/>
+          <wedge number="1" type="crescendo"/>
         </direction-type>
       </direction>
@@ -59,7 +54,6 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notations>
-          <slur bezier-x="27.962317" bezier-y="-21.63124" number="1" orientation="under" placement="below" type="start"/>
+          <slur number="1" orientation="under" placement="below" type="start"/>
           <articulations>
             <staccato/>
@@ -76,5 +70,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notations>
           <articulations>
@@ -92,5 +85,4 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notations>
           <articulations>
@@ -108,7 +100,6 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notations>
-          <slur bezier-x="38.891248" bezier-y="-36.731298" number="2" type="start"/>
+          <slur number="2" type="start"/>
           <fermata type="upright"/>
         </notations>
@@ -130,7 +121,6 @@
         <voice>1</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <notations>
-          <slur bezier-x="-27.962317" bezier-y="-21.63124" number="1" type="stop"/>
… (diff tronqué)

=== v3/undoResequenceMeasure : 11 lignes d'écart ===
--- reference
+++ sortie
@@ -9,8 +9,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <part-list>
@@ -20,5 +16,4 @@
       <score-instrument id="P1-I1">
         <instrument-name>Flute</instrument-name>
-        <instrument-sound>wind.flutes.flute</instrument-sound>
       </score-instrument>
       <midi-device id="P1-I1" port="1"></midi-device>
@@ -93,5 +88,5 @@
       <direction placement="above" system="only-top">
         <direction-type>
-          <rehearsal font-size="14" font-weight="bold" justify="center">5</rehearsal>
+          <rehearsal font-size="14" font-weight="bold">5</rehearsal>
         </direction-type>
       </direction>
@@ -142,5 +137,5 @@
       <direction placement="above" system="only-top">
         <direction-type>
-          <rehearsal font-size="14" font-weight="bold" justify="center">QQ</rehearsal>
+          <rehearsal font-size="14" font-weight="bold">QQ</rehearsal>
         </direction-type>
       </direction>
@@ -191,5 +186,5 @@
       <direction placement="above" system="only-top">
         <direction-type>
-          <rehearsal font-size="14" font-weight="bold" justify="center">Foo</rehearsal>
+          <rehearsal font-size="14" font-weight="bold">Foo</rehearsal>
         </direction-type>
       </direction>

=== v3/voices : 80 lignes d'écart ===
--- reference
+++ sortie
@@ -12,8 +12,4 @@
       <supports element="stem" type="yes"/>
     </encoding>
-    <miscellaneous>
-      <miscellaneous-field name="creationDate">2026-11-16</miscellaneous-field>
-      <miscellaneous-field name="platform">Linux</miscellaneous-field>
-    </miscellaneous>
   </identification>
   <credit page="1">
@@ -31,5 +27,5 @@
       <midi-device id="P1-I1" port="1"></midi-device>
       <midi-instrument id="P1-I1">
-        <midi-channel>3</midi-channel>
+        <midi-channel>1</midi-channel>
         <midi-program>1</midi-program>
         <volume>78.7402</volume>
@@ -46,5 +42,5 @@
       <midi-device id="P2-I1" port="1"></midi-device>
       <midi-instrument id="P2-I1">
-        <midi-channel>1</midi-channel>
+        <midi-channel>2</midi-channel>
         <midi-program>58</midi-program>
         <volume>78.7402</volume>
@@ -57,5 +53,5 @@
       <attributes>
         <divisions>1</divisions>
-        <key number="1">
+        <key>
           <fifths>-4</fifths>
         </key>
@@ -101,5 +97,4 @@
         <voice>2</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <staff>1</staff>
       </note>
@@ -113,5 +108,4 @@
         <voice>2</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <staff>1</staff>
       </note>
@@ -124,5 +118,4 @@
         <voice>2</voice>
         <type>quarter</type>
-        <stem>down</stem>
         <staff>1</staff>
       </note>
@@ -146,5 +139,4 @@
         <voice>3</voice>
         <type>quarter</type>
-        <stem>up</stem>
         <staff>1</staff>
       </note>
@@ -157,5 +149,4 @@
         <voice>3</voice>
         <type>quarter</type>
-        <stem>up</stem>
… (diff tronqué)
```
