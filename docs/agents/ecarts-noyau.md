# Écarts du noyau musical sur le corpus v4

Ce document inventorie les écarts constatés par le test de bout en bout
`crates/mscz2xml/tests/e2e_corpus.rs` entre la conversion `X.mscx` →
MusicXML et la référence `X.ref.musicxml` exportée par MuseScore 4.7.4.

Méthode : les deux documents sont normalisés (métadonnées, mise en page,
`<direction>`, `<stem>`, `<beam>`, `<print>`, `<notations>` hors `tied`/`tuplet`,
`<barline>` hors `<repeat>` retirés), puis comparés ligne à ligne avec `similar`.
Le test `crates/mscz2xml/tests/noyau_corpus.rs` compare en plus, fichier par
fichier et mesure par mesure, les seuls éléments de la famille noyau (hauteurs,
`octave-shift`, n-olets, `measure-style`, noms de partie, volume/pan, liaisons,
`sound tempo`, `repeat times`), avec un nombre d'écarts attendu explicite.
Le tableau ci-dessous donne, pour chaque fichier, le nombre de lignes d'écart
après normalisation ; les fichiers marqués **identique** constituent la liste
`EXPECTED_CONFORMANT` du test.

Écarts triés par fréquence (nombre de fichiers touchés), puis par nombre
d'occurrences. Les causes sont classées « lecteur », « modèle » ou « export ».
Les écarts 1 à 14 sont corrigés (voir chaque section), ainsi que les écarts 16
et ceux du corpus 3.x listés plus bas.

## Tableau de conformité

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

**Corrigé** (lecteur/export) : `read4xx` apparie les deux moitiés
`<Spanner type="Ottava">` (nouveau `read/read4xx/ottava.rs`) et les range dans
`Score::spanners` ; `read302` le faisait déjà. L'export décale la hauteur écrite
de `ppitch - pitch` (une ou deux octaves) comme `pitch2xml`, et écrit
`<direction><direction-type><octave-shift>` au début et à la fin, y compris
pour une ottava invisible (le trait disparaît, la hauteur reste décalée).

Fichiers : copypaste_parts, partExclusion, testExcludeInvisibleElements.

## 6. `bracket` des n-olets — 1 fichier, 9 occurrences

**Corrigé** (lecteur) : les lecteurs calculent le crochet faute de ligatures
modélisées, d'après `Tuplet::calcHasBracket` et le corpus :

- 3.x : un n-olet simple n'a pas de crochet, un n-olet qui contient un n-olet
  imbriqué en a un, et le niveau interne le plus court s'en passe ;
- 4.x : crochet par défaut, sauf pour un n-olet imbriqué de base plus courte
  qu'une croche, ou de croches avec un ratio réduit.

Fichier : voiceswitching (conforme). Même règle appliquée à
`testTuplets8`, `copypaste_tuplet_01` et `layout_elements` (v3, conformes).

## 7. Nombres affichés des n-olets imbriqués réduits — 1 fichier, 3 occurrences

**Corrigé** (modèle/lecteurs/export) : `Tuplet` porte
`actual_notes`/`normal_notes` (nombres bruts du fichier) en plus du `ratio`
réduit ; `<tuplet-actual>`/`<tuplet-normal>` les écrivent tels quels
(`Tuplet::displayed_numbers`). Un `4:4` reste `4`/`4`.

Fichier : voiceswitching (conforme).

## 8. Nombre de reprises d'une barre de fin — 1 fichier

**Corrigé** (lecteur/modèle/export) : `Measure::repeat_count` reçoit
`<endRepeat>N</endRepeat>` dans les deux lecteurs et `<repeat>` écrit
`times="N"` au-delà de deux lectures. (Déjà corrigé avec les reprises, voir
`export/repeats.rs`.)

Fichier : repeat65 (conforme).

## 9. Silences multi-mesures (`<measure-style>`) — 1 fichier

**Corrigé** (lecteur/modèle/export) : `Measure::multi_measure_rest` reçoit le
compteur du marqueur `<multiMeasureRest>N</multiMeasureRest>` sur la mesure qui
ouvre le groupe (les marqueurs suivent leur première mesure dans le `.mscx`).
Quand le style `<createMultiMeasureRests>` est actif et que toute la partition
est faite de mesures vides, une mesure vide isolée reçoit `1` (mmrest.mscx).
L'export écrit `<attributes><measure-style><multiple-rest>N`.

Fichier : mmrest (conforme).

## 10. `<part-name-display>` / `<part-abbreviation-display>` — 1 fichier

**Corrigé** (export) : `writeDisplayName` décompose les noms contenant `♭`/`♯`
en `<display-text>` / `<accidental-text>`, pour le nom long et l'abréviation.
Le `<part-name>` reste en texte plat (`Bb Clarinet`).

Fichier : part-textlines (conforme).

## 11. Réglages de canal MIDI (volume) — 1 fichier

**Corrigé** (lecteur/modèle/export) : `Instrument::volume`/`pan` reprennent les
contrôleurs 7 et 10 de `<Channel>` ; l'export calcule
`volume/127*100` (`79.5276` pour 101) et
`int((pan-63.5)/63.5*90)`, avec 100/64 par défaut.

Fichiers : hairpin_dynamic_inside, hairpin_dynamic_jump, hairpin_simple
(conformes).

## 12. `<sound tempo>` d'un tempo invisible — 1 fichier

**Corrigé** (lecteur/modèle/export) : `Tempo::visible` est lu et un tempo
invisible n'exporte que son `<sound tempo>` autonome, hors `<direction>`.

Fichier : testExcludeInvisibleElements (conforme).

## 13. Liaisons : `<measures>` de `<location>` ignoré — 1 fichier

**Corrigé** (lecteur) : `register_tie` conserve la `<location>` complète
(`<measures>`, `<fractions>`, `<staves>`, `<voices>`) et l'appariement calcule
la position absolue comme `Location::toAbsolute` : position relative dans la
mesure cible, décalée par `<measures>`.

Fichiers : testExcludeInvisibleElements, dynamics_niente (conformes).

## 14. Indication de mesure locale par portée — 1 fichier

**Corrigé** (lecteur/modèle/export) : voir l'écart 4 (`<duration>` explicite et
`Staff::time_stretch`). testLocalTimesig1 est conforme.

## 15. Paroles au-dessus de la portée et crédits sans mise en page — corrigé

**Corrigé** (modèle, lecteurs 3.x et 4.x, export) :

- `Lyrics::placement` par défaut vaut `below`, comme `lyricsPlacement` dans
  MuseScore (`engraving/dom/lyrics.cpp`, `propertyDefault(Pid::PLACEMENT)`).
  Les deux lecteurs lisent `<Style><lyricsPlacement>` et ne prennent
  `<placement>` d'une syllabe que s'il est présent ; l'export n'écrit
  `placement="above"` que pour un placement explicitement supérieur.
  Avant, tout `<Lyrics>` sans `<placement>` était lu comme « au-dessus ».
- `StyleSettings` (nouveau `model/style.rs`) conserve le format de page, les
  marges, les tailles de police des crédits et le nombre de pages. Les lecteurs
  lisent `<Style>` (`Spatium`, `pageWidth`/`pageHeight`, `page*Margin`,
  `pageTwosided`, `lyricsPlacement`, `*FontSize`) ; les valeurs absentes
  prennent les défauts de MuseScore pour la version du fichier (marges 10/20 mm
  en 3.x, 15 mm en 4.x, titre 24 puis 22 pt).
- L'export écrit `<defaults>` (`<scaling>`, `<page-layout>` et ses
  `<page-margins>`) et positionne les `<credit>` comme `textAsCreditWords` :
  `default-x` d'après l'alignement et les marges, `default-y` d'après la
  hauteur de page, la marge haute, le décalage du sous-titre (10 mm) et la
  hauteur du `VBox` pour les alignements bas. Le copyright est répété sur
  chaque page déduite des `<LayoutBreak>` de type `page`.

Tests : `crates/mscz2xml/tests/credits_corpus.rs` compare crédits, format de
page et paroles des corpus v3 et v4.

Écarts restants, listés dans ce test :

- `<defaults>` n'écrit pas encore `<appearance>`, `<music-font>`,
  `<word-font>` ni `<lyric-font>` : ces blocs demandent une trentaine de
  valeurs de style (largeurs de traits, polices) absentes du modèle.
- `moonlight.mscx` : les entités nommées d'un texte de crédit
  (`&quot;`) sont ré-échappées par l'écrivain XML (`&amp;quot;`).
- `testBreaksPage.mscx` : un `VBox` placé après deux systèmes n'est pas en
  haut de page ; sans moteur de mise en page, son `default-y` calculé est
  celui d'un cadre en haut de page (1626,67 au lieu de 1446,67, soit 18 sp).
- Les nombres du format de page sont rendus à six chiffres significatifs comme
  MuseScore ; les marges du corpus v3 diffèrent parfois de 0,0001 dixième
  (`56.6893` au lieu de `56.6894`) par arrondi de la valeur de style.

## 16. Numérotation après un saut de section — 2 fichiers (v3 et v4)

**Corrigé** (modèle, lecteurs 3.x et 4.x, export) : `Measure::section_break_before`
reçoit un `<LayoutBreak><subtype>section</subtype></LayoutBreak>` rencontré dans
la mesure ou dans un cadre (`HBox`/`VBox`). L'export recommence alors les numéros
à `1` et réécrit les `<attributes>` de début de section (`<divisions>` compris),
comme `MeasureNumberStateHandler`.

Fichiers : repeat66 (v4), keysig_78216 (v3).

## Écarts du corpus v3 relevés par `e2e_corpus.rs`

Le test de bout en bout couvre `v3/` avec la même normalisation. Les fichiers
conformes constituent `EXPECTED_CONFORMANT_V3` (Beam-CrossM1, barlinedelete,
clef-1, copypaste_tuplet_01, grace, keysig_78216, lyricsline01, measure-4,
repeat01, repeat06, repeat07, repeat11, repeat20, repeat39, selectionfilter1,
testBreaksPage, testChordSymbols, testHairpinDynamics, testHarmony6,
testTboxWords1, testTuplets8, testWedge4, timesig-05, undoResequenceMeasure,
voices).

Les points 1 à 5 et 9 à 12 sont corrigés ; restent les points 6 à 8 et 13.

1. **`<instrument-sound>` et `<midi-program>` absents — 13 fichiers.**
   **Corrigé** (lecteur/modèle) : `read/instruments.rs` porte la table des
   instruments de MuseScore (`share/instruments/instruments.xml`, généré par
   `scripts/gen-instruments-table.py`). Pour un fichier antérieur à 3.6,
   `updateInstrumentId` est reproduit : reconnaissance par le nom de piste puis
   les noms long/court (poids 4/2/1), à défaut par le programme MIDI, sinon
   `keyboard.piano` ; le programme par défaut vient du même instrument.
   `<midi-channel>` est lu quand le fichier le fixe.
2. **Changements d'instrument — 3 fichiers.**
   **Corrigé côté modèle/lecteur/export** : `Part` porte les instruments
   successifs, `<InstrumentChange>` enregistre le sien au tick du segment
   (`Segment::add`), et l'export écrit un `<score-instrument>`/
   `<midi-instrument>` par instrument, `<instruments>` à la première mesure,
   `<instrument>` par note, les canaux MIDI dans l'ordre (canal « harmony »
   compris) et la hauteur écrite avec la transposition en vigueur.
   **Reste** `<sound><instrument-change>` et les `<attributes><transpose>` du
   changement, portés par `writeInstrumentChange` (famille textes) : `change`,
   `layout_elements`, `testMultiInstrumentPart3`.
3. **Harmonies 3.x — 4 fichiers.** **Corrigé** (modèle/lecteur/export) :
   `Harmony` garde le suffixe (`<name>`), le type (`normal`/`roman`/`nashville`),
   la fonction et les `<degree>`. `export/harmony.rs` analyse le suffixe comme
   `ParsedChord::parse` (qualités et extensions courantes), écrit
   `<root>`/`<kind>` (+ texte, symboles, parenthèses, degrés),
   `<function>`/`<numeral>` des chiffrages romain et Nashville, la basse et
   `<offset>`. Un accord sans note est rattaché au segment précédent avec son
   décalage, comme `harmonies()`.
4. **Clef de la seconde portée — 2 fichiers.** **Corrigé** (lecteur) :
   `readVoice` place la première clef d'une voix au début de mesure dans un
   segment `HeaderClef` même si une autre portée a déjà posé la métrique.
   `clef-1` et `moonlight` sont conformes pour ce point.
5. **Numérotation après saut de section — 1 fichier.** **Corrigé**, voir
   l'écart 16.
6. **Orientation des liaisons — 1 fichier, 7 occurrences.** **Ouvert** :
   `moonlight` écrit `<tied orientation="over|under">` d'après le `TieSegment`
   calculé par la mise en page ; le modèle n'a pas cette direction (elle ne
   figure pas dans le `.mscx`). L'orientation est donc absente de la sortie.
7. **Détails de portée et de remplissage — 1 fichier.** **Corrigé en partie** :
   `<staffMove>` place les notes croisées sur l'autre portée (`<staff>` juste)
   et `<role>` utilisateur d'une altération produit `cautionary="yes"
   parentheses="no"` (ou `editorial` avec des crochets). **Reste** les
   `<forward>` scindés en fin de mesure (`36` au lieu de `12`+`24`), qui
   viennent du découpage des segments par la mise en page.
8. **`<measure-repeat>` — 1 fichier.** **Ouvert** : `layout_elements` recopie
   les mesures répétées au lieu d'écrire
   `<measure-style><measure-repeat>`.
9. **`<key number="1">` — 1 fichier.** **Corrigé** (export) : une armure n'est
   commune (sans `number`) que si toutes les portées de la partie en ont une,
   identique (`keysigTimesig`).
10. **`<part-group>` — 1 fichier.** **Corrigé** (modèle/lecteurs/export) :
    `<bracket>` et `<barLineSpan>` sont lus ; `part-group` ouvre et ferme les
    groupes comme `partGroupStart`, en filtrant l'accolade implicite d'une
    partie à plusieurs portées.
11. **Altérations d'avertissement — `moonlight`.** **Corrigé**, voir point 7.
12. **Notes croisées de portée — `moonlight`.** **Corrigé**, voir point 7.
13. **`<staff-details/>` des portées masquées — `barline01`.** **Ouvert** : les
    mesures 5 et 7 cachent une portée vide (`hideEmptyStaves`), que MuseScore
    signale par `<staff-details print-object="no"/>`. Le modèle ne lit pas
    `<hideEmptyStaves>` et la décision dépend de la mise en page (distance
    nulle entre portées).

## Tests dédiés

- `crates/mscz2xml/tests/instruments_corpus.rs` compare, pour chaque fichier
  des corpus v3 et v4, les `<score-instrument>`/`<midi-instrument>`,
  `<part-group>`, numéros de mesure, `<attributes>` (`instruments`, `key`,
  `clef`, `transpose`, `staff-details`), `<harmony>`, `<instrument>` et
  `<staff>` des notes, et les altérations. Les écarts restants y sont listés
  (`transpose` du changement d'instrument, `staff-details`) et filtrés.
- `crates/mscz2xml/src/export/tests.rs` couvre sur des partitions construites à
  la main les parties multi-instruments, la numérotation après saut de section
  et les altérations d'avertissement.
