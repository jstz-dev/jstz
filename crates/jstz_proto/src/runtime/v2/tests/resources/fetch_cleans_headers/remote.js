/** @param {Request} req */
export default async (req) => {
  if (req.headers.get("referrer") !== "KT1LC1JVTpMZZRXoaHBZNHvvAaFYiwCZi9Fu") {
    throw new Error("Unexpected referrer " + req.headers.get("referrer"));
  }
  if (req.headers.get("x-jstz-amount") !== "1000000") {
    throw new Error("Unexpected amount " + req.headers.get("x-jstz-amount"));
  }
  let headerKeys = Array.from(req.headers.keys());
  if (headerKeys.length != 4) {
    throw new Error("too few keys");
  }
  let keys = ["accept", "accept-language", "referrer", "x-jstz-amount"];
  for (let i in keys) {
    if (req.headers.get(keys[i]) === null) {
      throw new Error("missingkey! " + keys[i]);
    }
  }
  return new Response(null, {
    headers: {
      "X-JSTZ-AMOUNT": 5000000,
      "X-JSTZ-NON-EXISTENT": "test",
      REFERRER: "tz1eLbDXYceRsPZoPmaJXZgQ6pzgnTQvZtpo",
    },
  });
};
