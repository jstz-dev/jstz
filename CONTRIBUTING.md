# Contributing 👩‍💻

Before contributing to `jstz`, please read these guidelines carefully.

## Getting Started

### Setting up your environment 🌿

Install the [Rust](https://rustup.rs/) toolchain:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Once `rustup` is installed, the build dependencies can be installed with:

```sh
make build-deps
```

Alternatively, `jstz` is packaged with Nix. See the [Nix docs](https://nixos.org/download.html) for instructions for your system.
Additionally, ensure [Nix flakes are enabled](https://nixos.wiki/wiki/Flakes#Enable_flakes).

Once Nix is installed, the dev environment can be built with:

```sh
nix develop
```

### Building 👷‍♂️

The `.wasm` file for `jstz`'s kernel is built with:

```sh
make build
```

You can locate the resulting build artifact at `/target/wasm32-unknown-unknown/release/jstz_kernel.wasm`.

To build the installer for `jstz`, execute the following:

```sh
make build-installer
```

### Running `jstz` locally ⚙️

#### Installing Octez 🐙

Our sandbox network uses a custom distribution of Octez found [here](https://gitlab.com/tezos/tezos/-/tree/6c0621760ddce94afeff3484d9e8a650d8535f25). See the [Octez docs](https://tezos.gitlab.io/introduction/howtoget.html?highlight=building#compiling-with-make) for instructions on building Octez from source.

Alternative, with Nix, execute the following:

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

Once Octez has been built, copy the following binaries to `jstz`:

- `octez-client`
- `octez-node`
- `octez-smart-rollup-node`
- `octez-smart-rollup-wasm-debugger`

### Running the Sandbox 🏖️

You can now start the sandbox with:

```sh
cargo run -- sandbox start
```

This will initially run `octez-node` and initialize `octez-client`. Once the client is initialized, the `jstz_kernel` and `jstz_bridge` is originated and a smart-rollup node is spun up.

## Hacking on `jstz` 👨‍⚖️

<!-- TODO -->

### `jstz` Crates

- [**`jstz_core`**](/jstz_core) - `jstz`'s core functionality: host functions, transactional storage, and execution.
- [**`jstz_api`**](/jstz_api) - `jstz`'s JavaScript web standard runtime apis.
- [**`jstz_kernel`**](/jstz_kernel) - `jstz`'s smart rollup kernel, compiled to WASM.
- [**`jstz_crypto`**](/jstz_crypto) - `jstz`'s crypto library. Primarily a wrapper around `tezos_crypto_rs`.
- [**`jstz_proto`**](/jstz_proto) - `jstz`'s protocol: `jstz` specific runtime apis, storage context, execution of operations.
- [**`jstz_cli`**](/jstz_cli) - `jstz`'s client CLI tool: used to create, call, and manage `jstz` contracts and accounts.

### Testing ✅

Unit and integration tests can be run using:

```sh
make test
```

To run `jstz_kernel` in debug mode, the `octez-smart-rollup-wasm-debugger` should be used.

```sh
make build
./octez-smart-rollup-wasm-debugger \
    --kernel ./target/wasm32-unknown-unknown/release/jstz_kernel.wasm \
    --inputs ./inputs.json
```

Once the REPL loads, to populate the rollup inbox, run:

```sh
> load inputs
```

To run the kernel with the inputs:

```sh
> step inbox
```

## Creating a pull request 📩

Please write a meaningful description for your pull request. If your pull request references an issue or Asana task, please mention it in the description. The format for pull request titles is `component/kind: description`.

For 'stacked' pull requests, please ensure your pull request lists all dependencies and uses the predecessor branch as the target.

To make sure your pull request is easy to review:

- **Use `git rebase`**. We maintain a semi-linear git history. This means that your branch should be a direct suffix of `main` (or the target branch). Addtionally, it should not contain any merge commits.
- **Don't push fixup commits\***. When your reviewer asks for changes, they will want you to rewrite your branch history so that the commit history is clean.

  If you branch history is dirty (containing fixup commits, etc) then we will squash-merge\*. However, this is undesirable as we lose the information that individual commits provide.

- **Follow the Rust style guide**. Please see the [Rust style guide](https://doc.rust-lang.org/nightly/style-guide/). Additionally ensure your code is formatted using

  ```sh
  make fmt
  ```

  Consider installing our pre-commit hook using

  ```sh
  ./scripts/install-pre-commit-hook.sh
  ```

- **Document your code**. Write documentation for your changes, either as comments or as a markdown file in `/docs`.

- **Do not submit untested code**. If you are not able to build or run `jstz` locally to verify that your changes work as expected, please do not submit the changes -- unless the PR is marked as a 'draft'.
