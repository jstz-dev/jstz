# 👨‍⚖️ jstz

`jstz` (pronouced: "justice") is a JavaScript server runtime that powers Tezos 2.0.

## Development

### Building from Source

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

### Tests

Unit and integration tests can be run using:

```sh
make test
```

To run `jstz_kernel` is debug mode, `octez-smart-rollup-wasm-debugger` (built from [6c062176](https://gitlab.com/tezos/tezos/-/tree/6c0621760ddce94afeff3484d9e8a650d8535f25)) should be used.

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

### Running the Sandbox Network

Our sandbox network uses a custom distribution of Octez found at [commit 6c062176](https://gitlab.com/tezos/tezos/-/tree/6c0621760ddce94afeff3484d9e8a650d8535f25). See the [Octez docs](https://tezos.gitlab.io/introduction/howtoget.html?highlight=building#compiling-with-make) for instructions on building Octez from source.

Once Octez has been built, copy the following binaries to `jstz`:

- `octez-client`
- `octez-node`
- `octez-smart-rollup-node`

Then build the `jstz` kernel installer using the following command:

```sh
make build-installer
```

You can now start the sandbox with:

```sh
eval `./scripts/sandbox.sh`
```

This will initially run `octez-node` and initialize `octez-client`. Once the client is initialized, the `jstz_kernel` is originated and a smart-rollup node is spun up.

## `jstz` Crates

- [**`jstz_core`**](/jstz_core) - `jstz`'s core functionality: host functions, transactional storage, and execution.
- [**`jstz_api`**](/jstz_api) - `jstz`'s JavaScript web standard runtime apis.
- [**`jstz_kernel`**](/jstz_kernel) - `jstz`'s smart rollup kernel, compiled to WASM.
- [**`jstz_crypto`**](/jstz_crypto) - `jstz`'s crypto library. Primarily a wrapper around `tezos_crypto_rs`.
- [**`jstz_proto`**](/jstz_proto) - `jstz`'s protocol: `jstz` specific runtime apis, storage context, execution of operations.
