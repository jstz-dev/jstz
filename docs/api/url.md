---
title: URL
sidebar_label: URL
---

`jstz`'s implementation of the `URL` API defines utilities for URL resolution and parsing according to the [URL specification](https://url.spec.whatwg.org/#urlsearchparams).

## Quick Start

There are two ways to create a URL: either as an absolute URL or a relative URL.

```typescript
let url: URL = new URL(`jstz://${my_function.address}/entrypoint`);
let url2: URL = new URL("../entrypoint_2", url.href);
```

Each `jstz` smart function is assigned a unique address, akin to an IP address, starting with `KT1` when the function is deployed.
To decode these addresses, `jstz` employs its own URL scheme `jstz://`.
An example URL for a `jstz` smart function would therefore be `jstz://KT19mYzcaYk55KttezwP4TbMrGCDpVuPW3Jw/`.

It's important to note that if the base URL or the resulting URL is not valid, the constructor will raise a `TypeError` exception.
To check whether URLs can be parsed correctly, you can use the static method [`URL.canParse()`](#canParse).

```typescript
if (URL.canParse(relativePath, baseUrl)) {
  let url = new URL(relativePath, baseUrl);
  console.log(url.href);
} else {
  // the URL cannot be parsed, take appropriate action.
  console.error("Invalid URL");
}
```

You can also modify a URL by setting its properties.

```typescript
let url = new URL("jstz://KT19mYzcaYk55KttezwP4TbMrGCDpVuPW3Jw/"); // not a valid address, we'll have to change it
url.hostname = Ledger.selfAddress;
url.pathname = "accounts";
url.hash = "#id";
console.log(url.href); // jstz://KT1../accounts#id
```

The [`URLSearchParams`](./url_search_params.md) API may be used to build and manipulate search parameters. To get the search parameters from the URL, you can make use of the `.searchParams` instance property.

```typescript
let url = new URL(`jstz://${address}/?first_name=Dave`);
switch (url.searchParams.get("first_name")) {
  case "Jim":
    url.searchParams.set("last_name", "Jones");
    break;
  case "Sarah":
    url.searchParams.set("last_name", "Smith");
    break;
  case "Dave":
    url.searchParams.set("last_name", "Davies");
    break;
}
```

## Constructor

### `new URL(url: string, base?: string): URL`

Constructs a URL from a given URL string and an optional base URL.
If `base` if present then `url` will be interpreted as a relative URL.
If `base` is not present then `url` will be interpreted as an absolute URL.
Raises a `TypeError` exception if the base URL or resulting URL aren't valid URLs.

## Instance Properties

### `URL.hash: string`

The fragment identifier of the URL.

### `URL.host: string`

The host, a string containing the hostname (see below), followed by a ':' and the port of the URL.

### `URL.hostname: string`

The hostname of the URL. In `jstz` this will usually be a `KT1` address of a smart function.

### `URL.href: string` {#href}

A stringifier, returns the whole URL.

### `readonly URL.origin: string`

The origin of the URL, specifically the scheme, the domain and the port.

### `URL.password: string`

The password specified before the domain name.

### `URL.pathname: string`

The URL path. This will always begin with a `'/'` and contains the part of the URL up until the query string or fragment.

### `URL.port: string`

The port number of the URL. This has no special meaning within `jstz` and will not usually be present.

### `URL.protocol: string`

The protocol scheme of the URL. Within `jstz` this will usually be `tezos:`

### `URL.search: string`

The URL's search parameter string, This will include all the search parameters of the URL, each of which begins with a `'?'`.

### `readonly URL.searchParams: URLSearchParams`

The search parameter object. See [`URLSearchParams`](./url_search_params.md) for more information.

### `URL.username: string`

The username specified before the domain name.

## Static Methods

### `URL.canParse(url: string, base?: string): boolean` {#canParse}

Returns `true` if the URL and base URL strings can be parsed into a valid URL.

## Instance Methods

### `URL.toString(): string`

An alias for [`href`](#href); returns the whole URL as a string.

### `URL.toJSON(): string`

An alias for [`href`](#href); returns the whole URL as a string.
