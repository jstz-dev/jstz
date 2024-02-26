export default async function (request) {
  // Extract the requestor's address and message from the request
  const requester_address = request.headers.get("Referer");
  const { message } = await request.json();

  console.log(`${requester_address} says: ${message}`);

  // Check if the requestor is polite, and decline the request if not
  if (!message.toLowerCase().includes("please")) {
    return new Response("Sorry, I only fulfill polite requests");
  }

  // Check how much tez the requestor already received in the Kv store
  let receivedTez = Kv.get("received/" + requester_address);
  receivedTez = receivedTez === null ? 0 : receivedTez;
  console.debug(`Requestor already received ${receivedTez} tez`);

  // If the requestor already received too much tez, decline the request
  if (receivedTez >= 10000) {
    return new Response("Sorry, you already received too much tez");
  }

  // Process the request and send the tez to the requestor if you have enough tez
  if (Ledger.balance(Ledger.selfAddress) > 1000) {
    try {
      console.log(
        `Transferring 1000 XTZ from ${Ledger.selfAddress} to ${requestor}...`,
      );
      Ledger.transfer(requestor, 1000);
    } catch (e) {
      console.error(e);
    }
  } else {
    return new Response(
      "Sorry, I don't have enough tez to fulfill your request",
    );
  }

  // Update the amount of tez the requestor received in the Kv store
  Kv.set("received/" + requester_address, received + 1000);

  // Pay taxes on the gift by calling a nested smart function.
  // await fetch(
  //  new Request(`tezos://pay_tax/`, { // Luckily, pay_tax sf doesn't exist yet
  //    method: `POST`,
  //    body: `{ "amount": 1000, "address": "${requester_address}"}`,
  //  }),
  // ));

  // Inform the requestor about the successful transfer
  return new Response(
    "Thank you for your polite request. You received 1000 tez!",
  );
}
