export default () => {
  try {
  let code = "export default () => console.log('hello world')";
  console.log(code);
  let contract = Ledger.createContract(code);
  console.log("created", contract)
  } catch (error) { console.error(error) }
}
