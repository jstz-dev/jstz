{
  pkgs,
  lib,
  stdenv,
  crane,
  rust-toolchain,
  octez,
  mozjs,
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

  common = with pkgs; {
    pname = "jstz";
    inherit src;

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

    MOZJS_ARCHIVE = mozjs;
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

  # A common set of attributes for workspace crates
  commonWorkspace =
    common
    // {
      inherit (craneLib.crateNameFromCargoToml {inherit src;}) version;
      cargoArtifacts = cargoDeps;
      doCheck = false;
      buildInputs = common.buildInputs ++ [pkgs.iana-etc octez pkgs.cacert];
      preBuildPhases = ["cpJstzKernel"];
      cpJstzKernel = ''
        cp ${jstz_kernel}/lib/jstz_kernel.wasm ./crates/jstz_cli/jstz_kernel.wasm
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
in {
  packages = {
    # A list of all the crates in the workspace
    # When adding a new crate, add it to this list
    # in alphabetical order.
    jstz_api = crate "jstz_api";
    jstz_cli = craneLib.buildPackage (commonWorkspace
      // rec {
        pname = "jstz_cli";
        cargoExtraArgs = "-p ${pname}";
        # The `jstz_cli` crate depends on the `jstz_kernel` crate
        # to build the `jstz_kernel.wasm` file.
        preBuildPhases = ["mkJstzKernelForCli"];
        mkJstzKernelForCli = ''
          cp ${jstz_kernel}/lib/jstz_kernel.wasm ./crates/jstz_cli/jstz_kernel.wasm
        '';
      });
    jstz_core = crate "jstz_core";
    jstz_crypto = crate "jstz_crypto";
    jstz_engine = crate "jstz_engine";
    inherit jstz_kernel;
    jstz_mock = crate "jstz_mock";
    jstz_node = crate "jstz_node";
    jstz_proto = crate "jstz_proto";
    jstz_rollup = crate "jstz_rollup";
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
        cargoNextestExtraArgs = "--bins --lib";
      });

    cargo-test-int = craneLib.cargoNextest (commonWorkspace
      // {
        buildInputs = commonWorkspace.buildInputs ++ [pkgs.iana-etc octez pkgs.cacert];
        doCheck = true;
        # Run the integration tests
        #
        # FIXME(https://linear.app/tezos/issue/JSTZ-186):
        # Don't run the `jstz_api` integration tests until they've been paralellized
        #
        # Note: --workspace is required for --exclude. Once --exclude is removed, remove --workspace
        # FIXME(https://linear.app/tezos/issue/JSTZ-237):
        # Fix tests that only fail in CI/Nix
        cargoNextestExtraArgs = "--workspace --test \"*\" --exclude \"jstz_api\" --features \"skip-rollup-tests\" --config-file ${src}/.config/nextest.toml";
      });

    cargo-llvm-cov = craneLib.cargoLlvmCov (commonWorkspace
      // {
        buildInputs = commonWorkspace.buildInputs ++ [pkgs.cargo-nextest pkgs.iana-etc octez pkgs.cacert];
        # Use nextest for test harness (instead of `cargo test`)
        cargoLlvmCovCommand = "nextest";
        # Generate coverage reports for codecov
        cargoLlvmCovExtraArgs = "--workspace --exclude-from-test \"jstz_api\" --codecov --output-path $out --features \"skip-rollup-tests\" --config-file ${src}/.config/nextest.toml";
      });

    cargo-clippy = craneLib.cargoClippy (commonWorkspace
      // {
        cargoClippyExtraArgs = "--all-targets -- --deny warnings";
      });
  };
}
