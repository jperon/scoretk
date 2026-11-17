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

// Test de fumée du paquet WebAssembly de la façade scoretk, exécuté par Node.
//
//   nix build .#wasm
//   node crates/scoretk-wasm/tests/node-smoke.mjs
//
// Le test charge la variante ESM depuis `result/` (ou depuis SCORETK_PKG_DIR,
// utilisé par le check de flake) et couvre les sept formats sur le corpus.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const pkgDir = process.env.SCORETK_PKG_DIR ?? join(root, 'result');
const corpus = process.env.SCORETK_CORPUS ?? join(root, 'tests', 'corpus');

const esmDir = join(pkgDir, 'esm');
const moduleUrl = pathToFileURL(join(esmDir, 'scoretk_wasm.js'));

// Node ne sait pas charger un .wasm comme module ESM : on compile les octets
// et on initialise le binding de façon synchrone, sans fetch.
const wasm = await import(moduleUrl.href);
wasm.initSync({
  module: new WebAssembly.Module(readFileSync(join(esmDir, 'scoretk_wasm_bg.wasm'))),
});

const decoder = new TextDecoder();
const read = (path) => new Uint8Array(readFileSync(join(corpus, path)));

function text(bytes) {
  return decoder.decode(bytes);
}

function startsWith(bytes, prefix) {
  return String.fromCharCode(...bytes.slice(0, prefix.length)) === prefix;
}

// 1. version() suit package.json.
const identity = JSON.parse(readFileSync(join(pkgDir, 'package.json'), 'utf8'));
assert.equal(wasm.version(), identity.version, 'version() doit suivre package.json');

// 2. formats() décrit les sept formats, tous lisibles et écrivables ici.
const formats = wasm.formats();
assert.equal(formats.length, 7, 'formats() doit décrire sept formats');
const names = new Set(formats.map((format) => format.name));
for (const name of ['mscz', 'mscx', 'musicxml', 'mxl', 'ly', 'abc', 'midi']) {
  assert.ok(names.has(name), `formats() doit contenir ${name}`);
}
for (const format of formats) {
  assert.equal(format.read, true, `${format.name} doit être lisible`);
  assert.equal(format.write, true, `${format.name} doit être écrivable`);
}
const musicxmlFormat = formats.find((format) => format.name === 'musicxml');
assert.ok(musicxmlFormat.extensions.includes('musicxml'));
assert.ok(musicxmlFormat.extensions.includes('xml'));

// 3. Détection d'un .mscx.
const mscx = read(join('v4', 'dynamics.mscx'));
assert.equal(wasm.detect(mscx), 'mscx');

// 4. Conversion par défaut (mscx -> musicxml), options d'encodage.
const xml = wasm.convert(mscx, { encodingDate: '2026-01-01', software: 'scoretk-test' });
assert.ok(xml instanceof Uint8Array, 'convert doit renvoyer un Uint8Array');
const xmlText = text(xml);
assert.ok(xmlText.startsWith('<?xml'), 'la sortie MusicXML doit commencer par <?xml');
assert.ok(xmlText.includes('<score-partwise'), 'la sortie doit être du MusicXML partwise');
assert.ok(xmlText.includes('2026-01-01'), 'encodingDate doit apparaître dans la sortie');
assert.ok(xmlText.includes('scoretk-test'), 'software doit apparaître dans la sortie');

// 5. Écriture MuseScore.
const mscxOut = text(wasm.convert(mscx, { from: 'mscx', to: 'mscx' }));
assert.ok(mscxOut.includes('<museScore'), 'la sortie .mscx doit contenir <museScore');

// 6. .mscz produit puis relu, puis conversion .mscz -> MIDI.
const mscz = wasm.convert(mscx, { to: 'mscz' });
assert.equal(wasm.detect(mscz), 'mscz');
const midiFromMscz = wasm.convert(mscz, { to: 'midi' });
assert.ok(startsWith(midiFromMscz, 'MThd'), 'la sortie MIDI doit commencer par MThd');

// 7-9. MusicXML de référence -> LilyPond, ABC et MIDI.
const musicxml = read(join('v4', 'dynamics.ref.musicxml'));
assert.equal(wasm.detect(musicxml), 'musicxml');
const ly = text(wasm.convert(musicxml, { to: 'ly' }));
assert.ok(ly.includes('\\version'), 'la sortie LilyPond doit contenir \\version');
assert.ok(ly.includes('\\score'), 'la sortie LilyPond doit contenir \\score');
const abc = text(wasm.convert(musicxml, { to: 'abc' }));
assert.ok(abc.includes('X:'), 'la sortie ABC doit contenir X:');
assert.ok(abc.includes('K:'), 'la sortie ABC doit contenir K:');
assert.ok(startsWith(wasm.convert(musicxml, { to: 'midi' }), 'MThd'));

// 10. Contenu inconnu : detect renvoie null.
const unknown = new Uint8Array([0xde, 0xad, 0xbe, 0xef]);
assert.equal(wasm.detect(unknown), null);

// 11-12. Formats inconnus : erreur explicite.
assert.throws(() => wasm.convert(mscx, { to: 'pdf' }), /format inconnu/);
assert.throws(() => wasm.convert(mscx, { from: 'pdf' }), /format inconnu/);

// 13. Entrée non reconnue sans `from` : erreur explicite.
assert.throws(() => wasm.convert(unknown), /non reconnu/);

// 14. ABC -> MusicXML.
const abcInput = read(join('abc', 'v4_dynamics.abc'));
assert.equal(wasm.detect(abcInput), 'abc');
assert.ok(text(wasm.convert(abcInput, { to: 'musicxml' })).includes('<score-partwise'));

// 15. LilyPond -> MusicXML.
const lyInput = read(join('ly', 'v4_dynamics.ly'));
assert.equal(wasm.detect(lyInput), 'ly');
assert.ok(text(wasm.convert(lyInput, { to: 'musicxml' })).includes('<score-partwise'));

// 16. convertWithTimeline : MusicXML identifié, MIDI et carte des temps.
const repeat = read(join('v4', 'repeat65.mscx'));
const playback = wasm.convertWithTimeline(repeat);
assert.ok(playback && typeof playback === 'object', 'convertWithTimeline doit renvoyer un objet');
for (const key of ['musicxml', 'midi', 'timeline']) {
  assert.ok(key in playback, `convertWithTimeline doit renvoyer ${key}`);
}
assert.ok(playback.musicxml instanceof Uint8Array, 'musicxml doit être un Uint8Array');
assert.ok(playback.midi instanceof Uint8Array, 'midi doit être un Uint8Array');

// 17. Le MusicXML porte id ET xml:id, avec la même valeur.
const playbackXml = text(playback.musicxml);
assert.ok(
  playbackXml.includes('id="p0-m0"') && playbackXml.includes('xml:id="p0-m0"'),
  'le MusicXML doit porter id et xml:id de mesure',
);
assert.ok(
  /<measure[^>]*\sid="p0-m0"[^>]*\sxml:id="p0-m0"/.test(playbackXml),
  'la mesure doit porter id et xml:id de même valeur',
);

// 18. Le MIDI est un SMF et la timeline est remplie.
assert.ok(startsWith(playback.midi, 'MThd'), 'le MIDI de playback doit commencer par MThd');
const timeline = playback.timeline;
assert.ok(Array.isArray(timeline.measures), 'timeline.measures doit être un tableau');
assert.ok(timeline.measures.length > 0, 'timeline.measures doit être non vide');
assert.equal(timeline.measures[0].tick, 0, 'la première mesure doit commencer au tick 0');
assert.equal(timeline.measures[0].timeUs, 0, 'la première mesure doit commencer au temps 0');
assert.ok(
  timeline.measures.every((measure) => measure.ordinal === measure.measure),
  'ordinal doit être l\'indice 0-basé de la mesure (égal à measure)',
);

// 19. Ticks de mesures strictement croissants et mesures rejouées.
for (let index = 1; index < timeline.measures.length; index += 1) {
  assert.ok(
    timeline.measures[index - 1].tick < timeline.measures[index].tick,
    'les ticks de mesures doivent être strictement croissants',
  );
}
const seen = new Set();
let repeatedMeasure = false;
for (const measure of timeline.measures) {
  if (seen.has(measure.xmlId)) {
    repeatedMeasure = true;
  }
  seen.add(measure.xmlId);
}
assert.ok(repeatedMeasure, 'au moins une mesure doit être rejouée (reprises)');

// 20. Sans déroulement des reprises, il y a moins de mesures jouées.
const flat = wasm.convertWithTimeline(repeat, { expandRepeats: false });
assert.ok(
  flat.timeline.measures.length < timeline.measures.length,
  'expandRepeats: false doit réduire le nombre de mesures jouées',
);

// 21. Les identifiants de notes de la timeline existent dans le MusicXML.
assert.ok(timeline.notes.length > 0, 'timeline.notes doit être non vide');
for (const note of timeline.notes) {
  assert.ok(
    playbackXml.includes(`id="${note.xmlId}"`),
    `l'identifiant de note ${note.xmlId} doit exister dans le MusicXML`,
  );
}

console.log(
  `node-smoke: ok (${pkgDir}, format détecté ${wasm.detect(mscx)}, ` +
    `taille ${new Uint8Array(readFileSync(join(esmDir, 'scoretk_wasm_bg.wasm'))).length} octets, ` +
    `scoretk ${wasm.version()})`,
);
