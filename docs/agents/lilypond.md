# LilyPond (`scoretk-lilypond`)

Crate `crates/scoretk-lilypond`, branchée dans la façade `scoretk` par la
feature `lilypond` (`Format::Ly` en écriture) et `lilypond-import` (`Format::Ly`
en lecture, t40). Ce document décrit l'écriture (correspondance modèle →
LilyPond), la lecture d'un sous-ensemble de LilyPond, les choix de notation, les
limites mesurées sur le corpus et la procédure de mise à niveau des moteurs
suivis. Le noyau du projet reste dans `AGENTS.md`.

## 1. Rôle et API

```rust
pub fn write_lilypond(score: &Score) -> Result<String, Error>;
pub fn read_lilypond(src: &str) -> Result<Score, Error>;
pub fn read_lilypond_reporting(src: &str) -> Result<(Score, Vec<Warning>), Error>;
```

La crate ne fait aucune entrée/sortie : elle renvoie le texte en mémoire, ce
qui la garde compilable pour `wasm32-unknown-unknown`, Android et iOS. Elle ne
dépend que de `scoretk-core` (modèle pivot) et `thiserror` ; aucune crate tierce
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
| Lecture : lexer, grammaire, arbre, interpréteur, construction | `src/read/` |
| Instantanés du corpus | `tests/corpus_lilypond.rs`, `tests/corpus/ly/` |
| Compilation par LilyPond | `tests/lilypond_compile.rs` |
| Aller-retour de lecture | `tests/read_corpus.rs` |
| Petites sources écrites à la main | `tests/read_lilypond.rs` |

Référence de comportement : **`musicxml2ly`** (paquet LilyPond,
GPL-3.0-or-later), qui définit les correspondances idiomatiques entre notation
MusicXML et LilyPond. MuseScore 4 n'a pas d'export LilyPond ; les tables
d'articulations, d'ornements, de nuances et de figures reprennent donc celles
de `musicxml2ly`, réécrites en Rust. L'attribution est dans `NOTICE` et dans
l'en-tête des fichiers concernés.

## 2. Lecture : méthode et sous-ensemble

LilyPond n'est pas intégrable : son parseur est un couple Bison/Flex
(`lily/parser.yy`, `lily/lexer.ll`) qui dépend de Guile Scheme pour presque
toutes ses actions sémantiques. La lecture est donc une **réécriture en Rust
d'un sous-ensemble de la grammaire**, sur le modèle de `python-ly`
(`ly.music`, `ly.lex`), qui lit LilyPond sans Scheme. Les sources de référence
sont une copie de LilyPond **2.26.0** conservée **hors du dépôt** (voir
`NOTICE` et § 8) : `lily/parser.yy`, `lily/lexer.ll`, `lily/pitch.cc` et
`scm/define-note-names.scm`.

| Étape | Fichier | Rôle |
| --- | --- | --- |
| Lexer | `src/read/lexer.rs` | jetons (`\nom`, mots, nombres, fractions, chaînes, ponctuation), commentaires `%` et `%{ %}`, états du lexer amont |
| Arbre | `src/read/ast.rs` | `music_list`, `sequential_music`, `simultaneous_music`, `note_chord_element`, `post_events`, `\repeat`, `\new`, paroles |
| Grammaire | `src/read/parser.rs` | analyse descendante des productions utiles ; tout le reste est ignoré avec un `Warning` |
| Hauteurs | `src/read/pitch.rs` | tables de `define-note-names.scm` (5 langues), octaves `\fixed`/`\relative`/absolues (`Pitch::to_relative_octave`), clés, armures |
| Interprétation | `src/read/events.rs` | octaves, transposition, n-olets, ornements, spanners, reprises, événements datés |
| Construction | `src/read/builder.rs` | trame des mesures (`\time`, `\partial`, `Timing.measureLength`, `R`), segments, altérations affichées, paroles, liaisons |

Sous-ensemble couvert : notes, silences, accords, durées et points,
`\relative`, `\fixed` et notation absolue, `\transpose`, `\tuplet`/`\times`,
`~`, `(`/`)` et `\=N(`/`\=N)`, `\repeat volta`/`unfold` et `\alternative`,
`\key`, `\time`, `\clef`, `\partial`, `\bar`, `\language` (néerlandais,
anglais, allemand, italien, français), nuances, `\< \> \!`, articulations et
ornements courants, `\grace`, `\acciaccatura`, `\appoggiatura`,
`\afterGrace`, `R`/`s`, voix `<< \\ >>`, variables (`nom = { … }` puis
`\nom`), `\new Staff`/`PianoStaff`/`ChoirStaff`/`Voice`/`Lyrics`,
`\addlyrics`, `\lyricsto`, `\lyricmode`, `\header`, `\tempo`, `\mark` et
`\markup` simple (texte).

Tout ce qui sort de ce sous-ensemble — Scheme `#…`/`$…`, commandes inconnues,
propriétés non modélisées — est **ignoré avec un avertissement collecté**
(`read_lilypond_reporting`), jamais une erreur : seule une source sans bloc
`\score` échoue.

## 3. Choix de notation

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

### Conventions d'écriture pour l'aller-retour

Trois conventions, choisies pour que le lecteur retrouve le modèle sans
ambiguïté, sont visibles dans les `.ly` produits :

- **Trous de voix** : un intervalle sans élément dans une voix du modèle
  s'écrit `s` (spacer), et non `r` ; un silence réel rendu invisible s'écrit
  `\once \override Rest.transparent = ##t r…`. Le lecteur distingue ainsi
  « pas d'élément » de « silence invisible ».
- **Nombre de n-olet** : `TupletNumberType::None` s'écrit
  `\override TupletNumber.stencil = ##f` … `\revert TupletNumber.stencil`,
  `Ratio` s'écrit `\override TupletNumber.text =
  #tuplet-number::calc-fraction-text` … `\revert`.
- **Métadonnées** : les champs non standard de `\header` (`workTitle`,
  `movementTitle`…) sont acceptés par LilyPond et permettent de restituer
  exactement les métadonnées du `.mscx`.

### Une variable par voix

```lilypond
staffOneVoiceOne = \fixed c' { … }
staffTwoVoiceTwo = \fixed c' { … }
```

Le nom sert à la fois de variable et de cible à `\lyricsto`. Les voix d'une
portée qui en compte plusieurs reçoivent `\voiceOne`, `\voiceTwo`… pour que
LilyPond gère hampes et collisions. Dans le bloc `\score`, une portée à
plusieurs voix et sans paroles utilise le séparateur idiomatique `\\` :

```lilypond
\new Staff = "staffOne" <<
  { \staffOneVoiceOne }
  \\
  { \staffOneVoiceTwo }
>>
```

Dès qu'une voix porte des paroles, les voix deviennent des `\new Voice`
nommés, seuls acceptés comme cible par `\lyricsto`.

### Regroupement des portées

D'après les accolades de MuseScore (`<bracket>`, champ `StaffBracket::kind`) :

| Crochet MuseScore | LilyPond |
| --- | --- |
| accolade (`1`) | `\new PianoStaff` |
| crochet, crochet carré (`0`, `3`) | `\new ChoirStaff` |
| plusieurs portées sans crochet | `\new StaffGroup` |
| une seule portée | `\new Staff` |

Les noms d'instrument passent par `\with { instrumentName = … shortInstrumentName = … }`.

## 4. Correspondance modèle → LilyPond

### Partition et mesures

| Modèle | LilyPond |
| --- | --- |
| `ScoreMetadata` (et crédits en repli) | `\header { title, workTitle, subtitle, movementTitle, movementNumber, workNumber, composer, poet, arranger, copyright, source }` ; `workTitle` n'est écrit que si le modèle porte un titre d'œuvre |
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

## 5. Rattachement des événements

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

## 6. Tests

- `tests/corpus_lilypond.rs` convertit les 56 fichiers `.mscx` du corpus v3/v4
  et compare le résultat aux instantanés `tests/corpus/ly/<version>_<nom>.ly`.
  Il vérifie aussi l'en-tête (`\version "2.24.0"`, `\language "english"`),
  la présence de `\fixed c'` et l'absence de `\relative`.
  Régénération après un changement volontaire de forme :

  ```sh
  nix develop -c env UPDATE_LILYPOND_SNAPSHOTS=1 \
      cargo test -p scoretk-lilypond --test corpus_lilypond
  ```

- `tests/read_corpus.rs` relit les 56 `.ly` du corpus et compare l'export
  MusicXML au `.mscx` d'origine, aux deux niveaux de `e2e_corpus`. Les listes
  `EXPECTED_CORE` et `EXPECTED_FULL` sont **non régressives** : un fichier qui
  y figure doit rester identique. Le test affiche les écarts des autres.

  ```sh
  nix develop -c cargo test -p scoretk-lilypond --test read_corpus -- --nocapture
  SHOW_DIFF=1 nix develop -c cargo test -p scoretk-lilypond --test read_corpus -- --nocapture
  ```

- `tests/read_lilypond.rs` couvre ce que l'écrivain n'émet pas : `\relative`,
  notation absolue, néerlandais par défaut, variables, reprises, `\transpose`,
  `\partial`, n-olets, ornements, paroles par `\addlyrics`, commandes
  ignorées avec avertissement.

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

### Aller-retour mesuré (t40)

État mesuré le 2026-11-17 par `tests/read_corpus.rs` : **24/56 fichiers
identiques au niveau du noyau musical** et **3/56 à parité complète**
(`v3_copypaste_tuplet_01`, `v3_grace`, `v3_lyricsline01`). Les 24 fichiers du
noyau sont listés dans `EXPECTED_CORE`. Les écarts restants ont trois causes,
toutes documentées :

1. **Informations absentes du `.ly`** (l'écrivain ne peut pas les écrire) :
   sous-type d'indication de mesure (`C`, `C barré` deviennent des chiffres),
   texte et numéros d'ending au-delà de la règle des passes, canaux et
   programmes MIDI explicites, distinction `part-name`/`instrument-name`,
   chiffrages d'accords (`\markup` au lieu de `<harmony>`), silences gap
   découpés en plusieurs `<forward>` (un seul est réémis), armure de mode
   inconnu, types d'ornements (`\grace` ne distingue pas 8/16/32) ;
2. **Mise en page** : positions, polices, sauts de système, largeurs
   (`<print>`, `default-x`, `relative-y`…) — hors périmètre, retirées au
   niveau complet ;
3. **Sous-ensemble non couvert** : `\bar` exotiques, modes anciens,
   percussions non accordées, micro-intervalles.

Le noyau identique prouve que hauteurs, durées, mesures, reprises, paroles,
liaisons, armures, clés et accords sont relus correctement sur plus d'un tiers
du corpus ; les fichiers restants servent de mesure des pertes de l'écrivain,
pas de régression du lecteur.

## 7. Limites connues

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

### Limites de la lecture

- **Sous-ensemble**, pas LilyPond complet : Scheme, `\include`, modes
  (`\drummode`, `\chordmode`, `\figuremode`), `\new` imbriqué dans une voix,
  `\partCombine`, `\book`, `\paper` sont ignorés avec un avertissement.
- **Pas de mise en page** : `\layout`, positions, portées masquées et sauts
  calculés ne sont pas relus.
- **`\relative`** : la règle de `Pitch::to_relative_octave` est portée, mais
  un accord dont les notes n'ont pas de marques se place par rapport à la
  dernière hauteur résolue (`\relative` ne s'applique qu'au premier son d'un
  accord dans LilyPond) ; les accords ambigus sont rapprochés de l'octave la
  plus proche.
- **`\transpose`** : l'intervalle est appliqué à la hauteur écrite (le
  modèle garde `pitch` concert et `tpc_transposed` écrit) sans déclarer
  d'instrument transpositeur ; un `\transpose` local change donc l'octave
  écrite et non l'armure.
- **Armures** : `\major` est lu comme `KeyMode::Unknown`, parce que MuseScore
  n'écrit `<mode>` que s'il est explicite et que l'écrivain écrit `\major`
  aussi bien pour un majeur explicite que pour une armure sans mode ; `\minor`
  reste mineur.
- **Indications de mesure** : les sous-types `C`/`C barré` ne sont pas
  distingués de `4/4` ; `\set Timing.measureLength` est suivi pour la trame
  des mesures.
- **Silences** : `R` devient un silence de mesure entière (`multiMeasureRest`
  quand il couvre plusieurs mesures) ; `s` (spacer) ne crée aucun élément,
  ce qui reproduit les trous du modèle MuseScore ; un silence rendu invisible
  s'écrit `\once \override Rest.transparent = ##t r…` et se relit comme
  `Rest::visible = false`. Les silences de mesure entière des voix autres que
  la première sont ignorés : l'écrivain les utilisait pour combler les trous.
- **Altérations** : les altérations affichées sont recalculées (armure et
  dernière altération de la mesure, par degré et octave) ; les altérations
  éditoriales et les `force-accidental` autres que `?` ne sont pas distinguées.
- **N-olets** : le nombre affiché est encodé par `\override
  TupletNumber.stencil = ##f` (aucun) ou `TupletNumber.text =
  #tuplet-number::calc-fraction-text` (ratio), relu par le parseur ; le
  crochet est déduit de `Tuplet::calcHasBracket` faute d'information dans
  `\tuplet`.
- **Paroles** : `\lyricsto`/`\addlyrics` sont attachés à la voix nommée si
  elle existe, sinon à la première voix de la portée ; `\skip` et les
  silences ne reçoivent pas de syllabe, et les syllabes en trop sont ignorées.
- **Reprises** : `\repeat volta N` avec k blocs `\alternative` produit les
  endings `i, i+k, i+2k…` (règle des passes) ; les imbrications de reprises et
  les numéros d'ending explicites hors de cette règle ne sont pas couverts.
- **Spanners** : les liaisons d'expression, soufflets, ottavas, pédales,
  trilles et glissandos sont relus ; leurs réglages de forme et de texte ne
  sont pas modélisés.

## 8. Mise à niveau de LilyPond (lecture)

Le lecteur suit la grammaire de **LilyPond 2.26.0**, commit
`3596756be0fac1f81066a99d2054329212172b2d`. La copie de référence est
**hors du dépôt** (comme `ref-musescore/`), par exemple
`../lilypond-ref/`, et n'est jamais versionnée.

**Récupérer la copie** (clone partiel, suffisant pour les fichiers portés) :

```sh
git clone --filter=blob:none --sparse \
    --branch v2.26.0 https://github.com/lilypond/lilypond.git ../lilypond-ref
git -C ../lilypond-ref sparse-checkout set --no-cone \
    '/lily/parser.yy' '/lily/lexer.ll' '/lily/pitch.cc' '/lily/pitch.hh' \
    '/lily/include/*' '/scm/define-note-names.scm' '/COPYING' '/VERSION'
git -C ../lilypond-ref rev-parse HEAD   # doit afficher 3596756b…
```

Le miroir officiel est <https://gitlab.com/lilypond/lilypond> (le dépôt
GitHub en est un miroir) ; la version stable est aussi dans nixpkgs
(`nix shell nixpkgs#lilypond`).

**Table des fichiers** :

| Rust (`crates/scoretk-lilypond/src/read/`) | LilyPond |
| --- | --- |
| `lexer.rs` | `lily/lexer.ll` (états `notes`, `lyrics`, `markup`, commentaires) |
| `parser.rs` | `lily/parser.yy` (`music_list`, `event_chord`, `chord_body`, `post_events`, `repeated_music`, `context_change`, `\score`) |
| `pitch.rs` | `lily/pitch.cc` (`Pitch::to_relative_octave`, `steps`, `up_to`/`down_to`), `scm/define-note-names.scm` (tables par langue) |
| `ast.rs`, `events.rs`, `builder.rs` | aucune source directe : sémantique des objets de `engraving/dom/` de MuseScore |

**Repérer les changements pertinents** :

```sh
git -C ../lilypond-ref fetch origin
git -C ../lilypond-ref log --oneline 3596756b..origin/master -- \
    lily/parser.yy lily/lexer.ll lily/pitch.cc scm/define-note-names.scm
git -C ../lilypond-ref diff 3596756b..origin/master -- lily/parser.yy
```

Surveiller en priorité : les productions `note_chord_element`,
`chord_body_element`, `post_events`, `duration`, `repeated_music`,
`alternative_music`, `context_change` ; les états et expressions régulières
du lexer (`SYMBOL`, `FRACTION`, `RESTNAME`, `MULTI_MEASURE_REST`,
`EXTENDER`, `HYPHEN`) ; les tables de noms (`define-note-names.scm`) et
`Pitch::to_relative_octave`. Les changements de mise en page, de Scheme ou de
backend ne concernent pas le lecteur.

**Après un portage**, régénérer et revalider :

```sh
nix develop -c cargo test -p scoretk-lilypond
nix develop -c cargo test -p scoretk-lilypond --test read_corpus -- --nocapture
nix develop -c cargo test -p scoretk-lilypond --test read_lilypond
nix develop -c cargo test -p scoretk-lilypond --test lilypond_compile -- --nocapture
nix flake check
```

Mettre à jour la liste `EXPECTED_CORE`/`EXPECTED_FULL` et le tableau
« Aller-retour mesuré » ci-dessus. **Quand le faire** : quand une version
stable de LilyPond change la syntaxe lue (nouvelle commande, lexer modifié) ou
quand le corpus contient un fichier écrit par une version plus récente ; pas
pour suivre les versions de développement.

## 9. Mise à niveau de python-ly

`python-ly` (<https://github.com/frescobaldi/python-ly>, GPL-3.0-or-later) a
servi de **modèle** pour un lecteur sans Scheme, pas de source portée : aucune
ligne n'est copiée. Version consultée : `master` du 2026-11-17 (licence
`LICENSES/GPL-3.0-or-later.txt`). Pour la revoir :

```sh
git clone --depth 1 https://github.com/frescobaldi/python-ly ../python-ly-ref
sed -n '1,80p' ../python-ly-ref/ly/music.py   # modèle d'événements
sed -n '1,80p' ../python-ly-ref/ly/lex.py     # lexer LilyPond sans Scheme
git -C ../python-ly-ref log -1 --format='%H %ad'
```

Elle n'est utile que pour comparer l'approche (événements datés, tolérance aux
commandes inconnues) : aucun fichier de ce dépôt n'en dépend, et une mise à
jour n'impose ni régénération ni test.

## 10. Mise à niveau de musicxml2ly (écriture)

L'écrivain ne porte aucun fichier C++ de MuseScore (MuseScore 4.7 n'a pas
d'export LilyPond) : il suit les **tables et le comportement** de
`musicxml2ly`, livré avec **LilyPond 2.26.0** (même commit
`3596756b…` que la lecture, cf. § 8). Sources Python, à ajouter à la copie
hors dépôt (jamais distribuées) :

```sh
git -C ../lilypond-ref sparse-checkout add --no-cone \
    '/scripts/musicxml2ly.py' '/python/musicexp.py' '/python/musicxml.py' \
    '/python/musicxml2ly_conversion.py' \
    '/python/musicxml2ly_definitions.py' '/python/musicxml2ly_globvars.py'
```

| Rust (`crates/scoretk-lilypond/src/`) | `musicxml2ly` |
| --- | --- |
| `pitch.rs` (noms, altérations, têtes de note) | `musicexp.py` (`NotestyleEvent.notehead_styles_dict`, noms de hauteurs) ; `musicxml.py` (lecture des `<pitch>`) |
| `chord.rs` (articulations, ornements, figures) | tables d'articulations et d'ornements de `musicexp.py` |
| `annotations.rs` (nuances) | `dynamics` de `musicexp.py` |
| `duration.rs`, `structure.rs`, `spanners.rs` | constructions `\repeat`, n-olets, spanners de `musicexp.py` |
| `writer.rs`, `voice.rs`, `text.rs` | trame `\score`/`\header`, ordre des événements et `\lyricsto` de `musicxml2ly.py` |

**Repérer les changements pertinents** :

```sh
git -C ../lilypond-ref fetch origin
git -C ../lilypond-ref log --oneline 3596756b..origin/master -- \
    python/musicexp.py python/musicxml2ly_conversion.py scripts/musicxml2ly.py
git -C ../lilypond-ref diff 3596756b..origin/master -- python/musicexp.py
```

Surveiller les dictionnaires de noms (`notehead_styles_dict`, noms
d'articulations, `dynamics`) et les constructions de sortie (`\\repeat`,
`\\tuplet`, `\\fixed`/`\\relative`, `\\lyricsto`). Les changements de mise en
page, d'import MusicXML ou de Scheme ne concernent pas l'écrivain.

**Après une mise à jour**, régénérer les instantanés puis revalider :

```sh
nix develop -c cargo test -p scoretk-lilypond --test read_corpus -- --nocapture
nix develop -c cargo test -p scoretk-lilypond --test lilypond_compile -- --nocapture
# si une convention d'écriture change (silences, n-olets, en-tête) :
nix develop -c env UPDATE_LILYPOND_SNAPSHOTS=1 \
    cargo test -p scoretk-lilypond --test corpus_lilypond
```

**Quand le faire** : quand une version stable de LilyPond change une
correspondance audible (nom de note, articulation, durée) ou qu'un `.ly`
produit n'est plus compilé par la version de référence ; pas pour suivre les
versions de développement ni les changements de mise en page.
