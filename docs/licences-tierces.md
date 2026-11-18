# Licences des dépendances tierces

Ce document liste les crates tierces **effectivement liées** dans les binaires
produits par le workspace (`scoretk-core`, `scoretk` et sa CLI, plus les crates
wasm et MIDI), avec leur licence, et vérifie leur compatibilité avec la licence
du projet, **GPL-3.0-only**.

## Portée et méthode

```console
$ cargo tree --offline --workspace --edges normal,build --prefix none \
      --format '{p}|{l}'
```

La commande ne fait aucun accès réseau et n'inclut ni les dépendances de
développement (`--edges normal,build`), ni les paquets présents dans
`Cargo.lock` pour d'autres cibles ou pour des fonctionnalités optionnelles
(voir plus bas) ; les dépendances de développement, dont celles du banc de
performance `criterion`, sont listées à part (§ « Dépendances de
développement »). Résultat au 2026-11-17 : **46 crates tierces**, toutes
compatibles.

## Tableau

| Crate | Version | Licence déclarée | Rôle |
| --- | --- | --- | --- |
| `adler2` | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | core (via miniz_oxide) |
| `anstream` | 1.0.0 | MIT OR Apache-2.0 | CLI (via clap) |
| `anstyle` | 1.0.14 | MIT OR Apache-2.0 | CLI (via clap) |
| `anstyle-parse` | 1.0.0 | MIT OR Apache-2.0 | CLI (via anstream) |
| `anstyle-query` | 1.1.5 | MIT OR Apache-2.0 | CLI (via anstream) |
| `bumpalo` | 3.20.4 | MIT OR Apache-2.0 | core et wasm (via zopfli, wasm-bindgen-macro-support) |
| `cfg-if` | 1.0.5 | MIT OR Apache-2.0 | core (via zip, js-sys) |
| `clap` | 4.6.7 | MIT OR Apache-2.0 | CLI (arguments) |
| `clap_builder` | 4.6.7 | MIT OR Apache-2.0 | CLI (via clap) |
| `clap_derive` | 4.6.7 | MIT OR Apache-2.0 | CLI (proc-macro) |
| `clap_lex` | 1.1.1 | MIT OR Apache-2.0 | CLI (via clap) |
| `colorchoice` | 1.0.5 | MIT OR Apache-2.0 | CLI (via anstream) |
| `crc32fast` | 1.5.5 | MIT OR Apache-2.0 | core (via zip, flate2, zopfli) |
| `equivalent` | 1.0.3 | Apache-2.0 OR MIT | core (via indexmap) |
| `flate2` | 1.1.11 | MIT OR Apache-2.0 | core (via zip) |
| `futures-core` | 0.3.34 | MIT OR Apache-2.0 | wasm (via js-sys) |
| `futures-task` | 0.3.34 | MIT OR Apache-2.0 | wasm (via futures-util) |
| `futures-util` | 0.3.34 | MIT OR Apache-2.0 | wasm (via js-sys) |
| `hashbrown` | 0.17.1 | MIT OR Apache-2.0 | core (via indexmap) |
| `heck` | 0.5.0 | MIT OR Apache-2.0 | CLI (proc-macro, via clap_derive) |
| `indexmap` | 2.14.2 | Apache-2.0 OR MIT | core (via zip) |
| `is_terminal_polyfill` | 1.70.2 | MIT OR Apache-2.0 | CLI (via anstream) |
| `js-sys` | 0.3.107 | MIT OR Apache-2.0 | wasm (bindings JS) |
| `log` | 0.4.35 | MIT OR Apache-2.0 | core et wasm (via zopfli, wasm-bindgen) |
| `memchr` | 2.8.3 | Unlicense OR MIT | core (via quick-xml, zip) |
| `midly` | 0.5.3 | Unlicense | MIDI (format de fichier, `scoretk-midi-import` et `scoretk-midi-export`) |
| `miniz_oxide` | 0.9.1 | MIT OR Zlib OR Apache-2.0 | core (via flate2) |
| `pin-project-lite` | 0.2.17 | Apache-2.0 OR MIT | wasm (via futures-util) |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 | proc-macro |
| `quick-xml` | 0.42.0 | MIT | core (lecture XML) ; tests de `scoretk-lilypond-import` (`read_corpus`) |
| `quote` | 1.0.47 | MIT OR Apache-2.0 | proc-macro |
| `rustversion` | 1.0.23 | MIT OR Apache-2.0 | wasm (build, via wasm-bindgen) |
| `simd-adler32` | 0.3.10 | MIT | core (via miniz_oxide, zopfli) |
| `slab` | 0.4.12 | MIT | wasm (via futures-util) |
| `strsim` | 0.11.1 | MIT | CLI (via clap) |
| `syn` | 3.0.10 | MIT OR Apache-2.0 | proc-macro |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 | core (erreurs) |
| `thiserror-impl` | 2.0.21 | MIT OR Apache-2.0 | core (proc-macro) |
| `unicode-ident` | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | proc-macro |
| `utf8parse` | 0.2.2 | Apache-2.0 OR MIT | CLI (via anstyle-parse) |
| `wasm-bindgen` | 0.2.130 | MIT OR Apache-2.0 | wasm (bindings) |
| `wasm-bindgen-macro` | 0.2.130 | MIT OR Apache-2.0 | wasm (proc-macro) |
| `wasm-bindgen-macro-support` | 0.2.130 | MIT OR Apache-2.0 | wasm (proc-macro) |
| `wasm-bindgen-shared` | 0.2.130 | MIT OR Apache-2.0 | wasm (via wasm-bindgen) |
| `zip` | 2.6.1 | MIT | core (conteneur .mscz) |

## Compatibilité avec GPL-3.0-only

Toutes les licences relevées sont **permissives** et compatibles avec la
GPL-3.0-only, qui peut absorber du code sous licence permissive et le
redistribuer sous GPL :

- **MIT**, **ISC**, **0BSD**, **Zlib**, **X11**, **BSD-2-Clause**,
  **BSD-3-Clause**, **CC0-1.0**, **Unlicense** : permissives, compatibles
  avec toutes les versions de la GPL. Les clauses « publicité » de BSD-3 et
  X11 sont satisfaites par le fichier `NOTICE` (origine du projet, pas de
  liste de contributeurs à afficher en plus).
- **Apache-2.0** (seule ou en alternative) : compatible avec la GPL **v3**
  (la FSF la classe compatible GPLv3 ; l'incompatibilité connue concerne
  GPLv2). La clause de brevets et les obligations de notice d'Apache-2.0
  s'appliquent en plus : le fichier `NOTICE` du projet et ce document en
  tiennent lieu pour l'ensemble.
- **Unicode-3.0** (via `unicode-ident`) : licence permissive de données
  Unicode, compatible.
- Les expressions `A OR B` n'imposent que le respect de la branche choisie ;
  toutes les alternatives proposées sont dans la liste blanche.

Aucune dépendance n'est sous GPL, LGPL, AGPL, MPL, EUPL, SSPL ou une licence
non identifiée : aucune question de relicenciement ne se pose.

## Crates présentes dans `Cargo.lock` mais non liées

Ces paquets n'apparaissent pas dans le graphe `normal,build` ci-dessus :

| Paquet | Raison | Licence connue |
| --- | --- | --- |
| `similar` | dépendance de **développement** de `scoretk-core` et `scoretk-midi-import` (tests) | Apache-2.0 |
| `arbitrary`, `derive_arbitrary` | fonctionnalité optionnelle de `zip`, non activée | MIT OR Apache-2.0 |
| `anstyle-wincon`, `windows-sys`, `windows-link` | uniquement pour la cible Windows (couleurs du terminal) | MIT OR Apache-2.0 |
| `once_cell_polyfill` | chaîne Windows de `clap`/`anstream` | MIT OR Apache-2.0 |
| `serde_core`, `serde_derive` | optionnel (via `zip`/`bstr`), non activé | MIT OR Apache-2.0 |
| `bstr` | optionnel, non activé | MIT OR Apache-2.0 |
| `crossbeam-utils` | optionnel, non activé | MIT OR Apache-2.0 |

Les builds **Windows** lieraient `anstyle-wincon`, `windows-sys` et
`windows-link`, tous sous MIT OR Apache-2.0 : compatibles eux aussi.

### Dépendances de développement : banc de performance (`criterion`)

`crates/scoretk` déclare en **dépendance de développement** `criterion` 0.8.2,
avec `default-features = false` et la seule fonctionnalité
`cargo_bench_support` (ni `plotters`, ni `html_reports`, ni `rayon`), pour
`crates/scoretk/benches/conversion.rs` (voir `docs/agents/performance.md`).
Ces crates ne sont compilées que par `cargo bench` et `cargo test` : elles
n'entrent pas dans le graphe `normal,build` ci-dessus — qui reste à
**46 crates** — et ne sont donc liées dans aucun binaire livré. Relevé le
2026-11-18 avec :

```console
$ cargo tree -p scoretk --edges normal,dev --prefix none --format '{p}|{l}'
```

| Crate | Version | Licence déclarée | Rôle |
| --- | --- | --- | --- |
| `alloca` | 0.4.0 | MIT | critère (mesure sans allocation pendant le chronométrage) |
| `anes` | 0.1.6 | MIT OR Apache-2.0 | critère (statistiques) |
| `cast` | 0.3.0 | MIT OR Apache-2.0 | critère (conversions numériques) |
| `ciborium` | 0.2.2 | Apache-2.0 | critère (stockage des résultats) |
| `ciborium-io` | 0.2.2 | Apache-2.0 | critère (via ciborium) |
| `ciborium-ll` | 0.2.2 | Apache-2.0 | critère (via ciborium) |
| `criterion` | 0.8.2 | Apache-2.0 OR MIT | banc de performance |
| `criterion-plot` | 0.8.2 | Apache-2.0 OR MIT | critère (dépendance dure, tracés désactivés) |
| `either` | 1.19.0 | MIT OR Apache-2.0 | critère (via itertools) |
| `half` | 2.7.1 | MIT OR Apache-2.0 | critère (via ciborium) |
| `itertools` | 0.13.0 | MIT OR Apache-2.0 | critère |
| `itoa` | 1.0.19 | MIT OR Apache-2.0 | critère (via serde_json) |
| `libc` | 0.2.191 | MIT OR Apache-2.0 | critère (via page_size) |
| `num-traits` | 0.2.19 | MIT OR Apache-2.0 | critère (via half) |
| `oorandom` | 11.1.5 | MIT | critère (échantillonnage aléatoire) |
| `page_size` | 0.6.0 | MIT/Apache-2.0 | critère (alloca) |
| `regex` | 1.13.2 | MIT OR Apache-2.0 | critère |
| `regex-automata` | 0.4.19 | MIT OR Apache-2.0 | critère (via regex) |
| `regex-syntax` | 0.8.12 | MIT OR Apache-2.0 | critère (via regex) |
| `same-file` | 1.0.6 | Unlicense/MIT | critère (via walkdir) |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | critère (résultats) |
| `serde_core` | 1.0.229 | MIT OR Apache-2.0 | critère (via serde) |
| `serde_derive` | 1.0.229 | MIT OR Apache-2.0 | critère (proc-macro, via serde) |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 | critère (résultats) |
| `syn` | 2.0.119 | MIT OR Apache-2.0 | proc-macro (via serde_derive) |
| `tinytemplate` | 1.2.1 | Apache-2.0 OR MIT | critère (rapports texte) |
| `walkdir` | 2.5.0 | Unlicense/MIT | critère |
| `zerocopy` | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | critère (via half) |
| `zerocopy-derive` | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | proc-macro (via zerocopy) |
| `zmij` | 1.0.23 | MIT | critère (via serde_json) |

Toutes ces licences sont déjà dans la liste blanche de
`scripts/check-licences.py` (MIT, Apache-2.0, BSD-2-Clause, Unlicense) et
compatibles avec GPL-3.0-only. `page_size` déclare `MIT/Apache-2.0` (barre
oblique, forme non SPDX) : les deux branches sont permises, et la crate reste
hors du graphe contrôlé puisqu'elle n'est tirée que par le banc.

Outils de construction hors Cargo (non liés dans les binaires livrés) :
`wasm-bindgen-cli` (MIT OR Apache-2.0), `binaryen` (Apache-2.0),
`nodejs` (MIT) et la chaîne Rust (MIT OR Apache-2.0).

## Contrôle automatisé

`scripts/check-licences.py` exécute la commande ci-dessus, évalue les
expressions SPDX (`AND` lie plus fort que `OR`, parenthèses gérées) et échoue
si une licence sort de la liste blanche ou manque. Le check de flake
l'exécute hors réseau sur les sources vendues :

```console
$ nix flake check            # inclut le check « licences »
$ nix build .#checks.<système>.licences
$ nix develop -c python3 scripts/check-licences.py   # en développement
```

Toute nouvelle dépendance doit être ajoutée ici **et**, si sa licence est
réellement nouvelle, dans `LICENCES_PERMISES` de `scripts/check-licences.py`
après vérification de compatibilité GPL-3.0-only.
