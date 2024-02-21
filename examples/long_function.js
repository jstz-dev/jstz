async function handler() {
  const code = `
export default (request) => {
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("I wish I'd learned about for loops");
  console.log("Finished!");
  return new Response()
}
  `;

  const smartFunctionAddress = await SmartFunction.create(code);
  console.log("created", smartFunctionAddress);

  await fetch(new Request(`tezos://${smartFunctionAddress}/`));

  return new Response();
}

export default handler;
