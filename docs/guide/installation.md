# Installing scoretk

scoretk is a command-line program: you run it from a terminal, not from a menu. There is no graphical installer. This page gives the three ways to obtain it. Pick the one that matches your system.

## Before you start

Open a terminal:

- **Linux:** the terminal application of your distribution (for example GNOME Terminal or Konsole).
- **macOS:** Terminal, found in Applications → Utilities.
- **Windows:** PowerShell (or Windows Terminal).

Commands below are typed into that terminal and confirmed with Enter. Do not type the `#` comments.

## Option A — with Nix (Linux and macOS)

If you already use Nix with flakes enabled, no installation is needed: the command is fetched and run directly. From a checkout of the scoretk repository:

```sh
nix run .#scoretk -- --version
```

Everything after `--` is passed to scoretk itself. To convert a file:

```sh
nix run .#scoretk -- input.mscz -o output.musicxml
```

If Nix reports that `nix-command` or `flakes` is experimental, add this line to `~/.config/nix/nix.conf` (create the file if needed), then retry:

```nix
experimental-features = nix-command flakes
```

To install the command permanently instead of typing `nix run` each time:

```sh
nix profile install .#scoretk
```

Or, without a local checkout, install directly from the repository:

```sh
nix profile install github:jperon/scoretk#scoretk
```

**Windows:** Nix does not run natively. Use Option B, or install Nix inside WSL2 and follow the Linux instructions there.

## Option B — with Rust / cargo (Linux, macOS and Windows)

First install the Rust toolchain from <https://rustup.rs> and follow its instructions. Then download the source code. In a terminal:

```sh
git clone https://github.com/jperon/scoretk.git
cd scoretk
cargo install --path crates/scoretk-cli
```

`cargo install` compiles the command-line tool and places it in `~/.cargo/bin` on Linux and macOS, or in `%USERPROFILE%\.cargo\bin` on Windows. That directory must be in your `PATH`; rustup normally adds it for you during installation. If it is not, restart the terminal (or add it manually) before continuing.

If you are working from a scoretk checkout that uses Nix, prefer the pinned toolchain from the repository:

```sh
nix develop -c cargo install --path crates/scoretk-cli
```

## Are there precompiled binaries?

No. scoretk does not currently publish ready-to-run binaries or a Windows `.exe`/`.msi` installer. Build it from source with Option A or Option B.

## Check that it works

Whatever installation method you chose, run:

```sh
scoretk --version
```

It prints `scoretk 0.1.0` followed by the GPL-3.0-only licence notice. You can also list the supported formats:

```sh
scoretk --list-formats
```

and see all available switches:

```sh
scoretk --help
```

## Common problems

- **`scoretk: command not found` (or “not recognized” on Windows).** The binary is not in your `PATH`. Add `~/.cargo/bin` (Windows: `%USERPROFILE%\.cargo\bin`), or run the tool through `nix run .#scoretk -- …`.
- **Nix complains about an “experimental feature”.** Enable flakes as shown above.
- **`cargo` fails with a `rust-std` or toolchain conflict inside this repository.** The repository pins its toolchain through Nix: run `nix develop -c cargo …` instead of plain `cargo`.
- **“format d'entrée indéterminé” (input format undetermined).** The file extension is unknown or the input came from standard input. Add `--from mscx` (or the appropriate format name).
- **“format de sortie indéterminé” (output format undetermined).** You are writing to standard output, where there is no extension to guess from. Add `--to musicxml` (or the desired format).
- **The file cannot be read.** Check the path and your file permissions; use the full path to the file if you are not in its directory.

Ready to convert something? Continue with the [usage guide](usage.md).
