import { test_result_callback, test_completion_callback } from "ext:core/ops";

Object.defineProperty(globalThis, "location", {
  value: {},
  enumerable: true,
  configurable: true,
  writable: true,
});

// `setTimeout` and `clearTimeout` are referenced by the test setup. It checks the presence
// of these two functions and uses a mock if they are not defined. These two functions defined
// in jstz throw an exception when they are called because timer is not yet handled. This
// breaks the test setup. Before timer is properly enabled, these two functions need to be
// removed from the test setup. This also means that this test API plugin needs to be imported
// after other global scope API plugins are loaded.
delete globalThis.setTimeout;
delete globalThis.clearTimeout;

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
