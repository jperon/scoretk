# AGENTS.md — noyau des consignes

Ce fichier est lu à **chaque requête** : il ne contient que le noyau. Les
détails vont dans `docs/agents/` (index en fin de fichier).

## Le projet

**scoretk** (GPL-3.0-only) est une boîte à outils de conversion de partitions.
Premier convertisseur : **mscz2xml**, portage en Rust de l'export MusicXML de
**MuseScore Studio** (`.mscz`/`.mscx` → MusicXML `partwise`). Objectif à
terme : MuseScore, MusicXML, LilyPond, ABC et MIDI (voir `docs/TODO.md`).

Carte des crates et modules :

- `crates/mscz2xml` — bibliothèque portable, sans I/O : `container.rs`
  (ZIP `.mscz` → `.mscx`, version), `read/read302` (3.x) et `read/read4xx`
  (4.x/5.x) → modèle, `model/` (modèle en mémoire), `export/` (MusicXML).
- `crates/mscz2xml-cli` — binaire `mscz2xml` : lecture/écriture des fichiers.
- `crates/mscz2xml-wasm` — enveloppe wasm-bindgen (`convert`, `version`),
  sans logique de conversion.
- `crates/scoretk` — façade multi-formats : `Format`, détection par extension
  ou contenu, `read`/`write` vers le modèle pivot ; features
  `musicxml-import` (t34) et `midi` (t35) à brancher dans `src/hooks.rs`.
- `crates/scoretk-cli` — binaire `scoretk` (`entrée -o sortie`, `--from`,
  `--to`, `-` pour stdin/stdout) : I/O disque de la façade.
- `tests/corpus/{v3,v4}/` — `.mscx` amont + références `*.ref.musicxml` ;
  `crates/mscz2xml/tests/` — tests par famille et e2e de conformité.
- `scripts/` — références du corpus, table des instruments, licences ;
  `flake.nix` — shell, checks, paquet wasm.

## Invariants

1. **Bibliothèque portable.** Aucun `std::fs`/`env`/`net`/`process`, aucun
   thread, aucune horloge, aucune dépendance C dans `crates/mscz2xml` : elle
   doit compiler pour `wasm32-unknown-unknown` (sans WASI), Android et iOS.
   Entrées/sorties en octets en mémoire. La CLI et les tests, eux, peuvent
   utiliser le disque.
2. **Fidélité à MuseScore 4.7.4.** La conformité se mesure contre les
   références `*.ref.musicxml` produites par MuseScore 4.7.4. La source C++
   portée est un snapshot amont 5.0.0 de développement : en cas de divergence,
   c'est le comportement 4.7.4 qui fait foi (sauf décision explicite de changer
   de référence, cf. `docs/agents/upstream.md`). Sortie reproductible : pas
   d'horloge, date d'encodage optionnelle avec constante par défaut.
3. **Nouveau code dans de nouveaux fichiers, par famille** (lecteur 3.x,
   lecteur 4.x, modèle, export). Un fichier existant ne reçoit que le
   branchement (`mod.rs`) ou un correctif localisé. Portage fidèle : suivre les
   noms et découpages du C++ (`TRead::read(Chord*)`, `write_measure`,
   `calc_divisions`), avec la référence amont en commentaire ; ne pas coder de
   mémoire.
4. **Licence GPL-3.0-only.** Tout fichier source nouveau ou modifié porte
   l'identifiant SPDX, le copyright, le texte GPL-3.0-only et, s'il est porté,
   l'avis de modification « Portage de `<chemin C++>` …, modifié (réécrit en
   Rust) en 2026 ». Toute nouvelle source ou donnée amont se déclare dans
   `NOTICE` ; toute nouvelle crate tierce dans `docs/licences-tierces.md`
   (vérifiée par `scripts/check-licences.py`). Modèle d'en-tête :
   `docs/agents/developpement.md`.
5. **Contrat wasm stable pour cantus.** Ne pas changer sans coordination :
   paquet `package.json` `name: "scoretk"` ; `esm/mscz2xml_wasm.js` +
   `mscz2xml_wasm_bg.wasm` + `.d.ts` ; `classic/mscz2xml.js` exposant
   `self.mscz2xml = { ready, convert, version, baseUrl }` ;
   `convert(bytes, { encodingDate? }) → string` ; `version() → string`.
6. **Pas de C++ amont distribué.** Le portage est une réécriture ; la copie de
   référence `ref-musescore/` est en lecture seule et non versionnée.

## Commandes

```sh
nix develop -c cargo test --workspace
nix develop -c cargo clippy --workspace --all-targets -- -D warnings
nix develop -c cargo fmt --all -- --check
nix develop -c cargo build -p mscz2xml --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk --target wasm32-unknown-unknown
nix flake check
nix build .#wasm

# Conformité, deux niveaux (noyau musical, puis parité complète)
nix develop -c cargo test -p mscz2xml --test e2e_corpus -- --nocapture
```

Les données dérivées se régénèrent avec `scripts/gen-references.sh` et
`scripts/gen-instruments-table.py` ; le paquet wasm se teste avec
`crates/mscz2xml-wasm/tests/node-smoke.mjs` (après `nix build .#wasm`).

## Pièges qui cassent en silence

- **Hors `nix develop -c`, le `cargo` du PATH est un shim rustup** en conflit
  (`rust-std`) : toujours passer par le shell Nix.
- **Deux niveaux e2e, et le noyau seul est bloquant pour
  `EXPECTED_CONFORMANT`** : un fichier non listé peut régresser sans bruit ;
  une différence de mise en page n'est pas un écart musical, ne pas
  « corriger » l'export pour un attribut de position.
- **Ne pas régénérer les références avec une autre version de MuseScore** sans
  décision explicite : la cible de conformité change en silence
  (`NOTICE`, `SOURCES.md`, `DEFAULT_SOFTWARE`, documents d'écarts à jour).
- **`wasm-bindgen` est épinglé `=0.2.130`**, égal au CLI du flake ; sinon
  « schema version mismatch ».
- **Le corpus doit rester dans la source des checks Nix** (filtre
  `tests/corpus` de `flake.nix`), sinon le check `tests` échoue.
- **Fichiers générés** : `read/instruments.rs` et
  `docs/agents/ecarts-complets.md` se régénèrent, ne s'éditent pas.
- **Ordre des `<direction>`, spanners et `<credit>` significatif** pour la
  parité : ne pas trier une collection sans vérifier le C++ et la référence.
- **`ref-musescore/` est absent d'une extraction fraîche** : le recréer avec
  `docs/agents/upstream.md` avant de lire le C++.
- **Lecture du format** : tick implicite, `actualTicks()` ≠ `ticks()`, ordre
  `<dots>` avant `<durationType>`, voix positionnelles, `<tick>` absolu vs
  `<location>` relatif, liaisons 3.x/4.x en deux moitiés vs `<Tie>` à EID en
  5.0, spanners 5.0 hors mesure, premier `rootfile` `.mscx` (pas `.mss`).
  Détail : `docs/agents/mscx-format.md` § 17.

## Index `docs/agents/`

Une ligne par document ; lire celui qui correspond au sujet avant de coder.

- `docs/agents/upstream.md` — version portée, clone de `ref-musescore`, table
  C++ → Rust, suivi amont : à lire avant tout portage ou mise à jour.
- `docs/agents/developpement.md` — conventions, en-têtes de licence,
  portabilité, contrat wasm, outillage : à lire avant d'écrire du code.
- `docs/agents/mscx-format.md` — référence du format `.mscx` 3.x/4.x/5.x et
  pièges de lecture : à lire avant de toucher à un lecteur.
- `docs/agents/ecarts-noyau.md` — écarts de conformité du noyau, fichier par
  fichier et causes : à lire pour choisir la prochaine correction.
- `docs/agents/ecarts-complets.md` — rapport **généré** de parité complète :
  à lire pour mesurer, pas pour décider seul d'un correctif.
- `docs/agents/wasm.md` — construction, API et usage du paquet wasm (ESM,
  worker cantus) : à lire avant de toucher au crate wasm.
- `docs/agents/scoretk.md` — façade multi-formats : API, détection, points de
  branchement t34/t35, CLI et tests : à lire avant de toucher à `scoretk`.
- `docs/agents/licence.md` — obligations GPL-3.0-only d'un consommateur :
  à lire avant toute distribution ou intégration.
