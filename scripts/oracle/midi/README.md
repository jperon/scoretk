# Oracle MIDI (phase 0-1 du plan de fiabilisation)

Outillage de mesure **indépendant de scoretk**, pour les deux sens MIDI :

* **lecture** (MIDI → MusicXML) : l'oracle est le MIDI lui-même. On compare les
  hauteurs (N1) et les attaques quantifiées (N2) du MIDI à ceux du MusicXML
  produit, avec une tolérance de ±1/16 de noire et une grille Q16 (double
  croche), qui sont les réglages par défaut de l'import de MuseScore.
  L'alternative mesurée sur le même lot est l'import MIDI de MuseScore 4.7.4.
* **écriture** (partition → MIDI) : comparateur **MIDI ↔ MIDI** commun
  (hauteur, attaque, durée, piste). La référence est le MIDI de LilyPond pour
  les sources `.ly`, et le MIDI exporté par MuseScore 4.7.4 pour les sources
  `.mscx`/`.mscz`.
* **robustesse** : corpus Lakh sans source appariée ; on relève seulement
  échecs, délais, sorties vides et tailles.

Aucune donnée tierce n'est versionnée ici : les manifestes et les sorties
brutes sont écrits dans un dossier hors dépôt. Aucun binaire n'est lancé sans
`timeout` et `prlimit --as=2000000000`.

## Emploi

```sh
nix develop -c cargo build -p scoretk-cli
OUT=/tmp/chemin/hors/depot
scripts/oracle/midi/lots.sh "$OUT"
scripts/oracle/midi/mesure.sh "$OUT" target/debug/scoretk mscore
```

`lots.sh` accepte `LAKH_STRIDE=N` pour ne retenir qu'un fichier sur N du corpus
de robustesse : les imports Lakh denses dépassent souvent la borne de 10 s, ce
qui rend le lot complet prohibitif (le taux de dépassement fait partie du
relevé). Le pas utilisé pour un relevé doit être indiqué dans `midi.md`.

`mesure.sh` ajoute `python3Packages.mido` (absent du shell Nix du projet) et
transmet les arguments à `mesure.py`. Options utiles : `--limit N` (essai
rapide sur les N premiers fichiers de chaque lot), `--alt-stride N` (ne lancer
MuseScore que sur un fichier sur N ; scoretk reste mesuré sur tout le lot —
utile quand la machine est chargée), `--resume` (régénère le résumé à partir
des `.jsonl` déjà produits sans relancer les conversions).

Sorties dans `$OUT` : un `.jsonl` par lot (une ligne de mesure par fichier et
par outil), les fichiers convertis sous `travail/`, et `resume.md` (tableaux et
dix premières causes d'écart). Le résumé versé dans `docs/agents/midi.md` est
une mise en forme de `resume.md`.

## Interprétation

Un fichier est « exact » quand le nombre de notes, le multiset des hauteurs,
les attaques et les durées correspondent tous (N1 = N2 = N2d = référence).
Les pourcentages N1/N2 sont pondérés par le nombre de notes de référence, pour
ne pas donner le même poids à un court et à un long morceau. Les causes sont
classées par priorité (échec, délai, vide, octave, transposition, attaque
décalée, notes manquantes/supplémentaires, durée seule) : la première cause
d'un fichier sert à regrouper les écarts.
