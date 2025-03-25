import { op_self_address } from "ext:core/ops";

const global_impl = {
  get selfAddress() {
    return op_self_address();
  },
};

Object.defineProperty(globalThis, "global", {
  value: global_impl,
  enumerable: true,
  configurable: false,
  writable: false,
});
