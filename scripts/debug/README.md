# Debug Scripts - jstzd Equivalent

These scripts replicate exactly what `cargo run --bin jstzd -- run` does.

## Overview

These scripts break down the jstzd startup process into individual steps for debugging. They use the **same configuration, accounts, and parameters** that jstzd uses.

## Key Differences from Old Debug Scripts

1. **8 Bootstrap Accounts**: Uses all 8 bootstrap accounts from `crates/jstzd/resources/bootstrap_account/accounts.json`

   - activator (1 mutez)
   - injector (100,000,000,000 mutez)
   - **rollup_operator (100,000,000,000 mutez)** ← Bootstrap account, doesn't need manual funding!
   - bootstrap1-5 (each 100,000,000,000 mutez)

2. **Protocol Parameters**: Uses octez sandbox parameters from `crates/octez/resources/protocol_parameters/sandbox/ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK`

   - Not the jstzd/tests params!

3. **NO Boot Sector File**: The rollup node does NOT use `--boot-sector-file`

   - RISC-V rollups get their kernel from origination: `kernel:<path>:<checksum>`
   - Boot sector files are only for legacy WASM rollups

4. **Block Level Timing**: Waits for level 5 after origination (not just level 3 + 10 seconds)

## Usage

Open 4 terminals and run these scripts in order:

### Terminal 1: Start Node

```bash
./scripts/debug/1-start-node.sh
```

Wait for the node to start and show "Node is now running".

### Terminal 2: Setup Protocol

```bash
./scripts/debug/2-setup-protocol.sh
```

This imports all 8 bootstrap accounts and activates the protocol.

### Terminal 3: Start Baker

```bash
./scripts/debug/3-start-baker.sh
```

Wait for the baker to start producing blocks.

### Terminal 4: Originate Rollup

```bash
./scripts/debug/4-originate-rollup.sh
```

This originates the RISC-V rollup. No manual funding needed since rollup_operator is a bootstrap account.

### Terminal 4 (same): Start Rollup Node

```bash
./scripts/debug/5-start-rollup-node.sh
```

Starts the rollup node WITHOUT boot sector file.

## What Each Script Does

### 1-start-node.sh

- Creates temp directories (like jstzd does)
- Generates node identity
- Initializes node config
- Starts octez-node in sandbox mode

### 2-setup-protocol.sh

- Waits for node readiness (30 retries × 1 second)
- Imports ALL 8 bootstrap accounts (matching jstzd)
- Uses octez sandbox protocol parameters
- Adds bootstrap accounts to parameters
- Activates protocol

### 3-start-baker.sh

- Starts baker for injector account
- Uses --without-dal flag (matching jstzd)

### 4-originate-rollup.sh

- Waits for block level 3
- Computes RISC-V kernel checksum
- Originates rollup with `kernel:<path>:<checksum>` format
- NO manual funding (rollup_operator has funds from genesis)
- Waits for block level 5 (2 more blocks)

### 5-start-rollup-node.sh

- Starts rollup node for the originated rollup
- **DOES NOT use --boot-sector-file** (critical!)
- Uses full history mode
- RPC on port 18745

## Debugging Tips

1. **Check Node Health**: `curl http://localhost:18731/health/ready`
2. **Check Block Level**: `curl http://localhost:18731/chains/main/blocks/head/header | grep level`
3. **Check Rollup Status**: `curl http://localhost:18745/global/block/head/status`
4. **View Account Balance**:
   ```bash
   octez-client --base-dir /tmp/jstz-debug-*/octez-client --endpoint http://localhost:18731 \
     get balance for rollup_operator
   ```

## Cleanup

When done, press Ctrl+C in each terminal. The files are in `/tmp/jstz-debug-*` and will be cleaned up automatically.

## Common Issues

### Rollup Node Fails to Start

- **Old Issue**: Used boot sector file for RISC-V rollup ❌
- **Fixed**: No boot sector file for RISC-V rollups ✅

### Rollup Operator Has No Funds

- **Old Issue**: rollup_operator wasn't a bootstrap account ❌
- **Fixed**: rollup_operator is a bootstrap account with 100B mutez ✅

### Wrong Protocol Parameters

- **Old Issue**: Used jstzd/tests params ❌
- **Fixed**: Uses octez sandbox params ✅

## References

- Bootstrap accounts: `crates/jstzd/resources/bootstrap_account/accounts.json`
- Protocol parameters: `crates/octez/resources/protocol_parameters/sandbox/ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK`
- Kernel path: `crates/jstzd/resources/jstz_rollup/lightweight-kernel-executable` (no .elf extension!)
- Kernel checksum: Computed at build time by `crates/jstzd/build.rs`

## Comparison with jstzd

| Aspect                  | jstzd         | These Scripts    |
| ----------------------- | ------------- | ---------------- |
| Bootstrap Accounts      | 8 accounts    | 8 accounts ✅    |
| Protocol Params         | octez sandbox | octez sandbox ✅ |
| Rollup Operator Funding | From genesis  | From genesis ✅  |
| Boot Sector File        | None          | None ✅          |
| Wait for Level          | 3, then 5     | 3, then 5 ✅     |
| Health Checks           | Yes (60×2s)   | Manual ⚠️        |
| jstz_node               | Optional      | Not included ⚠️  |

These scripts are functionally equivalent to `cargo run --bin jstzd -- run` for the core Tezos infrastructure (node, baker, rollup). The jstz_node startup would need to be added separately if needed.
