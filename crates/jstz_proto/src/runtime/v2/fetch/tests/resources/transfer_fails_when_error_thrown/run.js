/** @param {Request} req */
export default async (req) => {
  let address = new URL(req.url).pathname.substring(1);
  let newRequest = new Request(`jstz://${address}`, {
    headers: {
      "X-JSTZ-TRANSFER": 3000000, // 3 XTZ
    },
  });
  let response = await fetch(newRequest);
  if (response.ok) {
    throw new Error("Expected fetch to fail");
  }
  let amount = response.headers.get("X-JSTZ-AMOUNT");
  if (amount !== null) {
    throw new Error(`Expected amount to be null in run. Found ${amount}`);
  }
  return new Response();
};
