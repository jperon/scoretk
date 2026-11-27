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
