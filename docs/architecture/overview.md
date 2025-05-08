---
title: Architecture
---

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
