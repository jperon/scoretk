# Paquet WebAssembly (navigateur)

Le crate `crates/mscz2xml-wasm` enveloppe la bibliothèque avec wasm-bindgen et
produit un paquet prêt à servir à une page ou à un Web Worker. Il ne contient
aucune logique de conversion : l'API publique de `crates/mscz2xml` reste la
seule source de vérité.

## Construction

```sh
nix build .#wasm
ls -la result/esm result/classic
```

`result/` contient :

```
esm/mscz2xml_wasm.js          module ES (wasm-bindgen --target web)
esm/mscz2xml_wasm_bg.wasm     binaire optimisé (wasm-opt -Oz)
esm/mscz2xml_wasm.d.ts        types TypeScript
classic/mscz2xml_wasm.js      script classique (--target no-modules)
classic/mscz2xml_wasm_bg.wasm même binaire, optimisé séparément
classic/mscz2xml.js           wrapper : self.mscz2xml = { ready, convert, version }
classic/mscz2xml_wasm.d.ts    types
package.json                  name "mscz2xml", type "module", exports/types, repository
README.md                     mode d'emploi court du paquet
LICENSE                       texte de la GPL-3.0-only
NOTICE                        origine MuseScore Studio, données reprises, marque
```

Taille obtenue (wasm-bindgen 0.2.130, `opt-level = "z"`, `lto`, `wasm-opt -Oz`) :
**669 279 octets** pour `mscz2xml_wasm_bg.wasm` (≈ 653 Kio, 300 055 octets
gzip, ≈ 293 Kio), glue JS ≈ 10 Kio. Les deux variantes partagent le même
binaire à l'octet près.

La dérivation est en deux étapes, dans `flake.nix` :

1. `wasm32Build` : `craneLib.cargoBuild` avec
   `CARGO_BUILD_TARGET = "wasm32-unknown-unknown"`, sur les artefacts de
   dépendances déjà compilés (`wasmCargoArtifacts`). Le `.wasm` du cdylib est
   copié seul dans la sortie.
2. `wasm` : `runCommand` qui appelle `wasm-bindgen` deux fois (cibles `web` et
   `no-modules`), passe chaque `.wasm` à `wasm-opt`, installe le wrapper
   `classic/mscz2xml.js` et un `package.json` généré depuis `version`.

Aucune dérivation n'accède au réseau : crane vend les crates à partir de
`Cargo.lock` (dérivations à sortie fixe), et `--locked` interdit toute
modification du verrou.

**Version de wasm-bindgen** : la crate `wasm-bindgen` est épinglée
(`=0.2.130`) sur la version de `wasm-bindgen-cli` fournie par le shell nix.
Vérifier avec `wasm-bindgen --version` ; en cas de mise à jour du flake, changer
l'épingle dans `crates/mscz2xml-wasm/Cargo.toml`, sinon le CLI refuse le
`.wasm` (« schema version mismatch »).

## API

Les deux variantes exposent la même surface :

| Fonction | Signature | Notes |
| --- | --- | --- |
| `convert` | `(bytes: Uint8Array, options?: { encodingDate?: string }) => string` | MusicXML partwise, texte UTF-8 |
| `version` | `() => string` | version de la bibliothèque |

`encodingDate` alimente `<encoding-date>` (format AAAA-MM-JJ). Absent, la
valeur par défaut de la bibliothèque est utilisée : la conversion reste
reproductible.

Les erreurs de conversion (entrée qui n'est ni `.mscz` ni `.mscx`, ZIP
corrompu, XML illisible) deviennent une `Error` JavaScript dont le message est
celui de l'erreur Rust. Aucun `console_error_panic_hook` n'est installé : une
panique Rust apparaît comme un `RuntimeError` (« unreachable »), ce qui ne doit
arriver que sur un bug interne.

## Usage ESM (page, module, bundler)

```js
import init, { convert, version } from 'mscz2xml';

await init(); // charge mscz2xml_wasm_bg.wasm à côté du module
const bytes = new Uint8Array(await file.arrayBuffer());
const xml = convert(bytes, { encodingDate: '2026-01-01' });
```

Le `package.json` du paquet déclare `"type": "module"`, `exports` (racine →
`esm/`) et `types`. Pour un bundler, `import init from 'mscz2xml'` suffit ; le
`.wasm` doit être servi par le serveur (ou traité comme asset par le bundler).

## Usage Web Worker classique (PWA cantus)

Cantus charge ses scripts en classiques et convertit dans un worker via
`importScripts`. Servir le dossier `result/` sous `mscz2xml/` (copie dans
`dist/` ou alias), puis :

```js
// worker.js (script classique)
importScripts('mscz2xml/classic/mscz2xml.js');

await self.mscz2xml.ready; // wasm instancié

self.onmessage = async (event) => {
  const bytes = new Uint8Array(event.data);
  const xml = self.mscz2xml.convert(bytes, { encodingDate: '2026-01-01' });
  self.postMessage(xml);
};
```

`self.mscz2xml` vaut `{ ready, convert, version, baseUrl }` ; `convert` et
`version` sont synchrones une fois `ready` résolue et lèvent une `Error` claire
si on les appelle trop tôt. Le wrapper retrouve l'URL de ses fichiers (le
`.js` et le `.wasm` du même dossier) à partir de sa propre URL : en Worker il
n'y a pas de `document.currentScript`, donc il lit la pile d'appels
(`new Error().stack`), puis les entrées de `performance`, et se rabat sur
l'emplacement du script worker. Si aucun de ces moyens ne donne le bon chemin
(par exemple worker servi depuis un blob), définir avant l'import :

```js
self.MSCZ2XML_BASE_URL = 'https://exemple/mscz2xml/classic/';
importScripts(self.MSCZ2XML_BASE_URL + 'mscz2xml.js');
```

Le `.wasm` doit être servi en `application/wasm` pour que le glue utilise
`WebAssembly.instantiateStreaming` (sinon il retombe sur un `arrayBuffer`,
correct mais plus lent).

## Tests

```sh
nix build .#wasm
node crates/mscz2xml-wasm/tests/node-smoke.mjs   # dans nix develop
```

Le test charge la variante ESM depuis `result/` avec
`initSync({ module: new WebAssembly.Module(...) })` (pas de `fetch`), convertit
`tests/corpus/v4/dynamics.mscx`, vérifie le préfixe `<?xml`, la présence de
`<score-partwise>` et de `encodingDate`, puis qu'une entrée invalide lève une
`Error`. `MSCZ2XML_PKG_DIR` et `MSCZ2XML_CORPUS` permettent de pointer le paquet
et le corpus ailleurs : c'est ce qu'utilise le check `nodeSmoke`
(`nix flake check`), qui construit `packages.wasm` avant de lancer le test.

## Exemples

`examples/web/` contient deux pages minimales (fichier choisi ou déposé,
MusicXML affiché en `<pre>`, téléchargement, pas de dépendance réseau) :

```sh
nix build .#wasm
python3 -m http.server 8000        # à la racine du dépôt
# http://localhost:8000/examples/web/index.html        (variante esm)
# http://localhost:8000/examples/web/worker-demo.html  (variante classic en Worker)
```

Le worker de la démo charge le paquet par
`importScripts('../../result/classic/mscz2xml.js')` : même chemin de code que
cantus, wrapper compris.
