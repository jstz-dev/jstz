const SELF = "tz492MCfwp9V961DhNGmKzD642uhU8j6H5nB";
const OTHER = "tz4FENGt5zkiGaHPm1ya4MgLomgkL1k7Dy7q";

const logBalance = (address) => {
  console.log(`Balance of "${address}": ${Ledger.balance(address)}`);
};

const doTransfer = (n) => {
  console.log(`Transferring ${n} XTZ from ${SELF} to ${OTHER}...`);
  Ledger.transfer("tz4FENGt5zkiGaHPm1ya4MgLomgkL1k7Dy7q", n);
};

const doDemo = () => {
  logBalance(SELF);
  logBalance(OTHER);
  doTransfer(10);
  logBalance(SELF);
  logBalance(OTHER);
  return new Response();
};

console.log("Hello JS 👋");

export default doDemo;
