// 1 tez = 1 million mutez
const ONE_TEZ = 1000000;

// Maximum amount of tez a requester can receive
const MAX_TEZ = 10000;

const getRecievedTez = (requester: Address): number => {
  let receivedTez: number | null = Kv.get(`received/${requester}`);
  receivedTez = receivedTez === null ? 0 : receivedTez;
  console.debug(`Requestor already received ${receivedTez} tez`);
  return receivedTez;
};

const setRecievedTez = (requester: Address, received: number): void => {
  Kv.set(`received/${requester}`, received + 1);
};

const handler = async (request: Request): Promise<Response> => {
  // Extract the requester's address and message from the request
  const requester = request.headers.get("Referer") as Address;
  const { message } = await request.json();

  console.log(`${requester} says: ${message}`);

  // Check if the requester is polite, and decline the request if not
  if (!message.toLowerCase().includes("please")) {
    return new Response("Sorry, I only fulfill polite requests");
  }

  // If the requester already received too much tez, decline the request
  const recievedTez = getRecievedTez(requester);
  if (recievedTez >= MAX_TEZ) {
    return new Response("Sorry, you already received too much tez");
  }

  // Process the request and send the 1 tez = 1 million mutez to the requester if you can
  if (Ledger.balance(Ledger.selfAddress) > ONE_TEZ) {
    console.log(
      `Transferring 1 tez from ${Ledger.selfAddress} to ${requester}...`,
    );
    Ledger.transfer(requester, ONE_TEZ);
  } else {
    return new Response(
      "Sorry, I don't have enough tez to fulfill your request",
    );
  }
  setRecievedTez(requester, recievedTez + 1);

  return new Response("Thank you for your polite request. You received 1 tez!");
};

export default handler;
