# Client SDK

The [`jstz-client SDK`](https://www.npmjs.com/package/@jstz-dev/jstz-client) allows you to call smart functions from JavaScript/TypeScript applications.

Below is an example TypeScript snippet that a DApp could use to interact with the [`counter`](https://github.com/jstz-dev/jstz/tree/main/examples/counter) smart function via the client SDK. For a more concrete and practical example with front-end and wallet integration, refer to the [Example web applications](../examples.md#example-web-applications) section.

```typescript
import Jstz from "@jstz-dev/jstz-client";
import * as signer from "@jstz-dev/jstz_sdk";

const smartFunctionAddress = "KT1...";
const myAddress = "tz1...";
const myPublicKey = "";
const mySecretKey = "";
const jstzRpcEndpoint = "https://...";
const jstzClient = new Jstz.Jstz({
  baseURL: jstzRpcEndpoint,
});

async function signAndSend() {
  // sign the run function operation to increment
  const nonce = await jstzClient.accounts.getNonce(myAddress);
  const content: Jstz.Operation.RunFunction = {
    _type: "RunFunction",
    body: null,
    gasLimit: 5000,
    headers: {},
    method: "GET",
    uri: `jstz://${smartFunctionAddress}/increment`,
  };
  const operation: Jstz.Operation = {
    content,
    nonce,
    publicKey: myPublicKey,
  };
  const signature = signer.sign_operation(operation, mySecretKey);

  // Submit the operation and wait
  const {
    result: { inner },
  } = await jstzClient.operations.injectAndPoll({
    inner: operation,
    signature,
  });
  let returnedMessage = "No message";
  if (typeof inner === "object" && "body" in inner) {
    returnedMessage = inner.body && JSON.parse(atob(inner.body));
  }
  if (typeof inner === "string") {
    returnedMessage = inner;
  }
  return returnedMessage;
  // counter is incremented! can use /get method in a similar way to verify it
}
```
