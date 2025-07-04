---
title: Calling external APIs
---

:::warning

This feature is under development and unstable.
Implementation details might change.

:::

As described in [Enshrined oracle](/architecture/oracle), smart functions cannot call external APIs directly and must instead use the oracle.
The oracle acts as a proxy gateway for network-accessible APIs.

Smart functions can make API requests through ordinary HTTP and HTTPS calls via the `fetch` API and Jstz automatically routes these requests through the oracle.

:::note

See the [limitations](/architecture/oracle#limitations) for the oracle before using it.

One important limitation is that smart functions can call the oracle only from a clean transaction context.
This means that smart functions cannot call the oracle after they have initiated any of these operations:

- Reading from or writing to the key-value store
- Sending tez
- Calling other smart functions

The smart function must make oracle calls before attempting any of these operations, or else the oracle rejects the `fetch` promise.
They also cannot initiate any of these operations while awaiting a response from the oracle.

:::

To call an external API via the oracle, make a `fetch` request as usual, as in [`oracle_basic`](https://github.com/jstz-dev/jstz/blob/main/examples/oracle_basic.js) example:

```javascript
const handler = async () => {
  console.log("Fetching uuid4");
  try {
    const responsePromise = fetch("http://httpbin.org/uuid");
    console.log("Running something else while waiting");
    const response = await responsePromise;
    if (!response.ok) {
      throw new Error(`HTTP error! Status: ${response.status}`);
    }
    const { uuid } = await response.json();
    console.log("UUID:", uuid);
    return new Response(uuid);
  } catch (error) {
    console.error("Failed to fetch UUID:", error.message);
  }
};

export default handler;
```

Jstz automatically sends this request to the oracle and returns a response within 20 seconds or rejects the promise that the `fetch` API returns.

For an example, see the [`oracle_basic`](https://github.com/jstz-dev/jstz/blob/main/examples/oracle_basic.js) example.
