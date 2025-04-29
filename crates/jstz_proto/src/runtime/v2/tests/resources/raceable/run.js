/** @param {Request} req */
export default async (req) => {
  let address = new URL(req.url).pathname.substring(1);
  let promise1 = fetch(`jstz://${address}/1`);
  let promise2 = fetch(`jstz://${address}/2`);
  // Let's await in the opposite order of call
  let response2 = await promise2;
  let response1 = await promise1;
  return new Response(
    JSON.stringify({
      data: (await response1.json()).data + (await response2.json()).data,
    }),
  );
};
