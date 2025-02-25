---
title: 👨‍⚖️ jstz
---

<script setup>
import VPButton from "vitepress/dist/client/theme-default/components/VPButton.vue";
</script>

# 👨‍⚖️ Jstz

Jstz (pronounced "justice") is a JavaScript server runtime for Tezos [Smart Rollups](https://docs.tezos.com/architecture/smart-rollups) with a great developer experience.
With Jstz, you can deploy JavaScript applications known as _smart functions_ that can act as the backend for web applications, including handling logic, storing data, and accepting and distributing payments.

In particular, Jstz is:

- 🚀 **Fast**: Jstz is built on [Boa](https://boajs.dev/), a blazingly fast JavaScript engine written in Rust.
- 📚 **Easy to learn**: Jstz is built with the developer in mind.
- ⚡️ **Fully local**: You can test and develop smart functions locally with a sandbox.

The Jstz command-line toolkit, sandbox, SDK, and other tools in this repository are free and open source software under the [MIT license](https://github.com/jstz-dev/jstz/blob/main/LICENSE).

<VPButton href="/quick_start" size="big" theme="alt" text="Get Started!" style="border-radius:4px;text-decoration:none" />

## JavaScript secured by Tezos

Jstz runs on the [Tezos](https://tezos.com) blockchain network, which means that the JavaScript smart functions that you deploy with Jstz are secure, transparent, and censorship-resistant:

- Smart functions cannot be changed or stopped after they are deployed
- Smart functions are permissionless, so anyone can call them
- Anyone can inspect the code and storage of deployed smart functions
- Because smart functions run in a decentralized manner on many Jstz Smart Rollup nodes, they are censorship-resistant

## Jstz architecture

The Jstz runtime runs as a layer on top of the Tezos blockchain network.
A group of Tezos Smart Rollup nodes host the runtime as a kind of cluster, which means that they perform these tasks:

- Accepting new smart functions for deployment
- Maintaining smart function storage
- Accepting calls to smart functions from clients and running smart function logic

To secure the Jstz runtime, these nodes frequently report the state of Jstz to Tezos, including the state of all smart functions, storage, and user accounts.
These reports, known as _commitments_, ensure that the nodes are processing calls to Jstz smart functions correctly.
If any two nodes post commitments that don't match, the framework of Smart Rollups allows the nodes to step through the Jstz transactions and determine the correct state.
For more information about how Smart Rollups nodes work, see [Smart Rollups](https://docs.tezos.com/architecture/smart-rollups) on docs.tezos.com.

This diagram summarizes the interaction between Jstz Smart Rollup nodes and clients:

![Diagram showing clients sending transactions to Smart Rollup nodes, which run smart function code and publish commitments to Tezos](/img/architecture.png)
