# Écriture MIDI (`scoretk-midi`)

Écriture d'un **Standard MIDI File format 1** à partir du modèle de
`mscz2xml`. C'est le portage de l'export MIDI de MuseScore Studio : la
partition est rendue en performance (reprises déroulées, notes, vélocités,
tempo), puis encodée.

## API

```rust
use mscz2xml::read;
use mscz2xml::MsczContainer;
use scoretk_midi::write_midi;

let container = MsczContainer::from_bytes(&mscx)?;
let score = read::read_score(&container)?;
let midi: Vec<u8> = write_midi(&score)?;
```

- `write_midi(&Score) -> Result<Vec<u8>, Error>` : réglages par défaut de
  MuseScore (reprises déroulées).
- `write_midi_with(&Score, MidiOptions)` : `expand_repeats: false` produit un
  fichier « à plat », sans dérouler les reprises (`RepeatList::flatten`).
- `RepeatList` / `RepeatSegment` sont publics : le déroulement des reprises,
  voltas et sauts est réutilisable par un futur lecteur MIDI ou un export
  LilyPond/ABC.

La crate est portable comme `mscz2xml` : aucun accès disque, aucun thread,
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
- **Notes** : hauteur concert, vélocité × multiplicateurs d'articulations
  (0,6 pour une note fantôme), durée effective divisée par `timeStretch` et les
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
