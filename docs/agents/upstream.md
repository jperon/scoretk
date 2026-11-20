# Amont MuseScore Studio : référence et suivi

Ce document décrit la version de MuseScore Studio dont le code est porté, la
façon d'en obtenir une copie locale, la correspondance entre les fichiers C++
et les fichiers Rust, et la procédure à suivre pour absorber une nouvelle
version amont.

## 1. Référence actuelle

| Élément | Valeur |
| --- | --- |
| Projet | **MuseScore Studio**, <https://github.com/musescore/MuseScore> |
| Commit porté | `2b9e82df847111d673c2051a93deeaaeea3f5a29` (2026-11-16) |
| `version.cmake` | version **5.0.0** (`MUSE_APP_UNSTABLE ON`, `MUSE_APP_IS_PRERELEASE ON`) |
| Copie locale | `ref-musescore/`, **non versionnée** (`.gitignore`), en lecture seule |
| Licence amont | GNU GPL version 3 (`LICENSE.txt`), Copyright (C) 1999-2026 MuseScore Limited and others |

Vérifications faites le 2026-11-17 : `NOTICE`, `tests/corpus/SOURCES.md` et le
`version.cmake` du commit amont concordent. La copie `ref-musescore/` n'existe
pas dans une extraction fraîche : elle se recrée avec la procédure ci-dessous.

Deux références coexistent, et il ne faut pas les confondre :

- **source portée** : le snapshot amont 5.0.0 ci-dessus, pour la structure du
  code et les noms (`read*.cpp`, `exportmusicxml.cpp`, `engraving/dom/…`) ;
- **référence de comportement** : **MuseScore 4.7.4** (paquet nixpkgs), qui a
  produit les `*.ref.musicxml` du corpus. `DEFAULT_SOFTWARE` annonce cette
  version, et la date d'encodage par défaut est figée pour la reproductibilité.

En cas de divergence entre le code amont 5.0.0 et la sortie 4.7.4, **la
référence 4.7.4 gagne** tant qu'une décision explicite n'a pas été prise de
changer de version de référence (voir § 4).

### Données dérivées

- **Références du corpus** — `tests/corpus/{v3,v4}/*.ref.musicxml`, produites
  par `scripts/gen-references.sh` avec MuseScore **4.7.4** de nixpkgs, lancé
  en mode headless (`QT_QPA_PLATFORM=offscreen`, `LD_LIBRARY_PATH` neutralisé).
  Les sources `.mscx` viennent du même commit amont (voir
  `tests/corpus/SOURCES.md`), principalement de
  `src/importexport/musicxml/tests/data/` et `src/engraving/tests/`.
- **Table des instruments** — `crates/scoretk-musescore-import/src/read/instruments.rs`,
  **fichier généré** par `scripts/gen-instruments-table.py` à partir de
  `share/instruments/instruments.xml`. Le fichier généré et `NOTICE`
  mentionnent la version **4.4** du catalogue ; ne pas modifier la table à la
  main, la régénérer.
- **Tableaux de noms** — `read302/tables.rs` et `read4xx/names.rs` reprennent
  `TConv::fromXml` (`engraving/rw/types/typesconv.cpp`, fichier **généré à la
  construction** chez MuseScore, donc absent de la copie de référence) et les
  valeurs de `engraving/types/types.h`.

## 2. Obtenir `ref-musescore/`

Clone complet au commit exact (plus simple, plusieurs centaines de Mo) :

```sh
git clone https://github.com/musescore/MuseScore ref-musescore
git -C ref-musescore checkout 2b9e82df847111d673c2051a93deeaaeea3f5a29
git -C ref-musescore rev-parse HEAD   # doit afficher le commit ci-dessus
```

Clone partiel, suffisant pour les sources et le corpus (méthode utilisée pour
constituer `tests/corpus/`) :

```sh
git clone --depth 1 --filter=blob:none --sparse \
    https://github.com/musescore/MuseScore ref-musescore
git -C ref-musescore sparse-checkout set \
    src/importexport/musicxml/tests/data \
    src/engraving/tests \
    src/engraving/rw src/engraving/dom src/engraving/types \
    src/engraving/style src/engraving/rendering src/engraving/infrastructure \
    src/importexport/musicxml/internal share/instruments
git -C ref-musescore fetch --depth 1 origin 2b9e82df847111d673c2051a93deeaaeea3f5a29
git -C ref-musescore checkout 2b9e82df847111d673c2051a93deeaaeea3f5a29
```

`ref-musescore/` est ignoré par Git : ne jamais l'ajouter au dépôt, ne jamais
copier de fichier C++ dans les sources distribuées (le portage est une
réécriture, voir `NOTICE`).

## 3. Correspondance C++ → Rust

Chemins amont relatifs à `ref-musescore/src/`. La colonne Rust est relative à la
racine du dépôt. Cette table est dressée à partir des en-têtes
« Portage de … » des fichiers Rust : **mettre à jour l'en-tête et cette table**
à chaque nouveau portage.

### Racine, conteneur et aiguillage

| Rust | C++ amont |
| --- | --- |
| `crates/scoretk-musicxml-export/src/lib.rs` | `importexport/musicxml/internal/export/` |
| `crates/scoretk-musescore-import/src/container.rs` | `engraving/infrastructure/mscreader.cpp` |
| `crates/scoretk-musescore-import/src/read/mod.rs` | `engraving/rw/rwregister.cpp` |
| `crates/scoretk-musescore-import/src/read/instruments.rs` | `engraving/dom/instrument.cpp` (`updateInstrumentId`, `recognizeMidiProgram`) et `share/instruments/instruments.xml` |

### Modèle (`crates/scoretk-core/src/model/`)

| Rust | C++ amont |
| --- | --- |
| `crates/scoretk-core/src/model/mod.rs` | `engraving/dom/` |
| `crates/scoretk-core/src/model/annotation.rs` | `engraving/dom/{segment,dynamic,stafftext,systemtext,tempotext,rehearsalmark,instrument,harmony}.h` |
| `crates/scoretk-core/src/model/chord.rs` | `engraving/dom/{chordrest,chord,rest,articulation}.h` |
| `crates/scoretk-core/src/model/duration.rs` | `engraving/dom/durationtype.h`, `engraving/types/types.h` |
| `crates/scoretk-core/src/model/fraction.rs` | `engraving/types/fraction.h` |
| `crates/scoretk-core/src/model/hairpin.rs` | `engraving/dom/hairpin.h` |
| `crates/scoretk-core/src/model/ids.rs` | `engraving/dom/{segment,score}.h` |
| `crates/scoretk-core/src/model/lyrics.rs` | `engraving/dom/lyrics.h`, `engraving/types/types.h` |
| `crates/scoretk-core/src/model/measure.rs` | `engraving/dom/measure.h` |
| `crates/scoretk-core/src/model/notation.rs` | `engraving/dom/{fermata,ornament,tremolosinglechord,tremolotwochord,arpeggio,glissando,chordline,fingering}.h` |
| `crates/scoretk-core/src/model/note.rs` | `engraving/dom/{note,tie,pitchspelling}.h` |
| `crates/scoretk-core/src/model/part.rs` | `engraving/dom/{part,staff,instrument}.h` |
| `crates/scoretk-core/src/model/repeat.rs` | `engraving/dom/{marker,jump,volta}.h` |
| `crates/scoretk-core/src/model/score.rs` | `engraving/dom/score.h` |
| `crates/scoretk-core/src/model/segment.rs` | `engraving/dom/{segment,keysig,timesig,clef,barline}.h` |
| `crates/scoretk-core/src/model/spanner.rs` | `engraving/dom/spanner.h` |
| `crates/scoretk-core/src/model/style.rs` | `engraving/style/style.h`, `importexport/musicxml/internal/export/exportmusicxml.cpp` |
| `crates/scoretk-core/src/model/text.rs` | `engraving/dom/textbase.cpp`, `importexport/musicxml/internal/shared/musicxmlfonthandler.cpp`, `…/exportmusicxml.cpp` |
| `crates/scoretk-core/src/model/tuplet.rs` | `engraving/dom/tuplet.h` |

### Lecteur 2.x (`crates/scoretk-musescore-import/src/read/read206/`)

Le format n'a pas de spécification publiée : la syntaxe suit `read206.cpp`, et
la référence de comportement reste MuseScore 4.7.4 (§1). `Read206` délègue la
lecture des éléments au lecteur commun `read400` ; le portage réécrit donc les
conventions d'ensemble (voix `<track>`/`<tick>`, `<Tie id>`/`<endSpanner id>`,
`<Tuplet id>`) avant de confier l'arbre à `read302`.

| Rust | C++ amont |
| --- | --- |
| `crates/scoretk-musescore-import/src/read206/mod.rs` | `engraving/rw/read206/read206.cpp` (`Read206::readScoreFile`, `readScoreTag`, `Score::connectTies`) |
| `crates/scoretk-musescore-import/src/read206/transform.rs` | `read206.cpp` (`readStaffContent206`, `readMeasure206`, `readChord`, `readNote206`, `readTuplet206`, `readTextStyle206`) et `rw/rwregister.cpp` |
| `crates/scoretk-musescore-import/src/read206/spanners.rs` | `read206.cpp` (`readVolta206`, `readSlur206`, `readHairpin206`, `readOttava`, `readMeasure206` pairing) et `dom/spanner.h` |
| `crates/scoretk-musescore-import/src/read206/xml.rs` | `engraving/rw/xmlreader.cpp` (lecture des attributs et du contenu mixte) |

`src/engraving/rw/read114/` n'a été consulté que pour comprendre les seuils de
version de `rwregister.cpp` ; il n'est pas porté (versions `<= 114` non prises
en charge).

### Lecteurs 3.x (`crates/scoretk-musescore-import/src/read/read302/`)

| Rust | C++ amont |
| --- | --- |
| `crates/scoretk-musescore-import/src/read302/mod.rs` | `engraving/rw/read302/read302.cpp`, `engraving/rw/read400/` |
| `crates/scoretk-musescore-import/src/read302/annotations.rs` | `engraving/rw/read400/tread.cpp` |
| `crates/scoretk-musescore-import/src/read302/chord.rs` | `engraving/rw/read400/tread.cpp` |
| `crates/scoretk-musescore-import/src/read302/credits.rs` | `engraving/rw/read302/read302.cpp`, `read400/tread.cpp` |
| `crates/scoretk-musescore-import/src/read302/measure.rs` | `engraving/rw/read400/{staffrw,measurerw}.cpp` |
| `crates/scoretk-musescore-import/src/read302/notations.rs` | `engraving/rw/read400/tread.cpp` |
| `crates/scoretk-musescore-import/src/read302/nuances.rs` | `engraving/rw/read400/tread.cpp` |
| `crates/scoretk-musescore-import/src/read302/part.rs` | `engraving/rw/read400/tread.cpp`, `read302/read302.cpp` |
| `crates/scoretk-musescore-import/src/read302/repeats.rs` | `engraving/rw/read400/tread.cpp` |
| `crates/scoretk-musescore-import/src/read302/spanner.rs` | `engraving/rw/read400/connectorinforeader.cpp` |
| `crates/scoretk-musescore-import/src/read302/tables.rs` | `engraving/types/types.h`, `engraving/rw/` (tables de noms) |
| `crates/scoretk-musescore-import/src/read302/textlines.rs` | `engraving/rw/read400/tread.cpp` |

### Lecteurs 4.x/5.x (`crates/scoretk-musescore-import/src/read/read4xx/`)

| Rust | C++ amont |
| --- | --- |
| `crates/scoretk-musescore-import/src/read4xx/mod.rs` | `engraving/rw/read400/read400.cpp`, `read410/read410.cpp`, `read460/read460.cpp`, `read500/read500.cpp` |
| `crates/scoretk-musescore-import/src/read4xx/names.rs` | `engraving/types/types.h`, `engraving/rw/` (tables de noms) |
| `crates/scoretk-musescore-import/src/read4xx/notations.rs` | `engraving/rw/read400/tread.cpp` |
| `crates/scoretk-musescore-import/src/read4xx/nuances.rs` | `engraving/rw/read410/tread.cpp` |
| `crates/scoretk-musescore-import/src/read4xx/ottava.rs` | `engraving/rw/read400/connectorinforeader.cpp` |
| `crates/scoretk-musescore-import/src/read4xx/pedals.rs` | `engraving/rw/read400/{connectorinforeader,tread}.cpp` |
| `crates/scoretk-musescore-import/src/read4xx/repeats.rs` | `engraving/rw/read400/{tread,connectorinforeader}.cpp` |
| `crates/scoretk-musescore-import/src/read4xx/style.rs` | `engraving/rw/compat/readstyle.cpp` |
| `crates/scoretk-musescore-import/src/read4xx/textlines.rs` | `engraving/rw/read400/{connectorinforeader,tread}.cpp` |

### Export (`crates/scoretk-musicxml-export/src/`)

| Rust | C++ amont |
| --- | --- |
| `crates/scoretk-musicxml-export/src/lib.rs` | `importexport/musicxml/internal/export/` |
| `crates/scoretk-musicxml-export/src/exportmusicxml.rs` | `importexport/musicxml/internal/export/exportmusicxml.cpp`, `importexport/musicxml/internal/shared/` |
| `crates/scoretk-musicxml-export/src/beams.rs` | `engraving/rendering/score/beamlayout.cpp`, `engraving/dom/groups.cpp`, `…/exportmusicxml.cpp` |
| `crates/scoretk-musicxml-export/src/harmony.rs` | `…/exportmusicxml.cpp`, `engraving/dom/chordlist.cpp` |
| `crates/scoretk-musicxml-export/src/layout.rs` | `…/exportmusicxml.cpp` (`defaults`, `writePageFormat`, `textAsCreditWords`) |
| `crates/scoretk-musicxml-export/src/notations.rs` | `…/exportmusicxml.cpp` (`chordAttributes`, `fermata`, `symIdToArtic`) |
| `crates/scoretk-musicxml-export/src/nuances.rs` | `…/exportmusicxml.cpp` (`dynamic`, `hairpin`) |
| `crates/scoretk-musicxml-export/src/ottava.rs` | `…/exportmusicxml.cpp` (`ottava`) |
| `crates/scoretk-musicxml-export/src/pedals.rs` | `…/exportmusicxml.cpp`, `importexport/musicxml/internal/shared/` |
| `crates/scoretk-musicxml-export/src/pitch.rs` | `…/exportmusicxml.cpp` (`tpc2stepName`), `engraving/dom/pitchspelling.cpp` |
| `crates/scoretk-musicxml-export/src/repeats.rs` | `…/exportmusicxml.cpp` (`barlineLeft/Middle/Right`, marqueurs, sauts) |
| `crates/scoretk-musicxml-export/src/stems.rs` | `engraving/rendering/score/chordlayout.cpp` |
| `crates/scoretk-musicxml-export/src/text.rs` | `…/exportmusicxml.cpp`, `…/shared/musicxmlfonthandler.cpp`, `engraving/dom/tempotext.cpp` |
| `crates/scoretk-musicxml-export/src/textlines.rs` | `…/exportmusicxml.cpp` (`textLine`, `writeHairpinText`) |
| `crates/scoretk-musicxml-export/src/writer.rs` | `engraving/rw/xmlwriter.cpp` |

### Lecteur MusicXML (`crates/scoretk-musicxml-import/`)

Cette crate est hors de `crates/scoretk-core` mais porte le même code amont ; les
chemins Rust y sont relatifs à `crates/scoretk-musicxml-import/src/`.

| Rust | C++ amont |
| --- | --- |
| `pass1.rs` | `importexport/musicxml/internal/import/importmusicxmlpass1.cpp`, `musicxmlpart.cpp`, `musicxmlvoicedesc.cpp` |
| `pass2.rs` | `importexport/musicxml/internal/import/importmusicxmlpass2.cpp`, `importmusicxmlnotepitch.cpp`, `musicxmltupletstate.cpp` |
| `noteduration.rs` | `importexport/musicxml/internal/import/importmusicxmlnoteduration.cpp` |
| `container.rs` | `importexport/musicxml/internal/import/importmusicxml.cpp` (lecture `.mxl`) |
| `xml.rs` | remplace `muse::XmlStreamReader` par un arbre en mémoire (fichier original) |

Les écarts de cet import sont suivis dans `docs/agents/musicxml-import.md`.

### Écriture MuseScore (`crates/scoretk-musescore-export/src/`)

Cette crate est hors de `crates/scoretk-core` mais porte le même code amont ; les
chemins Rust y sont relatifs à `crates/scoretk-musescore-export/src/`.

| Rust | C++ amont |
| --- | --- |
| `write/writer.rs` | `engraving/rw/write/writer.cpp` (`Writer::writeScore`) |
| `write/staffwrite.rs` | `engraving/rw/write/staffwrite.cpp`, `engraving/dom/` (parties, portées, cadres, crédits) |
| `write/measurewrite.rs` | `engraving/rw/write/measurewrite.cpp` (`MeasureWrite::writeMeasure`) |
| `write/twrite.rs` | `engraving/rw/write/twrite.cpp` (`TWrite::writeItem`, `writeProperties`) |
| `write/connectorinfowriter.rs` | `engraving/rw/write/connectorinfowriter.cpp` |
| `write/context.rs` | `engraving/rw/write/writecontext.h` |
| `xmlwriter.rs` | `engraving/rw/xmlwriter.cpp` |
| `names.rs` | `engraving/types/types.h`, `engraving/rw/` (tables `TConv::toXml`) |
| `text.rs` | `engraving/dom/textbase.cpp` (écriture du balisage) |
| `container.rs` | `engraving/infrastructure/mscwriter.cpp` |

### MIDI (`crates/scoretk-midi-common/src/`, `crates/scoretk-midi-import/src/`, `crates/scoretk-midi-export/src/`)

Écriture (t35) :

| Rust | C++ amont |
| --- | --- |
| `scoretk-midi-export/src/lib.rs` | `importexport/midi/internal/midiexport/exportmidi.cpp`, `engraving/compat/midi/compatmidirender.cpp` |
| `scoretk-midi-export/src/repeat_list.rs` | `engraving/dom/repeatlist.cpp` |
| `scoretk-midi-export/src/render.rs` | `engraving/compat/midi/{compatmidirender,compatmidirenderinternal}.cpp` |
| `scoretk-midi-export/src/velocity.rs` | `engraving/compat/midi/velocitymap.cpp` |
| `scoretk-midi-export/src/pause_map.rs` | `engraving/compat/midi/pausemap.cpp` |
| `scoretk-midi-export/src/writer.rs` | écriture `MidiFile` (format SMF, `no_std`) |

Lecture (t42, `import/`) :

| Rust | C++ amont |
| --- | --- |
| `scoretk-midi-import/src/lib.rs` | `importexport/midi/internal/midiimport/importmidi.cpp` |
| `scoretk-midi-import/src/midifile.rs` | `importexport/midi/internal/midishared/{midifile,midievent}.cpp` |
| `scoretk-midi-import/src/timesigmap.rs` | `engraving/dom/sig.cpp` |
| `scoretk-midi-import/src/chord.rs` | `importexport/midi/internal/midiimport/importmidi_chord.cpp` |
| `scoretk-midi-import/src/quant.rs` | `importexport/midi/internal/midiimport/importmidi_quant.cpp` |
| `scoretk-midi-import/src/meter.rs` | `importexport/midi/internal/midiimport/importmidi_meter.cpp`, `engraving/dom/durationtype.cpp` |
| `scoretk-midi-import/src/simplify.rs` | `importexport/midi/internal/midiimport/importmidi_simplify.cpp` |
| `scoretk-midi-import/src/tuplet.rs` | `importexport/midi/internal/midiimport/importmidi_tuplet*.cpp` |
| `scoretk-midi-import/src/voice.rs` | `importexport/midi/internal/midiimport/importmidi_voice.cpp` |
| `scoretk-midi-import/src/key.rs` | `importexport/midi/internal/midiimport/importmidi_key.cpp` |
| `scoretk-midi-import/src/clef.rs` | `importexport/midi/internal/midiimport/importmidi_clef.cpp` |
| `scoretk-midi-import/src/lrhand.rs` | `importexport/midi/internal/midiimport/importmidi_lrhand.cpp` |
| `scoretk-midi-import/src/drum.rs` | `importexport/midi/internal/midiimport/importmidi_drum.cpp` |
| `scoretk-midi-import/src/lyrics.rs` | `importexport/midi/internal/midiimport/importmidi_lyrics.cpp` |
| `scoretk-midi-import/src/tempo.rs` | `importexport/midi/internal/midiimport/importmidi_tempo.cpp` |
| `scoretk-midi-import/src/instrument.rs` | `importexport/midi/internal/midiimport/importmidi_instrument.cpp`, `midishared/generalmidi.cpp` |
| `scoretk-midi-import/src/operations.rs` | `importexport/midi/internal/midiimport/importmidi_operations.h` |

### Écriture LilyPond (`crates/scoretk-lilypond-export/src/`)

Cette crate n'a **pas** de source MuseScore dédiée : MuseScore 4 n'a pas
d'export LilyPond. Elle porte la sémantique des objets du modèle et suit, pour
les correspondances de notation, le comportement de **`musicxml2ly`** du
paquet LilyPond (GPL-3.0-or-later), cité dans `NOTICE` et dans les en-têtes.
La **lecture** (t40) porte en plus un sous-ensemble de `lily/parser.yy`,
`lily/lexer.ll`, `lily/pitch.cc` et `scm/define-note-names.scm` de LilyPond
2.26.0 (copie hors dépôt) ; les correspondances complètes et les procédures de
mise à niveau de ces moteurs (LilyPond, `musicxml2ly`, `python-ly`) sont dans
`docs/agents/lilypond.md` § 8 à § 10.

| Rust | Référence |
| --- | --- |
| `chord.rs` | `engraving/dom/{chordrest,chord,rest,articulation}.h` ; table d'articulations et d'ornements de `musicxml2ly` |
| `structure.rs` | `engraving/dom/{volta,measure}.h`, `ExportMusicXml::barlineRight` |
| `pitch.rs` | `engraving/dom/pitchspelling.cpp` (`tpc2stepName`, `tpc2alter`), `engraving/dom/clef.h` |
| `voice.rs` | `engraving/dom/{segment,measure,chordrest}.h` (parcours d'une voix) |
| `scoretk-midi-export/src/writer.rs` | `engraving/dom/{part,staff,score}.h` (regroupements et en-tête) |

## 4. Suivi d'une nouvelle version amont

Préparer un lot par famille (conteneur, lecteur 3.x, lecteur 4.x, modèle,
export) et un commit par lot. Ne jamais mélanger le portage d'un diff amont
avec une correction de parité indépendante.

1. **Récupérer et identifier.**

   ```sh
   git -C ref-musescore fetch origin
   git -C ref-musescore log -1 --format='%H %ad %s' origin/master
   git -C ref-musescore show origin/master:version.cmake | grep MUSE_APP_VERSION
   ```

   Noter l'ancien commit (`2b9e82d…` ou celui du dernier suivi) et le nouveau.

2. **Lister les changements des seuls chemins portés.** Les chemins exacts sont
   ceux du § 3 ; pour un premier survol, restreindre aux répertoires portés :

   ```sh
   git -C ref-musescore log --oneline <ancien>..<nouveau> -- \
     src/importexport/musicxml/internal/export \
     src/importexport/musicxml/internal/shared \
     src/importexport/musicxml/tests/data \
     src/engraving/rw src/engraving/dom src/engraving/types \
     src/engraving/style src/engraving/rendering/score \
     src/engraving/infrastructure/mscreader.cpp
   git -C ref-musescore diff --stat <ancien>..<nouveau> -- <chemins>
   git -C ref-musescore diff <ancien>..<nouveau> -- <fichier>
   ```

   `src/engraving/dom/` est vaste : filtrer ensuite par fichier de la table
   (§ 3) pour ne pas lire le bruit (interface graphique, playback, layout non
   porté). Les tables de noms (`typesconv.cpp`) sont **générées à la
   construction** chez MuseScore et absentes de la copie : vérifier alors
   `engraving/types/types.h` et les valeurs de `TConv::fromXml`.

3. **Trier.** Écarter ce qui ne concerne pas le périmètre (UI, playback,
   engraving non porté). Pour chaque changement retenu, noter le fichier C++,
   le comportement observable et le fichier Rust à modifier.

4. **Régénérer les données dérivées** avec la version cible :

   ```sh
   # Table des instruments (depuis la copie amont)
   python3 scripts/gen-instruments-table.py \
     ref-musescore/share/instruments/instruments.xml \
     crates/scoretk-musescore-import/src/read/instruments.rs

   # Références du corpus (MuseScore 4.7.4 par défaut, headless)
   scripts/gen-references.sh            # tout le corpus
   scripts/gen-references.sh tests/corpus/v4/foo.mscx   # un fichier
   ```

   Ne changer la version de MuseScore qui produit les références qu'après une
   décision explicite : cela change la cible de conformité, et impose de
   relancer tout le corpus et de mettre à jour `NOTICE`, `tests/corpus/SOURCES.md`,
   `DEFAULT_SOFTWARE` (`export/exportmusicxml.rs`) et les documents d'écarts.

5. **Porter les diffs fichier par fichier**, dans l'ordre lecteur → modèle →
   export, en respectant les conventions de `docs/agents/developpement.md` :
   nouveau code dans de nouveaux fichiers de la famille, en-tête « Portage de »
   actualisé, commentaires citant les noms C++ (`TRead::read(Chord*)`,
   `ExportMusicXml::chord`, …). Ne pas réécrire de mémoire : relire le C++ et
   les tests amont.

6. **Vérifier.** La conformité de bout en bout ne doit pas régresser :

   ```sh
   nix develop -c cargo test --workspace
   nix develop -c cargo test -p scoretk --test e2e_corpus -- --nocapture
   nix flake check
   ```

   Le test e2e a deux niveaux (noyau musical, puis parité complète) ; lire le
   tableau et `docs/agents/ecarts-{noyau,complets}.md`. Un fichier qui devient
   conforme s'ajoute à `EXPECTED_CONFORMANT` **et** se documente ; un écart
   nouveau se justifie ou se corrige.

7. **Mettre à jour la traçabilité** : `NOTICE` (nouveau commit, date, version,
   éventuelles nouvelles sources ou données), le présent document (référence,
   table C++ → Rust), les en-têtes des fichiers Rust touchés, et
   `tests/corpus/SOURCES.md` si le corpus change.

**Critère d'acceptation d'un suivi** : `cargo test --workspace` et
`nix flake check` passent, et la conformité e2e n'est pas dégradée par rapport
à l'état antérieur. Toute dégradation doit être corrigée dans le lot, ou
documentée et validée explicitement avant fusion.
