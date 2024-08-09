{pkgs}: let
  inherit (builtins) fromTOML readFile;
  toolchain = (fromTOML (readFile ../rust-toolchain.toml)).toolchain;
in
  pkgs.rust-bin.fromRustupToolchain (toolchain
    // {
      # FIXME(https://linear.app/tezos/issue/JSTZ-69):
      # rust-overlay doesn't support riscv64gc-unknown-hermit, so
      # we override the targets to avoid a build failure.
      targets = ["wasm32-unknown-unknown"];
    })
