# Performance : banc natif et profil de la chaîne complète

Ce document consigne la campagne de mesure de la façade `scoretk` — temps par
sens et par chaîne, points chauds, coût des allocations — puis les
optimisations qui en ont été tirées et leur effet. Les sections qui décrivent
les mesures (§ 3, § 5, § 6) valent pour l'état **avant** optimisation ; § 4
liste les changements et l'effet mesuré de chacun. Le noyau du projet (règles,
commandes, index) reste dans `AGENTS.md`.

Toutes les valeurs ci-dessous viennent de commandes réellement exécutées sur
cette machine (§ 2) ; chaque mesure est datée et accompagnée des conditions de
son exécution. Aucun chiffre n'est extrapolé d'une autre machine.

## 1. Méthode

### 1.1 Banc criterion

`crates/scoretk/benches/conversion.rs` (criterion 0.8.2 en dépendance de
développement, `[[bench]] harness = false`, licences dans
`docs/licences-tierces.md`) mesure onze groupes sur six fichiers du corpus :

- les deux plus gros `.mscx` en octets (`v3/moonlight`, 409 776 o ;
  `v4/voiceswitching`, 30 868 o) ;
- quatre fichiers qui couvrent les deux versions du format, plusieurs portées,
  plusieurs voix, les reprises et la mise en page
  (`v4/testExcludeInvisibleElements`, `v3/layout_elements`, `v3/voices`,
  `v4/hairpins_and_repeats`).

Les onze groupes sont : `lecture_mscx`, `lecture_musicxml`, `ecriture_musicxml`,
`ecriture_midi`, `ecriture_mscx`, `ecriture_mscz`, `ecriture_mxl`,
`ecriture_lilypond`, `ecriture_abc`, `chaine_mscx_musicxml` et `playback`
(équivalent de `convertWithTimeline`).

Les octets et les modèles sont chargés **hors mesure** (`OnceLock`, une fois
par processus) ; seules les fonctions de la façade sont chronométrées, et la
libération de la sortie est comprise dans la mesure (sémantique `iter` de
criterion). Chaque groupe déclare son débit en octets **d'entrée** (`.mscx`,
ou la référence MusicXML pour `lecture_musicxml`) : le débit se lit donc
« octets de partition source traités par seconde », pas « octets produits ».

```sh
nix develop -c cargo bench -p scoretk --bench conversion
```

Réglages : 20 échantillons, 1 s d'échauffement, 3 s de mesure par point,
**médiane** retenue, débit en Mo (10⁶ octets) par seconde. Le profil `bench`
hérite de `[profile.release]` (`opt-level = "z"`, `lto = true`,
`codegen-units = 1`) : les chiffres valent pour les binaires livrés, pas pour
une compilation `debug`.

Exécution retenue pour le tableau : 2026-11-18, 11 h 55–12 h 00, `loadavg`
4,3 → 7,8 sur 6 processeurs logiques, sans épinglage de cœur.

### 1.2 Profilage

Le profil porte sur la chaîne complète `.mscx` → MusicXML du plus gros fichier
(`chaine_mscx_musicxml/v3/moonlight`), avec le binaire de banc compilé avec
`CARGO_PROFILE_BENCH_DEBUG=2` et lancé en mode profilage criterion
(`--bench --profile-time 15`, ou `30` sous Valgrind).

- **Temps réel, self** : perf 7.3.1,
  `perf record -e task-clock -F 999 -- <banc> --bench --profile-time 15 "chaine_mscx_musicxml/v3/moonlight"`
  → 16 699 échantillons. Les call-graphs sont **inutilisables ici** : avec
  `--call-graph fp` ou `dwarf`, perf échoue à ouvrir l'événement
  (`kernel.perf_event_paranoid = 2` → `Failure to open any events`), d'où
  l'absence de colonne inclusive dans ce profil.
- **Inclusif** : Valgrind 3.26 / Callgrind,
  `valgrind --tool=callgrind --profile-time 30 …`, en **instructions (Ir)** sur
  1 940 637 507 instructions, dépouillé par
  `callgrind_annotate [--inclusive=yes]`. Les coûts inclusifs **se
  recouvrent** (on ne les additionne pas) et Callgrind répartit mal l'inclusif
  des fonctions récursives (`drop_glue` d'un arbre, `xml::element`) entre les
  membres du cycle : ces dernières sont écartées du tableau.
- **Allocations** : heaptrack 1.6.80 sur la CLI de release
  (`./target/release/scoretk tests/corpus/v3/moonlight.mscx -o …`), plus un
  compteur global (`GlobalAlloc` déléguant à `System`, binaire de mesure hors
  dépôt) compilé avec le même profil que la release, qui isole le coût de
  chaque appel de la façade (lecture du fichier hors mesure).

## 2. Machine et conditions

| Élément | Valeur |
| --- | --- |
| Processeur | Intel(R) Processor U300, 6 processeurs logiques, 8 Mio de cache |
| Cœurs | 2 threads à 4,4 GHz (P-core) + 4 cœurs à 3,3 GHz (E-cores) |
| Fréquence observée | 1,4 à 2,7 GHz pendant la campagne (`/proc/cpuinfo`) |
| Mémoire | 15,7 Gio |
| Système | NixOS, Linux 6.18.60, x86_64 |
| Chaîne Rust | rustc 1.100.0 (0ef1bc386 2026-11-09) |
| Charge (`loadavg`) | 4,3 à 11,3 sur 6 processeurs logiques |

La machine est **partagée** (navigateur, autres agents) : les médianes varient
d'un facteur 1 à 6 d'une exécution à l'autre et dérivent même en cours
d'exécution (§ 7). Un essai épinglé sur un E-core (`taskset -c 3`) a donné des
médianes jusqu'à 6 fois plus hautes pour le même point (48,3 ms contre 8,1 ms
pour la lecture `.mscx` de moonlight) : les valeurs retenues sont celles d'une
exécution sans épinglage.

## 3. Temps mesurés (avant optimisation)

| Mesure | `v3/moonlight` | `v4/voiceswitching` | `v4/testExcludeInvisibleElements` | `v3/layout_elements` | `v3/voices` | `v4/hairpins_and_repeats` |
| --- | --- | --- | --- | --- | --- | --- |
| lecture `.mscx` → modèle | 8,06 ms · 51 Mo/s | 0,75 ms · 41 Mo/s | 1,39 ms · 21 Mo/s | 0,46 ms · 54 Mo/s | 0,81 ms · 30 Mo/s | 0,50 ms · 20 Mo/s |
| lecture MusicXML → modèle | 14,60 ms · 43 Mo/s | 0,87 ms · 39 Mo/s | 1,23 ms · 23 Mo/s | 1,60 ms · 22 Mo/s | 1,20 ms · 26 Mo/s | 0,32 ms · 31 Mo/s |
| modèle → MusicXML | 28,64 ms · 14 Mo/s | 1,06 ms · 29 Mo/s | 0,49 ms · 60 Mo/s | 0,59 ms · 42 Mo/s | 0,42 ms · 58 Mo/s | 0,17 ms · 59 Mo/s |
| modèle → MIDI | 1,46 ms · 280 Mo/s | 0,06 ms · 548 Mo/s | 0,06 ms · 470 Mo/s | 0,04 ms · 681 Mo/s | 0,03 ms · 969 Mo/s | 0,03 ms · 316 Mo/s |
| modèle → `.mscx` | 3,16 ms · 130 Mo/s | 0,13 ms · 243 Mo/s | 0,13 ms · 231 Mo/s | 0,12 ms · 209 Mo/s | 0,19 ms · 131 Mo/s | 0,04 ms · 262 Mo/s |
| modèle → `.mscz` | 10,09 ms · 41 Mo/s | 0,83 ms · 37 Mo/s | 0,76 ms · 39 Mo/s | 1,00 ms · 25 Mo/s | 0,84 ms · 29 Mo/s | 0,45 ms · 22 Mo/s |
| modèle → `.mxl` | 19,51 ms · 21 Mo/s | 0,89 ms · 35 Mo/s | 0,98 ms · 30 Mo/s | 0,88 ms · 28 Mo/s | 0,93 ms · 26 Mo/s | 0,25 ms · 40 Mo/s |
| modèle → LilyPond | 1,67 ms · 245 Mo/s | 0,13 ms · 236 Mo/s | 0,05 ms · 545 Mo/s | 0,05 ms · 500 Mo/s | 0,13 ms · 196 Mo/s | 0,01 ms · 793 Mo/s |
| modèle → ABC | 1,73 ms · 236 Mo/s | 0,06 ms · 506 Mo/s | 0,06 ms · 468 Mo/s | 0,03 ms · 895 Mo/s | 0,06 ms · 406 Mo/s | 0,01 ms · 774 Mo/s |
| chaîne `.mscx` → MusicXML | 16,21 ms · 25 Mo/s | 2,14 ms · 14 Mo/s | 2,58 ms · 11 Mo/s | 1,13 ms · 22 Mo/s | 1,03 ms · 24 Mo/s | 0,64 ms · 15 Mo/s |
| `write_playback` | 22,71 ms · 18 Mo/s | 1,74 ms · 18 Mo/s | 1,57 ms · 19 Mo/s | 1,42 ms · 17 Mo/s | 2,05 ms · 12 Mo/s | 0,45 ms · 22 Mo/s |

Tailles des entrées : 409 776 o (moonlight), 30 868, 29 297, 24 855, 24 571 et
9 954 o.

Lecture du tableau :

- ordre de grandeur : un `.mscx` de 410 ko se lit en **8 à 13 ms** et s'exporte
  en MusicXML en **13 à 29 ms** (les deux extrêmes observés sur la campagne) ;
  les autres écritures restent sous 3,2 ms sur le même modèle ;
- les débits sont d'environ 20 à 60 Mo/s pour les sens XML, 130 à 260 Mo/s pour
  `.mscx`/LilyPond/ABC et 280 à 1 100 Mo/s pour MIDI ;
- **les lignes ne s'additionnent pas** : ici la chaîne complète (16,2 ms)
  ressort plus rapide que l'export MusicXML seul (28,6 ms), mesuré trois
  groupes plus tôt. La machine a accéléré en cours d'exécution (§ 7). Seuls
  les rapports entre fichiers d'un même groupe sont stables ;
- `write_playback` vaut à peu près l'export MusicXML seul plus le MIDI et la
  carte des temps.

## 4. Optimisations appliquées

Six commits ont été fusionnés le 2026-11-18, après la campagne de mesure
initiale. Le tableau de § 3 n'a **pas** été refait depuis : la machine est
restée chargée (loadavg 8 à 111 pendant les tentatives, cf. § 7) et une
ré-exécution aurait mesuré la contention, pas le code. Les effets ci-dessous
sont donc ceux mesurés au moment de chaque commit — banc ou profil de § 5 sur
le chaînage `moonlight` —, sans extrapolation d'un commit à l'autre.

- **`e983fa5` — table précalculée dans `tick_to_xml`** (`pitch.rs`) : la
  fonction reconstruisait, pour chaque note, les 65 candidats (13
  `DURATION_TYPES` × 5 points) et normalisait une `Fraction` par comparaison —
  26 % du temps d'export d'après la mesure du commit. Une table `OnceLock`
  remplie une seule fois, dans le même ordre, la remplace ; un test
  d'équivalence la compare à l'ancien algorithme. Effet mesuré : le poste
  `tick_to_xml` (**25,6 %** inclusif) et les `Duration::ticks` qu'il appelait
  (**29,6 %**, § 5.2) sortent du profil.
- **`e8fc8ce` et `eff23e7` — arithmétique de `Fraction` par chemin rapide
  `i64`** : PGCD et produits courts restent en `i64` et n'appellent plus les
  primitives 128 bits (**18,6 %** des instructions du chaînage, § 5.3) ;
  `eff23e7` corrige au passage la négation de `i64::MIN` sur ce chemin. Effet
  mesuré : `normalize` (**12,1 %** en self, **30,7 %** inclusif) et
  `__divti3`/`__umodti3`/`u128_div_rem` disparaissent du profil ; le gain
  porte sur les deux sens, l'arithmétique servant aussi à la lecture.
- **`374bd61` — index des spanners dans `NotationsState`** : `spanner_indices`
  ne balaie plus `score.spanners` en entier à chaque note et chaque mesure
  (`SpannerType::eq` **4,8 %** en self, itérateur **1,9 %**, **7,1 %**
  inclusif, § 5.1 et 5.2). L'index `(type, tick, track) → indices` est
  construit une fois par mesure.
- **`6bd58e6` — `zopfli` retiré du graphe de dépendances** : les features
  DEFLATE de `zip` 2.6.1 sont unifiées sur `deflate-flate2` + `flate2`
  (`miniz_oxide`), le niveau 6 ne l'utilisait de toute façon jamais (§ 5.3
  bis). Effet mesuré sur le paquet wasm : **1 546 119 → 1 474 491 o** bruts
  (−71,6 ko) et **640 218 → 616 877 o** en gzip (−23,3 ko).
- **`8b11c78` — arbre XML 3.x empruntant le texte source** : `read302::xml`
  recopiait chaque nom d'élément, chaque attribut et chaque nœud de texte ;
  `Node` porte désormais des `Cow<'a, str>` et `parse` emprunte le texte
  analysé (pistes 5 et 6, § 6 ; `push_text` **6,9 %** et `to_vec` **5,9 %**
  inclusifs). Effet mesuré au commit : lecture `.mscx` de `moonlight` de
  **6,83 à 4,94 ms**.
- **Essayé, non retenu — compaction de la table des instruments** (branche
  `perf-instr`) : **−13 ko** de paquet brut, mais **+189 o en gzip** : DEFLATE
  élimine déjà les chaînes dupliquées de la table, et la compaction ajoute de
  l'indexation. Écarté.

## 5. Points chauds

Profil pris **avant** les optimisations de § 4, au commit de la campagne :
c'est lui qui motive chacune d'elles. Les mesures de cette section ne sont pas
refaites après coup (part d'instructions et nombres d'allocations restent
cependant reproductibles, contrairement aux temps, cf. § 7).

### 5.1 Self en temps réel (perf, chaîne `moonlight`)

Part du temps CPU de la chaîne complète, 25 premières entrées :

| Part | Fonction |
| --- | --- |
| 8,22 % | `__umodti3` (reste 128 bits) |
| 8,16 % | `scoretk_core::model::fraction::normalize` |
| 5,33 % | `__divti3` (division 128 bits) |
| 4,90 % | `compiler_builtins::…::u128_div_rem` |
| 4,82 % | `<SpannerType as PartialEq>::eq` |
| 3,42 % | `_int_malloc` |
| 2,96 % | `quick_xml::reader::Reader::read_event_impl` |
| 2,62 % | `__memmove_avx_unaligned_erms` |
| 2,58 % | `exportmusicxml::Context::write_measure` |
| 2,49 % | `exportmusicxml::Context::new` |
| 1,92 % | itérateur de `notations::spanner_indices` (`Map<Filter<Enumerate<Iter<Spanner>>>>`) |
| 1,92 % | `__memcmp_avx2_movbe` |
| 1,86 % | `exportmusicxml::Context::chord` |
| 1,80 % | `Vec<read302::xml::Content>::push` |
| 1,79 % | `_int_free_create_chunk` |
| 1,78 % | `core::str::converts::from_utf8` |
| 1,77 % | `Vec<u8>::append_elements` |
| 1,49 % | `scoretk_musescore_import::read::read_score` |
| 1,45 % | `cfree` |
| 1,35 % | `malloc` |
| 1,32 % | `alloc::string::String::push` |
| 1,19 % | `exportmusicxml::stems::active_clef` |
| 1,18 % | `quick_xml::reader::XmlSource::read_with` |
| 1,15 % | `<Duration>::ticks` |
| 1,06 % | `drop_glue::<read302::xml::Content>` |

Le même classement se retrouve en instructions (Callgrind sans `--inclusive`) :
`normalize` 12,1 %, `__divti3` 9,4 %, `u128_div_rem` 5,6 %,
`SpannerType::eq` 4,8 %, `_int_malloc` 3,5 %.

### 5.2 Coût inclusif (Callgrind, instructions)

25 premières fonctions, hors infrastructure de banc, `main`, allocation brute
et glue de destruction récursive :

| Part des instructions | Fonction |
| --- | --- |
| 63,99 % | `scoretk::write` / `export_musicxml_with` |
| 54,50 % | `exportmusicxml::Context::write_measure` |
| 46,60 % | `exportmusicxml::Context::chord` |
| 35,48 % | `scoretk::read` / `musescore_import::read::read_score` |
| 30,70 % | `scoretk_core::model::fraction::normalize` |
| 29,59 % | `<Duration>::ticks` |
| 28,21 % | `fraction::from_i128` |
| 25,64 % | `musicxml_export::pitch::tick_to_xml` |
| 15,00 % | `__divti3` |
| 10,50 % | `<Fraction as Add>::add` |
| 10,10 % | `<Fraction as AddAssign>::add_assign` |
| 8,82 % | `<Fraction as MulAssign>::mul_assign` |
| 8,65 % | `quick_xml::reader::Reader::read_event_impl` |
| 8,49 % | `<Fraction as Mul>::mul` |
| 8,24 % | `exportmusicxml::Context::new` |
| 7,08 % | `musicxml_export::notations::spanner_indices` |
| 6,94 % | itérateur de `spanner_indices` |
| 6,94 % | `read302::xml::push_text` |
| 6,56 % | `read302::Reader::read_score` |
| 6,32 % | `read302::measure::read_voice` |
| 5,87 % | `<[u8]>::to_vec` |
| 5,56 % | `compiler_builtins::…::u128_div_rem` |
| 5,32 % | `Fraction::new` |
| 5,08 % | `__rust_alloc` |
| 4,42 % | `musicxml_export::writer::XmlWriter::tag` |

Autrement dit : **64 % du chaînage est l'export MusicXML, 35 % la lecture** ;
dans les deux sens, la même famille domine — l'arithmétique exacte des
fractions (`Fraction`, `Duration::ticks`), recalculée pour chaque note et
chaque position.

### 5.3 Parsing XML, `String`, `format!`

Parts **additives** (somme des coûts self en instructions, donc sans
recouvrement) sur les 1 940 637 507 instructions du chaînage `moonlight` :

| Poste | Part |
| --- | --- |
| Parsing XML : `quick_xml` + `memchr` (5,4 %) et construction de l'arbre `read302::xml` (3,3 %) | **8,6 %** |
| Arithmétique 128 bits (`__divti3`, `__umodti3`, `u128_div_rem`) | **18,6 %** |
| Module `fraction` seul (dont `normalize`) | **17,0 %** (12,1 % pour `normalize`) |
| `String` (`alloc::string::String`, dont `push` 1,5 %) | **2,0 %** |
| `format!` / `write!` (`core::fmt`, `alloc::fmt`) | **0,01 %** |
| Allocateur (`malloc`/`free`/libc) | **11,0 %** |
| `from_utf8` / `Utf8Chunks` / `str` | **4,3 %** |
| `memcpy`/`memmove`/`memcmp` | **3,0 %** |

Le formatage est donc **hors sujet** : les écrivains poussent des octets dans un
`Vec<u8>`/`String`, et les rares `format!` (identifiants `P<n>`, altérations
d'harmonie) restent sous 0,02 % des instructions. L'UTF-8, en revanche,
apparaît deux fois : validation à la lecture XML et `lossy` sur les textes.

### 5.3 bis — Compression des conteneurs (`.mscz`, `.mxl`)

À l'origine, la feature `deflate` de `zip` 2.6.1 tirait **aussi**
`deflate-zopfli` (d'où la présence de `zopfli` dans le graphe, cf.
`docs/licences-tierces.md`), mais le niveau par défaut (6) restait dans la
plage flate2 et le profil Callgrind montre
`<flate2::mem::Compress>::compress_vec` à 1,16 % des instructions, **aucun
appel à `zopfli`**. La compression passait donc déjà par **flate2 /
miniz_oxide**, pas par zopfli ; `6bd58e6` (§ 4) a depuis retiré `zopfli` du
graphe. Le coût d'écriture `.mscz` (10,09 ms) et `.mxl` (19,51 ms)
sur `moonlight` est dominé par la compression DEFLATE du `.mscx` (≈ 410 ko) ;
il est du même ordre que l'écriture MusicXML non compressée (20,73 ms).

### 5.4 Allocations

Une conversion complète `moonlight.mscx` → MusicXML, mesurée par le compteur
global (profil de release) :

| Opération | Allocations | Octets demandés |
| --- | --- | --- |
| lecture `.mscx` → modèle | 62 064 | 8,8 Mo |
| modèle → MusicXML | 23 682 | 2,4 Mo |
| chaîne `.mscx` → MusicXML | 85 746 | 11,2 Mo |
| lecture MusicXML → modèle | 71 707 | 8,5 Mo |
| `write_playback` | 27 584 | 3,7 Mo |
| modèle → `.mscx` | 46 438 | 2,7 Mo |
| modèle → LilyPond | 16 729 | 1,0 Mo |
| modèle → ABC | 8 610 | 0,4 Mo |
| modèle → MIDI | 1 346 | 0,7 Mo |

Sur un petit fichier (`v4/hairpins_and_repeats`, 10 ko), la même chaîne demande
2 953 allocations : le coût suit le nombre d'éléments XML, pas le nombre de
mesures.

Contrôle heaptrack sur la CLI (`scoretk moonlight.mscx -o moonlight.musicxml`) :
85 864 appels à l'allocateur, 11 632 allocations temporaires, **pic de tas
7,03 Mo** (17 × le `.mscx` d'entrée), 548 o non libérés en fin de processus,
0,14 s sous heaptrack (surcoût d'instrumentation compris).

La lecture concentre **72 % des allocations** (`push_text` seul : 20 871
allocations de texte recopié, plus ~10 300 vecteurs de nœuds), alors qu'elle ne
représente que 35 % des instructions : l'arbre XML intermédiaire
(`read302::xml::Content`/`Node`, un `Vec<u8>` par texte) est le poste
d'allocation dominant.

## 6. Pistes chiffrées

Chaque piste donne la mesure qui la motive et le **plafond** théorique sur le
chaînage `moonlight` (part des instructions ou des allocations à laquelle elle
peut toucher), pas un gain garanti. Les pistes 1 à 4 portent sur l'écriture,
5 et 6 sur la lecture : ce sont deux chantiers séparés. L'état de chacune est
indiqué en tête ; celles qui ont été suivies sont détaillées en § 4, avec leur
effet mesuré.

1. **Table statique des durées dans `tick_to_xml`** (`pitch.rs`) — **fait**
   (`e983fa5`, § 4) : pour chaque note, la fonction balayait les 13
   `DURATION_TYPES` × 5 points et construisait à chaque essai un
   `Duration::with_dots(...).ticks()`, donc une `Fraction` normalisée par
   comparaison. Coût mesuré : `tick_to_xml` **25,6 %** inclusif,
   `Duration::ticks` **29,6 %**. Plafond : la quasi-totalité de l'arithmétique
   128 bits (**18,6 %**).
2. **`Fraction::normalize` sans `u128`** — **fait** (`e8fc8ce`/`eff23e7`,
   § 4) : PGCD et produits passaient par `i128`/`u128` alors que les durées
   tiennent dans `i64` (dénominateurs ≤ 1024 dans le corpus). Coût mesuré :
   `normalize` **30,7 %** inclusif, **12,1 %** en self, plus
   `__divti3`/`__umodti3`/`u128_div_rem` (**18,6 %** au total). Plafond :
   jusqu'à ~15 % du chaînage si le chemin `i64` évite les divisions 128 bits.
3. **Mémoïser `Duration::ticks`** — **partiellement fait** : la valeur est
   pure et le `Duration` est immuable ; l'export la recalcule pour chaque note
   (`write_measure` 54,5 % inclusif). Plafond : la part des appels redondants,
   non isolée ici — au moins la moitié des **29,6 %** de `ticks`. La table de
   § 4 supprime les 65 comparaisons par note, mais `ticks` reste appelé une
   fois par note : la mémoïsation elle-même n'est pas faite.
4. **Indexer les spanners** — **fait** (`374bd61`, § 4) : `spanner_indices`
   parcourait `score.spanners` en entier à chaque note et chaque mesure
   (`SpannerType::eq` **4,8 %** en self, itérateur **1,9 %**) ; un index
   `(type, tick, track) → indices` construit une fois par mesure a remplacé
   ce balayage. Plafond : **7,1 %** inclusif.
5. **Supprimer ou alléger l'arbre XML intermédiaire** — **fait** pour le
   lecteur 3.x (`8b11c78`, § 4) : la lecture construisait `Content`/`Node`
   avec un `Vec<u8>` par texte (**72 %** des allocations), puis le détruisait ;
   le parsing pesait **8,6 %** des instructions et l'allocateur **11,0 %**.
   Plafond : la totalité du poste allocateur côté lecture, soit ~8 à 10 % du
   chaînage.
6. **Emprunter les textes XML** (`&[u8]` vers la source) au lieu de les
   recopier — **fait** pour le lecteur 3.x (`8b11c78`, § 4) : `push_text`
   **6,9 %** inclusif, `to_vec` **5,9 %**. C'était le moyen le plus direct pour
   la piste 5, avec le même plafond.
7. **`String` et `format!`** : **2,0 %** et **0,01 %** des instructions. Rien à
   gagner à court terme ; ne pas s'y attaquer.
8. **Priorité au sens** — **suivie** : l'export MusicXML représentait **64 %**
   du chaînage contre **35 %** pour la lecture ; les pistes 1 à 4 ont donc été
   traitées d'abord, les pistes 5 et 6 ensuite. Les autres écritures
   (MIDI, `.mscx`, LilyPond, ABC) restent sous 3,2 ms sur 410 ko : hors sujet
   pour l'instant.

## 7. Limites de la mesure

- **Pas de ré-exécution après optimisation** : le 2026-11-18, la machine a
  porté d'autres campagnes en parallèle (tests LilyPond, navigateurs headless,
  compilations) et le `loadavg` est resté entre 8 et 111 pendant les
  tentatives ; les médianes prises dans ces conditions (lecture `.mscx` de
  `moonlight` : 7,7 à 15,5 ms d'une exécution à l'autre) ne séparent pas le
  gain des optimisations de la contention. Le tableau de § 3 reste donc celui
  de la campagne initiale, et § 4 s'appuie sur les mesures faites au moment de
  chaque commit.
- **Variabilité de la machine** : le même point (lecture `.mscx` de
  `moonlight`) a donné 8,1 ms, 12,7 ms et 48,3 ms de médiane selon l'exécution,
  et la dérive continue en cours d'exécution — au point que le tableau de § 3
  contient une chaîne complète plus rapide que l'export seul mesuré trois
  groupes avant. Les temps absolus ne sont **pas** reproductibles à quelques
  pour cent près sur cette machine ; les parts d'instructions (§ 5.2, 5.3) et
  les nombres d'allocations (§ 5.4) le sont.
- Une seule machine, partagée, sans épinglage stable (les E-cores à 3,3 GHz
  sont 1,3 × plus lents que le P-core et étaient occupés par d'autres
  processus) ; aucun réglage de gouvernance CPU n'est possible ici.
- Le corpus est petit (410 ko au maximum) : la mesure décrit des partitions de
  quelques dizaines de milliers de notes, pas un livre d'orchestre.
- `opt-level = "z"` et `lto = true` (profil de livraison) changent l'inlining :
  perf et Callgrind ne voient que les fonctions qui restent après
  optimisation, et certaines sont regroupées (`Context::new` absorbe plusieurs
  constructeurs).
- Callgrind compte des instructions, pas des cycles : il ignore les défauts de
  cache et les branchements mal prédits ; perf, lui, ne peut pas produire de
  call-graphs dans cet environnement.
- Les mesures `iter` de criterion incluent l'allocation **et** la libération
  de la sortie : c'est le coût complet de l'appel, pas seulement le calcul.
