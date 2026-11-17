# Écriture MuseScore : `.mscx` et `.mscz`

Crate `crates/scoretk-mscx`, branchée dans la façade `scoretk` par la feature
`mscx` (`Format::Mscx` et `Format::Mscz` en écriture). Le noyau du projet et
les invariants sont dans `AGENTS.md` ; la correspondance C++ → Rust est dans
`docs/agents/upstream.md`.

## 1. Rôle et API

```rust
pub fn write_mscx(score: &Score) -> Result<String, Error>; // <museScore version="4.70">
pub fn write_mscz(score: &Score) -> Result<Vec<u8>, Error>; // archive ZIP
```

L'écrivain est le portage de `engraving/rw/write/` (`Writer::writeScore`,
`StaffWrite::writeStaff`, `MeasureWrite::writeMeasure`, `TWrite::writeItem`,
`ConnectorInfoWriter`, `WriteContext`) et de
`engraving/infrastructure/mscwriter.cpp` pour le conteneur. Les noms de
fonctions et le découpage suivent le C++ : `write/writer.rs`,
`write/staffwrite.rs`, `write/measurewrite.rs`, `write/twrite.rs`,
`write/connectorinfowriter.rs`, `write/context.rs`.

La bibliothèque est portable (aucun `std::fs`, thread ni horloge) et compile
pour `wasm32-unknown-unknown` ; le test de conformité tourne sur l'hôte.

## 2. Contenu du `.mscz`

`MscSaver::writeMscz` écrit le style à part et le reste dans l'archive. La
crate produit, dans cet ordre et de façon déterministe (deflate, horodatage ZIP
figé) :

| Entrée | Rôle |
| --- | --- |
| `META-INF/container.xml` | `rootfile` vers `score_style.mss` puis `score.mscx`, comme MuseScore 4.7 |
| `score_style.mss` | `<Style>` du modèle (mêmes valeurs que dans le `.mscx`) |
| `score.mscx` | partition complète |
| `audiosettings.json`, `viewsettings.json` | `{}` |

La vignette (`Thumbnails/thumbnail.png`) et les médias ne sont pas produits :
la bibliothèque ne fait pas de rendu. MuseScore les recrée à l'enregistrement.

## 3. Fidélité mesurée : aller-retour

Le test `crates/scoretk-mscx/tests/roundtrip.rs` rejoue, pour chaque fichier du
corpus, la chaîne

```text
lire (.mscx 3.x ou 4.x) → écrire .mscx → relire avec le lecteur 4.x
→ exporter MusicXML
```

et compare le MusicXML obtenu à l'export **direct** du modèle d'origine, avec la
normalisation exacte de `crates/mscz2xml/tests/e2e_corpus.rs`, aux deux niveaux
(noyau musical et parité complète). Résultat au 2026-11-17 :

| Corpus | Aller-retour identique (noyau et complet) |
| --- | --- |
| `tests/corpus/v4` | **26/26** |
| `tests/corpus/v3` (lu en 3.x, écrit en 4.x) | **30/30** |

Tous ces fichiers sont listés dans `EXPECTED_ROUNDTRIP_V4` et
`EXPECTED_ROUNDTRIP_V3` : l'un d'eux qui cesse d'être identique fait échouer le
test. La liste est donc non régressive, comme `EXPECTED_CONFORMANT` du test e2e.

Un second test (`archive_mscz_relue`) vérifie le conteneur `.mscz` ; la CLI
`scoretk` a ses propres tests (`.mscx` et `.mscz` écrits puis relus).

## 4. Ce qui est conservé, et comment

Le modèle ne stocke que ce dont l'export MusicXML a besoin ; l'écrivain produit
les éléments que le lecteur relit pour reconstruire ce même modèle :

- **partition** : `<Division>`, `<Style>` complet (format de page, marges,
  `lyricsPlacement`, tailles de police, `hairpinHeight`, portées vides),
  `<metaTag>` (tous les champs, même vides : un `<metaTag name="platform">`
  vide ne vaut pas « absent ») ;
- **parties et portées** : `<Part>` (noms, `<Instrument>` avec transposition,
  programme, canal MIDI, volume, panoramique, articulations MIDI),
  déclarations `<Staff>` (lignes, `barLineSpan`, clef par défaut, accolades) ;
- **mesures** : `len` des mesures irrégulières, `<startRepeat>`/`<endRepeat>`,
  `<multiMeasureRest>` (mesure de mise en page après celle qui ouvre le
  groupe), `<LayoutBreak>` (`section` sur la précédente, `line` sur celle qui
  finit le système), `<Marker>`/`<Jump>`, `MeasureRepeat` (avec
  `measureRepeatCount`, `<durationType>measure</durationType>` et sa durée :
  MuseScore 4.7 refuse le fichier sans ces deux enfants ; le contenu est
  recopié par le lecteur, non réécrit) ;
- **voix et rythme** : `<voice>` positionnelles, positions exactes par
  `<location><fractions>` (aucun arrondi), `<durationType>`, `<dots>`,
  `<duration>` explicite, `<Beam>`/`<BeamMode>`, `<Tuplet>` (avec `<bracket>`),
  accords d'agrément en `<Chord>` séparés, silences `gap`, paroles, doigtés ;
- **notations** : armures (`concertKey`/`actualKey`, mode, courtoisie),
  indications de mesure (sous-type, étirement, courtoisie), clefs (`isHeader`),
  barres (extension), articulations, ornements, points d'orgue, trémolos,
  arpèges, lignes d'accord, altérations (`role`/`parens`), liaisons de
  prolongation (deux moitiés avec `<location>`), slurs, trilles, glissandos,
  soufflets, ottavas, pédales, voltas, lignes de texte ;
- **annotations** : nuances, textes de portée/expression/système (balisage
  MuseScore reconstruit, entités `&quot;` conservées, `<sym>` rétabli),
  tempos, repères, changements d'instrument, accords chiffrés
  (`harmonyType`, degrés), chiffrages figurés, barres de vibrato ;
- **cadres** : crédits (`<VBox>` par page, hauteur, style, alignement), textes
  `TBox` (au-dessus ou au-dessous de la mesure, taille de police).

Les positions sont écrites en fractions exactes (`<location><fractions>`), ce
qui évite toute perte sur les n-olets (3, 5, 7…), contrairement à un `<tick>`
entier.

## 5. Limites connues

- **Mise en page non modélisée** : positions fines (`default-x/y`, hampes,
  ligatures calculées, largeur des systèmes) ne sont pas dans le modèle ; la
  sortie est musicalement identique, pas graphiquement. C'est la même limite
  que l'export MusicXML.
- **Ordre de `spanners` recopié** : un spanner entièrement contenu dans une
  mesure répétée n'est pas écrit ; le lecteur le recrée en développant la
  mesure source.
- **Champs sans équivalent `.mscx`** : `Accidental.user` passe par `<role>`,
  parenthèses par `<parens>` ; `Dimension`/`BarLine` construits à la main
  peuvent perdre `EndStartRepeat` (écrit comme `<endRepeat>` + `<startRepeat>`),
  ce que l'export MusicXML ne distingue pas.
- **Format 3.x** : la sortie est toujours 4.70 ; le lecteur 3.x n'est pas
  écrit. Les éléments lus en 3.x mais sans équivalent 4.x (par exemple les
  encodages anciens d'armure `<subtype>`) sont convertis par la lecture 3.x,
  pas par l'écriture.
- **Dates** : `<programVersion>` est figée à `4.7.4` et `<programRevision>`
  vide ; aucune horloge n'est lue (sortie reproductible).

## 6. Vérifications

```sh
nix develop -c cargo test -p scoretk-mscx
nix develop -c cargo build -p scoretk-mscx --target wasm32-unknown-unknown
# MuseScore 4.7.4 rouvre-t-il les sorties ? (headless, hors session graphique)
scripts/check-mscx-rewrite.sh
```

`scripts/check-mscx-rewrite.sh` réécrit chaque fichier du corpus en `.mscx` et
`.mscz`, puis demande à MuseScore 4.7.4 (`QT_QPA_PLATFORM=offscreen`) de
l'ouvrir et de l'exporter. Il vérifie l'ouverture, pas la parité graphique :
au 2026-11-17, **56/56 fichiers** (les 26 v4 et les 30 v3, en `.mscx` et en
`.mscz`) sont rouverts sans `--force`.
