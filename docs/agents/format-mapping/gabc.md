# GABC ↔ scoretk model

There is no published GABC ↔ model specification. This table is derived from
two local references: Gregorio's syntax (`specs/gabc/gregorio/doc/Gabc.tex`,
commit `b63f074`, cited as `Gabc.tex:<line>`) and **gabctk** tag `v240107`
(`40b58b62`, `specs/gabc/gabctk/gabctk.py`, cited as `gabctk.py:<line>`), which
is the only available implementation mapping GABC to modern notation. The
pivot types and their line numbers come from `crates/scoretk-core/src/model/`;
layer names **L0…L6** are those of
[model-modularisation.md](../model-modularisation.md) §5.1. Licence: Gregorio
GPL-3.0-or-later, gabctk GPL-2.0-or-later (`specs/README.md` § GABC).
Reader and writer: **none yet** — phase 0, no code. All status cells are the
*contract to implement*, not an existing mapping; they use the legend of the
other format-mapping documents (supported/partial/missing/not representable).

| Format property (spec section) | Model type / field (file:line), layer | Reader | Writer | Notes |
| --- | --- | --- | --- | --- |
| Headers `name`, `author`, `language`, `date`, `transcriber` (`Gabc.tex:79-93,151-152`) | `ScoreMetadata` (`score.rs:51-91`), `Score::metadata` (`:125`), L0 | missing | missing | gabctk reads only `name` (`gabctk.py:391-393`) and `language` is never used; `date`/`transcriber` have no whole-field equivalent beyond `creation_date`/`arranger`. |
| Liturgical headers `office-part`, `occasion`, `mode`, `mode-modifier`, `mode-differentia`, `annotation` (`Gabc.tex:94-127,141-142`) | no field; L6 `Gregorian::Header` | not representable | not representable | `ScoreMetadata` has no mode/office-part; Gregorio typesets mode as annotation (`:156-188`), which the pivot's `Credit` (`text.rs:586-610`) could hold only as text. |
| Copyright headers `gabc-copyright`, `score-copyright` (`Gabc.tex:83-91`) | `ScoreMetadata::copyright` (`score.rs:71`), L0 | missing | missing | Two distinct GABC notices collapse to one pivot string; a faithful port needs two L6 fields. |
| `staff-lines` 2–5 (`Gabc.tex:128,410-443`) | `Staff::lines` (`part.rs:361`), L0 | missing | missing | gabctk ignores the header entirely: its pitch table assumes 4 lines (`gabctk.py:1214-1261`). |
| Clef `c`/`f` + optional `b` + line, `c1@c4` (`Gabc.tex:847-870`) | `Clef`, `ClefType`, `ClefSign` (`segment.rs:214-406`), `Staff::default_clef` (`part.rs:391`), L1 | missing | missing | gabctk recognises `[cf]b?[1234]` (`gabctk.py:736`), stores it on the word (`:939-944`) and **derives MIDI pitch from it** with a fixed `decalage` table (`:1220-1261`). Two simultaneous clefs (`c1@c4`) are not modelled by gabctk either. |
| Pitch letters `a`–`n`,`p` (staff position, not pitch name) (`Gabc.tex:408-443`) | `Note::pitch`, `tpc`, `tpc_transposed` (`note.rs:268-273`), L0 | missing | missing | The pivot stores concert MIDI pitch + tonal class; GABC letters are typographical positions requiring a clef to resolve (`gabctk.py:1214-1264`). Lowercase = quadratum, uppercase = inclinatum (`Gabc.tex:454-468`) — no direct `NoteHeadGroup` (`note.rs:138`). |
| Flats/sharps/naturals and soft forms `x # y X ## Y` (`Gabc.tex:481-489,512-521`) | `Accidental`, `AccidentalType` (`note.rs:201-245`), L0 | missing | partial | gabctk propagates alterations from syllable to syllable (`gabctk.py:579-594,850-874`) and warns for a flat on a note other than `si` (`:1270-1271`); "soft" accidentals have no pivot counterpart. |
| One-note neume shapes `o w v V s ~ < > = r R` (`Gabc.tex:460-490`) | `Note::head`, `head_group` (`note.rs:298-300`), L6 `Gregorian::NeumeShape` | not representable | not representable | `NoteHeadGroup` (`note.rs:138-199`) covers modern heads, not square notation. gabctk keeps the original character in `Signe.gabc` (`gabctk.py:807`) and only renders a few shapes in LilyPond/ABC (`:1068-1099`). |
| Complex neumes, fusion `@`, `@[ … ]`, `-`, `q`, `ss`, `sss`, `vv` (`Gabc.tex:556-666`) | `Chord::notes` (`chord.rs:222`), L6 | not representable | not representable | A GABC neume is *one group of pitches*, which could project to a pivot `Chord` (gabctk iterates `neume` signs at `gabctk.py:1371-1373`), but the shape/fusion intent is lost. |
| Liquescents `~ < >`, quilisma `w`, oriscus `o`, virga/stropha (`Gabc.tex:469-478,543-579`) | `SigneRythmique` → `Note::_nuances` (`gabctk.py:992-1005,1022`), L6 | not representable | not representable | gabctk's duration model: quilisma lengthens the **preceding** note to `DUREE_AVANT_QUILISMA = 2` (`:1051-1052`, `:35`); liquescence is rendered as small size in LilyPond (`:1080-1081`) and is a no-op in ABC (`:1097-1098`). No pivot field for either. |
| Punctum mora `.`/`..`, ictus `'`, horizontal episema `_` with modifiers (`Gabc.tex:702-797`) | `Duration::dots` (`duration.rs:163`), `Spanner`/`SpannerType` (`spanner.rs:268-326`), L2/L6 | missing | missing | gabctk treats all three as duration modifiers: `DUREE_POINT = 2.3`, `DUREE_EPISEME = 1.7` (`gabctk.py:34-36`, `:1042-1045`); in ABC an episema becomes `!tenuto!` and an ictus `!wedge!` (`:1091-1094`). A pivot `<dot/>` cannot represent the other two. |
| Bar kinds `` ` `` `,` `;` `:` `:?` `::` `;1`…`;8` (`Gabc.tex:801-821`) | `BarLineType` (`segment.rs:413-441`), `Measure::start_bar_line`/`end_bar_line` (`measure.rs:80-82`), L1 | missing | missing | gabctk maps only four forms to ABC/LilyPond and uses them to **pad the previous note's duration** (`gabctk.py:877-936`, weights `;`:0.5, `:`:1, `::`:1.2 at `:904-910`), and splits lines at `::`/`:`/`;` (`:439-445`). |
| No metre, no fixed durations (`Gabc.tex:742-845`; gabctk `M: none`, `gabctk.py:38-48`) | `Measure::len`, `nominal_len` (`measure.rs:63-65`), `Duration` (`duration.rs:37-164`), `ChordRest::ticks` (`chord.rs:348`), L1 | not representable | not representable | gabctk assigns a default duration `1` (`gabctk.py:1017-1019`), equalises neume durations (`:1030-1036`) and truncates to `int(duree)` in MIDI (`:1443-1454`). Any pivot `Measure` needs an invented metre — a *derived projection*, not source data. |
| Syllable text, word spaces, elision `<e>`, Euouae `<eu>`, centering `{…}` (`Gabc.tex:228-343`) | `Lyrics`, `LyricsSyllabic` (`lyrics.rs:51-63`), `RichText` (`text.rs:115`), L3 | missing | missing | gabctk strips spaces, `<i>`/`<b>`, braces and maps `<sp>` codes to Unicode (`gabctk.py:617-652`), then emits one `w:` line and joins syllables with `-`/`_` (`:1361-1388`). `LyricsSyllabic` can carry end/melisma but not elision or centring. |
| Translation text `[…]`, `[/]` (`Gabc.tex:337-343`) | no field; L3 extension | not representable | not representable | Not used by gabctk. |
| Special characters `<sp>R/</sp>` etc. (`Gabc.tex:345-378`) | `RichText` (`text.rs:115`), L3 | missing | missing | Mapped to `℟`/`℣`/ligatures by gabctk (`gabctk.py:639-652,681-694`); the pivot has no SMuFL glyph table. |
| Above-lines text `[alt:…]`, choral signs `[cs:…]`, braces, slurs (`Gabc.tex:908-931,1000-1019,1088-1091`) | `Annotation` (`annotation.rs:80-115`), `Spanner` (`spanner.rs:295-326`), L2/L3/L5 | missing | missing | gabctk strips all `[^\[\]]*` commands before parsing (`gabctk.py:378-379`), so they are invisible to the oracle reference too. |
| Line breaks `z`/`Z`, custos `z0`, `g+` (`Gabc.tex:872-906`) | `Measure::line_break` (`measure.rs:126`), L5 | missing | missing | gabctk models `z` as `Fin` (`gabctk.py:746,946-954`) and never emits a MusicXML system break; custos is not represented. |
| Verbatim TeX `[nv]/[gv]/[ev]`, macros `def-m0..9` (`Gabc.tex:205-226,1093-1118`) | no field; L6 | not representable | not representable | gabctk removes bracketed commands (`gabctk.py:378-379`); no non-TeX pivot representation exists. |
| Transposition to a comfortable tessiture | `Part::instrument` transpose (`part.rs:135-139`), `Score::playback_transposition` (`score.rs:144`), L4 | missing | missing | gabctk computes `66 − mean(tessiture)` (`gabctk.py:486-492`) and writes `K: transpose=` (`:38-48`) or adds semitones to MIDI pitches (`:1431-1443`); this is a *rendering choice*, not source data. |
| Tempo (external `-t` option) | `Tempo` (`annotation.rs:322-364`), L4 | missing | missing | Default `TEMPO = 165` (`gabctk.py:33`); MIDI divides by 2 and writes a tempo per note (`:1419-1425,1445-1447`), so the value is not a score property. |
| Reference MusicXML via ABC→`abc2xml` | `Score` whole document, L0–L5 | n/a | n/a | gabctk's `-x` is `Abc.ecrire(..., xml=True)` (`gabctk.py:1390-1410`): MusicXML is **derived from ABC**, not exported natively. The oracle must not treat its `<divisions>`/voice choices as GABC facts. |

## Model gaps

Ranked by musical importance (what the pivot `Score` cannot represent today):

1. **Neume/syllable stream**: no `Syllable`, `Neume`, `NeumeShape` or staff
   position type (`model/mod.rs:99-145`). This is the primary GABC data and
   belongs in L6, with a derived L0 projection — see
   `model-modularisation.md` §7.1.
2. **No metric spine**: `Measure::len`/`nominal_len` require a metre
   (`measure.rs:63-65`) that GABC does not have; any measure is invented.
3. **Clef-dependent pitch**: `Note::pitch` is concert MIDI pitch
   (`note.rs:268`), while GABC letters are typographical positions whose
   concert value depends on `Staff::default_clef` (`part.rs:391`); the
   mapping must be applied at read time (`gabctk.py:1220-1264`).
4. **Square-notation heads and liquescents**: `NoteHeadGroup` (`note.rs:138`)
   and `Note::_nuances`-equivalents do not exist in the pivot; quilisma,
   oriscus, liquescents and episemata are lost.
5. **Rhythmic signs**: `Duration::dots` (`duration.rs:163`) covers only the
   punctum mora; ictus and horizontal episema have no type.
6. **Rich liturgical metadata and typography**: office-part, occasion, mode,
   annotation, `gabc-copyright` vs `score-copyright`, translation text,
   elision and centring (`Gabc.tex:83-153,228-343`).
7. **GABC → modern notation is lossy by construction**: durations, metre,
   transposition and bar weight are *rules* in gabctk (`gabctk.py:34-36`,
   `:486-492`, `:902-914`, `:1017-1066`), not facts in the GABC file. A
   round trip must be measured against the L6 payload, not against the
   derived MusicXML/MIDI.
