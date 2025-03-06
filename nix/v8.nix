{
  fetchurl,
  stdenv,
}: let
  # `v8` downloads its archive in its build.rs which breaks the CI sandbox.
  # Pre-emptively download the archive here instead.
  #
  # NOTE: This tag must be updated when the `v8` crate is updated
  tag = "v130.0.7";
  target = stdenv.hostPlatform.rust.rustcTarget;
  hashes = {
    "aarch64-apple-darwin" = "sha256-9tvQD08OdW6GoNnx/3vgS27D9Aj9YdQdNJ9SgNvwAOo=";
    "x86_64-unknown-linux-gnu" = "sha256-pkdsuU6bAkcIHEZUJOt5PXdzK424CEgTLXjLtQ80t10=";
  };
in
  fetchurl {
    url = "https://github.com/denoland/rusty_v8/releases/download/${tag}/librusty_v8_release_${target}.a.gz";
    hash = hashes.${target};
  }
