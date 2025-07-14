#!/bin/sh
TARGET_SHA=426ca553141d5ac41764beb9078bd27efd980756

# Fetch the baseline file and the script with helper functions that are part of the test extension.
# This script must be run from the root of this crate.

# The source baseline file is a JSON file. Importing JSON files in a deno extension doesn't seem to
# be straightforward, so here it is changed to a script file whose only job is to export the baseline
# data as a JSON object.
curl -s --output /tmp/baseline.json https://raw.githubusercontent.com/cloudflare/workers-nodejs-compat-matrix/$TARGET_SHA/data/baseline.json
(
  echo "export default "
  cat /tmp/baseline.json
) >./tests/api_coverage/baseline.js

curl -s --output ./tests/api_coverage/utils.js https://raw.githubusercontent.com/cloudflare/workers-nodejs-compat-matrix/$TARGET_SHA/dump-utils.mjs
