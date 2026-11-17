# Écriture MIDI (`scoretk-midi`)

Écriture d'un **Standard MIDI File format 1** à partir du modèle de
`scoretk-core`. C'est le portage de l'export MIDI de MuseScore Studio : la
partition est rendue en performance (reprises déroulées, notes, vélocités,
tempo), puis encodée.

## API

```rust
use scoretk_core::read;
use scoretk_core::MsczContainer;
use scoretk_midi::write_midi;

let container = MsczContainer::from_bytes(&mscx)?;
let score = read::read_score(&container)?;
let midi: Vec<u8> = write_midi(&score)?;
```

- `write_midi(&Score) -> Result<Vec<u8>, Error>` : réglages par défaut de
  MuseScore (reprises déroulées).
- `write_midi_with(&Score, MidiOptions)` : `expand_repeats: false` produit un
  fichier « à plat », sans dérouler les reprises (`RepeatList::flatten`).
  L'option est exposée par la façade (`Options::midi.expand_repeats`) et par
  la CLI (`--no-expand-repeats`).
- `write_midi_with_timemap(&Score)` et
  `write_midi_with_timemap_and(&Score, MidiOptions)` renvoient en plus la
  carte des temps du même rendu : `(Vec<u8>, Timemap)`. Les octets MIDI sont
  identiques à ceux de `write_midi`/`write_midi_with`.
- `RepeatList` / `RepeatSegment` sont publics : le déroulement des reprises,
  voltas et sauts est réutilisable par un futur lecteur MIDI ou un export
  LilyPond/ABC.
- `Timemap` (`timemap.rs`) : `division`, `ticks`, `tempo`, `measures`, `notes`,
  `pauses`, `time_us_at`, `measure_at`. Les identifiants `xml_id()` viennent de
  `scoretk_core::model::xmlid`, comme l'export MusicXML. Structure complète,
  sémantique des passages et limites : `docs/agents/timemap.md`.

La crate est portable comme `scoretk-core` : aucun accès disque, aucun thread,
aucune horloge ; elle compile pour `wasm32-unknown-unknown`. Seule dépendance
tierce ajoutée : `midly` (`no_std`, `alloc`), sous licence Unlicense, pour le
format de fichier.

## Modules

| Module | Rôle | C++ amont |
| --- | --- | --- |
| `lib.rs` | point d'entrée et options | `exportmidi.cpp` (`ExportMidi::write`) |
| `repeat_list.rs` | déroulement reprises/voltas/sauts | `engraving/dom/repeatlist.cpp` |
| `render.rs` | partition → évènements datés | `engraving/compat/midi/compatmidirender*.cpp` |
| `velocity.rs` | vélocités des nuances et soufflets | `engraving/compat/midi/velocitymap.cpp` |
| `pause_map.rs` | pauses de section (ticks + tempo) | `engraving/compat/midi/pausemap.cpp` |
| `writer.rs` | assemblage SMF via `midly` | écriture `MidiFile` |
| `timemap.rs` | carte ticks ↔ temps, mesures, notes, pauses | sous-produit du rendu (pas de C++ dédié) |

Division : **480 ticks par noire** (`Constants::DIVISION`), comme MuseScore. Les
ticks internes du modèle (`Fraction` de ronde) sont convertis en
`1920 × fraction`, avec la même troncature entière que `Fraction::ticks()`.

## Rendu : ce qui est porté

- **Une piste MIDI par portée**, avec le nom de la partie ; canal, programme,
  volume et panoramique depuis l'instrument (volume 100, panoramique 64 par
  défaut) ; contrôleurs initiaux (reset, RPN de bend, programme, volume,
  panoramique, réverbération, chœur) sur la première portée de chaque partie.
- **Reprises** : `RepeatList::unwind` (reprises simples et `times`, voltas avec
  endings, D.C./D.S., cibles `start`/`end`/`segno`/`coda`/`fine`/`codab`, sauts
  de section). Une mesure après la fin d'une reprise reste attachée au dernier
  segment, comme dans le C++.
- **Sauts de section** : une pause de 3 s (`Sid::sectionPause`) insère une
  mesure de ticks et deux changements de tempo, conformément à `PauseMap`.
- **Tempo** : tempo par défaut 2 battements/s (500 000 µs) puis indications
  `<Tempo>`, déroulées par segment ; l'indication de mesure et l'armure sont
  écrites en méta-évènements, également déroulées.
- **Vélocités** : `VelocityMap` complet (nuances, soufflets, collisions,
  interpolation), valeur par défaut 80.
- **Notes** : hauteur concert, vélocité issue de la carte des nuances et des
  soufflets, × multiplicateurs d'articulations (0,6 pour une note fantôme),
  puis **`Note::customizeVelocity` par note** : en mode `offset`
  (`<veloType>offset</veloType>`) la vélocité calculée est augmentée de
  `userVelocity` pour cent (`velo + velo * pct / 100`) ; en mode `user` (valeur
  par défaut) `<velocity>` est la vélocité absolue ; le résultat est borné à
  1–127 (`compatmidirenderinternal.cpp:227-229`,
  `dom/note.cpp:2753-2760`). Durée effective divisée par `timeStretch` et les
  ratios de n-olets (`DurationElement::actualTicks`), note-off à
  `début + durée - 1`.
- **Liaisons de prolongation fusionnées** : une seule attaque, la durée somme
  les notes liées en tenant compte de leurs propres ornements après
  (`calculateTieLength`).
- **Notes d'ornement** : appoggiatures avant (au plus 50 % de l'accord),
  acciaccaturas avant (au plus la moitié de l'accord précédent), ornements
  après (50 %, 66,7 % ou 57,1 % selon les points).
- **Articulations** : vélocité et durée de tenue depuis la table MIDI de
  l'instrument (`<Instrument><Articulation>`, lue dans le modèle) ou, à
  défaut, les valeurs intégrées de MuseScore (`staccato` 50 %,
  `staccatissimo` 30 %, `portato`, `tenuto`, `accent` 1,2, `marcato` 1,44,
  `sforzato` 1,69). Sous une liaison d'expression, la durée par défaut reste
  100 %.
- **Trémolos** : trémolo sur une note (répétition à la figure du trémolo) et
  trémolo à deux notes (alternance des deux accords) ; la version non mesurée
  est ignorée, comme dans MuseScore (`buzz roll` non rendu en MIDI).

## Écarts et limites connus

Mesurés sur le sous-ensemble de référence (`tests/corpus/midi/`) : **21/21
fichiers identiques** (notes et changements de tempo effectifs). Les points
suivants restent hors périmètre, faute d'information dans le modèle :

**Écart connu de déroulement.** Au `RepeatEnd`, le C++ ferme **toute** volta
encore ouverte (`ref-musescore/src/engraving/dom/repeatlist.cpp:581-588`, idem
sur un saut, `:525-543`) ; le portage ne la ferme que si elle se termine là
(`crates/scoretk-midi/src/repeat_list.rs:739-752`). Une volta débordant
l'`endRepeat` peut donc différer de MuseScore ; non déclenché par le corpus
(`v3/repeat06`, `v4/repeat65/66/68/69` conformes), gardé par les 21 références
de `corpus_midi`. Correctif minimal : fermer la volta à la mesure du
`RepeatEnd`, puis `cargo test -p scoretk-midi --test corpus_midi`.

- **Pauses de fermatas et respirations** : la carte des pauses ne traite que
  les sauts de section (pause par défaut de 3 s). Une fermata jouée ou une
  respiration décalerait la suite du fichier.
- **Dynamique progressive intégrée aux nuances** (`fp`, `sfz`…) : la baisse de
  vélocité après l'attaque n'est pas modélisée ; la vélocité initiale, elle,
  est juste.
- **Nuances rendues en CC** (`singleNoteDynamics`) : les contrôleurs continus
  (CC2/CC11) ne sont pas émis ; seules les vélocités d'attaque le sont.
- **Soufflets avec `<veloChange>` explicite** : la valeur est ignorée (le
  modèle ne la stocke pas) ; les soufflets du corpus s'en remettent à la
  nuance suivante.
- **`Note::velocity` (import MIDI) non appliquée** : le rendu MIDI utilise les
  vélocités des nuances et des soufflets, puis la vélocité utilisateur
  MuseScore (`Note::user_velocity`, `<velocity>`/`<veloType>` du `.mscx`), pas
  la vélocité brute de l'import MIDI, qui sert à l'export MusicXML. La carte
  des temps ne porte pas de vélocité.
- **Trémolo à deux accords** : dans `compatmidirender.cpp:364-440`, les
  évènements de la seconde hauteur sont placés dans la liste du **premier**
  accord et `playNote` reçoit la note du premier accord. Seule la vélocité de
  la **première note du premier accord** pilote les deux attaques ; la seconde
  n'est jamais lue. Le portage est fidèle à ce comportement.
- **Changements d'instrument, paroles, repères de répétition, pédales** : non
  exportés (`exportmidi.cpp` écrit aussi des méta-évènements de paroles et de
  marqueurs).
- **Instruments à percussion** : pas de table de batterie (`<Drumset>` non
  modélisé) ni de canal 9 automatique.
- **`playRepeats` d'un saut** : non modélisé, traité comme faux (valeur par
  défaut de MuseScore et des fichiers du corpus).
- **Mesures de multi-repos** : le contenu recopié par le lecteur est joué ; la
  logique `MeasureRepeat` de `renderStaff` n'est pas rejouée.
- **Fichiers à staves liées** (`linked-ties`) : le modèle ne duplique pas les
  portées liées ; l'export suit le contenu réellement modélisé.

## Références et test de conformité

`scripts/gen-midi-references.sh` exporte un sous-ensemble court du corpus avec
MuseScore **4.7.4** (headless, `QT_QPA_PLATFORM=offscreen`) dans
`tests/corpus/midi/{v3,v4}/<nom>.ref.mid`. Le test
`crates/scoretk-midi/tests/corpus_midi.rs` lit la source `.mscx`, écrit le MIDI
et compare après normalisation :

- notes : piste, canal, hauteur, tick de début, tick de fin, vélocité ;
  l'ordre des évènements simultanés est neutralisé et les attaques/extinctions
  sont appariées ;
- tempo : changements effectifs seulement (MuseScore réécrit parfois le tempo
  en vigueur à une frontière de reprise).

Le test affiche `MIDI : N/M identiques` et n'échoue que sur
`EXPECTED_IDENTICAL`, qui ne doit pas régresser.

```sh
scripts/gen-midi-references.sh            # toutes les références du sous-ensemble
scripts/gen-midi-references.sh tests/corpus/v4/dynamics.mscx
nix develop -c cargo test -p scoretk-midi --test corpus_midi -- --nocapture
```

Ne pas régénérer avec une autre version de MuseScore sans décision explicite :
la cible de conformité changerait en silence (même règle que les références
MusicXML, voir `upstream.md`).

## Chaîne MusicXML → MIDI

`crates/scoretk-midi/tests/musicxml_midi.rs` vérifie la chaîne complète
MusicXML → modèle (`scoretk_musicxml::read_musicxml`) → MIDI : pour chaque
`tests/corpus/{v3,v4}/X.ref.musicxml`, il écrit le MIDI et le compare à
`tests/corpus/midi/{v3,v4}/X.ref.mid` avec la même normalisation que
`corpus_midi.rs` (notes et tempos effectifs). C'est la garantie que les
nuances et tempos lus depuis `<direction>` produisent les vélocités et
changements de tempo de MuseScore.

```sh
nix develop -c cargo test -p scoretk-midi --test musicxml_midi -- --nocapture
```

Résultat au 2026-11-17 : **7/21 identiques** (`v4/dynamics`,
`hairpin_simple`, `linked-ties`, `mmrest`, `repeat68`, `repeat69`,
`tenuto_accent`). Les écarts restants viennent de la durée de tenue
(`gateTime`) des instruments, que MusicXML ne transmet pas, et des
articulations non lues pour certains fichiers.


## Lecture MIDI (import, t42)

La crate expose la lecture d'un Standard MIDI File vers le modèle pivot :

```rust
let score = scoretk_midi::read_midi(&bytes)?;          // options de MuseScore
let score = scoretk_midi::read_midi_with(&bytes, &options)?; // options explicites
```

La façade `scoretk` la branche avec la feature `midi` :
`read(bytes, Format::Midi)` et `scoretk entrée.mid -o sortie.musicxml`
fonctionnent. Le paquet wasm de la façade les expose aussi.

### Modules

Portage de `importexport/midi/internal/midiimport/` (mêmes noms et découpage
que le C++, référence amont dans chaque en-tête) :

| Rust (`crates/scoretk-midi/src/import/`) | C++ amont |
| --- | --- |
| `mod.rs` | `importmidi.cpp` (`convertMidi`, `createMTrackList`, `createMeasures`, `createNotes`, `processMeta`) |
| `midifile.rs` | `midishared/midifile.cpp`, `midievent.h` (lecture SMF par `midly`, canaux, fusion note-on/off) |
| `timesigmap.rs` | `engraving/dom/sig.cpp` (`TimeSigMap`) |
| `chord.rs` | `importmidi_chord.cpp` (`MChord`) |
| `quant.rs` | `importmidi_quant.cpp` (`Quantize`) |
| `meter.rs` | `importmidi_meter.cpp`, `engraving/dom/durationtype.cpp` (`toDurationList`) |
| `simplify.rs` | `importmidi_simplify.cpp` (`lengthenNote`, `minimizeNumberOfRests`) |
| `tuplet.rs` | `importmidi_tuplet*.cpp` (détection simplifiée, voir limites) |
| `voice.rs` | `importmidi_voice.cpp` (séparation par chevauchement, tri par hauteur) |
| `key.rs` | `importmidi_key.cpp`, `engraving/dom/keylist.h` |
| `clef.rs` | `importmidi_clef.cpp` (clef principale seulement) |
| `lrhand.rs` | `importmidi_lrhand.cpp` (séparation des mains simplifiée) |
| `drum.rs` | `importmidi_drum.cpp` |
| `lyrics.rs` | `importmidi_lyrics.cpp` |
| `tempo.rs` | `importmidi_tempo.cpp` |
| `instrument.rs` (+ `instrument/gm_names.rs`) | `importmidi_instrument.cpp`, `midishared/generalmidi.cpp` |
| `operations.rs` | `importmidi_operations.h` (valeurs par défaut) |

Le pipeline suit `convertMidi` : pistes et indications de mesure, accords
(`collectChords`), fusion et découpe, séparation main gauche/droite, percussion,
quantification (attaques puis fins), suppression des chevauchements,
simplification des durées, séparation des voix, mesures, méta-évènements,
armures, notes et silences, n-olets du modèle, clefs, indications de mesure,
paroles, tempo.

Options par défaut (aucun panneau d'options) : quantification à la **double
croche** (`QuantValue::Q16`, `defaultQuantValueFromPreferences`), recherche des
3/5/7/9-olets, points de prolongation, staccato, quatre voix, séparation des
mains d'un piano, changement de clef, texte de tempo, anacrouse recherchée.
Comme MuseScore sur une performance déjà quantifiée, les attaques restent sur
la grille ; une performance humaine est détectée (moins de 60 % d'attaques sur
un temps) et quantifiée à la croche, sans détection de battements.

L'import ajoute `Note::velocity` au modèle (vélocité MIDI brute) ; l'export
MusicXML en tire l'attribut `dynamics` de la note (`velo * 100 / 90`, échelle de
MuseScore).

### Limites mesurées

- **N-olets** : la détection ne traite que les n-olets simples dont la grille
  `longueur / n` n'est pas binaire (3, 5, 7, 9), sur les divisions métriques de
  la mesure, sans note étrangère entre deux attaques. Les n-olets imbriqués,
  chevauchés par une autre voix ou d'une grille binaire (duolets, quadruolets)
  ne sont pas détectés : `v3/testTuplets8` et `v4/timesig-09` restent
  partiellement différents des références (voir ci-dessous).
- **Catalogue d'instruments** : les noms viennent de la table General MIDI
  (`getMidiName`) et non de la recherche dans le catalogue MuseScore
  (`findSuitableInstruments`). Un programme 0 s'appelle « Grand Piano » au lieu
  de « Piano », un programme 25 « Steel Gtr. » au lieu de « Acoustic Guitar »,
  et les noms courts (`Pno.`, `Vln.`, `Guit.`) ne sont pas produits. C'est la
  cause de la plupart des écarts de 1 à 7 lignes du test d'import.
- **Clefs** : seule la clef principale (hauteur moyenne, seuil 60) est posée ;
  la programmation dynamique des changements de clef n'est pas portée, et les
  clefs octaviées du catalogue (guitare, contrebasse) ne sont pas déduites.
- **Séparation des mains** : la décision de scinder est celle du C++
  (accord dépassant l'octave, hors grand staff déjà présent) ; la répartition
  des notes se fait par plus grand écart de hauteur, sans la programmation
  dynamique de `LRHand::findSplits`.
- **Voix** : répartition par chevauchement puis tri par hauteur moyenne ; la
  découpe d'accords et de n-olets entre voix de `MidiVoice` n'est pas portée.
- **Percussion** : la table de batterie n'est pas embarquée ; seules les
  hauteurs de voix 1 du kit standard (35, 36, 44) sont connues, et la découpe
  par hauteur (`splitDrumTracks`) n'est pas exercée (`doStaffSplit` faux).
- **Performances humaines** : pas de `MidiBeat` (battements, tempo moyen) ;
  seuls les tempos explicites sont posés.
- **Anacrouse** : seule la première mesure plus courte que la suivante est
  reconnue (`isPickupWithLessTimeSig`) ; le cas « première mesure plus longue »
  n'est pas traité.
- **Accords inégaux, ornements, tremolos, swing, noms d'accords** : les modules
  `importmidi_chordname`, `importmidi_swing`, `importmidi_beat`,
  `importmidi_tie`, `importmidi_operations` (interface) ne sont pas portés.

### Références et conformité

`scripts/gen-midi-import-references.sh` fait importer chaque
`tests/corpus/midi/{v3,v4}/<nom>.ref.mid` par MuseScore **4.7.4** (headless,
`QT_QPA_PLATFORM=offscreen`, `LD_LIBRARY_PATH` neutralisé) et exporte le
résultat en `tests/corpus/midi/import/<version>/<nom>.ref.musicxml`.
`crates/scoretk-midi/tests/import_corpus.rs` lit le MIDI avec `read_midi`,
exporte en MusicXML et compare le **noyau musical** avec la même normalisation
que `crates/scoretk-core/tests/e2e_corpus.rs` (`tests/core_norm.rs`, avec en plus
la normalisation des `<divisions>` : la valeur est une convention d'encodage,
les `<duration>` sont ramenées à 480 par noire).

Résultat mesuré au 2026-11-17 : **4/21 identiques** (`v4/mmrest`,
`v4/repeat68`, `v4/repeat69`, `v4/testTempoPrecision`, tous des fichiers sans
note ou presque). Les autres fichiers diffèrent de 1 à 7 lignes (noms de
parties et abréviations, clef octaviée) sauf `v3/testTuplets8` (224 lignes) et
`v4/timesig-09` (965 lignes), à cause des n-olets complexes. La liste
`EXPECTED_IDENTICAL` ne doit pas régresser.

L'aller-retour `.mscx` → MIDI → `read_midi` → MIDI est vérifié par
`crates/scoretk-midi/tests/roundtrip_midi.rs` sur les 21 sources : **21/21**
avec les mêmes hauteurs, des attaques à ±1/16 de noire et des durées à ±1/16,
sauf notes plus courtes que la grille, que la simplification des n-olets peut
allonger (documenté dans le test).

```sh
scripts/gen-midi-import-references.sh            # toutes les références
scripts/gen-midi-import-references.sh tests/corpus/midi/v4/dynamics.ref.mid
nix develop -c cargo test -p scoretk-midi --test import_corpus -- --nocapture
nix develop -c cargo test -p scoretk-midi --test roundtrip_midi -- --nocapture
```

## Mise à niveau de l'import MIDI de MuseScore

Même référence amont que le reste du portage (voir `docs/agents/upstream.md`) :
commit `2b9e82df847111d673c2051a93deeaaeea3f5a29` (2026-11-16, version 5.0.0 de
développement), copie locale `ref-musescore/` **non versionnée**. Le
comportement de référence reste **MuseScore 4.7.4** pour les sorties
d'import : en cas de divergence, c'est la 4.7.4 qui fait foi.

Où récupérer les sources (hors dépôt) :

```sh
git -C ref-musescore fetch origin
git -C ref-musescore checkout 2b9e82df847111d673c2051a93deeaaeea3f5a29
```

Fichiers à surveiller (sous `ref-musescore/src/`) :

- `importexport/midi/internal/midiimport/importmidi.cpp` (pipeline),
  `importmidi_chord.cpp`, `importmidi_quant.cpp`, `importmidi_simplify.cpp`,
  `importmidi_meter.cpp`, `importmidi_tuplet*.cpp` (detect/filter/voice/tonotes),
  `importmidi_voice.cpp`, `importmidi_key.cpp`, `importmidi_clef.cpp`,
  `importmidi_lrhand.cpp`, `importmidi_drum.cpp`, `importmidi_lyrics.cpp`,
  `importmidi_tempo.cpp`, `importmidi_instrument.cpp`,
  `importmidi_operations.{h,cpp}` (valeurs par défaut) ;
- `importexport/midi/internal/midishared/{midifile,midievent,generalmidi}.cpp`
  et `.h` ;
- `engraving/dom/{sig,durationtype}.cpp` (carte des mesures, `toDurationList`).

Correspondance C++ → Rust : tableau « Modules » ci-dessus. Pour repérer les
changements pertinents :

```sh
git -C ref-musescore log --oneline <ancien>..<nouveau> -- \
  src/importexport/midi/internal/midiimport \
  src/importexport/midi/internal/midishared \
  src/engraving/dom/sig.cpp src/engraving/dom/durationtype.cpp
git -C ref-musescore diff <ancien>..<nouveau> -- \
  src/importexport/midi/internal/midiimport/importmidi_quant.cpp
```

Trier : les changements d'interface (panneau, options) ne concernent pas la
bibliothèque ; les changements d'algorithme de quantification, de n-olets, de
simplification et de tempo, oui. Les tables générées (`typesconv.cpp`,
`generalmidi.cpp`) sont à revérifier à la main (`getMidiName`,
`isGrandStaffProgram`).

Régénérer et revalider :

```sh
scripts/gen-midi-import-references.sh                # avec MuseScore 4.7.4
nix develop -c cargo test -p scoretk-midi --test import_corpus -- --nocapture
nix develop -c cargo test -p scoretk-midi --test roundtrip_midi -- --nocapture
nix develop -c cargo test --workspace
nix flake check
```

Ne changer la version de MuseScore qui produit les références qu'après décision
explicite (même règle que `docs/agents/upstream.md`) : cela changerait la cible
de conformité, `NOTICE`, `tests/corpus/SOURCES.md` et les documents d'écarts.

**Quand le faire** : avant une nouvelle version de scoretk, si l'amont corrige
un bug de quantification ou d'import, si une nouvelle version de MuseScore
change les références, ou si un test du corpus régresse sans changement local.

## Mise à niveau de midly (format SMF)

`midly` est la seule crate tierce de lecture/écriture du format MIDI. Version
portée : **0.5.3** (`Cargo.lock`), licence Unlicense, sans `std`
(`default-features = false`, `features = ["alloc"]`). Le code du projet ne
dépend que de `Smf::parse`, de `TrackEventKind`/`MetaMessage` et de l'écriture
`no_std` ; l'adaptation au C++ est concentrée dans
`crates/scoretk-midi/src/import/midifile.rs` (lecture) et
`crates/scoretk-midi/src/writer.rs` (écriture).

Où récupérer les sources : <https://github.com/kovaxis/midly> (ou le paquet
`midly` de crates.io, sources dans le registre Cargo local). Pour une mise à
jour :

1. changer la version dans `crates/scoretk-midi/Cargo.toml` (et
   `Cargo.lock`), vérifier qu'elle reste `no_std + alloc` et compatible
   `wasm32-unknown-unknown` ;
2. lire le journal des versions (`CHANGELOG`, diffs de `src/smf.rs`,
   `src/event.rs`, `src/primitive.rs`) pour les changements d'API de parsing
   (en-tête, `Timing::Metrical`/`Timecode`, messages méta, `u7`/`u4`) ;
3. mettre à jour `docs/licences-tierces.md` si la licence change (elle ne doit
   rester que Unlicense ou équivalent permissif) ;
4. revalider :

```sh
nix develop -c cargo test -p scoretk-midi
nix develop -c cargo build -p scoretk-midi --target wasm32-unknown-unknown
nix develop -c cargo test -p scoretk-midi --test import_corpus -- --nocapture
nix develop -c cargo test -p scoretk-midi --test roundtrip_midi -- --nocapture
nix flake check
```

**Quand le faire** : lors d'une mise à jour de dépendances du projet, ou si un
fichier MIDI du corpus ne se lit plus après une montée de version de MuseScore
(le format SMF lui-même est stable ; c'est l'API de `midly` qui évolue).
