# 🔗 URL
An API for working with HTTP URLs. 

## Quick Start

We can construct a url in two ways, either as an absolute url or a relative url.
```typescript 
let url : URL = new URL(`tezos://${my_contract.address}/entrypoint`);
let url2 : URL = new URL('../entrypoint_2', url.href);
```
The constructor will raise and exception if the arguments cannot be parsed into a valid url.
The [`URL.canParse()`](#canParse) method can be used to check if the arguments can be parsed correctly.
```typescript
if (URL.canParse(foo, bar)) {
  let url = new URL(foo, bar)
  console.log(url.href)
} else {
  // the URL cannot be parsed, take appropriate action.
  console.error("Invalid URL")
}
```

We may edit or construct a url by setting values for its properties.
```typescript
let url = new URL('tezos://domain/);
url.pathname = "my_entrypoint"
url.hostname = Ledger.selfAddress;
url.hash = "my_fragment"
console.log(url.href) // tezos://tz4../my_entrypoint#my_fagment
```

The [`URLSearchParams`](./url_search_params.md) API may be used to build and manipulate search parameters. To get the search parameters from the URL, you may use the `.searchParams` instance property.
```typescript
let url = new URL(`tezos://${address}/?first_name=Dave`);
switch (url.searchParams.get("first_name")) {
  case "Jim":
    url.searchParams.set("last_name","Jones");
    break
  case "Sarah":
    url.searchParams.set("last_name","Smith");
    break
  case "Dave":
    url.searchParams.set("last_name","Davies");
    break
}
```


## Constructor
### `URL(url: string, base?: string): URL`
Constructs a URL from a url string and an optional base string. 
If `base` if present then `url` will be interpreted as a relative url. 
If `base` is not present then `url` will be interpreted as an absolute url.
## Instance Properties
### `hash: string`
The fragment identifier of the URL.
### `host: string`
The host, a string containing the hostname (see below), followed by a ':' and the port of the url.
### `hostname: string`
The hostname of the url. In `jstz` this will usually be a `tz4` address of a smart function.
### `href: string` {#href}
A stringifier, returns the whole url.
### `readonly origin: string`
The origin of the url, specifically the scheme, the domain and the port.
### `password: string`
The password specified before the domain name.
### `pathname: string`
The url path. This will always begin with a `'/'` and contains the part of the url up until the query string or fragment.
### `port: string`
The port number of the url.  This has no special meaning within `jstz` and will not usually be present.
### `protocol: string`
The protocol scheme of the url. Within `jstz` this will usually be `tezos:`
### `search: string`
The URL's search parameter string, This will include all the search parameters of the url, each of which begins with a `'?'`.
### `readonly searchParams: URLSearchParams`
The search parameter object. See [`URLSearchParams`](./url_search_params.md) for more information.
### `username: string`
The username specified before the domain name.
## Static Methods
### `canParse(url: string, base?: string): boolean` {#canParse}
Returns `true` if the url and base string can be parsed into a valid URL.
## Instance Methods
### `toString(): string`
An alias for [`href`](#href); returns the whole url as a string. 
### `toJSON(): string`
An alias for [`href`](#href); returns the whole url as a string. 
