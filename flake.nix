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
      url = "git+https://gitlab.com/tezos/tezos.git?rev=51117ed39f82ab60edd6fe4f6d63094605bb22c7&submodules=1";
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

            rustGitHashes = {
              "crypto-3.1.1" = "sha256-1jWMRi/uHYDxm1dgbyCbxe2htVA7vK7NFHKV4o6SpKQ=";
              "octez-riscv-0.0.0" = "sha256-bT5+4+M5//uemGcLq038siqfyCYKF5tzY39H9h+6RvA=";
              "quickcheck_derive-0.3.0" = "sha256-tTkcf/vE3GECIt1sriO80gnAB0MsTgAPokY8AeMoQkM=";
              "tezos-smart-rollup-build-utils-0.2.2" = "sha256-HXANyit8Hwuh7JaZ2/67lUa1qmPRmSM4myLES3AxyCA=";
            };

            rustGitHashes2 = {
              "crypto-3.1.1" = "sha256-1jWMRi/uHYDxm1dgbyCbxe2htVA7vK7NFHKV4o6SpKQ=";
              "octez-riscv-0.0.0" = "sha256-7TxDp0gltdoAC1Yhbb/roPbHBZYirlgcBaFROtYJYWw=";
              "quickcheck_derive-0.3.0" = "sha256-tTkcf/vE3GECIt1sriO80gnAB0MsTgAPokY8AeMoQkM=";
              "tezos-smart-rollup-build-utils-0.2.2" = "sha256-Z6Z3Jti5J4YzDKdsaZ5i/YdaSTctbPGmj5nMlOG7RuA=";
            };
          in {
            patches =
              (old.patches or [])
              ++ [
                ./nix/patches/octez/0001-fix-octez-rust-deps-for-nix.patch
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
              # For each lockfile, vendor and collect the git sources it references.
              vendorInfo = {
                dir,
                hashes ? {},
              }: let
                lockPath = "${old.src}/${dir}/Cargo.lock";
                lockToml = builtins.fromTOML (builtins.readFile lockPath);
                isGit = p: pkgs.lib.hasPrefix "git+" (p.source or "");
                gitPkgs = pkgs.lib.filter isGit (lockToml.package or []);
                gitKeys = map (p: "${p.name}-${p.version}") gitPkgs;
                vendoredDir = rustPlatform.importCargoLock {
                  lockFile = lockPath;
                  # Only pass hashes that actually appear in THIS lockfile
                  outputHashes = pkgs.lib.attrsets.filterAttrs (k: _v: pkgs.lib.elem k gitKeys) hashes;
                };
                gitSources = pkgs.lib.unique (map (p: p.source) gitPkgs);
              in {inherit vendoredDir gitSources;};

              vi_rust_deps = vendorInfo {
                dir = "src/rust_deps";
                hashes = rustGitHashes2;
              };
              vi_riscv = vendorInfo {
                dir = "src/riscv";
                hashes = rustGitHashes;
              };
              vi_rustzcash = vendorInfo {
                dir = "src/rustzcash_deps";
                hashes = rustGitHashes2;
              };

              # Merge all vendored outputs into a single directory so Cargo can use a single source.
              combinedVendor = pkgs.runCommand "cargo-vendor-all" {} ''
                mkdir -p $out
                cp -R ${vi_rust_deps.vendoredDir}/.   $out/ || true
                cp -R ${vi_riscv.vendoredDir}/.       $out/ || true
                cp -R ${vi_rustzcash.vendoredDir}/.   $out/ || true
              '';

              # Create [source."git+…"] sections for every distinct git source from both lockfiles.
              gitSources =
                pkgs.lib.unique (vi_rust_deps.gitSources ++ vi_riscv.gitSources ++ vi_rustzcash.gitSources);
              gitSourceSections = pkgs.lib.concatStringsSep "\n" (map (src: ''
                  [source."${src}"]
                  replace-with = "vendored-sources"
                '')
                gitSources);
            in
              # HACK: Does not run on macOS to match prior behavior.
              pkgs.lib.optionalString (!pkgs.stdenv.isDarwin) ''
                                export CARGO_HOME="$PWD/.cargo-home"
                                mkdir -p "$CARGO_HOME"

                                cat > "$CARGO_HOME/config.toml" << 'EOF'
                [net]
                offline = true

                [source.crates-io]
                replace-with = "vendored-sources"

                ${gitSourceSections}

                [source.vendored-sources]
                directory = "${combinedVendor}"
                EOF
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
              octez-baker-PsRiotum
              octez-baker-PtSeouLo
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

          riscvSandbox = with builtins; let
            craneLib = (crane.mkLib pkgs).overrideToolchain (_: rust-toolchain);
            fetchedSrc = fetchGit {
              url = "https://github.com/tezos/riscv-pvm.git";
              rev = "0de5159bcd6a25cb32249b161de19d5a72e1272c";
            };
            sandboxManifest = fromTOML (readFile "${fetchedSrc}/src/riscv/sandbox/Cargo.toml");
          in
            # Note on `craneLib` vs `buildRustPackage`
            #
            # `buildRustPackage` will attempt to vendor all dependencies in a workspace. Because
            # riscv sandbox depends on `tezos-smart-rollup-*` crates (which is a tezos workpace crate),
            # `buildRustPackage` vendors irrelevant dependencies from `tezos/tezos` like `rust_deps` which
            # tries to build `wasmer` and fails. Its overrides are completely broken. `craneLib` does the
            # right thing by only building the exact nested dependencies even if they were workpace dependent
            craneLib.buildPackage rec {
              src = "${fetchedSrc}/src/riscv";
              pname = sandboxManifest.package.name;
              version = sandboxManifest.package.version;
              doCheck = false;
              cargoExtraArgs = "--package ${pname} --features huge-memory";
            };

          llvmPackages = pkgs.llvmPackages_16;

          crates = pkgs.callPackage ./nix/crates.nix {inherit crane rust-toolchain octez;};
          js-packages = pkgs.callPackage ./nix/js-packages.nix {};

          # It is necessary to use fetchurl instead of fetchTarball to
          # preserve the hash compatability among case (in/)sensitive file systems
          riscvV8 = with pkgs; let
            tarball = fetchurl {
              url = "https://raw.githubusercontent.com/jstz-dev/rusty_v8/63dcedfc7ba101a4bbf4ce9fd94bba8ff71f8824/librusty_v8.tar.gz";
              sha256 = "sha256-Wi4guXiewq9zmAme5Oos31Gq4YJ5Oh2/yOxdm+NUPhM=";
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
              # Toml files (the only formatter available is toml-sort but it's problematic with Cargo.toml)
              ["*.toml"]
              ++
              # Test files
              ["crates/jstzd/tests/toy_rollup/**" "crates/jstzd/tests/resources/rollup/sr1Uuiucg1wk5aovEY2dj1ZBsqjwxndrSaao/**"]
              ++
              # Resource files
              ["crates/octez/resources/protocol_parameters/sandbox/**" "crates/jstz_node/src/services/logs/create_db.sql" "crates/jstz_wpt/hosts" "crates/jstz_wpt/wpt" "*.png" "*.umdx"]
              ++
              # Miscellaneous files
              ["*/**/.gitignore" "Makefile" "LICENSE" ".dockerignore" ".env.example" ".prettierignore" ".prettierrc"]
              ++
              # Unsupported languages (LIGO, Docker)
              ["contracts/**" "*/**/Dockerfile"];
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
          heaptrackNoGui = pkgs.heaptrack.overrideAttrs (old: {
            postInstall = ''
              ${old.postInstall}
              rm $out/bin/heaptrack_gui
            '';
          });
        in {
          packages =
            crates.packages
            // js-packages.packages
            // {
              inherit octez;
              default = self.packages.${system}.jstz_kernel;
            };
          checks = crates.checks // {formatting = fmt.config.build.check self;};

          formatter = fmt.config.build.wrapper;

          # Rust dev environment
          devShells.default = pkgs.mkShell {
            CC = "clang";

            # This tells the 'cc' Rust crate to build using this C compiler when
            # targeting other architectures.
            CC_wasm32_unknown_unknown = "${clangNoArch}/bin/clang";

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
                  ''
                    if [ ! -f ".git/hooks/pre-commit" ]; then
                      ./scripts/install-hooks.sh
                    fi
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
                riscvSandbox
              ]
              ++ lib.optionals stdenv.isLinux [
                pkg-config
                openssl.dev
                heaptrackNoGui
              ];
          };
        }
      );
}
