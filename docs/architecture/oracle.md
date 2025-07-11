---
title: Enshrined oracle
---

:::warning

This feature is under development and unstable.
Implementation details might change.

:::

For technical reasons, Web3 programs such as smart contracts and smart functions cannot access off-chain data directly, including calling external APIs.
They must use programs called oracles to get data that is not available to them on the chain.
For more information about oracles in general, see [Oracles](https://docs.tezos.com/smart-contracts/oracles) on docs.tezos.com.

For information about calling the oracle from a smart function,see [Calling external APIs](/functions/apis).

Jstz provides a built-in, or _enshrined_, oracle to provide off-chain data to smart functions in a deterministic, secure, and soon trust-minimal way.
You can imagine this oracle as a proxy gateway for network-accessible APIs.
Smart functions can call the oracle with an ordinary HTTP request and the oracle retrieves the data and returns it to the smart function.
This process has a 20 second timeout.
If it fails to get the data, such as if the endpoint is unreachable, the oracle returns a 502 Bad Gateway error.

Oracle calls take full advantage of Jstz's asynchronous nature; smart functions awaiting pending oracle calls do not block the Jstz chain or other calls to the same smart function.
As with asynchronous JavaScript, awaiting an async call suspends the smart function, allowing other operations to call the same smart function in the interim.
As described in [Smart functions](/functions/overview), the state of a running smart function is isolated and atomic, so other smart function calls are not aware of the suspended smart function's uncommitted state until it completes running and commits its changes to the key-value store and ledger.

:::note

The Jstz oracle's design was influenced by [Town Crier](https://www.town-crier.org/dev/) oracles.
However, unlike Town Crier, it is directly integrated into the Jstz protocol and exposed through Fetch API.

:::

## How the oracle works

Calling the oracle involves these general steps:

1. A smart function sends a `fetch` request to the on-chain component of the oracle and suspends to await the promise.
1. The oracle publishes the request.
1. The off-chain oracle node receives the request and executes it as usual for a `fetch` request.
1. When it receives the response, the off-chain oracle node signs it to authenticate it and injects it into Jstz as a specific operation type known as an `OracleResponseOperation` operation.
1. Jstz receives the operation, verifies the signature, and returns a Response object to the smart function.
1. The `fetch` request promise resolves with the Response object and the smart function resumes operation.

## Limitations

The oracle is under active development and is changing rapidly.
At this time it lacks some features that are necessary to be a truly trustless, decentralized web3 platform.
Here are some notes about limitations and how it may change to work better as a web3 tool:

- The oracle node does not run in a [trusted execution environment](https://en.wikipedia.org/wiki/Trusted_execution_environment) yet, but support for it is planned, which will reduce the trust required around the correctness of the response.
- Support is planned for private requests that will allow smart functions to access authenticated endpoints like those requiring API tokens.
- For now there is only a single centralized oracle node operator.

Additional details that are subject to change:

- Smart functions cannot call the oracle during or after they read from or write to the key-value store.
  They must make oracle calls first and wait for its promise to be resolved before accessing the key-value store.
  If the oracle detects that the smart function has accessed the key-value store before sending the request, it returns a rejected promise.
- Similarly, smart functions cannot send or receive tez or call other smart functions before or during an oracle call.
- Oracle calls are free today but will consume gas (transaction fees) in the future based on factors such as the size of the request and response.
  Similarly, a smart function consumes no gas while suspended but may in the future.
- The cap on the size of an API response is approximately 10MiB.
- Oracle calls cannot currently be cancelled.
- TTL for requests is 20 seconds.
- Requests that require a long-lived, persistent, or keep-alive connection are not supported.
