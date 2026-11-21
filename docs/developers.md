# Developer documentation

This page describes the Rust workspace, how to build and test it, and the WebAssembly package consumed by applications such as cantus. Contributor rules and in-depth format notes are indexed in [AGENTS.md](../AGENTS.md) and [docs/agents/](agents/).

## Architecture

The model in `crates/scoretk-core/src/model` is the **pivot**: every format has a reader into that model and a writer out of it, so there are no N×N converters. `crates/scoretk` (the facade) picks the reader or writer from the `Format` and exposes `read(bytes, format)` and `write(score, format, options)`. Everything is byte-in/byte-out: the libraries contain no `std::fs`, no network, no threads and no clock, and compile for `wasm32-unknown-unknown`, Android and iOS. Only `crates/scoretk-cli` touches the disk.

| Area | Crates |
| --- | --- |
| Pivot model | `scoretk-core` |
| MuseScore | `scoretk-musescore-import`, `scoretk-musescore-export` |
| MusicXML | `scoretk-musicxml-import`, `scoretk-musicxml-export` |
| MIDI | `scoretk-midi-common`, `scoretk-midi-import`, `scoretk-midi-export` |
| LilyPond | `scoretk-lilypond-common`, `scoretk-lilypond-import`, `scoretk-lilypond-export` |
| ABC | `scoretk-abc-common`, `scoretk-abc-import`, `scoretk-abc-export` |
| Facade and shells | `scoretk`, `scoretk-cli`, `scoretk-wasm` |

That is 17 crates. Each direction is a separate Cargo feature (`musescore-import`, `musescore-export`, `musicxml-import`, …, `abc-export`): ten symmetric features, all enabled by default, with no mixed alias. The CLI and the wasm crate enable all ten, so both handle all seven formats in both directions. `Format` also detects a format from file content; `--list-formats` reports the real capabilities of the compiled library.

`write_playback` (facade, needs `midi-export` and `musicxml-export`) returns identified MusicXML, the same MIDI as a normal export, and a `Timemap`: a note/measure/tempo-to-time map produced by the MIDI renderer. See [the facade notes](agents/scoretk.md) and [the timemap notes](agents/timemap.md).

## Build and test

Use the pinned Nix toolchain; plain `cargo` outside `nix develop` is a rustup shim that conflicts with it.

```sh
nix develop -c cargo test --workspace
nix develop -c cargo clippy --workspace --all-targets -- -D warnings
nix develop -c cargo fmt --all -- --check
nix develop -c cargo build -p scoretk-core --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-midi-common -p scoretk-midi-import -p scoretk-midi-export --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-lilypond-common -p scoretk-lilypond-import -p scoretk-lilypond-export --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-abc-common -p scoretk-abc-import -p scoretk-abc-export --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-musescore-export --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-musicxml-import --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk --target wasm32-unknown-unknown
nix develop -c cargo build -p scoretk-wasm --target wasm32-unknown-unknown
nix flake check
nix build .#wasm
```

The end-to-end corpus check has two levels: the musical core (blocking) and full byte-for-byte parity:

```sh
nix develop -c cargo test -p scoretk --test e2e_corpus -- --nocapture
```

`wasm-bindgen` is pinned to `=0.2.130`, the same version as the CLI in the Nix shell; a mismatch produces a “schema version mismatch” error.

## WebAssembly package

### Building

```sh
nix build .#wasm
cat result/SIZE
node crates/scoretk-wasm/tests/node-smoke.mjs
```

`nix build .#wasm` compiles `scoretk-wasm` for `wasm32-unknown-unknown`, runs `wasm-bindgen` twice (ES module and classic script), optimizes both binaries with `wasm-opt -Oz --strip-debug`, and assembles `result/`. The Node smoke test exercises the generated ESM package on real corpus files; the flake check `wasm-node-smoke` runs it in Nix. Two minimal browser demos live in `examples/web/` (serve the repository root with a static HTTP server, then open `examples/web/index.html` or `examples/web/worker-demo.html`).

### Package contents

```
esm/scoretk_wasm.js             ES module (wasm-bindgen --target web)
esm/scoretk_wasm_bg.wasm        optimized binary
esm/scoretk_wasm.d.ts           TypeScript declarations
classic/scoretk_wasm.js         classic script (--target no-modules)
classic/scoretk_wasm_bg.wasm    the same binary, optimized separately
classic/scoretk.js              wrapper: self.scoretk = { ready, convert, convertWithTimeline, version, detect, formats, baseUrl }
classic/scoretk_wasm.d.ts       TypeScript declarations
package.json                    name "scoretk", type "module", exports and types
README.md, LICENSE, NOTICE, SIZE
```

Both `.wasm` files are byte-identical. Serve `.wasm` as `application/wasm` so the glue can use `WebAssembly.instantiateStreaming`.

### JavaScript API

| Function | Signature | Role |
| --- | --- | --- |
| `convert` | `(input: Uint8Array, options?: ConvertOptions) => Uint8Array` | Convert bytes to bytes; `to` defaults to `"musicxml"`. |
| `convertWithTimeline` | `(input: Uint8Array, options?: ConvertOptions) => PlaybackResult` | Identified MusicXML + MIDI + timeline; `to` is ignored. |
| `detect` | `(input: Uint8Array) => string \| null` | Format guessed from content. |
| `formats` | `() => FormatInfo[]` | Recognized formats and compiled read/write capabilities. |
| `version` | `() => string` | Library version. |

All `ConvertOptions` are optional:

| Option | Type | Default |
| --- | --- | --- |
| `from` | `string` | detected from content |
| `to` | `string` | `"musicxml"` |
| `encodingDate` | `string` (`YYYY-MM-DD`) | library constant (reproducible output) |
| `software` | `string` | library constant |
| `concertPitch` | `boolean` | library default |
| `expandRepeats` | `boolean` | `true` |

`convert` always returns a `Uint8Array`, including for text formats: decode it with `new TextDecoder().decode(bytes)` for MusicXML, LilyPond or ABC. Failures throw a JavaScript `Error` carrying the Rust error message.

`convertWithTimeline` resolves `from` like `convert` and ignores `to`. It returns `{ musicxml, midi, timeline }`. The MusicXML carries `id` **and** `xml:id` (same value) on every `<measure>` and `<note>`; these are exactly the `xmlId` values in `timeline`. `timeline.measures[].ordinal` is the 0-based measure index and the only key to anchor a measure; `timeline.notes` is sorted by increasing `tick`, then by a stable key. Repeated passages appear as several timeline entries, so anchor with `ordinal`/`xmlId` **plus** `tick` rather than `xmlId` alone. The timeline has no page or system geometry. See [the timemap reference](agents/timemap.md) for every field.

### ESM example

```js
import init, { convert, convertWithTimeline, detect, formats, version } from 'scoretk';

await init(); // loads scoretk_wasm_bg.wasm next to the module
const bytes = new Uint8Array(await file.arrayBuffer());
console.log(version(), formats(), detect(bytes));

const xml = new TextDecoder().decode(convert(bytes)); // MusicXML by default
const midi = convert(bytes, { to: 'midi' });           // Uint8Array
const { musicxml, timeline } = convertWithTimeline(bytes);
```

### Classic script in a worker

```js
// worker.js, loaded with importScripts()
importScripts('scoretk/classic/scoretk.js');
await self.scoretk.ready;

self.onmessage = ({ data }) => {
  const midi = self.scoretk.convert(new Uint8Array(data), { to: 'midi' });
  self.postMessage(midi);
};
```

If the wrapper cannot locate its files from its own URL, define `self.SCORETK_BASE_URL` before `importScripts`. Full details: [the wasm reference](agents/wasm.md).

### Sizes

After `wasm-opt -Oz --strip-debug` the binary is roughly 1.9 MB raw (784 KB gzipped, 599 KB brotli) at commit `fdf22443`; `cat result/SIZE` gives the exact raw and gzip figures for the current build. The package stays below the roughly 2.5 MB raw budget of cantus. See [the wasm reference, section 2](agents/wasm.md) for the measurement method and breakdown.

### Stability contract for cantus

Do not change these without coordination:

- `package.json` name is `"scoretk"`; the package exposes `esm/scoretk_wasm.js`, `esm/scoretk_wasm_bg.wasm`, `esm/scoretk_wasm.d.ts` and the classic variant.
- `classic/scoretk.js` exposes `self.scoretk = { ready, convert, convertWithTimeline, version, detect, formats, baseUrl }`.
- `convert(bytes, { from?, to?, encodingDate?, software?, concertPitch?, expandRepeats? })` returns a `Uint8Array`; `to` defaults to `"musicxml"`; text results are decoded by the consumer with `TextDecoder`.
- `detect(bytes)` returns a string or `null`, `formats()` returns `FormatInfo[]`, `version()` returns a string.
- `convertWithTimeline(bytes, options?)` is additive and returns `{ musicxml, midi, timeline }`; `timeline.measures[].ordinal` is the 0-based measure index and the only anchor key, and `timeline.notes` is sorted by `tick` then by stable key.

## License

scoretk, including the WebAssembly package, is **GPL-3.0-only**; there is no more permissive license. Converting your own scores with the CLI creates no distribution obligation. Distributing an application that embeds the library or wasm likely creates a combined work: license it GPL-3.0-compatible, provide the corresponding source, keep SPDX headers, `LICENSE` and `NOTICE`, display the legal notices in the interface, and add no further restrictions. “MuseScore” remains a trademark of MuseScore Limited and implies no affiliation.

See [the consumer obligations](agents/licence.md), [LICENSE](../LICENSE) and [NOTICE](../NOTICE).
