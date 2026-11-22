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

## 30. 4.7.4 beam audit and the rejected `<Groups>` port (2026-11-22)

Section 29.3 recorded the open tuplet-bracket class. The 4.7.4 source was
fetched read-only outside the repository and diffed against the 5.0 snapshot:
`groups.cpp` (including `baseBeamMode` and its tuplet-change rule),
`tuplet.cpp::calcHasBracket` and `beamlayout.cpp::createBeams` carry the same
rules for pitched staves (details in `docs/agents/upstream.md`, appendix A).
The reference's beam across the tuplet therefore comes from score data —
staff `<Groups>` with `action=0x110` and explicit `<BeamMode>` — and from
layout predicates (`calcBeamBreaks`, `headSide`, `isCross`) that the pivot
model does not carry.

A reduced test first, then a port of `<Groups>` plus a beam-boundary check
based on the reconstructed beam groups made that test pass, but the full
oracle (`target/xml-beam474/`) went from 69 to **64** core-identical with
**12 files losing lines** — the same files as the earlier rejected heuristic.
The bracket rule needs more than beam start/end: C++ can still drop the
bracket through `calcBeamBreaks` or `headSide`. The changes were reverted;
no file regressed and the working tree is clean. This class remains a
documented limit, not a 4.7.4-vs-5.0 divergence.

## 31. Standalone `<sound>` for invisible jumps (2026-11-22)

`writeJump` (`exportmusicxml.cpp:6218-6224`) writes the playback sound as a
standalone `<sound …/>` when `canWrite` (`:9408-9411`) is false — for the
default `exportInvisibleElements` setting, that means a `<Jump>` carrying
`<visible>0</visible>`. The port always wrapped the sound inside a
`<direction><words>`, which the musical-core normalizer drops, so the D.C. and
D.S. sounds of `Dieu_seul_suffit_-_Du_Jonchay` were missing from the core.
`Jump::visible` is now read by both MuseScore readers, and `direction_jump`
takes the standalone-sound branch for an invisible jump. The analogous case
for invisible markers was already covered (`marqueur_segno_invisible_sans_direction`).
The reduced test `saut_invisible_sans_direction` (in `repeats.rs`) was written
first and failed before the fix.

Measurement (`target/xml-cue/` pre-fix vs `target/xml-jump/` post-fix, same
173-file manifest): core-identical **69 → 70**, N1/N2 1.0000, 0 XSD-invalid.
One file gained core lines (`Dieu_seul_suffit_-_Du_Jonchay` 2 → 0, now
core-identical) and **no file lost any**. Workspace tests, fmt and clippy
were green at the commit.

## 32. First-divergence inventory of the 103 non-core-identical files (2026-11-22)

Baseline `target/xml-jump/` (70/173 core-identical). One line per file:
`file | first core-divergence category | core-diff lines`. Categories are grouped; beams (`tuplet`) and pure layout attributes (`attributes`, `staff-details`) are excluded from the next fixes.

| Category | Files | Lines | File | Count |
| --- | ---: | ---: | --- | ---: |
| attributes (group) | 46 | 7983 | | |
| attributes | | | `Missa_in_Es_Hob._XXII4_II._Gloria_-_Haydn.mscz` | 797 |
| attributes | | | `Missa_in_Es_Hob._XXII4_III._Credo_-_Haydn.mscz` | 760 |
| attributes | | | `05- Il est né - Hautbois.mscz` | 384 |
| attributes | | | `01- Douce nuit - Violons.mscz` | 378 |
| attributes | | | `04- Il est né - Flûtes.mscz` | 332 |
| attributes | | | `02- Douce nuit - Alto.mscz` | 330 |
| attributes | | | `03- Adeste fideles - Vcl - Cb - Timb.mscz` | 324 |
| attributes | | | `JMHaydn_Viderunt_omnes.mscz` | 315 |
| attributes | | | `Missa_in_Es_Hob._XXII4_VI._Agnus_Dei_-_Haydn.mscz` | 288 |
| attributes | | | `01- Il est né - Violons.mscz` | 286 |
| attributes | | | `03- Douce nuit - Violoncelle - Basson.mscz` | 284 |
| attributes | | | `Sancta_Maria_KV_273__Wolfgang_Amadeus_Mozart_orgue.mscz` | 256 |
| attributes | | | `02- Minuit chretien - Alto.mscz` | 240 |
| attributes | | | `00- Conducteur - Ô Notre Dame.mscz` | 237 |
| attributes | | | `02- Il est né - Alto.mscz` | 223 |
| attributes | | | `05- God rest you - Basson.mscz` | 190 |
| attributes | | | `03- Minuit chretien - Vcl - Cb - Timb.mscz` | 180 |
| attributes | | | `03- Il est né - Vcl - Cb - Bas.mscz` | 178 |
| attributes | | | `01- Adeste fideles - Violons 1 et 2.mscz` | 162 |
| attributes | | | `02- God rest you - Alto.mscz` | 148 |
| attributes | | | `02- Adeste fideles - Alto.mscz` | 135 |
| attributes | | | `01- God rest you - Violons 1 et 2.mscz` | 133 |
| attributes | | | `00- God rest you merry, gentlemen - choeur.mscz` | 124 |
| attributes | | | `Vivaldi_-_Dixit_Dominus-RV_594__-_02_Donec_ponam_de_Antonio_Vivaldi.mscz` | 108 |
| attributes | | | `02- Once in royal - Alto.mscz` | 104 |
| attributes | | | `00- Les anges dans nos campagnes - Conducteur.mscz` | 102 |
| attributes | | | `Vivaldi_-_Dixit_Dominus-RV_594__-_09_Gloria_Patri_orgue.mscz` | 96 |
| attributes | | | `Vivaldi_-_Dixit_Dominus-RV_594__-_07_Judicabit_in_nationibus.mscz` | 84 |
| attributes | | | `00- Once in royal David_s city - conducteur.mscz` | 81 |
| attributes | | | `Vivaldi_-_Dixit_Dominus-RV_594__-_09_Gloria_Patri.mscz` | 75 |
| attributes | | | `00- Douce nuit - chant piano.mscz` | 72 |
| attributes | | | `Missa_in_Es_Hob._XXII4_IV._Sanctus_-_Haydn.mscz` | 67 |
| attributes | | | `00- Minuit chretien - Chant - piano.mscz` | 63 |
| attributes | | | `00- Minuit chretien - conducteur.mscz` | 63 |
| attributes | | | `00- God rest you merry, gentlemen - chant - piano.mscz` | 60 |
| attributes | | | `00- God rest you merry, gentlemen - conducteur.mscz` | 48 |
| attributes | | | `00- Adeste fideles - choeur.mscz` | 48 |
| attributes | | | `Ad_te_Suspiramus.mscz` | 42 |
| attributes | | | `Angels_Carol__John_Rutter.mscz` | 42 |
| attributes | | | `Magnificat__Prt.mscz` | 39 |
| attributes | | | `00- Il est né le divin enfant - Rutter - Chant et orgue.mscz` | 30 |
| attributes | | | `00- Il est né le divin enfant - Rutter - Conducteur.mscz` | 27 |
| attributes | | | `00- Adeste fideles - chant piano.mscz` | 24 |
| attributes | | | `00- Adeste fideles - conducteur.mscz` | 15 |
| attributes | | | `Vivaldi_-_Dixit_Dominus-RV_594__-_03-VirgamVirtutis_fredipi.mscz` | 6 |
| attributes | | | `Vivaldi_-_Dixit_Dominus-RV_594__-_06-Dominus_fredipi.mscz` | 3 |
| staff-details (group) | 43 | 8126 | | |
| staff-details | | | `02- Flûtes 1 et 2.mscz` | 423 |
| staff-details | | | `01- Trompettes 1, 2 et 3.mscz` | 399 |
| staff-details | | | `05- Douce nuit - Flûtes.mscz` | 382 |
| staff-details | | | `06- Douce nuit - Hautbois.mscz` | 345 |
| staff-details | | | `01- Les anges - Violons 1 et 2.mscz` | 330 |
| staff-details | | | `03- Hautbois 1 et 2.mscz` | 330 |
| staff-details | | | `05- Les anges - Hautbois 1 et 2.mscz` | 324 |
| staff-details | | | `04- Basson.mscz` | 324 |
| staff-details | | | `02- Les anges - Alto.mscz` | 300 |
| staff-details | | | `03- Les anges - Vcl - Basson.mscz` | 270 |
| staff-details | | | `04- Minuit chretien - Flûtes 1 et 2.mscz` | 267 |
| staff-details | | | `07- Adeste fideles - Trompettes 1, 2 et 3.mscz` | 264 |
| staff-details | | | `07- Minuit chretien - Trompettes 1, 2 et 3.mscz` | 258 |
| staff-details | | | `07- Veni veni Emmanuel - Trompettes 1, 2 et 3.mscz` | 252 |
| staff-details | | | `04- Adeste fideles - Flûtes 1 et 2.mscz` | 237 |
| staff-details | | | `04- Once in royal - Hautbois 1 et 2.mscz` | 226 |
| staff-details | | | `04- Veni veni Emmanuel - Flûtes 1 et 2.mscz` | 214 |
| staff-details | | | `05- Veni veni Emmanuel - Hautbois 1 et 2.mscz` | 196 |
| staff-details | | | `05- Minuit chretien - Hautbois 1 et 2.mscz` | 192 |
| staff-details | | | `05- Adeste fideles - Hautbois 1 et 2.mscz` | 192 |
| staff-details | | | `05- Once in royal - Basson.mscz` | 187 |
| staff-details | | | `06- God rest you - Trompettes 1,2 et 3.mscz` | 187 |
| staff-details | | | `00- Douce nuit - Choeur.mscz` | 170 |
| staff-details | | | `04- Les anges - Contrebasse.mscz` | 168 |
| staff-details | | | `06- Adeste fideles - Basson.mscz` | 168 |
| staff-details | | | `06- Veni veni Emmanuel - Basson.mscz` | 158 |
| staff-details | | | `01- Veni veni Emmanuel - Violons 1 et 2.mscz` | 151 |
| staff-details | | | `05- Violons 1 et 2.mscz` | 150 |
| staff-details | | | `04- God rest you - Hautbois 1 et 2.mscz` | 145 |
| staff-details | | | `01- Once in royal - Violons 1 et 2.mscz` | 143 |
| staff-details | | | `00- Il est né le divin enfant - Rutter - choeur.mscz` | 137 |
| staff-details | | | `Sancta_Maria_Mater_Dei_Mozart.mscz` | 126 |
| staff-details | | | `06- Alto.mscz` | 108 |
| staff-details | | | `02- Veni veni Emmanuel - Alto.mscz` | 92 |
| staff-details | | | `00- Les anges dans nos campagnes - choeur.mscz` | 63 |
| staff-details | | | `00- Once in royal David_s city - chant piano.mscz` | 56 |
| staff-details | | | `00- Once in royal David_s city - choeur.mscz` | 56 |
| staff-details | | | `00- Veni veni Emmanuel - conducteur.mscz` | 49 |
| staff-details | | | `00- Veni veni Emmanuel - chant piano.mscz` | 36 |
| staff-details | | | `07- Violoncelle et Contrebasse.mscz` | 24 |
| staff-details | | | `Marusia.mscz` | 15 |
| staff-details | | | `Veni, Creator Spiritus_Courtial_v3_SATB.mscz` | 8 |
| staff-details | | | `00- Veni veni Emmanuel - choeur.mscz` | 4 |
| tuplet (group) | 2 | 16 | | |
| tuplet | | | `Vivaldi_-_Dixit_Dominus-RV_594__-_08-DeTorrente_fredipi.mscz` | 14 |
| tuplet | | | `Vivaldi_-_Dixit_Dominus-RV_594__-_04-TecumPrincipium_fredipi.mscz` | 2 |
| frame (group) | 1 | 558 | | |
| frame | | | `bransle-de-chevaux-bransle-de-chevaux-rc-3.mscz` | 558 |
| measure-style (group) | 2 | 520 | | |
| measure-style | | | `Missa_in_Es_Hob._XXII4_I._Kyrie_-_Haydn.mscz` | 260 |
| measure-style | | | `Missa_in_Es_Hob._XXII4_V._Benedictus_-_Haydn.mscz` | 260 |
| dot (group) | 2 | 476 | | |
| dot | | | `06- Minuit chretien - Basson.mscz` | 313 |
| dot | | | `01- Minuit chretien - Violons 1 et 2.mscz` | 163 |
| notehead (group) | 1 | 198 | | |
| notehead | | | `Vivaldi_-_Dixit_Dominus-RV_594__-_01_Dixit_Dominus.mscz` | 198 |
| staff-size (group) | 1 | 187 | | |
| staff-size | | | `Vivaldi_-_Dixit_Dominus-RV_594__-_10._Sicut_erat_in_principio_orgue.mscz` | 187 |
| clef (group) | 1 | 186 | | |
| clef | | | `Vingt_danses_enfantines_pour_piano.mscz` | 186 |
| harmony (group) | 1 | 81 | | |
| harmony | | | `Adios_Muchachos.mscz` | 81 |
| staff (group) | 2 | 24 | | |
| staff | | | `Vivaldi_-_Dixit_Dominus-RV_594__-_10._Sicut_erat_in_principio.mscz` | 14 |
| staff | | | `Vivaldi_-_Dixit_Dominus-RV_594__-_05_Juravit_Dominus.mscz` | 10 |
| time-modification (group) | 1 | 8 | | |
| time-modification | | | `Ave_Maria_Schubert_in_C_with_chords.mscz` | 8 |

## 33. Three causes from the inventory (2026-11-22)

The grouped inventory (§32) was taken in order, skipping beams (`tuplet`) and
pure layout (`attributes`, `staff-details`).

### 33.1 Note-level `<visible>` in 2.x/3.x scores

`writeNotehead` (`exportmusicxml.cpp:3910-3913`) writes
`<notehead>none</notehead>` for an invisible note on a normal staff. The 4.x
reader already read `<visible>` on `<Note>`, but the 2.x/3.x reader did not.
Adding it removed 12 spurious `<notehead>none</notehead>` gaps: reduced facade
test first (failing before the fix). Measurement `target/xml-notevis/` vs
`target/xml-jump/`: **3 gains, 0 losses** — `Angels_Carol` 42 → 27,
`Vivaldi 01-Dixit_Dominus` 198 → 186, `Vivaldi 09-Gloria_Patri` 75 → 69.

### 33.2 `smallStaffMag` from `score_style.mss`

`Staff::staffMag` (`staff.cpp:806-809`) is
`(small ? smallStaffMag : 1) * userMag` and `writeStaffDetails`
(`exportmusicxml.cpp:8139-8144`) derives `<staff-size scaling=…>` from it.
The port hardcoded 0.7 and ignored `<smallStaffMag>` in the `.mscz`'s
`score_style.mss`: eight small staves of `Vivaldi Sicut erat` were written at
70 instead of 90. Reading the style value fixed them: reduced test first.
Measurement `target/xml-mag/` vs `target/xml-notevis/`: **1 gain, 0 losses** —
that file 187 → 171.

### 33.3 Inline `<Style>` `smallStaffMag`

`StyleSettings::set_value` now also accepts `smallStaffMag` from an inline
`<Style>` in an `.mscx` (the `.mss` path is §33.2); unit test added.
Measurement `target/xml-style/` vs `target/xml-mag/`: **0 gain, 0 loss** — the
173-file lot carries the value in `score_style.mss`, so no file changes. This
closes the inline path without altering the corpus.

### 33.4 Rejected attempts (measured, reverted)

- Section-start `HeaderClef` with `after-barline` (`target/xml-clefsec/`):
  one file gained 2 wrong lines (186 → 188) because the clef landed in the
  wrong `<attributes>` block. Reverted.
- Skipping dots on whole rests (`target/xml-restdot/`): fixed the two `dot`
  files but lost lines in 6 files — references do contain dotted whole rests
  (`00- Veni veni Emmanuel - chant piano` has `<type>whole</type><dot/>`).
  Reverted.
- Broadening `assign_multiple_rests` to all empty runs regressed
  `e2e_corpus` and was reverted before measurement.

Core-identical remains **70/173**, N1/N2 1.0000, 0 XSD-invalid throughout.

### 32.1 Sub-classification of `attributes` (46) and `staff-details` (43) by first differing element

From the 70/173 baseline `target/xml-style/`. One line per file: `file | group`; groups marked *layout* are excluded from further fixes.

| First differing element | Files | Class | File |
| --- | ---: | --- | --- |
| attributes block missing in tk | 41 | layout/other | `Ad_te_Suspiramus.mscz` |
|  |  |  | `Sancta_Maria_KV_273__Wolfgang_Amadeus_Mozart_orgue.mscz` |
|  |  |  | `00- Douce nuit - chant piano.mscz` |
|  |  |  | `01- Douce nuit - Violons.mscz` |
|  |  |  | `02- Douce nuit - Alto.mscz` |
|  |  |  | `03- Douce nuit - Violoncelle - Basson.mscz` |
|  |  |  | `00- Il est né le divin enfant - Rutter - Chant et orgue.mscz` |
|  |  |  | `00- Il est né le divin enfant - Rutter - Conducteur.mscz` |
|  |  |  | `01- Il est né - Violons.mscz` |
|  |  |  | `02- Il est né - Alto.mscz` |
|  |  |  | `03- Il est né - Vcl - Cb - Bas.mscz` |
|  |  |  | `04- Il est né - Flûtes.mscz` |
|  |  |  | `05- Il est né - Hautbois.mscz` |
|  |  |  | `00- Les anges dans nos campagnes - Conducteur.mscz` |
|  |  |  | `00- Once in royal David_s city - conducteur.mscz` |
|  |  |  | `02- Once in royal - Alto.mscz` |
|  |  |  | `00- Minuit chretien - Chant - piano.mscz` |
|  |  |  | `00- Minuit chretien - conducteur.mscz` |
|  |  |  | `02- Minuit chretien - Alto.mscz` |
|  |  |  | `03- Minuit chretien - Vcl - Cb - Timb.mscz` |
|  |  |  | `00- God rest you merry, gentlemen - chant - piano.mscz` |
|  |  |  | `00- God rest you merry, gentlemen - choeur.mscz` |
|  |  |  | `00- God rest you merry, gentlemen - conducteur.mscz` |
|  |  |  | `01- God rest you - Violons 1 et 2.mscz` |
|  |  |  | `02- God rest you - Alto.mscz` |
|  |  |  | `05- God rest you - Basson.mscz` |
|  |  |  | `00- Adeste fideles - chant piano.mscz` |
|  |  |  | `00- Adeste fideles - choeur.mscz` |
|  |  |  | `00- Adeste fideles - conducteur.mscz` |
|  |  |  | `01- Adeste fideles - Violons 1 et 2.mscz` |
|  |  |  | `02- Adeste fideles - Alto.mscz` |
|  |  |  | `03- Adeste fideles - Vcl - Cb - Timb.mscz` |
|  |  |  | `00- Conducteur - Ô Notre Dame.mscz` |
|  |  |  | `JMHaydn_Viderunt_omnes.mscz` |
|  |  |  | `Angels_Carol__John_Rutter.mscz` |
|  |  |  | `Missa_in_Es_Hob._XXII4_II._Gloria_-_Haydn.mscz` |
|  |  |  | `Missa_in_Es_Hob._XXII4_III._Credo_-_Haydn.mscz` |
|  |  |  | `Missa_in_Es_Hob._XXII4_IV._Sanctus_-_Haydn.mscz` |
|  |  |  | `Missa_in_Es_Hob._XXII4_VI._Agnus_Dei_-_Haydn.mscz` |
|  |  |  | `Vivaldi_-_Dixit_Dominus-RV_594__-_09_Gloria_Patri_orgue.mscz` |
|  |  |  | `Vivaldi_-_Dixit_Dominus-RV_594__-_10._Sicut_erat_in_principio_orgue.mscz` |
| ('staff-details', 'no', (), None, ('multiple-rest',)) | 15 | semantic | `01- Veni veni Emmanuel - Violons 1 et 2.mscz` |
|  |  |  | `05- Douce nuit - Flûtes.mscz` |
|  |  |  | `06- Douce nuit - Hautbois.mscz` |
|  |  |  | `01- Les anges - Violons 1 et 2.mscz` |
|  |  |  | `02- Les anges - Alto.mscz` |
|  |  |  | `03- Les anges - Vcl - Basson.mscz` |
|  |  |  | `04- Les anges - Contrebasse.mscz` |
|  |  |  | `05- Les anges - Hautbois 1 et 2.mscz` |
|  |  |  | `05- Once in royal - Basson.mscz` |
|  |  |  | `04- Minuit chretien - Flûtes 1 et 2.mscz` |
|  |  |  | `05- Minuit chretien - Hautbois 1 et 2.mscz` |
|  |  |  | `07- Minuit chretien - Trompettes 1, 2 et 3.mscz` |
|  |  |  | `06- Adeste fideles - Basson.mscz` |
|  |  |  | `04- Basson.mscz` |
|  |  |  | `06- Alto.mscz` |
| ('attributes child count', 5, 4) | 11 | layout/other | `Marusia.mscz` |
|  |  |  | `04- Veni veni Emmanuel - Flûtes 1 et 2.mscz` |
|  |  |  | `05- Veni veni Emmanuel - Hautbois 1 et 2.mscz` |
|  |  |  | `07- Veni veni Emmanuel - Trompettes 1, 2 et 3.mscz` |
|  |  |  | `00- Once in royal David_s city - chant piano.mscz` |
|  |  |  | `00- Once in royal David_s city - choeur.mscz` |
|  |  |  | `04- God rest you - Hautbois 1 et 2.mscz` |
|  |  |  | `06- God rest you - Trompettes 1,2 et 3.mscz` |
|  |  |  | `04- Adeste fideles - Flûtes 1 et 2.mscz` |
|  |  |  | `05- Adeste fideles - Hautbois 1 et 2.mscz` |
|  |  |  | `07- Adeste fideles - Trompettes 1, 2 et 3.mscz` |
| ('staff-details', 'no', ('staff-lines',), None, ('multiple-rest',)) | 9 | semantic | `00- Douce nuit - Choeur.mscz` |
|  |  |  | `00- Il est né le divin enfant - Rutter - choeur.mscz` |
|  |  |  | `00- Les anges dans nos campagnes - choeur.mscz` |
|  |  |  | `01- Once in royal - Violons 1 et 2.mscz` |
|  |  |  | `04- Once in royal - Hautbois 1 et 2.mscz` |
|  |  |  | `01- Trompettes 1, 2 et 3.mscz` |
|  |  |  | `02- Flûtes 1 et 2.mscz` |
|  |  |  | `03- Hautbois 1 et 2.mscz` |
|  |  |  | `05- Violons 1 et 2.mscz` |
| attributes block missing in ref | 7 | layout/other | `Magnificat__Prt.mscz` |
|  |  |  | `Vivaldi_-_Dixit_Dominus-RV_594__-_01_Dixit_Dominus.mscz` |
|  |  |  | `Vivaldi_-_Dixit_Dominus-RV_594__-_02_Donec_ponam_de_Antonio_Vivaldi.mscz` |
|  |  |  | `Vivaldi_-_Dixit_Dominus-RV_594__-_03-VirgamVirtutis_fredipi.mscz` |
|  |  |  | `Vivaldi_-_Dixit_Dominus-RV_594__-_06-Dominus_fredipi.mscz` |
|  |  |  | `Vivaldi_-_Dixit_Dominus-RV_594__-_07_Judicabit_in_nationibus.mscz` |
|  |  |  | `Vivaldi_-_Dixit_Dominus-RV_594__-_09_Gloria_Patri.mscz` |
| ('attributes child count', 2, 1) | 5 | layout/other | `Veni, Creator Spiritus_Courtial_v3_SATB.mscz` |
|  |  |  | `00- Veni veni Emmanuel - choeur.mscz` |
|  |  |  | `00- Veni veni Emmanuel - conducteur.mscz` |
|  |  |  | `02- Veni veni Emmanuel - Alto.mscz` |
|  |  |  | `06- Veni veni Emmanuel - Basson.mscz` |
| ('attributes child count', 4, 3) | 1 | layout/other | `Sancta_Maria_Mater_Dei_Mozart.mscz` |
| ('attributes child count', 8, 6) | 1 | layout/other | `00- Veni veni Emmanuel - chant piano.mscz` |
| ('staff-details', 'no', ('staff-lines',), None, ('diatonic', 'chromatic', 'octave-change')) | 1 | semantic | `07- Violoncelle et Contrebasse.mscz` |

### 32.2 Measured and reverted invisible-staff attempt

Following §32.1, the largest semantic sub-group is hidden-staff
`<staff-details print-object="no"/>`: 72 files have such a block in the
reference only. `print()` (`exportmusicxml.cpp:7559-7575`) adds every staff
with `!show()` to `m_hiddenStaves`, so a first attempt reported all invisible
staves at every explicit system start. The reduced test passed, but the full
oracle (`target/xml-invis/`) gave **35 gains and 29 losses**: the actual set
of hidden staves per system comes from MuseScore's automatic layout, which the
pivot model does not carry (a part with `<isStaffVisible>0</isStaffVisible>`
is not necessarily hidden on every system). The change was reverted. The
sub-classification stands; this group remains layout-bound, not fixable from
the model alone.

## 34. Layout classification measured, import-side sub-groups bounded

The oracle change (§33) was measured on the same 173-file lot with the same
binary (`target/xml-style/` → `target/xml-layout/`): core-identical is
**70/173 before and after**, 95 files lose core-diff lines and none gains.
Four core-identical files contain layout-hidden `<staff-details>` on both
sides (`03- Veni veni Emmanuel - Vcl et Cb`, `04- Douce nuit - Contrebasse`,
`03- Once in royal - Vcl - Cb`, `03- God rest you - Vcl et Cb`): they were
already identical before the reclassification, so the counter reports their
presence, not new agreement. N1/N2 remain 1.0000, 0 XSD-invalid.

Two import-side sub-groups were investigated and bounded:

- **Multi-measure rests** (`measure-style`, 2 files). `createMultiMeasureRestsIfNeed`
  (`mmrestlayout.cpp:723-760`) groups at least `minEmptyMeasures` (default 2,
  `legacy-style-defaults-v302.mss:329`) consecutive measures. A reduced test
  first, then a run-based port, still found no group in
  `Missa_in_Es_Hob._XXII4_I._Kyrie`: inspection of the model shows each measure
  carries **eight** full-measure rests (one per staff) together with notes on
  other staves. MuseScore groups multi-rests per `Staff`, while the pivot model
  stores one `Measure::multi_measure_rest` for the whole measure. The change
  was reverted; fixing it needs a per-staff field in the model and reader.
- **Layout-hidden staves** (`staff-details`, 41 files) remain layout-bound, as
  recorded in §32.2: the hidden set comes from `System`/`SysStaff` at automatic
  system starts, which the model does not carry.

Core-identical stays **70/173**, N1/N2 1.0000, 0 XSD-invalid throughout.

### 32.3 First-divergence inventory after the layout/empty-wrapper reclassification (2026-11-22)

Baseline `target/xml-emptyattr/`: 144/173 core-identical (was 70 with the old normalizer). One line per file: `file | first core-divergence category | core-diff lines`. Groups: `tuplet` is beams (closed), `attributes` and layout-only wrappers are not musical.

| Category | Files | Lines | File | Count |
| --- | ---: | ---: | --- | ---: |
| attributes | 13 | 1230 | `Missa_in_Es_Hob._XXII4_II._Gloria_-_Haydn.mscz` | 452 |
| attributes |  |  | `Missa_in_Es_Hob._XXII4_III._Credo_-_Haydn.mscz` | 273 |
| attributes |  |  | `Missa_in_Es_Hob._XXII4_VI._Agnus_Dei_-_Haydn.mscz` | 182 |
| attributes |  |  | `07- Veni veni Emmanuel - Trompettes 1, 2 et 3.mscz` | 90 |
| attributes |  |  | `05- Veni veni Emmanuel - Hautbois 1 et 2.mscz` | 78 |
| attributes |  |  | `04- Veni veni Emmanuel - Flûtes 1 et 2.mscz` | 46 |
| attributes |  |  | `06- Veni veni Emmanuel - Basson.mscz` | 32 |
| attributes |  |  | `01- Veni veni Emmanuel - Violons 1 et 2.mscz` | 26 |
| attributes |  |  | `Missa_in_Es_Hob._XXII4_IV._Sanctus_-_Haydn.mscz` | 25 |
| attributes |  |  | `02- Veni veni Emmanuel - Alto.mscz` | 20 |
| attributes |  |  | `05- Il est né - Hautbois.mscz` | 2 |
| attributes |  |  | `04- Once in royal - Hautbois 1 et 2.mscz` | 2 |
| attributes |  |  | `05- Once in royal - Basson.mscz` | 2 |
| frame | 1 | 558 | `bransle-de-chevaux-bransle-de-chevaux-rc-3.mscz` | 558 |
| measure-style | 2 | 241 | `Missa_in_Es_Hob._XXII4_I._Kyrie_-_Haydn.mscz` | 164 |
| measure-style |  |  | `Missa_in_Es_Hob._XXII4_V._Benedictus_-_Haydn.mscz` | 77 |
| clef | 1 | 186 | `Vingt_danses_enfantines_pour_piano.mscz` | 186 |
| harmony | 1 | 81 | `Adios_Muchachos.mscz` | 81 |
| staff | 4 | 48 | `Vivaldi_-_Dixit_Dominus-RV_594__-_01_Dixit_Dominus.mscz` | 18 |
| staff |  |  | `Vivaldi_-_Dixit_Dominus-RV_594__-_10._Sicut_erat_in_principio.mscz` | 14 |
| staff |  |  | `Vivaldi_-_Dixit_Dominus-RV_594__-_05_Juravit_Dominus.mscz` | 10 |
| staff |  |  | `Vivaldi_-_Dixit_Dominus-RV_594__-_09_Gloria_Patri.mscz` | 6 |
| tuplet | 2 | 16 | `Vivaldi_-_Dixit_Dominus-RV_594__-_08-DeTorrente_fredipi.mscz` | 14 |
| tuplet |  |  | `Vivaldi_-_Dixit_Dominus-RV_594__-_04-TecumPrincipium_fredipi.mscz` | 2 |
| time-modification | 1 | 8 | `Ave_Maria_Schubert_in_C_with_chords.mscz` | 8 |
| figured-bass | 1 | 3 | `JMHaydn_Viderunt_omnes.mscz` | 3 |
| notehead | 1 | 3 | `Angels_Carol__John_Rutter.mscz` | 3 |
| dot | 2 | 2 | `01- Minuit chretien - Violons 1 et 2.mscz` | 1 |
| dot |  |  | `06- Minuit chretien - Basson.mscz` | 1 |

## 35. Empty `<attributes>` wrappers and the re-checked mmrest claim

After the layout reclassification (§33/§34), the first divergence of 91 of
the 103 files was an **empty** `<attributes></attributes>` wrapper left behind
when its only child (`<staff-details print-object="no">`) had been removed.
Such an envelope has no musical content: the core normalizer now prunes empty
`<attributes>` recursively (unit test added). Full measurement
`target/xml-layout/` → `target/xml-emptyattr/` on the same 173 output pairs:
core-identical **70 → 144**, **95 files improved, 0 regressed**, N1/N2 1.0000,
0 XSD-invalid. The remaining 29 first divergences are tabled in §32.3.

The multi-measure-rest claim was re-checked against `Measure::isEmpty(staffIdx)`
(`measure.cpp:2286-2330`) and `validMMRestMeasure`/`createMMRest`
(`mmrestlayout.cpp:428-495, 723-760`). MuseScore groups at least
`minEmptyMeasures` (2) measures, skipping hidden staves, and a measure with
more than one rest segment is invalid. In `Missa_in_Es_Hob._XXII4_I._Kyrie`
every measure of the model carries both eight full-measure rests **and**
notes on other staves, so no run exists in the pivot model — even though the
reference writes 36 `<multiple-rest>` elements. The earlier run-based and
per-staff hypotheses were both reverted; a real port would need the
excerpt/staff layout relation, which the model does not carry.

A resolved-`headType()` experiment for `<notehead>` (`exportmusicxml.cpp:3897-3902`)
was also reverted: C++ keeps `HEAD_AUTO` for ordinary notes, and emitting
`filled="yes"` from the duration broke both e2e corpus tests.
