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

// Démo de la variante « classic » (wasm-bindgen --target no-modules) dans un
// Web Worker : c'est le mode d'emploi de la PWA cantus, qui charge ses scripts
// en classique et appelle importScripts.
//
// Le worker est à côté de cette page ; les fichiers du paquet sont dans
// `result/classic/` à la racine du dépôt (voir `nix build .#wasm`).

importScripts('../../result/classic/scoretk.js');

self.scoretk.ready.then(
  () => self.postMessage({ type: 'ready', version: self.scoretk.version() }),
  (error) =>
    self.postMessage({ type: 'error', message: error instanceof Error ? error.message : String(error) }),
);

self.onmessage = (event) => {
  const { id, bytes, encodingDate } = event.data;
  try {
    const xml = new TextDecoder().decode(
      self.scoretk.convert(new Uint8Array(bytes), { encodingDate }),
    );
    self.postMessage({ type: 'converted', id, xml });
  } catch (error) {
    self.postMessage({ type: 'error', id, message: error instanceof Error ? error.message : String(error) });
  }
};
