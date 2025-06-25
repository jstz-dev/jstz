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

You must provide the secret key of the user account to sign transactions.
For example, the [`call-from-web`](https://github.com/jstz-dev/jstz/tree/main/examples/call-from-web) sample application uses the Jstz client SDK to sign applications given the address, public key, and secret key of the application:

```typescript
import { Jstz } from "@jstz-dev/jstz-client";
import * as signer from "jstz_sdk";

// ...

// Sign operation using provided secret key
// DO NOT use this in production until Jstz has a way of signing in a secure manner
const signature = signer.sign_operation(operation, secretKey);
const response = await jstzClient.operations.injectAndPoll({
  inner: operation,
  public_key: publicKey,
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
