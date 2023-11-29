# TextEncoder

The `TextEncoder` interface takes a stream of code points as input and emits a stream of UTF-8 bytes.

::: danger ⚠️ jstz's implementation is not fully spec compliant ⚠️ :::

## Constructor

### `new TextEncoder(): TextEncoder`

Returns a newly constructed `TextEncoder` that will generate a byte stream with UTF-8 encoding.

## Instance properties

### `TextEncoder.encoding` (Read only)

Returns "utf-8".

## Instance methods

### `TextEncoder.encode(input: string): uint8Array`

Returns the result of running UTF-8's encoder.

`TextEncoder.encodeInto(source:string, destination: uint8Array): TextEncoderEncodeIntoResult`

Runs the UTF-8 encoder on source, stores the result of that operation into destination, and returns the progress made as a dictionary whereby read is the number of converted code units of source and written is the number of bytes modified in destination.

```typescript
type TextEncoderEncodeIntoResult = { read: number; write: number };
```
