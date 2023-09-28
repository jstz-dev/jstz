const ADDR_1 = "tz492MCfwp9V961DhNGmKzD642uhU8j6H5nB";
const ADDR_2 = "tz4FENGt5zkiGaHPm1ya4MgLomgkL1k7Dy7q";
const handler = async () => {
  console.log("Hello");
  const otherAddress = Ledger.selfAddress() == ADDR_1 ? ADDR_2 : ADDR_1;

  await Contract.call(
    otherAddress,
    "export default () => Kv.set('key', 'Hello World')",
  );
  try {
    await Contract.call(
      otherAddress,
      "export default () => { Kv.delete('key') ; throw 'Ha ha ha I deleted your key and threw an error' }",
    );
  } catch (error) {
    console.error("Caught: ", error);
  }
  await Contract.call(
    otherAddress,
    "export default () => console.log(Kv.get('key'))",
  );
};

export default handler;
