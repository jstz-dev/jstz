async function deployRefundSmartFunction() {
  const code = `
  export default (request) => {
     return new Response(null, {
        headers: {
           "X-JSTZ-TRANSFER": "1000000"
        },
     });
  }`;
  const smartFunctionAddress = await SmartFunction.create(code);
  console.log("refund smart function created", smartFunctionAddress);
  return smartFunctionAddress;
}

const handler = async (request) => {
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
  const amount = response.headers.get("X-JSTZ-TRANSFERRED");
  console.log("refunded amount", amount);
  // 4. transfer the refunded amount to the caller
  return new Response(null, {
    headers: {
      "X-JSTZ-TRANSFER": amount,
    },
  });
};

export default handler;
