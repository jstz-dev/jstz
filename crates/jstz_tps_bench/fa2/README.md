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
