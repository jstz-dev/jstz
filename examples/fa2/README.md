# `jstz` FA2 👨‍⚖️

This is a simple FA2 smart function that allows users to:

- Mint new tokens 🪙
- Transfer tokens between `tz1` accounts 🤝
- Get the token balance of a `tz1` account 💰
- Manage operators 🧑‍🤝‍🧑

Our implementation follows the TZIP-12 [specification](https://tzip.tezosagora.org/proposal/tzip-12/) with the following deviations:

- The `balance_of` entrypoint does not take a `callback` parameter. Instead, it returns the balance directly in a `Response` object.

## Install

To build and bundle the smart function, run:

```sh
npm install
npm run build
```

## Deploy

To deploy the smart function, run:

```sh
jstz sandbox start
jstz deploy dist/index.js
```

## Demo

This example contains a test scenario that demonstrates the above functionality using a `scenario` smart function.

The scenario performs the following:

1. Deploys two scenario 'actors' (i.e. smart functions that own tokens).
2. Mints two tokens with ids `1` and `2`, minting 3 of token 1 to the first actor and 3 of token 2 to the second actor.
3. Transfers 1 token 1 from the first actor to the second actor, and 1 token 2 from the second actor to the first actor.
4. Checks that the balances of the actors are as expected.
5. Attempts for the scenario smart function to steal all tokens, this initially should fail.
6. The first and second actors add the scenario smart function as an operator of their tokens.
7. The scenario smart function successfully steals all tokens from the first and second actors.
8. Checks that the balances of all actors are as expected.

To deploy and run, execute:

```sh
npm run build:test
fa2=tz1...
jstz deploy dist/test/index.js
scenario=tz1...
jstz run "tezos://$scenario/?fa2=$fa2"
```
