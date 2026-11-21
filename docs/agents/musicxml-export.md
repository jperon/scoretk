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

L'unique perte est `Tarentella_Napoletana_Accordion_Solo` (5 → 8 lignes) :
dans sa mesure 101, la référence n'écrit aucun `<forward>` alors que notre
export en écrit 2. L'héritage corrigé du 6/8 change la découpe des silences gap
de cette mesure ; le port de `toRhythmicDurationList` ne reproduit pas encore
tous ses paramètres (`maxDots`, `timeStretch`, contexte de n-olet,
`durationtype.cpp:511-537`). Le gain net reste de 655 lignes sur le lot et
cette mesure est la seule régression observée.

Reste : `attributes` 46 et `staff-details` 43 (portées masquées par la mise en
page, laissées pour la fin par priorité), puis des résidus de 1 à 3 fichiers
(`clef`, `tie`, `dot`, `measure-style`, `tuplet`, `staff`, `harmony`, `forward`,
`sound`). Pour le contenu musical porté par le modèle, l'export MusicXML est
déclaré terminé.
