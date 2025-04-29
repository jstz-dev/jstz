/** @param {Request} req */
export default async (req) => {
  let data = Number.parseInt(new URL(req.url).pathname.substring(1));
  return new Response(
    JSON.stringify({
      data,
    }),
  );
};
