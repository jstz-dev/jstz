---
title: 🧩 URLPattern
sidebar_label: URLPattern
---

`jstz`'s [`URLPattern`](hhttps://developer.mozilla.org/en-US/docs/Web/API/URLPattern) implementation is based on the [URL Pattern](https://urlpattern.spec.whatwg.org/) specification and using [`rust-urlpattern`](https://docs.rs/urlpattern/latest/urlpattern/).

:::danger
⚠️ `jstz`'s implementation is not spec compliant ⚠️
:::

## Example

```typescript
// A pattern can be constructed using a string for matching
const pattern = new URLPattern("http{s}?://*.example.com/books/:id");

// `test` can be used to check for match
console.log(pattern.test("https://store.example.com/books/123")); // prints true
console.log(pattern.test("https://example.com/books/123")); // prints false

// `exec` can be used to match and access components
const match = pattern.exec("https://store.example.com/books/123");
console.log(match.pathname); // prints { input: "/books/123", groups: { "id": "123" } }
```

## Constructor

### `new URLPattern(input?: URLPatternInput, baseURL?: string): URLPattern`

Creates a new `URLPattern` object from a given URL string or `URLPatternInit` and an optional base URL.

:::danger
**Spec deviation**: `options` argument is not supported in the constructor.
:::

```typescript
declare interface URLPatternInit {
  protocol?: string;
  username?: string;
  password?: string;
  hostname?: string;
  port?: string;
  pathname?: string;
  search?: string;
  hash?: string;
  baseURL?: string;
}

declare type URLPatternInput = string | URLPatternInit;
```

## Instance Properties

### `readonly URLPattern.protocol: string`

The pattern used to match the protocol part of a URL.

### `readonly URLPattern.username: string`

The pattern used to match the username part of a URL.

### `readonly URLPattern.password: string`

The pattern used to match the password part of a URL.

### `readonly URLPattern.hostname: string`

The pattern used to match the hostname part of a URL.

### `readonly URLPattern.port: string`

The pattern used to match the port part of a URL.

### `readonly URLPattern.pathname: string`

The pattern used to match the pathname part of a URL.

### `readonly URLPattern.search: string`

The pattern used to match the search part of a URL.

### `readonly URLPattern.hash: string`

The pattern used to match the fragment part of a URL.

## Instance Methods

### `URLPattern.test(input?: URLPatternInput, baseURL?: string): boolean`

Returns a boolean indicating if the given input matches the current pattern. The input is a URL or an object of URL parts.

### `URLPattern.exec(input?: URLPatternInput, baseURL?: string): URLPatternResult | null`

```typescript
declare interface URLPatternComponentResult {
  input: string;
  groups: Record<string, string | undefined>;
}

declare interface URLPatternResult {
  inputs: URLPatternInit[];
  protocol: URLPatternComponentResult;
  username: URLPatternComponentResult;
  password: URLPatternComponentResult;
  hostname: URLPatternComponentResult;
  port: URLPatternComponentResult;
  pathname: URLPatternComponentResult;
  search: URLPatternComponentResult;
  hash: URLPatternComponentResult;
}
```

Returns either an object containing the results of matching the URL to the pattern, or `null` if the URL does not match the pattern. The input is a URL or object of URL parts.
