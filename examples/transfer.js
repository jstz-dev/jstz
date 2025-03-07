async function deployRefundSmartFunction() {
  const code = `
  export default (request) => {
    const transferred_amount = request.headers.get("X-JSTZ-AMOUNT");
    console.log("transferred_amount", transferred_amount);
    if (transferred_amount !== "2000000") {
      return new Response();
    }
    return new Response(null, {
      headers: {
        "X-JSTZ-TRANSFER": "1000000",
      },
    });
  }`;
  const smartFunctionAddress = await SmartFunction.create(code);
  console.log("refund smart function created", smartFunctionAddress);
  return smartFunctionAddress;
}

/// 1. create `refund_sf` and call it with 2 tez.
/// 2. `refund_sf` refunds 1 tez to `this` smart function.
/// 3. `this` smart function transfers 1 tez back to the caller.
const handler = async (request) => {
  const transferred_amount = request.headers.get("X-JSTZ-AMOUNT");
  console.log("transferred_amount", transferred_amount);
  // 1. deploy the smart function that refunds 1 tez
  const refund_sf = await deployRefundSmartFunction();
  // 2. call the refund smart function with 2 tez.
  const call_request = new Request(`tezos://${refund_sf}`, {
    headers: {
      "X-JSTZ-TRANSFER": "2000000",
    },
  });
  const response = await fetch(call_request);
  if (!response.ok) {
    console.error("failed to call the refund smart function");
    return Response.error("failed to call the refund smart function");
  }
  // 3. extract the refunded amount - 1 tez is refunded
  const refund_amount = response.headers.get("X-JSTZ-AMOUNT");
  console.log("refund_amount", refund_amount);
  // 4. transfer the refunded amount to the caller
  return new Response(null, {
    headers: {
      "X-JSTZ-TRANSFER": refund_amount,
    },
  });
};

export default handler;
