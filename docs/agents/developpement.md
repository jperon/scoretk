# Développement : conventions, portabilité et outillage

Mode d'emploi pour écrire du code dans `scoretk`. Le noyau (règles
invariantes, commandes, index) est dans `AGENTS.md` ; ce document donne les
détails et les pièges d'outillage.

## 1. Arborescence et responsabilités

| Chemin | Rôle |
| --- | --- |
| `crates/mscz2xml/` | bibliothèque portable : conteneur, lecteurs `.mscx`, modèle, export MusicXML. **Aucune I/O, aucun thread, aucune horloge.** |
| `crates/mscz2xml/src/container.rs` | `.mscz` (ZIP) → `.mscx`, détection de version |
| `crates/mscz2xml/src/read/` | `read302` (3.x) et `read4xx` (4.x/5.x) → modèle |
| `crates/mscz2xml/src/model/` | modèle de partition en mémoire, pivot des lecteurs et de l'export |
| `crates/mscz2xml/src/export/` | modèle → MusicXML `partwise` / `.mxl` |
| `crates/mscz2xml-cli/` | binaire `mscz2xml` : lecture/écriture des fichiers |
| `crates/mscz2xml-wasm/` | enveloppe wasm-bindgen (`convert`, `version`), sans logique de conversion |
| `crates/scoretk-musicxml/` | lecteur MusicXML → modèle (`read_musicxml`), bibliothèque portable |
| `crates/scoretk-midi/` | écriture SMF format 1 et déroulement des reprises, sans I/O |
| `crates/scoretk-mscx/` | écriture MuseScore (`.mscx` 4.x, `.mscz`), sans I/O |
| `crates/scoretk/` | façade multi-formats : `Format`, détection, `read`/`write` vers le modèle pivot ; features `musicxml-import` et `midi` |
| `crates/scoretk-cli/` | binaire `scoretk` : I/O disque de la façade, formats déduits des extensions |
| `crates/mscz2xml/tests/` | tests d'intégration par famille + e2e de conformité |
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
  `crates/mscz2xml/src/read/instruments.rs` (par
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
code dans `crates/mscz2xml` :

- pas de `std::fs`, `std::env`, `std::net`, `std::process` ;
- pas de `std::thread`, `std::sync::mpsc`, `rayon` ;
- pas d'horloge (`SystemTime`, `Instant`) ni de génération aléatoire non
  déterministe ;
- entrées et sorties uniquement sous forme d'**octets en mémoire** ;
- aucune dépendance C, aucun `build.rs` avec compilation native : le ZIP est
  décodé en Rust pur (`zip` sans fonctionnalités par défaut, `miniz_oxide`) ;
- les tests peuvent utiliser le système de fichiers (ils tournent sur l'hôte),
  pas la bibliothèque ;
- vérifier la cible : `nix develop -c cargo build -p mscz2xml --target
  wasm32-unknown-unknown`.

Les CLI sont les seules à lire et écrire des fichiers ; le crate wasm ne fait
qu'envelopper l'API de la bibliothèque.

## 5. Contrat wasm stable (cantus)

Le paquet produit par `nix build .#wasm` est consommé par la PWA **cantus**.
Ne pas changer sans coordination les noms de fichiers, le nom du paquet ni les
signatures :

| Élément | Valeur stable |
| --- | --- |
| Paquet | `package.json` `name: "scoretk"`, `type: "module"` |
| ESM | `esm/mscz2xml_wasm.js`, `esm/mscz2xml_wasm_bg.wasm`, `esm/mscz2xml_wasm.d.ts` |
| Script classique | `classic/mscz2xml_wasm.js`, `classic/mscz2xml_wasm_bg.wasm`, `classic/mscz2xml_wasm.d.ts` |
| Wrapper worker | `classic/mscz2xml.js` → `self.mscz2xml = { ready, convert, version, baseUrl }` |
| Conversion | `convert(bytes: Uint8Array, options?: { encodingDate?: string }) => string` (MusicXML UTF-8) |
| Version | `version() => string` |

`convert` et `version` du wrapper sont synchrones une fois `ready` résolue et
lèvent une `Error` sinon. Les erreurs Rust deviennent des `Error` JavaScript.
Le détail (initialisation, base URL, taille, exemples web, test Node) est dans
`docs/agents/wasm.md`.

## 6. Commandes

Toujours passer par `nix develop -c` : le `cargo` du `PATH` hors shell Nix est
un shim rustup dont l'installation des composants entre en conflit dans cet
environnement (`detected conflict: lib/rustlib/manifest-rust-std-…`).

```sh
nix develop -c cargo test --workspace
nix develop -c cargo clippy --workspace --all-targets -- -D warnings
nix develop -c cargo fmt --all -- --check
nix develop -c cargo build -p mscz2xml --target wasm32-unknown-unknown

nix flake check          # clippy, tests, fmt, licences, wasm32, smoke Node
nix build .#wasm         # result/{esm,classic,package.json,…}

# Conformité : les deux niveaux (noyau, puis parité complète)
nix develop -c cargo test -p mscz2xml --test e2e_corpus -- --nocapture

# Données dérivées
scripts/gen-references.sh
python3 scripts/gen-instruments-table.py \
  ref-musescore/share/instruments/instruments.xml \
  crates/mscz2xml/src/read/instruments.rs

# Paquet wasm déjà construit
nix develop -c node crates/mscz2xml-wasm/tests/node-smoke.mjs
```

`nix flake check` travaille hors réseau (crane vend les crates depuis
`Cargo.lock`, `CARGO_NET_OFFLINE`) : toute modification de dépendance exige un
`Cargo.lock` à jour et une entrée documentée dans `docs/licences-tierces.md`.

## 7. Tests de conformité : deux niveaux

`crates/mscz2xml/tests/e2e_corpus.rs` compare chaque `.mscx` à sa référence
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
  ou pointer `MSCZ2XML_PKG_DIR` / `MSCZ2XML_CORPUS` ailleurs.
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
