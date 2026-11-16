# Format MSCX — noyau musical

Référence de lecture des fichiers `.mscx` MuseScore 3.x et 4.x/5.x, destinée
aux agents qui écrivent en Rust les lecteurs du projet `mscz2xml`.

Sources C++ de référence, en lecture seule :

- `ref-musescore/src/engraving/rw/read302`, `read400`, `read410`,
  `read460`, `read500` ;
- `ref-musescore/src/engraving/dom` (modèle) ;
- `ref-musescore/src/engraving/infrastructure` (conteneur `.mscz`).

Les chemins cités dans ce document sont relatifs à
`ref-musescore/src/engraving/`. Les numéros de ligne correspondent à la copie
de référence utilisée pour rédiger ce document ; en cas de doute, la fonction
nommée fait foi.

Ce document ne couvre pas l'export MusicXML lui-même. Il décrit uniquement ce
qu'il faut extraire du MSCX pour alimenter un modèle musical : structure,
portées, mesures, voix, accords/repos/notes, durées, hauteurs, armures,
métriques, clés, altérations, liaisons, paroles, ornements et déplacements.

## 1. Versions et aiguillage

La version du format est portée par l'attribut `version` de l'élément racine,
**pas** par `<programVersion>` :

```xml
<museScore version="3.02">
  <programVersion>3.6.2</programVersion>
```

Un fichier écrit par MuseScore 3.6.2 peut donc déclarer le format `3.02`.
`MscLoader::readMasterScore` (`rw/mscloader.cpp:219-231`) calcule :

```text
mscVersion = partie_majeure * 100 + partie_mineure
```

soit `"3.02" -> 302`, `"4.10" -> 410`, `"4.60" -> 460`, `"5.00" -> 500`.
`RWRegister::reader` (`rw/rwregister.cpp:39-58`) choisit ensuite :

| `mscVersion` | Lecteur | Période |
|---|---|---|
| `< 400` | `Read302` | 3.00 à 3.99 |
| `400..409` | `Read400` | 4.0.x |
| `410..459` | `Read410` | 4.1 à 4.5 |
| `460..499` | `Read460` | 4.6 à 4.9 |
| `>= 500` | `Read500` | 5.0 et suivants |

Conséquences pratiques :

- le lecteur 3.x (`Read302`) réutilise massivement le code de `read400` :
  `Read302::readScoreTag` (`rw/read302/read302.cpp:54`) appelle
  `read400::StaffRead::readStaff` et `read400::TRead` ;
- les seuils de compatibilité internes sont parfois différents du seuil de
  dispatch : 4.1, 4.5 et 5.0 introduisent des changements de balises ;
- ne jamais comparer les versions comme des chaînes (`"5.0"` contre
  `"5.00"`). Utiliser l'entier `mscVersion`.

## 2. Structure du fichier

### 2.1 Squelette

Un `.mscx` est un XML dont la racine est `<museScore>` et qui contient un
unique `<Score>` (plus, éventuellement, des `<Score>` imbriqués pour les
parties liées) :

```xml
<?xml version="1.0" encoding="UTF-8"?>
<museScore version="3.02">
  <programVersion>3.6.2</programVersion>
  <Score>
    <Division>480</Division>
    <Style>...</Style>
    <metaTag name="composer">...</metaTag>
    <Part>...</Part>
    <Staff id="1">
      <Measure>...</Measure>
    </Staff>
  </Score>
</museScore>
```

En 4.x/5.x, la racine et le `Score` sont identiques, mais :

- le style est normalement dans `score_style.mss` du conteneur `.mscz`
  (`MscLoader::loadMscz`, `rw/mscloader.cpp:105-118`) et non plus dans
  `<Style>` ; le lecteur accepte encore `<Style>` inline
  (`Read400::readScoreTag`, `rw/read400/read400.cpp:170-186`) ;
- les parties liées sont normalement dans
  `Excerpts/<nom>/<nom>.mscx` du conteneur, avec leur propre `.mss` ;
- 5.0 ajoute des `<eid>` (identifiants stables) sur la plupart des objets.

### 2.2 Enfants de `<Score>` utiles

| Élément | Rôle | Lecteur C++ |
|---|---|---|
| `<Division>` | division MIDI du fichier (souvent 480), sert aux `<tick>` | `Read302::readScoreTag`, `Read400::readScoreTag` |
| `<Style>` | style inline (surtout 3.x) | `compat::ReadStyleHook::readStyleTag` |
| `<metaTag name="...">` | métadonnées (composer, workTitle, movementTitle…) | `Score::setMetaTag` |
| `<Part>` | déclaration d'une partie et de ses portées | `TRead::read(Part*)` |
| `<Staff id="N">` | contenu musical d'une portée | `StaffRead::readStaff` |
| `<Score>` imbriqué | partie liée inline (3.x/4.x) | `Read302/400::readScoreTag` (récursion) |
| `<SpannerMap>` | conteneur des spanners 5.0 | `TRead::readScoreSpanners` |

`<SystemObjects>`, `<Order>`, `<PageLocks>`, `<SystemLocks>`,
`<SystemDividers>` ne concernent pas le noyau musical et peuvent être ignorés
dans un premier lecteur.

### 2.3 Conteneur `.mscz`

Le conteneur ZIP est déjà traité par `crates/mscz2xml/src/container.rs`.
Pour mémoire, les fichiers utiles sont :

- `META-INF/container.xml` : `rootfile/@full-path` désigne le MSCX principal ;
- `<nom>.mscx` : partition principale ;
- `score_style.mss` : style 4.x/5.x ;
- `Excerpts/<nom>/<nom>.mscx` et `.mss` : parties liées ;
- `chordlist.xml`, images, etc. : hors noyau.

`MscReader::readScoreFile` (`infrastructure/mscreader.cpp:139-163`) cherche
d'abord le nom annoncé, puis le premier `.mscx` à la racine. Le lecteur Rust
fait déjà ce repli.

## 3. Modèle de temps, de piste et de portée

### 3.1 Fractions, ticks et `<Division>`

Toutes les durées musicales du MSCX sont des **fractions de ronde**
(`Fraction` exacte, réductible) :

- `1/4` = noire, `1/2` = blanche, `1/1` = ronde, `3/8` = trois croches ;
- `<duration>`, `<len>`, `<fractions>`, `<ticks_f>` utilisent cette forme ;
- `XmlReader::readFraction` (`rw/xmlreader.cpp:117-134`) accepte aussi un
  entier seul, interprété comme des ticks internes, et les anciens attributs
  `z`/`n` (`<duration z="1" n="4"/>`).

`<Division>` est une survivance MIDI : elle vaut typiquement `480` et sert
uniquement à convertir les `<tick>` exprimés en ticks de fichier.
`Score::fileDivision(int)` (`dom/score.h:565`) applique :

```text
interne = (tick_fichier * 480 + Division/2) / Division
```

où `480` est `Constants::DIVISION`, la résolution interne de MuseScore. En
pratique, avec `<Division>480</Division>`, la conversion est l'identité. Ne
pas confondre cette résolution avec les fractions de durée, qui n'en
dépendent pas.

### 3.2 Tick absolu et tick relatif

- `ctx.tick()` est un tick **absolu depuis le début de la partition**.
- `ctx.rtick()` est le tick **relatif au début de la mesure courante**
  (`ReadContext::rtick`, `rw/read400/readcontext.cpp:219-223`).
- Une mesure connaît son `tick()` et sa durée `ticks()`.

Le lecteur maintient ce tick au fil de la lecture ; il n'est presque jamais
écrit dans le fichier. Voir la section 6 pour l'avancement implicite.

### 3.3 Piste, portée, voix

MuseScore a toujours 4 voix par portée (`VOICES = 4`,
`dom/mscore.h:30`) :

```text
track = staffIdx * 4 + voiceIdx      (voiceIdx de 0 à 3)
```

- `staffIdx` est l'indice global de portée, dans l'ordre des déclarations
  `<Staff>` des `<Part>` (voir section 4).
- `voiceIdx` est la **position** du bloc `<voice>` dans la mesure, pas un
  attribut (voir section 6).

### 3.4 Durée réelle d'un accord/repos

La durée écrite (`<durationType>`, `<dots>`, `<duration>`) donne des ticks
nominaux. La durée réellement consommée dans la voix est :

```text
globalTicks = duree_nominale
              / ratio(olets imbriques, du plus proche au plus externe)
actualTicks = globalTicks / timeStretch(portée, tick)
```

- `DurationElement::globalTicks` (`dom/durationelement.cpp:87-94`) divise par
  `t->ratio()` pour chaque tuplet parent ;
- `DurationElement::actualTicksAt` (`dom/durationelement.cpp:108-117`) divise
  ensuite par `Staff::timeStretch(tick)` ;
- `Staff::timeStretch` (`dom/staff.cpp:560-564`) renvoie le `stretch` de la
  métrique courante, c'est-à-dire `<stretchN>/<stretchD>` (section 12).

C'est `actualTicks()` que `MeasureRead::readVoice` ajoute au tick après chaque
accord ou repos non-orphelin (`rw/read400/measurerw.cpp:289,328`). Les
ornements (appoggiatures) ne consomment aucun tick.

## 4. `<Part>`, `<Staff>` et `<Instrument>`

### 4.1 Syntaxe

```xml
<Part>
  <Staff id="1">
    <StaffType group="pitched">
      <name>stdNormal</name>
    </StaffType>
  </Staff>
  <trackName>Flute</trackName>
  <Instrument id="Winds">
    <longName>Flute</longName>
    <shortName>Fl.</shortName>
    <trackName>Flute</trackName>
    <instrumentId>wind.group</instrumentId>
    <transposeDiatonic>0</transposeDiatonic>
    <transposeChromatic>0</transposeChromatic>
    <Channel>
      <program value="73"/>
    </Channel>
  </Instrument>
</Part>
```

### 4.2 Sémantique

`<Part>` ne contient pas de musique : il déclare une partie, ses portées et
son instrument. La musique est dans les `<Staff>` de premier niveau du
`<Score>` (section 5).

- `<Staff id="N">` dans `<Part>` : déclaration de la N-ième portée globale.
  `N` est **1-based**. La portée globale `staffIdx` vaut `N - 1`.
  `TRead::readProperties(Part*)` (`rw/read400/tread.cpp:3358`) crée la
  `Staff` et l'ajoute au score (`appendStaff`).
- L'ordre des déclarations `<Staff>` dans tous les `<Part>` définit l'ordre
  global des portées. Il doit correspondre aux `<Staff id="N">` de contenu.
- `<StaffType group="...">` : `pitched`, `percussion` ou `tab`.
- `<trackName>` de `<Part>` : nom de piste affiché. Il peut différer du
  `trackName` de l'instrument.
- `<Instrument id="...">` : instrument MuseScore. `id` est un identifiant de
  modèle (parfois erroné en 3.6, voir 4.4).
- `<instrumentId>` : identifiant MusicXML (par exemple `wind.group`,
  `keyboard.organ`). C'est la valeur à utiliser pour le son / l'export.
- `<longName>`, `<shortName>` : noms d'instrument.
- `<transposeChromatic>` / `<transposeDiatonic>` : transposition de
  l'instrument, en demi-tons et en degrés. La transposition peut varier au
  cours du temps via des changements d'instrument (section 4.5).
- `<Channel><program value="..."/>` : programme MIDI. Non indispensable au
  noyau, mais utile pour `<midi-instrument>`.
- `<clef staff="2">F</clef>` : clé par défaut de la portée `staff` (1-based),
  utile quand aucune `<Clef>` n'est écrite au début.

### 4.3 Portées multiples

Une partie à deux portées (piano, orgue) déclare deux `<Staff>` :

```xml
<Part>
  <Staff id="1"><StaffType group="pitched">...</StaffType></Staff>
  <Staff id="2"><StaffType group="pitched">...</StaffType></Staff>
  ...
</Part>
```

Les contenus correspondants sont deux `<Staff id="1">` et `<Staff id="2">`
de premier niveau, chacun avec ses mesures. Les mesures ne sont pas partagées
dans le XML : chaque portée répète sa propre séquence de `<Measure>`.

### 4.4 Différences 3.02 / 4.x / 5.0

- **3.x et 4.0–4.5** : `<Part>` n'a pas d'attribut `id`. Les `<Staff>` de
  déclaration portent `id`.
- **5.0** : `<Part id="1">` est explicite ; les `<Staff>` de déclaration n'ont
  plus d'`id` et sont ordonnés. Les noms d'instrument sont dans
  `<InstrumentLabel>` :

  ```xml
  <Part id="1">
    <Staff>...</Staff>
    <Instrument id="organ">
      <InstrumentLabel>
        <longName>Organ</longName>
        <shortName>Organ</shortName>
      </InstrumentLabel>
      ...
    </Instrument>
  </Part>
  ```

- **3.6.x** : certains `id` d'instrument sont faux. `Read302::fixInstrumentId`
  (`rw/read302/read302.cpp:292-334`) corrige par exemple `Winds` en `winds`,
  les harmonicas `d12*` en `d10*`, et plusieurs identifiants `mdl-*`. Ne pas
  se fier à `Instrument/@id` seul : préférer `<instrumentId>` et
  `<trackName>` pour l'export.
- **5.0** : les parties partagées utilisent `<SharedPart>` et des références
  `<sharedPart>` par EID (`TRead::readProperties(Part*)`,
  `rw/read500/tread.cpp:3565`). Hors périmètre d'un export de partition
  complète.

### 4.5 Changements d'instrument

Un `<InstrumentChange>` peut apparaître dans une voix. Il porte un
`<Instrument>` complet et détermine la transposition à partir de son tick.
`Staff::transpose(tick)` et `Part::instrument(tick)` résolvent l'instrument
courant. Pour l'export, calculer la transposition par tick, pas une seule
fois pour la partie.

## 5. `<Measure>`

### 5.1 Syntaxe

```xml
<Measure len="3/4">
  <voice>
    <KeySig><accidental>2</accidental></KeySig>
    <TimeSig><sigN>3</sigN><sigD>4</sigD></TimeSig>
    <Chord>
      <durationType>quarter</durationType>
      <Note><pitch>69</pitch><tpc>17</tpc></Note>
    </Chord>
  </voice>
</Measure>
```

L'attribut `len` est optionnel. Il n'y a **pas** de numéro de mesure dans le
XML : la mesure est identifiée par sa position dans la séquence. Un
`<noOffset>` éventuel ne change que l'affichage du numéro
(`MeasureRead::readMeasure`, `rw/read400/measurerw.cpp:129-130`).

### 5.2 Sémantique

- Chaque `<Measure>` commence au tick courant de la portée.
- Sa durée par défaut est la métrique héritée de la mesure précédente
  (`4/4` pour la première) :
  `measure->setTicks(f); measure->setTimesig(f);`
  (`rw/read400/staffrw.cpp:58-61`).
- Une `<TimeSig>` au début de la mesure peut remplacer cette durée ;
  une `<TimeSig>` en fin de mesure annonce la suivante (section 12).
- `len="N/D"` force la durée de la mesure à `N/D` et marque la mesure comme
  irrégulière (section 18).
- Les `<voice>` sont lues dans l'ordre ; chaque voix repart du début de la
  mesure.
- Les autres enfants (`<startRepeat>`, `<endRepeat>`, `<Marker>`,
  `<Jump>`, `<BarLine>`, `<LayoutBreak>`, `<stretch>`, `<noOffset>`,
  `<measureNumberMode>`, `<irregular>`, `<breakMultiMeasureRest>`,
  `<vspacer*>`, `<visible>`, `<stemless>`, `<multiMeasureRest>`) ne changent
  pas le tick musical, sauf `<multiMeasureRest>` qui est un cas particulier
  de mesure de repos multi-mesures.
- La durée finale de la mesure est `measure->ticks()` ; c'est elle qui fait
  avancer le tick de portée (`ctx.setTick(measure->tick() + measure->ticks())`,
  `rw/read400/staffrw.cpp:72`).

### 5.3 Mesures multi-portées

`StaffRead::readStaff` ne crée les mesures que pour la portée d'indice 0
(`id="1"`). Pour les portées suivantes, il réutilise les mesures déjà créées,
dans l'ordre (`rw/read400/staffrw.cpp:95-126`). Si une mesure manque, le
lecteur en crée une vide et journalise `missing measure!`.

Conséquence : la première portée de premier niveau doit être `id="1"`, et
chaque portée doit avoir exactement la même séquence de mesures. Un décalage
décalle toutes les portées suivantes.

### 5.4 Lecteur C++

- `StaffRead::readStaff` : `rw/read400/staffrw.cpp:39`
- `MeasureRead::readMeasure` : `rw/read400/measurerw.cpp:70`
- `Measure::checkMeasure` : `dom/check.cpp:307` (contrôle de cohérence,
  comble les trous avec des repos, journalise les débordements)

### 5.5 Différences

- **3.02 / 4.0** : `len` alimente aussi `compatTimeSigMap` pour les vieilles
  conversions (`rw/read400/measurerw.cpp:99-100`).
- **4.1+** : cet ajout disparaît (`rw/read410/measureread.cpp:93-95`).
- **4.1+** : `<stretch>` n'est plus ignoré avant 4.0 ; il est lu comme
  `userStretch`. En 3.x, `<stretch>` est ignoré
  (`rw/read400/measurerw.cpp:118-128`).
- **5.0** : `<mmRestLast>` (EID de la dernière mesure d'un multi-repos)
  apparaît (`rw/read500/measureread.cpp:179-184`).

## 6. `<voice>`

### 6.1 Syntaxe

```xml
<Measure>
  <voice>
    <Chord>...</Chord>
    <Rest>...</Rest>
  </voice>
  <voice>
    <location><fractions>1/4</fractions></location>
    <Chord>...</Chord>
  </voice>
</Measure>
```

### 6.2 Sémantique

- Un bloc `<voice>` représente une voix. Son indice est **positionnel** :
  le premier `<voice>` de la mesure est la voix 0, le deuxième la voix 1,
  etc. `MeasureRead::readVoice` fait `ctx.setTrack(nextTrack++)`
  (`rw/read400/measurerw.cpp:106-108`), avec
  `nextTrack = staffIdx * 4`.
- Chaque `<voice>` démarre au tick de la mesure
  (`ctx.setTick(measure->tick())`).
- À l'intérieur, le tick avance implicitement de `actualTicks()` après chaque
  `<Chord>` ou `<Rest>` non-ornement. Il n'y a pas de tick par accord.
- `<location>` déplace le curseur de lecture (section 10).
- `<tick>` (legacy) fixe un tick absolu (section 10).
- Les éléments suivants ne consomment pas de tick : `<Clef>`, `<KeySig>`,
  `<TimeSig>`, `<BarLine>`, `<Breath>`, `<Dynamic>`, `<Expression>`,
  `<Harmony>`, `<Tempo>`, `<StaffText>`, `<RehearsalMark>`, `<Fermata>`,
  `<Symbol>`, `<Image>`, `<Spanner>`, `<Tuplet>` et `<endTuplet>`
  (le `<Tuplet>` encadre seulement des accords/repos), `<Beam>`,
  `<MeasureRepeat>` (qui consomme la mesure entière), `<Ambitus>`.
- `<MeasureRepeat>` / `<RepeatMeasure>` : consomme `measure->ticks()`
  (`rw/read400/measurerw.cpp:339-352`).
- Un `<voice>` vide est autorisé syntaxiquement ; il ne crée aucun élément.

### 6.3 Ordre et voix vides

Le lecteur C++ ne lit aucun numéro de voix : il incrémente un compteur par
`<voice>` rencontré. Il faut donc :

- préserver l'ordre des `<voice>` ;
- ne pas déduire l'indice de voix du contenu ;
- ne pas supprimer un `<voice>` vide lors du parsing, sous peine de décaler
  les voix suivantes.

Dans le corpus de test de la référence, on ne trouve pas de `<voice>` vide :
les mesures à une seule voix n'émettent qu'un bloc. Le modèle Rust doit donc
conserver la position ordinale, et non supposer que la voix 0 existe toujours.

### 6.4 Contenu d'une voix

Une voix contient, dans l'ordre du fichier :

- les éléments de début de mesure : `<Clef>`, `<KeySig>`, `<TimeSig>` ;
- des accords `<Chord>`, repos `<Rest>`, ornements `<Chord>` avec
  `<appoggiatura/>` ;
- des annotations (`<Dynamic>`, `<Harmony>`, `<Tempo>`, `<StaffText>`,
  `<RehearsalMark>`, `<Fermata>`, `<Symbol>`, `<Image>`) ;
- des spanners (`<Spanner type="...">`) en 3.x/4.x ;
- des tuplets (`<Tuplet>` … `<endTuplet/>`) ;
- des barres de mesure (`<BarLine>`) et soufflets (`<Breath>`).

### 6.5 Lecteur C++

- `MeasureRead::readVoice` : `rw/read400/measurerw.cpp:211`
- `MeasureRead::readMeasure` (création des MStaff et appel des voix) :
  `rw/read400/measurerw.cpp:70`

### 6.6 Différences

- **3.02 / 4.0** : `<location>` est lu et appliqué ; `<tick>` est accepté
  comme obsolète (`// obsolete?`).
- **4.1+** : `<location>` peut porter un drapeau `isTimeTick` et créer un
  ancrage de tick de métrique (`rw/read410/measureread.cpp:215-220`).
- **5.0** : le `<Spanner>` de niveau voix n'est plus lu dans une mesure ;
  les spanners sont dans `<SpannerMap>` au niveau du score (section 15).

## 7. `<Chord>`, `<Rest>` et `<Note>`

### 7.1 Syntaxe d'un accord

```xml
<Chord>
  <durationType>quarter</durationType>
  <dots>1</dots>
  <StemDirection>down</StemDirection>
  <Note>
    <pitch>75</pitch>
    <tpc>11</tpc>
  </Note>
</Chord>
```

### 7.2 Syntaxe d'un repos

```xml
<Rest>
  <durationType>quarter</durationType>
</Rest>
```

Repos de mesure :

```xml
<Rest>
  <durationType>measure</durationType>
  <duration>1/1</duration>
</Rest>
```

### 7.3 Sémantique des durées

`ChordRest` (base commune de `Chord` et `Rest`) lit :

- `<durationType>` : type de durée (`long`, `breve`, `whole`, `half`,
  `quarter`, `eighth`, `16th`, `32nd`, `64th`, `128th`, `256th`, `512th`,
  `1024th`, `measure`). `TRead::readProperties(ChordRest*)`
  (`rw/read400/tread.cpp:2405`) appelle `setDurationType`, puis, si le type
  n'est pas `V_MEASURE`, fixe les ticks à `actualDurationType().fraction()`.
- `<dots>` : nombre de points (0 à 4 en pratique). Le multiplicateur est
  `(2^(n+1)-1) / 2^n` (`TDuration::fraction`, `dom/durationtype.cpp:357-404`).
- `<duration>` : fraction de ronde qui **écrase** les ticks calculés. C'est
  la valeur autoritaire quand elle est présente.
- `<small>`, `<staffMove>`, `<BeamMode>`, `<Articulation>`, `<Ornament>`,
  `<Lyrics>`, `<Spanner>` : attributs secondaires.

**Ordre de lecture important** : `<dots>` doit précéder `<durationType>`.
`<durationType>` calcule les ticks immédiatement avec les points déjà lus ;
si `<dots>` vient après, les ticks n'incluent pas les points. Tous les
fichiers du corpus respectent cet ordre (points avant type). Le lecteur Rust
doit soit lire l'ensemble des enfants avant de calculer, soit reproduire
l'ordre et signaler l'anomalie.

### 7.4 Repos de mesure

`MeasureRead::readVoice` crée tout `<Rest>` avec
`DurationType::V_MEASURE` et
`ticks = measure->timesig() / timeStretch` avant de lire le contenu
(`rw/read400/measurerw.cpp:304-309`). Ensuite :

- si `<durationType>measure</durationType>` est présent, les ticks restent la
  durée de la mesure (sauf `<duration>` explicite) ;
- si un autre `<durationType>` est présent, il remplace les ticks ;
- `<duration>` explicite écrase le tout. Dans le corpus, il vaut souvent
  `1/1` ou `4/4` pour une mesure 4/4, `8/4` pour une mesure 8/4 : c'est bien
  la longueur de la mesure, sous forme parfois non réduite.

Pour un export, un repos `measure` doit couvrir du tick de la mesure à sa fin.
Utiliser `measure->ticks()`, pas la valeur brute de `<duration>`, si les deux
diffèrent.

### 7.5 Notes : hauteur et orthographe

```xml
<Note>
  <pitch>69</pitch>
  <tpc>17</tpc>
</Note>
```

- `<pitch>` : hauteur MIDI **sonnante**, 0–127, 60 = do central
  (`Note::pitch`, `dom/note.h:547`). Le lecteur borne à 0–127
  (`clampPitch`, `rw/read400/tread.cpp:3022`).
- `<tpc>` : « tonal pitch class », encodage de l'orthographe (degré +
  altération). Il est lu dans `tpc1` **et** `tpc2`
  (`TRead::readProperties(Note*)`, `rw/read400/tread.cpp:3026-3029`).
- `<tpc2>` : orthographe de la note écrite, quand elle diffère de
  l'orthographe sonnante. `tpc1` est l'orthographe de concert, `tpc2` celle
  de la partie transposée.
- `<Accidental>` : altération explicite affichée (objet graphique), distincte
  du `tpc`.
- `<track>` : surcharge de piste, rare (performance).
- `<velocity>`, `<veloType>`, `<play>`, `<tuning>`, `<fret>`, `<string>`,
  `<ghost>`, `<dead>`, `<head>`, `<headType>`, `<line>` : lecture/jeu.
- `<NoteDot>` : points graphiques individuels ; les points de durée sont
  dans `<dots>` du ChordRest.
- `<Text>`, `<Fingering>`, `<Symbol>`, `<Image>`, `<Bend>`, `<Events>` :
  annotations/jeu.

`Note::setupAfterRead` (`dom/note.cpp:1651-1712`) complète et corrige :

- si aucun `tpc` n'est valide, calcule un `tpc` par défaut à partir de la
  tonalité et de la hauteur (`pitch2tpc`) ;
- si un seul des deux est valide, dérive l'autre par transposition ;
- si `pitch` et `tpc1` ne sont pas cohérents (modulo 12), ajuste `pitch`
  pour rester cohérent avec l'orthographe, sauf sur une portée de
  percussion.

Pour l'export MusicXML, il est donc préférable de calculer step/alter/octave
depuis `tpc` et `pitch` selon la table ci-dessous, et de vérifier la
cohérence plutôt que de faire confiance aveuglément à `pitch` seul.

### 7.6 Table TPC

Les valeurs naturelles sont :

| Degré | F | C | G | D | A | E | B |
|---|---|---|---|---|---|---|---|
| `tpc` | 13 | 14 | 15 | 16 | 17 | 18 | 19 |

Chaque dièse ajoute `+7`, chaque bémol `-7` (jusqu'à 3 altérations) :
`F# = 20`, `Bb = 12`, `C## = 28`, `Ebb = 4`, etc.
`Tpc::TPC_MIN = -8` (`Fbbb`) et `Tpc::TPC_MAX = 40` (`B###`)
(`dom/pitchspelling.h:41-58`).

Conversion (`dom/pitchspelling.cpp` et `dom/pitchspelling.h:121-129`) :

```text
alter = (tpc - TPC_MIN) / 7 - 3          // -3..+3
step  = {F:3, C:0, G:4, D:1, A:5, E:2, B:6}[(tpc - TPC_MIN) % 7]
octave = (pitch - alter) / 12 - 1
```

La table `tpc2pitch` (`dom/pitchspelling.cpp:195-210`) donne les demi-tons
par rapport à C pour les naturels : `F=5, C=0, G=7, D=2, A=9, E=4, B=11`,
plus `alter`. Pour un accord transposé, la hauteur écrite est
`pitch - instrument.transpose.chromatic`.

### 7.7 Lecteur C++

- `TRead::read(Chord*)` : `rw/read400/tread.cpp:2304`
- `TRead::readProperties(Chord*)` : `rw/read400/tread.cpp:2323`
- `TRead::readProperties(ChordRest*)` : `rw/read400/tread.cpp:2405`
- `TRead::read(Rest*)` : `rw/read400/tread.cpp:3447`
- `TRead::read(Note*)` : `rw/read400/tread.cpp:3005`
- `TRead::readProperties(Note*)` : `rw/read400/tread.cpp:3020`
- `Note::setupAfterRead` : `dom/note.cpp:1651`

### 7.8 Différences 3.02 / 4.x / 5.0

- **3.02 / 4.0** : `<durationType>` contient encore les compatibilités
  pré-1.12 (conversion des rondes en repos de mesure, `ticklen` obsolète) et
  la gestion des métriques pré-1.14 (`rw/read400/tread.cpp:2405-2458`).
- **avant 4.1** : `<BeamMode>` traduit l'ancien vocabulaire
  (`begin64`/`begin32` deviennent `begin32`/`begin16`,
  `rw/read400/tread.cpp:2433-2443`). **4.1+** lit la valeur telle quelle
  (`rw/read410/tread.cpp:2501-2502`).
- **4.0+** : `ghost` est lu comme `ghost` (4.x) au lieu de `dead` (3.x) ;
  `<velocity>` est accepté comme vélocité utilisateur en 4.x, ignoré en 3.x
  (`rw/read400/tread.cpp:3056-3080`).
- **5.0** : les compatibilités pré-1.12 disparaissent de
  `readProperties(ChordRest*)` ; `<durationType>` fixe simplement les ticks
  (`rw/read500/tread.cpp:2576-2587`). `<Text>` et `<LaissezVib>`/`<PartialTie>`
  apparaissent dans `<Note>`.

## 8. `<Tuplet>` et `<endTuplet/>`

### 8.1 Syntaxe

```xml
<Tuplet>
  <normalNotes>2</normalNotes>
  <actualNotes>3</actualNotes>
  <baseNote>eighth</baseNote>
  <Number><text>3</text></Number>
  <Chord>
    <durationType>eighth</durationType>
    <Note><pitch>72</pitch><tpc>15</tpc></Note>
  </Chord>
  <Chord>...</Chord>
  <Chord>...</Chord>
</Tuplet>
<endTuplet/>
```

### 8.2 Sémantique

- `<normalNotes>` : dénominateur du ratio (nombre de notes « normales »).
- `<actualNotes>` : numérateur (nombre de notes jouées).
- `<baseNote>` : durée de base (`eighth`, `quarter`, …), avec
  `<baseDots>` éventuel.
- `<Number>` : texte affiché (facultatif) ; ses propriétés de police sont
  remontées dans le tuplet.
- `Tuplet::ticks()` vaut
  `baseLen.fraction() * ratio.denominator()` :
  un triolet de croches (`baseNote=eighth`, `normalNotes=2`) vaut `1/8 * 2 = 1/4`
  (`TRead::read(Tuplet*)`, `rw/read400/tread.cpp:1519-1521`).
- Les accords internes ont une durée nominale (`1/8`) mais une durée réelle
  divisée par le ratio (`1/12` pour un triolet), via
  `DurationElement::globalTicks`.
- `<endTuplet/>` est un élément **frère**, pas une balise fermante XML. Il
  ferme le tuplet courant et remonte au parent.
- Les tuplets peuvent être imbriqués : un `<Tuplet>` rencontré alors qu'un
  autre est ouvert devient son enfant
  (`rw/read400/measurerw.cpp:587-595`). Chaque `endTuplet` ferme un niveau.
- Un `endTuplet` sans tuplet ouvert est ignoré avec un log. Un tuplet vide
  est supprimé ; un tuplet non fermé en fin de voix est supprimé s'il est
  vide, sinon laissé en place avec un log
  (`rw/read400/measurerw.cpp:596-646`).

### 8.3 Avancement du tick

Les `<Tuplet>` eux-mêmes n'avancent pas le tick. Ce sont les accords/repos
internes qui l'avancent, avec leur `actualTicks()` déjà divisé par le ratio
du tuplet (et des tuplets parents). Ne pas ajouter `tuplet->ticks()` en plus.

### 8.4 Lecteur C++

- `MeasureRead::readVoice` (branches `Tuplet`/`endTuplet`) :
  `rw/read400/measurerw.cpp:587-612`
- `TRead::read(Tuplet*)` : `rw/read400/tread.cpp:1446`
- `DurationElement::globalTicks` : `dom/durationelement.cpp:87`

### 8.5 Différences

Le mécanisme est stable de 3.02 à 5.0. En 5.0, les branches `Tuplet` et
`endTuplet` restent dans `MeasureRead::readVoice`
(`rw/read500/measureread.cpp:320-345`) ; seule la gestion des spanners
change autour.
