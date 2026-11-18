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

#[cfg(feature = "midi")]
pub fn write_playback(score: &Score, options: &Options) -> Result<PlaybackOutput>;
```

`Options` contient `software`, `encoding_date`, `concert_pitch` et `midi`.
`MidiOptions { expand_repeats: bool }` (défaut `true`) règle le déroulement
des reprises de la sortie MIDI ; la CLI l'expose par `--no-expand-repeats`.
L'export MusicXML ordinaire ne pose aucun identifiant (`stable_ids: false`) :
il reste identique aux références MuseScore 4.7.4.

`write_playback` (feature `midi`) renvoie
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

État de la **bibliothèque** selon les features ; la CLI `scoretk` active les
six (`musicxml-import`, `midi`, `lilypond`, `lilypond-import`, `mscx`, `abc`),
et c'est cet état que montre son `--list-formats`.

| Format | Lecture | Écriture |
| --- | --- | --- |
| `.mscz` / `.mscx` | oui (`scoretk-musescore-import`) | oui (feature `mscx`, `scoretk-mscx`) |
| MusicXML `.musicxml` / `.xml` | feature `musicxml-import` (t34) | oui |
| MusicXML `.mxl` | feature `musicxml-import` (t43) | oui |
| LilyPond `.ly` | oui (feature `lilypond-import`, t40) | oui (feature `lilypond`, `scoretk-lilypond`) |
| ABC `.abc` | oui (feature `abc`, `scoretk-abc`, t41) | oui (feature `abc`, `scoretk-abc`, t41) |
| MIDI `.mid` | oui (feature `midi`, `scoretk-midi`, t42) | oui (feature `midi`, `scoretk-midi`) |

Un format absent dans le sens demandé renvoie
`Error::FormatNonPrisEnCharge { format, sens }`, jamais une sortie vide.
L'écriture `.mxl` réutilise `scoretk_musicxml_export::export_mxl_with` (archive ZIP
`META-INF/container.xml` + `score.xml`, comme `saveMxl` de MuseScore).

## 3. Brancher les convertisseurs (t34, t35, t36, t38)

Tout le code conditionnel est dans `crates/scoretk/src/hooks.rs`. Les features
existent déjà, désactivées par défaut ; les variantes d'erreur
`MusicXmlImport`, `Midi`, `LilyPond` et `Mscx` aussi.

### t34 — lecture MusicXML (`crates/scoretk-musicxml`)

1. Dans `crates/scoretk/Cargo.toml`, ajouter la dépendance optionnelle
   `scoretk-musicxml = { path = "../scoretk-musicxml", optional = true }` et
   remplacer `musicxml-import = []` par
   `musicxml-import = ["dep:scoretk-musicxml"]`.
2. Rien d'autre : la fonction `hooks::read_musicxml` appelle déjà
   `scoretk_musicxml::read_musicxml(bytes)` et convertit l'erreur en
   `Error::MusicXmlImport` (l'erreur de la crate doit implémenter `Display`).

**Branché** : la CLI active la feature ; `scoretk entrée.musicxml -o sortie.mscx`
(ou `.ly`, `.mid`) fonctionne, et le `.mxl` en entrée aussi depuis t43.

### t43 — lecture MusicXML compressé (`.mxl`)

**Branché** : `hooks::read_mxl` appelle `scoretk_musicxml::read_mxl(bytes)` et
convertit l'erreur en `Error::MusicXmlImport`. `Format::Mxl` répond oui à
`supports_read` quand `musicxml-import` est active, et
`scoretk read(bytes, Format::Mxl)` comme `scoretk entrée.mxl -o sortie.mscx`
fonctionnent. L'extraction du conteneur, le repli sans `container.xml` et les
erreurs sont décrits dans `docs/agents/musicxml-import.md`.

### t38 — écriture MuseScore (`crates/scoretk-mscx`)

**Branché** : dépendance optionnelle `scoretk-mscx`, feature
`mscx = ["dep:scoretk-mscx"]`, variante d'erreur `Mscx`, et fonctions
`hooks::write_mscx`/`write_mscz` (`.mscx` texte, `.mscz` archive ZIP avec
`META-INF/container.xml`, `score.mscx` et `score_style.mss`). La CLI active la
feature : `scoretk entrée.mscx -o sortie.mscx` et `… -o sortie.mscz`.

### t35 — écriture MIDI (`crates/scoretk-midi`)

**Branché** : la dépendance optionnelle et `midi = ["dep:scoretk-midi"]` sont
en place ; `hooks::write_midi` appelle `scoretk_midi::write_midi(score)` et
convertit l'erreur en `Error::Midi`. La CLI active la feature : `scoretk
entrée.mscx -o sortie.mid` et `scoretk entrée.musicxml -o sortie.mid`
fonctionnent. Pour un test de la bibliothèque seule, activer explicitement la
feature (`cargo test -p scoretk --features midi`).

### t42 — lecture MIDI (`crates/scoretk-midi`)

**Branché** : la même feature `midi` porte la lecture ; `hooks::read_midi`
appelle `scoretk_midi::read_midi(bytes)` et convertit l'erreur en
`Error::MidiImport`. `Format::Midi.supports_read()` vaut vrai avec la feature,
et `scoretk read(bytes, Format::Midi)` comme `scoretk entrée.mid -o
sortie.musicxml` (ou `.mscx`, `.ly`) fonctionnent. L'import et ses limites sont
décrits dans `docs/agents/midi.md` (§ « Lecture MIDI »).

### t40 — lecture LilyPond (`crates/scoretk-lilypond`)

**Branché** : feature `lilypond-import = ["dep:scoretk-lilypond"]` (même crate
que l'écriture), `hooks::read_lilypond(bytes)` décode l'UTF-8 puis appelle
`scoretk_lilypond::read_lilypond(&str)` et convertit l'erreur en
`Error::LilyPond`. `Format::Ly` répond donc oui à `supports_read` quand la
feature est active. Le sous-ensemble lu, ses limites et l'aller-retour mesuré
sont dans `docs/agents/lilypond.md`.

### t41 — lecture et écriture ABC (`crates/scoretk-abc`)

**Branché** : dépendance optionnelle `scoretk-abc`, feature
`abc = ["dep:scoretk-abc"]`, variante d'erreur `Abc`, et fonctions
`hooks::read_abc`/`hooks::write_abc` (conversion UTF-8 et erreurs). La CLI
active la feature : `scoretk entrée.mscx -o sortie.abc` et
`scoretk entrée.abc -o sortie.musicxml` fonctionnent. Détail du sous-ensemble,
des limites et de l'aller-retour mesuré : `docs/agents/abc.md`.

### t36 — écriture LilyPond (`crates/scoretk-lilypond`)

**Branché** : dépendance optionnelle, `lilypond = ["dep:scoretk-lilypond"]`,
`hooks::write_lilypond` appelle `scoretk_lilypond::write_lilypond(score)` et
convertit l'erreur en `Error::LilyPond`. La CLI active la feature : `scoretk
entrée.mscx -o sortie.ly` fonctionne. Détail de la correspondance et limites :
`docs/agents/lilypond.md`.

La lecture MIDI n'a pas encore de point d'accroche : elle suivra le même
modèle (une variante de feature, une fonction dans `hooks.rs`).

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
- Le check `wasm32-unknown-unknown` compile `-p scoretk-core -p
  scoretk-musescore-import -p scoretk-musicxml-export -p scoretk -p scoretk-abc
  -p scoretk-midi -p scoretk-lilypond -p scoretk-mscx -p scoretk-wasm`.
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
- `crates/scoretk/tests/abc.rs` (feature `abc`) : lecture ABC, écriture,
  relecture et nombre de notes stable ; `crates/scoretk-cli/tests/abc.rs` :
  `.mscx` → `.abc`, `.abc` → MusicXML, stdin/stdout, `--list-formats`.
- `crates/scoretk-cli/tests/lilypond.rs` : `.ly` en `\relative` → MusicXML et
  `--list-formats` avec la lecture LilyPond active.
- `crates/scoretk-abc/tests/` : airs écrits à la main, instantanés du corpus,
  aller-retour noyau (`EXPECTED_ROUNDTRIP`), outils externes.
- `crates/scoretk/tests/mscx.rs` (feature `mscx`) : écriture `.mscx` relue
  avec un modèle identique, sortie MusicXML identique, `.mscz` détecté et relu,
  version `4.70`.
- `crates/scoretk/tests/lilypond_read.rs` (feature `lilypond-import`) :
  `read(..., Format::Ly)` renvoie un modèle exportable, une source sans
  `\score` est une erreur.
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
