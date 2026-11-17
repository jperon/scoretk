# TODO

## Feuille de route : les autres formats de scoretk

**scoretk** est une boîte à outils de conversion **bidirectionnelle** entre
cinq formats de partition ; **mscz2xml** en est le premier convertisseur
(MuseScore → MusicXML).

| Format | Lecture | Écriture |
|---|---|---|
| MuseScore (`.mscz` / `.mscx`, 3.x et 4.x) | fait | fait (`scoretk-mscx`) |
| MusicXML (`.musicxml` / `.mxl`) | fait (noyau, cf. `docs/agents/musicxml-import.md`) | fait |
| LilyPond (`.ly`) | fait (sous-ensemble, t40) | fait (`scoretk-lilypond`, t36) |
| ABC (`.abc`) | fait (`scoretk-abc`, t41) | fait (`scoretk-abc`, t41) |
| MIDI (`.mid`) | à faire | fait (`scoretk-midi`) |

Toutes les combinaisons doivent fonctionner : mscz ↔ musicxml, musicxml ↔ ly,
musicxml ↔ abc, musicxml ↔ midi, et donc aussi toutes les paires entre mscz,
ly, abc et midi.

### État de la phase 1 (un crate par convertisseur)

| Crate | Rôle | État |
| --- | --- | --- |
| `scoretk` + `scoretk-cli` | façade (`Format`, détection, `read`/`write`) et CLI `scoretk` | fait : lecture `.mscz`/`.mscx`/MusicXML/`.ly`/ABC, écriture MusicXML/`.mxl`/`.mscx`/`.mscz`/`.ly`/`.mid`/`.abc` ; les six features (`musicxml-import`, `midi`, `lilypond`, `lilypond-import`, `mscx`, `abc`) sont activées par la CLI |
| `scoretk-musicxml` | lecteur MusicXML vers le modèle pivot | fait (t34, t37) : noyau 26/30 v3 et 26/26 v4, parité complète 23/30 et 21/26 (écarts dans `docs/agents/musicxml-import.md`) |
| `scoretk-midi` | écrivain MIDI depuis le modèle pivot | fait (t35) |
| `scoretk-lilypond` | écrivain et lecteur LilyPond depuis/vers le modèle pivot | fait (t36, t40) : 56/56 fichiers compilent, aller-retour noyau 24/56 (t40) |
| `scoretk-mscx` | écrivain MuseScore (`.mscx` 4.x, `.mscz`) | fait (t38) : aller-retour identique sur tout le corpus |
| `scoretk-abc` | lecteur et écrivain ABC (`.abc`) | fait (t41) : 56/56 instantanés, aller-retour noyau 46/56, écarts dans `docs/agents/abc.md` |

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
- **LilyPond.** Écrivain et lecteur sont faits (`scoretk-lilypond`, features
  `lilypond` et `lilypond-import` de la façade) ; voir
  `docs/agents/lilypond.md`.
  - Écrivain : les voix sont notées en mode **`\fixed`**, jamais `\relative`,
    car ce mode résiste mieux aux erreurs de copier-coller et assimilées.
  - Lecteur (t40) : le parseur de LilyPond dépend de Guile Scheme ; on porte en
    Rust la grammaire de `parser.yy` et `lexer.ll` pour un sous-ensemble
    (`\relative`, `\fixed`, `\transpose`, `\tuplet`, `\repeat`, `\key`,
    `\time`, `\clef`, `\partial`, `\language`, paroles, `\markup` simple).
    Le Scheme arbitraire et les commandes inconnues sont ignorés avec un
    avertissement collecté, sur le modèle de python-ly (`ly.music`).
    Aller-retour du corpus : 24/56 fichiers identiques au noyau musical,
    3/56 à parité complète ; les écarts restants viennent surtout
    d'informations que l'écrivain ne peut pas représenter (sous-types de
    mesure, endings, canaux MIDI, accords chiffrés). Détail et procédure de
    mise à niveau : `docs/agents/lilypond.md`.
- **Contraintes inchangées.** La bibliothèque reste portable : wasm pour la PWA
  `../cantus`, Android, sans fs ni threads, et sans dépendance C.
- **Calendrier.** À entreprendre une fois la parité mscz → MusicXML atteinte.
  Le premier livrable reste mscz2xml en wasm pour cantus.
