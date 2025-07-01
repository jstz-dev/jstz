const REFUND_ADDRESS = "KT1Ucc8SZpnuQW6R7mpjJqX3fKozbjgYzrgj"; // to be filled in

const handler = async (request) => {
  const transferred_amount = request.headers.get("X-JSTZ-AMOUNT");
  console.log("transferred_amount", transferred_amount);

  // Call the refund smart function with 2 tez.
  const call_request = new Request(`jstz://${REFUND_ADDRESS}`, {
    headers: {
      "X-JSTZ-TRANSFER": "2000000",
    },
  });
  const response = await fetch(call_request);
  if (!response.ok) {
    console.error("failed to call the refund smart function");
    return Response.error("failed to call the refund smart function");
  }
  // Extract the refunded amount: 1 tez is refunded
  const refund_amount = response.headers.get("X-JSTZ-AMOUNT");
  console.log("refund_amount", refund_amount);
  // Transfer the refunded amount to the caller
  return new Response(null, {
    headers: {
      "X-JSTZ-TRANSFER": refund_amount,
    },
  });
};

export default handler;
