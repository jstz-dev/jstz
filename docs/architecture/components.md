# Jstz components

Jstz has several components that provide functionality to different elements of the Jstz ecosystem:

## Jstz SDK

Smart functions must be built with the Jstz SDK, [`jstz_sdk`](https://www.npmjs.com/package/jstz_sdk), to be deployed on Jstz.
The SDK allows them to receive requests, return responses, and access the Jstz [API](/api/).

## Client SDK

The Jstz client SDK, [`@jstz-dev/jstz-client`](https://www.npmjs.com/package/@jstz-dev/jstz-client), is a JavaScript/TypeScript SDK that allows applications outside Jstz to:

- Send requests to Jstz smart functions
- Inspect the state of Jstz, such as getting account balances and values from the key-value store
- Deploy smart functions

## Jstz CLI

The [Jstz CLI](/cli), available as the package [`@jstz-dev/cli`](https://www.npmjs.com/package/@jstz-dev/cli) provides commands that you can use to deploy and interact with smart functions and the sandbox locally.

## `jstzd` daemon

The [`jstzd` daemon](/jstzd) runs services that are necessary to run the Jstz sandbox, including:

- The layer 1 bootstrap accounts that you can use to fund accounts in Jstz
- The [bridge](/architecture/bridge) that you can use to move tez from those accounts to your Jstz accounts
- A local version of the Jstz runtime

You can use the Jstz CLI to run the sandbox or use the `jstzd` daemon directly.
