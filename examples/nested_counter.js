const KEY = "counter";

const handler = async (request) => {
  const url = new URL(request.url);
  const incr = url.searchParams.get("incr");

  let counter = Kv.get(KEY);
  console.log(`Counter: ${counter}`);

  counter = counter === null ? 0 : counter + 1;
  Kv.set(KEY, counter);

  if (incr !== null) {
    console.log(`Nested transaction: ${incr}`);
    // Nested transaction
    await fetch(new Request(`tezos://${incr}/`));
  }

  return new Response();
};

export default handler;
