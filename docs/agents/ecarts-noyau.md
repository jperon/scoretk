# Écarts du noyau musical sur le corpus v4

Ce document inventorie les écarts constatés par le test de bout en bout
`crates/mscz2xml/tests/e2e_corpus.rs` entre la conversion `X.mscx` →
MusicXML et la référence `X.ref.musicxml` exportée par MuseScore 4.7.4.

Méthode : les deux documents sont normalisés (métadonnées, mise en page,
`<direction>`, `<stem>`, `<beam>`, `<print>`, `<notations>` hors `tied`/`tuplet`,
`<barline>` hors `<repeat>` retirés), puis comparés ligne à ligne avec `similar`.
Le tableau ci-dessous donne, pour chaque fichier, le nombre de lignes d'écart
après normalisation ; les fichiers marqués **identique** constituent la liste
`EXPECTED_CONFORMANT` du test.

Écarts triés par fréquence (nombre de fichiers touchés), puis par nombre
d'occurrences. Les causes sont classées « lecteur », « modèle » ou « export ».
Les écarts 1 à 4 sont corrigés (voir chaque section) ; les suivants restent
ouverts.

## Tableau de conformité

| Fichier | Lignes d'écart |
| --- | ---: |
| beamStemDir | **identique** |
| changevisibility | **identique** |
| copypaste_parts | 10 |
| copypastesymbollist-lyrics | **identique** |
| dynamics | **identique** |
| dynamics_niente | 16 |
| expression-1 | **identique** |
| hairpin_dynamic_inside | 2 |
| hairpin_dynamic_jump | 2 |
| hairpin_simple | 2 |
| hairpins_and_repeats | **identique** |
| linked-ties | **identique** |
| mmrest | 23 |
| part-textlines | 10 |
| partExclusion | 8 |
| repeat65 | **identique** |
| repeat66 | 13 |
| repeat68 | **identique** |
| repeat69 | **identique** |
| tenuto_accent | **identique** |
| testExcludeInvisibleElements | 20 |
| testLocalTimesig1 | **identique** |
| testNoteheads2 | **identique** |
| testTempoPrecision | **identique** |
| timesig-09 | **identique** |
| voiceswitching | 30 |

## 1. Clef par défaut absente — 14 fichiers

**Corrigé** (lecteur/modèle/export) : `Staff::default_clef` reprend
`<defaultClef>` (ou ses variantes concert/transposée) et l'export écrit une
clef du type par défaut de la portée quand aucun `<Clef>` n'ouvre la partition,
comme la mise en page de MuseScore.

Fichiers : beamStemDir, changevisibility, copypaste_parts, dynamics,
hairpin_dynamic_inside, linked-ties, mmrest, part-textlines, repeat65,
tenuto_accent, testExcludeInvisibleElements, testLocalTimesig1, timesig-09,
voiceswitching.

Diff (beamStemDir) :

```diff
-        <clef number="1">
-          <sign>G</sign>
-          <line>2</line>
-        </clef>
-        <clef number="2">
-          <sign>F</sign>
-          <line>4</line>
-        </clef>
```

Ces `.mscx` ne contiennent aucun élément `<Clef>` : la clef vient de
`<defaultClef>F</defaultClef>` dans `<Staff id="2">` (parties à plusieurs
portées) ou de la clef par défaut « sol » d'une portée simple. **Cause :
lecteur/modèle.** `read4xx` n'insère un segment `HeaderClef` que s'il rencontre
un `<Clef>` ; `<defaultClef>` n'est pas lu et le modèle n'a pas de clef par
défaut par portée. L'export ne peut donc écrire aucune clef (son repli
« sol » suppose un segment `HeaderClef` existant), alors que MuseScore crée
cette clef à la lecture (`read400`).

## 2. `<instrument-name>` pris dans le nom long au lieu du nom de piste — 3 fichiers

**Corrigé** (lecteur/modèle/export) : `Instrument::track_name` reprend
`<trackName>` et `<instrument-name>` l'écrit tel quel, comme
`MScoreTextToMusicXml::toPlainText(instr->trackName())` ; le nom long ne sert
plus de repli.

Fichiers : hairpin_dynamic_inside, part-textlines, testLocalTimesig1.

Diffs :

```diff
-        <instrument-name></instrument-name>
+        <instrument-name>Steel Guitar</instrument-name>
```

```diff
-        <instrument-name>B♭ Clarinet</instrument-name>
+        <instrument-name>Bb Clarinet</instrument-name>
```

```diff
-        <instrument-name>Piano</instrument-name>
+        <instrument-name>Piano 1</instrument-name>
```

MuseScore écrit dans `<instrument-name>` le `<trackName>` de l'instrument, non
son `<longName>` (d'où la chaîne vide de `hairpin_dynamic_inside`, dont le
`<trackName>` est vide, et le « Piano » de `testLocalTimesig1`, dont le
`<longName>` vaut « Piano 1 »). **Cause : lecteur/modèle.** `read_instrument`
ne stocke pas le nom de piste (TODO explicite) ; l'export se replie sur
`instrument.long_name`. Le même défaut explique le « Bb » au lieu de « B♭ »
(le nom long est déjà dépouillé du bémol par `plain_text`).

## 3. Têtes de note : mauvais nom, têtes nommées perdues — 3 fichiers

**Corrigé** (lecteur/modèle/export) : `Note` distingue désormais le groupe de
tête (`head_group`, `NoteHeadGroup`) du type (`head`), et `visible`.
L'export reproduit `writeNotehead` : valeur MusicXML du groupe (`x`, `cross`,
`circle-x`, `slashed`, `triangle`, `inverted triangle`, solfège…), nom SMuFL
avec `other` pour les groupes sans équivalent, `none` pour une note invisible,
attribut `filled` d'après `<headType>`.

Fichiers : testNoteheads2 (conforme), voiceswitching, testExcludeInvisibleElements.

Diff (testNoteheads2) :

```diff
-        <notehead>x</notehead>
+        <notehead>cross</notehead>
...
-        <notehead>circle-x</notehead>
+        <notehead>x</notehead>
...
-        <notehead>slashed</notehead>
+        <notehead>slash</notehead>
...
-        <notehead>so</notehead>
```

Deux problèmes se cumulent :

- **lecteur/modèle** : `names::note_head` replie des têtes MuseScore distinctes
  (`cross`, `xcircle`, `withx`, `slashed1/2`, `diamond-old`, `large-arrow`,
  `plus`, `circled`, têtes de solfège `do/ré/mi/fa/sol/la/ti`, `none`…) sur
  quelques variantes du modèle ; `NoteHeadType::Other(u8)` ne conserve pas le
  nom d'origine ;
- **export** : `write_notehead` réécrit ces variantes avec des noms MusicXML
  approximatifs (`Cross` → `cross` au lieu de `x`, `X` → `x` au lieu de
  `circle-x`, `Slash` → `slash` au lieu de `slashed`) et n'émet rien pour
  `Other`, `none` ou les têtes nommées.

## 4. Durée réelle (`<duration>`) ignorée : durée, divisions, `time-modification`, `backup` — 2 fichiers

**Corrigé** (lecteur/modèle/export) : `ChordRest::ticks` conserve le
`<duration>` explicite, `Staff::time_stretch` l'étirement d'une indication
locale, et `Chord::tremolo_two_chord` le trémolo à deux notes (appariement de
l'accord suivant dans la même voix, comme `Measure::connectTremolo`). L'export
écrit `<duration>` depuis les ticks réels, avance la position avec
`actualTicks` (ticks / étirement), compte la moitié d'un trémolo dans
`calcDivisions` et ajoute `<time-modification>` 2:1 pour un trémolo.

Fichiers : timesig-09 (conforme), copypaste_parts (ne garde que l'écart 5).

Diff initial (timesig-09) :

```diff
-        <duration>2</duration>
+        <duration>4</duration>
         <voice>1</voice>
         <type>whole</type>
-        <time-modification>
-          <actual-notes>2</actual-notes>
-          <normal-notes>1</normal-notes>
-        </time-modification>
       </note>
+      <backup>
+        <duration>2</duration>
+      </backup>
```

Ces accords portent `<durationType>whole</durationType>` **et**
`<duration>1/2</duration>` (figure ronde étirée à une demi-ronde). `read_chord`
lit `<duration>` dans `ParsedChord::ticks` et avance le curseur en
conséquence ; le modèle ne gardait que `duration` (la figure) et l'export
recalculait la durée avec `duration.ticks()`, d'où l'absence de
`<time-modification>` 2:1, de `backup` corrects et de divisions justes.

## 5. Spanner `Ottava` non appliqué à la hauteur — 2 fichiers

Fichiers : copypaste_parts, testExcludeInvisibleElements.

Diff (copypaste_parts) :

```diff
         <pitch>
           <step>B</step>
-          <octave>5</octave>
+          <octave>4</octave>
```

Les mesures concernées sont sous un `<Spanner type="Ottava">` (`8va`). MuseScore
exporte la hauteur **écrite** (B5) et la direction `<octave-shift>`, que la
normalisation retire ; l'export de `mscz2xml` ignore tous les spanners, donc
 écrit la hauteur concert (B4). **Cause : export** (spanners non modélisés).

## 6. `bracket` des n-olets — 1 fichier, 9 occurrences

Fichier : voiceswitching.

Diff :

```diff
-        <tuplet bracket="yes" type="start"/>
+        <tuplet bracket="no" type="start"/>
```

MuseScore décide de l'affichage du crochet (règle de style/ratio) ; le modèle
`Tuplet::bracket` reste à `false` (le lecteur ne le calcule pas, TODO
« crochets »). **Cause : lecteur/modèle**, l'export ne faisant que recopier le
drapeau.

## 7. Nombres affichés des n-olets imbriqués réduits — 1 fichier, 3 occurrences

Fichier : voiceswitching.

Diff :

```diff
           <tuplet-actual>
-            <tuplet-number>4</tuplet-number>
+            <tuplet-number>1</tuplet-number>
```

Un n-olet `<normalNotes>4</normalNotes><actualNotes>4</actualNotes>` a un ratio
4/4. Le modèle stocke un `Fraction` réduit à 1/1 et perd les nombres affichés ;
l'export écrit `number()` (numérateur réduit). **Cause : modèle/export.**

## 8. Nombre de reprises d'une barre de fin — 1 fichier

Fichier : repeat65.

Diff :

```diff
-      <repeat direction="backward" times="3"/>
+      <repeat direction="backward"/>
```

`<endRepeat>3</endRepeat>` donne le nombre 3 ; `read4xx` ne teste que la
présence de l'élément et `BarLineType::EndRepeat` ne porte pas de compteur.
**Cause : lecteur/modèle** (l'export ne peut pas écrire `times`).

## 9. Silences multi-mesures (`<measure-style>`) — 1 fichier

Fichier : mmrest.

Diff :

```diff
-      <attributes>
-        <measure-style>
-          <multiple-rest>4</multiple-rest>
-        </measure-style>
-      </attributes>
```

Le `.mscx` contient `<multiMeasureRest>4</multiMeasureRest>` ; le lecteur
l'ignore et le modèle n'a pas de champ correspondant (TODO `<measure-style>`
côté export). **Cause : lecteur/modèle.**

## 10. `<part-name-display>` / `<part-abbreviation-display>` — 1 fichier

Fichier : part-textlines.

Diff :

```diff
-      <part-name-display>
-        <display-text>B</display-text>
-        <accidental-text>flat</accidental-text>
-        <display-text>Clarinet</display-text>
-      </part-name-display>
```

MuseScore décompose les noms contenant des altérations en fragments
`display-text` / `accidental-text`. Le modèle ne stocke que du texte plat.
**Cause : export** (TODO explicite dans `write_part_list`).

## 11. Réglages de canal MIDI (volume) — 1 fichier

Fichier : hairpin_dynamic_inside.

Diff :

```diff
-        <volume>79.5276</volume>
+        <volume>78.7402</volume>
```

Le `.mscx` a `<controller ctrl="7" value="101"/>` ; MuseScore en dérive
79,5276. Le lecteur ne lit pas les contrôleurs et l'export écrit la valeur par
défaut 78,7402 en dur (commentaire « le modèle ne porte pas les réglages de
canal »). **Cause : lecteur/modèle + export.**

## 12. `<sound tempo>` d'un tempo invisible — 1 fichier

Fichier : testExcludeInvisibleElements.

Diff :

```diff
-      <sound tempo="90"/>
```

Le `.mscx` contient un `<Tempo><tempo>1.5</tempo><visible>0</visible>`.
MuseScore n'écrit pas de `<direction>` (texte invisible) mais écrit un
`<sound tempo="90"/>` autonome dans la mesure. Le modèle `Tempo` n'a pas de
champ de visibilité et l'export écrit toujours `<sound>` **dans** la direction,
que la normalisation retire. **Cause : lecteur/modèle + export.**

## 13. Liaisons : `<measures>` de `<location>` ignoré — 1 fichier

Fichier : testExcludeInvisibleElements (une paire de liaisons sur trois).

Diff :

```diff
-        <tie type="start"/>
...
-        <tied type="start"/>
```

`register_tie` calcule la position de l'autre extrémité avec les seules
`<fractions>` de `<location>`, en ignorant `<measures>`. Pour des liaisons
séparées d'une mesure avec `<fractions>-3/4</fractions>`, l'appariement échoue
ou relie la mauvaise paire (deux paires sur trois se raccordent ici par
coïncidence, la première reste orpheline). **Cause : lecteur.** Les trois
liaisons sont présentes dans la référence, y compris celles dont le trait est
`<visible>0</visible>`.

## 14. Indication de mesure locale par portée — 1 fichier

**Corrigé pour le corpus** (mêmes ajouts que l'écart 4) : le silence de la
seconde portée porte un `<duration>` explicite, désormais conservé, et
`Staff::time_stretch` permet d'avancer la position comme
`DurationElement::actualTicks`. testLocalTimesig1 est conforme.

Fichier : testLocalTimesig1.

Diff initial :

```diff
-        <duration>6</duration>
+        <duration>9</duration>
```

Deux portées d'une même partie portent des indications locales différentes
(9/8 et 3/4 avec `<stretchN>2</stretchN><stretchD>3</stretchD>`). Le modèle
n'a qu'une durée de mesure globale (`Measure::len`), calculée à partir de la
première portée ; sans `<duration>` explicite, le silence « mesure » de la
seconde portée prendrait encore 9/8 au lieu de 3/4 (durée de mesure par portée
non modélisée).

## 15. Textes de ligne, cadres `TBox`, changements d'instrument, swing — famille textes

**Corrigé** (lecteur/modèle/export) et couvert par
`crates/mscz2xml/tests/texts_corpus.rs` sur le corpus **v3 et v4** (l'e2e v4
ignore les `<direction>` et `<print>`) :

- **lignes de texte** (`<Spanner type="TextLine">`) : `TextLineInfo` porte
  `lineVisible`, `beginText`/`endText`, style de trait, crochets, hauteurs de
  crochet, polices et `system`. L'export reproduit `ExportMusicXml::textLine`
  et `writeHairpinText` : `<words>` de début/fin, `<bracket type=… number=…
  line-end=… line-type=… end-length=…>`, `<dashes>` pour un trait sans
  crochet, et l'`<offset sound="yes">` négatif des fins de ligne invisible
  (la position sentinelle `Fraction(-1,1)` du C++) ;
- **textes de `TBox`** (`tboxTextAsWords`) : lus dans les deux lecteurs,
  exportés en `<words valign="top">` au début de la mesure, au-dessus ou
  au-dessous selon le cadre (défauts de style 3.x : 12 points) ;
- **changements d'instrument** (`writeInstrumentChange`) :
  `<print><part-name-display>`/`<part-abbreviation-display>` (♭/♯ en
  `<accidental-text>`), `<transpose>` dans `<attributes>` et
  `<sound><instrument-change id="Pn-Im"><instrument-sound>`. L'identifiant
  MusicXML absent des `.mscx` 3.x est retrouvé par nom ou programme MIDI
  (`recognize_musicxml_id`) ;
- **tempo** : `<type>aTempo</type>`/`<type>tempoPrimo</type>` reprennent le
  dernier tempo normal (`tempoSound`) ;
- **swing** : `<swing unit=… ratio=…>` d'un `SystemText` exporté en
  `<sound><swing>` (`straight`, `first`/`second`/`swing-type`), offset
  compris pour un texte invisible ;
- **crédits 3.x** : les `VBox` sont lus (styles 3.x : titre 24, sous-titre 14,
  compositeur/parolier 12, `frame` 12, `instrument_excerpt` 18, copyright 8),
  avec pagination et répétition du copyright ; les tailles de police suivent
  `QString::arg(double)` (6 chiffres significatifs) au lieu d'un arrondi à
  deux décimales.

Écart restant, documenté et filtré dans `texts_corpus.rs` : les `<words>` et
`<dashes>` des **soufflets** (`HairPin` avec `beginText`/`endText`), dont la
mise en forme relève de `writeHairpinText` côté famille nuances
(`testHairpinDynamics`, `voices`). Le repère « A » ré-exporté de
`layout_elements` vient d'une reprise de mesure (`RepeatMeasure`), hors
famille textes.
