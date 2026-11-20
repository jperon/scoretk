# MusicXML export: reliability campaign against MuseScore 4.7.4

The writer lives in `crates/scoretk-musicxml-export`. This document records the
file-by-file measurement campaign started on 2026-11-20, its method and its
first corrected cause. The oracle and its manifests live outside the repository
(`scripts/oracle/musicxml/README.md`).

## 1. Oracle and method

For each `.mscz` of the 174-file lot (`/tmp/ly95-oracle/manifestes/mscz.txt`),
`scripts/oracle/musicxml/mesure.sh` produces four files: scoretk → MusicXML,
MuseScore 4.7.4 → MusicXML, and the two corresponding MIDI files. Three
measurements follow: MusicXML 4.0 XSD validity, N1/N2 on the MIDI, and the
canonical-line `core`/`full` comparison of `crates/scoretk/tests/oracle_core.rs`
(which uses the same normalisation as `e2e_corpus`). The `core` field of
`mesure.jsonl` is merged at the end of the run from `core.tsv`.

Every campaign must rebuild the measured binary from the **current** HEAD: the
first ranking of this campaign (`/tmp/pause-xml/current`, and the older
`/tmp/ott-xml/current` and `/tmp/anchor-xml/current`) was taken with binaries
built before commit `5283edb` ("conserver les sauts de ligne des noms
d'instrument"). That commit fixed the apparent top cause (`part-name`, 50
files: MuseScore keeps the `<br/>` of `<longName>S<br/>A</longName>` as a
newline, and so must the reader). A clean run after `5283edb` confirms that
`part-name` has disappeared from the ranking. Do not rank causes from an old
`mesure.jsonl`.

## 2. Clean ranking (2026-11-20)

First differing canonical tag, by number of files (173 converted, 40 already
core-identical):

| Tag | Files |
| --- | ---: |
| `staff-details` | 72 |
| `tuplet` | 14 |
| `attributes` | 14 |
| `measure` | 9 |
| `midi-channel` | 6 |
| `sound` | 5 |
| `movement-number` | 4 |
| other | 9 |

XSD validity: 0 errors on the scoretk side, 2 on the MuseScore side (both
pre-existing in the reference). N1 0.9966, N2 0.9492.

## 3. Corrected cause: invisible staves in `<staff-details>`

`writeStaffDetails` (`src/importexport/musicxml/internal/export/exportmusicxml.cpp:8059-8104`)
writes a `<staff-details>` block for every staff that is hidden, even a normal
5-line staff, with `print-object="no"` and `<staff-lines>`. A staff is hidden
when `!st->show()` (`:8066`) or when it is in the `hiddenStaves` list passed by
`writeMeasure` (`:8691`). The visibility flag is read from `<isStaffVisible>`
by `TRead::read(Staff*)` (`src/engraving/rw/read500/tread.cpp:4111`).

The Rust port skipped every 5-line staff (`staff.lines == 5`) and did not read
`<isStaffVisible>` at all, so the block was missing in 72 files. The fix:

- `Staff` gains a `visible` field, default `true` (`scoretk-core`);
- the 4.x reader reads `<isStaffVisible>` (`read4xx/mod.rs`), the 3.x reader
  reads `<visible>`/`<isStaffVisible>` (`read302/part.rs`);
- `write_staff_details` emits `print-object="no"` for an invisible staff and
  for a staff hidden by `hideEmptyStaves` (`systems.hidden_at`), with a reduced
  test (`tests::portee_masquee_ecrit_staff_details`) that failed before the fix.

### Result, file by file against the same clean baseline

The 173 pairs were reconverted with the corrected binary and compared with the
same `oracle_core` normalisation: **69 files gain, 0 file loses, 104 unchanged**;
the total number of differing canonical lines drops from 42 107 to 39 755
(−2 352). On the 72 `staff-details` files alone: 66 gains, 0 losses, 6
unchanged. After the fix, the first difference of those files is `dot` (27),
`staff-details` (31), `tuplet` (7) or `attributes` (6).

The remaining 31 `staff-details` differences come from staves that MuseScore's
layout hides without an explicit `<isStaffVisible>` (distance to the previous
staff ≤ 0 in `exportmusicxml.cpp:7553-7575`). The pivot model carries no layout,
so these are a documented limit, not a regression.

## 4. Round trip de `Staff::visible`

Le nouveau champ ne doit pas être perdu aux autres frontières :

- l'écrivain `.mscx` (`write/twrite.cpp:2945-2947`) n'écrivait jamais
  `<isStaffVisible>` ; il l'écrit maintenant quand la portée est invisible
  (`crates/scoretk-musescore-export/src/write/staffwrite.rs`), avec un test
  lecture → écriture → relecture (`tests/portee_invisible.rs`) ;
- le lecteur MusicXML ne lisait que `<staff-lines>` dans `<staff-details>` ;
  `print-object="no"` rend désormais la portée invisible
  (`MusicXmlParserPass2::staffDetails`, `importmusicxmlpass2.cpp:3262-3292`),
  avec un test dans `tests/lecture.rs`.

### Correction du défaut de visibilité à la relecture

`MusicXmlParserPass2::staffDetails` (`importmusicxmlpass2.cpp:3262-3297`) ne
rend la portée entière invisible que si le cas n'est ni un cutaway
(`print-spacing="yes"`) ni une mesure ne contenant que des silences : dans ce
dernier cas il active le style `hideEmptyStaves`. Or notre export écrit justement
`print-object="no"` pour une portée masquée sur un seul système. La lecture
positionnelle le transformait à tort en portée invisible pour toute la
partition, et un `<staff-details>` postérieur sans attribut la rétablissait.
La décision est maintenant différée en fin de mesure (les `<attributes>`
précèdent les notes) et suit les trois cas du C++. Deux tests de lecture couvrent
le cas « portée entière » et le cas « portée vide masquée sur un système ».

Les nuances non modélisées (visibilité par mesure du cutaway) restent une
limite du modèle pivot.

## 5. Cause suivante : numérotation des mesures

`MeasureNumberStateHandler::updateForMeasure` (`exportmusicxml.cpp:8274-8300`)
ajoute `Measure::measureNumberOffset()` (`<noOffset>`) au numéro courant, et ne
relance la numérotation à 1 que si le saut de section porte
`startWithMeasureOne`. Le portage ignorait `<noOffset>` et relançait à 1 sur
**tout** saut de section. Le modèle porte maintenant `number_offset` et
`section_keeps_numbering` (lecteurs 3.x et 4.x), et l'export s'en sert. Deux
tests réduits : décalage initial (494 → 495) et section avec
`startWithMeasureOne="0"` (numéros 318/644 au lieu d'un retour à 1).

Mesure ciblée sur les 9 fichiers concernés (RV 594 de Vivaldi) : **9 gains,
0 perte**, lignes canoniques divergentes 9 577 → 543. Les écarts résiduels
sont les causes suivantes du classement.

## 6. Cause suivante : canaux MIDI des instruments

`midiInstrument` (`exportmusicxml.cpp:7337-7346`) écrit le canal alloué au
premier `<Channel>` de l'instrument par `rebuildMidiMapping`
(`midimapping.cpp:286-310`) : **tous** les canaux d'un instrument, y compris les
canaux secondaires (mute, harmony…), consomment un canal MIDI. Le portage
n'en allouait qu'un seul par instrument. `Instrument::channel_count` est lu par
les lecteurs 3.x/4.x, réécrit par l'écrivain `.mscx`, et l'export en tient
compte. Test réduit : deux instruments à trois canaux donnent `<midi-channel>1`
et `4` au lieu de `1` et `2`.

Mesure ciblée sur les 6 fichiers concernés : **6 gains, 0 perte** (par exemple
`Hallelujah - Haendel` : 16 → 0 lignes divergentes).

## 7. Correction XSD détectée par la re-mesure

La re-mesure complète a révélé 5 erreurs XSD sur 2 fichiers : une clef
`after-barline` était écrite dans le même `<attributes>` que `<staff-details>`,
ce que la XSD 4.0 interdit (`clef*` précède `staff-details*`). Les fonctions
`writeStaffDetails`/`writeInstrumentDetails` laissent `<attributes>` ouvert
quand il n'y a pas de `<transpose>`. L'export le referme désormais uniquement
quand un bloc `<staff-details>` vient d'être écrit
(`crates/scoretk-musicxml-export/src/exportmusicxml.rs`), avec un test de non-
régression structurel. Les deux fichiers concernés (`Sancta Maria KV 273`,
`Dixit Dominus — Sicut erat`) sont de nouveau valides (0 erreur XSD).

### Re-mesure complète après les deux causes (174 fichiers)

Comparaison fichier par fichier avec la référence propre d'avant campagne :
**86 fichiers gagnent, 1 fichier perd une seule ligne canonique, 86 sont
inchangés** ; lignes divergentes 42 107 → 28 979 ; fichiers core-identiques
40 → 43. Les erreurs XSD scoretk passent de 5 (2 fichiers) à 0.

L'unique perte est `Sancta Maria KV 273` (285 → 286 lignes divergentes) : la
correction XSD sépare en deux `<attributes>` la clef `after-barline` du bloc
`<staff-details>`, ce qui ajoute une ligne au diff canonique. C'est le prix
d'un document valide, sans effet musical ; la première différence du fichier
reste `staff-details`, et le gain net du corpus est de plus de 13 000 lignes.
Le classement après correction est : `attributes` 39, `staff-details` 26,
`tuplet` 22, `dot` 22, `sound` 5.

## 8. Cause suivante : bloc `<attributes>` et `<measure-style>`

`Attributes::doAttr` garde `<attributes>` ouvert entre `writeStaffDetails` et
`measureStyle` (`exportmusicxml.cpp:8691-8710`) : MuseScore écrit
`<measure-style>` dans le **même** bloc que `<staff-details>`. La correction XSD
précédente refermait le bloc dès `<staff-details>`, ce qui créait un bloc
supplémentaire sur 39 fichiers. La fermeture est désormais différée jusqu'à une
clef `after-barline` (seul cas où la XSD l'interdit), conditionnée par la
présence effective d'un `<staff-details>` dans le bloc. Test réduit : un silence
multi-mesures avec portée masquée garde `<measure-style>` et `<staff-details>`
dans le même `<attributes>`.

Mesure ciblée sur les 39 fichiers : **21 gains, 0 perte**, lignes divergentes
9 932 → 9 528 ; les 2 fichiers sensibles à la XSD restent valides.

### Cause `dot` : constat non corrigeable sans mise en page

Sur les 22 fichiers dont la première différence est `dot`, l'écart vient des
**silences pointés** : `writeRest` (`exportmusicxml.cpp:4655-4669`) n'écrit
`<dot>` que depuis `rest->dotList()` (objets `NoteDot` créés par la mise en
page), alors que le portage le déduit de la figure `TDuration`. Exemple :
`Minuit chrétien — Vcl — Cb — Timb` a 175 silences `<dots>` dans le `.mscx`,
mais MuseScore 4.7.4 n'écrit le point que sur 20 d'entre eux (ceux des voix et
mesures où la mise en page a créé un `NoteDot`). Supprimer les points de
silences fait gagner 21 fichiers sur 22 mais casse la conformité du corpus
(`changevisibility` v4, `moonlight` v3), où le point doit être conservé : la
décision dépend de données de mise en page absentes du modèle pivot. Aucun
correctif n'a donc été commité pour cette cause.

## 9. Cause suivante : crochet des n-olets (`calcHasBracket`)

`Tuplet::calcHasBracket` (`tuplet.cpp:204-268`) retire le crochet d'un n-olet
entièrement ligaturé. Le portage écrivait le crochet par défaut sur les 22
fichiers concernés. Le modèle pivot n'a pas les groupes de ligatures calculés
par la mise en page, mais porte les durées notées : une ligature n'est possible
que pour une croche ou plus courte (`:231-245`, `beamCount < 1` → crochet). Un
nouveau fichier `tuplet_bracket.rs` porte cette règle et ses garde-fous
(n-olet dégénéré `cr1 == cr2` **sans** crochet, `tuplet.cpp:209-211` ; silence
avec crochet, `:212-214` ; ratio non réduit `4:4` avec limite à la double-croche).
`bracket_explicit` distingue les valeurs lues (`<bracket>`, `<bracketType>`,
attribut MusicXML `bracket`) qui restent prioritaires. Le lecteur 3.x lit
désormais `<bracketType>` comme le lecteur 4.x.

Mesure ciblée sur les 22 fichiers : **22 gains, 0 perte**, lignes divergentes
7 660 → 6 382. Conformité du corpus et aller-retour MusicXML conservés.

### Re-mesure complète après ces deux causes (174 fichiers)

La re-mesure a donné **51 gains, 1 perte, 121 inchangés** (42 107 → 28 979 →
26 775 lignes ; 54 fichiers core-identiques ; XSD scoretk toujours 0 erreur).
L'unique perte, `Dixit Dominus — Judicabit in na` (84 → 104), a été ramenée à
**84** après correction d'un détail du portage : `calcHasBracket` renvoie
`false` pour un n-olet dégénéré (`cr1 == cr2`, `:209-211`), alors que la
première version du portage le traitait comme un silence. Vérification ciblée
sur les 31 fichiers `attributes`/`tuplet` + ce fichier : 1 gain, 0 perte. Le
résultat corrigé est donc **51 gains, 0 perte, 122 inchangés** et 26 755 lignes
divergentes. Le classement restant est `staff-details` 35, `dot` 27,
`attributes` 24, `tuplet` 7.

### Données manquantes pour un portage complet

`calcHasBracket` utilise aussi l'appartenance exacte aux objets `Beam`, le
nombre de ligatures de chaque note, la direction de hampe calculée et les
ruptures 32/64 (`:218-268`). Ces objets sont créés par la mise en page et ne
sont pas dans le `.mscx` : le modèle pivot ne peut les reconstruire. La règle
ci-dessus reprodut le cas observé dans le lot sans ces données.

## 8. Next causes

Work the ranking of §2 in order, re-measuring after each committed cause:
`tuplet` (bracket direction), `attributes`, `measure`, `midi-channel`, `sound`.
Each cause gets its own commit, a reduced test written first, and a C++
citation.
