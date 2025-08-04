import { workerGlobalScope } from "ext:jstz_main/98_global_scope.js";

Object.defineProperties(globalThis, workerGlobalScope);
Object.defineProperty(globalThis, "self", {
  value: globalThis,
  configurable: false,
  enumerable: false,
  writable: false,
});
