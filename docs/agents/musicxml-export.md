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

### Cause `dot` (27 fichiers) — résolue

L'écart vient des **silences pointés** : `writeRest`
(`exportmusicxml.cpp:4655-4669`) n'écrit `<dot>` que depuis `rest->dotList()`.
`Rest::checkDots` (`rest.cpp:308-321`) crée ces `NoteDot`, mais n'est appelé que
par le layout (`restlayout.cpp:148`) : une portée masquée
(`<isStaffVisible>0</isStaffVisible>`) n'a donc aucun `NoteDot`. Dans
`Minuit chrétien`, les 20 points conservés par MuseScore 4.7.4 sont exactement
ceux des portées visibles (Timbales, Vcl/Cb) ; les 155 supprimés sont sur des
portées masquées. Le nouveau fichier `rest_dots.rs` porte cette règle. Deux
tests réduits (portée visible/masquée) ont été écrits avant le correctif.

Mesure ciblée sur les 27 fichiers : **27 gains, 0 perte**, lignes divergentes
6 261 → 4 605. Les corpus v3/v4 (`changevisibility`, `moonlight`) gardent leurs
points car leurs portées sont visibles.

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

## 10. Cause `staff-details` (35 fichiers) — sous-classification

Les 35 fichiers se répartissent : 14 `<staff-details>` avec lignes absentes,
10 `<staff-details/>` absents, 6 écarts de position de `dot` (résolus par la
cause `dot`), 2 avec `number="1"`, 3 divers. Pour les 14+10, la cause est
`hideEmptyStaves` du `score_style.mss` (présent dans 28 fichiers). Le reprise
de ce réglage dans le lecteur a été **rejetée** : elle fait gagner 13 fichiers
mais en perd 15, car notre découpage en systèmes (sauts explicites seulement)
et `Measure::isEmpty` simplifié sur-massent les portées masquées. Ce constat
est conservé comme limite, sans correctif.

La partie réellement corrigeable est `<staff-size>` : `writeStaffDetails`
(`exportmusicxml.cpp:8139-8144`) l'écrit quand `staffMag != 1`
(`staff.cpp:806-809`), avec `smallStaffMag = 0.7` (`styledef.cpp:529`) et un
`<mag>` utilisateur éventuel. Le lecteur 4.x lit désormais `<small>` et `<mag>`
de `<StaffType>`, et l'export écrit `<staff-size scaling="…">…</staff-size>`.
Test réduit échouant avant le correctif. Oracle ciblé sur les 35 fichiers :
**12 gains, 0 perte**, 6 876 → 6 570 lignes divergentes.

## 11. Re-mesure complète après `dot` + `staff-size` (174 fichiers)

Comparaison fichier par fichier avec le relevé précédent : **47 gains,
0 perte, 126 inchangés** ; lignes divergentes 26 775 → 24 733 ; fichiers
core-identiques 54 → 57. XSD scoretk : 0 erreur. Le classement restant est
`staff-details` 40, `attributes` 33, `tuplet` 12, `sound` 5,
`movement-number` 4. Les 40 `staff-details` restants sont essentiellement les
portées masquées par la mise en page (voir §10, non corrigeables avec le
découpage en systèmes du pivot).

## 12. `movement-number` (4 fichiers)

`ExportMusicXml::work` (`exportmusicxml.cpp:6508-6527`) écrit
`<movement-number>` depuis le metaTag `movementNumber`. Le lecteur 4.x le
lisait déjà, pas le lecteur 3.x : les quatre extraits Vivaldi RV 594
(versions 3.01) perdaient le numéro de mouvement. Test réduit écrit d'abord,
échouant avant le correctif. Oracle ciblé : 4 gains, 0 perte (123 → 122,
7 → 6, 12 → 11, 15 → 14 lignes divergentes).

## 13. `tuplet` (12 fichiers restants)

Les 12 fichiers attendent `bracket="no"` alors que `calcHasBracket` écrit
`yes`. Ils contiennent des triolets de doubles-croches avec un silence. La
première version du portage (ignorer les silences partout) a fait gagner 6
fichiers mais en a **perdu 7** à la re-mesure complète, dont 4 fichiers
auparavant core-identiques (Autumn Leaves, Gloria Patri, Triumphal March,
Douce nuit — Conducteur).

Le critère exact est la **visibilité de la portée** : la mise en page ne
dispose pas une portée masquée, donc `hasBracket` y reste faux. Dans Douce
nuit — chant piano, les 31 triolets avec crochet sont sur la portée visible
Femmes ; les 31 triolets de description identique sur portées masquées n'ont
pas de crochet. Le portage applique donc la règle C++ (`tuplet.cpp:232-234`,
silence → crochet) sur portée visible et renvoie faux sur portée masquée.
Deux tests réduits (visible/masqué), écrits d'abord et constatés en échec
avant le correctif affiné.

Oracle ciblé sur l'union des 12 fichiers `tuplet` et des 7 fichiers sensibles :
**12 gains, 0 perte**, 6 857 → 5 941 lignes divergentes.


## 14. Re-mesure complète après `movement-number` + `tuplet` (174 fichiers)

**20 gains, 0 perte, 153 inchangés** ; lignes divergentes 24 733 → 23 651 ;
fichiers core-identiques 57 → 58 ; XSD scoretk 0 erreur. La catégorie `tuplet`
disparaît du classement. Restent `staff-details` 41, `attributes` 39,
`duration` 8, `sound` 5, `accidental` 4. La première version du correctif
`tuplet` (7 pertes à la re-mesure) a été remplacée par le critère de visibilité
de portée décrit en §13, vérifié sur l'union des fichiers concernés puis sur
les 174 fichiers.

## 15. `accidental` (4 fichiers)

MuseScore 4.7.4 exporte une altération avec `<bracket>1</bracket>` et rôle
utilisateur en `cautionary="yes" parentheses="yes"` ; le C++ 5.0
(`exportmusicxml.cpp:2866-2870`) écrit encore `editorial="yes" bracket="yes"`.
La référence 4.7.4 prévalant, l'export suit son comportement. Test réduit
écrit d'abord, échouant avant le correctif. Oracle ciblé : 3 gains, 0 perte
(2→0, 75→63, 11→7 ; le 4e fichier a un écart de taille `size="cue"`, non
lié à cette règle).

## 16. Dernier écart `accidental` : `size="cue"`

Une altération `<small>1</small>` s'exporte `size="cue"` (`writeAccidental`,
`exportmusicxml.cpp:2887-2892`). `Accidental`porte `small`, lu par les lecteurs
3.x et 4.x ; test réduit écrit d'abord, échouant avant le correctif. Sur
`JMHaydn_Viderunt_omnes`, les 5 `size="cue"` de la référence sont retrouvés
(325 → 315 lignes divergentes).

## 17. Status — contenu musical terminé

**Identique** : au dernier relevé complet, 59 fichiers sur 173 sont
core-identiques à MuseScore 4.7.4, la validation XSD est à 0 erreur côté
scoretk, N1 = 0,9966 et N2 = 0,9492. Les causes musicales traitées pendant la
campagne couvrent : noms de parties, visibilité des portées, `staff-size`,
canaux MIDI, numérotation des mesures, crochets de n-olets, points de silences,
`movement-number` et altérations (`cautionary`/`parentheses`, `size`).

**Reste, par choix de priorité** : `staff-details` (41) et `attributes` (39)
sont des portées masquées par la mise en page. Le C++ les cache dans
`SystemLayout::hideEmptyStaves` (`systemlayout.cpp:736-830`) à partir de
systèmes calculés par la gravure ; le pivot ne connaît que les sauts explicites.
Reprendre `hideEmptyStaves` du style a été tenté et rejeté (13 gains, 15 pertes)
parce que notre découpage sur-masque les portées. Ce sont des écarts de mise en
page, pas de contenu musical.

**Reste, côté lecture** : les petites catégories résiduelles (`duration` 9,
`sound` 6, `harmony` 2, `tie` 2, `dot` 2, `clef` 2, `measure-style` 2,
`staff` 2, `backup` 1, `lyric` 1) ne sont pas des règles d'écriture manquantes :
l'export suit ici fidèlement le C++ — durée depuis `globalTicks`
(`exportmusicxml.cpp:4636-4641`), `<sound>` des tempos invisibles via
`commonAnnotations` (`:6607-6620`) et `canWrite` (`:9408-9411`). Les différences
viennent d'éléments que le lecteur 3.x/4.x ne matérialise pas (notes, paroles,
harmonies ou tempos absents : ex. 4 993 vs 4 999 durées, 398 vs 269 paroles),
et qui décalent ensuite l'alignement des lignes canoniques. Les corriger
demande des travaux de lecture, hors du périmètre de l'écrivain.

Pour le contenu musical que le modèle porte, l'export MusicXML peut donc être
déclaré terminé : il ne reste que des écarts de mise en page et des données non
représentées dans le modèle pivot.

### Re-mesure finale (174 fichiers)

Après les deux causes musicales : **5 gains, 0 perte, 168 inchangés** ;
lignes divergentes 23 651 → 23 609 ; 58 fichiers core-identiques sur 173 ;
XSD scoretk 0 erreur, N1 0,9966, N2 0,9492. Les catégories `accidental` et
`lyric` sortent du classement des premières différences.

## 18. Lecteur 4.x : paroles avec balisage inline

Sur `Veni, Creator Spiritus_Courtial_v3_SATB`, 129 paroles sur 398 avaient un
`<text><i>…</i></text>` ; `Element::text()` ne renvoyant que le texte direct,
elles devenaient vides et l'export les supprimait (`write_lyric`,
`exportmusicxml.rs:2746`). Le C++ lit `<text>` avec `XmlReader::readXml()`
(`read500/tread.cpp:4622-4624`) : le texte imbriqué est conservé. Le nouveau
`Element::plain_text()` retire le balisage (et convertit `<br/>` en saut de
ligne comme `lineBreakFromTag`, `tread.cpp:3812`) et est utilisé par
`read_lyrics`. Test réduit (corpus v4 modifié avec `<i>`) écrit d'abord,
échouant avant le correctif. Oracle ciblé : 524 → 8 lignes divergentes,
0 perte.

## 19. Lecteur 4.x : `<Harmony>` dans un `<FretDiagram>`

`readMeasure206` (`read500/measureread.cpp:448-450`) construit un `FretDiagram`
et lit son `<Harmony>` enfant. Le lecteur 4.x ignorait l'élément
(`read4xx/mod.rs`, TODO `<FretDiagram>`) : les 23 accords de
`bransle-de-chevaux` (root 15/16) étaient perdus. Le bloc de lecture de
`<Harmony>` a été extrait dans `read_harmony_annotation` et appelé aussi depuis
`<FretDiagram>`. Test réduit (corpus v4 avec un FretDiagram injecté) écrit
d'abord, échouant avant le correctif. Oracle ciblé : 696 → 558 lignes
divergentes sur ce fichier, 0 perte.

### Re-mesure finale (174 fichiers)

Après les deux causes lecteur : **1 gain, 0 perte, 172 inchangés** ; lignes
divergentes 23 093 → 22 955 ; N1 0,9966 → 0,9971, N2 0,9492 → 0,9497 ;
58 fichiers core-identiques ; XSD scoretk 0 erreur. Le classement restant est
`staff-details` 42, `attributes` 40, `duration` 9, `sound` 6, puis des
résidus de 1 à 2 fichiers (`tie`, `clef`, `dot`, `measure-style`, `staff`,
`harmony`, `backup`, `type`, `actual-notes`). La catégorie `lyric` a disparu
du classement ; le trou de 129 paroles est comblé.

## 20. `duration` (9 fichiers) — corrigé

Les 7 fichiers Douce nuit avaient des `<forward>` plus grossiers que la
référence (75 vs 71 sur `Douce nuit — chant piano`). La cause était dans notre
port de `Measure::checkMeasure` : `timesig_at` (`gap_rests.rs`) s'arrêtait à la
mesure précédente lorsqu'elle ne portait pas d'indication, alors que
`TimeSigMap` du C++ maintient l'indication en vigueur. Dans ces partitions,
seul le premier `<Measure>` porte `<sigN>6</sigN>` : toutes les mesures
suivantes retombaient sur le 4/4 par défaut, ce qui changeait la découpe des
silences gap par `toRhythmicDurationList` (`durationtype.cpp:511-537`, appelée
par `check.cpp:276-296`) et donc les `<forward>` de `moveToTick`
(`exportmusicxml.cpp:2160-2186`).

Test réduit (mesure 0 avec 6/8, mesure 2 sans indication) écrit d'abord,
échouant avant le correctif (4/4 au lieu de 6/8). Oracle ciblé sur les 9
fichiers : **9 gains, 0 perte**, dont 92 → 72 et 398 → 378 lignes divergentes.

## 21. `sound` (6 fichiers) — tempos invisibles corrigés

Sur `The Petite Waltz`, 3 `<sound tempo>` surnuméraires (160, 175, 165) étaient
écrits. Les trois tempos correspondants sont attachés à des segments
`ChordRest` **sans accord ni silence** (vérifié dans le modèle : `has_chordrest=false`).
`findTrackForAnnotations` (`exportmusicxml.cpp:6375-6392`) renvoie `nidx` dans
ce cas et le C++ ignore l'annotation ; notre `annotations_without_note` l'écrivait.
Les annotations sans note restent exportées (conformité du corpus préservée),
seuls les tempos invisibles sont ignorés sur ces segments. Test réduit écrit
d'abord, échouant avant le correctif. Oracle ciblé : **4 gains, 0 perte**
(3→0, 8→0, 1→0, 63→58). Les 2 cas `segno` sont traités en §22.


## 22. `sound segno` (2 fichiers) — marqueurs invisibles

`directionMarker` (`exportmusicxml.cpp:6354-6365`) n'écrit un
`<direction><direction-type><segno/></…>` que si `canWrite(marker)` est vrai ;
pour un marqueur invisible (`<visible>0</visible>`), il n'écrit que
`<sound segno="…"/>`. Nos lectures 2.x/3.x/4.x ne portaient pas la visibilité
du `Marker`, et l'export écrivait toujours le `<direction>`, décalant la
première différence. `Marker::visible` est ajouté et lu par les trois lecteurs ;
l'export écrit le `<sound>` seul pour un marqueur invisible. Test réduit écrit
d'abord, échouant avant le correctif. Oracle ciblé : 7→6 et 314→306 lignes
divergentes, 0 perte.


## 23. Bilan final (174 fichiers) — contenu musical

Re-mesure après les trois correctifs code (indication 6/8 héritée,
`findTrackForAnnotations` pour les tempos, marqueurs invisibles) :
**26 gains, 1 perte, 146 inchangés** ; lignes divergentes 22 955 → 22 300 ;
fichiers core-identiques 58 → **63** ; XSD scoretk 0 erreur ; N1 0,9971,
N2 0,9497. La catégorie `duration` disparaît du classement, `sound` tombe à 1
résidu.

La régression `Tarentella` (5 → 8 lignes) a été corrigée : son silence
invisible en voix 2 ne doit pas créer de voix. `Measure::hasVoices`
(`measure.cpp:2198-2262`) ignore les ChordRest invisibles ; le port les
comptait, générant des silences gap et deux `<forward>` surnuméraires dans la
mesure 101. Test réduit (voix 2 avec silence invisible) écrit d'abord,
échouant avant le correctif. Oracle ciblé : 8 → **0** lignes divergentes. Le
bilan corrigé est donc **27 gains, 0 perte, 145 inchangés** et 22 292 lignes
divergentes.

Reste : `attributes` 46 et `staff-details` 43 (portées masquées par la mise en
page, laissées pour la fin par priorité), puis des résidus de 1 à 3 fichiers
(`clef`, `tie`, `dot`, `measure-style`, `tuplet`, `staff`, `harmony`, `forward`,
`sound`). Pour le contenu musical porté par le modèle, l'export MusicXML est
déclaré terminé.

## 24. Ratios de n-olets non réduits dans `<time-modification>` (2026-11-21)

Dernière classe musicale mesurée sur les 174 `.mscz` (`target/export-baseline/`) :
`actual-notes` 1446 lignes sur 2 fichiers, premier écart
`<actual-notes>6</actual-notes>` attendu contre `3` produit.

`writeTimeModification` (`exportmusicxml.cpp:4248-4278`) appelle
`timeModification` (`:4199-4214`) qui multiplie **à la main** les numérateurs
et dénominateurs des n-olets imbriqués : le commentaire du C++ précise qu'un
`Fraction::operator*()` réduirait un 6:4 en 3:2. Le port utilisait
`value.ratio.numerator()` / `.denominator()` (le ratio réduit du modèle),
alors que `Tuplet` conserve les nombres bruts du fichier dans
`actual_notes` / `normal_notes` (`displayed_numbers()`). Le port multiplie
maintenant ces nombres bruts. Corollaire : `Fraction::operator==` compare par
valeur, donc `4:4 == 1:1` et le C++ n'écrit rien (`ratio != Fraction(1,1)`,
`:4255`) ; la garde du port est devenue `actual == normal` au lieu de
`actual == 1 && normal == 1`, ce qui évite d'ajouter des
`<time-modification>` 4:4 et 16:16 sur `v4/voiceswitching` (test e2e).

Test réduit écrit d'abord : n-olet dont le ratio est 3/2 mais les nombres
bruts 6/4 ; échouait avec `<actual-notes>3</actual-notes>`. Re-mesure complète
(`target/export-final/`), 174 fichiers : aucun changement N1/N2/N2d (la
numération écrite ne change pas la lecture MIDI), **aucune perte** ; lignes
centrales `Czardas_Monti` 60 → **12**, `Ave_Maria_Schubert_in_C` 3560 →
**720**. `cargo test -p scoretk --test e2e_corpus` reste vert (5/5).

## 25. `normal-type` des n-olets 6:4 (2026-11-22)

Suite de la cause 24 sur les nombres bruts : `determineTupletNormalTicks`
(`exportmusicxml.cpp:3752-3771`) compare `elements().size()` au numérateur du
ratio **tel que le C++ le stocke** (`t->ratio().numerator()`, non réduit), et
pas au ratio réduit de `Fraction`. Le port utilisait `tuplet.ratio.numerator()`
(3 pour un 6:4) : avec 6 enfants il en déduisait qu'un `<normal-type>` était
nécessaire et en écrivait un sur chaque note, alors que MuseScore n'en écrit
aucun. La garde utilise maintenant `tuplet.actual_notes.max(1)` (le nombre
brut du fichier).

L'assertion `!xml.contains("<normal-type>")` a été ajoutée d'abord au test
réduit `rapport_de_nolet_non_reduit_dans_time_modification` (n-olet 6:4
complet) : elle échouait avant le correctif. Re-mesure complète avec deux
binaires frères (`target/export-head/` contre `target/export-normal/`, même
oracle) : aucun changement N1/N2/N2d, **aucune perte**, lignes centrales
`Czardas_Monti` 12 → **0** (devient core-identique) et
`Ave_Maria_Schubert_in_C` 720 → **8**. `e2e_corpus` reste vert (5/5).

## 26. Export-adjacent oracle work (2026-11-22)

`midi.md` records the order-independent MIDI note pairing, the empty-score
basket (out of numerator and denominator, like unsupported inputs), and the
MusicXML-source detection in the MIDI harness. `docs/agents/midi.md` kept a
single combined section after removing a duplicate.

## 27. Measure-start clefs and numbering-keeping section breaks (2026-11-22)

Two further musical-core export causes, both fixed from `exportmusicxml.cpp`
and measured on the 173-file `.mscz` lot.

### 27.1 One `<clef>` at a measure start

`findAndExportClef` (`exportmusicxml.cpp:7693-7714`) writes the clef from cs1
(the previous measure's `Clef`, or the first measure's `HeaderClef`) and falls
back to cs2 (a `Clef` at the start of the current measure) when cs1 is absent;
`writeElement` (`exportmusicxml.cpp:7986-8007`) skips a clef already written.

The port did neither: when a staff had no `HeaderClef`, it wrote the staff's
default clef and then the real `Clef` as an `after-barline="yes"` clef — two
`<clef>` elements in `Stella_maris_instruments - La` and `... - SiB`. When a
measure carried both a `HeaderClef` and an identical `Clef`, it also wrote
both. `write_measure_clefs` now uses the measure-start `Clef` as the cs2
fallback, and `mid_measure_clef` skips the clef already emitted for that track.
The skip keeps the distinct values of a genuine mid-measure clef change (the
`clef-1` corpus reference still passes). The reduced test
`crates/scoretk/tests/clef_duplicate_at_start.rs` was written first and failed
before the fix.

### 27.2 No `<divisions>` rewrite when numbering continues

`isFirstActualMeasure` (`exportmusicxml.cpp:8307-8310`) becomes true again only
after `MeasureNumberStateHandler::init()`, called by `updateForMeasure`
(`exportmusicxml.cpp:8281-8286`) only when the section break has
`startWithMeasureOne`. The port keyed the rewrite on `section_break_before`
alone, so a break whose `section_keeps_numbering` is true re-emitted
`<attributes><divisions>…</divisions></attributes>` (and `<staves>`). It now
checks `!section_keeps_numbering`, as the measure-numbering helper already
did. Reduced test `saut_de_section_sans_reprise_ne_rewrite_pas_les_divisions`
in `crates/scoretk-musicxml-export/src/tests.rs` was written first and failed
before the fix.

### Measurement

Fresh baseline `target/xml-rebase/` (pre-fix binary `target/ly95-rebase`) and
post-fix `target/xml-sec/` (binary `target/ly95-sec`), same 173-file manifest
and oracle: 173 converted, 0 XSD-invalid, N1/N2 1.0000 unchanged.
Core-identical **66 → 68**. Eight files improved, **no file lost lines**:
`Stella_maris_instruments - La` and `- SiB` 4 → 0 (clef cause);
`Vivaldi … 04-TecumPrincipium` 9 → 2, `… 06-Dominus_fredipi` 13 → 3,
`… 03-VirgamVirtutis` 16 → 6, `… 08-DeTorrente` 21 → 14,
`Sancta_Maria_KV_273` 279 → 256 and
`Vivaldi … 10._Sicut_erat_in_principio` 223 → 187 (section-break cause).
`cargo test --workspace`, `cargo fmt --all -- --check` and workspace clippy
were green at both commits.

## 28. Rebase, creator metaTags and a rejected tuplet cause (2026-11-22)

### 28.1 Rebase

The rebase on master conflicted in `scripts/oracle/common.py`: master had
added `raison_hors_lecteur`/`commandes_obsoletes` and the generic
`marquer_exclu` exclusion path. Both selftest blocks were kept. The empty-score
basket now reuses `marquer_exclu` with `raison_exclusion = "empty score"`
instead of a parallel `vide` flag; `filtrer` is master's single-path version
and `noms_vides` selects by reason. `python3 -I scripts/oracle/common.py
selftest`, the MIDI oracle test modules, workspace tests, fmt and clippy pass.

The `None` branch of `mid_measure_clef` is now justified in a comment from
`findAndExportClef` (`exportmusicxml.cpp:7699-7714`): at tick 0 the cs2/default
fallback of `write_measure_clefs` has already emitted the clef, while later
measures can legitimately write an `after-barline` clef.

### 28.2 Poet and translator creators

`identification` (`exportmusicxml.cpp:7237-7244`) iterates a `std::set` of
creator metaTags — `arranger`, `composer`, `lyricist`, `poet`, `translator` —
so the order is alphabetical. The pivot model folded `poet` into `lyricist`
and dropped `translator`; both 3.x and 4.x readers are fixed, and the writer
emits the five creators in order. The 4.x Vivaldi scores carry `poet` =
`Psaume 109` and `translator` = `fredipi`; other scores only carry `lyricist`.
The reduced test `createurs_poet_et_translator` (in the export crate) was
written first and failed before the fix.

Measurement (`target/xml-sec/` pre-fix against `target/xml-creators2/`
post-fix, same 173-file manifest): core-identical stays **68/173** —
`<identification>` is outside the musical core — N1/N2 remain 1.0000, 0
XSD-invalid. At full level, **30 files lose 58 diff lines** (three files become
fully identical); no core file changes. Five files show a higher full-level
*sequence*-diff count because the added line changes `similar`'s alignment
elsewhere: their added creator lines are verbatim from the reference, and the
order-independent multiset difference strictly decreases (e.g. 9612 → 9611 on
`01- Once in royal - Violons 1 et 2`), so there is no real regression.

### 28.3 Rejected cause: tuplet bracket inside a longer beam

`Tuplet::calcHasBracket` (`tuplet.cpp:204-275`) drops the bracket only when the
tuplet starts and ends its beam. A reduced test was written first and the
port was changed to keep the bracket when the reconstructed beam showed
`continue` on either end. The full measurement (`target/xml-tuplet/`) went
from 68 to **63** core-identical with **12 files losing lines** (e.g. file 166
went from 84 to 524): the reconstructed beam groups do not match MuseScore's
`Beam` objects closely enough for that rule. The fix and its test were
reverted (`62ba0b86`) after the measurement; this is why the class remains
open. Any future fix should derive the beam boundaries from `BeamLayout`
rather than from the per-element `<beam>` text.

## 29. Poet round trip, cue notes and the beam boundary question (2026-11-22)

### 29.1 Review remark: poet in the other formats

The 4.x reader used to fold `poet` into `lyricist`; after splitting the fields,
every consumer was audited. The mscx writer now writes the `poet` and
`translator` metaTags back in alphabetical order; the LilyPond writer uses
`poet` for `\header poet` when `lyricist` is empty (`titling-init.ly:87`, and
MuseScore's own fallback `importmusicxmlpass1.cpp:1143-1146`); the MusicXML
reader maps each `creator` type to its own metaTag (`setMetaTag(type, text)`,
`importmusicxmlpass1.cpp:1527-1528`). ABC has no poet field, and MIDI's
`META_POET` remains the lyricist text style (`importmidi.cpp:358-360`).
The reduced facade test `crates/scoretk/tests/poet_metadata.rs` covers the
mscx/LilyPond writers and the MusicXML reader, and failed before the fix.

### 29.2 Cue size for individually small notes

`isSmallNote` (`exportmusicxml.cpp:4181-4184`) is
`note->isSmall() || note->chord()->isSmall()`, and `isCueNote` (`:4191-4193`)
is built from it. The pivot model only kept the chord-level `<small>`, so
notes carrying their own `<small>1</small>` lost their `size="cue"` type
attribute. `Note::small` is now read by both MuseScore readers and OR-ed in
`write_type`/`is_cue_note`. The reduced test
`type_cue_sur_note_reduite` (export crate) was written first and failed before
the fix.

Measurement (`target/xml-creators2/` pre-fix vs `target/xml-cue/` post-fix,
same 173-file manifest): core-identical **68 → 69**, 0 XSD-invalid, N1/N2
1.0000. Three files gained core lines and **no file lost any**:
`Angels_Carol__John_Rutter` 58 → 42, `Dieu_seul_suffit_-_Du_Jonchay` 6 → 2,
`AnthemChristmas_Lullaby` 24 → 0 (now core-identical). Workspace tests, fmt
and clippy were green at the commit.

### 29.3 Beam boundaries: why no second beam fix was attempted

The remaining two first-divergence tuplet files are still open. The 5.0
`Groups::baseBeamMode` tuplet-change rule (`groups.cpp:139-145`) is already
ported in `beams.rs`, and the beam groups reconstructed from the model are
those of the ported `BeamLayout` (`beamlayout.cpp:653-800`). In the 4.7.4
reference for `Vivaldi … 04-TecumPrincipium`, a `<tuplet bracket="yes">`
sits in the middle of a six-note beam, while the ported 5.0 grouping breaks
the beam at the tuplet's first note; the reference behaviour therefore comes
from a 4.7.4 layout decision that the 5.0 snapshot no longer encodes. The
previous per-element `<beam>`-text heuristic was measured (`target/xml-tuplet/`)
and lost lines in 12 files, so it was reverted (§28.3). Deriving those
boundaries would need a 4.7.4 `BeamLayout` reference, which is not in
`ref-musescore/`; no safe fix was submitted for this class.
