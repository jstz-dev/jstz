---
title: 🆗 Response
sidebar_label: Response
---

`jstz`'s [`Response`](https://developer.mozilla.org/en-US/docs/Web/API/Response) implementation is based on the [Fetch API](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API) specification.
The `Response` interface of the Fetch API represents the response to a request.

:::danger
⚠️ `jstz`'s implementation is not spec compliant ⚠️
:::

## Quick Start

We can create a `Response` instance from a simple object of response properties:

```typescript
function handler(): Response {
  return new Response("Hello world! 👋", {
    headers: {
      "Content-Type": "text/utf-8",
    },
  });
}
```

Alternatively, we can create a `Response` instance using one of the static methods:

```typescript
function handler(): Response {
  return Response.json({ message: "Hello world! 👋" });
}
```

## Constructor

### `new Response(body?: BodyInit | null, init?: ResponseInit): Response`

Creates a new `Response` object.

:::danger
**Spec deviation**: `Blob`, `FormData`, `ReadableStream` and `URLSearchParams` are not supported for `BodyInit`.
:::

```typescript
type BodyInit = string | BufferSource;

interface ResponseInit {
  status?: number;
  headers?: HeadersInit;
}
```

## Instance Properties

### `readonly Response.bodyUsed: boolean`

A boolean property for whether this `Response` has already been used or not.

### `readonly Response.headers: Headers`

A `Headers` object.

### `readonly Response.ok: boolean`

A boolean property for whether the response was successful (status in the range 200–299) or not.

### `readonly Response.status: number`

A number property for the HTTP status code of the response.

### `readonly Response.statusText: string`

A string property for the status message corresponding to the status code.

### `Response.url: string`

A string property for the URL of the response.

## Instance Methods

### `Response.arrayBuffer(): Promise<ArrayBuffer>`

Returns a promise that resolves with an `ArrayBuffer`.

### `Response.json(): Promise<any>`

Returns a promise that resolves with the result of parsing the body text as JSON.

### `Response.text(): Promise<string>`

Returns a promise that resolves with a UTF-16 `string`.

## Static Methods

### `Response.error(): Response`

Returns a `Response` object associated with a network error.

### `Response.json(value: unknown): Response`

:::danger
**Spec deviation**: `Response.json` doesn't permit an optional `init` parameter.
:::

Returns a `Response` object with a `JSON` body.
