export default async function (request) {
  // Extract the requester's address and message from the request
  const requester_address = request.headers.get("Referer");
  const { message } = await request.json();

  console.log(`${requester_address} says: ${message}`);

  // Check if the requester is polite, and decline the request if not
  if (!message.toLowerCase().includes("please")) {
    return new Response("Sorry, I only fulfill polite requests");
  }

  // Check how much tez the requester already received in the Kv store
  let receivedTez = Kv.get("received/" + requester_address);
  receivedTez = receivedTez === null ? 0 : receivedTez;
  console.debug(`Requestor already received ${receivedTez} tez`);

  // If the requester already received too much tez, decline the request
  if (receivedTez >= 10000) {
    return new Response("Sorry, you already received too much tez");
  }

  // Process the request and send the 1 tez = 1 million mutez to the requester if you can
  if (Ledger.balance(Ledger.selfAddress) > 1000000) {
    console.log(
      `Transferring 1 tez from ${Ledger.selfAddress} to ${requester_address}...`,
    );
    Ledger.transfer(requester_address, 1000000);
  } else {
    return new Response(
      "Sorry, I don't have enough tez to fulfill your request",
    );
  }

  // Update the amount of tez the requester received in the Kv store
  Kv.set("received/" + requester_address, receivedTez + 1);

  // Pay taxes on the gift by calling a nested smart function.
  let response = await fetch(
    new Request(`tezos://tz1ZYCJg2mTNtfGZVEoGjEMVg16eVspJsZhi/`, {
      method: `POST`,
      body: `{ "amount": 1000000, "address": "${requester_address}"}`,
    }),
  );

  console.log(`Nested call response text: ${await response.text()}`);

  // Inform the requester about the successful transfer
  return new Response("Thank you for your polite request. You received 1 tez!");
}
