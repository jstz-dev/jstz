/** @param {Request} req */
export default async (req) => {
  let address = new URL(req.url).pathname.substring(1);
  let newRequest = new Request(`jstz://${address}`, {
    headers: {
      "X-JSTZ-AMOUNT": 3000000, // 3 XTZ
      REFERER: "tz1eLbDXYceRsPZoPmaJXZgQ6pzgnTQvZtpo",
      "X-JSTZ-TRANSFER": 1000000,
      "X-JSTZ-NON-EXISTENT": "test",
    },
  });
  let response = await fetch(newRequest);
  if (!response.ok) {
    return response;
  }

  let headerKeys = Array.from(response.headers.keys());
  if (headerKeys.length !== 0) {
    throw new Error(`Found headers ${headerKeys}`);
  }
  return new Response();
};
