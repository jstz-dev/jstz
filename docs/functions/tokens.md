---
title: Managing tokens
---

Jstz maintains a ledger of how many tez tokens (also known as XTZ) each user and smart function account owns, similar to but separate from the ledger that Tezos layer 1 uses.
You can use the [Asset bridge](/architecture/bridge) to move tez from layer 1 to Jstz and back.

:::tip

Internally, Jstz tracks tez not as individual tez but as _mutez_, which are equal to one-millionth of one tez.

:::

## Sending tez

If a smart function has tez, it can send tez to a user account or smart function as part of a request by putting the amount of mutez in the `X-JSTZ-TRANSFER` header in requests, as in this example:

```typescript
// 1 tez = 1 million mutez
const ONE_TEZ = 1000000;
// Transfer 1 XTZ to smart function B
const smartFunctionB = "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU5";
const send_request = new Request(`jstz://${smartFunctionB}`, {
  headers: {
    "X-JSTZ-TRANSFER": ONE_TEZ,
  },
});
await fetch(send_request);
```

<!-- Blocked by JSTZ-657
To send tez to a smart function without calling the smart function and running its handler function, send the tez in a request to `jstz://<ADDRESS>/-/noop`, as in this example:

```typescript
const ONE_TEZ = 1000000;
const call_request = new Request(`jstz://${smart_function}/-/noop`, {
  headers: {
    "X-JSTZ-TRANSFER": ONE_TEZ.toString(),
  },
});
```
-->

As described in [Errors](/functions/errors), any transfers are reverted if a smart function throws an uncaught error.

## Receiving tez and transferring tez in a response

Smart functions automatically accept tez sent to them.
You can see how many mutez are in a request by checking the `X-JSTZ-AMOUNT` header, which includes the amount as a string.

A smart function can transfer tez in a response by setting the `X-JSTZ-TRANSFER` header in the response.
For example, this smart function receives tez and returns it in the response:

```typescript
const ONE_TEZ = 1000000; // 1 XTZ in mutez

const handler = async (request: Request): Promise<Response> => {
  const transferred_amount_string = request.headers.get("X-JSTZ-AMOUNT");
  const transferred_amount = parseInt(transferred_amount_string || "0");
  console.log(`Received ${transferred_amount} mutez`);

  if (transferred_amount < ONE_TEZ) {
    throw "Send at least one tez and I will return it";
  }

  return new Response("Thank you!", {
    headers: {
      "Content-Type": "text/utf-8",
      "X-JSTZ-TRANSFER": transferred_amount.toString(),
    },
  });
};

export default handler;
```

To prevent a smart function from receiving tez, throw an exception to revert the transfer if the `X-JSTZ-AMOUNT` header is set, as in this example:

```typescript
const handler = (request: Request): Response => {
  const transferred_amount_string = request.headers.get("X-JSTZ-AMOUNT");
  const transferred_amount = parseInt(transferred_amount_string || "0");
  console.log(`Received ${transferred_amount} mutez`);
  if (transferred_amount > 0) {
    throw "Don't send tez to this smart function.";
  }
  return new Response("OK", {
    headers: {
      "Content-Type": "text/utf-8",
    },
  });
};

export default handler;
```
