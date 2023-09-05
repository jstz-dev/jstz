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
            inherit jstz;
            default = jstz.jstz_kernel;
          };

          # Rust dev environment
          devShells.default = pkgs.mkShell {
            NIX_CFLAGS_COMPILE = "-mcpu=generic";

            buildInputs = with pkgs; [
              alejandra
              (rust-bin.stable."1.66.0".default.override {
                targets = ["wasm32-unknown-unknown"];
              })

              rust-analyzer
              wabt

              python311Packages.base58
              jq
            ];
          };
        }
      );
}
