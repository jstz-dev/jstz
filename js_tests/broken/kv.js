const subcontract = (name) => `
export default () => {
  Kv.set("Name", "${name}");
  const nonce = Kv.get("Nonce") || 0;
  Kv.set("Nonce", nonce + 1)
  return new Response()
}
`;

const handler = async (request) => {
  try {
    const url = new URL(request.url);
    const path = url.pathname;
    const name = url.searchParams.get("name");
    if (!name) {
      throw "No name supplied";
    }
    switch (path) {
      case "/create":
        const createdAddress = await SmartFunction.create(subcontract(name));
        console.log(`Created account ${name} at ${createdAddress}`);
        Kv.set(name, createdAddress);

        break;
      case "/increment":
        const functionAddress = Kv.get(name);
        if (!functionAddress) {
          throw `${name} does not exist`;
        }
        await fetch(new Request(`jstz://${functionAddress}`));
        console.log(`Incremened account ${name} at ${functionAddress}`);
        break;
      default:
        throw "unrecognised path";
    }
  } catch (error) {
    console.error(error);
  }
  return new Response();
};
export default handler;
