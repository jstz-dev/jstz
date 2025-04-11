import { test_result_callback, test_completion_callback } from "ext:core/ops";

Object.defineProperty(globalThis, "location", {
  value: {},
  enumerable: true,
  configurable: true,
  writable: true,
});

Object.defineProperty(globalThis, "self", {
  value: globalThis,
  enumerable: true,
  configurable: true,
  writable: true,
});

Object.defineProperty(globalThis, "test_result_callback", {
  value: test_result_callback,
  enumerable: true,
  configurable: true,
  writable: true,
});

Object.defineProperty(globalThis, "test_completion_callback", {
  value: test_completion_callback,
  enumerable: true,
  configurable: true,
  writable: true,
});
