---
title: Sandbox
---

The Jstz sandbox is a local environment that you can use to develop and test smart functions.
Its core features are:

- Low block time and commitment period
- Pre-loaded bootstrap accounts
- Pre-deployed native bridge
- Centralized configuration between Jstz and Octez infrastructure

:::note

The sandbox runs in a Docker container, so it persists only as long as you leave that container running.

:::

The sandbox includes:

- A simulated Tezos layer 1 environment, including an Octez node and bootstrap accounts that have tez
- The layer 1 infrastructure that hosts the Jstz Smart Rollup
- The [Asset bridge](/architecture/bridge), which allows you to bridge tez from the bootstrap accounts to Jstz smart functions and accounts
- The Jstz environment itself, which runs smart functions and keeps track of Jstz accounts

## Running the sandbox

To start the sandbox, make sure that Docker is installed and then run this command to start the sandbox in a Docker container:

```sh
jstz sandbox --container start
```

If you see an error that says that the configuration file is improperly configured, delete the `~/.config/jstz/` folder and try to start the sandbox again.

:::tip

The `--detach` (`-d`) flag starts the sandbox in the background, allowing you to continue working in the same terminal.
You can stop or reset the sandbox with the commands `jstz sandbox --container stop` or `jstz sandbox --container restart`, but the state of the sandbox is not persistent.

:::

If you see the following output, the sandbox is running:

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

When the sandbox starts, it shows the bootstrap accounts, which are on the sandbox's simulation of Tezos layer 1, not on Jstz.
At this point, no Jstz accounts have any tez.
To fund Jstz smart functions and user accounts, you can bridge tez from these bootstrap accounts.
For example, this command bridges 1,000 tez from a layer 1 bootstrap account to a Jstz account with the address `tz1N8BsvfrSjGdomFi5V9RwwYLasgD8s4pxF`:

```sh
jstz bridge deposit --from bootstrap1 --to tz1N8BsvfrSjGdomFi5V9RwwYLasgD8s4pxF --amount 1000 -n dev
```

Then you can deploy and call smart functions in the sandbox.

## The `jstzd` daemon

The `jstzd` daemon orchestrates and manages the core components of the `jstz` local sandbox.
It serves as the central coordinator for setting up the necessary infrastructure, such as the Jstz node and Octez binaries that run an instance of the Jstz kernel.

:::note

The `jstzd` daemon does not run a complete sandbox by itself.
Running the sandbox without using the `jstz sandbox` command requires you to configure the Octez binaries for use with the daemon and to configure the Jstz CLI to use it.

:::

To start the daemon directly, run this command:

```bash
jstzd run
```

By default, `jstzd` runs on port 54321.
You can customize the configuration through a JSON configuration file.
For example, if you want to start `jstzd` on port 54320 and the Octez node on port 3000, you can create a config.json file with the following content:

```json
{
  "server_port": 54320,
  "octez_node": {
    "rpc_endpoint": "localhost:3000"
  }
}
```

Then start the daemon with this configuration by running this command:

```bash
jstzd run config.json
```

### Bootstrap Accounts

Jstzd automatically sets up 6 bootstrap accounts with an initial balance of 100,000,000,000 mutez each:

- `tz1TGu6TN5GSez2ndXXeDX6LgUDvLzPLqgYV` (bootstrap0)
- `tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx` (bootstrap1)
- `tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN` (bootstrap2)
- `tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU` (bootstrap3)
- `tz1b7tUupMgCNw2cCLpKTkSD1NZzB5TkP2sv` (bootstrap4)
- `tz1ddb9NMYHZi5UzPdzTZMYQQZoMub195zgv` (bootstrap5)

### API Endpoints

The `jstzd` daemon provides endpoints to monitor the health, retrieve the configuration of the running processes, and shut down the system:

- `GET /health` - Returns 200 if the processes are running properly
- `GET /config/` - Get full system configuration
- `GET /config/:config_type` - Get specific component config (including the Jstz node and the Octez node)
- `PUT /shutdown` - Gracefully shut down the system
