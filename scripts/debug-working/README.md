# RISC-V Rollup Debug Scripts

This directory contains shell scripts to manually reproduce what `cargo run --bin jstzd` does, but split into separate components for easier debugging.

## Prerequisites

- Octez binaries in PATH:
  - `octez-node`
  - `octez-client`
  - `octez-baker-alpha`
  - `octez-smart-rollup-node-alpha`
- The RISC-V kernel at: `crates/jstzd/resources/jstz_rollup/lightweight-kernel-executable`

## Usage

Run each script in a separate terminal window in order:

### Terminal 1: Start Octez Node

```bash
./scripts/debug/1-start-node.sh
```

This will:

- Create temporary directories
- Generate node identity
- Initialize node config
- Start the octez node
- Keep running (showing node logs)

**Wait for**: Node to start accepting connections

### Terminal 2: Setup Protocol

```bash
./scripts/debug/2-setup-protocol.sh
```

This will:

- Wait for node to be ready
- Import bootstrap accounts (activator, injector, rollup_operator)
- Activate the protocol
- Exit when done

**Wait for**: Script to complete successfully

### Terminal 3: Start Baker

```bash
./scripts/debug/3-start-baker.sh
```

This will:

- Start the baker
- Begin producing blocks automatically
- Keep running (showing baker logs)

**Wait for**: A few blocks to be produced (check Terminal 1 logs)

### Terminal 4: Originate Rollup

```bash
./scripts/debug/4-originate-rollup.sh
```

This will:

- Compute the RISC-V kernel checksum
- Get the operator address for the whitelist
- Wait for block level 3
- Originate the RISC-V smart rollup
- Save the rollup address
- Exit when done

**Output**: You'll see the rollup address (sr1...)

### Terminal 5: Start Rollup Node

```bash
./scripts/debug/5-start-rollup-node.sh
```

This will:

- Start the octez-smart-rollup-node
- Connect to the L1 node
- Begin syncing
- Keep running (showing rollup node logs)

**Watch for**:

- Connection messages
- Block processing
- Any timeout errors

## Debugging Tips

### Check Node Health

```bash
curl http://localhost:18731/health/ready
```

### Check Current Block Level

```bash
source /tmp/jstz-debug-env.sh
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
    rpc get /chains/main/blocks/head/header | grep level
```

### List Rollups

```bash
source /tmp/jstz-debug-env.sh
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
    list known smart rollups
```

### Check Rollup Node Health

```bash
curl http://localhost:18745/health/ready
```

### View Rollup Node Status

```bash
curl http://localhost:18745/local/batcher/queue
```

## Environment Variables

All scripts save and load environment variables from `/tmp/jstz-debug-env.sh`:

- `BASE_DIR`: Temporary directory for all data
- `NODE_DIR`: Octez node data directory
- `CLIENT_DIR`: Octez client data directory
- `NODE_RPC`: Node RPC endpoint
- `ROLLUP_ADDR`: Originated rollup address (after script 4)

## Cleanup

To clean up after testing:

```bash
source /tmp/jstz-debug-env.sh
rm -rf "$BASE_DIR"
rm /tmp/jstz-debug-env.sh
```

## Common Issues

### "Kernel not found" error

Make sure the kernel exists at:

```bash
ls -lh crates/jstzd/resources/jstz_rollup/lightweight-kernel-executable
```

### "Connection timeout" in rollup node

- Check that the node is still running (Terminal 1)
- Check the node RPC is accessible: `curl http://localhost:18731/health/ready`
- The rollup node may show some timeout warnings initially - this is often normal

### "Whitelist" JSON error

The operator address must be a valid tz1 address, not an alias. Script 4 handles this automatically.

### Baker not producing blocks

- Make sure the protocol was activated (script 2 completed successfully)
- Check node logs for any errors
- The baker needs the injector account which should have XTZ from the bootstrap

## What This Replicates

These scripts replicate the key steps that `jstzd` does:

1. ✅ Start octez node
2. ✅ Import bootstrap accounts
3. ✅ Activate protocol
4. ✅ Start baker
5. ✅ Originate RISC-V rollup (with proper kernel format)
6. ✅ Start rollup node

The main differences:

- No jstz_node (JavaScript execution layer)
- No oracle node
- Simpler protocol parameters
- Manual control over timing
- Easier to see what's happening at each step
