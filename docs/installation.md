---
title: Installing Jstz
sidebar_label: Installation
---

Jstz has several [components](/architecture/overview#components), but the primary tool that you need to develop on Jstz is the [Jstz CLI](/cli), which is available as the NPM package [`@jstz-dev/cli`](https://www.npmjs.com/package/@jstz-dev/cli).
It allows you to start the Jstz sandbox, deploy smart functions to it, and interact with them.

:::note

Jstz is available only on Unix-based systems.

:::

Follow these instructions to install the Jstz CLI:

Ensure that [Docker](https://docs.docker.com/get-docker/) is installed on your system.
Then, download and install `jstz` via NPM with this command:

```sh
npm i -g @jstz-dev/cli
```

or with yarn:

```sh
yarn global add @jstz-dev/cli
```

Congratulations! 🎉 `jstz` is now installed and configured on your system.
You are now ready to [write your first smart function](/quick_start) 🚀.

## Building from source

For simplicity, you can install and initialize the Nix package management and system configuration tool, which provides the dependencies to build Jstz.
If you don't use Nix, you must install the dependencies yourself.

These sections show how to build Jstz on MacOS and Linux systems:

### Building on MacOS

Nix is the easiest way to build Jstz on MacOS:

1. Clone the Jstz repository:

   ```sh
   git clone https://github.com/jstz-dev/jstz.git
   ```

1. Install and configure Nix:

   1. Install Nix as described in its documentation: https://nixos.org/download.html.

   1. Ensure that Nix flakes are enabled: https://nixos.wiki/wiki/Flakes#Enable_flakes.

   1. Run `nix develop` to enter a shell with the build dependencies or use `direnv` to automatically enter the shell when you enter the `jstz` directory.

1. Build the Jstz kernel by running this command:

   ```sh
   make build
   ```

   You can locate the resulting built artifact at `/target/wasm32-unknown-unknown/release/jstz_kernel.wasm`.

1. Build and start the sandbox by running these commands:

   ```sh
   make build-cli
   PATH=.:$PATH cargo run --bin jstz -- sandbox start
   ```

This command runs `octez-node` and initializes `octez-client`, which are installed by Nix and required to start the sandbox.
When the client is initialized, it originates the `jstz_kernel` and `jstz_bridge` components and starts a `octez-smart-rollup-node` and `jstz-node`.

### Building on Linux

Nix is not required on Linux systems but it is easier than installing dependencies individually.

1. Clone the Jstz repository:

   ```sh
   git clone https://github.com/jstz-dev/jstz.git
   ```

1. (Optional) If you are using Nix, install and configure it:

   1. Install Nix as described in its documentation: https://nixos.org/download.html.

   1. Ensure that Nix flakes are enabled: https://nixos.wiki/wiki/Flakes#Enable_flakes.

   1. Run `nix develop` to enter a shell with the build dependencies or use `direnv` to automatically enter the shell when you enter the `jstz` directory.

1. If you are not using Nix, install these dependencies manually:

   1. Install LLVM:

      Ubuntu:

      ```sh
      sudo apt install clang
      export CC=clang
      ```

      Fedora:

      ```sh
      sudo dnf install clang
      export CC=clang
      ```

      Linux:

      ```sh Arch
      pacman -S clang
      export CC=clang
      ```

   1. From the `jstz` directory, install the specific version of Rust that Jstz requires, which is specified in the `rust-toolchain` file:

      ```sh
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
      ```

   1. Install the following Octez binaries and ensure that they are on your system `$PATH`:

      - `octez-client`
      - `octez-node`
      - `octez-smart-rollup-node`

      You can get the Octez suite of tools from the Octez release page here: https://gitlab.com/tezos/tezos/-/releases.

1. Build the Jstz kernel by running this command:

   ```sh
   make build
   ```

   You can locate the resulting built artifact at `/target/wasm32-unknown-unknown/release/jstz_kernel.wasm`.

1. Build and start the sandbox by running these commands:

   ```sh
   make build-cli
   PATH=.:$PATH cargo run --bin jstz -- sandbox start
   ```

This command runs `octez-node` and initializes `octez-client`.
When the client is initialized, it originates the `jstz_kernel` and `jstz_bridge` components and starts a `octez-smart-rollup-node` and `jstz-node`.
