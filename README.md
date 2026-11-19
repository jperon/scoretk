# scoretk

scoretk converts music scores between MuseScore, MusicXML, LilyPond, ABC and MIDI, on Linux, macOS and Windows.

It is a small command-line tool: you give it a score file and it writes the same music in another format. It does not lay out or print scores — open the result in your notation software for the final appearance.

## Supported formats

| Format | Extensions | Read | Write |
| --- | --- | --- | --- |
| MuseScore | `.mscz`, `.mscx` | Yes | Yes |
| MusicXML | `.musicxml`, `.xml` | Yes | Yes |
| Compressed MusicXML | `.mxl` | Yes | Yes |
| LilyPond | `.ly` | Yes | Yes |
| ABC | `.abc` | Yes | Yes |
| MIDI | `.mid`, `.midi` | Yes | Yes |

The tool also reports the formats it supports with `scoretk --list-formats`.

## What converts well, and what does not

**Usually preserved:** pitches and note durations, rests, voices and staves, clefs, key and time signatures, ties, lyrics, repeats and endings, dynamics, articulations, tempo changes and chord names. Converting between MuseScore and MusicXML is the most faithful path.

**Not preserved:** page layout, margins, system breaks, exact spacing and text positions. Output is *musically* equivalent, not visually identical; let MuseScore or LilyPond do the final engraving. Fine engraving details (beam hooks, stem directions, notehead shapes) may also differ.

**LilyPond and ABC are subsets.** Advanced LilyPond constructs (Scheme, layout blocks, drum or chord modes, some ornaments, microtones) and ABC features outside the common subset may be ignored or simplified. Only the first score in a LilyPond file is read, and ABC keeps a single key and time signature per tune. A round trip through ABC does not restore MuseScore transposition settings.

**MIDI is playback, not notation.** It carries no layout, lyrics, text, instrument names or engraving details. Fermatas and breaths add no playback time, and drum maps and instrument changes are not exported. Note velocities in the output come from dynamics, hairpins and velocities set on the notes, rather than from raw note velocities in a MIDI file. Converting MIDI back to MusicXML restores notes and rhythm, but not how the music was written on the page.

If a detail matters to you, open both files in your notation program and compare before using the result.

## License

scoretk is free software licensed under GPL-3.0-only; see [LICENSE](LICENSE).

## Guides

- [Installation](docs/guide/installation.md) — get the `scoretk` command on Linux, macOS or Windows.
- [Usage](docs/guide/usage.md) — convert files, choose formats, use standard input/output and batch convert.
- [Developer documentation](docs/developers.md) — architecture, builds, tests and the WebAssembly package.
