let smartFunctionCode = `
const handler = async (request) => {
    try {
        const message = await request.text()
        console.log(message);
        console.log(\`My address is \${Ledger.selfAddress}\`)
        const response = new Response("Success!");
        return response;
    } catch (error) { console.error("child function error", error)
                      return Response.error(error)
                    }
}
export default handler`;

const handler = async () => {
  console.log("Hello JS 👋");
  console.log(`My address is ${Ledger.selfAddress}`);

  try {
    const newSmartFunction = await SmartFunction.create(smartFunctionCode);
    console.log("created new smart function with address", newSmartFunction);
    const url = `jstz://${newSmartFunction}/myEndPoint`;
    const request = new Request(url, {
      method: "POST",
      body: "Hello from child smart function 👋",
    });

    const response = await fetch(request);
    console.log(await response.text());
  } catch (error) {
    console.error(error);
    return Response.error("😿");
  }

  console.log("The root smart function has control again!");
  console.log(`And to confirm, my address is ${Ledger.selfAddress}`);
  const response = new Response("😸");
  return response;
};

export default handler;
