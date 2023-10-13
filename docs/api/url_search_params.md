# 🔍 URLSearchParams

`jstz`'s implementation of the `URLSearchParams` API defines utility methods for working with the query string of a URL according to the [URL specification](https://url.spec.whatwg.org/#urlsearchparams). It is used for building and manipulating search parameters.

## Example

```typescript
// Parse query string from URL
let url = new URL("https://example.com?foo=1&bar=2");
let params = new UrlSearchParams(url.search);

// Add a new parameter
params.append("baz", 3);

// Remove parameter
params.delete("bar");
```

## Constructor

### `new UrlSearchParams(init?: [string, string][] | Record<string, string> | string)`

Creates a new instance of `UrlSearchParams` with the provided key-value pairs.

- **init**:
  One of:
  - An array of key-value pairs. Each pair is an array where the first element is the key (Name) and the second is the value.
  - A record of string keys and string values.
  - A string, which will be parsed from application/x-www-form-urlencoded format. A leading '?' character is ignored.

## Instance Properties

### `readonly size: number`

- **Returns**: The number of search parameters present.

## Instance Methods

### `append(name: string, value: string): void`

Appends a specified key/value pair as a new search parameter.

- **name**: The name of the search parameter.
- **value**: The value of the search parameter.

### `delete(name: string, value?: string): void`

Removes search parameters that match the given name. If a value is provided, only parameters with that name-value pair are removed.

- **name**: The name of the search parameter to be removed.
- **value** (optional): The specific value of the search parameter to be removed.

### `get(name: string): string | null`

Returns the first value associated with the given search parameter.

- **name**: The name of the search parameter.

- **Returns**: The value associated with the given search parameter or `null` if not found.

### `getAll(name: string): string[]`

Returns all the values associated with a given search parameter.

- **name**: The name of the search parameter.

- **Returns**: An array of values associated with the given search parameter.

### `has(name: string, value?: string): boolean`

Determines whether the `UrlSearchParams` object has a certain parameter, optionally with a specific value.

- **name**: The name of the parameter you want to check for.
- **value** (optional): The value of the parameter you want to check for.
- **Returns**: `true` if the parameter, or parameter-value pair, exists. Otherwise, returns `false`.

### `set(name: string, value: string): void`

Sets the value associated with a given parameter. If there are several matching parameters, it updates the first and removes the others.

- **name**: The name of the parameter you want to set or update.
- **value**: The new value for the parameter.

If the parameter does not exist, this method will append the parameter-value pair.

### `sort(): void`

Sorts all key/value pairs in the `UrlSearchParams` object by their keys. The sorting is done by comparing the code units of the keys. The relative order between pairs with equal names is preserved.

### `toString(): string`

Returns a query string suitable for use in a URL.
