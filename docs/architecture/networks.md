---
title: Networks
---

Like Tezos layer 1 and most other Web3 systems, you can deploy and test smart functions on test networks before deploying to production.

:::note

Jstz does not yet have a stable production network.
Until it does, consider all networks to be temporary test networks.

:::

The primary network that Jstz developers use is the the local [Sandbox](/sandbox).
Other networks are becoming available as Jstz expands.

## Switching networks

If you have access to other networks, you can add them to your configuration file with the CLI and deploy smart functions to them.

To add a network, you need:

- The RPC endpoint of the Octez node for the network, represented in the following instructions by the variable `<OCTEZ_NODE_RPC>`.

- The URL of the Jstz node for the network, represented by the variable `<JSTZ_NODE>`.

To set up and use a different network from the sandbox, run

```bash
jstz network add <NETWORK_NAME> \
  --octez-node-rpc-endpoint <OCTEZ_NODE_RPC> \
  --jstz-node-endpoint <JSTZ_NODE>
```

This example uses the variable `<NETWORK_NAME>` for the local alias of the new network.
You can give the network any alias and then use this name as the value for the `--network` argument in Jstz commands, as in `jstz deploy dst/index.js -n <NETWORK_NAME>`.

Now you can use the bridge or faucet on the network to send some XTZ to your account.
Then you can use the alias of the network in the config file to deploy and interact with smart functions, as in this example:

```bash
jstz deploy examples/counter.js -n <NETWORK_NAME>
```

You can set a network as the default with the CLI as well, as in this example:

```bash
jstz network set-default <NETWORK_NAME>
```

Now, when you omit the `-n` argument from a Jstz command, it uses the default network.

:::tip

To set the local sandbox as the default network, set the `default_network` field to `dev`.

:::

For more information about the CLI `network` command, see the [CLI page](/cli#network).
