# MuseScore `.mscx` / `.mscz` ↔ scoretk model

There is **no published specification** of this format. The reference is
`docs/agents/mscx-format.md` (section numbers below), the upstream C++ snapshot
in `/home/jacques/programmes/musique/musescore/ref-musescore/` at commit
`2b9e82df847111d673c2051a93deeaaeea3f5a29`, and the 4.7.4 reference exports in
`tests/corpus/`. See `docs/agents/upstream.md` and `docs/agents/mscx-write.md`.
Reader: `crates/scoretk-musescore-import` (`read/read302` for 3.x and
`read/read4xx` for 4.x/5.x). Writer: `crates/scoretk-musescore-export`
(always MuseScore 4.70). Statuses are based on the code in this tree.

| Format property (spec section) | Model type / field (file:line) | Reader | Writer | Notes |
| --- | --- | --- | --- | --- |
| `<museScore version="…">`, `<programVersion>` (§1, §2) | `Score::metadata.msc_version` (`crates/scoretk-core/src/model/score.rs:86`), `ScoreMetadata` (`:50-87`) | supported (`import/src/read/read302/mod.rs`, `read4xx/dom.rs`) | partial (`export/src/write/writer.rs`) | Any input version is read; output is always `version="4.70"` with `<programVersion>4.7.4</programVersion>`. |
| `<Division>` (§2) | `Score::division` (`score.rs:122`) | supported | supported (`export/src/write/staffwrite.rs`) | Default 480 divisions per quarter note. |
| `<Style>` (§2) | `StyleSettings` (`.../model/style.rs:48-101`) | supported (`import/src/read/read4xx/style.rs`) | supported (`export/src/write/staffwrite.rs:39`) | Page size, margins, lyric placement, font sizes, hairpin height and empty-stave flags; other style keys are ignored. |
| `<metaTag name="…">` (§2) | `ScoreMetadata` (`score.rs:50-87`) | supported | supported (`export/src/write/staffwrite.rs:107`) | All fields are written, including empty ones (`platform` differs from an absent tag). |
| `<Part>`, `<Staff>`, `<Instrument>` (§4) | `Part` (`.../model/part.rs:200-221`), `Staff` (`:308-333`), `Instrument` (`:57-88`) | supported (`import/src/read/read302/part.rs`) | supported (`export/src/write/staffwrite.rs:139`) | Names, MIDI program/channel, volume, pan and MIDI articulations are kept. |
| `<Staff id>` lines, `<bracket>`, `<barLineSpan>`, default clef (§4) | `Staff::lines`, `small`, `time_stretch`, `bar_line_span`, `brackets`, `default_clef` (`part.rs:314-332`), `StaffBracket` (`:295-302`) | supported | supported (`export/src/write/staffwrite.rs:183,196`) | Bracket kind/span/visibility round-trip. |
| `< transpose>`, `<InstrumentChange>` (§4, §13) | `Transpose` (`part.rs:120-135`), `Part::instrument_changes` (`:218`), `InstrumentChange` (`annotation.rs:400-421`) | supported (`read4xx/mod.rs`) | supported (`export/src/write/connectorinfowriter.rs:68`) | Instrument changes are stored as `(tick, Instrument)` pairs; concert/transposed pitch are kept separately. |
| `<Measure>` `len`, `<startRepeat>`, `<endRepeat>`, `<multiMeasureRest>`, `<LayoutBreak>`, `<Marker>`, `<Jump>` (§5, §14) | `Measure` (`.../model/measure.rs:52-101`), `RepeatMark` (`repeat.rs:252-257`) | supported (`read302/measure.rs`, `read4xx/mod.rs`) | supported (`export/src/write/measurewrite.rs:46,137`) | `MeasureRepeatMark` (§5) is copied from the source measure; a line break is modelled as `Measure::line_break`. |
| `<voice>` positions, `<location><fractions>`, `<tick>` (§6, §9) | `Segment` (`.../model/segment.rs:708-805`), `Track`/`VOICES` (`.../model/ids.rs:123`), `Measure::add_segment` (`measure.rs:153`) | supported (`read4xx/dom.rs`) | supported (`export/src/write/twrite.rs:296`) | Exact fractions avoid rounding on odd tuplets; `__location` is relative, `<tick>` absolute. |
| `<Chord>`, `<Rest>`, `<Note>` (`pitch`, `tpc`, `<velocity>`, `<veloType>`, note head) (§7) | `Chord` (`.../model/chord.rs:216-254`), `Rest` (`:297-315`), `Note` (`.../model/note.rs:263-335`) | supported (`read302/chord.rs`) | supported (`export/src/write/twrite.rs:66,192`) | `Note::velocity` (imported MIDI dynamics) and `Note::user_velocity` are distinct fields. |
| `<durationType>`, `<dots>`, `<duration>` (§7) | `Duration`, `DurationType` (`.../model/duration.rs:37-164`), `ChordRest::ticks` (`chord.rs:332`) | supported | supported (`export/src/write/twrite.rs:152`) | Dots are read before `<durationType>`; real duration may differ from the written note value. |
| `<Beam>`, `<StemDirection>` (§7) | `ChordRest::beam_mode`, `beam_direction` (`chord.rs:337-345`) | supported | supported (`export/src/write/twrite.rs:66`) | Beam objects are attached to the first chord of a group. |
| `<Tuplet>`, `<endTuplet>` (§8) | `Tuplet` (`.../model/tuplet.rs:53-81`), `ChordRest::tuplet` (`chord.rs:334`) | supported | supported | `<bracket>`, `<actualNotes>`/`<normalNotes>` and nesting via `Tuplet::parent` are preserved. |
| `< Tie>` (3.x two halves, 5.x EID links) (§11) | `Note::tie_forward`, `tie_back`, `tie_up`, visibility (`note.rs:271-291`) | supported (`read302/chord.rs`, `read4xx/dom.rs`) | supported (`export/src/write/twrite.rs:268`) | Older files store two halves with `<location>`; newer files use EID links. |
| `<Slur>`, `<HairPin>`, `<Ottava>`, `<Pedal>`, `<Volta>`, `<TextLine>`, `<Trill>`, `<Glissando>` (§12) | `Spanner`, `SpannerType` (`.../model/spanner.rs:268-322`), `SpannerNotation` (`notation.rs:397-404`) | supported (`read302/spanner.rs`, `read4xx/repeats.rs`) | supported (`export/src/write/connectorinfowriter.rs:42`) | A spanner fully inside a repeated bar is recreated by the reader when the source bar is expanded. |
| `<KeySig>`, `<TimeSig>`, `<Clef>` (§10) | `KeySig` (`.../model/segment.rs:118-129`), `TimeSig` (`:166-179`), `Clef`, `ClefType` (`:250-406`) | supported (`read302/measure.rs`) | supported (`export/src/write/twrite.rs:429,447,474`) | Concert/transposed keys, courtesy flags, time-sig subtype and stretch are kept. |
| `<BarLine>` (§10) | `BarLine`, `BarLineType` (`.../model/segment.rs:413-480`), `Measure::start_bar_line`/`end_bar_line` (`measure.rs:63-65`) | supported | supported (`export/src/write/twrite.rs:487`) | `EndStartRepeat` is written as `<endRepeat>` + `<startRepeat>` and cannot be distinguished again on import. |
| `<Lyrics>`, appoggiatures (§13) | `Lyrics` (`.../model/lyrics.rs:51-63`), `GraceChord` (`.../model/chord.rs:264-270`) | supported | supported (`export/src/write/twrite.rs:116,410`) | Verse index, syllabification, melisma and placement round-trip. |
| `<Dynamic>`, `<StaffText>`, `<SystemText>`, `<Tempo>`, `<RehearsalMark>`, `<Harmony>`, `<FiguredBass>`, `<TremoloBar>` (§13, §18) | `Annotation` and its variants (`.../model/annotation.rs:80-115,197-487`) | supported (`read302/annotations.rs`, `nuances.rs`, `read4xx/figures.rs`) | supported (`export/src/write/twrite.rs:539-718`) | MuseScore rich-text markup is reconstructed with `<sym>` and entities; `Accidental.user`/parentheses round-trip via `<role>`/`<parens>`. |
| `<VBox>/<Text>` (credits), `<TBox>` (§18) | `Credit` (`.../model/text.rs:586-610`), `TboxText` (`:417-426`), `Score::credits`/`tbox_texts` (`score.rs:130-132`) | supported (`read302/credits.rs`) | supported (`export/src/write/staffwrite.rs:262,300`) | VBox height, style, alignment and font size are kept; precise `default-x/y` are not modelled. |
| `<Excerpt>` / linked staves (§15) | — | partial (`import/src/read/read302/spanner.rs:334-341`) | missing | Linked spanners are reconciled on input, but linked staves are not duplicated in the pivot and the writer creates no excerpts (only the `instrumentExcerptFontSize` style is emitted). |
| `<FretDiagram>`, `<Image>`, `<Symbol>` as independent objects (§5, §7) | `Harmony::text` only (`annotation.rs:428-449`) | partial (some symbols are translated to `Articulation`/`Annotation::Other`) | missing | No diagram/image object exists in the model; no independent symbol objects are written. |
| Per-element `<offset>`, `<autoplace>`, `<align>`, `<size>` layout (§17, §18) | — (`Credit::font_size`, `TboxText::font_size` and `vbox_height` are the only partial exceptions) | missing | missing | Library-internal layout offsets are not stored; only the musical positions are exact. |
| `audiosettings.json`, `viewsettings.json`, thumbnail, embedded media (§2) | — | partial (`import/src/container.rs` reads the container) | partial (`export/src/container.rs` writes `{}` for both JSON files) | The writer produces no thumbnail and no media: there is no renderer. |

## Model gaps

Ranked by musical importance:

1. **Excerpts and linked staves** (`docs/agents/mscx-format.md` §15): the pivot
   has no notion of a linked duplicate, so `<linked-ties>`-style behaviour and
   excerpt parts cannot be represented.
2. **Independent graphical objects** (`<FretDiagram>`, `<Image>`, free
   `<Symbol>`): only `Harmony` text and a few articulation names survive.
3. **Fine engraving data** (`offset`, `autoplace`, `align`, staff distances,
   beam geometry, computed rests): the model keeps musical ticks but not
   graphic positions; this is the source of the two known 4.7.4 reference
   differences (`v3/moonlight`, `v3/voices`).
4. **Program-version fidelity**: every file is rewritten as 4.70; 3.x-specific
   encodings are converted by the 3.x reader rather than preserved by the
   writer, and there is no 3.x writer.
5. **Media/thumbnail and audio/view settings**: the container is reproduced
   with empty JSON and no rendered assets, so a real MuseScore session recreates
   them on save.
6. **A single `BarLineType` cannot express `EndStartRepeat` as one object**:
   it is decomposed into an end-repeat and a start-repeat on two bars.
