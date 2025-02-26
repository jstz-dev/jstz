const handler = async (request) => {
  const sfC = new Request("tezos://KT1...", {
    headers: {
      "X-JSTZ-TRANSFER": 1,
    },
  });
  const response = await fetch(sfC);
  /// indicates the this function has been transferred
  const amount = response.headers.get("X-JSTZ-TRANSFERRED");

  return new Response(null, {
    headers: { "X-JSTZ-TRANSFER": amount },
  });
};

export default handler;
