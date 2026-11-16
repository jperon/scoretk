# TODO

## Évolution de mscz2xml vers musictk

Faire évoluer mscz2xml en **musictk**, une boîte à outils de conversion
**bidirectionnelle** entre quatre formats de partition :

| Format | Lecture | Écriture |
|---|---|---|
| MuseScore (`.mscz` / `.mscx`, 3.x et 4.x) | fait | à faire |
| MusicXML (`.musicxml` / `.mxl`) | à faire | fait |
| LilyPond (`.ly`) | à faire | à faire |
| ABC (`.abc`) | à faire | à faire |

Toutes les combinaisons doivent fonctionner : mscz ↔ musicxml, musicxml ↔ ly,
musicxml ↔ abc, et donc aussi mscz ↔ ly, mscz ↔ abc et ly ↔ abc.

Pistes :

- **Modèle pivot.** Le modèle interne (`crates/mscz2xml/src/model`) devient le
  pivot commun. Chaque format fournit un lecteur vers le modèle et un écrivain
  depuis le modèle, ce qui évite N×N convertisseurs. Il faudra sans doute le
  rendre moins spécifique à MuseScore. On peut aussi convertir via MusicXML
  quand c'est plus simple, mais au prix de pertes cumulées.
- **Organisation.** Prévoir un crate par format, ou des features, et un crate
  `musictk` qui les regroupe. La CLI déduit les formats des extensions
  (`musictk entrée.ly -o sortie.mscz`).
- **Références existantes.**
  - MusicXML → mscz : l'import MusicXML de MuseScore
    (`src/importexport/musicxml/internal/import`).
  - MusicXML → ly : `musicxml2ly` (LilyPond).
  - ly → MusicXML : `python-ly`.
  - MusicXML ↔ abc : `xml2abc` et `abc2xml` (W. Vree).
  - Vérifier les licences de chacune avant tout portage.
- **Contraintes inchangées.** La bibliothèque reste portable : wasm pour la PWA
  `../cantus`, Android, sans fs ni threads, et sans dépendance C.
- **Calendrier.** À entreprendre une fois la parité mscz → MusicXML atteinte.
  Le premier livrable reste mscz2xml en wasm pour cantus.
