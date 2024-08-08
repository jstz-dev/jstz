final: prev: {
  cargo-llvm-cov = prev.cargo-llvm-cov.overrideAttrs (old: {
    doCheck = false;

    meta =
      old.meta
      // {
        # Nixpkgs currently marks the `cargo-llvm-cov` package as broken on Darwin.
        # This is however not the case :) It appears to work just fine (only the tests
        # are broken, but we disable them anyway).
        broken = with prev; old.meta.broken && !stdenv.isDarwin;
      };
  });
}
