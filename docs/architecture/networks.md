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

If you have access to other networks, you can set them up manually in your configuration file and deploy smart functions to them.

To add a network, you need:

- The RPC endpoint of the Octez node for the network, represented in the following instructions by the variable `<OCTEZ_NODE_RPC>`.

- The URL of the Jstz node for the network, represented by the variable `<JSTZ_NODE>`.

Follow these steps to set up and use a different network from the sandbox:

1. In your Jstz configuration file at `~/.config/jstz/config.json`, add an entry to the `networks` field to represent the new network, as in this example:

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

1. Bridge some tez from layer 1 on the network to your Jstz account for transaction fees.
   You can use the same Jstz account that you use on other Jstz networks.

   1. Create a Node.JS project by running this command:

      ```bash
      mkdir jstz-network-bridge && cd jstz-network-bridge && npm init -y
      ```

   1. In the project, add the `@taquito/signer` and `@taquito/taquito` dependencies:

      ```bash
      npm add @taquito/signer @taquito/taquito
      ```

   1. In the project, add a script named `deposit.mjs` with this code:

      ```javascript
      import { TezosToolkit } from "@taquito/taquito";
      import { InMemorySigner } from "@taquito/signer";

      // Bridge address, which is the same for every Jstz network
      const JSTZ_BRIDGE_ADDRESS = "KT1GFiPkkTjd14oHe6MrBPiRh5djzRkVWcni";

      // Octez node base endpoint
      const Tezos = new TezosToolkit("<OCTEZ_NODE_RPC>");

      // Amount to bridge
      const amount = 0.5;

      // The private key of the layer 1 source account to bridge XTZ from.
      // For sandboxes and test networks, use a bootstrap or faucet account.
      // For public networks, use a user account that has some XTZ in it.
      // This key is from a bootstrap account used in the sandbox and other Jstz test networks.
      const signer = await InMemorySigner.fromSecretKey(
        "edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6",
      );

      // The Jstz address to bridge the XTZ to.
      const targetAddress = "<MY_JSTZ_ADDRESS>";

      Tezos.setProvider({ signer });

      Tezos.contract
        .transfer({
          to: JSTZ_BRIDGE_ADDRESS,
          amount: amount,
          parameter: {
            entrypoint: "deposit",
            value: { string: targetAddress },
          },
        })
        .then((r) => console.log("ok", r))
        .catch((e) => console.error("err", e));
      ```

   1. In the script, replace the variables `<OCTEZ_NODE_RPC>` with the RPC endpoint of the Octez node for the network and `<MY_JSTZ_ADDRESS>` with the address of the account to bridge the XTZ to.

   1. In most cases, leave the private key the same as in the code above, which is the private key of one of the Jstz bootstrap accounts.
      However, if you are bridging XTZ from an account that already has XTZ, such as if you used the `jstz bridge` command to send XTZ from a bootstrap account ot a user account, change it to the private key of that account.

   1. Run the script by running this command:

      ```bash
      npx node ./deposit.mjs
      ```

      The script sends 0.5 XTZ to your Jstz account.

   1. Verify that your account received the XTZ by running this command, where `<ALIAS_OR_ADDRESS>` is the address or local alias of your Jstz account and `<NETWORK_NAME>` is the alias of the network in the Jstz config file:

      ```bash
      jstz account balance -a <ALIAS_OR_ADDRESS> -n <NETWORK_NAME>
      ```

      The response should show the bridged XTZ.

Now you can use the alias of the network in the config file to deploy and interact with smart contracts, as in this example:

```bash
jstz deploy examples/counter.js -n <NETWORK_NAME>
```
