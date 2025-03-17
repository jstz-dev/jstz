# 🚀 Quick Start

This guide will instruct you in writing, deploying, and using your first _smart function_ in under 10 minutes.

## Prerequisites

Before you begin, ensure that you have [installed Jstz](installation.md), Node.JS, `npm`, and Docker.

To verify your installation, run this command to check the version of Jstz:

```bash
jstz --version
```

It will also help to have a basic familiarity with [Typescript](https://www.youtube.com/watch?v=zQnBQ4tB3ZA).

## What is Jstz?

Jstz is a specialized JavaScript runtime for [Tezos Smart Rollups](https://docs.tezos.com/architecture/smart-rollups) that aims to be compatible with web conventions.
It bridges the gap between JavaScript/TypeScript applications and the Tezos blockchain by allowing you to deploy _smart functions_.

Smart functions have the same security advantages as Tezos [smart contracts](https://docs.tezos.com/smart-contracts) with the additional benefits of low gas cost and reduced latency of running on Tezos layer 2, and integration with the JavaScript ecosystem.

## 1. A sample smart function

Like smart contracts, you compile and deploy smart functions and then they cannot be changed.
However, smart functions behave more like web applications because they accept requests and return responses through HTTP.

The sample smart function in the `get-tez` folder of the Jstz repository stores tez and sends 1 tez to requesters who ask politely.
Its code is written in TypeScript:

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

  A smart function processes an HTTP `Request` object and returns a `Response` object, mirroring the functionality of conventional web server handlers or cloud functions.

- An `export default` statement.

  `export default` is JavaScript syntax required for defining an ECMAScript module.
  Smart functions _must_ have a default export of a function that has the following type:

  ```typescript
  type Handler = (req: Request) => Response | Promise<Response>;
  ```

In addition to several [standard Web APIs](./api/index.md#web-platform-apis), Jstz introduces several concepts and APIs specific to smart functions:

- **Self address**.

  On deployment, each smart function is allocated a unique `KT1` address, akin to an IP address for the function.
  `Ledger.selfAddress` contains the (self) address of the smart function.

- **Referer header**.

  The `"Referer"` header contains the `tz1` address of the account that sent the request to the smart function.
  This address can be retrieved using `request.headers.get("Referer")`.

- **Ledger**

  Jstz maintains a persistent ledger of all accounts and their balances (in mutez, or one-millionth of a tez).
  The [`Ledger`](./api/ledger.md) API provides methods for transferring tez between accounts and querying account balances.

- **Key-Value store**

  Jstz maintains a persistent key-value store for each smart function, accessible through the [`Kv`](./api/kv.md) API.
  This smart function logs the requests it gets in that key-value store.

- **`SmartFunction` API**

  Smart functions can invoke other smart functions using `fetch`, similar to network requests in JavaScript.
  Additionally, smart functions can deploy other smart functions with the [`SmartFunction`](./api/smart_function.md) API.

## 2. Deploying the smart function {#deploying-the-smart-function}

Follow these instructions to deploy the sample smart function to a local sandbox:

1.  Clone the Jstz repository and navigate to the `get-tez` example:

    ```sh
    git clone https://github.com/jstz-dev/jstz.git && cd jstz/examples/get-tez
    ```

    Install the dependencies for the smart function:

    ```sh
    npm install
    ```

1.  Start the local sandbox in a Docker container:

    ```sh
    jstz sandbox --container start
    ```

    If you see an error that says that the configuration file is improperly configured, delete the `~/.jstz/` folder and try to start the sandbox again.

    ::: tip

    The `--detach` (`-d`) flag starts the sandbox in the background, allowing you to continue working in the same terminal.
    You can stop or reset the sandbox with the commands `jstz sandbox --container stop` or `jstz sandbox --container restart`, but the state of the sandbox is not persistent.

    :::

    When the sandbox starts, it shows the bootstrap accounts and their balances on Tezos layer 1, which you can use to fund smart functions and user accounts on layer 2:

    <details>
    <summary>Output</summary>
    <pre style="border: 1px solid #ccc; padding: 10px; border-radius: 4px; overflow-x: auto;">
    <code style="color: #FFF;">$ jstz sandbox start

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

    +--------------------------------------+---------------------+
    | Address | XTZ Balance (mutez) |
    +======================================+=====================+
    | tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV | 40000000000 |
    +--------------------------------------+---------------------+
    | tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx | 60000000000 |
    +--------------------------------------+---------------------+

    </code>
    </pre>
    </details>

1.  Compile and deploy the smart function to the sandbox:

    ```sh
    npm run build
    jstz deploy dist/index.js -n dev
    ```

    If this is your first time deploying a smart function, the `deploy` command prompts you to create a Jstz account.
    You can use any local name and passphrase for the account.
    Later, you can create accounts with the `jstz account create` and switch accounts with the `jstz login` and `jstz account` commands.

    <details>
    <summary>Output</summary>
    <pre style="border: 1px solid #ccc; padding: 10px; border-radius: 4px; overflow-x: auto;">
    <code style="color: #FFF;">$ npm run build
    > @jstz-dev/get-tez@0.0.0 build
    > esbuild index.ts --bundle --format=esm --target=esnext --minify --outfile=dist/index.js

    dist/index.js 777b

    ⚡ Done in 10ms

    $ jstz deploy dist/index.js -n dev
    You are not logged in. Please type the account name that you want to log into or create as new: alan
    Logged in to account alan with address tz1N8BsvfrSjGdomFi5V9RwwYLasgD8s4pxF

    Smart function deployed by alan at address: KT1FZuQ4SDP7ahLRyybtNnNxNnRskBGyAXVw
    Run with `jstz run tezos://KT1FZuQ4SDP7ahLRyybtNnNxNnRskBGyAXVw/ --data <args> --trace`

    </code>
    </pre>
    </details>

    Upon successful deployment, Jstz assigns the smart function a unique `KT1` address, serving as its identifier, similar to an IP address or a smart contract address.

    In the example above, the smart function was deployed to the address `KT1FZuQ4SDP7ahLRyybtNnNxNnRskBGyAXVw`.
    Now the smart function is accessible through a URL of the format `tezos://KT1FZuQ4SDP7ahLRyybtNnNxNnRskBGyAXVw/`.

1.  Fund the smart function by running this command, using your smart function address for the `<ADDRESS>` variable:

    ```sh
    jstz bridge deposit --from bootstrap1 --to <ADDRESS> --amount 1000 -n dev
    ```

    This command bridges tez from layer 1 bootstrap to a Jstz account.
    Like Tezos smart contracts, Jstz smart functions are a type of account and can store and transfer tez.
    For more information about bridging to Jstz, see [Asset Bridge](bridge.md).

## 3. Calling the smart function

After a successful deployment, you can call the smart function in a way similar to sending an HTTP request.

1. Ask the smart function for tez in an impolite way by running this command, with your smart function's address:

   ```sh
   jstz run tezos://<ADDRESS>/ --data '{"message":"Give me tez now."}' -n dev
   ```

   The smart function returns the message "Sorry, I only fulfill polite requests."

1. Ask the smart function politely by running this command, which includes the word "please" in the message:

   ```sh
   jstz run tezos://<ADDRESS>/ --data '{"message":"Please, give me some tez."}' -n dev
   ```

   The function returns the message "Thank you for your polite request. You received 1 tez!"

1. Check your balance by running this command:

   ```sh
   jstz account balance -n dev
   ```

   The response is the current balance of the currently logged in account, including the 1 tez that the smart function sent.

Congratulations! 🎉 You have now successfully deployed and crafted an HTTP request to run your first smart function.

::: tip
To deploy and interact with your function on networks beyond the local sandbox, like weeklynet, use the `--network` (`-n`) flag.
:::

For debugging, Jstz provides the following tools:

- `jstz logs trace` enables tailing the logs of a given smart function.
- `jstz kv` allows exploring the current state of the KV store, listing subkeys or retrieving values for a particular account.

::: tip
The `--trace` flag for `jstz run` prints the logs of smart functions, akin to the `jstz logs trace` command.
:::

## 4. Interacting with the smart function in Node.JS

As an example of how an off-chain application can interact with a smart function, the folder `examples/show-tez` is a CLI application written in Node.JS that accesses the smart function that you just deployed.
Follow these steps to build and run this application:

1. Build the `show-tez` example from within the `jstz/examples/show-tez` folder:

   ```sh
   # git clone https://github.com/jstz-dev/jstz
   cd jstz/examples/show-tez
   npm install
   npm run build
   ```

1. Ensure that you are logged in to Jstz.
   You can verify that you are logged in by running the command `jstz whoami`.
   To log in, run the command `jstz login <ALIAS>`, where `<ALIAS>` is the alias of your Jstz account.

1. Deploy the CLI application and pass the address of the `get-tez` smart function that you deployed in [Deploying the smart function](#deploying-the-smart-function):

   ```sh
   node dist/bundle.js <ADDRESS>
   ```

   The app shows the message `Please ask for tez politely. Type "show" to see past messages. Ctrl+C to quit`.

   The CLI accepts two commands.
   If you input the text `show`, it prints the history of commands.
   If you input any other text, it sends that text as a request to the smart function, as in the command you ran earlier: `jstz run tezos://<ADDRESS>/ --data '{"message":"Please, give me some tez."}' -n dev`.

1. Send a request that includes the word "please" and see that the smart function sends you one tez.

1. Try other requests and the `show` command, as in this example:

   ```sh
   # Example
   $ node dist/bundle.js KT1FZuQ4SDP7ahLRyybtNnNxNnRskBGyAXVw
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

1. When you are finished, press Ctrl+C to stop the CLI program.

1. Check your balance with the `jstz account balance -n dev` command.

The complete code of the function is in the file `examples/show-tez/src/intex.ts`.
Here are some details about how the CLI application works with the smart function:

It imports the Jstz client library, client library types, and the signing library, which signs Jstz transactions:

```javascript
// line 1
import { Jstz } from "@jstz-dev/jstz-client";
import JstzType from "@jstz-dev/jstz-client";
...
import * as jstz_sdk from "jstz_sdk"; // <- signing library
```

::: warning
The signing library is a temporary solution while we build out the secure signing interface.
Essentially, it is a WASM program directly compiled from the core Jstz Rust code.
Although it is semantically correct, do not use this library in production because it involves copying users' secret keys, which is not secure.
:::

The `buildRequest` function constructs a `RunFunction` operation that targets the smart function by its `tezos://<ADDRESS>` URL.

```typescript
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

Then the application injects this operation to Jstz by following these steps:

1. Build the operation content (`RunFunction` in this case)
2. Fetch the account nonce, which is a unique value that prevents a transaction from being duplicated
3. Construct the `Operation` object
4. Sign the `Operation` object with the user's secret key
5. Inject and (optionally) poll for the receipt, which you can think of as a response

```typescript
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

The smart function stores data about the accounts it has sent tez to in the Jstz key-value store.
The CLI application inspects this store when the user sends the "show" command and prints messages that are related to the logged-in user:

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

And that's it! You are now equipped to battle the evil forces of centralization. Go forth and do Jstz 👊!
