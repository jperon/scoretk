/* SPDX-License-Identifier: GPL-3.0-only
 * Copyright (C) 2026 jperon
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 3 as published by
 * the Free Software Foundation.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see the LICENSE file at the root of this repository or
 * <https://www.gnu.org/licenses/gpl-3.0.html>.
 */

// Node test of the optional engraving module (feature `engrave`).
//
//   nix build .#wasm
//   node crates/scoretk-wasm/tests/node-engraving.mjs
//
// The main package is loaded too: the stable conversion contract must be
// untouched, and every stable note id of `convertWithTimeline` must resolve to
// an element of a rendered page. The classic wrapper is exercised in a stubbed
// Worker context (importScripts + fetch).

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import vm from 'node:vm';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const pkgDir = process.env.SCORETK_PKG_DIR ?? join(root, 'result');
const corpus = process.env.SCORETK_CORPUS ?? join(root, 'tests', 'corpus');
const esmDir = join(pkgDir, 'esm');
const classicDir = join(pkgDir, 'classic');

function load(esmDir, name) {
  const module = readFileSync(join(esmDir, `${name}_bg.wasm`));
  return {
    module: undefined,
    wasm: undefined,
    async init() {
      const wasm = await import(pathToFileURL(join(esmDir, `${name}.js`)).href);
      this.module = wasm;
      this.wasm = wasm.initSync({ module: new WebAssembly.Module(module) });
      return this;
    },
  };
}

const engraveModule = await load(esmDir, 'scoretk_engrave').init();
const mainModule = await load(esmDir, 'scoretk_wasm').init();
const engrave = engraveModule.module;
const main = mainModule.module;
const read = (path) => new Uint8Array(readFileSync(join(corpus, path)));

// 1. The main package is unchanged: no `engrave`, nine conversion formats.
const identity = JSON.parse(readFileSync(join(pkgDir, 'package.json'), 'utf8'));
assert.equal(main.version(), identity.version);
assert.equal(typeof main.engrave, 'undefined', 'main package must not expose engrave');
assert.equal(main.formats().length, 9);
assert.equal(engrave.version(), identity.version);
assert.equal(typeof engrave.engrave, 'function');
const engraveFormats = engrave.formats().map((format) => format.name);
assert.ok(engraveFormats.includes('svg'), 'the engraving module can write SVG');

// 2. Handle basics: structured SVG, compact layout JSON and stable ids.
const voiceswitching = read('v4/voiceswitching.mscx');
const handle = engrave.engrave(voiceswitching);
assert.ok(handle.pageCount() >= 1);
const svg = handle.renderPage(0);
assert.ok(svg.startsWith('<?xml') && svg.includes('<svg'), 'an SVG document is returned');
assert.ok(svg.includes('class="system"') && svg.includes('data-system='), 'systems are grouped');
assert.ok(svg.includes('class="measure"') && svg.includes('data-ordinal='), 'measures are grouped');
assert.ok(svg.includes('class="note"') && svg.includes('data-note='), 'notes are grouped');
const layout = JSON.parse(handle.layout(0));
assert.equal(layout.page, 0);
assert.equal(layout.unit, '1/1024 staff space');
assert.equal(layout.size.length, 2);
assert.ok(layout.systems.length > 0 && layout.measures.length > 0 && layout.notes.length > 0);
const allPages = JSON.parse(handle.layout());
assert.equal(allPages.length, handle.pageCount());
assert.deepEqual(allPages[0], layout);
for (const note of layout.notes) {
  assert.equal(typeof note.id, 'string');
  assert.equal(note.bbox.length, 4);
  const page = handle.pageOfElement(note.id);
  assert.ok(Number.isInteger(page) && page >= 0 && page < handle.pageCount());
}
assert.equal(handle.pageOfElement('does-not-exist'), null);
assert.throws(() => handle.renderPage(handle.pageCount()), /hors limites/);

// 3. Every timeline note id is drawn on some page, with the same spelling as
//    the unmodified conversion package.
for (const file of [
  'v4/voiceswitching.mscx',
  'v4/changevisibility.mscx',
  'v3/moonlight.mscx',
]) {
  const bytes = read(file);
  const ids = new Set(main.convertWithTimeline(bytes).timeline.notes.map((note) => note.xmlId));
  assert.ok(ids.size > 0, `${file}: timeline notes`);
  const drawn = new Set();
  const engraving = engrave.engrave(bytes);
  for (let page = 0; page < engraving.pageCount(); page += 1) {
    for (const match of engraving.renderPage(page).matchAll(/id="([^"]+)"/g)) {
      drawn.add(match[1]);
    }
  }
  const missing = [...ids].filter((id) => !drawn.has(id));
  assert.deepEqual(missing, [], `${file}: timeline ids missing from the SVG`);
  engraving.free();
}
handle.free();

// 4. Verovio-style options and relayout, including per-part staff sizes.
const verovioOptions = {
  pageWidth: 3000,
  pageHeight: 2000,
  scale: 40,
  lyricSize: 4.5,
  adjustPageHeight: true,
  condense: 'auto',
  systemMaxPerPage: 1,
  transpose: 0,
  partSizes: { 0: 70 },
};
const original = engrave.engrave(voiceswitching);
const originalPages = original.pageCount();
const originalSvg = original.renderPage(0);
const originalSize = JSON.parse(original.layout(0)).size;
original.relayout(verovioOptions);
assert.equal(original.pageCount(), 4, 'systemMaxPerPage: 1 puts one system per page');
assert.notDeepEqual(JSON.parse(original.layout(0)).size, originalSize);
assert.notEqual(original.renderPage(0), originalSvg);
const resized = original.renderPage(0);
original.relayout(); // no argument restores the options of `engrave`
assert.equal(original.pageCount(), originalPages);
assert.equal(original.renderPage(0), originalSvg);
assert.notEqual(resized, originalSvg);
original.free();

// 5. Linear memory of the largest score. Memory is reported (not a parity
//    assertion) so a Kobo/phone regression is visible; 64 MiB is a generous
//    ceiling for an engraving of this score.
const memory = () => engraveModule.wasm.memory.buffer.byteLength;
const moonlight = read('v3/moonlight.mscx');
const before = memory();
const big = engrave.engrave(moonlight);
const engravedSvg = big.renderPage(0);
const afterEngraveRender = memory();
big.relayout({ pageWidth: 2100, pageHeight: 2970, scale: 40, transpose: 2 });
big.renderPage(0);
const afterRelayout = memory();
console.log(
  `node-engraving: wasm memory ${before} B (init) → ${afterEngraveRender} B ` +
    `(moonlight engrave + render, ${big.pageCount()} pages, ${engravedSvg.length} B SVG) → ` +
    `${afterRelayout} B (relayout)`,
);
for (const bytes of [afterEngraveRender, afterRelayout]) {
  assert.ok(bytes > 0 && bytes < 64 * 1024 * 1024, `linear memory ${bytes} B`);
}
big.free();

// 6. The classic worker wrapper exposes `self.scoretk.engrave`.
const ctx = vm.createContext({
  self: {},
  console,
  WebAssembly,
  Response,
  URL,
  TextEncoder,
  TextDecoder,
  performance,
  fetch: async (url) =>
    new Response(readFileSync(new URL(url)), { headers: { 'Content-Type': 'application/wasm' } }),
  importScripts: (url) => vm.runInContext(readFileSync(new URL(url), 'utf8'), ctx),
});
ctx.self.importScripts = ctx.importScripts;
ctx.self.SCORETK_BASE_URL = pathToFileURL(`${classicDir}/`).href;
vm.runInContext(readFileSync(join(classicDir, 'scoretk-engrave.js'), 'utf8'), ctx);
const workerApi = ctx.self.scoretk;
assert.ok(workerApi && typeof workerApi.engrave === 'function');
await workerApi.ready;
const workerEngraving = workerApi.engrave(voiceswitching);
assert.equal(workerEngraving.pageCount(), 1);
assert.ok(workerEngraving.renderPage(0).includes('class="system"'));
workerEngraving.free();

console.log('node-engraving: ok');
