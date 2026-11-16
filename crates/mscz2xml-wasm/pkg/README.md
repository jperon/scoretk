# mscz2xml (WebAssembly)

Conversion hors ligne d'un fichier MuseScore (`.mscz` ou `.mscx`) en MusicXML,
dans le navigateur. Construit depuis le crate Rust
[`mscz2xml`](../../README.md) ; aucune donnée ne quitte la page.

Le paquet contient deux variantes du même binaire :

| Dossier    | Cible wasm-bindgen | Usage                                          |
| ---------- | ------------------ | ---------------------------------------------- |
| `esm/`     | `web`              | `import` depuis une page ou un module          |
| `classic/` | `no-modules`       | `importScripts()` dans un Web Worker classique |

## Variante ESM (page, module, bundler)

```js
import init, { convert, version } from 'mscz2xml';

await init(); // charge mscz2xml_wasm_bg.wasm à côté du module
console.log('mscz2xml', version());

const bytes = new Uint8Array(await file.arrayBuffer());
const xml = convert(bytes, { encodingDate: '2026-01-01' }); // -> string
```

`convert` lève une `Error` dont le message est celui de la bibliothèque Rust
quand l'entrée n'est pas un fichier MuseScore lisible.

## Variante classique (Web Worker, `importScripts`)

```js
// worker.js (script classique)
importScripts('mscz2xml/classic/mscz2xml.js');

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

## Licence et sources

<<<<<<< HEAD
Le paquet est distribué sous **GPL-3.0-only** (GNU General Public License
version 3 seulement), comme la bibliothèque dont il est issu. Le texte complet
accompagne ce paquet dans `LICENSE` ; l'origine du code et des données est
décrite dans `NOTICE`.

Le paquet contient du code porté de **MuseScore Studio**
(<https://github.com/musescore/MuseScore>), Copyright (C) MuseScore Limited
and others, réécrit en Rust en 2026, ainsi que du code généré par
wasm-bindgen. La **source correspondante** au sens de la GPL §6 est le dépôt
mscz2xml :

- URL : **TODO** — aucun remote git n'est encore défini dans ce dépôt ;
  renseigner l'URL dans `package.json` (champ `repository`) et ici.

Le programme est distribué **sans aucune garantie**. « MuseScore » est une
marque de MuseScore Limited ; ce paquet n'est ni affilié à, ni approuvé par
MuseScore Limited.
