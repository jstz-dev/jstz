# 🧰 API Reference

This is a reference for all runtime APIs available in `jstz`. Since `jstz` is a JavaScript server runtime
running on Tezos's smart optimistic rollups, some APIs are not available (e.g. `DOM`).

::: danger
⚠️ `jstz`'s APIs are currently very unstable and not compliant with specifications. ⚠️
:::

## Web Platform APIs

- `console`
- `atob`
- `btoa`
- Fetch API:
  - `Request`
  - `Response`
  - [`Headers`](./headers.md)
- URL API:
  - `URL`
  - `URLSearchParams`

## `jstz`-specific APIs

- [`Kv`](./kv.md)
- `Contract`
- [`Ledger`](./ledger.md)
