# Plan de fiabilisation des conversions (dix sens, cinq formats)

Rédigé le 2026-11-19. But : pour chaque format et chaque sens, un **lot de
référence** et un **oracle indépendant de scoretk**, puis des campagnes de
correction mesurées comme celle de la lecture LilyPond (612 → 1 121 fichiers
exacts sur `~/Musique` en deux jours, grâce à une mesure automatique fichier
par fichier et sans régression tolérée).

## 1. Principes communs

1. **Un oracle externe par sens.** La référence est produite par l'outil qui
   fait foi pour le format : MuseScore 4.7.4, LilyPond, `abc2midi`/`abc2xml`.
   Jamais par scoretk lui-même (l'aller-retour seul masque les erreurs
   symétriques).
2. **Mesure graduée, du plus robuste au plus fin** :
   - N1 hauteurs (multiset par voix/portée) ;
   - N2 attaques et durées (tick, normalisées en noires) ;
   - N3 structure (mesures, armures, chiffrages, clefs, reprises) ;
   - N4 texte (paroles, titres, indications) ;
   - N5 noyau musical complet (`core_norm`), déjà en place pour MusicXML.
   Chaque campagne vise d'abord N1–N2 sur tout le lot, puis monte.
3. **Deux indicateurs suivis** : notes justes / notes de référence, et
   fichiers entièrement justes (au niveau mesuré). Plus : échecs, délais,
   vides.
4. **Garde anti-régression** : chaque fusion compare au dernier relevé ;
   aucune baisse par fichier tolérée sans explication écrite.
5. **Sécurité** : tout lancement de scoretk sous
   `timeout 10 prlimit --as=2000000000`.
6. **Lots non versionnés** (fichiers personnels, gros corpus tiers) : listes
   de chemins + références dérivées hors dépôt. Un **sous-lot versionné**
   (fichiers à licence compatible, déclarés dans `NOTICE`) alimente les tests
   `cargo test` bloquants.

## 2. Infrastructure (phase 0, préalable)

- Promouvoir l'oracle actuel (`one.sh` + `cmp.py`, hauteurs MusicXML vs MIDI
  LilyPond) en outil versionné `scripts/oracle/` : entrée = (fichier,
  référence, sens), sortie = ligne de mesure N1/N2 ; agrégation et
  comparaison de relevés (`baisses`).
- Comparateur commun **MIDI ↔ MIDI** (notes : hauteur, attaque, durée, piste)
  et **MusicXML ↔ MusicXML** (réutilise `core_norm`).
- Un relevé de base par sens, archivé (`docs/agents/releves/`, résumé
  seulement), pour suivre l'évolution.

## 3. Par format

### MuseScore (`.mscz`/`.mscx`)

| Sens | Lot de référence | Oracle | État / cible |
|---|---|---|---|
| Lecture | 56 fichiers du corpus (versionnés) ; **174 `.mscz` de `~/Musique`** ; ~150 `.mscx` des tests amont (`ref-musescore/src/importexport/musicxml/tests/data`) | export MusicXML de MuseScore 4.7.4 (`gen-references.sh`), comparé au noyau | 54/56 au noyau ; étendre aux 174 fichiers réels |
| Écriture | tout ce que scoretk sait lire (mscz, xml, ly, abc) | réouverture par MuseScore 4.7.4 **puis export MusicXML**, comparé au MusicXML direct de scoretk (fidélité, pas seulement ouverture) | 56/56 ouverts ; mesurer la fidélité |

### MusicXML

| Sens | Lot | Oracle | État / cible |
|---|---|---|---|
| Écriture | les mêmes que la lecture MuseScore | référence MuseScore 4.7.4 + validation **XSD MusicXML 4.0** | noyau quasi complet ; ajouter XSD et `~/Musique` |
| Lecture | **suite de tests MusicXML** (207 fichiers, `lilypond-ref/input/regression/musicxml`, conçue pour couvrir la norme) ; **451 `.xml` des tests amont MuseScore**, avec leur `.mscx` attendu ; 80 `.xml` + 27 `.mxl` de `~/Musique` | import puis export MusicXML par MuseScore 4.7.4, comparé au noyau ; pour les tests amont, le `.mscx` attendu | 26/30, 26/26 en aller-retour ; mesurer sur ces ~750 fichiers |

### MIDI

| Sens | Lot | Oracle | État / cible |
|---|---|---|---|
| Écriture | 174 `.mscz` ; corpus ; **paires `.ly`/`.midi` de `~/Musique`** (1 228 MIDI LilyPond) | MIDI de MuseScore 4.7.4 (pour mscz) ; MIDI de LilyPond (pour ly : teste la chaîne ly → modèle → MIDI) ; N2 notes | 7/21 identiques ; écarts = `gateTime`, articulations |
| Lecture | **tests amont MuseScore** (`importexport/midi/tests`, 118 `.mid` + `.mscx` attendus) ; 1 228 MIDI de `~/Musique` (dont on a la source `.ly`) | `.mscx` attendu de MuseScore ; et pour les MIDI LilyPond : hauteurs/attaques de la source (N1–N2 après quantification) | 4/21 ; mesurer sur ~1 350 fichiers |

### LilyPond (`.ly`)

| Sens | Lot | Oracle | État / cible |
|---|---|---|---|
| Lecture | **1 447 `.ly` de `~/Musique`** ; **1 693 fichiers de `lilypond-ref/input/regression`** | MIDI de LilyPond (N1 en place ; ajouter N2 attaques/durées et N4 paroles) | 98,8 % des notes, 1 121 fichiers exacts ; `regression` : 84 % |
| Écriture | 174 `.mscz`, corpus 56, `.xml` ; en aller-retour les 1 447 `.ly` | compilation **LilyPond sans erreur**, puis MIDI de LilyPond comparé au MIDI de scoretk (N2) ; et au MIDI de la source | 56/56 compilent, noyau 24/56 |

### ABC

| Sens | Lot | Oracle | État / cible |
|---|---|---|---|
| Lecture | à constituer : **tests de `abc2ly`** (`lilypond-ref/input/regression/abc2ly`, 8) ; exemples de la norme 2.1 ; un **recueil public** (The Session, abcnotation.com, Nottingham Music Database — licence à vérifier avant tout téléchargement, accord de l'utilisateur requis) | `abc2midi` (abcMIDI, nixpkgs) → MIDI, N1–N2 ; `abc2xml.py` pour N3 | aucune mesure réelle aujourd'hui |
| Écriture | tous les autres lots (mscz, xml, ly) | `abc2midi` sur la sortie de scoretk vs MIDI de scoretk / de la référence ; `abcm2ps` sans erreur | 46/56 en aller-retour |

## 4. Phases et ordre proposé

Ordre guidé par l'usage (cantus lit surtout mscz/ly/xml) et par le rendement
attendu des lots déjà disponibles :

1. **Phase 0 — outillage** (§ 2). Rapide, débloque tout le reste.
2. **Phase 1 — relevés de base** des dix sens, sans correction : un tableau
   chiffré complet, qui remplace les chiffres disparates d'aujourd'hui.
3. **Phase 2 — MusicXML lecture** sur la suite de tests et les tests amont
   (format pivot de cantus, ~750 fichiers d'oracle déjà prêts).
4. **Phase 3 — LilyPond** : finir la lecture (N2, paroles, `\chordmode`,
   `\partCombine`, `\tag`, 91 fichiers vides du corpus `regression`), puis
   l'écriture mesurée par compilation + MIDI.
5. **Phase 4 — MIDI** : lecture sur les 1 228 MIDI LilyPond (source connue :
   oracle idéal de quantification) et les tests amont ; écriture sur paires
   ly/midi.
6. **Phase 5 — MuseScore** : lecture des 174 fichiers réels, fidélité de
   l'écriture par réouverture.
7. **Phase 6 — ABC** : constituer le lot (avec accord), puis les deux sens.

Chaque phase suit la boucle de la campagne LilyPond : relevé → regroupement
des écarts par cause → une tâche par cause (fichier réduit + test) → mesure
complète → fusion si aucune baisse → reconstruction wasm/cantus.

## 5. Points à décider

- Téléchargement d'un corpus ABC public (licence, volume).
- Versionner un sous-lot de `~/Musique` (fichiers personnels) ou seulement
  des listes de chemins.
- Cible N5 (noyau complet) ou N2 suffisant pour les formats de performance
  (MIDI) et les formats « légers » (ABC).
