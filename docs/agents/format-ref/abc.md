# ABC 2.1 — condensed reference for coding agents

Target: **ABC notation standard v2.1** (Dec 2011). The v2.2 draft in
`specs/abc/` is *not* the target. Local sources: `specs/README.md` § ABC,
`specs/abc/abc-standard-v2.1.wiki.txt` (section numbers below, e.g. `§4.3`),
`specs/abc/abc-standard-v2.1.html`. Licence: CC BY-NC-SA 3.0 Unported,
non-commercial use with attribution and share-alike; this is an original
summary, not copied spec text.
Companion: [ABC ↔ pivot model](../format-mapping/abc.md).
Reader `crates/scoretk-abc-import`, writer `crates/scoretk-abc-export`.

## File and tune structure

- File = one or more tunes, separated by blank lines; each tune starts with a
  header of `X:` (reference number, mandatory) followed by information fields
  `T:`, `C:`, `M:`, `L:`, `Q:`, `P:`, `K:`, etc. `%` starts a comment; `%%`
  starts a directive. (§2.1, §2.2, §2.2.4, §2.2.5, §3.1.1–3.1.18, §11)
- The **first `K:` field ends the header**; all later lines are tune body.
  A tune without `K:` is invalid. (§3.1.14)
- Inline fields `[M:3/4]`, `[K:G]`, `[V:1]` may appear in the body and take
  effect from that point on; `[V:…]` selects a voice. (§7.3, §7.1)
- `P:AAB` in the header + `P:A`/`P:B` in the body marks played sections;
  `P:` names a *section*, not a polyphonic voice. (§3.1.9, §4.10)

## Units and time encoding

- `L:1/8` sets the **unit note length**; a bare note letter `A` = 1 unit,
  `A2` = 2 units, `A3` = 3 units (dotted when 3 = 1.5×2), `A4` = 4 units,
  `A6` = dotted quarter at `L:1/8`. A trailing `/` halves: `A/` = `A/2`,
  `A//` = `A/4`, `A3/2` = dotted eighth at `L:1/8`. (§4.3)
- If no `L:`, the unit is inferred from the meter value: <0.75 → 1/16,
  ≥0.75 → 1/8; `M:C`, `M:C|`, `M:none` → 1/8. A meter change in the body
  does **not** change the unit. (§3.1.6, §3.1.7)
- `M:4/4`, `M:C` = common time, `M:C|` = cut time, `M:none` = free meter,
  `M:(2+3+2)/8` = additive meter. (§3.1.6)
- `Q:1/4=120` = quarter = 120 BPM; up to four beat units may be summed;
  text like `Q:"Allegro" 1/4=120` is legal. (§3.1.8)
- Broken rhythm `a>b` = first note dotted, next halved; `a<b` = first halved,
  next dotted; `>>`/`<<` double-dot and quarter. Undefined on unequal note
  lengths. (§4.4)
- Rests `z` (visible) and `x` (invisible) take the same length modifiers as
  notes; `Z4`/`X4` = 4-measure (invisible) rest, `Z` alone = 1 measure. (§4.5)
- Barlines `|`, `|]`, `||`, `[|`, `|:`, `:|`, `::` (= `:|` + `|:`), `.|`
  (dotted), `[|]` (invisible). Parsers should accept loose mixtures of
  `| [ ] :`. (§4.8)

## Pitch, accidentals, key

- Notes `C`–`B` (upper = lower octave); `,` lowers an octave, `'` raises one,
  repeatable and combinable (`C,',` = `C,`). Only letter case and octave marks
  encode pitch; key/clef never alter the written letter. (§4.1, §4.2)
- Accidentals before the note: `^` sharp, `_` flat, `=` natural, `^^`/`__`
  double. In `K:` they may modify the signature: `K:D Phr ^f`, `K:D exp _b _e ^f`.
  (§4.2, §3.1.14)
- `K:G`, `K:A minor` (or `K:Am`), `K:D dorian`, `K:C exp …`, `K:none` /
  empty `K:` = no signature; only the first three mode letters are parsed.
  `K:HP`/`K:Hp` are bagpipe keys. (§3.1.14)
- Clef/transposition in `K:` or `V:`: `[clef=]treble|alto|tenor|bass|perc|none`
  + line number + `+8|-8`, `middle=<pitch>` (`m=`), `transpose=<semitones>`
  (`t=`), `octave=<n>`, `stafflines=<n>`; default treble. (§4.6, §7.1)

## Voices, staves, parts

- `V:ID` (ID = number or string, first 20 chars significant) starts a voice
  in the body; notes up to the next `V:` or EOF belong to it. `V:` in the
  header only sets properties (`name`, `subname`, `stem`, `clef`, …). (§7, §7.1)
- Voice grouping on staves is not defined by `V:` itself; it needs the
  directive `%%score (T1 T2) (B1 B2)`; GM instruments use `%%MIDI program …`.
  (§7, §11.1, §11.2)
- Voice overlay `&` combines simultaneous notes/voices; same-pitch notes in
  `[…]` are a chord (unison `[DD]` allowed). (§4.17, §7.4)
- `w:` lyrics apply to the **current voice** only; repeat `w:` in every voice
  that needs them. (§5, §7.3)

## Repeats, ties, slurs, tuplets, grace

- `|: … :|` repeats; `::` = `:|` + `|:`; `|::`/`::|` repeat three times;
  unmatched `:|` restarts from the tune start (or the last double barline).
  (§4.8)
- First/second endings `[1 … :|2 … |]`; with `P:A4.B4` an ending can be
  `[1,3 … :|`, `[1-3 … :|`, `[2,4 … :|`. (§4.9, §4.10)
- Tie `-` joins **adjacent notes of equal pitch**, directly after the first
  note (`c4-c4`, `abc-|cba` valid; `c4 -c4` and `abc|-cba` invalid — a space
  before the tie or a tie after the barline breaks it). A space *after* the
  tie is allowed. Slur `( … )` spans any notes, nests, may start/end on the
  same note; dotted forms `.(`/`.-`. Tie ≠ slur for playback. (§4.11)
- Tuplets `(2`…`(9` mean 2:3, 3:2, 4:3, 6:2, 8:3 (n=3 in compound meter,
  else 2). General form `(p:q:r` = p notes in the time of q for the next r
  notes; `(3` ≡ `(3:2:3`, `(3::2` = only two notes affected. Spaces after
  the specifier are ignored. (§4.13)
- Grace notes `{…}` take no melodic time; `{/g}C` is an acciaccatura. Grace
  notes are transparent to broken rhythm: `A<{g}A` ≡ `A/2{g}A3/2`. (§4.12)
- Chord `[CEG]`: no spaces inside; duration is that of the first note;
  `[C2E2G2]3` ≡ `[CEG]6`. (§4.17)
- Decorations: `.` staccato, `H` fermata, `!trill!`, `+` (etc.); symbol lines
  and redefinable symbols use `%%` directives. (§4.14–§4.16)

## Lyrics

- `w:<syllables>` follows a music line and aligns to notes of the current
  voice from the first unaligned note onward; extra syllables are ignored,
  missing ones leave blank syllables. `W:` is separate printed text. (§5, §5.1)
- Alignment tokens: `-` syllable break, `_` hold over an extra note, `*` skip
  one note, `~` space (multiple words on one note), `\-` literal hyphen, `|`
  advance to next bar. (§5.1)
- Grace notes, rests and spacers are **not** aligned; tied, slurred and beamed
  notes count as separate notes. Adjacent `w:` lines are successive verses.
  (§5.1, §5.2)

## Traps that silently break data

1. Missing/first `K:` ends the header — a `K:` in the body silently changes
   the key instead of opening a tune. (§3.1.14)
2. Length is in `L:` units, not note values: `A2` means 2 units, so changing
   `L:` re-reads the whole body. (§3.1.7, §4.3)
3. A mid-tune `M:` does not change `L:`; assuming it does shifts all rhythms.
   (§3.1.7)
4. `z` vs `x` (visible vs invisible rest), `Z4` vs `z4` (measures vs units).
   (§4.5)
5. Tie must touch the first note; slur and tie are not interchangeable.
   (§4.11)
6. Unmatched `:|`, `|::`, `[1` without `[2`, unexpected `[N`: be liberal but
   deterministic, or the repeat tree is lost. (§4.8–§4.10)
7. Spaces inside `[CEG]` break the chord; uneven note lengths inside a chord
   are undefined. (§4.17)
8. Grace notes consume no time, so lyric alignment and tick accounting must
   skip them. (§4.12, §5.1)
9. `w:` in one voice does not cover other voices. (§5, §7.3)
10. Deprecated 2.0 forms still occur in real files (old continuations,
    old decorations, old line breaks). (§10)

## Tiny valid example

```
X:1
T:Tiny
M:4/4
L:1/8
K:G
V:1
|: {c}G2 [GBd]2 (3abc d2 | e2 f2 g2 e2 |
[1 d2 B2 G2 B2 :|[2 c2 e2 g2 c'2 |]
w: Heigh- ho my dear
V:2
C,2 E,2 G,2 B,2 | C2 E2 G2 B2 |
C2 E2 G2 B2 | C,2 E,2 G,2 B,2 |]
```
Per bar at `L:1/8`: 8 units. Bar 1 = `G2` (2) + `[GBd]2` (2) + `(3abc` (2)
+ `d2` (2); `{c}` is a grace note (0). `w:` aligns to voice 1 only.
