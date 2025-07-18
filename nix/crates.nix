{
  pkgs,
  lib,
  stdenv,
  crane,
  rust-toolchain,
  octez,
  riscvV8,
  riscv64MuslPkgs,
}: let
  craneLib = (crane.mkLib pkgs).overrideToolchain (_: rust-toolchain);

  # TODO(https://linear.app/tezos/issue/JSTZ-68):
  # Filter crate srcs into a file set to avoid rebuilding all
  # crates when a single crate changes.
  src = let
    # Include all Rust files / cargo related files
    # Include all files in the `contracts` and `crates` directories
    regexes = [".*\.toml$" ".*\.rs$" "^\.cargo.*$" "^Cargo.lock$" "^crates.*$" "^contracts.*$" "\.config.*$"];
  in
    lib.sourceByRegex (lib.cleanSource ../.) regexes;

  cargoVendorDir = let
    isDenoRepo = p: lib.hasPrefix "git+https://github.com/jstz-dev/deno" p.source;
  in
    craneLib.vendorCargoDeps {
      inherit src;
      overrideVendorGitCheckout = ps: drv:
        if lib.any isDenoRepo ps
        then
          drv.overrideAttrs (_old: {
            # Deno cli/bench crate depends on JS node modules which crane doesn't know
            # how to obtain. Since we don't need the cli crate, we remove it for simplicity
            patches = [
              ./patches/crane/0001-deno-remove-cli.patch
            ];

            # Deno sources at tests/util/std have relative symbolic links that fail when
            # crane tries to vendor them. We fix this by patching the symbolic links for
            # sources in the temp directory
            postPatch = ''
              mkdir -p $TMPDIR/source/tests/util/std/fs/testdata/copy_dir_link_file/
              ln -sf $src/tests/util/std/fs/testdata/copy_dir/0.txt $TMPDIR/source/tests/util/std/fs/testdata/copy_dir_link_file/0.txt
            '';
          })
        else drv;
    };

  common = with pkgs; {
    pname = "jstz";
    inherit src;
    inherit cargoVendorDir;

    # Needed to get openssl-sys (required by `jstz_proto`) to use pkg-config.
    nativeBuildInputs = lib.optionals stdenv.isLinux [pkg-config];

    # Needed to get openssl-sys to use pkg-config.
    # Doesn't seem to like OpenSSL 3
    OPENSSL_NO_VENDOR = 1;

    buildInputs =
      lib.optionals stdenv.isLinux [openssl openssl.dev]
      ++ lib.optionals
      stdenv.isDarwin
      (with darwin.apple_sdk.frameworks; [Security SystemConfiguration]);

    RUSTY_V8_ARCHIVE = pkgs.callPackage ./v8.nix {};
    RISCV_V8_ARCHIVE_DIR = "${riscvV8}";
    RUSTY_V8_SRC_BINDING_PATH = "${riscvV8}/src_binding.rs";
  };

  # Build *just* the workspace dependencies.
  # This is useful for caching the dependencies when in CI.
  cargoDeps = craneLib.buildDepsOnly common;

  jstz_kernel = craneLib.buildPackage (common
    // rec {
      inherit (craneLib.crateNameFromCargoToml {inherit src;}) version;
      cargoArtifacts = cargoDeps;
      doCheck = false;
      pname = "jstz_kernel";
      target = "wasm32-unknown-unknown";
      cargoExtraArgs = "-p ${pname} --target ${target}";
    });

  jstz_kernel_riscv = craneLib.buildPackage (common
    // rec {
      inherit (craneLib.crateNameFromCargoToml {inherit src;}) version;
      cargoArtifacts = cargoDeps;
      doCheck = false;
      buildInputs = common.buildInputs ++ [riscv64MuslPkgs.pkgsStatic.stdenv.cc];
      pname = "jstz_kernel";
      target = "riscv64gc-unknown-linux-musl";
      cargoExtraArgs = "-p ${pname} --target ${target} --no-default-features --features riscv_kernel";
    });

  # A common set of attributes for workspace crates
  commonWorkspace =
    common
    // {
      inherit (craneLib.crateNameFromCargoToml {inherit src;}) version;
      cargoArtifacts = cargoDeps;
      doCheck = false;
      buildInputs = common.buildInputs ++ [pkgs.iana-etc octez pkgs.cacert pkgs.sqlite];
      preBuildPhases = ["cpJstzKernel"];
      cpJstzKernel = ''
        cp ${jstz_kernel}/lib/jstz_kernel.wasm ./crates/jstzd/resources/jstz_rollup/jstz_kernel.wasm
      '';
    };

  # Build a crate in the workspace
  crate = pname:
    craneLib.buildPackage (commonWorkspace
      // {
        inherit pname;
        cargoExtraArgs = "-p ${pname}";
      });

  # Build a crate in the workspace for a specific target (cross compiled)
  # Uncomment when we have more than one target.
  #
  # crossCrate = pname: target:
  #   craneLib.buildPackage (commonWorkspace
  #     // {
  #       inherit pname;
  #       cargoExtraArgs = "-p ${pname} --target ${target}";
  #     });

  workspace = craneLib.cargoBuild commonWorkspace;
in let
  packages = {
    # A list of all the crates in the workspace
    # When adding a new crate, add it to this list
    # in alphabetical order.
    jstz_api = crate "jstz_api";
    jstz_cli = crate "jstz_cli";
    jstz_core = crate "jstz_core";
    jstz_crypto = crate "jstz_crypto";
    inherit jstz_kernel;
    inherit jstz_kernel_riscv;
    jstz_mock = crate "jstz_mock";
    jstz_node = crate "jstz_node";
    jstz_proto = crate "jstz_proto";
    jstz_runtime = crate "jstz_runtime";
    jstz_tps_bench = craneLib.buildPackage (commonWorkspace
      // rec {
        pname = "jstz_tps_bench";
        cargoExtraArgs = "-p ${pname} --features v2_runtime";
        doCheck = false;
      });
    jstz_wpt = crate "jstz_wpt";
    jstzd = craneLib.buildPackage (commonWorkspace
      // rec {
        pname = "jstzd";
        cargoExtraArgs = "-p ${pname}";
        preBuildPhases = ["mkJstzKernelForJstzd"];
        mkJstzKernelForJstzd = ''
          cp ${jstz_kernel}/lib/jstz_kernel.wasm ./crates/jstzd/resources/jstz_rollup/jstz_kernel.wasm
        '';
      });
    octez = crate "octez";

    # Special target to build all crates in the workspace
    all = workspace;
  };

  checks = {
    # Build the workspace as part of `nix flake check`

    cargo-test-unit = craneLib.cargoNextest (commonWorkspace
      // {
        buildInputs = commonWorkspace.buildInputs ++ [pkgs.iana-etc octez pkgs.cacert];
        doCheck = true;
        # Run the unit tests
        cargoNextestExtraArgs = "--bins --lib --features \"skip-wpt\",\"v2_runtime\" --config-file ${src}/.config/nextest.toml";
      });

    cargo-test-int = craneLib.cargoNextest (commonWorkspace
      // {
        buildInputs = commonWorkspace.buildInputs ++ [pkgs.iana-etc octez pkgs.cacert];
        doCheck = true;
        # Run the integration tests
        #
        # FIXME(https://linear.app/tezos/issue/JSTZ-237):
        # Fix tests that only fail in CI/Nix
        cargoNextestExtraArgs = "--test \"*\" --features \"skip-wpt\" --features \"skip-rollup-tests\",\"v2_runtime\" --config-file ${src}/.config/nextest.toml";
      });

    cargo-llvm-cov = craneLib.cargoLlvmCov (commonWorkspace
      // {
        buildInputs = commonWorkspace.buildInputs ++ [pkgs.cargo-nextest pkgs.iana-etc octez pkgs.cacert];
        # Use nextest for test harness (instead of `cargo test`)
        cargoLlvmCovCommand = "nextest";
        # Generate coverage reports for codecov
        cargoLlvmCovExtraArgs = "--codecov --output-path $out --features \"skip-rollup-tests\" --features \"skip-wpt\",\"v2_runtime\" --config-file ${src}/.config/nextest.toml";
      });

    cargo-clippy = craneLib.cargoClippy (commonWorkspace
      // {
        cargoClippyExtraArgs = "--all-targets -- --deny warnings";
      });
  };
in {
  inherit packages;
  inherit checks;

  apps = {
    tps_bench = let
      bench-script = pkgs.writeShellScriptBin "run-bench" ''
        ${packages.jstz_tps_bench}/bin/bench generate --transfers $transfer_count --inbox-file ./crates/jstz_tps_bench/src/kernel/inbox.json
        ls ${packages.jstz_kernel_riscv}
      '';
    in { type = "app"; program = "${bench-script}/bin/run-bench"; };
  };
}
