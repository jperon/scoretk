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

// Démo de la variante ESM du paquet wasm.
//
// Préparation :
//   nix build .#wasm
//   python -m http.server 8000        # à la racine du dépôt
// puis http://localhost:8000/examples/web/index.html
//
// Le chemin d'import pointe vers `result/esm/` ; adapter la ligne suivante si
// le paquet est servi depuis un autre dossier.

import init, { convert, version } from '../../result/esm/mscz2xml_wasm.js';

const versionLabel = document.querySelector('#version');
const fileInput = document.querySelector('#file');
const dropZone = document.querySelector('#drop');
const errorLabel = document.querySelector('#error');
const downloadButton = document.querySelector('#download');
const output = document.querySelector('#xml');

let current = { blob: null, filename: 'partition.musicxml' };

function today() {
  return new Date().toISOString().slice(0, 10);
}

function showError(error) {
  errorLabel.textContent = error instanceof Error ? error.message : String(error);
}

function showMusicXml(xml, baseName) {
  errorLabel.textContent = '';
  output.hidden = false;
  output.textContent = xml;
  current = {
    blob: new Blob([xml], { type: 'application/vnd.recordare.musicxml+xml' }),
    filename: `${baseName.replace(/\.(mscz|mscx)$/i, '')}.musicxml`,
  };
  downloadButton.disabled = false;
}

async function handleFile(file) {
  if (!file) {
    return;
  }
  try {
    const bytes = new Uint8Array(await file.arrayBuffer());
    const xml = convert(bytes, { encodingDate: today() });
    showMusicXml(xml, file.name);
  } catch (error) {
    output.hidden = true;
    downloadButton.disabled = true;
    showError(error);
  }
}

try {
  await init();
  versionLabel.textContent = `mscz2xml ${version()} — module ESM chargé`;
} catch (error) {
  versionLabel.textContent = 'échec du chargement du module';
  showError(error);
}

fileInput.addEventListener('change', () => handleFile(fileInput.files[0]));

for (const type of ['dragenter', 'dragover']) {
  dropZone.addEventListener(type, (event) => {
    event.preventDefault();
    dropZone.classList.add('over');
  });
}

for (const type of ['dragleave', 'drop']) {
  dropZone.addEventListener(type, (event) => {
    event.preventDefault();
    dropZone.classList.remove('over');
  });
}

dropZone.addEventListener('drop', (event) => handleFile(event.dataTransfer.files[0]));

downloadButton.addEventListener('click', () => {
  if (!current.blob) {
    return;
  }
  const url = URL.createObjectURL(current.blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = current.filename;
  link.click();
  URL.revokeObjectURL(url);
});
