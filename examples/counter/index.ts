// Get the current number from storage
const get = (): number => {
  const num: number | null = Kv.get("myNumber");
  return num || 0;
};

// Set the number in storage
const set = (num: number) => {
  Kv.set("myNumber", num);
};

const handler = async (request: Request): Promise<Response> => {
  // Extract the requester's address and URL path from the request
  const requester = request.headers.get("Referer") as Address;
  const url = new URL(request.url);
  const path = url.pathname.toLowerCase();

  console.log(`${requester} calls ${path}`);

  let responseMessage = "";

  switch (path) {
    case "/increment":
      set(get() + 1);
      responseMessage = "Incremented. Current value is " + get();
      break;

    case "/decrement":
      set(get() - 1);
      responseMessage = "Decremented. Current value is " + get();
      break;

    case "/get":
      responseMessage = "Current value is " + get();
      break;

    default:
      responseMessage =
        "Call the URL path '/get', '/increment', or '/decrement'.";
      break;
  }

  return new Response(JSON.stringify(responseMessage));
};

export default handler;
