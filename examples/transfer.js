const handler = async (request) => {
  // const rq = new Request("tezos://tz1dbGzJfjYFSjX8umiRZ2fmsAQsk8XMH1E9", {
  //   headers: {
  //     "X-JSTZ-TRANSFER": 1,
  //   },
  // });
  // await fetch(rq);
  return new Response(null, {
    headers: { "X-JSTZ-TRANSFER": 1 },
  });
};

export default handler;
