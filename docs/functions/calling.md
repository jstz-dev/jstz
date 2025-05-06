# Calling other smart functions

Smart functions can call other smart functions with the `SmartFunction.call()` method, which returns a promise that resolves to a Jstz [`Response`](/api/response) object.
Here is an example of calling another contract:

```typescript
const targetContract: Address = "KT1L8ZzGDzaXZSTmzHkoF2azQRf7dCAfxtqx";

const response = await SmartFunction.call(
  new Request(`jstz://${targetContract}`, {
    method: "POST",
    body: JSON.stringify({ message: "hello" }),
  }),
);
console.log(await response.json());
```

The URL for the [`Request`](/api/request) object must be `jstz://` followed by the address of a Jstz smart function.
Smart functions cannot call external APIs or Tezos smart contracts directly.
You can set the method in the `Request` object but you cannot set the `Referer` header because it automatically becomes the address of the smart function.

::: tip

To transfer tez with the call, use the `X-JSTZ-TRANSFER` header as described in [Managing tokens](/functions/tokens).

:::

Failed calls to smart functions cause Jstz to immediately revert the current transaction.
See [Handling errors](/functions/errors).
