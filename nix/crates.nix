{
  pkgs,
  lib,
  stdenv,
  crane,
  rust-toolchain,
  octez,
  riscvV8,
  riscv64MuslCc,
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

  # RISC-V lightweight kernel (native executable)
  jstz_lightweight_kernel = craneLib.buildPackage (common
    // rec {
      inherit (craneLib.crateNameFromCargoToml {inherit src;}) version;
      cargoArtifacts = cargoDeps;
      doCheck = false;
      pname = "jstz_lightweight_kernel";
      target = "riscv64gc-unknown-linux-musl";
      cargoExtraArgs = "-p ${pname} --features lightweight-kernel --target ${target}";
      # Provide cross toolchain and V8 artifacts for RISC-V build
      buildInputs = (common.buildInputs or []) ++ [riscv64MuslCc];
      NIX_LDFLAGS = "";
      RUSTY_V8_ARCHIVE = "${riscvV8}/librusty_v8.a";
      RUSTY_V8_SRC_BINDING_PATH = "${riscvV8}/src_binding.rs";
    });

  # Fetch the necessary scripts for the runtime API coverage test in jstz_runtime
  apiCoverageTestScripts = with pkgs; let
    TARGET_SHA = "426ca553141d5ac41764beb9078bd27efd980756";
    baseline = fetchurl {
      url = "https://raw.githubusercontent.com/cloudflare/workers-nodejs-compat-matrix/${TARGET_SHA}/data/baseline.json";
      sha256 = "sha256-TuRKy5HRwkZD5Ng8kNsFQxhKBFKj/3uUwu66cjVCO1c=";
    };
    utils = fetchurl {
      url = "https://raw.githubusercontent.com/cloudflare/workers-nodejs-compat-matrix/${TARGET_SHA}/dump-utils.mjs";
      sha256 = "sha256-2NtB5g1nkezVQYTytOk6aR5uw5L1aMNJZQAMz6uC2yQ=";
    };
  in
    runCommand "fetch-scripts" {} ''
      mkdir -p $out
      cp ${baseline} $out/baseline.json
      cp ${utils} $out/utils.js
    '';

  # A common set of attributes for workspace crates
  commonWorkspace =
    common
    // {
      inherit (craneLib.crateNameFromCargoToml {inherit src;}) version;
      cargoArtifacts = cargoDeps;
      doCheck = false;
      buildInputs = common.buildInputs ++ [pkgs.iana-etc octez pkgs.cacert pkgs.sqlite];
      preBuildPhases = ["cpJstzKernel" "cpLightweightKernel" "setUpApiCoverageTest"];
      cpJstzKernel = ''
        cp ${jstz_kernel}/lib/jstz_kernel.wasm ./crates/jstzd/resources/jstz_rollup/jstz_kernel.wasm
      '';
      cpLightweightKernel = ''
        cp ${jstz_lightweight_kernel}/bin/lightweight-kernel-executable ./crates/jstzd/resources/jstz_rollup/lightweight-kernel-executable
      '';
      # This is the same as the script at `jstz_runtime/tests/api_coverage/setup.sh`.
      setUpApiCoverageTest = ''
        cp ${apiCoverageTestScripts}/utils.js ./crates/jstz_runtime/tests/api_coverage/
        (
          echo "export default "
          cat ${apiCoverageTestScripts}/baseline.json
        ) > ./crates/jstz_runtime/tests/api_coverage/baseline.js
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
    jstz_cli = crate "jstz_cli";
    jstz_core = crate "jstz_core";
    jstz_crypto = crate "jstz_crypto";
    inherit jstz_kernel jstz_lightweight_kernel;
    jstz_mock = crate "jstz_mock";
    jstz_node = crate "jstz_node";
    jstz_proto = crate "jstz_proto";
    jstz_runtime = crate "jstz_runtime";
    jstz_wpt = crate "jstz_wpt";
    jstzd = craneLib.buildPackage (commonWorkspace
      // rec {
        pname = "jstzd";
        cargoExtraArgs = "-p ${pname}";
        preBuildPhases = ["mkJstzKernelForJstzd" "mkLightweightKernelForJstzd"];
        mkJstzKernelForJstzd = ''
          cp ${jstz_kernel}/lib/jstz_kernel.wasm ./crates/jstzd/resources/jstz_rollup/jstz_kernel.wasm
        '';
        mkLightweightKernelForJstzd = ''
          cp ${jstz_lightweight_kernel}/bin/lightweight-kernel-executable ./crates/jstzd/resources/jstz_rollup/lightweight-kernel-executable
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
}
