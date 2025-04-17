# 💸 Asset Bridge

`jstz` maintains a persistent ledger of all accounts and their balances of L2 tez (stored as mutez).

The `jstz` _bridge_ implements a bridge protocol that allows to transfer tezos tokens from Tezos to the `jstz` rollup and back.

## Quick Start

### Deposit

The `jstz` CLI empowers you to effortlessly transfer assets between a Tezos address (`tz1`) and a `jstz` L2 address (`tz1`) using the provided `bridge` commands.

To deposit assets from a Tezos address to a `jstz` L2 address, run the following command:

```bash
jstz bridge deposit --from <TZ1_ADDRESS/ALIAS> --to <TZ1_ADDRESS/ALIAS> --amount <AMOUNT>
```

Replace `<TZ1_ADDRESS/ALIAS>` with the source Tezos address or alias (managed by `octez-client`), `<TZ1_ADDRESS/ALIAS>` with the destination `jstz` address, and `<AMOUNT>` with the quantity of XTZ to deposit.

For example, running:

```bash
jstz bridge deposit --from tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU \
    --to tz1ZvXcDBWMAys2ro6kJXrgiWUcUF8RvCHYy \
    --amount 42
```

successfully deposits 42 XTZ from `tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU` to the `tz1ZvXcDBWMAys2ro6kJXrgiWUcUF8RvCHYy` `jstz` address, outputting:

```
Deposited 42 XTZ to tz4N7y3T2e2dfCyHB1Ama68jnt3Fps7Ufu6d
```

### Withdraw

::: danger
⚠️ Withdrawals on `jstz` to Tezos is still a work in progress
:::

To withdraw assets from `jstz` L2 address to a Tezos address, run the following command:

```bash
 jstz bridge withdraw --to <TZ1_ADDRESS/ALIAS> --amount <AMOUNT>
```

This will withdraw `<AMOUNT>`ꜩ from the current logged in `jstz` account to `<TZ1_ADDRESS/ALIAS>` Tezos account

For example, running:

```bash
jstz bridge withdraw --to tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU \
    --amount 42
```

will output the following response on success:

```bash
Running function at tezos://jstz/withdraw
Status code: 200 OK
Headers: {}
```

Communication from L2 and L1 within the Tezos ecosystem is performed through [outbox messages](https://tezos.gitlab.io/shell/smart_rollup_node.html#triggering-the-execution-of-an-outbox-message). Use [execute_latest_outbox_message](https://github.com/jstz-dev/jstz/blob/main/scripts/execute_latest_outbox_message.sh) to execute the withdraw message.

::: warning
⚠️ The following example will not work with `octez-client` in the Nix shell.
:::

```bash
# Requires octez client binary as an argument
./scripts/execute_latest_outbox_message.sh octez-client
```

## How it Works?

::: danger
⚠️ Under construction ⚠️
:::
