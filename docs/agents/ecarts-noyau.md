# Écarts du noyau musical sur le corpus v4

Ce document inventorie les écarts constatés par le test de bout en bout
`crates/scoretk/tests/e2e_corpus.rs` entre la conversion `X.mscx` →
MusicXML et la référence `X.ref.musicxml` exportée par MuseScore 4.7.4.

Méthode : les deux documents sont normalisés (métadonnées, mise en page,
`<direction>`, `<stem>`, `<beam>`, `<print>`, `<notations>` hors `tied`/`tuplet`,
`<barline>` hors `<repeat>` retirés), puis comparés ligne à ligne avec `similar`.
Le test `crates/scoretk/tests/noyau_corpus.rs` compare en plus, fichier par
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

Tests : `crates/scoretk/tests/credits_corpus.rs` compare crédits, format de
page et paroles des corpus v3 et v4.

Écarts restants, listés dans ce test :

- `<defaults>` n'écrit pas encore `<appearance>`, `<music-font>`,
  `<word-font>` ni `<lyric-font>` : ces blocs demandent une trentaine de
  valeurs de style (largeurs de traits, polices) absentes du modèle.
- `moonlight.mscx` : **corrigé** (famille textes) — les fragments de texte
  sont écrits bruts (`XmlWriter::tag_raw`), les entités nommées (`&quot;`)
  sont donc conservées comme dans MuseScore.
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
change, clef-1, copypaste_tuplet_01, grace, keysig_78216, lyricsline01,
measure-4, repeat01, repeat06, repeat07, repeat11, repeat20, repeat39,
selectionfilter1, testBreaksPage, testChordSymbols, testHairpinDynamics,
testHarmony6, testMultiInstrumentPart3, testTboxWords1, testTuplets8,
testWedge4, timesig-05, undoResequenceMeasure, voices).

Les points 1 à 5 et 9 à 12 sont corrigés ; restent les points 6 à 8 et 13.

1. **`<instrument-sound>` et `<midi-program>` absents — 13 fichiers.**
   **Corrigé** (lecteur/modèle) : `read/instruments.rs` porte la table des
   instruments de MuseScore (`share/instruments/instruments.xml`, généré par
   `scripts/gen-instruments-table.py`). Pour un fichier antérieur à 3.6,
   `updateInstrumentId` est reproduit : reconnaissance par le nom de piste puis
   les noms long/court (poids 4/2/1), à défaut par le programme MIDI, sinon
   `keyboard.piano` ; le programme par défaut vient du même instrument.
   `<midi-channel>` est lu quand le fichier le fixe.
2. **Changements d'instrument — 3 fichiers.** **Corrigé** (modèle, lecteurs,
   export, avec la famille textes) : `Part` porte les instruments successifs,
   `<InstrumentChange>` enregistre le sien au tick du segment (`Segment::add`),
   et l'export écrit un `<score-instrument>`/`<midi-instrument>` par instrument,
   `<instruments>` à la première mesure, `<instrument>` par note, les canaux
   MIDI dans l'ordre (canal « harmony » compris) et la hauteur écrite avec la
   transposition en vigueur. `writeInstrumentChange` (famille textes) écrit
   `<print><part-name-display>`, `<transpose>` et `<sound><instrument-change>`.
   `change` et `testMultiInstrumentPart3` sont conformes.
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
   `<forward>`/`<backup>` scindés en fin de mesure (`36` au lieu de `12`+`24`),
   qui viennent du découpage des segments par la mise en page.
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

Écarts de `layout_elements` restants, hors familles instruments et accords
(voir écarts 8 et 18) : `<figured-bass>`, `<notehead parentheses="yes">` et
`<measure-repeat>`.

## Tests dédiés

- `crates/scoretk/tests/instruments_corpus.rs` compare, pour chaque fichier
  des corpus v3 et v4, les `<score-instrument>`/`<midi-instrument>`,
  `<part-group>`, numéros de mesure, `<attributes>` (`instruments`, `key`,
  `clef`, `transpose`, `staff-details`), `<harmony>`, `<instrument>` et
  `<staff>` des notes, et les altérations. Les écarts restants y sont listés
  (`transpose` du changement d'instrument, `staff-details`) et filtrés.
- `crates/scoretk-musicxml-export/src/tests.rs` couvre sur des partitions construites à
  la main les parties multi-instruments, la numérotation après saut de section
  et les altérations d'avertissement.
- `crates/scoretk/tests/texts_corpus.rs` et
  `crates/scoretk/tests/notations_corpus.rs` couvrent les familles textes et
  notations (voir écarts 17 et 18).

## 17. Textes de ligne, cadres `TBox`, changements d'instrument, swing — famille textes

**Corrigé** (lecteur/modèle/export) et couvert par
`crates/scoretk/tests/texts_corpus.rs` sur le corpus **v3 et v4** (l'e2e v4
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

## 18. Notations : écarts résiduels d'une famille désormais couverte

La famille des notations (articulations, points d'orgue, liaisons d'expression,
ornements, trilles, trémolos, techniques, arpèges, glissandos, respirations et
lignes d'accord) est couverte par un test dédié,
`crates/scoretk/tests/notations_corpus.rs`, qui compare les `<notations>` de
ces éléments à la référence, mesure par mesure et dans l'ordre. Les tests de
bout en bout ci-dessus continuent de retirer `<notations>` sauf `tied`/`tuplet` :
les écarts ci-dessous n'apparaissent donc pas dans leurs tableaux.

Corrigé dans cette passe :

- **lecteurs 3.x et 4.x** : articulations (y compris l'éclatement des
  articulations composées en `staccato`/`tenuto`/`accent`…), points d'orgue,
  ornements et leurs altérations, trémolos simples et à deux notes, arpèges,
  lignes d'accord, doigtés, respirations/césures, et les spanners `Slur`,
  `Trill`, `Glissando` avec leurs propriétés (sens, texte, style de trait,
  visibilité) ;
- **export** : `<articulations>`, `<fermata>`, `<slur>` avec numéro et
  `orientation`/`placement`, `<ornaments>` (`trill-mark`, `turn`, `mordent`,
  `wavy-line`, `tremolo`, `accidental-mark`), `<technical>` (`up-bow`,
  `down-bow`, `harmonic`, `open`, `stopped`, `snap-pizzicato`, `fingering`,
  `pluck`), `<arpeggiate>`/`<non-arpeggiate>`, `<glissando>`/`<slide>`,
  `<breath-mark>`/`<caesura>` et `falloff`/`doit`/`plop`/`scoop` ;
- **respirations** : les deux lecteurs suivent les noms de `SymNames`
  (`symnames.cpp:798-822`) — `breathMarkTick`, `breathMarkUpbow`,
  `breathMarkSalzedo`, `caesura*`, `chantCaesura` — et non les noms tronqués
  `breathTick`/`stringsUpBow` ; `<subtype>` numérique reste lu comme forme
  obsolète (2 = césure courbe, 3 = césure). `<placement>` l'emporte sur le
  placement déduit de la voix (0 au-dessus, 1 au-dessous), que le C++ pose
  avant de lire la balise. L'écriture `.mscx` et l'export MusicXML suivent les
  mêmes noms, `breathMarkSalzedo` compris ;
- `print-object="no"` sur un `<notations>` dont l'élément est invisible,
  réouverture d'un `<notations>` quand la visibilité change.

Corrigé lors de la passe de parité complète (`ECARTS_CONNUS` est désormais
vide, le test exige une parité exacte) :

- **v3/layout_elements** : la reprise de mesure est dépliée à la lecture
  (`read/measure_repeat.rs` : marque `MeasureRepeat`, copie des accords,
  annotations de segments d'accords et spanners de la mesure source) ;
  `<measure-repeat>` est écrit par `measureRepeat` à l'export. Les
  `TremoloBar` (bend de guitare) sont lus dans les deux lecteurs et écrits en
  `<technical><bend>` par `writeGuitarBend`. **Cause : lecteur/modèle/export.**
- **v4/copypaste_parts** : MuseScore 4.7 remplit son `TrillHash` avec
  `Trill::endElement()`, calculé une seule fois par `Score::addSpanner` quand
  `tick2 == tick` et jamais recalculé : le `wavy-line type="stop"` sort donc
  sur l'accord de départ, avec le `type="start"`. L'export reproduit ce
  rattachement (`write_trills`). **Cause : export.**
- **v4/testExcludeInvisibleElements** : `Note` porte la visibilité du départ et
  de l'arrivée de liaison (`<Tie><visible>`, lu par les deux lecteurs) ; le
  `<tied>` invisible ouvre donc un `<notations print-object="no">` et le
  regroupement des notations redevient celui de MuseScore. **Cause :
  lecteur/modèle/export.**

## 19. Parité complète : hampes, ligatures, barres, métadonnées

Le test de bout en bout a un second niveau, `Level::Full` (`corpus_v3_complet`,
`corpus_v4_complet`, `ecarts_complets_document`), qui ne retire que la mise en
page pure (attributs `default-x`/`default-y`/`relative-x`/`relative-y`/`width`,
points de contrôle `bezier-x`/`bezier-y`, blocs `<defaults>` et `<print>`,
`<system-layout>`, `<staff-layout>`, `<encoding-date>`) et produit
`docs/agents/ecarts-complets.md`.

Corrigé dans cette passe :

- **hampes** : toujours écrites (`stem.ts`), direction calculée comme
  `Chord::up()` (`export/stems.rs`) : direction explicite hors ligature,
  ligature, trémolo, plusieurs voix (parité de piste), ornement, accord croisé,
  puis hauteurs (`computeAutoStemDirection`, `Tpc` → `relStep` avec la clef en
  vigueur et la hauteur écrite des instruments transpositeurs). Résultat :
  aucun écart de hampe hors `v3/moonlight` (9 notes croisées dont la ligature
  impose une direction que le modèle ne restitue pas).
- **ligatures** : `export/beams.rs` reconstruit `BeamLayout::createBeams` et
  `Groups::baseBeamMode`/`actualBeamMode` (table `noteGroups`, subdivisions de
  temps, anacrouse, n-olets, reprise entre mesures), puis écrit
  `begin`/`continue`/`end`/`forward hook`/`backward hook` par niveau comme
  `writeBeam`, y compris les ornements (`beamGraceNotes`). `BeamMode` gagne
  `begin16`/`begin32` et `ChordRest::beam_direction` porte le
  `<Beam><StemDirection>` lu. Aucun écart de `<beam>` sur les deux corpus.
- **barres** : la fermata posée en fin de mesure est écrite dans
  `<barline location="right">` comme `barlineRight`, et non dans les notations
  du silence.
- **accords croisés** : `Chord::staff_move` (lecture des deux formats) décale
  `<staff>` et la direction de hampe ; `v3/moonlight` passe de 213 à 5 lignes
  d'écart `<staff>`.
- **métadonnées** : `<miscellaneous>` reprend `creationDate`, `mscVersion`,
  `platform` et `subtitle` (plateforme `Linux` et date d'encodage par défaut,
  comme `MasterScore`).
- **soufflets** : `spread` vient de `hairpinHeight` (12 en 3.x, 11.5 en 4.x).
- **silences invisibles** : `<note print-object="no">` pour un silence masqué.
- **n-olets** : `Tuplet::placement` reprend `<direction>` et écrit
  `placement="above|below"` sur `<tuplet type="start">` ; les fichiers
  `testTuplets8` et `voiceswitching` deviennent conformes.
- **ordre des directions** : les débuts de spanner suivent l'ordre des
  spanners (`spannerStart`), et non un ordre par famille : soufflet avant
  ligne de texte, pédale avant ottava (`part-textlines` conforme).
- **pédales** : nouveau `PedalInfo` (trait, textes, crochets, placement) lu
  par les deux lecteurs (nouveau `read/read4xx/pedals.rs`) et exporté comme
  `ExportMusicXml::pedal` (`start`/`stop`, `line`, `sign`, `<staff>`).
- **textes de soufflet** : la police (`beginFontFace`/`Size`/`Style`, défaut
  FreeSerif 12 italique) est portée par `Hairpin` et écrite sur les `<words>`.
- **repères de répétition** : `justify="center"` (style `rehearsalMarkAlign`).
- **armure et mesure invisibles** : `KeySig::visible` / `TimeSig::visible`
  écrivent `print-object="no"`.
- **têtes entre parenthèses** : `Note::parentheses` (symboles
  `noteheadParenthesisLeft/Right`) écrit `parentheses="yes"`.
- **soufflets invisibles** : `Spanner::visible` est lu dans `<HairPin>` et un
  soufflet invisible n'est pas exporté (`canWrite`).

Corrigé dans cette passe :

- **chiffrages figurés** : `model/figured_bass.rs` (items, préfixe/suffixe,
  trait de prolongation, durée), lus par les deux lecteurs
  (`read4xx/figures.rs`, `read302/annotations.rs`) et exportés comme
  `ExportMusicXml::figuredBass` (`export/figured_bass.rs` : `<figure>`,
  `<figure-number>`, `<extend>`, `<duration>`, prolongation sur les accords
  suivants).
- **barres de courtoisie** : `Measure::line_break` reprend les
  `<LayoutBreak>` `line`/`page` des deux lecteurs et `end_bar_line_type`
  écrit `light-light` avant un changement d'armure ou de métrique en début de
  système, comme le `BarLine` `DOUBLE` que la mise en page de MuseScore ajoute
  alors. Sans saut explicite dans le fichier (système coupé par la largeur de
  page), la règle ne peut pas s'appliquer : c'est le seul cas restant, dans
  `v3/voices`.
- **`<measure-repeat>`** : voir écart 18.
- **`TBox` « below »** : un texte de cadre sous une mesure couverte par un
  groupe de silences multi-mesures est rattaché à la mesure qui remplace le
  groupe (`mmrest_group_start`), comme `coveringMMRestOrThis`.
- **regroupement des `<notations>` autour d'un `tied` invisible** : voir
  écart 18.

Corrigé dans la passe de parité `t31` :

- **silences « gap »** : `read/gap_rests.rs` porte `Measure::checkMeasure` et
  `Measure::fillGap` (`check.cpp`), ainsi que la découpe rythmique de
  `toRhythmicDurationList` (`durationtype.cpp`, `sig.cpp`). Les trous d'une
  voix deviennent des silences invisibles dont la position fait avancer le
  curseur d'export : c'est l'origine des `<forward>`/`<backup>` scindés de
  MuseScore (`moonlight`).
- **orientation des liaisons de prolongation** : le `.mscx` porte la direction
  explicite dans `<Tie><up>up|down</up>` (`Pid::SLUR_DIRECTION`) ; les deux
  lecteurs la conservent (`Note::tie_up`) et l'export écrit
  `orientation`/`placement` comme `slurTieLineStyle` (`moonlight`).
- **portées masquées** : `Measure::line_break`/`section_break_before` donnent
  les systèmes explicites ; `export/systems.rs` applique
  `SystemLayout::hideEmptyStaves` et l'export écrit
  `<staff-details print-object="no"/>` à la première mesure du système
  (`barline01`).
- **durée totale des n-olets** : `TRead::read(Tuplet*)` calcule
  `baseLen * normalNotes` avec le ratio **non réduit** ; les deux lecteurs
  utilisaient le dénominateur réduit (un `4:4` durait une figure au lieu de
  quatre).

Écarts complets restants (détail et fréquence dans `ecarts-complets.md`) :
direction de hampe d'une ligature croisée (9 notes de `moonlight`, calculée
par `BeamLayout::layout1` à partir du groupement et de la géométrie des
ligatures) et barres de courtoisie des systèmes coupés automatiquement par la
largeur de page (2 barres de `v3/voices`, sans `<LayoutBreak>` dans le
fichier).

## 20. MusicXML export — schema validity and measured fidelity (English)

### Frozen baseline (before the fixes of this campaign)

- Frozen binary: `/tmp/scoretk-base`, built from commit `5ef2686`, sha256 starting
  `4084ce2dc32f6377`. The fixed binary is built from the same worktree after the
  the two fixes below; the frozen one stays available for A/B measurement.
- Lot: the 174 `.mscz` of `~/Musique`, manifest built by
  `scripts/oracle/musicxml/lots.sh` (same list as
  `scripts/oracle/lilypond/lots-ecriture.sh`). No corpus file or third-party
  script is versioned.
- Oracle: `scripts/oracle/musicxml/mesure.sh` (wraps `mesure.py` with
  `python3Packages.mido` and `libxml2`). For each score: scoretk → MusicXML,
  MuseScore 4.7.4 → MusicXML (fidelity reference), both → MIDI (what each tool
  plays). Every binary runs under `timeout` and `prlimit --as=2000000000`.
  Core comparison reuses `core_norm` through
  `crates/scoretk/tests/oracle_core.rs`; N1/N2 reuse `midi_notes.py` at the
  Q16 grid (±1/16 quarter). music21 is not installed in this environment, so
  the only alternative measured is MuseScore 4.7.4.
- XSD: MusicXML 4.0 (`specs/musicxml/musicxml-4.0.xsd`), its `xml.xsd` and
  `xlink.xsd` imports resolved locally through a catalog. Errors are counted by
  message. XSD text: `xsd:<type>`; narrative text: `ref:<element>` in
  `docs/agents/format-ref/musicxml.md`.

| Metric | Baseline (frozen) |
| --- | ---: |
| Scores | 174 |
| Converted | 152 |
| XSD-invalid scoretk outputs | 2 |
| XSD errors, scoretk / MuseScore 4.7.4 | 3 / 2 |
| Core-identical to MuseScore | 24 / 152 |
| N1 pitch multiset | 0.9791 |
| N2 pitch + attack | 0.9739 |

### XSD errors by cause

1. `<measure-style number="0">` — 2 occurrences, scoretk only
   (`Waltz_No._2.mscz`). `xsd:measure-style` declares `number` as
   `xsd:staff-number`, a restriction of `xs:positiveInteger`
   (`musicxml-4.0.xsd:780-785`); the spec says the number selects the staff from
   the top, 1 being the first (`ref:measure-style`, format-ref § "Clef, time,
   staves, parts"). Cause: export. `write_measure_repeat` used `Some(0)` as a
   sentinel for the closing `<measure-repeat type="stop">`. Port of
   `measureRepeat()` (`exportmusicxml.cpp:6534-6571`), which builds the
   `number` attribute once per staff in the loop and reuses it for both the
   start and the stop. Fixed by passing the 0-based staff position and adding
   one inside `write_measure_repeat`. Reduced test:
   `repetition_de_mesure_stop_numero_de_porte_valide`.

2. `<metronome>` with only `<beat-unit>` — 1 occurrence, scoretk **and** 1 in
   MuseScore 4.7.4 (`Magnificat__Prt.mscz`). `xsd:metronome`
   (`musicxml-4.0.xsd:3795-3825`) is a choice whose first branch is a sequence
   `<beat-unit>` then either `<per-minute>` or another `<beat-unit>`; the spec
   describes the same regular/metric-modulation forms (`ref:metronome`,
   format-ref § "Repeats, jumps, directions, lyrics"). Cause: upstream export,
   faithfully reproduced by scoretk. `findMetronome()`
   (`exportmusicxml.cpp:4809-4884`) does not test `posEq == npos`: when the text
   is `♩ ≈ 90` (no `=`), it returns true with `metroRight` empty and
   `wordsMetronome()` writes the invalid element followed by `<words> ≈ 90</words>`.
   Fixed by returning `None` when neither `per_minute` nor `second_beat` was
   found, so the full text becomes `<words>` (valid and keeps the `≈`).
   Deliberate divergence from MuseScore 4.7.4, which is invalid here: this is a
   validity fix, not a fidelity regression. Only formatting changes on that
   file (the symbol moves into a `<words>` and the bold style is no longer
   attached to the number); pitches, durations and the `<sound tempo="90"/>`
   are unchanged. Reduced test: `tempo_sans_equation_reste_en_words`.

3. `<accidental-mark cautionary="no">` — 1 occurrence, MuseScore 4.7.4 only.
   `xsd:accidental-mark` has no `cautionary` attribute; scoretk already emits a
   valid element. No change needed.

### Out of scope: MuseScore 2.x sources

22 of the 174 files cannot be read by scoretk and are therefore not converted:
20 at version `2.06`, one at `1.14` (`Canon_in_D_Pachelbel_-_arranged_for_accordion.mscz`)
and one reported as `2` (`Autumn_Leaves_Frank_Marocco.mscz`). They are all under
`Partitions/Accordeon`, `Partitions/Chorales` and
`Partitions/Piano/Vingt_danses_enfantines_pour_piano.mscz`; the exact absolute
paths are in the `erreur` field of `/tmp/oracle-xml-full/mesure.jsonl` (raw
measurement, not versioned). The reader supports 3.x and 4.x/5.x only: adding
2.x support is a separate task and is out of scope here.

### Final measurement against the frozen baseline

Fixed binary `/tmp/scoretk-fix`, sha256 starting `b95222eff66c7169`; same 174-file
manifest and oracle as above.

| Metric | Frozen baseline | After fixes | Δ |
| --- | ---: | ---: | ---: |
| Converted | 152 | 152 | 0 |
| XSD-invalid scoretk outputs | 2 | **0** | −2 |
| XSD errors, scoretk | 3 | **0** | −3 |
| XSD errors, MuseScore 4.7.4 (unchanged reference) | 2 | 2 | 0 |
| Core-identical to MuseScore | 24 | **25** | +1 |
| N1 pitch multiset | 0.9791 | 0.9791 | 0 |
| N2 pitch + attack | 0.9739 | 0.9739 | 0 |

No per-file loss: across all 174 rows, `ok`, `ref`, `n1`, `n2` and `n2d` are
identical to the frozen baseline. The only core change is
`Waltz_No._2.mscz` (4 → 0 differing lines); `Magnificat__Prt.mscz` keeps its 39
core lines because `<direction>` is removed by `core_norm`, but its XSD error is
gone. The `directions` family gains 5 changed lines (the `♩ ≈ 90` text is now
two `<words>` instead of one `<metronome>` plus one `<words>`), which is the
documented validity fix and not a pitch, duration or tempo change (`<sound
tempo="90"/>` is preserved). `EXPECTED_CONFORMANT` in `e2e_corpus.rs` is
unchanged.

### Regression guard (oracle diff)

`compare.sh` reports DIFFs on this worktree, but none is caused by the two
fixes. After normalising the encoding date (the reference outputs were frozen
on 2026-11-18, this run was made on 2026-11-20) **every** `.musicxml`, `.mxl`
and `.mscz` output is byte-identical to the reference. The remaining differences
are 28 `.abc`, 14 `.midi` and 14 `.ly` outputs, i.e. formats untouched by this
campaign: they come from the pre-existing branch commit `4d78e80` (ABC export)
against the older `master` reference. In particular `EXPECTED_CONFORMANT` in
`e2e_corpus.rs` is unchanged and `cargo test -p scoretk --test e2e_corpus` still
passes (5/5).

### Fidelity campaign (English)

#### Frozen baseline and grouping

The binary was frozen from `master` before any fix (`/tmp/scoretk-base3`,
sha256 `7944c0d7e7598157`); the 174 `.mscz` of `~/Musique` were measured with
`scripts/oracle/musicxml/mesure.sh`. With the new 1.15–2.07 reader, 173 files
convert (the 22 files of the previous campaign are no longer failures).

| Metric | Frozen baseline | Final | Δ |
| --- | ---: | ---: | ---: |
| Converted | 173 | 173 | 0 |
| Core-identical to MuseScore 4.7.4 | 27 | **39** | +12 |
| N1 pitch multiset | 0.9779 | 0.9778 | −0.0001 |
| N2 pitch + attack | 0.9086 | 0.9086 | 0 |
| XSD errors, scoretk / MuseScore | 0 / 2 | 0 / 2 | 0 |

`groupes.py` groups the 146 non-conformant baseline files by **first differing
core family** (the first changed canonical line, in document order, extracted
by `oracle_core.rs`). Counts and reduced examples:

| First differing family | Files | Reduced example (ref → scoretk) |
| --- | ---: | --- |
| instruments / transposition | 77 | `<midi-channel>3</midi-channel>` → `2` |
| barlines / repeats (measure number) | 18 | `<measure implicit="yes" number="0">` → `number="1"` |
| structure / part-list | 16 | `<group-symbol>square</group-symbol>` → `line` |
| notes / pitch | 7 | `<accidental>sharp</accidental>` → absent |
| tuplets | 6 | `<tuplet bracket="no">` → `bracket="yes"` |
| voices / staves | 5 | `<staff-details number="1">` → absent |
| lyrics | 5 | absent → `<extend/>` |
| metadata (layout artefact) | 4 | `<movement-number>8</movement-number>` → `<measure number="1">` |
| ties | 3 | `<tie type="start"/>` duplicated vs missing half |
| harmony / chords | 2 | `<harmony print-frame="no">` → same tag, different children |
| directions | 2 | absent → `<sound tempo="160"/>` |
| duration / rhythm | 1 | `<backup>` → `<forward>` |

#### Fixes, one commit per cause

1. **MIDI port, 52 files** — `Instrument::midi_port` read from
   `<Channel><midiPort>` by `read302` (therefore `read206`) and `read4xx`, then
   written as `<midi-device port>` by `score-instruments`. Port of
   `ExportMusicXml::partList` (`exportmusicxml.cpp:7939-7947`,
   `midiMapping(channel)->port() + 1`). Test `port_midi_de_l_instrument`.
2. **Pickup measure numbering, 18 files** — `Measure::exclude_from_numbering`
   and a port of `MeasureNumberStateHandler::updateForMeasure`
   (`exportmusicxml.cpp:8272-8299`). Test `anacrouse_implicite_et_numeration`.
   A follow-up commit restricts the flag to `<irregular>1</irregular>`: a bare
   `@len` makes a measure irregular but **does not** remove its number
   (`Buenos_dias` vs `L'Aigle_mexicain`). Test `irregular_et_len_sont_distincts`.
3. **Part-group symbol, 16 files** — `bracket_symbol` mapped `BracketType`
   (`src/engraving/types/types.h:1289`: `NORMAL`, `BRACE`, `SQUARE`, `LINE`,
   `GROUP`); types 2 and 3 were swapped. `partGroupStart`
   (`exportmusicxml.cpp:4742-4770`) confirms the names. Test
   `symbole_de_groupe_carre_et_ligne`.
4. **Legacy accidental names, 7 files** — 2.x `<subtype>sharp|natural|flat`
   was not recognised by `TConv::fromXml` (`read302/tables.rs`); the written
   `<accidental>` was missing. Test `alteration_explicite_2x`.
5. **Null melisma, 5 files** — `write_lyric` emitted `<extend/>` for
   `Some(Fraction::ZERO)`, read from `<ticks_f>0/16</ticks_f>`. Port of
   `ExportMusicXml::lyrics` (`exportmusicxml.cpp:6130`, `ticks().isNotZero()`).
   Test `melisma_nul_n_ecrit_pas_extend`.
6. **2.x ties, 3 files** — the note ordinal used to pair `<Tie id>` with
   `<endSpanner id>` was reset at each chord, so all first notes of chords got
   rank 0 and `fix_ties` discarded intra-measure ties (`from == to`). The
   counter is now per voice over the whole measure. Test
   `liaison_2x_entre_deux_accords_separeres`.

#### Causes deliberately left unchanged (layout artefacts, not music)

- **Tuplet brackets, 6 files** — `bracket` is computed by
  `Tuplet::calcHasBracket` (`src/engraving/dom/tuplet.cpp:204-274`) from beams,
  head side and staff crossing, none of which is in the pivot model. The
  heuristic reproduces the versioned corpus but cannot match both directions
  observed on real files (4 files want `no`, 2 want `yes`). Changing it would
  break `voiceswitching`; no export change.
- **`<staff-details>`, 5 files** — the reference writes `<staff-size scaling=
  "70">70</staff-size>` (small staff) or `print-object="no"` (hidden empty
  staff), both layout decisions absent from the model (`writeStaffDetails`,
  `exportmusicxml.rs:2256`). No export change.
- **`<movement-number>` / bezier points, 4 files** — `core_norm` keeps
  `<movement-number>` and the `bezier-x`/`bezier-y` attributes that
  `Level::Full` drops. These files differ only by metadata and layout geometry
  (slurs), not by notes, durations or structure.
- **`<sound tempo>` / `<sound segno>`, 2 files** — invisible tempo marks and a
  jump's `sound` attribute depend on export context (`tempoSound`,
  `exportmusicxml.cpp:5006-5016`); the first file has 3 extra `<sound tempo>`
  out of 59 invisible tempos, with identical pitches.
- **`<backup>` vs `<forward>`, 1 file** — an equivalent timeline encoding of a
  gap rest (`fillGap`); the resulting cursor and notes are unchanged.

#### Per-file changes and corpus

No file went from core-identical to non-identical. The 11 changed rows are all
2.x files: 6 N1 and 4 N2 variations come from the tie fix, which now merges
tied notes in scoretk's MIDI as MuseScore does (fewer note-ons, different
multiset, same sounding music); one core increase (`ZORBA_THE_GREEK`, 35 → 56)
comes from preserving explicit 2.x accidentals that MuseScore 4.7.4 suppresses
as redundant against the key, while pitches and durations are unchanged.
The versioned corpus stays at **54/56**; `barline01` was added to
`EXPECTED_CONFORMANT_V3` now that it is conformant. `moonlight` (18 lines) and
`voices` (6 lines) keep their documented layout differences.

### Final re-measurement (after rebase and concert pitch)

The binary was rebuilt from the rebased tree (2.x spanners, per-voice tie
ordinals, style-driven concert pitch) and measured against the same frozen
`/tmp/oracle-base3` baseline. The 2.x spanner port and the concert-pitch fix
are the only behavioural changes since the previous table.

| Metric | Frozen baseline | Final | Δ |
| --- | ---: | ---: | ---: |
| Converted | 173 | 173 | 0 |
| Core-identical to MuseScore 4.7.4 | 27 | **39** | +12 |
| N1 pitch multiset | 0.9779 | 0.9776 | −0.0003 |
| N2 pitch + attack | 0.9086 | **0.9257** | +0.0171 |
| XSD errors, scoretk / MuseScore | 0 / 2 | 0 / 2 | 0 |

#### N2 split by score version

| Group | Files | Reference notes | N1 | N2 |
| --- | ---: | ---: | ---: | ---: |
| 3.x / 4.x | 152 | 317 317 | 0.9791 | **0.9739** |
| 1.x / 2.x | 22 (21 converted) | 60 809 | 0.9700 | **0.6742** |

The 3.x/4.x figure is unchanged from the frozen baseline. The 2.x figure rose
from 0.5680 to 0.6742; it remains low for one reason, not a general reader
failure: `Vingt_danses_enfantines_pour_piano.mscz` alone carries 26 477 of the
60 809 reference notes and has N2 = 0.3106. The other converted 2.x scores are
mostly at 0.97–1.00 (`La_Partida` 0.779, `Czardas` 0.801, `ZORBA` 0.790 and
`Triumphal_March` 0.676 are the four remaining outliers).

Reduced cause for the piano file (raw MIDI in `/tmp/oracle-final4/travail/173`):
- repeat/jump expansion diverges. Matching notes show attack offsets of +6,
  +4.5, +13 and +3 quarters (4 241, 2 427, 1 756 and 1 716 notes); the score
  has 30 `<Volta>`, `<endRepeat>` and `<Jump>` markers. Both tools expand
  repeats, but MuseScore's `RepeatList` and scoretk's walker do not make the
  same passes, so most attacks in the second half fall outside the ±1/16
  tolerance and N2 collapses while N1 stays at 0.9896.
- ottava playback: the first pitch divergence is at tick 856, reference 88
  (E6) vs scoretk 76 (E5), i.e. one octave; 271 notes are affected. scoretk's
  MIDI writer does not apply `<Ottava>` to playback, unlike MuseScore.
- Disabling scoretk's repeat expansion makes N2 worse (0.0706), confirming
  that the reference does expand repeats.

Both effects are layout/playback-policy differences of the MIDI oracle, not
lost notes: N1 is 0.9896 and the MusicXML core diff on that file is 403 lines
of layout and ottava placement. No export change is made here: faithfully
reproducing MuseScore's 2.x jump passes would require porting `RepeatList`
beyond the versioned corpus, with a high regression risk.

#### Concert pitch

`Ad_te_Suspiramus` (Piccolo, `Sid::concertPitch` in `<Style>`) went from 797 to
56 differing core lines: `<concert-score/>` is now emitted, pitches are written
at concert pitch (D5 instead of D4) and the transposition becomes
`<for-part><part-transpose>`. N1 and N2 were already 1.0000 (MIDI uses sounding
pitch on both sides) and remain so. The wasm `concertPitch` option still maps
to `Some(value)` and overrides the style; the JS contract is unchanged.

#### Per-file changes

The 16 rows that changed are all 2.x files. N2 **increased** for 7 of them
(Czardas 1 272 → 1 734, La_foule 450 → 813, La_Partida 431 → 557, ZORBA 795 →
1 335, …) thanks to the 2.x spanner port; the 8 small N1 decreases and the
piano N2 drop are the tie-merge and repeat-expansion effects described above.
`ZORBA_THE_GREEK` gains 21 core lines because explicit 2.x accidentals are now
preserved even where MuseScore 4.7.4 suppresses them as redundant. No file
went from core-identical to non-identical, and the versioned corpus remains
54/56 with `EXPECTED_CONFORMANT` unchanged.
