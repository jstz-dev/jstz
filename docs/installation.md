---
title: 📦 Installing Jstz
sidebar_label: Installation
---

## Download and Install

:::danger
⚠️ `jstz` is only available on Unix-based systems. ⚠️
:::

Ensure `docker` is installed on your system. If not, please follow [this guide](https://docs.docker.com/get-docker/).
Download and install `jstz` via NPM with this command:

```sh
npm i -g @jstz-dev/cli
```

or with yarn:

```sh
yarn global add @jstz-dev/cli
```

Congratulations! 🎉 `jstz` is now installed and configured on your system.
You are now ready to [write your first smart function](./quick_start.md) 🚀.

## Building from Source

Below are instruction on how to build `jstz` from source. Additionally, this section details how to install all the prerequisites needed to build `jstz` from sources.

### Cloning the Repository

```sh
git clone https://github.com/jstz-dev/jstz.git
```

### Prerequisites 📋

:::tip
Both `jstz` and Octez are packaged with Nix, a package manager and system configuration tool that makes building from sources easy! See the [Nix docs](https://nixos.org/download.html) for instructions for your system. Additionally, ensure [Nix flakes are enabled](https://nixos.wiki/wiki/Flakes#Enable_flakes).
:::

#### LLVM 🛠️

MacOS:

```sh
brew install llvm
export CC="$(brew --prefix llvm)/bin/clang"
```

Ubuntu:

```sh
sudo apt-get install clang-11
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

#### Rust 🦀

> `jstz` requires a specific release of Rust. The version of Rust required is specified in the `rust-toolchain` file.

Install the [Rust](https://rustup.rs/) toolchain:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### Octez 🐙

:::tip

The Nix shell ships with Octez binaries for convenience but it does take a little while to build for the very first time.
Skip ahead to [Building](#building-%EF%B8%8F)

:::
The jstz sandbox uses a custom distribution of Octez found [here](https://gitlab.com/tezos/tezos/-/tree/jstz@octez-dev). See the [Octez docs](https://tezos.gitlab.io/introduction/howtoget.html?highlight=building#compiling-with-make) for instructions on building Octez from source.

Once Octez has been built, copy the following binaries to `jstz`:

- `octez-client`
- `octez-node`
- `octez-smart-rollup-node`

### Building 👷‍♂️

:::tip
Using Nix, simply run `nix develop` to enter a shell with all build dependencies or use `direnv` to automatically enter the shell when you `cd` into the `jstz` directory.
:::

The kernel can be built with:

```sh
make build
```

You can locate the resulting build artifact at `/target/wasm32-unknown-unknown/release/jstz_kernel.wasm`.

### Running the Sandbox 🏝️

You can start the sandbox with:

```sh
make build-cli
PATH=.:$PATH cargo run --bin jstz -- sandbox start
```

This will initially run `octez-node` and initialize `octez-client`. Once the client is initialized, the `jstz_kernel` and `jstz_bridge`
is originated, a `octez-smart-rollup-node` and `jstz-node` is spun up.
