# Import MusicXML : état du portage et écarts mesurés

`crates/scoretk-musicxml` lit un document MusicXML `partwise`
(`.musicxml`, `.xml`, ou archive `.mxl`) et renvoie le modèle pivot de
`scoretk-core` (`scoretk_core::model::Score`). Le portage suit l'import de MuseScore
Studio, `src/importexport/musicxml/internal/import/`
(`importmusicxmlpass1.cpp`, `importmusicxmlpass2.cpp`,
`importmusicxmlnoteduration.cpp`, `importmusicxmlnotepitch.cpp`,
`musicxmlpart.cpp`, `musicxmltupletstate.cpp`, `musicxmlvoicedesc.cpp`).

## 1. Point d'entrée et organisation

```rust
use scoretk_musicxml::{read_musicxml, read_mxl};
let score = read_musicxml(bytes)?;            // bytes = .musicxml, .xml ou .mxl
let score = read_mxl(bytes)?;                 // bytes = archive .mxl uniquement
let xml = scoretk_core::export::export_musicxml(&score)?;
```

| Fichier | Rôle |
| --- | --- |
| `src/lib.rs` | `read_musicxml`, `read_mxl`, passes 1 et 2, `gap_rests::fill` |
| `src/container.rs` | choix entre document XML nu et archive (`.mxl`) |
| `src/mxl.rs` | `.mxl` (ZIP) → fichier racine via `META-INF/container.xml` |
| `src/xml.rs` | arbre XML minimal (nom local, attributs dé-échappés, texte) |
| `src/pass1.rs` | structure : `part-list`, longueurs de mesures, voix, chevauchements |
| `src/pass2.rs` | modèle : segments, notes, n-olets, liaisons, barres, ornements |
| `src/direction.rs` | `<direction>` : nuances, soufflets, textes, tempos, reprises |
| `src/barline.rs` | `<barline>` : styles, `<repeat>`, voltas, points d'orgue de barre |
| `src/harmony.rs` | `<harmony>` et `<figured-bass>` |
| `src/notations.rs` | articulations, ornements, fermates, arpèges, slurs, glissandos, trilles, doigtés |
| `src/credits.rs` | `<credit>` : titre, sous-titre, compositeur, droits |
| `src/noteduration.rs` | `<type>`/`<dot>`/`<duration>`/`<time-modification>` |
| `src/error.rs` | `Error` (XML, ZIP, racine inconnue, `part-list` absent) |

La bibliothèque reste portable : aucun accès disque, aucune horloge, aucun
thread ; elle compile pour `wasm32-unknown-unknown`. Le ZIP `.mxl` est décodé
en Rust pur (`zip` sans fonctionnalités par défaut).

### Conteneur `.mxl` (t43)

`src/mxl.rs` porte `extractRootfile` de `importmusicxml.cpp` :

1. lire `META-INF/container.xml` et prendre le premier `<rootfile full-path>`
   (la descente `container` → `rootfiles` → `rootfile` du C++, avec tolérance
   d'un préfixe d'espace de noms) ;
2. lire l'entrée ainsi désignée ; absente → `Error::RootFileNotFound`.

Deux écarts au C++, volontaires :

- **repli** quand `container.xml` manque : premier `.musicxml`/`.xml` hors
  `META-INF`, dans l'ordre de l'archive (sinon `Error::NoMusicXmlInMxl`) ;
  MuseScore, lui, échoue avec un fichier racine vide ;
- **erreurs explicites** au lieu d'un import silencieusement vide pour une
  entrée absente.

`read_mxl` refuse un document MusicXML nu (`Error::Zip`) : c'est
`read_musicxml` qui accepte les deux. Côté façade, `Format::Mxl` est lu avec la
feature `musicxml-import` (`hooks::read_mxl`), et la détection par contenu
reconnaît un ZIP contenant `META-INF/container.xml` **après** avoir écarté un
`.mscz` (un `.mscz` contient aussi un `container.xml`).

## 2. Mesure de fidélité : test d'aller-retour

`crates/scoretk-musicxml/tests/aller_retour.rs` lit chaque
`tests/corpus/{v3,v4}/*.ref.musicxml`, le réexporte avec l'export de
`scoretk-core`, et compare les deux documents avec la normalisation de
`crates/scoretk-core/tests/e2e_corpus.rs`, aux **deux niveaux** :

- **noyau musical** (`Level::Core`) : niveau bloquant, identique à la
  conformité `mscx → MusicXML` ;
- **parité complète** (`Level::Full`) : directions, articulations, spanners et
  crédits sont comparés aussi ; il mesure la couverture du lecteur et n'échoue
  que sur `EXPECTED_FULL_V3` / `EXPECTED_FULL_V4`.

```sh
nix develop -c cargo test -p scoretk-musicxml --test aller_retour -- --nocapture
```

Résultat au 2026-11-17 :

| Niveau | v3 | v4 |
| --- | ---: | ---: |
| noyau musical | **26/30** | **26/26** |
| parité complète | **23/30** | **21/26** |

Toute régression sur un fichier listé dans `EXPECTED_V3`, `EXPECTED_V4`,
`EXPECTED_FULL_V3` ou `EXPECTED_FULL_V4` fait échouer le test.

### Aller-retour `.mxl`

`crates/scoretk-musicxml/tests/mxl.rs` vérifie sur tout le corpus que
`.mscx` → `export_mxl` → `read_mxl` → export MusicXML est **identique octet
pour octet** au `.musicxml` équivalent exporté puis relu (détection du
conteneur, donc indépendamment du lecteur). Il couvre aussi le repli sans
`container.xml`, l'archive ZIP invalide, l'archive sans MusicXML et le
`rootfile` absent. Résultat : **56/56** fichiers identiques.

```sh
nix develop -c cargo test -p scoretk-musicxml --test mxl -- --nocapture
```

### Chaîne MusicXML → MIDI

`crates/scoretk-midi/tests/musicxml_midi.rs` lit les références
`tests/corpus/{v3,v4}/X.ref.musicxml`, écrit le MIDI avec `write_midi` et
compare notes (piste, canal, hauteur, début, fin, vélocité) et tempos à
`tests/corpus/midi/{v3,v4}/X.ref.mid`, comme `corpus_midi.rs`. Résultat :
**7/21 identiques** (`v4/dynamics`, `hairpin_simple`, `linked-ties`, `mmrest`,
`repeat68`, `repeat69`, `tenuto_accent`). C'est la vérification que les
nuances et tempos lus depuis `<direction>` produisent les bonnes vélocités.

## 3. Ce qui est lu

- parties (`part-list` : nom, abréviation, variantes `-display`,
  `score-instrument`, `instrument-sound`, canal/programme/volume/pan MIDI),
  `<part-group>` (accolades, crochets, barre commune) ;
- portées (`<staves>`, `<staff-details>/<staff-lines>`), transposition
  (`<transpose>`) ;
- mesures : longueur réelle (maximum sur les parties, correction 1/128),
  longueur nominale (indication de mesure, étirement local des indications
  par portée), saut de section déduit d'un numéro de mesure qui recule ;
- armures (`fifths`, `mode`, `print-object`), indications de mesure
  (`symbol="common"`/`"cut"`), clefs (y compris clefs octaviées et
  changements en fin de mesure précédente) ;
- accords, silences (mesure entière, invisibles), notes (hauteur, `tpc`,
  altération affichée et son rôle, tête et groupe de tête, y compris SMuFL) ;
- durées `<type>`/`<dot>`/`<duration>`, trémolos à deux notes (durée réelle
  moitié, `<time-modification>` 2:1) ;
- voix : `backup`/`forward`, attribution des voix MusicXML aux voix MuseScore
  (`allocateStaves`/`allocateVoices`), détection de chevauchement inter-portées
  (`VoiceOverlapDetector`) et `staffMove` ;
- n-olets : `<tuplet>` (crochet, nombre, placement), ratio du
  `<time-modification>` (combiné pour l'imbrication), durée de base lue dans
  `<tuplet-type>`, longueur `baseLen × normalNotes` comme
  `TRead::read(Tuplet*)` ;
- ligatures : `<beam>` (`begin`/`continue`/`end`/crochets), direction de hampe
  du groupe reprise du `<stem>` du premier accord ;
- liaisons de prolongation (`<tie>`, `<tied>`, orientation) ;
- paroles (`<lyric>` : couplet, syllabisation, placement, extension) ;
- ornements : accords d'agrément avant **et après** l'accord
  (`Chord::toGraceAfter`), appoggiature/acciaccatura ;
- directions (`<direction>`) : nuances `<dynamics>` (vélocité depuis
  `<sound dynamics>`), soufflets `<wedge>` (début/arrêt, niente, textes de
  début et de fin, y compris nuances insérées), `<words>` (texte de portée ou
  système, formats italique/gras/souligné/police), `<rehearsal>`, métronome
  `<metronome>` et `<sound tempo>` (tempos visibles et invisibles), `<offset>`,
  `<staff>`, pédales, ottavas, lignes `<dashes>`/`<bracket>`, segno/coda/fine
  et sauts D.C./D.S. ;
- notations (`<notations>`) : articulations, ornements (`<turn>`, mordants,
  `trill-mark`, `wavy-line`), fermates, arpèges (`arpeggiate`,
  `non-arpeggiate`), `<slur>` (numéros, orientation), `<slide>`/`<glissando>`,
  doigtés `<technical><fingering>`, lignes d'accord (`falloff`, `doit`,
  `plop`, `scoop`) et respirations/césures ;
- reprises (`<repeat>` avant/arrière, `times`), voltas (`<ending>`), silences
  multi-mesures (`<multiple-rest>`), mesures répétées (`<measure-repeat>`) ;
- harmonies (`<harmony>` : `root`, `kind`/`text`, `bass`, `numeral`,
  `function`, `degree`, `offset`) et chiffrage figuré (`<figured-bass>`) ;
- changements d'instrument (`<sound><instrument-change>`, `<transpose>` au
  tick du changement, texte `<words>` associé) ;
- crédits (`<credit>` : plusieurs `<credit-words>`, type, page, alignements,
  police), copyright déduit de `<rights>` ou du crédit `rights` ;
- silences de remplissage : trous de la première voix (`fillGapsInFirstVoices`
  de la passe 2) puis trous de toutes les voix par `gap_rests::fill`
  (`Measure::checkMeasure` de MuseScore) — c'est l'origine des
  `<forward>`/`<backup>` scindés de MuseScore.

## 4. Écarts restants au niveau noyau (4 fichiers v3, 0 en v4)

| Fichier | Lignes | Cause |
| --- | ---: | --- |
| `v3/barline01` | 6 | `<staff-details print-object="no"/>` de portées masquées : information de **mise en page** absente de MusicXML (MuseScore la recalcule) |
| `v3/layout_elements` | 26 | harmonies rattachées à une autre voix que MuseScore (ordre des `<direction>`/`<harmony>` après `backup`), bend `<technical><bend>` non modélisé |
| `v3/testTuplets8` | 31 | fichier de **n-olets incomplets** : MuseScore insère des silences de complément (`MusicXmlTupletState::missingTupletDuration`) que `<type>` seul ne permet pas de reconstituer ; `<normal-type>` en trop et `<duration>` calculées |
| `v3/voices` | 42 | **numérotation des voix** : MuseScore renumérote les voix MusicXML (1,2,3,5 → 1,2,3,4), le `.mscx` d'origine gardait 1,2,3,4 ; l'aller-retour par MusicXML perd cette numérotation |

Deux écarts sont **irréductibles** sans informations absentes du format : les
portées masquées (mise en page) et la renumérotation des voix (MuseScore la
fait aussi à l'import). Les deux autres sont des fonctions non portées.

## 5. Écarts restants au niveau complet (7 fichiers v3, 5 en v4)

v3 : `barline01`, `layout_elements`, `moonlight` (6 `<stem>` de ligatures
croisées, 3 `<credit-words>` multi-lignes), `testTuplets8`, `testWedge4`
(soufflets simultanés numérotés 1/2), `voices` (voix), et les fichiers
ci-dessus.

v4 : `expression-1` (`font-style="normal"` de réinitialisation entre deux
`<words>`), `repeat65`, `repeat66`, `repeat68` (`<words></words>` vide d'un
cadre sans texte, `font-family`/`font-size` explicites égaux au style),
`voiceswitching` (paroles multi-blocs, hampes).

Ne sont pas lus (ou sans équivalent dans le modèle) : `<technical><bend>` et
autres techniques non listées, `<harmony><frame>` (diagrammes de manche),
formats de `<lyric>` étendus, `<defaults>`/`<print>` (mise en page),
`<barline>` `ending` multiples sur une même mesure, `<sound><swing>`
(partiellement).

## 6. Pièges rencontrés

- **`Fraction` toujours réduite** : la correction de longueur de mesure du C++
  (`measureDurationAsFraction`) réécrit `4/4` en `8/8` sans changer la valeur ;
  avec une fraction réduite, la boucle du C++ ne termine pas. L'import
  conserve donc la valeur et documente l'écart de représentation.
- **N-olets imbriqués** : le `<time-modification>` donne le ratio *combiné* ;
  le ratio interne est le quotient par les ratios parents. La longueur se
  calcule comme `baseLen × normalNotes`, pas comme l'étendue des enfants.
- **Voix chevauchantes** : une même voix MusicXML peut changer de portée ; le
  `VoiceOverlapDetector` de MuseScore marque alors la voix comme chevauchante,
  ce qui change l'attribution des voix et le `staffMove`.
- **Silences de remplissage** : sans `Measure::checkMeasure`, les
  `<forward>`/`<backup>` ne correspondent pas et des notes manquent.
- **Ornements après l'accord** : un ornement portant `<slur type="stop">` sans
  départ est un ornement *après* (`mustStopGraceAfter`) ; son type de note
  devient `GRACE8_AFTER`/`GRACE16_AFTER`/`GRACE32_AFTER` à l'attachement.
- **Ottava** : les hauteurs MusicXML sont **écrites** (décalées par
  `octave-shift`) ; l'import retire le décalage (`xmlSetPitch`, `pitch += 12 ×
  octaveShift`) pour stocker la hauteur sonore, l'export le réapplique.
- **Transposition** : un `<transpose>` qui suit un changement d'instrument
  appartient à ce changement ; la hauteur de chaque note se calcule avec
  l'instrument en vigueur à **son tick** (`Part::instrument_at`), pas avec le
  dernier `<transpose>` lu.
- **Direction de hampe d'une ligature** : MusicXML ne porte pas la direction
  du groupe ; elle se reprend du `<stem>` du premier accord (`beam_direction`),
  sinon l'export recalcule et se trompe sur les ligatures croisées.
- **Groupes `<notations>` multiples** : une même note peut avoir plusieurs
  `<notations>` (tied invisible puis fingering visible) ; il faut les parcourir
  tous, et `print-object="no"` s'hérite sur chacun de leurs enfants.
- **Ordre des annotations** : les harmonies, directions et spanners sont
  écrits dans l'ordre du fichier ; l'ordre des voix traitées par l'export peut
  déplacer une annotation d'une voix à l'autre (`v3/layout_elements`).
