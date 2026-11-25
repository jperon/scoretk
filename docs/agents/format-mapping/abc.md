# ABC 2.1 ↔ scoretk model

Reference documents (outside the repository): `specs/README.md`, section ABC;
`abc/abc-standard-v2.1.html` and its DokuWiki source (standard 2.1, Dec 2011),
plus `abc-standard-v2.2-draft.*` (published **draft** 2.2, not final).
Section numbers below refer to the 2.1 text. Reader: `crates/scoretk-abc-import`;
writer: `crates/scoretk-abc-export`. Statuses reflect the code in this tree.

| Format property (spec section) | Model type / field (file:line) | Reader | Writer | Notes |
| --- | --- | --- | --- | --- |
| `X:`, `T:`, `C:`, `M:`, `L:`, `K:` headers (§2.1, §3.1) | `ScoreMetadata` (`crates/scoretk-core/src/model/score.rs:50-87`) | supported (`reader/mod.rs:624`) | supported (`writer.rs:161`) | The reader selects the first tune by default; `read_abc_tune` selects another `X:`. |
| `K:` key and mode, `K:none`, `K:HP` (§3.1, §4.6) | `KeySig`, `Key`, `KeyMode` (`.../model/segment.rs:43-129`) | supported (`reader/mod.rs:255-272`) | supported (`writer.rs:176`) | Modal keys are reduced to their signature; the tonic is not stored (`docs/agents/abc.md` §4). |
| `M:C`, `M:C\|`, numeric metre (§3.1) | `TimeSig`, `TimeSigType` (`.../model/segment.rs:148-207`) | supported | supported (`writer.rs:171`) | `M:none` is treated as 4/4; local bar-length changes are written with `Timing.measureLength` equivalents only in `ChordRest::ticks`. |
| Pitch, accidentals, note lengths (§4.1-§4.3) | `Note::pitch`, `tpc`, `accidental` (`.../model/note.rs:263-293`); `Duration` (`.../model/duration.rs:159-164`) | supported (`reader/music.rs:411,569`) | supported (`voice.rs`) | Written pitch is used; accidentals are emitted only when required by the key/measure state. |
| Broken rhythm `<` / `>` (§4.4) | `ChordRest::duration`, `ChordRest::ticks` (`.../model/chord.rs:323-348`) | supported (`reader/music.rs:588`) | supported | Up to two levels: `<<`, `>>`. |
| Rests, including full-bar rests (§4.5) | `Rest`, `ChordRestKind::Rest` (`.../model/chord.rs:297-357`) | supported | supported | Multi-bar rests map to `Measure::multi_measure_rest` where possible. |
| `clef=`, `transpose=` in `K:`/`V:` (§4.6) | `ClefType` (`.../model/segment.rs:250-302`), `Transpose` (`.../model/part.rs:120-135`) | supported (`reader/mod.rs:236,272`, `build.rs:383`) | partial (`writer.rs:183`) | `transpose=` is a non-standard extension; the writer emits written pitch and a clef but no transpose field, so transposition is lost on round-trip. |
| Beams (§4.7) | `ChordRest::beam_mode`, `beam_direction` (`.../model/chord.rs:337-345`) | missing | missing | ABC beams are implicit; neither half sets `BeamMode`. |
| Bar lines, `\|:`, `:\|`, `::`, first/second and variant endings (§4.8-§4.10) | `Measure::start_bar_line`, `end_bar_line`, `repeat_count` (`.../model/measure.rs:63-70`), `SpannerType::Volta`, `VoltaInfo` (`.../model/repeat.rs:265-270`) | supported (`reader/music.rs`) | supported (`voice.rs:955`) | `repeat_count > 2` has no ABC syntax; multi-bar voltas are approximated by the first bar. |
| Ties `-`, slurs `( )` (§4.11) | `Note::tie_forward`/`tie_back` (`.../model/note.rs:271-276`), `SpannerType::Slur` (`.../model/spanner.rs:270`) | supported (`reader/music.rs:707`) | supported | A tie on a grace note has no standard ABC equivalent. |
| Grace notes `{…}` (§4.12) | `GraceType`, `GraceChord` (`.../model/chord.rs:87-104,264-270`) | supported | supported | One grace group per chord on output; consecutive groups are merged. |
| Duplets/triplets `(p:q:r` (§4.13) | `Tuplet` (`.../model/tuplet.rs:53-81`), `ChordRest::tuplet` (`chord.rs:334`) | supported (`reader/music.rs:736`) | supported | Nested tuplets are a non-standard extension; external ABC tools reject them. |
| Decorations (§4.14) | `Articulation` (`.../model/chord.rs:125-210`), `Ornament` (`.../model/notation.rs:100-167`), `Dynamic` (`.../model/annotation.rs:197-225`) | partial (`reader/music.rs:183-256`) | partial (`voice.rs`) | A named subset is mapped (`!staccato!`, `!accent!`, `!trill!`, `!mordent!`, `!fermata!`, dynamics, fingerings); stress/unstress and microtonal accidentals are not. |
| Symbol lines (§4.15) | `SpannerType::Hairpin`, `TextLine` (`.../model/spanner.rs:268-287`) | partial (`reader/music.rs:209`) | partial | `!<(!`/`!<)!` map to hairpins; arbitrary text lines lose their drawn rule. |
| Redefinable symbols `U:`, inline macros (§4.16, §9) | — | partial (`reader/music.rs:236-278`, `reader/mod.rs:674`) | missing | User symbols may be redefined on input (`docs/agents/abc.md` §14.3); macros are not emitted on output. |
| Chords and unisons `[CEG]` (§4.17) | `Chord::notes: Vec<NoteId>` (`.../model/chord.rs:216-218`) | supported (`reader/music.rs:280`) | supported | Unisons are modelled as two notes with the same pitch. |
| Chord symbols `"C"` (§4.18) | `Harmony` (`.../model/annotation.rs:428-471`) | supported (`reader/music.rs:153`) | supported | Text is preserved; the structured `Harmony::name`/`root_tpc` fields are only filled where the text can be parsed. |
| Annotations `"^above"`, `"_below"` (§4.19) | `StaffText` (`.../model/annotation.rs:231-249`) | supported | supported (`voice.rs`) | `RichText` markup is not emitted in ABC annotations. Dynamic `velocity` is carried by the private decoration `!scoretk-velo-N!` (§4.14/§11); `W:` fields (§3.1) carry the model's credits. Tests `abc_dynamic_velocity.rs`, `abc_credits.rs`. |
| Lyrics `w:`, hyphen/underscore/`*` (§5) | `Lyrics`, `LyricsSyllabic` (`.../model/lyrics.rs:36-63`) | supported (`reader/lyrics.rs:36`) | supported | One `w:` line per verse; missing syllables become `*`. |
| Typesetting `%%score`, `%%staves` (§6.1) | `Part`, `Staff`, `StaffBracket` (`.../model/part.rs:200-333`) | supported (`reader/mod.rs:679`) | supported (`writer.rs:192`) | Only the first `%%score` directive is honoured. |
| Playback directives `%%MIDI` (§6.2) | `Instrument::midi_program`, `midi_channel`, `volume`, `pan` (`.../model/part.rs:71-84`) | partial (`reader/mod.rs::handle_directive`) | partial (`writer.rs::render`) | `%%MIDI program` (GM 1–128) and `%%MIDI channel` (1–16) round trip as zero-based model indices. The MuseScore instrument *id* has no standard ABC field; `V: … name=`/`snm=` (§7.1) carry the display names and the private `%%scoretk instrument` directive (§11, unknown directives must be ignored) carries the id. Test `abc_instrument_identity.rs`. |
| Multiple voices `V:`, inline `[V:]`, voice overlay `&` (§7) | `Track` (`.../model/ids.rs:123`), `Part::staves`, `Segment::elements` (`.../model/segment.rs:708-805`) | partial (`reader/build.rs:40`) | supported (`writer.rs:131`) | `V:` and inline fields work; `&` overlay is ignored with a warning (`docs/agents/abc.md` §7). |
| Embedded ABC fragments (§2.3), macros (§9) | — | partial (`reader/mod.rs`) | missing | Fragment extraction is limited; macros are not written. |

## Model gaps

Ranked by musical importance:

1. **Local key/metre/clef changes per staff**: ABC has one global `K:`/`M:`
   per tune, so `Part`-local `KeySig`, `TimeSig` and mid-bar `Clef` segments
   cannot be expressed. `docs/agents/abc.md` §7 lists the resulting 3 round-trip
   failures.
2. **Transposition**: `Transpose` (`part.rs:120`) exists in the model, but ABC
   has no standard field for it; a round-trip writes sounding/written pitch and
   cannot restore `<transpose>`.
3. **Complete articulation and ornament vocabulary**, including microtonal
   accidentals (`AccidentalType::Other`) and exact `stress`/`unstress`/
   soft-accent variants.
4. **Per-element notation layout**: beams (`BeamMode`), stems, note-head groups
   and computed system breaks; ABC only carries implicit beaming.
5. **Instrument/playback semantics** (program, channel, volume, pan, MIDI
   articulation table): partially read from non-standard directives. The
   instrument id now round trips through `%%scoretk instrument` (§11); the
   full MuseScore instrument definition and its articulation table remain
   outside ABC.
6. **Repeat count above 2, multi-bar voltas and nested tuplets**: either
   outside ABC 2.1 or not expressible in its grammar.
