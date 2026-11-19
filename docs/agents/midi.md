# MIDI (`scoretk-midi-common`, `scoretk-midi-import`, `scoretk-midi-export`)

Le format MIDI est réparti en trois crates sans dépendance croisée entre les
deux sens :

- `scoretk-midi-common` : erreur et résultat partagés (et rien d'autre) ;
  l'erreur vient d'un `Error` qui doit rester identique dans les deux sens.
- `scoretk-midi-import` : lecture MIDI (`read_midi`, portage de `midiimport/`),
  avec les types du SMF (`RawEvent`, `MidiTrack`, `MidiFile`, `EventKind`,
  méta-évènements et contrôleurs) et la lecture en mémoire d'un SMF dans
  `midifile.rs`. Aucune conversion vers le modèle dans `midifile.rs`.
- `scoretk-midi-export` : écriture d'un **Standard MIDI File format 1** depuis
  le modèle de `scoretk-core`, rendu en performance (reprises déroulées, notes,
  vélocités, tempo), puis encodé.

Les deux crates métier dépendent de `-common` ; ni l'une ni l'autre ne dépend de
sa contrepartie en bibliothèque.

## API

```rust
use scoretk_musescore_import::read;
use scoretk_core::MsczContainer;
use scoretk_midi_export::write_midi;

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
  `scoretk_core::model::stable_ids`, comme l'export MusicXML. Structure complète,
  sémantique des passages et limites : `docs/agents/timemap.md`.

Les trois crates sont portables comme `scoretk-core` : aucun accès disque,
aucun thread, aucune horloge ; elles compilent pour `wasm32-unknown-unknown`.
Seule dépendance tierce ajoutée : `midly` (`no_std`, `alloc`), sous licence
Unlicense, pour le format de fichier (`-import` en lecture, `-export` en
écriture).

## Modules

### Commun (`crates/scoretk-midi-common/src/`)

| Module | Rôle | C++ amont |
| --- | --- | --- |
| `error.rs` | `Error` et `Result` partagés par les deux sens | — |

Rien d'autre : un `-common` ne contient que ce que les deux sens utilisent
(`docs/agents/developpement.md` § 2). La lecture SMF n'est utilisée que par
l'import et vit donc dans `scoretk-midi-import` (voir ci-dessous).

### Écriture (`crates/scoretk-midi-export/src/`)

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
fichiers identiques** (notes et changements de tempo effectifs ; `v3/repeat06`
et `v4/repeat65/66/68/69`, voltas et sauts compris). Les points suivants
restent hors périmètre, faute d'information dans le modèle :

- **Fermeture des voltas** : au `RepeatEnd`, le portage ferme désormais **toute**
  volta encore ouverte, sans condition sur sa mesure de fin, et un saut ferme
  la volta qui s'achève avant lui (`repeatlist.cpp:581-588` et `:525-543`) :
  une volta qui déborde l'`endRepeat` se termine sur cette mesure, comme dans
  le C++. Couvert par `repeat_list::tests::volta_debordant_le_repeat_end`.
- **Pauses de fermatas et respirations** : aucune n'est appliquée, ce qui est
  fidèle. `engraving/compat/midi/` ne mentionne aucune fermata (seul
  `CTRL_BREATH` y apparaît, `compatmidirender.cpp:55`), et
  `Breath::BREATH_LIST` (`breath.cpp:33-44`) ne sert qu'à `isCaesura()`
  (`breath.cpp:64-74`) : les secondes de césure vivent dans le `PausesMap`
  (`tempotimeline.h:33`) rempli par le module *playback*, absent de
  `ref-musescore/`. `PauseMap` ne traite donc que les sauts de section (pause
  par défaut de 3 s). Gardé par
  `pause_map::tests::points_d_orgue_et_respirations_n_allongent_pas_le_rendu`,
  à reconfirmer sur 4.7.4 si le module *playback* devient disponible.
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
`crates/scoretk-midi-export/tests/corpus_midi.rs` lit la source `.mscx`, écrit le MIDI
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
nix develop -c cargo test -p scoretk-midi-export --test corpus_midi -- --nocapture
```

Ne pas régénérer avec une autre version de MuseScore sans décision explicite :
la cible de conformité changerait en silence (même règle que les références
MusicXML, voir `upstream.md`).

## Chaîne MusicXML → MIDI

`crates/scoretk-midi-export/tests/musicxml_midi.rs` vérifie la chaîne complète
MusicXML → modèle (`scoretk_musicxml_import::read_musicxml`) → MIDI : pour
chaque `tests/corpus/{v3,v4}/X.ref.musicxml`, il écrit le MIDI et le compare à
`tests/corpus/midi/{v3,v4}/X.ref.mid` avec la même normalisation que
`corpus_midi.rs` (notes et tempos effectifs). C'est la garantie que les
nuances et tempos lus depuis `<direction>` produisent les vélocités et
changements de tempo de MuseScore.

```sh
nix develop -c cargo test -p scoretk-midi-export --test musicxml_midi -- --nocapture
```

Résultat au 2026-11-17 : **7/21 identiques** (`v4/dynamics`,
`hairpin_simple`, `linked-ties`, `mmrest`, `repeat68`, `repeat69`,
`tenuto_accent`). Les écarts restants viennent de la durée de tenue
(`gateTime`) des instruments, que MusicXML ne transmet pas, et des
articulations non lues pour certains fichiers.


## Lecture MIDI (import, t42)

La crate expose la lecture d'un Standard MIDI File vers le modèle pivot :

```rust
let score = scoretk_midi_import::read_midi(&bytes)?;          // options de MuseScore
let score = scoretk_midi_import::read_midi_with(&bytes, &options)?; // options explicites
```

La façade `scoretk` la branche avec la feature `midi-import` (l'écriture a sa
propre feature `midi-export`) :
`read(bytes, Format::Midi)` et `scoretk entrée.mid -o sortie.musicxml`
fonctionnent. Le paquet wasm de la façade les expose aussi.

### Modules

Portage de `importexport/midi/internal/midiimport/` (mêmes noms et découpage
que le C++, référence amont dans chaque en-tête) :

| Rust (`crates/scoretk-midi-import/src/`) | C++ amont |
| --- | --- |
| `lib.rs` | `importmidi.cpp` (`convertMidi`, `createMTrackList`, `createMeasures`, `createNotes`, `processMeta`) |
| `midifile.rs` | `midishared/midifile.cpp`, `midievent.h` (évènements SMF, `MidiFile::read`, séparation des canaux, fusion note-on/off, `to_muse_score_ticks`, par `midly`) |
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
`crates/scoretk-midi-import/tests/import_corpus.rs` lit le MIDI avec `read_midi`,
exporte en MusicXML et compare le **noyau musical** avec la même normalisation
que `crates/scoretk/tests/e2e_corpus.rs` (`tests/core_norm.rs`, avec en plus
la normalisation des `<divisions>` : la valeur est une convention d'encodage,
les `<duration>` sont ramenées à 480 par noire).

Résultat mesuré au 2026-11-17 : **4/21 identiques** (`v4/mmrest`,
`v4/repeat68`, `v4/repeat69`, `v4/testTempoPrecision`, tous des fichiers sans
note ou presque). Les autres fichiers diffèrent de 1 à 7 lignes (noms de
parties et abréviations, clef octaviée) sauf `v3/testTuplets8` (224 lignes) et
`v4/timesig-09` (965 lignes), à cause des n-olets complexes. La liste
`EXPECTED_IDENTICAL` ne doit pas régresser.

L'aller-retour `.mscx` → MIDI → `read_midi` → MIDI est vérifié par
`crates/scoretk-midi-export/tests/roundtrip_midi.rs` sur les 21 sources : **21/21**
avec les mêmes hauteurs, des attaques à ±1/16 de noire et des durées à ±1/16,
sauf notes plus courtes que la grille, que la simplification des n-olets peut
allonger (documenté dans le test).

```sh
scripts/gen-midi-import-references.sh            # toutes les références
scripts/gen-midi-import-references.sh tests/corpus/midi/v4/dynamics.ref.mid
nix develop -c cargo test -p scoretk-midi-import --test import_corpus -- --nocapture
nix develop -c cargo test -p scoretk-midi-export --test roundtrip_midi -- --nocapture
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
nix develop -c cargo test -p scoretk-midi-import --test import_corpus -- --nocapture
nix develop -c cargo test -p scoretk-midi-export --test roundtrip_midi -- --nocapture
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
`crates/scoretk-midi-import/src/midifile.rs` (lecture) et
`crates/scoretk-midi-export/src/writer.rs` (écriture).

Où récupérer les sources : <https://github.com/kovaxis/midly> (ou le paquet
`midly` de crates.io, sources dans le registre Cargo local). Pour une mise à
jour :

1. changer la version dans `crates/scoretk-midi-import/Cargo.toml` et
   `crates/scoretk-midi-export/Cargo.toml` (et `Cargo.lock`), vérifier qu'elle
   reste `no_std + alloc` et compatible `wasm32-unknown-unknown` ;
2. lire le journal des versions (`CHANGELOG`, diffs de `src/smf.rs`,
   `src/event.rs`, `src/primitive.rs`) pour les changements d'API de parsing
   (en-tête, `Timing::Metrical`/`Timecode`, messages méta, `u7`/`u4`) ;
3. mettre à jour `docs/licences-tierces.md` si la licence change (elle ne doit
   rester que Unlicense ou équivalent permissif) ;
4. revalider :

```sh
nix develop -c cargo test -p scoretk-midi-common -p scoretk-midi-import -p scoretk-midi-export
nix develop -c cargo build -p scoretk-midi-common -p scoretk-midi-import -p scoretk-midi-export --target wasm32-unknown-unknown
nix develop -c cargo test -p scoretk-midi-import --test import_corpus -- --nocapture
nix develop -c cargo test -p scoretk-midi-export --test roundtrip_midi -- --nocapture
nix flake check
```

**Quand le faire** : lors d'une mise à jour de dépendances du projet, ou si un
fichier MIDI du corpus ne se lit plus après une montée de version de MuseScore
(le format SMF lui-même est stable ; c'est l'API de `midly` qui évolue).

## Mesure (plan de fiabilisation, phase 0-1)

Relevé du 2026-11-19, **sans aucune correction de scoretk**. Outils versionnés
dans `scripts/oracle/midi/` (`lots.sh`, `mesure.sh`, `mesure.py`,
`midi_notes.py`) ; manifestes (listes de chemins, aucune donnée tierce dans le
dépôt) et sorties brutes dans
`/tmp/nix-shell.7XCuVq/claude-1000/-home-jacques-programmes-musique-musescore/2ae55baa-d7cc-4f59-831d-f77ca3cc7ebd/scratchpad/midi-mesure/`.
L'oracle est indépendant des deux sens : `mido` extrait les notes du MIDI de
référence, `ElementTree` (bibliothèque standard) celles du MusicXML produit.
Chaque lancement de scoretk passe par `timeout 10 prlimit --as=2000000000`.

**Niveaux mesurés.** N1 = multiset des hauteurs ; N2 = hauteur + attaque ;
N2d = hauteur + attaque + durée. Attaques et durées sont exprimées en noires,
ramenées à la grille Q16 (1/16 de noire, réglage par défaut de l'import
MuseScore) avec une tolérance de rapprochement de ±1/16 de noire. Un fichier
est « exact » lorsque N1 = N2 = N2d = nombre de notes de référence ; un fichier
sans note de référence n'est pas compté exact. Les pourcentages sont pondérés
par le nombre de notes de référence, pour ne pas donner le même poids à un
court et à un long morceau.

**Lots et échantillonnage.** Lecture : 1 020 MIDI LilyPond de `~/Musique`
appariés à un `.ly` et 122 MIDI de Mutopia. Écriture : les 1 020 `.ly` appariés
(référence = MIDI LilyPond), 21 `.mscx` du corpus (référence = MIDI exporté par
MuseScore 4.7.4) et 8 `.mscz` de `~/Musique` ayant un MIDI à côté. Robustesse :
corpus Lakh sans source appariée. MuseScore 4.7.4, alternative de lecture, est
mesuré sur un échantillon déterministe de **1 fichier sur 10** (102 fichiers de
`~/Musique`, 13 de Mutopia ; `--alt-stride 10`) et Lakh sur **1 fichier sur 20**
(96 fichiers ; `LAKH_STRIDE=20`) : avec la charge de la machine, MuseScore et
les imports Lakh denses rendaient les lots complets prohibitifs. scoretk est
mesuré sur la totalité des lots de lecture et d'écriture.

### Résultats

| Lot | Sens | Outil | Fichiers | Réussis | Échecs | Délais | Vides | Exacts | N1 % | N2 % | Taille moy. | Temps max |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| ecriture_ly | écriture | scoretk | 1020 | 1020 | 0 | 0 | 0 | 753 (73,8 %) | 90,33 | 83,25 | 984 | 0,10 s |
| ecriture_mscz_corpus | écriture | scoretk | 21 | 21 | 0 | 0 | 0 | 17 (81,0 %) | 100,00 | 100,00 | 248 | 0,01 s |
| ecriture_mscz_musique | écriture | scoretk | 8 | 7 | 1 | 0 | 0 | 1 (12,5 %) | 66,25 | 52,20 | 6451 | 0,22 s |
| lecture_musique | lecture | scoretk | 1020 | 1018 | 0 | 2 | 0 | 245 (24,0 %) | 100,00 | 97,26 | 46 575 | 10,07 s |
| lecture_musique | lecture | MuseScore 4.7.4 | 102 | 102 | 0 | 0 | 0 | 20 (19,6 %) | 100,00 | 97,73 | 66 786 | 7,81 s |
| lecture_mutopia | lecture | scoretk | 122 | 115 | 0 | 7 | 0 | 11 (9,0 %) | 99,74 | 91,20 | 201 211 | 10,01 s |
| lecture_mutopia | lecture | MuseScore 4.7.4 | 13 | 13 | 0 | 0 | 0 | 1 (7,7 %) | 100,00 | 74,38 | 521 281 | 3,32 s |
| robustesse_lakh (1/20) | robustesse | scoretk | 96 | 54 | 2 | 40 | 0 | 0 (0,0 %) | — | — | 979 050 | 10,01 s |

Lecture : scoretk place **100 % des hauteurs** sur le lot `~/Musique` (99,74 %
sur Mutopia) et devance MuseScore sur Mutopia (91,20 % d'attaques contre
74,38 % sur 13 fichiers), tout en lui étant très proche sur `~/Musique`
(97,26 % contre 97,73 % sur 102 fichiers). Sur l'écriture, les 21 fichiers du
corpus avec référence MuseScore
sont conformes sur les notes ; les 4 non comptés « exacts » (`mmrest`,
`repeat68`, `repeat69`, `testTempoPrecision`) ont une référence sans note.
Le lot `.mscz` de `~/Musique` est le plus faible : un fichier est refusé
(version MuseScore 2.06 non prise en charge) et deux présentent un effectif de
notes très différent de la référence. En robustesse (Lakh, échantillon 1/20),
**40 fichiers sur 96 dépassent la borne de 10 s**, 2 produisent un MusicXML mal
formé et 54 passent, pour une taille de sortie moyenne de 979 ko : c'est le
signal d'usure le plus net de ce relevé.

### Causes d'écart

Dix causes observées, par fréquence décroissante, avec un fichier exemple
chacune. Les huit premières sont les drapeaux du regroupement automatique du
relevé (tous lots confondus) ; les deux dernières sont des diagnostics
ciblés sur des fichiers du lot `.mscz` et de Lakh :

| Cause | Fichiers | Exemple |
| --- | ---: | --- |
| Durée de tenue (`gateTime`) différente, notes et attaques justes | 724 | `~/Musique/Partitions/Accordeon/Encantada/Encantada.ly` |
| Attaques décalées après quantification | 341 | `~/Musique/Ecole/2024_Noel_1re/À-Notre-Dame.ly` |
| Octave de clef / clef octaviée | 87 | `~/Musique/Ecole/2206_Pere_Pro/partitions/02_02_Segura_L-aigle-mexicain_partition.ly` |
| Notes manquantes (voix, n-olets) | 55 | `~/Musique/Partitions/ChantsScouts/Fait/0a/lily-8546410f.ly` |
| Dépassement de la borne de 10 s | 49 | `~/Musique/Partitions/Chorales/Mendelssohn/Laudate_pueri_Op._39_No._2_-_Felix_Mendelssohn.midi` |
| Notes supplémentaires (reprises, ornements) | 35 | `~/Musique/Ecole/Promos/22_StMichel/22_HymneStMichel.ly` |
| Décalage de hauteur constant (transposition, -12 demi-tons) | 13 | `~/Musique/Ecole/Promos/22_StMichel/22_StMichel.ly` |
| Référence sans note (non comptée exacte) | 6 | `~/Musique/Partitions/ChantsScouts/Partitions/LaRondeDeNuit/LaRondeDeNuit.ly` |
| MusicXML de sortie mal formé | 2 | `~/programmes/musique/corpus/lakh-midi-clean/clean_midi/Backstreet Boys/Back to Your Heart.1.mid` |
| Version MuseScore 2.06 non prise en charge | 1 | `~/Musique/Partitions/Chorales/AdTeSuspiramus/Ad_te_Suspiramus.mscz` |

Les délais ne sont pas contournés en augmentant la borne : ce sont des boucles
ou des traitements quadratiques à corriger en phase 2. Le fichier
`Dreadlock Holiday.1.mid` (114 ko, Lakh) dépasse encore la limite à 25 s en
diagnostic isolé.

### Phase 2 — Lakh robustness fixes (2026-11-19)

The phase-0 findings above were reproduced and fixed in three independent
causes. Profiling used a release build with temporary per-stage instrumentation
(`perf` is not available in the Nix shell) on `Dreadlock Holiday.1.mid`,
`Fernando.2.mid` and `Beautiful Life.mid`. Parsing the SMF and merging note-on /
note-off pairs took less than 15 ms per file; all the time was spent in
`convert_midi`.

**1. Quadratic chord scans (performance).** `MidiTuplet::findAllTuplets`
accounted for roughly 3.5 s of a 3.7 s conversion on `Dreadlock Holiday.1.mid`:
for each candidate start position of each metric span of each bar,
`detect_tuplet` rebuilt its chord list by walking **all** chords of the track,
i.e. `O(bars × spans × positions × chords)`. Upstream
(`importexport/midi/internal/midiimport/importmidi_tuplet.cpp`,
`findAllTuplets`) iterates `[startBarIt, endBarIt)` ranges of the
`std::multimap`, which contain only the chords of the current bar. The port now
mirrors that with `ChordMap::range(start, end)`, a `BTreeMap::range` on
`(tick, rank)`; the original time and `bar_index` filters are kept, so the
selected chords and their order are unchanged. The same pattern was fixed in
two other stages:

* `MChord::minimizeNumberOfRests` looked for the next chord of the same voice by
  scanning the whole table for every chord (`O(n²)`); `ChordMap::next_in_voice`
  now uses an **exclusive** bound at `(tick, u64::MAX)` so chords at the same
  tick remain excluded, exactly as the original `next_on_time > key.0` filter.
* `MidiVoice::separateVoices` rebuilt each bar's key list by scanning the whole
  table once per bar; a single pass now fills a sparse
  `BTreeMap<bar_index, keys>`.

A progress guard was added to the tuplet start loop: a non-positive `step`
(which a malformed bar length could produce) skips the span instead of spinning
forever. None of these upstream algorithms is intrinsically exponential; the
cost came from repeated linear scans over ordered data, so the results are
unchanged and only the time drops.

`tests/dense_bars.rs` covers the correctness of the range queries with 12 bars,
two tracks, 352 sixteenth notes plus a triplet in each track: it checks the
part, measure, chord and note counts, the pitches and durations, and that the
triplet is still detected.

**2. Malformed MusicXML (NUL in track names).** Many Lakh MIDI files terminate
their text meta events with a NUL byte (for example `Back To Your Heart.1.mid`:
`'Back To Your Heart'\0`). `String::from_utf8_lossy` preserves NUL, which XML
1.0 forbids and cannot escape, so the raw byte ended up inside `<part-name>` and
`xml.etree.ElementTree` rejected the output. This is faithful to upstream
`midishared/midifile.cpp`: `MidiFile::writeEvent` writes `len - 1` bytes for text
meta events ("Don't null terminate text meta events") and the import converts
the data with C-string semantics. `midifile::decode_meta_text` now removes the
XML 1.0 forbidden characters (C0 controls other than tab/LF/CR, `U+FFFE`,
`U+FFFF`) at decoding time, and is used for track names, metadata and lyrics.
`tests/meta_text.rs` checks an end-to-end MIDI → MusicXML conversion (no NUL,
parseable XML, exact part/measure/note counts and pitches) plus an unaffected
case with ordinary names and whitespace.

**3. Latent panic in `removeOverlappingNotes`.** Once the timeouts were gone,
`Toto/Don't Chain My Heart.mid` exposed a pre-existing out-of-bounds panic: a
note discarded for being shorter than the minimum duration shrinks its chord,
but the loop kept a cached note count and indexed `notes[0]` on an empty vector
(`chord.rs`). The loop now reads the note at the current index on every
iteration. `tests/overlapping_notes.rs` reproduces the exact Lakh timing
(two same-pitch chords 1/192 apart, below the 1/128 minimum) and checks that
only the later chord survives.

**Remeasurement (`LAKH_STRIDE=20`, same manifests).** The table below replaces
the phase-0 rows; it was produced with `scripts/oracle/midi/mesure.sh` and the
same `timeout 10 prlimit --as=2000000000` bounds.

| Lot | Files | OK | Timeouts | Exact | N1 % | N2 % | Max time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lecture_musique | 1020 | 1020 | 0 | 245 (24.0 %) | 100.00 | 97.08 | 0.25 s |
| lecture_mutopia | 122 | 122 | 0 | 11 (9.0 %) | 99.80 | 90.47 | 0.50 s |
| robustesse_lakh (1/20) | 96 | 96 | 0 | 0 (0.0 %) | — | — | 4.23 s |

The small drop in the aggregate N2/N2d percentages is **not** a regression: the
nine files that used to time out (two in `lecture_musique`, seven in
`lecture_mutopia`) now complete with partial matches below 100 %, which lowers
the weighted average. Restricted to the 1018 + 115 files that already succeeded
in phase 0, the note counts, N1, N2, N2d and the exact flags are **unchanged**.
On Lakh, the 40 timeouts, the two malformed outputs and the latent panic are
gone; all 96 files now produce a non-empty MusicXML output, with a maximum
conversion time of 4.23 s instead of 10 s. The two previously malformed files
(`Back to Your Heart.1.mid`, `Besame Mucho.mid`) and the three slowest files
(`Dreadlock Holiday.1.mid`, `Fernando.2.mid`, `Beautiful Life.mid`) were also
checked individually with `ElementTree`: all parse cleanly in 0.26–1.71 s.
