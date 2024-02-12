# 💻 CLI for `jstz`

This guide will instruct through how to use the command line interface for `jstz` where the user can easily run the sandbox environment and test smart functions.

`jstz` offers a number of commands to manage and test your smart functions.

- [sandbox](#sandbox) - Locally deploy a sandbox environment.
- [bridge](#bridge) - Interact with the XTZ asset bridge between Tezos and `jstz`.
- [deploy](#deploy) - Deploy your smart function.
- [run](#run) - Run your smart function.
- [repl](#repl) - Enter an interactive REPL for the `jstz` runtime.

::: tip

- `jstz help`: Displays a general help message or help information for specific subcommands.

- `jstz --version (-V)`: Displays the current version of `jstz`.

:::

# Setup config

In order to run `jstz` cli, you need to create a setup file in `~/.jstz/config.json` that looks as follows:

```json
{
  "jstz_path": "<path_to_jstz>",
  "octez_path": "<path_to_octez>",
  "octez_node_port": <octez_node_port_number>, # typically 18731
  "octez_node_rpc_port": <octez_node_rpc_port_number>, # typically 18730
  "sandbox": null
}
```

In the file, you should set your path to `jstz` and `octez` and also set the port numbers.
Once the sandbox gets started with the `sandbox start` command, the `"sandbox"` property will contain the information about its run.

# Commands

## Sandbox

The sandbox commands are responsible for managing the `jstz` sandbox environment.

### Commands:

- `start`: Starts the sandbox environment.

- `stop`: Shuts down the sandbox environment.

### Example:

```bash
# Terminal #1:
$ jstz sandbox start
$
$ Configuring sandbox... done
$ Initializing octez-node configuration... done
$ Generating identity... done
$ Starting node... done
$ Waiting for node to initialize.... done
$ Waiting for node to bootstrap... done
$ Importing activator account... done
$ Activating alpha... done
$ Importing account bootstrap1:unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh
$ Importing account bootstrap2:unencrypted:edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo
$ Importing account bootstrap3:unencrypted:edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ
$ Importing account bootstrap4:unencrypted:edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3
$ Importing account bootstrap5:unencrypted:edsk4QLrcijEffxV31gGdN2HU7UpyJjA8drFoNcmnB28n89YjPNRFm
$ Client initialized
$ Starting baker... done
$ Deploying bridge...
$         `jstz_bridge` deployed at KT1Qc6k3U3EYmQBEZfB58zt69wk5PFzy4XXM
$ Creating installer kernel...done
$ `jstz_rollup` originated at sr1UXVmFED596weKjgPCN8GgWhRrfDGMPxH9
$ Starting rollup node... done
$         `jstz_bridge` `rollup` address set to sr1UXVmFED596weKjgPCN8GgWhRrfDGMPxH9
$ Bridge deployed
$ Sandbox started 🎉
$ Saving sandbox config

# Terminal #2:
$ # Work with the sandbox ...
$ jstz sandbox stop
```

## Bridge

Bridge commands facilitate the interaction between L1 and L2.

### Commands:

- `deposit`: Transfer assets between an L1 sandbox address and an L2 sandbox address.

### Usage:

```bash
jstz bridge deposit --from <TZ1_ADDRESS> --to <TZ4_ADDRESS> --amount <AMOUNT>
```

### Options:

- `--from (-f) <TZ1_ADDRESS>`: The L1 sandbox address or alias to withdraw from.

- `--to (-t) <TZ4_ADDRESS>`: The L2 sandbox address or alias to deposit to.

- `--amount (-a) <INTEGER>`: The quantity in ctez to transfer.

### Example:

```bash
$ jstz bridge deposit --from tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU --to tz4N7y3T2e2dfCyHB1Ama68jnt3Fps7Ufu6d --amount 57
```

## Deploy

This command allows users to deploy smart functions.

### Usage:

```bash
jstz deploy --self-address <SELF_ADDRESS> --function-code <FUNCTION_CODE> --balance <BALANCE>
```

### Options:

- `--self-address (-s) <SELF_ADDRESS>`: Address used when deploying the smart function.

- `--function-code (-f) <FUNCTION_CODE>`: The code for the function being deployed.

- `--balance (-b) <BALANCE>`: Specifies the initial balance for the function.

### Example

```bash
$ jstz deploy --self-address tz4CNucLU82UYRcnkGvk1UWmVdVdj8AfDzvU --function-code "$(cat examples/counter.js)" --balance 42
```

## Run

Execute a smart function using a specified URL.

### Usage:

```bash
jstz run [OPTIONS] <URL> <referrer>
```

### Arguments:

- `<URL>`: The URL containing the function's address or alias.
- `<referrer>`: The address of the entity calling the function.

### Options:

- `--request (-r) <request>`: Specifies the HTTP method used in the request. Default is `GET`.

- `--data (-d) <data>`: Defines the JSON data to be included in the request body.

### Example

```bash
$ export counter=tz4CYGgcFtphw3AXS2Mx2CMmfj6voV5mPc9b # Address of the previously deployed smart function examples/counter.js
$ cargo run -- run "tezos://${counter}/"  tz4CNucLU82UYRcnkGvk1UWmVdVdj8AfDzvU
$ cargo run -- run "tezos://${counter}/"  tz4CNucLU82UYRcnkGvk1UWmVdVdj8AfDzvU
$ cargo run -- run "tezos://${counter}/"  tz4CNucLU82UYRcnkGvk1UWmVdVdj8AfDzvU
$ cargo run -- run "tezos://${counter}/"  tz4CNucLU82UYRcnkGvk1UWmVdVdj8AfDzvU
```

In the logs, you should be able to see an output of the counter smart function looking like this:

```
[🪵] Counter: null
[🪵] Counter: 0
[🪵] Counter: 1
[🪵] Counter: 2
```

## REPL

Starts a REPL environment for experimentation and testing of smart functions.

### Usage:

```bash
jstz repl [OPTIONS]
```

### Options:

- `--self-address (-s) <SELF_ADDRESS>`: Address used when deploying the smart function.

### Example

```bash
$ jstz repl
$ Using mock self-address tz4RepLRepLRepLRepLRepLRepLRepN7Cu8j.
$ >> const childFunction1 = SmartFunction.create(`export default (() => { return Response.json({ message: "hello world" }); });`)
$ [📜] Smart Function created: tz4JGZp7XEojgrpnzL8UdTi3Kn4NaPRVQNwS
$ >> const response = child1.then(address => SmartFunction.call(new Request(`tezos://${address}/`)))
$ Evaluating: "export default (() => { return Response.json({ message: \"hello world\" }); });"
$ >> response.then(async response => console.log((await response.json()).message))
$ [🪵] hello world
$ [object Promise]
$ >> exit
```

::: tip

Remember, the `-h` or `--help` flag can always be used after any command or subcommand to receive more detailed information about its usage. This guide is a brief overview, and the `help` command will provide the most current and detailed instructions.

:::

# Logs

In order to see the output logs of the above commands, you should check the logs/kernel.log file.
We provide `scripts/commands/view-console.sh` script through which conveniently shows the tail of most relevant parts of the log file.
Alternatively you can use tools like `tail` or `grep` for lookup in the logs.
