const handler = async (request) => {
  let data = await request.text();
  return new Response(data);
};

export default handler;
