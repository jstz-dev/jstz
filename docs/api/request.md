---
title: 🙏 Request
sidebar_label: Request
---

`jstz`'s [`Request`](https://developer.mozilla.org/en-US/docs/Web/API/Request) implementation is based on the [Fetch API](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API) specification. This API permits you to manipulate and inspect HTTP requests.

:::danger
⚠️ `jstz`'s implementation is not spec compliant ⚠️
:::

## Constructors

### `new Request(input: Request)`

:::danger
**Spec deviation**: The `referer` is copied from the given request. Additionally the `mode` conversion is not supported.
:::

Constructs a new `Request` object as a copy of the given request.

### `new Request(input: string, init?: RequestInit)`

:::danger
**Spec deviation**: Many of the `RequestInit` properties are not supported.
:::

Creates a new `Request` object, given a URL string and optionally any request settings.
The possible settings are:

- `method` (`string`, optional)

  A string representing the HTTP method of the request. If omitted the default value is `'GET'`.

- `headers` (`HeadersInit`, optional)

  Any headers that should be attached to the request. Either a [`Headers`](./headers.md) object, an `Array` of key-value pairs, or a `Record<string, string>`.

- `body` (`BodyInit | null`, optional)

  :::danger
  **Spec deviation**: `Blob`, `FormData`, `ReadableStream` and `URLSearchParams` are not supported for `BodyInit`.
  :::

  The body attached to the request. Either a `string` or `BufferSource` (an `ArrayBuffer` or `ArrayBufferView`). The body is required for the `'PUT'`, `'POST'` and `'PATCH'` methods and forbidden for the `'GET'`, `'CONNECT'`, `'TRACE'`, `'OPTIONS'` and `'HEAD'` methods.

```typescript
type BodyInit = string | BufferSource;

interface RequestInit {
  body?: BodyInit | null;
  headers?: HeadersInit;
  method?: string;
}
```

## Instance Properties

### `readonly Request.bodyUsed: bool`

A boolean property for whether the `body` of this `Request` has already been used or not.

### `readonly Request.headers`

A `Headers` object containing the headers attached to the request

### `readonly Request.method: string`

A string representing the HTTP method of the request, eg `'GET'`, `'PUT'`, `'POST'`.

### `readonly Request.url: string`

A string property for the URL of the request.

## Instance Methods

### `Request.arrayBuffer(): Promise<ArrayBuffer>`

Returns a promise that resolves with an `ArrayBuffer`.

### `Request.json(): Promise<any>`

Returns a promise that resolves with the result of parsing the body text as JSON.

### `Request.text(): Promise<string>`

Returns a promise that resolves with a UTF-16 `string`.
