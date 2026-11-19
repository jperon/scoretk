# Installing scoretk

scoretk is a command-line program: you run it from a terminal, not from a menu. There is no graphical installer. This page gives the four ways to obtain it. Pick the one that matches your system.

## Before you start

Open a terminal:

- **Linux:** the terminal application of your distribution (for example GNOME Terminal or Konsole).
- **macOS:** Terminal, found in Applications → Utilities.
- **Windows:** PowerShell (or Windows Terminal).

Commands below are typed into that terminal and confirmed with Enter. Do not type the `#` comments.

## Option 0 — download a ready-made program

This is the simplest option if you do not want to install any development tools. Ready-to-run programs are published on the [releases page](https://github.com/jperon/scoretk/releases).

**Which version to choose**

- A **stable version** has a name starting with `v` (for example `v0.1.0`). Pick the most recent one.
- **nightly** is the latest development version. It contains the newest changes but may be less tested.

**Which archive to download**

On the releases page, under the version you chose, expand "Assets" and download the archive matching your computer:

- **Windows (64-bit):** `scoretk-…-x86_64-pc-windows-msvc.zip`
- **Mac with an Apple Silicon chip (M1, M2, M3…):** `scoretk-…-aarch64-apple-darwin.tar.gz`
- **Mac with an Intel chip:** `scoretk-…-x86_64-apple-darwin.tar.gz`
- **Linux (64-bit, Intel/AMD):** `scoretk-…-x86_64-unknown-linux-musl.tar.gz`

The `…` stands for the version number. Files ending in `-wasm.tgz` are a JavaScript/npm package for web projects, not the command-line program.

**Unpack it and run it**

- **Windows:** right-click the `.zip` file and choose "Extract All…" (or open it with 7-Zip). In the extracted folder, double-click `scoretk.exe`. To use it from PowerShell, run `.\scoretk.exe --version` in that folder.
- **macOS:** double-click the `.tar.gz` file to unpack it (or run `tar -xzf scoretk-….tar.gz` in Terminal). Then, in Terminal, go into the extracted folder and run `./scoretk --version`. macOS may refuse to open the program with a message saying it is "from an unidentified developer" (Gatekeeper). If that happens, run this command in the folder containing `scoretk`:

  ```sh
  xattr -d com.apple.quarantine scoretk
  ```

  then run `./scoretk --version` again.

- **Linux:** unpack the archive with `tar -xzf scoretk-….tar.gz`, then make the file executable and run it:

  ```sh
  chmod +x scoretk
  ./scoretk --version
  ```

**Check the download (optional but recommended)**

Each release also contains a `SHA256SUMS` file. Download it next to the archive, then:

- **Linux:** run `sha256sum -c SHA256SUMS`; the line for your archive must say `OK`.
- **macOS:** run `shasum -a 256 -c SHA256SUMS` (or `sha256sum -c SHA256SUMS` if you installed coreutils).
- **Windows (PowerShell):** run `Get-FileHash scoretk-….zip -Algorithm SHA256` and compare the printed hash with the value on the line for your archive in `SHA256SUMS`. They must be identical.

**Important:** release files will only exist after the first release has been published. Until then, the releases page may be empty; use Option A or Option B in the meantime.

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

Yes. Ready-to-run archives (including a Windows `scoretk.exe`) are published on the [releases page](https://github.com/jperon/scoretk/releases); see Option 0. They become available once the first release is published. You can always build from source instead with Option A or Option B.

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
