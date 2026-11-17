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

// Page hôte de la démo Web Worker. Le module se contente de piloter le worker
// (worker-demo-worker.js), qui exécute la conversion avec la variante classic.

const versionLabel = document.querySelector('#version');
const fileInput = document.querySelector('#file');
const dropZone = document.querySelector('#drop');
const errorLabel = document.querySelector('#error');
const downloadButton = document.querySelector('#download');
const output = document.querySelector('#xml');

let current = { blob: null, filename: 'partition.musicxml' };
let nextId = 1;
let ready = false;
const pending = new Map();

const worker = new Worker('worker-demo-worker.js');

function today() {
  return new Date().toISOString().slice(0, 10);
}

function showError(message) {
  errorLabel.textContent = message;
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

worker.addEventListener('message', (event) => {
  const message = event.data;
  if (message.type === 'ready') {
    ready = true;
    fileInput.disabled = false;
    versionLabel.textContent = `mscz2xml ${message.version} — worker prêt (variante classic)`;
    return;
  }
  if (message.type === 'error' && message.id === undefined) {
    showError(message.message);
    return;
  }
  const request = pending.get(message.id);
  if (!request) {
    return;
  }
  pending.delete(message.id);
  if (message.type === 'error') {
    request.reject(new Error(message.message));
  } else {
    request.resolve(message.xml);
  }
});

worker.addEventListener('error', (event) => showError(event.message));

function convertInWorker(file) {
  return new Promise((resolve, reject) => {
    const id = nextId++;
    pending.set(id, { resolve, reject });
    file.arrayBuffer().then((buffer) => {
      // ArrayBuffer est cloné (transféré), le worker reçoit une copie.
      worker.postMessage({ id, bytes: buffer, encodingDate: today() }, [buffer]);
    }, reject);
  });
}

async function handleFile(file) {
  if (!file) {
    return;
  }
  if (!ready) {
    showError('le worker n’est pas encore prêt');
    return;
  }
  try {
    const xml = await convertInWorker(file);
    showMusicXml(xml, file.name);
  } catch (error) {
    output.hidden = true;
    downloadButton.disabled = true;
    showError(error instanceof Error ? error.message : String(error));
  }
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
