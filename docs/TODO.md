# TODO

## Feuille de route : les autres formats de scoretk

**scoretk** est une boîte à outils de conversion **bidirectionnelle** entre
cinq formats de partition ; **mscz2xml** en est le premier convertisseur
(MuseScore → MusicXML).

| Format | Lecture | Écriture |
|---|---|---|
| MuseScore (`.mscz` / `.mscx`, 3.x et 4.x) | fait | fait (`scoretk-mscx`) |
| MusicXML (`.musicxml` / `.mxl`) | fait (noyau, cf. `docs/agents/musicxml-import.md`) | fait |
| LilyPond (`.ly`) | à faire | fait (`scoretk-lilypond`, t36) |
| ABC (`.abc`) | à faire | à faire |
| MIDI (`.mid`) | à faire | fait (`scoretk-midi`) |

Toutes les combinaisons doivent fonctionner : mscz ↔ musicxml, musicxml ↔ ly,
musicxml ↔ abc, musicxml ↔ midi, et donc aussi toutes les paires entre mscz,
ly, abc et midi.

### État de la phase 1 (un crate par convertisseur)

| Crate | Rôle | État |
| --- | --- | --- |
| `scoretk` + `scoretk-cli` | façade (`Format`, détection, `read`/`write`) et CLI `scoretk` | fait : lecture `.mscz`/`.mscx`/MusicXML, écriture MusicXML/`.mxl`/`.mscx`/`.mscz`/`.ly`/`.mid` ; les quatre features (`musicxml-import`, `midi`, `lilypond`, `mscx`) sont activées par la CLI |
| `scoretk-musicxml` | lecteur MusicXML vers le modèle pivot | fait (t34, t37) : noyau 26/30 v3 et 26/26 v4, parité complète 23/30 et 21/26 (écarts dans `docs/agents/musicxml-import.md`) |
| `scoretk-midi` | écrivain MIDI depuis le modèle pivot | fait (t35) |
| `scoretk-lilypond` | écrivain LilyPond depuis le modèle pivot | fait (t36) : 56/56 fichiers du corpus compilent |
| `scoretk-mscx` | écrivain MuseScore (`.mscx` 4.x, `.mscz`) | fait (t38) : aller-retour identique sur tout le corpus |

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
- **LilyPond.** L'écrivain vient avant le lecteur : ses sorties servent de
  corpus d'aller-retour au lecteur. L'écrivain est fait (`scoretk-lilypond`,
  feature `lilypond` de la façade) ; voir `docs/agents/lilypond.md`.
  - Écrivain : les voix sont notées en mode **`\fixed`**, jamais `\relative`,
    car ce mode résiste mieux aux erreurs de copier-coller et assimilées.
  - Lecteur : on ne peut pas intégrer le parseur de LilyPond, qui dépend de
    Guile Scheme. On porte en Rust la grammaire de `parser.yy` et `lexer.ll`
    pour un sous-ensemble : `\relative`, `\fixed`, `\transpose`, `\tuplet`,
    `\repeat`, `\key`, `\time`, `\clef`, `\partial`, `\language`, paroles et
    `\markup` simple. Le Scheme arbitraire est ignoré avec un avertissement,
    sur le modèle de python-ly (`ly.music`).
- **Contraintes inchangées.** La bibliothèque reste portable : wasm pour la PWA
  `../cantus`, Android, sans fs ni threads, et sans dépendance C.
- **Calendrier.** À entreprendre une fois la parité mscz → MusicXML atteinte.
  Le premier livrable reste mscz2xml en wasm pour cantus.
