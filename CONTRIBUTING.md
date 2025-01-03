# Contributing 👩‍💻

Before contributing to `jstz`, please read these guidelines carefully.

## Overview

`jstz` is a JavaScript server runtime for Tezos smart optimistic rollups designed to provide a great developer experience by aiming to be compatible with web standards.

Through `jstz` developers can set up, deploy and test so called _smart functions_ written in Javascript/Typescript that can get directly executed on the `jstz` smart rollup node.
It provides a simple interface through which one can deploy smart functions and then call them by sending HTTP requests to a particular _smart function address_.

`jstz` also provides a local sandboxed environment for developers to test their functions without deploying them to production.

## How it works?

Since smart rollups must compile to WASM, `jstz` needs to use a JavaScript engine that compiles to WASM - the assembly used for writing Smart Rollups. Therefore `jstz` is built on _Boa_ - a Javascript engine written in Rust.

In the `jstz_core` crates, `jstz` uses Boa and enables Rust types to be passed around as JavaScript objects. This allows implementation and registration of various APIs written in Rust and their usage as if they were native Javascript objects.

When writing smart functions, we need a way to store data across different calls of the functions. Therefore, `jstz` _smart functions_ implement a persistent key-value store used for storing and retrieval of arbitrary JSON blobs. This store can be accessed through a global _Kv_ object.

The key-value store implements _optimistic concurrency control scheme_. It is optimistically assumed that conflicts between different transactions (snapshots of the persistent kv store) are sufficiently rare thus not interfering each other. Before committing, the transaction verifies whether no other transaction has modified the data it has read.

The transactions performed over the KV store offer ACID guarantees and serializability, therefore any transaction can be committed only if it does not conflict with any previously committed ones.

In each transaction, the repeated access to the same key is optimized through caching. Similarly, writes are buffered until the transaction is committed at which point it gets flushed to the persistent KV storage.

`jstz` implements several `jstz`-specific APIs such as `Kv`, `Ledger`, and `SmartFunction`. Additionally, `jstz` provides implementations for many web standard APIs in the `jstz_api` crate.

## `jstz`-specific APIs

### KV store

_Kv_ store is implemented on top of jstz\*core::kv. The API provides access to a persistent key-value database that can be used to store and retrieve JSON blobs built directly into the jstz runtime via a global _Kv_ object.

### Ledger

A specialised type of the KV store is the Ledger that provides access to the balances of the L2 tez. Additionally it also stores so-called 'self address' - the address of the smart function itself. Similarly to the KV store, all operations on the ledger are synchronous and atomic, committed only if the request to the smart function succeeds.

### SmartFunction

<!-- TODO SmartFunction -->

## Standard APIs

Additionally, `jstz` provide implementation of many standard web APIs in the `jstz_api` crate.

<!--//TODO: Explaining how exactly the following works and fits together:

- the APIs get registered to in the Realm that consists of a set of intrinsic objects and global environment
- The Realm wrapper implements various methods for registration and evaluation of different modules, types and host defined objects and handling of context
- JSNative permits Rust types to be passed around as JavaScript objects.
- There is implemented a wrapper over boa engines runtime and also a wrapper over the smart rollup's runtime - erased runtime.
- the APIs use the functionality of the rollup runtime to interact with the blockchain storage and other functionality implemented in jstz_proto
- jstz_kernel
-->

## Bridge

In order to transfer ctez from L1 address to an L2 address in `jstz`, `jstz` implements a simple ticket-based bridge smart contract built with LIGO. This contract enables users to deposit ctez from an L1 address (`tz1`/`KT1`) to a jstz address (`tz4`).

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

### Running `jstz` locally ⚙️

#### Installing Octez 🐙

An Octez distribution of version >= v20 is required to run our sandbox network. The easiest way to get Octez is by [downloading the static binaries](https://tezos.gitlab.io/introduction/howtoget.html#getting-static-binaries) or [installing the binaries](https://tezos.gitlab.io/introduction/howtoget.html#installing-binaries) for your system if it is supported. Otherwise, see the [Octez docs](https://tezos.gitlab.io/introduction/howtoget.html#setting-up-the-development-environment-from-scratch) for instructions on building Octez from source.

Alternative, with Nix, execute the following:

```sh
# Clone Octez
git clone git@gitlab.com:tezos/tezos.git
cd tezos
# Build using Nix
nix-build -j auto
```

After Nix successfully builds Octez (it takes a long time 🕣), the Octez binaries will be accessible from `result`.

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
- [**`jstz_cli`**](/jstz_cli) - `jstz`'s client CLI tool: used to create, call, and manage `jstz` smart functions and accounts.

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

### Documentation 📚

#### Runtime API documentation

To edit documentation:

- Find or add a documentation file in `docs/api/`
- Modify documentation in markdown
- Locally test the documentation (with live reload) using
  ```sh
  npm run docs:dev
  ```

## Creating a pull request 📩

Please adhere to the following PR guidelines

### PR preparation

- Have a title of the form `type(component): subject`. See [commitlint](https://github.com/conventional-changelog/commitlint/#what-is-commitlint) for possible types
- Give meaning context and description about the issue solved/feature added
  - Context - Why is this change required? What problem does it solve?
  - Description - Describe your changes in detail. Add anything else to highlight to the reviewer.
- Anticipate questions: explain anything which may look surprising either in PR description or in code comments if it will be useful for future readers
- Include [magic word + link/issue ID](https://linear.app/docs/github?tabs=206cad22125a#link-using-pull-requests) in the description for project tracking
- 1 Linear issue per PR
- Clean commit history (jstz uses a semi-linear commit history)
- [Meaningful commit messages](https://cbea.ms/git-commit/)
- Have tests / testing instructions
- Aim for above 80% patch coverage
- Favour small PRs
- Author's are responsible to have assignees / reviewers allocated
- Stacked PRs should target their predecessor

### PR management

- Use assignee field to indicate action is required from that particular person
- 1 assignee at a time to avoid responsibility dilution
- Commenter responsible for resolving conversations except in code suggestions which automatically close a conversation when applied
- Assignee assigns the next person to take action Eg. Reviewer assigns author after providing feedback. Author assigns Reviewer after applying feedback.
- Mark as Draft to indicate PR isn’t ready to be reviewed/merged
- Maintain a logical commit history if possible. Squash commit history if not (not ideal as lose information that the commit provides).
- Force push to relevant commits when applying simple changes
- Use Fixup commits when applying complicated changes
- Do not merge Fixup commits. The must always be squashed, ideally, each sections goes into their relevant commits
- Create follow up issues for suggestions that are out of scope
- Rebase branch on main frequently

### Code

- `TODO/FIXME`s in code should be linked to an issue number and of the form
  ```
  // TODO: <link to issue>
  // <one-line explanation>
  // <longer explanation if the issue description is not the right place.>
  ```
- Commented code should be removed
- Code should compile and tests should pass in between commits
- Follow the [Rust style guide](https://doc.rust-lang.org/nightly/style-guide/). Additionally ensure your code is formatted using

  ```sh
  make fmt
  ```

  Consider installing our pre-commit hook using

  ```sh
  ./scripts/install-hooks.sh
  ```

- Write documentation for your changes, either as comments or as a markdown file in `/docs`. Prefer writing doc comments for code with `pub` visibility.
