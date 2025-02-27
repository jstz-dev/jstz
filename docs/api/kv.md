# 🪣 KV

As described in [Storing data](/functions/data_storage), Jstz smart functions store data in a persistent key-value database.
This database is built directly into the Jstz runtime, available using the global `Kv` object.
Smart functions can read only their own data, not the data of other smart functions.
Therefore, it is not necessary to worry about name collisions for keys with other smart functions.
Other clients can read the data, but only a smart function can write to the keys it puts in the database.

Data is stored as a persistent collection of key-value pairs, much like the properties of a JavaScript object or a Map object.
The keys are strings and the values are serializable JavaScript objects.
Keys are unique within the database, and the last value to be set is the one that is returned when next reading the key.

As described in [Errors](/functions/data_storage#errors) operations on the database are synchronous and atomic, committed only if the request to the smart function succeeds.
If a smart function writes to storage and throws an uncaught error later in the same transaction, the changes to the database are reversed and the storage is set to what it was before the request that failed.

To create a key-value pair and store data, pass the key and the value to the `Kv.set` function, as in this example:

```typescript
Kv.set("foo", { bar: "baz" });
```

To read it, pass the key to the `Kv.get()` function:

```typescript
const data = Kv.get("foo");
console.log(JSON.stringify(data)); // { "bar": "baz" }
```

You can also delete a key-value pair using `Kv.delete()`.

```typescript
Kv.delete("foo");
```

For more examples, see [Storing data](/functions/data_storage).

## Instance Methods

### `Kv.set(key: string, value: unknown): void`

Set the value for the given key in the database. If a value already exists for the key, it will be overwritten.

### `Kv.get<T = unknown>(key: string): T | null`

Retrieve the value for the given key from the database. If no value exists for the key, this function returns `null`.

### `Kv.delete(key: string): void`

Deletes the value for the given key from the database. If no value exists for the key, this function is a no-op.

### `Kv.has(key: string): boolean`

Returns `true` if a value exists for the given key in the database, `false` otherwise.
