# TPS benchmark

Benchmark Jstz against the RISCV sandbox and native execution.
There are two types of benchmarks:

- FA2 mock contract
- other transaction types such as KV updates or sending tez

Please refer to https://docs.google.com/document/d/1kg3QuRXQO8mtlv33VO3TwaSmaiV5t6_Gl61EzINZ8Zg/edit?tab=t.0#heading=h.uzju5d9h4e8c for more details.

## Run

RISCV sandbox:

```
cd $(git rev-parse --show-toplevel)
make riscv-pvm-kernel
# riscv-sandbox needs to be in '$PATH'.
# run_all.sh must be executed from the root of this crate.
cd crates/jstz_tps_bench && sh run_all.sh
```

Native execution:

```
cd $(git rev-parse --show-toplevel)
make riscv-pvm-kernel
# riscv-sandbox needs to be in '$PATH'.
# run_all.sh must be executed from the root of this crate.
cd crates/jstz_tps_bench
export RUN_NATIVELY=1; sh run_all.sh
```
