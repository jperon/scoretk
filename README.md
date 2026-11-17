# scoretk

Boîte à outils de conversion de partitions, sous licence GPL-3.0-only. Elle
vise les allers-retours entre MuseScore (`.mscz` / `.mscx`), MusicXML,
LilyPond, ABC et MIDI ; la feuille de route est dans
[`docs/TODO.md`](docs/TODO.md).

Le convertisseur MuseScore → MusicXML, **scoretk-core**, est un portage en Rust
de l'export MusicXML de MuseScore Studio.

## Objectifs de portabilité

La bibliothèque `scoretk-core` est conçue pour tourner partout où Rust compile :

- **wasm32-unknown-unknown** (navigateur, sans WASI) ;
- **Android** et **iOS** ;
- **Linux, macOS, Windows**.

Pour cela, elle respecte des contraintes strictes :

- aucune entrée/sortie fichier : l'API travaille sur des **octets en mémoire** ;
- aucun thread, aucune horloge, aucun accès réseau ;
- aucune dépendance C : le ZIP est décodé en Rust pur (`zip` avec
  `default-features = false`, décompression `deflate` via `miniz_oxide`) ;
- pas de `std::fs`, pas de `std::env` dans la bibliothèque.

Le crate `scoretk-cli` est la seule partie qui touche au système de
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
$ scoretk partition.mscz -o partition.musicxml
$ scoretk partition.musicxml -o partition.mscz
```

## Compilation et tests

```console
$ cargo build --workspace
$ cargo test --workspace
$ rustup target add wasm32-unknown-unknown
$ cargo build -p scoretk-core --target wasm32-unknown-unknown
$ cargo build -p scoretk-midi --target wasm32-unknown-unknown
$ cargo build -p scoretk-wasm --target wasm32-unknown-unknown
```

## Paquet WebAssembly

`nix build .#wasm` produit le paquet navigateur de la façade (module ES et
script classique) dans `result/` : `package.json` `name: "scoretk"`,
`esm/scoretk_wasm.*`, `classic/scoretk.js`. Son API est documentée dans
[`docs/agents/wasm.md`](docs/agents/wasm.md) :

```js
import init, { convert } from 'scoretk';

await init();
const xml = new TextDecoder().decode(convert(bytes)); // musicxml par défaut
const midi = convert(bytes, { to: 'midi' });          // Uint8Array
```

`convert` renvoie un `Uint8Array` et expose `detect`, `formats` et
`convertWithTimeline` (voir [`docs/agents/wasm.md`](docs/agents/wasm.md)).

## Structure

- `crates/scoretk-core` : bibliothèque portable (aucune I/O) — modèle pivot,
  lecteurs MuseScore et export MusicXML.
- `crates/scoretk-wasm` : enveloppe WebAssembly de la façade (`convert`,
  `detect`, `formats`, `version`), unique paquet wasm.
- `crates/scoretk-midi` : lecture et écriture MIDI, déroulement des reprises,
  sans I/O.
- `crates/scoretk-musicxml`, `crates/scoretk-lilypond`, `crates/scoretk-abc`,
  `crates/scoretk-mscx` : lecteurs et écrivains des autres formats.
- `crates/scoretk` et `crates/scoretk-cli` : façade multi-formats et binaire
  `scoretk` (convertisseurs optionnels par features).

## Licence

GPL-3.0-only. Le texte complet est dans [`LICENSE`](LICENSE).

Ce projet est un portage du code d'import/export de MuseScore Studio
(© MuseScore Limited and others), distribué sous GPL-3.0-only. Il s'agit
d'une œuvre dérivée : seule la version 3 de la GPL s'applique.
