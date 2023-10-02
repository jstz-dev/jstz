# 📦 Installing `jstz`

Currently, `jstz` can only be installed by building from sources using [Rust](https://www.rust-lang.org/).

## Download and Install

::: danger
⚠️ Under construction ⚠️
:::

## Building from Source

Below are instruction on how to build `jstz` from source. Additionally, this section
details how to install Octez, which is used for our local sandbox.

### Cloning the Repository

```sh
git clone https://github.com/trilitech/jstz.git
```

### Prerequisites 📋

::: tip  
Both `jstz` and Octez are packaged with Nix, a package manager and system configuration tool that makes building from sources easy! See the [Nix docs](https://nixos.org/download.html) for instructions for your system. Additionally, ensure [Nix flakes are enabled](https://nixos.wiki/wiki/Flakes#Enable_flakes).
:::

#### Rust 🦀

> `jstz` requires a specific release of Rust. The version of Rust required is specified in the `rust-toolchain` file.

Install the [Rust](https://rustup.rs/) toolchain:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Once `rustup` is installed, the build dependencies can be installed with:

```sh
make build-deps
```

::: tip
Using Nix, simply run `nix develop` to enter a shell with all build dependencies
:::

#### Octez 🐙

Our sandbox network uses a custom distribution of Octez found [here](https://gitlab.com/tezos/tezos/-/tree/6c0621760ddce94afeff3484d9e8a650d8535f25). See the [Octez docs](https://tezos.gitlab.io/introduction/howtoget.html?highlight=building#compiling-with-make) for instructions on building Octez from source.

Once Octez has been built, copy the following binaries to `jstz`:

- `octez-client`
- `octez-node`
- `octez-smart-rollup-node`
- `octez-smart-rollup-wasm-debugger`

::: tip

Using Nix, simply execute the following:

```sh
# Clone Octez
git clone git@gitlab.com:tezos/tezos.git
cd tezos
# Checkout custom distribution
git checkout ole@next-gen@floats
# Build using Nix
nix-build -j auto
```

After Nix successfully builds Octez (it takes a long time 🕣), the Octez binaries will be accessable from `result`.
:::

### Building 👷‍♂️

The `.wasm` file for `jstz`'s kernel is built with:

```sh
make build
```

You can locate the resulting build artifact at `/target/wasm32-unknown-unknown/release/jstz_kernel.wasm`.

### Running the Sandbox 🏝️

You can now start the sandbox with:

```sh
make build-installer
cargo run -- sandbox start
```

This will initially run `octez-node` and initialize `octez-client`. Once the client is initialized, the `jstz_kernel` and `jstz_bridge` is originated and a smart-rollup node is spun up.
