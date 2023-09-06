<h1 align="center"> 👨‍⚖️ jstz </h1>
<section align="center" id="shieldio-badges">
</section>

`jstz` is the command line tool for building and interacting with jstz contracts.

## Quick Start
```sh
# Make a jstz contract
echo "export default (() => new Response('hello world'))" > index.js
# Start standbox
jstz sandbox start
# Deploy it
jstz deploy index.js --name example-contract
# Run it
jstz run tezos://example-contract/
```

## Commands

### `jstz sandbox start`

Starts a jstz sandbox, starting an octez-node, rollup node, baker. 
Deploys the jstz rollup kernel and jstz bridge. 

Stores sandbox running config in file.

### `jstz sandbox stop`

Stops the sandbox (if currently running).

Reads config file to determine which processes to stop. 

### `jstz bridge deposit [OPTIONS]`

Deposits from an existing L1 sandbox address to a L2 sandbox address. 

```sh
jstz bridge deposit
  --from alice
  --to tz4...
  --amount 42
```

`--from` [`address:tz1` or `alias`]  
&nbsp;&nbsp;&nbsp;&nbsp;The L1 sandbox address that the `amount` will be withdrawn from.

`--to` [`address:tz4` or `alias`]  
&nbsp;&nbsp;&nbsp;&nbsp;The L2 sandbox address that the `amount` will be deposited to.

`--amount` [`int`]  
&nbsp;&nbsp;&nbsp;&nbsp;The amount (in ctez) that is withdrawn from `from` and deposited to `to`.

### `jstz deploy [script] [OPTIONS]`

Publishes the given script to the local sandbox. 

```sh
jstz deploy hello_world.js
```

`script` [`path`]  
&nbsp;&nbsp;&nbsp;&nbsp;The path to the contract script

`--name` [`string`]  
&nbsp;&nbsp;&nbsp;&nbsp;The alias for the address of the deployed contract

### `jstz run [url] [OPTIONS]`

```sh
jstz run
  -X GET \
  -d '{"id":"alistair"}' \
  tezos://tz4.../accounts/get
```

`url` [`URL`, required to use `tezos` scheme]  
&nbsp;&nbsp;&nbsp;&nbsp;The URL containing the contract's address (or local alias) as the domain. The path is used for the entrypoint. 

`-X` (optional, default `GET`) [one of `GET`, `POST`, `DELETE`, `PUT`, `PATCH`, ...]  
&nbsp;&nbsp;&nbsp;&nbsp;The *HTTP* method used in the request to the contract.

`-d` (optional) [`JSON`]  
&nbsp;&nbsp;&nbsp;&nbsp;The JSON data in the request body 


### `jstz repl [OPTIONS]`

```sh
jstz repl
> console.log("Hello from jstz 👨‍⚖️");
Hello from jstz 👨‍⚖️
> :host.storage.keys("/jstz_kv/tz4..")
/blogs
/blobs
...
> :host.storage.get("/jstz_kv/tz4../blogs/1")
{ "title": "Lorem ipsum ...", ... }
```

`--self-address` (optional) [`address:tz4`]  
&nbsp;&nbsp;&nbsp;&nbsp;Sets the address of the repl environment. If set, permits use of `proto` APIs (e.g. `Contract`, `Kv`, etc). 


## Configuration

`jstz` is configured via a `jstz.json` file. 

By default, `jstz` will expect this file in the `cwd`, but it can be specified using the `--config` flag (on any `jstz` command). 






