# Écarts complets sur le corpus (parité complète)

Ce document est produit par le test `ecarts_complets_document` de `crates/mscz2xml/tests/e2e_corpus.rs`. Il compare chaque `.mscx` à sa référence `X.ref.musicxml` après retrait de la **mise en page pure** uniquement : attributs `default-x`, `default-y`, `relative-x`, `relative-y`, `width`, blocs `<defaults>`, `<print>` (et leurs `<system-layout>`/`<staff-layout>`) et `<encoding-date>`. Tout le reste est comparé : `<stem>`, `<beam>`, `<barline>`, `<direction>`, `<notations>`, `<credit>`, `<part-list>`, `<identification>`, etc.

## Tableau par fichier

### Corpus v3

| Fichier | Lignes d'écart |
| --- | ---: |
| Beam-CrossM1 | **identique** |
| barline01 | **identique** |
| barlinedelete | **identique** |
| change | **identique** |
| clef-1 | **identique** |
| copypaste_tuplet_01 | **identique** |
| grace | **identique** |
| keysig_78216 | **identique** |
| layout_elements | **identique** |
| lyricsline01 | **identique** |
| measure-4 | **identique** |
| moonlight | 18 |
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
| `stem` | 18 | 1 | `<stem>down</stem>` `<stem>up</stem>` |
| `barline` | 4 | 1 | `<barline location="right">` `</barline>` |
| `bar-style` | 2 | 1 | `<bar-style>light-light</bar-style>` |

## Écarts qui dépendent de la mise en page calculée

Certains écarts ne viennent pas d'une donnée du `.mscx` mais d'un objet que MuseScore fabrique pendant la mise en page, absente du modèle. Ils sont irréductibles sans porter le moteur de mise en page :

- **direction de hampe d'une ligature croisée** (`v3/moonlight`, 9 notes) : `BeamLayout::layout1` fixe la direction de la ligature à partir de `computeUpForMovedCross` et `isBelowCrossBeam`, donc du groupement des ligatures et de la géométrie des notes, pas du `<Beam><StemDirection>` du fichier ;
- **barres de courtoisie d'un système coupé par la largeur de page** (`v3/voices`, 2 barres `light-light`) : `MeasureLayout::createEndBarLines` ne pose la barre double que si la mesure est la dernière du système (`isLastMeasureInSystem`) et que la suivante change d'armure. Quand le saut de système figure dans le fichier (`<LayoutBreak>`), il est reproduit (voir `export/systems.rs` et `read/measure_repeat.rs`) ; une coupure automatique dépend de la largeur calculée des mesures, donc de toute la mise en page horizontale.

## Diffs

```diff

=== v3/moonlight : 18 lignes d'écart ===
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
