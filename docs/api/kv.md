# 🪣 KV

A persistent key-value database that can be used to store and retrieve JSON blobs built directly into the `jstz` runtime,
available using the global `Kv` object.

Data in `Kv` is stored as a persistent collection of key-value pairs, much like to properties of a JavaScript object or a Map object.
The keys are represented as strings, while the values are serializable JavaScript objects. Keys are unique within the database, and
the last value to be set is the one that is returned when next reading the key.

All operations on `Kv` are synchronous and atomic, committed if the request to the smart function succeeds.

## Quick Start

We create a key-value pair using the key `"foo"` and the value `{ bar: "baz" }`:

```typescript
Kv.set("foo", { bar: "baz" });
```

Once a key-value pair is set, you can read it using `Kv.get()`:

```typescript
const data = Kv.get("foo");
console.log(JSON.stringify(data)); // { "bar": "baz" }
```

You can also delete a key-value pair using `Kv.delete()`.

```typescript
Kv.delete("foo");
```

## Instance Methods

### `Kv.set(key: string, value: unknown): void`

Set the value for the given key in the database. If a value already exists for the key, it will be overwritten.

### `Kv.get<T = unknown>(key: string): T | null`

Retrieve the value for the given key from the database. If no value exists for the key, this returns `null`.

### `Kv.delete(key: string): void`

Deletes the value for the given key from the database. If no value exists for the key, this is an no-op.

### `Kv.has(key: string): boolean`

Returns `true` if a value exists for the given key in the database, `false` otherwise.
