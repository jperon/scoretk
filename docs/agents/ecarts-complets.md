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
| layout_elements | **identique** |
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
| testTboxWords1 | **identique** |
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
| copypaste_parts | **identique** |
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
| testExcludeInvisibleElements | **identique** |
| testLocalTimesig1 | **identique** |
| testNoteheads2 | **identique** |
| testTempoPrecision | **identique** |
| timesig-09 | **identique** |
| voiceswitching | **identique** |

## Écarts groupés par balise

Fréquence = nombre de lignes canoniques modifiées sur les deux corpus ; fichiers = nombre de fichiers touchés. Tri par fréquence décroissante.

| Balise | Lignes | Fichiers | Exemples |
| --- | ---: | ---: | --- |
| `duration` | 18 | 1 | `<duration>12</duration>` `<duration>24</duration>` `<duration>36</duration>` |
| `stem` | 18 | 1 | `<stem>down</stem>` `<stem>up</stem>` |
| `forward` | 16 | 1 | `</forward>` `<forward>` |
| `tied` | 14 | 1 | `<tied orientation="over" placement="above" type="start"/>` `<tied type="start"/>` `<tied orientation="under" placement="below" type="start"/>` |
| `attributes` | 4 | 1 | `<attributes>` `</attributes>` |
| `backup` | 4 | 1 | `</backup>` `<backup>` |
| `barline` | 4 | 1 | `<barline location="right">` `</barline>` |
| `bar-style` | 2 | 1 | `<bar-style>light-light</bar-style>` |
| `staff-details` | 2 | 1 | `<staff-details print-object="no"/>` |

## Écarts qui dépendent de la mise en page calculée

Certains écarts ne viennent pas d'une donnée du `.mscx` mais d'un objet que MuseScore fabrique pendant la mise en page, absente du modèle :

- **direction de hampe d'une ligature croisée** : `Chord::up()` dépend des notes de la ligature, elle-même groupée par la mise en page ;
- **découpage des `<forward>`/`<backup>` et durées d'une voix à trous** : la position des segments vient de `Measure::layout`, pas du fichier ;
- **`<tied orientation>`** : portée par le `TieSegment` calculé par la mise en page (la direction n'est pas dans le `.mscx`) ;
- **barres de courtoisie** : ajoutées par la mise en page quand un système commence par un changement d'armure ou de métrique ; reproductibles seulement si le saut de système figure dans le fichier (`<LayoutBreak>`), pas quand la largeur de page coupe le système ;
- **`<staff-details print-object="no">` des portées masquées** : décidé par `hideEmptyStaves` après calcul des hauteurs de systèmes.

## Diffs

```diff

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
