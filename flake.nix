{
  description = "scoretk — boîte à outils de conversion de partitions (MuseScore, MusicXML, LilyPond, ABC, MIDI), bibliothèque portable (wasm32, Android, iOS) et CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # crane est une librairie pure (mkLib pkgs), elle n'a pas d'entrée nixpkgs
    # à faire suivre.
    crane.url = "github:ipetkov/crane";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
    crane,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        # Instance nixpkgs « normale » : rien d'unfree n'y entre, donc le
        # shell par défaut reste utilisable sans accepter de licence.
        pkgs = import nixpkgs {
          inherit system;
          overlays = [(import rust-overlay)];
        };

        # Instance distincte pour Android : le SDK et le NDK sont marqués
        # unfree et exigent l'acceptation explicite de leur licence.
        pkgsAndroid = import nixpkgs {
          inherit system;
          config = {
            allowUnfree = true;
            android_sdk.accept_license = true;
          };
          overlays = [(import rust-overlay)];
        };

        # La toolchain vient de rust-toolchain.toml : une seule source de
        # vérité pour le channel, les composants et les cibles.
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # cleanCargoSource ne garde que ce qui influence la compilation Cargo
        # (.rs, *.toml, Cargo.lock, .cargo/config, plus les répertoires) : pas
        # besoin de reconstruire quand la doc change. Il faut y ajouter
        # `tests/corpus/` (données .mscx/.mscz/.musicxml) : les tests
        # d'intégration les lisent via CARGO_MANIFEST_DIR/../../tests/corpus,
        # et sans elles le check `tests` échoue (« No such file or directory »),
        # crane ne conservant que le répertoire vide.
        # buildDepsOnly n'est pas alourdi par ce corpus : crane y remplace les
        # sources par un dummy src, donc les dépendances ne sont pas
        # recompilées quand le corpus change.
        src = pkgs.lib.cleanSourceWith {
          src = pkgs.lib.cleanSource ./.;
          filter = path: type:
            craneLib.filterCargoSources path type
            || pkgs.lib.hasPrefix (toString ./tests/corpus + "/") (toString path);
          name = "scoretk-source";
        };

        # Une seule source de vérité pour la version, partagée par les
        # dérivations et par le package.json du paquet navigateur.
        version = "0.1.0";

        commonArgs = {
          inherit src version;
          strictDeps = true;
          nativeBuildInputs = [pkgs.pkg-config];
          # Le Cargo.toml racine sera un manifeste de workspace (pas de
          # [package]), donc crane ne peut pas en déduire nom et version :
          # on les fixe ici. Ils ne servent qu'à nommer les dérivations.
          pname = "scoretk";
        };

        # Dépendances compilées une seule fois et partagées par la CLI et par
        # les checks (clippy, tests, wasm).
        #
        # ATTENTION : crane dérive les dépendances à partir de Cargo.lock, qui
        # n'existe pas encore dans ce dépôt (les crates sont écrites en
        # parallèle). `nix build` et `nix flake check` échoueront tant qu'il
        # manque ; `nix develop` et `nix flake show` fonctionnent. Dès que le
        # workspace Cargo est en place, lancer `cargo generate-lockfile` : rien
        # à changer dans les expressions ci-dessous.
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Dépendances précompilées pour wasm32. Crane impose que
        # buildDepsOnly et la construction qui consomme ses artefacts aient le
        # même CARGO_BUILD_TARGET : d'où une dérivation distincte des artefacts
        # hôte.
        wasmCargoArtifacts = craneLib.buildDepsOnly (commonArgs
          // {
            CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          });

        # --- Paquet navigateur (`nix build .#wasm`) ----------------------
        #
        # Compilation wasm32 de l'enveloppe de la façade (crate scoretk-wasm)
        # avec les dépendances déjà compilées (wasmCargoArtifacts), puis
        # wasm-bindgen et wasm-opt. C'est le seul paquet wasm du dépôt.
        wasm32Build = craneLib.cargoBuild (commonArgs
          // {
            cargoArtifacts = wasmCargoArtifacts;
            CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
            cargoExtraArgs = "--locked -p scoretk-wasm";
            doCheck = false;
            doInstallCargoArtifacts = false;
            installPhaseCommand = ''
              mkdir -p $out
              cp target/wasm32-unknown-unknown/release/scoretk_wasm.wasm $out/
            '';
          });

        wasmPackageJson = pkgs.writeText "scoretk-package.json" (builtins.toJSON {
          name = "scoretk";
          inherit version;
          description = "Conversion de partitions (MuseScore, MusicXML, LilyPond, ABC, MIDI) dans le navigateur, WebAssembly";
          license = "GPL-3.0-only";
          repository = {
            type = "git";
            url = "https://github.com/jperon/scoretk";
          };
          type = "module";
          main = "./esm/scoretk_wasm.js";
          types = "./esm/scoretk_wasm.d.ts";
          exports = {
            "." = {
              types = "./esm/scoretk_wasm.d.ts";
              default = "./esm/scoretk_wasm.js";
            };
            "./classic/scoretk.js" = "./classic/scoretk.js";
            "./classic/*" = "./classic/*";
            "./package.json" = "./package.json";
          };
          files = ["esm" "classic" "README.md" "LICENSE" "NOTICE"];
          sideEffects = ["./classic/scoretk.js"];
        });

        wasmLicenseBanner = pkgs.writeText "scoretk-wasm-license-banner.js" ''
          /*! @license GPL-3.0-only — scoretk (façade multi-formats, WebAssembly)
           * Copyright (C) 2026 jperon
           * Portage de MuseScore Studio, Copyright (C) MuseScore Limited and others,
           * modifié (réécrit en Rust) en 2026.
           * source : https://github.com/jperon/scoretk
           * Distribué SANS AUCUNE GARANTIE ; licence complète dans LICENSE.
           */
        '';

        wasm = pkgs.runCommand "scoretk-wasm-${version}" {
          nativeBuildInputs = [pkgs.wasm-bindgen-cli pkgs.binaryen pkgs.gzip];
        } ''
          mkdir -p $out/esm $out/classic

          wasm-bindgen --target web \
            --out-dir $out/esm --out-name scoretk_wasm \
            ${wasm32Build}/scoretk_wasm.wasm

          wasm-bindgen --target no-modules \
            --out-dir $out/classic --out-name scoretk_wasm \
            ${wasm32Build}/scoretk_wasm.wasm

          for module in $out/esm/scoretk_wasm_bg.wasm $out/classic/scoretk_wasm_bg.wasm; do
            echo "avant wasm-opt : $(stat -c%s "$module") octets ($module)"
            wasm-opt -Oz --strip-debug "$module" -o "$module.opt"
            mv "$module.opt" "$module"
          done

          raw=$(stat -c%s $out/esm/scoretk_wasm_bg.wasm)
          gz=$(gzip -9 -c $out/esm/scoretk_wasm_bg.wasm | wc -c)
          echo "scoretk : $raw octets ($gz octets gzip)"
          echo "$raw octets ($gz gzip)" > $out/SIZE

          install -m 0644 ${./crates/scoretk-wasm/pkg/classic/scoretk.js} \
            $out/classic/scoretk.js

          for module in $out/esm/scoretk_wasm.js $out/classic/scoretk_wasm.js; do
            cat ${wasmLicenseBanner} "$module" > "$module.banner"
            mv "$module.banner" "$module"
          done

          install -m 0644 ${wasmPackageJson} $out/package.json
          install -m 0644 ${./crates/scoretk-wasm/pkg/README.md} $out/README.md
          install -m 0644 ${./LICENSE} $out/LICENSE
          install -m 0644 ${./NOTICE} $out/NOTICE
        '';

        wasmNodeSmoke = pkgs.runCommand "scoretk-wasm-node-smoke" {
          nativeBuildInputs = [pkgs.nodejs];
        } ''
          export SCORETK_PKG_DIR=${wasm}
          export SCORETK_CORPUS=${./tests/corpus}
          node ${./crates/scoretk-wasm/tests/node-smoke.mjs}
          touch $out
        '';

        sharedDevTools = with pkgs; [
          rustToolchain
          cargo-ndk
          wasm-pack
          wasm-bindgen-cli
          binaryen
          cargo-nextest
          pkg-config
        ];

        androidPackages = pkgsAndroid.androidenv.composeAndroidPackages {
          # Versions « latest » du dépôt androidenv : pas de version figée à
          # maintenir quand Google publie un nouveau NDK.
          includeNDK = true;
          includeEmulator = false;
          includeSystemImages = false;
          includeSources = false;
        };
        androidSdk = androidPackages.androidsdk;
      in {
        devShells.default = pkgs.mkShell {
          packages = sharedDevTools;
          # Pour rust-analyzer : les sources de la bibliothèque standard.
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };

        # Shell Android : même outillage, plus le SDK/NDK et les variables
        # d'environnement que cargo-ndk attend.
        devShells.android = pkgsAndroid.mkShell {
          packages = sharedDevTools ++ [androidSdk];
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          ANDROID_HOME = "${androidSdk}/libexec/android-sdk";
          ANDROID_SDK_ROOT = "${androidSdk}/libexec/android-sdk";
          # cargo-ndk attend la racine du NDK (contenant toolchains/llvm/...).
          ANDROID_NDK_HOME = "${androidSdk}/libexec/android-sdk/ndk-bundle";
          ANDROID_NDK_ROOT = "${androidSdk}/libexec/android-sdk/ndk-bundle";
          NDK_HOME = "${androidSdk}/libexec/android-sdk/ndk-bundle";
        };

        packages.default = craneLib.buildPackage (commonArgs
          // {
            inherit cargoArtifacts;
            # Le binaire vit dans crates/scoretk-cli.
            cargoExtraArgs = "-p scoretk-cli";
          });

        # Paquet navigateur : `nix build .#wasm` → result/{esm,classic,package.json,README.md}.
        packages.wasm = wasm;

        # Façade multi-formats et sa CLI (`nix build .#scoretk`).
        packages.scoretk = craneLib.buildPackage (commonArgs
          // {
            inherit cargoArtifacts;
            pname = "scoretk";
            cargoExtraArgs = "-p scoretk-cli";
          });

        # `nix run .#scoretk -- entrée.mscz -o sortie.musicxml`.
        apps.scoretk = flake-utils.lib.mkApp {
          drv = self.packages.${system}.scoretk;
          name = "scoretk";
        } // {
          meta.description = "Conversion de partitions entre formats (MuseScore, MusicXML, MIDI, …)";
        };

        checks = {
          # `-D warnings` : un warning casse le check, comme en CI.
          clippy = craneLib.cargoClippy (commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- -D warnings";
            });

          tests = craneLib.cargoTest (commonArgs
            // {
              inherit cargoArtifacts;
            });

          fmt = craneLib.cargoFmt commonArgs;

          # Vérifie que les licences des crates liées restent compatibles avec
          # GPL-3.0-only (docs/licences-tierces.md). crane vend les sources des
          # dépendances et pose CARGO_NET_OFFLINE : `cargo tree --offline` ne
          # fait aucun accès réseau pendant le check.
          licences = craneLib.mkCargoDerivation {
            pname = "scoretk-licences";
            inherit version;
            src = pkgs.lib.cleanSource ./.;
            cargoArtifacts = null;
            nativeBuildInputs = [pkgs.python3];
            buildPhaseCargoCommand = "python3 scripts/check-licences.py";
            doInstallCargoArtifacts = false;
            installPhaseCommand = ''
              mkdir -p $out
              cp docs/licences-tierces.md $out/
            '';
          };

          # Test Node du paquet produit par `packages.wasm` : il vérifie la
          # variante ESM telle que livrée (initSync, conversion du corpus,
          # entrée invalide).
          wasm-node-smoke = wasmNodeSmoke;

          # La librairie doit au moins compiler pour wasm32 ; pas de tests ici
          # (doCheck = false) puisqu'on ne peut pas les exécuter sur cette cible.
          wasm32-unknown-unknown = craneLib.cargoBuild (commonArgs
            // {
              cargoArtifacts = wasmCargoArtifacts;
              CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
              cargoExtraArgs = "-p scoretk-core -p scoretk-musescore-import -p scoretk-musicxml-export -p scoretk -p scoretk-abc -p scoretk-midi -p scoretk-lilypond -p scoretk-musicxml -p scoretk-mscx -p scoretk-wasm";
              doCheck = false;
            });
        };

        formatter = pkgs.nixpkgs-fmt;
      });
}
