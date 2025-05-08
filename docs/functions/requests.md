---
title: Handling requests
---

Jstz smart functions accept requests from smart functions and clients via their `handler` function, which they must export as their default function.
Smart functions can have only this single entrypoint.
This function receives a Jstz [`Request`](/api/request) object and must return a promise that resolves to a Jstz [`Response`](/api/response) object.

For an example of a `handler` function, see [Smart functions](/functions/overview).

## Requests

The [`Request`](/api/request) object is the only information that the smart function receives from the caller.
It can branch and respond to this data in any way.
The `Request` object includes this data:

- The data payload of the request as a JSON object, in the `request.json()` promise
- The address of the account that sent the request (which can be a `tz1` user account of a `KT1` smart function), in the `Referer` header
- The URL called, in the `request.url` property
- The HTTP method of the request, in the `request.method` property
- The query parameters from the URL, in the `url.searchParams` object

For example, this code prints the information from the `Request` object:

```typescript
const handler = async (request: Request): Promise<Response> => {
  const requestBody = await request.json();
  console.log(JSON.stringify(requestBody, null, 2));

  console.log("Caller:", request.headers.get("Referer") as Address);

  const url = new URL(request.url);
  console.log("Full URL:", url.toString());
  console.log("URL path:", url.pathname);
  console.log("Method:", JSON.stringify(request.method));
  url.searchParams.forEach((value, key) =>
    console.log(`Query param: ${value}: ${key}`),
  );
  // ...
};
```

You can branch the code of the smart function based on any of this information.
For example, you can parse the URL that the request went to and allow callers to call different URLs as though they were API endpoints.
For example, this code parses the URL and does different things based on whether the request came to the URL `jstz://<ADDRESS>/ping` or `jstz://<ADDRESS>/marco`:

```typescript
const handler = async (request: Request): Promise<Response> => {
  const url = new URL(request.url);
  const path = url.pathname.toLowerCase();
  console.log(path);

  switch (path) {
    case "/ping":
      return new Response("Pong");
      break;

    case "/marco":
      return new Response("Polo");
      break;

    default:
      return new Response("Default", {
        headers: {
          "Content-Type": "text/utf-8",
        },
      });
      break;
  }
};

export default handler;
```

For more information, see the reference for the Jstz [`Request`](/api/request) object.

## Responses

The `handler` function must return a promise that resolves to a Jstz [`Response`](/api/response) object.

You can create the response object by passing a value and content type to its constructor, as in this example:

```typescript
function handler(): Response {
  return new Response("Hello world! 👋", {
    headers: {
      "Content-Type": "text/utf-8",
    },
  });
}
```

If you want to return a JSON object, you can use the `Response.json()` static method, as in this example:

```typescript
function handler(): Response {
  return Response.json({ message: "Hello world! 👋" });
}
```

Similarly, to return a error, use the `Response.error()` static method, which takes no parameters:

```typescript
function handler(): Response {
  return Response.error();
}
```

As described in [Calling other smart functions](/functions/calling), returning an error reverts all calls in the chain and any changes to smart function storage that the calls caused.
