// Démo de la variante « classic » (wasm-bindgen --target no-modules) dans un
// Web Worker : c'est le mode d'emploi de la PWA cantus, qui charge ses scripts
// en classique et appelle importScripts.
//
// Le worker est à côté de cette page ; les fichiers du paquet sont dans
// `result/classic/` à la racine du dépôt (voir `nix build .#wasm`).

importScripts('../../result/classic/mscz2xml.js');

self.mscz2xml.ready.then(
  () => self.postMessage({ type: 'ready', version: self.mscz2xml.version() }),
  (error) =>
    self.postMessage({ type: 'error', message: error instanceof Error ? error.message : String(error) }),
);

self.onmessage = (event) => {
  const { id, bytes, encodingDate } = event.data;
  try {
    const xml = self.mscz2xml.convert(new Uint8Array(bytes), { encodingDate });
    self.postMessage({ type: 'converted', id, xml });
  } catch (error) {
    self.postMessage({ type: 'error', id, message: error instanceof Error ? error.message : String(error) });
  }
};
