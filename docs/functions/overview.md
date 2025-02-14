# Smart functions

Smart functions are the main processing unit of Jstz.
They behave like [serverless applications](https://en.wikipedia.org/wiki/Serverless_computing), small applications that run only when called and do not have a long-term presence on any server.
Clients call them, servers load them into memory to run them, they return a value, and they are removed from the server's memory until they are called again.

As described in [Accepting requests](/functions/requests), each smart function must have a `handler` function that receives these requests from callers.
This function receives a Jstz [Request](/api/request) object that includes the message sent from the client and metadata such as the address of the account that called the smart function.
The function runs and returns a Jstz[Response](/api/response) object to the caller.

Smart functions store persistent data within Jstz; see [Storing data](/functions/data_storage).

## Example smart function

This smart function stores a number and allows users to add or subtract a number from that number.
They can also get the current value of the number:

```typescript
// Get the current number from storage
const get = (): number => {
  const num: number | null = Kv.get("myNumber");
  return num || 0;
};

// Set the number in storage
const set = (num: number) => {
  Kv.set("myNumber", num);
};

// Pass the message `get` to get the current value in storage
// Pass `increment` to add one
// Pass `decrement` to subtract one
// Any other message returns a message
const handler = async (request: Request): Promise<Response> => {
  // Extract the requester's address and message from the request
  const requester = request.headers.get("Referer") as Address;
  const { message } = await request.json();

  console.log(`${requester} says: ${message}`);

  const currentValue = get();

  let responseString = "";

  if (message === "increment") {
    set(currentValue + 1);
    responseString = "Incremented. ";
  } else if (message === "decrement") {
    set(currentValue - 1);
    responseString = "Decremented. ";
  } else if (message !== "get") {
    return new Response(
      JSON.stringify("Pass 'get', 'increment', or 'decrement'."),
    );
  }

  responseString += "Current value is " + get();
  return new Response(JSON.stringify(responseString));
};

export default handler;
```

## Accounts

Smart functions are a kind of cryptocurrency account.
Like user accounts, they can receive, store, and send tez (XTZ).

## Error handling

Calls to smart functions are atomic, which means that all of the request completes or none of it does.
If a smart function throws an exception and does not catch it, all effects of the request are reverted, so it is as if the call to the smart function never happened.
For example, if a smart function writes values to storage early in processing a request and generates an uncaught exception later while processing the same request, the changes to storage are reverted.
Similarly, any calls to other smart contracts are reverted.

Smart functions can catch and handle errors with try/catch blocks just like in ordinary JavaScript/TypeScript.

## Differences from other JavaScript/TypeScript applications

Smart functions look like ordinary JavaScript functions, but because they run on Jstz, they have some differences in their behavior.

- Smart functions cannot be changed or stopped after they are deployed.
- Smart functions are permissionless, so anyone can call them, but you can add your own logic to them to restrict who can call them.
- Anyone can inspect the code and storage of deployed smart functions.
- Because smart functions run in a decentralized manner on many Jstz Smart Rollup nodes, they are censorship-resistant.

## Limitations of smart functions

Smart functions behave much like other serverless JavaScript/TypeScript applications, but they have these limitations:

- Smart functions cannot call external APIs.
- Smart functions are currently restricted to 3915 bytes.
- Smart functions can import and use packages, but they can use only certain JavaScript APIs, which limits the packages that they can use.
<!-- https://huancheng-trili.github.io/jstz-api-coverage/ -->
