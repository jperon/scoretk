# Paquet WebAssembly de la façade scoretk

Crate `crates/scoretk-wasm`, paquet Nix `packages.wasm`, check
`checks.wasm-node-smoke`. C'est l'unique paquet wasm du dépôt. Le noyau du
projet (règles, commandes, index) reste dans `AGENTS.md`.

## 1. Rôle

`crates/scoretk-wasm` est une **enveloppe wasm-bindgen** de la façade
`crates/scoretk` : aucune logique de conversion, seulement l'exposition de
`read`/`write`/`Format` à JavaScript. Elle active **toutes** les features de la
façade (`musicxml-import`, `midi`, `lilypond`, `lilypond-import`, `mscx`,
`abc`), comme la CLI `scoretk` : le paquet navigateur sait donc lire et écrire
les sept formats.

| Crate | Surface | Paquet Nix |
| --- | --- | --- |
| `crates/scoretk-wasm` | `convert`, `convertWithTimeline`, `detect`, `formats`, `version` | `packages.wasm` (`nix build .#wasm`) |

## 2. Construction et taille

```sh
nix build .#wasm
cat result/SIZE
ls -la result/esm result/classic
```

`result/` contient :

```
esm/scoretk_wasm.js           module ES (wasm-bindgen --target web)
esm/scoretk_wasm_bg.wasm      binaire optimisé (wasm-opt -Oz)
esm/scoretk_wasm.d.ts         types TypeScript (section custom : voir § 3)
classic/scoretk_wasm.js       script classique (--target no-modules)
classic/scoretk_wasm_bg.wasm  même binaire, optimisé séparément
classic/scoretk.js            wrapper : self.scoretk = { ready, convert, version, detect, formats, baseUrl }
classic/scoretk_wasm.d.ts     types
package.json                  name "scoretk", type "module", exports/types, repository
README.md                     mode d'emploi court du paquet
LICENSE                       texte de la GPL-3.0-only
NOTICE                        origine MuseScore Studio, données reprises, marque
SIZE                          mesure brute et gzip du .wasm
```

Mesure (wasm-bindgen 0.2.130, `opt-level = "z"`, `lto`, `codegen-units = 1`,
`wasm-opt -Oz --strip-debug`) :

| Binaire | Brut | Gzip |
| --- | --- | --- |
| `scoretk_wasm_bg.wasm` (avant `wasm-opt`) | 3 373 069 octets | — |
| `scoretk_wasm_bg.wasm` (après `wasm-opt`) | **1 552 892 octets** | **641 472 octets** |

Les deux variantes `esm` et `classic` partagent le même binaire à l'octet près.
Le paquet reste **complet, toutes features activées** : il reste sous la
limite de Cantus (≈ 2,5 Mo brut).

`panic = "abort"` a été **mesuré puis écarté** sur une version antérieure du
paquet : il ne retirait que 20 octets bruts (1 468 917 au lieu de 1 468 937,
soit 0,001 %) et 34 octets gzip, très en dessous du seuil de 2 %. Le profil
`[profile.release]` reste inchangé.

Les ajouts mesurés depuis la taille de référence de 1 472 792 octets bruts
portaient le binaire à 1 482 794 octets bruts / 621 633 octets gzip. Trois
contributions distinctes : +9 555 octets bruts (+0,65 %) pour
`convertWithTimeline` (`timemap_js.rs`, tri des notes), +414 octets bruts
(+0,03 %) pour la vélocité de note (`Note::user_velocity`) et +33 octets bruts
pour le calcul en `i64` de `customize_velocity` (anti-débordement), soit un
cumul de +10 002 octets bruts (+0,68 %). Le surcoût reste sous le seuil de 2 %.

### Effet du découpage en crates

La restructuration en 17 crates (un `-common`, un `-import` et un `-export`
par format bidirectionnel) se mesure au même pipeline (`nix build .#wasm`) :

| État | Brut | Gzip |
| --- | --- | --- |
| Avant restructuration (`4767464`) | 1 485 733 | 621 845 |
| Étape 1 : cœur scindé en `scoretk-core` + `-musescore-import` + `-musicxml-export` (`609efad`) | 1 544 160 | 636 991 |
| Après intégration (scission LilyPond, ABC, MIDI ; renommages) | **1 552 892** | **641 472** |

Soit +67 159 octets bruts (+4,52 %) et +19 627 octets gzip (+3,16 %), dont
+58 427 pour la seule scission du cœur et +8 732 pour l'intégration, pour un
refactor sans changement de logique. Diagnostic, chiffres à l'appui :

- **Pas de duplication nouvelle.** `cargo tree -d` est vide (aucun crate
  compilé deux fois) et le nombre de fonctions de même nom apparaissant
  plusieurs fois est identique avant/après (6, dont quatre `hashbrown`) ;
  `wasm-opt -Oz --duplicate-function-elimination` ne récupère que 110 octets.
- **Pas de double chemin.** `scoretk::convert` n'est plus atteint par le
  paquet (l'API wasm passe par `read`/`write`/`write_playback`) et son symbole
  a disparu du binaire : la voie d'orchestration historique ne coûte rien.
  `write_playback` réutilise le même `export_musicxml_with` et le même rendu
  MIDI que `write`.
- **LTO toujours actif.** À pipeline identique, `lto = false` donne
  1 535 725 octets (base) et 1 609 180 (après intégration), contre 1 482 791 et
  1 550 324 en `lto = true` : les 53 à 59 ko d'écart sont conservés, donc le
  découpage n'a pas cassé le LTO. `lto = "thin"` est à égalité avec `lto = true`
  (moins de 300 octets), donc rien à gagner de ce côté.
- **Ce qui grossit : le code, en nombre de fonctions.** Le code passe de
  3 311 à 3 543 fonctions (noms conservés, `wasm-opt -Oz --debuginfo`), pour
  +52 ko de code et +13 ko de données statiques (`.rodata`). Les fonctions
  moyennes restent de même taille (≈ 380 octets) : c'est le nombre de petites
  et moyennes fonctions qui augmente, c'est-à-dire des helpers qui étaient
  inlinés/fusionnés dans leur appelant et restent maintenant des fonctions à
  part entière, aux frontières de crates.

C'est donc l'effet normal du découpage sur l'inlining inter-crates, et non une
régression de code : la scission coûte ≈ 3,2 % de gzip par perte d'inlining
inter-crates (3 311 → 3 543 fonctions), sans duplication ni double chemin.
Cette croissance est **acceptée** (décision prise), le paquet restant sous la
limite de Cantus. Les leviers restent documentés à titre indicatif,
**non appliqués** (aucun ne dépasse grossièrement le seuil de 2 % du contrat
wasm) : garder `lto = "fat"` (déjà le cas), éviter les frontières de crates sur
les chemins chauds (lire/écrire une note, un accord, une mesure), regrouper les
deux crates les plus entrelacées (`scoretk-musescore-import` +
`scoretk-musicxml-export` étaient dans `scoretk-core` avant l'étape 1 : leur
retour dans un même crate rendrait les 58 ko de l'étape 1), ou activer
`-Z share-generics` (nightly), qui dédupliquerait les monomorphisations
génériques entre crates.

La construction est en deux étapes :

1. `wasm32Build` : `craneLib.cargoBuild` avec
   `CARGO_BUILD_TARGET = "wasm32-unknown-unknown"`, sur les artefacts de
   dépendances déjà compilés (`wasmCargoArtifacts`) ; seul le `.wasm` du cdylib
   `scoretk_wasm` est copié dans la sortie.
2. `wasm` : `runCommand` qui appelle `wasm-bindgen` deux fois (cibles `web` et
   `no-modules`), passe chaque `.wasm` à `wasm-opt`, installe le wrapper
   `classic/scoretk.js`, un `package.json` généré depuis `version`, et écrit
   `SIZE`.

Aucune dérivation n'accède au réseau : crane vend les crates à partir de
`Cargo.lock`, et `--locked` interdit toute modification du verrou. Ajouter un
membre au workspace invalide `buildDepsOnly` pour wasm32 : la première
construction après une telle modification reconstruit les dépendances, puis
le cache Nix reprend.

**Version de wasm-bindgen** : la crate est épinglée `=0.2.130`, égale au
`wasm-bindgen-cli` du shell Nix. En cas de mise à jour du flake, changer
l'épingle dans `crates/scoretk-wasm/Cargo.toml`, sinon le CLI refuse le
`.wasm` (« schema version mismatch »).

## 3. API

Le `.d.ts` produit contient la section TypeScript custom : toutes les
fonctions sont marquées `skip_typescript` côté Rust, pour que `detect` sorte
en `string | null`, `formats` en `FormatInfo[]` et `convertWithTimeline` en
`PlaybackResult`.

| Fonction | Signature | Notes |
| --- | --- | --- |
| `convert` | `(input: Uint8Array, options?: ConvertOptions) => Uint8Array` | octets dans les deux sens, tous formats |
| `convertWithTimeline` | `(input: Uint8Array, options?: ConvertOptions) => PlaybackResult` | MusicXML identifié + MIDI + carte des temps ; `to` ignoré |
| `detect` | `(input: Uint8Array) => string \| null` | `Format::detect` (contenu) |
| `formats` | `() => FormatInfo[]` | sept entrées, capacités réelles |
| `version` | `() => string` | version de la bibliothèque |

```ts
interface ConvertOptions {
  from?: string;        // mscz, mscx, musicxml, xml, mxl, ly, lilypond, abc, mid, midi
  to?: string;          // défaut : "musicxml"
  encodingDate?: string; // <encoding-date> (AAAA-MM-JJ)
  software?: string;    // <software> (MusicXML)
  concertPitch?: boolean; // hauteurs en sons réels (MusicXML)
  expandRepeats?: boolean; // dérouler les reprises MIDI (défaut : true)
}

interface FormatInfo {
  name: string;
  extensions: string[];
  read: boolean;
  write: boolean;
}

interface TimelineTempo { tick: number; timeUs: number; microsPerQuarter: number }
interface TimelineMeasure { measure: number; ordinal: number; segment: number; passage: number; xmlId: string; tick: number; endTick: number; timeUs: number; endTimeUs: number; timeMs: number }
interface TimelineNote { tick: number; timeUs: number; durationTicks: number; measure: number; staff: number; voice: number; chord: number; note: number; pitch: number; passage: number; grace: boolean; tremolo: boolean; xmlId: string }
interface TimelinePause { tick: number; endTick: number; timeUs: number; endTimeUs: number; seconds: number }
interface Timeline { division: number; ticks: number; tempo: TimelineTempo[]; measures: TimelineMeasure[]; notes: TimelineNote[]; pauses: TimelinePause[] }
interface PlaybackResult { musicxml: Uint8Array; midi: Uint8Array; timeline: Timeline }
```

`convertWithTimeline` résout `from` comme `convert` (détection par le contenu
si absent) et ignore `to`. Le MusicXML renvoyé est exporté avec les
identifiants stables : chaque `<measure>` et chaque `<note>` porte `id` **et**
`xml:id` (même valeur), qui sont exactement les `xmlId` de `timeline`.
`timeline.measures[].ordinal` est l'indice 0-basé de la mesure dans la
partition complète (égal à `measure`) : c'est la seule clé d'ancrage à
utiliser, `number` n'étant pas exposé. `timeline.notes` est trié par `tick`
croissant (puis par clé stable), côté Rust. Le MIDI et la carte viennent du
même rendu ; `expandRepeats: false` produit un MIDI
« à plat » et moins de mesures dans `timeline.measures`.

Le type Rust reste `Timemap` (crate `scoretk-midi-export`, réexporté par `scoretk`) :
la correspondance champ à champ Rust `snake_case` → JS `camelCase` est
documentée dans `docs/agents/timemap.md` § 6. `MeasureSync::number`,
`pause_before`, `grace_index` et `grace_before` ne sont pas exposés côté JS.

`from` absent est détecté par le contenu (`Format::detect`) ; une entrée non
reconnue lève une erreur explicite plutôt que de deviner. `to` absent vaut
`musicxml` : `mscz → musicxml` reste l'appel à zéro option.

## 4. Erreurs

Toutes les erreurs deviennent une `Error` JavaScript dont le message est celui
de la bibliothèque Rust :

- format d'entrée non reconnu d'après le contenu (quand `from` est absent) ;
- `from` ou `to` inconnu (`Error::FormatInconnu`, message listant les noms
  acceptés) ;
- format reconnu mais non pris en charge dans le sens demandé
  (`Error::FormatNonPrisEnCharge`) — impossible dans ce paquet, qui active
  toutes les features, mais le garde-fou reste ;
- erreurs de conversion : `Error::Core`, `MusicXmlImport`, `Midi`,
  `MidiImport`, `LilyPond`, `Mscx`, `Abc`, `Zip`.

Aucun `console_error_panic_hook` n'est installé : une panique Rust apparaît
comme un `RuntimeError` (« unreachable »), ce qui ne doit arriver que sur un
bug interne.

## 5. Usage ESM et classic

```js
import init, { convert, detect, formats, version } from 'scoretk';

await init();
const midi = convert(bytes, { to: 'midi' });            // Uint8Array
const xml = new TextDecoder().decode(convert(bytes));   // musicxml par défaut
```

```js
// Worker classique
importScripts('scoretk/classic/scoretk.js');
await self.scoretk.ready;
const midi = self.scoretk.convert(bytes, { to: 'midi' });
```

`self.scoretk` vaut `{ ready, convert, version, detect, formats, baseUrl }` ;
en cas de base URL incorrecte, définir `self.SCORETK_BASE_URL` avant
l'`importScripts`. Le wrapper charge `scoretk_wasm.js` et
`scoretk_wasm_bg.wasm`.

Le `.wasm` doit être servi en `application/wasm` pour que le glue utilise
`WebAssembly.instantiateStreaming` (sinon il retombe sur un `arrayBuffer`,
correct mais plus lent).

## 6. Tests

`crates/scoretk-wasm/tests/node-smoke.mjs` charge la variante ESM du paquet
tel que `nix build` le produit et couvre les cas suivants (numérotés 1 à 21
dans le script) :

- `version()` égale `package.json` ;
- `formats()` : sept entrées, `read`/`write` à `true`, extensions MusicXML ;
- `detect` : `.mscx`, `.musicxml`, `.abc`, `.ly`, contenu inconnu (`null`) ;
- `convert` : défaut MusicXML avec `encodingDate`/`software`, `.mscx` → `.mscx`,
  `.mscx` → `.mscz` relu puis → MIDI, MusicXML → LilyPond/ABC/MIDI,
  ABC → MusicXML, LilyPond → MusicXML ;
- erreurs : `to: 'pdf'`, `from: 'pdf'`, entrée inconnue sans `from` ;
- `convertWithTimeline` sur `v4/repeat65.mscx` : présence de `musicxml`,
  `midi` et `timeline`, attributs `id` **et** `xml:id` dans le MusicXML, MIDI
  `MThd`, mesures non vides avec premier tick/temps à zéro, `ordinal` égal à
  `measure`, ticks strictement croissants, au moins une mesure rejouée,
  `expandRepeats: false` plus court, et chaque `xmlId` de note présent dans le
  MusicXML.

Variables d'environnement : `SCORETK_PKG_DIR` (défaut `result/`) et
`SCORETK_CORPUS` (défaut `tests/corpus/`). Le check de flake
`checks.wasm-node-smoke` exécute ce script sur le paquet Nix.

```sh
nix build .#wasm
node crates/scoretk-wasm/tests/node-smoke.mjs
SCORETK_PKG_DIR=$PWD/result node crates/scoretk-wasm/tests/node-smoke.mjs
```

## 7. Licence et sources

Le paquet est distribué sous **GPL-3.0-only**, comme la bibliothèque. La
source correspondante au sens de la GPL §6 est le dépôt
<https://github.com/jperon/scoretk> (champ `repository` du `package.json`).
Le paquet embarque du code porté de MuseScore Studio, Copyright (C) MuseScore
Limited and others, réécrit en Rust en 2026, et du code généré par
wasm-bindgen. « MuseScore » est une marque de MuseScore Limited ; le paquet
n'y est ni affilié ni approuvé.

## 8. Exemples

`examples/web/` contient deux pages minimales (fichier choisi ou déposé,
MusicXML affiché en `<pre>`, téléchargement, pas de dépendance réseau) :

```sh
nix build .#wasm
python3 -m http.server 8000        # à la racine du dépôt
# http://localhost:8000/examples/web/index.html        (variante esm)
# http://localhost:8000/examples/web/worker-demo.html  (variante classic en Worker)
```

Le worker de la démo charge le paquet par
`importScripts('../../result/classic/scoretk.js')`.

