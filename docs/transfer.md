# 💰 Transfer

This guide explains how to transfer XTZ between accounts using request and response headers. Transfers can be made to both smart functions and user addresses.

## 1. Transfer via Request Headers

Smart functions can transfer XTZ using the `X-JSTZ-TRANSFER` header in requests. The recipient can access the transferred amount via the `X-JSTZ-AMOUNT` header.

**Smart function A (Sender)**

```typescript
const ONE_TEZ = "1000000"; // 1 XTZ in mutez

const handler = async (request) => {
  // Transfer 1 XTZ to smart function B
  const smartFunctionB = "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU5";
  const call_request = new Request(`jstz://${smartFunctionB}`, {
    headers: {
      "X-JSTZ-TRANSFER": ONE_TEZ,
    },
  });
  await fetch(call_request);

  return new Response();
};

export default handler;
```

**Smart function B (Recipient)**

```typescript
const handler = async (request) => {
  const transferred_amount = request.headers.get("X-JSTZ-AMOUNT");
  console.log(`Received ${transferred_amount} mutez`); // Output: Received 1000000 mutez
  return new Response();
};

export default handler;
```

## 2. Refund via Response Headers

Smart functions can refund XTZ using the `X-JSTZ-TRANSFER` header in responses. The original sender can access the refunded amount via the `X-JSTZ-AMOUNT` header in the response.

**Smart function A (Sender)**

```typescript
const ONE_TEZ = "1000000"; // 1 XTZ in mutez

const handler = async (request) => {
  // Transfer 1 XTZ to smart function B
  const smartFunctionB = "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU5";
  const call_request = new Request(`jstz://${smartFunctionB}`, {
    headers: {
      "X-JSTZ-TRANSFER": ONE_TEZ,
    },
  });

  const response = await fetch(call_request);
  const refund_amount = response.headers.get("X-JSTZ-AMOUNT");
  console.log(`Received ${refund_amount} mutez back`); // Output: Received 500000 mutez back
  return new Response();
};

export default handler;
```

**Smart function B (Refunder)**

```typescript
const HALF_TEZ = "500000"; // 0.5 XTZ in mutez

const handler = async (request) => {
  const transferred_amount = request.headers.get("X-JSTZ-AMOUNT");
  console.log(`Received ${transferred_amount} mutez`); // Output: Received 1000000 mutez

  // Refund 0.5 XTZ to the sender
  return new Response(null, {
    headers: {
      "X-JSTZ-TRANSFER": HALF_TEZ,
    },
  });
};

export default handler;
```

## Important Notes

1. All amounts are specified in mutez (1 XTZ = 1,000,000 mutez)
2. Transfers can be made to:
   - Smart function addresses (KT1...)
   - User addresses (tz1...)
3. Ensure sufficient balance before initiating transfers
