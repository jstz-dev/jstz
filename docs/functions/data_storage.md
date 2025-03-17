# Storing data

Smart functions store persistent data in a key-value database that is specific to each function.
Only the function itself can write to its storage.
Smart functions cannot read the data of other functions, but the data is publicly visible to applications outside of Jstz via the client API.

Keys are strings and values can be any serializable JavaScript object.
A smart function can write a new value for a certain key and the next time it reads that key, it receives the new value.

## Errors

Uncaught errors in smart functions reverse the entire transaction, as if the initial call to the smart function never happened.
This reversal includes any changes to the database.

For example, this code writes a value to storage in one line and throws an error on the next line.
If the smart function does not catch this error, the change to the key-value database on the first line does not happen.
The key keeps the same value that it had before the failed transaction.

```typescript
Kv.set("myKey", "This value is not stored because the next line fails");
throw "There is a problem, so reverse this transaction.";
```

You can catch errors in smart functions with try/catch blocks just like in ordinary JavaScript/TypeScript.

Although changes to the key-value database are atomic and committed only when the smart function request completes successfully, changes to the database are visible inside the smart function.
For example, if you set a value on one line and read it on the next line, you get the new value of the key, not the value prior to when the smart function was called.

## Smart functions

To store a value in a smart function, use the function `Kv.set`.
This example assigns the JSON object `{ myValue: 5 }` to the key `myKey`:

```typescript
Kv.set("myKey", { myValue: 5 });
```

To read a key-value pair, use the function `Kv.get`, as in this example:

```typescript
const data: object | null = Kv.get("myKey");
if (data) {
  const { myValue } = data;
  console.log(myValue); // 5
}
```

To delete a value, pass the key to the function `Kv.delete`.

To check if a value exists, pass the key to the function `Kv.has`, which returns a Boolean value.

The database stores keys separated with slashes as subkeys.
For example, it splits the key `myKey/subKey` into the subkey `subKey` of the key `myKey`.
This does not change how smart functions store and access data, but it allows API clients to browse subkeys when a smart function stores complex data.

For more information about the smart function key-value API, see [KV](/api/kv).

## TypeScript applications

Other applications can read the Jstz key-value store if they know the address of the smart function that wrote the value and the key that it used.
They cannot write to the key-value store.

This example uses the Jstz client SDK to get a value:

```typescript
import { Jstz } from "@jstz-dev/jstz-client";

type myData = {
  myValue: number;
};

const storedData: myData | null = await jstzClient.accounts
  .getKv(contractAddress, {
    key: "myKey",
  })
  .catch(() => {
    console.log("Value is not set");
  });

if (storedData) {
  console.log("myValue:", storedData.myValue);
}
```

## Command line

You can retrieve the value of a key with the `jstz kv get` command, where `<ADDRESS_OR_ALIAS>` is the address or alias of the smart function and `<KEY>` is the key:

```bash
jstz kv get -a <ADDRESS_OR_ALIAS> -n dev <KEY>
```

If the smart function stores data with subkey, delimited by slashes, you can get those subkey with the `jstz kv list` command.
For example, if a smart function stores data in the keys `myKey/a`, `myKey/b`, and `myKey/c`, you can get a list of these three subkey with this command:

```bash
jstz kv list -a <ADDRESS_OR_ALIAS> -n dev myKey
```

This command helps you explore complex data stored by smart functions.
To get the data for one of these subkey, use the `jstz kv get` command as usual, as in this example:

```bash
jstz kv get -a <ADDRESS_OR_ALIAS> -n dev `myKey/a
```
