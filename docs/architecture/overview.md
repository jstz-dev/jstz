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

## Components

Jstz has several components that provide functionality to different elements of the Jstz ecosystem:

### Jstz CLI

The [Jstz CLI](/cli), available as the package [`@jstz-dev/cli`](https://www.npmjs.com/package/@jstz-dev/cli) provides commands that you can use to deploy and interact with smart functions and the sandbox locally.

### Smart function SDK

Smart functions must be built with the Jstz smart function SDK, [`jstz_sdk`](https://www.npmjs.com/package/jstz_sdk), to be deployed on Jstz.
The SDK allows them to receive requests, return responses, and access the Jstz [API](/api/).

### Client SDK

The Jstz client SDK, [`@jstz-dev/jstz-client`](https://www.npmjs.com/package/@jstz-dev/jstz-client), is a JavaScript/TypeScript SDK that allows applications outside Jstz to:

- Send requests to Jstz smart functions
- Inspect the state of Jstz, such as getting account balances and values from the key-value store
- Deploy smart functions

### Development wallet

The [Jstz dev wallet](https://github.com/jstz-dev/dev-wallet) is a web browser extension that works as a wallet, signing transactions with an account's private key.
You can use this extension as a wallet to sign transactions on web applications for the purposes of development, but it is not yet a secure wallet to use in production applications.

### `jstzd` daemon

The `jstzd` daemon runs services that are necessary to run the Jstz sandbox, including:

- The layer 1 bootstrap accounts that you can use to fund accounts in Jstz
- The [bridge](/architecture/bridge) that you can use to move tez from those accounts to your Jstz accounts
- A local version of the Jstz runtime

You can use the Jstz CLI to run the sandbox or use the `jstzd` daemon directly.
For more information, see [Sandbox](/sandbox).
