## Large Smart Function Deployment

Jstz implements a mechanism to deploy large smart functions that exceed the L1's inbox message size limit (4KB) using a Reveal Data Channel (RDC) approach. The system uses a Merkle tree encoding algorithm to handle data sizes up to approximately 10MB.

### Architecture Overview

```

┌───────────────────────────── rollup machine ─────────────────────────────┐
│                                                                          │
│  ┌─────── jstz proto ──────┐         inbox        ┌──── jstz node -────┐ │
│  │                         │ ←── 2. signed RLP -─ |                    │ │  ←──── Large operation
│  │ 3. authenticate         │                      │ 1.1. encode large  │ │
│  │ 4. request preimages    │                      │      operation into│ │
│  └──────────┬──────────────┘                      │      preimages     │ │
│             │                                     │                    │ │
│  ┌─────── jstz core ───────┐                      │ 1.2. sign RLP &    │ │
│  │                         │                      │      make data     │ │
│  │ 5. Large operation      |                      |      available     | |
|  |    revealed             │                      └─────────  ─────────┘ │
│  └─────────────────────────┘                                 |           │
│             ↑                      ┌ ─ ─ ─ ─ ─ ┐             |           │
│             └─── 4. preimages ────-            ←──────── preimages       │
│                                    │ local fs  │                         │
│                                    │           │                         │
│                                    └ ─ ─ ─ ─ ─ ┘                         |
└──────────────────────────────────────────────────────────────────────────┘
```

### RevealLargePayload Operation

The core of the large payload handling is the `RevealLargePayload` operation which contains:

- `root_hash`: The root hash of the preimage containing the operation data
- `reveal_type`: The type of operation being revealed (currently supports `DeployFunction`)
- `original_op_hash`: The hash of the original operation being revealed (e.g. hash of `DeployFunction` operation)

While the `RevealLargePayload` operation currently supports only one operation type (`DeployFunction` with large code), its design allows for easy extension to support any type of large payload operation.

### Example Flow

The protocol assumes the preimages are made available to the kernel before processing the `RevealLargePayload` operation. Only authorized injectors such as the Jstz node can submit `RevealLargePayload` operations.

The protocol handles large payloads through the following flow:

1. A large operation (e.g. `DeployFunction`) exceeds the 4KB size limit
2. The operation is encoded into preimages and made available to the kernel
3. A `RevealLargePayload` operation is created and signed by an authorized injector
4. The `RevealLargePayload` operation is injected into the rollup
5. Kernel verifies that the operation is signed by an authorized injector
6. Kernel uses the `RevealData` interface to decode the preimages
7. Kernel loads the complete operation data into memory
8. Kernel verifies the revealed operation type matches the expected type
9. Kernel executes the revealed operation (e.g., deploys the smart function)
10. Results are returned to the user

#### Size Limits

- Maximum direct operation size: 3915 bytes
- Maximum reveal size: 10MB (configurable via `MAX_REVEAL_SIZE`)
