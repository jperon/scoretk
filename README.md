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
$ cargo build -p scoretk-midi-common -p scoretk-midi-import -p scoretk-midi-export \
    --target wasm32-unknown-unknown
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

- `crates/scoretk-core` : bibliothèque portable (aucune I/O) — modèle pivot.
- `crates/scoretk-musescore-import` / `crates/scoretk-musescore-export` :
  lecture `.mscz`/`.mscx` et écriture MuseScore (`.mscx`, `.mscz`).
- `crates/scoretk-musicxml-import` / `crates/scoretk-musicxml-export` :
  lecture et export MusicXML (`.musicxml`, `.xml`, `.mxl`).
- `crates/scoretk-midi-common` / `-import` / `-export` : lecture et écriture
  MIDI, déroulement des reprises, carte des temps.
- `crates/scoretk-lilypond-{common,import,export}` : lecture d'un
  sous-ensemble LilyPond et écriture `.ly`.
- `crates/scoretk-abc-{common,import,export}` : lecture et écriture ABC.
- `crates/scoretk` et `crates/scoretk-cli` : façade multi-formats et binaire
  `scoretk` (convertisseurs optionnels par features, une par sens).
- `crates/scoretk-wasm` : enveloppe WebAssembly de la façade (`convert`,
  `detect`, `formats`, `version`), unique paquet wasm.

## Licence

GPL-3.0-only. Le texte complet est dans [`LICENSE`](LICENSE).

Ce projet est un portage du code d'import/export de MuseScore Studio
(© MuseScore Limited and others), distribué sous GPL-3.0-only. Il s'agit
d'une œuvre dérivée : seule la version 3 de la GPL s'applique.
