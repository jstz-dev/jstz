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
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";

    # NPM support
    # FIXME(https://linear.app/tezos/issue/JSTZ-70)
    # This is a temporary workaround for the ENOTCACHED error in the Nixpkgs buildNpmPackage derivation
    npm-buildpackage = {
      url = "github:serokell/nix-npm-buildpackage";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # Octez

    # We explicitly have opam-nix-integration as an input to avoid having two versions of nixpkgs
    opam-nix-integration = {
      url = "github:vapourismo/opam-nix-integration";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };

    octezPackages = {
      inputs.nixpkgs.follows = "nixpkgs";
      url = "gitlab:tezos/tezos/octez-v22.0-rc1";
      inputs.flake-utils.follows = "flake-utils";
      inputs.rust-overlay.follows = "rust-overlay";
      inputs.opam-nix-integration.follows = "opam-nix-integration";
    };
  };

  outputs = inputs:
    with inputs;
      flake-utils.lib.eachDefaultSystem (
        system: let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [
              (import ./nix/overlay.nix)
              (import rust-overlay)
              npm-buildpackage.overlays.default
            ];
          };

          # Build octez release for this system
          #
          # TODO(https://linear.app/tezos/issue/JSTZ-152):
          # This patch here should be upstreamed to tezos/tezos
          octez = octezPackages.packages.${system}.default.overrideAttrs (old: let
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
                ./nix/patches/0002-allow-floats-in-wasm-rollup.patch
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
            preBuild = let
              # Configure cargo to get dependencies from vendored dir
              vendorDeps = {
                dir,
                gitDepHashes ? {},
              }: let
                vendoredDir = rustPlatform.importCargoLock {
                  lockFile = "${old.src}/${dir}/Cargo.lock";
                  outputHashes = gitDepHashes;
                };
              in ''
                mkdir -p ${dir}/.cargo
                cat >> ${dir}/.cargo/config.toml << EOF
                [net]
                offline = true

                [source.crates-io]
                replace-with = "vendored-sources"

                [source.vendored-sources]
                directory = "${vendoredDir}"
                EOF
              '';
            in
              # HACK: For some spooky reason, vendoring dependencies does not work on MacOS
              # but does for Linux.
              pkgs.lib.optionalString (!pkgs.stdenv.isDarwin) ''
                ${vendorDeps {dir = "src/rust_deps";}}
                ${vendorDeps {dir = "src/rustzcash_deps";}}
              '';

            # The `buildPhase` for `octez` compiles *all* released and experimental executables for Octez.
            # However, many of these executables are unnecessary, leading to longer build times. Additionally, some
            # targets are not properly sandboxed for Nix. To address this, we specify the set of Octez executables
            # required by Jstz using the `OCTEZ_EXECUTABLES` environment variable. This overrides the default set
            # defined in the `experimental-release` target of the root Makefile.
            #
            # NOTE: When updating the protocol, remember to update the protocol versions for the Baker executables here.
            OCTEZ_EXECUTABLES = ''
              octez-client
              octez-node
              octez-smart-rollup-node
              octez-smart-rollup-wasm-debugger
              octez-baker-PsQuebec
              octez-baker-PsRiotum
              octez-baker-alpha
            '';

            # The build phase for `octez` does not execute the pre- and post-phase hooks as expected.
            # We require the `preBuild` hook to run to configure Cargo to use vendored dependencies
            # instead of making network calls to crates.io.
            buildPhase = ''
              runHook preBuild
              ${old.buildPhase}
              runHook postBuild
            '';

            nativeBuildInputs =
              (old.nativeBuildInputs or [])
              ++ [
                # See https://nixos.org/manual/nixpkgs/stable/#compiling-non-rust-packages-that-include-rust-code
                # for more information.
                #
                rustToolchain
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
          llvmPackages = pkgs.llvmPackages_16;

          crates = pkgs.callPackage ./nix/crates.nix {inherit crane rust-toolchain octez;};
          js-packages = pkgs.callPackage ./nix/js-packages.nix {};

          # It is necessary to use fetchurl instead of fetchTarball to
          # preserve the hash compatability among case (in/)sensitive file systems
          riscvV8 = with pkgs; let
            tarball = fetchurl {
              url = "https://raw.githubusercontent.com/jstz-dev/rusty_v8/130.0.7/librusty_v8.tar.gz";
              sha256 = "sha256-8cywAe9kofNPxCwdzdkegtlRPwlqqR986m25wvDWbyo=";
            };
          in
            runCommand "fetch-riscv-v8" {} ''
              mkdir -p $out
              tar -xzf ${tarball} -C $out --strip-components=1
            '';

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

            # NOTE: For language specific ignores, use the specific ignore files:
            #   rustfmt: use .rustfmt.toml
            #   prettier: use .prettierignore
            settings.global.excludes =
              # Build/install directories (ignored by all formatters)
              ["target" "result" "node_modules/**" "**/dist"]
              ++
              # Dot files
              [".direnv"]
              ++
              # Unsupported languages (LIGO, Docker)
              ["contracts/**" "Dockerfile"];
          };

          mkFrameworkFlags = frameworks:
            pkgs.lib.concatStringsSep " " (
              pkgs.lib.concatMap
              (
                framework: [
                  "-F${pkgs.darwin.apple_sdk.frameworks.${framework}}/Library/Frameworks"
                  "-framework ${framework}"
                ]
              )
              frameworks
            );

          riscv64MuslPkgs = let
            crossPkgs = import nixpkgs {
              inherit system;
              crossSystem.config = "riscv64-unknown-linux-musl";
            };
          in
            crossPkgs.pkgsCross.riscv64;
        in {
          packages =
            crates.packages
            // js-packages.packages
            // {
              inherit octez;
              default = self.packages.${system}.jstz_kernel;
            };
          checks = crates.checks // {formatting = fmt.config.build.check self;};
          apps = crates.apps;

          formatter = fmt.config.build.wrapper;

          # Rust dev environment
          devShells.default = pkgs.mkShell {
            CC = "clang";

            # This tells the 'cc' Rust crate to build using this C compiler when
            # targeting other architectures.
            CC_wasm32_unknown_unknown = "${clangNoArch}/bin/clang";
            CC_riscv64gc_unknown_hermit = "${clangNoArch}/bin/clang";

            RISCV_V8_ARCHIVE_DIR = "${riscvV8}";

            NIX_LDFLAGS = pkgs.lib.optionals pkgs.stdenv.isDarwin (
              mkFrameworkFlags [
                "SystemConfiguration"
                "Security"
                "Foundation"
              ]
            );

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
                # C toolchain
                llvmPackages.clangNoLibc
                llvmPackages.llvm # for llvm-objdump

                # Rust toolchain
                rust-toolchain
                rust-analyzer
                wabt
                wasm-pack
                cargo-sort
                cargo-nextest
                cargo-llvm-cov
                cargo-expand

                # JavaScript/TypeScript toolchain
                nodejs
                prefetch-npm-deps

                # Nix toolchain
                alejandra

                # Runtime dependencies
                sqlite # for jstz-node
                octez # for jstzd
                python39 # for running web-platform tests

                riscv64MuslPkgs.pkgsStatic.stdenv.cc
              ]
              ++ lib.optionals stdenv.isLinux [pkg-config openssl.dev];
          };
        }
      );
}
