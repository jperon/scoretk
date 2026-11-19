# AGENTS.md — noyau des consignes

Ce fichier est lu à **chaque requête** : il ne contient que le noyau. Les
détails vont dans `docs/agents/` (index en fin de fichier).

## Le projet

**scoretk** (GPL-3.0-only) est une boîte à outils de conversion de partitions.
Portage en Rust des convertisseurs de **MuseScore Studio** : cœur générique,
lecture `.mscz`/`.mscx`, export MusicXML `partwise`, puis MuseScore, MusicXML,
LilyPond, ABC et MIDI (voir `docs/TODO.md`).

Carte des crates et modules (17 membres : un cœur, une façade, sa CLI et son
paquet wasm, plus les formats découpés par sens) :

```
crates/
  scoretk-core              modèle pivot, aucune I/O
  scoretk-musescore-import  .mscz/.mscx → modèle (3.x et 4.x/5.x)
  scoretk-musescore-export  modèle → .mscx / .mscz
  scoretk-musicxml-import   .musicxml/.xml/.mxl → modèle
  scoretk-musicxml-export   modèle → MusicXML partwise / .mxl
  scoretk-midi-common       erreur partagée, rien de plus
  scoretk-midi-import       MIDI → modèle
  scoretk-midi-export       modèle → MIDI (reprises, carte des temps)
  scoretk-lilypond-common   erreur, tpc, clefs, partagés par les deux sens
  scoretk-lilypond-import   sous-ensemble LilyPond → modèle
  scoretk-lilypond-export   modèle → .ly
  scoretk-abc-common        tables et helpers ABC partagés par les deux sens
  scoretk-abc-import        .abc → modèle
  scoretk-abc-export        modèle → .abc
  scoretk                   façade : Format, read/write/convert, hooks
  scoretk-cli               binaire `scoretk` (I/O disque)
  scoretk-wasm              enveloppe wasm-bindgen
```

- `crates/scoretk-core` — cœur portable et générique, sans I/O ni format :
  `model/` (modèle en mémoire), `error.rs`, `gap_rests.rs` et
  `model/stable_ids.rs` (adressage d'éléments).
- `crates/scoretk-musescore-import` — lecture MuseScore : `container.rs` (ZIP
  `.mscz` → `.mscx`, version), `read/read302` (3.x) et `read/read4xx`
  (4.x/5.x) → modèle.
- `crates/scoretk-musescore-export` — écrivain MuseScore : `write_mscx`
  (XML 4.x) et `write_mscz` (ZIP), portage de `engraving/rw/write/`.
- `crates/scoretk-musicxml-import` — lecteur MusicXML (`read_musicxml`) vers le
  modèle : `.musicxml`/`.xml`/`.mxl` → `Score`.
- `crates/scoretk-musicxml-export` — export MusicXML : modèle → `partwise` /
  `.mxl`, avec `ExportOptions` et les constantes `DEFAULT_*`.
- `crates/scoretk-midi-common` — erreur et résultat partagés par les deux
  sens, sans I/O ; rien d'autre (la lecture SMF est dans `-import`).
- `crates/scoretk-midi-import` — lecture MIDI (`read_midi`, portage de l'import
  de MuseScore) : `midifile.rs` (format SMF, via `midly`), quantification,
  voix, `timesigmap`, `tuplet`…, sans I/O.
- `crates/scoretk-midi-export` — écriture MIDI (SMF format 1) avec déroulement
  des reprises (`repeat_list.rs`) et carte des temps (`timemap.rs`), sans I/O.
- `crates/scoretk-lilypond-common` — ce que la lecture et l'écriture LilyPond
  partagent : erreur, spirale des quintes (`tpc`), table des clefs. Aucune
  logique de conversion.
- `crates/scoretk-lilypond-import` — lecture d'un sous-ensemble de LilyPond
  par portage de `lily/parser.yy`/`lexer.ll` (t40), sans I/O.
- `crates/scoretk-lilypond-export` — écriture LilyPond (`.ly`) depuis le
  modèle (t36, voix en `\fixed c'`), sans I/O.
- `crates/scoretk-abc-common` — tables et helpers ABC partagés par les deux
  sens (hauteurs, armures, clés, durées, décorations, paroles) ; aucun I/O.
- `crates/scoretk-abc-import` — lecture ABC (`.abc`) vers le modèle, norme 2.1
  et comportement d'`abc2xml.py`, sans I/O (t41).
- `crates/scoretk-abc-export` — écriture ABC (`.abc`) depuis le modèle,
  comportement de `xml2abc.py`, sans I/O (t41).
- `crates/scoretk` — façade multi-formats : `Format`, détection par extension
  ou contenu, `read`/`write`/`convert` vers le modèle pivot ; dix features
  symétriques `<format>-import` / `<format>-export` (`musescore`, `musicxml`,
  `midi`, `lilypond`, `abc`), toutes actives par défaut, branchées dans
  `src/hooks.rs` et sans alias mixte.
- `crates/scoretk-cli` — binaire `scoretk` (`entrée -o sortie`, `--from`,
  `--to`, `-` pour stdin/stdout) : I/O disque de la façade.
- `crates/scoretk-wasm` — enveloppe wasm-bindgen de la façade (`convert`,
  `detect`, `formats`, `version`) ; unique paquet wasm du dépôt
  (`nix build .#wasm`).
- `tests/corpus/{v3,v4}/` — `.mscx` amont + références `*.ref.musicxml` ;
  `tests/corpus/midi/` — références `*.ref.mid` ; `crates/scoretk/tests/`
  (aller-retour, e2e de conformité et branchement de la façade) ; les tests
  par sens vivent dans le crate du sens (`crates/scoretk-*-import/tests/`,
  `crates/scoretk-*-export/tests/`).
- `scripts/` — références du corpus, table des instruments, licences ;
  `flake.nix` — shell, checks, paquet wasm.

## Invariants

1. **Bibliothèque portable.** Aucun `std::fs`/`env`/`net`/`process`, aucun
   thread, aucune horloge, aucune dépendance C dans `crates/scoretk-core` : elle
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
   paquet `package.json` `name: "scoretk"` ; `esm/scoretk_wasm.js` +
   `esm/scoretk_wasm_bg.wasm` + `esm/scoretk_wasm.d.ts` ;
   `classic/scoretk.js` exposant
   `self.scoretk = { ready, convert, convertWithTimeline, version, detect,
   formats, baseUrl }` ;
   `convert(Uint8Array, { from?, to?, encodingDate?, software?, concertPitch?,
   expandRepeats? }) → Uint8Array` (`to` vaut `"musicxml"` par défaut) ;
   `detect(bytes) → string | null` ; `formats() → FormatInfo[]` ;
   `version() → string` ; `convertWithTimeline(bytes, options?) →
   { musicxml, midi, timeline }` (additif : MusicXML identifié et carte des
   temps, voir `docs/agents/timemap.md` ; `timeline.measures[].ordinal` est
   l'indice 0-basé de la mesure et la seule clé d'ancrage, `timeline.notes`
   est trié par `tick` croissant puis par clé stable). `convert` renvoie des
   `Uint8Array` que le consommateur décode avec `TextDecoder`
   (voir `docs/agents/wasm.md`).
6. **Pas de C++ amont distribué.** Le portage est une réécriture ; la copie de
   référence `ref-musescore/` est en lecture seule et non versionnée.

## Commandes

```sh
nix develop -c cargo test --workspace
nix develop -c cargo clippy --workspace --all-targets -- -D warnings
nix develop -c cargo fmt --all -- --check
nix develop -c cargo build -p scoretk-core --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-midi-common -p scoretk-midi-import -p scoretk-midi-export --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-lilypond-common -p scoretk-lilypond-import -p scoretk-lilypond-export --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-abc-common -p scoretk-abc-import -p scoretk-abc-export --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-musescore-export --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-musicxml-import --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk --target wasm32-unknown-unknown
nix flake check
nix build .#wasm
nix develop -c cargo build -p scoretk-wasm --target wasm32-unknown-unknown

# Conformité, deux niveaux (noyau musical, puis parité complète)
nix develop -c cargo test -p scoretk --test e2e_corpus -- --nocapture
```

Les données dérivées se régénèrent avec `scripts/gen-references.sh` et
`scripts/gen-instruments-table.py` ; le paquet wasm se teste avec
`crates/scoretk-wasm/tests/node-smoke.mjs` (après `nix build .#wasm`).

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
- **Règle des crates `-common` (factoriser intelligemment) :** ce qui sert à
  tous les formats va dans un crate commun (p. ex. utilitaires XML partagés),
  ce qui sert aux deux sens d'un format va dans son `-common`, ce qui ne sert
  qu'à un seul sens reste dans son fichier ; jamais de module à un seul sens
  dans un `-common`, ni de logique de format dans un crate commun
  (règle complète dans `docs/agents/developpement.md`).
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
- `docs/agents/wasm.md` — unique paquet wasm (façade scoretk) : construction,
  tailles, API (convert/detect/formats/version), options, usage ESM/classic,
  tests Node : à lire avant de toucher au
  crate `scoretk-wasm`.
- `docs/agents/midi.md` — lecture et écriture MIDI, `RepeatList`, pauses,
  vélocités, références et écarts : à lire avant de toucher aux crates
  `scoretk-midi-*`.
- `docs/agents/timemap.md` — carte des temps (`Timemap`) : structure Rust et
  JS, identifiants MusicXML stables, passages de reprises, pauses, usage côté
  client et limites : à lire avant de toucher à `write_playback` ou
  `convertWithTimeline`.
- `docs/agents/scoretk.md` — façade multi-formats : API, détection, points de
  branchement t34/t35/t36/t38, CLI et tests : à lire avant de toucher à `scoretk`.
- `docs/agents/licence.md` — obligations GPL-3.0-only d'un consommateur :
  à lire avant toute distribution ou intégration.
- `docs/agents/musicxml-import.md` — portage de l'import MusicXML, résultat
  du test d'aller-retour et écarts restants : à lire avant de toucher au
  lecteur MusicXML.
- `docs/agents/lilypond.md` — LilyPond : écriture (correspondance modèle →
  `.ly`, mode `\fixed`, instantanés), lecture (sous-ensemble de
  `parser.yy`/`lexer.ll`, aller-retour mesuré), limites et mise à niveau des
  sources : à lire avant de toucher aux crates `scoretk-lilypond-*`.
- `docs/agents/abc.md` — lecture et écriture ABC : découpage en trois crates,
  sous-ensemble, voix, instantanés, aller-retour mesuré, mise à niveau de
  `xml2abc`/`abc2xml` : à lire avant de toucher aux crates `scoretk-abc-*`.
- `docs/agents/mscx-write.md` — écrivain `.mscx`/`.mscz`
  (`scoretk-musescore-export`) :
  méthode d'aller-retour, résultats mesurés et limites : à lire avant de
  toucher à l'écriture MuseScore.
- `docs/agents/performance.md` — banc criterion natif (`benches/conversion.rs`),
  temps par sens sur le corpus, profil `perf`/Callgrind, allocations et pistes
  chiffrées : à lire avant d'optimiser une conversion.
- `docs/agents/plan-fiabilite.md` — plan de fiabilisation des dix sens :
  lots de référence, oracles, phases : à lire avant une campagne de mesure.
- docs/agents/format-mapping/ — tables format ↔ modèle par format et lacunes
  du modèle : à lire avant d'étendre un lecteur ou un écrivain.
- docs/agents/format-ref/ — références synthétiques de syntaxe (anglais) pour
  agents, une par format, avec n° de section et pièges : à lire avant d'écrire
  ou relire un lecteur/écrivain.
