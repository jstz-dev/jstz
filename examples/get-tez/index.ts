// 1 tez = 1 million mutez
const ONE_TEZ = 1000000;

// Maximum amount of tez a requester can receive
const MAX_TEZ = 10000;

// Get the amount of tez that the smart function has sent to an address
const getReceivedTez = (requester: Address): number => {
  let receivedTez: number | null = Kv.get(`received/${requester}`);
  receivedTez = receivedTez === null ? 0 : receivedTez;
  console.debug(`Requestor already received ${receivedTez} tez`);
  return receivedTez;
};

// Update the record of the amount of tez that an address has received
const setReceivedTez = (requester: Address, received: number): void => {
  Kv.set(`received/${requester}`, received);
};

// Log the message that the user sent
const addPoliteMessage = (requester: Address, message: string): void => {
  let length: number | null = Kv.get(`messages/${requester}/length`);
  if (length === null) {
    length = 0;
  }
  Kv.set(`messages/${requester}/${length}`, message);
  Kv.set(`messages/${requester}/length`, length + 1);
};

// Main function: handle calls to the smart function
const handler = async (request: Request): Promise<Response> => {
  // Extract the requester's address and message from the request
  const requester = request.headers.get("Referer") as Address;
  const { message } = await request.json();

  console.log(`${requester} says: ${message}`);

  // Check if the requester is polite, and decline the request if not
  if (!message.toLowerCase().includes("please")) {
    return new Response(
      JSON.stringify("Sorry, I only fulfill polite requests"),
    );
  }

  // If the requester already received too much tez, decline the request
  const receivedTez = getReceivedTez(requester);
  if (receivedTez >= MAX_TEZ) {
    return new Response(
      JSON.stringify("Sorry, you already received too much tez"),
    );
  }

  // Process the request and send the 1 tez = 1 million mutez to the requester if you can
  if (Ledger.balance(Ledger.selfAddress) > ONE_TEZ) {
    console.log(
      `Transferring 1 tez from ${Ledger.selfAddress} to ${requester}...`,
    );
    Ledger.transfer(requester, ONE_TEZ);
  } else {
    return new Response(
      JSON.stringify("Sorry, I don't have enough tez to fulfill your request"),
    );
  }

  // Log the updates
  setReceivedTez(requester, receivedTez + 1);
  addPoliteMessage(requester, message);

  return new Response(
    JSON.stringify("Thank you for your polite request. You received 1 tez!"),
  );
};

export default handler;
