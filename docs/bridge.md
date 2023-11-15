# 💸 Asset Bridge

`jstz` maintains a persistent ledger of all accounts and their balances of L2 tez (stored as mutez).

The `jstz` _bridge_ implements a bridge protocol that allows to transfer fungible tokens (namely [CTEZ](https://ctez.app/)) from Tezos to the `jstz` rollup (and back *soon*™️).

::: danger
⚠️ Withdrawals from `jstz` to Tezos is not supported ⚠️
:::

## Quick Start

The `jstz` CLI empowers you to effortlessly transfer assets between a Tezos address (`tz1`) and a `jstz` L2 address (`tz4`) using the provided `bridge` commands.

To deposit assets from a Tezos address to a `jstz` L2 address, run the following command:

```bash
jstz bridge deposit --from <TZ1_ADDRESS/ALIAS> --to <TZ4_ADDRESS/ALIAS> --amount <AMOUNT>
```

Replace `<TZ1_ADDRESS/ALIAS>` with the source Tezos address or alias (managed by `octez-client`), `<TZ4_ADDRESS/ALIAS>` with the destination `jstz` address, and `<AMOUNT>` with the quantity of CTEZ to deposit.

For example, running:

```bash
jstz bridge deposit --from tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU \
    --to tz4N7y3T2e2dfCyHB1Ama68jnt3Fps7Ufu6d \
    --amount 42
```

sucessfully deposits 42 CTEZ from `tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU` to the `tz4N7y3T2e2dfCyHB1Ama68jnt3Fps7Ufu6d` `jstz` address, outputting:

```
Deposited 42 CTEZ to tz4N7y3T2e2dfCyHB1Ama68jnt3Fps7Ufu6d
```

## How it Works?

::: danger
⚠️ Under construction ⚠️
:::
