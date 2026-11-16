/* SPDX-License-Identifier: GPL-3.0-only
 * Copyright (C) 2026 les auteurs de mscz2xml
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

// Test de fumée du paquet WebAssembly, exécuté par Node (pas de navigateur).
//
//   nix build .#wasm
//   node crates/mscz2xml-wasm/tests/node-smoke.mjs
//
// Le test charge la variante ESM depuis `result/` (ou depuis MSCZ2XML_PKG_DIR,
// utilisé par le check de flake) et convertit un fichier du corpus.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const pkgDir = process.env.MSCZ2XML_PKG_DIR ?? join(root, 'result');
const corpus =
  process.env.MSCZ2XML_CORPUS ?? join(root, 'tests', 'corpus', 'v4', 'dynamics.mscx');

const esmDir = join(pkgDir, 'esm');
const moduleUrl = pathToFileURL(join(esmDir, 'mscz2xml_wasm.js'));

// Node ne sait pas charger un .wasm comme module ESM : on compile les octets
// et on initialise le binding de façon synchrone, sans fetch.
const wasm = await import(moduleUrl.href);
wasm.initSync({
  module: new WebAssembly.Module(readFileSync(join(esmDir, 'mscz2xml_wasm_bg.wasm'))),
});

const identity = JSON.parse(readFileSync(join(pkgDir, 'package.json'), 'utf8'));
assert.equal(wasm.version(), identity.version, 'version() doit suivre package.json');

const input = readFileSync(corpus);
const xml = wasm.convert(input, { encodingDate: '2026-01-01' });

assert.ok(xml.startsWith('<?xml'), 'la sortie doit commencer par <?xml');
assert.ok(xml.includes('<score-partwise'), 'la sortie doit être du MusicXML partwise');
assert.ok(xml.includes('2026-01-01'), 'encodingDate doit apparaître dans la sortie');

// Entrée invalide : exception JavaScript portant le message Rust.
assert.throws(
  () => wasm.convert(new Uint8Array([0xde, 0xad, 0xbe, 0xef])),
  (error) => error instanceof Error && error.message.length > 0,
  'une entrée invalide doit lever une Error',
);

// Les options absentes ou vides retombent sur les valeurs par défaut.
const defaultXml = wasm.convert(input);
assert.ok(defaultXml.includes('<score-partwise'));

console.log(
  `node-smoke: ok (${corpus}, ${xml.length} caractères de MusicXML, mscz2xml ${wasm.version()})`,
);
