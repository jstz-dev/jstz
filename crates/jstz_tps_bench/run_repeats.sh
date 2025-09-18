#!/bin/bash
set -e

# Number of times to run the benchmark
REPEATS=${1:-5}
BENCHMARK_SCRIPT=${2:-run_other.sh}

DIR="$(realpath $(dirname "$0"))"

# Clear the result_repeats.log file at the start
>"$DIR/result_repeats.log"

echo "Running benchmark $REPEATS times..."

for i in $(seq 1 $REPEATS); do
  echo "Starting run $i of $REPEATS..."

  "$DIR/$BENCHMARK_SCRIPT"

  # Check if result.log exists and has content
  if [ -f "$DIR/result.log" ] && [ -s "$DIR/result.log" ]; then
    # Append a header for this run
    echo "=== Run $i ===" >>"$DIR/result_repeats.log"

    # Append the contents of result.log to result_repeats.log
    cat "$DIR/result.log" >>"$DIR/result_repeats.log"

    echo "" >>"$DIR/result_repeats.log"

    echo "Run $i completed and results appended to result_repeats.log"
  else
    echo "Warning: result.log is empty or doesn't exist after run $i"
  fi
done

echo "All $REPEATS runs completed. Results saved in result_repeats.log"
