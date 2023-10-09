const contractCode = `
async function handler (request) {
  try {
    const url = new URL(request.url);
    switch(url.pathname) {
      case "/set": {
        simpleKv.set("key", url.searchParams.get("msg").toString());
        return new Response();
      }
      case "/log": {
        simpleConsole.logWithAddress(simpleKv.get("key"));
        return new Response();
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
`

async function handler () {
  try {
    const contract = await Ledger.createContract(contractCode);
    console.log(contract);
    await Contract.call(new Request(`tezos://${contract}`));

  }
  catch (error) {
    console.error(error);
  }
  /*
  contract.call(`tezos://${contract}/set?msg=hello`);
  contract.call(`tezos://${contract}/log`);

*/
  return new Response();
}

export default handler;
