class Ledger {
  static get selfAddress() {
    return globalThis.Deno.core.ops.op_self_address();
  }

  static balance(address) {
    return globalThis.Deno.core.ops.op_balance(address);
  }

  static transfer(dst, amount) {
    return globalThis.Deno.core.ops.op_transfer(dst, amount);
  }
}

Object.defineProperties(globalThis, {
  Ledger: {
    value: Ledger,
    enumerable: false,
    configurable: false,
    writable: false,
  },
});
