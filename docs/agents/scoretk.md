# scoretk : façade de conversion

Crate `crates/scoretk` (bibliothèque) et `crates/scoretk-cli` (binaire
`scoretk`). Ce document décrit l'aiguillage, les points de branchement des
convertisseurs à venir et les tests. Le noyau du projet reste dans `AGENTS.md`.

## 1. Rôle

Le modèle de `crates/scoretk-core/src/model` est le **pivot commun** : chaque format
fournit un lecteur vers ce modèle et un écrivain depuis ce modèle, ce qui évite
N×N convertisseurs. `scoretk` choisit simplement le lecteur ou l'écrivain en
fonction du `Format`.

La bibliothèque reste portable : aucun `std::fs`/`env`/`net`/`process`, aucun
thread, aucune horloge ; elle compile pour `wasm32-unknown-unknown`. La CLI est
la seule à toucher au disque.

| Élément | Emplacement |
| --- | --- |
| Formats, détection | `crates/scoretk/src/format.rs` |
| Erreurs | `crates/scoretk/src/error.rs` |
| Options d'écriture | `crates/scoretk/src/options.rs` |
| Aiguillage `read`/`write` | `crates/scoretk/src/lib.rs` |
| Points d'accroche des features | `crates/scoretk/src/hooks.rs` |
| CLI | `crates/scoretk-cli/src/main.rs` |
| Tests | `crates/scoretk/tests/`, `crates/scoretk-cli/tests/cli.rs` |

## 2. API

```rust
pub enum Format { Mscz, Mscx, MusicXml, Mxl, Ly, Abc, Midi }
pub enum Sens { Lecture, Ecriture }

pub fn read(bytes: &[u8], format: Format) -> Result<Score>;
pub fn write(score: &Score, format: Format, options: &Options) -> Result<Vec<u8>>;

#[cfg(all(feature = "midi-export", feature = "musicxml-export"))]
pub fn write_playback(score: &Score, options: &Options) -> Result<PlaybackOutput>;
```

`Options` contient `software`, `encoding_date`, `concert_pitch` et `midi`.
`MidiOptions { expand_repeats: bool }` (défaut `true`) règle le déroulement
des reprises de la sortie MIDI ; la CLI l'expose par `--no-expand-repeats`.
L'export MusicXML ordinaire ne pose aucun identifiant (`stable_ids: false`) :
il reste identique aux références MuseScore 4.7.4.

`write_playback` (features `midi-export` et `musicxml-export`) renvoie
`PlaybackOutput { musicxml: Vec<u8>, midi: Vec<u8>, timemap: scoretk::Timemap }`.
Le MusicXML y est exporté avec `stable_ids` forcé : chaque `<measure>` et
chaque `<note>` porte `id` **et** `xml:id` (même valeur), exactement les
`xmlId` de la carte des temps. Le MIDI est identique à celui de `write(...,
Format::Midi, ...)` avec les mêmes options. La carte, ses formules
d'identifiants et son usage côté client sont décrits dans
`docs/agents/timemap.md`.

`Format` fournit `from_extension`, `from_path` (extension seule, pas d'accès
disque), `detect` (contenu), `supports_read`/`supports_write` (état réel,
features comprises) et `FromStr` (`mscz`, `mscx`, `musicxml`/`xml`, `mxl`,
`ly`/`lilypond`, `abc`, `mid`/`midi`).

### Détection par contenu

Ordre de `Format::detect` :

1. signature ZIP (`.mscz` s'il contient un `.mscx`, sinon `.mxl` s'il contient
   `META-INF/container.xml` ou un `.xml`/`.musicxml`) ;
2. `MThd` → MIDI ;
3. `<museScore` → `.mscx` ;
4. `<score-partwise`/`<score-timewise`, ou `<?xml` seul → MusicXML ;
5. `\version` en tête → LilyPond ;
6. `%abc` ou un en-tête `X:` → ABC.

Rien ne correspond → `None` (la CLI demande alors `--from`).

### Limites actuelles

Chaque sens a sa feature, et les dix sont actives par défaut ; la CLI `scoretk`
et le crate wasm les activent toutes. `--list-formats` montre l'état réel de la
bibliothèque compilée.

| Format | Lecture | Écriture |
| --- | --- | --- |
| `.mscz` / `.mscx` | feature `musescore-import` | feature `musescore-export` |
| MusicXML `.musicxml` / `.xml` | feature `musicxml-import` (t34) | feature `musicxml-export` |
| MusicXML `.mxl` | feature `musicxml-import` (t43) | feature `musicxml-export` |
| LilyPond `.ly` | feature `lilypond-import` (t40) | feature `lilypond-export` (t36) |
| ABC `.abc` | feature `abc-import` (t41) | feature `abc-export` (t41) |
| MIDI `.mid` | feature `midi-import` (t42) | feature `midi-export` (t35) |

Un format absent dans le sens demandé renvoie
`Error::FormatNonPrisEnCharge { format, sens }`, jamais une sortie vide.

## 3. Brancher les convertisseurs : dix features symétriques

La façade branche chaque convertisseur par une feature et une seule par sens :
`<format>-import` pour `read`, `<format>-export` pour `write`. Les dix sont
actives par défaut et il n'existe **aucun alias mixte** (pas de
`midi = ["midi-import", "midi-export"]`) : un besoin des deux sens liste les
deux features, comme le font la CLI et le crate wasm. Chaque feature branche
exactement un crate :

| Feature | Crate | Fonctions de `hooks.rs` |
| --- | --- | --- |
| `musescore-import` | `scoretk-musescore-import` | `read_musescore` |
| `musescore-export` | `scoretk-musescore-export` | `write_mscx`, `write_mscz` |
| `musicxml-import` | `scoretk-musicxml-import` | `read_musicxml`, `read_mxl` |
| `musicxml-export` | `scoretk-musicxml-export` | `write_musicxml`, `write_mxl` |
| `midi-import` | `scoretk-midi-import` | `read_midi` |
| `midi-export` | `scoretk-midi-export` | `write_midi`, `write_playback` |
| `lilypond-import` | `scoretk-lilypond-import` | `read_lilypond`, `read_lilypond_with_includes` |
| `lilypond-export` | `scoretk-lilypond-export` | `write_lilypond` |
| `abc-import` | `scoretk-abc-import` | `read_abc` |
| `abc-export` | `scoretk-abc-export` | `write_abc` |

Tout le code conditionnel est dans `hooks.rs` : chaque fonction a **deux
définitions exclusives**, la vraie sous `#[cfg(feature = "…")]` et un repli sous
`#[cfg(not(feature = "…"))]` engendré par la macro `non_branche!`, qui renvoie
`Error::FormatNonPrisEnCharge { format, sens }`. Aucun `cfg(not(any(...)))`
global : ajouter une feature revient à ajouter une paire de fonctions.

`error.rs` suit la même règle : les variantes qui transportent directement
l'erreur d'un crate (`MuseScoreImport`, `MusicXmlExport`) n'existent que si la
feature correspondante est active.

Pour ajouter un convertisseur :

1. déclarer le crate de sens en dépendance optionnelle et la feature
   `<format>-<sens> = ["dep:…"]` ;
2. écrire la fonction dans `hooks.rs` (vraie définition + `non_branche!`) ;
3. brancher le bras correspondant de `read`/`write` dans `lib.rs` et compléter
   `Format::{supports_read,supports_write}` ;
4. ajouter la feature à `default`, à la CLI et au crate wasm.

`write_playback` est le seul point qui demande deux features (`midi-export` pour
le MIDI et la carte des temps, `musicxml-export` pour le MusicXML identifié) :
il est absent si l'une des deux manque.

### MuseScore (`.mscz`, `.mscx`) — `musescore-import`, `musescore-export`

La lecture (t38) passe par `MsczContainer::from_bytes` puis `read::read_score`
(`scoretk-musescore-import`) ; l'écriture par `write_mscx`/`write_mscz`
(`scoretk-musescore-export`) : `.mscx` texte, `.mscz` archive ZIP avec
`META-INF/container.xml`, `score.mscx` et `score_style.mss`.

### MusicXML (`.musicxml`, `.xml`, `.mxl`) — `musicxml-import`, `musicxml-export`

La lecture (t34, t43) appelle `read_musicxml`/`read_mxl` de
`scoretk-musicxml-import` et convertit l'erreur en `Error::MusicXmlImport`.
L'export passe par `scoretk-musicxml-export`, et `.mxl` réutilise
`export_mxl_with` (archive `META-INF/container.xml` + `score.xml`, comme
`saveMxl` de MuseScore). Détail de l'extraction du conteneur et des replis :
`docs/agents/musicxml-import.md`.

### MIDI — `midi-import`, `midi-export`

`read_midi` (t42) vient de `scoretk-midi-import` (`Error::MidiImport`) ;
`write_midi` (t35) et `write_playback` de `scoretk-midi-export`
(`Error::Midi`). L'import et ses limites sont décrits dans
`docs/agents/midi.md`, la carte des temps dans `docs/agents/timemap.md`.

### LilyPond — `lilypond-import`, `lilypond-export`

`read_lilypond` (t40, `scoretk-lilypond-import`) décode l'UTF-8 puis lit un
sous-ensemble de LilyPond ; `read_lilypond_with_includes` suit en plus les
`\include` par un rappel fourni par l'appelant (sans entrée/sortie dans la
façade, la CLI lit les fichiers) ; `write_lilypond` (t36,
`scoretk-lilypond-export`) écrit en voix `\fixed c'`. Les deux crates partagent
`scoretk-lilypond-common` (erreur, `tpc`, clefs). Sous-ensemble, limites et
aller-retour mesuré : `docs/agents/lilypond.md`.

### ABC — `abc-import`, `abc-export`

`read_abc` et `write_abc` (t41) viennent de `scoretk-abc-import` et
`scoretk-abc-export` (conversion UTF-8 et erreurs en `Error::Abc`) ; les deux
partagent `scoretk-abc-common`. Sous-ensemble et aller-retour :
`docs/agents/abc.md`.

## 4. CLI

```sh
scoretk entrée.mscz -o sortie.musicxml      # formats déduits des extensions
scoretk entrée.mscx --to mxl -o -           # sortie standard
scoretk entrée.mscx -o copie.mscx           # réécriture MuseScore 4.70
scoretk entrée.mscx -o copie.mscz           # conteneur MuseScore
scoretk entrée.musicxml -o sortie.mscx      # lecture MusicXML (feature)
scoretk entrée.mxl -o sortie.mscx           # MusicXML compressé → MuseScore
scoretk entrée.musicxml -o sortie.ly        # MusicXML → LilyPond
scoretk entrée.musicxml -o sortie.mid       # MusicXML → MIDI
scoretk entrée.mscx -o sortie.mid           # MuseScore → MIDI
scoretk entrée.mid -o sortie.musicxml       # MIDI → MusicXML (import)
scoretk entrée.ly -o sortie.musicxml        # LilyPond (sous-ensemble) → MusicXML
scoretk entrée.mscx -o sortie.abc           # MuseScore → ABC
scoretk entrée.abc -o sortie.musicxml       # ABC → MusicXML
cat entrée.mscx | scoretk - --from mscx --to musicxml
scoretk --list-formats                      # formats et sens pris en charge
scoretk --version                           # version + licence (GPL-3.0-only)
```

- Formats déduits de l'extension ; si elle manque ou est inconnue, l'entrée est
  devinée par le contenu.
- `--from`/`--to` forcent le format ; ils sont obligatoires quand l'entrée ou la
  sortie est `-`.
- `-o` absent ou `-` → sortie standard ; entrée `-` → entrée standard.
- `--encoding-date AAAA-MM-JJ` fige `<encoding-date>` (défaut : date du jour en
  UTC, comme `scoretk-core`).
- `--no-expand-repeats` ne déroule pas les reprises dans la sortie MIDI
  (`Options::midi.expand_repeats = false`) ; la sortie est alors « à plat ».

## 5. Flake

- `packages.scoretk` construit la CLI (`cargoExtraArgs = "-p scoretk-cli"`).
- `apps.scoretk` la rend exécutable par `nix run .#scoretk -- …`.
- `packages.default` construit lui aussi la CLI `scoretk`.
- Le check `wasm32-unknown-unknown` compile les seize crates de bibliothèque
  (`-p scoretk-core`, `-p scoretk-musescore-import`, `-p scoretk-musescore-export`,
  `-p scoretk-musicxml-import`, `-p scoretk-musicxml-export`,
  `-p scoretk-midi-common`, `-p scoretk-midi-import`, `-p scoretk-midi-export`,
  `-p scoretk-lilypond-common`, `-p scoretk-lilypond-import`,
  `-p scoretk-lilypond-export`, `-p scoretk-abc-common`, `-p scoretk-abc-import`,
  `-p scoretk-abc-export`, `-p scoretk`, `-p scoretk-wasm`).
- `packages.wasm` est le paquet navigateur de la façade (crate
  `scoretk-wasm`) ; voir `docs/agents/wasm.md`.
- `checks.wasm-node-smoke` vérifie ce paquet sous Node.

## 6. Tests

- `crates/scoretk/tests/formats.rs` : extensions, chemins, `FromStr`, détection
  par contenu (XML, ZIP `.mscz`/`.mxl`, MIDI, LilyPond, ABC, inconnu).
- `crates/scoretk/tests/parite.rs` : `.mscz` → MusicXML **identique octet pour
  octet** à `scoretk::convert` sur le corpus (`dynamics`, `hairpin_simple`,
  `hairpins_and_repeats`), `.mxl` (container + `score.xml`), lecture MIDI vers
  MusicXML avec la feature, erreurs explicites des formats non branchés.
- `crates/scoretk/tests/mxl.rs` (feature `musicxml-import`) : `.mxl` lu comme
  le `.musicxml` équivalent, repli sans `container.xml`, ZIP invalide.
- `crates/scoretk-cli/tests/mxl.rs` : `.mxl` → `.mscx` et `--list-formats`
  avec la lecture `.mxl` active.
- `crates/scoretk/tests/abc.rs` (features `abc-import` et `abc-export`) :
  lecture ABC, écriture, relecture et nombre de notes stable ;
  `crates/scoretk-cli/tests/abc.rs` : `.mscx` → `.abc`, `.abc` → MusicXML,
  stdin/stdout, `--list-formats`.
- `crates/scoretk-cli/tests/lilypond.rs` : `.ly` en `\relative` → MusicXML et
  `--list-formats` avec la lecture LilyPond active.
- `crates/scoretk/tests/abc_*.rs` (aller-retour dans la façade, puisqu'il
  exerce les deux sens) : airs écrits à la main (`abc_roundtrip.rs`),
  instantanés du corpus (`abc_corpus.rs`), aller-retour noyau
  `EXPECTED_ROUNDTRIP` (`abc_aller_retour.rs`), outils externes
  (`abc_outils_externes.rs`) ; `crates/scoretk-abc-import/tests/lecture.rs`
  pour la lecture seule.
- `crates/scoretk/tests/mscx.rs` (feature `musescore-export`) : écriture `.mscx` relue
  avec un modèle identique, sortie MusicXML identique, `.mscz` détecté et relu,
  version `4.70`.
- `crates/scoretk/tests/lilypond_read.rs` (feature `lilypond-import`) :
  `read(..., Format::Ly)` renvoie un modèle exportable, une source sans
  `\score` est une erreur.
- Tests par sens : `crates/scoretk-musescore-import/tests/`,
  `crates/scoretk-musicxml-import/tests/`,
  `crates/scoretk-musicxml-export/tests/`,
  `crates/scoretk-midi-import/tests/`, `crates/scoretk-midi-export/tests/`
  (`corpus_midi`, `timemap_alignment`), `crates/scoretk-lilypond-import/tests/`,
  `crates/scoretk-lilypond-export/tests/`, `crates/scoretk-musescore-export/tests/`.
- `crates/scoretk-cli/tests/cli.rs` : binaire réel (`CARGO_BIN_EXE_scoretk`),
  `--list-formats` (état des features : MusicXML lecture, MIDI écriture),
  `--version`, stdin/stdout, `.mscz` → MusicXML comparé à `scoretk::convert`, sortie
  `.mxl`, `.mscx`/`.mscz` écrits et relus, `.mscx` → `.ly`, `.musicxml` →
  `.mscx`/`.ly`/`.mid`, `.mscx` → `.mid`, entrée `.mxl` refusée (pas de
  lecteur), erreurs de `--from`/`--to` manquants et de format non branché.
- Le corpus est lu via `CARGO_MANIFEST_DIR/../../tests/corpus` ; il doit rester
  dans le filtre de `flake.nix` (voir `docs/agents/developpement.md`).

## 7. Écarts connus

- `MsczContainer::from_bytes` accepte un `.mscx` nu : `read(..., Format::Mscx)`
  et `read(..., Format::Mscz)` suivent donc le même chemin.
- `detect` classe tout ZIP sans `.mscx` mais avec `META-INF/container.xml` (ou
  un `.xml`/`.musicxml`) comme `.mxl` ; un ZIP quelconque sans XML, sans
  conteneur ni `.mscx` n'est pas reconnu.
- Un XML inconnu est supposé MusicXML (`<?xml` seul) : c'est le seul format XML
  de scoretk aujourd'hui ; `--from` reste prioritaire.
- Aucun écart mesuré avec les crates de lecture et d'export : la façade
  appelle leurs fonctions, la comparaison du test est une égalité stricte.
