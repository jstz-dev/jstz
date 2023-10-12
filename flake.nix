{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = inputs:
    with inputs;
      flake-utils.lib.eachDefaultSystem (
        system: let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [(import rust-overlay)];
          };
          jstz = pkgs.callPackage ./nix/jstz.nix {};
        in {
          packages = {
            inherit (jstz) jstz_core jstz_api jstz_crypto jstz_proto jstz_kernel jstz_cli js_jstz js_jstz-types;
            default = jstz.jstz_kernel;
          };

          # Rust dev environment
          devShells.default = pkgs.mkShell rec {
            NIX_CFLAGS_COMPILE = "-mcpu=generic";
            CC = "clang";

            shellHook = ''
              npm install
              export PATH="$PWD/node_modules/.bin/:$PATH"
            '';

            buildInputs = with pkgs; [
              llvmPackages_16.clangNoLibc
              (rust-bin.stable."1.71.0".default.override {
                targets = ["wasm32-unknown-unknown"];
              })
              rust-analyzer
              wabt

              nodejs
              prefetch-npm-deps

              alejandra

              python311Packages.base58
              jq
            ];
          };
        }
      );
}
