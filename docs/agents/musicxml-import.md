# Import MusicXML : état du portage et écarts mesurés

`crates/scoretk-musicxml` lit un document MusicXML `partwise`
(`.musicxml`, `.xml`, ou archive `.mxl`) et renvoie le modèle pivot de
`mscz2xml` (`mscz2xml::model::Score`). Le portage suit l'import de MuseScore
Studio, `src/importexport/musicxml/internal/import/`
(`importmusicxmlpass1.cpp`, `importmusicxmlpass2.cpp`,
`importmusicxmlnoteduration.cpp`, `importmusicxmlnotepitch.cpp`,
`musicxmlpart.cpp`, `musicxmltupletstate.cpp`, `musicxmlvoicedesc.cpp`).

## 1. Point d'entrée et organisation

```rust
use scoretk_musicxml::read_musicxml;
let score = read_musicxml(bytes)?;            // bytes = .musicxml, .xml ou .mxl
let xml = mscz2xml::export::export_musicxml(&score)?;
```

| Fichier | Rôle |
| --- | --- |
| `src/lib.rs` | `read_musicxml`, extraction du conteneur, passes 1 et 2, `gap_rests::fill` |
| `src/container.rs` | `.mxl` (ZIP) → fichier racine via `META-INF/container.xml` |
| `src/xml.rs` | arbre XML minimal (nom local, attributs dé-échappés, texte) |
| `src/pass1.rs` | structure : `part-list`, longueurs de mesures, voix, chevauchements |
| `src/pass2.rs` | modèle : segments, notes, n-olets, liaisons, barres, ornements |
| `src/noteduration.rs` | `<type>`/`<dot>`/`<duration>`/`<time-modification>` |
| `src/error.rs` | `Error` (XML, ZIP, racine inconnue, `part-list` absent) |

La bibliothèque reste portable : aucun accès disque, aucune horloge, aucun
thread ; elle compile pour `wasm32-unknown-unknown`. Le ZIP `.mxl` est décodé
en Rust pur (`zip` sans fonctionnalités par défaut).

## 2. Mesure de fidélité : test d'aller-retour

`crates/scoretk-musicxml/tests/aller_retour.rs` lit chaque
`tests/corpus/{v3,v4}/*.ref.musicxml`, le réexporte avec l'export de
`mscz2xml`, et compare les deux documents avec la normalisation « noyau
musical » de `crates/mscz2xml/tests/e2e_corpus.rs` (dupliquée dans
`tests/common/mod.rs`). Les deux niveaux de comparaison sont donc exactement
ceux de la conformité `mscx → MusicXML`.

```sh
nix develop -c cargo test -p scoretk-musicxml --test aller_retour -- --nocapture
```

Résultat au 2026-11-17 : **47/56 identiques** — **26/26 pour le corpus v4**,
**21/30 pour le corpus v3**. Les fichiers identiques sont listés dans
`EXPECTED_V3` / `EXPECTED_V4` ; toute régression sur un fichier listé fait
échouer le test.

## 3. Ce qui est lu (noyau)

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
- liaisons de prolongation (`<tie>`, `<tied>`, orientation) ;
- paroles (`<lyric>` : couplet, syllabisation, placement, extension) ;
- ornements : accords d'agrément avant **et après** l'accord
  (`Chord::toGraceAfter`), appoggiature/acciaccatura ;
- reprises (`<repeat>` avant/arrière, `times`), silences multi-mesures
  (`<multiple-rest>`), mesures répétées (`<measure-repeat>`), tempos
  invisibles (`<sound tempo>` hors direction) ;
- silences de remplissage : trous de la première voix (`fillGapsInFirstVoices`
  de la passe 2) puis trous de toutes les voix par `gap_rests::fill`
  (`Measure::checkMeasure` de MuseScore) — c'est l'origine des
  `<forward>`/`<backup>` scindés de MuseScore.

## 4. Écarts restants (9 fichiers v3)

Ces écarts sont mesurés par le test d'aller-retour ; ils ne portent pas sur le
noyau musical déjà couvert.

| Fichier | Lignes | Cause |
| --- | ---: | --- |
| `v3/change` | 33 | changements d'instrument (`<instrument id>`, `<instruments>`) : non lus |
| `v3/testMultiInstrumentPart3` | 43 | idem, plus les transpositions par changement d'instrument |
| `v3/layout_elements` | 91 | changements d'instrument (45) et chiffrage figuré (`<figured-bass>`, 12) : non lus |
| `v3/testHarmony6` | 50 | harmonies (`<harmony>`) : non lues |
| `v3/testChordSymbols` | 15 | harmonies : non lues |
| `v3/timesig-05` | 13 | harmonies : non lues |
| `v3/voices` | 49 | harmonies (2) et **numérotation des voix** : MuseScore renumérote les voix MusicXML (1,2,3,5 → 1,2,3,4), le `.mscx` d'origine gardait 1,2,3,4 ; l'aller-retour par MusicXML perd cette numérotation |
| `v3/testTuplets8` | 31 | fichier de **n-olets incomplets** : MuseScore y stocke des durées réelles (`<duration>` du `.mscx`) que MusicXML ne transmet pas ; le `<type>` seul ne suffit pas à reconstituer 5/6 |
| `v3/barline01` | 6 | `<staff-details print-object="no"/>` de portées masquées : information de **mise en page** absente de MusicXML (MuseScore la recalcule) |

Deux de ces écarts sont **irréductibles** sans informations absentes du
format : la renumérotation des voix (MuseScore la fait aussi à l'import) et
les portées masquées (mise en page). Les autres sont des fonctions non encore
portées, par ordre d'intérêt :

1. **harmonies** (`<harmony>` : `root`, `kind`, `degree`, `bass`) — 3 fichiers.
   Le modèle attend le suffixe (`Harmony::name`) analysé à l'export par
   `export/harmony.rs` ; l'import doit reconstruire ce suffixe depuis
   `<kind text="…">`.
2. **changements d'instrument** (`<instrument>`, `<instruments>`) — 3 fichiers,
   via `Part::instrument_changes`.
3. **chiffrage figuré** (`<figured-bass>`) — `layout_elements`.
4. **fichiers à n-olets incomplets** : nécessiterait de déduire la durée réelle
   d'un n-olet non rempli, ce que le C++ fait par des corrections
   heuristiques (`MusicXmlTupletState::missingTupletDuration`).

## 5. Hors périmètre de cette première version

Les éléments suivants ne sont pas lus du tout (ils ne comptent pas dans les
écarts du § 4, la normalisation du noyau les retire) : `<direction>` (nuances,
soufflets, tempos visibles, textes, pédales, ottavas), `<notations>` autres que
`tied`/`tuplet` (articulations, ornements, liaisons de trait, trémolos de
notation), `<slur>`, `<credit>`, `<defaults>`/`<print>` (mise en page),
`<barline>` hors reprise. Les implémenter fera passer d'éventuels tests de
parité complète, pas le test d'aller-retour en niveau noyau.

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
