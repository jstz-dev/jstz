---
title: 🔣 Encoding
sidebar_label: Encoding
---

An implementation of the Web standard encoding API.
The Encoding API provides a mechanism for handling text in various character encodings, including legacy non-UTF-8 encodings.

:::danger
⚠️ `jstz`'s implementation is not fully spec compliant ⚠️
:::

## Interface

- [`TextEncoder`](./text_encoder.md)
- [`TextDecoder`](./text_decoder.md)
- `atob`
- `btoa`
- ~~`TextDecoderStream`~~ (🔨 Work in progress)
- ~~`TextEncoderStream`~~ (🔨 Work in progress)

## Global

### `btoa(s: string): string`

Creates a base-64 ASCII encoded string from the input string.

```js
console.log(btoa("hello world")); // outputs "aGVsbG8gd29ybGQ="
```

### `atob(s: string): string`

Decodes a string of data which has been encoded using base-64 encoding.

```js
console.log(atob("aGVsbG8gd29ybGQ=")); // outputs 'hello world'
```
