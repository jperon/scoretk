{
  description = "mscz2xml — conversion MuseScore (.mscz) vers MusicXML : bibliothèque portable (wasm32, Android, iOS) et CLI";

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
        # (Cargo.toml, Cargo.lock, .cargo/config, src/, tests/, benches/,
        # examples/) : pas besoin de reconstruire quand la doc change.
        src = craneLib.cleanCargoSource ./.;

        commonArgs = {
          inherit src;
          strictDeps = true;
          nativeBuildInputs = [pkgs.pkg-config];
          # Le Cargo.toml racine sera un manifeste de workspace (pas de
          # [package]), donc crane ne peut pas en déduire nom et version :
          # on les fixe ici. Ils ne servent qu'à nommer les dérivations.
          pname = "mscz2xml";
          version = "0.1.0";
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
            # Le binaire vit dans crates/mscz2xml-cli.
            cargoExtraArgs = "-p mscz2xml-cli";
          });

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

          # La librairie doit au moins compiler pour wasm32 ; pas de tests ici
          # (doCheck = false) puisqu'on ne peut pas les exécuter sur cette cible.
          wasm32-unknown-unknown = craneLib.cargoBuild (commonArgs
            // {
              inherit cargoArtifacts;
              CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
              cargoExtraArgs = "-p mscz2xml";
              doCheck = false;
            });
        };

        formatter = pkgs.nixpkgs-fmt;
      });
}
