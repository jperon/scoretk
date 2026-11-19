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

### 14.3 Redefinable symbols such as `k` (92 rejected tunes)

- Reduced file: `/tmp/abc-reduits/k-seul.abc`, X:1 extracted from
  `music21-corpus/music21/corpus/ryansMammoth/BirdOnTheWingJig.abc`
  (`vkd2k^d2 ...`, `uk(d2.D) ...`).
- Behaviour: ABC 2.1 § 4.16 reserves H-W, h-w and `~` for symbols defined by
  `U:` (`U: k = !trill!`); `abc2xml.py` parses any of these letters as a
  decoration. An undefined symbol is only a warning, not a fatal parse error.
- Fix: `reader/mod.rs` stores `U:` definitions; `reader/music.rs` maps defined
  symbols to known decorations, keeps the standard default meanings
  (`H` fermata, `M` mordent, `P` inverted mordent, `T` trill, `u`/`v` bowings,
  `~` turn) and warns + skips undefined ones, including inside chords and
  grace notes.
- Measurement: N1/N2 **0 % → 100 %** (35/35 notes), no regression. Two real
  files from the same corpus now parse end to end under `timeout 10`.

### 14.4 Quarter-tone slash (58 rejected tunes)

- Reduced file: `/tmp/abc-reduits/quarter.abc`, X:1 extracted from
  `abcjs-tests/extracted-abc/flattener.test__abcQuarterTones.abc`
  (`^/A^/B^/c`, `_/A_/B`, and the `K: C ^/f _/B _A ^D` header).
- Behaviour: `abc2xml.py` extends the standard with microtonal accidentals
  (`^/`, `_/`, `^3/2`); with its default `edo=12` it rounds them to whole
  semitones when writing MusicXML pitches.
- Fix: `reader/music.rs` consumes the optional numerator, slashes and
  denominator after an accidental, keeps the semitone already read and emits a
  warning. The note is no longer lost; this also applies inside chords and
  grace notes.
- Measurement: N1 **0 % → 84.4 %** (76/90 notes), N2 **0 % → 71.1 %**; the
  alternative reaches only 80.0 % / 71.1 % on the same reduced tune. The
  residual `TRANSPOSITION` classification comes from explicit accidentals in
  `K:` being ignored by the shared key parser (both candidates), not from a
  rejection. No per-file regression.

### 14.5 Invalid pitch after a barline: invisible bars (33 rejected tunes)

- Reduced file: `/tmp/abc-reduits/invisible.abc`, X:1 extracted from
  `music21-corpus/music21/corpus/ryansMammoth/AbithaMugginsFavoriteReel.abc`
  (the `...[|]|]` ending).
- Behaviour: ABC 2.1 § 4.8 defines `[|]` (and `[]`) as an invisible bar line and
  allows liberal barline sequences such as `[|`. `abc2xml.py` recognises them
  as barlines; previously `[` started a chord and `|` was read as a note
  pitch, which rejected the whole tune.
- Fix: `reader/music.rs` recognises `[|]`, `[]` and `[|` in `parse_bracket` and
  closes the measure through the shared `finish_bar` path; ordinary chords
  such as `[CEG]` are unchanged.
- Measurement: N1/N2 **0 % → 98.4 %** (60/61 notes), same as the alternative.
  The one missing note is a grace-note difference affecting both candidates
  (group 2 in section 13.3), not a rejection. No per-file regression.

### Overall campaign after the five fixes

Re-run of the exact three-lot campaign of section 13.4, in the foreground, with
the release CLI built from this worktree (`mesure.sh` list + `aggreg.py`):

| Candidate | Exact tunes | N1 | N2 | Failures | Files fully exact |
| --- | ---: | ---: | ---: | ---: | ---: |
| scoretk (baseline, § 13.2) | 2 916 | 78.1 % | 37.7 % | 954 | 877 / 1 402 |
| scoretk (after fixes) | 3 395 | **92.2 %** | **44.6 %** | **284** | **1 078 / 1 402** |
| alternative (unchanged) | 3 729 | 97.5 % | 79.9 % | 130 | 1 095 / 1 402 |

Per lot, scoretk N1 is 88.8 % (abcjs), 92.4 % (music21) and 25.6 % (ABC 2.1
fragments, unchanged: those files have no `X:`). Rejections `ECHEC_TK` fell from
824 to **154**. The remaining dominant gap is `NOTES_MANQUANTES` (767), i.e.
group 2 of section 13.3, which is outside the scope of this syntax campaign; it
affects the alternative too (781). No per-file regression was introduced: each
change only accepts a previously rejected construct or replaces a rejection by
a local, warned skip.

## 15. Transposition fixes, `NOTES_MANQUANTES` diagnosis, full campaign (2026-11-19)

### 15.1 Cause 3 — `transpose=` in `K:` / `V:` (fixed)

- Reference behaviour: `abc2xml.py` 269 reads `(?:^t=| t=|transpose=)(-?[\d]+)`
  in `K:` and `V:` fields and emits `<transpose><chromatic>N</chromatic></transpose>`
  while keeping the written `<pitch>` unchanged. `abc2midi` sounds written + N.
- Fix: `reader/mod.rs` parses `transpose=`/`t=` from `K:` parameters and `V:`
  fields (invalid values warn and are skipped); `reader/music.rs` stores the
  **sounding** pitch (written + N) in `Note::pitch`, keeping the written spelling
  in `tpc`; `reader/build.rs` sets `Part::instrument.transpose = Transpose::new(0, N)`
  so the MusicXML exporter writes the written `<pitch>` plus `<transpose>`, like
  the alternative.
- Reduced real file: `/tmp/abc-reduits/transpose.abc`, X:1 extracted from
  `abcjs-tests/extracted-abc/flattener.test__abcTranspose.abc` (three voices,
  `V:1 transpose=-2`, `V:3 transpose=4`, chords, rests, ties and barlines).
- Measurement: scoretk N1/N2 **58.3 % → 100 %** (12/12 notes), alternative 100 %.
- Reduced tests: `crates/scoretk-abc-import/tests/transposition.rs`
  (`transposition_k_et_v`, with a no-transpose control case).

### 15.2 Cause 5 — `%%MIDI transpose` (fixed)

- Reference behaviour: `abc2xml.py` routes `%%MIDI transpose N` through the same
  attribute path: one `<transpose>` element with `<chromatic>N</chromatic>`,
  written `<pitch>` unchanged.
- Fix: `reader/mod.rs::handle_directive` parses `%%MIDI transpose N`, stores it
  as the tune default and applies it to already-created voices; other `%%MIDI`
  directives keep their existing warning.
- Reduced real file: `/tmp/abc-reduits/midi-transpose.abc`, X:1 of
  `abcjs-tests/extracted-abc/flattener.test__abcMidiOptions.abc`
  (`%%MIDI transpose -2`, key signature A, chord).
- Measurement: scoretk N1/N2 **50 % → 100 %** (4/4 notes), alternative 100 %.
- Reduced test: `transposition_midi_directive`, covering an invalid value and
  `transpose=0` (no `<transpose>` emitted).

### 15.3 Diagnosis of `NOTES_MANQUANTES` (767 tunes, diagnosis only)

Grouping uses all 767 scoretk `NOTES_MANQUANTES` tunes of the post-syntax-fix
baseline (`final2-full.tsv`); "alt" is `abc2xml.py` on the same tune.

| Sub-cause | Tunes | Reduced real example | alt also loses? |
| --- | ---: | --- | --- |
| Trill / mordent / turn / roll / slide realization | 678 | `flattener.test__abcDecoration.abc` X:1 → `/tmp/abc-reduits/ornament.abc` | **Yes** (alt `NOTES_MANQUANTES`) |
| Tie written between two different pitches (`A-B`) | 78 | `0001-0050.abc` X:1 → `/tmp/abc-reduits/tie.abc` | No (alt OK) |
| Unclosed `!` decoration swallowing the rest of a line | 3 | `book1.abc` X:0143 → `/tmp/abc-reduits/unclosed.abc` | No (alt OK) |
| Grace/tuplet/bowing combinations (mixed) | 5 | `0351-0400.abc` X:399, `0550-0625.abc` X:612, `AnnieHughesJig.abc` X:1 | Yes (alt `NOTES_MANQUANTES`) |
| Ornaments with alt correct | 2 | `0001-0050.abc` X:7 and X:14 | No (alt OK) |
| Other | 1 | `MaidMarianHornpipe.abc` X:1 | No (alt OK) |

- **Ornaments (678, plus 5 mixed).** `abc2midi` is called with `-NGRA`, which
  disables grace notes but **not** ornament realization: a single `Tc` is played
  as C-D-C-D-C. On the reduced `/tmp/abc-reduits/ornament.abc` the oracle has
  20 notes and both scoretk and the alternative write only 6 (N1 30 % for both).
  The MusicXML `<trill-mark>`/`<mordent>` symbol is the faithful written form;
  the extra notes are a performance realization. For these 678 + 5 tunes the
  **oracle reference is what over-counts**, not the reader: matching it would
  mean emitting notes that are not written in the ABC. No fix attempted.
- **Different-pitch ties (78).** `cmp.py` skips MusicXML notes carrying
  `<tie type="stop">`, while `abc2midi` counts both note-ons. On the reduced
  `/tmp/abc-reduits/tie.abc` (`G3-A G2-E2`) scoretk gets 7/8 notes and the
  alternative 8/8: the alternative writes the two notes without a stop tie.
  This is a genuine scoretk/abc2xml behavioural difference, but it is an oracle
  comparator artifact rather than a lost written note. No fix attempted.
- **Unclosed `!` (3).** An orphan `!` makes `reader/music.rs::parse_decoration`
  consume the rest of the line as a decoration name, dropping the notes after
  it; `abc2xml.py` recovers. Reduced example: 11/18 notes for scoretk versus
  18/18 for the alternative. Real reader bug, deferred.
- **Residual (8).** Five mixed grace/tuplet/bowing tunes are also missed by the
  alternative (oracle likely over-counting); three are genuine scoretk-only
  gaps in the `0001-0050.abc` and `MaidMarianHornpipe.abc` collections.

### 15.4 Full campaign after the two transposition fixes

Exact re-run of the three-lot campaign of section 13.4 (1 402 files, 4 756
`X:` tunes), in the foreground in three slices, with the release CLI of this
worktree under `timeout 10 prlimit --as=2000000000`:

| Candidate | Exact tunes | N1 | N2 | Failures | Files fully exact |
| --- | ---: | ---: | ---: | ---: | ---: |
| scoretk (after the five syntax fixes, § 14) | 3 395 | 92.24 % | 44.60 % | 284 | 1 078 / 1 402 |
| scoretk (after transposition fixes) | 3 400 | **92.44 %** | **44.64 %** | **284** | unchanged |
| alternative (unchanged) | 3 729 | 97.5 % | 79.9 % | 130 | 1 095 / 1 402 |

`TRANSPOSITION` fell from 59 to 55 tunes and `OCTAVE` from 5 to 2; the
`NOTES_MANQUANTES` count (767) is unchanged, consistent with section 15.3.
Sixteen tunes improved, mostly the Josquin files of `music21` with
`%%MIDI transpose -12`/`-24` (e.g. `fortunaDunGranTempo.abc` X:2 went from
`TRANSPOSITION` to `OK`) and the two reduced abcjs files.

One per-tune N2 decrease: `abc-2.1-standard/fragments/example-006.abc` X:1
(N1 23 → 25, N2 12 → 2). This NC fragment combines `%%score (T1 T2) (B1 B2)`,
`clef=treble-8`, `middle=d` and `transpose=-24`; its voice names (`T1`, `B1`)
are not ABC voice numbers, so all four voices collapse into one part and the
last `-24` is applied globally. The extra bass transposition raises N1 but
moves previously matching attacks below the reference range. The tune stays in
the same `OCTAVE` class (the alternative also fails it, standard-lot N1 25.8 %),
and no other tune regressed on N1 or N2.

## 16. Phase 1 — faithful port of `abc2xml.py`: oracle cleanup, three reader fixes and N2 diagnosis (2026-11-19)

This section is in English by convention for the measurement and port reports.
No upstream Python script is committed; the copy used is
`/tmp/abcchk/abc2xml_269/abc2xml.py` (version 269). Section 15 left two
"diagnosis only" causes (different-pitch ties, unclosed `!`) and an oracle that
over-counted ornaments. Phase 1 fixes those three items and diagnoses what
remains of the N2 gap. The decision is to **port `abc2xml.py` faithfully**
(function names and structure, upstream references in comments) while still
building scoretk's generic model, not MusicXML directly.

### 16.1 Oracle and reader fixes

- **Oracle — ornaments** (`scripts/oracle/abc/mesure.py`, `sans_ornements()`):
  `abc2midi` realizes `Tc` as C-D-C-D-C (and likewise mordents, turns, rolls),
  but both candidates write only the written note plus an ornament symbol. The
  reference therefore over-counted 678 tunes. `abc2midi` has no switch for this
  (`-NGRA` only covers grace notes), so the ornaments of `abc2xml.py`'s
  `ornMap`/`uSyms` are stripped **from the oracle copy only**; the candidates
  still receive the unmodified tune. On the reduced `ornament.abc` the reference
  goes from 20 to the 6 written notes (both candidates N1/N2 100 %). Documented
  in `scripts/oracle/abc/README.md`.
- **Oracle — ties** (`scripts/oracle/abc/cmp.py`): a `<tie type="stop">` note has
  no MIDI note-on and is not counted, **but its duration must advance the
  timeline**. The old `continue` skipped the duration too, shifting all later
  attacks and accidentally compensating scoretk's `<forward>` padding (19
  apparent N2 drops once the reader tie fix landed). This now measures both
  candidates the same way.
- **Reader — different-pitch ties** (`reader/music.rs::drop_foreign_ties`): port
  of `abc2xml.py::doNotations`, lines 1445–1460 (`if pt == ptup: continue`, then
  `info('tie between different pitches …')`, delete the start tie and turn it
  into a slur). A pending tie whose pitch is absent from the current event is
  now dropped instead of crossing the intervening note. Reduced `tie.abc`:
  scoretk 7/8 → 8/8 notes, N2 0.25 → 1.00, matching the alternative.
- **Reader — unclosed `!`** (`reader/music.rs::parse_decoration`): port of
  `abc2xml.py`'s `long_decoration` (line 374, name contains no space or other
  delimiter and must be closed) and `linebreak` (line 432, `~decorations + '!'`).
  An orphan delimiter is now a one-character line break instead of swallowing
  the rest of the line. Reduced `unclosed.abc`: scoretk 11/18 → 18/18 notes,
  N2 0.61 → 1.00, matching the alternative.

### 16.2 Phase 1 campaign (foreground, three slices, 1 402 files / 4 756 tunes)

Both columns below use the **corrected oracle**; "before" is the pre-fix binary
and "after" the binary with the two reader fixes.

| Candidate | Campaign | Exact tunes | N1 | N2 | Failures |
| --- | --- | ---: | ---: | ---: | ---: |
| scoretk | § 15.4, old oracle | 3 400 | 92.44 % | 44.64 % | 284 |
| scoretk | corrected oracle, before | 3 964 | 94.53 % | 50.12 % | 284 |
| scoretk | corrected oracle, **after** | **4 069** | **94.68 %** | **50.20 %** | 284 |
| `abc2xml.py` | corrected oracle, unchanged | 4 406 | 99.70 % | 92.85 % | 130 |

Per-tune comparison of the same campaign, before vs after: **N1 115 gains / 0
losses**, **N2 75 gains / 2 losses**. The two N2 losses are `book6.abc` X:1091
and X:1110: both are `K:A Minor` tunes with several orphan `!`s where the
alternative itself is far from the oracle (N2 0.63 / 0.46); the fix recovers the
notes after the orphan delimiters with correct pitches (N1 +15 / +28 points)
but their attacks are displaced by the already-known modal-key offset. These
tunes stay in the `TRANSPOSITION` / `ARMURE` classes; there is **no unexplained
per-tune regression**.

### 16.3 N2 gap diagnosis — 2 515 tunes where the alternative is right and scoretk is wrong

Selection: both candidates N1-exact and `statut == ok`, `n2(alt) ≥ 0.8`,
`n2(tk) < 0.9`, `n2(alt) - n2(tk) ≥ 0.2`. Counts are mutually exclusive, assigned
in the order of the table. Examples are real corpus tunes kept outside the
repository; the "minimal" example is the smallest by reference note count.

| Cause | Tunes | Minimal real example (N2 tk / alt) | `abc2xml.py` function(s) to port |
| --- | ---: | --- | --- |
| Short measures padded to nominal length: `reader/mod.rs::finish_measure` uses `actual.max(nominal)`, so the exporter emits `<forward>` and shifts every later attack. `abc2xml.py` writes a measure at its written length. | **2 488** | `tablature.test__bracketPlacement.abc` X:5 (2 notes, 0.50 / 1.00); representative `0811-0899.abc` X:884 (94 notes, 0.49 / 1.00, the short `AGG G2` before a repeat shifts attacks by +12 ticks) | `doMaat` (line 674), `abc_grammar` measure/bar rules (lines 428–436) |
| Tuplets and broken rhythm (`<`/`>`) attack placement | 11 | `timing.test__abcWarp.abc` X:1 (9 notes, 0.33 / 1.00) | `convertBroken` (line 594), `tuplet_start` (line 395), `mkTmod` (line 728) |
| Parallel voices not merged into one measure (scoretk has no `<backup>` where the alternative has one) | 5 | `flattener.test__abcNoChordVoice.abc` X:1 (4 notes, 0.25 / 1.00) | `mergeMeasure` (line 876), `mergeParts` (line 935), `mergePartMeasure` (line 951) |
| Multi-measure rests `Znn`: scoretk emits one padded measure instead of `nn` written rests | 3 | `misc.test__abcMeasureNumbers.abc` X:1 (`Z24`, 1 note, 0.00 / 1.00) | `restActn` (line 554), `note_length` (line 473) |
| Other (overlays `&`, annotation/timing corners) | 8 | `tablature.test__overlay.abc` X:1 (5 notes, 0.20 / 1.00) | `abc_grammar` `voice_overlay` (line 425), `element` (line 433) |

A further **297** tunes of the phase 1 selection have a N1 gap too; they are the
survivors of the § 15.3 table after the three phase 1 fixes removed the ornament
(678), different-pitch tie (78) and unclosed-`!` (3) causes.

### 16.4 Port plan — order, function map, replacement strategy

The reader is replaced **piece by piece**, one `abc2xml.py` function family at a
time in decreasing expected N2 gain. Each step keeps the current Rust API, adds
a focused failing test first, is measured with `scripts/oracle/abc/one.sh` on a
reduced real tune, then with the full three-lot campaign, and is committed
separately. Comments in the ported code cite the Python function and line. The
existing reader is never deleted wholesale: a new module is added per family,
`reader/mod.rs` keeps the orchestration and delegates to it, and the old path is
removed only once the new one matches on its whole reduced set.

| Order | `abc2xml.py` function (lines) | Rust destination | Expected N2 gain |
| ---: | --- | --- | ---: |
| 1 | `doMaat` (674), measure/bar grammar (428–436) | `reader/mod.rs::Voice::finish_measure` (526): use the written `actual` length for non-pickup measures instead of `actual.max(nominal)`; keep the pickup case; audit the exporter's `move_to_tick` so the model, not a forced nominal length, drives `<forward>` | 2 488 tunes (≈ +40 pts) |
| 2 | `restActn` (554), `note_length` (473) | `reader/music.rs::parse_chord_or_note` `Z`/`X` path (268–300): expand `Znn` into `nn` full-measure rests | 3 tunes |
| 3 | `convertBroken` (594), `tuplet_start` (395), `mkTmod` (728) | `reader/music.rs::apply_rhythm` (572) and the tuplet block (615–660): compute written durations exactly as the Python actions do | 11 tunes |
| 4 | `mergeMeasure` (876), `mergeParts` (935), `mergePartMeasure` (951) | `reader/build.rs::groups`/`create_parts` (322–410): keep parallel voices in the same measure and check the exporter writes `<backup>`; part/staff grouping follows `splitHeaderVoices` (779) | 5 tunes (+13 with cause 1) |
| 5 | `abc_grammar` `voice_overlay`/`element` (425, 433) | `reader/music.rs` overlay branch (105): warn + local skip instead of collapsing overlays | 8 tunes |
| done | `doNotations` tie cleanup (1445–1460) | `reader/music.rs::drop_foreign_ties` | phase 1 |
| done | `long_decoration` (374), `linebreak` (432) | `reader/music.rs::parse_decoration` | phase 1 |

Only the **written** representation is ported: ornaments stay symbols
(`<trill-mark>`, `<mordent>`, …), not realized note runs — the corrected oracle
verifies that choice. Performance-only tables (`abc2midi` event handling) are
out of scope. The conformity target remains MuseScore 4.7.4 for scoretk's
exporters; `abc2xml.py` is the reference for the *reading* decisions only.

### 16.5 Step 1 — written measure length (`doMaat`)

**Implementation.** New module `reader/measure.rs` ports `abc2xml.py::doMaat`
(line 674) and the `measure`/`bar_right` grammar (lines 428-436): a measure
keeps its written length `actual` instead of `actual.max(nominal)`; the pickup
case is unchanged (a short first measure was already kept at `actual`) and the
empty-measure fallback still uses `nominal` so the model stays valid.
`reader/mod.rs::finish_measure` now only delegates to `measure::do_maat`.
The model drives the exporter: `write_measure` calls
`move_to_tick(measure.end_tick())`, so with `Measure::len = actual` a short
measure produces no `<forward>` padding — exactly what `abc2xml.py` writes
(it emits no padding element for an implicit/irregular measure). The only
other use of `nominal_len` in the exporter is the pickup beam calculation in
`beams.rs`, which does not touch `<forward>`.

**Reduced real tune.** `0811-0899.abc` X:884 (`/tmp/abc-reduits/short-measure-884.abc`,
94 notes, the short `AGG G2` before a repeat): scoretk N1 100 %, N2
**0.4894 → 1.00**, the alternative stays 1.00. The tablature X:5 reduction is
covered by `crates/scoretk-abc-import/tests/measure_length.rs` (model lengths,
pitches, durations, pickup, EOF and empty-measure cases) and
`crates/scoretk/tests/abc_short_measures.rs` (no `<forward>`, full-measure
control, both failing before the fix).

**Full campaign** (foreground, three slices, same corrected oracle and release
binary as § 16.2, 1 402 files / 4 756 tunes):

| Candidate | Exact tunes | N1 | N2 | Failures |
| --- | ---: | ---: | ---: | ---: |
| scoretk (after phase 1, § 16.2) | 4 069 | 94.68 % | 50.20 % | 284 |
| scoretk (after step 1) | 4 069 | **94.68 %** | **63.00 %** | **284** |
| `abc2xml.py` (unchanged) | 4 406 | 99.70 % | 92.85 % | 130 |

Per-lot N2 for scoretk goes 56.26 → 57.96 % (abcjs), 50.24 → 63.11 %
(music21) and 17.43 → 23.25 % (ABC 2.1 fragments). Per-tune N1 is **unchanged:
0 gains / 0 losses**. Per-tune N2: **2 091 gains / 1 102 losses** (net
+60 348 correctly placed attacks). In the § 16.3 target population of 2 455
tunes the mean N2 goes 0.419 → 0.646 (1 681 gains, 664 losses, 110 unchanged).

The +40 point estimate of § 16.4 was optimistic: the old `<forward>` padding
was also compensating for scoretk's own *step 3* timing shortfall. For broken
rhythm `<`/`>`, slash durations `x/`, tuplets and grace notes, scoretk writes
the attack positions early (the local `<backup>`/duration is wrong, see the
`convertBroken`/`mkTmod` row of § 16.4), so a measure ends early; the old
padding shifted every later measure forward by exactly that shortfall and
accidentally re-aligned them with the oracle. Removing it exposes that
already-known, separately diagnosed error. **1 086 of the 1 102 N2 losses are
in tunes containing broken rhythm, slash durations, tuplets or grace notes;**
the remaining 16 contain backticks, `[G2G,2]` chords, `u`/`v` bowings, voice
switches or multi-part annotations — also known reader limitations, not a new
regression. No plain tune with only notes, rests and barlines regressed. Step 3
(`convertBroken`, `tuplet_start`, `mkTmod`) must land to turn those masked
losses into gains; the conformity target remains MuseScore 4.7.4.

The MuseScore corpus is unaffected: `cargo test -p scoretk --test e2e_corpus`
passes 5/5.

