{
  fetchurl,
  stdenv,
}: let
  # `mozjs` builds SpiderMonkey automatically unless given an archive.
  # Building SpiderMonkey is rather slow, so instead we rely on a pre-build
  # archive distributed here: https://github.com/servo/mozjs/releases/tag
  #
  # See https://github.com/jstz-dev/jstz/pull/?? for more information
  # on how we could build mozjs ourselves (and some of the issues we ran into).
  # TODO():
  # Extract the tag from our Cargo.toml file
  #
  # NOTE: This tag must be updated when the `mozjs-sys` crate is updated
  tag = "mozjs-sys-v0.128.3-0";
  target = stdenv.hostPlatform.rust.rustcTarget;
  hashes = {
    "x86_64-apple-darwin" = "sha256-kt+vfs1qZJ9lcwBwINvMq6Jm4DtNa3aqcNdd9gFLb+o=";
    "aarch64-apple-darwin" = "sha256-sFi3zl03t9dZxpqdYPyS4yAzY1OozdP0aw9lKiG47lk=";
    "x86_64-unknown-linux-gnu" = "sha256-mbzLtbSMsLSfmqJp2Sy8PdwWV2bVaeTRePNsL6QQlS8=";
  };
in
  fetchurl {
    url = "https://github.com/servo/mozjs/releases/download/${tag}/libmozjs-${target}.tar.gz";
    hash = hashes.${target};
  }
