# 👨‍⚖️ jstz

`jstz` (pronouced: "justice") is a JavaScript server runtime that powers Tezos 2.0 that uses [Boa](https://boajs.dev/) and is built in [Rust](https://www.rust-lang.org/). 

## Install

Build from source using Rust:
```sh
make build-deps
make build
```

See [installing Octez](/CONTRIBUTING.md#installing-octez-🐙) for installing the necessary dependencies for running `jstz`.

## Quick Start

```sh
# Make a javascript file
echo "export default (() => new Response('hello world'))" > index.js
# Start the sandbox
make build-installer
cargo run -- sandbox start
# Deploy smart function
cargo run -- deploy index.js --name example
# Send request to smart function
cargo run -- run tezos://example/
```

## Documentation
<!-- TODO: Host documentation using github pages -->
For the latest `jstz` documentation, [click here]().

## Contributing

Please, check the [CONTRIBUTING.md](/CONTRIBUTING.md) file to know how to effectively contribute 
to the project.

## License

This project is licensed under the MIT license.
