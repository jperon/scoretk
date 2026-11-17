# scoretk : façade de conversion

Crate `crates/scoretk` (bibliothèque) et `crates/scoretk-cli` (binaire
`scoretk`). Ce document décrit l'aiguillage, les points de branchement des
convertisseurs à venir et les tests. Le noyau du projet reste dans `AGENTS.md`.

## 1. Rôle

Le modèle de `crates/mscz2xml/src/model` est le **pivot commun** : chaque format
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
```

`Format` fournit `from_extension`, `from_path` (extension seule, pas d'accès
disque), `detect` (contenu), `supports_read`/`supports_write` (état réel,
features comprises) et `FromStr` (`mscz`, `mscx`, `musicxml`/`xml`, `mxl`,
`ly`/`lilypond`, `abc`, `mid`/`midi`).

### Détection par contenu

Ordre de `Format::detect` :

1. signature ZIP (`.mscz` s'il contient un `.mscx`, sinon `.mxl` s'il contient
   un `.xml`/`.musicxml`) ;
2. `MThd` → MIDI ;
3. `<museScore` → `.mscx` ;
4. `<score-partwise`/`<score-timewise`, ou `<?xml` seul → MusicXML ;
5. `\version` en tête → LilyPond ;
6. `%abc` ou un en-tête `X:` → ABC.

Rien ne correspond → `None` (la CLI demande alors `--from`).

### Limites actuelles

| Format | Lecture | Écriture |
| --- | --- | --- |
| `.mscz` / `.mscx` | oui (`mscz2xml`) | oui (feature `mscx`, `scoretk-mscx`) |
| MusicXML `.musicxml` / `.xml` | feature `musicxml-import` (t34) | oui |
| MusicXML `.mxl` | non | oui |
| LilyPond `.ly` | non | oui (feature `lilypond`, `scoretk-lilypond`) |
| ABC | non | non |
| MIDI | non | oui (feature `midi`, `scoretk-midi`) |

Un format absent dans le sens demandé renvoie
`Error::FormatNonPrisEnCharge { format, sens }`, jamais une sortie vide.
L'écriture `.mxl` réutilise `mscz2xml::export::export_mxl_with` (archive ZIP
`META-INF/container.xml` + `score.xml`, comme `saveMxl` de MuseScore).

## 3. Brancher les convertisseurs (t34, t35, t36)

Tout le code conditionnel est dans `crates/scoretk/src/hooks.rs`. Les features
existent déjà, désactivées par défaut ; les variantes d'erreur
`MusicXmlImport`/`Midi` aussi.

### t34 — lecture MusicXML (`crates/scoretk-musicxml`)

1. Dans `crates/scoretk/Cargo.toml`, ajouter la dépendance optionnelle
   `scoretk-musicxml = { path = "../scoretk-musicxml", optional = true }` et
   remplacer `musicxml-import = []` par
   `musicxml-import = ["dep:scoretk-musicxml"]`.
2. Rien d'autre : la fonction `hooks::read_musicxml` appelle déjà
   `scoretk_musicxml::read_musicxml(bytes)` et convertit l'erreur en
   `Error::MusicXmlImport` (l'erreur de la crate doit implémenter `Display`).

### t38 — écriture MuseScore (`crates/scoretk-mscx`)

**Branché** : dépendance optionnelle `scoretk-mscx`, feature
`mscx = ["dep:scoretk-mscx"]`, variante d'erreur `Mscx`, et fonctions
`hooks::write_mscx`/`write_mscz` (`.mscx` texte, `.mscz` archive ZIP avec
`META-INF/container.xml`, `score.mscx` et `score_style.mss`). La CLI active la
feature : `scoretk entrée.mscx -o sortie.mscx` et `… -o sortie.mscz`.

### t35 — écriture MIDI (`crates/scoretk-midi`)

**Branché** : la dépendance optionnelle et `midi = ["dep:scoretk-midi"]` sont
en place ; `hooks::write_midi` appelle `scoretk_midi::write_midi(score)` et
convertit l'erreur en `Error::Midi`. La feature reste désactivée par défaut :
l'activer explicitement (`cargo test -p scoretk --features midi`, ou la
dépendance de la CLI) pour que `scoretk` écrive du MIDI.

### t36 — écriture LilyPond (`crates/scoretk-lilypond`)

**Branché** : dépendance optionnelle, `lilypond = ["dep:scoretk-lilypond"]`,
`hooks::write_lilypond` appelle `scoretk_lilypond::write_lilypond(score)` et
convertit l'erreur en `Error::LilyPond`. La CLI active la feature : `scoretk
entrée.mscx -o sortie.ly` fonctionne. Détail de la correspondance et limites :
`docs/agents/lilypond.md`.

La lecture MIDI, LilyPond et ABC n'ont pas encore de point d'accroche : elles
suivront le même modèle (une variante de feature, une fonction dans `hooks.rs`).

## 4. CLI

```sh
scoretk entrée.mscz -o sortie.musicxml      # formats déduits des extensions
scoretk entrée.mscx --to mxl -o -           # sortie standard
scoretk entrée.mscx -o copie.mscx           # réécriture MuseScore 4.70
scoretk entrée.mscx -o copie.mscz           # conteneur MuseScore
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
  UTC, comme `mscz2xml`).
- La CLI `mscz2xml` reste inchangée.

## 5. Flake

- `packages.scoretk` construit la CLI (`cargoExtraArgs = "-p scoretk-cli"`).
- `apps.scoretk` la rend exécutable par `nix run .#scoretk -- …`.
- `packages.default` reste `mscz2xml` (pas de changement de défaut).
- Le check `wasm32-unknown-unknown` compile `-p mscz2xml -p scoretk`.

## 6. Tests

- `crates/scoretk/tests/formats.rs` : extensions, chemins, `FromStr`, détection
  par contenu (XML, ZIP `.mscz`/`.mxl`, MIDI, LilyPond, ABC, inconnu).
- `crates/scoretk/tests/parite.rs` : `.mscz` → MusicXML **identique octet pour
  octet** à `mscz2xml::convert` sur le corpus (`dynamics`, `hairpin_simple`,
  `hairpins_and_repeats`), `.mxl` (container + `score.xml`), erreurs explicites
  des formats non branchés.
- `crates/scoretk/tests/mscx.rs` (feature `mscx`) : écriture `.mscx` relue
  avec un modèle identique, sortie MusicXML identique, `.mscz` détecté et relu,
  version `4.70`.
- `crates/scoretk-cli/tests/cli.rs` : binaire réel (`CARGO_BIN_EXE_scoretk`),
  `--list-formats`, `--version`, stdin/stdout, `.mscz` → MusicXML comparé à
  `mscz2xml`, sortie `.mxl`, `.mscx`/`.mscz` écrits et relus, `.mscx` →
  `.ly`, erreurs de `--from`/`--to` manquants et de format non branché.
- Le corpus est lu via `CARGO_MANIFEST_DIR/../../tests/corpus` ; il doit rester
  dans le filtre de `flake.nix` (voir `docs/agents/developpement.md`).

## 7. Écarts connus

- `MsczContainer::from_bytes` accepte un `.mscx` nu : `read(..., Format::Mscx)`
  et `read(..., Format::Mscz)` suivent donc le même chemin.
- `detect` classe tout ZIP sans `.mscx` mais avec un `.xml` comme `.mxl` ; un
  ZIP quelconque sans XML ni `.mscx` n'est pas reconnu.
- Un XML inconnu est supposé MusicXML (`<?xml` seul) : c'est le seul format XML
  de scoretk aujourd'hui ; `--from` reste prioritaire.
- Aucun écart mesuré avec `mscz2xml` : la façade appelle ses fonctions, la
  comparaison du test est une égalité stricte.
