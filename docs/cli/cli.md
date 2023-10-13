# 💻 CLI for `jstz`

This guide will instruct through how to use the command line interface for `jstz`.

## Overview

`jstz` provides a simple command line interface where the user can easily run the sandbox environment and test smart functions.

## Basic Commands

- `jstz <COMMAND>`: The primary way to interact with `jstz`.
- `help`: Displays a general help message or help information for specific subcommands.
- `--version (-V)`: Displays the current version of `jstz`.

## Sandbox

The sandbox commands are responsible for managing the `jstz` sandbox environment.

### Usage:

```bash
jstz sandbox <COMMAND>
```

### Commands:

- `start`: Starts the sandbox environment.

- `stop`: Shuts down the sandbox environment.

## Bridge

Bridge commands facilitate the interaction between L1 and L2.

### Usage:

```bash
jstz bridge <COMMAND>
```

### Commands:

- `deposit`: Transfer assets between an L1 sandbox address and an L2 sandbox address.

### Usage:

```bash
jstz bridge deposit --from <FROM> --to <TO> --amount <AMOUNT>
```

### Options:

- `--from (-f) <FROM>`: The L1 sandbox address or alias to withdraw from.

- `--to (-t) <TO>`: The L2 sandbox address or alias to deposit to.

- `--amount (-a) <AMOUNT>`: The quantity in ctez to transfer.

### Example:

```bash
$ jstz bridge deposit --from tz4TGu1Bv6G5nbbQT6zCeMdWL2wFBJMu2Kv --to tz4N7y3T2e2dfCyHB1Ama68jnt3Fps7Ufu6d --amount 57


```

## Deploy

This command allows users to deploy smart functions.

### Usage:

```bash
jstz deploy --self-address <SELF_ADDRESS> --function-code <FUNCTION_CODE> --balance <BALANCE>
```

### Options:

- `--self-address (-s) <SELF_ADDRESS>`: Address used when deploying the contract.

- `--function-code (-f) <FUNCTION_CODE>`: The code for the function being deployed.

- `--balance (-b) <BALANCE>`: Specifies the initial balance for the function.

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

## REPL

Starts a REPL environment for experimentation and testing of smart functions.

### Usage:

```bash
jstz repl [OPTIONS]
```

### Options:

- `--self-address (-s) <SELF_ADDRESS>`: Address used when deploying the contract.

---

Remember, the `-h` or `--help` flag can always be used after any command or subcommand to receive more detailed information about its usage. This guide is a brief overview, and the `help` command will provide the most current and detailed instructions.
