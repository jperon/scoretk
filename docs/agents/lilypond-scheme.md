# A minimal in-house Scheme evaluator for `scoretk-lilypond-import`

Costed study, 2026-11-23, master `c51ba17d`. **No production code was written.**
This document inventories the Scheme surface of the LilyPond reader, measures
Scheme use in the import lot, prices three implementation levels and
recommends a course of action. All line counts and file:line references were
taken from the tree at `c51ba17d`; corpus figures come from the run
`/tmp/meas-vf` (oracle `/tmp/ly95lect-refs-clean`, manifest
`/tmp/ly-import-lots/manifestes/ra.txt`).

## 0. Summary

- The reader special-cases Scheme in **≈1 700 lines** (9.4 % of its 18 049
  lines), spread over three files: the lexer's datum scanner (≈420 lines),
  the parser's value/definition helpers and inline branches (≈1 240), and
  `include.rs` (≈50). Those lines are not a coherent "Scheme support": they
  are five independent families of ad-hoc recognisers (§ 1.3).
- A single evaluator would **replace ≈1 250–1 400 of them** but would itself
  need ≈900–1 200 lines for Level A/B and ≈1 800–2 500 for Level C. The gain
  is correctness and robustness, **not size**; net line count is close to zero
  at Level B.
- On the `ra` lot (1 724 files), `#(…)` calls occur in 437 files (25 %) but
  music-producing definitions in only 31 (1.8 %): 24 of those 31 already read
  exactly.
  Of the **48 currently failing files**, 18 depend on Scheme (5 through an
  included file), and **13–15 could plausibly become exact** — +0.8 point at
  best (97.2 % → 98.0 %), for the price of a music-object layer.
- Level A (values, arithmetic, `define`) is expected to unlock **1 file**
  (`beat-performer.ly`, `\* #(+ 4 32)`); Level B adds **2–3**; Level C adds
  **10–12**, of which several still need non-Scheme semantics (instrument
  definitions, microtonal key signatures, drum modes).
- Gain/cost is 5–10× worse than the recent targeted fixes (`\volta` +3 files
  for ~120 lines, quotations +9 for ~200 lines, `partCombine` +5 for ~150).
- **Recommendation: do not build Levels B or C now.** A Level A "typed datum"
  spike is defensible only as a refactor of the numeric/boolean helpers, with
  a hard gate of ≥ 5 exact files on `ra` + `mut`, 0 loss. `steel` is wasm-capable
  but far too large; `scheme-rs` cannot target `wasm32-unknown-unknown`
  (cranelift is a mandatory dependency). No third-party crate should be added.

## 1. The reader's Scheme surface

### 1.1 Why Scheme is there

LilyPond embeds Scheme at the **token** level and delegates evaluation to
Guile; the grammar only says where a datum may appear:

- `lily/lexer.ll:407-419`: a `#` reads **one** datum with Guile's `scm_read`
  (`parse_embedded_scheme`, `lily/parse-scm.cc:206-307`) and returns it as
  `SCM_TOKEN`; `$` does the same and **evaluates immediately**
  (`lily/lexer.ll:421-441`). `#@`/`$@` splice a list as multiple values
  (`lily/parse-scm.cc:236-239`, `apply values`).
- `lily/parser.yy:565-615`: `embedded_scm` accepts that token, a
  `scm_function_call` or a lookup; `function_arglist` expects `EXPECT_SCM`
  arguments (`:1924`, `:2115`, `:2423`).
- `#{ … #}` is **LilyPond, not Scheme**: the upstream lexer switches to
  `embedded_lexer`; `scoretk` mirrors this with `Kind::Embedded`
  (`crates/scoretk-lilypond-import/src/lexer.rs:36-39,254-277`).
- `\include` accepts a Scheme expression for its file name
  (`lily/lexer.ll:353-377`), which `include.rs` recognises for one call form.

The reader's contract (`crates/scoretk-lilypond-import/src/parser.rs:26-29`,
`lib.rs:48`) means all of this must degrade to a warning, never to an error.
Any evaluator inherits that requirement.

### 1.2 Inventory by construct

Line ranges are inclusive; "lines" is the block size, not a count of
Scheme-only statements. `parser.rs` is 8 061 lines, `lexer.rs` 985,
`include.rs` 615.

| # | Construct | Location | Lines | Role |
| --- | --- | --- | ---: | --- |
| 1 | Datum token (`#…`, `$…`) | `lexer.rs:57-63, 278-298` | 28 | one `Kind::Scheme` token per datum |
| 2 | Datum scanner | `lexer.rs:549-869` | 321 | lists, dotted pairs, vectors, chars, strings, block comments `#\|…\|#`, `#;`, booleans, embedded `#{…#}`, quasiquote prefixes |
| 3 | Token value helpers | `lexer.rs:118-165` | 48 | `is_scheme`, `scheme_decimal`, `scheme_fraction` (atom scan of `#3/4`, `(ly:make-moment n d)`) |
| 4 | Display of Scheme/Embedded | `lexer.rs:179-186` | 8 | re-printing `#`/`#{` |
| 5 | Embedded music token | `lexer.rs:254-277`; `parser.rs:2360-2364, 2563-2593` | 60 | `#{…#}` re-parsed as music via `parse_token_stream` |
| 6 | Music-function definition | `parser.rs:142-176, 6330-6407` | 112 | `MusicFunction` struct + `music_function_definition` (head, params, predicates, `make-relative` flag, body tokens) |
| 7 | Function body expansion | `parser.rs:2595-2659` | 65 | depth guard, saved duration state, argument re-parse, `$`/`#`/`\` substitution |
| 8 | Predicate-driven argument reading | `parser.rs:2660-2783` | 124 | `read_function_argument(s)` by predicate suffix, `ly:context-mod?`, `music?`, `duration?` |
| 9 | Body-returns-parameter heuristic | `parser.rs:6408-6479` | 72 | last Scheme form names a parameter ⇒ return that music |
| 10 | `#(define …)` music variable | `parser.rs:2838-2848, 6232-6262` | 42 | `#(define name #{…#})`, inserted by `$name` |
| 11 | `afterGraceFraction` | `parser.rs:2819-2837, 7489-7498` | 29 | `#(define afterGraceFraction (cons 15 16))` |
| 12 | `ly:make-duration` identifier | `parser.rs:6281-6329`; use at `5597-5671` | 60 | log2, dots, compression factor |
| 13 | Note-language datum | `parser.rs:6263-6280` | 18 | `#(ly:parser-set-note-names …)`, also mid-music |
| 14 | Numeric literals | `parser.rs:5902-5938, 7499-7535` | 74 | `#9/8`, `#(ly:make-moment …)`, `` #`(n . d) `` for `\scaleDurations`; `\ottava #1`, `bendAfter #10`, `\time`, `\partial`, `\tuplet` |
| 15 | Booleans/settings | `parser.rs:4427-4510` (+ inline `##t` tests in `parse_command`) | ~95 | `\set Timing.timing = ##f`, `tieWaitForNote`, `skipTypesetting`, `melismaBusyProperties = #'()`, `\once \override … transparent = ##t` |
| 16 | Quoted symbols | `parser.rs:248-271, 2784-2818, 5981-6041`; `lexer.rs` `#'` scanning | ~110 | tags (`\tag`, `\keepWithTag`, `\tagGroup`), old `\override Context #'prop`, `\tweak #'style`, `\parallelMusic #'(A B)` |
| 17 | Property values | `parser.rs:4544-4669` | 126 | `\override`/`\tweak`/`\offset`: evaluate-and-discard, with sign handling |
| 18 | Skip helpers | `parser.rs:5939-5980, 6051-6115` | 70 | `skip_scheme_value`, `skip_paren_list`, unknown-command argument skipping |
| 19 | `\include` of a Scheme expression | `include.rs:136-146, 196-207, 301-341` | 50 | only `(ly:parser-include-string "…")`; escape decoding (`lily/lily-lexer.cc:302-316`) |
| 20 | Inline command hooks in `parse_command` | `parser.rs:2910-4259` | ~150 | `applyMusic`/`musicMap`/`skip-of-length`, `contextPropertyCheck`, `cueDuring`, `voices`, `incipit`, `markupMap`, `beamExceptions`, `stringTuning`, `\*` Scheme count, `partCombine` chord-range, `ottava`, `bendAfter` … |
| 21 | Text helpers | `parser.rs:6480-6580` | 101 | `skip_scheme_space`, `read_scheme_word`, `read_scheme_list`, `split_scheme_words` |
| 22 | Tag-name extraction | `parser.rs:248-271` | 24 | `#'a`, `#'(a b)`, `"a"`, `a,b` |
| 23 | Text/value conversion | `parser.rs:7666-7680` | 15 | `scheme_text_value`, `is_empty_scheme_list` |
| | **Total** | | **≈1 700** | |

Rows 10–16 partly overlap rows 3–5 and 20 (the numeric, boolean and tag
helpers are also reached from `parse_command`); row 22 repeats row 16. The
de-duplicated total is ≈1 700. Cross-check with a mechanical count: 343
source lines in
`crates/scoretk-lilypond-import/src/` contain `scheme`, `datum` or `embedded`
(223 in `parser.rs`, 100 in `lexer.rs`, 12 in `include.rs`, 8 elsewhere);
the difference with 1 700 is the body of the dedicated helpers, whose
statements do not repeat the word.

### 1.3 What one evaluator would actually replace

| Family | Removed (est.) | Kept |
| --- | ---: | --- |
| Datum scanner (1–4) | 380 | a real s-expression reader of similar size (~350 lines) |
| Numeric/bool/symbol extraction (3, 14, 15, 16, 23) | 300 | none |
| `define` / music functions (6–13) | 380 | argument *consumption* for `ly:music?` (~80 lines), embedded-music callback (~60) |
| Property/skip/text helpers (17–19, 21) | 220 | `\include` resolution itself |
| Inline command hooks (20) | 100 | the semantic hooks that change playback (`skipTypesetting`, `note-names`, `duration identifiers`) |
| **Total** | **≈1 380** | ≈250–300 |

A Level A/B evaluator adds ≈900–1 200 lines (reader ~350, values/env ~250,
eval ~250, builtins ~200, errors/tests ~100). Net at Level B: **−1 380 +
1 050 ≈ −330 lines** if the old helpers are deleted in the same commit;
**+1 000** if they are kept side by side during a transition. The claim "one
evaluator replaces a lot of code" is therefore true only after deletion, and
the replacement itself is not smaller than what it removes. The real benefit
is that `#(string-join (map symbol->string style) ".")` stops being a text
scan and becomes a value.

### 1.4 What an evaluator would *not* fix

Property values (`\override X.y = #…`), settings and most toplevel data are
already **evaluated and discarded** correctly: `\override` 715 occurrences and
`\set` 556 in `ra` never need the value, only its extent. Likewise
`#(ly:set-option …)` (214 occ/213 files), `#(set-global-staff-size …)`
(99/97), `#(set-paper-size …)` (69/69) and `#(ly:expect-warning …)`
(128/59) are configuration noise that the reader consumes silently today.
An evaluator buys nothing there, and a wrong extent (already avoided) would be
a regression, not a gain.

## 2. Scheme in the import lot

### 2.1 Method

The three manifests `/tmp/ly-import-lots/manifestes/{ra,mus,mut}.txt` were
scanned in one single-threaded Python process (no parallelism, no cargo):
comments (`%`, `%{…%}`) and strings were stripped, then regexes counted the
forms. Occurrences and **files containing at least one occurrence** are
reported separately. The scanner is reproduced in § 6 so the figures can be
re-derived; no source or reference is modified.

### 2.2 Counts

| Form | `ra` (1 724 files) | `mus` (1 445) | `mut` (1 377) | Total (4 546) |
| --- | ---: | ---: | ---: | ---: |
| `#(…)` | 960 / 437 | 1 913 / 849 | 3 193 / 929 | 6 066 / 2 215 |
| `#'…` | 1 038 / 357 | 6 346 / 1 282 | 13 401 / 1 367 | 20 785 / 3 006 |
| booleans (`#t` / `##t`) | 1 943 / 1 000 | 2 088 / 960 | 3 993 / 1 045 | 8 024 / 3 005 |
| `#number` | 696 / 281 | 876 / 112 | 16 467 / 1 362 | 18 039 / 1 755 |
| `#{…#}` | 60 / 35 | 27 / 27 | 153 / 91 | 240 / 153 |
| `#(define …)` (all) | 168 / 132 | 20 / 18 | 216 / 121 | 404 / 271 |
| `#(define-music-function …)` | 41 / 31 | 27 / 27 | 134 / 88 | 202 / 146 |
| `#@` / `$@` (splicing) | 8 / 5 | — | — | 8 / 5 |
| `$` (immediate) | 29 / 8 | — | — | 29 / 8 |
| `#\` (character) | 1 / 1 | — | — | 1 / 1 |

Values are `occurrences / files`. `#number` is dominated by layout and option
values in `mut` (16 467), largely staff sizes and paper settings, not by
music.

The `ra` lot is the LilyPond regression suite: it over-represents Scheme on
purpose. In `mus` (real scores) 1.9 % of files define a music function; in
`mut` 6.4 %. `#(…)` calls occur in 2 215 of 4 546 files (49 %), but
`#(define-music-function …)` in 146 files (3.2 %): the two figures must not
be confused.

Most frequent callees in `ra`: `ly:set-option` 214, `ly:expect-warning` 116,
`define` 114, `set-global-staff-size` 99, `set-paper-size` 69, `set!` 69,
`define-music-function` 41, `lambda` 27, `magstep` 17. `music-map` appears in
2 files (4 occurrences), `\applyMusic`/`\musicMap` in 3, `ly:music-property`
in 1 (6 occurrences), `ly:music-set-property!` in 1, `eval-string` in 1,
`pitch->name` in 1. No file has more than 19 `#(` occurrences.

### 2.3 The failing files

Latest measurement (`/tmp/meas-vf`, master `c51ba17d`, 1 724 files, oracle
`/tmp/ly95lect-refs-clean`): **1 676 exact (97.2 %)**, N1 98.14 %, N2 95.75 %,
N2-grace 96.72 %, 7 empty outputs, **48 non-exact**. (The table of 135 files in
`docs/agents/lilypond.md` § “Table unique des 135 fichiers non exacts” is an
older snapshot at 1 589 exact; it is not the current list.)

`S?` = Scheme is involved; `L` = level that would be needed. "Cause" is the
first divergence group of the run; where this study did not re-diagnose a
file, it is marked "not this study".

| File | Flags | S? | Scheme form / need | L | Expected |
| --- | --- | --- | --- | --- | --- |
| `accidental-styles.ly` | notes_supp | yes | `define-music-function` + `#{…#}` + `map symbol->string` + `string-join` + optional `*unspecified*` arg | B + opt. args | likely exact |
| `alteration-glyphs.ly` | sortie_vide | yes (`.ily`) | `define-scheme-function` + `let` + `ly:make-pitch` + `add-score` | C | likely exact |
| `arabic-accidental-glyphs.ly` | sortie_vide | yes (`.ily`) | idem + `arabic.ly` alterations | C | likely exact |
| `makam-accidental-glyphs.ly` | sortie_vide | yes (`.ily`) | idem + `makam.ly` | C | likely exact |
| `turkish-makam-accidental-glyphs.ly` | sortie_vide | yes (`.ily`) | idem + `turkish_makam.ly` | C | likely exact |
| `beat-performer.ly` | notes_manquantes | yes | `\* #(+ 4 32) 32`: arithmetic as repeat count (ref 190 vs test 122 = 68 = 2 staves × (36 − 2), exactly the missing repeats) | A | likely exact |
| `context-descend-only-not-found.ly` | notes_manquantes | yes | `#(descend-to-context #{…#} 'Staff "fail" #{…#})` | C + builtin | likely exact |
| `drum-scripts.ly` | transposition, notes_manquantes | yes | `#(define predefined-drumstyles …)`, lambdas, `#{…#}`, `symbol->string`, drum modes | C+ | uncertain |
| `instrument-switch.ly` | transposition | yes | `` #`((instrumentTransposition . ,(ly:make-pitch -1 6 FLAT)) …) `` + `\addInstrumentDefinition` | C + instrument table | unlikely |
| `ledger-line-properties.ly` | notes_manquantes | yes | `#(define … '(…))`, `$@(map (lambda …) (iota …))`, `#(list i)`, `format`, `number->string` | B + splicing | likely exact |
| `make-relative.ly` | octave | yes | `make-relative` macro (`scm/music-functions.scm:2384`) + `make-event-chord` | C | likely exact |
| `make-relative-music.ly` | octave | yes | idem | C | likely exact |
| `midi-key-signature.ly` | sortie_vide | yes (`arabic.ly`) | `` bayati = #`(…) `` alist + `\key fa \bayati` + microtonal key signature | A + key semantics | uncertain |
| `music-map.ly` | notes_supp | yes | `\applyMusic #(lambda (x) (music-map notes-to-skip x))` + `make-music 'SkipEvent` | C | likely exact |
| `optional-args.ly` | octave, notes_manquantes | yes | optional arguments with `#{ … #}` defaults in `define-music-function` | B + opt. args | likely exact |
| `persian-key-signatures.ly` | octave, notes_manquantes | yes | `define-music-function` + `#(eval-string key)` + `pitch->name` + `\key` alteration | C+ | uncertain |
| `scheme-list-splicing.ly` | sortie_vide | yes | `#@`/`$@` splicing, `map`, `iota`, `ly:make-pitch`, `apply values` | B + C pitch | likely exact |
| `tie-chord.ly` | sortie_vide | yes | `define-music-function` + `make-music 'TieEvent` + `ly:music-deep-copy` + `set!` + `append-map` | C | likely exact |
| `alignment-order.ly` | attaque_decalee | no | — | — | not this study |
| `beam-position.ly` | attaque_decalee, notes_supp | no | — | — | not this study |
| `bend-after-start-in-grace.ly` | notes_manquantes | no | — | — | not this study |
| `chord-name-entry.ly` | octave, notes_supp | no | chordmode/chord names | — | not this study |
| `chord-names-spanbar.ly` | octave, notes_manquantes | no | chord names | — | not this study |
| `chord-voicings.ly` | octave, notes_manquantes | no | chord names | — | not this study |
| `midi-unisons.ly` | notes_supp | no | — | — | not this study |
| `modal-transforms.ly` | octave, transposition, notes_supp | no | — | — | not this study |
| `note-names-makam.ly` | octave, notes_manquantes | no | makam note names | — | not this study |
| `part-combine-global.ly` | notes_manquantes | no | `\partCombine` | — | not this study |
| `polymetric-time-ossia-staff.ly` | attaque_decalee | no (layout only) | `#(ly:set-option …)` | — | not this study |
| `push-to-tag.ly` | octave, notes_manquantes | no | `\pushToTag` (built-in music function) | — | not this study |
| `quote-cyclic.ly` | notes_manquantes | no | `\quoteDuring` | — | not this study |
| `quote-during-subvoice.ly` | octave, notes_manquantes | no | `\quoteDuring` | — | not this study |
| `quote-transposition.ly` | transposition | no | `\quoteDuring` / transposition | — | not this study |
| `repeat-unfold-all.ly` | attaque_decalee, notes_supp | no | `\unfoldRepeats` | — | not this study |
| `skiptypesetting-tuplet.ly` | octave, transposition | no | `skipTypesetting` + `\relative` cursor | — | not this study |
| `spacing-grace.ly` | attaque_decalee, notes_supp | no | grace timing | — | not this study |
| `spacing-non-adjacent-columns3.ly` | octave, notes_manquantes | no | — | — | not this study |
| `spacing-strict-spacing-grace.ly` | attaque_decalee | no | grace timing | — | not this study |
| `staff-highlight-ossia.ly` | attaque_decalee | no | ossia | — | not this study |
| `staff-ossia-end-at-time-change.ly` | attaque_decalee | no | ossia | — | not this study |
| `swing-test.ly` | attaque_decalee | no | `\tripletFeel` / `\applySwing` (built-in, Scheme timing) | — | not this study |
| `tablature-full-notation.ly` | attaque_decalee, notes_supp | no (layout only) | `#(set-global-staff-size 18)` | — | not this study |
| `tablature-harmonic-functions.ly` | octave, transposition | no | tablature harmonics | — | not this study |
| `tablature-tie-behaviour.ly` | attaque_decalee | no | tablature ties | — | not this study |
| `v4_linked-ties.ly` | notes_supp | no | — | — | not this study |
| `v4_testExcludeInvisibleElements.ly` | octave | no | — | — | not this study |
| `voices-command.ly` | notes_supp | no | `\voices` residual | — | not this study |
| `volta-spec-tie.ly` | notes_manquantes | no | `\volta` + tie | — | not this study |

**18 of 48** files involve Scheme; **14** need only the evaluator (plus one
builtin for `descend-to-context`, and one separate optional-argument feature
for `optional-args.ly`), **4** also need unrelated semantics. Two
files carrying Scheme (`polymetric-time-ossia-staff`, `tablature-full-notation`)
are not blocked by it: the datum is a side-effect call, already consumed
correctly.

Outside `ra`, the same analysis cannot be done without a `mut` measurement of
the current binary; 88 `mut` files use `define-music-function`, so the
potential there is real but unmeasured. That measurement is the first step of
any Level C decision (§ 4).

## 3. Levels

Estimates below use the measured density of this crate in the delivered wasm:
`scoretk_lilypond_import` = 165 056 bytes for 18 049 source lines
(`docs/agents/wasm.md`, "Aggregation by crate"), i.e. **≈9.1 B/line**, call it
8–12 B/line ±50 %. The last measured delivered binary is 1 924 141 raw bytes
(`docs/agents/wasm.md`); Cantus's limit is ≈2.5 MB, i.e. **≈576 KB of
headroom**.

### Level A — literals, quote, arithmetic, booleans, `define` of values

**Features.** s-expression reader (booleans `#t/#f`, integers, exact
rationals `#1/2`, floats, strings, chars, symbols, lists/vectors, dotted
pairs, `quote`/`quasiquote`/`unquote`, R7RS §4.1.2, §4.2.8); evaluator with
lexical environment, `define`, `set!`, `if`, `and/or/not`, `+ - * / = < > <=
>=`, `quotient/remainder/modulo`, `cons/car/cdr/list/null?/pair?`,
`symbol->string/string->symbol/number->string/string-append`; errors mapped to
`Warning`. No closures, no music.

**Files unlocked.** `beat-performer.ly` (1, expected). `midi-key-signature.ly`
becomes reachable only with a separate `\key` alteration-list feature (1 more,
uncertain). `instrument-switch` needs an instrument table, `persian` needs
`eval-string`. → **1–2 files, +0.06 to +0.12 point.**

**Code.** +500–700 new, −250–350 removed (numeric/boolean/tag/text helpers).
Net **+250 to +400 lines** if the old helpers go.

**Wasm.** +2 to +5 KB raw (0.1–0.3 %), i.e. within noise.

**Risk.** Low to medium. R7RS exact rationals already exist in the model
(`Fraction`), and `#1/2` is the common case; floats must not leak into
durations. The main trap is Guile's *read-then-eval* order and `$` vs `#`
(`#` evaluates in the parser module, `$` immediately and may splice); the
current code already distinguishes them by token text, and the evaluator must
keep that. No memory-management risk at this level.

### Level B — plus `lambda`, `let`, `if/cond`, lists (`map`, `append`, …)

**Features.** Closures and lexical scope (R7RS §4.1.4, §4.2.2), derived
conditionals (`cond`, `case`, `when`, `unless`, §4.2.1), list library
(`map`, `for-each`, `apply`, `append`, `reverse`, `length`, `list-tail`,
`assoc`, `iota` — §6.4), `set!` on pairs and vectors, recursion with a depth
guard.

**Files unlocked.** `accidental-styles.ly` and `ledger-line-properties.ly`
(2); `optional-args.ly` only if optional-argument matching with `#{…#}`
defaults is added (a separate ~60-line change in `read_function_argument`);
`scheme-list-splicing.ly` needs `#@`/`$@` *and* `ly:make-pitch` (Level C).
→ **2, at most 3 files, +0.12 to +0.17 point.**

**Code.** +1 000–1 300 new, −500–700 removed. Net **+400 to +700 lines**.
**Wasm.** +6 to +12 KB raw (0.3–0.6 %).
**Risk.** Medium. Rust ownership of closures that capture `Music` values
forces `Rc<RefCell<…>>` or an arena; a cycle leaks, and wasm has no GC hook.
Mutation (`set!`) is required by `music-map` and must not alias the model.
Error propagation through 5–6 recursive eval frames needs the existing
`MAX_EXPANSION_DEPTH`-style guard, or a malicious/looping score stalls the
reader. Test surface: 24 Scheme-specific integration tests
(`tests/read_lilypond.rs`) plus 4 lexer tests would need rewriting.

### Level C — plus the LilyPond music API

**Features.** A music-object layer over the current `ast::Music`:
`make-music` (`lily/music-scheme.cc:74-88`), `ly:music-property`
(`:48-60`), `ly:music-set-property!`, `ly:music-deep-copy`,
`music-is-of-type?`, `music-map` (`scm/music-functions.scm:62-76`),
`make-event-chord` (`:736-738`), `descend-to-context` (`:715-721`),
`make-relative` (macro at `:2384-2430`), `ly:make-pitch`,
`ly:make-duration` (`lily/duration-scheme.cc:53-99`), `ly:make-moment`
(`lily/moment-scheme.cc:28`), `add-score` (`scm/lily-library.scm:280`), plus
the `#{…#}` callback and deferred score assembly.

**`scm/` files to port.** A strict subset only:
`scm/music-functions.scm` (music-map/selective-map/filter, make-relative,
make-event-chord, descend-to-context, make-non-relative-music — ≈400 lines of
a 2 998-line file), `scm/lily-library.scm` (`add-score`, ≈30 lines of 1 848),
and the bodies of the built-ins actually called (`ly/music-functions-init.ly`
as a reference). Porting whole files is not required; `define-music-types.scm`
(1 030 lines of class definitions) can be reduced to the handful of classes
used (`SkipEvent`, `TieEvent`, `SequentialMusic`, `EventChord`,
`ContextSpeccedMusic`).

**Files unlocked.** The four glyph tests, `make-relative` ×2, `music-map`,
`tie-chord`, `scheme-list-splicing`, `context-descend-only-not-found` → **10
likely**; conditional additions (`persian-key-signatures`, `drum-scripts`,
`instrument-switch`, `midi-key-signature`) → **12–14 optimistic**, several
still needing non-Scheme semantics. → **+0.6 to +0.8 point.**

**Code.** +1 800–2 500 new (object layer ~600, API ~500, evaluator ~1 100),
−700 to −1 000 removed. Net **+1 100 to +1 700 lines**, plus 400–700 ported
Scheme lines carrying GPL headers and a `NOTICE` entry (AGENTS invariant 4).

**Wasm.** +15 to +35 KB raw (0.8–1.8 %). Fits the headroom, but consumes 3–6 %
of it for 10–12 files.

**Risk.** High. The pivot AST is not LilyPond's music-object graph: properties
are typed fields, not a property alist, and `make-music` must map class names
to enum variants. `add-score` builds a score *outside* the reader's single
partition choice (`parser.rs:435-505`), so it needs an explicit deferred-score
path. `music-map` must preserve durations/onsets exactly (the 156-note
`tie-chord.ly` is the test). `make-relative` is a syntax-rules macro, not a
procedure: a hand-coded special form is required (the reader already has a
`relative_body` heuristic for it, `parser.rs:6373-6378`). Each ported
`scm` function must be tested against its one file, and the licence notice
must be added before the first commit.

### Level comparison with `steel` and `scheme-rs`

Checked on 2026-11-23 (crates.io/GitHub, not from memory):

| | `steel-core` 0.8.3 | `scheme-rs` 0.2.0 |
| --- | --- | --- |
| Language | Scheme, R5RS + modules, bytecode VM, optional cranelift JIT | R6RS, JIT (cranelift), delimited continuations |
| Licence | MIT OR Apache-2.0 (compatible with GPL-3.0-only) | MPL-2.0 (compatible, file-level copyleft) |
| wasm32-unknown-unknown | yes: `[target.'cfg(target_family = "wasm")']` deps `getrandom`/`js-sys`; JS playground exists | **no**: `cranelift`, `cranelift-jit`, `cranelift-native`, `cranelift-module` are **non-optional** dependencies; JIT cannot host on wasm |
| Default features | `std`, `modules` (std pulls `chrono`) | `load-libraries-from-fs` (filesystem by default) |
| Size (source) | 66 148 Rust lines + 3 352 Scheme | 24 565 Rust lines |
| Estimated wasm cost | ≥ 0.6–1 MB raw for the interpreter alone, plus a large new dependency graph (`serde`, `bincode`, `lasso`, `num-bigint`, `crossbeam`, `icu_casemap`, …) → **exceeds the 576 KB headroom**; not built here | not buildable |
| Fit to LilyPond | generic Scheme; all LilyPond primitives (`ly:*`, music objects, pitches) still have to be written as Rust callbacks | idem, plus async/tokio runtime and JIT |

Both are disproportionate: even with `steel`, the expensive part — the music
API and the LilyPond primitives — remains ours. `steel` would add a second
language implementation to maintain and a dependency whose own parser/VM
is four times the size of our whole reader. `scheme-rs` is excluded on wasm
grounds alone. Neither is a shortcut.

### Summary

| Level | New lines | Net lines | Wasm | Files | Risk | Files / 100 lines |
| --- | ---: | ---: | ---: | ---: | --- | ---: |
| A | 500–700 | +250–400 | +2–5 KB | 1–2 | low-med | 0.2–0.3 |
| B | 1 000–1 300 | +400–700 | +6–12 KB | 2–3 | medium | 0.2 |
| C | 1 800–2 500 | +1 100–1 700 | +15–35 KB | 10–12 | high | 0.4–0.55 |

With `steel`, the binary grows by ≥ 0.6 MB (estimate) and no line of the
LilyPond music API disappears; with `scheme-rs` there is no wasm build at all.

For comparison, recent targeted reader fixes measured in
`docs/agents/lilypond.md`: `\volta` +3 files for ~120 lines (2.5 per 100),
`\killCues` +1 for ~50 (2.0), absolute-time quotations +9 for ~200 (4.5),
`partCombine` +5 for ~150 (3.3). These are order-of-magnitude comparisons
(the exact commits differ and the line counts are estimated from them), but
the gap is large enough to be robust. Level C is **5–10× less efficient per
line**, and Level B is worse still.

## 4. Recommendation

**Do not implement Levels B or C now.** The measured ceiling is 13–15 files
(one point), the cost is a music-object layer and 2 000+ lines, and the
remaining failures are dominated by causes (grace timing, quotations,
`partCombine`, chord names, ossia, tablature) where a Scheme evaluator is
irrelevant.

**Level A is optional, and only as a refactor.** The current numeric/boolean
recognisers (`scheme_duration_definition`, `scale_scheme_fraction`,
`after_grace_pair`, `scheme_fraction`, the `##t` tests in `parse_set`) are
text scanners that will keep needing patches (`#(+ 4 32)` is exactly the kind
of expression they miss). Replacing them with one typed datum reader is
defensible *if* it is measured, not assumed. Gate: **≥ 5 exact files on
`ra` + `mut`, 0 loss, wasm ≤ +2 % (≤ 38 KB)**; otherwise revert and keep the
current helpers.

Step plan if the Level A spike is approved:

1. Add `scheme/` (or `scm.rs`) to `scoretk-lilypond-import`: `Datum` enum,
   reader over `&str` (reuse `lexer.rs:549-869` spans), no I/O, no threads;
   tests taken from R7RS §4.1/§6.2 examples. One reduced test per form.
2. Add `eval` with a global environment owned by `Parser`: literals,
   `define`, `set!`, `if`, arithmetic, list/string/symbol primitives; every
   error becomes a `Warning` and `Music::Empty` (reader contract).
3. Replace the numeric/bool helpers one at a time, each behind its existing
   integration test; delete the text scanner of the replaced family in the
   same commit.
4. Add `\key` alteration lists only if step 3 shows the `midi-key-signature`
   gain; that is a separate semantic feature, not evaluator work.
5. Measurement protocol (non-negotiable, see `docs/agents/lilypond.md`
   § “Vérification des gains”): same manifest, same oracle
   (`/tmp/ly95lect-refs-clean`), only the binary changes; compare exact files
   and per-file N1/N2; at most 2 workers (`PARALLEL=2` /
   `CARGO_BUILD_JOBS=2 nice -n 10`); never compare across masters.
6. Go/no-go on the gate. A negative result is documented here and the spike is
   reverted, not merged "because it is cleaner".

Step plan if Level C is ever requested (e.g. `mut` users need music
functions):

1. Measure `mut` first with the current binary; count files whose *first*
   divergence is a music function. If < 8, stop.
2. Build the music-object layer (class name ↔ AST variant, property
   get/set/copy) with round-trip tests on `SkipEvent`, `TieEvent`,
   `SequentialMusic`, `EventChord`.
3. Port `music-map`, `make-event-chord`, `make-relative`,
   `descend-to-context`, `add-score` with file:line citations and GPL
   headers; declare `scm/music-functions.scm` and `scm/lily-library.scm` in
   `NOTICE`.
4. Add `ly:make-pitch/duration/moment`, `ly:music-property`,
   `ly:music-deep-copy`; test against `scheme-list-splicing.ly` and
   `tie-chord.ly`.
5. One reduced test per file class; measure each step; abort if the
   cumulative gain at step 4 is < 6 files or any loss appears.
6. Re-check wasm size after every step (`nix build .#wasm`), since 35 KB is
   the stated ceiling for this feature.

Do not add `steel` or `scheme-rs` in any scenario covered here: `steel` blows
the wasm headroom, `scheme-rs` cannot be built for wasm, and both leave the
LilyPond music API — the actual cost — entirely to us.

## 5. Sources

LilyPond (reference copy `/home/jacques/programmes/musique/lilypond-ref`,
2.26.0):

- `lily/lexer.ll:302-316` (escapes), `:353-377` (`\include` of a Scheme
  expression), `:407-441` (`#` embedded Scheme, `$` immediate Scheme).
- `lily/parse-scm.cc:206-307` (`scm_read`, `@`/multiple values, `apply
  values`).
- `lily/parser.yy:565-615` (`embedded_scm`, `embedded_scm_bare`,
  `scm_function_call`), `:1924`, `:2115`, `:2423` (`EXPECT_SCM` arguments).
- `lily/music-scheme.cc:48-60` (`ly:music-property`), `:74-88`
  (`ly:make-music`).
- `lily/duration-scheme.cc:53-99` (`ly:make-duration`);
  `lily/moment-scheme.cc:28` (`ly:make-moment`).
- `lily/lily-parser-scheme.cc:326` (`ly:parser-set-note-names`).
- `scm/music-functions.scm:62-76` (`music-map`), `:715-721`
  (`descend-to-context`), `:736-738` (`make-event-chord`), `:2384-2430`
  (`make-relative`, a `define-syntax-rule-public`).
- `scm/lily-library.scm:280` (`add-score`).
- `ly/music-functions-init.ly` (bodies of the built-ins named above).

Scheme standard (R7RS-small, “Revised^7 Report on the Algorithmic Language
Scheme”): §4.1.2 literal expressions, §4.1.3 procedure calls, §4.1.4
`lambda`, §4.1.5 conditionals, §4.1.6 assignment, §4.2.1 derived
conditionals, §4.2.2 binding constructs, §4.2.8 quasiquotation, §5.1
programs, §5.3 variable definitions, §6.2 numbers, §6.3 booleans, §6.4 pairs
and lists, §6.5 symbols, §6.6 characters, §6.7 strings, §6.8 vectors.

Third-party engines (consulted 2026-11-23):

- `steel-core` 0.8.3, https://github.com/mattwparas/steel, MIT OR
  Apache-2.0; `crates/steel-core/Cargo.toml` (`cfg(target_family = "wasm")`
  dependencies), README (R5RS, bytecode VM).
- `scheme-rs` 0.2.0, https://github.com/maplant/scheme-rs, MPL-2.0;
  `Cargo.toml` (cranelift non-optional, `load-libraries-from-fs` default),
  README (R6RS, JIT, async/tokio).
- crates.io metadata for both (versions, licences, line counts).

Local measurement artefacts (outside the repository, not versioned):

- `/tmp/meas-vf/{ra.jsonl,resume.md}` — 1 724 files, 1 676 exact.
- `/tmp/ly95lect-refs-clean` — reference tree (LilyPond MIDI).
- `/tmp/ly-import-lots/manifestes/{ra,mus,mut}.txt`.
- `scripts/oracle/lilypond/{mesure.py,ly_notes.py,subdivise.py}`.

## 6. Reproducing the corpus figures

Single process, no parallelism (the machine is shared). Comments and strings
are stripped before counting; each form is counted per occurrence and per
file. The comment/string normaliser is reproduced below.

```python
import re
def strip(t):
    out, i, n = [], 0, len(t)
    while i < n:
        c = t[i]
        if c == '%' and i + 1 < n and t[i+1] == '{':
            j = t.find('%}', i + 2); i = n if j < 0 else j + 2
        elif c == '%':
            j = t.find('\n', i); i = n if j < 0 else j
        elif c == '"':
            j = i + 1
            while j < n:
                if t[j] == '\\': j += 2; continue
                if t[j] == '"': j += 1; break
                j += 1
            out.append(t[i:j]); i = j
        else:
            out.append(c); i += 1
    return ''.join(out)
```

Forms counted: `#\(`, `#'`, `#'\(`, quasiquote (`` #` ``), `#@\(`, `$@\(`,
`#\{`, `##?[tf]`, `#[-+]?\d`, `#\((define|define-public)`,
`define-music-function`, `define-scheme-function`, `define-void-function`,
`\$[A-Za-z_]`, `\bly:[a-z][a-z0-9-]*\b`.

The failing-file list is regenerated with:

```sh
# build once, then measure with at most 2 workers (never two runs at once)
nix develop -c env CARGO_BUILD_JOBS=2 nice -n 10 \
  cargo build --release -p scoretk-cli
python3 scripts/oracle/lilypond/mesure.py \
  --lot ra=/tmp/ly-import-lots/manifestes/ra.txt \
  --refs /tmp/ly95lect-refs-clean --refs-only \
  --scoretk target/release/scoretk --out /tmp/ly-scheme-study
```

`mesure.py` takes `--lot NAME=MANIFEST` (one per lot) and `--refs` (root of
the `ly.midi` tree); `--refs-only` never regenerates a missing reference, so
the denominator is stable. The run summary is `resume.md` in `--out`.

No production test campaign was run for this study; the existing campaign
logs above are quoted as-is, with their own oracle and manifest.
