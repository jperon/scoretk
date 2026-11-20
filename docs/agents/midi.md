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
- **Changements d'instrument, repères de répétition, pédales** : non exportés.
  Les paroles, elles, sont écrites en `FF 05` comme dans `exportmidi.cpp` (voir
  « Fidélité SMF » ci-dessous) ; les marqueurs restent non exportés.
- **Instruments à percussion** : pas de table de batterie (`<Drumset>` non
  modélisé) ni de canal 9 automatique.
- **`playRepeats` d'un saut** : non modélisé, traité comme faux (valeur par
  défaut de MuseScore et des fichiers du corpus).
- **Mesures de multi-repos** : le contenu recopié par le lecteur est joué ; la
  logique `MeasureRepeat` de `renderStaff` n'est pas rejouée.
- **Fichiers à staves liées** (`linked-ties`) : le modèle ne duplique pas les
  portées liées ; l'export suit le contenu réellement modélisé.

## Fidélité SMF (décisions documentées)

Ces points concernent le format SMF lui-même, pas le rendu Musical. Ils sont
mesurés par des tests reduced construisant les octets SMF avec `midly` (aucun
fixture binaire ajouté).

- **Drapeau mineur `FF 59`** : le lecteur lit `mi` (`0` majeur, `1` mineur) et le
  stocke dans `KeySig::mode`, comme l'import MusicXML. L'écrivain le restitue
  depuis le modèle. MuseScore 4.7.4 fait autrement : `processMeta`
  (`importmidi.cpp`) ne lit que `sf`, et `writeHeader` (`exportmidi.cpp`) écrit
  toujours `0 /* major */`. Le portage diverge volontairement pour ne pas perdre
  le mode d'un SMF lu. Vérifié par `smf_key_signature` (lecture) et
  `key_signature_export` (écriture).
- **Division SMPTE** : `frames/s × ticks/frame` (29,97 arrondi pour le
  drop-frame 30) donne des ticks par seconde. `toMuseScoreTicks`
  (`importmidi_fraction.cpp`) les traite avec `ReducedFraction::fromTicks`,
  c'est-à-dire une équivalence fixe sans carte des tempos ; le portage fait de
  même mais divise par la division déclarée (960 ticks MuseScore par seconde à
  120 BPM). Une carte des tempos SMPTE serait une fonctionnalité distincte.
  Vérifié par `smf_smpte` (30 et 29,97 images/s, plus un cas métrique inchangé).
- **Paroles RP-017** : `parse_rp017` reconnaît l'évènement vide comme un
  mélisme (durée cumulée sur la syllabe précédente), le hyphen final comme
  `Begin`/`Middle` puis `End`, et ignore le drapeau « on » du tick 0. MuseScore
  (`importmidi_lyrics.cpp`) pose chaque texte en `Single` et saute les évènements
  vides : le portage suit la norme. RP-026 (commandes `\r`, jeux de caractères,
  ruby) reste non décodé. Vérifié par `smf_lyrics` sur deux mesures avec huit
  notes, hauteurs et durées incluses.
- **Format 2** : les pistes sont des motifs indépendants ; `MidiFile::read`
  décale chaque piste de la fin de la précédente avant la conversion. MuseScore
  rejette le format 2 (`midifile.cpp` : `midi file format 2 not implemented`) ;
  le portage l'accepte. Les formats 0 et 1 gardent leur superposition habituelle.
  Vérifié par `smf_format2` (trois motifs, cinq notes, deux mesures, plus le cas
  format 1 inchangé).
- **Paroles à l'écriture** : contrairement à l'état antérieur du portage,
  `exportmidi.cpp` de MuseScore 4.7.4 émet bien les paroles du modèle en
  `FF 05`, avec un espace final ajouté aux syllabes `Single`/`End` seulement.
  Le portage suit ce comportement, y compris à travers les reprises. Vérifié par
  `lyrics_export` (quatre noires, trois syllabes, hauteurs et attaques
  inchangées, aucun `FF 05` sans parole).

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

## Phase 3 — alternatives, output size and N2 (2026-11-19)

### Alternatives on the same lots and the same oracle

The oracle (`scripts/oracle/midi/`) now runs the alternatives with option
flags, on the same manifests and with the same N1/N2 comparison as scoretk:
`--mscore` (MuseScore 4.7.4, headless), `--midi2ly` (`midi2ly` → `.ly` →
`lilypond -s` → MIDI, compared MIDI↔MIDI) and `--music21`
(`music21.converter.parse` → MusicXML). **music21 is not packaged in nixpkgs**
as of 2026-11-19, so the flag is skipped with a warning rather than recorded
as a failure. Alternatives are sampled with `--alt-stride 10`; scoretk is always
measured on the whole lot. Every launch uses `timeout` + `prlimit --as=2000000000`.

| Lot | Tool | Files | OK | Fail | Exact | N1 % | N2 % | Avg size | Max time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lecture_musique | scoretk | 1020 | 1020 | 0 | 251 (24.6 %) | **100.00** | **100.00** | 48 384 B | 0.79 s |
| lecture_musique | MuseScore 4.7.4 | 102 | 102 | 0 | 20 (19.6 %) | **100.00** | **100.00** | 66 786 B | 2.08 s |
| lecture_musique | midi2ly 2.26.0 | 102 | 90 | 12 | 87 (85.3 %) | 99.11 | 99.11 | 1 705 B¹ | 5.17 s |
| lecture_mutopia | scoretk | 122 | 122 | 0 | 12 (9.8 %) | 99.80 | **99.80** | 262 149 B | 2.09 s |
| lecture_mutopia | MuseScore 4.7.4 | 13 | 13 | 0 | 1 (7.7 %) | 100.00 | 83.73 | 521 281 B | 1.88 s |
| lecture_mutopia | midi2ly 2.26.0 | 13 | 12 | 1 | 11 (84.6 %) | 97.84 | 96.67 | 7 661 B¹ | 16.46 s |

¹ midi2ly's output is a MIDI file, not MusicXML: its size is not comparable
with the two MusicXML producers. The 12 + 1 failures are `midi2ly`/`lilypond`
rejections of dense files, not scoretk failures.

**Result: scoretk ≥ alternatives.** It ties MuseScore on `~/Musique` and beats
it clearly on Mutopia (N2 99.80 % against 83.73 %), and beats midi2ly on both
lots. scoretk also has zero failures and a much lower maximum conversion time
than midi2ly. On the 96-file Lakh robustness sample, scoretk converts 96/96
files with a 6.30 s maximum on an idle machine (one file hit the 10 s bound
during the loaded campaign at 10.01 s but completes in 5.77 s in isolation).

### Output size — `Dreadlock Holiday.1.mid`

The 114 KB Lakh file yields **6 834 457 B** of MusicXML from scoretk and
**8 652 812 B** from MuseScore 4.7.4: the ratio scoretk/MuseScore is **0.79**.
The source MIDI is single-track format 0, 192 tpq, 15 668 note-ons (14 965
after note-on/off pairing), one 4/4 signature, one tempo (582 524 µs ≈ 103 BPM)
and nine programs. The size is therefore inherent to a dense ~15 000-note
piece, not a runaway conversion. Element counts confirm it:

| Element | scoretk | MuseScore |
| --- | ---: | ---: |
| `<note>` (pitched / rest) | 22 115 (17 616 / 4 499) | 22 364 (13 809 / 5 241) |
| `<chord/>`, ties | 9 420, 4 280 | 8 305, 3 282 |
| `<beam>`, `<stem>` | 13 011, 17 594 | 6 654, 17 123 |
| All elements / attributes | 225 074 / 40 935 | 234 165 / 100 226 |

MuseScore emits three times more layout attributes (`default-x`, `default-y`,
`width`), which is why its file is larger despite fewer notes. scoretk actually
*preserves* more notes: with the fixed oracle, its output matches
14 953 / 14 965 reference attacks (99.92 %) while MuseScore's matches only
1 353 (9.04 %) — MuseScore's performance detection rewrites the piece as 3/4
at 222 BPM with a −12 transposition. Reducing scoretk's size would mean
dropping notes or ties, which would lose N1/N2; no faithful importer change is
warranted. Average size ratios over the lots: **0.72** (`lecture_musique`),
**0.50** (`lecture_mutopia`), and **0.65–0.80** (mean **0.73**) on a nine-file
Lakh sample. scoretk is smaller than MuseScore everywhere measured.

### N2 losses — cause and fix

Before the fix, the flagged N2 losses were almost entirely `attaque_decalee`:
194 files / 3 983 lost attacks on `~/Musique` and 78 / 7 773 on Mutopia, with
N1 and the note count always correct. Grouping by cause showed this was **not
an importer defect**: the oracle's `read_xml` ignored `<backup>` and
accumulated durations per (voice, staff) over the whole part. A voice that
only appears after bar N therefore had all its attacks shifted to the start of
the piece. Reduced demonstration: `test_backup.py` (a two-voice score where
voice 2 starts in bar 2) — it failed before the fix and passes after.

The oracle now uses a per-measure cursor moved by `<backup>`/`<forward>`, with
the next measure starting at the end of the previous one. After re-measuring
the same manifests with the same oracle:

| Lot | Before N2 | After N2 | Files improved | Files dropped |
| --- | ---: | ---: | ---: | ---: |
| lecture_musique | 97.08 % | **100.00 %** | 194 | 0 |
| lecture_mutopia | 90.47 % | **99.80 %** | 79 | 0 |

N1 is unchanged (pitch multisets do not depend on attack placement). The
remaining `duree_seule` flags concern N2d only, not N2: the oracle still skips
`<tie type="stop"/>` notes, so tied-note durations are undercounted (documented
oracle limitation, left unchanged to keep the measurement stable). No importer
change was needed for N2: the import was already faithful, and porting more
MuseScore voice-separation code could only have degraded the result.

## Phase 4 — tied-note oracle fix and N2d causes (2026-11-19)

### Oracle: a tied chain is one note

The `duree_seule` limitation above is now fixed in
`scripts/oracle/midi/midi_notes.py::read_xml`. A sustained note crossing a
barline is written in MusicXML as a chain of tied fragments
(`<tie type="start"/>` … `<tie type="stop"/>`), one per measure, while the
reference MIDI has a single note whose duration covers the whole sustain.
The old reader discarded `<tie type="stop"/>` notes, so only the first
fragment was counted. The reader now merges each chain into **one note** whose
attack is the first fragment's and whose duration is the **sum** of all
fragments. Chains are keyed by `(part, staff, voice, pitch)`, so two voices (or
two staves) sustaining the same pitch in parallel stay separate. Grace notes
are still skipped. `read_xml` is shared by scoretk and MuseScore, so the fix
applies equally to both tools and the comparison stays fair. The reduced
regression test `scripts/oracle/midi/test_ties.py` covers a chain across a
barline, a three-fragment chain (stop + start in the middle), two parallel
chains on different voices/staves, an unterminated tie at end of file and an
untied repeated pitch (which must not merge). `test_backup.py` still passes.

### Re-measurement (same manifests, same oracle, both tools)

| Lot | Tool | Files | Exact before → after | N1 % | N2 % | N2d % |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| lecture_musique | scoretk | 1020 | 24.6 % → **58.4 %** | 100.00 | 100.00 | 97.83 |
| lecture_musique | MuseScore 4.7.4 | 102 | 19.6 % → **46.1 %** | 100.00 | 100.00 | 97.85 |
| lecture_mutopia | scoretk | 122 | 9.8 % → **40.2 %** | 99.80 | 99.80 | 96.08 |
| lecture_mutopia | MuseScore 4.7.4 | 13 | 7.7 % → **23.1 %** | 100.00 | 83.73 | 75.38 |

N1 and N2 are unchanged by the tie fix (they do not depend on the duration of
a note). The exact rate jumps because N2d now matches the reference MIDI for
sustained notes. On the Lakh robustness sample (1/20, 96 files) scoretk still
converts 96/96 files, with a 7.02 s maximum and no timeout, under the standing
`timeout 10 prlimit --as=2000000000` bound. In this worktree the
`ecriture_mscz_corpus` manifest still points to a scratch worktree that no
longer exists, so its 21 rows are oracle-side failures (missing reference
MIDI), not scoretk failures; the other writing lots are unchanged from
phase 3.

### Remaining N2d mismatches grouped by cause

Causes were counted note by note on the files sampled for both tools (every
10th file, 102 on `~/Musique` and 13 on Mutopia), after pairing each reference
note with the test note of the same pitch and an attack within ±1/16 quarter.
Only pairs whose duration differs by more than that tolerance are counted.
A *fair* subset was also built, restricted to files where both tools match
every reference attack (N2 = reference), so the duration comparison is not
confounded by MuseScore's attack shifts.

| Cause | `~/Musique` tk / MuseScore | Mutopia tk / MuseScore | Verdict |
| --- | ---: | ---: | --- |
| Staccato/legato length (short note lengthened by one grid step) | 230 / 239 | 567 / 1058 | scoretk better |
| Last note of the piece (one extra grid step at the end) | 27 / 40 | 7 / 14 | scoretk better |
| Note-off quantisation (off-grid reference offsets) | 1 / 15 | 116 / 24 | see below |
| Overlapping notes in one voice | 0 / 0 | 0 / 0 | equivalent |
| Grace notes | 0 / 0 | 0 / 0 | equivalent |

On the fair subset the aggregate is also in scoretk's favour: N2d
**98.09 % vs 97.85 %** on `~/Musique` and **95.29 % vs 90.42 %** on Mutopia;
the exact-file counts are 57 vs 47 and 6 vs 3. The aggregate N2d over all
files (97.83 % vs 97.85 % on `~/Musique`) is a tie to within 0.02 percentage
points, and scoretk wins clearly on Mutopia (96.08 % vs 75.38 %).

The note-off-quantisation line is the only one where scoretk has more local
mismatches, and it is an artefact of the comparison, not a defect worth a
port: all 116 Mutopia cases come from three human-performance files
(`KV331_2_1_menuetto`, `SchubertF-D899-2-Impromptu`, `KingCotton`), where the
reference has offsets such as 191/384 or 1/3 quarter and MuseScore's own
attack placement is wrong (for example N2 = 18.53 % on `KV331_2_1_menuetto`,
against 100.00 % for scoretk). On those files the two tools are not aligning
the same notes, so the duration counts are not comparable. Restricted to the
fair subset, scoretk matches more note-offs than MuseScore on `~/Musique`
(1 vs 15). The remaining local files where scoretk is lower on N2d
(`Fait/04/lily-47b4b301`, `Fait/32/lily-a931e49d`, `Encantada`, `Etude1`) all
belong to the staccato/legato and last-note causes above, where the aggregate
still favours scoretk.

### No importer change

For every cause above scoretk equals or beats MuseScore, so per the stated
rule the importer is left untouched: no port from
`ref-musescore/src/importexport/midi/` was committed, and no file was dropped.
One candidate was investigated and rejected: upstream
`Simplify::minimizeNumberOfRests` skips a next chord that starts before the
current note's off-time (`next->first < note.offTime`), whereas the port's
`ChordMap::next_in_voice` does not. Adding the equivalent
`next_in_voice_after` filter and rebuilding the CLI produced **identical**
N2d on the affected files (92.48 %, 96.67 % and 94.05 % respectively), so the
change was reverted rather than committed as a no-op with a misleading test.
The other upstream difference, the `isHumanPerformance` flag that
`lengthenNote` uses to relax its accuracy guard, is not threaded into
`simplify.rs`, but it has no effect on these grid-aligned lots (both tools
detect the same human performances, and the fair subset still favours
scoretk). `MidiBeat::adjustChordsToBeats`, which aligns human performances to
recognised beats, remains a documented limitation (§ "Limites mesurées"): it
would be a separate feature port with a real risk to the 100 % N1/N2 baseline,
not a localised N2d fix, so it is deliberately out of scope here. No
conversion in this phase was run without `timeout` + `prlimit --as=2000000000`.

## Phase 5 — MIDI writing: baseline, alternatives and the tie-gate fix (2026-11-19)

### Frozen baseline and MuseScore on the same lots

The CLI was built once, copied aside as a frozen binary, and the three writing
lots were measured with `scripts/oracle/midi/lots.sh` + `mesure.py` (same
manifests, same oracle, `timeout 10 prlimit --as=2000000000` on every launch).
MuseScore 4.7.4 itself was then exported on the two `.mscz` lots with the same
oracle. All numbers are "files exact" / N1 / N2 percentages.

| Lot | Tool | Files | Exact | N1 % | N2 % |
| --- | --- | ---: | ---: | ---: | ---: |
| ecriture_ly | scoretk (frozen) | 1020 | 753 (73.8 %) | 90.33 | 83.25 |
| ecriture_mscz_corpus | scoretk (frozen) | 21 | 17 (81.0 %) | 100.00 | 100.00 |
| ecriture_mscz_corpus | MuseScore 4.7.4 | 21 | 17 (81.0 %) | 100.00 | 100.00 |
| ecriture_mscz_musique | scoretk (frozen) | 8 | 1 (12.5 %) | 66.25 | 52.20 |
| ecriture_mscz_musique | MuseScore 4.7.4 | 8 | 3 (37.5 %) | — | — |

MuseScore is the reference for both `.mscz` lots, so its 17/21 on the corpus
is the ceiling (the four non-exact rows have a reference with **zero notes**:
`mmrest`, `repeat68`, `repeat69`, `testTempoPrecision`). On `~/Musique` it gets
3/8; the three extras are `Ad_te_Suspiramus` (MuseScore 2.06, which scoretk's
reader rejects), `Stella_maris_voix-égales` and `Puer_natus_est_nobis_-_De_Martre`.
On the five other files MuseScore's fresh headless export differs widely from
the stored `.midi` reference (e.g. `Wellerman`: 732 produced notes for 183
reference notes), so those rows are reference/tool artefacts, not scoretk
defects. The two actionable gaps are therefore the 2.06 reader (out of scope
for the writer) and the tied-note duration below.

### Non-exact files grouped by first cause, with one reduced example each

`ecriture_ly` (267 non-exact): `octave` 81
(`2206_Pere_Pro/.../02_02_Segura_L-aigle-mexicain_partition.ly`, clef/ottava
octave from the LilyPond reader), `notes_manquantes` 55
(`Fait/0a/lily-8546410f.ly`, voices/nested input), `duree_seule` 49
(`Partitions/Accordeon/Encantada/Encantada.ly`), `attaque_decalee` 46
(`2024_Noel_1re/À-Notre-Dame.ly`), `notes_supp` 35
(`Promos/22_StMichel/22_HymneStMichel.ly`), `autre` 1 (empty reference).
`ecriture_mscz_corpus`: only the four empty references above.
`ecriture_mscz_musique`: `octave`/`notes_manquantes` 5, `duree_seule` 1
(`Puer_natus_est_nobis_-_De_Martre.mscz`), plus the unreadable 2.06 file.

The 46 `attaque_decalee` files have no single systematic offset (the per-note
deltas spread from −24 to +16 quarters); they are repeat/volta and pickup
differences between the two notations, not a localized writer defect. Most
`duree_seule` hits are a **comparator artefact**: in a two-voice staff LilyPond
merges simultaneous unisons into one note while MuseScore (and scoretk) writes
both, so the oracle's `(channel, pitch)` note-on map collapses the duplicates.
Reduced example: `Fait/8a/lily-376d6605.ly`, pitch 60 at 89.5 quarters —
LilyPond has one note-on/one note-off (1 quarter), scoretk has two note-ons
(one per voice), which the oracle pairs as a 0.5-quarter note. This is faithful
to MuseScore's `EventsHolder` (`events[channel].emplace`, no unison merge) and
was intentionally left unchanged. The same mechanism accounts for the
"ratio 0.5" family (163 notes of 1→0.5 quarter, 132 of 0.5→0.25).

### Cause fixed: tie duration must carry the instrument gateTime

`calculateTieLength` (`compatmidirenderinternal.cpp:771-786`) reads
`nel[0].len()` of every tied note, and that play-event length has already been
multiplied by the instrument `gateTime` in `renderChord`
(`compatmidirender.cpp:263`, `e.setLen(e.len() * gateTime / 100)`). The port
added the raw `actualTicks()` of the tied note, so a tied note on an instrument
with a default articulation below 100 % was too long. Reduced test
(`tests/gate_time_ties.rs`): an instrument with an unnamed articulation at
95 % and two tied quarter notes gave a note-off at tick 935 instead of 911;
a single note (455) and a 100 % gate (959) are unchanged. The fix passes
`instrument`/`source_tick` to `tie_length` and applies the same gated
`playEvents` length per tied chord.

### Before / after (same frozen manifests and oracle)

| Lot | Before exact | After exact | Gain | Loss |
| --- | ---: | ---: | ---: | ---: |
| ecriture_ly | 753 (73.8 %) | 753 (73.8 %) | 0 | 0 |
| ecriture_mscz_corpus | 17 (81.0 %) | 17 (81.0 %) | 0 | 0 |
| ecriture_mscz_musique | 1 (12.5 %) | **2 (25.0 %)** | 1 (`Puer_natus`) | 0 |

Per-file comparison of N1, N2 and N2d shows exactly one gain and zero losses
across the 1049 files. The `.ref.mid` corpus test (`corpus_midi`) and the
`e2e_corpus` test remain at 21/21 and unchanged. Every conversion in this
phase used `timeout` + `prlimit --as=2000000000`.

The byte-level oracle (`scratchpad/oracle/compare.sh`) is **not a usable gate
for this change**: its reference was generated on 2026-11-18 and is stale with
respect to later commits (262 diffs, of which 248 are in unrelated
MusicXML/Mxl/LilyPond/ABC outputs). Re-running the frozen binary and the new
binary on the 14 differing `.midi` cases shows that only two actually change:
`v3_voices.mscx` and `v4_testExcludeInvisibleElements.mscx`. Both declare a
default instrument `gateTime` of 95 %, so the new note-off placement is the
intended, MuseScore-faithful result, not a regression.

## Playback transposition from a LilyPond MIDI-only score (2026-11-20)

### What is stored and where it is applied

Some LilyPond files engrave one `\score` but render their MIDI from a second,
transposed `\score` (`\midi` without `\layout`, Notation Reference §24.3;
`\transpose frompitch topitch`, §1.2.2). The pivot model still carries the
engraved score; `Score::playback_transposition`
(`scoretk-core/src/model/playback.rs`, `Option<PlaybackTransposition>` with a
chromatic semitone count) additionally records the interval. Every writer
except MIDI ignores it.

The LilyPond reader only records it when the two scores are structurally the
same music after removing the leading `\transpose` envelopes (same groups,
staves, voices, notes, durations, rests, chords and ties). A MIDI score that
differs in any other way is ignored; see `docs/agents/lilypond.md` §15 for the
seven concrete cases in the corpus.

The MIDI writer applies the interval in `render.rs::played_pitch` to the note
numbers of **Note On / Note Off messages only** (MIDI 1.0 §Channel Voice), for
main notes, grace notes and both chords of a two-chord tremolo. The `FF 59`
key-signature meta-event keeps the engraved key. Reason from the SMF
specification (§Meta-Events): `FF 59 02 sf mi` is a *meta-event* carrying the
key signature for score display/analysis; it is not a channel voice message
and never changes a note number. Transposing it would make the MIDI key
disagree with the pivot's engraved `KeySig` and with the MusicXML, LilyPond,
ABC and MuseScore outputs, with no audible effect. The same choice keeps the
`sound` pitch and the displayed key consistent for every other input format
(the field is `None` there).

### Measurement (same manifests and oracles as §"Final re-measurement")

Baseline: frozen corrected-FIFO campaign `lym-fifo-final` (before this
change). All conversions used `timeout 10 prlimit --as=2000000000`.

| Lot | Baseline exact | Current exact | N1 gains / losses | N2 gains / losses |
| --- | ---: | ---: | ---: | ---: |
| ecriture_ly | 692 (67.8 %) | 692 (67.8 %) | 6 / 0 | 6 / 0 |
| ecriture_mscz_corpus | 17 | 17 | 0 / 0 | 0 / 0 |
| ecriture_mscz_musique | 2 | 2 | 0 / 0 | 0 / 0 |
| lecture_musique | 596 | 596 | 0 / 0 | 0 / 0 |
| lecture_mutopia | 49 | 49 | 0 / 0 | 0 / 0 |

`ecriture_ly` N1 rises 91.00 → 92.27 % and N2 83.86 → 85.77 %. The six files
that gain are `Gueguen.ly` (23 → 258 matched notes), `Sonis (copie 1).ly`
(47 → 258), `Sonis.ly` (46 → 266), and the three `GaudeteChristusEstNatus*`
files (189 → 402, 189 → 402, 441 → 1074). None becomes exact: the three
Gaudete files now match all pitches and attacks but have duration-only
differences, and the Gueguen/Sonis references still have the extra notes of an
unfolded `\repeat volta` (the engraved score is not unfolded). No file loses
N1 or N2; the `octave` flag falls from 60 to 55 files. The two MIDI reading
lots and the LilyPond reading oracle (108/120/46 exact on `mus`/`ra`/`mut`)
are unchanged, because MusicXML output ignores the new field.

Tests: `crates/scoretk/tests/playback_transposition.rs` (reduced `.ly` with an
engraved score, a `\transpose c d` MIDI score and ordinary music; byte-identical
MusicXML, MIDI pitched a tone higher, and a MIDI score that differs otherwise
records nothing) plus the interval assertions added to the reader's existing
`partition_explicite_prioritaire_et_premiere` test.

## Ottava playback and the MuseScore 2.x repeat gap (2026-11-20)

### Ottava: `Staff::pitchOffset` in the MIDI writer

`crates/scoretk-midi-export/src/render.rs` now builds, once per staff, the
`pitchOffset` map of `Staff::updateOttava` (`engraving/dom/staff.cpp:1279-1301`):
each `SpannerType::Ottava` writes its `pitchShift` at its start tick and `0` at
its end tick (`BTreeMap::insert` reproduces `insert_or_assign`, last writer
wins). `played_pitch` adds the last value at a tick ≤ the note's **source** tick
(`findLessOrEqual`, `staff.cpp:1293-1301`). Shifts come from `ottavaDefault`
(`engraving/dom/ottava.h:67-74`): 8va/8vb ±12, 15ma/15mb ±24 semitones. This is
`Note::ppitch = m_pitch + ottaveCapoFret()` (`note.cpp:2664-2690`), which
`compatmidirenderinternal.cpp:860` uses for Note On/Off. The source tick, not
the unfolded `utick`, is correct: up to `RepeatList` expansion the C++ note
object still belongs to the original segment. SMF §Channel Voice defines note
numbers but no ottava (the tool decides the playback transposition), so the
behaviour follows MuseScore, not a normative text.

`Spanner::visible` is intentionally ignored: an invisible 8va still shifts
playback (`setPlaySpanner`, `ottava.cpp:304-309`, is independent of `visible`).
The model has no `playSpanner` flag and MuseScore's default is true.

Tests (`crates/scoretk-midi-export/tests/ottava.rs`): an 8va over two notes
then a note outside it, mid-measure end with the unmodified control, two staves
with 8va/8vb, a chord, a rest and a tied note, and the MusicXML
`<octave-shift>` → model → MIDI chain. The MusicXML importer subtracts the
shift from the written pitch (`pass2.rs::build_note`), so MIDI restores the
sounding pitch of the XML; all four expected pitch/attack/duration triples
come from the reduced scores rendered with MuseScore 4.7.4, without a binary
fixture.

### Measurement against a frozen master binary

Frozen baseline `/tmp/scoretk-master-ottava` built from this worktree's HEAD
(`20e6833`, before the change); same manifests and oracle for both runs,
`timeout 10 prlimit --as=2000000000` on every launch
(`scripts/oracle/midi/mesure.py`, writing lots regenerated with `lots.sh`).

| Lot | Baseline exact / N1 / N2 | Current exact / N1 / N2 |
| --- | --- | --- |
| `ecriture_ly` (1020) | 692 / 92.27 / 85.77 | 692 / 92.25 / 85.76 |
| `ecriture_mscz_corpus` (21) | 17 / 100.00 / 100.00 | 17 / 100.00 / 100.00 |
| `ecriture_mscz_musique` (8) | 2 / 69.89 / 57.25 | 2 / 69.89 / 57.25 |

The two `.mscz` lots and all 21 corpus files are byte-identical in score. On
`ecriture_ly`, two files lose a few matches and **must be explained**:
`Ackermans_NoelDesPetitsSantons.ly` (N1 2201 → 2180, N2 2011 → 1994) and
`Salazar_ALaEstrellaQueEsmalta.ly` (N1 1240 → 1239, N2 1240 → 1238). Both use
`\ottava`; in these two contexts the LilyPond reader's ottava spanner covers
one more chord than LilyPond's own MIDI playback, so a shift that was inert
before is now audible (for example a note at 165 quarters is 48 in the
reference and becomes 72 under `\ottava #2`). Correcting that anchor belongs
to the LilyPond reader and is deliberately left out of this MIDI-only change;
the aggregate loss is 0.02 N1 / 0.01 N2 percentage points. No other file
changes.

MusicXML oracle (`scripts/oracle/musicxml/mesure.py`, same 174-file manifest,
current binary, N2 per version line as in §20): 3.x/4.x N1 0.9792, N2 0.9740
(unchanged); 1.x/2.x N1 0.9739 (was 0.9700), N2 0.6754 (was 0.6742). The gain
is concentrated in `Vingt_danses_enfantines_pour_piano.mscz` (N1 0.9896 →
0.9985, N2 0.3106 → 0.3134, 26437/26477 matched pitches). The MIDI writer
does not change `core`, XSD or `exact` counts.

The byte-level oracle `scratchpad/oracle/compare.sh` is not a usable gate for
this change either: its frozen reference predates several commits and already
reports 276 DIFFs on the untouched master tree. With the ottava change it
reports 281. The five new rows are exactly the MIDI outputs of the four corpus
scores carrying ottavas (`v3/layout_elements`, `v4/copypaste_parts`,
`v4/partExclusion`, `v4/testExcludeInvisibleElements`, each as `.mscx` and as
`.ref.musicxml`). All four are now **closer** to the conformance target:
`corpus_midi` still reports `MIDI : 21/21 identiques` against the `.ref.mid`
produced by MuseScore 4.7.4, and no non-MIDI output changes.

### Repeats: the 2.x reader and the unfolding are not the cause

The piano file was reduced to the *first* passage where attacks shift: a
hand-written 2.06 score with `segno`/`fine` markers, a D.C. `<Jump>`, a
`<startRepeat>`/`<endRepeat>4` and voltas with `endings` `1, 3` and `2, 4`.
`crates/scoretk-midi-export/tests/repeat_2x.rs` checks its 65 MuseScore 4.7.4
attacks: they match exactly. The real file is also read completely — 2040
measures, 60 voltas, 8 ottavas, 144 marker/repeat elements, all `<Marker>`,
`<Jump>`, `<Volta id>` + `<endSpanner id>` and all `<endings>` values converted
by `read206` (`docs/agents/mscx-format.md` §19). `RepeatList::unwind`
(`repeat_list.rs:348-586`) was compared line by line with
`RepeatList::unwind` (`engraving/dom/repeatlist.cpp:787-1069`), including
`findMarker` (`:643-726`), `performJump` (`:737-782`) and the repeat/volta
rewind (`:886-908`): no algorithmic difference was found that would explain
the measured offsets. Neither the reader nor the unfolding needed a change.

The remaining +6/+4.5/+13/+3-quarter offsets are **section-pause tempo
differences**, not repeat passes. In the reference MIDI the pause at 1118
quarters is 40 BPM for 2 quarters (a 2/4 measure, `quarterNotesPerMeasure /
3 s`) whereas scoretk writes 60 BPM for 3 quarters, and the reference has 93
tempo events against 58 in scoretk (`PauseMap::calculate`,
`engraving/compat/midi/pausemap.cpp:47-76`, versus `pause_map.rs:74-115`).
Removing the `pause.tick == segment.tick` guard decreased N2 from 8298 to
5411 on that file, so that guard is correct and was kept. The remaining
pitch divergences are the 2.x ottavas whose extent ends mid-measure
(`docs/agents/mscx-format.md` §19.8), where the model anchor is less precise.
Fixing either would require a separate port of the tempo timeline / 2.x
spanner anchoring, not a change to `read206` or `repeat_list.rs`; none is made
here because a wrong change there would regress the 21/21 conformant corpus.
