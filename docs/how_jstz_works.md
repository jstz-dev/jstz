---
title: 👨‍⚖️ How jstz works?
---

# 👨‍⚖️ How `jstz` works?

## Overview

`jstz` is a JavaScript server runtime for Tezos 2.0 designed to provide a great developer experience by aiming to be compatible with web standards.

Through `jstz` developers can set up, deploy and test so called _smart functions_ written in Javascript/Typescript that can get directly executed on the `jstz` smart rollup node.
It provides a simple interface through which one can deploy smart functions and then call them by sending HTTP requests to a particular _smart function address_.

`jstz` also provides a local sandboxed environment for developers to test their functions without deploying them to production.

## How it works?

Since smart rollups must compile to WASM, `jstz` needs to use a JavaScript engine that compiles to WASM - the assembly used for writing Smart Rollups. Therefore `jstz` is built on _Boa_ - a Javascript engine written in Rust.

In the jstz_core crates, `jstz` uses Boa and enables Rust types to be passed around as JavaScript objects. This allows implementation and registration of various APIs written in Rust and their usage as if they were native Javascript objects.

When writing smart functions, we need a way to store data across different calls of the functions. Therefore, `jstz` _smart functions_ implement a persistent key-value store used for storing and retrieval of arbitrary JSON blobs. This store can be accessed through a global _Kv_ object.

The key-value store implements _optimistic concurrency control scheme_. It is optimistically assumed that conflicts between different transactions (snapshots of the persistent kv store) are sufficiently rare thus not interfering each other. Before commiting, the transaction verifies whether no other transaction has modified the data it has read.

The transactions performed over the KV store offer ACID guarantees and serializability, therefore any transaction can be commited only if it does not conflict with any previously commited ones.

In each transaction, the repeated access to the same key is optimized through caching. Similarly, writes are buffered until the transaction is commited at which point it gets flushed to the persistent KV storage.

`jstz` implements several `jstz`-specific APIs such as `Kv`, `Ledger`, and `Contract`. Additionally, `jstz` provides implementations for many web standard APIs in the `jstz_api` crate.

## `jstz`-specific APIs

### KV store

_Kv_ store is implemented on top of jstz\*core::kv. The API provides access to a persistent key-value database that can be used to store and retrieve JSON blobs built directly into the jstz runtime via a global _Kv_ object.

### Ledger

A specialised type of the KV store is the Ledger that provides access to the balances of the L2 tez. Additionally it also stores so-called 'self address' - the address of the smart function itself. Similarly to the KV store, all operations on the ledger are synchronous and atomic, commited only if the request to the smart function succeeds.

### Contract

<!-- TODO Contract -->

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
