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
    const subcontractAddress = Ledger.createContract(code);
    console.log("created", contract);

    await Contract.call(new Request(`tezos://${subcontractAddress}/`, { 
      method: "POST", 
      body: "Hello World" 
    }));

    return new Response();
  } catch (error) { console.error(error) }
}
