# LilyPond (`scoretk-lilypond-common`, `-import`, `-export`)

Trois crates : `crates/scoretk-lilypond-common` (ce que les deux sens
partagent : erreur, spirale des quintes, table des clefs),
`crates/scoretk-lilypond-export` (modèle → `.ly`, t36) et
`crates/scoretk-lilypond-import` (lecture `.ly` → modèle, t40). Elles sont
branchées dans la façade `scoretk` par les features `lilypond-export`
(`Format::Ly` en écriture) et `lilypond-import` (`Format::Ly` en lecture). Ce document décrit l'écriture (correspondance modèle →
LilyPond), la lecture d'un sous-ensemble de LilyPond, les choix de notation, les
limites mesurées sur le corpus et la procédure de mise à niveau des moteurs
suivis. Le noyau du projet reste dans `AGENTS.md`.

## 1. Rôle et API

```rust
// crates/scoretk-lilypond-export
pub fn write_lilypond(score: &Score) -> Result<String, Error>;
// crates/scoretk-lilypond-import
pub fn read_lilypond(src: &str) -> Result<Score, Error>;
pub fn read_lilypond_reporting(src: &str) -> Result<(Score, Vec<Warning>), Error>;
pub fn read_lilypond_with_includes(
    src: &str,
    resolve: &mut dyn FnMut(&str, Option<&str>) -> Option<String>,
) -> Result<(Score, Vec<Warning>), Error>;
// crates/scoretk (façade) : read_lilypond_with_includes(bytes, resolve) -> Result<Score>
// crates/scoretk-lilypond-common : `Error`, `Result`, `pitch`, `clefs`
```

`read_lilypond` et `read_lilypond_reporting` ne suivent pas les `\include` de
fichiers ordinaires : ils les ignorent avec un avertissement. La fonction
`read_lilypond_with_includes` reçoit un résolveur
`resolve(chemin_demandé, chemin_du_fichier_qui_inclut)` qui renvoie le texte du
fichier inclus ou `None` ; le second argument vaut `None` au niveau supérieur,
puis le chemin normalisé du fichier inclus (relatif au fichier principal). La
CLI fournit un résolveur disque (dossier du fichier qui inclut, puis dossier du
fichier principal) ; la bibliothèque n'ouvre aucun fichier elle-même.

Aucune des trois crates ne fait d'entrée/sortie : elles travaillent sur du
texte ou des valeurs en mémoire, ce qui les garde compilables pour
`wasm32-unknown-unknown`, Android et iOS. Elles ne dépendent que de
`scoretk-core` (modèle pivot), de `scoretk-lilypond-common` pour les deux sens
et de `thiserror` ; aucune crate tierce nouvelle, donc rien à ajouter à
`docs/licences-tierces.md`.

**Règle de découpage** : `-common` ne contient **aucune logique de
conversion**, seulement des tables et des types utilisés par les deux sens.
`src/pitch.rs` (lecture et écriture) est donc coupé : la spirale des quintes
(`tpc`, degré, altération) va dans `scoretk-lilypond-common/src/pitch.rs`, la
table des clefs dans `scoretk-lilypond-common/src/clefs.rs` ; restent dans
l'écriture les noms de notes anglais du mode `\fixed`, les armures et les têtes
de note, et dans la lecture les tables de noms par langue, les octaves et la
transposition. `text.rs` reste entièrement dans l'écriture : la lecture ne
s'en sert pas.

| Élément | Emplacement |
| --- | --- |
| Erreur, `Result` | `crates/scoretk-lilypond-common/src/error.rs` |
| Spirale des quintes (`tpc`) | `crates/scoretk-lilypond-common/src/pitch.rs` |
| Table des clefs (deux sens) | `crates/scoretk-lilypond-common/src/clefs.rs` |
| API d'écriture, documentation | `crates/scoretk-lilypond-export/src/lib.rs` |
| Hauteurs anglaises, armures, têtes de note | `crates/scoretk-lilypond-export/src/pitch.rs` |
| Durées, points, multiplicateurs | `crates/scoretk-lilypond-export/src/duration.rs` |
| Accords, silences, articulations, ornements | `crates/scoretk-lilypond-export/src/chord.rs` |
| Annotations (nuances, tempo, textes, accords) | `crates/scoretk-lilypond-export/src/annotations.rs` |
| Spanners (liaisons, soufflets, ottavas, pédales, trilles, glissandos) | `crates/scoretk-lilypond-export/src/spanners.rs` |
| Structure des reprises (`\repeat volta`, `\alternative`) | `crates/scoretk-lilypond-export/src/structure.rs` |
| Trame d'une voix, paroles, rattachement des événements | `crates/scoretk-lilypond-export/src/voice.rs` |
| En-tête, variables, bloc `\score`, regroupements | `crates/scoretk-lilypond-export/src/writer.rs` |
| API de lecture | `crates/scoretk-lilypond-import/src/lib.rs` |
| Lecture : lexer, grammaire, arbre, interpréteur, construction | `crates/scoretk-lilypond-import/src/` |
| Instantanés du corpus (écriture) | `crates/scoretk-lilypond-export/tests/corpus_lilypond.rs`, `tests/corpus/ly/` |
| Compilation par LilyPond | `crates/scoretk-lilypond-export/tests/lilypond_compile.rs` |
| Aller-retour de lecture | `crates/scoretk-lilypond-import/tests/read_corpus.rs` |
| Petites sources écrites à la main | `crates/scoretk-lilypond-import/tests/read_lilypond.rs` |
| Unités des tables partagées | `#[cfg(test)]` de `scoretk-lilypond-common` |

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
| Lexer | `scoretk-lilypond-import/src/lexer.rs` | jetons (`\nom`, mots, nombres, fractions, chaînes, ponctuation), commentaires `%` et `%{ %}`, états du lexer amont |
| `\include` | `scoretk-lilypond-import/src/include.rs` | expansion textuelle avant l'analyse : fichiers de langue → `\language`, fichiers livrés sans musique ignorés, autres fichiers par le résolveur |
| Arbre | `scoretk-lilypond-import/src/ast.rs` | `music_list`, `sequential_music`, `simultaneous_music`, `note_chord_element`, `post_events`, `\repeat`, `\new`, paroles |
| Grammaire | `scoretk-lilypond-import/src/parser.rs` | analyse descendante des productions utiles ; tout le reste est ignoré avec un `Warning` |
| `\parallelMusic` | `scoretk-lilypond-import/src/parallel.rs` | découpage aux contrôles de mesure et répartition entre les variables (`ly/music-functions-init.ly`) |
| Hauteurs | `scoretk-lilypond-import/src/pitch.rs` (+ `clefs`/`tpc` de `-common`) | tables de `define-note-names.scm` (toutes les langues), octaves `\fixed`/`\relative`/absolues (`Pitch::to_relative_octave`), clés, armures |
| Interprétation | `scoretk-lilypond-import/src/events.rs` | octaves, transposition, n-olets, ornements, spanners, reprises, événements datés |
| Construction | `scoretk-lilypond-import/src/builder.rs` | trame des mesures (`\time`, `\partial`, `Timing.measureLength`, `R`), segments, altérations affichées, paroles, liaisons |

Sous-ensemble couvert : notes, silences (y compris le post-événement `\rest`,
qui transforme la note écrite en silence, comme `event_chord` de
`lily/parser.yy`), accords, durées et points,
`\relative`, `\fixed` et notation absolue, `\transpose`, `\tuplet`/`\times`,
`~`, `(`/`)` et `\=N(`/`\=N)`, `\repeat volta`/`unfold` et `\alternative`,
`\key`, `\time`, `\clef`, `\partial`, `\bar`, `\cadenzaOn`/`\cadenzaOff`
(et `\set Timing.timing = ##f/##t`), `\language` (toutes les langues de
`language-pitch-names` : néerlandais, catalan, allemand, anglais, espagnol,
français, italien, norvégien, portugais, finnois, suédois, flamand), `\include`
des fichiers de langue et des fichiers livrés sans musique, nuances,
`\< \> \!`, articulations et
ornements courants, `\grace`, `\acciaccatura`, `\appoggiatura`,
`\afterGrace`, `R`/`s`, voix `<< \\ >>` (passages polyphoniques
temporaires) et `\new Voice` rencontré dans une voix d'une portée (voix
ouverte au tick courant), variables (`nom = { … }` puis
`\nom`), `\parallelMusic` (voir ci-dessous),
`\new Staff`/`PianoStaff`/`ChoirStaff`/`Voice`/`Lyrics`,
`\context Staff`/`PianoStaff`/`ChoirStaff`/`StaffGroup`/`Voice` (noms de
contextes insensibles à la casse : `\new voice`, `\context staff`…),
`\addlyrics`, `\lyricsto`, `\lyricmode`, `\header`, `\tempo`, `\mark` et
`\markup` simple (texte), `\chordmode` dans une portée (les noms d'accords
sont **joués** : `c:m7` → MIDI 60/63/67/70, tables de `scm/chord-entry.scm`),
`\drummode` et `\new DrumStaff` (noms de fûts → hauteurs MIDI de
`ly/drumpitch-init.ly`), musique embarquée `#{ … #}`/`$#{ … #}` et fonctions
musicales simples (`nom = #(define-music-function (a b) (prédicat…) #{ … #})`
puis `\nom arg…`). Une expression musicale écrite directement au
niveau supérieur du fichier (partition implicite de `toplevel-music-handler`)
est lue comme un `\score { musique }` sans `\layout` ni `\midi` ; chaque
expression de haut niveau forme sa propre partition, comme LilyPond (qui en
produit un fichier MIDI par expression) ;
`\book`/`\bookpart` sont parcourus pour y chercher `\score`, `\header` et
cette musique de haut niveau.

**`\parallelMusic`** (`ly/music-functions-init.ly`, fonction `parallelMusic`) :
la musique est coupée aux contrôles de mesure `|` et les tranches sont
affectées circulairement aux variables nommées, une par voix ; c'est
l'idiome des grilles d'accords (`\parallelMusic #'(PianoI PianoII …)`, ou
`\parallelMusic haut,bas` depuis 2.24). Les contrôles de mesure ne sont pas
recopiés dans les voix : le lecteur les ignore de toute façon (`|` ne coupe
pas la trame), et un contrôle de mesure imbriqué (`\repeat`, bloc `{ … }`)
ne sépare pas les voix — LilyPond, lui, découpe aussi les éléments
imbriqués. Sans aucun `|`, la musique est ignorée avec un avertissement,
comme LilyPond.

Tout ce qui sort de ce sous-ensemble — commandes inconnues, propriétés non
modélisées — est **ignoré avec un avertissement collecté**
(`read_lilypond_reporting`), jamais une erreur : seule une source sans
aucune musique (`\score` ou expression de haut niveau) échoue. Les
expressions Scheme `#…`/`$…` font exception : elles sont lues **d'un seul
tenant** (un datum complet, comme le lecteur Guile appelé par le lexer
amont) et ignorées sans bruit. Un `#{ … #}` qui n'appartient pas à un datum
Scheme est en revanche de la **musique** : le lexer en fait un jeton à part
(`Kind::Embedded`) et le parseur relit son contenu comme une séquence, en
respectant les chaînes et commentaires LilyPond intérieurs.

Une définition `#(define-music-function (a b) (prédicat… ) #{ corps #})` est
mémorisée (signature ancienne `(parser location a b)` comprise) ; chaque
appel `\nom arg…` consomme un argument par prédicat (musique `{…}`, note,
nombre, chaîne, symbole Scheme, datum) puis rejoue le corps en remplaçant
les jetons `#a`, `$a` et `\a` par l'argument : c'est une substitution de
jetons, pas une évaluation Scheme. Un corps sans `#{ … #}` laisse la
fonction inconnue (avertissement unique au premier appel, arguments
consommés) ; `define-void-function` et `define-scheme-function` sont
consommées sans musique ni avertissement de « mot ignoré ».

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
| `Measure.len` irrégulière (hors levée) | `\set Timing.measureLength = #num/den` avant la mesure, puis restauration à la métrique en vigueur |
| `KeySig` | `\key <tonique> \major` ou `\minor` |
| `Clef` | `\clef treble`, `bass`, `alto`, `tenor`, `"treble_8"`, `varbaritone`, `percussion`… |
| `Staff.default_clef` sans `Clef` en tête de première mesure | `\clef …` au début de la voix 0 ; `read4xx` ne matérialise pas de segment `HeaderClef`, contrairement à `read302` |
| `Measure.multi_measure_rest` | `R1*3`, `R2.*4`… |
| `Measure.start_bar_line`/`end_bar_line` | `\repeat volta` pour les reprises, `\bar "|."`, `\bar "||"`, `\bar ";"` |
| `section_break_before` | `\section` |
| `line_break` | `\break` (sauf dernière mesure) |
| `MeasureRepeatMark` | non rendu (la musique copiée est écrite) |
| barre de contrôle | `|` en fin de mesure quand la longueur réelle correspond à la métrique en vigueur, héritée comprise ; `Timing.measureLength` est posé sinon, jamais un `\time` parasite |

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
couplets reçoivent `\set stanza = "n."` **dans** le bloc de paroles : c'est la
forme de `musicxml2ly`, compilable par LilyPond 2.26 (contrairement à
`\stanza "n."`, fonction musicale ajoutée en 2.27). Le
trait d'union n'est émis que si l'accord suivant du même couplet porte une
syllabe, et les continuations de liaison (toutes notes liées depuis l'accord
précédent) ne reçoivent pas de jeton, car `\lyricsto` les saute.

Côté lecture, un mot de paroles est une suite de caractères non blancs
(`<lyrics>` de `lily/lexer.ll`, `lexer::mot_de_paroles`) : ponctuation collée
(`bon,`, `gne.`), apostrophe (`qu'il`, `l’ami`) et guillemets typographiques
en font partie, et un `_` **dans** le mot devient une espace (`lyric_fudge`
amont) : `Ah_!` est une seule syllabe « Ah ! ». Seul un `_` isolé entre blancs
(`__` pour le trait) reste un mélisme ; un `~` sépare les mots (élision). Les
chiffres sont hors du mot : `la4` donne la durée de la syllabe. À l'écriture,
une syllabe qui porte une espace est mise entre guillemets (`"Ah !"`), seule
forme qui la relise comme un seul mot.

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

Répartition : un test qui n'exerce qu'un sens vit dans la crate de ce sens ; un
aller-retour (`.ly` → modèle → `.ly`, ou lecture puis export) vit dans
`crates/scoretk/tests/` (façade) ou dans le crate d'intégration. Aucune des
deux moitiés ne dépend de l'autre : la lecture compare au MusicXML du `.mscx`
d'origine, pas à une réécriture LilyPond.

- `crates/scoretk-lilypond-export/tests/corpus_lilypond.rs` convertit les 56 fichiers `.mscx` du corpus v3/v4
  et compare le résultat aux instantanés `tests/corpus/ly/<version>_<nom>.ly`.
  Il vérifie aussi l'en-tête (`\version "2.24.0"`, `\language "english"`),
  la présence de `\fixed c'` et l'absence de `\relative`.
  Régénération après un changement volontaire de forme :

  ```sh
  nix develop -c env UPDATE_LILYPOND_SNAPSHOTS=1 \
      cargo test -p scoretk-lilypond-export --test corpus_lilypond
  ```

- `crates/scoretk-lilypond-import/tests/read_corpus.rs` relit les 56 `.ly` du corpus et compare l'export
  MusicXML au `.mscx` d'origine, aux deux niveaux de `e2e_corpus`. Les listes
  `EXPECTED_CORE` et `EXPECTED_FULL` sont **non régressives** : un fichier qui
  y figure doit rester identique. Le test affiche les écarts des autres.

  ```sh
  nix develop -c cargo test -p scoretk-lilypond-import --test read_corpus -- --nocapture
  SHOW_DIFF=1 nix develop -c cargo test -p scoretk-lilypond-import --test read_corpus -- --nocapture
  ```

- `crates/scoretk-lilypond-import/tests/read_lilypond.rs` couvre ce que l'écrivain n'émet pas : `\relative`,
  notation absolue, néerlandais par défaut, variables, reprises, `\transpose`,
  `\partial`, n-olets, ornements, paroles par `\addlyrics`, commandes
  ignorées avec avertissement, `\drummode`/`\new DrumStaff` (deux portées,
  silences, durées, nom inconnu, contrôle sans mode) et `\chordmode` joué dans
  une portée (accords, basse, variable, `\relative`, contrôle `\chords`).

- `crates/scoretk-lilypond-export/tests/lilypond_compile.rs` fait compiler
  chaque sortie par `lilypond` et échoue sur toute erreur. Il vit dans la crate
  d'écriture : il ne compile que du `.ly` **produit** par l'écrivain, jamais une
  source lue, et le lecteur n'y intervient pas. Le test ne fait rien si
  `lilypond` n'est pas dans le `PATH` (cas de `nix flake check`, qui travaille
  hors réseau) :

  ```sh
  nix develop -c cargo test -p scoretk-lilypond-export --test lilypond_compile -- --nocapture
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
  de n-olet qui franchit une barre. Le comblement de fin de mesure utilise la
  durée réelle (`DurationElement::actualTicks`, ratio des n-olets compris), et
  non la seule durée notée.
- **Reprises** : imbrication et enchaînement de plusieurs reprises sur la même
  mesure non couverts ; les numéros d'ending multiples (`1, 2`) ne changent
  pas le nombre de blocs `\alternative`.
- **Indications de mesure locales** : affichées, mais `Timing.measureLength`
  impose la longueur réelle de la mesure ; une portée en 3/4 dans une mesure
  de 9/8 n'est donc pas réengravée en trois mesures de 3/4. Les mesures
  irrégulières sans `<TimeSig>` local (3/4 sous un 4/4 hérité) reçoivent de
  même un `measureLength` temporaire, restauré à la mesure régulière suivante.
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
  première. Une syllabe portée par une continuation de liaison est perdue,
  `\lyricsto` ne l'attachant pas à cette note.

### Limites de la lecture

- **Sous-ensemble**, pas LilyPond complet : Scheme, `\figuremode`,
  `\partCombine`, `\paper` sont ignorés avec un avertissement. `\chords` /
  `\new ChordNames` reste une grille d'affichage non jouée (comme avant) ;
  un export conforme écrirait `<harmony>` pour ces grilles, ce que le modèle
  pivot sait porter mais que le lecteur ne produit pas encore. `\book` et
  `\bookpart` sont parcourus (leur `\header`, leurs `\score` et leur musique
  de haut niveau).
- **`\context Staff`** : retrouvé au niveau d'une partition, il ouvre une
  portée (les partitions d'orchestre `\context StaffGroup << \context Staff =
  "haut" … >>` sont ainsi lues en autant de portées). Retrouvé dans la
  musique d'une portée déjà ouverte, il retrouve cette portée et ses voix y
  sont ouvertes. Quand son contenu porte des `\new Voice`/`\context Voice`
  nommés ou une enveloppe `\relative`/`\fixed`/`\transpose`, il est traversé
  comme avant le portage des contextes structurels : les voix restent sur la
  portée englobante (idiome du piano
  `\context Voice = "one" \rightHand`) et la référence relative s'applique à
  tout le bloc polyphonique (`\context Staff \relative << … \\ … >>` de
  collisions.ly). En revanche, un `\context Staff = "nom"` dont le contenu
  porte plus de quatre voix nommées (pathetique-1 de Mutopia) ouvre bien sa
  portée et répartit les voix surnuméraires en portées sœurs : les regrouper
  dans une seule portée les ferait écraser par la limite de quatre voix.
- **`\rest` et `\relative`** : la hauteur écrite d'un `\rest` est résolue et
  avance la référence relative, comme le `RestEvent` de `event_chord` dans
  `lily/parser.yy` (vérifié sur `d'2\rest d16\rest g, c e g` de bwv651.ly
  contre le MIDI de LilyPond 2.27). La baisse du nombre de « notes communes »
  observée après avoir traité `\rest` comme un silence n'est donc **pas** un
  changement de hauteur : les 124 notes retirées sont exactement les silences
  écrits, et 61 d'entre elles portaient une hauteur présente dans le MIDI de
  référence (mesure par intersection de multiensembles de hauteurs). Aucune
  note jouée ne change de hauteur.
- **Voix ouvertes dans une voix** : `\new Voice` (ou `\context Voice`) et
  chaque branche d'un `<< … >>` rencontrés dans la musique d'une voix ouvrent
  une voix de la portée au tick courant (`context_prefix` /
  `NEWCONTEXT`, `lily/parser.yy` ; « Temporary polyphonic passages ») ; la
  voix d'origine avance ensuite de la durée du contexte ouvert, comme
  LilyPond. Une simultanée sans `\\` est lue en voix sœurs plutôt qu'en
  accords fusionnés. Les branches anonymes de passages temporaires successifs
  partagent la même piste tant qu'elles ne se chevauchent pas : une pièce à
  nombreux passages garde deux voix au lieu d'en ouvrir une par passage et de
  dépasser le modèle. Les voix sans accord (préambule `\clef`/`\key`) sont
  repliées dans la première voix musicale, et une portée reste limitée à
  quatre voix (les suivantes avertissent et sont ignorées).
- **`\change`** : `\change <Contexte> = <nom>` est lu comme un changement de
  portée : les notes suivantes gardent leur voix et leur place et portent un
  `Chord.staff_move` calculé d'après la position de `\new Staff = "<nom>"`.
  Le type de contexte n'est pas vérifié ; comme `Chord::staffMove` de MuseScore,
  la résolution se limite aux portées nommées de la même partie. Un nom inconnu
  ou étranger à la partie avertit une seule fois par voix, à la position du
  `\change`, et laisse les notes sur leur portée d'origine.
- **Une seule partition** : quand un fichier en contient plusieurs (`\score`
  explicites ou expressions de haut niveau successives), le premier `\score`
  explicite l'emporte, sinon la première expression ; chaque autre est ignorée
  avec un avertissement. C'est le cas de `regression/trill-spanner-consecutive.ly`
  (`\test 2.0 \test 2.1 \test 2.2 \test 2.3`), dont seule la première
  expression (2 notes) est retenue. Le MusicXML représente la partition gravée :
  même si un `\score` ultérieur porte un `\midi` (partition que LilyPond joue,
  transposée), c'est le premier `\score` gravé qui fait foi ; une éventuelle
  différence avec la référence MIDI est une limite de l'outil de mesure.
  `\articulate` (`ly/articulate.ly`, fonction musicale prenant la musique
  suivante) est transparent : il ne change pas les hauteurs et laisse passer
  les enveloppes `\transpose`/`\unfoldRepeats`.
- **Fonctions musicales** : seules les définitions
  `#(define-music-function …)` à corps `#{ … #}` produisent de la musique.
  La substitution est purement lexicale (`#a`, `$a`, `\a`) : pas d'appel
  Scheme, pas de valeur calculée, pas de fonction musicale définie dans un
  `\include` non résolu. Le nombre d'arguments suit les prédicats ; un
  prédicat inconnu se lit comme un jeton (ou un bloc `{…}`/`<<…>>`). La
  profondeur d'expansion des blocs et des appels est bornée à 32, avec
  avertissement au-delà ; les arguments restants sont consommés sans être
  interprétés.
- **`\include`** : l'inclusion est textuelle et faite sur les jetons, comme le
  lexer amont, au niveau supérieur comme dans la musique. Un fichier de langue
  livré par LilyPond vaut `\language`, les fichiers livrés sans musique
  (`articulate.ly`, `gregorian.ly`, `arabic.ly`…) sont ignorés sans
  avertissement. Les autres fichiers passent par le résolveur de
  `read_lilypond_with_includes` (la CLI lit le disque) ; sans résolveur, ils
  sont ignorés avec un avertissement. Profondeur maximale 32, cycles
  détectés, `\include` d'une expression Scheme ignoré avec un avertissement.
  La position des avertissements du fichier principal est conservée ; ceux des
  fichiers inclus portent leur propre ligne, sans nom de fichier.
- **Pas de mise en page** : `\layout`, positions, portées masquées et sauts
  calculés ne sont pas relus.
- **`\relative`** : la règle de `Pitch::to_relative_octave` est portée
  (`lily/pitch.cc`). Dans un accord, chaque note se place par rapport à la
  précédente, mais la hauteur rendue pour la suite est celle de la première
  note (`Event_chord::to_relative_octave`) ; `q` répète les hauteurs déjà
  résolues du dernier accord, pas ses marques écrites. Une hauteur hors de
  0..127 est écartée avec un avertissement. Les branches d'un `<< … >>` et
  les contextes de la séquence sont parcourus dans l'ordre de
  `Music_sequence::simultaneous_relative_callback` (`lily/music-sequence.cc`) :
  chaque branche part de la dernière note de la précédente, et la musique qui
  suit le `>>` de la dernière note de la **dernière** branche (les lire depuis
  la note qui précède le `<<` décalait la suite d'une octave).
- **`\transpose`** : LilyPond transpose la musique écrite elle-même ;
  `pitch`, `tpc` et `tpc_transposed` portent donc tous la hauteur transposée
  et l'armure (`\key`) est transposée de même (`\key c \major` sous
  `\transpose c d` donne ré majeur), sans déclarer d'instrument transpositeur.
  Les transpositions imbriquées se composent, pour les notes comme pour
  l'armure.
- **Armures** : `\major` est lu comme `KeyMode::Unknown`, parce que MuseScore
  n'écrit `<mode>` que s'il est explicite et que l'écrivain écrit `\major`
  aussi bien pour un majeur explicite que pour une armure sans mode ; `\minor`
  reste mineur.
- **Indications de mesure** : les sous-types `C`/`C barré` ne sont pas
  distingués de `4/4` ; `\set Timing.measureLength` est suivi pour la trame
  des mesures.
- **Cadences** : `\cadenzaOn`/`\cadenzaOff` et `\set Timing.timing = ##f`/`##t`
  suspendent les barres automatiques. La mesure en cours s'allonge jusqu'au
  `\cadenzaOff` ou à la première barre explicite `\bar "…"` ; sa durée
  nominale reste celle du `\time`, ce qui produit une mesure irrégulière
  (`len != nominal_len`), comme MuseScore pour une mesure de durée réelle.
  La cadence peut être coupée en plusieurs mesures par des barres explicites
  `\bar "…"` ; elle reprend ensuite sur la métrique en vigueur après
  `\cadenzaOff`. Le contrôle de mesure `|` (bar check de `lily/parser.yy`,
  `'|'` → `BarCheck`) est consommé et ignoré : il ne crée jamais de barre ni
  de coupure, même en cadence. Une barre `\bar "|"` ordinaire a la valeur par
  défaut du modèle et ne modifie pas une barre plus forte placée au même tick.
- **Cadences et voix multiples** : les mesures étant communes à toutes les
  voix, on choisit l'union des intervalles de cadence : dès qu'une voix est
  en cadence, aucune barre automatique n'est insérée pour la partition
  entière pendant cet intervalle, et la mesure est allongée jusqu'à la
  fermeture de la cadence la plus proche. C'est le comportement le plus
  simple ; une cadence propre à une voix ne découpe donc pas les autres voix.
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
  Comme LilyPond, `\lyricsto` saute les notes occupées : celles à l'intérieur
  d'une liaison d'expression `( … )` (sauf la note qui l'ouvre) et les
  continuations de `~`. `\set melismaBusyProperties = #'()` désactive ces
  sauts ; `beamMelismaBusy` n'est pas modélisé car il ne s'active que si
  `autoBeaming = ##f` (`lily/beam-engraver.cc`). Le rang de couplet est lu
  dans `\set stanza = "n."` (forme écrite par l'écrivain, compilable dès
  2.26), dans `\stanza "n."` (LilyPond 2.27) et, pour compatibilité, dans
  l'ancien `\with { stanza = "n." }`.
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

| Rust (`crates/scoretk-lilypond-import/src/`) | LilyPond |
| --- | --- |
| `lexer.rs` | `lily/lexer.ll` (états `notes`, `lyrics`, `markup`, commentaires) |
| `parser.rs` | `lily/parser.yy` (`music_list`, `event_chord`, `chord_body`, `post_events`, `repeated_music`, `context_change`, `\score`) |
| `pitch.rs` | `lily/pitch.cc` (`Pitch::to_relative_octave`, `steps`, `up_to`/`down_to`), `scm/define-note-names.scm` (tables par langue) |
| `parallel.rs` | `ly/music-functions-init.ly` (fonction `parallelMusic`) |
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
nix develop -c cargo test -p scoretk-lilypond-common -p scoretk-lilypond-import
nix develop -c cargo test -p scoretk-lilypond-import --test read_corpus -- --nocapture
nix develop -c cargo test -p scoretk-lilypond-import --test read_lilypond
nix develop -c cargo test -p scoretk-lilypond-export --test lilypond_compile -- --nocapture
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

| Rust (`crates/scoretk-lilypond-export/src/`) | `musicxml2ly` |
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
nix develop -c cargo test -p scoretk-lilypond-import --test read_corpus -- --nocapture
nix develop -c cargo test -p scoretk-lilypond-export --test lilypond_compile -- --nocapture
# si une convention d'écriture change (silences, n-olets, en-tête) :
nix develop -c env UPDATE_LILYPOND_SNAPSHOTS=1 \
    cargo test -p scoretk-lilypond-export --test corpus_lilypond
```

**Quand le faire** : quand une version stable de LilyPond change une
correspondance audible (nom de note, articulation, durée) ou qu'un `.ly`
produit n'est plus compilé par la version de référence ; pas pour suivre les
versions de développement ni les changements de mise en page.

## 11. Empty-output audit (2026-11-19)

A corpus-wide comparison of written pitches against LilyPond MIDI references
found 97 `.ly` files that produced no notes although the reference did (76
`input/regression` tests and 21 Mutopia scores, 6925 reference notes in total).
Causes were grouped by yield (files × reference notes) and handled in that
order.

### Fixed: unknown commands swallowed the following music block

`skip_command_arguments` treated a `{ … }` block after an unknown command as
that command's argument. No-argument commands such as `\autoBeamOff`,
`\dynamicUp`, `\dynamicDown`, `\oneVoice` and `\compressMMRests` are ubiquitous
in the Mutopia idiom

```lilypond
\context Voice = Sopran { \voiceOne << \autoBeamOff \dynamicUp { \global \voice } >> }
```

so an ignored command at the head of the simultaneous block ate the only music
branch and the whole voice vanished. In music context the braced block is now
left in the stream; at top level, where `\layout`/`\paper` carry a settings
block, it is still skipped. This recovered 5198 of 5202 notes in all 21 Mutopia
scores, plus five regression files; 5247 notes now match in total. A reduced
test covers two staves, two named voices, variables, bar checks, chords, rests
and ties, plus an unknown command at end of file. Twenty previously exact files
were re-measured and are unchanged.

A later measurement on non-regression scores caught a side effect: the
`musicxml2ly` idiom `\mark \markup { \box { A } }` left the `\markup` token in
the stream, so its braced block was parsed as music and the letter `A` became a
spurious note. `\mark` now consumes `\markup` before calling
`parse_markup_text`, which drains the whole block (including nested braces).
`Angels_Carol.ly` recovers its 12 shared notes and now also exports its eight
rehearsal marks; the no-argument-command fix above is unchanged.

### Not fixed: causes abandoned as disproportionate

The remaining 71 regression files (1665 reference notes) fall into groups that
would each require a feature of their own, well beyond a parser fix:

- ~~**Chord grids played as notes**~~ **Fixed in §12**: `\chordmode` inside a
  `Staff` now produces the played notes via a chord-name-to-pitch table
  (`c:m7`, `c/g`, `dim7`, …); `\chords`/`ChordNames` remain display-only grids.
- ~~**Drum pitches**~~ **Fixed in §12**: `\drummode` abbreviations (`bd`, `sn`,
  `hh`, `cymr`, …) now map through LilyPond's `drumpitch-init.ly` tables. The
  pivot model still has no untuned-percussion pitch, so the MusicXML writer
  emits `<pitch>` with the GM MIDI note number; §12 documents how the oracle
  compares those notes to LilyPond's MIDI.
- **Missing installed templates** (7 SATB/SSAATTBB files, ~360 notes). They end
  in `\include "satb.ly"` / `"ssaattbb.ly"` and contain only variable
  definitions; the real `\score` lives in LilyPond's installed `ly/` directory.
  The CLI resolver searches the input file's directory and the library must
  remain I/O-free, so the include cannot be resolved here.
- **Scheme-generated music and partial music functions** (`ledger-line-properties`,
  `note-head-style`, `beat-performer`, `markup-map`, `parallelmusic-partial`, …).
  These rely on `$@(map …)`, `#{ … #}` splicing, `\etc` partial definitions or
  the drum tremolo repeat `\*`; supporting them would mean embedding a Scheme
  evaluator.
- **Small leftovers** (~15 files, ~90 notes): quoted variable names
  (`"|" = \bar "||"`), `\with` followed by a context modifier instead of a
  braced block, braced markup arguments inside property values, and a spurious
  empty first partition that shadows the real one. Each affects one or two
  notes and does not justify widening the subset.

These files are expected to keep producing zero notes; the reader's narrow,
Scheme-free subset is unchanged.

## 12. Measurement (oracle and reader audited 2026-11-19)

The versioned oracle in `scripts/oracle/lilypond/` compares the MIDI rendered
by LilyPond from the same `.ly` source with the MusicXML produced by each
reader. N1 is the multiset of pitches (as in the historical `xmlcmp/cmp.py`);
N2 is the **multiset intersection of `(pitch, quantized attack)` pairs** on the
1/16-of-a-quarter grid (`GRID = 0.25`). Percentages are weighted by reference
note counts. `scripts/oracle/lilypond/README.md` documents the conventions and
the reproduction commands.

### Oracle: one cursor per part, not per voice

Earlier versions of `read_xml()` keyed the attack counter by voice, and before
that by `(voice, staff)`. Neither is MusicXML semantics: a part has a **single
cursor**. The reader now implements it literally: a note without `<chord/>`
advances the cursor by its `<duration>` (converted with the current
`<divisions>`), a `<chord/>` note shares the previous attack, `<backup>` and
`<forward>` move the cursor in both directions, a new measure starts at the
previous measure's start plus that measure's **maximal cursor extent**, and a
`<grace>` note has no duration. A tie stop is not an attack but still occupies
its time. This subsumes the two former special cases (a leading `<forward>`
for a voice's first note; ignoring later `<backup>`/`<forward>` as "stream
artefacts"): they are now ordinary cursor moves.

`ly_notes.py selftest` was extended **before** the change and failed on the
old code: with the voice-keyed counter, A3 on a piano's second staff (voice 1
reused after a `<backup>`) landed at 4.0 quarters instead of 1.0. The retained
tests cover two staves sharing voice 1, a voice changing `<staff>`, chords
(shared attack, chord at end of measure), a `<divisions>` change with a
partial-measure `<forward>` and a mid-measure `<backup>`, a grace note, and
`<unpitched>` notes.

The 29 files that the voice-keyed counter had penalised relative to the older
`(voice, staff)` key were identified again from the full campaign's converted
MusicXML by replaying the exact predecessor oracle from commit `04374e5^`:
**28** of the 29 are reproduced (the last belongs to a conversion changed by
the later transposition fix). On every one of the 4543 measurable files the cursor and
voice models give the same N2 file by file: the cursor does not recover those
28, because their writers omit the `<backup>` that would make a voice reused
on two staves explicit. The cursor model is kept because it is the MusicXML
definition and fixes cross-staff cases; the 28 files remain in the raw rows for
inspection.

### Reader: chord mode and drum mode are played

LilyPond plays `\chordmode` events in a `Staff`; the same music in a
`ChordNames` context is display-only. The reader now resolves chord names to
played pitches (`c:m7` → 60/63/67/70, `c/g` → triad plus bass 55) with a table
modelled on `scm/chord-entry.scm`. The lexer splits `c:m7` into `c`, `:`, `m7`,
so the name is reassembled before lookup. `\chords`, `
ew ChordNames` and
`\context ChordNames` stay non-playing grids (a MuseScore-conformant export
would write `<harmony>` there; the reader does not produce harmony events yet).
A pre-existing leak in the `\context ChordNames` branch — invisible while the
grid was empty — became audible once its names were readable: the grid used to
turn into an extra staff. It is now skipped, with a test for both `
ew` and
`\context`.

`\drummode`, `\drums` and `
ew DrumStaff` now read the abbreviations of
`ly/drumpitch-init.ly` (`bd` 36, `sn` 38, `hh` 42, `cymr` 51, …) as notes. The
MIDI numbers were verified against LilyPond 2.27 MIDI for all 129 drum names.
The pivot model has no untuned-percussion pitch, so the MusicXML writer emits
`<pitch>` with the GM number; the oracle compares those directly with
LilyPond's MIDI, which is the meaningful drum comparison. `<unpitched>` from
other writers is counted from `<display-step>`/`<display-octave>`; python-ly
writes the same B4 head for every drum and carries no MIDI number, so its drum
pitches cannot be matched — a documented oracle limitation. The regression
examples followed are `input/regression/chord-changes.ly` and
`input/regression/hara-kiri-drumstaff.ly`.

### Full campaign, in the foreground

Three lots from column 6 of the `-base.txt` lists (`mus` 1447, `ra` 1726,
`mut` 1377 paths). Every conversion ran under
`timeout 10 prlimit --as=2000000000`; the MIDI references are LilyPond's,
precomputed in the out-of-repo `lyaudit/out3*` tree with `pre.ly` injecting a
`\midi` block. The full three lots were re-run **in the foreground** at each
step, with no timeout. `python-ly` is unaffected by this campaign.

| Lot | Step | Files | Repeats | OK | Fail | Empty | Exact | N1 % (all) | N2 % (no repeat) | Max time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| mus | before (voice key, old reader) | 1447 | 113 | 1444 | 3 | 0 | 1079 (74.6 %) | 99.09 | 85.19 | 0.36 s |
| mus | cursor oracle (old reader) | 1447 | 113 | 1444 | 3 | 0 | 1071 (74.0 %) | 99.09 | 88.61 | 0.21 s |
| mus | cursor + reader | 1447 | 113 | 1444 | 3 | 0 | 1072 (74.1 %) | 99.09 | 88.62 | 0.28 s |
| ra | before (voice key, old reader) | 1726 | 305 | 1724 | 2 | 69 | 1158 (67.1 %) | 88.66 | 70.06 | 0.15 s |
| ra | cursor oracle (old reader) | 1726 | 305 | 1724 | 2 | 69 | 1190 (68.9 %) | 88.66 | 73.99 | 0.08 s |
| ra | cursor + reader | 1726 | 305 | 1724 | 2 | 55 | 1198 (69.4 %) | 89.84 | 75.01 | 0.06 s |
| mut | before (voice key, old reader) | 1377 | 652 | 1375 | 1 | 0 | 269 (19.5 %) | 97.92 | 58.28 | 10.02 s |
| mut | cursor oracle (old reader) | 1377 | 652 | 1376 | 1 | 0 | 369 (26.8 %) | 97.73 | 67.85 | 8.04 s |
| mut | cursor + reader | 1377 | 652 | 1376 | 1 | 0 | 369 (26.8 %) | 97.73 | 67.85 | 5.44 s |
| **all** | **before** | **4550** | **1070** | **4543** | **6** | **69** | **2506 (55.1 %)** | **97.84** | **63.51** | **10.02 s** |
| **all** | **cursor oracle** | **4550** | **1070** | **4544** | **6** | **69** | **2630 (57.8 %)** | **97.67** | **71.80** | **8.04 s** |
| **all** | **cursor + reader** | **4550** | **1070** | **4544** | **6** | **55** | **2639 (58.0 %)** | **97.70** | **71.83** | **5.44 s** |

The oracle change alone raises N2 by 8.29 points and exact outputs from 2506
to 2639 after the reader change; N1 is unchanged by the oracle (same notes,
only attacks move) and moves only where the reader now plays chord and drum
events. Per-file, oracle before → cursor: **975 files gain N2, 449 lose N2,
268 gain exact, 144 lose exact, none gains or loses N1.** Every decrease is the
same documented mechanism — honouring `<backup>`/`<forward>` and per-measure
extents moves attacks — so none is unexplained. Cursor → final: **16 files
gain N1, 16 gain N2, 9 gain exact, and no file loses any of the three.**
The two earlier apparent regressions (a `\chordmode` grid used in `\context
ChordNames` turning into an extra staff, and a similar `
ew ChordNames` case)
were traced to the context leak above and fixed before the final run, rather
than left as unexplained differences.

Raw rows, converters and the generated summaries are in the out-of-repo
measurement directories `lyn3-avant`, `lyn3-oracle` and `lyn3-final`;
`scripts/oracle/lilypond/README.md` documents how to reproduce them.

## 13. Writing export: baseline against musicxml2ly (2026-11-19)

### Method

`scripts/oracle/lilypond/ecriture.py` measures the reverse direction of §12:
from a source `S` it writes scoretk's `.ly` (`scoretk S --to ly`) and
musicxml2ly's `.ly`, compiles both with LilyPond 2.26 under
`timeout 20 prlimit --as=2000000000` (`-dmidi-extension=midi`), and compares
the resulting MIDI with the reference notes of `S` using the MIDI ↔ MIDI
N1/N2/N2d of `scripts/oracle/midi/midi_notes.py` (Q16 grid). A `\midi` block
and an `\unfoldRepeats` wrapper are injected into a copy of **both** candidates
so that neither is penalised for LilyPond's default behaviour; failures are
recorded, never silently dropped.

Four lots were measured: the 56 versioned corpus `.mscx`, the 174 `.mscz` of
`~/Musique`, the 32 genuine MusicXML files of `~/Musique`, and a deterministic
1/5 sample of its `.ly` files (375). The reference MIDI is MuseScore 4.7.4's
for `.mscx`/`.mscz`/`.xml`/`.mxl`, and LilyPond's MIDI of the source for the
`.ly` round trips. musicxml2ly receives, in order: the versioned
`*.ref.musicxml` (corpus), MuseScore's own MusicXML export (`.mscz`), the
source itself (`.xml`/`.mxl`), or scoretk's MusicXML (`.ly`). Every row records
this origin in `xml_source`, so the report always states which input was used.
Each conversion ran under `timeout 10 prlimit --as=2000000000`.

"Errors"/"Warnings" below are the counts of lines matched by the oracle's
`Erreur|error` / `Avertissement|warning` regexes in LilyPond's stderr; they are
an indicator, not a musical judgement. Percentages are weighted by reference
note counts. On the XML lot musicxml2ly fails internally on 28 of 32 files
(`UnboundLocalError: part_list`), so its 98.79 % is measured on a single file:
this is an alternative's defect, recorded as such.

### Baseline (frozen `scoretk-base`, before the fixes below)

| Lot | Tool | Files | Compiled | Exact | N1 % | N2 % | N2d % | Errors | Warnings | Mean .ly bytes | Max time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 22 (39.3 %) | 78.19 | 76.12 | 73.66 | 0 | 11 | 1398 | 1.42 s |
| corpus | musicxml2ly | 56 | 51 | 28 (50.0 %) | 94.52 | 91.98 | 89.29 | 79 | 20 | 2929 | 20.00 s |
| mscz | scoretk | 174 | 77 | 18 (10.3 %) | 65.14 | 48.22 | 46.68 | 2782 | 1598 | 28225 | 16.77 s |
| mscz | musicxml2ly | 174 | 143 | 34 (19.5 %) | 93.12 | 91.45 | 90.90 | 80988 | 3820 | 41699 | 30.95 s |
| xml | scoretk | 32 | 19 | 6 (18.8 %) | 85.17 | 72.90 | 71.83 | 148 | 251 | 103892 | 23.07 s |
| xml | musicxml2ly | 32 | 2 | 1 (3.1 %) | 98.79 | 98.79 | 98.79 | 326 | 24 | 19391 | 1.50 s |
| ly | scoretk | 375 | 269 | 214 (57.1 %) | 90.19 | 75.81 | 74.98 | 97 | 189 | 1953 | 5.36 s |
| ly | musicxml2ly | 375 | 118 | 79 (21.1 %) | 94.87 | 85.76 | 85.11 | 0 | 258 | 3712 | 6.44 s |

### Loss grouping and fixes

Grouping the files where musicxml2ly was exact and scoretk was not (21 of the
first 119 `.mscz`), by first cause:

1. **Compilation failures (8 files)** — two independent causes: `\stanza`
   (a LilyPond 2.27-only function) in scores with several verses, and
   identifiers such as `staff11VoiceOne` for staves numbered 11+, which
   LilyPond parses as `staff` followed by an integer. Both are fixed: the
   writer now emits `\set stanza = "n."` (musicxml2ly's 2.26-compatible form,
   also read by our reader) and spells numbers beyond ten (`staffOneOne`).
2. **Shifted attacks (7 files)** — `01- Magnificat` and six other movements
   had N1 = 100 % but N2 ≈ 6–13 %: a multi-measure rest was written twice
   (`R2.*30` followed by 29 individual `R2.`), adding 30 bars. The writer now
   skips the covered measures and uses the nominal bar length for the
   multiplier. On `01- Magnificat` the fixed output goes from 11.75 % to
   100 % N2d. A first version skipped every following measure unconditionally
   and dropped notes from `.ly` sources (`R2*N` followed by real music): the
   final version skips a measure only when all its items are rests, which
   restored the 22 notes of `PriereASteJeanneDArc.ly` and the 139 of
   `Angels_Carol.ly`.
3. **Wrong pitches (5 corpus files)** — instrument transposition was written
   as altered note names but no `\transposition`, so LilyPond played the
   written pitch instead of the concert pitch (N1 = 0 %). The writer now emits
   `\transposition <pitch>` as musicxml2ly does.
4. **Unfixable here (4 files)** — MuseScore 1.x/2.x input (`version MuseScore
   non prise en charge : 2.06/1.14`): a reader limitation, outside the scope
   of this export campaign.
5. **Duration-only differences (2 files)** — 15 notes of 1017 (~1.5 %) differ
   only in N2d; likely tie/gate-time detail, not worth a targeted fix.

No file was dropped: every fix has a reduced test, and the only
snapshots regenerated are the seven files whose `.ly` legitimately changed
(five `\transposition`, one extra `R1` removed, one multi-rest duplication).

### Final campaign (all fixes, frozen `scoretk-final2`)

| Lot | Tool | Files | Compiled | Exact | N1 % | N2 % | N2d % | Errors | Warnings | Mean .ly bytes | Max time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 28 (50.0 %) | 79.91 | 78.36 | 75.90 | 0 | 11 | 1397 | 1.79 s |
| corpus | musicxml2ly | 56 | 51 | 28 (50.0 %) | 94.52 | 91.98 | 89.29 | 79 | 20 | 2927 | 20.00 s |
| mscz | scoretk | 174 | 150 | 28 (16.1 %) | 92.26 | 85.33 | 83.32 | 2 | 1785 | 27313 | 5.57 s |
| mscz | musicxml2ly | 174 | 143 | 34 (19.5 %) | 93.12 | 91.45 | 90.90 | 80988 | 3820 | 41697 | 28.48 s |
| xml | scoretk | 32 | 24 | 12 (37.5 %) | 99.19 | 53.41 | 50.56 | 66 | 251 | 103888 | 23.18 s |
| xml | musicxml2ly | 32 | 2 | 1 (3.1 %) | 98.79 | 98.79 | 98.79 | 326 | 24 | 19385 | 1.08 s |
| ly | scoretk | 375 | 289 | 216 (57.6 %) | 90.19 | 78.20 | 77.61 | 0 | 134 | 1952 | 2.48 s |
| ly | musicxml2ly | 375 | 118 | 79 (21.1 %) | 94.87 | 85.76 | 85.11 | 0 | 258 | 3710 | 3.34 s |

Per-file, baseline → final for scoretk: **6 gains / 0 losses** on the corpus,
**89 / 0** on the 174 `.mscz`, **11 / 0** on the XML lot and **8 gains / 3
losses** on the `.ly` sample. The three `.ly` losses are timing-only: note
count and N1 are unchanged (`GaudeteChristusEstNatus` 1.77 → 0.37 % N2d,
`Gevaert_LeMessageDesAnges-a4` 74.77 → 73.52 %, `GaiDuguesclin` 45.90 →
44.26 %). They come from the intended multi-rest normalisation
(`R1*11/2*11` → `R2*11`), which moves bar checks and therefore a few attacks
on the 1/16 grid; no note is lost. On `.mscz` the gain is concentrated in
compilation (77 → 150 files) and attacks (N2 48.22 → 85.33 %), with slightly
smaller output on average (28 225 → 27 313 bytes). On the `.ly` round-trip
sample scoretk stays well ahead of the alternative (216 vs 79 exact files).
Overall scoretk matches or beats musicxml2ly on three lots and is within
7.6 N2d points and 6 exact files on `.mscz`, with far fewer error lines and
output about a third smaller.

### Remaining known limits

- **Ottava (`v4_partExclusion`)** — MuseScore's MusicXML export writes the
  *sounding* pitch under an octave-shift mark, while the pivot model stores
  the *written* pitch. LilyPond's `\ottava` is display-only, so those notes
  are played an octave low. Fixing it would require changing the model↔`.ly`
  octave convention for every ottava and risks round-trip regressions; left
  as a documented one-file difference.
- **Glissando** — MuseScore's reference MIDI renders glissandi as chromatic
  runs, LilyPond's MIDI does not. Both converters lose those notes against
  the reference: an oracle artefact, not a scoretk defect.
- **`.ly` references without `\midi`** — about 7 % of the `.ly` sample
  consists of files with no playable `\score`; LilyPond emits no MIDI for
  them, so their reference is absent (recorded as `echec`, not excluded).

### Second pass: writing `.mscz` (2026-11-19)

#### Method and frozen baseline

The `scoretk` CLI was frozen before any edit (`128e5265…`, `scoretk-base`)
and the oracle re-run on the 56 corpus `.mscx` and the 174 `.mscz` under
`timeout 10 prlimit --as=2000000000` (LilyPond compilation under 20 s / 2 GB).
The frozen binary was later rebuilt as `97cc14bc…` (`scoretk-final2`) after
the single retained fix; reference MIDI and MusicXML were shared between both
campaigns through the oracle's `refs/` cache. The four lots are the same as
above: 56 corpus `.mscx`, 174 `.mscz`, 32 genuine MusicXML and a deterministic
1/5 sample of 375 `.ly`.

Baseline (frozen `128e5265…`, corpus and `.mscz` only):

| Lot | Tool | Files | Compiled | Exact | N1 % | N2 % | N2d % | Errors | Warnings | Mean .ly bytes | Max time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 28 (50.0 %) | 79.91 | 78.36 | 75.90 | 0 | 11 | 1397 | 3.11 s |
| corpus | musicxml2ly | 56 | 51 | 28 (50.0 %) | 94.52 | 91.98 | 89.29 | 79 | 20 | 2977 | 20.00 s |
| mscz | scoretk | 174 | 149 | 28 (16.1 %) | 92.26 | 85.33 | 83.32 | 2 | 1778 | 27313 | 23.34 s |
| mscz | musicxml2ly | 174 | 142 | 34 (19.5 %) | 93.12 | 91.45 | 90.90 | 80988 | 3812 | 41748 | 40.96 s |

Final campaign (rebuilt `97cc14bc…`, all four lots):

| Lot | Tool | Files | Compiled | Exact | N1 % | N2 % | N2d % | Errors | Warnings | Mean .ly bytes | Max time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | scoretk | 56 | 56 | 28 (50.0 %) | 79.91 | 78.36 | 75.90 | 0 | 11 | 1396 | 7.61 s |
| corpus | musicxml2ly | 56 | 51 | 28 (50.0 %) | 94.52 | 91.98 | 89.29 | 79 | 20 | 2986 | 20.03 s |
| mscz | scoretk | 174 | 149 | 28 (16.1 %) | 92.26 | 85.33 | 83.32 | 2 | 1902 | 27363 | 23.08 s |
| mscz | musicxml2ly | 174 | 143 | 34 (19.5 %) | 93.12 | 91.45 | 90.90 | 80988 | 3820 | 41755 | 39.41 s |
| xml | scoretk | 32 | 24 | 12 (37.5 %) | 97.81 | 95.34 | 94.67 | 66 | 251 | 103899 | 23.13 s |
| xml | musicxml2ly | 32 | 2 | 1 (3.1 %) | 98.79 | 98.79 | 98.79 | 326 | 24 | 19441 | 4.34 s |
| ly | scoretk | 375 | 289 | 216 (57.6 %) | 90.19 | 78.20 | 77.61 | 0 | 134 | 1952 | 6.04 s |
| ly | musicxml2ly | 375 | 118 | 79 (21.1 %) | 94.87 | 85.76 | 85.11 | 0 | 258 | 3765 | 7.64 s |

#### Loss grouping (frozen baseline, files where musicxml2ly beats scoretk)

Every group has one reduced example; the first three are *not* export defects.

1. **MuseScore 1.x/2.x input** (22 files of the 174 `.mscz`, 21 in 2.06 and
   one in 1.14) — e.g.
   `Jig_-_Tripping_up_the_Stairs.mscz`: `version MuseScore non prise en
   charge : 2.06`. The reader rejects the file before the writer runs, so no
   export-side change can recover it. Documented reader limit, out of scope.
2. **Multi-rest count disagrees inside MuseScore's own files** (about 20
   orchestral parts) — e.g. `01- Les anges - Violons 1 et 2.mscz`: the `.mscx`
   carries `<multiMeasureRest>13</multiMeasureRest>` on a measure of `len=
   52/4`, but MuseScore's own MusicXML export (given to musicxml2ly) writes
   only six rest measures there, and its reference MIDI places the next notes
   28 quarters earlier. scoretk faithfully writes `R1*13`; the two views of
   the same score differ by seven bars. **A tentative export fix was tried and
   reverted**: making the writer consume one more duplicate rest measure
   changed nothing on `Les anges` (N2d 38.03 → 37.24) and regressed more than
   thirty other scores (`01- Magnificat` 1174 → 610, `Once in royal - Violons
   1 et 2` 1731 → 1394, `02- Flûtes 1 et 2` 1597 → 371) because those extra
   rest measures are genuine. The discrepancy is in the input, not in the
   writer; fixing it would mean changing the MuseScore reader's measure model.
3. **MuseScore playback-only octave coupling** — e.g. `Adios_Muchachos.mscz`
   (Fisarmonica accordion): the `.mscx` has 444 notes and scoretk writes all
   444; MuseScore's reference MIDI has 636, of which 143 are an octave away
   from any written note. musicxml2ly also misses 94 of them. This is a
   reference artefact, not a scoretk defect.
4. **Tremolo rendered as repeated notes** — e.g. `timesig-09.mscx`: 20 written
   chords with `:32` tremolo become 320 strokes in MuseScore's reference MIDI,
   while LilyPond sustains them (20 notes). musicxml2ly fails to compile this
   file (`UnboundLocalError` in `group_tremolos`), so it is absent from its
   aggregate denominator and scoretk's 6.25 % N1 drags the corpus average
   down. Add this to the list of reference artefacts alongside glissando.
5. **Invisible / cross-staff elements** — e.g. `testExcludeInvisibleElements`
   (N1 47/75 vs 53/75), `copypaste_parts` (10/44 vs 26/44) and
   `partExclusion` (8/12 vs 12/12, the documented ottava limit). These need
   information the pivot model does not carry; documented limits of §7/§12.
6. **Duration-only** — e.g. `O salutaris Hostia - Saint-Saëns.mscz`: 15 notes
   of 1017 differ only in gate time (0.25 vs 0.125 quarter); every pitch and
   attack matches (N1 = N2 = 1017). Not worth a targeted fix.
7. **Timing-only** — e.g. `Waltz_No._2.mscz`: N1 100 %, N2d 98.34 vs 99.19;
   one stray attack on the 1/16 grid, no note lost.

#### Retained fix: redundant octave transposition

A pure octave displacement is not a harmonic transposition. Writing the
concert pitch directly and omitting `\transposition c` gives **exactly the
same MIDI** (measured on `v4_hairpin_simple`: 59/60/62/64 at 0/1/2/3 quarters,
identical to the reference) while shortening the file. The writer now maps
every `transpose_chromatic` multiple of 12 to zero before both the pitch
spelling and the `\transposition` event; real chromatic transpositions
(clarinette en si bémol, cor en fa) keep their written pitch and command.
One unit test and the three `v4_hairpin_*` snapshots were updated; no other
snapshot changed. A residual limit: a *chromatic* transposition followed by a
non-transposing instrument does not emit an identity reset, so LilyPond's
`\transposition` property persists — pre-existing and rare.

#### Result and remaining losses

Baseline → final: **no per-file gain and no per-file loss** on N1 or N2d (92
unchanged comparisons on the corpus, 290 on `.mscz`). The retained
fix is byte-level only: the corpus mean `.ly` size falls from 1397 to 1396
bytes and the `.mscz` mean from 27 313 to 27 363 (the latter varies with the
metadata of the scores actually compiled). Musically scoretk stays ahead of
musicxml2ly on the XML lot (12 exact files vs 1) and on the `.ly` lot
(216 vs 79); it ties on exact files on the corpus and trails by six on
`.mscz`, where the 22 unreadable MuseScore 2.x files alone account for most of
the gap.

The corpus aggregate (N1 79.91 vs 94.52) is **not** a per-file regression.
musicxml2ly fails to compile three files that scoretk measures (`timesig-09`,
`repeat39`, `testHarmony6`); `timesig-09` alone carries 320 of the 2320
reference notes of the lot and scoretk scores 6.25 % there because of the
tremolo artefact above. Restricted to the 46 files measurable by both tools,
the corpus figures are **92.18 % N1 / 88.73 % N2d for scoretk against
94.52 % / 89.29 % for musicxml2ly** — a 0.56-point N2d gap, entirely the four
documented cases. No file was dropped from either report.

## 14. MIDI writing: the `ecriture_ly` causes (2026-11-19)

### Method and frozen baseline

The `ecriture_ly` lot of the MIDI oracle (`scripts/oracle/midi/`) compares
scoretk's MIDI of each `.ly` with the MIDI **LilyPond itself** produces from
the same source. The baseline binary was built once and frozen as
`scoretk-base`; `lots.sh` + `mesure.sh` were rerun with `LAKH_STRIDE=20`, every
conversion under `timeout 10 prlimit --as=2000000000`. The two MIDI reading
lots and the LilyPond reading oracle (`scripts/oracle/lilypond/`, same
manifests, `--stride 10`, scoretk only, cached LilyPond references) are
remeasured as non-regression. The baseline reproduces the published figures:
`ecriture_ly` 753/1020 exact, `lecture_musique` 596/1020, `lecture_mutopia`
49/122; the LilyPond oracle gives 111/145 (`mus`), 119/173 (`ra`) and 46/138
(`mut`).

### Cause found and fixed: LilyPond plays chord grids

LilyPond 2.26 plays `\chords` and both `ChordNames` contexts (`\new` and
`\context`) in its MIDI; only `FiguredBass` grids are silent. This was checked
by compiling reduced sources: `\new ChordNames \chordmode { c1 d:m }` beside a
staff yields a first track with the chord notes. The reader skipped those
grids, so all their notes were missing. Example `03_08_LaCiteDeDieu.ly`: 107
reference notes of 319 were absent (N1 66.14 %, N2 36.36 %).

The parser now routes `chords` like `chordmode` and lets both ChordNames forms
produce an ordinary played staff; `figures`/`FiguredBass` remain skipped. The
reduced test `grille_daccords_jouee_dans_le_midi` covers `\chords` beside a
staff with a bar check, `\new ChordNames` with `\with`, `\context ChordNames`
with a variable, exact pitches/durations/measure counts and an unaffected
`FiguredBass` case. Three older tests asserting the former, incorrect
"display-only" behaviour were updated at the same level of detail.

### Octave: `\transpose` inside `\relative` (third pass)

A second cause was found by reading the specification rather than the
measurement. Notation Reference §1.2.2 states: "Music inside a `\transpose`
block is absolute unless a `\relative` is included in the block." In the
implementation, `TransposedMusic` carries the `to-relative-callback`
`ly:relative-octave-music::no-relative-callback`
(`scm/define-music-types.scm`, `lily/relative-octave-music.cc`): the
`\relative` conversion does not descend into the transposed music, and the
relative reference after the block is the last note *before* it, not the last
note inside it.

The reader was walking the transpose body with the enclosing `\relative`
state and did not restore that state afterwards. On the reduced source

```lilypond
\relative c' { c'''4 d4 \transpose c d { f4 <g b>4 } | c4~ c4 r2 | }
```

LilyPond 2.27 plays 96, 98, 55, 57/61, 96, 96; the reader produced 96, 98,
103, 105/109, 108, 108. `Context::Transpose` now applies the same
`OctaveState::sans_relative()` transformation as `Context::NonRelative`
(default absolute reference under `\relative`, current reference kept under
`\fixed`) and restores the saved octave state after the body. The reduced
test `transpose_dans_relative_reste_absolu` checks exact pitches, durations,
measure count, a second staff, a chord, a tie, a rest, bar checks, and the
unaffected `\fixed` case. Only two files of the 1020-file `ecriture_ly` lot
use `\transpose` inside `\relative`; the fix gains no exact file and loses
none. `\clef "treble_8"` and `\ottava` were compiled in isolation and do not
change MIDI pitch (Notation Reference §1.3.1, §1.3.3), so keeping the written
octave remains correct.

### Other causes: measurement artefacts, not reader defects

Each of the five groups was reduced and checked against LilyPond 2.26
directly:

* **`octave` (81 files).** Only 24 have N1 below 50 %. Among them, the
  chord-grid files are fixed above; the rest are multi-`\score` or MIDI-block
  artefacts. `22_StMichel.ly` has `\transpose c c'` inside the first score's
  `\midi` block, so the reference sits one octave above the engraved score;
  `Maiana.ly` has two `\score` blocks, the second (the one with `\midi`) under
  `\transpose do do''`. The reader deliberately keeps the first, engraved
  score (§7). The other 57 files are a **false positive of the oracle**:
  `compare()` counts a missing pitch `p` as an octave error whenever `p ± 12`
  exists anywhere in the output. In `02_02_Segura_...ly`, N1 is 257/259 and
  the two "octave" notes are actually lost by the comparator's note-on/off
  pairing. Clef and ottava are display-only in LilyPond's MIDI (`\clef
  "treble_8"` and `\ottava #1` were compiled in isolation and do not change
  pitch), so keeping the written octave is correct.
* **`notes_manquantes` (55 files).** 36 sources contain grace notes. The
  specification settles the semantics (Notation Reference §2.6.1): "Each grace
  note in MIDI output has a length of 1/4 of its actual duration. If the
  combined length of the grace notes is greater than the length of the
  preceding note a 'Going back in MIDI time' error will be generated."
  LilyPond therefore *shortens* the preceding note so that the grace note fits
  before the beat. MuseScore does not: `collectGraceBeforeChordEvents`
  (`src/engraving/compat/midi/compatmidirenderinternal.cpp:1080`) computes
  `graceOffsetOn = graceTickSum − graceTickOffset × i` and places the grace
  note-on before the main chord without emitting an earlier note-off for the
  previous chord. scoretk's writer follows that model, as it must for the
  MuseScore reference lots. The oracle's `read_midi` holds a single open note
  per `(channel, pitch)` in a dictionary: when the acciaccatura overlaps the
  preceding note on the same pitch the later note-on overwrites the earlier
  one, which is never paired. In `Fait/0a/lily-8546410f.ly` the missing B♭4 is
  present in scoretk's raw SMF (note-on at tick 17 280, grace note-on at
  17 460, one shared note-off at 17 519); LilyPond's own MIDI has the main note
  ending at 13 824 and the grace starting at 13 994. The same overlap occurs
  when two voices play the same pitch at the same tick on the shared part
  channel (`Fait/13/lily-0d80bdd1.ly`, 92 → 91 notes). Twelve more files are
  copies of the same `LaVerteAigrette` source and lose a tied note the same way.
  No writer change is warranted: shortening the previous note would break the
  MuseScore-rendered reference lots, and one channel per voice contradicts
  MuseScore's one-channel-per-part export.
* **`duree_seule` (49 files).** All have the exact reference note count, N1
  and N2; only durations differ. This is the unison-merge comparator artefact
  documented in `midi.md`. `Encantada.ly` also has four `\score` blocks, the
  first engraved under `\transpose a g` and the second with `\unfoldRepeats`;
  its reference comes from a different score than the one the reader uses.
* **`attaque_decalee` (46 files).** N1 is 100 %. LilyPond's MIDI does **not**
  unfold `\repeat volta` unless `\unfoldRepeats` is present (verified on a
  reduced score), while scoretk's MIDI follows MuseScore and unfolds it, so
  attacks after the repeat move. `À-Notre-Dame.ly` has `\repeat volta 2` and
  gains 100 notes. There is no single systematic offset.
* **`notes_supp` (35 files).** Same volta mechanism or a different `\score`
  reference. `Lallement_VenezBergersEtBergeres.ly` goes from 338 to 676 notes,
  exactly twice, the signature of a `\repeat volta 2` unfolded by scoretk
  only; `22_HymneStMichel.ly` has five `\score` blocks, four of them rehearsal
  MIDIs with `\unfoldRepeats`.

Changing the reader to follow the MIDI block rather than the engraved score,
or the writer to stop unfolding repeats, would contradict the model and
regress the MuseScore reference lots. These causes are left as documented
limits.

### Before / after (same frozen manifests and oracle)

| Lot | Before exact | After exact | N1 / N2 gains | N1 / N2 losses |
| --- | ---: | ---: | ---: | ---: |
| ecriture_ly | 753 (73.8 %) | **756 (74.1 %)** | 13 / 10 | 0 / 1 |
| ecriture_mscz_corpus | 17 (81.0 %) | 17 (81.0 %) | 0 | 0 |
| ecriture_mscz_musique | 2 (25.0 %) | 2 (25.0 %) | 0 | 0 |
| lecture_musique | 596 (58.4 %) | 596 (58.4 %) | 0 | 0 |
| lecture_mutopia | 49 (40.2 %) | 49 (40.2 %) | 0 | 0 |

On `ecriture_ly`, N2d also gains 9 files and loses 1. The three new exact
files are `03_01_Miguel_Les-grains-de-mais_partition.ly` (104 → 179 notes,
all matched), `02_07_LaFleurDesChampsVersLeCiel.ly` (113 → 209) and
`DuermeNoLlores.ly` (78 → 128). The single N2/N2d loss is
`LaMouetteGrise_EtBerceDansLaBrise.ly`: its engraved score has one chord grid
but the `\midi` score, which provides the reference, repeats it three times
(`\accords \accords \accords` under `\unfoldRepeats`). The new chord notes
therefore have the right pitches (N1 68 → 76) but attacks the reference does
not contain (N2 68 → 25); the file is far from exact in both runs. This is the
documented "first engraved score wins" limit (§7), not a regression.

The LilyPond reading oracle (same manifests, `--stride 10`, cached references)
goes from 111 to 108 exact on `mus`, 119 to 120 on `ra` (N1 88.58 → 92.99 %)
and stays at 46 on `mut`. Its three `mus` losses are all chord-grid files
(`DorogoiDlinnoyu_LeTempsDesFleurs`, `DuermeNoLlores`,
`LaFleurDesChampsVersLeCiel`): their cached `ly.midi` predates LilyPond's
chord-grid playback and contains only the voice track (113 notes for
`LaFleur...`), whereas a fresh LilyPond 2.26 compilation of the same source
contains both tracks (209 notes). Every reference note still matches (N1 and
N2 stay at 100 % on those three files); only the newly played chord grid is
counted as "extra" against the stale reference. No reference note is lost on
any lot, and no reading lot regresses. On the unrelated `robustesse_lakh` lot
(MIDI → MusicXML, no `.ly` input), the frozen baseline has one 10 s timeout and
the fixed run thirteen, because three campaigns and then the final workspace
checks ran concurrently. Six of the extra-timeout files were retried in
isolation with both binaries (`Dreadlock Holiday.1.mid`, `Sing A Song (bonus
track).mid`, `Broken Wings.1.mid`, `Please Don't Go.mid`, `Eternally.mid`,
`Another Day.2.mid`): all complete in 2–10 s with both the frozen and the
fixed binary. They are load artefacts, not an effect of the reader change.

The `\transpose`-inside-`\relative` fix above was measured again against the
same frozen baseline on `ecriture_ly`, `lecture_musique`, `lecture_mutopia` and
the LilyPond reading oracle (stride 10, cached references): the figures are
unchanged — 756/1020, 596/1020, 49/122 and 108/120/46 exact respectively — with
no per-file N1 or N2 loss on any lot. Only two of the 1020 `.ly` sources use
`\transpose` inside `\relative`, so the fix is a correctness improvement rather
than a gain in this sample. The other four causes were re-checked against the
notation reference and the MuseScore C++ source this time: none warrants a
reader or writer change without contradicting the pivot model (§2.6.1 for
grace notes, §4.1.2 for volta repeats, §1.3.1/§1.3.3 for clefs and ottavas).
