import { KV } from "ext:core/ops";

Object.freeze(KV);
Object.defineProperty(globalThis, "KV", {
  value: KV,
  enumerable: false,
  configurable: true,
  writable: false,
});
