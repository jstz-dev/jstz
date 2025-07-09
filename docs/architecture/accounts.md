---
title: Accounts
---

Jstz works with two kinds of accounts:

- User accounts start with `tz1` and store tez (XTZ).

- Smart function accounts start with `KT1` and store tez and the immutable code of the smart function.

## Working with user accounts

The CLI stores user accounts in the local file `~/.config/jstz/config.json`, including the alias, address, public key, and secret key for each account.

:::warning

You must keep the secret keys for the accounts secure.

:::

To create a user account, pass an alias for the new account to the `jstz account create` command, as in this example:

```bash
jstz account create <ALIAS>
```

When Jstz creates an account in this way, you can use it on any network, but for many commands you must specify the network to use with the `-n` argument.

The new account does not automatically become the active, logged in account.
To switch the active account, pass the account alias to the `jstz login` command.
The active account is the account that calls to smart functions and transfers of tez come from.
You can list all accounts with the `jstz account list` command and get information about the active account with the `jstz whoami` command.

Before you can use an account, it must be revealed.
To reveal an account, send tez to it.
For example, you can bridge one tez to it with this command, where `<ALIAS>` is the alias for the account:

```bash
jstz bridge deposit --from bootstrap1 --to <ALIAS> --amount 1 -n dev
```

## Signing transactions from user accounts

To sign transactions from user accounts, use a wallet, as in the Jstz development wallet: https://github.com/jstz-dev/dev-wallet.
For an example, see the [Quick start](/quick_start).

You can also sign transactions directly with the account's secret key, but in this case, managing the security of the key is up to you.
For example, the [`show-tez`](https://github.com/jstz-dev/jstz/tree/main/examples/show-tez) example signs accounts with the `jstz_sdk` library:

```typescript
import { Jstz } from "@jstz-dev/jstz-client";
import * as signer from "jstz_sdk";

// ...

const runFunction = buildRequest(functionAddress, input);
const nonce = await jstzClient.accounts.getNonce(address);
const operation = {
  content: runFunction,
  nonce,
  source: address,
  publicKey: publicKey,
};
// Sign the operation
const signature = signer.sign_operation(operation, secretKey);
// Send the operation
const response = jstzClient.operations.injectAndPoll({
  inner: operation,
  signature: signature,
});
```

## Working with smart function accounts

Like user accounts, you can set local aliases for smart functions with the `jstz account alias` command.
Then you can refer to that alias instead of the full address in other commands.

After you deploy a smart function, you can't modify its code or access its tez balance directly, but you can view its code with the `jstz account code` command.
For example to get the code of a smart function in the local sandbox, run this command, with the address or alias as `<ALIAS_OR_ADDRESS>`:

```bash
jstz account -a <ALIAS_OR_ADDRESS> -n dev
```

To call a smart function from the command line, use the `jstz run` command and pass the address or alias, as in this example from the [Quick start](/quick_start):

```bash
jstz run jstz://<ALIAS_OR_ADDRESS>/ --data '{"message":"Give me tez now."}' -n dev
```

The Jstz command-line client sends the command from the active local user account.
Using aliases like this works only in the command-line client, not in the client SDK.
