# Développement : conventions, portabilité et outillage

Mode d'emploi pour écrire du code dans `scoretk`. Le noyau (règles
invariantes, commandes, index) est dans `AGENTS.md` ; ce document donne les
détails et les pièges d'outillage.

## 1. Arborescence et responsabilités

| Chemin | Rôle |
| --- | --- |
| `crates/scoretk-core/` | cœur portable et générique : modèle, erreur, `gap_rests`, identifiants stables. **Aucune I/O, aucun thread, aucune horloge, aucun format.** |
| `crates/scoretk-musescore-import/src/container.rs` | `.mscz` (ZIP) → `.mscx`, détection de version |
| `crates/scoretk-musescore-import/src/read/` | `read302` (3.x) et `read4xx` (4.x/5.x) → modèle |
| `crates/scoretk-core/src/model/` | modèle de partition en mémoire, pivot des lecteurs et de l'export |
| `crates/scoretk-musicxml-export/src/` | modèle → MusicXML `partwise` / `.mxl` |
| `crates/scoretk-wasm/` | enveloppe wasm-bindgen de la façade (`convert`, `detect`, `formats`, `version`), unique paquet wasm (`nix build .#wasm`) |
| `crates/scoretk-musicxml-import/` | lecteur MusicXML → modèle (`read_musicxml`), bibliothèque portable |
| `crates/scoretk-musescore-export/` | écriture MuseScore (`.mscx` 4.x, `.mscz`), sans I/O |
| `crates/scoretk-midi-common/` | erreur et résultat partagés par les deux sens, sans conversion |
| `crates/scoretk-midi-import/` | lecture MIDI vers le modèle, sans I/O |
| `crates/scoretk-midi-export/` | écriture SMF format 1 et déroulement des reprises, sans I/O |
| `crates/scoretk-lilypond-common/` | tables partagées par la lecture et l'écriture LilyPond (erreur, `tpc`, clefs), **sans logique de conversion** |
| `crates/scoretk-lilypond-import/` | lecture d'un sous-ensemble de LilyPond vers le modèle, sans I/O |
| `crates/scoretk-lilypond-export/` | écriture LilyPond depuis le modèle (voix en `\fixed c'`), sans I/O |
| `crates/scoretk-abc-common/` | tables et helpers ABC partagés par les deux sens (hauteurs, armures, clés, durées, décorations, paroles), sans conversion |
| `crates/scoretk-abc-import/` | lecture ABC vers le modèle, sans I/O |
| `crates/scoretk-abc-export/` | écriture ABC depuis le modèle, sans I/O |
| `crates/scoretk/` | façade multi-formats : `Format`, détection, `read`/`write` vers le modèle pivot ; dix features symétriques `<format>-import` / `<format>-export`, une par crate de sens, branchées dans `src/hooks.rs` |
| `crates/scoretk-cli/` | binaire `scoretk` : I/O disque de la façade, formats déduits des extensions |
| `crates/scoretk/tests/` | tests d'aller-retour (lecture puis export) et e2e de conformité |
| `crates/scoretk-musescore-import/tests/` | tests de lecture seule par famille |
| `crates/scoretk-musicxml-export/tests/` | tests d'export seul par famille |
| `tests/corpus/{v3,v4}/` | `.mscx` et références `*.ref.musicxml` |
| `scripts/` | génération des références et de la table des instruments, contrôle des licences |
| `flake.nix` | shell de dev, checks, paquet wasm |

`tests/corpus/` doit rester inclus dans la source des checks Nix : le filtre
de `flake.nix` combine `craneLib.filterCargoSources` avec un préfixe
`tests/corpus`, sinon le check `tests` échoue (« No such file or directory »).
Ne pas retirer cette règle.

## 2. Conventions de code

- **Portage fidèle, pas réinvention.** Le code suit le C++ de près : mêmes
  noms de méthodes (`write_measure`, `calc_divisions`, `move_to_tick`), mêmes
  découpages, références au fichier et à la fonction amont dans les
  commentaires. En cas de doute, lire `ref-musescore/` (voir
  `docs/agents/upstream.md`) et les tests amont, jamais coder de mémoire.
- **Nouveau code dans de nouveaux fichiers, par famille.** Un fichier Rust par
  famille fonctionnelle (lecteur 3.x, lecteur 4.x, modèle, export) ; les
  fichiers existants ne reçoivent que le branchement (`mod.rs`, réexport) ou
  un correctif localisé. Cela limite les conflits entre agents parallèles et
  garde les diffs relisibles.
- **Règle des crates `-common`.** Factoriser intelligemment : ce qui est utile
  ou nécessaire à tous les formats va dans un crate commun (p. ex. un futur
  crate d'utilitaires XML partagé par tous les lecteurs/écrivains) ; ce qui
  sert aux deux sens d'un même format va dans le `-common` de ce format ; ce
  qui ne sert qu'à une fonction (un seul sens) reste dans le fichier
  spécifique de ce sens. Pas de module à un seul sens dans un `-common`, pas
  de logique propre à un format dans un crate commun. Le générique transversal
  va dans `scoretk-core`.
- **Commentaires, messages d'erreur et documentation en français.** Les noms
  de types et de fonctions suivent l'amont ou l'usage Rust.
- **Reproductibilité.** Pas d'horloge : `encoding_date` est une option, avec
  une constante par défaut. Pas d'itération sur `HashMap` quand l'ordre de
  sortie compte : utiliser `BTreeMap`/`BTreeSet`.
- **Tests.** Chaque famille a son test d'intégration (`nuances_corpus.rs`,
  `notations_corpus.rs`, `texts_corpus.rs`, `repeats_corpus.rs`, …) et ses
  tests unitaires dans le module. Un correctif de parité met à jour
  `EXPECTED_CONFORMANT` et le document d'écarts correspondant.
- **Fichiers générés, ne pas éditer à la main** :
  `crates/scoretk-musescore-import/src/read/instruments.rs` (par
  `scripts/gen-instruments-table.py`) et `docs/agents/ecarts-complets.md`
  (sortie du test `ecarts_complets_document`). Régénérer, puis relire le diff.

## 3. En-têtes de licence (obligatoire)

Tout fichier source nouveau ou modifié porte, en tête, l'identifiant SPDX, le
copyright et le texte GPL-3.0-only. Pour un fichier porté, ajouter l'avis de
modification exigé par la GPL §5(a) et citer les fichiers C++ d'origine.

Modèle pour un fichier Rust porté (adapter la liste des sources) :

```rust
// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/xxx.h` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see the LICENSE file at the root of this repository or
// <https://www.gnu.org/licenses/gpl-3.0.html>.
```

Même en-tête (commentaires `#`) pour un script Python ou shell, sans la partie
« Portage de » si le fichier est original. Un nouveau fichier de données ou une
nouvelle dépendance impose en plus une mise à jour de `NOTICE` et, pour une
crate tierce, de `docs/licences-tierces.md` ; `scripts/check-licences.py`
refuse toute licence absente de sa liste blanche.

## 4. Portabilité de la bibliothèque

La bibliothèque doit compiler et tourner sur `wasm32-unknown-unknown` (sans
WASI), Android, iOS, Linux, macOS et Windows. Checklist avant d'ajouter du
code dans les crates de bibliothèque (`scoretk-core`, `scoretk-musescore-import`,
`scoretk-musescore-export`, `scoretk-musicxml-import`, `scoretk-musicxml-export`,
`scoretk-midi-common`, `scoretk-midi-import`, `scoretk-midi-export`,
`scoretk-lilypond-common`, `scoretk-lilypond-import`, `scoretk-lilypond-export`,
`scoretk-abc-common`, `scoretk-abc-import`, `scoretk-abc-export`) :

- pas de `std::fs`, `std::env`, `std::net`, `std::process` ;
- pas de `std::thread`, `std::sync::mpsc`, `rayon` ;
- pas d'horloge (`SystemTime`, `Instant`) ni de génération aléatoire non
  déterministe ;
- entrées et sorties uniquement sous forme d'**octets en mémoire** ;
- aucune dépendance C, aucun `build.rs` avec compilation native : le ZIP est
  décodé en Rust pur (`zip` sans fonctionnalités par défaut, `miniz_oxide`) ;
- les tests peuvent utiliser le système de fichiers (ils tournent sur l'hôte),
  pas la bibliothèque ;
- vérifier la cible : `nix develop -c cargo build -p scoretk-core --target
  wasm32-unknown-unknown` (et de même pour chaque crate de bibliothèque).

Les CLI sont les seules à lire et écrire des fichiers ; le crate wasm ne fait
qu'envelopper l'API de la bibliothèque.

## 5. Contrat wasm stable (cantus)

Le paquet produit par `nix build .#wasm` est consommé par la PWA **cantus**.
Ne pas changer sans coordination les noms de fichiers, le nom du paquet ni les
signatures :

| Élément | Valeur stable |
| --- | --- |
| Paquet | `package.json` `name: "scoretk"`, `type: "module"` |
| ESM | `esm/scoretk_wasm.js`, `esm/scoretk_wasm_bg.wasm`, `esm/scoretk_wasm.d.ts` |
| Script classique | `classic/scoretk_wasm.js`, `classic/scoretk_wasm_bg.wasm`, `classic/scoretk_wasm.d.ts` |
| Wrapper worker | `classic/scoretk.js` → `self.scoretk = { ready, convert, convertWithTimeline, version, detect, formats, baseUrl }` |
| Conversion | `convert(bytes: Uint8Array, options?: { from?, to?, encodingDate?, software?, concertPitch?, expandRepeats? }) => Uint8Array` (`to` = `"musicxml"` par défaut) |
| Lecture + timeline | `convertWithTimeline(bytes, options?) => { musicxml: Uint8Array, midi: Uint8Array, timeline: Timeline }` (additif, `to` ignoré) |
| Détection | `detect(bytes: Uint8Array) => string \| null` |
| Formats | `formats() => FormatInfo[]` |
| Version | `version() => string` |

`convert`, `convertWithTimeline`, `detect`, `formats` et `version` du wrapper
sont synchrones une fois `ready` résolue et lèvent une `Error` sinon. Les erreurs Rust deviennent
des `Error` JavaScript. Le détail (initialisation, base URL, taille, exemples
web, test Node, migration) est dans `docs/agents/wasm.md`.

C'est une rupture assumée avec l'ancien contrat mono-format (qui renvoyait
du texte et exposait un global dédié) : Cantus décode désormais le
`Uint8Array` avec `TextDecoder` (détail : `docs/agents/wasm.md` § 9).

## 6. Commandes

Toujours passer par `nix develop -c` : le `cargo` du `PATH` hors shell Nix est
un shim rustup dont l'installation des composants entre en conflit dans cet
environnement (`detected conflict: lib/rustlib/manifest-rust-std-…`).

```sh
nix develop -c cargo test --workspace
nix develop -c cargo clippy --workspace --all-targets -- -D warnings
nix develop -c cargo fmt --all -- --check
nix develop -c cargo build -p scoretk-core --target wasm32-unknown-unknown

nix flake check          # clippy, tests, fmt, licences, wasm32, smoke Node
nix build .#wasm         # result/{esm,classic,package.json,…}

# Conformité : les deux niveaux (noyau, puis parité complète)
nix develop -c cargo test -p scoretk --test e2e_corpus -- --nocapture

# Données dérivées
scripts/gen-references.sh
python3 scripts/gen-instruments-table.py \
  ref-musescore/share/instruments/instruments.xml \
  crates/scoretk-musescore-import/src/read/instruments.rs

# Paquet wasm déjà construit
nix develop -c node crates/scoretk-wasm/tests/node-smoke.mjs
```

`nix flake check` travaille hors réseau (crane vend les crates depuis
`Cargo.lock`, `CARGO_NET_OFFLINE`) : toute modification de dépendance exige un
`Cargo.lock` à jour et une entrée documentée dans `docs/licences-tierces.md`.

## 7. Tests de conformité : deux niveaux

`crates/scoretk/tests/e2e_corpus.rs` compare chaque `.mscx` à sa référence
4.7.4 après normalisation, à deux niveaux :

1. **Noyau musical** (`Level::Core`) : retire `<identification>`, `<defaults>`,
   `<credit>`, `<direction>`, `<stem>`, `<beam>`, `<print>`, les attributs de
   mise en forme et de position (`default-x`, `placement`, `font-*`,
   `print-object`, …), `<notations>` sauf `tied`/`tuplet`, `<barline>` sauf
   `repeat`. Seuls les fichiers de `EXPECTED_CONFORMANT` doivent être
   identiques ; un écart sur ces fichiers fait échouer le test.
2. **Parité complète** (`Level::Full`) : ne retire que la mise en page pure
   (`default-x/y`, `relative-x/y`, `width`, `bezier-x/y`, `<defaults>`,
   `<print>`, `<system-layout>`, `<staff-layout>`, `<encoding-date>`). Le test
   est non bloquant et produit `docs/agents/ecarts-complets.md` (marqueurs
   `---DEBUT DOC---` / `---FIN DOC---` dans la sortie).

Les écarts connus et leurs causes sont dans `docs/agents/ecarts-noyau.md`.
Lire le tableau avant de choisir quoi corriger ; un fichier non listé dans
`EXPECTED_CONFORMANT` peut régresser sans faire échouer la suite.

## 8. Pièges d'outillage et de portage

Ceux qui ne produisent aucune erreur visible :

- **Références régénérées avec une autre version de MuseScore** : la cible de
  conformité change en silence. `scripts/gen-references.sh` utilise
  `nix shell nixpkgs#musescore` (4.7.4 au moment de la rédaction) ; vérifier la
  version affichée au lancement et ne la changer qu'explicitement (voir
  `docs/agents/upstream.md`).
- **`wasm-bindgen` épinglé** : la crate est en `=0.2.130`, égale au
  `wasm-bindgen-cli` du flake. Une mise à jour du flake sans changer
  l'épingle fait échouer `nix build .#wasm` (« schema version mismatch »).
- **Tests e2e et corpus** : ils lisent `tests/corpus` via
  `CARGO_MANIFEST_DIR/../../tests/corpus`. Lancer les tests depuis n'importe
  quel répertoire fonctionne, mais le corpus doit être présent (voir § 1).
- **Smoke Node** : il charge `result/esm` ; lancer `nix build .#wasm` avant,
  ou pointer `SCORETK_PKG_DIR` / `SCORETK_CORPUS` ailleurs.
- **`ref-musescore/` absent** : le dépôt ne le versionne pas. Les en-têtes
  citent des chemins qui n'existent pas dans une extraction fraîche ; recréer
  la copie avec `docs/agents/upstream.md`.
- **Fichiers générés** : `instruments.rs` et `ecarts-complets.md` sont des
  sorties de scripts/tests. Une modification manuelle est perdue à la
  régénération.
- **Ordre de sortie** : l'ordre des `<direction>`, des spanners et des
  `<credit>` est significatif pour la parité ; ne pas « ranger » une
  collection sans vérifier le C++ et la référence.
- **Silences apparents** : un élément non modélisé est ignoré sans message
  (`// TODO(parité)` dans l'export). Un écart e2e qui ne bouge pas après un
  correctif vient souvent d'un champ non lu en amont, pas de l'export.
- **`<tick>` et durées** : voir les pièges du format dans
  `docs/agents/mscx-format.md` § 17 (tick implicite, `actualTicks`, ordre de
  `<dots>`, voix positionnelles, liaisons 3.x/4.x vs 5.0, spanners 5.0).

## 9. Intégration continue et releases

Deux workflows GitHub Actions vivent dans `.github/workflows/` et s'appuient
sur Nix (même toolchain que `nix develop -c`) :

- `ci.yml` — sur push et pull request vers `master` : `cargo fmt --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, les builds `wasm32-unknown-unknown` de la liste du
  § Commandes de `AGENTS.md`, `nix build .#wasm` puis le test Node
  `node-smoke.mjs`. Le corpus
  `tests/corpus` est versionné ; le seul test lisant un fichier hors dépôt
  (`change_dans_beam_cross_staff` de `scoretk-lilypond-import`) retourne
  silencieusement quand ce fichier est absent, donc sans effet sur la CI.
- `release.yml` — voir ci-dessous.

Les deux fichiers se valident localement avec `actionlint` (`act` n'est pas
installé dans le shell de dev) :

```sh
nix-shell -p actionlint --run 'actionlint .github/workflows/*.yml'
```

### Publier une version

1. Mettre à jour `version` dans `[workspace.package]` de `Cargo.toml` (et, si
   besoin, `version` de `flake.nix`) pour que `scoretk --version` et `version()`
   du wasm coïncident avec l'étiquette.
2. Laisser la CI passer sur `master`, puis :

   ```sh
   git tag v0.2.0 && git push origin v0.2.0
   ```

3. Le déclencheur `v*` crée la release GitHub nommée d'après l'étiquette, avec
   des notes générées automatiquement. Ne pas la créer à la main.

### Rolling release « nightly »

Chaque push sur `master` produit une release **prerelease** sur l'étiquette
mobile `nightly` : l'étiquette est déplacée sur le dernier commit et les
anciens assets sont supprimés avant l'envoi des nouveaux (la concurrence annule
un run de rolling encore en cours). Elle est toujours au même endroit :

- <https://github.com/jperon/scoretk/releases/tag/nightly>

`releases/latest` continue de désigner la dernière release `v*`, pas la
rolling. Les archives portent la version calculée : l'étiquette `vX.Y.Z` pour
une release normale, `0.1.0-nightly.AAAAAMMJJ.<sha court>` pour la rolling
(version de base lue dans `Cargo.toml`). Le binaire CLI est compilé avec la
version du `Cargo.toml` : la suffixation n'est pas réécrite dans le code Rust,
elle apparaît dans les noms d'archives et dans la version du paquet npm. Pour
qu'une nightly affiche son suffixe dans `scoretk --version`, il faudrait modifier
le code : ce n'est pas fait ici.

### Contenu des artefacts

Pour chaque cible (`x86_64-unknown-linux-musl`, `aarch64-apple-darwin`,
`x86_64-apple-darwin`, `x86_64-pc-windows-msvc`), une archive
`scoretk-<version>-<cible>.tar.gz` (`.zip` sur Windows) contient le binaire
`scoretk`, `LICENSE`, `NOTICE`, `README.md` et `SOURCES.txt`. Le paquet wasm
(sortie de `nix build .#wasm`, empaquetée par `npm pack`) donne
`scoretk-<version>-wasm.tgz` avec les mêmes avis. `SOURCES.txt` porte l'URL
exacte de la source correspondante (étiquette ou commit de la nightly), comme
l'exige la GPL §6 (voir `docs/agents/licence.md`). Un fichier `SHA256SUMS`
global est joint à chaque release.

Le workflow publie avec `GITHUB_TOKEN` et `permissions: contents: write`
uniquement dans le job de publication ; les autres jobs restent en lecture
seule. Aucune étiquette ni release n'est créée depuis le poste de travail.
