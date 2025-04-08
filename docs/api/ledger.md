# 💰 Ledger

::: warning
The Ledger API is deprecated and will be removed in future versions of Jstz
:::

The `Ledger` object maintains a persistent ledger of all accounts and their
balances of L2 tez (stored as mutez). Additionally, the `Ledger` object stores the 'self address' of
the smart function, which is the address of the smart function itself.

All operations on `Ledger` are synchronous and atomic, committed if the request to the smart function succeeds.

## Quick Start

We can obtain the balance of an account using `Ledger.balance()`:

```typescript
const alice: Address = "tz1abc...";
console.log(Ledger.balance(alice)); // 0
```

The _self address_ of the smart function is accessible from the readonly property `Ledger.selfAddress`:

```typescript
console.log(Ledger.balance(Ledger.selfAddress)); // 420
```

Transfers are performed using `Ledger.transfer()`:

```typescript
Ledger.transfer(alice, 420); // Transfer 420 mutez to Alice from the balance of the smart function
console.log(Ledger.balance(alice)); // 420
console.log(Ledger.balance(Ledger.selfAddress)); // 0
```

## Types

### `type Address = string`

An address is a string of 36 characters, starting with `KT1`.

## Instance Properties

### `readonly Ledger.selfAddress: Address`

The `selfAddress` property of the `Ledger` object is the address of the smart function.

## Instance Methods

### `Ledger.balance(address: Address): Mutez`

Returns the balance of the given address in mutez, or `0` if the address is not in the ledger.

### `Ledger.transfer(dst: Address, amount: Mutez): void`

Transfers the given amount of mutez from the balance of the smart function to the given address. If the smart function does not have enough balance, this throws an error.
