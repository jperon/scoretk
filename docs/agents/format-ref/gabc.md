# GABC — condensed reference for coding agents

Target: **Gregorio's GABC**, as specified in the locally fetched Gregorio sources at
commit `b63f074253bd2f6b837f9f9778bba299ac139ec0`. Local sources:
`specs/README.md` § GABC / Gregorian chant;
`specs/gabc/gregorio/doc/Gabc.tex` (cited below as `Gabc.tex:<line>`), the
grammar `specs/gabc/gregorio/src/gabc/gabc-score-determination.y`
(`grammar:<line>`) and lexer `.l` (`lexer:<line>`), plus Gregorio's dump
writer (`src/dump/dump.c`) and clef/key rules (`src/struct.c`). The oracle
uses `gregorio -f gabc -F dump` as its only reference. Licence: Gregorio
GPL-3.0-or-later (`gregorio/COPYING.md`). This is
an original summary, not copied spec text. Companion:
[GABC ↔ pivot model](../format-mapping/gabc.md). No scoretk reader/writer
exists yet (phase 0).

## 1. File structure (`Gabc.tex:36-64`)

A `.gabc` file has two sections separated by a line containing `%%`
(`Gabc.tex:51,58`). The first is the **header** section (`name: value;`);
the second is the **notation** section mixing syllable text and parenthesized
neume code:

```gabc
name: incipit;
language: latin;
mode: 6;
%%

(clef) text(notes)
com(notes)plex(notes) word(notes)
```

- Spaces **and line ends in the notation section are significant**
  (`Gabc.tex:60-61`). A comment starts with `%` and runs to end of line; it
  also suppresses the end-of-line space (`Gabc.tex:63-64`).
- Multi-line header values omit the final `;` on the first line and end with
  `;;` (`Gabc.tex:69-74`).
- Grammar: a file is `definitions` followed by the notation (`grammar:737-769`);
  a note is `NOTES CLOSING_BRACKET` or `NOTES closing_bracket_with_space`
  (`grammar:854-925`). The lexer switches to its `notes` state at `lexer:386`
  and recognises `(`…`)` at `lexer:389-401`.

## 2. Headers (`Gabc.tex:67-154`)

Meaningful to Gregorio: `name` (**required**, `:79-82`), `gabc-copyright`
(`:83-87`), `score-copyright` (`:88-91`), `author` (`:92`), `language`
(`:93`), `mode` (`:94-102`), `mode-modifier` (`:103-105`),
`mode-differentia` (`:106-109`), `annotation` (one or two, `:110-127`),
`staff-lines` (`:128`), `nabc-lines` (`:129-130`),
`oriscus-orientation` (`:131-133`), `def-m0`…`def-m9` (`:134-135,205-226`).
Suggested but not interpreted: `office-part` (`:141`), `occasion` (`:142`),
`meter` (`:143`), `commentary`, `arranger`, `date`, `manuscript`,
`manuscript-reference`, `manuscript-storage-place`, `book`, `transcriber`,
`transcription-date` (format `yyyymmdd`, `:152`), `user-notes`
(`:144-153`). Mode headers are typeset only if no `annotation` is present
(`:156-188`).

## 3. Syllable text (`Gabc.tex:228-317`)

Text outside parentheses is the syllable; parentheses delimit the notes
(`:241-245`). A space after the last syllable of a word ends that word.

- `$` escapes the next character (`$(` inserts a literal parenthesis, `$$`
  a literal `$`) (`:247-250`).
- Style tags `<b>`, `<c>`, `<i>`, `<sc>`, `<tt>`, `<ul>` (`:254-270`).
- Syllable controls `<clear>`/`<clear/>`, `<e>…</e>` (elision),
  `<eu>…</eu>` (Euouae), `<nlba>…</nlba>`, `<pr>`/`<pr:.5>` (`:272-295`).
- Other tags `<alt>…</alt>` (above staff), `<sp>…</sp>` (special character),
  `<v>…</v>` (verbatim TeX) (`:297-309`).
- `{…}` forces lyric centering (`:319-335`); `[…]` is translation text,
  with `[/]` centring between two syllables (`:337-343`); `~` is a lyric tie
  (`:314-317`). Default centring is on the vowel (`:319-332,383-400`).
- Built-in `<sp>` characters include `ae`, `oe`, `A/`, `R/`, `V/`, `*`, `+`,
  `-`, `ot`, `ut`, `dt`, `~` (`:345-378`).

## 4. Pitches (`Gabc.tex:403-450`)

A pitch is one letter `a`–`n` or `p`, denoting a **typographical staff
position**, not a note name relative to the clef (`:410-413`). The allowed
range depends on `staff-lines`: 2 lines → `a`–`i` (`:415-416`), 3 → `a`–`k`
(`:425`), 4 (default) → `a`–`m` (`:434`), 5 → `a`–`n`,`p` (`:443`). A
lowercase letter is a *punctum quadratum*; uppercase is a *punctum
inclinatum*, with optional `0`/`1`/`2` for leaning direction
(`:452-468`). This is why any mapping to concert pitch is **clef-dependent**
(`Gabc.tex:847-870`; Gregorio's key rule `src/struct.c:1273-1288` and clef
changes `src/gabc/gabc-score-determination.c:89-100`).

## 5. Neume shapes (`Gabc.tex:452-622`)

One-note shapes are formed by appending characters to the pitch letter
(`:460-490`): `o` oriscus, `w` quilisma, `v` virga, `V` virga reversa,
`s` stropha, `~` liquescent deminutus, `<` augmented liquescent, `>`
diminished liquescent, `=` linea, `r` cavum, `R`/`r0` variants; alterations
`x` flat, `#` sharp, `y` natural, `?` parenthesized, `X`/`##`/`Y` soft
(`:512-521`). Oriscus orientation may be forced with `o0`/`o1`
(`:543-550`). Complex neumes are concatenations plus `-` (initio debilis),
`O` (oriscus scapus), `q` (quadratum), `W` (quilisma quadratum), `ss`
distropha, `sss` tristropha, `vv` bivirga, `vvv` trivirga
(`:556-579`). `@` fuses two primitives; `@[ … ]` fuses automatically
(`:603-666`). Spacing inside a neume uses `/0`, `/!`, `/`, `//`, `/<factor>`
and `!` (`:668-687`); `[shape:stroke]` alters clivis/flexus rendering
(`:689-700`).

## 6. Symbols added to notes (`Gabc.tex:702-797,1000-1119`)

- `.` punctum mora (`.0` below, `.1` above, max two, `:742-755`).
- `'` vertical episema / ictus (`'0` below, `'1` above, `:757-768`).
- `_` horizontal episema; numeric modifiers `0` below, `1` above, `2` no
  bridging, `3`/`4`/`5` small left/centre/right (`:769-797`); tuning
  `[oh:p]`/`[uh:p]` and block forms (`:1032-1086`).
- Accents/circles/musica ficta `r1`…`r8` (`:717-724`).
- Clefs `c`/`f`, optional `b` (flat), line `1`…`N`, two clefs with `@`
  (`:847-870`). Custos `z0`, `g+`, `[nocustos]` (`:872-884`); line breaks
  `z`, `z+`, `z-`, `Z`, `Z+`, `Z-` (`:886-906`).
- Choral signs `[cs:…]`, NABC `[cn:…]` (`:908-914`); braces `[ob:…]` etc.
  (`:916-931`); stem length `[ll:0]`/`[ll:1]` (`:933-968`); custom ledger
  lines `[oll:…]`/`[ull:…]` (`:970-998`); slurs `[oslur:…]`/`[uslur:…]`
  (`:1000-1019`); above-lines text `[alt:…]` (`:1088-1091`); verbatim TeX
  `[nv:…]`/`[gv:…]`/`[ev:…]` (`:1093-1102`); macros `[nm n]`/`[gm n]`/
  `[em n]` (`:1106-1118`).

## 7. Separation bars and "no fixed durations" (`Gabc.tex:801-845`)

Bars: `` ` `` virgula, `` `0 ``, `^` divisio minimis, `,` divisio minima,
`;` divisio minor, `:` divisio maior, `:?` dotted, `::` divisio finalis,
`;1`…`;8` Dominican (`:805-821`); `'` and `_` add symbols (`:823-830`).
Bars separate **sections**, not metrical measures: GABC has **no time
signature, no measure numbering and no fixed note durations**. The only
rhythmic information is the neume shape, punctum mora, episemata and bar
weight (`:742-799`, `:801-845`). A converter that assigns durations is
making a choice external to GABC and must state the rule it used; Gregorio
itself keeps staff positions and signs, not durations (`src/struct.h:809-813`,
`src/dump/dump.c:78-88,435-455`).

## 8. Pitfalls for implementers

1. **Spaces are music** in the notation section (`Gabc.tex:60-61,668-687`):
   dropping them changes neume grouping and therefore the derived shape.
2. `z` is not a note: it is a line break/custos (`Gabc.tex:872-906`); the
   grammar treats `)`, spaces and `|` as structural tokens, not pitches
   (`grammar:854-925`, `lexer:386-401`).
3. `w` after a pitch is a quilisma, `~` a liquescent, `'` an ictus, `_` an
   episema — all are signs attached to the preceding note, not separate
   notes (`Gabc.tex:452-490,702-797`; `src/dump/dump.c:358-455`).
4. Two-note primitives (`gf` flexus, `gh` pes) are one neume but two pitches;
   the GABC letter sequence, not the glyph count, defines the note count
   (`Gabc.tex:556-579,610-622`).
5. Bracketed `[…]` material is Gregorio layout/tuning data or verbatim TeX,
   not notes (`Gabc.tex:689-700,908-931,1093-1118`); it is parsed as an
   `attribute`/definition, not as a note (`grammar:741-769`).
6. The same letter means different concert pitches under different clefs
   (`Gabc.tex:410-413,847-870`); Gregorio resolves this with the clef key
   (`src/struct.c:1273-1288`) and adjusts pitch differences at clef changes
   (`src/gabc/gabc-score-determination.c:89-100`).
7. `::` is the divisio finalis, not two divisio maiores (`Gabc.tex:815-817`);
   Gregorio emits one `B_DIVISIO_FINALIS` element (`src/dump/dump.c:805-821`):
   do not count bar characters naively.
8. GABC records no metre at all (`Gabc.tex:801-845`); a target format that
   requires a time signature must invent one and report it as derived.
9. The `<sp>`/`<v>` markup in lyrics is typography, not lyrics
   (`Gabc.tex:297-309,345-378`): Gregorio records style boundaries and
   characters separately in its dump (`src/dump/dump.c:41-66`).

## 9. Model implications

See [GABC ↔ pivot model](../format-mapping/gabc.md): the primary data is a
syllable/neume stream with no metric layer; durations, bars and clefs in
modern-notation outputs are **derived**, so a round trip is lossy by design.
