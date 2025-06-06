---
title: TextDecoder
sidebar_label: TextDecoder
---

The TextDecoder interface represents a decoder for a specific text encoding, such as `UTF-8`, `ISO-8859-2`, `KOI8-R`, `GBK`, etc. A decoder takes a stream of bytes as input and emits a stream of code points.

:::danger
⚠️ `jstz`'s implementation is not fully spec compliant ⚠️
:::

## Constructor

### `new TextDecoder(label?: string, options?: TextDecoderOptions): TextDecoder`

The `TextDecoder()` constructor returns a newly created TextDecoder object for the encoding specified in parameter.

**parameters:**

- `label` A string, defaulting to `"utf-8"`. This may be [any valid label](https://developer.mozilla.org/en-US/docs/Web/API/Encoding_API/Encodings).

- `options` An object with the property:
  - `fatal` A boolean value indicating if the `TextDecoder.decode()` method must throw a `TypeError` when decoding invalid data. It defaults to `false`, which means that the decoder will substitute malformed data with a replacement character.
  - `ignoreBOM` A boolean value indicating whether the byte order mark is ignored. It defaults to `false`.

```typescript
interface TextDecoderOptions {
  fatal?: boolean;
  ignoreBOM?: boolean;
}
```

## Instance properties

### `readonly TextDecoder.encoding: string`

Returns encoding's name, lowercased. The encoding is set by the `TextDecoder()` constructor `label` parameter, and defaults to `"utf-8"`.

### `readonly TextDecoder.fatal: boolean`

The `fatal` read-only property of the `TextDecoder` interface is a boolean indicating whether the error mode is `fatal`.

If the property is `true`, then a decoder will throw a `TypeError` if it encounters malformed data while decoding. If `false`, the decoder will substitute the invalid data with the replacement character `U+FFFD` (�). The value of the property is set in the `TextDecoder()` constructor.

### `readonly TextDecoder.ignoreBOM: boolean`

The `ignoreBOM` read-only property of the `TextDecoder` interface is a boolean indicating whether the _byte order mark_ is ignored.

## Instance methods

### `TextDecoder.decode(input?: BufferSource, options?: TextDecodeOptions): string`

:::danger
⚠️ Spec deviation: input can not be a `SharedArrayBuffer` ⚠️
:::

Returns the result of running encoding's decoder. The method can be invoked zero or more times with `options.stream` set to `true`, and then once without `options. stream` (or set to `false`), to process a fragmented stream. If the invocation without `options.stream` (or set to `false`) has no input, it's clearest to omit both arguments.

**parameters:**

- `input` An `ArrayBuffer`, a `TypedArray`, or a `DataView` object containing the encoded text to decode.
- `options` An object with the property:
  - `stream` If the error mode is "fatal" and encoding's decoder returns error, throws a `TypeError`.

```typescript
interface TextDecodeOptions {
  stream?: boolean;
}
```

## Examples

Encodes and decodes the euro symbol, `€`.

```js
const encoder = new TextEncoder();
const array = encoder.encode("€"); // Uint8Array(3) [226, 130, 172]
document.getElementById("encoded-value").textContent = array;

const decoder = new TextDecoder();
const str = decoder.decode(array); // String "€"
document.getElementById("decoded-value").textContent = str;
```

---

Decode from a buffer in a loop using the `stream` option.

```js
let string = "";
let decoder = new TextDecoder(encoding);

let buffer;
while ((buffer = stream_next_chunk())) {
  string += decoder.decode(buffer, { stream: true });
}

string += decoder.decode(); // end-of-stream
```
