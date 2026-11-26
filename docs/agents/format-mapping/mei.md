# MEI 5.1 ↔ scoretk model

Reference documents (outside the repository): `specs/README.md` § MEI;
`specs/mei/` — guidelines index, introduction, Common Music Notation
(`cmn.html`), and the normative schema `mei-all-5.1.rng`. Format reference:
[`format-ref/mei.md`](../format-ref/mei.md). Implementation to port:
`ref-musescore/.474/src/importexport/mei/` (MuseScore **4.7.4**, the
compliance target). Section numbers `§x.y` refer to MEI 5.1's CMN chapter.

**Status.** Reader and writer now exist in `scoretk-mei-{common,import,export}`
and are ported from MuseScore 4.7.4 (see [`mei.md`](mei.md)). The *Model* column
gives the pivot capability
(`crates/scoretk-core/src/model`, file:line) and the **Status** column says
whether the port is **direct** (existing field, no semantics
lost), **conversion** (existing field but a computed mapping or an
approximation), or **missing** (no field today). Layers are those of
`docs/agents/model-modularisation.md` §5.1: L0 core, L1 metric, L2
events, L3 text, L4 playback, L5 engraving, L6 format extension.

"MuseScore" is the 4.7.4 C++ reference: `meiimporter.cpp` (import),
`meiexporter.cpp` (export), `meiconverter.cpp` (element/attribute helpers),
all under `ref-musescore/.474/src/importexport/mei/internal/`.

| MEI property (spec) | Model type / field (file:line) | Status | Layer | MuseScore 4.7.4 (C++ file:line) | Notes |
| --- | --- | --- | --- | --- | --- |
| `<mei meiversion>` container | — | direct | L0 | `meimodule.cpp:46-60` (.mei only); `meiimporter.cpp:1011-1047` | Plain UTF-8 XML, no container. Reader must accept both `mei-basic` and `mei-all`. |
| `meiHead` titles, `persName@role`, availability, date (§header) | `ScoreMetadata` `work_title`, `subtitle`, `composer`, `arranger`, `lyricist`, `translator`, `copyright` (`score.rs:51-91`) | direct | L3 | `meiimporter.cpp:933-1010`; `meiexporter.cpp:198-258` | `title[@type="subordinate"]` → subtitle; `pubStmt/availability` → copyright; encoding date has no dedicated field (only in the head blob). |
| Entire `meiHead` verbatim | — | missing | L6 | `meiimporter.cpp:940-948` (meta tag `meiHead`); `meiexporter.cpp:198-211` | MuseScore round-trips the whole header as an opaque string. Needed for `workList`, `sourceDesc`, `encodingDesc`. |
| `<staffGrp symbol="bracket\|brace">`, `bar.thru` | `StaffBracket { kind, span, visible }` (`part.rs:341-347`), `Staff::brackets` (`part.rs:385`) | conversion | L5 | `meiconverter.cpp:3122-3149`; `meiimporter.cpp:1293-1345` | MuseScore reduces the group to a bracket kind + span; nested groups lose ordering. |
| `<staffDef n lines>` | `Staff::lines` (`part.rs:361`), `Staff::id` (`part.rs:357`) | direct | L0 | `meiconverter.cpp:3064-3121` | `@lines="1"` gives a 1-line percussion staff. |
| `<label>` / `<labelAbbr>` | `Part::long_name` / `Part::short_name` (`part.rs:253-255`), `Staff::default_clef` (`part.rs:391`) | direct | L3 | `meiexporter.cpp:737-766`; `meiimporter.cpp:3390-3436` | MuseScore also rebuilds parts from `staffGrp`/`staffDef` labels; the pivot's part grouping is explicit. |
| `staffDef@keysig`, `scoreDef@keysig`, mid-score `<keySig sig>` | `KeySig { concert_key, transposed_key, mode, courtesy, visible }` (`segment.rs:118-129`) | conversion | L1 | `meiconverter.cpp:2066-2090`; `meiimporter.cpp:1052-1101`, `:1226-1292` | `keysig="3s"` maps to `Key(3)`; modal/`x`/`ff` signatures reduced to a fifths count; `mode` has no MEI source in MuseScore's output. |
| `staffDef@meter.count/@meter.unit/@meter.sym` | `TimeSig` (`segment.rs:166-180`), `Measure::time_sig_numerator/denominator` (`measure.rs:71-73`) | direct | L1 | `meiconverter.cpp:2286-2334`; `meiimporter.cpp:2295-2320` | `common`/`cut` → `TimeSigType`; additive meters (`meter.count="3+2"`) are not read (warning). |
| `<clef shape line>` and mid-measure `<clef>` | `Clef { concert, transposed }` (`segment.rs:377-382`), `ClefType` (`segment.rs:250-302`) | conversion | L1 | `meiconverter.cpp:810-1047`; `meiimporter.cpp:1879-1909` | MEI `shape` G/F/C + `line` + `@dis`/`@dis.place` → `ClefType`; unusual shapes (percussion, TAB) only partially covered. |
| `staffDef@trans.diat/@trans.semi` | `Transpose { diatonic, chromatic }` (`part.rs:135-140`) | direct | L4 | `meiconverter.cpp:3064-3084`; `meiexporter.cpp:666-736` | Semantics are "written → sounding", same as the pivot. |
| `<instrDef @midi.instrnum @midi.channel @volume @pan>` | `Instrument::midi_program/channel/volume/pan` (`part.rs:71-97`), `InstrumentChannel` (`instrument_channel.rs:35-55`) | direct | L4 | `meiimporter.cpp:2849-2866`; `meiexporter.cpp:767-792` | Program is 0-based in MEI, 0-based in the pivot. |
| `scoreDef` inside a `section` (parameter change) | `KeySig`/`TimeSig`/`Clef` segments at any tick (`segment.rs:118-486`) | direct | L1 | `meiimporter.cpp:1052-1101`, `:1346-1393`; `meiexporter.cpp:520-610` | Mid-score changes are segments; the pivot handles per-staff values, unlike ABC. |
| `<section>` | — | missing | L0 | `meiimporter.cpp:1346-1393` | Structural only; MuseScore flattens it. Could stay unreported (no musical content). |
| `<measure @n @metcon>` | `Measure::number`, `number_offset`, `nominal_len` (`measure.rs:56-78`) | conversion | L1 | `meiconverter.cpp:2211-2253`; `meiimporter.cpp:1434-1501` | `@n` only parsed when it starts with a digit; missing `@n` ⇒ irregular measure; `@metcon="false"` is written when `ticks != timesig`. |
| `<measure @left @right>` barlines and repeats | `Measure::start_bar_line/end_bar_line` (`measure.rs:80-83`), `BarLineType` (`segment.rs:413-487`) | direct | L1 | `meiconverter.cpp:574-625`, `:2211-2253` | `rptstart`/`rptend`/`rptboth` set start/end repeat flags; `dotted`, `dashed`, `invis` map to `BarLineType`. |
| `<measure type="mscore-repeat-N">` | `Measure::repeat_count` (`measure.rs:87`) | conversion | L1 | `meiconverter.cpp:2211-2253` (`mscore-repeat-` prefix, `meiconverter.h:80`) | MuseScore stores the count in `@type`; the pivot has `u8`. |
| `<ending n="1,2">` | `SpannerType::Volta` (`spanner.rs:268-276`) + `VoltaInfo { endings, closed }` (`repeat.rs:282-287`), `Measure::repeat_marks` (`measure.rs:129`) | conversion | L1 | `meiimporter.cpp:1394-1433`; `meiexporter.cpp:796-833` | The volta is a spanner over the measures of the ending; openness is inferred from `@n` lists. |
| `<staff n>` | `StaffId` (`ids.rs:78-85`), `Staff::id` (`part.rs:357`) | direct | L0 | `meiimporter.cpp:1544-1612`, `:3309-3347` | `@n` is 1-based and authoritative; build the map before parsing. |
| `<layer n>` | `Track = staff*4 + voice` (`ids.rs:118-140`, `VOICES` `:31`) | conversion | L0 | `meiimporter.cpp:1613-1653`, `:224-241` | `@n` 1-based → voice 0-based; **more than 4 layers per staff is not representable** (report as `Loss`). |
| Implicit tick of layer events | `Segment::tick` (`segment.rs:715-721`), `ChordRest::ticks` (`chord.rs:341-348`) | conversion | L0 | `meiimporter.cpp:1654-1699`, `:242-379` | MEI has no tick attribute on events; the reader accumulates exact `Fraction` ticks. Lossless for durations that divide the beat. |
| `@tstamp`, `@tstamp2`, `@dur` on control events | `SpannerAnchor.tick`, `SegmentAnnotation.tick` (`spanner.rs:39-44`, `segment.rs:679-685`) | conversion | L1 | `meiconverter.cpp:3828-3878` | `@tstamp` is a **1-based float beat index**; the importer approximates by continued fractions (precision 0.01). Needs a documented rounding policy; prefer `@startid`/`@endid` when present. |
| `<space>` | `Rest { gap: true, visible: false }` (`chord.rs:313-320`) | direct | L0 | `meiimporter.cpp:2160-2178` | Advances time without a symbol; `Rest::gap` is exactly the pivot equivalent. |
| `<note @pname @oct>` (§4.2.5) | `Note::pitch`, `tpc` (`note.rs:266-293`) | conversion | L0 | `meiconverter.cpp:2765-2819` | Middle C is `pname="c" oct="3"`; `pitch = step + alter + (oct+1)*12`. |
| `<note @dur @dots>` | `DurationType` (`duration.rs:37-99`), `Duration.dots` (`duration.rs:159-164`), `ChordRest::duration` (`chord.rs:341`) | direct | L0 | `meiconverter.cpp:2916-2959` | `@dur` is a denominator (`4` = quarter); `dots` 0–3. `0` = `DurationType::Zero` (grace/none). |
| `<accid @accid>` written, `@accid.ges` sounding | `Note::accidental: Option<Accidental>` (`note.rs:232-246`, field `:296`), `AccidentalType` (`note.rs:201-231`) | conversion | L0 | `meiconverter.cpp:160-232`, `:2765-2874` | Written accidental wins; `.ges` supplies the alteration when the written one is absent. `user` flag = `@accid` present. |
| `<accid @enclose="brack\|paren">` | `Accidental::bracket`, `Accidental::parentheses` (`note.rs:236-238`) | direct | L5 | `meiconverter.cpp:2765-2874` | Two booleans replace the MEI enumeration. |
| `@oct.ges`, `@pitch`, `@pname`/`@oct` divergence | `Note::pitch` + `PlaybackTransposition` (`playback.rs:40-42`), `Note::unperformed` (`note.rs:322`) | partial | L1/L4 | `meiconverter.cpp:2820-2874` (export only) | MuseScore exports `@oct.ges` when played pitch differs, but does not import it. The pivot separates written pitch and playback transposition globally, not per note. |
| `<chord @dur @dots>` | `Chord::notes` (`chord.rs:220-262`) | direct | L0 | `meiimporter.cpp:1835-1878`; `meiexporter.cpp:1237-1314` | Notes in a chord share the chord duration. |
| `<rest>`, `<mRest>`, `<mRpt>` | `Rest` (`chord.rs:313-320`), `Rest::full_measure`, `Measure::multi_measure_rest` (`measure.rs:93`), `Measure::measure_repeat` (`measure.rs:118`) | direct | L1 | `meiimporter.cpp:1972-2039`, `:2139-2178`; `meiexporter.cpp:1455-1528` | `mRest` = whole-measure rest; `mRpt` = one-measure repeat symbol, never expanded. |
| `<beam>` container | `ChordRest::beam_mode` (`chord.rs:354-356`), `BeamMode` (`chord.rs:45-69`) | conversion | L5 | `meiconverter.cpp:626-676`; `meiimporter.cpp:1784-1811` | A container is turned into per-event `BEGIN/MID/END/NONE`; beaming is not an object in the pivot. |
| `@type="mscore-beam-begin\|mscore-beam-mid\|mscore-beam-none"`, `@breaksec` | `BeamMode::Begin16/Begin32`, `ChordRest::beam_mode` | conversion | L5 | `meiconverter.cpp:626-691` | Non-Basic short form read by MuseScore; `@breaksec` 1/2 only. |
| `<tuplet @num @numbase>` | `Tuplet { ratio, base_len, tick, len, parent }` (`tuplet.rs:53-81`), `ChordRest::tuplet` (`chord.rs:350`) | conversion | L1 | `meiconverter.cpp:3595-3670`; `meiimporter.cpp:2200-2246` | Ratio is `num/numbase`; base length and span are **derived** from the contained events. Nested tuplets rejected by 4.7.4. |
| `<bTrem @unitdur>`, `<fTrem>` | `Tremolo { kind, count }` (`notation.rs:200-227`), `Chord::tremolo` (`chord.rs:253`), `Chord::tremolo_two_chord` (`:249`) | conversion | L2 | `meiconverter.cpp:2960-2996`, `:3193-3218`; `meiimporter.cpp:1812-1834`, `:1910-1932` | Single-chord tremolo by `@unitdur`; two-chord tremolo halves the written values. |
| `<graceGrp grace="acc\|unacc">`, `@grace` on note | `GraceType` (`chord.rs:87-107`), `Chord::grace`/`grace_slash` (`chord.rs:240-242`), `GraceChord` (`chord.rs:268-281`) | conversion | L2 | `meiconverter.cpp:1700-1729`; `meiimporter.cpp:1938-1971`, `:3217-3239` | `acc` = acciaccatura (slashed), `unacc` = appoggiatura. MuseScore stacks grace chords on the following chord. |
| `<tie @startid @endid>` | `Note::tie_forward`, `tie_back` (`note.rs:276-294`) | conversion | L2 | `meiconverter.cpp:3526-3544`; `meiimporter.cpp:3111-3146`, `:742-786` | Ties are per-note ids in MEI; the pivot stores both ends per note. A chord tie must be exploded into per-note ties. |
| `<slur>`, `<phrase>` | `SpannerType::Slur`, `Spanner::number` (`spanner.rs:268-330`), `Slur` notation (`notation.rs:387-403`) | conversion | L2 | `meiconverter.cpp:2997-3063`; `meiimporter.cpp:3058-3082` | `@curvedir` → placement; slurs can nest (`number`). |
| `<dynam>`, `@label`, `<rend>` | `Dynamic { text, velocity, placement }` (`annotation.rs:197-228`) | conversion | L3 | `meiconverter.cpp:1275-1405` | MuseScore writes `@label` + text; velocity is lost (MEI has `@velocity` only in `mei-all`). |
| `<dir>` (staff/system text, text line) | `StaffText` (`annotation.rs:231-249`), `SystemText` (`annotation.rs:292-316`), `TextLineInfo` (`spanner.rs:197-248`) | conversion | L3/L5 | `meiconverter.cpp:1133-1274`; `meiimporter.cpp:2511-2547` | `@type` selects `mscore-staff-text` / `mscore-system-text` / `mscore-hairpin` (a text line); `@extender` + `@tstamp2` makes a line. |
| `<tempo @midi.bpm @mm>`, text | `Tempo { text, beats_per_second, follow_text, kind }` (`annotation.rs:322-360`) | conversion | L3/L4 | `meiconverter.cpp:3327-3390`; `meiimporter.cpp:3083-3110` | `@midi.bpm` is BPM; the pivot stores beats/second and the display text separately. |
| `<reh>` | `RehearsalMark { text, frame }` (`annotation.rs:370-386`) | direct | L3 | `meiimporter.cpp:2998-3028`; `meiexporter.cpp:2085-2111` | — |
| `<hairpin @form>` | `SpannerType::Hairpin(HairpinType)` (`spanner.rs:57-67`), `Hairpin` (`hairpin.rs:41-60`) | direct | L2 | `meiconverter.cpp:1730-1799` | `cres`/`dim`; `@tstamp2`/`@endid` for the end. |
| `<pedal>` | `SpannerType::Pedal` + `PedalInfo` (`spanner.rs:268-276`, `:81-113`) | conversion | L2 | `meiconverter.cpp:2693-2764` | `@dir`/`@func`; MEI has no line/hook split, so `line_visible` is a port decision. |
| `<octave @dis @dis.place>` | `SpannerType::Ottava(OttavaType)` (`spanner.rs:68-80`) | conversion | L2 | `meiconverter.cpp:2462-2567` | `8`/`15`/`22` × above/below → `OttavaType`. |
| `<trill>`, `<mordent>`, `<turn>`, `<ornam>` | `Ornament { kind, placement, accidentals, intervals }` (`notation.rs:158-183`), `Trill` (`notation.rs:346-383`) | conversion | L2 | `meiconverter.cpp:2335-2461` (mordent), `:2568-2692` (ornam), `:3545-3594` (trill), `:3671-3730` (turn); `meiimporter.cpp:2896-2921`, `:2947-2972`, `:3147-3216` | MuseScore splits "trill" into a `Trill` spanner and the others into `Ornament`; the pivot does the same. |
| `<artic @artic>`, chord-line articulations | `Articulation { kind, anchor, up, play }` (`chord.rs:187-205`), `ArticulationType` (`chord.rs:125-185`) | conversion | L2 | `meiconverter.cpp:306-573`; `meiimporter.cpp:1700-1783` | A subset (`stacc`, `acc`, `ten`, `marc`, `spicc`, `port`, …); unknown names must be reported. |
| `<arpeg @arpeg>` | `Arpeggio { kind }` (`notation.rs:229-259`) | direct | L2 | `meiconverter.cpp:254-305` | up/down. |
| `<fermata @form>` | `Fermata { kind, placement }` (`notation.rs:48-98`) | conversion | L2 | `meiconverter.cpp:1517-1613` | Shape + placement → `FermataType`. |
| `<breath>`, `<caesura>` | `Breath { kind, placement, pause_ms }` (`segment.rs:488-533`) | direct | L2 | `meiconverter.cpp:692-809` | `@place` → placement; `pause_ms` has no MEI source. |
| `<gliss @lform>` | `Glissando { kind, text, show_text }` (`notation.rs:307-340`) | conversion | L2 | `meiconverter.cpp:1654-1699` | `@lform` (`solid`/`dashed`/`dotted`/`wavy`) → `GlissandoType`. |
| `<fing @place>` | `Fingering { text, placement }` (`notation.rs:431-445`), `Note::fingerings` (`note.rs:351`) | direct | L3 | `meiconverter.cpp:1614-1653` | String numbers are exported as fingerings of style `STRING_NUMBER`; MEI has no distinct string-number element. |
| `<harm>`, `<rend>` text | `Harmony { text, name, root_tpc, bass_tpc, kind, degrees }` (`annotation.rs:428-478`) | conversion | L3 | `meiconverter.cpp:1800-1863`; `meiimporter.cpp:2795-2823` | MEI writes the chord symbol as text; the structured fields can only be filled by parsing that text. |
| `<fb>` + `<f>` (figured bass) | `FiguredBass { items, ticks, place_above, parentheses }` (`figured_bass.rs:118-165`) | conversion | L3 | `meiconverter.cpp:1471-1516`; `meiimporter.cpp:2577-2648` | MuseScore uses MEI `<harm>`+`<fb>`; the pivot has a structured figured-bass type. |
| `<harpPedal>` | — | missing | L6 | `meiconverter.cpp:1864-1961`; `meiimporter.cpp:2824-2848` | Not in the pivot; L6 extension or `Loss`. |
| `<lv>` laissez vibrer | — (no `LaissezVib` type) | missing | L6 | `meiconverter.cpp:1962-1981` | MuseScore has a dedicated symbol; the pivot does not. |
| `<repeatMark @func>` | `Marker`/`Jump` (`repeat.rs:109-240`, `:269-280`), `RepeatMark` (`repeat.rs:269`) | conversion | L1 | `meiconverter.cpp:1982-2210`; `meiimporter.cpp:3029-3057` | `@func` `daCapo`/`dalSegno` → `Jump`; `coda`/`segno`/`fine` → `Marker`. Text is regenerated from the type, not carried. |
| `<verse n>` / `<syl @con>` | `Lyrics { verse, syllabic, text, melisma }` (`lyrics.rs:36-63`) | conversion | L3 | `meiconverter.cpp:3219-3297`; `meiimporter.cpp:2248-2357` | `@con` `d/u/m/t` → `LyricsSyllabic`; melisma is an explicit `Fraction` in the pivot, inferred from `@wordpos`/empty syls in MEI. |
| `<pb>`, `<sb>` | `Measure::line_break`, `section_break_before` (`measure.rs:100-128`) | conversion | L5 | `meiimporter.cpp:1502-1543`; `meiexporter.cpp:259-323` | Only user-typed breaks; page breaks are a layout option. |
| `<pgHead>` | `Credit` (`text.rs:586-…`), `TboxText` (`text.rs:417`) | conversion | L5/L3 | `meiexporter.cpp:386-459`; `meiimporter.cpp:1102-1153` | MuseScore turns the first-page head into a title `VBox`; the pivot's `Credit` is the closest equivalent. |
| `@color` | — (no color field) | missing | L5 | `meiconverter.cpp:1048-1111` | MuseScore reads/writes colour on most objects; the pivot drops it. Report as L5 loss or add a colour field. |
| `xml:id` | — (stable ids are generated, `stable_ids.rs`) | missing | L6 | `meiimporter.cpp:907-932`; `meiexporter.cpp` (`getXmlIdFor`) | Needed for exact MEI→MEI round-trip and for `@startid`/`@endid`. The pivot derives ids from position, which changes when a measure is inserted. |
| `@copyof`, `@sameas` | — | missing | L6 | not read (`meiimporter.cpp` dispatch has no case) | Identity/alias relations; must be reported, not silently merged. |
| `@facs`, `<facsimile>`, `<timeline>`, `<expansion>` | — | missing | L6 | not read | Facsimile linking, gestural timeline and variant expansions are scholarly-editing features with no pivot analogue. |
| `@dur.ges`, `@tstamp.ges`, `@tstamp.real`, `@ppq` | — (single time axis: `ChordRest::duration`+`ticks`) | missing | L1 | not read; `@dur.ges`/`@tstamp.ges` are defined in `cmn.html:1140`, `:3925` | The written/perform distinction is the main MEI model gap (§7.2 of `model-modularisation.md`). |
| `@staff` on a note/chord (cross-staff) | `Chord::staff_move` (`chord.rs:238`) | direct | L5 | `meiimporter.cpp:242-379` (staff move limited to ±1) | MuseScore also limits to one staff; report larger moves. |
| `@stem.dir`, `@stem.mod` | `Chord::stem` (`chord.rs:233`), `StemDirection` (`chord.rs:72-85`), `Tremolo` (`notation.rs:200-227`) | direct | L5 | `meiconverter.cpp:3150-3218`; `meiimporter.cpp:3240-3261` | `@stem.mod` values `1slash`…`4slash`, `z` map to tremolo counts. |
| `@head.shape` / `@head` | `Note::head`, `head_group` (`note.rs:298-300`), `NoteHeadGroup` (`note.rs:138-200`) | conversion | L5 | not in `meiconverter.cpp` | MuseScore writes the shape in `@head.shape` for percussion; reading it is optional for the first port. |

## Neume Notation (MEI 5.1 `mei-all`, extension additive)

MuseScore 4.7.4 ne porte pas ce module ; les porteurs sont cités dans
*Guidelines* ch. 6 (§6.1, §6.1.1, §6.4.1–§6.4.10) et dans le schéma
`mei-all-5.1.rng`. Les colonnes `MuseScore` ci-dessus ne s'appliquent pas à
ces lignes. Le flux source est le payload L6 `Gregorian`
(`model/gregorian.rs`) ; la vue L0/L1 reste dérivée.

| MEI property (spec) | Model type / field | Status | Layer | Notes |
| --- | --- | --- | --- | --- |
| `<section>/<staff>/<layer>/<syllable>` | `Gregorian::syllables` (`model/gregorian.rs:288`) | direct | L6 | Organisation hiérarchique du chant ; §6.1, §6.4.10. |
| `<syl @wordpos @con>`, `<rend>` | `Syllable::text` (`:248`), `space_after` | direct | L3/L6 | Texte exact, balises GABC incluses ; `@con="s"` = espace de mot, `@con="t"` = élision ; `<i>/<b>/<c>/<sc>/<tt>/<ul>` → `<rend @fontstyle/@fontweight/@color>` (§6.4.10). |
| `<neume type>` / `<nc @pname @oct>` | `GregorianNote::letter` (`:230`) | conversion | L6 | Un `<neume>` par groupe GABC ; type = nom de neume (`punctum`, `pes`, `clivis`, …) ; clé GABC → degré diatonique → `@pname/@oct` (do moyen = `c`/`4`) ; réciproque au retour. |
| `<nc @tilt @curve @con @type>` + `<liquescent>`, `<quilisma>`, `<oriscus>`, `<strophicus>`, `<signifLet>` | `GregorianNote::glyph` (`:232`), `NeumeSign { kind, raw }` (`:208`) | conversion | L2/L6 | La casse majuscule (punctum inclinatum) est `@type="punctum-inclinatum"`/`@tilt="se"` ; les glyphes et signes sont décomposés en attributs/éléments MEI (§6.4.1, §6.4.2, §6.4.8) et reconstruits par `neumes::glyph_parts` / `part_from_mei` ; **aucune annotation requise**. |
| `<episema form="h|v" place>` | `SignKind::HorizontalEpisema` / `Ictus` (`:177-179`) | conversion | L2/L6 | `h` = `_`, `v` = `'` ; placement et chiffres dans `@place`/`@n` (§6.4.4). Le punctum mora passe par `<signifLet type="PunctumMora">` (§6.4.9). |
| `<divLine form>` | `BarKind` (`:59`) | conversion | L1/L6 | Valeurs suggérées + NMTOKEN (`virgula-ledger`, `minima-ledger`, `maior-dotted`, `dominican-1`…`8`) : réversible, aucun jeton atomique nécessaire (RNG `att.divLine.log/@form`). |
| `<clef shape="C|F" line @type="clef-flat">`, `<staffDef @lines>` | `Gregorian::clefs` (`:281`), `staff_lines` (`:277`) | conversion | L1/L6 | `cb3` → `@type="clef-flat"` ; **la clé double `c1@c4` n'a pas de porteur** (`mei_clefGrp` n'est pas admissible dans `staffDef`). |
| `titleStmt/title`, `respStmt/persName`, `workList/work/title`, `classification/termList/term`, `sourceDesc/source/bibl/rend` | `GregorianHeader { name, value }` (`:46`) | direct | L6 | `@n` = index source, `@type`/`@role` = nom du champ (Guidelines ch. 3) ; les `<annot type="gabc-header">` ne sont plus émis par défaut. |
| `<custos>` | — | missing | L6 | MEI 5.1 le définit (§6.4.3) mais le payload GABC ne porte pas de custos (`gabc.md` §10) : aucun champ à alimenter. |
| `<accid>` sur `<nc>` | `NeumeSign::Alteration` | missing | L6 | Le module neumes ne définit pas `<accid>` enfant de `<nc>` ; `x`/`X`/`#`/`y` sont encodés comme `<signifLet type="Alteration" n="flat|sharp|natural…">` (§6.4.9), pas comme des notes chantées (`Gabc.tex:512-540`). |

## NABC (manuscript neumes) ↔ MEI 5.1, second unpitched staff

Sources, read before writing this table (2026-11-26):

- **MEI 5.1 *Guidelines*, chapter 6 "Repertoire: Neume Notation"**, read at
  <https://music-encoding.org/guidelines/v5/content/neumes.html> (the local
  copy under `~/programmes/musique/specs/mei/` holds only the index, the
  introduction and the CMN chapter). §6.1 defines `<syllable>` as the basic
  unit, §6.1.1 the four elements `<syllable>/<syl>/<neume>/<nc>`; §6.3 notes
  that regional styles (St. Gall, Old Hispanic, Aquitanian) are *not*
  reducible to one notation and shows adiastematic encodings with unpitched
  `<nc>` plus `@intm` (Listings 244–247, 253); §6.4.1 lists `<signifLet>`
  among the neume elements and shows `<signifLet place="above-right">c</signifLet>`
  inside `<nc>` (Listing 282); §6.4.10 states that staff information is omitted
  from the skeleton "for brevity", so those Listings are fragments, not
  Schematron-valid documents; §4.2.2 "Re-definition of Score Parameters"
  (<https://music-encoding.org/guidelines/v5/content/cmn.html#cmnReDef>)
  sanctions `staffDef` inside a staff under MEI All (not recommended for CMN);
  §6.4.2 defines `@pname`, `@oct`, `@intm`,
  `@con`, `@curve`, `@tilt`; §6.4.4 `<episema @form @place>`; §6.4.5
  `<liquescent @curve @looped>`; §6.4.7 `<quilisma @waves>`; §6.4.8
  `@place` on `<signifLet>`; §6.4.9 allows `glyph.auth/@name/@num/@uri`
  for marks without a dedicated element; §6.4.10 gives the basic skeleton.
- **Normative schema** `~/programmes/musique/specs/mei/mei-all-5.1.rng`
  (ECL-2.0): `mei_data.NOTATIONTYPE` (:7495, includes `neume`),
  `mei_att.notationType.attribute.notationtype` (:8402),
  `mei_att.intervalMelodic.attribute.intm` (:8612), `mei_nc` (:19350),
  `mei_neume` (:19398), `mei_signifLet` (:19449, free text),
  `mei_att.linking.attribute.corresp` (:21213, URI list + Schematron
  warning that each value should resolve to an `xml:id`),
  `mei_att.signifLet.vis.attribute.place` (:31029),
  `mei_staff` (:28007, accepts `staffDef` and covers non-diastematic signs),
  Schematron `mei-all-staff-checkStaff_n` (:28025–28031),
  `mei_att.staffDef.log.attribute.lines` (:22738, `positiveInteger`),
  `mei_att.staffDef.vis.attribute.lines.visible` (:31111–31117).
- **Gregorio NABC reference**
  `~/programmes/musique/specs/gabc/gregorio/doc/GregorioNabcRef.tex`
  (GPL-3.0-or-later, commit `b63f074`): basic glyph table (:207–240),
  glyph modifiers (:241–257), pitch descriptors (:258–269), subpunctis /
  prepunctis (:270–291), significant letters and positions (:293–372),
  Laon table (:402–437), Tironian notes (:481–495); grammar
  `src/gabc/gabc-score-determination.y:870–907` (the `|` cut),
  `doc/Gabc.tex:129` (`nabc-lines`), `:913–914` (`[cn:…]`).
- **Existing encoded manuscripts.** The official
  `music-encoding/sample-encodings` repository (commit
  `f3f1baba02e32279b25dad660b82c834bac034b4`, ECL-2.0) has **no** neume
  file in its `MEI_5.1` folder (checked against its recursive GitHub tree):
  its 167 encodings are CMN only, as already recorded in `mei.md` §9.2.
  The published conventions for adiastematic notation therefore come from
  the *Guidelines* chapter 6 examples above (Tübingen/Hildegard project,
  §6.2) and from the Neume Notation Interest Group's samples they are
  distilled from. They use unpitched `<nc>` with `@tilt`/`@intm` and
  `<signifLet>` for significative letters — the convention adopted here.
  No published encoding puts a St. Gall/Laon NABC line in a second
  `<staff>`; this two-staff layout is an application of the general MEI
  `@corresp` mechanism (RNG :21213) to juxtapose square and manuscript
  notation, validated against `mei-all-5.1.rng` (see `mei.md` §13).

### Encoding rule

Staff 1 keeps the square notation, with `notationtype="neume"` and pitched
`<nc @pname @oct>`. Staff 2 is a **parallel, unpitched** manuscript
transcription: `notationtype="neume"`, **no `@lines`**, declared by a
`<staffDef>` child of its `<staff>` (Guidelines §4.2.2, MEI All;
`mei_staff` :28007, Schematron :28025–28031) rather than by a `<scoreDef>`
`<staffDef>` that would violate Schematron; one `<neume>` per
NABC complex glyph descriptor, `<nc>` children without `@pname`/`@oct`
(their number is the glyph's note count), `@intm` for the melodic direction
between successive pitch descriptors, modifier elements where MEI has one,
and `<signifLet type="gabc-nabc">` carrying the exact NABC snippet text.
The two `<syllable>`s of a pair carry `xml:id` and `@corresp` to each other
(validated with `jing` and the embedded Schematron; `mei.md` §12.3.1, §13.3).

| NABC fact (Gregorio reference) | MEI 5.1 carrier | Structural / text | Citation |
| --- | --- | --- | --- |
| `nabc-lines: n` header (`:129`; grammar `:870–907`) | kept in `Gregorian::headers` → `meiHead` as before; no staff geometry follows from it | text | `Gabc.tex:129`; Guidelines ch. 3 |
| Second manuscript line | `<staff n="2"><staffDef n="2" notationtype="neume" label="…"/><layer n="1">` (no `lines`) | structural | RNG `mei_data.NOTATIONTYPE` :7504, `mei_staff` :28007 + Schematron :28025–28031; Guidelines §4.2.2 (`#cmnReDef`), §6.1, §6.4.10; `mei.md` §12.3.1 |
| Link square ↔ manuscript syllable | `<syllable xml:id="sNa" corresp="#sNb">` on both staves | structural | RNG `mei_att.linking.attribute.corresp` :21213 |
| Basic glyph `vi` virga | `<neume type="virga"><nc/></neume>` | structural | `GregorioNabcRef.tex:209`; Guidelines §6.1.1, §6.4.2 |
| `pu` punctum, `ta` tractulus, `gr` gravis, `st` stropha, `or` oriscus, `ni` nihil | `<neume type="punctum\|tractulus\|gravis\|stropha\|oriscus\|nihil"><nc/></neume>` | structural | `GregorioNabcRef.tex:210–212,216,233–234,240`; RNG `att.neumeType/@type` (NMTOKEN allowed) |
| `cl` clivis, `pe` pes, `bv` bivirga, `ds` distropha, `pq` pes quassus, `pt` pes stratus | `<neume type="clivis\|pes\|bivirga\|distropha\|pes-quassus\|pes-stratus">` with **two** `<nc/>` | structural | `GregorioNabcRef.tex:213–214,227,229,238–239`; Guidelines §6.1.1 |
| `po` porrectus, `to` torculus, `ci` climacus, `sc`/`sa` scandicus/salicus, `tg` trigonus, `ts` tristropha, `tv` trivirga, `pr`/`pi` pressus, `ql`/`qi` quilisma | `<neume type="…">` with **three** `<nc/>`, except `ql`/`qi` (one `<nc>` + `<quilisma waves="2|3"/>`) | structural | `GregorioNabcRef.tex:215,217–226,231–232,235–237`; Guidelines §6.4.7, Listing 281 |
| `pf` porrectus flexus, `sf` scandicus flexus, `tr` torculus resupinus | `<neume type="…">` with **four** `<nc/>` | structural | `GregorioNabcRef.tex:223–225` |
| Laon `un` uncinus, `oc` oriscus-clivis | `<neume type="uncinus\|oriscus-clivis">` with 1 / 2 `<nc/>` | structural | `GregorioNabcRef.tex:402–410` |
| Modifier `-` (episema) | `<episema form="h" place="above"/>` in the last `<nc>` | structural | `GregorioNabcRef.tex:245`; Guidelines §6.4.4 |
| Modifier `>` / `~` (augmentive / diminutive liquescence) | `<liquescent curve="c\|a"/>` in the last `<nc>` | structural | `GregorioNabcRef.tex:246–247`; Guidelines §6.4.5 |
| Modifier `S`, `G`, `M` and variant digit | **no MEI equivalent**: kept as text in the `<signifLet type="gabc-nabc">` carrier | text-only | `GregorioNabcRef.tex:242–257` |
| Pitch descriptor `h` + `a`…`n`,`p` (default `hf`) | `@intm="u\|d\|s"` on the `<nc>` following the previous pitch (direction only); exact letter kept as text | structural + text | `GregorioNabcRef.tex:258–269`; RNG `mei_att.intervalMelodic.attribute.intm` :8612; Guidelines §6.4.2 |
| Subpunctis / prepunctis `su\|pp [tuvwxy] n` (St. Gall), `su\|pp [nqzx] n` (Laon) | one extra `<nc intm="d\|u"/>` per repetition; kind and count kept as text | structural + text | `GregorioNabcRef.tex:270–291,437–448`; Guidelines §6.4.2 |
| Significant letter shorthands `ls…` (St. Gall), `lseq-`, `lt…` (Tironian) | `<signifLet place="…">c</signifLet>`; position digits 1,2,3,4,6,7,8,9 → `above-left`, `above`, `above-right`, `left`, `right`, `below-left`, `below`, `below-right`; **position 5 (Laon "inside") has no `data.EVENTREL` value** and is kept only in the element text | structural | `GregorioNabcRef.tex:293–372,449–495`; Guidelines §6.4.8, Listing 282; RNG `mei_att.signifLet.vis.attribute.place` :31029 |
| Horizontal spacing `` ` `` `/` `//` `` `` ``` `` | **no MEI equivalent**: kept as text in the `<signifLet type="gabc-nabc">` carrier | text-only | `GregorioNabcRef.tex:177–185,376–377` |
| Exact NABC snippet (all of the above, in source order) | `<signifLet type="gabc-nabc" n="snippet-index">` text | text-only | Guidelines §6.4.1/§6.4.9 (free-text `<signifLet>`); RNG `mei_signifLet` :19449 |
| Choral NABC sign `[cn:…]` | `Gregorian::Syllable::text` (unchanged), no neume carrier | text-only | `Gabc.tex:913–914` |

No row above claims a universal semantic equivalence: Guidelines §6.3 warns
that regional signs with a similar shape can carry different meanings. The
mapping is a *documented transcription convention* between GABC NABC and
MEI neumes, and the exact NABC text is always retained so the conversion is
reversible independently of that interpretation.

## Gaps in the model, ranked

1. **L1 gestural/perform time.** `ChordRest` has one `duration`+`ticks`
   (chord.rs:341-363); `@dur.ges`, `@tstamp.ges`, `@tstamp.real` and `@ppq`
   cannot be stored. This is the gap already identified in
   `model-modularisation.md` §7.2 and it is the only one that changes the
   *musical* reading, not the engraving.
2. **L6 opaque header and identity.** No field holds the raw `meiHead`; no
   field holds `xml:id`. Without them an MEI→MEI round trip rewrites the
   header and every id, so a diff of two exports is meaningless.
3. **L6 referencing/linking.** `@copyof`, `@sameas`, `@facs`, `<facsimile>`,
   `<timeline>`, `<expansion>` have no representation. They must be reported
   as losses, not dropped.
4. **More than four voices per staff** (`VOICES = 4`, ids.rs:31). MEI layers
   are unbounded; a fifth layer must be a reported loss.
5. **Colour** (L5). Cheap to add, but it is layout, not music.
6. **`lv`, harp pedal, `<annot>`, `<ornam>` combinations** (L2/L6): small
   dedicated symbols missing from the pivot.
7. **`@n` semantics.** `Measure::number` is a `usize` plus an offset
   (measure.rs:56-59); MEI labels may be `"1a"`, `"1-3"`, `"A"`. A faithful
   port must keep the label as text (L6) or accept the documented
   approximation.

No row above contradicts `model-modularisation.md` §7.2; the L1 `GesturalTime`
and L6 `ExternalRef`/`MeiExpansion` proposals are confirmed, and this table
adds the header blob, `xml:id`, colour and the >4-layer limit to them.

## Oracle and reference candidates

- **MuseScore 4.7.4 CLI is installed** (`mscore`, `/run/current-system/sw/bin`)
  and is the compliance target:
  - MEI import oracle: `mscore -o out.musicxml in.mei` (MusicXML is the
    already-trusted pivot direction) and `mscore -o out.mscx in.mei`.
  - MEI export oracle: `mscore -o out.mei in.mscx` for the existing
    `tests/corpus` scores.
  - Record the exact command and version in the measurement script, as
    `docs/agents/plan-fiabilite.md` requires.
- **MuseScore's own MEI test corpus** is local and paired:
  `ref-musescore/.474/src/importexport/mei/tests/data/*.mei` with the
  `*.mscx` references written by 4.7.4 (64 pairs: beam, tuplet, slur, tie,
  lyric, dynamic, harm, fig-bass, gliss, octave, pedal, ending, jump, …).
  Licence: **GPL-3.0-only** (`mei/internal/*.cpp` headers; the whole
  `ref-musescore/` tree is the same licence), so they may be used as an
  out-of-repository oracle exactly like the existing corpus was produced.
  Do **not** copy them into `tests/corpus` without the NOTICE entry and the
  licence header review.
- **Verovio** also reads MEI and exports MEI; useful as a second opinion for
  `@tstamp` rounding and beam types, but its output is not the compliance
  reference. Verovio is LGPL-3 (`specs/humdrum/verovio/COPYING.LESSER`) and
  is *not* local as an MEI tool (only `iohumdrum.cpp` was fetched).
- **External MEI samples** (music-encoding.org sample files, RISM, DIAMM):
  not local, licences vary and some are not redistributable. Use only after
  recording the licence in `specs/README.md`, and never commit the file.

## Corpus candidates

1. MuseScore's 64 paired `*.mei`/`*.mscx` files above — the first batch, no
   download, GPL-3, exactly the 4.7.4 behaviour.
2. The repository's own `tests/corpus/{v3,v4}/*.mscx` exported to `.mei` by
   `mscore` — gives MEI export coverage over the real corpus (the same scores
   already used for MusicXML export), with the `.mscx` as the round-trip
   reference.
3. Hand-written `mei-basic` files for the traps of
   [`format-ref/mei.md`](../format-ref/mei.md) §Traps: `oct=3`, `@tstamp=0`,
   `@type` beams, a 5-layer staff, a nested tuplet, an unknown `<dir>`.
4. Only then, a small licensed external sample (public-domain score from a
   library with an explicit CC0/PD statement), with its licence recorded.

## Suggested task breakdown

Numbers continue the existing t-sequence (`t43` is the last used).

- **t44.1 — crate skeleton and facade.** `scoretk-mei-{common,import,export}`
  with the licence headers, `mei-import`/`mei-export` facade features,
  detection of `mei` by extension and by the root element; wasm build.
- **t44.2 — reader core (L0/L1).** `scoreDef`/`staffDef`, measures, staves,
  layers, notes/chords/rests, `@dur`/`@dots`/`@pname`/`@oct`, implicit ticks,
  barlines and repeat flags. Test on `beam-01`, `measure-01`,
  `time-signature-01`.
- **t44.3 — reader control events (L2).** Ties, slurs, beams, tuplets,
  articulations, dynamics, directions, hairpins, pedal, octave, ornaments,
  fermata, breath. Use `startid`/`endid` first and `tstamp` as the fallback.
- **t44.4 — reader text and header (L3).** `meiHead` → `ScoreMetadata`,
  verses/syllables, harmony, figured bass, tempo, rehearsal marks. Store the
  raw head in an L6 extension (needs t44.7).
- **t44.5 — writer.** Model → `mei-basic` in MuseScore's element order
  (control events after staves), with `@xml:id` generated stably; compare
  *musically* to `mscore -o out.mei` by re-importing both into the pivot,
  never by XML diff.
- **t44.6 — measurement.** Paired corpus, two levels (musical core, then full
  parity) as in `docs/agents/plan-fiabilite.md`; at most two workers.
- **t44.7 — model additions** (only those the measurement proves necessary):
  `Extensions::Mei { head_xml, xml_id, refs }` in L6, `GesturalTime` in L1,
  colour in L5, and the `Loss` records of §6 of `model-modularisation.md`.
- **t44.8 — round trip and limits.** MEI→model→MEI on the paired corpus;
  document per-file causes in `docs/agents/mei.md`.

## Risks

- **`@tstamp` rounding.** MuseScore's continued-fraction approximation
  (`meiconverter.cpp:3828-3878`) can place a dynamic one tick away from the
  oracle. Prefer ids; document the difference as a converter artefact, not a
  musical error.
- **Element order.** MEI is order-sensitive for control events; a DOM
  writer that sorts children (or emits control events inside layers) changes
  the reading. Mirror `writeMeasure`/`writeLayer` order
  (`meiexporter.cpp:834-1035`).
- **Nested tuplets and >4 layers** are rejected by 4.7.4; accepting them
  would make scoretk "better" but not comparable, and the report would hide
  the divergence. Reject with a `Loss` like the reference.
- **The guidelines HTML is not licensed** in the fetched copy. Cite section
  numbers, do not copy prose; the schema (ECL-2.0) is the normative text to
  quote.
- **`mei-all` elements** (mensural, neumes, tablature) have no local
  guideline and no MuseScore support: a first port must skip them by name
  and report them, not guess.
