import * as console from "ext:deno_console/01_console.js";

const jstzConsole = new console.Console(
  (msg, level) => globalThis.Deno.core.ops.op_debug_msg(msg, level),
  { noColorStdout: true, noColorStderr: true },
);

export default jstzConsole;
