---
title: Enshrined Oracle
---

:::note

This feature is bleeding edge and hence highly unstable. Constants and implementation details might change but the experience of fetching off chain data, the reliability, latency and trust assumptions will only improve

:::

Smart functions can leverage the enshrined oracle to access off chain data in a deterministic and secure (and soon trust minimal) way. The Oracle can be thought off as a Proxy Gateway any network accessible API. If you're coming from web3, tts design was influened by [Town Crier](https://www.town-crier.org/dev/) oracles. However. unlike TC, it is directly inetegrated into the Jstz protocol and exposed through Fetch API. Instead of targetting an endpoint with the jstzs scheme, target an endpoint using the http/https schemes. HTTP/S URLs will be routed to the protocol's enshrined oracle which wil handle the request and return a promise that will eventually resolve with a Response from the Oracle node, 408 Request Timeout if the Oracle node cannot fulfill the request within roughly 20 seconds (subject to change) or 502 Bad Gateway if the endpoint is totally unreachable eg. DNS cannot be resolved.

Most of the time, however, the response should arrive within a few seconds or less. The oracle calls take full advantage of Jstz's async nature; awaiting on pending fetch calls do not block the Jstz chain and will instead suspend
your smart function, allowing other operations to interact with your contract. Note that uncommited state is both isolated and atomic. That means that local/global updates within the smart function will not be reflected in another RunFunction call of your contract. Similarly, KV updates will not be reflected to another RunFunction either.

Underneath the hood
When SF makes an oracle call, the request is routed to the enshrined Oracle which will leak it via the Events channel being listened too by off chain Oracle Node. The Oracle Node will pick up the request, execute it, sign the Respopnse and inject an OracleResponseOperation. When the protocol processes the operation, it will check that signature is valid and attempt to resume the suspended execution with the Response.

The oracle node today does not run in a TEE but support for it is planned which will reduce the trust required around Response correctnes. There is also planned support for private requests, enabling acess to authenticated endpoints like those requiring API tokens. These are planned for mainnet launch.

Decentralization

For the time being, there will only be a single Oracle Node operator as we plan to release Jstz as a private network in the short term as we build out and design the best user experience possible while gradually decentralizing over the long term.

The Oracle infra is shipped with jstzd and will be available on privatenet

Additional details that are subject to change

- Since the internal atomic state update component (called Transaction) does not support async yet, only calls within a clean Transaction context is allowed to call the oracle and receive a response. That means your smart function must not use the KV before making an Oracle request or while an oracle request is in flight. If the protocol detects that the Traansaction state is dirty, the oracle call promise will be rejected. This is also true if you call another smart function or was called by another smart function; any KV access/update is any part of the compute chain will reject the Oracle call promises
- Oracle calls are free today but will consume gas in the future. Additionally, suspended execution cost no gas
  The current planned model for gas is such that the caller must define upfront a maximum limit similar to how Tezos does it.
- Oracle calls cannot be cancelled but support for cancellation is planned
- TTL for requests is 20 seconds - roughly 20 blocks in Jstzd/Privatenet
- Long requests are not supported and will likely not be supported for a while
- There is roughly a 10MiB cap on the size of a response but do keep in mind that processing larger responses will cost more
