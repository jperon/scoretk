# scoretk

Boîte à outils de conversion de partitions, sous licence GPL-3.0-only. Elle
vise les allers-retours entre MuseScore (`.mscz` / `.mscx`), MusicXML,
LilyPond, ABC et MIDI ; la feuille de route est dans
[`docs/TODO.md`](docs/TODO.md).

Le premier convertisseur, **mscz2xml**, est un portage en Rust de l'export
MusicXML de MuseScore (MuseScore → MusicXML).

## Objectifs de portabilité

La bibliothèque `mscz2xml` est conçue pour tourner partout où Rust compile :

- **wasm32-unknown-unknown** (navigateur, sans WASI) ;
- **Android** et **iOS** ;
- **Linux, macOS, Windows**.

Pour cela, elle respecte des contraintes strictes :

- aucune entrée/sortie fichier : l'API travaille sur des **octets en mémoire** ;
- aucun thread, aucune horloge, aucun accès réseau ;
- aucune dépendance C : le ZIP est décodé en Rust pur (`zip` avec
  `default-features = false`, décompression `deflate` via `miniz_oxide`) ;
- pas de `std::fs`, pas de `std::env` dans la bibliothèque.

Le crate `mscz2xml-cli` est la seule partie qui touche au système de
fichiers ; il n'est pas destiné au wasm.

## État actuel (étape 1)

- lecture d'un conteneur `.mscz` (ZIP) : `MsczContainer::from_bytes(&[u8])` ;
- résolution du fichier `.mscx` principal via `META-INF/container.xml`
  (`rootfile` / `full-path`), avec repli sur le premier `.mscx` à la racine ;
- acceptation d'un `.mscx` nu ;
- détection de la version du format : `detect_version(&[u8]) -> Result<MscVersion>`,
  par exemple `"3.02"` → `302`, `"4.60"` → `460`.

L'export MusicXML lui-même viendra dans les étapes suivantes.

## Utilisation

```console
$ mscz2xml partition.mscz
fichier principal : score.mscx
version mscx     : 4.60 (460)

$ mscz2xml partition.mscz -o partition.musicxml
fichier principal : score.mscx
version mscx     : 4.60 (460)
sortie demandée  : partition.musicxml (export MusicXML non implémenté à cette étape)
```

## Compilation et tests

```console
$ cargo build --workspace
$ cargo test --workspace
$ rustup target add wasm32-unknown-unknown
$ cargo build -p mscz2xml --target wasm32-unknown-unknown
$ cargo build -p scoretk-midi --target wasm32-unknown-unknown
```

## Structure

- `crates/mscz2xml` : bibliothèque portable (aucune I/O).
- `crates/mscz2xml-cli` : binaire `mscz2xml`, lecture du fichier et
  affichage des informations de conteneur.
- `crates/mscz2xml-wasm` : enveloppe WebAssembly (`convert`, `version`).
- `crates/scoretk-midi` : écriture MIDI (Standard MIDI File format 1) et
  déroulement des reprises, sans I/O.
- `crates/scoretk` et `crates/scoretk-cli` : façade multi-formats et binaire
  `scoretk` (convertisseurs optionnels par features).

## Licence

GPL-3.0-only. Le texte complet est dans [`LICENSE`](LICENSE).

Ce projet est un portage du code d'import/export de MuseScore Studio
(© MuseScore Limited and others), distribué sous GPL-3.0-only. Il s'agit
d'une œuvre dérivée : seule la version 3 de la GPL s'applique.
