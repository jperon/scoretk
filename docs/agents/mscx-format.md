# Format MSCX — noyau musical
Référence de lecture des `.mscx` 3.x/4.x/5.x pour les agents qui écrivent en
Rust les lecteurs de `mscz2xml`. Sources C++ vérifiées, en lecture seule :
`ref-musescore/src/engraving/{rw/read302,read400,read410,read460,read500,dom}`.
Chemins relatifs à `ref-musescore/src/engraving/`. Corpus de contrôle :
`src/importexport/musicxml/tests/data/*.mscx` (3.01 à 5.00).

## 1. Versions et aiguillage
La version du format est l'attribut `version` de `<museScore>`, pas
`<programVersion>` : un fichier 3.6.2 déclare `version="3.02"`.
```xml
<museScore version="3.02">
  <programVersion>3.6.2</programVersion>
```
`MscLoader::readMasterScore` (`rw/mscloader.cpp:219-231`) calcule
`mscVersion = majeur*100 + mineur` : `3.02→302`, `4.10→410`, `4.60→460`,
`5.00→500`. `RWRegister::reader` (`rw/rwregister.cpp:39-58`) choisit :

| `mscVersion` | Lecteur | Période |
|---|---|---|
| `< 400` | `Read302` | 3.00–3.99 |
| `400..409` | `Read400` | 4.0.x |
| `410..459` | `Read410` | 4.1–4.5 |
| `460..499` | `Read460` | 4.6–4.9 |
| `>= 500` | `Read500` | 5.0+ |

Le lecteur 3.x réutilise `read400` : `Read302::readScoreTag`
(`rw/read302/read302.cpp:54`) appelle `read400::StaffRead::readStaff` et
`read400::TRead`. Les seuils internes diffèrent parfois du dispatch (4.1, 4.5,
5.0) : comparer l'entier `mscVersion`, jamais les chaînes.

## 2. Structure du fichier
Squelette 3.x :
```xml
<museScore version="3.02"><Score>
  <Division>480</Division><Style>...</Style>
  <metaTag name="composer">...</metaTag>
  <Part>...</Part>
  <Staff id="1"><Measure>...</Measure></Staff>
</Score></museScore>
```
En 4.x/5.x, le style est normalement dans `score_style.mss` du `.mscz`
(`MscLoader::loadMscz`, `rw/mscloader.cpp:105-118`), mais `<Style>` inline
reste accepté (`Read400::readScoreTag`, `rw/read400/read400.cpp:170-186`).
Les parties liées sont dans `Excerpts/<nom>/<nom>.mscx`. 5.0 ajoute des
`<eid>` sur la plupart des objets.

Enfants utiles de `<Score>` :

| Élément | Rôle | Lecteur |
|---|---|---|
| `<Division>` | division MIDI (souvent 480), sert aux `<tick>` | `Read302/400::readScoreTag` |
| `<Style>` | style inline (surtout 3.x) | `compat::ReadStyleHook::readStyleTag` |
| `<metaTag name>` | composer, workTitle, movementTitle… | `Score::setMetaTag` |
| `<Part>` | partie et déclaration de ses portées | `TRead::read(Part*)` |
| `<Staff id>` | contenu musical d'une portée | `StaffRead::readStaff` |
| `<Score>` imbriqué | partie liée inline | récursion `readScoreTag` |
| `<SpannerMap>` | spanners 5.0 | `TRead::readScoreSpanners` |

`<SystemObjects>`, `<Order>`, `<PageLocks>`, `<SystemLocks>`,
`<SystemDividers>` sont hors noyau. Le conteneur `.mscz` est traité par
`crates/mscz2xml/src/container.rs` : `META-INF/container.xml`, `<nom>.mscx`,
`score_style.mss`, `Excerpts/`.
`MscReader::readScoreFile` (`infrastructure/mscreader.cpp:139-163`) cherche le
nom annoncé puis le premier `.mscx` à la racine.

## 3. Temps, piste, portée
Les durées sont des **fractions de ronde** exactes : `1/4` = noire, `1/2` =
blanche, `1/1` = ronde, `3/8` = trois croches. `<duration>`, `<len>`,
`<fractions>`, `<ticks_f>` utilisent cette forme. `XmlReader::readFraction`
(`rw/xmlreader.cpp:117-134`) accepte aussi un entier seul (ticks internes) et
les anciens attributs `z`/`n`.

`<Division>` ne sert qu'aux `<tick>` : `Score::fileDivision(int)`
(`dom/score.h:565`) applique `(tick_fichier * 480 + Division/2) / Division`,
où 480 est `Constants::DIVISION` (identité si `<Division>480</Division>`). Les
fractions de durée n'en dépendent pas.

- `ctx.tick()` : tick absolu depuis le début ; `ctx.rtick()` : relatif à la
  mesure (`ReadContext::rtick`, `rw/read400/readcontext.cpp:219-223`).
- Une mesure connaît `tick()` et `ticks()` ; le tick est presque toujours
  implicite (section 6).

`VOICES = 4` (`dom/mscore.h:30`) : `track = staffIdx*4 + voiceIdx` (0..3).
`staffIdx` suit l'ordre global des déclarations `<Staff>` des `<Part>` ;
`voiceIdx` est la position du bloc `<voice>`, pas un attribut.

Durée réelle d'un accord/repos :
```text
globalTicks = duree_nominale / ratios_des_olets_imbriques
actualTicks = globalTicks / timeStretch(portee, tick)
```
`DurationElement::globalTicks` (`dom/durationelement.cpp:87-94`) divise par
`t->ratio()` pour chaque tuplet parent ; `actualTicksAt`
(`dom/durationelement.cpp:108-117`) divise par `Staff::timeStretch`
(`dom/staff.cpp:560-564`), le `stretch` de la métrique courante.
`MeasureRead::readVoice` ajoute `actualTicks()` après chaque accord/repos
non-ornement (`rw/read400/measurerw.cpp:289,328`) ; les ornements ne
consomment aucun tick.

## 4. `<Part>`, `<Staff>`, `<Instrument>`
```xml
<Part>
  <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
  <trackName>Flute</trackName>
  <Instrument id="Winds">
    <longName>Flute</longName><shortName>Fl.</shortName>
    <trackName>Flute</trackName><instrumentId>wind.group</instrumentId>
    <transposeChromatic>0</transposeChromatic>
    <Channel><program value="73"/></Channel>
  </Instrument>
</Part>
```
`<Part>` ne contient pas de musique : il déclare une partie, ses portées et son
instrument. `<Staff id="N">` dans `<Part>` est **1-based** (`staffIdx=N-1`) ;
`TRead::readProperties(Part*)` (`rw/read400/tread.cpp:3358`) crée la `Staff` et
l'ajoute (`appendStaff`). L'ordre des déclarations dans tous les `<Part>`
définit l'ordre global des portées.

- `<StaffType group="pitched|percussion|tab">` ; `<trackName>` : nom de piste,
  pouvant différer du `trackName` d'instrument.
- `<Instrument id>` : modèle MuseScore ; `<instrumentId>` : identifiant
  MusicXML (`wind.group`), à préférer pour l'export.
- `<transposeChromatic>`/`<transposeDiatonic>` : transposition, variable dans
  le temps via les changements d'instrument.
- `<clef staff="2">F</clef>` : clé par défaut de la portée 1-based indiquée ;
  `<Channel><program value>` : programme MIDI.
- Une partie à deux portées déclare deux `<Staff>` ; chaque portée répète sa
  propre séquence de `<Measure>` (mesures non partagées en XML).
- `<InstrumentChange>` porte un `<Instrument>` complet et fixe la transposition
  à partir de son tick. `Staff::transpose(tick)` et `Part::instrument(tick)`
  résolvent l'instrument courant : calculer par tick.

Différences : **3.x/4.0–4.5** n'ont pas `Part/@id` ; les `<Staff>` de
déclaration portent `id`. **5.0** a `<Part id="1">`, des `<Staff>` sans `id`
(ordre), et des noms dans `<InstrumentLabel>` (`<longName>`, `<shortName>`).
**3.6.x** peut avoir des `Instrument/@id` faux ; `Read302::fixInstrumentId`
(`rw/read302/read302.cpp:292-334`) corrige `Winds→winds`, harmonicas
`d12*→d10*`, plusieurs `mdl-*`. **5.0** : parties partagées `<SharedPart>` /
`<sharedPart>` (EID), hors périmètre d'un export complet.

## 5. `<Measure>`
```xml
<Measure len="3/4">
  <voice>
    <KeySig><accidental>2</accidental></KeySig>
    <TimeSig><sigN>3</sigN><sigD>4</sigD></TimeSig>
    <Chord><durationType>quarter</durationType>
      <Note><pitch>69</pitch><tpc>17</tpc></Note></Chord>
  </voice>
</Measure>
```
`len` est optionnel. Il n'y a **pas** de numéro de mesure : la position dans la
séquence fait foi ; `<noOffset>` ne change que l'affichage
(`rw/read400/measurerw.cpp:129-130`). Chaque mesure commence au tick courant de
la portée. Sa durée par défaut est la métrique héritée (`4/4` au départ) :
`measure->setTicks(f); measure->setTimesig(f);`
(`rw/read400/staffrw.cpp:58-61`). Une `<TimeSig>` au début remplace la durée ;
une `<TimeSig>` en fin annonce la suivante (section 10). `len="N/D"` force
`ticks=N/D` et marque la mesure irrégulière (section 14). Les `<voice>`
repartent du début de la mesure.

Les autres enfants (`<startRepeat>`, `<endRepeat>`, `<Marker>`, `<BarLine>`,
`<LayoutBreak>`, `<stretch>`, `<irregular>`, `<vspacer*>`,
`<multiMeasureRest>`) ne changent pas le tick. `measure->ticks()` fait avancer le tick de portée
(`rw/read400/staffrw.cpp:72`).

`StaffRead::readStaff` (`rw/read400/staffrw.cpp:39`) ne crée les mesures que
pour la portée 0 (`id="1"`) ; les suivantes les réutilisent par ordre
(`:95-126`). Une mesure manquante est créée vide (`missing measure!`). La
première portée doit être `id="1"` et toutes doivent avoir la même séquence.

`MeasureRead::readMeasure` : `rw/read400/measurerw.cpp:70` ;
`Measure::checkMeasure` : `dom/check.cpp:307` (cohérence, comble les trous,
journalise les débordements). **3.02/4.0** : `len` alimente
`compatTimeSigMap` (`:99-100`) et `<stretch>` est ignoré avant 4.0
(`:118-128`). **4.1+** : plus de `compatTimeSigMap`, `<stretch>` lu.
**5.0** : `<mmRestLast>` (EID de fin de multi-repos) apparaît
(`rw/read500/measureread.cpp:179-184`).

## 6. `<voice>`
```xml
<Measure>
  <voice><Chord>...</Chord><Rest>...</Rest></voice>
  <voice>
    <location><fractions>1/4</fractions></location>
    <Chord>...</Chord>
  </voice>
</Measure>
```
L'indice de voix est **positionnel** : premier `<voice>` = voix 0, deuxième =
voix 1… `MeasureRead::readVoice` fait `ctx.setTrack(nextTrack++)`
(`rw/read400/measurerw.cpp:106-108`), avec `nextTrack=staffIdx*4`. Chaque voix
démarre au tick de la mesure (`ctx.setTick(measure->tick())`). Le tick avance
de `actualTicks()` après chaque `<Chord>`/`<Rest>` non-ornement ; il n'y a pas
de tick par accord.

Ne consomment pas de tick : `<Clef>`, `<KeySig>`, `<TimeSig>`, `<BarLine>`,
`<Breath>`, `<Dynamic>`, `<Expression>`, `<Harmony>`, `<Tempo>`,
`<StaffText>`, `<RehearsalMark>`, `<Fermata>`, `<Symbol>`, `<Image>`,
`<Spanner>`, `<Tuplet>`/`<endTuplet>`, `<Beam>`, `<Ambitus>`. `<MeasureRepeat>`
/ `<RepeatMeasure>` consomme `measure->ticks()` (`:339-352`). Un `<voice>`
vide ne crée rien.

Le lecteur C++ incrémente un compteur par `<voice>` : préserver l'ordre, ne
pas déduire l'indice du contenu, ne pas supprimer un `<voice>` vide (cela
décale les suivants). Le corpus n'a pas de `<voice>` vide : une mesure à une
voix n'émet qu'un bloc.
Contenu typique : `<Clef>`/`<KeySig>`/`<TimeSig>`, accords/repos/ornements,
annotations (`<Dynamic>`, `<Harmony>`, `<Tempo>`…), spanners, tuplets, barres.

`MeasureRead::readVoice` : `rw/read400/measurerw.cpp:211`. **3.02/4.0** :
`<tick>` accepté comme obsolète. **4.1+** : `<location>` peut porter
`isTimeTick` et créer un ancrage de métrique
(`rw/read410/measureread.cpp:215-220`). **5.0** : plus de `<Spanner>` dans une
mesure ; les spanners sont dans `<SpannerMap>` (section 12).

## 7. `<Chord>`, `<Rest>`, `<Note>`
```xml
<Chord><durationType>quarter</durationType><dots>1</dots>
  <StemDirection>down</StemDirection>
  <Note><pitch>75</pitch><tpc>11</tpc></Note></Chord>
<Rest><durationType>quarter</durationType></Rest>
<Rest><durationType>measure</durationType><duration>1/1</duration></Rest>
```
`ChordRest` lit `<durationType>` (`long`, `breve`, `whole`, `half`, `quarter`,
`eighth`, `16th`, `32nd`, `64th`, `128th`, `256th`, `512th`, `1024th`,
`measure`). `TRead::readProperties(ChordRest*)` (`rw/read400/tread.cpp:2405`)
fixe les ticks à `actualDurationType().fraction()` sauf pour `V_MEASURE`.
`<dots>` (0 à 4) applique le multiplicateur `(2^(n+1)-1)/2^n`
(`TDuration::fraction`, `dom/durationtype.cpp:357-404`). `<duration>` est une
fraction de ronde qui **écrase** les ticks calculés. `<small>`,
`<staffMove>`, `<BeamMode>`, `<Articulation>`, `<Ornament>`, `<Lyrics>`,
`<Spanner>` sont secondaires.

**Ordre** : `<dots>` doit précéder `<durationType>`, car les ticks sont
calculés en lisant le type avec les points déjà lus. Le corpus respecte cet
ordre ; un lecteur Rust peut lire tous les enfants avant de calculer, mais doit
signaler l'ordre inverse.

`readVoice` crée tout `<Rest>` en `V_MEASURE` avec
`ticks = measure->timesig() / timeStretch` (`measurerw.cpp:304-309`). Puis
`<durationType>` peut changer le type, et `<duration>` écrase les ticks. Dans
le corpus, `<duration>` vaut souvent `1/1` ou `4/4` pour une mesure 4/4, `8/4`
pour 8/4 : c'est la longueur de la mesure, parfois non réduite. Pour l'export,
utiliser `measure->ticks()` si `<duration>` et la mesure diffèrent.

Notes :
```xml
<Note><pitch>69</pitch><tpc>17</tpc><tpc2>17</tpc2></Note>
```
- `<pitch>` : hauteur MIDI **sonnante** 0–127, 60 = do central
  (`dom/note.h:547`), bornée par `clampPitch` (`rw/read400/tread.cpp:3022`).
- `<tpc>` : orthographe (degré + altération), fixe `tpc1` **et** `tpc2`
  (`:3026-3029`). `<tpc2>` surcharge l'orthographe écrite ; `tpc1` = concert,
  `tpc2` = partie transposée.
- `<Accidental>` : altération graphique, distincte du `tpc`.
- `<track>`, `<velocity>`, `<play>`, `<tuning>`, `<fret>`, `<string>`,
  `<ghost>`, `<dead>`, `<head>`, `<line>` : lecture/jeu. `<NoteDot>` : points
  graphiques ; les points de durée sont dans `<dots>` du ChordRest.

`Note::setupAfterRead` (`dom/note.cpp:1651-1712`) dérive un `tpc` manquant de
la tonalité et de `pitch`, dérive l'autre orthographe par transposition, et
ajuste `pitch` si `tpc1` est incohérent modulo 12 (sauf percussion).

Table TPC (`dom/pitchspelling.h:41-58`) :

| Degré | F | C | G | D | A | E | B |
|---|---|---|---|---|---|---|---|
| `tpc` | 13 | 14 | 15 | 16 | 17 | 18 | 19 |

Chaque dièse ajoute 7, chaque bémol retranche 7 (`F#=20`, `Bb=12`,
`C##=28`). `TPC_MIN=-8` (`Fbbb`), `TPC_MAX=40` (`B###`). Conversion
(`dom/pitchspelling.cpp`, `dom/pitchspelling.h:121-129`) :
```text
alter  = (tpc - TPC_MIN) / 7 - 3          // -3..+3
step   = {F:3,C:0,G:4,D:1,A:5,E:2,B:6}[(tpc - TPC_MIN) % 7]
octave = (pitch - alter) / 12 - 1
```
`tpc2pitch` (`dom/pitchspelling.cpp:195-210`) donne les demi-tons des naturels
(`F=5, C=0, G=7, D=2, A=9, E=4, B=11`), plus `alter`. Pour un instrument
transpositeur, la hauteur écrite est `pitch - instrument.transpose.chromatic`.

Lecteurs : `TRead::read(Chord*)` (`rw/read400/tread.cpp:2304`),
`readProperties(Chord*)` (`:2323`), `readProperties(ChordRest*)` (`:2405`),
`read(Rest*)` (`:3447`), `read(Note*)` (`:3005`), `readProperties(Note*)`
(`:3020`). **3.02/4.0** : compatibilités pré-1.12 dans `<durationType>`
(rondes converties en repos de mesure, `ticklen` obsolète) et métriques
pré-1.14 (`:2405-2458`). **Avant 4.1** : `<BeamMode>` traduit
`begin64`/`begin32` en `begin32`/`begin16` (`:2433-2443`) ; **4.1+** lit
direct (`rw/read410/tread.cpp:2501`). **4.0+** : `ghost` = `ghost` (3.x :
`dead`) ; `<velocity>` utilisateur en 4.x, ignoré en 3.x. **5.0** : plus de
compat pré-1.12 (`rw/read500/tread.cpp:2576`), ajout de `<Text>`,
`<LaissezVib>`, `<PartialTie>` dans `<Note>`.

## 8. `<Tuplet>` et `<endTuplet/>`
```xml
<Tuplet>
  <normalNotes>2</normalNotes>
  <actualNotes>3</actualNotes>
  <baseNote>eighth</baseNote>
  <Number><text>3</text></Number>
  <Chord><durationType>eighth</durationType>...</Chord>
  <Chord>...</Chord><Chord>...</Chord>
</Tuplet>
<endTuplet/>
```
`<normalNotes>` = dénominateur du ratio, `<actualNotes>` = numérateur,
`<baseNote>` (+ `<baseDots>`) = durée de base.
`ticks = baseLen.fraction() * ratio.denominator()` : triolet de croches =
`1/8*2 = 1/4`
(`TRead::read(Tuplet*)`, `rw/read400/tread.cpp:1519-1521`). Les accords
internes gardent leur durée nominale (`1/8`) mais leur durée réelle est divisée
par le ratio (`1/12`).

`<endTuplet/>` est un **frère**, pas une balise fermante XML : il ferme le
tuplet courant et remonte au parent. Tuplets imbriqués : un `<Tuplet>` rencontré
alors qu'un autre est ouvert devient son enfant
(`rw/read400/measurerw.cpp:587-595`) ; chaque `endTuplet` ferme un niveau.
`endTuplet` sans ouverture : ignoré avec log. Tuplet vide : supprimé. Tuplet
non fermé non vide : laissé avec log (`:596-646`). Les `<Tuplet>` n'avancent
pas le tick : seuls les accords/repos le font, avec `actualTicks()` déjà divisé
par les ratios.

Lecteurs : `MeasureRead::readVoice` (`measurerw.cpp:587-612`),
`TRead::read(Tuplet*)` (`tread.cpp:1446`), `DurationElement::globalTicks`
(`dom/durationelement.cpp:87`). Mécanisme stable de 3.02 à 5.0.

## 9. `<location>`, `<tick>`, `<move>`
```xml
<voice>
  <location><fractions>1/4</fractions></location>
  <Chord>...</Chord>
</voice>
```
Champs : `<staves>`, `<voices>`, `<measures>`, `<fractions>`, `<grace>`,
`<notes>` (`TRead::read(Location*)`, `rw/read400/tread.cpp:2859-2880`).
`<measures>` = décalage en mesures (0 courante, 1 suivante, -1 précédente) ;
`<fractions>` = décalage en fraction de ronde relatif au début de la mesure
cible ; `<staves>`/`<voices>` = décalage de portée/voix, rare ; `<grace>` =
indice d'ornement ; `<notes>` = indice de note dans un accord.

`ReadContext::setLocation` (`rw/read400/readcontext.cpp:274-298`) fait
`Location::toAbsolute(location())` (addition de la position courante), puis
`setTrack`, `setTick(l.frac())`, et en lecture normale
`incTick(currentMeasure()->tick())` pour rendre la fraction absolue. Ainsi
`<fractions>1/4</fractions>` place la suite au quart de la mesure ;
`<measures>1</measures><fractions>-1/4</fractions>` au quart avant la fin de
la mesure suivante. Le déplacement persiste pour les accords suivants.

`<tick>1920</tick>` dans `<Staff>` ou `<voice>` fixe un tick **absolu** en
ticks de fichier :
`ctx.setTick(Fraction::fromTicks(ctx.fileDivision(e.readInt())))`
(`staffrw.cpp:89-90,121-122` ; `measurerw.cpp:229-231`). Ce n'est pas relatif à
la mesure ; la balise est obsolète et absente du corpus.

Aucune balise `<move>` n'est lue par les lecteurs 3.x/4.x/5.x.
`XmlReader::readFraction` mentionne `<move z="2" n="4"/>` et `<move>2/4</move>`
dans un commentaire (`rw/xmlreader.cpp:111-115`) : vestige de très vieux
formats. Un `<move>` serait ignoré par `e.unknown()`. Le déplacement se fait par
`<location>`.

**4.1+** : `<location>` peut porter `isTimeTick` et créer un ancrage de
métrique (`rw/read410/measureread.cpp:215-220`), sans effet sur le tick
musical. **5.0** : les spanners n'utilisent plus `<location>` (section 12).

## 10. Armure, métrique, clé
### 10.1 `<KeySig>`
```xml
<KeySig><accidental>2</accidental></KeySig>
<KeySig><concertKey>2</concertKey><actualKey>2</actualKey></KeySig>
```
`<accidental>` : entier `Key` de -7 à +7 (0 = do majeur / la mineur).
`<concertKey>` : armure de concert ; `setConcertKey` fixe aussi la tonalité
écrite (`dom/key.cpp:205-210`). `<actualKey>` : armure écrite, sinon = concert. `<subtype>` : ancien encodage, secours via `initFromSubtype`
(`dom/key.cpp:270`). `<custom>`, `<CustDef>`/`<KeySym>` : armures
personnalisées ; `<mode>`, `<showCourtesy>` : secondaires. Une armure en début de mesure est
réelle ; en fin de mesure, c'est une courtoisie annonçant la suivante.

Lecteur : `TRead::read(KeySig*)` (`rw/read400/tread.cpp:1189`) ; placement
`rw/read400/measurerw.cpp:418-429` ; `Staff::setKey` (`dom/staff.h:109`).
**3.02/4.0** : `<accidental>`. **4.1** : `<accidental>` ou
`concertKey`/`actualKey` (`rw/read410/tread.cpp:1331-1345`). **4.6+** :
`concertKey`/`actualKey` seuls (`rw/read460/tread.cpp:1318-1321`). **4.5+** :
courtoisie par `<isCourtesy>`, plus devinée par la position.
### 10.2 `<TimeSig>`
```xml
<TimeSig>
  <sigN>3</sigN><sigD>4</sigD>
  <stretchN>1</stretchN><stretchD>1</stretchD>
</TimeSig>
```
`<sigN>`/`<sigD>` : métrique. `<subtype>` : type graphique
(`dom/timesig.h:37-43`) : 0 normal, 1 common time, 2 alla breve, 3 cut Bach,
4 cut triple ; sans effet sur la durée. `<stretchN>`/`<stretchD>` : étirement
local ; divise la durée réelle (`actualTicks = globalTicks / timeStretch`).
`<textN>`/`<textD>`, `<Groups>` : affichage/groupement. `<isCourtesy>` :
courtoisie (4.5+). Une métrique en début de mesure fixe `timesig` et, si la
mesure n'est pas irrégulière, `ticks`. En fin de mesure, elle annonce la
suivante (`ctx.setTimeSigForNextMeasure`,
`rw/read410/measureread.cpp:381-384`).

Lecteur : `TRead::read(TimeSig*)` (`rw/read400/tread.cpp:3979`) ; placement
`rw/read400/measurerw.cpp:395-417`. **3.02/4.0** : courtoisie devinée par
`currTick > measure->tick()`. **4.1–4.4** : même devinette si
`mscVersion < 450` (`rw/read410/measureread.cpp:370`). **4.5+** :
`isCourtesy()` seul (`rw/read460/measureread.cpp:364`). Les anciens
`<den>`/`<nom1..4>` restent lus, hors périmètre 3.02+.
### 10.3 `<Clef>`
```xml
<Clef>
  <concertClefType>G</concertClefType>
  <transposingClefType>G</transposingClefType>
  <isHeader>1</isHeader>
</Clef>
```
`<concertClefType>`/`<transposingClefType>` : clés concert/transposée.
`<isHeader>` : clé d'en-tête (4.1+). `<showCourtesyClef>`, `<clefToBarlinePos>`,
`<forInstrumentChange>` : secondaires. Aucune clé valide → `G` forcé
(`rw/read400/tread.cpp:2551-2553`). Valeurs du corpus : `G`, `F`, `C3`, `G8vb`,
`G8vbp`, `PERC`, `TAB` ; table complète dans `TConv::fromXml` (généré, absent
de la copie). Début de mesure = en-tête ; ailleurs = changement en cours.
Avant 4.5, l'en-tête est deviné par la position et l'armure/métrique avant
(`rw/read400/measurerw.cpp:360-385`).

Lecteur : `TRead::read(Clef*)` (`rw/read400/tread.cpp:2527`) ; placement
`rw/read400/measurerw.cpp:353-394`. **3.02/4.0** : pas de `<isHeader>`,
devinette. **4.1–4.4** : `<isHeader>` avec repli
(`rw/read410/measureread.cpp:350-360`). **4.5+** : `<isHeader>` autoritaire,
plus de correction `AFTER` (`rw/read460/measureread.cpp:352-356`).

## 11. Liaisons de prolongation (`Tie`)
**Correction d'une idée reçue.** En 3.x, une liaison n'est pas un `<Tie>`
autonome : c'est déjà un `<Spanner type="Tie">` contenant `<Tie>` et une
extrémité `<next>`/`<prev>` (`testChordSymbols.mscx`, 3.01 ; `Read302` délègue
à `read400::TRead`, qui ne connaît pas de `<Tie>` autonome dans `Note`). Vraie
différence : **3.x/4.x** = deux demi-spanners avec `<location>` ; **5.0** = un
`<Tie>` direct dans la note, avec `<startElement>`/`<endElement>` (EID).
```xml
<Note><pitch>75</pitch><tpc>11</tpc>
  <Spanner type="Tie"><Tie/>
    <next><location><fractions>1/4</fractions></location></next>
  </Spanner></Note>
<Note>
  <Spanner type="Tie">
    <prev><location><fractions>-1/4</fractions></location></prev>
  </Spanner><pitch>75</pitch><tpc>11</tpc></Note>
```
```xml
<!-- 5.0 -->
<Note>
  <eid>Z_Z</eid>
  <Tie>
    <startElement>S_S</startElement>
    <endElement>Z_Z</endElement>
    <eid>a_a</eid>
    <track>1</track>
  </Tie>
  <pitch>66</pitch><tpc>20</tpc>
</Note>
```
Une liaison ne consomme aucun tick et ne change pas la durée. Elle relie deux
notes de même hauteur ; l'export produit `<tie>` (son) et `<tied>` (notation).
En 3.x/4.x, un `ConnectorInfo` par demi-spanner, appariement par
type/piste/location, `tieFor` au départ et `tieBack` à l'arrivée
(`ConnectorInfoReader::readAddConnector(Note*)`,
`rw/read400/connectorinforeader.cpp:367-430`). En 5.0,
`TRead::readProperties(Spanner*)` enregistre les EID, puis
`ReadContext::connectNoteAnchoredSpanners` les résout en fin de lecture
(`rw/read500/tread.cpp:3880-3935` ; `rw/read500/readcontext.cpp:261-292`). Le
`<Tie>` 5.0 est placé dans la note de fin dans les fichiers de test, mais nomme
les deux extrémités : un seul élément suffit.

Lecteurs : `TRead::read(Tie*)` (`rw/read400/tread.cpp:3974`,
`rw/read500/tread.cpp:4411`) ; `ConnectorInfoReader::read`
(`rw/read400/connectorinforeader.cpp:73`) ; `Score::connectTies` en fin de
lecture (`rw/read302/read302.cpp:227`, `rw/read400/read400.cpp:297`).

## 12. Spanners génériques (`Slur`, `HairPin`, …)
Mécanisme 3.x/4.x :
```xml
<Spanner type="Slur">
  <Slur><SlurSegment no="0"/></Slur>
  <next><location><fractions>1/2</fractions></location></next>
</Spanner>
<!-- autre extrémité -->
<Spanner type="Slur">
  <prev><location><fractions>-1/2</fractions></location></prev>
</Spanner>
```
`type` = nom d'élément : `Slur`, `HairPin`, `Ottava`, `TextLine`, `Volta`,
`Trill`, `Pedal`, `LetRing`, `Vibrato`, `PalmMute`, `WhammyBar`, `Rasgueado`,
`HarmonicMark`, `GradualTempoChange`… L'enfant porte le même nom et ses
propriétés (`<subtype>`, `<color>`, `<SlurSegment>`…). L'extrémité
courante est implicite (position de l'élément conteneur) ; `<next>`/`<prev>`
donne l'autre via une `<location>` relative. Le lecteur apparie les deux
moitiés par type, piste et location absolue, puis crée un seul objet. Un
`<Spanner>` isolé peut être reconnecté en fin de score
(`ctx.reconnectBrokenConnectors()`) ou supprimé comme orphelin.

Mécanisme 5.0 :
```xml
<SpannerMap>
  <Slur>
    <startElement>N_N</startElement>
    <endElement>s_s</endElement>
    <eid>ZG_ZG</eid>
    <track>0</track>
  </Slur>
</SpannerMap>
```
`<SpannerMap>` est un enfant direct de `<Score>`. Chaque spanner est un élément
direct, sans `<Spanner type>` ni `<next>`/`<prev>`. `<startElement>` /
`<endElement>` sont des EID d'accords, repos ou mesures ; `<track>` donne la
piste. `TRead::readScoreSpanners` (`rw/read500/tread.cpp:617-632`) crée l'objet
via `tryCreateSpanner` (`:595-615`). Les spanners ancrés sur note (Tie,
Glissando, NoteLine) restent dans la note.

Lecteurs : `TRead::readSpanner` (`rw/read400/tread.cpp:4338`) ;
`ConnectorInfoReader::read`/`readAddConnector`
(`rw/read400/connectorinforeader.cpp:73-500`) ; `TRead::readScoreSpanners`
(`rw/read500/tread.cpp:617`). **3.02/4.0** : `<Spanner>` peut aussi apparaître
au niveau `<Score>` pour `HairPin`, `Ottava`, `TextLine`, `Volta`, `Trill`,
`Slur`, `Pedal`, lus par `TRead::readItem` (`rw/read400/read400.cpp:222-230`).
**5.0** : plus de `<Spanner>` au niveau voix/mesure ; tout passe par
`<SpannerMap>` sauf les spanners ancrés sur note.

## 13. `<Lyrics>` et ornements
### 13.1 `<Lyrics>`
```xml
<Chord>
  <durationType>quarter</durationType>
  <Lyrics>
    <no>1</no>
    <syllabic>begin</syllabic>
    <text>beau-</text>
  </Lyrics>
  <Note><pitch>72</pitch><tpc>15</tpc></Note>
</Chord>
```
Enfant du **Chord**, pas de la Note. `<no>` : ligne **0-based** (absent = 0,
`1` = 2e ligne). `<syllabic>` : `single`, `begin`, `end`, `middle`. `<text>` :
syllabe (peut contenir `<sym>`). `<ticks_f>` : mélisme, remplace `<ticks>`.
`<LyricsLine>` : trait de prolongation (5.0). `<placement>`/`<offset>` : hors
noyau.

Lecteurs : `TRead::read(Lyrics*)` (`rw/read400/tread.cpp:2882`) ;
`readProperties(Lyrics*)` (`:2926`) ; création depuis `<Chord>` (`:2488-2492`).
**3.02** : `<ticks>` obsolète encore lu. **4.0+** : `<ticks_f>`. **5.0** :
`<LyricsLine>` et `PartialLyricsLine`, plus de `<ticks>`.
### 13.2 Appoggiatures
```xml
<Chord>
  <durationType>eighth</durationType>
  <appoggiatura/>
  <Note><pitch>72</pitch><tpc>15</tpc></Note>
</Chord>
<Chord><durationType>quarter</durationType>
  <Note><pitch>74</pitch><tpc>16</tpc></Note></Chord>
```

| Balise | `NoteType` |
|---|---|
| `<appoggiatura/>` | `APPOGGIATURA` |
| `<acciaccatura/>` | `ACCIACCATURA` |
| `<grace4/>` | `GRACE4` |
| `<grace16/>` | `GRACE16` |
| `<grace32/>` | `GRACE32` |
| `<grace8after/>` | `GRACE8_AFTER` |
| `<grace16after/>` | `GRACE16_AFTER` |
| `<grace32after/>` | `GRACE32_AFTER` |

Un accord ornement est un `<Chord>` avec `NoteType` non `NORMAL`, placé
**avant** l'accord principal. `MeasureRead::readVoice` ne l'ajoute pas à un
segment et n'avance pas le tick : il le met dans `graceNotes`
(`measurerw.cpp:276-285`), puis le rattache à l'accord normal suivant avec
`graceIndex = 0,1,…` (`:280-284`). `<durationType>` d'un ornement est indicatif
(taille du crochet), pas une durée musicale. `grace8after` signifie « après la
note principale » ; le lecteur le stocke quand même dans la liste de l'accord
suivant, l'ordre étant résolu par `NoteType`. Un ornement sans accord normal
suivant reste non rattaché : corruption à signaler.

Lecteurs : `TRead::readProperties(Chord*)` (`tread.cpp:2349-2375`) ;
`MeasureRead::readVoice` (`measurerw.cpp:214,265-290`) ; `Chord::add`
(`dom/chord.cpp:686-694`). Mécanisme identique en 3.02/4.x/5.0 ; 5.0 ajoute des
EID.

## 14. Mesures irrégulières (`len`)
```xml
<Measure len="3/4">
  <voice>
    <Chord><durationType>half</durationType>...</Chord>
    <Chord><durationType>quarter</durationType>...</Chord>
  </voice>
</Measure>
```
`len` est un **attribut**, parsé par `Fraction::fromString` ; fixe
`measure->ticks(len)`. La métrique (`timesig`) reste héritée/déclarée. La
mesure est marquée `irregular` : une `<TimeSig>` lue dedans ne réécrit pas
`ticks` (`rw/read400/measurerw.cpp:414-416`). `len` invalide ou `< 1/128` :
erreur MSCX (`:90-101`). `len` peut être plus long ou plus court que la
métrique (3/4 dans 4/4, 8/4, 23/8…) ; le tick avance de `len`.

Lecteur : `MeasureRead::readMeasure` (`rw/read400/measurerw.cpp:89-101`).
**3.02/4.0** : ajoute `compatTimeSigMap` (`:99-100`). **4.1+** : ces entrées
disparaissent (`rw/read410/measureread.cpp:90-95`).

## 15. Excerpts et parties liées
Il n'y a **pas** de balise `<Excerpt>` dans un MSCX.

Score imbriqué (3.x/4.x) :
```xml
<Score>...
  <Staff id="1">...</Staff>
  <Score><name>Flute</name>
    <Tracklist sTrack="0" dstTrack="0"/>
    <Staff id="1">...</Staff></Score>
</Score>
```
`Read302/400::readScoreTag` traite `<Score>` par récursion : crée un `Score`
séparé, lit son contenu, puis `Excerpt::linkMeasures(s, m)` relie les mesures
au maître (`rw/read302/read302.cpp:143-164` ; `rw/read400/read400.cpp:233-257`).
`<name>` ne s'applique qu'au score enfant ; `<Tracklist>` mappe les pistes.

Fichiers séparés (4.x/5.x) : `MscLoader::loadMscz`
(`rw/mscloader.cpp:156-207`) liste `Excerpts/*.mscx`, lit chaque fichier avec
le même lecteur, puis `Excerpt::linkMeasures(partScore, masterScore)`. Le style
est dans `Excerpts/<nom>/<nom>.mss`.

Pour l'export : partition complète → ignorer les `<Score>` imbriqués et les
fichiers `Excerpts/` (le maître contient tout) ; parties → lire l'excerpt et
appliquer `<Tracklist>` (ou les EID 5.0), sans fusionner les mesures. 5.0 :
`<SharedPart>`/`<sharedPart>` (EID), hors périmètre complet. Piège : « tous
les `<Staff>` descendants » ramasse les excerpts et double la musique ;
s'arrêter aux enfants directs du `<Score>` maître.

## 16. Synthèse 3.02 → 5.0

| Sujet | 3.02 | 4.0 | 4.1 | 4.6 | 5.0 |
|---|---|---|---|---|---|
| Lecteur | `Read302` | `Read400` | `Read410` | `Read460` | `Read500` |
| `Part/@id` | non | non | non | non | oui |
| `Staff` déclaration | `id` | `id` | `id` | `id` | ordre, sans `id` |
| Noms instrument | `longName` | idem | idem | idem | `InstrumentLabel` |
| `KeySig` | `accidental` | `accidental` | les deux | `concertKey` | idem |
| Courtoisie | devinée | devinée | devinée si `<450` | `isCourtesy` | `isCourtesy` |
| Clé d'en-tête | devinée | devinée | `isHeader`+repli | `isHeader` | `isHeader` |
| Liaison | `Spanner` | `Spanner` | `Spanner` | `Spanner` | `<Tie>`+EID |
| Spanners | `Spanner` | `Spanner` | `Spanner` | `Spanner` | `SpannerMap` |
| `<eid>` | non | non | non | non | oui |
| Durées pré-1.12 | oui | oui | oui | oui | non |
| `BeamMode` ancien | traduit | traduit | direct | direct | direct |
| Excerpts | inline | inline/fichiers | fichiers | fichiers | +SharedPart |

## 17. Pièges
1. **Tick implicite.** Pas de `<tick>` par accord : il commence au tick courant
   et avance de `actualTicks()`.
2. **`actualTicks()` ≠ `ticks()`.** Diviser par les ratios d'olets et par
   `timeStretch` (section 3).
3. **`<dots>` avant `<durationType>`.** Les ticks sont calculés en lisant le
   type ; des points après sont ignorés. `<duration>` écrase tout : pour un
   repos `measure`, utiliser la longueur de la mesure, pas la valeur brute.
4. **Voix positionnelles et portées non partagées.** L'indice est l'ordre du
   `<voice>` ; ne pas le déduire du contenu, ne pas supprimer un `<voice>`
   vide. La portée 0 crée les mesures, les suivantes les réutilisent par
   ordre ; un décalage décale tout.
5. **`<tick>` absolu, `<location>` relatif.** `<tick>` est en ticks de fichier
   depuis le début ; `<fractions>` est relatif à la mesure cible. `<move>`
   n'existe pas en 3.02+ : utiliser `<location>`.
6. **Olets imbriqués.** Chaque `endTuplet` ferme un niveau ; le tick avance
   par accord déjà divisé par tous les ratios, ne pas ajouter
   `tuplet->ticks()`. Olet vide supprimé, non fermé non vide laissé avec log :
   signaler, ne pas inventer.
7. **Ornements sans durée.** `appoggiatura`/`acciaccatura`/`grace*` ne
   consomment aucun tick et se rattachent à l'accord suivant par `graceIndex`.
   Un ornement final sans accord suivant est corrompu.
8. **Liaison 3.x/4.x = deux moitiés** (`<next>` et `<prev>`). En 5.0, un
   `<Tie>` avec EID : ne pas chercher `<Spanner type="Tie">` en 5.0, ni
   `<Tie>` autonome en 3.x/4.x.
9. **Spanners 5.0 hors mesure.** Ils sont dans `<SpannerMap>` au niveau
   `<Score>` ; un parcours des seules voix les manque.
10. **Excerpts.** Ignorer pour l'export complet ; traiter séparément pour les
    parties. Ne pas ramasser les `<Staff>` des `<Score>` imbriqués.
11. **Armure 4.6+.** `<accidental>` n'est plus lu ; utiliser
    `concertKey`/`actualKey`. En 4.0/4.1, `<accidental>` est encore écrit.
12. **Courtoisie et clé d'en-tête avant 4.5.** Devinées par la position ;
    depuis 4.5, seuls `isCourtesy`/`isHeader` font foi (4.1 écrit déjà
    `<isHeader>`).
13. **Pitch sonnant, tpc écrit.** `<pitch>` est sonnant ; `tpc1` = concert,
    `tpc2` = écrit. Pour la hauteur écrite, soustraire la transposition.
    `setupAfterRead` ajuste `pitch` si `tpc1` est incohérent modulo 12.
14. **Instruments 3.6.** `Instrument/@id` peut être faux ; préférer
    `<instrumentId>`/`<trackName>` et `Read302::fixInstrumentId`.
15. **`<Division>`, formes et version.** `<Division>` ne sert qu'aux `<tick>` ;
    `1/1`, `4/4`, `8/8` peuvent être la même durée (comparer après réduction) ;
    `version="5.00"` → 500, ne pas comparer `"5.0"` à `"5.00"` ni déduire le
    schéma de `<programVersion>`. Une mesure sans `<voice>` garde sa durée :
    générer un repos de mesure à l'export.
