---
title: Networks
---

Like Tezos layer 1 and most other Web3 systems, you can deploy and test smart functions on test networks before deploying to production.

:::note

Jstz does not yet have a stable production network.
Until it does, consider all networks to be temporary test networks.

:::

These are the main networks that are available:

| Network       | Dashboard                               | Description                                                 |
| ------------- | --------------------------------------- | ----------------------------------------------------------- |
| Local sandbox | None                                    | Local network used for development; see [Sandbox](/sandbox) |
| Privatenet    | https://privatenet.dashboard.jstz.info/ | A public network for testing                                |

Other networks are becoming available as Jstz expands.

## Switching networks

If you have access to other networks, you can set them up manually in your configuration file and deploy smart functions to them.

To add a network, you need:

- The RPC endpoint of the Octez node for the network, represented in the following instructions by the variable `<OCTEZ_NODE_RPC>`.

- The URL of the Jstz node for the network, represented by the variable `<JSTZ_NODE>`.

To set up and use a different network from the sandbox open your Jstz configuration file at `~/.config/jstz/config.json` and add an entry to the `networks` field to represent the new network, as in this example:

```json
{
  "current_alias": "my_account",
  "networks": {
    "<NETWORK_NAME>": {
      "octez_node_rpc_endpoint": "<OCTEZ_NODE_RPC>",
      "jstz_node_endpoint": "<JSTZ_NODE>"
    }
  }
}
```

This example uses the variable `<NETWORK_NAME>` for the local alias of the new network.
You can give the network any alias and then use this name as the value for the `--network` argument in Jstz commands, as in `jstz deploy dst/index.js -n <NETWORK_NAME>`.

For example, to use Privatenet, use `https://rpc.privatenet.jstz.info` for the RPC node endpoint and `https://privatenet.jstz.info` for the Jstz node, as in this example:

```json
{
  "current_alias": "my_account",
  "networks": {
    "privatenet": {
      "octez_node_rpc_endpoint": "https://rpc.privatenet.jstz.info",
      "jstz_node_endpoint": "https://privatenet.jstz.info"
    }
  }
}
```

Then you can use the alias of the network in the config file to deploy and interact with smart functions, as in this example:

```bash
jstz deploy examples/counter.js -n privatenet
```

You can set a network as the default by putting its name in the `default_network` field, as in this example:

```json
{
  "current_alias": "my_account",
  "default_network": "privatenet",
  "networks": {
    "privatenet": {
      "octez_node_rpc_endpoint": "https://rpc.privatenet.jstz.info",
      "jstz_node_endpoint": "https://privatenet.jstz.info"
    }
  }
}
```

Now, when you omit the `-n` argument from a Jstz command, it uses the default network.

:::tip

To set the local sandbox as the default network, set the `default_network` field to `dev`.

:::
