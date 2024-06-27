const code = `
  export default (req) => {
    const url = new URL(req.url);
    const path = url.pathname;
    
    switch (path) {
      case '/':
        Kv.set('key', 'Hello World');
        break;
      case '/delete':
        Kv.delete('key');
        throw 'Ha ha ha I deleted the key and threw an error';
      case '/log':
        console.log(Kv.get('key'));
        break;
    }

    return new Response();
  }
`;

const handler = async () => {
  console.log("Hello from JS 👋");

  const addr = await SmartFunction.create(code);

  await fetch(new Request(`tezos://${addr}/`));
  try {
    await fetch(new Request(`tezos://${addr}/delete`));
  } catch (error) {
    console.error("Caught: ", error);
  }

  await fetch(new Request(`tezos://${addr}/log`));

  return new Response();
};

export default handler;
