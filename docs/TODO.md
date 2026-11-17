# TODO

## Feuille de route : les autres formats de scoretk

**scoretk** est une boîte à outils de conversion **bidirectionnelle** entre
cinq formats de partition ; **mscz2xml** en est le premier convertisseur
(MuseScore → MusicXML).

| Format | Lecture | Écriture |
|---|---|---|
| MuseScore (`.mscz` / `.mscx`, 3.x et 4.x) | fait | à faire |
| MusicXML (`.musicxml` / `.mxl`) | à faire | fait |
| LilyPond (`.ly`) | à faire | à faire |
| ABC (`.abc`) | à faire | à faire |
| MIDI (`.mid`) | à faire | à faire |

Toutes les combinaisons doivent fonctionner : mscz ↔ musicxml, musicxml ↔ ly,
musicxml ↔ abc, musicxml ↔ midi, et donc aussi toutes les paires entre mscz,
ly, abc et midi.

### État de la phase 1 (un crate par convertisseur)

| Crate | Rôle | État |
| --- | --- | --- |
| `scoretk` + `scoretk-cli` | façade (`Format`, détection, `read`/`write`) et CLI `scoretk` | fait : `.mscz`/`.mscx` → MusicXML/`.mxl` ; features `musicxml-import` et `midi` prêtes pour t34/t35 |
| `scoretk-musicxml` | lecteur MusicXML vers le modèle pivot | en cours (t34) |
| `scoretk-midi` | écrivain MIDI depuis le modèle pivot | en cours (t35) |

MIDI est à part : c'est un format de performance, pas de notation. L'écriture
MIDI demande de dérouler les reprises et sauts (comme `RepeatList` de
MuseScore), d'appliquer nuances, articulations et tempo. La lecture MIDI
demande une quantification (durées, voix, mesures, armure déduite), donc une
conversion approximative.

Pistes :

- **Modèle pivot.** Le modèle interne (`crates/mscz2xml/src/model`) devient le
  pivot commun. Chaque format fournit un lecteur vers le modèle et un écrivain
  depuis le modèle, ce qui évite N×N convertisseurs. Il faudra sans doute le
  rendre moins spécifique à MuseScore. On peut aussi convertir via MusicXML
  quand c'est plus simple, mais au prix de pertes cumulées.
- **Organisation.** Prévoir un crate par format, ou des features, et un crate
  `scoretk` qui les regroupe. La CLI déduit les formats des extensions
  (`scoretk entrée.ly -o sortie.mscz`).
- **Références existantes.**
  - MusicXML → mscz : l'import MusicXML de MuseScore
    (`src/importexport/musicxml/internal/import`).
  - MusicXML → ly : `musicxml2ly` (LilyPond).
  - ly → MusicXML : `python-ly`.
  - MusicXML ↔ abc : `xml2abc` et `abc2xml` (W. Vree).
  - MIDI : l'export MIDI de MuseScore (`src/importexport/midi`, `engraving/compat/midi`), son import MIDI (quantification), et la crate Rust `midly` (pure Rust, `no_std`) pour le format de fichier.
  - Vérifier les licences de chacune avant tout portage.
- **Contraintes inchangées.** La bibliothèque reste portable : wasm pour la PWA
  `../cantus`, Android, sans fs ni threads, et sans dépendance C.
- **Calendrier.** À entreprendre une fois la parité mscz → MusicXML atteinte.
  Le premier livrable reste mscz2xml en wasm pour cantus.
