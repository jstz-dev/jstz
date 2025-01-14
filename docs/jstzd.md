# jstzd

jstzd is a daemon service that orchestrates and manages the core components of the `jstz` local sandbox. It serves as the central coordinator for automatically setting up the necessary infrastructure, such as the jstz node and octez binaries, for running an instance of the jstz kernel

The core features are:

- Low block time and commitment period
- Pre-loaded bootstrap accounts
- Pre-deployed native bridge
- Centralized configuration between jstz and octez

## Getting Started

Start the daemon:

```bash
jstzd run
```

If you see the following output, jstzd is running successfully

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

By default, jstzd will start on port 54321. You can customize the configuration through a JSON configuration file:

For example, if you want to start jstzd on port 54320 and octez_node on port 3000, you can create a config.json file with the following content:

```json
{
  "server_port": 54320,
  "octez_node": {
    "rpc_endpoint": "localhost:3000"
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

jstzd automatically sets up two bootstrap accounts with initial balances:

### Rollup Operator Account

- Address: `tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx`
- Balance: 60,000,000,000 mutez

### Activator Account

- Address: `tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV`
- Balance: 40,000,000,000 mutez

These accounts are used for initial setup and can be used for funding user accounts.
