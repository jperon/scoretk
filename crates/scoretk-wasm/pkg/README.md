# scoretk (WebAssembly)

Façade multi-formats de **scoretk** : conversion hors ligne de partitions entre
MuseScore, MusicXML, LilyPond, ABC et MIDI, dans le navigateur. Construite
depuis la bibliothèque Rust [`scoretk`](../../README.md) ; aucune donnée ne
quitte la page.

Le paquet contient deux variantes du même binaire :

| Dossier    | Cible wasm-bindgen | Usage                                          |
| ---------- | ------------------ | ---------------------------------------------- |
| `esm/`     | `web`              | `import` depuis une page ou un module          |
| `classic/` | `no-modules`       | `importScripts()` dans un Web Worker classique |

## Formats

| Format | Extensions | Lecture | Écriture |
| --- | --- | --- | --- |
| MuseScore `.mscz` / `.mscx` | `mscz`, `mscx` | oui | oui |
| MusicXML | `musicxml`, `xml` | oui | oui |
| MusicXML compressé `.mxl` | `mxl` | oui | oui |
| LilyPond | `ly`, `lilypond` | oui | oui |
| ABC | `abc` | oui | oui |
| MIDI | `mid`, `midi` | oui | oui |

Le format d'entrée est détecté par le contenu (`detect`), ou forcé avec
l'option `from`. Le format de sortie vaut `musicxml` par défaut.

## Variante ESM (page, module, bundler)

```js
import init, { convert, convertWithTimeline, detect, formats, version } from 'scoretk';

await init(); // charge scoretk_wasm_bg.wasm à côté du module
console.log('scoretk', version(), formats());

const bytes = new Uint8Array(await file.arrayBuffer());
const midi = convert(bytes, { to: 'midi' }); // -> Uint8Array
const xml = new TextDecoder().decode(convert(bytes, { to: 'musicxml' }));
const { musicxml, timeline } = convertWithTimeline(bytes); // + carte des temps
```

`convert` renvoie toujours des octets (`Uint8Array`), quel que soit le format
de sortie : décoder en texte avec `TextDecoder` pour MusicXML, LilyPond ou ABC.
Il lève une `Error` dont le message est celui de la bibliothèque Rust quand la
conversion échoue.

## Variante classique (Web Worker, `importScripts`)

```js
// worker.js (script classique)
importScripts('scoretk/classic/scoretk.js');

await self.scoretk.ready;
self.onmessage = (event) => {
  const midi = self.scoretk.convert(new Uint8Array(event.data), { to: 'midi' });
  self.postMessage(midi);
};
```

`self.scoretk` expose `{ ready, convert, convertWithTimeline, version, detect,
formats, baseUrl }` ;
`ready` est une promesse résolue quand le `.wasm` est instancié (et rejetée en
cas d'échec de chargement). Le wrapper retrouve ses fichiers à partir de sa
propre URL ; si l'environnement ne le permet pas, définir
`self.SCORETK_BASE_URL` avant l'`importScripts`.

## API

| Fonction | Signature | Rôle |
| --- | --- | --- |
| `convert` | `(input: Uint8Array, options?: ConvertOptions) => Uint8Array` | conversion |
| `convertWithTimeline` | `(input: Uint8Array, options?: ConvertOptions) => PlaybackResult` | MusicXML identifié + MIDI + carte des temps (`to` ignoré) |
| `detect` | `(input: Uint8Array) => string \| null` | format détecté |
| `formats` | `() => FormatInfo[]` | formats et sens pris en charge |
| `version` | `() => string` | version de la bibliothèque |

## Options de `convert`

| Option         | Type      | Défaut              | Rôle |
| -------------- | --------- | ------------------- | ---- |
| `from`         | `string`  | détection           | format d'entrée (`mscz`, `mscx`, `musicxml`, `xml`, `mxl`, `ly`, `lilypond`, `abc`, `mid`, `midi`) |
| `to`           | `string`  | `"musicxml"`        | format de sortie |
| `encodingDate` | `string`  | constante Rust      | contenu de `<encoding-date>` (AAAA-MM-JJ) |
| `software`     | `string`  | constante Rust      | contenu de `<software>` (MusicXML) |
| `concertPitch` | `boolean` | constante Rust      | exporter les hauteurs en sons réels (MusicXML) |
| `expandRepeats`| `boolean` | `true`              | dérouler les reprises dans la sortie MIDI |
| `includes`     | `Record<string, Uint8Array>` | absent | fichiers d'un dossier indexés par chemin relatif, pour résoudre les `\include` LilyPond (chemin du fichier incluant, puis racine) |

`convertWithTimeline` renvoie `{ musicxml, midi, timeline }`. Le MusicXML porte
`id` **et** `xml:id` sur chaque mesure et chaque note, exactement les `xmlId`
de `timeline` ; la structure de `Timeline` est documentée dans
`docs/agents/timemap.md`.

## Licence et sources

Le paquet est distribué sous **GPL-3.0-only** (GNU General Public License
version 3 seulement), comme la bibliothèque dont il est issu. Le texte complet
accompagne ce paquet dans `LICENSE` ; l'origine du code et des données est
décrite dans `NOTICE`.

Le paquet contient du code porté de **MuseScore Studio**
(<https://github.com/musescore/MuseScore>), Copyright (C) MuseScore Limited
and others, réécrit en Rust en 2026, ainsi que du code généré par
wasm-bindgen. La **source correspondante** au sens de la GPL §6 est le dépôt
scoretk :

- URL : <https://github.com/jperon/scoretk> (champ `repository` du
  `package.json`).

Le programme est distribué **sans aucune garantie**. « MuseScore » est une
marque de MuseScore Limited ; ce paquet n'est ni affilié à, ni approuvé par
MuseScore Limited.
