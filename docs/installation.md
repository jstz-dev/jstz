# 📦 Installing `jstz`

## Download and Install

::: danger
⚠️ `jstz` is only available on Unix-based systems. ⚠️
:::

Ensure `docker` is installed on your system. If not, please follow [this guide](https://docs.docker.com/get-docker/).
To download and install `jstz`, run the following command in your terminal:

```sh
source <(curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/jstz-dev/jstz/main/scripts/install-jstz-cli.sh)
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

::: tip  
Both `jstz` and Octez are packaged with Nix, a package manager and system configuration tool that makes building from sources easy! See the [Nix docs](https://nixos.org/download.html) for instructions for your system. Additionally, ensure [Nix flakes are enabled](https://nixos.wiki/wiki/Flakes#Enable_flakes).
:::

#### LLVM 🛠️

::: code-group

```sh [MacOS]
brew install llvm
export CC="$(brew --prefix llvm)/bin/clang"
```

```sh [Ubuntu]
sudo apt-get install clang-11
export CC=clang-11
```

```sh [Fedora]
sudo dnf install clang
export CC=clang
```

```sh [Arch Linux]
pacman -S clang
export CC=clang
```

```sh [Nix]
nix-env -iA nixpkgs.llvm
```

:::

#### Rust 🦀

> `jstz` requires a specific release of Rust. The version of Rust required is specified in the `rust-toolchain` file.

Install the [Rust](https://rustup.rs/) toolchain:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### Octez 🐙

The jstz sandbox uses a custom distribution of Octez found [here](https://gitlab.com/tezos/tezos/-/tree/jstz@octez-dev). See the [Octez docs](https://tezos.gitlab.io/introduction/howtoget.html?highlight=building#compiling-with-make) for instructions on building Octez from source.

Once Octez has been built, copy the following binaries to `jstz`:

- `octez-client`
- `octez-node`
- `octez-smart-rollup-node`

::: tip

Using Nix, simply execute the following:

```sh
# Clone Octez
git clone git@gitlab.com:tezos/tezos.git && cd tezos
# Checkout custom distribution for jstz
git checkout jstz@octez-dev
# Build using Nix
nix-build -j auto
```

After Nix successfully builds Octez (it takes a long time 🕣), the Octez binaries will be accessable from `result`.
:::

### Building 👷‍♂️

::: tip
Using Nix, simply run `nix develop` to enter a shell with all build dependencies or use `direnv` to automatically enter the shell when you `cd` into the `jstz` directory.
:::

Additional build dependencies can be installed with:

```sh
make build-deps
```

The `.wasm` file for `jstz`'s kernel is built with:

```sh
make build-kernel
```

You can locate the resulting build artifact at `/target/wasm32-unknown-unknown/release/jstz_kernel.wasm`.

### Running the Sandbox 🏝️

You can start the sandbox with:

```sh
make build-cli-kernel
PATH=.:$PATH cargo run --bin jstz -- sandbox start
```

This will initially run `octez-node` and initialize `octez-client`. Once the client is initialized, the `jstz_kernel` and `jstz_bridge` is originated, a `octez-smart-rollup-node` and `jstz-node` is spun up.
