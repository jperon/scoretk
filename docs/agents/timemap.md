# Carte des temps (`timemap`)

La carte des temps est un **sous-produit du rendu MIDI** : elle associe à
chaque tick d'exécution du fichier produit son temps en microsecondes et
recense les évènements utiles à un lecteur de partition (mesures jouées,
attaques de notes, changements de tempo, pauses de section). Elle est définie
dans `crates/scoretk-midi/src/timemap.rs` et remplie par `render.rs` aux mêmes
endroits que les évènements MIDI : ses ticks sont donc exactement ceux du SMF,
pauses comprises. L'invariance à l'octet est vérifiée par
`crates/scoretk-midi/tests/timemap_alignment.rs`.

## 1. API

```rust
use scoretk_midi::{write_midi_with_timemap, write_midi_with_timemap_and, MidiOptions};

let (midi, timemap) = write_midi_with_timemap(&score)?;      // reprises déroulées
let (midi, timemap) = write_midi_with_timemap_and(&score, MidiOptions { expand_repeats: false })?;
```

La façade `scoretk` expose la version complète :

```rust
use scoretk::{write_playback, Options};

let playback = write_playback(&score, &Options::default())?;
// playback.musicxml : Vec<u8> (id + xml:id forcés)
// playback.midi     : Vec<u8> (identique au MIDI ordinaire)
// playback.timemap  : scoretk::Timemap
```

`write_playback` force `stable_ids` pour le MusicXML : les `xmlId` de la carte
existent tels quels dans le document. L'export MusicXML ordinaire reste sans
identifiants (`Options::default()`), pour rester identique aux références
MuseScore 4.7.4.

## 2. Structure Rust

```rust
pub struct Timemap {
    pub division: u32,          // 480 ticks par noire
    pub ticks: i64,             // durée totale, pauses comprises
    pub tempo: Vec<TempoChange>,
    pub measures: Vec<MeasureSync>,
    pub notes: Vec<NoteSync>,
    pub pauses: Vec<PauseSync>,
}

pub struct TempoChange { pub tick, pub time_us, pub micros_per_quarter }
pub struct MeasureSync {
    pub tick, pub end_tick, pub time_us, pub end_time_us,
    pub measure, pub number, pub segment, pub pass, pub pause_before,
}
pub struct NoteSync {
    pub tick, pub time_us, pub duration_ticks,
    pub measure, pub staff, pub voice, pub chord, pub note, pub pitch,
    pub pass, pub grace, pub grace_index, pub grace_before, pub tremolo,
}
pub struct PauseSync { pub tick, pub end_tick, pub time_us, pub end_time_us, pub seconds }
```

Tous les ticks sont des ticks d'exécution (après déroulement des reprises et
décalage par les pauses). `MeasureSync::end_tick` est exclu ; un tick de fin de
mesure appartient à la mesure suivante (ou à une pause).

Les bornes de chaque occurrence de mesure sont **figées au rendu**, à l'endroit
où la mesure est parcourue (`render.rs`, juste avant `render_measure`), et non
recalculées dans `Builder::finish` : la carte ne peut donc pas dériver du MIDI.
`finish` vérifie seulement en debug que le recalcul depuis `RepeatList` et
`PauseMap` redonne la même valeur.

`Timemap::notes` est trié par `tick` croissant, puis, à tick égal, par
`measure`, `staff`, `voice`, `chord`, `note`, `pitch`. Le tri est fait à la
construction de la carte (`Builder::finish`), pas côté JavaScript : la carte
Rust et `timeline.notes` ont donc le même ordre déterministe, et `tick` y est
non décroissant.

## 3. Identifiants MusicXML

Les formules viennent de `scoretk_core::model::stable_ids`, partagées avec l'export :
aucune recopie, donc égalité exacte entre la carte et le MusicXML.

| Élément | Fonction | Forme |
| --- | --- | --- |
| Mesure | `stable_measure_id(part, measure)` | `p{part}-m{measure}` |
| Note / silence | `stable_note_id(measure, staff, voice, chord, note)` | `m{measure}-s{staff}-v{voice}-c{chord}-n{note}` |
| Ornement | `stable_grace_note_id(measure, staff, voice, chord, grace_index, note, before)` | `m{measure}-s{staff}-v{voice}-c{chord}-g{grace_index}{b\|a}n{note}` |

- `part`, `measure`, `chord`, `note`, `grace_index` sont des indices du modèle
  (base zéro), pas des numéros affichés ni des rangs de passage.
- `chord` est le rang du `ChordRest` dans sa voix, **silences compris**, dans
  l'ordre du rendu ; `note` vaut zéro pour un silence.
- `staff` est l'indice de la portée (`StaffId::raw()`), `voice` la voix 0 à 3.
- Quand `stable_ids` est actif, l'export écrit **`id` et `xml:id`** avec la
  même valeur sur chaque `<measure>` et chaque `<note>` ; c'est cette valeur
  que `MeasureSync::xml_id` et `NoteSync::xml_id` renvoient. L'identifiant de
  mesure `p{part}-m{measure}` est **unique dans tout le document** : ce qui est
  répété par le MusicXML `partwise`, c'est la *présence* du même `<measure>`
  dans chaque `<part>`, et le préfixe `p{part}` lève l'ambiguïté. Côté
  JavaScript, `MeasureSync` n'expose que `xmlId` de la partie 0.

## 4. Reprises : `segment` et `passage`

Une mesure du modèle peut être jouée plusieurs fois. La carte liste chaque
**occurrence** dans l'ordre d'exécution :

- `measure` : indice de la mesure dans `Score::measures` (identique d'un
  passage à l'autre) ;
- `number` : numéro de mesure du modèle (base zéro) ;
- `segment` : indice du segment de reprise (`RepeatList::segments`) ;
- `passage` (`pass` en Rust) : numéro de passage du segment, à partir de 1 ;
- `tick` / `end_tick` : position de **cette** occurrence dans la performance.

Avec `expand_repeats: false` (`RepeatList::flatten`), chaque mesure
n'apparaît qu'une fois. Un saut D.S./D.C. peut rejouer une mesure dans des
segments de même `playback_count` : `passage` peut donc être identique alors
que `segment` ou `tick` diffèrent ; l'association fiable reste `xmlId` +
`tick`.

## 5. Temps, tempos et pauses

Le temps fait foi en **arithmétique entière** : chaque segment de tempo
contribue `delta_ticks × micros_per_quarter / DIVISION`, accumulé en `i64`.
Il n'y a ni flottant ni horloge, donc pas de dérive avec la longueur du
morceau. `time_us_at(tick)` prolonge le dernier tempo en vigueur au-delà du
dernier changement.

Une pause de section insère une mesure de ticks et deux changements de tempo
(début et fin de pause, 3 s par défaut). Entre les deux, `measure_at(tick)`
renvoie `None` : le tick tombe dans la pause. `MeasureSync::pause_before`
signale la mesure qui suit une pause ; la fin de la mesure précédente reste
fixée au début de la pause.

## 6. Structure JavaScript

`convertWithTimeline` renvoie `{ musicxml, midi, timeline }`. Le type Rust
reste `Timemap` ; les champs JS sont en `camelCase` :

| Rust (`Timemap`) | JS (`Timeline`) |
| --- | --- |
| `division`, `ticks` | `division`, `ticks` |
| `tempo[].time_us`, `micros_per_quarter` | `tempo[].timeUs`, `microsPerQuarter` |
| `measures[].measure/segment/pass` | `measures[].measure/segment/passage` |
| `measures[].measure` | `measures[].ordinal` (indice 0-basé, seule clé d'ancrage) |
| `measures[].xml_id(0)` | `measures[].xmlId` |
| `measures[].end_tick`, `time_us`, `end_time_us` | `measures[].endTick`, `timeUs`, `endTimeUs`, `timeMs` |
| `notes[].duration_ticks`, `pass`, `grace`, `tremolo`, `xml_id()` | `notes[].durationTicks`, `passage`, `grace`, `tremolo`, `xmlId` |
| `pauses[].end_tick`, `time_us`, `end_time_us`, `seconds` | `pauses[].endTick`, `timeUs`, `endTimeUs`, `seconds` |

`timeMs` vaut `timeUs / 1000`. `TimelineMeasure::ordinal` vaut exactement
`measure` (indice 0-basé dans `Score::measures`, identique à la position de la
`<measure>` dans chaque `<part>` du MusicXML) : c'est la seule clé d'ancrage à
utiliser. Les champs Rust absents de la surface JS sont `MeasureSync::number`,
`MeasureSync::pause_before`, `NoteSync::grace_index` et
`NoteSync::grace_before` : ils servent au calcul des identifiants et au rendu.
`number` n'est pas exposé (0-basé côté modèle, 1-basé et remis à 1 par section
côté MusicXML), précisément pour éviter une confusion avec `ordinal`.

## 7. Limites

- **Pas de géométrie** : ni abscisse, ni système, ni page. Placer un curseur à
  l'écran demande un moteur de mise en page, hors de cette carte.
- **`Note::velocity` non appliquée** : les vélocités MIDI viennent des nuances,
  des soufflets et des articulations. `NoteSync` ne porte donc pas de vélocité
  et la vélocité saisie sur la note (import MIDI) n'apparaît pas.
- **Fermatas non modélisées** : leur allongement n'est ni dans les ticks ni
  dans les temps.
- **Trémolos** : plusieurs `NoteSync` peuvent partager un même `xmlId` (les
  attaques répétées d'un même événement du modèle), comme les notes d'un
  accord partagent une mesure.
- **Notes sans attaque** : les silences et les notes de prolongation
  (`<tie type="stop">` / `<tied type="stop">`) ont un `id` dans le MusicXML
  stable mais pas de `NoteSync` : le rendu MIDI n'émet aucune attaque pour
  eux, une chaîne liée n'ayant qu'une attaque initiale. Le test
  `timemap_alignment::note_ids_match_stable_musicxml` compare le multiensemble
  des notes de la carte à celui des `<note>` du MusicXML **après soustraction
  explicite** de ces deux catégories.
- **Pauses** : seules les pauses de section sont représentées ; une respiration
  ou une fermata ne décale rien.

## 8. Côté client : associer un temps à une mesure

1. chercher avec `measure_at(tick)` (ou `timeline.measures` triées par `tick`)
   la dernière mesure dont `tick <= temps < endTick` ;
2. si le tick tombe dans une pause, aucune mesure ne correspond : utiliser
   `pauses` pour afficher la pause ;
3. pour retrouver un élément graphique, utiliser `xmlId` : il existe à
   l'identique dans le MusicXML (`id` et `xml:id`) ; pour ancrer une mesure,
   utiliser `ordinal` (indice 0-basé) ;
4. en cas de passages multiples, ne pas indexer par `xmlId` seul : garder le
   couple (`xmlId`, `tick`) ou l'indice de l'occurrence dans `measures`.

Exemple minimal :

```js
const at = timeline.measures.find((m) => tick >= m.tick && tick < m.endTick);
const el = at && document.querySelector(`[xml\\:id="${at.xmlId}"], [id="${at.xmlId}"]`);
```
