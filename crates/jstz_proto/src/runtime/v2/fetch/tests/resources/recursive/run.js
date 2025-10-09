/** @param {Request} req */
export default async (req) => {
  let count = Kv.get("count") ?? 0;
  let countLimit = req.headers.get("count-limit") ?? "3";
  if (count < Number(countLimit)) {
    Kv.set("count", count + 1);
    let response = await fetch(
      new Request(req.url, {
        headers: {
          "count-limit": countLimit,
        },
      }),
    );
    return response;
  } else {
    return new Response(
      JSON.stringify({
        count,
      }),
    );
  }
};
