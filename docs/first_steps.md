# 🚀 Quick start

This guide will instruct you in writing and deploying your first _smart function_ in less than 10 minutes.

It assumes that you have already [installed `jstz`](installation.md) and have a basic familiarity with [Javascript](https://www.youtube.com/watch?v=lkIFF4maKMU).

## What is jstz?

`jstz` is a JavaScript runtime for Tezos smart optimistic rollups that aims to be compatible with web conventions.

Through `jstz` you can deploy so called _smart functions_ which are akin to cloud functions, however running on Tezos L2 and
providing additional security and blockchain-specific functionality typical for smart contracts.

## 1. Write your smart function

Let's see how a smart function looks like. In the following example,
the `request_tez` smart function allows others to request tez from you if asked politely.

```javascript
// examples/request_tez.js
export default async function (request) {
  // Extract the requestor's address and message from the request
  const requester_address = request.headers.get("Referer");
  const { message } = await request.json();

  console.log(`${requester_address} says: ${message}`);

  // Check if the requestor is polite, and decline the request if not
  if (!message.toLowerCase().includes("please")) {
    return new Response("Sorry, I only fulfill polite requests");
  }

  // Check how much tez the requestor already received in the Kv store
  let receivedTez = Kv.get("received/" + requester_address);
  receivedTez = receivedTez === null ? 0 : receivedTez;
  console.debug(`Requestor already received ${receivedTez} tez`);

  // If the requestor already received too much tez, decline the request
  if (receivedTez >= 10000) {
    return new Response("Sorry, you already received too much tez");
  }

  // Process the request and send the 1 tez = 1 million mutez to the requestor if you can
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

  // Update the amount of tez the requestor received in the Kv store
  Kv.set("received/" + requester_address, received + 1);

  // Pay taxes on the gift by calling a nested smart function.
  // await fetch( // Luckily, pay_tax sf doesn't exist yet
  //  new Request(`tezos://pay_tax/`, {
  //    method: `POST`,
  //    body: `{ "amount": 1000000, "address": "${requester_address}"}`,
  //  }),
  // ));

  // Inform the requestor about the successful transfer
  return new Response("Thank you for your polite request. You received 1 tez!");
}
```

As we can see, the typical smart function has the following attributes that make it unique compared to the typical javascript functions:

- **Input/Output**

  Smart function takes an HTTP [`Request`]() object and returns a [`Response`]() object,
  similarly to standard web server handlers or cloud functions.

- **`export default` statement**

  `export default` is JavaScript syntax required for defining an EMCAScript module.
  Smart functions _must_ have an default export of a function, which has the following type:

```javascript
type Handler = (req: Request) => Response | Promise<Response>;
```

- **Self address**

  After deployment, the smart function will be assigned its own `tz1 self-address`. It can be though of as analogous to an IP address of the smart function and can be accessed via `Ledger.selfAddress` value

- **Referer**

  Similarly, the account of the `tz1` address of the smart function that made the request is called `Referer`.
  It is automatically set as a special request header and get be accessed via `request.headers.get("Referer")`

- **Ledger**

  The persistent ledger of all accounts and their L2 tez balances (stored as mutez) is maintained via the global [`Ledger`](./api/ledger.md) object.
  Through [`Ledger`](./api/ledger.md) you can check the account balances and transfer tez.

- **Kv store**

  A persistent key-value database that can be used to store and retrieve JSON blobs available using the global [`Kv`](./api/kv.md) object. object.
  It has properties similar to the JavaScript Map object, however the values are kept between different calls of the smart function. The keys are represented as strings, while the values are serializable JavaScript objects.

- **Nested smart function calls**

  If you would like to call other smart functions, you can do so through `fetch`. This works similarly to how you request resources from network in Javascript, however in `jstz` the addresses should point to other smart functions.
  All operations of Ledger and Kv are synchronous and atomic, so the values in all nested calls get commited only if the smart function succeeds.

  If you want, you can also deploy new smart functions directly from the code via [`SmartFunction`](./api/smart_function.md) object.

Since `jstz` is a JavaScript server runtime running on Tezos's smart optimistic rollups, some APIs are not available.
Please see the API reference for all APIs currently available in jstz.

## 2) Deploy

After you've finished coding you smart function, you can deploy it as follows:

```sh
jstz deploy examples/request_tez.js
```

<details>
<summary>
Output
</summary>
<pre style="border: 1px solid #ccc; padding: 10px; border-radius: 4px; overflow-x: auto;">
<code style="color: #FFF;">$ jstz deploy examples/request_tez.js
No sandbox is currently running.
Start the sandbox in daemon mode now? Tip: Use 'jstz sandbox start' for an interactive session instead. yes
Sandbox pid: 2132.   Use `jstz sandbox stop` to stop the sandbox background process.
Use `jstz sandbox restart --detach` to start from a clear sandbox state.

You are not logged in. Please type the account name that you want to log into or create as new: alan
Logged in to account alan with address tz1N8BsvfrSjGdomFi5V9RwwYLasgD8s4pxF

Smart function deployed by alan at address: tz1Tp5wSRWiVJwLoT8WqN1yRapdq6UmdRf6W
Run with `jstz run tezos://tz1Tp5wSRWiVJwLoT8WqN1yRapdq6UmdRf6W/ --data <args> --trace`

</code>
</pre>
</details>

This commands deploys your smart function to your dev network that runs locally on your machine.
As this was the first time that you have tried to deploy a smart function, you have likely been prompted to:

- Start your local sandbox environment in detached mode as none was yet running. This will start sandbox processes running in background. Alternatively, an interactive session can be also started in a separate terminal via `jstz sandbox start`.
  If you want to stop the sandbox or restart from a clear state, you can do so through `jstz sandbox stop`/`jstz sandbox restart` commands.
- Create and login to a new account. This will be the account from which you can deploy and run your smart functions.
  You can login to a different account at any point via `jstz login` and manage your accounts through `jstz account` commands.

If everything goes well, the function will be deployed from your account and assigned its own `tz1 self address` similarly to an IP address. The deployed smart function will have its own URL address of form `tezos://<self address>/`.

### Optional: Funding the smart function account

In our example smart function, we can only successfully send tez if the smart function account(self address) has sufficient funds.
To achieve this, we can use the [`bridge deposit`](bridge.md) command to transfer funds from an L1 to L2 address as follows:

```sh
jstz bridge deposit --from <TZ1_ADDRESS/ALIAS> --to <TZ1_ADDRESS/ALIAS> --amount <AMOUNT>
```

In the sandbox environment, there are already provided funded `bootsrap1`-`bootstrap5` accounts that you can use.

## 3) Run, debug and test

After a succesful deployment, you will be able to run the smart function with the provided command to run your smart function similar as follows:

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
If you would like to deploy your function outside of your sandbox to another network such as `dailynet` or `weeklynet`, use `--network` flag with `deploy`, `run` and `bridge` commands.
:::

You can see the smart function response and if `--trace` is chosen also its `console` log,warn, debug and error outputs together with state changes.

For debugging, `jstz` provides the following tools:

- `jstz logs trace` for tracing of your smart functions similarly to `--trace` flag and filtering logs more specifically.
- `jstz kv` for exploring the current state of the Kv store, listing subkeys or getting values for a particular account.
