# 📰 Headers

`jstz`'s [`Headers`](https://developer.mozilla.org/en-US/docs/Web/API/Headers) implementation is based on the [Fetch API](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API) specification. This API permits you to manipulate and inspect HTTP request and response headers.

::: danger
⚠️ `jstz`'s implementation is not spec compliant ⚠️
:::

## Quick Start

We can create a `Headers` instance from a simple object of header names and values:

```typescript
const headers = new Headers({ "Content-Type": "application/json" });
```

We can then add more headers to the `Headers` instance using `Headers.append()`:

```typescript
headers.append("Authorization", "Bearer 123456789");
```

We can also retrieve a header value using `Headers.get()`:

```typescript
const authHeader = headers.get("Authorization");
console.log(authHeader); // "Bearer 123456789"
```

## Referer

The `Referer` header is a special header that is automatically set by `jstz` when it makes a request. The value of the `Referer` header is the `tz4` address of the smart function (or account) that made the request.

```typescript
async function handler(request: Request): Promise<Response> {
    const referer = request.headers.get("Referer"); // "tz4..."
    ...
}
```

## Constructor

### `new Headers(init?: HeadersInit): Headers`

Creates a new `Headers` object.
A `HeadersInit` object can be an `Array` of key-value pairs, `Record<string, string>` or a `Headers` object.

```typescript
type HeadersInit = [string, string][] | Record<string, string> | Headers;
```

## Instance Methods

### `Headers.append(name: string, value: string): void`

Appends a new value onto an existing header inside a `Headers` object, or adds the header if it does not already exist.

### `Headers.delete(name: string): void`

Deletes a header from the `Headers` object.

### `Headers.get(name: string): string | string[] | null`

::: danger
**Spec deviation**: `.get()` returns `string[]` in the case of many header values for a given header name, instead of concatenating them with `,` as per the spec.
:::

Returns the associated header value of the given name, or `null` if no values are found. If the header has more than 1 value, then a sequence of values is returned.

### `Headers.has(name: string): boolean`

Returns a boolean stating whether a `Headers` object contains a certain header.

### `Headers.set(name: string, value: string): void`

Sets a new value for an existing header inside a `Headers` object, or adds the header if it does not already exist.
