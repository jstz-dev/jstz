# 🚀 Quick Start

This guide will instruct you in writing and deploying your first _smart function_ in under 10 minutes.

It assumes that you have already [installed `jstz`](installation.md) and have a basic familiarity with [JavaScript](https://www.youtube.com/watch?v=lkIFF4maKMU) and have `npm` (`>= 9.6.7`) installed.

## What is jstz?

`jstz` is a specialized JavaScript runtime for [Tezos Smart Rollups](https://docs.tezos.com/architecture/smart-rollups) that aims to be compatible with web conventions.

With `jstz` you can deploy so called _smart functions_ which are operating similarly to cloud functions, while running on Tezos L2 and
providing additional security and blockchain-specific functionality typical for smart contracts.

## 1. Your First Smart Function

First we will clone the `jstz` repository and navigate to the `get-tez` example:

```sh
git clone https://github.com/jstz-dev/jstz.git && cd jstz/examples/get-tez
```

In this example, the smart function provides a way to send a tez to the requester if asked politely.
It takes a HTTP `Request` object with a message and returns a `Response` object informing whether the request succeeded.

```typescript
// <src="examples/get-tez/index.ts">

// 1 tez = 1 million mutez
const ONE_TEZ = 1000000;

// Maximum amount of tez a requester can receive
const MAX_TEZ = 10000;

const getRecievedTez = (requester: Address): number => {
  let receivedTez: number | null = Kv.get(`received/${requester}`);
  receivedTez = receivedTez === null ? 0 : receivedTez;
  console.debug(`Requestor already received ${receivedTez} tez`);
  return receivedTez;
};

const setRecievedTez = (requester: Address, received: number): void => {
  Kv.set(`received/${requester}`, received + 1);
};

const handler = async (request: Request): Promise<Response> => {
  // Extract the requester's address and message from the request
  const requester = request.headers.get("Referer") as Address;
  const { message } = await request.json();

  console.log(`${requester} says: ${message}`);

  // Check if the requester is polite, and decline the request if not
  if (!message.toLowerCase().includes("please")) {
    return new Response("Sorry, I only fulfill polite requests");
  }

  // If the requester already received too much tez, decline the request
  const recievedTez = getRecievedTez(requester);
  if (recievedTez >= MAX_TEZ) {
    return new Response("Sorry, you already received too much tez");
  }

  // Process the request and send the 1 tez = 1 million mutez to the requester if you can
  if (Ledger.balance(Ledger.selfAddress) > ONE_TEZ) {
    console.log(
      `Transferring 1 tez from ${Ledger.selfAddress} to ${requester}...`,
    );
    Ledger.transfer(requester, ONE_TEZ);
  } else {
    return new Response(
      "Sorry, I don't have enough tez to fulfill your request",
    );
  }
  setRecievedTez(requester, recievedTez + 1);

  return new Response("Thank you for your polite request. You received 1 tez!");
};

export default handler;
```

The smart function consists of:

- A `handler` function.

  A smart function processes an HTTP [`Request`]() object and yields a [`Response`]() object, mirroring the functionality of conventional
  web server handlers or cloud functions.

- An `export default` statement.

  `export default` is JavaScript syntax required for defining an EMCAScript module.
  Smart functions _must_ have an default export of a function, which has the following type:

  ```typescript
  type Handler = (req: Request) => Response | Promise<Response>;
  ```

In addition to several [standard Web APIs](./api/index.md#web-platform-apis), `jstz` introduces several concepts and APIs specific to smart functions:

- **Self address**.

  Upon deployment, each smart function is allocated a unique `tz1` address, akin to an IP address for the function.
  `Ledger.selfAddress` contains the (self) address of the smart function.

- **Referer header**.

  The `"Referer"` header contains the `tz1` address of the account initiating the request to the smart function.
  This can be retrieved using `request.headers.get("Referer")`.

- **Ledger**

  `jstz` maintains a persistent ledger of all accounts and their balances (in mutez).
  The [`Ledger`](./api/ledger.md) API provides methods for transferring tez between accounts and querying account balances.

- **Key-Value store**

  `jstz` maintains a persistent key-value store for each smart function, accessible through the [`Kv`](./api/kv.md) API.

- **`SmartFunction` API**

  Smart functions can invoke other smart functions using `fetch`, similiar to network requests in JavaScript.
  Additionally, new smart functions can be deployed by a smart function using the [`SmartFunction`](./api/smart_function.md) API.

## 2. Deploying your Smart Function

First we must install the dependencies for our smart function and start the local sandbox.

```sh
npm install
jstz sandbox start
```

<details>
<summary>Output</summary>
<pre style="border: 1px solid #ccc; padding: 10px; border-radius: 4px; overflow-x: auto;">
<code style="color: #FFF;">$ npm install
up to date, audited 282 packages in 562ms

42 packages are looking for funding
run `npm fund` for details

found 0 vulnerabilities

$ jstz sandbox start

           __________
           \  jstz  /
            )______(
            |""""""|_.-._,.---------.,_.-._
            |      | | |               | | ''-.
            |      |_| |_             _| |_..-'
            |______| '-' `'---------'` '-'
            )""""""(
           /________\
           `'------'`
         .------------.
        /______________\

        0.1.0-alpha.0 https://github.com/jstz-dev/jstz

octez-node is listening on: http://127.0.0.1:18731
octez-smart-rollup-node is listening on: http://127.0.0.1:8932
jstz-node is listening on: http://127.0.0.1:8933

Tezos bootstrap accounts:
+---------------------------------------------------+---------------+--------------+
| Address | XTZ Balance | CTEZ Balance |
+===================================================+===============+==============+
| (bootstrap1) tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx | 4000000000000 | 100000000000 |
+---------------------------------------------------+---------------+--------------+
| (bootstrap2) tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN | 4000000000000 | 100000000000 |
+---------------------------------------------------+---------------+--------------+
| (bootstrap3) tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU | 4000000000000 | 100000000000 |
+---------------------------------------------------+---------------+--------------+
| (bootstrap4) tz1b7tUupMgCNw2cCLpKTkSD1NZzB5TkP2sv | 4000000000000 | 100000000000 |
+---------------------------------------------------+---------------+--------------+
| (bootstrap5) tz1ddb9NMYHZi5UzPdzTZMYQQZoMub195zgv | 4000000000000 | 100000000000 |
+---------------------------------------------------+---------------+--------------+

</code>
</pre>
</details>

Now, in a new terminal, we can compile our TypeScript code to JavaScript using and deploy it using:

```sh
npm run build
jstz deploy dist/index.js
```

<details>
<summary>Output</summary>
<pre style="border: 1px solid #ccc; padding: 10px; border-radius: 4px; overflow-x: auto;">
<code style="color: #FFF;">$ npm run build
> @tezos/get-tez@0.0.0 build
> esbuild index.ts --bundle --format=esm --target=esnext --minify --outfile=dist/index.js

dist/index.js 777b

⚡ Done in 10ms

$ jstz deploy dist/index.js
You are not logged in. Please type the account name that you want to log into or create as new: alan
Logged in to account alan with address tz1N8BsvfrSjGdomFi5V9RwwYLasgD8s4pxF

Smart function deployed by alan at address: tz1Tp5wSRWiVJwLoT8WqN1yRapdq6UmdRf6W
Run with `jstz run tezos://tz1Tp5wSRWiVJwLoT8WqN1yRapdq6UmdRf6W/ --data <args> --trace`

</code>
</pre>
</details>

Since this is your first deployment, you need to:

1. **Start your local sandbox**.

   The `jstz sandbox start` command starts the local sandbox. Press `Ctrl+C` to stop the sandbox.

   ::: tip
   **(Only for non-Docker users)**

   The `--detach` (`-d`) flag starts the sandbox in the background, allowing you to continue working in the same terminal.
   The sandbox can be stopped or reset using `jstz sandbox stop` or `jstz sandbox restart`.
   :::

2. **Login / Signup**.

   You need an account to deploy and run your smart functions.
   Switching accounts or managing multiple accounts is possible with `jstz login` and `jstz account` commands.

   ::: tip
   `jstz account create` can be used to create a new account.
   :::

Upon successful deployment, your smart function will be assigned a unique `tz1` address, serving as its identifier, similar to an IP address.

In the example above, the smart function was deployed to `tz1Tp5wSRWiVJwLoT8WqN1yRapdq6UmdRf6W`. The smart function will be accessible through a URL of the format `tezos://tz1Tp5wSRWiVJwLoT8WqN1yRapdq6UmdRf6W/`.

### Optional: Funding Accounts

For the example smart function to send tez successfully, its account must have sufficient funds.
The [`jstz bridge deposit`](bridge.md) command is used to transfer funds from a Layer 1 address to a jstz account.

Within the sandbox environment, there are pre-funded L1 accounts `bootstrap1` through `bootstrap5` that you can use.

```sh
jstz bridge deposit --from bootstrap1 --to tz1Tp5wSRWiVJwLoT8WqN1yRapdq6UmdRf6W --amount 10000000
```

## 3. Running and debugging your Smart Function

After a succesful deployment, you will be able to run the smart function with the provided command to run your smart function similarly to the following:

```sh
jstz run tezos://tz1Tp5wSRWiVJwLoT8WqN1yRapdq6UmdRf6W/ --data '{"message":"Please, give me some tez."}'
```

<details>
<summary>
Output
</summary>
<pre style="border: 1px solid #ccc; padding: 10px; border-radius: 4px; overflow-x: auto;">
<code style="color: #FFF;">$jstz run tezos://tz1Tp5wSRWiVJwLoT8WqN1yRapdq6UmdRf6W/ --data '{"message":"Please, give me some tez."}'
▐ Running function at tezos://tz1Tp5wSRWiVJwLoT8WqN1yRapdq6UmdRf6W/ 
Status code: 200 OK
Headers: {"content-type": "text/plain;charset=UTF-8"}
Body: Thank you for your polite request. You received 1 tez!
</code></pre>
</details>

Congratulations! 🎉 You have now successfully deployed and crafted an HTTP request to run your first smart function.

::: tip  
To deploy and interact with your function on networks beyond the sandbox, like `weeklynet`, use the `--network` (`-n`) flag.
:::

For debugging, `jstz` provides the following tools:

- `jstz logs trace` enables tailing the logs of a given smart function.
- `jstz kv` allows exploring the current state of the KV store, listing subkeys or retrieving values for a particular account.

::: tip
The `--trace` flag for `jstz run` will tail the logs of smart function, akin to the `jstz logs trace` command.
:::
