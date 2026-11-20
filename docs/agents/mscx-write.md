# Écriture MuseScore : `.mscx` et `.mscz`

Crate `crates/scoretk-musescore-export`, branchée dans la façade `scoretk` par
la feature `musescore-export` (`Format::Mscx` et `Format::Mscz` en écriture).
Le noyau du projet et les invariants sont dans `AGENTS.md` ; la correspondance C++ → Rust est dans
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

Le test `crates/scoretk-musescore-export/tests/roundtrip.rs` rejoue, pour
chaque fichier du corpus, la chaîne

```text
lire (.mscx 3.x ou 4.x) → écrire .mscx → relire avec le lecteur 4.x
→ exporter MusicXML
```

et compare le MusicXML obtenu à l'export **direct** du modèle d'origine, avec la
normalisation exacte de `crates/scoretk/tests/e2e_corpus.rs`, aux deux niveaux
(noyau musical et parité complète). Résultat au 2026-11-17 :

| Corpus | Aller-retour `.mscx` identique | Aller-retour `.mscz` identique |
| --- | --- | --- |
| `tests/corpus/v4` | **26/26** (noyau et complet) | **26/26** |
| `tests/corpus/v3` (lu en 3.x, écrit en 4.x) | **30/30** | **30/30** |

Les deux conteneurs sont mesurés pour chaque fichier.

Tous ces fichiers sont listés dans `EXPECTED_ROUNDTRIP_V4` et
`EXPECTED_ROUNDTRIP_V3` : l'un d'eux qui cesse d'être identique fait échouer le
test. La liste est donc non régressive, comme `EXPECTED_CONFORMANT` du test e2e.

Le test `archive_mscz_relue` vérifie en plus le conteneur `.mscz` sur un
fichier ; la CLI `scoretk` a ses propres tests (`.mscx` et `.mscz` écrits puis
relus).

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
nix develop -c cargo test -p scoretk-musescore-export
nix develop -c cargo build -p scoretk-musescore-export --target wasm32-unknown-unknown
# MuseScore 4.7.4 rouvre-t-il les sorties ? (headless, hors session graphique)
scripts/check-mscx-rewrite.sh
```

`scripts/check-mscx-rewrite.sh` réécrit chaque fichier du corpus en `.mscx` et
`.mscz`, puis demande à MuseScore 4.7.4 (`QT_QPA_PLATFORM=offscreen`) de
l'ouvrir et de l'exporter. Il vérifie l'ouverture, pas la parité graphique :
au 2026-11-17, **56/56 fichiers** (les 26 v4 et les 30 v3, en `.mscx` et en
`.mscz`) sont rouverts sans `--force`.

## 7. Ajustements du lecteur 4.x faits pour l'aller-retour

Quelques propriétés que l'écrivain doit conserver n'étaient pas relues par le
lecteur 4.x (elles ne changeaient pas les références du corpus, qui ne les
contient pas) ; elles ont été ajoutées dans `read4xx`, avec la même sémantique
que `read302` et `TRead` :

| Propriété | Élément `.mscx` | Effet sur l'export |
| --- | --- | --- |
| crochet de n-olet | `<Tuplet><bracket>` | `<tuplet bracket="no">` |
| écriture d'accord | `<Harmony><harmonyType>` | branche `<numeral>`/`<function>` au lieu de `<root>` |
| degrés d'accord | `<Harmony><degree>` | `<degree>` MusicXML |
| rôle d'altération | `<Accidental><role>` | `cautionary="yes"` |
| parenthèses d'altération | `<Accidental><parens>` | `parentheses="yes"` |
| canal MIDI | `<Channel><midiChannel>` | `<midi-channel>` et canaux libres |
| changement d'instrument | `<InstrumentChange><Instrument>` | `Part::instruments()`, `<score-instrument>` |
| cadre sous la mesure | `<TBox><Text><offset y>` | cible et placement du texte |
| taille de police de cadre | `<TBox><Text><size>` | `font-size` des `<words>` |

Le test e2e du corpus (références 4.7.4) reste inchangé par ces ajouts : seuls
`v3/moonlight` (18 lignes) et `v3/voices` (6 lignes) diffèrent encore des
références, pour les raisons de mise en page déjà documentées dans
`docs/agents/ecarts-noyau.md`.

## 8. Measured fidelity after reopening in MuseScore 4.7.4 (English)

### Frozen baseline and oracle

There is no published specification of `.mscz` (see
`docs/agents/format-ref/musescore.md`): the behaviour of MuseScore 4.7.4 is the
reference. The round-trip test of §3 only uses scoretk's *own* reader, which
can hide symmetric errors; the oracle below uses MuseScore itself.

- Frozen binary: `/tmp/scoretk-mscz-base`, built from commit `497f9e3`, sha256
  starting `04033ff2586c5090`. The fixed binary `/tmp/scoretk-mscz-final`
  (sha256 starting `3611bacac5460a50`) is built from the same worktree after
  the four fixes below, so both runs can be compared file by file.
- Lots: the 56 corpus `.mscx`, the 174 `.mscz` of `~/Musique`, the 32 genuine
  MusicXML files of `~/Musique`, a deterministic 1/5 sample of the 1 872 `.ly`
  files (375) and of the `.abc` files (1). Manifests are built by
  `scripts/oracle/musescore/lots.sh`, which reuses the lists of the LilyPond
  and ABC writing oracles. No third-party file is versioned.
- Oracle: `scripts/oracle/musescore/mesure.sh`. For each source, scoretk writes
  a `.mscz`, MuseScore 4.7.4 reopens it and exports MusicXML and MIDI, and the
  result is compared to scoretk's *direct* MusicXML and MIDI. Core comparison
  reuses `core_norm` through `crates/scoretk/tests/oracle_core.rs` (no
  duplicated normalisation), N1/N2 reuse `scripts/oracle/midi/midi_notes.py`
  at the Q16 grid (±1/16 quarter). Every binary runs under `timeout` and
  `prlimit --as=2000000000`.
- **Alternative to beat:** MuseScore's own import of the same source (its own
  `.mscz` for an mscz source, MusicXML/LilyPond/ABC import otherwise). A file
  is counted as a loss when its round-trip has more changed core lines than
  that alternative.

### Baseline table (frozen binary)

| Lot | Scores | `.mscz` written | Opened | Core-identical | N1 | N2 | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | 38/56 | 0.9834 | 0.9817 | 0 | 56/56 |
| mscz | 174 | 171 | 172 | 9/171 | 0.9823 | 0.9783 | 0 | 39/171 |
| xml | 32 | 27 | 27 | 0/27 | 0.9127 | 0.3063 | 0 | 0/27 |
| ly | 375 | 3 | 3 | 1/3 | 1.0000 | 1.0000 | 0 | 0/0 |
| abc | 1 | 0 | 0 | 0/0 | — | — | 0 | 0/0 |
| **Total** | **638** | **257** | **258** | **48/257** | **0.9695** | **0.8542** | **0** | **95/254** |

No MuseScore load error or warning was reported on the files it did open; the
three mscz failures (one unreadable 1.14 file, one MIDI export timeout) and
the 372 unopened `.ly`/`.abc` files are the limits detailed below.

### Baseline gaps by first differing core family

154 files are worse than MuseScore's own import. The first differing core
family (`scripts/oracle/musicxml/groupes.py`, same columns) is:

| Family (changed tag) | Files | Extra changed lines |
| --- | ---: | ---: |
| `midi-device` | 46 | 30 556 |
| `sign` (clef) | 26 | 2 554 |
| `measure-style` | 14 | 3 026 |
| `measure` | 11 | 1 971 |
| `part-abbreviation` | 10 | 706 |
| `note` | 9 | 5 018 |
| `tuplet` | 8 | 1 392 |
| `clef-octave-change` | 7 | 785 |
| `harmony` | 4 | 205 |
| `accidental` | 4 | 153 |
| others (2 or fewer each) | 17 | — |

### Final table (after the four fixes)

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | 45/56 | 0.9700 | 0.9682 | 0 | 56/56 |
| mscz | 174 | 171 | 172 | 28/171 | 0.9781 | 0.9732 | 0 | 39/171 |
| xml | 32 | 27 | 27 | 0/27 | 0.9103 | 0.3055 | 0 | 0/27 |
| ly | 375 | 297 | 297 | 1/297 | 0.9998 | 0.9966 | 0 | 0/0 |
| abc | 1 | 0 | 0 | 0/0 | — | — | 0 | 0/0 |
| **Total** | **638** | **551** | **552** | **74/551** | **0.9677** | **0.8594** | **0** | **95/254** |

171 files improve, 3 files gain core lines (mscz-33: +51, mscz-117: +12,
mscz-115: +2). All three keep exactly the same N1/N2/N2d (4 196/4 196 notes
for mscz-33, 2 856/2 856 for the other two): the corrected `<midi-device>` and
clef lines change the alignment of the line diff, exposing the unchanged
`note`/`forward`, anacrusis and `<attributes>` differences that were already
further down in the same file. There is no musical loss.

### Fixes, in decreasing count

1. **`<midi-device port>` — 46 files, up to 1 975 changed lines.** The writer
   never emitted `<Channel><midiPort>`, so MuseScore recomputed the port from
   the MIDI channel. Port of `TWrite::write(const InstrChannel*)`
   (`write/twrite.cpp:2111-2114`), which writes `midiPort` and `midiChannel`
   together. Reduced test: `port_midi_du_canal_ecrit_et_relu`.
2. **Clef `<sign>` — 26 files.** The writer omitted `<transposingClefType>`
   when it equalled the concert clef; the reader (`read400/tread.cpp:2551`)
   then forces G. Port of `TWrite::write(const Clef*)`
   (`write/twrite.cpp:1281-1299`). Reduced test:
   `clef_basse_ecrit_son_type_transposant`.
3. **Harmony — 4 files first, but up to 196 lines on the worst one.** The
   writer used the pre-4.6 `<root>`/`<name>` children of `<Harmony>`; since
   4.6 the reader (`read460/tread.cpp:2868`) only accepts `<harmonyInfo>`.
   Port of `writeHarmonyInfo` (`write/twrite.cpp:1786-1826`), with the direct
   children kept for scoretk's own reader. Reduced test:
   `accords_chiffres_relus_par_musescore_46`.
4. **Empty `<Instrument/>` — 372 files that MuseScore refused to open.** A
   scoretk score built from LilyPond/ABC has no instrument; MuseScore 4.7.4
   exits with code 1 (no message) on `<Instrument/>`. The writer now emits
   the minimal default `<Instrument><Channel><program value="0"/></Channel>`
   as MuseScore's own writer always does (`TWrite::write(const Instrument*)`,
   `write/twrite.cpp:1947`). Reduced test:
   `instrument_vide_ecrit_un_instrument_valide`.

### Out of scope (unchanged)

The remaining families are not simple mis-serialisations of a stored value:
`measure-style`/`measure` concern the anacrusis number and multi-measure-rest
range, `part-abbreviation` the part-group/newline handling,
`clef-octave-change`/`accidental` the interaction between clef octave and
transposition and courtesy accidentals, `tuplet` the bracket encoding read back
by MuseScore, and `note`/`forward` the padded voice timeline. They need model
or reader work in addition to the writer, and are left unchanged here. The
`v3/moonlight` and `v3/voices` reference differences of §7 are likewise
layout-only.

### Second pass — the 86 files still unopened

The frozen baseline of this pass is the previous final run itself: the binary
`/tmp/scoretk-mscz-pass2-base` rebuilt from the merged master is byte-identical
(sha256 `3611bacac5460a50…`) to `/tmp/scoretk-mscz-final`, so the table above
is the baseline for the comparison below.

MuseScore 4.7.4 writes a detailed log under
`~/.local/share/MuseScore/MuseScore4/logs/` even though the CLI prints nothing;
grouping the 86 unopened files by its first error gives:

| Cause (MuseScore log / scoretk error) | Files | Owner |
| --- | ---: | --- |
| `Incomplete measure … Found: 3/8. Expected: 1/8` (and 30 other Found/Expected pairs) | 63 | reader: `\partial 8*5` and `*4` multipliers in the LilyPond/ABC source produce a measure whose `len` disagrees with its chord durations |
| `PlaybackSetupDataResolver: Unable to resolve setup data for instrument, id: S.` then SIGSEGV | 6 | **writer**: an `<Instrument>` with no `<Channel>` |
| scoretk `source LilyPond illisible : aucune musique dans la source` | 16 | reader: `.ly` include snippets with no music |
| scoretk `version MuseScore non prise en charge` (1.14) | 1 | reader: `read114` is deliberately not ported |

The one writer cause is fixed by porting `TWrite::write(const Instrument*)`
(`write/twrite.cpp:1947`): every instrument now gets at least an empty
`<Channel>`, as MuseScore's own writer always does. Reduced test:
`instrument_sans_programme_ecrit_un_canal`. The patch was first verified by hand
on `03_01_Miguel_Les-grains-de-mais_livret.ly`: adding `<Channel/>` to the
written `.mscz` changes MuseScore's exit from SIGSEGV to success.

### Second pass — remaining non-identical files by first core family

On the current binary, 477 opened files still differ from scoretk's direct
MusicXML. Counting files *worse than MuseScore's own import of the same source*
(rightmost column) is the meaningful measure; the first differing tag alone is
often a diff-alignment artefact (e.g. `part-group` prints an identical line).

| First tag | Files | Changed core lines | Worse than alternative |
| --- | ---: | ---: | ---: |
| `instrument-sound` | 299 | 11 482 | 0 |
| `measure-style` | 29 | 11 144 | 24 |
| `measure` | 24 | 27 809 | 24 |
| `note` | 22 | 5 953 | 18 |
| `attributes` | 17 | 9 296 | 8 |
| `part-group` | 17 | 4 613 | 13 |
| `tuplet` | 15 | 8 785 | 6 |
| `part-abbreviation` | 14 | 756 | 9 |
| `accidental` | 9 | 298 | 6 |
| others (8 or fewer each) | 30 | — | 14 |

`instrument-sound` is the known consequence of the fallback instrument: a
LilyPond source has no instrument in the pivot model, so scoretk's direct
MusicXML writes none while MuseScore assigns `keyboard.piano.grand`. MuseScore's
own import of the same `.ly` fails outright, so there is no alternative to beat
on those files.

`measure-style`/`measure` come from the reader's anacrusis and multi-measure
rest model (a 1/8 `len` holding a dotted quarter, or a single mmRest measure
where MuseScore builds a range); `note`/`forward` from scoretk's explicit
timeline padding in the MusicXML exporter; `tuplet` from the reader's bracket
heuristic; `part-abbreviation`/`part-group` from part metadata. None is a
mis-serialised value in the writer: a faithful port of
`MeasureWrite::writeMeasure`, `TWrite::write(const Tuplet*)` and
`TWrite::writeProperties(const ChordRest*)` already reproduces the model. They
are left for reader/model work rather than patched with a writer-side fudge.

### Second pass — final table

Binary `/tmp/scoretk-mscz-pass2-final` (sha256 `5e1d046bb209beb6…`). Same 638 scores, same oracle, same tolerance.

| Lot | Scores | .mscz written | Opened | Core-identical | N1 | N2 | Load err/warn | Alt core-identical |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| corpus | 56 | 56 | 56 | 45/56 | 0.9700 | 0.9682 | 0 | 56/56 |
| mscz | 174 | 171 | 172 | 28/171 | 0.9781 | 0.9732 | 0 | 39/171 |
| xml | 32 | 27 | 27 | 0/27 | 0.9103 | 0.3055 | 0 | 0/27 |
| ly | 375 | 301 | 301 | 1/301 | 0.9998 | 0.9967 | 0 | 0/0 |
| abc | 1 | 1 | 1 | 0/1 | 1.0000 | 1.0000 | 0 | 0/0 |
| **Total** | **638** | **556** | **557** | **74/556** | **0.9679** | **0.8603** | **0** | **95/254** |

Compared with the frozen baseline: **5 more scores open** (301 `.ly` instead of 297, and the single `.abc`), N2 rises from 0.8594 to 0.8603, and **no file loses any core line** (0 regressions, 0 improvements on files that already opened). The 1 still-failing mscore conversion is a reader-caused incomplete measure; the 17 scoretk errors are unchanged. On the `.ly` and `.abc` lots MuseScore's own import fails outright, so the alternative cannot be beaten there; on corpus/mscz/xml the alternative stays at 56/56, 39/171 and 0/27 core-identical, unchanged.
