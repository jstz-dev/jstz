{
  pkgs,
  system,
  crane,
  octezPackages,
}:
octezPackages.packages.${system}.default.overrideAttrs (new: old: let
  rustToolchain = pkgs.rust-bin.fromRustupToolchainFile "${old.src}/rust-toolchain";
  craneLib = (crane.mkLib pkgs).overrideToolchain (_: rustToolchain);
in {
  # Build octez release for this system
  #
  # TODO(https://linear.app/tezos/issue/JSTZ-152):
  # This patch here should be upstreamed to tezos/tezos
  patches =
    (old.patches or [])
    ++ [
      ./patches/octez/0001-fix-octez-rust-deps-for-nix.patch
      ./patches/octez/0002-allow-floats-in-wasm-rollup.patch
    ];

  preBuild = let
    vendorDeps = {dir}: let
      vendoredDir = craneLib.vendorCargoDeps {
        cargoLock = "${old.src}/${dir}/Cargo.lock";
      };
    in ''
      mkdir -p ${dir}/.cargo
      cat >> ${dir}/.cargo/config.toml << EOF
      [net]
      offline = true

      [source.crates-io]
      replace-with = "vendored-sources"

      [source.vendored-sources]
      directory = "${vendoredDir}"
      EOF
    '';
  in ''
    ${vendorDeps {dir = "src/riscv";}}
    ${vendorDeps {dir = "src/rust_deps";}}
    ${vendorDeps {dir = "src/rustzcash_deps";}}
  '';

  # The `buildPhase` for `octez` compiles *all* released and experimental executables for Octez.
  # However, many of these executables are unnecessary, leading to longer build times. Additionally, some
  # targets are not properly sandboxed for Nix. To address this, we specify the set of Octez executables
  # required by Jstz using the `OCTEZ_EXECUTABLES` environment variable. This overrides the default set
  # defined in the `experimental-release` target of the root Makefile.
  #
  # NOTE: When updating the protocol, remember to update the protocol versions for the Baker executables here.
  OCTEZ_EXECUTABLES = ''
    octez-client
    octez-node
    octez-smart-rollup-node
    octez-baker-PtSeouLo
    octez-baker-PsRiotum
    octez-baker-alpha
  '';

  # The build phase for `octez` does not execute the pre- and post-phase hooks as expected.
  # We require the `preBuild` hook to run to configure Cargo to use vendored dependencies
  # instead of making network calls to crates.io.
  buildPhase = ''
    runHook preBuild
    ${old.buildPhase}
    runHook postBuild
  '';

  nativeBuildInputs =
    (old.nativeBuildInputs or [])
    ++ [
      # See https://nixos.org/manual/nixpkgs/stable/#compiling-non-rust-packages-that-include-rust-code
      # for more information.
      #
      rustToolchain
    ];
})
