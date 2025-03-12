import { Kv } from "ext:core/ops";

Object.freeze(Kv);
Object.defineProperty(globalThis, "Kv", {
  value: Kv,
  enumerable: false,
  configurable: false,
  writable: false,
});
