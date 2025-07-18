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

Follow these steps to build Jstz from its source code:

1. Install the Nix package management and system configuration tool.
   See https://nixos.org/download.html.

1. Ensure that Nix flakes are enabled: https://nixos.wiki/wiki/Flakes#Enable_flakes.

1. Clone the repository:

   ```sh
   git clone https://github.com/jstz-dev/jstz.git
   ```

1. Install LLVM:

   MacOS:

   ```sh
   brew install llvm
   export CC="$(brew --prefix llvm)/bin/clang"
   ```

   Ubuntu:

   ```sh
   sudo apt install clang-11
   export CC=clang-11
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

   Nix:

   ```sh
   nix-env -iA nixpkgs.llvm
   ```

1. From the `jstz` directory, install the specific version of Rust that Jstz requires, which is specified in the `rust-toolchain` file:

   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

1. Download the following Octez binaries to the `jstz` directory:

   - `octez-client`
   - `octez-node`
   - `octez-smart-rollup-node`

   You can get the Octez suite of tools from the Octez release page here: https://gitlab.com/tezos/tezos/-/releases.

1. If you are using Nix, run `nix develop` to enter a shell with all build dependencies or use `direnv` to automatically enter the shell when you `cd` into the `jstz` directory.

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
