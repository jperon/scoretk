# Licence: a consumer's obligations

This document describes the obligations of anyone who **uses or distributes**
`scoretk-core` — the Rust library or the `scoretk` package (CLI and
WebAssembly, `nix build .#wasm`) — and not those of the project itself (see
`NOTICE`). The project is under **GPL-3.0-only**; there is **no more permissive
licence** for the library, including for the wasm.

> This text is a technical aid, not legal advice. For a commercial
> product, have the arrangement reviewed by a lawyer.

## Simple use: no obligations

GPL v3 restricts neither private use, nor internal use in a company, nor
running the program: using `scoretk-core` to convert your own scores triggers
nothing. Likewise, an online service that **runs** the binary or the
wasm on demand is not, in itself, a distribution: GPL v3, unlike
AGPL, has no « network use » clause. (The situation changes
as soon as the code itself is delivered: static assets, npm package, mobile
or desktop application, public CDN.)

## The case of an application embedding the wasm or the library

Loading the wasm module into a JavaScript application (page, worker, PWA,
extension, packaged application) and calling it from non-GPL code creates
**most likely a combined work** within the meaning of the GPL: the module and its
calling code run in the same program, share its data structures and form a functional whole. This is the FSF's usual analysis
for dynamic linking, of which loading a wasm module is the
technological equivalent. "Mere aggregation" (§5) covers only
independent programs, communicating at best through files or
command-line arguments — not a module loaded and driven in memory.

Consequences for such a **distributed** application:

1. **Compatible licence.** The whole (application + wasm) must be distributed
   under GPL-3.0-only or a licence compatible with it — in practice
   GPL-3.0-only, or GPL-3.0-or-later if you want to leave the choice to
   recipients. A permissive licence (MIT, Apache-2.0) is not sufficient:
   it permits use of the wasm, but the distributed whole remains subject to the
   GPL.
2. **Corresponding source.** Provide the "Corresponding Source" of
   the whole: the application code, the exact version of the wasm used,
   and everything needed to rebuild it from that
   source (the `scoretk-core` crate, the `Cargo.lock`, the build recipe —
   here `flake.nix`, the scripts and the wasm-bindgen version). The source must
   be delivered with the binary or offered in writing (§6), and remain
   available for as long as the binary is distributed.
3. **Notices preserved.** Do not remove SPDX headers, copyright
   notices, `LICENSE` or `NOTICE` from the package. Modified files must
   carry a prominent modification notice (§5a): if you modify the
   wasm or its wrapper, indicate it with the date.
4. **Legal notices in the interface.** For an interactive program, display the
   "Appropriate Legal Notices" (§0): at minimum the program name, the
   licence (GPL-3.0-only), the absence of warranty and a link to the
   corresponding source — typically in an « About » or « Licence » screen.
   This is the counterpart of the GPL's ban on additional advertising.
5. **No additional restriction.** No added condition (EULA,
   DRM preventing copying of the software, limitation of the number of installations,
   a non-reverse-engineering clause stricter than the law) may be added
   to the rights granted by the GPL (§7, §10). A unilateral termination
   clause or a separate "usage licence" is incompatible.
6. **Patents.** GPL v3 grants an implicit patent licence (§11); do
   not add a clause that would contradict it. Conversely, distributing
   the whole does not oblige you to surrender your own patents beyond what
   the GPL provides.
7. **Trademark.** The GPL grants no right over the « MuseScore »
   trademark (MuseScore Limited). A derived application may describe its
   input format, but must not suggest affiliation or endorsement.

## Special cases

- **Modifying the wasm or the library**: the same obligations, plus the
  modification notice (§5a) and the prohibition on renaming or hiding the origin. The
  modified code remains GPL-3.0-only.
- **Distributing only the binary** (`.wasm`, npm package, APK): this is a
  distribution; you must include the corresponding source or a valid written
  offer, with the URL and the exact version.
- **Integrating into a hardware product** (device firmware): §6
  also requires the "Installation Information" needed to install
  a modified version on the device, insofar as the device is a
  "User Product".
- **Mixing with GPL-incompatible code** (linked proprietary
  library, GPLv2-only code): distribution of the whole is impossible
  without changing licence or actually isolating the programs (separate
  processes, exchange via files).
- **Using the wasm server-side without distributing it**: no obligation to
  publish as long as the code is not delivered to the client. Beware: serving
  the `.js`/`.wasm` files to the browser is a distribution.

## Practical summary

| Action | Obligation |
| --- | --- |
| Converting your scores with the CLI | none |
| Calling the wasm from a page served to third parties | distribute the application under GPL-compatible terms + corresponding source + notices |
| Publishing the wasm package as-is on a CDN or npm | include `LICENSE`, `NOTICE` and the corresponding source (notices already in the package; source in the repository) |
| Modifying the code | dated modification notice, published source, licence preserved |
| Integrating into firmware/a device | + Installation Information (§6) |

The wasm package delivered by `nix build .#wasm` already contains `LICENSE`, `NOTICE`,
the `/*! @license … */` banner at the head of the JS files and the
"Licence and sources" section of its README; the host application must, for its part, display
the licence and the link to its own corresponding source.
