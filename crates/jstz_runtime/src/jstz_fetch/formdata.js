import * as formData from "ext:deno_fetch/21_formdata.js";

Object.defineProperty(globalThis, "FormData", {
  value: formData.FormData,
  enumerable: false,
  configurable: true,
  writable: true,
});
