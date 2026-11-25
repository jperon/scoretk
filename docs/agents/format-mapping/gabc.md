# GABC ↔ scoretk model

There is no published GABC ↔ model specification. This table is derived from
Gregorio's own sources, the reference implementation of the format: the
syntax reference `doc/Gabc.tex`, the grammar `src/gabc/gabc-score-determination.y`
and lexer `.l`, the score structures `src/struct.h`, key/clef rules
`src/struct.c:1273-1288`, and the dump writer `src/dump/dump.c`. All are at
commit `b63f074` in `specs/gabc/gregorio/`, GPL-3.0-or-later
(`gregorio/COPYING.md`). Pivot types and lines come from
`crates/scoretk-core/src/model/`; layers **L0…L6** are those of
[model-modularisation.md](../model-modularisation.md) §5.1. The reader and
writer now exist (`scoretk-gabc-{common,import,export}`): the reader fills the
L6 payload, the writer **serialises its structured fields** (no raw source is
kept) and, for a score with no payload, projects concert pitch, lyrics and
bars into GABC. Status cells remain the format-level *contract*: "missing"
means the pivot field is not populated by this direction. See
[gabc.md](../gabc.md) §7-§10 for the implemented subset and measured limits.

| Format property (spec section) | Model type / field (file:line), layer | Reader | Writer | Notes |
| --- | --- | --- | --- | --- |
| Headers `name`, `author`, `language`, `date`, `transcriber` (`Gabc.tex:79-93,151-152`) | `ScoreMetadata` (`score.rs:51-91`), `Score::metadata` (`:125`), L0 | missing | missing | `name` is required (`:79-82`); Gregorio keeps the other headers as metadata lines (`src/dump/dump.c:128-162`), not as score events. |
| Liturgical headers `office-part`, `occasion`, `mode`, `mode-modifier`, `mode-differentia`, `annotation` (`Gabc.tex:94-127,141-142`) | no field; L6 `Gregorian::Header` | not representable | not representable | `ScoreMetadata` has no mode/office-part; mode headers are typeset above the initial (`:156-188`), so `Credit` (`text.rs:586-610`) could hold only rendered text. |
| Copyright headers `gabc-copyright`, `score-copyright` (`Gabc.tex:83-91`) | `ScoreMetadata::copyright` (`score.rs:71`), L0 | missing | missing | Two distinct notices collapse to one pivot string; a faithful port needs two L6 fields. Gregorio stores both (`src/dump/dump.c:131-134`). |
| `staff-lines` 2–5 (`Gabc.tex:128,410-443`) | `Staff::lines` (`part.rs:361`), L0 | missing | missing | Gregorio uses the header to bound the letter range (`Gabc.tex:415-443`) and records it in the dump (`src/dump/dump.c:149`). |
| Clef `c`/`f`, optional `b` (flat), line `1`…`N`, two clefs with `@` (`Gabc.tex:847-870`) | `Clef`, `ClefType`, `ClefSign` (`segment.rs:214-406`), `Staff::default_clef` (`part.rs:391`), L1 | missing | missing | Gregorio resolves a clef to a diatonic key `2*line-1` (C) or `2*line-4` (F) (`src/struct.c:1273-1288`) and adjusts pitches at clef changes (`src/gabc/gabc-score-determination.c:89-100`). Two simultaneous clefs have no single `Clef` equivalent. |
| Pitch letters `a`–`n`,`p` (staff position, not pitch name) (`Gabc.tex:408-443`) | `Note::pitch`, `tpc`, `tpc_transposed` (`note.rs:268-273`), L0 | missing | missing | The pivot stores concert MIDI pitch + tonal class; GABC letters are typographical positions (`:410-413`) converted to staff heights `LOWEST_PITCH = 3` with `o → p` (`src/struct.h:809-813`, `src/dump/dump.c:78-88`). |
| Flats/sharps/naturals and soft forms `x # y X ## Y` (`Gabc.tex:481-489,512-521`) | `Accidental`, `AccidentalType` (`note.rs:201-245`), L0 | missing | partial | Alterations are one-note neumes in Gregorio (`:512-521`); "soft" forms depend on the `\gresetalterationeffect` convention (`:518-521`) and have no pivot counterpart. |
| One-note neume shapes `o w v V s ~ < > = r R` (`Gabc.tex:452-490`) | `Note::head`, `head_group` (`note.rs:298-300`), L6 `Gregorian::NeumeShape` | not representable | not representable | `NoteHeadGroup` (`note.rs:138-199`) covers modern heads, not square notation; Gregorio stores these as glyph shapes (`src/dump/dump.c:358-366`). |
| Complex neumes, fusion `@`, `@[ … ]`, `-`, `q`, `ss`, `sss`, `vv` (`Gabc.tex:556-666`) | `Chord::notes` (`chord.rs:222`), L6 | not representable | not representable | A GABC neume is a group of pitches; Gregorio builds glyphs from the letter sequence (`:556-579`) and dumps one `GRE_GLYPH` per shape (`src/dump/dump.c:358-366`). Fusion/shape intent is lost. |
| Liquescents `~ < >`, quilisma `w`, oriscus `o`, virga/stropha (`Gabc.tex:469-478,543-579`) | no direct field; L6 | not representable | not representable | Gregorio records `liquescentia` on glyphs and notes (`src/dump/dump.c:366,407`) and quilisma as a glyph (`G_PUNCTUM`/`G_QUILISMA` at `:358-366`); no pivot type. |
| Punctum mora `.`/`..`, ictus `'`, horizontal episema `_` with modifiers (`Gabc.tex:702-797`) | `Duration::dots` (`duration.rs:163`), `Spanner`/`SpannerType` (`spanner.rs:268-326`), L2/L6 | missing | missing | Gregorio dumps punctum mora and vertical episema as note `signs` (`src/dump/dump.c:435-455`); horizontal episema is a layout decision (`:458-486`). A pivot `<dot/>` cannot represent the others. |
| Bar kinds `` ` `` `,` `;` `:` `:?` `::` `;1`…`;8` (`Gabc.tex:801-821`) | `BarLineType` (`segment.rs:413-441`), `Measure::start_bar_line`/`end_bar_line` (`measure.rs:80-82`), L1 | missing | missing | Gregorio emits one `GRE_BAR` with a `B_*` name (`src/dump/dump.c:292-296,805-821`); the pivot has a much smaller vocabulary. |
| No metre, no fixed durations (`Gabc.tex:742-845`) | `Measure::len`, `nominal_len` (`measure.rs:63-65`), `Duration` (`duration.rs:37-164`), `ChordRest::ticks` (`chord.rs:348`), L1 | not representable | not representable | Gregorio stores staff positions, signs and bars, not durations (`src/struct.h:809-813`, `src/dump/dump.c:78-88`); any pivot `Measure` needs an invented metre — a *derived projection*, not source data. |
| Syllable text, word spaces, elision `<e>`, Euouae `<eu>`, centering `{…}` (`Gabc.tex:228-343`) | `Lyrics`, `LyricsSyllabic` (`lyrics.rs:51-63`), `RichText` (`text.rs:115`), L3 | missing | missing | Gregorio keeps characters and style boundaries separately (`src/dump/dump.c:41-66,223-242`); `LyricsSyllabic` cannot carry elision or centring. |
| Translation text `[…]`, `[/]` (`Gabc.tex:337-343`) | no field; L3 extension | not representable | not representable | Gregorio dumps `Translation type` and `Translation` separately (`src/dump/dump.c:230-238`). |
| Special characters `<sp>R/</sp>` etc. (`Gabc.tex:345-378`) | `RichText` (`text.rs:115`), L3 | missing | missing | The dump prints them as characters with `ST_STYLE` boundaries (`src/dump/dump.c:41-66`); the pivot has no SMuFL glyph table. |
| Above-lines text `[alt:…]`, choral signs `[cs:…]`, braces, slurs (`Gabc.tex:908-931,1000-1019,1088-1091`) | `Annotation` (`annotation.rs:80-115`), `Spanner` (`spanner.rs:295-326`), L2/L3/L5 | missing | missing | Gregorio dumps abovelinestext, choral signs and TeX strings (`src/dump/dump.c:242,287,384-431`); the grammar parses them as attributes, not notes (`grammar:741-769`). |
| Line breaks `z`/`Z`, custos `z0`, `g+` (`Gabc.tex:872-906`) | `Measure::line_break` (`measure.rs:126`), L5 | missing | missing | Gregorio marks `ragged`/`forces custos` on elements (`src/dump/dump.c:324-329`) and dumps custos with their pitch (`:251-259`); the pivot has no custos. |
| Verbatim TeX `[nv]/[gv]/[ev]`, macros `def-m0..9` (`Gabc.tex:205-226,1093-1118`) | no field; L6 | not representable | not representable | Gregorio stores them as TeX strings in the score structures (`src/dump/dump.c:277,346,423`); not representable in the pivot. |
| Transposition to a comfortable tessiture | `Part::instrument` transpose (`part.rs:135-139`), `Score::playback_transposition` (`score.rs:144`), L4 | missing | missing | Gregorio does not transpose: it keeps staff positions and a clef key (`src/struct.c:1273-1288`). Any transposition is a rendering choice of the target format. |
| Tempo | `Tempo` (`annotation.rs:322-364`), L4 | missing | missing | GABC has no tempo header (`Gabc.tex:67-154`); adding one is a target-format choice, not source data. |

## Model gaps

Ranked by musical importance (what the pivot `Score` cannot represent today):

1. **Neume/syllable stream**: no `Syllable`, `Neume`, `NeumeShape` or staff
   position type (`model/mod.rs:99-145`). This is the primary GABC data and
   belongs in L6, with a derived L0 projection — see
   `model-modularisation.md` §7.1.
2. **No metric spine**: `Measure::len`/`nominal_len` require a metre
   (`measure.rs:63-65`) that GABC does not have (`Gabc.tex:801-845`).
3. **Clef-dependent pitch**: `Note::pitch` is concert MIDI pitch
   (`note.rs:268`), while GABC letters are staff positions resolved by a
   clef key (`src/struct.c:1273-1288`); the mapping must be applied at read
   time.
4. **Square-notation heads and liquescents**: `NoteHeadGroup` (`note.rs:138`)
   has no equivalent for `S_*` glyphs and `liquescentia`
   (`src/dump/dump.c:358-407`).
5. **Rhythmic signs**: `Duration::dots` (`duration.rs:163`) covers only the
   punctum mora; ictus and horizontal episema have no type.
6. **Rich liturgical metadata and typography**: office-part, occasion, mode,
   the two copyright notices, translation text, elision and centring
   (`Gabc.tex:83-153,228-343`; `src/dump/dump.c:128-162,223-242`).
7. **GABC → modern notation is lossy by construction**: durations, metre and
   bar weight are choices of the target format, not facts in GABC
   (`Gabc.tex:742-845`). A round trip must be measured against the L6
   payload, not against a derived MusicXML/MIDI projection.
