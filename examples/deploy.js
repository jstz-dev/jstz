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
  const subcontractAddress = await Contract.create(code);
  console.log("created", subcontractAddress);

  await Contract.call(
    new Request(`tezos://${subcontractAddress}/`, {
      method: "POST",
      body: "Hello World",
    }),
  );

  return new Response();
}

export default handler;
