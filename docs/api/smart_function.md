# 💡 SmartFunction

The `SmartFunction` namespace provides an API to create and call`jstz` smart functions.
New smart functions can be created with the `SmartFunction.create()` method
and `SmartFunction.call()` is used for calling other smart functions.

All operations on `SmartFunction` are asynchronous.

## Quick Start

We may deploy a new smart function programmatically by calling `SmartFunction.create()` with a single `string` argument.
The smart function code must be valid ECMAScript. TypeScript is not supported when deploying functions using
`SmartFunction.create`.

```typescript
const newSmartFunction = SmartFunction.create(
  "export default handler () => new Response()",
);
```

This will deploy a new smart function with the code `export default handler () => new Response()`,
returning a _promise_ which will resolve to the address of the new function.

Once a smart function is deployed we may call it from another smart function using the
`SmartFunction.call()` method. To call a smart function we create a new [Request](request.md) object with
scheme `tezos` and the address as the hostname.

```typescript
async function handler(_: Request): Promise<Response> {
  const newAddress = await SmartFunction.create(
    "export default handler () => new Response()",
  );
  return SmartFunction.call(new Request(`tezos://${newAddress}`));
}
```

## Instance Methods

### `SmartFunction.call(request: Request): Promise<Response>`

Calls a `jstz` smart function with the given request, returning a promise that resolves to an
HTTP [`Response`](response.md) object.

The `request` parameter is a HTTP [`Request`](request.md) object.
The URL scheme _must_ be `tezos` and the host _must_ be the address of a deployed `jstz` smart function.
The `Referer` header _must_ not be set.

### `SmartFunction.create(code : string): Promise<Address>`

Creates and deploys a new `jstz` smart function with the given code, returning a promise that resolves to the address of the newly deployed smart function.

The `code` must be a `string` containing an ECMAscript module.
The module _must_ define a default export of type `(request: Request) => Response | Promise<Response>`.
