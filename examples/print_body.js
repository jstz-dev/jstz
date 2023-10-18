async function handler(request) {
  let json = await request.json();
  console.log(`${JSON.stringify(json)}`);
  return new Response();
}
export default handler;
