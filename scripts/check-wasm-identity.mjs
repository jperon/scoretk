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

// Byte-identity check between two builds of the WebAssembly package.
//
//   nix build .#wasm && cp -rL result /tmp/scoretk-old
//   # ... change the build ...
//   nix build .#wasm
//   node scripts/check-wasm-identity.mjs /tmp/scoretk-old result
//
// Both packages are loaded in the same Node process (ESM variant, initSync with
// a precompiled module) and every conversion of a corpus sample is compared
// byte for byte, output by output. A single difference makes the script fail.
//
// Covered: the ten import/export directions (`detect` then `convert` from each
// readable format to each writable one), the `.mscz`/`.mxl` containers,
// `convertWithTimeline` (MusicXML, MIDI and the time map), `formats()`,
// `version()` and the error messages.
//
// Environment: SCORETK_CORPUS (default: tests/corpus of this repository).

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const corpus = process.env.SCORETK_CORPUS ?? join(root, 'tests', 'corpus');

const [oldDir, newDir] = process.argv.slice(2);
if (!oldDir || !newDir) {
  console.error('usage: node scripts/check-wasm-identity.mjs <old-pkg-dir> <new-pkg-dir>');
  process.exit(2);
}

async function load(pkgDir) {
  const dir = join(pkgDir, 'esm');
  const wasm = await import(pathToFileURL(join(dir, 'scoretk_wasm.js')).href);
  wasm.initSync({
    module: new WebAssembly.Module(readFileSync(join(dir, 'scoretk_wasm_bg.wasm'))),
  });
  const packageVersion = JSON.parse(readFileSync(join(pkgDir, 'package.json'), 'utf8')).version;
  return { wasm, packageVersion };
}

const read = (path) => new Uint8Array(readFileSync(join(corpus, path)));
const cmp = (label, a, b) => {
  if (!Buffer.from(a).equals(Buffer.from(b))) {
    assert.fail(`${label}: outputs differ (${a.length} vs ${b.length} bytes)`);
  }
};

const old = await load(oldDir);
const now = await load(newDir);
const oldWasm = old.wasm;
const nowWasm = now.wasm;

// Package-level surface.
assert.equal(now.wasm.version(), old.wasm.version(), 'version() differs');
assert.equal(now.wasm.version(), now.packageVersion, 'version() must match package.json');
assert.equal(
  JSON.stringify(now.wasm.formats()),
  JSON.stringify(old.wasm.formats()),
  'formats() differs',
);

// Corpus samples: one per readable format of the facade, both MuseScore
// versions, plus repeats (for the timeline).
const samples = {
  mscx: [
    ['v4/dynamics.mscx', read(join('v4', 'dynamics.mscx'))],
    ['v3/voices.mscx', read(join('v3', 'voices.mscx'))],
    ['v4/repeat65.mscx', read(join('v4', 'repeat65.mscx'))],
  ],
  musicxml: [
    ['v4/dynamics.ref.musicxml', read(join('v4', 'dynamics.ref.musicxml'))],
    ['v3/voices.ref.musicxml', read(join('v3', 'voices.ref.musicxml'))],
  ],
  midi: [['midi/v4/dynamics.ref.mid', read(join('midi', 'v4', 'dynamics.ref.mid'))]],
  ly: [['ly/v4_dynamics.ly', read(join('ly', 'v4_dynamics.ly'))]],
  abc: [['abc/v4_dynamics.abc', read(join('abc', 'v4_dynamics.abc'))]],
};

// Containers are built once with the old package (identical bytes fed to both):
// `.mscz` from a MuseScore score, `.mxl` from a MusicXML score.
samples.mscz = [['v4/dynamics.mscz', oldWasm.convert(samples.mscx[0][1], { to: 'mscz' })]];
samples.mxl = [['v4/dynamics.mxl', oldWasm.convert(samples.musicxml[0][1], { to: 'mxl' })]];

// From-format per sample, then the ten directions are covered by converting
// each input to each writable format.
const fromBySample = {
  mscx: 'mscx',
  mscz: 'mscz',
  musicxml: 'musicxml',
  mxl: 'mxl',
  midi: 'mid',
  ly: 'ly',
  abc: 'abc',
};

const targets = ['mscx', 'mscz', 'musicxml', 'mxl', 'midi', 'ly', 'abc'];

const family = {
  mscx: 'musescore',
  mscz: 'musescore',
  musicxml: 'musicxml',
  mxl: 'musicxml',
  mid: 'midi',
  midi: 'midi',
  ly: 'lilypond',
  abc: 'abc',
};

let conversions = 0;
const directions = new Set();

for (const [fromFormat, files] of Object.entries(samples)) {
  for (const [name, input] of files) {
    assert.equal(nowWasm.detect(input), oldWasm.detect(input), `detect(${name}) differs`);
    // Content detection without `from` as well as the explicit from-format.
    for (const to of targets) {
      const options = { to, encodingDate: '2026-01-01', software: 'scoretk-identity' };
      const label = `${name} (${fromBySample[fromFormat]} -> ${to})`;
      let oldOut;
      let oldErr;
      try {
        oldOut = oldWasm.convert(input, options);
      } catch (error) {
        oldErr = error;
      }
      if (oldErr) {
        assert.throws(
          () => nowWasm.convert(input, options),
          (error) => error.message === oldErr.message,
          `${label}: error message differs`,
        );
        directions.add(`${family[fromBySample[fromFormat]]}-import`);
        directions.add(`${family[to]}-export`);
        continue;
      }
      const newOut = nowWasm.convert(input, options);
      cmp(label, oldOut, newOut);
      directions.add(`${family[fromBySample[fromFormat]]}-import`);
      directions.add(`${family[to]}-export`);
      conversions += 1;

      const explicit = nowWasm.convert(input, { ...options, from: fromBySample[fromFormat] });
      cmp(`${label} (explicit from)`, oldOut, explicit);
    }
  }
}

// Timeline: MusicXML with stable ids, MIDI and the time map.
for (const name of ['v4/repeat65.mscx', 'v4/dynamics.mscx']) {
  const input = read(name);
  for (const options of [{}, { expandRepeats: false }]) {
    const a = oldWasm.convertWithTimeline(input, options);
    const b = nowWasm.convertWithTimeline(input, options);
    cmp(`timeline ${name} musicxml`, a.musicxml, b.musicxml);
    cmp(`timeline ${name} midi`, a.midi, b.midi);
    assert.equal(
      JSON.stringify(b.timeline),
      JSON.stringify(a.timeline),
      `timeline ${name} map differs`,
    );
  }
}

// User-visible errors.
const unknown = new Uint8Array([0xde, 0xad, 0xbe, 0xef]);
const errorCases = [
  () => nowWasm.convert(samples.mscx[0][1], { to: 'pdf' }),
  () => nowWasm.convert(samples.mscx[0][1], { from: 'pdf' }),
  () => nowWasm.convert(unknown),
];
const oldErrorCases = [
  () => oldWasm.convert(samples.mscx[0][1], { to: 'pdf' }),
  () => oldWasm.convert(samples.mscx[0][1], { from: 'pdf' }),
  () => oldWasm.convert(unknown),
];
for (let index = 0; index < errorCases.length; index += 1) {
  let oldMessage;
  let newMessage;
  try {
    oldErrorCases[index]();
  } catch (error) {
    oldMessage = error.message;
  }
  try {
    errorCases[index]();
  } catch (error) {
    newMessage = error.message;
  }
  assert.equal(newMessage, oldMessage, `error case ${index + 1} message differs`);
}

const expected = [
  'musescore-import',
  'musescore-export',
  'musicxml-import',
  'musicxml-export',
  'midi-import',
  'midi-export',
  'lilypond-import',
  'lilypond-export',
  'abc-import',
  'abc-export',
];
for (const direction of expected) {
  assert.ok(directions.has(direction), `direction ${direction} not covered`);
}

console.log(
  `check-wasm-identity: ok (${conversions} conversions byte-identical, ` +
    `${expected.length} directions; old ${oldDir}, new ${newDir})`,
);
