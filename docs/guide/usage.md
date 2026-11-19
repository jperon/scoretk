# Using the scoretk command line

This guide shows real conversions with files from the repository's `tests/corpus/` folder. Replace those paths with your own scores. scoretk takes one input file per invocation and writes one output file.

## Convert a file

The format is normally deduced from the file extensions:

```sh
scoretk input.mscz -o output.musicxml
```

The examples below have all been run on the repository's test corpus:

```sh
scoretk tests/corpus/v3/barline01.mscx -o barline01.musicxml
scoretk tests/corpus/v3/barline01.mscx --to ly -o barline01.ly
scoretk tests/corpus/v4/hairpins_and_repeats.mscx --to midi -o playback.mid
scoretk tests/corpus/midi/v3/repeat01.ref.mid -o from_midi.musicxml
```

Rewriting MuseScore files works in both directions; a compressed `.mscz` can be opened again afterwards:

```sh
scoretk tests/corpus/v3/barline01.mscx -o copy.mscz
scoretk copy.mscz -o roundtrip.musicxml
```

If you omit `-o`, the result is written to standard output instead of a file.

## Choose the formats explicitly

Use `--from` and `--to` when the extension is missing or misleading. Both accept these names: `mscz`, `mscx`, `musicxml` (also `xml`), `mxl`, `ly` (also `lilypond`), `abc`, `midi` (also `mid`).

```sh
scoretk score.dat --from mscx --to mxl -o score.mxl
```

To see what the compiled tool supports:

```sh
scoretk --list-formats
```

It prints the seven formats with their extensions and whether each can be read and written.

## Standard input and standard output

A single dash `-` means standard input as the input file, or standard output after `-o`:

```sh
cat tests/corpus/abc/v3_barline01.abc | scoretk - --from abc --to musicxml -o out.musicxml
cat tests/corpus/v3/barline01.mscx | scoretk - --from mscx --to midi -o out.mid
```

Two rules matter:

- With input `-`, the format is guessed from the file content; add `--from` if scoretk cannot recognize it.
- With `-o -` (or no `-o` at all), there is no extension to guess from, so `--to` is **required**.

This is also useful to inspect a conversion without creating a file:

```sh
scoretk tests/corpus/v3/barline01.mscx --to musicxml -o - | head
```

Avoid piping binary formats (MIDI, `.mscz`, `.mxl`) through text-oriented shells on Windows: pass them as file names instead.

## Convert several files

scoretk has no batch flag; use a simple loop. In a POSIX shell (Linux, macOS, WSL):

```sh
mkdir -p out
for f in tests/corpus/v3/*.mscx; do
  scoretk "$f" -o "out/$(basename "$f" .mscx).musicxml"
done
```

In PowerShell (Windows):

```powershell
New-Item -ItemType Directory -Force out | Out-Null
Get-ChildItem .\scores\*.mscx | ForEach-Object {
  scoretk $_.FullName -o (Join-Path out ($_.BaseName + '.musicxml'))
}
```

## All options

These are exactly the options shown by `scoretk --help`:

| Option | What it does |
| --- | --- |
| `-o`, `--output <SORTIE>` | Output file. `-` or an omitted `-o` writes to standard output. |
| `--from <FORMAT>` | Force the input format instead of guessing it. |
| `--to <FORMAT>` | Force the output format instead of guessing it from the extension. |
| `--list-formats` | Print the recognized formats and their read/write support, then exit. |
| `--encoding-date <YYYY-MM-DD>` | Fix the MusicXML `<encoding-date>`. By default it is today's date in UTC. Invalid dates are rejected. |
| `--no-expand-repeats` | For MIDI output, do not unfold repeats, endings and jumps. By default they are unfolded. |
| `-h`, `--help` | Print the help text. |
| `-V`, `--version` | Print the version and the GPL-3.0-only notice. |

Examples for the two conversion options:

```sh
scoretk tests/corpus/v3/barline01.mscx --to musicxml --encoding-date 2020-01-02 -o dated.musicxml
scoretk tests/corpus/v4/hairpins_and_repeats.mscx --to midi --no-expand-repeats -o flat.mid
```

`--encoding-date` only affects MusicXML output; `--no-expand-repeats` only affects MIDI output.

## FAQ

**“Input format undetermined” / “output format undetermined.”** Add `--from` for the input, or `--to` when writing to standard output.

**The file is empty and conversion fails immediately.** An empty file has no recognizable content. Check that the file is not empty and that you passed the right path; if the extension is unusual, add `--from`.

**Some notes or musical details are missing.** Check the format limits in the [README](../../README.md): LilyPond and ABC only support a subset, a LilyPond staff cannot hold more than four voices, and MIDI carries no notation details such as beams, lyrics or how a note was spelled. Compare the source and the result in your notation program.

**Transposition looks wrong.** scoretk converts the pitches stored in the score; it does not recalculate transposition. ABC has no standard transposition field, so a round trip through ABC does not restore MuseScore's `<transpose>`. The WebAssembly API has a `concertPitch` option for MusicXML export; the command line does not expose this switch.

**Can I use the tool from another directory?** Yes. Give full paths, for example `scoretk ~/scores/quartet.mscz -o ~/exports/quartet.musicxml`.

See also the [installation guide](installation.md) and the [developer documentation](../developers.md).
