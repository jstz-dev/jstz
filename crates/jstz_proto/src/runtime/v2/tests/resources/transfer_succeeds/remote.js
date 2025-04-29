/** @param {Request} req */
export default async (req) => {
  let requestHeaders = req.headers;
  if (requestHeaders.get("X-JSTZ-AMOUNT") !== "3000000") {
    throw new Error("Expected amount to be 3000000 in remote");
  }
  return new Response(null, {
    headers: {
      "X-JSTZ-TRANSFER": 1000000, // 1 XTZ
    },
  });
};
