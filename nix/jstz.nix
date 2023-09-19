{
  pkgs,
  makeRustPlatform,
}: let
  rustVersion = "1.70.0";

  wasmTarget = "wasm32-unknown-unknown";

  rustWithWasmTarget = pkgs.rust-bin.stable.${rustVersion}.default.override {
    targets = [wasmTarget];
  };

  rustPlatformWasm = makeRustPlatform {
    cargo = rustWithWasmTarget;
    rustc = rustWithWasmTarget;
    llvmPackages = pkgs.llvm_16;
  };

  common = {
    version = "0.1.0";
    src = ../.;

    cargoLock = {
      lockFile = ../Cargo.lock;
      outputHashes = {
        "tezos-smart-rollup-0.2.1" = "sha256-ETMYanG7BINBhuKdALCShHhtLYSOCmG+Ak/G5QK88ks=";
        "boa_engine-0.17.0" = "sha256-Gnt3rfF/XrwyR6kTiy3UO53vuez99ZCx31HLqJyXjB8=";
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

        buildPhase = ''
          cargo build --release -p ${pname} --target=wasm32-unknown-unknown
        '';

        installPhase = ''
          mkdir -p $out/lib
          cp target/wasm32-unknown-unknown/release/*.wasm $out/lib/
        '';
      });
in {
  jstz_core = crate "jstz_core";

  jstz_api = crate "jstz_api";

  jstz_crypto = crate "jstz_crypto";

  jstz_proto = crate "jstz_proto";

  jstz_kernel = kernel "jstz_kernel";
}
