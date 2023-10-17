const contractCode = `
async function handler (request) {
  try {
    const url = new URL(request.url);
    switch(url.pathname) {
      case "/set": {
        simpleKv.set("key", url.searchParams.get("msg").toString());
        break;
      }
      case "/log": {
        simpleConsole.logWithAddress(simpleKv.get("key"));
        break;
      }

      default:
        console.error("unrecognised path", request.url);
    }
  } catch (error) {
    console.error(error);
  }
  return new Response();
}

export default handler;
`;

async function handler(request) {
  try {
    const url = new URL(request.url);
    switch (url.pathname) {
      case "/create": {
        console.log(`Hello from ${Ledger.selfAddress}`);
        let contract = await Contract.create(contractCode);
        simpleKv.set("address", contract);
        break;
      }
      case "/call": {
        console.log(`Hello from ${Ledger.selfAddress}`);
        let address = simpleKv.get("address") || Ledger.selfAddress;
        console.log("calling set", address);
        await Contract.call(new Request(`tezos://${address}/set?msg=hello`));
        console.log("calling log");
        await Contract.call(new Request(`tezos://${address}/log`));
        console.log("done!");
        break;
      }
      case "/set": {
        simpleKv.set("key", url.searchParams.get("msg").toString());
        break;
      }
      case "/log": {
        simpleConsole.logWithAddress(simpleKv.get("key"));
        break;
      }

      default:
        let address = await Contract.create(contractCode);
        await Contract.call(new Request(`tezos://${address}/set?msg=hello`));
        await Contract.call(new Request(`tezos://${address}/log`));
        console.error("unrecognised path", request.url);
    }
    return new Response();
  } catch (error) {
    console.error(error);
  }
  /*
  contract.call(`tezos://${contract}/set?msg=hello`);
  contract.call(`tezos://${contract}/log`);

*/
  return new Response();
}

export default handler;
