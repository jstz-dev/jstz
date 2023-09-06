## Suggested spec
This is a suggestion, so feel free to make any decisions or changes as you see fit.

### Cli

The command should either take a script file as an argument or enter interactive mode
options 
`--self-address`
  specifies an address to use as a default for run command
`--addresses`
  specifies a json file to read addresses from with human readable names
`--out`
  specifies an output file, if specified create the messages in json format when the program exits
`--` if specified the remaining inputs will be considered as a single command 
eg. `cli --addresses addresses.json -- run contract alan_address `console.log("Hello Alan")`


### Commands
* `run contract <addr> <code>` takes an address as the first argument, optional if --self-address has been specified second argument is either a code file or some (block) quoted code
* `deploy bridge <addr>` takes an address as the first argument, optional if --self-address has been specified
* `deposit <amount> tez from <addr1> to <addr2>`
* `load addresses <file>` loads adresses from a file as if the `--addresses` flag was set
* `set self address <addr>` sets the self address

in interactive mode only
* `write inputs <filename>` writes the session so far to a json file (filename is optional if output has been set)
* `exit` exits the session

