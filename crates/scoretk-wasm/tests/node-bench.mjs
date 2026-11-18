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

// Mesure du temps d'exécution du paquet WebAssembly de la façade scoretk sous
// Node. Outil de mesure uniquement : aucune logique de conversion.
//
//   nix build .#wasm
//   nix shell nixpkgs#nodejs -c node crates/scoretk-wasm/tests/node-bench.mjs ./result
//   nix shell nixpkgs#nodejs -c node crates/scoretk-wasm/tests/node-bench.mjs ./result --json
//
// Le chemin passé en argument est le dossier du paquet produit par `nix build`
// (celui qui contient `esm/`). Le corpus est `tests/corpus/{v3,v4}/*.mscx`,
// sauf si SCORETK_CORPUS est défini. Pour chaque fichier, après 3 itérations
// de chauffe, on relève la médiane de 15 itérations de chacune des trois
// opérations : convert vers musicxml, convert vers midi, convertWithTimeline.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const WARMUP = 3;
const ITERATIONS = 15;

const args = process.argv.slice(2);
const asJson = args.includes('--json');
const paths = args.filter((arg) => arg !== '--json');
if (paths.length !== 1) {
  console.error('usage : node node-bench.mjs <dossier-du-paquet> [--json]');
  process.exit(2);
}

const root = resolve(new URL('../../../', import.meta.url).pathname);
const pkgDir = resolve(process.cwd(), paths[0]);
const corpus = resolve(process.cwd(), process.env.SCORETK_CORPUS ?? join(root, 'tests', 'corpus'));
const esmDir = join(pkgDir, 'esm');

// 1. Instanciation du module (une seule fois, hors des mesures par fichier).
const memBefore = process.memoryUsage();
const wasmBytes = readFileSync(join(esmDir, 'scoretk_wasm_bg.wasm'));
const compileStart = performance.now();
const module = new WebAssembly.Module(wasmBytes);
const compileMs = performance.now() - compileStart;

const glue = await import(pathToFileURL(join(esmDir, 'scoretk_wasm.js')).href);
const readyStart = performance.now();
const instance = glue.initSync({ module });
const readyMs = performance.now() - readyStart;
const memAfterInit = process.memoryUsage();

// Le glue wasm-bindgen expose les exports de l'instance via initSync ; la
// mémoire linéaire n'est pas forcément exportée (elle est alors "null").
let wasmMemory = null;
if (instance && typeof instance === 'object') {
  for (const value of Object.values(instance)) {
    if (value instanceof WebAssembly.Memory) {
      wasmMemory = value;
      break;
    }
  }
}

const files = [];
for (const version of ['v3', 'v4']) {
  const dir = join(corpus, version);
  for (const name of readdirSync(dir).sort()) {
    if (!name.endsWith('.mscx')) continue;
    const path = join(dir, name);
    files.push({ version, name, path, bytes: statSync(path).size });
  }
}
if (files.length === 0) {
  console.error(`aucun .mscx trouvé dans ${corpus}/v3 et ${corpus}/v4`);
  process.exit(2);
}

function median(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

// Conserve une trace des sorties pour éviter toute élimination des appels.
let sink = 0;
function measure(fn) {
  for (let index = 0; index < WARMUP; index += 1) sink += fn().length;
  const samples = [];
  for (let index = 0; index < ITERATIONS; index += 1) {
    const start = performance.now();
    sink += fn().length;
    samples.push(performance.now() - start);
  }
  return { median: median(samples), samples };
}

const results = [];
for (const file of files) {
  const bytes = new Uint8Array(readFileSync(file.path));
  const musicxml = measure(() => glue.convert(bytes, { to: 'musicxml' }));
  const midi = measure(() => glue.convert(bytes, { to: 'midi' }));
  const timeline = measure(() => glue.convertWithTimeline(bytes));
  results.push({
    file: `${file.version}/${file.name}`,
    bytes: file.bytes,
    version: file.version,
    musicxml,
    midi,
    timeline,
  });
}
const memAfterBench = process.memoryUsage();
const totals = {
  musicxml: results.reduce((sum, row) => sum + row.musicxml.median, 0),
  midi: results.reduce((sum, row) => sum + row.midi.median, 0),
  timeline: results.reduce((sum, row) => sum + row.timeline.median, 0),
};

const report = {
  package: pkgDir,
  corpus,
  node: process.version,
  wasmBytes: wasmBytes.length,
  warmup: WARMUP,
  iterations: ITERATIONS,
  init: {
    compileMs,
    readyMs,
    memory: {
      before: memBefore,
      afterInit: memAfterInit,
      afterBench: memAfterBench,
      wasmLinearMemoryBytes: wasmMemory ? wasmMemory.buffer.byteLength : null,
    },
  },
  results,
  totals,
  sink,
};

if (asJson) {
  console.log(JSON.stringify(report, null, 2));
  process.exit(0);
}

function ms(value) {
  return value.toFixed(3).padStart(9);
}

console.log(`node-bench: ${results.length} fichiers, ${WARMUP} chauffes, ${ITERATIONS} itérations, Node ${process.version}`);
console.log(`paquet : ${pkgDir}   corpus : ${corpus}`);
console.log(
  `init : compilation ${compileMs.toFixed(3)} ms, initSync (ready) ${readyMs.toFixed(3)} ms, ` +
    `.wasm ${wasmBytes.length} octets, mémoire linéaire wasm ` +
    `${wasmMemory ? wasmMemory.buffer.byteLength : 'non exposée'} octets`,
);
console.log(
  `mémoire Node (octets) : RSS ${memBefore.rss} → ${memAfterInit.rss} (init) → ${memAfterBench.rss} (fin), ` +
    `heapUsed ${memBefore.heapUsed} → ${memAfterInit.heapUsed} → ${memAfterBench.heapUsed}, ` +
    `arrayBuffers ${memBefore.arrayBuffers} → ${memAfterInit.arrayBuffers} → ${memAfterBench.arrayBuffers}`,
);
console.log();
console.log('fichier                          octets   musicxml      midi  timeline');
console.log('─'.repeat(76));
for (const row of results) {
  console.log(
    `${row.file.padEnd(32)} ${String(row.bytes).padStart(7)} ${ms(row.musicxml.median)} ${ms(row.midi.median)} ${ms(row.timeline.median)}`,
  );
}
console.log('─'.repeat(76));
console.log(`TOTAL (somme des médianes)               ${ms(totals.musicxml)} ${ms(totals.midi)} ${ms(totals.timeline)}`);

const slowest = [...results]
  .map((row) => ({ file: row.file, total: row.musicxml.median + row.midi.median + row.timeline.median }))
  .sort((a, b) => b.total - a.total)
  .slice(0, 5);
console.log();
console.log('5 fichiers les plus lents (somme des trois médianes) :');
for (const row of slowest) console.log(`  ${row.file.padEnd(32)} ${row.total.toFixed(3)} ms`);
