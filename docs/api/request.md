# 🙏 Request

`jstz`'s [`Request`](https://developer.mozilla.org/en-US/docs/Web/API/Request) implementation is based on the [Fetch API](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API) specification. This API permits you to manipulate and inspect HTTP request and response headers.

::: danger
⚠️ `jstz`'s implementation is not spec compliant ⚠️
:::

## Constructors
### `new Request(input : Request)`
Constructs a new request object as a copy of its argument.
### `new Request(input : string | URL, options: RequestOptions)`
Creates a new `Request` object.
The first argument may be either a string url, or a [`URL`](./url.md) object.
The second, optional, argument is a javascript object containing any custom settings for the request.
```typescript
type RequestOptions =
  | {
      method?: 'GET' | 'CONNECT' | 'TRACE' | 'OPTIONS' | 'HEAD' | 'DELETE';
      headers?: Headers | { [headerName: string]: string };
    }
  | {
      method: 'POST' | 'PUT' | 'PATCH' | 'DELETE';
      headers?: Headers | { [headerName: string]: string };
      body: ArrayBuffer | string;
    };
```
The available settings are.
#### `method`
A string representing the `http` method of the request, eg `'GET'`, `'PUT'`, `'POST'`. 
If omitted the default value is `'GET'`.
#### `headers`
Any headers that should be attatched to the request.
Either a [`Headers`](./headers.md) object or an object literal whos values are strings.
#### `body`
The body attached to the request. 
This can either be a string or a javascript array buffer.
The body is required for the `'PUT'`, `'POST'` and `'PATCH'` methods and forbidden for the 
`'GET'`, `'CONNECT'`, `'TRACE'`, `'OPTIONS'` and `'HEAD'` methods.

## Instance Properties
### `readonly bodyUsed: bool`
A boolean value representing whether the body has been accessed.
### `readonly headers`
A `Headers` object containing the headers attached to the request
### `readonly method: string`
A string representing the `http` method of the request, eg `'GET'`, `'PUT'`, `'POST'`. 
### `readonly url: URL`
A `URL` object containing the request url.

## Instance Methods
### `async arrayBuffer() : Promise<ArrayBuffer>`
Returns a promise containing the response body as an
[`ArrayBuffer`](https://developer.mozilla.org/en-US/docs/Web/API/Request/arrayBuffer).

### `async json() : Promise<unknown>`
Returns a promise containing the response body parsed as JSON.
### `async text() : Promise<String>`
Returns a promise containing the response body as a string.
