# TPS benchmark

Using the fa2 example in examples/fa2. `fa2.js` is created with [this script](https://gitlab.com/tezos/tezos/-/blob/55a6ca91e0d63cda78d3985472b5dc00d537f63b/src/riscv/scripts/get-fa2.sh).

## Run

```
cd $(git rev-parse --show-toplevel)
# build the crate first to get the bench binary to generate an inbox file
cargo build -p jstz_tps_bench
./target/debug/bench generate --transfers 10 --inbox-file ./crates/jstz_tps_bench/src/kernel/inbox.json
# build the crate again with the feature flag so that the kernel executable compiles with the inbox file
cargo build -p jstz_tps_bench --features static-inbox
./target/debug/kernel --timings > /tmp/benchmark-log
# show the result
./target/debug/bench results --inbox-file ./crates/jstz_tps_bench/src/kernel/inbox.json --log-file /tmp/benchmark-log --expected-transfers 10
```
