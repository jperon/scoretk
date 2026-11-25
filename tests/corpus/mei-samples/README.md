# MEI 5.1 sample encodings — pinned manifest

This directory contains **only manifests and documentation**: no upstream
encoding is vendored. The data are fetched, outside the repository and outside
any build path, by:

```sh
SCORETK_MEI_SAMPLES=$HOME/.cache/scoretk/mei-samples scripts/fetch-mei-samples.sh
```

The script pins commit `f3f1baba02e32279b25dad660b82c834bac034b4` of
<https://github.com/music-encoding/sample-encodings> and verifies the SHA-256
of every file in `MANIFEST.tsv`. The files are untrusted data: they are read
as XML by the measurement tests and never executed.

- `MANIFEST.tsv` — the 130 CMN files that schema-validate against MEI 5.1
  (`jing mei-all_anyStart.rng`, schema repository
  `music-encoding/schema`, directory `5.1`), and contain a `<score>`.
- `EXCLUDED.tsv` — the 37 other schema-valid files: header-only or
  document-start snippets with no `<score>`, hence no music events to
  measure. The MEI 5.1 folder contains no neume or mensural encoding; those
  traditions are not in this lot (they would need the `mei-Neumes.rng` /
  `mei-Mensural.rng` schemas and a separate measurement basket).

Licence: **ECL-2.0**, Copyright (c) Music Encoding Initiative (MEI) Board
(`LICENSE` of the upstream repository). ECL-2.0 is GPL-3 compatible; see
`NOTICE`, `docs/licences-tierces.md` and `docs/agents/mei.md` §9.
