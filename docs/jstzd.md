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

        0.1.1-alpha.1 https://github.com/jstz-dev/jstz

  +---------------------------------------------------+---------------------+
  | Address                                           | XTZ Balance (mutez) |
  +===================================================+=====================+
  | (bootstrap0) tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV | 100000000000        |
  +---------------------------------------------------+---------------------+
  | (bootstrap1) tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx | 100000000000        |
  +---------------------------------------------------+---------------------+
  | (bootstrap2) tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN | 100000000000        |
  +---------------------------------------------------+---------------------+
  | (bootstrap3) tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU | 100000000000        |
  +---------------------------------------------------+---------------------+
  | (bootstrap4) tz1b7tUupMgCNw2cCLpKTkSD1NZzB5TkP2sv | 100000000000        |
  +---------------------------------------------------+---------------------+
  | (bootstrap5) tz1ddb9NMYHZi5UzPdzTZMYQQZoMub195zgv | 100000000000        |
  +---------------------------------------------------+---------------------+
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

Jstzd automatically sets up 6 bootstrap accounts with an initial balance of 100,000,000,000 mutez each:

- `tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV` (bootstrap0)
- `tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx` (bootstrap1)
- `tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN` (bootstrap2)
- `tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU` (bootstrap3)
- `tz1b7tUupMgCNw2cCLpKTkSD1NZzB5TkP2sv` (bootstrap4)
- `tz1ddb9NMYHZi5UzPdzTZMYQQZoMub195zgv` (bootstrap5)

These accounts are used for initial setup and can be used for funding user accounts.
