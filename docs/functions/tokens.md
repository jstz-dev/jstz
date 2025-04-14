# Managing tokens

Jstz maintains a ledger of how many tez tokens (also known as XTZ) each user and smart function account owns, similar to but separate from the ledger that Tezos layer 1 uses.
You can use the [Asset bridge](/bridge) to move tez from layer 1 to Jstz and back.

::: tip

Internally, Jstz tracks tez not as individual tez but as _mutez_, which are equal to one-millionth of one tez.

:::

::: warning

The Ledger API used on this page is deprecated and will be removed in future versions of Jstz.

:::

## Sending tez

If a smart function has a balance of tez, it can send tez to a user account or smart function by passing the target address and the amount in mutez to the `Ledger.transfer` function, as in this example:

```typescript
// 1 tez = 1 million mutez
const ONE_TEZ = 1000000;

// Main function: handle calls to the smart function
const handler = (request: Request): Response => {
  // Extract the requester's address and message from the request
  const requester = request.headers.get("Referer") as Address;

  console.log(
    `Requester's account has ${Ledger.balance(requester) / ONE_TEZ} tez.`,
  );

  const myBalance = Ledger.balance(Ledger.selfAddress) / ONE_TEZ;
  console.log(`I have ${myBalance} tez.`);

  if (Ledger.balance(Ledger.selfAddress) > ONE_TEZ) {
    Ledger.transfer(requester, ONE_TEZ);
  }

  return new Response(JSON.stringify("OK"));
};

export default handler;
```

Smart functions automatically accept tez sent to them.

As described in [Errors](/functions/calling#errors), any transfers are reverted if a smart function throws an uncaught error.
