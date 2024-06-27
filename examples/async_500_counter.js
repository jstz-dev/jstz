const KEY = "counter";

const handler = async () => {
  let counter = Kv.get(KEY);

  console.log(`Stored counter: ${counter}`);

  if (counter === null) {
    counter = 0;
  } else {
    counter++;
  }

  console.log(`Setting counter to: ${counter}`);
  Kv.set(KEY, counter);
  console.log(`Stored counter: ${Kv.get(KEY)}`);

  return Response.error();
};

export default handler;
