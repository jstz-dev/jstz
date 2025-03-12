import * as console from "ext:deno_console/01_console.js";

Object.defineProperty(globalThis, "console", {
  value: new console.Console((msg, level) =>
    globalThis.Deno.core.ops.op_debug_msg(msg, level),
  ),
  enumerable: false,
  configurable: true,
  writable: true,
});
