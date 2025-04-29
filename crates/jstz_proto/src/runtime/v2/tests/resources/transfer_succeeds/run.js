/** @param {Request} req */
export default async (req) => {
  let address = new URL(req.url).pathname.substring(1);
  let newRequest = new Request(`jstz://${address}`, {
    headers: {
      "X-JSTZ-TRANSFER": 3000000, // 3 XTZ
    },
  });
  let response = await fetch(newRequest);
  let amount = response.headers.get("X-JSTZ-AMOUNT");
  if (amount !== "1000000") {
    throw new Error(`Expected amount to be 1000000 in run. Found ${amount}`);
  }
  return new Response();
};
