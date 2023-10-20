{
  pkgs,
  makeRustPlatform,
  buildNpmPackage,
  makeFrameworkFlags,
}: let
  # TODO: read this from the rust-toolchain file
  rustVersion = "1.71.0";

  wasmTarget = "wasm32-unknown-unknown";

  rustWithWasmTarget = pkgs.rust-bin.stable.${rustVersion}.default.override {
    targets = [wasmTarget];
  };

  rustPlatformWasm = makeRustPlatform {
    cargo = rustWithWasmTarget;
    rustc = rustWithWasmTarget;
    llvmPackages = pkgs.llvmPackages_16;
  };

  common = {
    version = "0.1.0";
    src = ../.;

    NIX_LDFLAGS = pkgs.lib.optional pkgs.stdenv.isDarwin (
      makeFrameworkFlags [
        "Security"
        "SystemConfiguration"
      ]
    );

    cargoLock = {
      lockFile = ../Cargo.lock;
      outputHashes = {
        "tezos-smart-rollup-0.2.1" = "sha256-ETMYanG7BINBhuKdALCShHhtLYSOCmG+Ak/G5QK88ks=";
        "boa_engine-0.17.0" = "sha256-bf6i5ESIHwepb1a4dUYREPprz7Rijq+P5z+NXpsT16Q=";
      };
    };
  };

  crate = pname:
    pkgs.rustPlatform.buildRustPackage (common
      // {
        pname = pname;
        cargoBuildFlags = "-p ${pname}";
      });

  kernel = pname:
    rustPlatformWasm.buildRustPackage (common
      // {
        pname = pname;

        NIX_CFLAGS_COMPILE = "-mcpu=generic";
        nativeBuildInputs = [pkgs.llvmPackages_16.clangNoLibc];

        buildPhase = ''
          CC=clang cargo build --release -p ${pname} --target=wasm32-unknown-unknown
        '';

        installPhase = ''
          mkdir -p $out/lib
          cp target/wasm32-unknown-unknown/release/*.wasm $out/lib/
        '';
      });

  jsPackage = pname:
    buildNpmPackage {
      name = pname;
      src = ../packages/${pname};
      npmDepsHash = "sha256-gHkv831Mknd7McNJzzvIf7s5gwdErdHtMti8nkZGBjk=";
    };
in {
  jstz_core = crate "jstz_core";

  jstz_api = crate "jstz_api";

  jstz_crypto = crate "jstz_crypto";

  jstz_proto = crate "jstz_proto";

  jstz_kernel = kernel "jstz_kernel";

  jstz_cli = crate "jstz_cli";

  jstz_node = crate "jstz_node";

  js_jstz = jsPackage "jstz";

  js_jstz-types = jsPackage "jstz-types";
}
