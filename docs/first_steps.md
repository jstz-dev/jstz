# 👶 First Steps

This guide will instruct through setting up and deploying your first _smart function_.

## Prerequisites

1. Install `jstz`
2. Install Octez

## Hello World

`jstz` is a JavaScript runtime for Tezos 2.0 that aims to be compatible with web conventions.
`jstz` additionally has support for TypeScript, as our example below highlights.

In this example, the smart function takes a HTTP `Request` object and returns a `Response` object
containing the message `Hello [name]` (which is also printed to the console).

```typescript
// <src="examples/hello-world/index.ts">
const capitalize = (word: string): string => {
  return word.charAt(0).toUpperCase() + word.slice(1);
};

const hello = (name: string): string => {
  return "Hello " + capitalize(name);
};

const handler = (request: Request): Response => {
  const url = new URL(request.url);
  const name = url.searchParams.get("name") || "World!";

  const msg = hello(name);
  console.log(`Message: ${msg}`);

  return new Response(msg);
};

export default handler;
```

The smart function must consist of:

1. An `export default` statement.

   `export default` is JavaScript syntax required for defining an EMCAScript module.
   Smart functions _must_ have an default export of a function, which has the following type:

   ```typescript
   type Handler = (req: Request) => Response | Promise<Response>;
   ```

2. A `handler` function.

   Incoming HTTP requests to the smart function are passed to the `handler` as a [`Request`]() object.
   The runtime expects the handler to return a [`Response`]() object.

## Deploying your Smart Function

First we must compile our TypeScript code to JavaScript using:

```sh
npm run build
```

Once built, we can deploy our smart function in a local `jstz` sandbox.

### Running the Sandbox 🏝️

To start the sandbox, simply run:

```sh
cargo run -- sandbox start
```

<!-- TODO: CLI -->

To view responses to any sandbox operation, you must run the following (in a separate terminal):

```sh
cargo run -- trace
```

Once the sandbox has spun up, we may now deploy our smart function.

```sh
cargo run -- deploy index.js
```

In the window running `trace`, you should see the following output (or something similar):

```text
[📜] Smart function created: tz4RQn8huKS9KLoHZxWkghytBzxwbn84JnSb
```

This is the _address_ of your smart function, which is analogous to an IP address. It is often useful to store temporary addresses is environment variables:

```sh
hello_world=tz4RQn8huKS9KLoHZxWkghytBzxwbn84JnSb
```

## Testing your Smart Function

To test and run a smart function, we need to craft a HTTP request. To aid us with this, the `jstz` CLI provides a `curl`-inspired `run` command: `jstz run [url] --request --data`.

The URL that `jstz run` expects differs from a standard HTTP URL in the following ways:

1. The URL scheme must be `tezos` not `http` or `https`.
2. The hostname of the URL is the _smart function address_ (beginning with `tz4`).

For example, run the following to send a `GET` request to our `hello_world` smart function.

```sh
cargo run -- run "tezos://${hello_world}/?name=world"
```

If everything worked correctly, the logs should contain the message

```text
[🪵] Message: Hello World
```
