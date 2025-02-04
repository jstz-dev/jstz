# 🚀 Quick Start

This guide will instruct you in writing and deploying your first _smart function_ in under 10 minutes.

It assumes that you have already [installed `jstz`](installation.md), have a basic familiarity with [JavaScript](https://www.youtube.com/watch?v=lkIFF4maKMU) and have `npm` installed.

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

const getReceivedTez = (requester: Address): number => {
  let receivedTez: number | null = Kv.get(`received/${requester}`);
  receivedTez = receivedTez === null ? 0 : receivedTez;
  console.debug(`Requestor already received ${receivedTez} tez`);
  return receivedTez;
};

const setReceivedTez = (requester: Address, received: number): void => {
  Kv.set(`received/${requester}`, received + 1);
};

const addPoliteMessage = (requester: Address, message: string): void => {
  let length: number | null = Kv.get(`messages/${requester}/length`);
  if (length === null) {
    length = 0;
  }
  Kv.set(`messages/${requester}/${length}`, message);
  Kv.set(`messages/${requester}/length`, length + 1);
};

const handler = async (request: Request): Promise<Response> => {
  // Extract the requester's address and message from the request
  const requester = request.headers.get("Referer") as Address;
  const { message } = await request.json();

  console.log(`${requester} says: ${message}`);

  // Check if the requester is polite, and decline the request if not
  if (!message.toLowerCase().includes("please")) {
    return new Response(
      JSON.stringify("Sorry, I only fulfill polite requests"),
    );
  }

  // If the requester already received too much tez, decline the request
  const recievedTez = getReceivedTez(requester);
  if (recievedTez >= MAX_TEZ) {
    return new Response(
      JSON.stringify("Sorry, you already received too much tez"),
    );
  }

  // Process the request and send the 1 tez = 1 million mutez to the requester if you can
  if (Ledger.balance(Ledger.selfAddress) > ONE_TEZ) {
    console.log(
      `Transferring 1 tez from ${Ledger.selfAddress} to ${requester}...`,
    );
    Ledger.transfer(requester, ONE_TEZ);
  } else {
    return new Response(
      JSON.stringify("Sorry, I don't have enough tez to fulfill your request"),
    );
  }

  setReceivedTez(requester, recievedTez + 1);
  addPoliteMessage(requester, message);

  return new Response(
    JSON.stringify("Thank you for your polite request. You received 1 tez!"),
  );
};

export default handler;
```

The smart function consists of:

- A `handler` function.

  A smart function processes an HTTP [`Request`]() object and yields a [`Response`]() object, mirroring the functionality of conventional
  web server handlers or cloud functions.

- An `export default` statement.

  `export default` is JavaScript syntax required for defining an ECMAScript module.
  Smart functions _must_ have an default export of a function, which has the following type:

  ```typescript
  type Handler = (req: Request) => Response | Promise<Response>;
  ```

In addition to several [standard Web APIs](./api/index.md#web-platform-apis), `jstz` introduces several concepts and APIs specific to smart functions:

- **Self address**.

  Upon deployment, each smart function is allocated a unique `KT1` address, akin to an IP address for the function.
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

  Smart functions can invoke other smart functions using `fetch`, similar to network requests in JavaScript.
  Additionally, new smart functions can be deployed by a smart function using the [`SmartFunction`](./api/smart_function.md) API.

## 2. Deploying your Smart Function {#deploying-your-smart-function}

First we must install the dependencies for our smart function and start the local sandbox.

```sh
npm install
jstz sandbox start
```

To run a containerized sandbox, make sure docker is installed in your system and run:

```sh
jstz sandbox --container start
```

Note: in case you get an error about the configuration file improperly configured, please clean up the `~/.jstz/` folder.

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
jstz deploy dist/index.js -n dev
```

<details>
<summary>Output</summary>
<pre style="border: 1px solid #ccc; padding: 10px; border-radius: 4px; overflow-x: auto;">
<code style="color: #FFF;">$ npm run build
> @jstz-dev/get-tez@0.0.0 build
> esbuild index.ts --bundle --format=esm --target=esnext --minify --outfile=dist/index.js

dist/index.js 777b

⚡ Done in 10ms

$ jstz deploy dist/index.js
You are not logged in. Please type the account name that you want to log into or create as new: alan
Logged in to account alan with address tz1N8BsvfrSjGdomFi5V9RwwYLasgD8s4pxF

Smart function deployed by alan at address: KT1SJJxRXXxdiL6c4h4LisgYopyA14JxECXv
Run with `jstz run tezos://KT1SJJxRXXxdiL6c4h4LisgYopyA14JxECXv/ --data <args> --trace`

</code>
</pre>
</details>

Since this is your first deployment, you need to:

1. **Start your local sandbox**.

   The `jstz sandbox start` command starts the local sandbox. Press `Ctrl+C` to stop the sandbox.

   ::: tip
   **(Only for non-Docker users)**

   The `--detach` (`-d`) flag starts the sandbox in the background, allowing you to continue working in the same terminal.
   The sandbox can be stopped or reset using `jstz sandbox stop` or `jstz sandbox restart`. if you are running the containerized sandbox, use `jstz sandbox --container stop` or `jstz sandbox --container restart`.
   :::

2. **Login / Signup**.

   You need an account to deploy and run your smart functions.
   Switching accounts or managing multiple accounts is possible with `jstz login` and `jstz account` commands.

   ::: tip
   `jstz account create` can be used to create a new account.
   :::

Upon successful deployment, your smart function will be assigned a unique `KT1` address, serving as its identifier, similar to an IP address.

In the example above, the smart function was deployed to `KT1SJJxRXXxdiL6c4h4LisgYopyA14JxECXv`. The smart function will be accessible through a URL of the format `tezos://KT1SJJxRXXxdiL6c4h4LisgYopyA14JxECXv/`.

### Optional: Funding Accounts

For the example smart function to send tez successfully, its account must have sufficient funds.
The [`jstz bridge deposit`](bridge.md) command is used to transfer funds from a Layer 1 address to a jstz account.

Within the sandbox environment, there are pre-funded
accounts `bootstrap1` through `bootstrap5` that you can use (for the containerized sandbox, please check the [jstzd](jstzd.md) documentation).

```sh
jstz bridge deposit --from bootstrap1 --to KT1SJJxRXXxdiL6c4h4LisgYopyA14JxECXv --amount 1000 -n dev
```

## 3. Running and debugging your Smart Function

After a successful deployment, you will be able to run the smart function with the provided command to run your smart function similarly to the following:

```sh
jstz run tezos://KT1SJJxRXXxdiL6c4h4LisgYopyA14JxECXv/ --data '{"message":"Please, give me some tez."}' -n dev
```

<details>
<summary>
Output
</summary>
<pre style="border: 1px solid #ccc; padding: 10px; border-radius: 4px; overflow-x: auto;">
<code style="color: #FFF;">$jstz run tezos://KT19mYzcaYk55KttezwP4TbMrGCDpVuPW3Jw/ --data '{"message":"Please, give me some tez."}'
▐ Running function at tezos://KT19mYzcaYk55KttezwP4TbMrGCDpVuPW3Jw/
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

## 4. Interacting with Smart Functions in NodeJs

We are going to look at an example cli app that uses the jstz client and signer to interact with the `get-tez` contract deployed earlier. To start, build the `show-tez/` example from within the `jstz/examples` folder

```sh
# git clone https://github.com/jstz-dev/jstz
cd jstz/examples/show-tez
npm install
npm run build
```

To use the cli app, you need to be logged in and provide the `get-tez` smart function hash deployed in [Deploying your Smart Function](#deploying-your-smart-function)

```sh
jstz login <alias>
node dist/bundle.js KT1SJJxRXXxdiL6c4h4LisgYopyA14JxECXv
```

This app has 2 functions - you can request for tez from the get-tez smart contract and inspect the history of your polite requests by issuing the "Show" command

```sh
# Example
$ node dist/bundle.js KT1SJJxRXXxdiL6c4h4LisgYopyA14JxECXv
🤖: Please ask for tez politely. Type "show" to see past messages. Ctrl+C to quit
Please give me some tez
🤖: Thank you for your polite request. You received 1 tez!
I want tez now!
🤖: Sorry, I only fulfill polite requests
Ok, sorry, please give me a little more
🤖: Thank you for your polite request. You received 1 tez!
Show # <- show history
[0] Please, give me some tez.
[1] Please give me some tez
[2] Ok, sorry, please give me a little more
```

Let's take a look at how it works. For concision, we will be focusing on the parts that are specific to jstz but feel free to jump into the codebase and take a closer look.

There are 3 jstz specific imports that we need; importing the client lib, the client lib types and the sigining library.

```javascript
// line 1
import { Jstz } from "@jstz-dev/jstz-client";
import JstzType from "@jstz-dev/jstz-client";
...
import * as jstz_sdk from "jstz_sdk"; // <- signing library

```

::: warning
The signing library is a temporary solution while we build out the secure signing interface. Essentially, it is a wasm program directly compiled from the core Jstz rust code. Although semantically correct, we discourage using this library in production because it involves copying around users' secret key which is not secure.
:::

Next, the `buildRequest(..)` function contructs a `RunFunction` operation that targets the `tezos://KT1SJJxRXXxdiL6c4h4LisgYopyA14JxECXv` url.

```typescript
// line 13
function buildRequest(
  contractAddress: string,
  message: string,
): JstzType.Operation.RunFunction {
  return {
    _type: "RunFunction",
    body: Array.from(
      encoder.encode(
        JSON.stringify({
          message: message,
        }),
      ),
    ),
    gas_limit: 55000,
    headers: {},
    method: "GET",
    uri: `tezos://${contractAddress}`,
  };
}
```

To inject an operation, we always need to

1. Build the operation content (`RunFunction` in this case)
2. Fetch the account nonce
3. Construct the `Operation` structure
4. Sign the `Operation`
5. Inject and (optionally) poll for the receipt. You can think of the receipt as a response.

```typescript
// line 87
const runFunction = buildRequest(contractAddress, input);
const nonce = await jstzClient.accounts.getNonce(address);
const operation = {
  content: runFunction,
  nonce,
  source: address,
};
const signature = jstz_sdk.sign_operation(operation, secretKey);
const response = jstzClient.operations.injectAndPoll({
  inner: operation,
  public_key: publicKey,
  signature: signature,
});
...
const {
  result: {
    inner: { body },
  },
} = await response; // Async so we need to await
```

To inspect the storage, we can use the kv API. In this case, when the "show" command is issued, we query the current number of messages for the logged in user, then fetch and `console.log` each one in order

```typescript
if (input.toLocaleLowerCase() === "show") {
  const length: number = Number.parseInt(
    await jstzClient.accounts.getKv(contractAddress, {
      key: `messages/${address}/length`,
    }),
  );
  for (let index = 0; index < length; index++) {
    const message = await jstzClient.accounts.getKv(contractAddress, {
      key: `messages/${address}/${index}`,
    });
    console.log(`[${index}]`, message);
  }
}
```

And that's it! You are now equipped to battle the evil forces of centralization. Go forth and do jstz 👊!
