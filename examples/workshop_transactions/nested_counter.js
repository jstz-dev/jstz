const KEY = "counter";

const handler = async (request) => {
  const url = new URL(request.url);
  const n = url.searchParams.get("n") || 0;

  let counter = Kv.get(KEY);

  console.log(`New nested call. n = ${n}`);
  console.log(`   Counter: ${counter}`);

  counter = counter === null ? 1 : counter + 1;

  console.log(`   Setting counter to: ${counter}`);
  Kv.set(KEY, counter);

  if (n > 1) {
    // Nested transaction
    await fetch(new Request(`tezos://${Ledger.selfAddress}/?n=${n - 1}`));
  }

  return new Response();
};

export default handler;
