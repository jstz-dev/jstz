export default async () => {
  try {
    const code = ```
export default (request) => {
  const arg = request.text();
  console.log(arg);
  return new Response();
}
```;
    console.log(code);
    const subcontractAddress = await Contract.create(code);
    console.log("created", contract);
    let response = await Contract.call(contract, "Hello World");

    await Contract.call(
      new Request(`tezos://${subcontractAddress}/`, {
        method: "POST",
        body: "Hello World",
      }),
    );

    return new Response();
  } catch (error) {
    console.error(error);
  }
};
