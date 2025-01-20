# JSTZD

jstzd is a daemon service that orchestrates and manages the core components of the `jstz` environment. It serves as the central coordinator for running a jstz client node and the octez binaries(node, baker, rollup node).

The core features are:

- Automatic process spawning
- Graceful shutdown
- Health monitoring
- Configuration settings

## Getting Started

Start the daemon:

```bash
jstzd run
```

if you see the following output, jstzd is running successfully

```

           __________
           \  jstz  /
            )______(
            |""""""|_.-._,.---------.,_.-._
            |      | | |               | | ''-.
            |      |_| |_             _| |_..-'
            |______| '-' `'---------'` '-'
            )""""""(
           /________\
           `'------'`
         .------------.
        /______________\

        0.1.0-alpha.0 https://github.com/jstz-dev/jstz

  +--------------------------------------+---------------------+
  | Address                              | XTZ Balance (mutez) |
  +======================================+=====================+
  | tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx | 60000000000         |
  +--------------------------------------+---------------------+
  | tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV | 40000000000         |
  +--------------------------------------+---------------------+
```

By default, jstzd will start on port 55555. You can customize the configuration through a JSON configuration file:

For example to run jstzd on port 66666 and the octez node on port 8888, you can create a config.json file with the following content:

```json
{
  "server_port": 66666,
  "octez_node": {
    "rpc_endpoint": "localhost:8888"
  }
}
```

Then start JSTZD with your config:

```bash
jstzd run config.json
```

## API Endpoints

JSTZD provides endpoints to monitor the health, retrieve the configuration of the running processes, and shutdown the system:

- `GET /health` - Check health status of all components
- `GET /config/` - Get full system configuration
- `GET /config/:config_type` - Get specific component config (e.g., jstz_node, octez_node)
- `PUT /shutdown` - Gracefully shutdown the system

## Bootstrap Accounts

jstzd automatically sets up two bootstrap accounts:

1. Rollup Operator Account (60,000,000,000 mutez)
2. Activator Account (40,000,000,000 mutez)

These accounts are used for initial setup.
