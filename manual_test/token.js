// 1 million tokens
const MINT_AMOUNT = 1000000 * 1000000;

const handler = async (request) => {
  const url = new URL(request.url);
  const path = url.pathname;

  if (path === "/mint") {
    return tryMint(request);
  } else if (path === "/transfer") {
    return await tryTransfer(request);
  } else {
    return new Response({
      status: 404,
      body: "Path not found",
    });
  }
};

const tryMint = (request) => {
  if (Kv.contains("minted")) {
    return new Response({ status: 400, body: "Already minted" });
  }
  let account = request.headers.get("referer");
  Kv.set("minted", true);
  Kv.set(account, MINT_AMOUNT);
  return new Response(`Minted ${MINT_AMOUNT} mutez to ${account}`);
};

const tryTransfer = async (request) => {
  let sender = request.headers.get("referer");
  let { dest, amount } = await request.json();
  let senderBalance = Kv.get(sender);
  let destBalance = Kv.get(dest) || 0;
  if (senderBalance < amount) {
    return new Response({
      status: 400,
      body: "Insufficient funds",
    });
  }

  let senderNewBalance = senderBalance - amount;
  let destNewBalance = destBalance + amount;
  Kv.set(sender, senderNewBalance);
  Kv.set(dest, destNewBalance);

  let body = {};
  body[sender] = senderNewBalance;
  body[dest] = destNewBalance;
  return new Response(JSON.stringify(body));
};

export default handler;
