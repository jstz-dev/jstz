# Jstz Runtime

To find compatible deno extension versions, look for their version number in the [Cargo.toml](https://github.com/denoland/deno/blob/v2.1.10/Cargo.toml#L74) of `deno_core:v2.1.0`

### API coverage test

This test produces a report file following the format defined in [cloudflare/workers-nodejs-compat-matrix](https://github.com/cloudflare/workers-nodejs-compat-matrix). It basically checks which Web APIs and Node.js APIs are supported. Since jstz_runtime does not support Node.js APIs at all, most values should be `missing`.

Before running the test, some required scripts need to be fetched from the upstream. `./tests/api_coverage/setup.sh` handles this by reading the files from a fork. To run the test,

```sh
./tests/api_coverage/setup.sh
OUTPUT_PATH=<path> cargo test --test api_coverage
```

The report file will be dumped to `OUTPUT_PATH`. `OUTPUT_PATH` is optional. To visualise the report, see [jstz-dev/nodejs-compat-matrix](https://github.com/jstz-dev/nodejs-compat-matrix).
