# Manual test Jstz RISC-V kernel

This test is temporary until we get an automated, auto generated testing flow set up.

The following command will run Jstz RISC-V kernel within the RISC-V sandbox with `inbox.json`. The commands should be run in `jstz/` root directory

```
make run-manual-test
```

## Details

1. `inbox.json` consist of 1 level of inbox messages encoded in the format understood by the RISC-V sandbox.

It contains 201 operations where the first operation deploys `transfer.js`, the second operation mints 1 \* 10^12 tokens (wrapped mutez in this case) to a user account, then the rest of the operations transfers 1000 wrapped mutez to random accounts

2. Each message in `inbox.json` targets the smart rollup address `sr1FXevDx86EyU1BBwhn94gtKvVPTNwoVxUC` so address must be passed into the `--address` flag of the `riscv-sandbox run` command
