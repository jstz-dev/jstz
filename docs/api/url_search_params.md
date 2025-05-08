---
title: 🔍 URLSearchParams
sidebar_label: URLSearchParams
---

`jstz`'s implementation of the `URLSearchParams` API defines utility methods for working with the query string of a URL according to the [URL specification](https://url.spec.whatwg.org/#urlsearchparams). It is used for building and manipulating search parameters.

## Example

```typescript
// Parse query string from URL
let url = new URL("https://example.com?foo=1&bar=2");
let params = new URLSearchParams(url.search);

// Add a new parameter
params.append("baz", 3);

// Remove parameter
params.delete("bar");
```

## Constructor

### `new URLSearchParams(init?: [string, string][] | Record<string, string> | string): URLSearchParams`

Creates a new instance of `URLSearchParams` with the provided key-value pairs. The `init` parameter can be one of the following:

- An array of key-value pairs. Each pair is an array where the first element is the key (Name) and the second is the value.
- A record of `string` keys and `string` values.
- A `string`, which will be parsed from [`application/x-www-form-urlencoded`](https://url.spec.whatwg.org/#application/x-www-form-urlencoded) format. The leading '?' character is ignored.

## Instance Properties

### `readonly URLSearchParams.size: number`

Returns the number of search parameters present.

## Instance Methods

### `URLSearchParams.append(name: string, value: string): void`

Appends a specified name-value pair as a new search parameter.

### `URLSearchParams.delete(name: string, value?: string): void`

Removes search parameters that match the given name. If a value is provided, only parameters with that name-value pair are removed.

### `URLSearchParams.get(name: string): string | null`

Returns the first value associated with the given search parameter `name` or `null` if not found.

### `URLSearchParams.getAll(name: string): string[]`

Returns all the values associated with a given search parameter `name`.

### `URLSearchParams.has(name: string, value?: string): boolean`

Determines whether the `UrlSearchParams` object has a certain parameter, optionally with a specific value.

### `URLSearchParams.set(name: string, value: string): void`

Sets the value associated with a given parameter. If there are several matching parameters, it updates the first and removes the others.

If the parameter does not exist, this method will append the name-value pair.

### `URLSearchParams.sort(): void`

Sorts all name-value pairs in the `UrlSearchParams` object by their names. The sorting is done by comparing the code units of the names. The relative order between pairs with equal names is preserved.

### `URLSearchParams.toString(): string`

Returns a query string suitable for use in a URL.

### `URLSearchParams[Symbol.iterator](): Iterator<[string, string]>`

Returns an iterator over the list of search parameter name-value pairs. This makes `URLSearchParams` instances [iterable](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Iteration_protocols#the_iterable_protocol).

### `URLSearchParams.entries(): Iterator<[string, string]>`

Returns an iterator over the list of search parameter name-value pairs.

### `URLSearchParams.keys(): Iterator<string>`

Returns an iterator over the search parameter names.

### `URLSearchParams.values(): Iterator<string>`

Returns an iterator over the search parameter values.

### `URLSearchParams.forEach(callback: (value: string, name: string, parent: URLSearchParams) => void): void`

Calls the callback for each search parameter. Note that the search parameter value is the _first_ callback argument, while the name is the second argument.
