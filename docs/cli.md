# 💻 CLI for `jstz`

This guide will instruct through how to use the command line interface for `jstz` where the user can easily run the sandbox environment and test smart functions.

`jstz` offers a number of commands to manage and test your smart functions.

- [sandbox](#sandbox) - Locally deploy a sandbox environment.
- [bridge](#bridge) - Interact with the XTZ asset bridge between Tezos and `jstz`.
- [deploy](#deploy) - Deploy your smart function.
- [run](#run) - Run your smart function.
- [transfer](#transfer) - Transfer XTZ across `jstz` accounts.
- [repl](#repl) - Enter an interactive REPL for the `jstz` runtime.

::: tip

- `jstz help`: Displays a general help message or help information for specific subcommands.

- `jstz --version (-V)`: Displays the current version of `jstz`.

:::

# Config

<!-- In order to run `jstz` cli, you need to create a setup file in `~/.jstz/config.json` that looks as follows: -->

::: danger
⚠️ Under construction ⚠️
:::

<!-- ```json
{
  "jstz_path": "<path_to_jstz>",
  "octez_path": "<path_to_octez>",
  "octez_node_port": <octez_node_port_number>, # typically 18731
  "octez_node_rpc_port": <octez_node_rpc_port_number>, # typically 18730
  "sandbox": null
}
``` -->

<!-- In the file, you should set your path to `jstz` and `octez` and also set the port numbers.
Once the sandbox gets started with the `sandbox start` command, the `"sandbox"` property will contain the information about its run. -->

# Commands

::: tip

Remember, the `-h` or `--help` flag can always be used after any command or subcommand to receive more detailed information about its usage. This guide is a brief overview, and the `help` command will provide the most current and detailed instructions.

:::

## Sandbox

The sandbox commands are responsible for managing the `jstz` sandbox environment.

### Commands:

- `start`: Starts the sandbox environment.

- `stop`: Shuts down the sandbox environment.

- `restart`: Restarts the sandbox.

### Usage:

```bash
jstz sandbox start [OPTIONS]
jstz sandbox restart [OPTIONS]
jstz sandbox stop
```

### Options:

- `--detach (-d)`: Detach the process to run in the background.

### Example:

```bash
$ jstz sandbox start -d

$ jstz sandbox restart -d

$ jstz sandbox stop
```

## Bridge

Bridge commands facilitate the interaction between L1 and L2.

### Commands:

- `deposit`: Deposits CTEZ from an existing Tezos L1 address to a jstz address.

### Usage:

```bash
jstz bridge deposit [OPTIONS]
```

### Options:

- `--from (-f) <ALIAS|ADDRESS>`: Tezos L1 address or alias to withdraw from (must be stored in octez-client's wallet).

- `--to (-t) <ALIAS|ADDRESS>`: jstz address or alias to deposit to.

- `--amount (-a) <INTEGER>`: The amount in CTEZ to transfer.

- `--network (-n) <NETWORK>`: Specifies the network from the config file. Use `dev` for the local sandbox.

### Example:

```bash
$ jstz bridge deposit --from tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU --to tz1iA2Mu65WR3enRHEx9HDfBNRNTecwoz263 --amount 57
```

## Deploy

This command allows users to deploy smart functions.

### Usage:

```bash
jstz deploy [OPTIONS] [CODE|PATH]
```

### Arguments:

- `[CODE|PATH]`: Function code or the file path to the function code.

### Options:

- `--balance (-b) <BALANCE>`: Specifies the initial balance for the function.

- `--name <NAME>`: Name (or alias) of the function.

- `--network (-n) <NETWORK>`: Specifies the network from the config file. Use `dev` for the local sandbox.

### Example

```bash
$ jstz deploy examples/counter.js --name my_counter --balance 42
```

## Run

Execute a smart function using a specified URL.

### Usage:

```bash
jstz run [OPTIONS] <URL>
```

### Arguments:

- `<URL>`: The URL containing the function's address or alias.

### Options:

- `--gas-limit (-g) <GAS_LIMIT>`: The maximum amount of gas to be used. Default is `100000`.

- `--request (-r) <request>`: Specifies the HTTP method used in the request. Default is `GET`.

- `--data (-d) <data>`: Defines the JSON data to be included in the request body.

- `--amount (-a) <data>`: The amount in XTZ to transfer.

- `--network (-n) <NETWORK>`: Specifies the network from the config file. Use `dev` for the local sandbox.

- `--include (-i)`: Include response headers in the output.

- `--trace (-t)`: Flag to show the logs of the function.

### Example

```bash
$ export counter=tz4CYGgcFtphw3AXS2Mx2CMmfj6voV5mPc9b # Address of the previously deployed smart function examples/counter.js
$ jstz run --trace "tezos://${counter}/"
$ jstz run --trace "tezos://${counter}/"
$ jstz run --trace "tezos://${counter}/"
$ jstz run --trace "tezos://${counter}/"
```

You should be able to see an output of the counter smart function looking like this:

```
[🪵] Counter: null
[🪵] Counter: 0
[🪵] Counter: 1
[🪵] Counter: 2
```

## Transfer

Transfer XTZ.

### Usage:

```bash
jstz jstz transfer [OPTIONS] <AMOUNT> <ADDRESS|ALIAS>
```

### Arguments:

- `<AMOUNT>`: The amount in XTZ to transfer.

- `<ADDRESS|ALIAS>`: Destination address or alias of the account (user or smart function).

### Options:

- `--gas-limit (-g) <GAS_LIMIT>`: The maximum amount of gas to be used. Default is `100000`.

- `--network (-n) <NETWORK>`: Specifies the network from the config file. Use `dev` for the local sandbox.

- `--include (-i)`: Include response headers in the output.

### Example

```bash
$ jstz transfer 2 KT1CexuXoXuShARedvKSmxwDTLxRHNcvxqKR
```

## REPL

Starts a REPL environment for experimentation and testing of smart functions.

### Usage:

```bash
jstz repl [OPTIONS]
```

### Options:

- `--account (-a) <ADDRESS|ALIAS>`: Sets the address of the REPL environment.

### Example

```bash
$ jstz repl
$ >> const foo = () => {console.log("hey")};
$ >> foo()
$ [🪵] hey
$ >> exit
```

## Logs

Explore logs from deployed smart functions. The full output of a smart function can also be checked with --trace flag when running it.

### Commands:

- `trace`: Trace the logs from the function that is running.

### Usage:

```bash
jstz logs trace [OPTIONS] <ALIAS|ADDRESS>
```

### Arguments:

- `<ALIAS|ADDRESS>`: The function's address or alias.

### Options:

- `--level (-l) <level>`: Specifies the level of log. Default is `log`.

- `--network (-n) <NETWORK>`: Specifies the network from the config file. Use `dev` for the local sandbox.

### Example

```bash
$ jstz deploy examples/logs.js --name my_function
$ jstz logs trace my_function
```

In a new terminal, run the counter function and you will see the following output:

```bash
[🪵]: log
[🪵]: debug
[🟢]: info
[🟠]: warn
[🔴]: error
[🔴]: Assertion failed
```
