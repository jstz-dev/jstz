#!/bin/sh
set -ex

rollup_address=sr163Lv22CdE8QagCwf48PWDTquk6isQwv57
inbox_file_path=./inbox.json
log_file_path=./output.log
result_path=./result.log
dir="$(realpath $(dirname "$0"))"
riscv_kernel_path=${RISCV_KERNEL_PATH:-"$dir/../../target/riscv64gc-unknown-linux-musl/release/kernel-executable"}
contract_folder_path=./tps_test

init_endpoint=${1:-init}
transfer_endpoint=${2:-benchmark_transaction1}
check_endpoint=${3:-check}
n_transfer=${4:-transfers}

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

cd $dir
contract_file_path=$contract_folder_path/dist/index.js

# Generate inbox file
$dir/../../target/debug/bench generate other --transfers $n_transfer --inbox-file $inbox_file_path --address $rollup_address --contract-file $contract_file_path --init-endpoint $init_endpoint --transfer-endpoint $transfer_endpoint --check-endpoint $check_endpoint

# Run kernel
if [ -n "${RUN_NATIVELY+x}" ]; then
  make -C $dir/../.. riscv-native-kernel
  $dir/../../target/release/native-kernel-executable --timings >$log_file_path 2>&1 &
  kernel_pid=$!

  # Watch the log and kill kernel when we see the end marker
  (
    tail -n +1 -F "$log_file_path" | sed -n '/Internal message: end of level/q'
    kill "$kernel_pid"
  ) &
  watcher_pid=$!

  # Wait for kernel to exit, then ensure watcher stops
  wait "$kernel_pid" || true
  kill "$watcher_pid" 2>/dev/null || true
else
  riscv-sandbox run --timings --address $rollup_address --inbox-file $inbox_file_path --input $riscv_kernel_path >$log_file_path
fi

# Process results and calculate TPS
$dir/../../target/debug/bench results --expected-transfers $n_transfer --inbox-file $inbox_file_path --log-file $log_file_path | tee $result_path
