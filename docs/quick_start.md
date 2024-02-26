# 🚀 Quick start

This guide will instruct you in writing and deploying your first _smart function_ in under 10 minutes.

It assumes that you have already [installed `jstz`](installation.md) and have a basic familiarity with [Javascript](https://www.youtube.com/watch?v=lkIFF4maKMU).

## What is jstz?

`jstz` is a specialized JavaScript runtime for Tezos smart optimistic rollups that aims to be compatible with web conventions.

With `jstz` you can deploy so called _smart functions_ which are operating similarly to cloud functions, while running on Tezos L2 and
providing additional security and blockchain-specific functionality typical for smart contracts.

## 1. Write your smart function

Let's see how a smart function looks like with the following example.
The `request_tez` smart function allows users to request tez from you if asked politely.

```javascript
// examples/request_tez.js
export default async function (request) {
  // Extract the requester's address and message from the request
  const requester_address = request.headers.get("Referer");
  const { message } = await request.json();

  console.log(`${requester_address} says: ${message}`);

  // Check if the requester is polite, and decline the request if not
  if (!message.toLowerCase().includes("please")) {
    return new Response("Sorry, I only fulfill polite requests");
  }

  // Check how much tez the requester already received in the Kv store
  let receivedTez = Kv.get("received/" + requester_address);
  receivedTez = receivedTez === null ? 0 : receivedTez;
  console.debug(`Requestor already received ${receivedTez} tez`);

  // If the requester already received too much tez, decline the request
  if (receivedTez >= 10000) {
    return new Response("Sorry, you already received too much tez");
  }

  // Process the request and send the 1 tez = 1 million mutez to the requester if you can
  if (Ledger.balance(Ledger.selfAddress) > 1000000) {
    console.log(
      `Transferring 1 tez from ${Ledger.selfAddress} to ${requestor}...`,
    );
    Ledger.transfer(requestor, 1000000);
  } else {
    return new Response(
      "Sorry, I don't have enough tez to fulfill your request",
    );
  }

  // Update the amount of tez the requester received in the Kv store
  Kv.set("received/" + requester_address, received + 1);

  // Pay taxes on the gift by calling a nested smart function.
  // await fetch( // Luckily, pay_tax sf doesn't exist yet
  //  new Request(`tezos://pay_tax/`, {
  //    method: `POST`,
  //    body: `{ "amount": 1000000, "address": "${requester_address}"}`,
  //  }),
  // ));

  // Inform the requester about the successful transfer
  return new Response("Thank you for your polite request. You received 1 tez!");
}
```

As we can see, a typical smart function has several attributes that set it apart from standard JavaScript functions:

- **Input/Output Handling**

  A smart function processes an HTTP [`Request`]() object and yields a [`Response`]() object, mirroring the functionality of conventional web server handlers or cloud functions.

- **`export default` statement**

  `export default` is JavaScript syntax required for defining an EMCAScript module.
  Smart functions _must_ have an default export of a function, which has the following type:

```javascript
type Handler = (req: Request) => Response | Promise<Response>;
```

- **Self Address**

  Upon deployment, each smart function is allocated a unique `tz1 self-address`, akin to an IP address for the function. It is accessible via `Ledger.selfAddress` property.

- **Referer**

  The `Referer` refers to the `tz1` account address initiating the request to the smart function. This is automatically included as a special request header and can be retrieved using `request.headers.get("Referer")`

- **Ledger**

  The global [`Ledger`](./api/ledger.md) object maintains a persistent ledger of all accounts and their Layer 2 tez balances (in mutez). It also enables account balance inquiries and tez transfers.

- **Key-Value Store**

  The global [`Kv`](./api/kv.md) object provides access to a persistent key-value store for JSON blobs. Functioning similarly to the JavaScript Map object, it preserves data across different smart function invocations, with string keys and serializable JavaScript object values.

- **Nested Smart Function Calls**
  Smart functions can invoke other smart functions using `fetch`, similiarly network resource requests in JavaScript. However, in `jstz`, addresses should only correspond to other smart functions. `Ledger` and `Kv` operations within these calls are synchronous and atomic, ensuring all changes made by all nested calls are committed only upon the successful execution of the smart function.

  Additionally, new smart functions can be deployed directly from the code using the [`SmartFunction`](./api/smart_function.md) object if needed.

Given `jstz` operates within Tezos's smart optimistic rollups, certain JavaScript server runtime APIs are unavailable. For a comprehensive list of available APIs in `jstz`, please consult the API reference.

## 2) Deploy

Once your smart function's code is ready, deploy it using the following commands:

```sh
jstz sandbox start --detach
jstz deploy examples/request_tez.js
```

<details>
<summary>
Output
</summary>
<pre style="border: 1px solid #ccc; padding: 10px; border-radius: 4px; overflow-x: auto;">
<code style="color: #FFF;">$jstz sandbox start --detach
Sandbox pid: 2132.   Use `jstz sandbox stop` to stop the sandbox background process.
Use `jstz sandbox restart --detach` to start from a clear sandbox state.

$ jstz deploy examples/request_tez.js
You are not logged in. Please type the account name that you want to log into or create as new: alan
Logged in to account alan with address tz1N8BsvfrSjGdomFi5V9RwwYLasgD8s4pxF

Smart function deployed by alan at address: tz1Tp5wSRWiVJwLoT8WqN1yRapdq6UmdRf6W
Run with `jstz run tezos://tz1Tp5wSRWiVJwLoT8WqN1yRapdq6UmdRf6W/ --data <args> --trace`

</code>
</pre>
</details>

These commands deploys your smart function on a local development network.
Since it's your first deployment, you need to:

- Start your local sandbox environment detached mode if it's not already running, initiating background processes. For an interactive session, you can start it in a separate terminal without the `--detach` flag. The sandbox can be stopped or reset using `jstz sandbox stop` and `jstz sandbox restart`, respectively.
- Log into or create a new account, which will be used to deploy and run your smart functions. Switching accounts or managing multiple accounts is possible with `jstz login` and `jstz account` commands.

Upon successful deployment, your smart function will be assigned a unique `tz1 self address`, serving as its identifier, similar to an IP address. The smart function will be accessible through a URL of the format `tezos://<self address>/`.

### Optional: Funding the smart function account

For the example smart function to send tez successfully, its account (self-address) must have sufficient funds.
Use the [`bridge deposit`](bridge.md) command to transfer funds from a Layer 1 to a Layer 2 address:

```sh
jstz bridge deposit --from <TZ1_ADDRESS/ALIAS> --to <TZ1_ADDRESS/ALIAS> --amount <AMOUNT>
```

Within the sandbox environment, you have access to pre-funded L1 accounts `bootsrap1` through `bootstrap5` that you can use.

## 3) Run, debug and test

After a succesful deployment, you will be able to run the smart function with the provided command to run your smart function similarly to the following:

```sh
jstz run tezos://tz1cfGTBtDnTcNHCPkSG1fkNeW6AQghNSDnL/ --data '{"message":"Please, give me some tez."} --trace
```

<details>
<summary>
Output
</summary>
<pre style="border: 1px solid #ccc; padding: 10px; border-radius: 4px; overflow-x: auto;">
<code style="color: #FFF;">$jstz run tezos://tz1cfGTBtDnTcNHCPkSG1fkNeW6AQghNSDnL/ --data '{"message":"Please, give me some tez."}'
▐ Running function at tezos://tz1cfGTBtDnTcNHCPkSG1fkNeW6AQghNSDnL/ 
Status code: 200 OK
Headers: {"content-type": "text/plain;charset=UTF-8"}
Body: Thank you for your polite request. You received 1 tez!
</code></pre>
</details>

Congratulations! 🎉 You have now successfully deployed and crafted an HTTP request to run your first smart function.

::: tip  
To deploy and interact with your function on networks beyond the sandbox, like `dailynet` or `weeklynet`, apend `--network <NETWORK_NAME>` flag with `deploy`, `run` and `bridge` commands.
:::

You can see the smart function response and if `--trace` is chosen also its `console` log,warn, debug and error outputs together with state changes.

For debugging, `jstz` provides the following tools:

- `jstz logs trace` enables detailed tracing of smart function executions, akin to the `--trace` flag, allowing for refined log filtering.
- `jstz kv` allows exploring the current state of the Kv store, listing subkeys or retrieving values for a particular account.
