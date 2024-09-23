{
  pkgs,
  crane,
  lib,
  stdenv,
  rust-toolchain,
}: let
  craneLib = (crane.mkLib pkgs).overrideToolchain (_: rust-toolchain);

  # TODO(https://linear.app/tezos/issue/JSTZ-68):
  # Filter crate srcs into a file set to avoid rebuilding all
  # crates when a single crate changes.
  src = let
    # Include all Rust files / cargo related files
    # Include all files in the `contracts` and `crates` directories
    regexes = [".*\.toml$" ".*\.rs$" "^\.cargo.*$" "^Cargo.lock$" "^crates.*$" "^contracts.*$"];
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
  };

  # Build *just* the workspace dependencies.
  # This is useful for caching the dependencies when in CI.
  cargoDeps = craneLib.buildDepsOnly common;

  # A common set of attributes for workspace crates
  commonWorkspace =
    common
    // {
      inherit (craneLib.crateNameFromCargoToml {inherit src;}) version;
      cargoArtifacts = cargoDeps;
      doCheck = false;

      # HACK
      # To build the `jstz_cli` crate, we need a `jstz_kernel.wasm` file
      # in the `jstz_cli` crate. This is a dummy kernel that is used to
      # build the `jstz_cli` crate. See the `jstz_cli` derivation below
      # for building the actual kernel.
      preBuildPhases = ["mkDummyJstzKernelForCli"];
      mkDummyJstzKernelForCli = ''
        touch ./crates/jstz_cli/jstz_kernel.wasm
      '';
    };

  workspace = craneLib.cargoBuild commonWorkspace;

  # Build a crate in the workspace
  crate = pname:
    craneLib.buildPackage (commonWorkspace
      // {
        inherit pname;
        cargoExtraArgs = "-p ${pname}";
      });

  # Build a crate in the workspace for a specific target (cross compiled)
  crossCrate = pname: target:
    craneLib.buildPackage (commonWorkspace
      // {
        inherit pname;
        cargoExtraArgs = "-p ${pname} --target ${target}";
      });
in {
  packages = let
    jstz_kernel = crossCrate "jstz_kernel" "wasm32-unknown-unknown";
  in {
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
    jstz_mock = crate "jstz_mock";
    jstz_node = crate "jstz_node";
    jstz_proto = crate "jstz_proto";
    jstz_rollup = crate "jstz_rollup";
    inherit jstz_kernel;
    jstz_wpt = crate "jstz_wpt";
    jstzd = crate "jstzd";
    octez = crate "octez";

    # Special target to build all crates in the workspace
    all = workspace;
  };

  checks = {
    # Build the workspace as part of `nix flake check`
    cargo-build = workspace;

    cargo-test-unit = craneLib.cargoNextest (commonWorkspace
      // {
        cargoArtifacts = cargoDeps;
        # Run the unit tests
        cargoNextestExtraArg = "--bins --lib";
      });

    cargo-test-int = craneLib.cargoNextest (commonWorkspace
      // {
        cargoArtifacts = cargoDeps;

        RUST_LOG = "debug";
        buildInputs = commonWorkspace.buildInputs ++ [pkgs.docker];
        # Run the integration tests
        #
        # FIXME():
        # Don't run the `jstz_api` integration tests until they've been paralellized
        #
        # Note: --workspace is required for --exclude. Once --exclude is removed, remove --workspace
        cargoNextestExtraArgs = "--workspace --test \"*\" --exclude \"jstz_api\" --no-capture";
      });

    cargo-llvm-cov = craneLib.cargoLlvmCov (commonWorkspace
      // {
        RUST_LOG = "debug";
        cargoArtifacts = cargoDeps;
        buildInputs = commonWorkspace.buildInputs ++ [pkgs.docker];
        # Generate coverage reports for codecov
        cargoLlvmCovExtraArgs = "--workspace --exclude-from-test \"jstz_api\" --no-capture --codecov --output-path $out";
      });

    cargo-clippy = craneLib.cargoClippy (commonWorkspace
      // {
        cargoArtifacts = cargoDeps;
        cargoClippyExtraArgs = "--all-targets -- --deny warnings";
      });
  };
}
