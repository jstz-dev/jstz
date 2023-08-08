{
  pkgs,
  makeRustPlatform,
}: let
  rustVersion = "1.66.1";

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
    };
  };
in {
  jstz_core = pkgs.rustPlatform.buildRustPackage (common
    // {
      pname = "jstz_core";
      cargoBuildFlags = "-p jstz_core";
    });

  jstz_api = pkgs.rustPlatform.buildRustPackage (common
    // {
      pname = "jstz_api";
      cargoBuildFlags = "-p jstz_api";
    });

  jstz_kernel = rustPlatformWasm.buildRustPackage (common
    // {
      pname = "jstz_kernel";

      NIX_CFLAGS_COMPILE = "-mcpu=generic";

      buildPhase = ''
        cargo build --release -p jstz_kernel --target=wasm32-unknown-unknown
      '';

      installPhase = ''
        mkdir -p $out/lib
        cp target/wasm32-unknown-unknown/release/*.wasm $out/lib/
      '';
    });
}
