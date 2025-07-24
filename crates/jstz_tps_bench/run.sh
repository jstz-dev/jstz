#!/bin/sh
set -ex

rollup_address=sr163Lv22CdE8QagCwf48PWDTquk6isQwv57
inbox_file_path=./inbox.json
n_transfer=10
log_file_path=./output.log
result_path=./result.log

cargo build --bin bench

# Generate inbox file
../../target/debug/bench generate --transfers $n_transfer --inbox-file $inbox_file_path --address $rollup_address

# Run riscv kernel with inbox file
riscv-sandbox run --timings --address $rollup_address --inbox-file $inbox_file_path --input ../../target/riscv64gc-unknown-linux-musl/release/kernel-executable >$log_file_path

# Process results and calculate TPS
../../target/debug/bench results --expected-transfers $n_transfer --inbox-file $inbox_file_path --log-file $log_file_path >$result_path
