async function deploySmartFunction(code) {
  console.log(code);
  const smartFunctionAddress = await SmartFunction.create(code);
  console.log("created", smartFunctionAddress);
  return smartFunctionAddress;
}
async function deployTwice() {}
async function handler() {
  const code = `
export default (request) => {
  console.log("Hello World");
  const arg = request.text();
  console.log(arg);
  return new Response();
}
  `;
  const rustCode = `
pub fn default (request: Request) -> Response {
  console.log("Hello World");
  let arg = request.text();
  console.log(arg);
  Response::new();
}
  `;

  let smartFunctionAddress;
  try {
    console.log("Trying to deploy smart contract in rust.");
    smartFunctionAddress = await deploySmartFunction(rustCode);
  } catch (err) {
    console.error(err);
    console.log("Trying again with javascript.");
    smartFunctionAddress = await deploySmartFunction(code);
  }

  await fetch(
    new Request(`tezos://${smartFunctionAddress}/`, {
      method: "POST",
      body: "Hello World",
    }),
  );
  return new Response();
}

export default handler;
