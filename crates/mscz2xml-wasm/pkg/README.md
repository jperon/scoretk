# scoretk (WebAssembly)

Premier convertisseur de **scoretk** : conversion hors ligne d'un fichier
MuseScore (`.mscz` ou `.mscx`) en MusicXML, dans le navigateur. Construit
depuis le crate Rust [`mscz2xml`](../../README.md) ; aucune donnée ne quitte
la page.

Le paquet contient deux variantes du même binaire :

| Dossier    | Cible wasm-bindgen | Usage                                          |
| ---------- | ------------------ | ---------------------------------------------- |
| `esm/`     | `web`              | `import` depuis une page ou un module          |
| `classic/` | `no-modules`       | `importScripts()` dans un Web Worker classique |

## Variante ESM (page, module, bundler)

```js
import init, { convert, version } from 'scoretk';

await init(); // charge mscz2xml_wasm_bg.wasm à côté du module
console.log('scoretk', version());

const bytes = new Uint8Array(await file.arrayBuffer());
const xml = convert(bytes, { encodingDate: '2026-01-01' }); // -> string
```

`convert` lève une `Error` dont le message est celui de la bibliothèque Rust
quand l'entrée n'est pas un fichier MuseScore lisible.

## Variante classique (Web Worker, `importScripts`)

```js
// worker.js (script classique)
importScripts('scoretk/classic/mscz2xml.js');

await self.mscz2xml.ready;
self.onmessage = (event) => {
  const xml = self.mscz2xml.convert(new Uint8Array(event.data));
  self.postMessage(xml);
};
```

`self.mscz2xml` expose `{ ready, convert, version }` ; `ready` est une promesse
résolue quand le `.wasm` est instancié (et rejetée en cas d'échec de
chargement). Le wrapper retrouve ses fichiers à partir de sa propre URL ; si
l'environnement ne le permet pas, définir `self.MSCZ2XML_BASE_URL` avant
l'`importScripts`.

## Options

| Option         | Type     | Défaut          | Rôle                                       |
| -------------- | -------- | --------------- | ------------------------------------------ |
| `encodingDate` | `string` | constante Rust  | contenu de `<encoding-date>` (AAAA-MM-JJ)  |

## Licence

GPL-3.0-only, comme la bibliothèque.
