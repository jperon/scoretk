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

`mesure.sh` ajoute `python3Packages.mido` (absent du shell Nix du projet),
ajoute `lilypond` si `--midi2ly` est demandé, et transmet les arguments à
`mesure.py`. Les alternatives de lecture sont mesurées sur le **même lot et le
même oracle** que scoretk, avec le même pas d'échantillonnage :

* `--mscore <binaire>` (ou 3e argument positionnel) : import MIDI de MuseScore
  4.7.4, `-o out.musicxml in.mid`, sous `QT_QPA_PLATFORM=offscreen` ;
* `--midi2ly` : `midi2ly` (MIDI → `.ly`) puis `lilypond -s` (`.ly` → MIDI), la
  comparaison se faisant alors MIDI ↔ MIDI avec le même extracteur de notes ;
* `--music21` : `music21.converter.parse` → MusicXML, si le module est
  disponible (il est absent de nixpkgs au 2026-11-19 : l'option est alors
  ignorée avec un avertissement, et non comptée comme un échec).

Options utiles : `--limit N` (essai rapide sur les N premiers fichiers de chaque
lot), `--alt-stride N` (ne lancer les alternatives que sur un fichier sur N ;
scoretk reste mesuré sur tout le lot — utile quand la machine est chargée),
`--resume` (régénère le résumé à partir des `.jsonl` déjà produits sans relancer
les conversions). `test_backup.py` vérifie que l'oracle place correctement les
attaques des voix qui n'apparaissent qu'après un `<backup>`, `test_ids.py`
que deux partitions homonymes reçoivent des identifiants distincts et
`test_ties_import.py` que les chaînes de liaisons sont fusionnées des deux
côtés (lancez-les avec
`nix-shell -p python3Packages.mido --run "cd scripts/oracle/midi && python3 test_backup.py"`).

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

## Import oracle

`import_mesure.py` measures MIDI **reading** (plan-fiabilite phase 4). It pairs
the MuseScore upstream tests (`importexport/midi/tests/midiimport_data/*.mid`
with their `-ref.mscx`) and the LilyPond MIDIs whose `.ly` source exists. For
each pair the CLI reads the MIDI, and the notes `(pitch, onset, duration,
voice)` are compared at the Q16 grid (±1/16 quarter) with the expected
`.mscx` (parsed directly) or the `.ly`. No file is copied into the repository.

```sh
nix develop -c cargo build -p scoretk-cli
python3 scripts/oracle/midi/import_mesure.py \
  --binary target/debug/scoretk --out /tmp/oracle-midi-import/full \
  --upstream /home/jacques/programmes/musique/musescore/ref-musescore/src/importexport/midi/tests/midiimport_data
```

## Unsupported inputs (exit code 3)

`scoretk` exits with **3** when the input is deliberately unsupported: a
MuseScore version outside the ported subset, or a format without a converter in
the requested direction (see `scoretk --help` and `docs/agents/scoretk.md`).
This is **not** a conversion failure. The drivers apply the same rule through
`scripts/oracle/common.py`, never copied per script:

- `mesure.py --lecture`: the input is a MIDI, which has no "unsupported
  version" variant today; the guard is nonetheless in place (a code 3 stops the
  file before any alternative row).
- `mesure.py --ecriture` and `--robustesse`: a `.mscz` score whose version is
  not ported is marked `exclu`, removed from every numerator and denominator,
  and reported once in `resume.md` plus one warning line on stderr. Example:
  the MuseScore 1.14 `Canon_in_D_Pachelbel_-_arranged_for_accordion.mscz` file.
- `import_mesure.py`: a pair is excluded as soon as the MIDI **or** its
  reference (`.mscx`/`.ly`) returns code 3; both sides leave the survey.
