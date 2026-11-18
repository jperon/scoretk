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

## Composition du binaire (twiggy)

Mesure réalisée sur le commit `a7f690c` (branche `mesure-twiggy`), profil
`[profile.release]` du workspace (`opt-level = "z"`, `lto = true`,
`codegen-units = 1`), avec `wasm-bindgen 0.2.130` et `wasm-opt -Oz`.

### Méthode de reproduction

```sh
nix develop -c cargo build --release -p scoretk-wasm --target wasm32-unknown-unknown
WB=/tmp/scratchpad/build-twiggy
mkdir -p "$WB/esm"
nix develop -c wasm-bindgen --target web --out-dir "$WB/esm" --out-name scoretk_wasm \
  target/wasm32-unknown-unknown/release/scoretk_wasm.wasm
# avec symboles (pour twiggy) :
nix develop -c wasm-opt -Oz --debuginfo "$WB/esm/scoretk_wasm_bg.wasm" \
  -o "$WB/esm/scoretk_wasm_bg.debug.wasm"
# sans symboles (cohérence avec la taille livrée) :
nix develop -c wasm-opt -Oz --strip-debug "$WB/esm/scoretk_wasm_bg.wasm" \
  -o "$WB/esm/scoretk_wasm_bg.strip.wasm"
nix shell nixpkgs#twiggy -c twiggy top -n 60 "$WB/esm/scoretk_wasm_bg.debug.wasm"
nix shell nixpkgs#twiggy -c twiggy dominators -d 3 "$WB/esm/scoretk_wasm_bg.debug.wasm"
nix shell nixpkgs#twiggy -c twiggy monos "$WB/esm/scoretk_wasm_bg.debug.wasm"
nix shell nixpkgs#twiggy -c twiggy garbage "$WB/esm/scoretk_wasm_bg.debug.wasm"
```

Mesures obtenues :

| Variante | Brut | Gzip |
| --- | --- | --- |
| avec symboles (`--debuginfo`) | 2 155 434 | — |
| sans symboles (`--strip-debug`) | 1 546 119 | 640 218 |
| référence documentée (§ 2) | 1 552 892 | 641 472 |

L'écart de 0,4 % entre la mesure locale et la référence documentée vient de
la toolchain (rustc 1.100.0 / wasm-bindgen 0.2.130 du shell Nix) ; les
ordres de grandeur sont cohérents. La section de noms (`"function names"`)
pèse 607 871 octets (28,20 %) dans la variante avec symboles ; elle est
absente du binaire livré.

### Agrégation par crate (shallow bytes, total 2 155 434)

Classification par préfixe de symbole (`crate[hash]::…`) ; les implémentations
de traits génériques sont attribuées au crate qui définit la fonction
générique (`core`, `alloc`). Les buckets `core::fmt` et `panics/messages`
sont isolés ensuite (voir (a)).

| Bucket | Octets | % |
| --- | ---: | ---: |
| `"function names"` (section de noms) | 607 871 | 28,20 |
| données statiques (`.rodata`) | 213 102 | 9,89 |
| `scoretk` (façade) | 196 636 | 9,12 |
| `core` (rust) | 194 787 | 9,04 |
| `alloc` | 181 775 | 8,43 |
| `scoretk_musicxml_import` | 102 234 | 4,74 |
| `scoretk_musicxml_export` | 96 798 | 4,49 |
| wasm-bindgen (shim + runtime) | 56 754 | 2,63 |
| `scoretk_midi_export` | 54 392 | 2,52 |
| `zip` | 47 092 | 2,18 |
| `scoretk_musescore_import` | 43 439 | 2,02 |
| `zopfli` | 42 464 | 1,97 |
| `scoretk_midi_import` | 42 450 | 1,97 |
| `scoretk_lilypond_import` | 40 825 | 1,89 |
| `scoretk_musescore_export` | 36 177 | 1,68 |
| `core::fmt` (isolé, voir (a)) | 32 686 | 1,52 |
| `scoretk_core` | 27 331 | 1,27 |
| `miniz_oxide` | 19 525 | 0,91 |
| `quick_xml` | 17 956 | 0,83 |
| `scoretk_abc_import` | 15 101 | 0,70 |
| `scoretk_lilypond_export` | 12 306 | 0,57 |
| `compiler_builtins` | 10 264 | 0,48 |
| `flate2` | 9 192 | 0,43 |
| `dlmalloc` | 7 427 | 0,34 |
| `memchr` | 5 760 | 0,27 |
| `scoretk_abc_common` | 4 943 | 0,23 |
| `indexmap` | 3 182 | 0,15 |
| `std` | 3 138 | 0,15 |
| `hashbrown` | 2 066 | 0,10 |
| `wasm_bindgen` | 1 979 | 0,09 |
| `scoretk_abc_export` | 1 926 | 0,09 |
| `midly` | 1 827 | 0,08 |
| `scoretk_wasm` | 1 630 | 0,08 |
| `panics/messages` (isolé, voir (a)) | 1 527 | 0,07 |
| autres dépendances + sections wasm | ~3 600 | 0,17 |
| **total** | **2 155 434** | **100,00** |

### (a) `core::fmt` et panics/messages

- `core::fmt` : **32 686 octets (1,52 %)** — implémentations `Debug`/`Display`
  générées pour les types du modèle et des erreurs (incluant les types
  `scoretk_core::*` via `<… as core::fmt::Display>::fmt`). C'est le poste
  de formatage, pas le moteur de rendu XML.
- panics/messages : **1 527 octets (0,07 %)** — `core::panicking` et messages
  associés. Marginal.

Total isolé : 34 213 octets (1,59 %). Les deux sont déjà marginalisés par
`panic = "abort"` (mesuré : −20 octets bruts, écarté).

### (b) Données statiques (`.rodata`)

Total `.rodata` : **213 102 octets (9,89 %)**, répartis en 115 segments.
Les plus gros :

| Segment | Octets | % |
| --- | ---: | ---: |
| `.rodata.18` | 53 135 | 2,47 |
| `.rodata.54` | 20 692 | 0,96 |
| `.rodata.1` | 20 059 | 0,93 |
| `.rodata.8` | 9 198 | 0,43 |
| `.rodata.13` | 7 803 | 0,36 |
| `.rodata.82` | 7 737 | 0,36 |

Le segment `.rodata.18` contient les chaînes de la table d'instruments
(`crates/scoretk-musescore-import/src/read/instruments.rs`, 4 934 lignes,
fichier généré) — on y retrouve `piccolo`, `wind.flutes.flute.piccolo`, etc.
Estimation : la table d'instruments pèse ≈ **53 Ko (2,47 %)** du binaire.
Le reste du `.rodata` est dominé par les messages d'erreur et les tables de
correspondance (clefs, armures, décoration ABC).

### (c) Les 20 plus grosses monomorphisations

`twiggy monos` : 297 instances, bloat cumulé ≈ 262 399 octets (12,2 %)
si toutes les instances redondantes étaient fusionnées. Top 20 instances :

```
 4285   0.20%  zopfli::deflate::add_lz77_block::<zip::write::MaybeEncrypted<Cursor<Vec<u8>>>> (×2)
 3382   0.16%  core::slice::sort::unstable::quicksort::<i32>
 3128   0.15%  core::slice::sort::unstable::quicksort::<usize>
 2708   0.13%  zopfli::squeeze::lz77_optimal::<ZopfliLongestMatchCache> (×2)
 2195   0.10%  core::slice::sort::stable::quicksort::<(i64, usize)> (midi)
 2036   0.09%  core::slice::sort::stable::quicksort::<Fraction> (midi tuplets)
 2031   0.09%  core::slice::sort::stable::quicksort::<(&str, &Element)> (musicxml)
 2025   0.09%  core::slice::sort::stable::quicksort::<Volta> (lilypond)
 1946   0.09%  core::slice::sort::stable::quicksort::<(TupletId, (usize, usize))>
 1940   0.09%  core::slice::sort::stable::quicksort::<Fraction> (PartialOrd)
 1878   0.09%  core::slice::sort::stable::quicksort::<VoltaSpan> (midi)
 1843   0.09%  core::slice::sort::stable::quicksort::<Event> (lilypond)
 1809   0.08%  core::slice::sort::stable::quicksort::<Half> (musescore)
 1739   0.08%  core::slice::sort::stable::quicksort::<Leaf> (zopfli)
 1523   0.07%  core::slice::sort::stable::drift::sort::<(i64, usize)>
 1500   0.07%  core::slice::sort::stable::drift::sort::<VoltaSpan>
 1461   0.07%  core::slice::sort::stable::drift::sort::<Volta>
 1371   0.06%  core::slice::sort::stable::drift::sort::<(&str, &Element)>
```

Leviers : `-Z share-generics` (nightly) dédupliquerait les instances identiques
entre crates ; gain réaliste estimé à 30–80 Ko bruts (borne haute 262 Ko).

### (d) Fonctions scoretk petites et non inlinées (candidates `#[inline]`)

Sélection : fonctions non génériques d'un crate `scoretk_*`, shallow bytes entre
8 et 96, triées par taille croissante (30 premières sur 91). La vérification
par `twiggy paths` sur un échantillon (ex. `scoretk_core::gap_rests::units_fraction`,
14 octets) confirme qu'elles sont bien appelées depuis un autre crate
(`scoretk_musicxml_import`), donc candidates à `#[inline]` aux frontières de
crates.

```
 13  scoretk_abc_common            duration::unit
 13  scoretk_lilypond_common       pitch::valid
 13  scoretk_musescore_import      read::read4xx::notations::accidental_type
 13  scoretk_musescore_import      read::read302::tables::accidental_type
 14  scoretk_core                  gap_rests::units_fraction
 14  scoretk_midi_import           chord::min_allowed_duration
 14  scoretk_midi_import           timesigmap::fraction_from_ticks
 14  scoretk_musescore_import      read::read302::credits::page_break
 15  scoretk_musicxml_export       beams::stems_hooks
 15  scoretk_musicxml_export       stems::clef_pitch_offset
 16  scoretk_midi_import           simplify::simplify_durations_not_drums
 19  scoretk_musescore_import      read::read4xx::textlines::rich_text
 19  scoretk_musicxml_export       exportmusicxml::borrow_attributes
 23  scoretk_musicxml_export       text::write_words
 24  scoretk_musescore_import      read::read302::spanner::read_chord_spanner
 24  scoretk_musicxml_export       repeats::sound
 25  scoretk_musescore_import      read::notation_names::line_style
 25  scoretk_musescore_import      read::read302::tables::stem_direction
 32  scoretk_midi_import           key::find_key::{closure#0}
 34  scoretk_musicxml_export       pitch::tpc_step_name
 34  scoretk_musicxml_export       harmony::is_separator
 37  scoretk_musescore_export      names::duration_type
 37  scoretk_musescore_export      names::clef_type
 37  scoretk_musescore_export      write::twrite::stem_name
 37  scoretk_musicxml_export       repeats::barline_style
 38  scoretk_musicxml_export       exportmusicxml::contains_accidental
 38  scoretk_musicxml_export       exportmusicxml::accidental_name
 39  scoretk_lilypond_export       annotations::text_event
 39  scoretk_midi_import           simplify::minimize_number_of_rests::{closure#1}
 40  scoretk_lilypond_import       builder::lyrics_build
```

Gain individuel faible (≤ 96 octets), mais ces fonctions restent des appels
inter-crates non inlinés par LTO fat aux frontières de crates : le levier
réel est le regroupement de crates (voir leviers), pas l'annotation seule.

### Dominators et garbage

`twiggy dominators -d 3` : la racine est l'export `convert` (et
`convertWithTimeline`), qui domine `scoretk::read` (194 175 octets, 9,01 %),
puis les lecteurs par format. Aucun cycle de domination anormal.

`twiggy garbage` : **282 octets** de types morts (types wasm inutilisés) ;
les 168 550 octets signalés dans les data segments sont des faux positifs
(segments référencés via des indices calculés à l'exécution).

### Leviers chiffrés (non appliqués)

| Levier | Gain brut estimé | Gain gzip estimé | Remarque |
| --- | ---: | ---: | --- |
| Regrouper `scoretk-musescore-import` + `scoretk-musicxml-export` dans `scoretk-core` (restaurer l'état pré-étape 1) | 58 427 | 15 146 | mesuré (§ « Effet du découpage ») |
| Rendre la table d'instruments (`instruments.rs`) optionnelle / chargée à la demande | ~53 000 | ~22 000 | estimé depuis `.rodata.18` |
| Remplacer `zopfli` par `flate2` deflate pour l'écriture `.mscz` | ~33 000 | ~13 500 | estimé (42 464 − flate2 déjà présent) |
| Limiter `core::fmt` (`Debug`/`Display`) sur les types internes | 32 686 | ~13 500 | mesuré |
| `-Z share-generics` (nightly) pour dédupliquer les monomorphisations | 30 000–80 000 | 12 000–33 000 | borne haute 262 399 ; nécessite nightly |
| Remplacer `dlmalloc` par `wee_alloc` | ~7 000 | ~3 000 | estimé |
| Éliminer les exports wasm-bindgen multivalue redondants | ~10 000 | ~4 000 | estimé (shim 49 489) |

Les gains en gzip sont estimés au ratio global 640 218 / 1 546 119 ≈ 0,41.
Aucun de ces leviers n'est appliqué : le paquet reste sous la limite de
Cantus (≈ 2,5 Mo brut) et les gains individuels restent sous le seuil de 2 %
du contrat wasm, sauf le regroupement de crates (3,8 %) et `-Z share-generics`.


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

## 9. Temps d'exécution (Node)

### Méthode

Arbre mesuré : commit `a7f690cf0817fd3f73a3657631e04a231cfc318a` (branche
`mesure-nodebench`), avant l'ajout du script de mesure et de cette section ;
aucun code de conversion (`crates/*/src`) n'a été modifié. Paquet construit par
`nix build .#wasm` (`./result`, binaire de 1 549 059 octets / 640 274 gzip,
`cat result/SIZE`), CLI native par
`nix develop -c cargo build --release -p scoretk-cli`
(`./target/release/scoretk`, profil `release` identique : `opt-level = "z"`,
`lto`, `codegen-units = 1`).

- Wasm : `nix shell nixpkgs#nodejs -c node crates/scoretk-wasm/tests/node-bench.mjs ./result`
  (et son variante `--json`, trois exécutions). Le script charge la variante ESM
  par `initSync` avec un `WebAssembly.Module` précompilé, parcourt les 56
  `.mscx` de `tests/corpus/v3` et `tests/corpus/v4`, et relève pour chacun,
  après 3 itérations de chauffe, la médiane de 15 itérations de
  `convert(bytes,{to:"musicxml"})`, `convert(bytes,{to:"midi"})` et
  `convertWithTimeline(bytes)`.
- Natif : le même ensemble de 56 fichiers, une invocation CLI par fichier et
  par sens (`scoretk "$f" --to musicxml|midi -o /dev/null`), 46 passages
  chronométrés par sens (`date +%s%N`) ; la CLI n'expose pas
  `convertWithTimeline`, donc le comparatif « playback » additionne ses deux
  sens. `time` sur un passage complet a donné 0,144 s (MusicXML) et 0,307 s
  (MIDI). Le coût d'amorçage des 56 processus a été mesuré à part : médiane de
  123 ms sur 15 passages de 56 `scoretk --list-formats` ; il est **inclus**
  dans les mesures natives.
- Machine : Intel(R) Processor U300, 6 cœurs / 6 threads, 4,4 GHz max, 15 GiB
  de RAM, Linux 6.18.60 (NixOS) x86_64, Node v24.22.1 (`lscpu`, `free -h`,
  `node --version`). Charge moyenne de 4,6 à 9,0 pendant les mesures (`uptime`)
  : une autre session exécutait un benchmark concurrent, d'où un bruit
  important. Les chiffres ci-dessous sont la médiane de trois exécutions wasm
  et de 46 passages natifs ; le minimum observé est aussi indiqué.

### Résultats

Instantiation (une seule fois, hors des mesures par fichier, `--json`) :
compilation du module 5,5 à 10,7 ms, `initSync`/`ready` 0,75 à 4,64 ms,
mémoire linéaire wasm 6 160 384 octets (5,9 Mio). `process.memoryUsage()` :
RSS 58,2 Mo avant chargement → 63,7 Mo après `initSync` → environ 180 Mo en fin
de campagne ; `arrayBuffers` 0,14 → 1,69 → 8,91 Mo.

Cinq plus gros fichiers du corpus (médianes en ms, somme des médianes par
fichier) :

| Fichier | Octets | MusicXML | MIDI | Timeline | Total |
| --- | ---: | ---: | ---: | ---: | ---: |
| `v3/moonlight.mscx` | 409 776 | 74,317 | 22,547 | 84,613 | 181,477 |
| `v4/voiceswitching.mscx` | 30 868 | 5,838 | 3,005 | 6,780 | 15,623 |
| `v4/testExcludeInvisibleElements.mscx` | 29 297 | 4,430 | 2,882 | 6,714 | 14,026 |
| `v3/layout_elements.mscx` | 24 855 | 3,501 | 1,569 | 4,933 | 10,003 |
| `v3/voices.mscx` | 24 571 | 2,895 | 1,367 | 4,383 | 8,645 |
| **Total (56 fichiers)** | | **145,568** | **64,740** | **204,010** | **414,318** |

### Ratio wasm/natif

| Chaîne | Wasm (ms) | Natif CLI (ms) | Ratio wasm/natif |
| --- | ---: | ---: | ---: |
| MusicXML | 145,6 | 277 | 0,53 |
| MIDI | 64,7 | 251 | 0,26 |
| Playback (MusicXML + MIDI) | 204,0 (`timeline`) | 528 | 0,39 |

Le natif est pénalisé par 56 démarrages de processus (123 ms mesurés). En les
retraitant : 154 ms / 128 ms / 282 ms côté natif, soit des ratios de 0,95 /
0,51 / 0,72. Sur les passages les moins chargés, le wasm descend à 128,7 /
55,6 / 183,1 ms contre 138 / 108 / 246 ms côté natif, soit 0,93 / 0,51 / 0,74.
L'ordre de grandeur est donc un wasm équivalent à légèrement plus rapide en
MusicXML, environ deux fois plus rapide en MIDI, et le bruit de la machine
empêche une précision meilleure que quelques dizaines de pourcents. Le ratio
« playback » reste approximatif : la carte des temps n'a pas d'équivalent dans
la CLI, seulement le coût MusicXML + MIDI.

### Fichiers les plus lents

`v3/moonlight.mscx` domine très largement (181,5 ms, environ 44 % du total
wasm, soit 74,3 ms MusicXML, 22,5 ms MIDI et 84,6 ms timeline) : avec 410 Ko,
il est quatorze fois plus gros que le suivant. Viennent ensuite
`v4/voiceswitching.mscx` (15,6 ms), `v4/testExcludeInvisibleElements.mscx`
(14,0 ms), `v4/timesig-09.mscx` (12,7 ms, seulement 9,7 Ko mais une timeline
coûteuse) et `v3/layout_elements.mscx` (10,0 ms).

Aucune piste d'optimisation n'a été appliquée : cette section ne consigne que
des mesures.

