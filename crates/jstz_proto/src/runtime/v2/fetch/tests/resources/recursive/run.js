/** @param {Request} req */
export default async (req) => {
  let count = Kv.get("count") ?? 0;
  if (count < 3) {
    Kv.set("count", count + 1);
    let response = await fetch(new Request(req.url));
    return response;
  } else {
    return new Response(
      JSON.stringify({
        count,
      }),
    );
  }
};
