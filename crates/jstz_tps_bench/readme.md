# TPS benchmark

Benchmark Jstz against the RISCV sandbox.

Using the fa2 example in examples/fa2. `fa2.js` is created with [this script](https://gitlab.com/tezos/tezos/-/blob/55a6ca91e0d63cda78d3985472b5dc00d537f63b/src/riscv/scripts/get-fa2.sh).

## Run

```
cd $(git rev-parse --show-toplevel)
make riscv-pvm-kernel
# riscv-sandbox needs to be in '$PATH'.
# run.sh must be executed from the root of this crate.
cd crates/jstz_tps_bench && sh run.sh
```
