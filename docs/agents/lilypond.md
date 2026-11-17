# Écriture LilyPond (`scoretk-lilypond`)

Crate `crates/scoretk-lilypond`, branchée dans la façade `scoretk` par la
feature `lilypond` (`Format::Ly` en écriture). Ce document décrit la
correspondance entre le modèle pivot et LilyPond, les choix de notation et les
limites mesurées sur le corpus. Le noyau du projet reste dans `AGENTS.md`.

## 1. Rôle et API

```rust
pub fn write_lilypond(score: &Score) -> Result<String, Error>;
```

La crate ne fait aucune entrée/sortie : elle renvoie le texte en mémoire, ce
qui la garde compilable pour `wasm32-unknown-unknown`, Android et iOS. Elle ne
dépend que de `mscz2xml` (modèle pivot) et `thiserror` ; aucune crate tierce
nouvelle, donc rien à ajouter à `docs/licences-tierces.md`.

| Élément | Emplacement |
| --- | --- |
| API, documentation | `src/lib.rs` |
| Erreur | `src/error.rs` |
| Hauteurs, armures, clés, têtes de note | `src/pitch.rs` |
| Durées, points, multiplicateurs | `src/duration.rs` |
| Accords, silences, articulations, ornements | `src/chord.rs` |
| Annotations (nuances, tempo, textes, accords) | `src/annotations.rs` |
| Spanners (liaisons, soufflets, ottavas, pédales, trilles, glissandos) | `src/spanners.rs` |
| Structure des reprises (`\repeat volta`, `\alternative`) | `src/structure.rs` |
| Trame d'une voix, paroles, rattachement des événements | `src/voice.rs` |
| En-tête, variables, bloc `\score`, regroupements | `src/writer.rs` |
| Instantanés du corpus | `tests/corpus_lilypond.rs`, `tests/corpus/ly/` |
| Compilation par LilyPond | `tests/lilypond_compile.rs` |

Référence de comportement : **`musicxml2ly`** (paquet LilyPond,
GPL-3.0-or-later), qui définit les correspondances idiomatiques entre notation
MusicXML et LilyPond. MuseScore 4 n'a pas d'export LilyPond ; les tables
d'articulations, d'ornements, de nuances et de figures reprennent donc celles
de `musicxml2ly`, réécrites en Rust. L'attribution est dans `NOTICE` et dans
l'en-tête des fichiers concernés.

## 2. Choix de notation

### Mode `\fixed`, jamais `\relative`

Chaque voix est une variable dont le corps est enveloppé dans
`\fixed c' { … }` : l'octave est **absolue**, `c` valant le do central
(MIDI 60), `c'` l'octave au-dessus, `c,` l'octave en dessous. Une note se
transpose ou se déplace sans changer le sens des notes voisines, contrairement
à `\relative` ; c'est une exigence explicite du projet (`docs/TODO.md`).

### Langue anglaise explicite

`\language "english"` est déclaré en tête : les altérations s'écrivent `cs`
(do dièse), `bf` (si bémol), `css`/`bff` (doubles). Sans cette ligne, le
fichier dépendrait de la langue par défaut de LilyPond (`nederlands`).

### Une variable par voix

```lilypond
staffOneVoiceOne = \fixed c' { … }
staffTwoVoiceTwo = \fixed c' { … }
```

Le nom sert à la fois de variable et de cible à `\lyricsto`. Les voix d'une
portée qui en compte plusieurs reçoivent `\voiceOne`, `\voiceTwo`… pour que
LilyPond gère hampes et collisions.

### Regroupement des portées

D'après les accolades de MuseScore (`<bracket>`, champ `StaffBracket::kind`) :

| Crochet MuseScore | LilyPond |
| --- | --- |
| accolade (`1`) | `\new PianoStaff` |
| crochet, crochet carré (`0`, `3`) | `\new ChoirStaff` |
| plusieurs portées sans crochet | `\new StaffGroup` |
| une seule portée | `\new Staff` |

Les noms d'instrument passent par `\with { instrumentName = … shortInstrumentName = … }`.

## 3. Correspondance modèle → LilyPond

### Partition et mesures

| Modèle | LilyPond |
| --- | --- |
| `ScoreMetadata` (et crédits en repli) | `\header { title, subtitle, composer, poet, arranger, copyright }` |
| `Measure.len < nominal_len` (première mesure) | `\partial <durée>` |
| `TimeSig` | `\time n/d` |
| `TimeSig` locale (longueur réelle ≠ mesure affichée) | `\time n/d \set Timing.measureLength = #num/den` |
| `KeySig` | `\key <tonique> \major` ou `\minor` |
| `Clef` | `\clef treble`, `bass`, `alto`, `tenor`, `"treble_8"`, `varbaritone`, `percussion`… |
| `Measure.multi_measure_rest` | `R1*3`, `R2.*4`… |
| `Measure.start_bar_line`/`end_bar_line` | `\repeat volta` pour les reprises, `\bar "|."`, `\bar "||"`, `\bar ";"` |
| `section_break_before` | `\section` |
| `line_break` | `\break` (sauf dernière mesure) |
| `MeasureRepeatMark` | non rendu (la musique copiée est écrite) |
| barre de contrôle | `|` en fin de mesure, omise quand l'indication de mesure ne correspond pas à la longueur réelle |

### Accords, silences et durées

| Modèle | LilyPond |
| --- | --- |
| `Duration` + points | `4`, `2.`, `8..`, `\breve`, `\longa` |
| `ChordRest.ticks` ≠ figure | multiplicateur `4*3/2` |
| `Chord` à une note / plusieurs | `c'4` / `<c e g>4` |
| `Note.pitch` + `tpc` | nom orthographié (`cs`, `bf`) et octave absolue |
| `Note` transposée (`tpc_transposed`, instrument) | hauteur écrite, comme `pitch2xml` |
| altération de courtoisie (`user`, parenthèses, crochets) | suffixe `?` |
| silence | `r4` ; invisible (`print-object="no"`) → `s4` ; mesure entière → `R1` |
| `Tuplet` | `\tuplet 3/2 { … }`, imbrication par accolades |
| `Note.tie_forward` | `~` (sur l'élément du `<c~ e g>` pour un accord) |
| `GraceType` | `\grace`, `\acciaccatura`, `\appoggiatura` ; types `…After` → `\afterGrace` |
| `ArticulationType` | `-.` staccato, `-!` staccatissimo/spiccato, `->` accent, `-^` marcato, `--` tenuto, `-_` portato, `\espressivo`, `\fermata` et variantes, `\breathe`, `\caesura`, `\upbow`, `\downbow`, `\flageolet`, `\open`, `\stopped`, `\snappizzicato` |
| `OrnamentType` | `\trill`, `\turn`, `\reverseturn`, `\slashturn`, `\mordent`, `\prall`, `\prallprall`, `\haydnturn`, `\upmordent` |
| `Arpeggio` | `\arpeggio`, `\arpeggioArrowUp`/`Down`, `\arpeggioBracket` |
| `Tremolo` | `:8`, `:16`, `:32` (une barre = croche) |
| `Fingering` | `-1` pour un chiffre, `-\markup { "T" }` sinon |
| `ChordLine` | `\bendAfter`, seule commande standard (chute, montée, plop et scoop ne sont pas distingués par LilyPond) |
| `StemDirection` | `\once \override Stem.direction = #UP`/`#DOWN` |
| `NoteHeadGroup` | `\tweak style #'cross`, `#'harmonic-mixed`, `#'slash`, `#'xcircle`, `#'do`… |
| `Chord.grace_slash` | porté par `\acciaccatura` |

### Annotations

| Modèle | LilyPond |
| --- | --- |
| `Dynamic` prédéfinie (`p`, `mf`, `sfz`…) | `\p`, `\mf` (avec `_`/`^` selon le placement) |
| `Dynamic` personnalisée | `-\markup { "…" }` |
| `Tempo` | `\tempo "Allegro" 4 = 120` (le texte seul s'il n'y a pas de battements par seconde) |
| `StaffText`, `Expression`, `SystemText`, `InstrumentChange` | `^\markup { "…" }` / `_\markup` |
| `RehearsalMark` | `\mark \markup { "A" }` ; un second repère au même instant devient un `\markup` |
| `Harmony` | `^\markup { "Cm7" }` (le texte du modèle, pas `\chordmode`) |
| `Fermata` | `\fermata`, `\shortfermata`, `\longfermata`, `\verylongfermata`, `\henzelongfermata`, `\henzeshortfermata` |
| `TboxText` | `^\markup` sur la première note de la mesure |
| `FiguredBass`, `TremoloBar` | non rendus |

### Spanners

| Modèle | LilyPond |
| --- | --- |
| `Slur` | `\=N(` … `\=N)` (numéroté pour les liaisons qui se chevauchent) |
| `Hairpin` | `\<` … `\!` (ou `\cresc`/`\decresc` pour un soufflet en texte) |
| `Ottava` | `\ottava 1`/`-1`/`2`/`-2` … `\ottava 0` |
| `Pedal` | `\sustainOn` … `\sustainOff` |
| `Trill` | `\startTrillSpan` … `\stopTrillSpan` |
| `Glissando` | `\glissando` |
| `TextLine` | textes de début et de fin en `\markup`, trait non dessiné |
| `Volta` | structure `\alternative`, pas de commande propre |

### Paroles

Un bloc `\new Lyrics \lyricsto "<voix>"` par couplet présent dans la voix.
La syllabisation devient ` --` (début ou milieu de mot) et le mélisme ` __`.
Les accords sans syllabe pour un couplet donnent `\skip 1` pour garder
l'alignement ; les silences sont sautés par `\lyricsto` lui-même. Plusieurs
couplets reçoivent `\with { stanza = "n." }`.

### Reprises

Une barre de début et une barre de fin (`<endRepeat>N</endRepeat>`) donnent
`\repeat volta N`. Les volatas de fin et les endings suivants deviennent :

```lilypond
\repeat volta 2 {
  corps
}
\alternative {
  { premier ending }
  { second ending }
}
```

Le corps s'arrête au début du premier ending ; un ending qui occupe toute la
section tient lieu de corps. Une reprise sans second ending est écrite comme
une reprise simple (la musique est la même dans les deux lectures).

## 4. Rattachement des événements

Le modèle range les objets en segments verticaux ; une voix LilyPond est une
suite linéaire. Les règles appliquées, dans `src/voice.rs` :

1. les accords et silences forment la trame ; les trous sont comblés par des
   silences de la bonne longueur, pour que chaque voix couvre exactement la
   mesure ;
2. clé, armure, indication de mesure, levée et saut de section sont posés au
   début de la mesure concernée ;
3. les annotations et les spanners sont rattachés à une note : la **dernière
   note au plus tard au tick** pour une fermeture (`)`, `\!`) ou une nuance,
   la **première note au tick ou après** pour un tempo ou un repère ;
4. à tick égal, les fermetures passent avant les ouvertures (`\!` puis `\<`),
   pour qu'un soufflet qui s'arrête là où le suivant commence reste valide ;
5. un soufflet dont le départ et l'arrivée tombent sur le même silence est
   omis (LilyPond refuse une durée nulle).

## 5. Tests

- `tests/corpus_lilypond.rs` convertit les 56 fichiers `.mscx` du corpus v3/v4
  et compare le résultat aux instantanés `tests/corpus/ly/<version>_<nom>.ly`.
  Il vérifie aussi l'en-tête (`\version "2.24.0"`, `\language "english"`),
  la présence de `\fixed c'` et l'absence de `\relative`.
  Régénération après un changement volontaire de forme :

  ```sh
  nix develop -c env UPDATE_LILYPOND_SNAPSHOTS=1 \
      cargo test -p scoretk-lilypond --test corpus_lilypond
  ```

- `tests/lilypond_compile.rs` fait compiler chaque sortie par `lilypond` et
  échoue sur toute erreur. Le test ne fait rien si `lilypond` n'est pas dans
  le `PATH` (cas de `nix flake check`, qui travaille hors réseau) :

  ```sh
  nix develop -c cargo test -p scoretk-lilypond --test lilypond_compile -- --nocapture
  ```

État mesuré le 2026-11-17 : **56/56 conversions sans erreur de compilation**
avec LilyPond 2.27 (les instantanés sont en `\version "2.24.0"`, version
stable de référence ; LilyPond applique ses conversions habituelles).
Seul `v3/moonlight.mscx` produit des avertissements de mise en page
(« page 1 comprimée »), sans rapport avec la musique.

## 6. Limites connues

- **Pas de mise en page** : ni marges, ni sauts de système calculés, ni
  positions de textes. `\layout { }` laisse LilyPond décider.
- **Accords chiffrés en `\markup`**, pas en `\chordmode` : le texte du modèle
  est conservé, sans grille d'accords ni réalisation.
- **Têtes de note** : seuls les groupes non standard passent par
  `\tweak style` ; le remplissage et la forme liés à la durée
  (`NoteHeadType`) ne sont pas forcés.
- **Altérations** : LilyPond décide seul des altérations à afficher d'après le
  `tpc` et l'armure. Les altérations éditoriales ne sont pas distinguées des
  autres ; seules les altérations de courtoisie (`user`, parenthèses,
  crochets) reçoivent `?`.
- **Trémolo à deux notes** : écrit comme un trémolo sur chacune des deux
  notes (`:N`), pas en `\repeat tremolo`.
- **Ornements avec altération** : les `<accidental-mark>` ne sont pas rendus.
- **Articulations sans équivalent** (`stress`, `unstress`, `spiccato` exact,
  variantes d'accent doux) : approximation ou omission, documentée dans
  `src/chord.rs`.
- **N-olets** : regroupés par appartenance des accords ; un n-olet dont le
  modèle donne une fin incohérente est fermé à la mesure, LilyPond n'ayant pas
  de n-olet qui franchit une barre.
- **Reprises** : imbrication et enchaînement de plusieurs reprises sur la même
  mesure non couverts ; les numéros d'ending multiples (`1, 2`) ne changent
  pas le nombre de blocs `\alternative`.
- **Indications de mesure locales** : affichées, mais `Timing.measureLength`
  impose la longueur réelle de la mesure ; une portée en 3/4 dans une mesure
  de 9/8 n'est donc pas réengravée en trois mesures de 3/4.
- **Accords croisés** (`Chord.staff_move`) : non rendus, la note reste sur sa
  portée.
- **Percussions non accordées et micro-intervalles** : le modèle ne les porte
  pas encore.
- **Texte** : le balisage (`<b>`, `<font>`, symboles `<sym>`) est perdu ; les
  caractères de la zone à usage privé (symboles MuseScore) sont retirés pour
  éviter des glyphes manquants. Un tempo dont le texte n'est qu'un symbole
  métronomique (`♩ = 80`) perd son libellé et garde `4 = 80`.
- **Unité du tempo** : `\tempo` est toujours écrit avec la noire
  (`4 = bpm`), faute d'unité de battement dans le modèle ; un tempo à la
  blanche ou à la blanche pointée est donc noté avec la noire.
- **Tempo invisible** : une indication de tempo `print-object="no"` n'est pas
  écrite du tout (MuseScore en conserve pourtant l'effet sur la lecture).
- **Paroles** : pas de `\addlyrics`, seulement `\lyricsto` ; les syllabes
  d'un accord qui en porte plusieurs pour le même couplet sont réduites à la
  première.
