# MuseScore `.mscx` / `.mscz` — condensed reference for coding agents

There is **no published specification** for this format: the authoritative
reference is [docs/agents/mscx-format.md](../mscx-format.md) (French, §1–§18),
backed by the read-only C++ snapshot at
`/home/jacques/programmes/musique/musescore/ref-musescore/` at commit
`2b9e82df847111d673c2051a93deeaaeea3f5a29` (see `docs/agents/upstream.md`).
Alignment target: MuseScore Studio **4.7.4** exports in `tests/corpus/`.
Licence: project GPL-3.0-only; no external spec text is copied here.
Companion: [MuseScore ↔ pivot model](../format-mapping/musescore.md).
Reader `crates/scoretk-musescore-import` (`read/read302` for 3.x,
`read/read4xx` for 4.x/5.x); writer `crates/scoretk-musescore-export`
(always writes `version="4.70"`).

**This file is a routing layer, not a second reference.** For syntax details,
read the linked section of `mscx-format.md`; do not re-derive behaviour from
general XML intuitions.

## Section index — what to read for each task

| Need | Authoritative section |
| --- | --- |
| Detect the schema version, dispatch reader | `mscx-format.md` §1 (version integer `maj*100+min`; `< 400` → 302, 400–409, 410–459, 460–499, ≥500) |
| Root tags, inline `<Style>`, `.mscz` paths | §2 |
| Fractions vs `<Division>`, ticks, `actualTicks()`, track = staff*4+voice | §3 |
| `<Part>`, `<Staff id>`, `<Instrument>`, transpose | §4 |
| `<Measure>`, `len`, barlines, courtesy elements | §5, §14 |
| `<voice>` blocks, implicit tick, require `<voice>` even when empty | §6 |
| `<Chord>`, `<Rest>`, `<Note>`, `<durationType>`/`<dots>`/`<duration>`, tpc | §7 |
| `<Tuplet>` / `<endTuplet>`, nesting | §8 |
| `<tick>` (absolute, in file divisions) vs `<location><fractions>` (relative) | §9 |
| `<KeySig>`, `<TimeSig>`, `<Clef>` and their version-dependent encodings | §10 |
| 3.x/4.x two-half ties vs 5.x `<Tie>` with EIDs | §11 |
| `<Slur>`, `<HairPin>`, `<Ottava>`, `<Pedal>`, `<Volta>`, `<SpannerMap>` | §12 |
| `<Lyrics>`, appoggiaturas/acciaccaturas (zero-duration grace chords) | §13 |
| Excerpts / linked staves | §15 |
| 3.02 → 5.0 feature matrix | §16 |
| Everything that silently breaks a reader/writer | §17 (**read before any change**) |
| `<StaffText>`, `<SystemText>`, `<VBox>`, `<TBox>` | §18 |

## `.mscz` container (adds detail to §2)

```text
META-INF/container.xml        rootfiles; take the first <rootfile> whose full-path ends .mscx
<name>.mscx                   the score named by that rootfile
score_style.mss               style override for 4.x/5.x (inline <Style> still legal)
Excerpts/<name>/<name>.mscx   linked parts — ignore for a full-score export
Thumbnails/thumbnail.png      artwork; no renderer in this library
audiosettings.json            writer emits "{}"
viewsettings.json             writer emits "{}"
```
`container.rs` in `scoretk-musescore-import` / `scoretk-musescore-export`
resolves the rootfile and falls back to the first `.mscx` at ZIP root.
A `.mscx` outside a container has its `<Style>` inline. (`mscx-format.md` §2)

## Unit system (one-line reminder, details in §3)

- Durations/lengths are **exact fractions of a whole note**: `1/4` quarter,
  `1/2` half, `3/8` dotted eighth. `<duration>`, `<len>`, `<fractions>`,
  `<ticks_f>` all use this form.
- `<Division>` only scales absolute `<tick>` values to the internal 480
  divisions per quarter; it does **not** scale duration fractions.
- `globalTicks = nominal / ∏ tuplet ratios`, then
  `actualTicks = globalTicks / staff time-stretch`; the cursor advances by
  `actualTicks()` after every non-grace chord/rest. (§3)

## Tiny valid example (schematic 4.x shape, not a 4.7.4 reference export)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.70">
  <programVersion>4.7.4</programVersion>
  <Score>
    <Division>480</Division>
    <metaTag name="workTitle">Tiny</metaTag>
    <Part id="1">
      <trackName>Piano</trackName>
      <Instrument id="keyboard.piano">
        <longName>Piano</longName>
        <Channel><program value="0"/></Channel>
      </Instrument>
      <Staff id="1"/>
    </Part>
    <Staff id="1">
      <Measure>
        <voice>
          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
          <Clef><concertClefType>G</concertClefType>
                <transposingClefType>G</transposingClefType></Clef>
          <KeySig><concertKey>0</concertKey><actualKey>0</actualKey></KeySig>
          <Chord>
            <durationType>quarter</durationType>
            <Note><pitch>60</pitch><tpc>14</tpc></Note>
          </Chord>
          <Rest><durationType>quarter</durationType></Rest>
          <Rest><durationType>quarter</durationType></Rest>
          <Rest><durationType>quarter</durationType></Rest>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
```
`<pitch>60</pitch>` is sounding MIDI pitch (C4); `<tpc>14</tpc>` is the
spelling (C natural). `concertKey`/`actualKey` are the 4.6+ encoding; 3.x/4.0
use `<accidental>2</accidental>` instead (§10.1, §17.11).

## Traps — checklist only (full explanations in §17)

1. Implicit tick; `actualTicks()` ≠ nominal `ticks()`.
2. `<dots>` before `<durationType>`; `<duration>` overrides both.
3. Positional `<voice>` blocks; unshared staves must repeat the measure
   sequence.
4. `<tick>` absolute vs `<location>` relative; `<move>` does not exist in
   3.02+.
5. Nested tuplets; each `<endTuplet>` closes one level.
6. Zero-duration grace chords attach to the next chord by `graceIndex`.
7. 3.x/4.x ties are two halves; 5.x uses EID links, and spanners can live in
   `<SpannerMap>` outside the measures.
8. Excerpts are separate `<Score>` trees; do not mix their staves into the
   full score.
9. `version="5.00"` is the integer 500 — never compare version strings.
10. A measure with no `<voice>` still has a duration: synthesise a
    full-measure rest on export.
