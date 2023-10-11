# `jstz` FA2 👨‍⚖️

This is a simple FA2 smart function that allows users to:

- Mint new tokens 🪙
- Transfer tokens between `tz4` accounts 🤝
- Get the token balance of a `tz4` account 💰
- Manage operators 🧑‍🤝‍🧑

Our implementation follows the TZIP-12 [specification](https://tzip.tezosagora.org/proposal/tzip-12/) with the following deviations:

- The `balance_of` entrypoint does not take a `callback` parameter. Instead, it returns the balance directly in a `Response` object.

## Install

To build and bundle the smart function, run:

```sh
npm run build
```

## Deploy

To deploy the smart function, run:

```sh
cargo run -- start sandbox
tz4=tz492MCfwp9V961DhNGmKzD642uhU8j6H5nB
cargo run -- deploy --self-address $tz4 --balance 0 --function-code "$(cat dist/index.js)"
```
