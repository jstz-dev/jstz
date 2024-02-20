async function handler() {
  const code = `
export default (request) => {
  console.log("Hello World");
  const arg = request.text();
  console.log(arg);
  return new Response();
}
  `;

  console.log(code);
  const smartFunctionAddress = await SmartFunction.create(code);
  console.log("created", smartFunctionAddress);

  await fetch(
    new Request(`tezos://${smartFunctionAddress}/`, {
      method: "POST",
      body: "Hello World",
    }),
  );

  await fetch(
    new Request(`tezos://${smartFunctionAddress}/`, {
      method: "POST",
      body: "Hello World",
    }),
  );

  return new Response();
}

export default handler;
