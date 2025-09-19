#!/bin/bash
set -e

# Number of times to run each benchmark configuration
REPEATS=${1:-5}

DIR="$(realpath $(dirname "$0"))"

# Clear the results_all.log file at the start
>"$DIR/results_all.log"

FA2_TRANSFERS=200
# Array of argument sets for run_other.sh
# Format: "init_endpoint transfer_endpoint check_endpoint n_transfer description"
OTHER_CONFIGS=(
  "init_1 benchmark_transaction1 check_1 200 benchmark1_config"
  "init_2 benchmark_transaction2 check_2 200 benchmark2_config"
  "init_3 benchmark_transaction3 check_3 200 benchmark3_config"
  "init_4 benchmark_transaction4 check_4 200 benchmark4_config"
)

echo "Running FA2 benchmark ($REPEATS times)..."
echo "=== FA2 Benchmark ===" >>"$DIR/results_all.log"

for i in $(seq 1 $REPEATS); do
  echo "Starting FA2 run $i of $REPEATS..."

  "$DIR/run_fa2.sh" "$FA2_TRANSFERS"

  # Check if result.log exists and has content
  if [ -f "$DIR/result.log" ] && [ -s "$DIR/result.log" ]; then
    # Append a header for this run
    echo "--- Run $i ---" >>"$DIR/results_all.log"

    # Append the contents of result.log to results_all.log
    cat "$DIR/result.log" >>"$DIR/results_all.log"

    echo "" >>"$DIR/results_all.log"

    echo "FA2 run $i completed and results appended to results_all.log"
  else
    echo "Warning: result.log is empty or doesn't exist after FA2 run $i"
  fi
done

echo "All $REPEATS FA2 runs completed."
echo "" >>"$DIR/results_all.log"

echo "Running other benchmarks with different configurations..."

# Run each configuration multiple times
for config in "${OTHER_CONFIGS[@]}"; do
  # Parse the configuration
  read -r init_endpoint transfer_endpoint check_endpoint n_transfer description <<<"$config"

  echo "Running configuration: $description ($REPEATS times)"
  echo "=== Configuration: $description ===" >>"$DIR/results_all.log"

  for i in $(seq 1 $REPEATS); do
    echo "Starting run $i of $REPEATS for $description..."

    "$DIR/run_other.sh" "$init_endpoint" "$transfer_endpoint" "$check_endpoint" "$n_transfer"

    # Check if result.log exists and has content
    if [ -f "$DIR/result.log" ] && [ -s "$DIR/result.log" ]; then
      # Append a header for this run
      echo "--- Run $i ---" >>"$DIR/results_all.log"

      # Append the contents of result.log to results_all.log
      cat "$DIR/result.log" >>"$DIR/results_all.log"

      echo "" >>"$DIR/results_all.log"

      echo "Run $i of $description completed and results appended to results_all.log"
    else
      echo "Warning: result.log is empty or doesn't exist after run $i of $description"
    fi
  done

  echo "All $REPEATS runs of $description completed."
  echo "" >>"$DIR/results_all.log"
done

echo "All benchmarks completed. Results saved in results_all.log"
