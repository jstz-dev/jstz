---
title: Quick start
sidebar_label: Quick start
---

This guide will instruct you in writing, deploying, and using your first Jstz _smart function_ in under 10 minutes.

## Prerequisites

Before you begin, ensure that you have [installed Jstz](installation.md), Node.JS, `npm`, and Docker.

To verify your installation, run this command to check the version of Jstz:

```bash
jstz --version
```

It will also help to have a basic familiarity with [Typescript](https://www.youtube.com/watch?v=zQnBQ4tB3ZA).

## What is Jstz?

Jstz allows you to deploy _smart functions_, which are JavaScript applications that behave like [serverless applications](https://en.wikipedia.org/wiki/Serverless_computing), small applications that run only when called and do not have a persistent presence in memory on any specific server.
Jstz smart functions run on the Tezos blockchain via [Smart Rollup](https://docs.tezos.com/architecture/smart-rollups) technology.
Running on Tezos provides smart functions with many of the same advantages as [smart contracts on Tezos](https://docs.tezos.com/smart-contracts):

- Smart functions are persistent, transparent, and immutable, which allows users to trust that they will stay available and will not change how they behave or be shut down
- Smart functions are censorship-resistant, because they are deployed on distributed Jstz Smart Rollup nodes and therefore no one can block calls to them
- Smart functions have no long-term hosting cost; they incur a cost only when called

Also, because smart functions run on a Tezos Smart Rollup instead of directly on Tezos, they have the additional benefits of low gas cost and reduced latency that Tezos layer 2 provides.

## 1. A sample smart function

Like smart contracts, you compile and deploy smart functions and then they cannot be changed.
However, smart functions behave more like web applications because they accept requests and return responses in a way similar to ordinary web-based HTTP requests.

The sample smart function in the `get-tez` folder of the Jstz repository stores tez and sends 1 tez (the primary cryptocurrency token of the Tezos chain and therefore the primary token of Jstz) to requesters who ask politely.
Its code is written in ordinary TypeScript:

```typescript
// <src="examples/get-tez/index.ts">

// 1 tez = 1 million mutez
const ONE_TEZ = 1000000;

// Maximum amount of tez a requester can receive
const MAX_TEZ = 10000;

// Get the amount of tez that the smart function has sent to an address
const getReceivedTez = (requester: Address): number => {
  let receivedTez: number | null = Kv.get(`received/${requester}`);
  receivedTez = receivedTez === null ? 0 : receivedTez;
  console.debug(`Requestor already received ${receivedTez} tez`);
  return receivedTez;
};

// Update the record of the amount of tez that an address has received
const setReceivedTez = (requester: Address, received: number): void => {
  Kv.set(`received/${requester}`, received);
};

// Log the message that the user sent
const addPoliteMessage = (requester: Address, message: string): void => {
  let length: number | null = Kv.get(`messages/${requester}/length`);
  if (length === null) {
    length = 0;
  }
  Kv.set(`messages/${requester}/${length}`, message);
  Kv.set(`messages/${requester}/length`, length + 1);
};

// Main function: handle calls to the smart function
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
  const receivedTez = getReceivedTez(requester);
  if (receivedTez >= MAX_TEZ) {
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

  // Log the updates
  setReceivedTez(requester, receivedTez + 1);
  addPoliteMessage(requester, message);

  return new Response(
    JSON.stringify("Thank you for your polite request. You received 1 tez!"),
  );
};

export default handler;
```

This smart function consists of:

- A few utility functions that read and write data with the Jstz key-value store.
  Each time a smart function is called, it runs in a new environment and therefore must store any persistent data in the key-value store.
  In this case, the smart function records how much tez it has sent to each address and also logs the messages that users send to it.

- A `handler` function.

  The handler function is the sole entrypoint of a Jstz smart function; it is what runs when a client calls the smart function.
  The handler function receives a Jstz [request](/api/request) object that includes the address of the account that called it in its `Referer` header and an optional request body.
  It must return a Jstz [response](/api/response) object.
  In this way, smart functions behave much like conventional web server handlers or cloud functions.

  In this case, the handler runs this logic:

  1. It gets the address that sent the request and the message from the request body.
  1. It verifies that the message was polite, in this case that it included the word "please."
  1. It checks to see if the account has already received the maximum amount of tez from it.
  1. It verifies that it has at least one tez in its account.
     The smart function's tez balance is in the Jstz persistent ledger of tez balances of all accounts, which the smart function can access with the [`Ledger`](./api/ledger.md) API.
  1. It sends one tez to the requester with the Ledger API.
  1. It updates its information in the key-value store, including the message that the requester sent and the new total amount of tez that the requester has received.
  1. It returns a text message to the requester.

- An `export default` statement.

  `export default` is JavaScript syntax required for defining an ECMAScript module.
  Smart functions _must_ have a default export of a function that has the following type:

  ```typescript
  type Handler = (req: Request) => Response | Promise<Response>;
  ```

## 2. Deploying the smart function {#deploying-the-smart-function}

Follow these instructions to deploy the sample smart function to a local sandbox:

1.  Clone the Jstz repository and navigate to the `get-tez` example:

    ```sh
    git clone https://github.com/jstz-dev/jstz.git && cd jstz/examples/get-tez
    ```

    You may see an error that says `.envrc is blocked.`
    You can ignore this error because it refers to setting up a development environment to build Jstz locally.

1.  Install the dependencies for the smart function:

    ```sh
    npm install
    ```

1.  Start the local sandbox in a Docker container:

    ```sh
    jstz sandbox --container start
    ```

    If you see an error that says that the configuration file is improperly configured, delete the `~/.config/jstz/` folder and try to start the sandbox again.

    :::tip

    The `--detach` (`-d`) flag starts the sandbox in the background, allowing you to continue working in the same terminal.
    You can stop or reset the sandbox with the commands `jstz sandbox --container stop` or `jstz sandbox --container restart`, but the state of the sandbox is not persistent.

    :::

    When the sandbox starts, it shows the bootstrap accounts and their balances on Tezos layer 1, which you can use to fund smart functions and user accounts in Jstz:

1.  Open a new terminal window, go to the `jstz/examples/get-tez` folder, and run this command to compile and deploy the smart function to the sandbox:

    ```sh
    npm run build
    jstz deploy dist/index.js -n dev
    ```

    If this is your first time deploying a smart function, the `deploy` command prompts you to create a Jstz account.
    You can use any local name and passphrase for the account.
    Later, you can create accounts with the `jstz account create` and switch accounts with the `jstz login` and `jstz account` commands.

    Upon successful deployment, Jstz assigns the smart function a unique `KT1` address.
    This address is its identifier, similar to an IP address or a smart contract address.

    In the example above, the smart function was deployed to the address `KT1FZuQ4SDP7ahLRyybtNnNxNnRskBGyAXVw`.
    Now the smart function is accessible through a URL of the format `jstz://KT1FZuQ4SDP7ahLRyybtNnNxNnRskBGyAXVw/`.

    After you deploy the smart function, you cannot delete it or change it.

1.  Fund the smart function by running this command, using your smart function address for the `<ADDRESS>` variable:

    ```sh
    jstz bridge deposit --from bootstrap1 --to <ADDRESS> --amount 1000 -n dev
    ```

    This command bridges tez from a layer 1 bootstrap account to a Jstz account.
    Like Tezos smart contracts, Jstz smart functions are a type of account and can store and transfer tez.
    For more information about bridging to Jstz, see [Asset Bridge](/architecture/bridge).

## 3. Calling the smart function

After a successful deployment, you can call the smart function in a way similar to sending an HTTP request.

1. Ask the smart function for tez in an impolite way by running this command, with your smart function's address:

   ```sh
   jstz run jstz://<ADDRESS>/ --data '{"message":"Give me tez now."}' -n dev
   ```

   The smart function returns the message "Sorry, I only fulfill polite requests."

1. Ask the smart function politely by running this command, which includes the word "please" in the message:

   ```sh
   jstz run jstz://<ADDRESS>/ --data '{"message":"Please, give me some tez."}' -n dev
   ```

   The function returns the message "Thank you for your polite request. You received 1 tez!"

1. Check your balance by running this command:

   ```sh
   jstz account balance -n dev
   ```

   The response is the current balance of the currently logged in account, including the 1 tez that the smart function sent.

1. Get your user account address by running this command:

   ```sh
   jstz whoami
   ```

   Like Tezos accounts, Jstz user account start with `tz1`.

1. Check the key-value store for the smart function to see that it recorded your tez and messages by running this command, with your user account address in place of the variable `<USER_ADDRESS>`:

   ```sh
   jstz kv get -a <ADDRESS> -n dev "received/<USER_ADDRESS>"
   ```

   The response is the amount of tez that the smart function has sent to your user account.
   Note that the key-value storage for a smart function is visible to all accounts, but only the smart function itself can write to its key-value store.

   You can also see the messages that you have sent to the smart function in the key-value store by looking them up by index, as in this example:

   ```sh
   jstz kv get -a <ADDRESS> -n dev "messages/<USER_ADDRESS>/0"
   ```

Congratulations! 🎉 You have now successfully deployed and crafted a Jstz request to run your first smart function.

:::tip
For debugging, you can listen to the log of a smart function with the command `jstz logs trace` which behaves like the Linux command `tail -f`.

You can also send a request and view the log messages generated from that request by adding the `--trace` flag to the `jstz run` command.
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
   If you input any other text, it sends that text as a request to the smart function, as in the command you ran earlier: `jstz run jstz://<ADDRESS>/ --data '{"message":"Please, give me some tez."}' -n dev`.

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

The complete code of the CLI application is in the file `examples/show-tez/src/index.ts`.
Here are some details about how the CLI application works with the smart function:

It imports the Jstz client library, client library types, and the signing library, which signs Jstz transactions:

```javascript
import { Jstz } from "@jstz-dev/jstz-client";
import JstzType from "@jstz-dev/jstz-client";
...
import * as signer from "jstz_sdk"; // <- signing library
```

:::warning
The signing library is a temporary solution while we build out the secure signing interface.
Essentially, it is a WASM program directly compiled from the core Jstz Rust code.
Although it is semantically correct, do not use this library in production because it involves copying users' secret keys, which is not secure.
:::

The `buildRequest` function constructs a `RunFunction` operation that calls the smart function by its `jstz://<ADDRESS>` URL.

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
    uri: `jstz://${contractAddress}`,
  };
}
```

The `main` function controls the behavior of the CLI application.
It starts the command-line terminal with the `readline.createInterface` function and processes each message from the user.

When it receives the message `show`, it uses the Jstz client API to get the messages from the smart function's key-value store:

```typescript
if (input.toLocaleLowerCase() === "show") {
  // If the user sends "show," print their messages from the contract's key-value store
  const length: number = Number.parseInt(
    // Get the total number of messages sent by the user account
    (await jstzClient.accounts.getKv(contractAddress, {
      key: `messages/${address}/length`,
    })) as string,
  );
  // Print each message
  for (let index = 0; index < length; index++) {
    const message = await jstzClient.accounts.getKv(contractAddress, {
      key: `messages/${address}/${index}`,
    });
    console.log(`[${index}]`, message);
  }
}
```

When it receives any other message, it follows these steps:

1. It uses the `buildRequest` function to create a Jstz.
1. It gets the account nonce, which is a unique value that prevents a transaction from being duplicated.
1. It signs the operation with the user's secret key and nonce.
1. It sends the transaction to Jstz and waits for the response.
1. It prints the response to the console.

This is the code that assembles, signs, and sends the request:

```typescript
// If the user sends any message other than "show,"
// send that message as a request to the smart function
const runFunction = buildRequest(contractAddress, input);
const nonce = await jstzClient.accounts.getNonce(address);
const operation = {
  content: runFunction,
  nonce,
  source: address,
};
// Sign the operation
const signature = jstz_sdk.sign_operation(operation, secretKey);
// Send the operation
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

And that's it! You are now equipped to battle the evil forces of centralization. Go forth and do Jstz 👊!

If you want to take these applications further, you can change how the smart function distributes tez or stores data.
You can also try running the example web application at `examples/call-from-web` to see how Jstz works with a web application.
