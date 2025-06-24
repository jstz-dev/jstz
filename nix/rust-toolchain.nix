{pkgs}: let
  inherit (builtins) fromTOML readFile;
  toolchain = (fromTOML (readFile ../rust-toolchain.toml)).toolchain;
in
  pkgs.rust-bin.fromRustupToolchain toolchain
