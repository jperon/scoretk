# Engraving: implementation plan for constrained devices

Status: **plan, not started** (2026-11-27). This document sets the goal, the
measured baseline, the architecture and the phases with their acceptance
gates. Update it as phases land; record measurements here, not in `AGENTS.md`.

## 1. Goal

Engrave (lay out and draw) scores from the pivot model on very constrained
devices, first the **Kobo Mini**, as fast and as beautifully as possible.

| Target device | Value |
| --- | --- |
| SoC | Freescale i.MX507, ARM Cortex-A8 800 MHz, single core, VFPv3 + NEON |
| RAM | 256 MB, **249 MB usable, no swap**, shared with KOReader |
| Screen | 5″ e-ink, 800 × 600, ~200 ppi, 16 grey levels |
| Host | KOReader (LuaJIT, FFI), plugin `score.koplugin` |

Budgets (gates of phase 7, measured on the device):

| Metric | Budget |
| --- | --- |
| Peak heap, 400 KB MusicXML | **≤ 8 MB** (Verovio: 54 MB) |
| Peak heap, 1.7 MB MusicXML (`Minuit.xml`, 88 pages) | ≤ 24 MB (Verovio: 190 MB, refused today) |
| First page on screen, 400 KB input | ≤ 1 s |
| Page turn onto a cached page (normal reading) | ≤ 50 ms + e-ink refresh |
| Page not yet cached (jump) | ≤ 300 ms |
| Page cache (6 rasters at 4 bits + ±8 display lists) | ≤ 2.5 MB, included in the heap budgets above |
| Native library, stripped ARM | ≤ 1.5 MB (Verovio: 2.5 MB + 6.5 MB data) |
| Font data shipped | ≤ 400 KB |
| wasm size increase if the engraver joins the wasm package | to be decided against the features (see `wasm.md`) |

"Beautiful" is measured, not asserted: § 6.

## 2. Measured baseline

From `~/programmes/musique/lomse-eval/` (`measure.sh`, development machine,
x86-64, MusicXML input, all pages):

| File | Lomse time | Lomse peak RSS | Verovio time | Verovio peak RSS |
| --- | ---: | ---: | ---: | ---: |
| OmniDie | 0.12 s | 16.7 MB | 0.07 s | 15.6 MB |
| La_foule | 0.13 s | 17.5 MB | 0.26 s | 21.5 MB |
| gloria | 0.25 s | 19.6 MB | 0.55 s | 30.2 MB |
| k525GTMvt1 | 0.18 s | 25.5 MB | 1.10 s | 72.9 MB |

From `score.koplugin/verovio.lua` (measured by the plugin's author):
Verovio's peak RSS grows by about **0.1 MB per input KB** (40 KB → 17 MB,
400 KB → 54 MB, 1.7 MB → 190 MB). For that reason the plugin refuses inputs
above 256 KB and caps the child at 96 MB of address space. It goes through
SVG files on disk, which KOReader then rasterises: two full passes over every
page.

The memory grows with the whole score because both engines lay out the
**entire** document before drawing page 1. That is the main thing to remove.

## 3. Sources and licences

The project rule is to port MuseScore first (`upstream.md`), and to borrow
from other engines only where MuseScore has nothing for constrained devices.

| Source | Licence | Use |
| --- | --- | --- |
| MuseScore 4.7.4 `src/engraving/rendering/score/` (layout: `HorizontalSpacing`, `SystemLayout`, `PageLayout`, `BeamLayout`, `SlurTieLayout`, `ChordLayout`, `Skyline`…) and `src/engraving/style/` | GPL-3.0 | **Primary port**: spacing, line breaking, beams, slurs, collision, default style values. Missing from the local `ref-musescore/` snapshot: fetch it as `upstream.md` describes |
| Verovio (`src/`, C++) | LGPL-3.0-or-later, GPL-3 compatible | Architecture ideas only: MEI-like view, alignment grid (`HorizontalAligner`), SVG output; ported code is declared in `NOTICE` |
| Lomse (`src/`, C++) | MIT (checked: `lomse-eval/lomse/LICENSE`) | Ideas for a lean engine: "engrouters", incremental per-system layout, the AGG-based raster backend, which is also why Lomse is the leaner engine above |
| SMuFL 1.4 specification | W3C CG, free | Glyph names, anchors (`stemUpSE`, `cutOutNE`…), `engravingDefaults` |
| Bravura / Leland fonts | SIL OFL 1.1 | Glyph outlines and metadata; subset; declared in `NOTICE` and `licences-tierces.md` |
| Gould, *Behind Bars*; Ross, *The Art of Music Engraving* | Books | Rules only, cited in comments when MuseScore has none |

## 4. Architecture

New crates, in line with invariant 3 (new code in new files, by family) and
invariant 1 (portable core, no I/O):

```
crates/
  scoretk-engrave         model → layout → display list; no_std + alloc
  scoretk-render-raster   display list → 8-bit / 4-bit grey buffer
  scoretk-render-svg      display list → SVG (debug, web, oracle diffs)
  scoretk-engrave-ffi     C ABI for KOReader (LuaJIT FFI), cdylib/staticlib
  scoretk-smufl           generated glyph table: outlines, metrics, anchors
```

Pipeline, each stage keeping only what the next stage needs:

1. **Atoms** (whole score, one pass, O(n)): for every measure, the
   horizontal minimum width and the spring constants of its segments
   (MuseScore `HorizontalSpacing`), computed from glyph metrics without
   building any drawing object. Kept: one compact record per measure
   (~32 bytes), so a 1 000-measure score costs ~32 KB.
2. **Line breaking** (whole score, on atoms only): MuseScore's breaking as the
   default for parity; a Knuth–Plass optimal breaking with a bounded window
   as an option (the "beautiful" mode). Output: system boundaries, then page
   boundaries from a vertical height estimate per system.
3. **Page layout** (**only the requested page**): stems, beams, accidentals,
   dots, ties, slurs, tuplets, texts and lyrics; vertical collision with
   skylines (MuseScore `Skyline`); system and staff distances.
4. **Display list** of that page: glyph id + position, line, filled path,
   allocated in a per-page arena. Memory does not grow with the score: it is
   bounded by the page cache below.
4b. **Page cache window** (for instant page turns): keep the current page
   plus *B* pages before and *A* pages after (defaults B = 2, A = 3,
   tuneable from the RAM left to KOReader). Two levels:
   - rasters (ready to blit: 800 × 600 at 4 bits = 240 KB, at 8 bits =
     480 KB per page);
   - display lists (~20–60 KB per page, to be measured), kept for a wider
     window (e.g. ±8) so a page outside the raster window only needs
     rasterising.
   Eviction: farthest from the current page first; on a jump (table of
   contents, go to page) the window is rebuilt around the target.
   **Prefetch** after each turn, in the reading direction first: the core
   stays single-threaded and clock-free (invariant 1), so it exposes an
   incremental call (`engrave_step(handle, budget)` that does a bounded
   amount of work — a few measures — and returns); the host (KOReader's
   `UIManager` idle scheduling) drives it while the user reads and stops at
   the first input, so prefetch never delays a page turn. A turn onto a
   cached page is a blit only (target ≤ 50 ms, dominated by the e-ink
   refresh).
5. **Backends**: the raster backend draws directly into the caller's buffer
   (KOReader `BlitBuffer8`), with no SVG or PNG in between; the SVG backend
   serves the web, debugging and oracle comparisons.

Optimisation choices, each to be checked by a measurement in phase 0 or 7:

- **Fixed-point geometry** (`i32`, 1/1024 staff space) in layout and
  rasterising: on the Cortex-A8 the VFP is not pipelined, so floats are slow,
  and fixed-point also makes the output reproducible across targets.
- **No whole-score object graph**: layout reads the pivot model directly;
  per-page structures live in the page arena.
- **Glyph cache**: each glyph is rasterised once per staff size into a
  coverage mask; the cache is bounded (LRU, budget ~512 KB). Staff lines,
  stems, ledger lines and barlines are drawn as rectangles snapped to the
  pixel grid, not as paths.
- **Pixel-grid hinting for e-ink**: staff line spacing rounded to whole
  pixels per staff (at 200 ppi a 7 mm staff is ~13.8 px: choose 14 and
  adjust the staff size), line thickness rounded to 1 or 2 px. This counts
  for most of the perceived quality at low resolution. Then gamma and
  quantisation to 16 levels (ordered dithering optional).
- **Font subset**: only the SMuFL glyphs the engraver emits (~300), stored
  as compact quadratic outlines plus SMuFL metadata, generated at build time
  into `scoretk-smufl` (like `instruments.rs`, regenerated, never
  hand-edited).
- **Lazy, cancellable rendering with a page window**: atoms and breaks are
  cached in the handle; pages are engraved on demand and prefetched
  incrementally around the current page (stage 4b), so a normal page turn
  is a blit.
- **No threads, no clock** in the core (invariant 1); the FFI may add a
  deadline check between measures.

## 5. Phases and gates

Each phase is one or more delegated tasks, in a dedicated worktree, merged
only when its gate passes. Measurements go into this document.

| Phase | Content | Gate |
| --- | --- | --- |
| **P0 Survey and spike** | Fetch the MuseScore rendering sources; map their classes to the stages above (table in this doc); read Lomse's and Verovio's layout loops; benchmark rasterisers (own scanline vs `tiny-skia`, BSD-3) on ARM: qemu-arm with `prlimit` 96 MB, then on the Kobo; set up the device benchmark (SSH, `/usr/bin/time` or our own counter) | Written choice of rasteriser and font, with numbers |
| **P1 Skeleton** | One staff: clefs, key and time signatures, notes, rests, barlines, ledger lines; fixed spacing; SVG backend; `scoretk` facade `Format::Svg` | Corpus files with one staff render; oracle harness of § 6 running |
| **P2 Spacing and breaking** | Port MuseScore horizontal spacing and system/page breaking; Knuth–Plass option | Measure widths and system breaks equal to MuseScore 4.7.4 on ≥ 90 % of measures of the corpus |
| **P3 Chords and voices** | Stems, beams (`BeamLayout`), accidental stacking, dots, chords, seconds, multiple voices | Element positions within 0.1 staff space of the oracle on ≥ 90 % of noteheads/stems |
| **P4 Curves and text** | Ties, slurs, tuplets, articulations, dynamics, hairpins, texts, lyrics; skylines | Same tolerance, plus 0 overlaps reported by the collision checker |
| **P5 Systems and pages** | Multiple staves and parts, brackets and braces, instrument names, system and page distances, titles | Page count and page breaks equal to the oracle on ≥ 90 % of files |
| **P6 Raster and KOReader** | Raster backend into `BlitBuffer8`, `scoretk-engrave-ffi`, Lua glue in `score.koplugin` (Verovio kept as a fallback); inputs: every format scoretk reads (`.mscz`, MusicXML, MEI, ABC, LilyPond, MIDI, GABC) | Plugin opens the corpus on the Kobo |
| **P7 Performance** | Profile on the device; meet the budgets of § 1 | All budgets of § 1 met on the device |
| **P8 Extensions** | Gregorian square notation from the GABC L6 payload (Gregorio as oracle, see `gabc.md`), NABC, tablature, percussion | One gate per extension |

Order rationale: P1–P5 are format work on the host and give the SVG output
early (useful for cantus too); P6 brings the device in as soon as the layout
is credible; P7 comes after P6 because only the device tells where time goes.

## 6. Measuring beauty and fidelity

- **Geometric oracle**: MuseScore 4.7.4 exports SVG with one element per
  object; a script extracts noteheads, stems, beams, accidentals, slurs and
  system boxes with their positions, and compares them to ours in staff
  spaces. Main figure: share of elements within tolerance, failures
  included. Same corpus and lots as the other oracles
  (`plan-fiabilite.md`).
- **Visual check**: SSIM of our raster against `mscore -o page.png` at
  the device resolution, as a second signal; never the only one.
- **Collision checker**: built into the engraver's tests; any overlap of
  bounding boxes that the oracle does not have is a defect.
- **Device review**: screenshots of the Kobo rendering are taken from the
  device's framebuffer dump only, never from the user's session.

Performance figures: peak heap (counting allocator in tests, `heaptrack` on
the host, `/proc/self/status` on the device), time to first page, time per
page, library size; for each corpus lot, worst case and median.

## 7. Risks

| Risk | Mitigation |
| --- | --- |
| MuseScore layout code is large and interdependent (style, `EngravingItem` graph) | Port by stage, behind our own small structures; record each deviation |
| Fixed-point rounding drifts from MuseScore's `double` positions | Tolerance in staff spaces in the oracle; compare float and fixed builds in P2 |
| Page-local layout cannot see cross-page spanners (slurs, ties, hairpins over a page break) | Keep a small "open spanners" record per page boundary in the atoms |
| Glyph rasterising quality at 14 px staff spacing | Spike in P0; hinting of stems/lines; optional pre-rasterised masks per staff size |
| Licence of fonts and borrowed code | `NOTICE`, `licences-tierces.md`, `scripts/check-licences.py` at each phase |
| wasm growth if the engraver is added to the cantus package | Separate wasm feature; size reported at each phase |

## 8. Open decisions

1. **Fidelity vs own style**: copy MuseScore's layout exactly (measurable, the
   project's usual rule), or MuseScore by default plus a "beautiful" mode
   (Knuth–Plass, e-ink hinting) measured separately? Proposed: both, with
   MuseScore as the default.
2. **Font**: Bravura (MuseScore's default for Leland's companion, largest
   coverage), Leland (MuseScore 4's default), or Leipzig (Verovio's, small)?
   Proposed: Leland, for parity with MuseScore 4.7.4.
3. **Targets beyond the Kobo**: also the web (SVG through the wasm package
   for cantus), Android and iOS? This changes the priority of the SVG backend
   and the wasm budget.
4. **First-phase notation scope**: the corpus' common notation only, or
   Gregorian square notation early because of GABC?

## 9. Phase P0: survey and spike (2026-11-27)

Decisions now closed: MuseScore 4.7.4 layout is the default (a "beautiful" mode
is an option measured separately), the font is Leland, the first target is the
Kobo with an SVG backend alongside it (web/wasm later), and Gregorian square
notation is deferred to P8.

Citations below use `$R` = `~/tmp/scoretk/ref-musescore-474/src/engraving/`,
a read-only, unversioned extraction of tag `v4.7.4`
(`7688c005ad963ba09ee715aae53dbc7e8c9d5ef8`, verified with `git rev-parse` and
`git describe --tags --exact-match` in the already-checked-out
`~/programmes/musique/musescore/ref-musescore/.474` clone). The files were
extracted with `git archive v4.7.4 … | tar -xf -`, i.e. data only; nothing was
compiled or executed from the extraction. It holds
`rendering/score/` (105 files), `style/` and, as a transitive dependency of
`SystemLayout`, `infrastructure/skyline.{h,cpp}`: 56 396 lines in total. The
5.0 snapshot `ref-musescore/` has no `src/engraving/rendering/` directory.
The eng7 follow-up measurements (stems, bar lines, voices, lyrics) are in §13.

### 9.1 MuseScore layout classes → our stages

Line counts are `.cpp + .h` of the 4.7.4 files. "Dependency to cut" names the
`EngravingItem`-graph edge that a page-local port must replace by plain data
(model reads, atoms, skyline intervals); it is not a proposal to port the whole
class.

| MuseScore class / file | Lines | Stage | Key dependencies to cut |
| --- | ---: | --- | --- |
| `HorizontalSpacing` — `rendering/score/{horizontalspacing.cpp,.h}` | 2081 | **atoms** (min width + spring constants) | `HorizontalSpacingContext` holds `System*`, `SegmentPosition` holds `Segment*` (`horizontalspacing.h:62-93`); spacing walks `system->measures()` and `Segment`s (`horizontalspacing.cpp:47-89`, `:262-327`); per-item kerning/padding reads `EngravingItem`/`Note`/`Lyrics` pairs (`:1438-1466`, `:1479-1604`), cross-staff beams read `Segment*` pairs (`:836-1011`). Replace with a per-segment width record and margins derived from glyph metrics |
| `SystemLayout` — `rendering/score/{systemlayout.cpp,.h}` | 3516 | **breaking** (system boundaries; vertical distances) then page layout | `collectSystem` mutates `Measure`/`System` via `LayoutContext` (`systemlayout.cpp:95-458`); `getNextSystem` reuses DOM `System*` (`:516-540`); `collectElementsToLayout` walks `measure->el()`, segments and tracks (`:1371-1533`), `collectSpannersToLayout` follows `Spanner`s (`:1534-1608`); `minDistance` reads `VBox*`/`SysStaff` skylines (`:2900-2991`). Replace with atom sequences + an "open spanners" record |
| `PageLayout` — `rendering/score/{pagelayout.cpp,.h}` | 974 | **breaking** (page boundaries) / **page layout** | `getNextPage` owns `dom.pages()` and `Page*`/`System*` (`pagelayout.cpp:72-118`); `collectPage` consumes whole `page->systems()` (`:119-417`); `layoutCrossStaffElements` reaches across staves (`:418-468`); `layoutPage` positions systems (`:524-613`). Replace with a page geometry computed from system heights only |
| `MeasureLayout` — `rendering/score/{measurelayout.cpp,.h}` | 3274 | **atoms** → **page layout** | `getNextMeasure`/`layoutMeasure` mutate `Measure*` and its segments (`measurelayout.cpp:1175-1188`, `:951-1129`); courtesy clefs/keys/timesigs and headers/trailers add or remove DOM segments (`:1811-2812`); multi-measure rests clone annotations (`:194-524`, `:525-839`). Replace with flags on the atom record; no DOM insertion |
| `ChordLayout` — `rendering/score/{chordlayout.cpp,.h}` | 3677 | **page layout** | `layout(Chord*)` and `layoutPitched` read `Chord`/`Note`/`Segment`/`Staff` (`chordlayout.cpp:82-100`, `:101-348`); `layoutSpanners` (`:757-780`), `layoutStem` (`:1234-1275`), chord offsets across `Segment`s (`:1754-1886`), `fillShape` stores shapes on `Chord::LayoutData` (`:3283-3298`, `:3352-3469`). Replace with note-list + staff geometry and plain interval shapes |
| `BeamLayout` — `rendering/score/{beamlayout.cpp,.h}` | 1533 | **page layout** | `layout(Beam*)` (`beamlayout.cpp:59-125`) and `createBeams` (`:624-809`) build `Beam*` from `ChordRest*`; cross-measure beams break over `Measure*` (`:475-554`); `layoutNonCrossBeams` reads `ChordRest*` stems (`:810-845`). Replace with beam groups from model durations/dots/tuplets and atom ticks |
| `SlurTieLayout` — `rendering/score/{slurtielayout.cpp,.h}` | 3364 | **page layout** (+ open-spanner atoms at page breaks) | `layoutSystem(Slur*, System*)` (`slurtielayout.cpp:58-327`), `slurPos` (`:356-829`), `layoutTieFor`/`layoutTieBack` (`:1527-1660`) read `Spanner`, `ChordRest*`, `Note*`, `Segment*`; collision shapes are built from `ChordRest*` (`:1124-1321`), `fillShape` writes on `SlurTieSegment` (`:3206-3231`). Replace with a boundary record (start/end tick, staff, anchor) and local bezier geometry |
| `Skyline` — `infrastructure/{skyline.cpp,.h}` | 459 | **page layout** (vertical collision) | `SkylineLine::add(const RectF&, EngravingItem*)` and `Skyline::add` keep an item pointer (`skyline.h:50-52`, `:102-128`); `SystemLayout::createSkylines` reads `e->shape()` on every item (`systemlayout.cpp:1609-1725`), `updateSkylineForElement` compares item identities (`:3003-3018`). Replace by x-intervals + y-extents with no item pointer |
| `StemLayout`, `AccidentalsLayout`, `LyricsLayout`, `TupletLayout` — `rendering/score/{stemlayout,accidentalslayout,lyricslayout,tupletlayout}.*` | 609 + 1341 + 957 + 848 | **page layout** | Each takes typed `EngravingItem*`s: `StemLayout::calcDefaultStemLength(Chord*)` (`stemlayout.cpp:42-161`) is reached from `ChordLayout::layoutStem` (`chordlayout.cpp:1234-1275`); `AccidentalsLayout::layoutAccidentals(const std::vector<Chord*>& …)` (`accidentalslayout.cpp:75-109`) operates on `Accidental*`s; lyrics placement consults `SkylineLine` (`lyricslayout.h:33`, `systemlayout.cpp:918-951`); tuplets read `ChordRest*` (`systemlayout.cpp:1745-1760`). Replace with local stem/accidental/lyric/tuplet records |
| `ScorePageViewLayout`, `ScoreLayout`, `ScoreRenderer` — `rendering/score/{scorepageviewlayout,scorelayout,scorerenderer}.*` | 330 + 139 + 205 | orchestration of **breaking / page layout** (to be replaced, not ported) | `ScoreLayout::layoutRange` creates a `LayoutContext(Score*)` and dispatches by view mode (`scorelayout.cpp:47-99`); `prepareScore` deletes all `System*`/`Page*` (`:133-166` of `scorepageviewlayout.cpp`), `doLayout` loops `PageLayout::getNextPage/collectPage` (`:199-229`); `ScoreRenderer::layoutScore` delegates to it (`scorerenderer.cpp:49-52`). Our handle never owns the whole DOM layout |
| `TLayout::layoutItem` / `TDraw::drawItem` / `Paint::paintScore` — `rendering/score/{tlayout.cpp,.h,tdraw.cpp,.h,paint.cpp,.h}` | 7182 + 3656 + 295 | **display list** | `TLayout::layoutItem` dispatches on every `EngravingItem` type (`tlayout.cpp:191`); `TDraw::drawItem` dispatches on typed pointers into `muse::draw::Painter` (`tdraw.cpp:177`, `tdraw.h:191-330`); `Paint::paintScore` traverses all `score->pages()` (`paint.cpp:38-70`). Replace by one display-list item type (glyph id, line, filled path) and the two backends |
| `MStyle`, `StyleDef`, `DefaultStyle`, `PageStyle` — `style/{style,styledef,defaultstyle,pagestyle}.*` | 909 + 4415 + 216 + 305 | all stages (constants) | `MStyle::styleV/styleD/styleMM` (`style.h:40-65`) and the `StyleDef::styleValues` table (`styledef.cpp:40-57`: A4 210×297 mm, printable 180 mm, 15 mm margins, 6.5 sp staff distance, 8.5 sp minimum system distance) give the values to freeze; `DefaultStyle::init` loads `.mss` files from disk (`defaultstyle.cpp:37-41`, `:57-86`) and `pagestyle.cpp:26-41` lists the page keys. A generated constant table replaces `MStyle` and all file I/O |

The fetched `rendering/score/` tree also contains the per-element layout
families that P3–P5 will consult family by family (e.g.
`headerfooterlayout.cpp` 527, `boxlayout.cpp` 362, `arpeggiolayout.cpp` 375,
`tupletlayout.cpp` 792, `dynamicslayout.cpp` 239, `textlayout.cpp` 414,
`autoplace.cpp` 461, `verticalgapdata.cpp` 257), plus the debug/dump helpers
(`debugpaint.cpp` 321, `paintdebugger.cpp` 240, `dumplayoutdata.cpp` 45) which
are not part of the port. The full 4.7.4 `tlayout.cpp` (6 781 lines) is the
measure of the work to avoid: our stage 4 emits a small display list instead of
an `EngravingItem`-typed draw dispatch.

### 9.2 Lomse and Verovio: layout loops and output backends

Sources read only (nothing built or run): `$L` =
`~/programmes/musique/lomse-eval/lomse/` (MIT, `LICENSE:1-21`) and `$V` =
`~/programmes/liseuse/koreader/plugins/score.koplugin/vendor/verovio/`
(LGPL-3.0-or-later, `COPYING.LESSER:1-6`).

| Idea worth taking | Source, with lines | How it maps to our stages | What not to copy |
| --- | --- | --- | --- |
| **Incremental page layout**: per-page entry point that keeps its cursor and resumes; when the page is full it returns "not finished" and the parent allocates a new page | `$L/src/graphic_model/layouters/lomse_score_layouter.cpp:181-268` (the comment at `:183-187` says `layout_in_box()` is invoked once per page, must not reinitialise and must continue from the current state; `:252-256` returns `k_layout_not_finished`) | Stage 3/4b: the `engrave_step(handle, budget)` call engraves one page from cached atoms; the host drives it between page turns | `$L/src/graphic_model/layouters/lomse_document_layouter.cpp:58-77` retries up to 30 times and `:80-98` deletes the whole `GraphicModel` on each failed trial. We never restart from the score |
| **Two-phase "measure then create" atoms** (Lomse's *engrouters*) | `$L/include/lomse_engrouters.h:70-120` (`Engrouter::measure()` and `create_gm_object()`); `$L/src/graphic_model/engravers/lomse_engrouters.cpp:57-126` measures an engrouter, compares its width with the available space, and saves a pending one for the next line | Stage 1: compute width/spring per atom first; stage 4 emits geometry only after the break is known | Each engrouter keeps `ImoContentObj*`, `ImoStyle*` and a six-field `LineReferences` (`lomse_engrouters.h:72-81`); our atom stores plain numbers, no model pointers |
| **Line breaks once, then per-system engraving** | `$L/src/graphic_model/layouters/lomse_score_layouter.cpp:157-179` splits the content into columns and runs the spacing algorithm; `:504-527` decides line breaks only on the first page; `:708-720` still exposes all columns; `$L/src/graphic_model/layouters/lomse_system_layouter.cpp:156-187` fills a system with columns, justifies and resolves collisions | Stages 1–2: whole-score atoms and breaks, then each system is engraved independently | `ScoreLayouter`/`SystemLayouter` keep all columns, boxes and layouters alive (`lomse_score_layouter.cpp:150-154`, `:730-760`; `lomse_system_layouter.cpp:238-260`); keep only compact atoms across pages |
| **Vertical profile / skyline collisions** | `$L/src/graphic_model/layouters/lomse_system_layouter.cpp:213-237` builds the profile, `:695-767` moves staves apart, `:904-926` sets up the auxiliary-shape aligner | Stage 3: vertical collision from skyline intervals, no object graph | In Lomse the profile shapes are `GmoShape`s tied to `Imo` objects; ours are intervals in a page arena |
| **AGG-style integer-coverage scanline raster into a caller buffer** | `$L/include/lomse_renderer.h:168-182` attaches `renderer_base`/`renderer_scanline_aa_solid` to a user buffer, `:229-247` runs `agg::rasterizer_scanline_aa<>` + `agg::scanline_p8` with gamma; `$L/src/agg/include/agg_rasterizer_scanline_aa.h:203-241` sweeps cells one scanline at a time | Stage 5: one scanline buffer reused across glyphs, writing directly into `BlitBuffer8`, no intermediate full-page image | `$L/src/agg/include/agg_rasterizer_cells_aa.h:467-490` grows cell blocks per rasteriser and `:648-655` allocates sorted cells and sorted Y arrays; do not rasterise a whole page as one path — cache bounded per-glyph masks (§ 4) |
| **One SVG element per object, with glyph definitions emitted once** | `$L/src/render/lomse_svg_drawer.cpp:51-63`, `:135-157`, `:190-208` (paths, quadratic/cubic segments, fill/stroke), `$L/src/mvc/lomse_graphic_view.cpp:775-786` renders one page; `$V/src/svgdevicecontext.cpp:103-128` deduplicates SMuFL glyph refs, `$V/src/toolkit.cpp:1731-1783` builds the SVG for one page | SVG backend for debugging/oracle diffs and later web; geometry oracle of § 6 needs element-level output | `$V/src/toolkit.cpp:1670-1722` + `$V/src/view_page.cpp:66-99` walk the whole page DOM again for each render; the KOReader plugin writes every page to SVG files on disk and rasterises them in a second pass (`~/programmes/liseuse/koreader/plugins/score.koplugin/verovio.lua:376`, `:413-424`). Our SVG backend emits from the display list; the raster backend never goes through SVG |
| **One alignment position per tick, shared by all elements** (*HorizontalAligner*, *VerticalAligner*) | `$V/include/vrv/horizontalaligner.h:77-105` (`Alignment` = time + `xRel`) and `:298-335` (`AlignmentReference` per element); `$V/src/horizontalaligner.cpp:154-177` creates an `Alignment` per distinct time/type; `$V/include/vrv/verticalaligner.h:33-75`, `:180` (`SystemAligner`/`StaffAlignment`) | Atoms keyed by segment/tick; one x per segment, one y per staff; no per-element alignment objects | Verovio attaches `AlignmentReference`s to elements and caches layout coordinates on every `LayerElement`, `Arpeg` and `Measure` (`$V/src/cachehorizontallayoutfunctor.cpp:27-48`; `include/vrv/horizontalaligner.h:298-335`): memory proportional to the score, unlike our page arena |
| **Horizontal layout, cast-off, then per-page vertical layout** | `$V/src/doc.cpp:1046-1110` lays out the whole score on one page then casts off into systems/pages; `$V/src/page.cpp:221-245`, `:396-420`, `:499-506`, `:509-530` lays out and justifies a page, with a cached horizontal pass | Stages 1–2 spread the one-off horizontal work, stage 3 stays page-local | `Doc::UnCastOffDoc` (`$V/src/doc.cpp:1155-1185`) rebuilds all pages when the cache is reset; `MeasureAligner::Reset` (`$V/src/horizontalaligner.cpp:132-146`) allocates four `Alignment`s per measure plus one per time/type. Our atoms keep one compact record per measure instead |

Net conclusion for P0: take the *resumable page loop*, the *measure-first atom*
pattern, a *bounded scanline rasteriser* and a *display-list-driven SVG output*;
reject whole-document retries, per-element alignment/cache objects, per-page DOM
traversals and SVG files on disk. These are architectural choices, not ports of
Lomse or Verovio code; no source from either engine is distributed.

### 9.3 Rasteriser spike: in-house scanlines vs `tiny-skia`

New crate `crates/scoretk-render-raster` (GPL-3.0-only, `#![no_std]` + `alloc`,
no dependency): `Point`/`Path` in fixed point 1/256 px, `Gray8` caller-owned 8-bit
buffer, `fill_rect` for pixel-snapped lines and `fill_path` for quadratic outlines.
`fill_path` computes exact x-crossings of lines and quadratics (integer quadratic
solver, no `libm`) on four subrows per pixel and accumulates horizontal coverage
in 1/256 px units. `tiny-skia` 0.11.4 is a **dev-dependency of `benches/spike.rs`
only** (declared in `docs/licences-tierces.md`), never linked into a delivered
crate.

The scene is one dense synthetic page, 800 × 600: 5 systems × 2 staves,
**600 quadratic noteheads** (four quadratic edges each) from one hardcoded
outline, plus 660 pixel-snapped rectangles (staff lines, stems, barlines). Both
rasterisers draw the same scene, and both allocate their output buffer inside
the measured region (480 152 B grey for ours, 1 982 895 B RGBA for tiny-skia).
The benchmark uses a counting global allocator and reports the median of N
iterations. Commands: `cargo bench -p scoretk-render-raster --bench spike -- 20`
(host) and the cross-built binary run through `qemu-arm`.

| Environment | Scanline, median per page | tiny-skia, median per page | Peak heap (ours / tiny-skia) |
| --- | --- | --- | --- |
| Host x86-64, 3 batches of 20 iterations | 1.8–3.5 ms | 13.2–27.5 ms (≈ 7.9× slower in each batch) | 480 152 B / 1 982 895 B |
| `qemu-arm`, ARMv7, 4 batches of 10 iterations | 62–133 ms | 153–276 ms (1.3–4.1× slower per batch) | same heap figures |

Cross-building **was possible**: the dev shell's Rust 1.100.0 has no ARMv7
`std`, so `rust-overlay` was used outside the repository to produce a matching
1.100.0 toolchain with the `armv7-unknown-linux-gnueabihf` target; linking used
the Nixpkgs `armv7l-unknown-linux-gnueabihf-gcc` wrapper, and execution used
`qemu-arm -L <glibc-armv7l-store-path>`. Nothing was added to `flake.nix` and
no cross toolchain is needed to build the workspace.

**Choice: keep the in-house scanline rasteriser**, and keep `tiny-skia` only in
the benchmark as a reference. Reasons: ≈ 8× faster on the host, faster in every
ARM batch, 4× less peak heap, no third-party dependency, direct write into the
caller's grey buffer, fixed-point output reproducible across targets. Its cost
is that it is not yet a complete engraving rasteriser: no cubic segments, no
font outlines, no stroke joins, no glyph cache and no gamma/dithering to 16
grey levels yet; P1–P6 will add what the display list actually needs.

What is **not** proved by this spike: visual parity with MuseScore 4.7.4 (no
SSIM or oracle comparison yet), behaviour on a real display list with text and
beams, quality at a 14 px staff, and real device time (QEMU timings above vary
by up to 2× between batches and are not a Kobo measurement). The `prlimit`/SSH
device benchmark of the P0 gate remains to be done on the Kobo.

### 9.4 Leland subset feasibility (no crate generated yet)

Data inspected locally, nothing downloaded and no script from a font package
executed. `$FL` = `~/programmes/liseuse/koreader/plugins/score.koplugin/vendor/verovio/fonts/Leland/`
(and `$VD` = the sibling `data/`): `Leland.otf` 89 132 B (CFF, cubic),
`Leland.svg` 280 307 B (`FontForge`/copyright header at `:5-7`, 488 glyphs with
path data, first glyph at `:28`), `leland_metadata.json` 150 602 B (30
`engravingDefaults` at `:2`, `fontVersion` 0.80 at `:38`, 489 bounding boxes
from `:39`, anchors from `:5225`), and `$VD/Leland.xml` 45 013 B / 792 lines
(bounding boxes and anchors in staff spaces). Licence: all Verovio fonts are
SIL OFL (`fonts/README.md:13` of the Verovio source tree); the SVG itself names
MuseScore BVBA and Klaus Rettinghaus (`Leland.svg:5-7`). The OFL text is not
present next to these files, so it will have to be shipped with the generated
crate, and `NOTICE`/`licences-tierces.md` updated at that time.

Measured subset: the 293 glyph names in the SVG that start with a notation
family the engraver will emit (notehead, clef, accidental, rest, flag, beam,
dot, dynamic, artic, slur, tie, fermata, trill, turn, breath, caesura, timeSig,
barline, bracket, brace, pedal, hairpin, ornament, fingering, harmonic, mordent,
arpeggio, glissando, repeat), all 293 present in the OTF. The SVG path text for
those glyphs contains 9 608 cubic `c` plus 874 smooth-cubic `s` commands
(≈ 21.7 cubic segments/glyph) and 4 394 line commands (≈ 9.7/glyph), averaging
460 B of path text per glyph. Converting the OTF with fontTools 4.61.1
`Cu2QuPen` (measurement only; fontTools is not a project dependency) gives:

| Cubic→quadratic error (font units, upem 1000) | Quadratic segments | Line segments | Outline bytes at 8 B/quad + 4 B/line |
| --- | ---: | ---: | ---: |
| 0.5 | 14 539 | 2 589 | 126 668 B |
| **1.0 (proposed)** | **11 614** | **2 589** | **103 268 B** |
| 2.0 | 10 020 | 2 589 | 90 516 B |

The proposed binary layout per glyph is a u16 glyph id/codepoint, i16 advance
width, four i16 bbox values and a contour/segment count (≈ 17 B/glyph), then
quadratics as four i16 coordinates (control + endpoint) and lines as two i16
coordinates. Metadata from the JSON subset: 293 bboxes, 283 anchors over 109
glyphs, 10 distinct SMuFL anchor names (`stemUpSE`, `cutOutNE`, …), 30
`engravingDefaults`.

| Component | Estimate |
| --- | ---: |
| Quadratic + line outlines (error 1.0, above) | 103 268 B |
| Per-glyph headers (293 × 17 B) | 4 981 B |
| Advance, bbox, codepoint (293 × 12 B) | 3 516 B |
| Anchors (283 × 5 B) + 10 names | ≈ 1 535 B |
| 30 engraving defaults, i16 fixed point | ≈ 120 B |
| **Total, raw generated constants** | **≈ 113 KB (≈ 111 KiB)** |
| With ≈ 10 % alignment/padding margin | ≈ 125 KB |

This is well inside the 400 KB font-data budget of § 1; even the stricter
error-0.5 conversion stays near 137 KB. Alternates (30), ligatures (7),
optional glyphs (1) and text fonts are excluded.

### 9.5 P1 font generation (2026-11-27)

**Done (P1).** The font bundled in MuseScore 4.7.4 (`git archive v4.7.4
fonts/leland`, nothing compiled or executed) is byte-identical to the Verovio
0.80 copy for `Leland.otf` (SHA-256 `293788a4…`) and `leland_metadata.json`
(`41a3dcd4…`); the MuseScore originals are vendored in
`crates/scoretk-smufl/fonts/leland/` with the OFL-1.1 text, FAQ and font log.
`scripts/gen-smufl-subset.py` is a **dependency-free** `python3 -I` script: it
parses the CFF/Type2 charstrings, `cmap` and `hmtx` with the Python standard
library and converts cubics to quadratics at a stricter tolerance of 0.5 font
units. Regeneration command (the generated file is never hand-edited):

```sh
python3 -I scripts/gen-smufl-subset.py \
  crates/scoretk-smufl/fonts/leland crates/scoretk-smufl/src/glyphs.rs
```

Measured output at P1: **308 glyphs, 10 468 quadratic and 3 281 line segments
(14 386 edges), 339 anchors over 10 anchor names, 29 numeric
`engravingDefaults`**. In eng5 the CFF interpreter was fixed to include the
first segment of every contour (which the old reader treated as a starting
point), so the regenerated tables contain **11 365 quadratics and 3 309 lines
for the SMuFL subset**; the Edwin subset contains 12 312 edges for 167
codepoints per style. The generated constants occupy about 107 KB of binary
data (the Rust source is ~500 KB of text); the font and metadata vendored as
generation input are 239 KB, with the OFL documents 313 KB in total, still
inside the 400 KB font-data budget. The remaining gates are a perceptual check
of the Cu2Qu tolerance at a 14 px staff and the geometric oracle of § 6.

### 9.6 P1 geometric oracle (2026-11-27)

`scripts/oracle/engrave/` exports the SVG of the 44 **single-staff** corpus
files with MuseScore 4.7.4 (`refs.sh`, `xargs -P 2`) and with our CLI
(`ours.sh`, `xargs -P 2`, `timeout 10 prlimit --as=2000000000`). `compare.py`
reads both SVGs as data, derives the staff scale from each document's own
staff lines, extracts notehead centres, stem extents and bar lines, and
matches them one-to-one (nearest first). Unmatched elements and failures are
counted in the denominator. Baseline results, 44 files, 0 failures, 1 319
elements:

| Metric | Value |
| --- | ---: |
| Within 0.1 sp, both axes | 0.0030 (4 / 1 319) |
| Within 0.1 sp, x only | 0.0106 |
| Within 0.1 sp, y only | 0.2600 |
| Vertical pitch / ledger agreement | 0.5138 (508 matched noteheads) |
| Stem-length agreement | 0.1053 (380 matched stems) |

Horizontal agreement is expected to stay low until P2 (fixed proportional
spacing by construction). The vertical figures are the meaningful P1 signal;
they are limited by clef/transposition handling (only the written pitch and
5-line staves are modelled), by accidental placement and by the remaining
stem-shortening cases. Reproduce with
`scripts/oracle/engrave/mesure.sh <out> <scoretk-binary>`; raw per-file rows
are in `<out>/raw.jsonl` and the summary in `<out>/resume.md`.

### 9.7 Oracle correction and P1 debt (2026-11-27)

`compare.py` originally converted every element to coordinates relative to
the **first** staff system of the page. With several systems per page, the
nearest-y matcher paired a note on system 4 of the reference with a note on
system 1 of ours: the reported pitch figure measured the system distance,
not the pitch. `staff_systems()` now groups staff lines into five-line
systems and each note, stem and bar line is localised to its own system
before matching. Noteheads are compared at their bounding-box centre in both
SVGs (scoretk's SMuFL outline has its origin at the head's left edge).
`worst.py <out> [count] [kind]` dumps the worst element-level differences.

Re-running the **unchanged** P1 SVGs with the corrected oracle gives:

| Metric | Original | Corrected |
| --- | ---: | ---: |
| Within 0.1 sp, both axes | 0.0030 | 0.0083 |
| Within 0.1 sp, x only | 0.0106 | 0.0159 |
| Within 0.1 sp, y only | 0.2600 | 0.4898 |
| Vertical pitch / ledger agreement | 0.5138 | **0.9547** (508 noteheads) |
| Stem-length agreement | 0.1053 | **0.0711** (380 stems) |

So the pitch debt was mostly a measurement artefact; the remaining pitch
differences are 23 noteheads (worst: 4.5 sp in `testMultiInstrumentPart3`,
from clef handling). Stem length is a real defect: `copypaste_parts` has
stems of 6.83/5.83 sp in MuseScore against 3.50 sp in ours, because
`StemLayout::calcDefaultStemLength`'s final "extend to the middle line" step
(`stemlayout.cpp:122-159`) is not ported yet.

Fix applied in step 0: the middle-line extension, the drawn stem anchor
(`tdraw.cpp:2602-2614`, calibrated to 0.235 sp against the 4.7.4 SVG),
multi-voice stem directions by track parity (`chordlayout.cpp:1584-1588`),
all voices of a segment instead of only the first, and an upright-glyph fix
in both backends (SMuFL y is up-positive, the canvas y is down-positive).

| Metric | P1 before | After step 0 |
| --- | ---: | ---: |
| Within 0.1 sp, both axes | 0.0083 | 0.0083 |
| Within 0.1 sp, y only | 0.4898 | 0.5246 |
| Vertical pitch / ledger agreement | 0.9547 | **0.9652** (517 noteheads) |
| Stem-length agreement | 0.0711 | **0.6967** (389 stems) |

The pitch target (≥ 95 %) is met. The stem-length target (≥ 90 %) is **not**:
the remaining deficits are special noteheads (`testNoteheads2`: 9/26),
beams/tuplets (`testTuplets8`: 9/21, `Beam-CrossM1`: 3/9) and multi-voice
overlaps (`voiceswitching`: 24/62), all belonging to P3's beam/voice work.
`worst.py /tmp/oracle-engrave 20 Stem` lists the worst cases.

### 9.8 Header segment (step 1, 2026-11-27)

The first measure's x positions are now derived from SMuFL outline widths and
the 4.7.4 style defaults (`horizontalspacing.cpp:1183-1214`,
`styledef.cpp:174-190`, `:220`, `:241`): clef at `clefLeftMargin` 0.75 sp,
width + `clefKeyRightMargin` 0.8 sp before the key, one accidental width +
`keysigAccidentalDistance` 0.3 sp between key accidentals,
`keyTimesigDistance` 1.0 sp before the time signature, and
`barNoteDistance` 1.25 sp before the first chord. `DisplayItem::bbox` gives
each emitted item's box and a unit test asserts that clef, key, time and
notehead boxes do not intersect.

| Metric | After step 0 | After step 1 |
| --- | ---: | ---: |
| Within 0.1 sp, both axes | 0.0083 | **0.0174** (23/1319) |
| Within 0.1 sp, x only | 0.0159 | **0.0273** |
| Within 0.1 sp, y only | 0.5246 | 0.5269 |
| Vertical pitch / ledger agreement | 0.9652 | 0.9652 |
| Stem-length agreement | 0.6967 | 0.6967 |

### 9.9 Horizontal springs (step 2, 2026-11-27)

Each chord/rest atom now carries the spring of `HorizontalSpacing`: natural
width `3.5 sp * measureSpacing^log2(ticks / quarter)` (`horizontalspacing.cpp:
730-812`), bounded below by notehead + accidental + dot widths plus
`minNoteDistance` 0.35 sp (`:1216-1276`, `styledef.cpp:219`). The slope-1.5
values are precomputed in 1/1024 so the `no_std` crate needs no `libm`;
`MeasureAtom::spring` is the sum of segment stretches (spring constant
`1 / stretch`, `horizontalspacing.cpp:204`).

The oracle's x figures do not improve yet: MuseScore **justifies** a system
by stretching its springs to the printable width, which arrives in step 3.
For `measure-4`, reference quarters are 6.8 sp apart against our unstretched
3.5 sp.

| Metric | After step 1 | After step 2 |
| --- | ---: | ---: |
| Within 0.1 sp, both axes | 0.0174 | 0.0175 (23/1318) |
| Within 0.1 sp, x only | 0.0273 | 0.0273 |
| Within 0.1 sp, y only | 0.5269 | 0.5303 |
| Vertical pitch / ledger agreement | 0.9652 | 0.9571 (513 noteheads) |
| Stem-length agreement | 0.6967 | 0.7013 (385 stems) |

### 9.10 Systems and pages (step 3, 2026-11-27)

`breaking.rs` collects measures into systems from atom widths only
(`systemlayout.cpp:95-225`) and assigns systems to pages using
`minSystemDistance` 8.5 sp and the A4 printable height
(`pagelayout.cpp:140-265`, `systemlayout.cpp:2900-2991`). Systems are
justified with the spring force of `stretchSegmentsToWidth`
(`horizontalspacing.cpp:1087-1111`): chord/rest widths become
`force * stretch` while header segments keep their style widths. The last
system is left unstretched when it fills less than `lastSystemFillLimit`
0.3 and nothing follows (`systemlayout.cpp:459-489`). `Engraver::page(n)`
reads only the systems assigned to page `n`; no `Score` is touched.

For `measure-4`, quarters are now 6.9 sp apart against MuseScore's 6.8 sp
(3.5 sp before justification). The residual offset of the first notehead
(6.3 sp vs 8.6 sp) comes from measures without explicit key/time segments.

| Metric | After step 2 | After step 3 |
| --- | ---: | ---: |
| Within 0.1 sp, both axes | 0.0175 | 0.0061 (8/1312) |
| Within 0.1 sp, x only | 0.0273 | 0.0244 |
| Within 0.1 sp, y only | 0.5303 | 0.5755 |
| Vertical pitch / ledger agreement | 0.9571 | 0.9652 (517 noteheads) |
| Stem-length agreement | 0.7013 | 0.6967 (389 stems) |

### 9.11 P2 gate (step 4, 2026-11-27) — **not met**

`compare.py` now derives measure widths from consecutive bar lines of each
system and compares them elementwise (unmatched measures count in the
denominator); it also reports files whose per-system measure counts match.
A layout bug was fixed on the way: a stretched measure was drawn with
scaled segment widths but the next measure was advanced by its natural
width.

| Metric | Before P2 | After P2 | Gate |
| --- | ---: | ---: | ---: |
| Measure widths within 0.1 sp | — | **0.0178** (5/281) | ≥ 0.90 — **failed** |
| Files with identical system breaks | — | **17/44** | ≥ 0.90 — **failed** |
| Within 0.1 sp, both axes | 0.0083 | 0.0115 (15/1307) | — |
| Within 0.1 sp, y only | 0.4898 | 0.6205 | — |

The remaining causes are the first-note offset in measures without explicit
key/time segments (6.3 sp vs 8.6 sp in `measure-4`), per-measure user
stretch and courtesy headers/trailers that change a system's natural width
before justification (`measurelayout.cpp:1811-2812`). P2 is therefore not
closed: the atoms and breaking plan are in place, but their width parity is
not yet at the acceptance threshold. See §9.12-§9.15 for the subsequent
steps and the final measured state; the gate remains **not met**.

### 9.12 Accidentals before their notehead (step 5, 2026-11-27)

`place_chord` used a fixed `x - 3.5 sp` offset for every accidental. With a
stretched system that offset landed on the *preceding* notehead: in
`copypaste_parts` measure 1 the G was drawn, then the ♯ of the following G♯
appeared 1.6 sp to its right. The offset is now derived from the glyph's own
bbox plus `accidentalDistance` 0.25 sp (`accidentalslayout.cpp:286-305`,
`:745-767`, `styledef.cpp:243`); a bbox unit test asserts that the accidental's
right edge is exactly 0.25 sp to the left of the head's left edge and that it
is emitted first in the display list. Before/after PNGs:
`/tmp/copypaste_parts-before.png`, `/tmp/copypaste_parts-after-accidental.png`,
with `/tmp/copypaste_parts-ref.png` as the oracle.

| Metric | Before | After |
| --- | ---: | ---: |
| Measure widths within 0.1 sp | 0.0178 (5/281) | 0.0178 (5/281) |
| Files with identical system breaks | 17/44 | 17/44 |
| Stem-length agreement | 0.6967 | 0.6967 |
| Vertical pitch/ledger agreement | 0.9652 | 0.9652 |

No horizontal metric changes: the accidental's *x* influences collision
spacing in MuseScore but our atom widths ignore it (P3 work).

### 9.13 Oracle correction for bar lines (step 6, 2026-11-27)

`compare.py` grouped bar lines by their **system-local** y, which is the
same on every system: all systems of a page were merged into one sequence
and consecutive widths could be negative. It also counted the two strokes
of a MuseScore double/final bar line (0.6-1.1 sp apart) as two successive
measure boundaries. Grouping is now by absolute y, double strokes within
1.5 sp merge into one boundary, and a plain start bar line is no longer
drawn by our engraver (the 4.7.4 SVG has none). The published 5/281 and
17/44 P2 figures were measured with the old grouping; on the *unchanged*
SVGs the corrected oracle gives:

| Metric | Old oracle | Corrected oracle |
| --- | ---: | ---: |
| Measure widths within 0.1 sp | 0.0178 (5/281) | 0.0148 (3/203) |
| Files with identical system breaks | 17/44 | 27/44 |
| Stem-length agreement | 0.6967 | 0.6967 |

The 203 vs 281 denominator is the count of real measure boundaries after
merging doubles; failures remain in the denominator. The corrected figures
are the baseline for the remaining P2 steps.

### 9.14 Style-aware spacing and breaking (step 7, 2026-11-27)

The engraver now derives its page geometry from the score's `<Style>`
instead of the 4.x defaults: `pagePrintableWidth`, page margins, `Spatium`,
`barNoteDistance` (0.6 sp in `measure-4`), `systemHeaderTimeSigDistance`
2 sp and `enableIndentationOnFirstSystem` (4.x indents the first system by
5 sp, 3.x does not; `styledef.cpp:43,143,186,225,539-540`). The first note
now sits at its own header x, the bar line at the last segment's right edge
(`horizontalspacing.cpp:1148-1152`), and the generated final double bar line
(reserved as `endBarWidth + endBarDistance`, `styledef.cpp:147,149`) is no
longer counted as a spring. The clef and a non-C key signature are generated
at every system start (`measurelayout.cpp:2530-2660`) and multi-measure rests
fold their source measures with `durationStretchForMMRests`
(`horizontalspacing.cpp:773-803`). A short last system is justified as long
as it contains a measure (`systemlayout.cpp:459-489`).

| Metric | Corrected baseline | After step 7 | Gate |
| --- | ---: | ---: | --- |
| Measure widths within 0.1 sp | 0.0148 (3/203) | **0.3750 (72/192)** | ≥ 0.90 — failed |
| Files with identical system breaks | 27/44 | **34/44** | ≥ 0.90 — failed |
| Stem-length agreement | 0.6967 | 0.6967 | ≥ 0.90 — failed |
| Within 0.1 sp, x only | 0.0207 | 0.2883 | — |

The denominator drops from 203 to 192 because multi-measure rests are now
folded like MuseScore's displayed measures. `userStretch` is not a cause in
this corpus: no `.mscx` contains `<stretch>`; the remaining deficits are
text/harmony-driven segment widths (P4), multi-voice stems (P3) and a few
courtesy-header cases.

### 9.15 Stem anchors per notehead (step 8, 2026-11-27)

Special noteheads no longer all render as `noteheadBlack`: `NoteHeadGroup`
is mapped to the corresponding SMuFL head, and each head's `stemUpSE` /
`stemDownNW` anchor shifts the drawn stem end and `shortestStem`
(`stemlayout.cpp:60-63, 220-228, 345-373`; `tlayout.cpp:5180-5201`). The
shortening cap for traditional flags was widened to two quarter spaces to
match the measured 4.7.4 lengths.

| Metric | Before | After | Gate |
| --- | ---: | ---: | --- |
| Stem-length agreement (389 matched stems) | 0.6967 | **0.7404** | ≥ 0.90 — failed |
| Stem length excluding the three beam-heavy files (352 stems) | — | 0.7727 | reported for context only |
| Measure widths within 0.1 sp | 0.3750 | 0.3750 | ≥ 0.90 — failed |
| Files with identical system breaks | 34/44 | 34/44 | ≥ 0.90 — failed |

Beams and grace notes are P3 work (`Beam-CrossM1`, `testTuplets8`,
`copypaste_tuplet_01`); the second figure above is reported alongside the
first, never in place of it. The remaining non-beam deficits are mostly
0.25–0.5 sp shortening differences in multi-voice and lyric-bearing scores.

### 9.16 Regression from eng2b: short last system (step 0, 2026-11-27)

Step 7 justified every system, including a final system holding one measure.
`SystemLayout::shouldBeJustified` (`systemlayout.cpp:459-489`) actually leaves
a system unstretched when `curSysWidth / targetSystemWidth < lastSystemFillLimit`
(0.3) **and no measure follows**. The eng3 breaking plan now applies that
exact rule using each score's own `lastSystemFillLimit`; a short last system
with a following measure is still justified. Complementary check over the six
corpus files whose reference ends with a single short system: the bar line of
that system is at the oracle x (within 0.1 sp) in 1/6 files after the fix
against 0/6 before (the other five are 0.19–0.94 sp off from unrelated
header offsets, down from 74–93 sp). Before/after/reference PNGs of
`copypaste_parts`: `/tmp/copypaste_parts-p3-before.png`,
`/tmp/copypaste_parts-p3-step0.png`, `/tmp/copypaste_parts-p3-ref.png`.

| Metric | Before step 0 | After step 0 | Gate |
| --- | ---: | ---: | --- |
| Measure widths within 0.1 sp | 0.3750 (72/192) | 0.3594 (69/192) | ≥ 0.90 — failed |
| Files with identical system breaks | 34/44 | 34/44 | ≥ 0.90 — failed |
| Stem-length agreement | 0.7404 (288/389) | 0.7404 (288/389) | ≥ 0.90 — failed |
| Short last-system x within 0.1 sp | 0/6 | 1/6 | complementary |

The headline width figure drops by three measures: they were counted as
matching only because a wrongly justified system pushed a bar line onto a
reference boundary by coincidence. Fidelity of the last system is the
regression being repaired; the width metric remains the primary P2 figure.

### 9.17 Beams, grace notes and stem direction (steps 1–2, 2026-11-27)

Beam groups now come from `BeamMode`, `<Beam>` starts and measure boundaries
(`beamlayout.cpp:546-809`, `:622-809`); the model carries a new
`ChordRestEngraving::beam_start` flag so an empty 4.x `<Beam>` still starts a
group. The slab, slant and stem extension follow `createBeamSegment`
(`beamlayout.cpp:1215-1307`) and `calculateAnchors`
(`beamtremololayout.cpp:591-770`): max slope by interval and beam width,
quarter-space anchors, and stems ending at the beam centre
(`tdraw.cpp:920-936`). Automatically grouped beams (tuplets) still use the
default stems — their slant depends on tuplet grouping not yet ported; the
main figure below includes them, no file is excluded. Grace notes before a
chord are emitted at `graceNoteMag` (`chordlayout.cpp:631-645`) with their
own stems. Finally, `computeAutoStemDirection` (`chordlayout.cpp:1626-1642`)
returns 0 when the notes cancel, so a middle-line note gets a **down** stem;
the old `true` default put those stems on the wrong side of the head.

| Metric | After step 0 | After steps 1–2 | Gate |
| --- | ---: | ---: | --- |
| Stem-length agreement (389 → 395 stems) | 0.7404 (288/389) | **0.8000** (316/395) | ≥ 0.90 — failed |
| Within 0.1 sp, both axes | 0.2887 (364/1261) | 0.2871 (362/1261) | — |
| Vertical pitch/ledger agreement | 0.9613 (517) | 0.9618 (523) | ≥ 0.95 — met |
| Measure widths within 0.1 sp | 0.3594 (69/192) | 0.3594 (69/192) | ≥ 0.90 — failed |
| Files with identical system breaks | 34/44 | 34/44 | ≥ 0.90 — failed |

The 395-stem denominator includes the six grace stems newly drawn (they
were unmatched before); 316 agree, against 288 before. The slight fall of
the both-axes share is those grace noteheads/stems, whose x is still
approximate. Before/after/reference PNGs of `Beam-CrossM1`:
`/tmp/beam-before.png`, `/tmp/beam-after.png`, `/tmp/beam-ref.png`.

### 9.18 Voices, chords and key cancellations (steps 3–4, 2026-11-27)

The remaining P3 items are ported as follows:
- voice-2 rests move one line space down (`restlayout.cpp:694-765`);
- noteheads of colliding voices shift right by a head width + 0.15 sp
  (`chordlayout.cpp:2060-2140`);
- seconds mirror the head on the stem side (`chordlayout.cpp:2309-2410`);
- key changes cancel the previous key's accidentals with naturals before
  drawing the new ones (`measurelayout.cpp:2597-2660`):
  `copypaste_parts` measure 3 now shows the four naturals missing before
  this step (9 accidentals instead of 5 in the page SVG).

An attempt to estimate lyrics/harmony/tempo text widths from character
counts made 17/44 systems break like the oracle instead of 34/44: the
rough estimate inflated atoms enough to move breaks. It was reverted. A
faithful port needs real text glyph metrics (P4), so attached-element
spacing remains debt here. Courtesy clefs/keys at system ends are not
triggered by any single-staff corpus file and are likewise deferred.

| Metric | Before steps 3–4 | After | Gate |
| --- | ---: | ---: | --- |
| Stem-length agreement | 0.8000 (316/395) | 0.8000 (316/395) | ≥ 0.90 — failed |
| Within 0.1 sp, both axes | 0.2871 (362/1261) | 0.2895 (365/1261) | — |
| Vertical pitch/ledger agreement | 0.9618 (523) | 0.9618 (523) | ≥ 0.95 — met |
| Measure widths within 0.1 sp | 0.3594 (69/192) | 0.3594 (69/192) | ≥ 0.90 — failed |
| Files with identical system breaks | 34/44 | 34/44 | ≥ 0.90 — failed |

Before/after/reference PNGs of `copypaste_parts`:
`/tmp/copypaste_parts-p3-before.png`, `/tmp/copypaste_parts-after.png`,
`/tmp/copypaste_parts-ref.png`.

### 9.19 Beam anchors and stem ends (eng3b, step 1, 2026-11-27)

Review of `/tmp/beam-after.png` against `/tmp/beam-ref.png` (`Beam-CrossM1`)
showed the beam slab stopping short of the outer stems by one notehead
width: `plan_beams` used each chord's notehead x, while `place_chord` draws
the stem at the head's right edge (`StemLayout::stemPosX`). The slab now
runs between the stem positions (`beamtremololayout.cpp:1293-1328`), the
height comes from the stem's own anchor line (`chordBeamAnchorY`,
`:1368-1375`), and the slant is flattened when an inner note is more
extreme than both ends (`getSlopeConstraint`, `:1157-1178`). The flat-beam
anchor shift follows `offsetBeamWithAnchorShortening` (`:213-286`),
calibrated against the second group of `Beam-CrossM1` (1.25 sp above the
staff, stems 3.5/2.5/3/4 sp). The slab remains symmetric around its segment
line (`tdraw.cpp:920-936`), so every stem far end is on that line; a unit
test now checks this for a slanted and a flat group.

| Metric | Before | After | Gate |
| --- | ---: | ---: | ---: |
| Stem-length agreement | 0.8000 (316/395) | **0.8076** (319/395) | ≥ 0.90 — failed |
| `Beam-CrossM1` stems | 6/9 | **9/9** | — |
| Measure widths within 0.1 sp | 0.3594 (69/192) | 0.3594 (69/192) | ≥ 0.90 — failed |
| Files with identical system breaks | 34/44 | 34/44 | ≥ 0.90 — failed |
| Vertical pitch/ledger agreement | 0.9618 (523) | 0.9618 (523) | ≥ 0.95 — met |

Before/after/reference PNGs: `/tmp/Beam-CrossM1-p3b-before.png`,
`/tmp/Beam-CrossM1-p3b-after.png`, `/tmp/Beam-CrossM1-p3b-ref.png`. The
oracle uses the SVGs in `/tmp/oracle-p3b-base` (before) and
`/tmp/oracle-p3b-step1b` (after), with the same MuseScore references.

### 9.20 Staff lines of a short last system (eng3b, step 2, 2026-11-27)

The five staff lines were drawn over `printable_width` for every system,
while a short final system is left unstretched (`shouldBeJustified`,
`systemlayout.cpp:459-489`): `Beam-CrossM1`'s last system showed staff lines
running past its final double bar line. `build_page` now replays the system's
segment widths and clips each staff line to the right edge of that system's
last bar line (`tdraw.cpp` bar-line drawing, `styledef.cpp:147,149`). On
justified systems the result is unchanged (the bar line is at the right
margin).

| Metric | Step 1 | Step 2 | Gate |
| --- | ---: | ---: | ---: |
| Stem-length agreement | 0.8076 (319/395) | 0.8076 (319/395) | ≥ 0.90 — failed |
| Measure widths within 0.1 sp | 0.3594 (69/192) | 0.3594 (69/192) | ≥ 0.90 — failed |
| Files with identical system breaks | 34/44 | 34/44 | ≥ 0.90 — failed |

The staff-line width is a vertical-decoration detail measured by visual
comparison, not by the element oracle (which only extracts noteheads, stems
and bar lines): in `Beam-CrossM1` the last staff lines end at the final
thick bar line in both SVGs. `/tmp/Beam-CrossM1-p3b-step2.png` shows the
result; the before image is `/tmp/Beam-CrossM1-p3b-after.png` from §9.19.

### 9.21 Centred measure rests (eng3b, step 3, 2026-11-27)

A `DurationType::Measure` rest was drawn at its segment's left edge, while
`MeasureLayout::layoutMeasure` centres it in the free space between the
previous segment's right edge and the bar line
(`measurelayout.cpp:1477-1485, 1501-1512`): the `AtomKind::Rest` now carries
the distinction from a plain whole rest, and `place_event` centres it using
`barNoteDistance` as the left inset. Multi-measure rests (folded atoms carry
`mm_rest_count`) are drawn as a bar inset by `multiMeasureRestMargin`
(1.25 sp), with 2 sp vertical end strokes, centred vertically on the middle
staff line (`measurelayout.cpp:1486-1492, 2997-3005`; `styledef.cpp:530-533`).
The MMRest number above the bar is still P4 text work. Two unit tests pin the
rest centre and the bar geometry.

| Metric | Step 2 | Step 3 | Gate |
| --- | ---: | ---: | ---: |
| Stem-length agreement | 0.8076 (319/395) | 0.8076 (319/395) | ≥ 0.90 — failed |
| Measure widths within 0.1 sp | 0.3594 (69/192) | 0.3594 (69/192) | ≥ 0.90 — failed |
| Files with identical system breaks | 34/44 | 34/44 | ≥ 0.90 — failed |

Rests are not among the oracle's extracted elements, so the change is
reported from the SVG comparison
(`/tmp/Beam-CrossM1-p3b-step3b.png` vs `/tmp/Beam-CrossM1-p3b-ref.png`): the
full-measure rests of the short last system now sit near the centre of their
measures instead of against the clef. Oracle run: `/tmp/oracle-p3b-step3b`.

### 9.22 Title frames, measure numbers and Edwin text (eng3b, step 4, 2026-11-27)

`score.engraving.credits` was ignored by the engraver: `Beam-CrossM1` had no
title, no subtitle and no measure numbers, and its first system started at
the top margin instead of below the VBox. The engraver now copies the credits
once in `Engraver::new`, draws VBox texts as Edwin glyph outlines (one SVG
path per character), offsets the first system by the VBox height plus
`frameSystemDistance` 7 sp (`boxlayout.cpp`, `systemlayout.cpp:2900-2991`),
and draws the measure number above the first measure of each system except
measure 1 (`styledef.cpp:467-471, 1210`). Title/subtitle are placed in
reading order even when the VBox lists the subtitle first.

The text subset comes from MuseScore 4.7.4's `fonts/edwin/` (OFL 1.1,
Copyright (C) MuseScore Limited) through `scripts/gen-edwin-subset.py`,
which reuses the dependency-free CFF reader of `gen-smufl-subset.py`: 167
codepoints per style, with **real** advance widths and quadratic outlines.
It is declared in `NOTICE` and `docs/licences-tierces.md`; the generated
`crates/scoretk-smufl/src/text.rs` is never hand-edited. Only the generated
constants (≈ 93 KB of raw edges for both styles) ship in binaries.

| Metric | Step 3 | Step 4 | Gate |
| --- | ---: | ---: | ---: |
| Stem-length agreement | 0.8076 (319/395) | 0.8076 (319/395) | ≥ 0.90 — failed |
| Measure widths within 0.1 sp | 0.3594 (69/192) | 0.3594 (69/192) | ≥ 0.90 — failed |
| Files with identical system breaks | 34/44 | 34/44 | ≥ 0.90 — failed |

The first staff line of three 4.7.4 files with a VBox (`dynamics`,
`mmrest`, `testNoteheads2`) is within 0.15 sp of the oracle after scaling
each SVG by its own staff spatium; first-system indentation is likewise
within 0.11 sp (`mmrest`: no indent for a legacy 3.x style; `dynamics`:
5 sp for a 4.x style). `/tmp/Beam-CrossM1-p3b-step4b.png` shows the title,
subtitle, measure numbers and the shifted first system; the same image
before this step is `/tmp/Beam-CrossM1-p3b-step3b.png`. Oracle run:
`/tmp/oracle-p3b-step4b`.

### 9.23 Edwin text widths: measurement before enabling (eng3b, step 5, 2026-11-27)

The Edwin subset of step 4 provides **real** advance widths. The P4
groundwork now computes them for lyrics (all verses),
harmony symbols and tempo text from the pivot model
(`atoms::attached_text_widths`, `horizontalspacing.cpp:1438-1604`), with the
font point sizes of `styledef.cpp`. Unlike the earlier character-count
estimate, no width is guessed. Seven integration variants were measured on
the same 44 files; the oracle directory and its per-file `raw.jsonl` are kept
for each (`/tmp/oracle-p3b-step5{,...,j}`).

| Variant | Breaks | Widths ≤ 0.1 sp | Both axes ≤ 0.1 sp |
| --- | ---: | ---: | ---: |
| Baseline (step 4) | 34/44 | 69/192 | 365/1261 |
| All texts added to segment width | 26/44 | 49/185 | 239/1261 |
| Lyrics + harmony + tempo added | 26/44 | 57/191 | 257/1260 |
| Lyrics added | 31/44 | 63/192 | 331/1261 |
| Overlap rule, 0.25 sp margin | 33/44 | 68/192 | 333/1261 |
| Overlap rule, no margin | 33/44 | 68/192 | 334/1261 |
| Overlap on short notes, event x shifted | 34/44 | 68/192 | 344/1261 |
| Overlap on short notes, break width only | 34/44 | 68/192 | 350/1261 |
| **`TEXT_WIDTH_SPACING = false` (kept)** | **34/44** | **69/192** | **365/1261** |

The best faithful variant kept the break count but still moved two files
away from the oracle: `copypastesymbollist-lyrics` fell from 35 to 26
elements within 0.1 sp and `testExcludeInvisibleElements` from 49 to 43 (one
measure width lost). Although the primary gates do not degrade, the
per-file x figures do, so the switch is left **off** and the measured helper
is ready for the full `computePadding`/`Skyline` port in P4. This is the
same decision as for the character-count estimate of §9.18, now with
measured font advances instead of a guess.

## 10. P3/P2 gate status at the end of eng3b (2026-11-27)

Final oracle on the 44 single-staff corpus files (44 files, 0 failures,
1 261 elements; failures and unmatched elements stay in every denominator),
run `/tmp/oracle-p3b-step5j` after the reviewed beam/staff/rest/frame fixes
and the measured-but-disabled text spacing of §9.23:

| Metric | P1 baseline | After eng3b | Gate | Result |
| --- | ---: | ---: | ---: | --- |
| Stem-length agreement | 0.0711 | **0.8076** (319/395) | ≥ 0.90 | not met |
| Measure widths within 0.1 sp | — | **0.3594** (69/192) | ≥ 0.90 | not met |
| Files with identical system breaks | — | **34/44** (0.773) | ≥ 0.90 | not met |
| Within 0.1 sp, both axes | 0.0083 | 0.2895 (365/1261) | — | reported |
| Vertical pitch/ledger agreement | 0.5138 | 0.9618 (523) | ≥ 0.95 | met |

Complementary figures, reported alongside the primary ones and never in
their place: short last-system bar line at the oracle x 1/6 after step 0
against 0/6 before; stem length on the 352 stems outside the three
beam-heavy files 0.7727 at step 8 (the figure now includes the beams and
grace stems, so it is no longer comparable).

The P3 gate is **not met**: the remaining 0.19 stem-length deficit is
mostly automatically grouped beams (tuplets) and 0.25–0.5 sp shortening
differences (the explicit beams of `Beam-CrossM1` now all match, §9.19). The P2 gate is **not met**: 123 measure widths and 10 system
breaks still differ, largely from text-driven segment widths (needs text
metrics, P4), multi-voice collisions and courtesy headers. Both remain
open work, not a redefinition of the gate.

## 11. P5: multi-staff engraving (eng5, 2026-11-27)

P5 was brought forward because the CLI refused a private choral score with
the message `P1 engraves a single staff, score has 17`. The 17 staves are
**all parts/excerpts of the `.mscz`**; 15 belong to hidden parts, leaving
four visible staves (S/A, T/B and the two organ staves). MuseScore's
`SystemLayout` likewise visits only `Staff::show()` staves
(`systemlayout.cpp:2477-2485`).

| Step | Change | Oracle figures before → after |
| --- | --- | --- |
| 1 | Remove the refusal; one atom stream per visible staff, shared segment x, own clef/key/time and spanning bar lines | 0/12 multi-staff corpus files engraved → 12/12; 44 single-staff files unchanged (34/44 breaks, 69/192 widths, 365/1261 elements) |
| 2 | `staffDistance`/`akkoladeDistance`, `minSystemDistance`, `distributeStaves` spread and page breaks by system height | System top y vs 4.7.4 on 77 page-1 systems: mean abs. error 11.19 → 5.05 sp, median 7.00 → 4.64 sp; single-staff figures unchanged. Private score: 2 → 8 pages (oracle 7), 42 → 22 systems |
| 3 | Choir brackets, piano brace (`Scaled` SMuFL geometry in SVG and raster), instrument names, `barLineSpan` | Private page 1: 0 → 12 bracket items, 0 → 21 Edwin name glyphs; 44-file oracle unchanged |
| 4 | Lyrics as Edwin text below their staff at the shared segment x | Private page 1: 0 → 268 Edwin text glyphs; 44-file oracle unchanged |
| 5 | Oracle over all 56 corpus files, every page, page breaks and staff y; fix the slashed Edwin `o` | 44 files: 366/1261 elements, 34/44 breaks, 42/44 page counts, 15/88 staff tops → 56 files: 385/2771, 34/56 breaks, 53/56 page counts, 19/198 staff tops; **0 failures** |

The `o` defect came from `gen-smufl-subset.py`: `glyph_edges` used
`contour[0]` as the starting point and then iterated `contour[1:]`, dropping
the first arc and replacing it with a chord. Edwin's lowercase `o` was
slashed in both SVG and raster output. Both generated tables were
regenerated; a unit test in `scoretk-smufl` pins the first arc.

The P5 gate (page count and breaks equal on ≥ 90 % of files) is **not met**:
53/56 page counts but only 48/57 page-break patterns and 19/198 staff tops
within 0.5 sp. The remaining causes are the title frame being 7 sp too tall
on some files, horizontal minimum widths (P2 debt) and the lack of
spacers/skylines in the vertical spread pass (P4 work). No file or failure
is excluded from these figures.

## 12. Review of the private choral score (eng6, 2026-11-27)

Five defects were reported by comparing page 1 of our render with MuseScore's
4.7.4 PNG. The private score is never copied into the repository; only the
final before/after/reference images live in the external scratchpad
(`engrave-img/eng6`). Steps are committed separately, each with an oracle
measurement before and after. The baseline oracle of step 1 (56 files) is
identical to eng5: 385/2771 elements within 0.1 sp, 69/192 measure widths,
34/56 system breaks, 53/56 page counts, 48/57 page-break patterns, 19/198
staff tops, 684 stems.

### 12.1 Compound time signatures (step 1)

`place_time_sig` skipped every value above 9, so 12/8 drew only the "8".
It now emits one SMuFL digit per decimal character, with the run widths and
heights from the Leland advances/bboxes and the two runs centred around the
middle staff line (`tlayout.cpp:6044-6185`, `utils.cpp:954-970`). A unit test
covers 12/8, 6/8 and 3/2; synthetic scores of those three signatures were
also rendered, and 12/8 (the only one MuseScore accepts with the source
measure durations used) was compared with a 4.7.4 SVG reference. On the
private score page 1 the signature now reads "12/8" instead of a lone "8".

| Metric | Before | After |
| --- | ---: | ---: |
| Elements within 0.1 sp (56 files) | 385/2771 | 385/2771 |
| Measure widths within 0.1 sp | 69/399 | 69/399 |
| Files with identical system breaks | 34/56 | 34/56 |
| Files with the oracle page count | 53/56 | 53/56 |

Time signatures are not among the oracle's extracted elements, so no figure
changes; the visual comparison is the evidence for this step.

### 12.2 Beams from time-signature groups (step 2)

Atoms now carry the effective `BeamMode` of each chord/rest, computed in the
atom pass from the signature's group nodes (`groups.cpp:43-150, 266-307`),
from `baseBeamMode`/`actualBeamMode` (`:107-263`) and from explicit `<Beam>`
modes. The page pass keeps its earlier beam geometry, but groups no longer
depend on an explicit `<Beam>`: eighths in 12/8 and 6/8 are beamed in threes,
in 3/2 by half-note beats, and automatic rests are excluded from groups. A
beam slab containing a rest no longer unwraps a missing chord (the
`moonlight` page-3 crash of the previous step is gone). Unit tests pin the
12/8, 6/8, 9/8 and 3/2 group nodes, the explicit-node override and the
automatic-rest rule.

| Metric | Before | After |
| --- | ---: | ---: |
| Elements within 0.1 sp (56 files) | 385/2771 | 385/2771 |
| Stem-length agreement | 0.6535 (684) | 0.6550 (684) |
| Measure widths within 0.1 sp | 69/399 | 69/399 |
| Files with identical system breaks | 34/56 | 34/56 |
| Pages with identical system breaks | 48/57 | 48/58 |

The page-break denominator grows by one because `moonlight` page 3 used to
panic in beam geometry and was therefore never produced; the number of
matching page-break patterns is unchanged. On the private score page 1 the
organ eighths now carry their 12/8 beam groups (chords included) instead of
isolated flags.

### 12.3 Chords of two voices on one staff (step 3)

The existing atom pass already forced voice 1 stems up and voice 2 stems
down by track parity, gave both voices the same segment x, and moved voice-2
rests one line space down (`restlayout.cpp:694-765`). The gap was in the
unison/second offsets: the old code shifted the later voice's head whenever
two voices shared a staff line. It now follows `layoutChords1` /
`calculateChordOffsets` (`chordlayout.cpp:1887-2130, 2228-2290`): a true
unison with the same head, duration and dots shares one head x, a second
shifts the down-stem chord enough for both stems to align, and an
unshareable overlap shifts the up-stem chord by a head width plus 0.15 sp.
Only the colliding chord's x changes; the segment itself stays shared.
Unit tests pin the three rules.

| Metric | Before | After |
| --- | ---: | ---: |
| Elements within 0.1 sp (56 files) | 385/2771 | 384/2771 |
| Within 0.1 sp, x only | 0.1610 | 0.1599 |
| Stem-length agreement | 0.6550 (684) | 0.6550 (684) |
| Measure widths within 0.1 sp | 69/399 | 69/399 |
| Files with identical system breaks | 34/56 | 34/56 |
| Pages with identical system breaks | 48/58 | 48/58 |

The single lost element is a `moonlight` notehead whose head-sharing x now
differs from the nearest reference head; no other figure changes. On the
private score page 1 the S/A unisons now share their head, with voice 1's
stem up and voice 2's stem down, as in the 4.7.4 page. The oracle's corpus
list had to be regenerated for this step because the eng5 worktree it
pointed at had been removed after the merge; the MuseScore references are
unchanged.

### 12.4 Lyrics, hyphens and melisma lines (step 4)

Atoms now keep each syllable's verse, text, `LyricsSyllabic` state and
whether it carries a melisma (`lyrics.cpp:120-170`). Syllables are centred
under the notehead with one `lyricsLineHeight` per verse
(`lyricslayout.cpp:45-170`); a `plan_lyric_lines` pass emits the hyphens
after `Begin`/`Middle` syllables and the dashed extender lines of melismas,
with the 0.4–0.6 sp dash lengths and 16 sp spacing of
`LyricsLayout::layoutDashes` (`lyricslayout.cpp:206-366`,
`styledef.cpp:75-101`). On the private score page 1 "Mi-nuit" and
"chré-tien" now carry their hyphen and their extender line instead of one
truncated, overlapping word.

The P4 text-width switch was measured for lyrics alone: real Edwin advances
enter the overlap test, and the required gap is propagated to the shared
segment x of every staff. On the whole 56-file oracle this kept 53/56 page
counts and 48/58 page-break patterns but moved `testHairpinDynamics` away
from its system breaks (**33/56** instead of 34/56) and reduced elements
within 0.1 sp from 384 to 363/2771. As in §9.23, the switch is therefore
**left off** (`LYRIC_WIDTH_SPACING = false`); the measured helpers remain in
place for the full `computePadding`/`Skyline` port. No syllable is hidden or
truncated by the engraver, but syllables on adjacent notes can still touch
when the note spacing itself is too tight (P2 debt).

| Metric | Before | Switch on | Kept (switch off) |
| --- | ---: | ---: | ---: |
| Elements within 0.1 sp (56 files) | 384/2771 | 363/2771 | 384/2771 |
| Measure widths within 0.1 sp | 69/399 | 69/399 | 69/399 |
| Files with identical system breaks | 34/56 | **33/56** | 34/56 |
| Files with the oracle page count | 53/56 | 53/56 | 53/56 |

Lyrics, hyphens and extender lines are not among the oracle's extracted
elements, so the kept variant produces no metric change; the visual
comparison of the private score is the evidence for the text fixes.

### 12.5 Header credits and multi-line VBox texts (step 5)

The private score's VBox composer and poet (lyricist) texts contain `<br/>`
line breaks, which `TextBase::createBlocks` turns into newline-separated
blocks. `RichText::from_markup` ignored that tag, so both texts collapsed
into one long line and the already style-aligned credits overlapped in the
middle of the frame. `<br>`/`<br/>` now end a text block like a literal
newline (`text.rs`), and `place_credits` draws one Edwin line per block with
the credit's own alignment: title/subtitle centred, composer right, lyricist
left, bottom-aligned styles stacking upward
(`textlayout.cpp`, `boxlayout.cpp`). On page 1 of the private score the
title is centred, the two lyricist lines sit at the left and the two
composer lines at the right, as in MuseScore's page. `e2e_corpus` still
passes: default MusicXML/MuseScore conversions are byte-identical to the
4.7.4 references.

| Metric | Before | After |
| --- | ---: | ---: |
| Elements within 0.1 sp (56 files) | 384/2771 | 384/2771 |
| Measure widths within 0.1 sp | 69/399 | 69/399 |
| Files with identical system breaks | 34/56 | 34/56 |
| Files with the oracle page count | 53/56 | 53/56 |

Credits and text lines are not extracted by the geometric oracle; the
visual comparison of the private header is the evidence for this step.

## 13. eng7: stem, barline and voice follow-up (2026-11-27)

The private score is still read only from its absolute path; only the page-1
PNG files below leave the scratchpad. The baseline oracle of this campaign is
`/tmp/.../scratchpad/oracle-eng7-base`: 56 files, 0 failures, 2 771 elements,
384 within 0.1 sp, 0.6550 stem-length agreement (684 matched stems), 69/399
measure widths, 34/56 system breaks, 53/56 page counts, 48/58 page-break
patterns and 19/198 staff tops. Failures and unmatched elements remain in
every denominator. The reference page is `engrave-img/eng7/ref.png`; the
before render is `engrave-img/eng7/before.png`.

### 13.1 Stem creation and length (step 1)

`has_stem` now follows `TDuration::hasStem` (`dom/durationtype.cpp:222-241`)
exactly: the long note carries a stem, breve and whole (including a dotted
whole) do not. `stem_length` ports `StemLayout::calcDefaultStemLength`
(`rendering/score/stemlayout.cpp:42-160`) with its beam-length addition
(`:326-342`), the beam-count reduction table (`:345-397`), the beamed minimum
(`:415-471`) and the `minStaffOverlap` target (`:165-182`). The hook cap is
kept at **two** quarter spaces, not the nominal one: a trial with the nominal
cap and the optical adjustment cost seven matched stems on `linked-ties`
(6→4) and `voiceswitching` (40→34) while gaining one on `changevisibility`
(3→4); both were dropped. Tests pin the duration table and the four-line
stem-length cases.

| Metric | Before | After |
| --- | ---: | ---: |
| Stem-length agreement (684 stems) | 0.6550 | 0.6550 |
| Within 0.1 sp, both axes | 0.1386 (384/2771) | 0.1386 (384/2771) |
| Measure widths | 69/399 | 69/399 |
| System breaks | 34/56 | 34/56 |

This measurement was taken with a **stale CLI binary**: `after.png` had been
rendered before the commit, so the identical figure said nothing about the
change. A fresh build leaves the 56-file figure unchanged (the duration table
was already correct) and a fresh private render is in §13.6. Root cause of
the crowding is still the page geometry (three systems on page 1 against
four in 4.7.4), not a missing `Chord::createStem` call.

### 13.2 End-of-measure spacing (step 2, reverted)

Adding a trailing `barNoteDistance` to the last segment's width, as suggested
by `horizontalspacing.cpp:1039,1195,1304-1307`, **regressed** every horizontal
figure: measure widths 69/399 → 56/398, system breaks 34/56 → 33/56, elements
384/2771 → 362/2771, page-break patterns 48/58 unchanged. The segment's
`minRight()` already contains that margin in MuseScore; adding it again
stretches every measure. The experiment was reverted and is not committed;
the last segment and bar line are at the oracle x in the private page, so the
reported "note beyond the bar line" is not reproducible on this render. A
second measurement on the fresh build (§13.7) gives the mechanism: the
measure width is `atom.width`, which already ends with the last segment's own
width; the bar line is drawn at that cursor, leaving 1.82 sp (system 1),
1.51 sp (system 2), 1.39 sp (system 3) and 1.41 sp (system 4) between the
last eighth-note segment and the bar line. Adding `barNoteDistance` again
double-counts the right margin.

### 13.3 Grand-staff bar lines (step 3)

`barline_groups` now spans to the next visible staff of the same part even
when that staff does not repeat `<barLineSpan>1</barLineSpan>`
(`dom/barline.cpp:193-210`, `nextVisibleSpannedStaff`). A unit test pins the
two-staff part. The private organ part already declared the flag on its first
staff and already emitted bar lines 11.29 sp tall over both staves; the
corpus figures are unchanged (69/399 widths, 34/56 breaks, 0.6550 stems).

### 13.4 Voice directions of beamed groups (step 4)

The measure-wide voice test was replaced by the tick-overlap test of
`Measure::hasVoices(staffIdx, tick, len)` (`dom/measure.cpp:2360-2419`,
called at `chordlayout.cpp:1584`): track parity is applied only while another
voice sounds during the chord. A unit test covers sequential and overlapping
voices. The 56-file figures and the private page-1 stems are unchanged
(0.6550, 384/2771, 34/56); the score's beams carry no explicit `<Beam>` or
`<StemDirection>`, so their automatic groups keep the same default stems.
At that point only the atoms were fixed: `layout.rs::plan_beams` still used a
system-wide voice count and ignored the chord's own direction (§13.8).

### 13.5 Lyrics spacing re-measured (step 5, switch kept off)

With the stem and voice fixes in place, `LYRIC_WIDTH_SPACING = true` was
measured again over all 56 files (`/tmp/.../oracle-eng7-lyrics-on2`):

| Metric | Kept (off) | Lyrics on |
| --- | ---: | ---: |
| Elements within 0.1 sp | 384/2771 | **369/2771** |
| Measure widths within 0.1 sp | 69/399 | 68/399 |
| Files with identical system breaks | 34/56 | **34/56** |
| Files with the oracle page count | 53/56 | 53/56 |
| Pages with identical system breaks | 48/58 | 48/58 |

The 34→33 break regression of §12.4 is gone, but the per-file cause is the
same: `copypastesymbollist-lyrics` drops from 35 to 26 elements within
0.1 sp and `testExcludeInvisibleElements` from 49 to 43 (one measure width
lost). The switch therefore stays off: hiding those losses to gain lyric
clearance would be a metric that excludes its own failures. A faithful fix
needs the full `HorizontalSpacing::computePadding`/`Skyline` port, where the
lyric shape pushes the *following* segment instead of inflating every
segment carrying text.

### 13.6 Beamed stems anchored in the group's direction (step 6)

The fresh private render of §13.1 still looked stemless on the beamed notes
of `moonlight`: 234 of its stems were emitted as rectangles of **negative**
height (−0.23 sp), hence invisible. `plan_beams` chose the group direction
from an explicit `<Beam><StemDirection>down</StemDirection>` but computed the
beam's anchor tips from each chord's *own* up direction
(`own_up = chord.stem_up`). The slab was consequently anchored above the
heads while the stems were drawn down, and `(far_end - note_y).max(0)`
collapsed to zero. `Beam::layout1` (`beamlayout.cpp:169-199`) applies one
direction to the whole group, so the anchor now uses that same `up`. A unit
test pins the explicit-down case (it failed before the fix).

| Metric | Before | After anchor fix |
| --- | ---: | ---: |
| Stem-length agreement (684 stems) | 0.6550 | **0.6608** |
| Within 0.1 sp, both axes | 384/2771 | 384/2771 |
| Measure widths | 69/399 | 69/399 |
| System breaks | 34/56 | 34/56 |

The private page's SVG still has 292 stems, 19 flags, 44 beams and no
negative-height stem; `engrave-img/eng7/after.png` was regenerated from the
fixed build and the visible up/down stems now reach their noteheads.

### 13.7 Voice direction of beamed groups and the end-of-measure diagnosis (step 7)

The second half of defect 4 was in `plan_beams`: when any two voices existed
in the system, it forced `voice.is_multiple_of(2)` regardless of whether they
overlapped in time, whereas the atoms already use `Measure::hasVoices`
(`measure.cpp:2360-2419`, `chordlayout.cpp:1584`). The group now takes the
first chord's `stem_up`, overridden by an explicit beam direction, and the
unit test `a_beam_follows_its_chord_when_two_voices_do_not_overlap` fails on
the previous code.

| Metric | Anchor fix | + chord direction |
| --- | ---: | ---: |
| Stem-length agreement (684 stems) | 0.6608 | 0.6579 |
| Within 0.1 sp, both axes | 384/2771 | 384/2771 |
| Measure widths | 69/399 | 69/399 |
| System breaks | 34/56 | 34/56 |

The −2 matched stems are both in `testTuplets8` (4→2 within 0.1 sp): its
first chord has an explicit down stem while the former group-wide pitch rule
gave up; following the chord is the faithful 4.7.4 behaviour. Net gain over
the campaign baseline is +2 stems (0.6550 → 0.6579). The private score has
no overlapping voices in a beamed group, so its page is unchanged; the
remaining overlap complaints there come from system height, not beam
direction.

For defect 2, the fresh private render was measured per measure on the
organ-right-hand staff: the distance from the last eighth-note segment to
the bar line is **1.82 sp** (system 1), **1.51 sp** (system 2), **1.39 sp**
(system 3) and **1.41 sp** (system 4), all measured at the system's own
justification scale (800, 663, 610, 619). The bar line is drawn at
`x0 + atom.width × scale`, i.e. after the last segment's own width, so the
noteheads are before it; the earlier `barNoteDistance` experiment
double-counted the margin and is not committed (§13.2).

### 13.8 Lyrics re-measured after the stem/beam fixes (step 8)

`LYRIC_WIDTH_SPACING = true` was measured once more on the final build
(`/tmp/.../oracle-eng7-lyrics-on3`):

| Metric | Kept (off) | Lyrics on |
| --- | ---: | ---: |
| Elements within 0.1 sp | 384/2771 | **369/2771** |
| Stem-length agreement | 0.6579 | 0.6579 |
| Measure widths within 0.1 sp | 69/399 | 68/399 |
| Files with identical system breaks | 34/56 | 34/56 |
| Files with the oracle page count | 53/56 | 53/56 |
| Pages with identical system breaks | 48/58 | 48/58 |

The per-file causes are unchanged from §13.5 (`copypastesymbollist-lyrics`
35→26, `testExcludeInvisibleElements` one measure width). The switch stays
off; the visual lyric overlap of the private page remains a P2 debt of the
incomplete `HorizontalSpacing::computePadding`/`Skyline` port.

## 14. eng8: vertical spacing without collisions (2026-11-27)

The private choral score is still read from its absolute path only; the
before/after/reference page-1 images are in `engrave-img/eng8/`
(`before.png`, `after.png`, `ref.png`, 1000 px wide). All three were rendered
from binaries built from the tree at the corresponding commit (the earlier
stale-image mistake is avoided). The 56-file oracle baseline is
`/tmp/.../scratchpad/oracle-eng8-base`: 0 failures, 2 771 elements,
384 within 0.1 sp, 0.6579 stem-length agreement, 69/399 measure widths,
34/56 system breaks, 53/56 page counts, 48/58 page-break patterns,
19/230 staff tops, 6/134 adjacent-staff gaps, mean staff top error 8.589 sp
and mean adjacent-staff gap 6.148 sp against 8.379 in the 4.7.4 reference.
Failures and unmatched elements, systems, pages and gaps stay in every
denominator. New oracle columns report staff y per five-line staff of each
system, the gaps between adjacent staves and lyric glyphs overlapping a
staff band (the latter compared by glyph count on both sides).

| Step | Port | Oracle before → after |
| --- | --- | --- |
| 1 | `skyline.rs`: per-staff north/south skylines of all drawn items, without item pointers; `systemlayout.cpp:2485-2581, 2900-2989` and `skyline.cpp:40-91, 117-143, 192-226`. `apply_skylines` takes `max(style distance, skyline clearance + minVerticalDistance)`; style values for `minVerticalDistance` and `skylineMinHorizontalClearance` come from `styledef.cpp:749-750` | Staff top mean abs. error 8.589 → 7.368 sp; mean adjacent-staff gap 6.148 → 7.346 sp (reference 8.379); staff tops within 0.5 sp 19 → 20/232; gaps within 0.5 sp 6 → 5/134 |
| 2 | Lyrics emitted as their own class at `lyricsPosBelow`/`lyricsLineHeight` (`styledef.cpp:75-79`, `lyricslayout.cpp:581-614`), moved down by `lyricsMinTopDistance` against the staff skyline (`:616-702`) and padded by `lyricsMinBottomDistance - minVerticalDistance` on that skyline (`:704-742`) | Staff top error 7.368 → 7.376 sp; gap error 5.949 → 5.906 sp; lyric/staff collisions 0 → 0 (128 lyric glyphs drawn, 30 in the reference) |
| 3 | Honest page breaks from measured skyline gaps and heights (`pagelayout.cpp:140-265`), `minSystemSpread`/`minSystemDistance` floors and `maxSystemDistance`/`layoutPage` equalisation when spreading is off (`pagelayout.cpp:524-613`) | Gap mean abs. error 5.906 → 3.537 sp; staff top error 7.376 → 9.604 sp; staff tops 20/232 → 20/216; page counts 53/56 unchanged; page-break patterns 48/58 → 48/59 |

The final 56-file run (`oracle-eng8-final`) has 0 failures, 384/2 771
elements within 0.1 sp, 0.9543 vertical pitch, 0.6447 stem length, 69/394
measure widths, 34/56 system breaks, 53/56 page counts, 48/59 page-break
patterns, 20/216 staff tops, 5/123 adjacent-staff gaps and 0 lyric/staff
collisions. On the private page the staves no longer collide with the lyrics
or with each other and the organ brace spans the right staves, but only two
systems fit where 4.7.4 fits four: the remaining height debt comes from
upstream stem directions and measure-number skylines, not from the skyline
pass itself. The P3/P2/P5 gates quoted above are not redefined by this work.

### 14.1 Review: one baseline per lyric verse (2026-11-27)

The page-1 render still spaced "Minuit" glyph by glyph: the capital in
`Minuit` made `place_text_kind` choose a taller cap height for that syllable
than for `nuit`, and `lyric_autoplace_moves` added a different `dy` to every
glyph according to the staff outline at its own x. `LyricsLayout` does neither:
`setDefaultPositions` gives every syllable of a verse one y
(`lyricslayout.cpp:602-613`), collisions are measured between the staff's
south skyline and the *verse's* skyline (`:616-676`), and
`moveThisVerseAndOuterOnes` moves the whole verse, plus the outer ones, by
one `diff` (`:679-702`). The engraver now emits lyrics after the notation,
computes one offset per verse from the staff skyline, and applies it to the
syllables and their hyphen/melisma lines. The cap height used for lyric
baselines is font-wide (`M`), not per-string. A unit test pins both rules.

| Metric | Before | After |
| --- | --- | ---: |
| Files / failures / elements (56 files) | 56 / 0 / 2490 | 56 / 0 / 2490 |
| Elements within 0.1 sp | 378/2490 | 378/2490 |
| Measure widths / system breaks | 69/394 / 34/56 | 69/394 / 34/56 |
| Page counts / page-break patterns | 53/56 / 48/59 | 53/56 / 48/59 |
| Staff tops within 0.5 sp | 20/216 | 20/216 |
| Adjacent-staff gaps within 0.5 sp | 5/123 | 5/123 |
| Lyric glyphs drawn (reference / ours) | 30 / 128 | 30 / 128 |
| Lyric glyphs overlapping a staff | 0 / 0 | 0 / 0 |

The glyph count is unchanged because MuseScore's SVG uses one path per
syllable while our Edwin backend emits one path per character; the oracle's
collision test compares the same glyph granularity on both sides. The visual
evidence is `engrave-img/eng8/after.png`, rendered from the committed tree:
every syllable of a verse now sits on one baseline on both systems of page 1.
The remaining horizontal touching of adjacent syllables is P2 spacing debt.

### 14.2 Review: read the `.mscz` style file before page layout (2026-11-27)

The page-1 render was still over-spread: two systems of four staves against
three in the 4.7.4 reference, with wide bands between the choir and organ
staves. The style values were not wrong in our vertical code — they were
**never read**. MuseScore loads `score_style.mss` from the `.mscz` first and
then applies the inline `<Style>` of the `.mscx`
(`rw/mscloader.cpp:86-94`, `infrastructure/mscreader.cpp:127-132`). Our 4.x
reader only took three fields from that file and the engraver therefore used
the built-in defaults: spatium 1.75 mm instead of the score's 1.3 mm,
15 mm margins instead of 10 mm, `enableVerticalSpread` true instead of the
score's `0`, and a 180 mm printable width instead of 190 mm. With a 170 sp
page instead of the score's 228 sp, only two systems fitted.

`read4xx::read_with_style` now applies every recognised `<Style>` key from
`score_style.mss` to the style settings before the inline overrides, and the
lower-case `spatium` key of 4.x style files is recognised alongside the
legacy `Spatium` spelling (`styledef.cpp:752`). The vertical pass keeps the
style distance as a floor and adds `minVerticalDistance` only where skylines
actually overlap (`systemlayout.cpp:2564-2575, 2931-2989`); no clearance is
added twice. Page breaking still uses `minSystemDistance` plus the measured
system height (`pagelayout.cpp:248-265`), not the spread distances.

| Private page 1 | Before | After | 4.7.4 |
| --- | ---: | ---: | ---: |
| Systems | 2 | **3** | 3 |
| Staff tops, sp | 25.6, 45.6, 56.1, 76.0, 99.3, 119.3, 129.8, 149.8 | 24.6, 40.7, 51.2, 61.7, 80.7, 97.5, 108.0, 118.5, 137.5, 153.4, 163.9, 174.4 | 25.4, 41.9, 52.4, 64.5, 83.5, 98.5, 109.0, 121.0, 140.0, 157.6, 168.1, 179.6 |
| System top-to-top, sp | 73.7 | **56.0, 56.9** | 58.0, 56.6 |
| Page count | not measured on the previous build | 5 | 7 |

| Oracle, 56 files | Before | After |
| --- | ---: | ---: |
| Failures / elements | 0 / 2490 | 0 / 2490 |
| Elements within 0.1 sp | 378/2490 | 378/2490 |
| Measure widths / system breaks | 69/394 / 34/56 | 69/394 / 34/56 |
| Page counts / page-break patterns | 53/56 / 48/59 | 53/56 / 48/59 |
| Staff tops / adjacent-staff gaps within 0.5 sp | 20/216 / 5/123 | 20/216 / 5/123 |

The corpus is made of bare `.mscx` files with no `score_style.mss`, which is
why its figures are unchanged; the private score's page geometry is the
evidence for this step. The remaining gap to 4.7.4 is the total page count
(5 vs 7) and the horizontal touching of adjacent syllables (P2 debt), neither
of which is hidden by a metric. A unit test pins the style-file precedence.

### 14.3 Lyric shapes in horizontal spacing (2026-11-27)

The earlier attempts (§12.4, §13.5, §13.8) switched on a width computed for
*every* segment carrying text. That is not how MuseScore spaces lyrics:
`HorizontalSpacing::minHorizontalDistance` walks the **shapes** of two
neighbouring segments (`horizontalspacing.cpp:1216-1266`), same-verse lyrics
are `NON_KERNING` (`:1785-1796`), and their distance is the right edge of the
previous syllable's shape minus the left edge of the next one, plus padding
from `computePadding`/`computeLyricsPadding` (`:1438-1460`, `:1572-1604`).
`Segment::createShape` adds `Lyrics` as annotation shapes
(`dom/segment.cpp:2640-2697`). The padding row is `lyricsMinDistance`
(`rendering/paddingtable.cpp:207-209`, `styledef.cpp:78`, 0.25 sp in 4.x and
0 in the legacy 3.x styles), floored at 0.1 sp by
`horizontalspacing.cpp:1218`, and raised to `lyricsDashMinLength + 2 *
lyricsDashPad` = 0.5 sp for `Begin`/`Middle` syllables when
`lyricsDashForce` is set (`styledef.cpp:80,83,90`).

The atom pass now measures one Edwin shape per verse of a chord/rest segment
and keeps the last shape of a verse in absolute system coordinates across
measure boundaries (`horizontalspacing.cpp:47-89`). The resulting demand
widens the measure's **minimum** width; `stretchSegmentsToWidth`
(`:1087-1111`) overwrites every chord/rest width with `force * stretch`, so
justification subtracts that fixed minimum from the spring budget instead of
stretching it a second time (`breaking.rs`). Harmony/tempo widths remain
measured but off (§9.23); the old whole-text-width switch is gone. A unit
test pins the half-shape, `lyricsMinDistance` and dash-padding rules.

| Oracle, 56 files | Before (`oracle-eng8-task1-before`) | After (`oracle-eng8-step1-final`) |
| --- | ---: | ---: |
| Failures / elements | 0 / 2490 | 0 / 2490 |
| Elements within 0.1 sp | 378/2490 | **343/2490** |
| Measure widths within 0.1 sp | 69/394 | **66/394** |
| Files with identical system breaks | 34/56 | **33/56** |
| Page counts / page-break patterns | 53/56 / 48/59 | 53/56 / 48/59 |
| Stem-length agreement | 0.6447 | 0.6447 |

The break count is honestly **worse**, not a redefinition of the gate: the
faithful demand moves `testHairpinDynamics` off its oracle breaks (23 → 13
elements within 0.1 sp) and reduces `copypastesymbollist-lyrics` (35 → 13).
The earlier char/whole-width switch also gave 33/56 (§12.4); this port keeps
that regression rather than hiding it, with the per-file causes named above.
`lyricsMinDistance` is not present in the bare corpus files, so the demand
there comes from the 0.1 sp floor and the 0.5 sp dash padding.

| Private page 1 | Before | After | 4.7.4 |
| --- | --- | --- | --- |
| Syllable word gaps (first verse, sp) | glyphs touching | **1.5–3.4 sp** | wider |
| Page count | 5 | **6** (a short sixth page, 43 notes) | 7 |

The words on the private page now have positive ink gaps instead of
touching, but the systems still hold too many measures: the 5 → 6 page count
is reported rather than attributed yet. The private score sets
`lyricsMinDistance` 0, so its floor is 0.1 sp; no score text is copied into
the repository. The before/after PNGs are `engrave-img/eng8/before.png` and
`after.png`, both rendered from the tree at the corresponding commit.

### 14.4 Dashes at the verse baseline (2026-11-27)

`plan_lyric_lines` placed every hyphen and melisma dash at
`text_top - cap(text)/2`, i.e. roughly half a cap height **above the top of
the glyphs**. `LyricsLineSegment::baseLineShift`
(`dom/lyricsline.cpp:181-191`) puts a dash at
`-styleD(lyricsDashYposRatio) * fontMetrics().xHeight()` *relative to the
lyrics' own baseline*; `LyricsLayout::setDefaultPositions`
(`lyricslayout.cpp:598,606-611`) gives the segment the verse's y plus that
shift. Our Edwin backend draws the glyphs with their baseline at
`top + cap("M")`, and x-height is the Edwin `x` glyph's bbox (466 font
units). The private score sets `lyricsDashYposRatio` 0.67 instead of the
4.7.4 default 0.5 (`styledef.cpp:92`), so `Sid::lyricsDashYposRatio` is now
read by `StyleSettings::set_value` and copied into `PageSpec`. A unit test
pins the resulting dash y; the horizontal origin of the dashes is left
untouched (moving it half a space changed 10 systems of `moonlight`
and inflated the oracle's element denominator from 2 490 to 2 771, so that
experiment was dropped).

| Oracle, 56 files | Before (`oracle-eng8-step1-final`) | After (`oracle-eng8-step2b`) |
| --- | ---: | ---: |
| Failures / elements | 0 / 2490 | 0 / 2490 |
| Elements within 0.1 sp | 343/2490 | 343/2490 |
| Measure widths / system breaks | 66/394 / 33/56 | 66/394 / 33/56 |
| Page counts / page-break patterns | 53/56 / 48/59 | 53/56 / 48/59 |
| Lyric glyphs overlapping a staff | 0 / 0 | 0 / 0 |

On the private page the dashes now sit at 0.67 x-height above each verse's
baseline (measured 1 041 fixed units at em 3 334) instead of floating above
the words. Lyrics, hyphens and dashes are not extracted by the geometric
oracle, so the unchanged figures above are expected; the visual comparison
of `engrave-img/eng8/after.png` against `before.png` and `ref.png` is the
evidence for this step.

### 14.5 Private page count after the lyric spacing port (2026-11-27)

The private score now engraves in **6 pages** (5 pages of 3 systems plus a
short sixth page with 1 system), against **7** in MuseScore 4.7.4 (six pages
of 3 systems and a final page of 3–4 systems). The page count is measured by
rendering each page until a page without `class="StaffLines"` is produced;
no score data is copied into the repository.

The difference is horizontal, not vertical. On page 1 the staff tops are
24.6, 40.5/51.0/61.5, 80.5/97.4/107.9/118.4 and
137.4/152.8/163.3/173.8 sp against 25.4, 41.9/52.4/64.4,
83.4/98.4/108.8/120.9 and 139.8/157.4/167.9/179.4 sp in 4.7.4 (within
about 1 sp), and the system top-to-top distances are 56–57 sp against
56–58 sp. What differs is how many measures a system holds: counting bar
line positions on the top staff of each system gives 4–5 measures per
system in 4.7.4 (22 systems over 7 pages) against 4–7 in our output (16
systems over 6 pages, with up to 7 on page 2).

`SystemLayout::getNextSystem` fills a system until its measured minimum
width reaches the printable width (`systemlayout.cpp:95-225`), and
`HorizontalSpacing::computeSpacingForFullSystem` supplies those widths
(`horizontalspacing.cpp:47-89`). Our break decision still discounts every
multi-staff measure to `header + (atom.width - header) * 3 / 5`
(`breaking.rs::collect_systems`, the eng5 estimate for dense organ/choir
measures). The §14.3 lyric demand widens `atom.width`, but the discount
absorbs most of it, so a fifth or sixth measure still fits where 4.7.4
breaks. A trial using the full `atom.width` there moved the organ notes past
the right margin (the system's fixed headers are staff-specific) and is not
committed. The remaining causes are the P2 spacing debt already measured in
§9.10–§9.23 (natural 3.5 sp springs, no harmony/tempo shapes, approximate
glyph minima) and the absent courtesies. This is a diagnosis only: the
page-count gate is not redefined, and no file or failure is excluded.

## 15. eng9: horizontal spacing of notes (2026-11-27)

The baseline oracle (`oracle-eng9-base`, 56 files, 0 failures) adds two
horizontal columns: mean absolute per-measure width error (a missing measure
counts with its whole width) and the notehead x agreement/mean error (an
unmatched head counts as a failure). Failures, unmatched elements, systems
and pages remain in every denominator.

| Metric | Baseline | After step 1 |
| --- | ---: | ---: |
| Failures / elements | 0 / 2 490 | 0 / 2 490 |
| Within 0.1 sp, both axes | 0.1378 (343/2 490) | **0.1486** (370/2 490) |
| Within 0.1 sp, x only | 0.1574 | 0.1687 |
| Measure widths within 0.1 sp | 66/394 | 66/394 |
| Measure width mean abs. error, sp | 9.857 | **8.857** |
| Note x within 0.1 sp (unmatched counted) | 168/2 490 | **172/2 490** |
| Note x mean abs. error, sp (matched heads) | 4.154 (876) | **2.716** (876) |
| Files with identical system breaks | 33/56 | **37/56** |
| Files with the oracle page count | 53/56 | 53/56 |
| Pages with identical system breaks | 48/59 | 48/59 |
| Stem-length agreement | 0.6447 (684) | 0.6447 (684) |

### 15.1 Duration spacing and one spring per segment (2026-11-27)

Three defects explained most of the horizontal error:

1. `StyleSettings` did not read `spacingDensity` or `measureSpacing`
   (`styledef.cpp:229-230`). `moonlight` and `layout_elements` set
   `measureSpacing` 1.14; the fixed 1.5 slope made every segment too wide.
   `spacing.rs` now computes `slope^log2(ticks / quarter)` with integer
   log2/exp2 (no libm in this `no_std` crate) and divides by
   `spacingDensity` (`horizontalspacing.cpp:743, 763, 805-812`). The padding
   table values (`paddingtable.cpp:55-64, 94-104, 214-248`) and shape-to-shape
   minimum distance (`:1216-1276, 1636-1646`) are ported alongside it; unit
   tests pin the slopes, the style-derived padding and overlapping shapes.
2. `compute_staff` added one spring **per voice** of a ChordRest segment.
   `justifySystem` collects one `Spring` per segment
   (`horizontalspacing.cpp:198-209`), so a two-voice measure received twice
   the force and could not be justified to the printable width. The segment
   stretch now comes from the segment's ticks and its shortest chord/rest
   (`computeSegmentDurationStretch`, `:748-781`), added once per segment.
3. `breaking.rs` derived the justification factor from
   `3.5 sp * spring`, ignoring minimum widths and fixed headers; a system
   could overflow the right margin by more than 20 sp. `stretch_budget`
   measures the real fixed and stretchable segment widths, and the force is
   solved against them (`stretchSegmentsToWidth`, `:1087-1111`). The cursor
   advances once per segment even when several voices share it.

The largest gains are `voiceswitching` (width error 44.5 → 3.1 sp, its four
explicit breaks preserved), `timesig-09` (39.5 → 3.0 sp), `layout_elements`
(424.4 → 372.0 sp), `moonlight` (896.5 → 846.2 sp width and 281.1 → 152.8 sp
x error) and `copypaste_parts`, whose break pattern now matches. The single
regression is `testWedge4` (28.7 → 45.4 sp width, 42.6 → 69.7 sp x error); it
is reported, not excluded. The oracle script and its README document the new
columns. The private score's before/after/reference page-1 PNGs are in
`engrave-img/eng9`; the page count is still measured separately in §15.4.

### 15.2 Notes beyond the bar line (2026-11-27)

The private score's measures 3, 7 and 11 had chords drawn through an end bar
line. The cause was not `barNoteDistance`: the earlier attempt to add that
padding to every segment was reverted because it inflated all measure widths.
The actual cause is shape geometry that the atom width did not contain:

- `ChordLayout::layoutChords2` (`chordlayout.cpp:2309-2410`) mirrors a note on
  the same or adjacent line, extending the chord one head width to the left
  (up stem) or right (down stem). A chord of seconds starting a measure
  therefore reached back onto its bar line. `compute_staff` now reserves one
  head width before the first chord with seconds and one after the last.
- `calculateChordOffsets` (`chordlayout.cpp:1887-2130`) shifts a colliding
  voice's head right; `fillShape` (`:3283-3298, 3352-3469`) records the shifted
  head. The segment now grows by the largest collision offset, so a shifted
  final head stays inside its measure.

A bbox unit test asserts that a shifted voice head clears the bar line; the
oracle gained a "noteheads crossing a bar line" column (notehead boxes against
the bar lines of the same five-line staff), reported on both sides. On the
private score's page 1 the two full-head-width crossings disappear (4 → 3
crossings, the three remaining ones overlapping by at most 0.03 sp except one
1.27 sp case at the first organ measure); the corpus oracle has none.

| Metric | Step 1 | After step 2 |
| --- | ---: | ---: |
| Failures / elements | 0 / 2 490 | 0 / 2 490 |
| Within 0.1 sp, both axes | 0.1486 (370/2 490) | 0.1478 (368/2 490) |
| Note x within 0.1 sp | 172/2 490 | 171/2 490 |
| Note x mean abs. error, sp | 2.716 | 2.713 |
| Measure widths within 0.1 sp | 66/394 | 66/394 |
| Measure width mean abs. error, sp | 8.857 | 8.857 |
| Files with identical system breaks | 37/56 | 37/56 |
| Pages with identical system breaks | 48/59 | 48/59 |

The two lost element matches are reported, not excluded: this step trades a
small corpus regression for removing the visible private-score collision.

### 15.3 Collision checks and mirrored heads (2026-11-27)

The oracle now counts overlapping notehead bounding-box pairs on both sides
(unisons with coincident centres are excluded; heads on adjacent steps touch
by ~0.06 sp in both engines, so the defect threshold is 0.2 sp). `place_chord`
uses the head glyph's full bbox width when mirroring a second
(`chordlayout.cpp:2316-2415`), rather than only its right bearing; this is
correct for heads whose bbox starts left of the origin. Bbox unit tests were
added for mirrored seconds and for an augmentation dot against the following
head, alongside the existing accidental/head and header tests. Voice-collision
offsets now widen the segment in which they are applied (§15.2), so a shifted
head stays inside the measure.

| Metric | Step 2 | After step 3 |
| --- | ---: | ---: |
| Failures / elements | 0 / 2 490 | 0 / 2 490 |
| Within 0.1 sp, both axes | 0.1478 (368/2 490) | 0.1478 (368/2 490) |
| Note x mean abs. error, sp | 2.713 | 2.713 |
| Measure widths within 0.1 sp | 66/394 | 66/394 |
| Overlapping notehead pairs (reference / ours) | 0 / 3 | 0 / 3 |
| Files with identical system breaks | 37/56 | 37/56 |

The three remaining overlaps are all `moonlight` pairs of heads on adjacent
steps about 1 sp apart; Leland's `noteheadBlack` has a zero left bearing, so
the full-width mirror change is inert for this corpus but prevents the same
defect for heads with a negative bearing. They remain reported as debt, not
excluded from the metric. The private page-1 PNG is regenerated from the
committed tree (`engrave-img/eng9/after.png`).

### 15.4 System breaks and the private page count (2026-11-27)

The 34 → 33 break regression from the lyric-shape port (§14.3) is recovered
and exceeded. Once justification is solved from measured segment widths, the
eng5 `header + (atom.width - header) * 3 / 5` discount used by the break
decision is no longer needed: `collect_systems` now uses the full atom width
for every staff, as `SystemLayout::collectSystem` measures it
(`systemlayout.cpp:153-225`). The two files named in §14.3 are back on their
oracle breaks (`testHairpinDynamics`, `copypastesymbollist-lyrics`).

| Metric | Step 3 | After step 4 |
| --- | ---: | ---: |
| Failures / elements | 0 / 2 490 | 0 / 2 209 |
| Within 0.1 sp, both axes | 0.1478 (368/2 490) | **0.1652** (365/2 209) |
| Measure widths within 0.1 sp | 66/394 | **81/394** |
| Measure width mean abs. error, sp | 8.857 | 9.757 (worse) |
| Note x mean abs. error, sp | 2.713 | 2.871 (worse) |
| Vertical pitch/ledger agreement | 0.9543 (876) | 0.9224 (876) |
| Stem-length agreement | 0.6447 (684) | 0.6082 (684) |
| Files with identical system breaks | 37/56 | 37/56 |
| Files with the oracle page count | 53/56 | **54/56** |
| Pages with identical system breaks | 48/59 | **50/60** |
| Overlapping notehead pairs (reference / ours) | 0 / 3 | **0 / 0** |

The element denominator drops from 2 490 to 2 209 because `moonlight`'s first
page no longer holds the same overflowing systems; no file or failure is
excluded. On the private score page 1 the three systems now hold 3–4 measures
each, and the score engraves in **7 pages**, matching MuseScore 4.7.4 (it was
5 pages after step 1 and 6 pages in eng8). The organ notes stay inside the
right margin, so the earlier trial that rejected full widths is no longer
applicable. The before/after/reference page-1 images remain
`engrave-img/eng9/before.png`, `after.png` and `ref.png`, all rendered from
their committed trees.

## 16. Review of the private page: shared segment x (2026-11-27)

The organ staves of each system used to draw their bar lines and beats at
different x from the choir staves, and the organ's left hand was offset from
its right hand. A `Segment` is shared by every staff of a MuseScore system:
one x per tick, with a minimum width equal to the largest per-staff shape
(`horizontalspacing.cpp:47-89`, `:262-327`, `Segment::minHorizontalDistance`).
`layout.rs` now builds that grid once per system from the aligned
`segment_bases` (`atoms.rs::compute`), justifies it with the system's single
spring force and maps every chord/rest event to its segment x. A voice's
collision offset and grace notes stay relative to their segment; a bar line
sits at the widest staff's last segment edge. Beam layout and staff-line
clipping read the same grid, and the new two-staff test uses two half notes
against four quarters to pin equal bar-line and coincident-tick x.

The oracle gained a "bar-line x spread across staves of one system" column:
bar lines are grouped by staff, staves are clustered into a system by their
shared staff-line extents and bar-line pattern, and the largest x distance
from a boundary is reported in staff spaces, on both sides. Boundaries that
do not exist on a staff count with the 3 sp cap, so a missing bar line cannot
improve the figure.

| Metric (`oracle-eng9-review-base` → `oracle-eng9-step5`) | Before | After |
| --- | ---: | ---: |
| Failures / elements | 0 / 2 209 | 0 / 2 209 |
| Within 0.1 sp, both axes | 0.1652 (365/2 209) | **0.1603** (354/2 209) |
| Stem-length agreement | 0.6082 (684) | 0.6082 (684) |
| Measure widths within 0.1 sp | 81/394 | 81/394 |
| Files with identical system breaks | 37/56 | 36/56 (one regression) |
| Max bar-line x spread, sp (reference / ours) | 2.046 / 2.001 | 2.046 / 2.001 |
| Boundaries with spread > 0.05 sp (reference / ours) | 17/229 / 15/137 | 17/229 / 15/214 |

The corpus figure is nearly unchanged because its systems are mostly
single-staff or already aligned; the comparison also counts far more
boundaries after the change, as the shared grid now supplies a bar-line edge
on every staff. The visible fix is on the private page: all four staves of
each system now share their bar-line and beat x, including the organ's two
hands (`engrave-img/eng9/step5.png`, rendered from this tree). The one lost
system break is reported, not excluded.

## 17. Review of the private page: instrument names in the indent (2026-11-27)

The short names "S A", "T B" and "Org" were cut by the left page edge: names
were drawn at `margin_left - bracket - instrumentNameOffset` without reserving
their width. `SystemLayout::layoutSystem` (`systemlayout.cpp:2082-2127`,
`:2192-2207`) instead sets the system's left margin to the widest name plus
`instrumentNameOffset` plus the brackets (and at least
`firstSystemIndentationValue` on the first system), then right-aligns each
name at that width. `decor::system_left_indent` now computes that value from
the same Edwin advance table used to draw the names; both the line breaker
and the page pass use it, so the shorter printable width moves the breaks too.
Long and short instrument font sizes now come from the score's `<Style>`
(`styledef.cpp:1065,1081`; 12 pt in the private score, 10 pt by default), and
a single-part score hides the names (`hideInstrumentNameIfOneInstrument`,
`styledef.cpp:543`).

The oracle gained an "instrument-name glyphs left of the page margin" column:
the margin is the viewBox width minus the staff lines' right edge, and each
name path starting left of it is counted on both sides.

| Metric (`oracle-eng9-review-base2` → `oracle-eng9-step6`) | Before | After |
| --- | ---: | ---: |
| Failures / elements | 0 / 2 209 | 0 / 2 209 |
| Within 0.1 sp, both axes | 0.1603 (354/2 209) | 0.1689 (373/2 209) |
| Measure widths within 0.1 sp | 81/394 | **89/394** |
| Files with identical system breaks | 36/56 | 36/56 |
| Stem-length agreement | 0.6082 (684) | 0.6082 (684) |
| Instrument-name glyphs drawn (reference / ours) | 23 / 476 | 23 / **123** |
| Instrument-name glyphs left of the page margin (reference / ours) | 2 / 418 | 2 / **12** |
| Max bar-line x spread, sp (reference / ours) | 2.046 / 2.001 | 2.046 / 2.001 |

On the private page all names are now fully inside the margin
(`engrave-img/eng9/step6.png`). The page count grows from 7 to **8** because
the faithful name/bracket indent removes printable width from every system;
this is reported as a regression, not hidden by a metric. The 12 remaining
clipped glyphs are all from corpus files whose long names still exceed the
style-derived width; they stay in the denominator.

## 18. Review of the private page: one line per lyric verse (2026-11-27)

The reader already keeps the verse number: `read4xx::read_lyrics`
(`read/read4xx/mod.rs:2425-2430`) reads `<no>` into `Lyrics::verse`, and the
private `.mscz` writes `<no>1</no>` for the second verse (`read302/chord.rs:
430-449` does the same for 3.x). The defect was in the vertical placement:
`place_lyrics` used `verse * spatium`, while MuseScore's
`LyricsLayout::setDefaultPositions` (`lyricslayout.cpp:602-613`) uses
`verse * lyrics->lineSpacing() * lyricsLineHeight`. Edwin's line spacing is
about 0.906 em (cap `M` 722 + descender 184 font units), which the 4.7.4 SVG
confirms: 10 pt at spatium 1.3 mm gives 2.46 sp between the two verses.
`text_line_height(em)` now supplies that value to `place_lyrics`,
`lyric_verse_moves` and `plan_lyric_lines`, so a hyphen and a melisma dash
follow their own verse's line. A two-verse unit test pins the baseline and
dash positions. The oracle gained a "overlapping lyric verse lines" column:
glyphs are grouped by their baseline, and two verse bands whose baselines
are more than 0.6 sp apart but whose vertical extents overlap count as one
defect (both sides).

| Metric (`oracle-eng9-review-base3` → `oracle-eng9-step7`) | Before | After |
| --- | ---: | ---: |
| Failures / elements | 0 / 2 209 | 0 / 2 209 |
| Within 0.1 sp, both axes | 0.1689 (373/2 209) | 0.1689 (373/2 209) |
| Measure widths within 0.1 sp | 89/394 | 89/394 |
| Stem-length agreement | 0.6082 (684) | 0.6082 (684) |
| Files with identical system breaks | 36/56 | 36/56 |
| Max bar-line x spread, sp (reference / ours) | 2.046 / 2.001 | 2.046 / 2.001 |
| Overlapping lyric verse lines (reference / ours) | 0 / 0 | 0 / 0 |

The corpus files carry only one verse, so their figure stays at zero on both
sides; the private score is the evidence: its first-system verse lines
overlapped once before the port and **0 times** after, with baselines 2.46 sp
apart instead of 1 sp. The page-1 PNG rendered from this tree
(`engrave-img/eng9/step7.png`) shows "Ou" on its own line below "Mi-nuit". The
lyrics now push the following staff further down, so the staff y positions
change, but the stem-length and bar-line-spread figures above are unchanged:
the shared-segment-x port did not worsen them.

## 19. Review of the private page: lyric collisions (2026-11-27)

The former oracle column "overlapping lyric verse lines" grouped *all* glyphs
of a page by absolute baseline, required two bands more than 0.6 sp apart and
counted only their vertical extents. It reported 0 on the private score while
four systems visibly piled syllables on top of one another. It has been
replaced by two columns, computed per system and per staff. Neither SVG
carries the source `<no>` of a lyric, so a verse is reconstructed from the
rendered baseline: syllables whose baselines are within 0.7 sp are one line
(a descender moves a MuseScore path's bbox bottom by up to 0.5 sp), and the
lines are numbered downwards from the staff. MuseScore writes one path per
syllable and scoretk one Edwin path per glyph, so each glyph is attached to
the nearest notehead x of its staff (within 3 sp); a simple left-to-right
word gap would merge overlapped syllables and hide the defect. A collision
is a pair of syllable bboxes overlapping by more than 0.1 sp in x and y,
reported separately for different verses and for adjacent syllables of the
same verse. The corpus files carry one verse and no collisions on either
side, so the private page is the evidence: **9 same-verse collisions before,
0 after, 0 in the 4.7.4 reference**, on the same fixed metric.

The previous verse-line port had placed each syllable on its verse's line,
but the horizontal demand was added to `MeasureAtom::min_extra`, i.e. to the
bar line at the end of the measure. Two neighbouring syllables therefore
kept their old x while the empty space grew after the last note.
`HorizontalSpacing::minHorizontalDistance` (`horizontalspacing.cpp:1216-1266`)
instead puts that demand in the width of the *preceding segment*, and
`stretchSegmentsToWidth` (`:1087-1111`) only grows the springs. The deficit
now shifts the current segment base (and everything after it in the
measure), so the pair is separated in the drawn grid; `min_extra` stays zero.
A unit test checks the resulting segment gap. The page now holds three
systems instead of four, and the remaining words have clear ink gaps.

A melisma with `<ticks_f>` but no later syllable of the same verse was
drawn as spaced dashes running to the system's right edge, under unrelated
words. `LyricsAtom::melisma_end` records the segment at `Lyrics::endTick()`
(`lyricslayout.cpp:395-480`), and `plan_lyric_lines` draws one continuous
`LyricsLayout::layoutMelismaLine` up to that segment, only falling back to
the next same-verse syllable or the system edge when the source gives no
tick count. The two-verse unit test also pins the line's end x.

| Metric (`oracle-eng9-step7` → `oracle-eng9-final`) | Before | After |
| --- | ---: | ---: |
| Failures / elements | 0 / 2 209 | 0 / 2 209 |
| Within 0.1 sp, both axes | 0.1689 (373/2 209) | **0.1698** (375/2 209) |
| Measure widths within 0.1 sp | 89/394 | **92/394** |
| Files with identical system breaks | 36/56 | 36/56 |
| Files with the oracle page count | 54/56 | 54/56 |
| Stem-length agreement | 0.6082 (684) | 0.6082 (684) |
| Max bar-line x spread, sp (reference / ours) | 2.046 / 2.001 | 2.046 / 2.001 |
| Boundaries with spread > 0.05 sp (reference / ours) | 17/229 / 14/214 | 17/229 / 14/214 |
| Overlapping lyric syllables, different verses (reference / ours) | 0 / 0 | 0 / 0 |
| Overlapping lyric syllables, same verse (reference / ours) | 0 / 0 | 0 / 0 |

On the private page the verse separation from §18 is unchanged: verse 0 is
the top line below the staff, verse 1 (`Ou`) the lower one; no syllable is
placed per measure or per word. `engrave-img/eng9/after.png` is rendered
from the committed tree.
## 20. Resource budget (Kobo Mini)

### 20.1 Method

`scripts/measure-engrave-memory.sh [--build] [--out DIR] FILE…` builds the CLI
with the **feature-gated** `memory-report` dependency (`dhat` 0.3.3, never in
a default build or a distributed binary), then runs
`scoretk <file> -o out.svg` once per file in a throwaway directory. dhat gives
total allocations, heap at peak (`t-gmax`) and its `dhat-heap.json` backtraces;
GNU `time` gives wall time and max RSS. The SVG and the JSON are written
outside the repository; the private score is read from its absolute path and
never copied into the worktree. Host: the shared Intel U300 machine of
`performance.md` §2, `release` profile (`opt-level = 1`, no LTO).

### 20.2 Measured baseline

The three corpus files are the largest by `<Note>` count in `tests/corpus`
(1 180, 63 and 56 notes). All figures are for one engraved page (page 0);
"heap" is dhat's `t-gmax`, not RSS.

| File | Input | Wall | Max RSS | Total allocated | Heap at peak | Allocations |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Private `.mscz` (4 871 notes, 2.05 MB `.mscx` in ZIP) | 348 KB | 1.06 s | 76.1 MB | 109.9 MB | **63.1 MB** | 577 053 |
| `v3/moonlight.mscx` | 410 KB | 0.11 s | 15.6 MB | 13.1 MB | 6.77 MB | 50 300 |
| `v4/voiceswitching.mscx` | 31 KB | 0.02 s | 10.4 MB | 1.43 MB | 0.64 MB | 9 372 |
| `v4/testExcludeInvisibleElements.mscx` | 29 KB | 0.03 s | 10.4 MB | 1.50 MB | 0.71 MB | 9 657 |

The RSS floor of ~10 MB is binary, static glyph/Edwin tables and libc, not
heap. Repeats of the private measurement gave 1.06–1.25 s wall and the same
dhat figures to within 0.01 %; the corpus wall times vary by ±0.02 s.

### 20.3 Breakdown at peak

Cumulative bytes per subsystem from the dhat backtraces (a single allocation
belongs to one family; the "peak live" column is the largest live set seen in
that family and is not additive across families). Raster is not used by the
SVG path; glyph tables are static and therefore absent from dhat.

| Family | Private: cumulative / peak live | `moonlight`: cumulative / peak live |
| --- | ---: | ---: |
| Parse (4.x DOM / 3.x XML tree, ZIP, input bytes) | 35.4 / 20.9 MB | 5.11 / 4.25 MB |
| Pivot model construction (`Reader`, `Score::add_note`) | 12.3 / 12.1 MB | 0.02 / 0.02 MB |
| Layout, skylines, display list, SVG string | 11.0 / 10.5 MB | 0.64 / 0.34 MB |
| Atoms (per-measure/per-event POD) | 1.47 / 1.23 MB | 0.69 / 0.63 MB |

Top 10 inclusive allocation frames on the private score (shares overlap; the
deepest useful frame is named): `read4xx::read_with_style` 57.7 MB,
`read::read_score` 45.5 MB, `hooks::read_musescore` 34.1 MB,
`read4xx::dom::parse` 31.8 MB, `Reader::read_score` 25.9 MB,
`Engraver::new` 23.6 MB, `build_page_with_skylines` 22.3 MB,
`Reader::read_voice` 20.9 MB, `dom::escape_into` 20.9 MB (51 753 blocks,
the intermediate XML text), `Reader::read_voice` (second site) 13.7 MB. On
`moonlight`: `read302::read_with_origin` 5.68 MB,
`read302::xml::parse` 4.33 MB, `xml::push_text` 4.25 MB (10 217 blocks),
`measure::read_measure` 1.26 MB, `MeasureAtom::with_system_header` /
`Segment::new` 0.69 MB. Interpretation: reading the source dominates, then
the model; the engraver itself is ≤ 11 MB even on the 88-page score, and no
glyph data is allocated per page.

### 20.4 ARMv7 build and size

Cross-compilation to `armv7-unknown-linux-gnueabihf` **works** with the
std artifact present in `/nix/store` (1.100.0) and the Nixpkgs GCC wrapper as
linker:

```sh
CC=/nix/store/…/armv7l-unknown-linux-gnueabihf-gcc \
CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=$CC \
RUSTFLAGS='--sysroot=/nix/store/…-rust-std-1.100.0-armv7-unknown-linux-gnueabihf' \
  cargo build --target armv7-unknown-linux-gnueabihf \
  -p scoretk-engrave -p scoretk-render-svg
```

Both engraving crates compile for that target. The whole CLI in the stripped
`dist` profile builds as a 32-bit ARM ELF of **5 217 408 B** (5.0 MiB),
including all twelve conversion directions; this is above the 1.5 MiB
library budget of §1, which applies to the engraving library/FFI, not to the
kitchen-sink CLI. No Kobo device measurement was made: the numbers above are
host measurements. The QEMU batches of §9.3 measured 35–75× slower than the
host for a raster-only loop; on real ARMv7 hardware a conservative
extrapolation factor of **10×** is used here (it is an estimate, not a device
measurement). The private score's 1.06 s host pass would therefore take
roughly 10 s for page 1 on the Kobo, and its 63 MB peak leaves no margin in
249 MB shared with KOReader.

### 20.5 Proposed budget and rule for agents

Target budgets for the 1.7 MB / 88-page class of score, to be verified on the
device (P7 gate), **not met by the current baseline**:

| Metric | Budget | Current host figure |
| --- | --- | --- |
| Peak heap, full engraving of a page | ≤ 32 MB | baseline 63.1 MB → **29.5 MB** after §15.6 (private); 6.8 MB (moonlight) |
| Time to first page on ARMv7 | ≤ 1 s | baseline ≈ 10 s → ≈ 8 s extrapolated (0.79 s × 10); **not met** |
| Heap of one engraved page once the score is loaded | ≤ 2 MB | ≤ 11 MB today (with the whole score live) |

**Rule for agents.** Port algorithms faithfully, choose compact data: no
per-element boxing when a `Vec` of POD works, layout system by system and
emit pages incrementally where upstream allows, only used glyphs. Measure
with `scripts/measure-engrave-memory.sh` before and after any change
claiming a memory gain; check SVG byte identity on the oracle corpus and the
private score. Leads, with ceilings from §20.3 and `performance.md` §4/6:
(a) stream/parse the 4.x DOM instead of materialising it (up to 21 MB, the
largest single item); (b) share text bytes instead of copying per XML node
(20.9 MB cumulative on the private score; 3.x `push_text` is the known lead
6 there); (c) drop the pivot model before engraving page > 0 (12 MB);
(d) re-use the per-page display-list and skyline arenas (10 MB). Leads (a)
and (b) are reader work; (c) and (d) touch the engraver but not the layout
algorithms.

### 20.6 Quick wins applied (2026-11-27)

Two reader-side changes clear the ≥ 10 % peak-heap gate; neither touches a
layout file. Each was measured with the script above and checked byte for byte
against the same 121 SVGs (all engraved pages of the 56 corpus files plus the
6 pages of the private score): SHA-256 manifests identical after each commit.

| Step | Peak heap | Allocations | Max RSS | Wall (private) |
| --- | ---: | ---: | ---: | ---: |
| Baseline (§15.2) | 63.06 MB | 577 053 | 76.1 MB | 1.06 s |
| (b) `read4xx::dom`: rebuild `raw_inner` only for elements whose mixed content is read as markup (`text`, `beginText`, `endText`, `name`, `shortName`, `longName`) and their descendants | 41.79 MB (−33.7 %) | 349 998 (−39.3 %) | 58.7 MB | 0.85 s |
| (b′) `resolve_ties`: precompute the compact `Vec<Tick>` of measure starts instead of cloning the whole `Vec<Measure>` (12.6 MB in one call) | **29.47 MB (−29.5 %)** | 336 812 (−3.8 %) | 46.7 MB | 0.79 s |

The corpus numbers are unchanged (dhat 6.77 MB peak on `moonlight` before and
after), because the 4.x-only paths are not exercised by the bare `.mscx`
files. The heap budget of §15.5 is now met on the host for the private score;
the time budget is not (≈ 8 s extrapolated for page 1), and the next step is
lead (a): incremental parsing/engraving rather than a whole-score pass.

## 21. Review of the private page: hyphens vs melisma extenders (2026-11-27)

The page still showed extender lines between syllables ("Mi nuit——",
"chré tien——", "ef fa cer——"). `LyricsLayout::createOrRemoveLyricsLine`
(`lyricslayout.cpp:383-504`) creates **one** `LyricsLine` per syllable when
the syllable has melisma ticks *or* is `Begin`/`Middle`, and
`LyricsLine::isEndMelisma()` (`dom/lyricsline.h:144`) is
`lyrics()->ticks().isNotZero()`: a syllable with `<ticks>`/`<ticks_f>` gets
the continuous extender of `layoutMelismaLine` (`lyricslayout.cpp:228-259`),
all other `Begin`/`Middle` syllables get the short dashes of `layoutDashes`
(`:261-360`). The 4.x reader already keeps both the `<syllabic>` type and the
`<ticks_f>` duration (`read4xx::read_lyrics`); the engraver collapsed them
into one rule and drew a dash *and* an extender for a melisma.

`plan_lyric_lines` now ports that split. A nonzero melisma draws one
continuous line from the syllable's right edge plus `lyricsMelismaPad` to its
`endTick`, and suppresses the hyphen; a dash-only `Begin`/`Middle` syllable
draws `ceil(gap / lyricsDashMaxDistance)` dashes (one per 16 sp, half gaps at
both ends), each capped at `lyricsDashMaxLength` and widened to
`lyricsDashMinLength` when `lyricsDashForce` is set (`:310-359`). A dash with
no following same-verse syllable emits no line (`:283-285`). The seven
`lyricsDash*`/`lyricsMelismaPad` style keys are read from `score_style.mss`
into `StyleSettings` and `PageSpec`, so the private score's 0.8 sp maximum
dash length and 0.15 sp dash thickness are used instead of the 4.7.4 defaults
(`styledef.cpp:80-83,89-91`). The atom pass treats `<ticks>` of zero as no
melisma, as `isEndMelisma()` does. Unit tests pin dash versus extender, the
melisma-overrides-hyphen rule, the style dash length and the dash count over a
34 sp gap.

| Metric (`oracle-eng9-review-base` → `oracle-eng9-review-step1`) | Before | After |
| --- | --- | ---: |
| Failures / elements | 0 / 2 209 | 0 / 2 209 |
| Within 0.1 sp, both axes | 0.1698 (375/2 209) | 0.1698 (375/2 209) |
| Measure widths / system breaks | 92/394 / 36/56 | 92/394 / 36/56 |
| Stem-length agreement | 0.6082 (684) | 0.6082 (684) |
| Overlapping lyric syllables, same verse | 0 / 0 | 0 / 0 |

Lyrics lines are not among the oracle's extracted elements, so the unchanged
figures are expected. On the private page page 1 (`engrave-img/eng9/step1.png`,
rendered from this tree) the hyphens are now 0.8 sp dashes exactly where the
4.7.4 page has them, and the long lines remain only on syllables carrying
`<ticks_f>` ("nuit", "tien", "Ou", "cer"), as in the reference.

## 22. Review of the private page: spacing between words (2026-11-27)

The page still read "latacheo ri gi nel" and "etdesonPère": the syllable
shapes were separated only by the 0.1 sp absolute floor of
`HorizontalSpacing::minHorizontalDistance` (`horizontalspacing.cpp:1218`)
because the private score sets `lyricsMinDistance` to 0, so the lyric row of
the padding table is zero (`paddingtable.cpp:207-209`). Per syllable pair on
page 1, our median gap was 1.74 sp against 2.61 sp in the 4.7.4 SVG, with
**14** pairs below 0.5 sp against **3** in the reference (the reference's
smallest gaps are 0.13–0.6 sp, mostly occupied by a hyphen inside a word).

`lyric_pair_distance` now applies `computeLyricsPadding`'s dash room
(`horizontalspacing.cpp:1594-1601`) to `Begin`/`Middle` syllables and, at a
word boundary (`Single`/`End`), reserves at least `LYRIC_WORD_SPACE` = 0.6 sp
above `lyricsMinDistance` and the 0.1 sp floor. That value is the smallest
word gap measured on the 4.7.4 private page, not a new style knob. The demand
still goes into the preceding segment's base (§19), so the shared system grid
separates the two syllables instead of widening only the bar line. Unit tests
pin the word, dash and style-padding cases and the segment demand.

| Metric (`oracle-eng9-review-step1` → `oracle-eng9-review-step2`) | Before | After |
| --- | ---: | ---: |
| Failures / elements | 0 / 2 209 | 0 / 2 209 |
| Within 0.1 sp, both axes | 0.1698 (375/2 209) | 0.1711 (378/2 209) |
| Measure widths within 0.1 sp | 92/394 | 90/394 |
| Measure width mean abs. error, sp | 9.779 | 9.661 |
| Note x within 0.1 sp | 174/2 209 | 173/2 209 |
| Note x mean abs. error, sp | 2.573 | 2.493 |
| Files with identical system breaks | 36/56 | 36/56 |
| Files with the oracle page count | 54/56 | 54/56 |
| Stem-length agreement | 0.6082 (684) | 0.6082 (684) |
| Private page 1: syllable pairs < 0.5 sp (4.7.4 / ours) | 3 / 14 | 3 / 8 |
| Private page 1: median syllable gap, sp (4.7.4 / ours) | 2.61 / 1.74 | 2.61 / 2.65 |

The two lost measure widths are reported, not excluded. On the private page
(`engrave-img/eng9/step2.png`, the whole page inspected) "la tache",
"o-ri-gi-nel" and "et de son" now have visible spaces, while the hyphens of
"Mi-nuit" and "l'heu-re" still occupy their intra-word gaps.

## 23. Review of the private page: measure rests after the header (2026-11-27)

In measure 1 the whole-measure rests were drawn at their segment x, which
sits immediately after the clef/key/12-8 header, so the T B rest touched the
time signature and the S A dotted whole rest sat on it.
`MeasureLayout::getMeasureStartEndPos` (`measurelayout.cpp:1396-1443`) instead
takes `x1` as the largest `s1->x() + s1->minRight()` of the visible segments
before the chord/rest segment and `x2` as the measure end (or the next header
segment); `layoutMeasureElements` centres the rest in that free space
(`:1473-1482, :1501-1514`). `place_measure` now computes the header's right
edge from the clef/key/time events and passes it together with the measure end
to `place_event`; a full-measure rest is centred in `[header_end, measure_end]`,
while the MMRest branch keeps upstream's header exception
(`:1435-1440`). The S A rest is written as a dotted whole in 12/8: its
duration fills the measure and the 4.7.4 SVG centres it too, so the atom pass
treats a measure-filling rest like `<durationType>measure</durationType>`.
Unit tests pin the headerless centre, the 12/8 header case and the dotted
whole atom.

| Metric (`oracle-eng9-review-step2` → `oracle-eng9-review-step3`) | Before | After |
| --- | --- | ---: |
| Failures / elements | 0 / 2 209 | 0 / 2 209 |
| Within 0.1 sp, both axes | 0.1711 (378/2 209) | 0.1711 (378/2 209) |
| Measure widths / system breaks | 90/394 / 36/56 | 90/394 / 36/56 |
| Files with the oracle page count | 54/56 | 54/56 |
| Stem-length agreement | 0.6082 (684) | 0.6082 (684) |
| Private page 1, rest x sp (S A / T B) | 30.6 / 30.7 | **46.3 / 46.3** |

Rests are not extracted by the geometric oracle, so its figures are unchanged
by construction. On the private page (`engrave-img/eng9/after.png`, the whole
page inspected) both measure-1 rests now sit in the middle of their measure,
clear of the 12/8 signature, as in the 4.7.4 reference.

## 24. Review of the private page 2: lyric collision at the source (2026-11-27)

Page 2 still showed "le Ré dem pteur" with the "é" of "Ré" on top of the "d"
of "dem" (m.22, 26) and "Noël" without a visible hyphen (m.20–21, 25). Two
root causes were measured on the SVG, not guessed from the picture.

`HorizontalSpacing` splits a segment's width into a spring and a fixed
`widthOffset` (`horizontalspacing.cpp:1073-1082`); `stretchSegmentsToWidth`
scales only `width - widthOffset` (`:1087-1111`). §19 put the lyric padding
into `segment_bases`, i.e. into the spring, so a system justification force
below 1 shrank the reserved gap again: the "Ré"→"dem" centre distance was
3.7 sp where `computeLyricsPadding` (`:1583-1601`) demands 5.7 sp. `MeasureAtom`
now carries `segment_offsets`, the fixed part of each segment's distance to
the next one, and `shared_measures` adds it back **after** scaling. The fixed
budget of `stretch_budget` includes it, so the system's total width is still
the justification target; only the distribution between spring and padding
changed. A unit test pins the offset to `needed - plain_gap`.

The hyphen was planned from `text_run_width` (an advance) while the glyphs
are centred by the same advance, but `place_lyrics` added `SP / 2` a second
time on top of the caller's `event_x + SP / 2`. Every syllable therefore sat
half a space right of the segment that `plan_lyric_lines` used: "No-ël"'s dash
landed on the "o". The page pass now hands the plain segment x to
`place_lyrics`; the half-space centring happens once, as in
`LyricsLayout::setDefaultPositions`. Dash ends additionally use
`text_ink_bounds` (Edwin glyph boxes, 250 units per sp) for
`LyricsLayout::lyricsLineStartX`/`EndX` (`lyricslayout.cpp:745-830`), so the
dash starts at the syllable's shape edge rather than at its advance edge.
A unit test checks that the dash lies between the rendered ink of "No" and
"ël".

| Metric (`oracle-eng9-review-step3` → `oracle-eng9-p2-step6b`) | Before | After |
| --- | ---: | ---: |
| Failures / elements | 0 / 2 209 | 0 / 2 209 |
| Within 0.1 sp, both axes | 0.1711 (378/2 209) | **0.1747** (386/2 209) |
| Measure widths within 0.1 sp | 90/394 | **91/394** |
| Note x within 0.1 sp | 173/2 209 | **178/2 209** |
| Stem-length agreement | 0.6082 (684) | 0.6082 (684) |
| Files with identical system breaks | 36/56 | **35/56** |
| Files with the oracle page count | 54/56 | 54/56 |

The one lost system-break file is `testHairpinDynamics`: its per-page
`breaks_equal` stays 1, but the count of bar lines per system in the SVG
changes after the horizontal redistribution, so the element-level metric
drops it. It is reported, not excluded. On the private page both renders
(`engrave-img/eng9/step7-p2.png`, `step7-p1.png`, whole pages inspected) keep
the same three systems per page: the collisions are gone and "No-ël" now has
its hyphen, while page 1 is unchanged.
