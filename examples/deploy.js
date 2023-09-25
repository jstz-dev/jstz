export default async () => {
  try {
    let code = "export default (arg) => console.log(arg)";
    console.log(code);
    let contract = Ledger.createContract(code);
    console.log("created", contract);
    let response = await Contract.call(contract, 'Hello World');

  } catch (error) { console.error(error) }
}
