# MusicXML export: reliability campaign against MuseScore 4.7.4

The writer lives in `crates/scoretk-musicxml-export`. This document records the
file-by-file measurement campaign started on 2026-11-20, its method and its
first corrected cause. The oracle and its manifests live outside the repository
(`scripts/oracle/musicxml/README.md`).

## 1. Oracle and method

For each `.mscz` of the 174-file lot (`/tmp/ly95-oracle/manifestes/mscz.txt`),
`scripts/oracle/musicxml/mesure.sh` produces four files: scoretk → MusicXML,
MuseScore 4.7.4 → MusicXML, and the two corresponding MIDI files. Three
measurements follow: MusicXML 4.0 XSD validity, N1/N2 on the MIDI, and the
canonical-line `core`/`full` comparison of `crates/scoretk/tests/oracle_core.rs`
(which uses the same normalisation as `e2e_corpus`). The `core` field of
`mesure.jsonl` is merged at the end of the run from `core.tsv`.

Every campaign must rebuild the measured binary from the **current** HEAD: the
first ranking of this campaign (`/tmp/pause-xml/current`, and the older
`/tmp/ott-xml/current` and `/tmp/anchor-xml/current`) was taken with binaries
built before commit `5283edb` ("conserver les sauts de ligne des noms
d'instrument"). That commit fixed the apparent top cause (`part-name`, 50
files: MuseScore keeps the `<br/>` of `<longName>S<br/>A</longName>` as a
newline, and so must the reader). A clean run after `5283edb` confirms that
`part-name` has disappeared from the ranking. Do not rank causes from an old
`mesure.jsonl`.

## 2. Clean ranking (2026-11-20)

First differing canonical tag, by number of files (173 converted, 40 already
core-identical):

| Tag | Files |
| --- | ---: |
| `staff-details` | 72 |
| `tuplet` | 14 |
| `attributes` | 14 |
| `measure` | 9 |
| `midi-channel` | 6 |
| `sound` | 5 |
| `movement-number` | 4 |
| other | 9 |

XSD validity: 0 errors on the scoretk side, 2 on the MuseScore side (both
pre-existing in the reference). N1 0.9966, N2 0.9492.

## 3. Corrected cause: invisible staves in `<staff-details>`

`writeStaffDetails` (`src/importexport/musicxml/internal/export/exportmusicxml.cpp:8059-8104`)
writes a `<staff-details>` block for every staff that is hidden, even a normal
5-line staff, with `print-object="no"` and `<staff-lines>`. A staff is hidden
when `!st->show()` (`:8066`) or when it is in the `hiddenStaves` list passed by
`writeMeasure` (`:8691`). The visibility flag is read from `<isStaffVisible>`
by `TRead::read(Staff*)` (`src/engraving/rw/read500/tread.cpp:4111`).

The Rust port skipped every 5-line staff (`staff.lines == 5`) and did not read
`<isStaffVisible>` at all, so the block was missing in 72 files. The fix:

- `Staff` gains a `visible` field, default `true` (`scoretk-core`);
- the 4.x reader reads `<isStaffVisible>` (`read4xx/mod.rs`), the 3.x reader
  reads `<visible>`/`<isStaffVisible>` (`read302/part.rs`);
- `write_staff_details` emits `print-object="no"` for an invisible staff and
  for a staff hidden by `hideEmptyStaves` (`systems.hidden_at`), with a reduced
  test (`tests::portee_masquee_ecrit_staff_details`) that failed before the fix.

### Result, file by file against the same clean baseline

The 173 pairs were reconverted with the corrected binary and compared with the
same `oracle_core` normalisation: **69 files gain, 0 file loses, 104 unchanged**;
the total number of differing canonical lines drops from 42 107 to 39 755
(−2 352). On the 72 `staff-details` files alone: 66 gains, 0 losses, 6
unchanged. After the fix, the first difference of those files is `dot` (27),
`staff-details` (31), `tuplet` (7) or `attributes` (6).

The remaining 31 `staff-details` differences come from staves that MuseScore's
layout hides without an explicit `<isStaffVisible>` (distance to the previous
staff ≤ 0 in `exportmusicxml.cpp:7553-7575`). The pivot model carries no layout,
so these are a documented limit, not a regression.

## 4. Next causes

Work the ranking of §2 in order, re-measuring after each committed cause:
`tuplet` (bracket direction), `attributes`, `measure`, `midi-channel`, `sound`.
Each cause gets its own commit, a reduced test written first, and a C++
citation.
