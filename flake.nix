{
  nixConfig = {
    extra-trusted-public-keys = "trilitech-jstz.cachix.org-1:+ShRijHoxI9xAIZRP6Mov3aFui5FvgMHJ2M360OEYTo=";
    extra-substituters = "https://trilitech-jstz.cachix.org";
  };

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    treefmt = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # Rust support
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # NPM support
    # FIXME(https://linear.app/tezos/issue/JSTZ-70)
    # This is a temporary workaround for the ENOTCACHED error in the Nixpkgs buildNpmPackage derivation
    npm-buildpackage = {
      url = "github:serokell/nix-npm-buildpackage";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # Octez

    octez-v21 = {
      # pin octez-v21 to a specific commit until the next release is available
      url = "gitlab:tezos/tezos/c6c7373f31917d1bcf1fbc6550937b3ae1d3d748";
    };
  };

  outputs = inputs:
    with inputs;
      flake-utils.lib.eachDefaultSystem (
        system: let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [(import ./nix/overlay.nix) (import rust-overlay) npm-buildpackage.overlays.default];
          };

          # Build octez release for this system
          #
          # TODO(https://linear.app/tezos/issue/JSTZ-152):
          # This patch here should be upstreamed to tezos/tezos
          octez = octez-v21.packages.${system}.default.overrideAttrs (old: let
            rustToolchain = pkgs.rust-bin.fromRustupToolchainFile "${old.src}/rust-toolchain";
            rustPlatform = pkgs.makeRustPlatform {
              rustc = rustToolchain;
              cargo = rustToolchain;
            };
          in {
            patches =
              (old.patches or [])
              ++ [
                ./nix/patches/0001-fix-octez-rust-deps-for-nix.patch
              ];

            # Network access for fetching cargo dependencies is disabled in sandboxed
            # builds. Instead we need to explicitly fetch the dependencies. Nixpkgs
            # provides two ways to do this:
            #
            #  - `fetchCargoTarball` fetches the dependencies using `cargo vendor`
            #     It requires an explicit `hash`.
            #
            #  - `importCargoLock` parses the `Cargo.lock` file and fetches each
            #     dependency using `fetchurl`. It doesn't require an explicit `hash`.
            #
            # The latter is slower but doesn't require an explicit `hash` and is therefore
            # more maintainable (since this derivation isn't built in CI).
            cargoDeps = rustPlatform.importCargoLock {
              lockFile = "${old.src}/src/rust_deps/Cargo.lock";
            };
            cargoRoot = "src/rust_deps";

            nativeBuildInputs =
              (old.nativeBuildInputs or [])
              ++ [
                # See https://nixos.org/manual/nixpkgs/stable/#compiling-non-rust-packages-that-include-rust-code
                # for more information.
                #
                # `cargoSetupHook` configures cargo to vendor dependencies using `cargoDeps`.
                rustToolchain
                rustPlatform.cargoSetupHook
              ];
          });

          clangNoArch =
            if pkgs.stdenv.isDarwin
            then
              pkgs.clang.overrideAttrs (old: {
                postFixup = ''
                  ${old.postFixup or ""}

                  # On macOS this contains '-march' and '-mcpu' flags. These flags
                  # would be used for any invocation of Clang.
                  # Removing those makes the resulting Clang wrapper usable when
                  # cross-compiling where passing '-march' and '-mcpu' would not
                  # make sense.
                  echo > $out/nix-support/cc-cflags-before
                '';
              })
            else pkgs.clang;

          rust-toolchain = pkgs.callPackage ./nix/rust-toolchain.nix {};
          crates = pkgs.callPackage ./nix/crates.nix {inherit crane rust-toolchain octez;};
          js-packages = pkgs.callPackage ./nix/js-packages.nix {};

          fmt = treefmt.lib.evalModule pkgs {
            projectRootFile = "flake.nix";

            programs.rustfmt.enable = true;
            programs.alejandra.enable = true;
            programs.prettier.enable = true;
            programs.shfmt.enable = true;

            # TODO(https://linear.app/tezos/issue/JSTZ-64)
            # Configure shellcheck for shell scripts
            # programs.shellcheck.enable = true;

            # TODO(https://linear.app/tezos/issue/JSTZ-63)
            # Configure formatter for LIGO contracts
            settings.global.excludes = ["target" "result" "node_modules/**" ".github" ".direnv" "contracts/**" "Dockerfile" "*.toml"];
          };
        in {
          packages = crates.packages // js-packages.packages // {default = self.packages.${system}.jstz_kernel;};
          checks = crates.checks // {formatting = fmt.config.build.check self;};

          formatter = fmt.config.build.wrapper;

          # Rust dev environment
          devShells.default = pkgs.mkShell {
            CC = "clang";

            # This tells the 'cc' Rust crate to build using this C compiler when
            # targeting other architectures.
            CC_wasm32_unknown_unknown = "${clangNoArch}/bin/clang";
            CC_riscv64gc_unknown_hermit = "${clangNoArch}/bin/clang";

            hardeningDisable =
              pkgs.lib.optionals
              (pkgs.stdenv.isAarch64 && pkgs.stdenv.isDarwin)
              ["stackprotector"];

            shellHook = with pkgs;
              lib.strings.concatLines
              ([
                  # FIXME(https://linear.app/tezos/issue/JSTZ-70)
                  # npm-buildpackage does not support version 3 package-lock.json files
                  # We need to use version 2 until it does or find a workaround the ENOTCACHED error
                  # in the Nixpkgs buildNpmPackage derivation.
                  ''
                    npm install --lockfile-version 2
                    export PATH="$PWD/node_modules/.bin/:$PATH"
                  ''
                ]
                ++ lib.optionals stdenv.isLinux [
                  ''
                    export PKG_CONFIG_PATH=${openssl.dev}/lib/pkgconfig
                  ''
                ]);

            buildInputs = with pkgs;
              [
                llvmPackages_16.clangNoLibc
                rust-toolchain
                rust-analyzer
                wabt
                wasm-pack
                cargo-sort
                cargo-nextest

                nodejs
                prefetch-npm-deps

                alejandra

                sqlite

                # Code coverage
                cargo-llvm-cov
                octez
              ]
              ++ lib.optionals stdenv.isLinux [pkg-config openssl.dev]
              ++ lib.optionals stdenv.isDarwin (with darwin.apple_sdk.frameworks; [Security SystemConfiguration]);
          };
        }
      );
}
