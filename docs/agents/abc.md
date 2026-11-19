# ABC : lecture et écriture dans scoretk

Crates `crates/scoretk-abc-common`, `crates/scoretk-abc-import` et
`crates/scoretk-abc-export`, features `abc-import` et `abc-export` de la
façade, activées par la CLI.
Ce document décrit le sous-ensemble ABC couvert, la correspondance avec le
modèle pivot, les résultats mesurés et les limites connues. Le noyau du projet
reste dans `AGENTS.md` ; la façade est décrite dans `docs/agents/scoretk.md`.

## 1. Origine et licence

MuseScore Studio n'a **pas** d'import ni d'export ABC natif : la copie de
référence `ref-musescore/` ne contient que `src/importexport/midi` et
`src/importexport/musicxml`. Il n'y a donc pas de code C++ de MuseScore à
porter pour le format lui-même. Le crate suit :

- la **norme ABC 2.1**, <https://abcnotation.com/wiki/abc:standard:v2.1> ;
- le comportement de **`xml2abc.py`** (MusicXML → ABC) et **`abc2xml.py`**
  (ABC → MusicXML) de Willem Vree :
  - `xml2abc.py` version **177**, `Copyright (C) 2012-2025: W.G. Vree` ;
  - `abc2xml.py` version **269**, `Copyright (C) 2012-2026: Willem G. Vree` ;
  - téléchargés le 2026-11-17 depuis
    <https://wim.vree.org/svgParse/xml2abc.html> et
    <https://wim.vree.org/svgParse/abc2xml.html> (`xml2abc.py-177.zip`,
    `abc2xml.py-269.zip`) ;
  - en-tête : « Lesser GNU General Public License as published by the Free
    Software Foundation », **sans numéro de version**. La LGPL 2.1 § 3
    autorise la conversion en GPL ; le projet est distribué en
    **GPL-3.0-only** (voir `NOTICE`).

Aucune ligne de ces scripts n'est copiée : l'implémentation est une
réécriture Rust. Les scripts servent de référence de comportement, comme
`musicxml2ly` pour `scoretk-lilypond-export`.

## 2. API et fichiers

```rust
pub fn write_abc(score: &Score) -> Result<String, Error>;
pub fn read_abc(src: &str) -> Result<Score, Error>;
pub fn read_abc_tune(src: &str, tune: usize) -> Result<Score, Error>;
pub fn read_abc_with(src: &str, options: &ReadOptions) -> Result<ReadOutput>;
```

Le code est réparti en **trois crates** : un socle partagé et un crate par
sens. Aucun ne fait d'entrée/sortie, tous compilent pour
`wasm32-unknown-unknown`.

| Crate | Contenu |
| --- | --- |
| `crates/scoretk-abc-common` | `error.rs` (erreurs, avertissements, `Result`), `pitch.rs` (hauteurs, armures, clefs, modes, `AccidentalState`), `duration.rs` (multiplicateurs `L:`), `decorations.rs` (nuances, articulations, ornements), `text.rs` (annotations et paroles `w:`) |
| `crates/scoretk-abc-import` | `src/lib.rs` (API de lecture), `src/reader/mod.rs` (lignes, en-tête, morceaux `X:`), `src/reader/music.rs` (ligne de musique), `src/reader/lyrics.rs` (paroles), `src/reader/build.rs` (construction du modèle) |
| `crates/scoretk-abc-export` | `src/lib.rs` (`write_abc`), `src/writer.rs` (en-tête et orchestration), `src/voice.rs` (écriture d'une voix) |

Le partage est **vérifié module par module** : chaque module de
`scoretk-abc-common` est employé par les deux sens. `pitch.rs` mêle des helpers
propres à un sens (`tpc_of`, `parse_key`, `parse_clef` pour la lecture ;
`note_spelling`, `abc_note`, `key_field`, `clef_name` pour l'écriture), mais
le module entier reste commun parce que `AccidentalState` et `key_alter` sont
partagés et que les tables de degrés et d'altérations sont la même source.

| Élément | Emplacement |
| --- | --- |
| API de lecture | `crates/scoretk-abc-import/src/lib.rs` |
| API d'écriture | `crates/scoretk-abc-export/src/lib.rs` |
| Erreurs et avertissements | `crates/scoretk-abc-common/src/error.rs` |
| Hauteurs, armures, clefs, modes | `crates/scoretk-abc-common/src/pitch.rs` |
| Durées, multiplicateurs, figures | `crates/scoretk-abc-common/src/duration.rs` |
| Décorations, nuances, articulations | `crates/scoretk-abc-common/src/decorations.rs` |
| Échappement des annotations et paroles | `crates/scoretk-abc-common/src/text.rs` |
| En-tête et orchestration de l'écriture | `crates/scoretk-abc-export/src/writer.rs` |
| Écriture d'une voix | `crates/scoretk-abc-export/src/voice.rs` |
| Préparation des lignes, en-tête, morceaux `X:` | `crates/scoretk-abc-import/src/reader/mod.rs` |
| Analyse d'une ligne de musique | `crates/scoretk-abc-import/src/reader/music.rs` |
| Paroles `w:` | `crates/scoretk-abc-import/src/reader/lyrics.rs` |
| Construction du modèle | `crates/scoretk-abc-import/src/reader/build.rs` |
| Lecture seule (airs écrits à la main) | `crates/scoretk-abc-import/tests/lecture.rs` |
| Aller-retour des airs | `crates/scoretk/tests/abc_roundtrip.rs` |
| Instantanés `.abc` du corpus | `crates/scoretk/tests/abc_corpus.rs` |
| Aller-retour et noyau musical | `crates/scoretk/tests/abc_aller_retour.rs` |
| Outils externes (ignoré par défaut) | `crates/scoretk/tests/abc_outils_externes.rs` |

## 3. Écriture

L'en-tête est `%abc-2.1`, `X:1`, `T:`, `C:`, `M:`, `L:1/8`, `Q:` (premier
tempo numérique) et `K:` avec la clef (`K:C clef=bass`). Les voix multiples
sont écrites une par `V:`.

- **Une voix ABC par (portée, voix du modèle) qui a du contenu.** Une portée
  vide garde une voix 0. `%%score` reprend la grammaire d'`abc2xml` :
  `(1 2)` regroupe des voix sur une même portée, `{…}` est une partie à
  accolade, `[…]` une partie à crochet, un numéro seul une portée isolée.
- **Durées relatives à `L:1/8`** : multiplicateur entier (`2`, `4`, `3`) ou
  fractionnaire (`3/2`, `2/3`) ; toute durée du modèle est représentable.
- **Hauteurs en hauteur écrite** (`pitch - transpose`), `tpc_transposed` si
  valide, octave d'ottava ajoutée comme `pitch2xml`. Une altération explicite
  n'est écrite que si elle diffère de l'armure ou de l'état de la mesure.
- **Barres** : `|`, `||`, `|]`, `|:`/`:|`/`::`, volatas `[1`, `[1,2`.
- **N-olets** : `(p`, `(p:q`, `(p:q:r` ; `r` est explicite dès qu'un n-olet
  contient des n-olets imbriqués ou un nombre de notes différent de `p`.
- **Accords** `[CEG]`, liaisons `-`, phrasés `( )`, agréments `{…}` : un
  seul groupe par accord, `/` devant chaque note d'acciaccatura (`{/g}`,
  `{g /a}`, l'espace évitant que la barre soit lue comme une fraction de
  durée) ; les groupes d'agréments consécutifs du modèle sont fusionnés.
- **Décorations** : nuances `!p!`, articulations `!staccato!`, `!accent!`,
  `!tenuto!`, `!fermata!`, `!trill!`, `!mordent!`, `!arpeggio!`, soufflets
  `!<(!`/`!<)!` et `!>(!`/`!>)!`, doigtés `!1!`.
- **Annotations** : `"^texte"` au-dessus, `"_texte"` au-dessous, accords
  chiffrés `"C"` ; les annotations système (tempo, repère) restent dans la
  voix 0 de la portée.
- **Paroles** : `w:` après chaque ligne de musique, une ligne par couplet ;
  césures `-`, prolongations `_`, syllabes absentes `*`, tiret littéral `\-`.

## 4. Lecture

Le lecteur lit le **premier morceau** (`X:`) d'un fichier ; `read_abc_tune`
choisit un autre indice. Il accepte le sous-ensemble de l'écriture, plus :

- les modes de `K:` (majeur, mineur, dorien, phrygien, lydien, mixolydien,
  éolien, locrien) : les modes autres que majeur/mineur sont ramenés à leur
  armure avec un avertissement (le modèle ne porte pas la tonique modale) ;
- `K:none` (armure vide) et `K:HP` (ramené à ré majeur) ;
- `M:C`, `M:C|` ;
- le rythme brisé `<`/`>` (jusqu'à deux niveaux), les ornements abrégés
  `.`, `~`, `H`, `M`, `P`, `T`, `u`, `v` ;
- les reprises `|:`, `:|`, `::`, les volatas `[1`/`[2` ;
- les champs en ligne `[K:…]`, `[M:…]`, `[L:…]`, `[V:…]` et les changements
  d'armure, de mesure et de clef en cours de mesure ;
- `%%score`/`%%staves` (première directive seulement), les autres `%%`
  ignorées avec un `Warning` ;
- plusieurs morceaux `X:`.

Les erreurs de structure (hauteur inconnue, durée nulle, `K:` absent, accord
non fermé) renvoient `Error` avec le numéro de ligne. Les constructions hors
sous-ensemble (`&`, `P:`, `U:`, `I:`) produisent un avertissement et sont
ignorées, pour ne pas perdre un air entier.

## 5. Correspondance avec le modèle

| ABC | Modèle |
| --- | --- |
| `V:n` + `%%score` | `Part` et `Staff` ; une voix ABC par `Track(staff, voice)` |
| `K:` | `KeySig` (`concert_key`, `transposed_key`, `mode`) |
| `M:` | `TimeSig` (`nominal_len`, segments `TimeSig`) |
| `clef=` | `ClefType` et `Staff::default_clef` |
| hauteur | `Note::pitch`, `Note::tpc`/`tpc_transposed`, `Accidental` |
| durée + `(p:q:r` | `Duration`, `ChordRest::ticks`, `Tuplet` |
| `-` | `Note::tie_forward`/`tie_back` |
| `( )` | `SpannerType::Slur` |
| `!<(!`/`!<)!` | `SpannerType::Hairpin` |
| `!p!` | `Annotation::Dynamic` |
| `"C"` | `Annotation::Harmony` |
| `"^texte"`/`"_texte"` | `Annotation::StaffText` |
| `w:` | `Lyrics` par couplet |
| `[1`/`[2` | `SpannerType::Volta` avec `VoltaInfo::endings` |

## 6. Résultats mesurés (2026-11-17)

- **Instantanés** : 56/56 fichiers du corpus v3/v4 écrits en `.abc` dans
  `tests/corpus/abc/` et relus sans erreur.
- **Aller-retour** `.mscx` → ABC → modèle → MusicXML comparé à l'export direct
  au niveau du **noyau musical** (mesures, armure, mesure, clef, hauteur,
  type, points, voix, portée, liaisons, n-olets, paroles, reprises) :
  **46/56 identiques**, listés dans `EXPECTED_ROUNDTRIP` de
  `crates/scoretk/tests/abc_aller_retour.rs`. Les 10 écarts restants :
  - `clef-1`, `copypaste_parts`, `layout_elements` : clef changée en cours de
    mesure ou double clef au même tick, que l'export direct réduit ;
  - `moonlight` : pièce longue avec armures/clefs/voix multiples ; les
    premiers écarts sont des silences de remplissage ;
  - `testMultiInstrumentPart3` : changement d'instrument et armure locale ;
  - `testWedge4`, `voices` : silences implicites (trous) que l'export direct
    rend en `<forward>` et l'écriture ABC en `z` ;
  - `linked-ties` : liaison sur note d'agrément, sans équivalent ABC standard ;
  - `timesig-09` : trémolo à deux notes (`time-modification` 2:1) non rendu ;
  - `voiceswitching` : n-olets imbriqués combinés à des voix multiples ; les
    hauteurs/positions restent justes, l'ordre des événements diffère.
- **Outils externes** : `scripts/check-abc-external.sh` compile
  `crates/scoretk/tests/abc_outils_externes.rs` (features `abc-import` et
  `abc-export`) et l'exécute
  sous `nix shell nixpkgs#abcm2ps nixpkgs#abcmidi`. 46 instantanés et 2 airs sont acceptés par `abcm2ps` et
  `abc2midi` ; les 10 fichiers de `IGNORED` sont refusés par ces outils
  (n-olets imbriqués, clefs anciennes, reprises sans note), pas par la norme.
- **Façade et CLI** : `crates/scoretk/tests/abc.rs` et
  `crates/scoretk-cli/tests/abc.rs` vérifient lecture, écriture, stdin/stdout
  et `--list-formats`. `abc_roundtrip`, `abc_corpus`, `abc_aller_retour` et
  `abc_outils_externes` vivent dans `crates/scoretk/tests/` : ils exercent les
  **deux moitiés** et le corpus, pas un sens isolé.
- **Comparaison à `xml2abc.py`** (2026-11-17, `dynamics`) : mêmes hauteurs,
  durées, voix et nuances ; les écarts sont la mise en page et les choix
  propres aux scripts (`L:1/4` au lieu de `L:1/8`, `I:linebreak`, `nm`/`snm`),
  sans effet sur le noyau musical.

## 7. Limites connues

- Les **n-olets imbriqués** sont une extension hors ABC 2.1 ; `abcm2ps` et
  `abc2midi` les refusent. L'écriture utilise `(p:q:r` explicite et le
  lecteur les accepte.
- **Pas de transposition standard** : ABC n'a pas de champ `transpose=` dans
  la norme. L'écriture emploie la hauteur écrite et le lecteur la conserve
  comme hauteur concert ; un aller-retour par ABC ne restitue donc pas
  `<transpose>`.
- **Armures, mesures et clefs locales** : ABC n'a qu'une armure et qu'une
  mesure par morceau. L'écriture prend celles de la première portée de la
  partie ; les indications locales de MuseScore sont perdues.
- **Mise en page** : pas de sauts de système, marges, hampes, ligatures,
  têtes de note, articulations non listées, trémolos à deux notes, pédales,
  ottavas (le décalage de hauteur est appliqué, le trait n'est pas écrit).
- **Reprises** : le nombre de lectures (`repeat_count` > 2) n'a pas de
  syntaxe ABC ; les volatas multi-mesures sont approximées par la première
  barre.
- **Micro-intervalles** (`AccidentalType::Other`, quarts de ton) non écrits.
- **`M:none`** est ramené à 4/4 ; **`K:` modal** perd la tonique modale.
- Le lecteur ignore `&` (superposition), `P:` (parties), `U:` et `I:` avec
  avertissement.

## 8. Tests et régénération

```sh
nix develop -c cargo test -p scoretk-abc-common
nix develop -c cargo test -p scoretk-abc-import
nix develop -c cargo test -p scoretk-abc-export
nix develop -c cargo test -p scoretk --features abc-import,abc-export --test abc_aller_retour -- --nocapture
UPDATE_ABC_SNAPSHOTS=1 nix develop -c cargo test -p scoretk --features abc-import,abc-export --test abc_corpus
scripts/check-abc-external.sh
```

`EXPECTED_ROUNDTRIP` est **non régressif** : un fichier listé qui cesse
d'être identique fait échouer `abc_aller_retour`. Les instantanés sont versionnés
dans `tests/corpus/abc/` et couverts par le filtre `tests/corpus` de
`flake.nix`.

## 9. Mise à niveau de `xml2abc.py` et `abc2xml.py`

Ces scripts ne sont **pas** versionnés dans le dépôt : les retélécharger
hors dépôt (par exemple `/tmp/abcchk/`) pour les consulter.

1. **Identifier la version portée.** En-têtes actuels :
   - `xml2abc.py` : `VERSION = 177`, `Copyright (C) 2012-2025: W.G. Vree` ;
   - `abc2xml.py` : `VERSION = 269`, `Copyright (C) 2012-2026: Willem G.
     Vree` ;
   - téléchargement du 2026-11-17, URLs du § 1.
2. **Récupérer une nouvelle version.** Télécharger `xml2abc.py-NNN.zip` et
   `abc2xml.py-NNN.zip` depuis <https://wim.vree.org/svgParse/xml2abc.html>
   et <https://wim.vree.org/svgParse/abc2xml.html>, puis `unzip` dans un
   répertoire temporaire. Vérifier l'en-tête de licence : une LGPL 2.1 ou 3,
   même « only », reste convertible en GPL-3.0 (LGPL 2.1 § 3, LGPL 3 § 3) ;
   seule une licence explicitement incompatible (propriétaire, CC-BY-NC…)
   imposerait de s'en tenir à la norme ABC 2.1 seule (voir `NOTICE`).
3. **Repérer les changements pertinents.** Comparer les fichiers
   (`diff -u ancien/xml2abc.py nouveau/xml2abc.py`) et surveiller :
   - `VERSION`, les tables `note_ornamentation_map`, les décorations
     (`!staccato!`, `!trill!`, nuances) ;
   - `%%score`/`%%staves` et la grammaire `( )`, `{ }`, `[ ]` ;
   - les champs `K:`, `M:`, `L:`, `Q:`, `w:`, les modes et les clefs ;
   - les n-olets (`(p:q:r`), le rythme brisé et les reprises.
   Les fichiers Rust correspondants sont `decorations.rs`, `pitch.rs`,
   `duration.rs`, `writer.rs`/`voice.rs` (écriture) et `reader/music.rs`,
   `reader/build.rs` (lecture).
4. **Comparer les sorties.** Pour un échantillon du corpus (`xml2abc`
   attend du MusicXML, `abc2xml` de l'ABC) :
   ```sh
   nix develop -c cargo run -q -p scoretk-cli -- tests/corpus/v4/dynamics.mscx -o /tmp/dynamics.musicxml
   python3 /tmp/abcchk/xml2abc_177/xml2abc.py /tmp/dynamics.musicxml -o /tmp/abc-ref
   diff -u /tmp/abc-ref/dynamics.abc tests/corpus/abc/v4_dynamics.abc
   python3 /tmp/abcchk/abc2xml_269/abc2xml.py tests/corpus/abc/v4_dynamics.abc -o /tmp/xml-ref
   ```
   Comparer les différences qui changent le **noyau musical**, pas la mise en
   page (`L:`, espacement, `I:linebreak`, `nm`/`snm` sont propres aux scripts).
5. **Régénérer et revalider.**
   ```sh
   UPDATE_ABC_SNAPSHOTS=1 nix develop -c cargo test -p scoretk --features abc-import,abc-export --test abc_corpus
   nix develop -c cargo test -p scoretk-abc-common -p scoretk-abc-import -p scoretk-abc-export
   nix develop -c cargo test -p scoretk --features abc-import,abc-export --test abc_aller_retour -- --nocapture
   scripts/check-abc-external.sh
   nix develop -c cargo test --workspace
   nix flake check
   ```
   Relire le diff des instantanés et mettre à jour `EXPECTED_ROUNDTRIP` (une
   entrée ne se retire qu'avec un écart documenté) et le § 6 de ce document.
6. **Quand le faire.** Uniquement si la norme ABC 2.1 évolue, si un
   désaccord de comportement bloque un fichier réel, ou lors d'une revue
   annuelle des références. Ne pas suivre chaque version de `xml2abc.py` :
   la cible de conformité du projet reste MuseScore 4.7.4 et le modèle pivot.

## 10. Mise à niveau de MuseScore

Le crate ne porte pas de fichier C++ de MuseScore : il dépend du **modèle
pivot** (`crates/scoretk-core/src/model`) et de ses lecteurs. Une mise à niveau de
MuseScore suit donc `docs/agents/upstream.md` (commit, version, régénération
du corpus, validation e2e). Vérifier à cette occasion que `ref-musescore/`
n'a toujours pas d'export ABC natif ; s'il en apparaissait un, ce serait une
nouvelle source de comportement à déclarer dans `NOTICE`.

## 11. Mise à niveau de la norme ABC 2.1

Le crate suit la **norme ABC 2.1** (version stable publiée par la communauté
ABC, <https://abcnotation.com/wiki/abc:standard:v2.1>), et non une
implémentation de référence. Il n'y a donc pas de code amont à comparer,
mais une spécification à relire et à revalider.

1. **Version suivie.** ABC 2.1, sans sous-version : c'est la version citée
   dans les en-têtes des fichiers de `crates/scoretk-abc-common/src/` et dans
   `NOTICE`. Le préfixe `%abc-2.1` écrit par `writer.rs` en découle.
2. **Où récupérer la spécification.** La page du § 1 (wiki abcnotation.com),
   ou son archive hors ligne. Les changements de version sont annoncés sur le
   forum/la liste ABC de abcnotation.com ; conserver une copie datée hors
   dépôt (par exemple `/tmp/abcstd/`) pour le diff.
3. **Repérer les changements pertinents.** Comparer la nouvelle version à
   2.1 et surveiller les sections qui correspondent aux fichiers Rust :
   - hauteurs, armures, modes, clefs : `pitch.rs` ;
   - `L:`, `M:`, durées, n-olets, rythme brisé : `duration.rs` ;
   - décorations, nuances, articulations : `decorations.rs` ;
   - champs d'en-tête, lignes `w:`, directives `%%` : `writer.rs`,
     `reader/mod.rs` ;
   - syntaxe de la musique (liaisons, phrasés, accords, agréments, barres,
     reprises) : `reader/music.rs` et `voice.rs`.
   Une nouveauté n'est à implémenter que si elle change le **noyau musical**
   ou débloque un fichier réel ; la mise en page reste hors périmètre.
4. **Revalider.**
   ```sh
   nix develop -c cargo test -p scoretk-abc-common -p scoretk-abc-import -p scoretk-abc-export
   nix develop -c cargo test -p scoretk --features abc-import,abc-export --test abc_aller_retour -- --nocapture
   UPDATE_ABC_SNAPSHOTS=1 nix develop -c cargo test -p scoretk --features abc-import,abc-export --test abc_corpus
   scripts/check-abc-external.sh
   nix develop -c cargo test --workspace
   nix flake check
   ```
   Relire le diff des instantanés : une extension de la norme ne doit pas
   changer la sortie des fichiers déjà conformes.
5. **Quand le faire.** Quand une version 2.2 (ou suivante) devient la norme
   stable, ou quand `abcm2ps`/`abc2midi` acceptent une syntaxe que le lecteur
   refuse sur un fichier réel. Ne pas suivre les brouillons : la référence
   reste 2.1 tant qu'aucune décision explicite n'est prise (mise à jour de
   `NOTICE`, du préfixe `%abc-2.1` et de ce document).

## 12. Outils de vérification externes

`abcm2ps` et `abc2midi` (paquet nixpkgs `abcm2ps` et `abcmidi`) servent de
contrôle d'acceptabilité, pas de source de comportement : aucune ligne de
leur code n'est suivie. Leurs versions ne sont **pas épinglées** (le script
`scripts/check-abc-external.sh` les prend dans le nixpkgs du moment), car ils
ne déterminent ni la sortie ni les instantanés. Si un fichier de `IGNORED`
devient accepté, le retirer de la liste ; si un fichier conforme devient
refusé, vérifier d'abord que la sortie reste lisible par le lecteur interne
avant de conclure à une régression.

## 13. Measurement against `abc2midi` (phases 0-1, 2026-11-19)

This section is in English by convention for the reliability measurement
reports. No scoretk code was changed for this baseline.

### 13.1 Oracle, alternative, scripts

- **Oracle**: `abc2midi` 5.03 (nixpkgs `abcmidi`) → MIDI. It is called with
  `-NGRA` (no grace notes) and `-NGUI` (no guitar-chord strumming from chord
  symbols such as `"C"`, which are harmony annotations, not written notes).
  Repeat barlines are replaced by plain bars and `P:` field lines are removed
  on a copy, so **written** notes are compared, not notes replayed by repeats.
  A non-zero exit code is accepted when a non-empty MIDI file was written.
- **Alternative to beat**: `abc2xml.py` 269 (Willem Vree, outside the
  repository), measured with the same oracle.
- **Scripts**: `scripts/oracle/abc/{mesure.sh,mesure.py,cmp.py,one.sh,aggreg.py}`
  (GPL-3.0-only headers, no third-party data or script copied into the repo).
  `cmp.py` computes **N1** (multiset of pitches) and **N2** (pitch + attack
  quantized to 1/24 of a quarter note) with the Python standard library only.

### 13.2 Lots and result

Three local lots, 1 402 files / 4 756 `X:` tunes: `abcjs-tests` (271 files,
MIT), `music21-corpus` (1 116 files; `essenFolksong` excluded), and
`abc-2.1-standard` (15 fragments, NC: local measurement only, never copied
into the repository). One tune is one measurement; files without `X:` are
counted but excluded from the note comparison. Raw outputs are in
`/tmp/abc-mesure/final2/` (not versioned).

| Lot | Tunes | Exact (tk / alt) | Ref notes | N1 correct tk | N1 correct alt | N2 tk | N2 alt | Failures tk / alt |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| abcjs | 272 | 105 / 120 | 2 448 | 1 595 (65.2 %) | 2 250 (91.9 %) | 46.7 % | 71.3 % | 143 / 124 |
| music21 | 4 465 | 2 810 / 3 601 | 478 557 | 374 404 (78.2 %) | 466 816 (97.5 %) | 37.7 % | 80.0 % | 805 / 0 |
| ABC 2.1 (NC) | 19 | 1 / 8 | 1 007 | 258 (25.6 %) | 910 (90.4 %) | 17.6 % | 56.2 % | 6 / 6 |
| **Total** | **4 756** | **2 916 / 3 729** | **482 012** | **376 257 (78.1 %)** | **469 976 (97.5 %)** | **37.7 %** | **79.9 %** | **954 / 130** |

Files whose *every* tune is exact: **877 / 1 402 (62.6 %)** for scoretk,
**1 095 / 1 402 (78.1 %)** for the alternative. Empty outputs: 23 (tk) vs
20 (alt). The alternative is ahead by **19.4 points on N1** and **42.2 on N2**;
it never failed when the oracle succeeded (its 130 failures are the 109 files
without `X:` and the 21 tunes rejected by `abc2midi`). The baseline to beat for
the ABC reading campaign is therefore N1 78.1 % / N2 37.7 %, target 97.5 % /
79.9 %.

### 13.3 Ten largest groups of gaps (scoretk, unless noted)

Counts are tune measurements; example paths are under
`/home/jacques/programmes/musique/corpus/`.

1. **Syntax rejected by the reader (824)** — mostly unexpected `` ` `` (336),
   inline ending `1` (195), `k` (92), quarter-tone `/` (58), invalid pitch
   after `|` (33). Example: `abcjs-tests/extracted-abc/flattener.test__abcRepeat.abc`
   X:1 (inline `1`).
2. **Missing notes (645)** — written notes absent from the MusicXML, partly
   from ornaments on chords. Example: `abcjs-tests/extracted-abc/misc.test__abcChordClass.abc` X:1.
3. **Mixed missing and extra notes (148)** — many come from per-voice
   `transpose=` ignored by the reader. Example: `abcjs-tests/extracted-abc/flattener.test__abcTranspose.abc` X:1.
4. **No `X:` field (109 files)** — ABC fragments, not comparable. Example:
   `abc-2.1-standard/fragments/example-010.abc`.
5. **Pitch offset: MIDI transpose, octave clefs, local key (48 + 4 + 2)** —
   `%%MIDI transpose -2` is applied by `abc2midi`, not by scoretk. Example:
   `abcjs-tests/extracted-abc/flattener.test__abcMidiOptions.abc` X:1.
6. **Oracle cannot convert the tune (21)** — `abc2midi` limitation, not a
   scoretk error; affects both candidates. Example:
   `abcjs-tests/extracted-abc/parsing.test__abc3.abc` X:360 (bare `[V:]` lines).
7. **Oracle has no notes (14)** — empty reference, affects both candidates.
   Example: `abc-2.1-standard/fragments/example-007.abc` X:2.
8. **Truncated output (11)** — e.g. drum tracks counted by `abc2midi` and
   absent from MusicXML. Example:
   `abcjs-tests/extracted-abc/flattener.test__abcMetronome.abc` X:1.
9. **Empty scoretk output (9)** — a global `%%stretchlast 1` makes the reader
   emit measures with no notes (reduced locally to a 6-line file). Example:
   `abcjs-tests/extracted-abc/wrap.test__abcShortMeasures.abc` X:1.
10. **Percussion / unpitched notes (3)** — drum maps have no MusicXML pitch.
    Example: `abcjs-tests/extracted-abc/flattener.test__abcSnare.abc` X:1.

Rarer residual groups: extra notes (2), rhythm/attack only (0 for scoretk,
1 for the alternative), ornament pitches (0 / 1).

### 13.4 Reproducing

```sh
BIN=./target/release/scoretk
ALT=/tmp/abcchk/abc2xml_269/abc2xml.py          # outside the repository
SORTIES=/tmp/abc-mesure/nouveau

scripts/oracle/abc/mesure.sh liste.txt "$SORTIES" "$BIN" "$ALT"
python3 scripts/oracle/abc/aggreg.py "$SORTIES/mesures.tsv"
# one file only:
scripts/oracle/abc/one.sh "$SORTIES/un" "$BIN" air.abc "$ALT"
```

`liste.txt` holds one `lot<TAB>/absolute/path/file.abc` line per file. Every
scoretk launch is bounded by `timeout 10 prlimit --as=2000000000`. Re-run the
complete campaign before merging any reader fix, and require no per-file
regression on N1 and N2 (see `docs/agents/plan-fiabilite.md`).

## 14. Reader fixes for rejected tunes (2026-11-19)

Each fix is measured with `scripts/oracle/abc/one.sh` on a reduced real-world
tune (one `X:` tune, kept outside the repository). The baseline is section 13:
scoretk N1 78.1 % / N2 37.7 %. Fixes are committed one by one, in decreasing
order of frequency. A construction we do not understand must produce a warning
and a local skip, never the loss of the whole tune.

### 14.1 Back quotes between notes (336 rejected tunes)

- Reduced file: X:1 extracted from `music21-corpus/music21/corpus/airdsAirs/book1.abc`
  into `/tmp/abc-reduits/backtick.abc` (back quotes between `B2` and `G2`,
  and between `FA` and `FD`).
- Behaviour: ABC 2.1 § 4.7 states that back quotes between beamed notes are
  typographic spacers that programs must ignore; `abc2xml.py` also skips them
  as recoverable characters.
- Fix: `reader/music.rs` skips U+0060 in the main parse loop and before every
  note, including notes inside chords and grace notes.
- Measurement: N1/N2 **0 % → 100 %** (69/69 notes), no regression.

### 14.2 Inline first/second endings (195 rejected tunes)

- Reduced file: `/tmp/abc-reduits/volta1.abc`, X:1 extracted from
  `abcjs-tests/extracted-abc/flattener.test__abcRepeat.abc`
  (`...|1"G"g4 fedc:|...`).
- Behaviour: ABC 2.1 § 4.9 allows `|1` and `:|2` as short forms of `|[1` and
  `:[2` (a space is required only in front of the explicit bracket).
  `abc2xml.py` accepts digits after a barline as volta numbers.
- Fix: `reader/music.rs` reads a volta number list after any barline and records
  it on the measure that follows; the bracketed and inline forms now share the
  same `finish_bar` path.
- Measurement: N1/N2 **0 % → 100 %** (13/13 notes), no regression. The unit
  test compares the inline form with the bracketed form note by note.
