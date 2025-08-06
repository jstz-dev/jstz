# Simple script to capture peak memory usage of jstz in riscv-sandbox. Peak allocation
# occurs on platform initialization which runs in the beginning so don't need to run
# with input

# This script works in Linux only
if [ "$(uname)" != "Linux" ]; then
  echo "heaptrack is only available on Linux"
  exit 1
fi

make riscv-pvm-kernel

heaptrack -o heaptrack.riscv-sandbox riscv-sandbox run \
  --timings \
  --address sr1FXevDx86EyU1BBwhn94gtKvVPTNwoVxUC \
  --input "target/riscv64gc-unknown-linux-musl/release/kernel-executable"

output="heaptrack-summary"

# Print heaptrack summary only; switch off in-depth traces
heaptrack_print -p 0 -a 0 -T 0 -n 0 -s 0 heaptrack.riscv-sandbox.zst >$output

echo ""
echo "The following summary was written to $output"
echo ""
cat $output
