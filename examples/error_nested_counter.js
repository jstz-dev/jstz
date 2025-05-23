const KEY = "counter";

const handler = async (request) => {
  const url = new URL(request.url);
  const n = url.searchParams.get("n") || 0;

  let counter = Kv.get(KEY);
  console.log(`Counter: ${counter}`);

  counter = counter === null ? 0 : counter + 1;

  console.log(`Setting counter to: ${counter}`);
  Kv.set(KEY, counter);
  console.log(`Stored counter: ${Kv.get(KEY)}`);

  if (n > 0) {
    console.log(`Nested transaction: ${n}`);
    // Nested transaction
    await fetch(new Request(`jstz://${Ledger.selfAddress}/?n=${n - 1}`));
  }

  // Throw an error at the most nested level
  if (n == 0) {
    throw new Error("Something is wrong");
  } else {
    return new Response();
  }
};

export default handler;
