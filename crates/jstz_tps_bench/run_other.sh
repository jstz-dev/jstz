#!/bin/sh
set -ex

rollup_address=sr163Lv22CdE8QagCwf48PWDTquk6isQwv57
inbox_file_path=./inbox.json
n_transfer=200
log_file_path=./output.log
result_path=./result.log
dir="$(realpath $(dirname "$0"))"
contract_folder_path=./tps_test
init_endpoint=init
transfer_endpoint=benchmark_transaction1
check_endpoint=check

case "${DISABLE_BUILD}" in
1 | true | yes) ;;
*)
  unset NIX_LDFLAGS && RUSTY_V8_ARCHIVE=$RISCV_V8_ARCHIVE_DIR/librusty_v8.a \
    RUSTY_V8_SRC_BINDING_PATH=$RISCV_V8_ARCHIVE_DIR/src_binding.rs \
    cargo build \
    -p jstz_kernel \
    --no-default-features \
    --features riscv_kernel \
    --release \
    --target riscv64gc-unknown-linux-musl
  ;;
esac

cargo build --bin bench --features v2_runtime

# Compile contract
cd $contract_folder_path
npm run build
cd $dir
contract_file_path=$contract_folder_path/dist/index.js

# Generate inbox file
$dir/../../target/debug/bench generate other --transfers $n_transfer --inbox-file $inbox_file_path --address $rollup_address --contract-file $contract_file_path --init-endpoint $init_endpoint --transfer-endpoint $transfer_endpoint --check-endpoint $check_endpoint

# Run riscv kernel with inbox file
riscv-sandbox run --timings --address $rollup_address --inbox-file $inbox_file_path --input $dir/../../target/riscv64gc-unknown-linux-musl/release/kernel-executable >$log_file_path

# Process results and calculate TPS
$dir/../../target/debug/bench results --expected-transfers $n_transfer --inbox-file $inbox_file_path --log-file $log_file_path | tee $result_path
