---
title: Managing tokens
---

Jstz maintains a ledger of how many tez tokens (also known as XTZ) each user and smart function account owns, similar to but separate from the ledger that Tezos layer 1 uses.
Smart functions can accept and store the tez that callers include with requests, and they can send tez to users and smart functions.
You can use the [Asset bridge](/architecture/bridge) to move tez from layer 1 to Jstz and back.

:::tip

Internally, Jstz tracks balances not as individual tez but as _mutez_, which are equal to one-millionth of one tez.

:::

## Sending tez

If a smart function has tez, it can send tez to a user account or smart function as part of a request by putting the amount of mutez in the `X-JSTZ-TRANSFER` header in requests, as in this example:

```typescript
// 1 tez = 1 million mutez
const ONE_TEZ = 1000000;
// Transfer 1 XTZ to smart function B
const smartFunctionB = "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU5";
if (Ledger.balance(Ledger.selfAddress) >= ONE_TEZ) {
  const send_request = new Request(`jstz://${smartFunctionB}`, {
    headers: {
      "X-JSTZ-TRANSFER": ONE_TEZ,
    },
  });
  await fetch(send_request);
}
```

To send tez to a smart function without calling the smart function and running its handler function, send the tez in a request to `jstz://<ADDRESS>/-/noop`, as in this example:

```typescript
const ONE_TEZ = 1000000;
const call_request = new Request(`jstz://${smart_function}/-/noop`, {
  headers: {
    "X-JSTZ-TRANSFER": ONE_TEZ.toString(),
  },
});
```

As described in [Errors](/functions/errors), any transfers are reverted if a smart function throws an uncaught error.

## Transferring tez in a response

Similarly, a smart function can transfer tez to the user or smart function that called it by setting the `X-JSTZ-TRANSFER` header in the response.
For example, this smart function sends one tez to the caller:

```typescript
const ONE_TEZ = 1000000; // 1 XTZ in mutez

const handler = async (request: Request): Promise<Response> => {
  if (Ledger.balance(Ledger.selfAddress) >= ONE_TEZ) {
    return new Response("Have one tez from me!", {
      headers: {
        "Content-Type": "text/utf-8",
        "X-JSTZ-TRANSFER": ONE_TEZ.toString(),
      },
    });
  } else {
    return new Response("I have no tez to send you");
  }
};

export default handler;
```

## Receiving tez

Smart functions automatically accept tez sent to them.
You can see how many mutez are in a request by checking the `X-JSTZ-AMOUNT` header, which includes the amount as a string:

```typescript
const ONE_TEZ = 1000000; // 1 XTZ in mutez

const handler = async (request: Request): Promise<Response> => {
  const transferred_amount_string = request.headers.get("X-JSTZ-AMOUNT");
  const transferred_amount = parseInt(transferred_amount_string || "0");
  console.log(`Received ${transferred_amount} mutez.`);

  return new Response("Thank you!");
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
  return new Response("OK");
};

export default handler;
```
