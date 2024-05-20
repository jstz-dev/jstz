export default async function (request) {
  const url = new URL(request.url);
  const { amount, address } = await request.json();

  let total = Kv.get("tax/" + address) || 0;
  total += parseInt(amount) * 0.2;
  Kv.set("tax/" + address, total);

  console.log(`${address} needs to pay ${total} mutez in taxes`);
  return new Response(`Success! ${address} needs to pay ${total}`);
}
