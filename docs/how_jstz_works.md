---
title: 👨‍⚖️ How jstz works?
---

<script setup>
import VPButton from "vitepress/dist/client/theme-default/components/VPButton.vue";
</script>

# 👨‍⚖️ `How jstz works?`

## Overview

`jstz` is a JavaScript/Typescript server runtime for Tezos 2.0 designed to provide a great developer experience by aiming to be compatible with web conventions.

Through `jstz` developers can set up, deploy and test so called _smart functions_ written in Javascript/Typescript that can get directly executed on Tezos L2.

Jstz provides a local sandboxed environment for developers to test the functions without deploying them to L2. It provides a simple interface through which one can deploy smart functions and then call them by sending HTTP requests to a particular _smart function address_.

## How it works?

Jstz has to ensure that smart functions written in JS/TS can compile directly to WASM - the assembly used for writing Smart Rollups. Therefore 'jstz' is built on _Boa_ - a Javascript engine written in Rust.

In its core (jstz_core), Jstz uses Boa and enables Rust types to be passed around as JavaScript objects. This allows implementation and registration of various APIs written in Rust and their usage as if they were native Javascript objects.

The APIs (implemented in jstz_api) enable interaction with the storage of a rollup kernel (a key-value store) and also implement Request and Response APIs based on the Fetch API specification enabling calling of other smart functions.

##

### KV store

When writing smart functions, we need a way to store data across different calls of the functions. Therefore, jstz _smart functions_ implement a persistent key-value store used for storing and retrieval of arbitrary JSON blobs. This store can be accessed through a global _Kv_ object.

The key-value store implements _optimistic concurrency control scheme_. It is optimistically assumed that conflicts between different transactions (snapshots of the persistent kv store) are sufficiently rare thus not interfering each other. Before commiting, the transaction verifies whether no other transaction has modified the data it has read.

The transactions performed over the KV store offer ACID guarantees and serializability, therefore any transaction can be commited only if it does not conflict with any previously commited ones.

In each transaction, the repeated access to the same key is optimized through caching. Similarly, writes are buffered until the transaction is commited at which point it gets flushed to the persistent KV storage.

### Ledger

A specialised type of the KV store is the Ledger that provides access to the balances of the L2 tez. Additionally it also stores so-called 'self address' - the address of the smart function itself. Similarly to the KV store, all operations on the ledger are synchronous and atomic, commited only if the request to the smart function succeeds.

### Console/URL/Textencoder

Additionally, jstz implements APIs for easier manipulation with debug logs (Console), with unique smart function addresses (URL) or simplification of encoding of passed JSON parameters (TextEncoder).

##

//TODO: Explaining how exactly the following works and fits together:

- the APIs get registered to in the Realm that consists of a set of intrinsic objects and global environment
- The Realm wrapper implements various methods for registration and evaluation of different modules, types and host defined objects and handling of context
- JSNative permits Rust types to be passed around as JavaScript objects.
- There is implemented a wrapper over boa engines runtime and also a wrapper over the smart rollup's runtime - erased runtime.
- the APIs use the functionality of the rollup runtime to interact with the blockchain storage and other functionality implemented in jstz_proto
- jstz_kernel

## Bridge

In order to transfer tez from L1 to an L2 address used by a smart function running on a `jstz` rollup, `jstz` implements a simple ticket-based jstz bridge smart contracts, through which it is possible to deposit tez from an L1 address to a jstz rollup address.
